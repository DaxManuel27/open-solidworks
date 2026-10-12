//! Sparse normal equations for pose solving.
//!
//! Unknowns come in blocks of six per body and each equation touches only a
//! few bodies. Bodies are ordered with reverse Cuthill–McKee so the normal
//! matrix has a narrow profile, then factored with a skyline Cholesky. The
//! same factorization, run without damping, yields a null-space basis for
//! DOF analysis.

// Triangular solves read clearest with explicit indices.
#![allow(clippy::needless_range_loop)]

/// A sparse Jacobian: rows of (column, value).
pub(crate) type Rows = Vec<Vec<(usize, f64)>>;

/// Reverse Cuthill–McKee order of `n` nodes with the given adjacency.
fn rcm(adjacency: &[Vec<usize>]) -> Vec<usize> {
    let n = adjacency.len();
    let mut seen = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let degree = |i: usize| adjacency[i].len();
    while order.len() < n {
        // Start each component at a node of least degree.
        let start = (0..n)
            .filter(|i| !seen[*i])
            .min_by_key(|i| degree(*i))
            .unwrap();
        seen[start] = true;
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(v) = queue.pop_front() {
            order.push(v);
            let mut next: Vec<usize> = adjacency[v].iter().copied().filter(|u| !seen[*u]).collect();
            next.sort_by_key(|u| degree(*u));
            next.dedup();
            for u in next {
                if !seen[u] {
                    seen[u] = true;
                    queue.push_back(u);
                }
            }
        }
    }
    order.reverse();
    order
}

/// Symmetric matrix in skyline (profile) storage, rows in permuted order.
pub(crate) struct Profile {
    n: usize,
    /// Original column -> permuted position.
    perm: Vec<usize>,
    /// First stored column of each permuted row.
    first: Vec<usize>,
    /// Start of each row in `values`.
    start: Vec<usize>,
    values: Vec<f64>,
}

impl Profile {
    /// Structure for `blocks` bodies of six unknowns with the given body
    /// adjacency.
    pub(crate) fn new(adjacency: &[Vec<usize>]) -> Profile {
        let blocks = adjacency.len();
        let order = rcm(adjacency);
        let mut position = vec![0; blocks];
        for (p, b) in order.iter().enumerate() {
            position[*b] = p;
        }
        let n = 6 * blocks;
        let mut perm = vec![0; n];
        for b in 0..blocks {
            for k in 0..6 {
                perm[6 * b + k] = 6 * position[b] + k;
            }
        }
        let mut first = vec![0; n];
        for b in 0..blocks {
            let lowest = adjacency[b]
                .iter()
                .map(|u| position[*u])
                .chain([position[b]])
                .min()
                .unwrap();
            for k in 0..6 {
                let row = 6 * position[b] + k;
                first[row] = 6 * lowest;
            }
        }
        let mut start = Vec::with_capacity(n + 1);
        let mut total = 0;
        for (i, f) in first.iter().enumerate() {
            start.push(total);
            total += i + 1 - f;
        }
        start.push(total);
        Profile {
            n,
            perm,
            first,
            start,
            values: vec![0.0; total],
        }
    }

    fn at(&self, i: usize, j: usize) -> f64 {
        debug_assert!(j <= i);
        if j < self.first[i] {
            0.0
        } else {
            self.values[self.start[i] + j - self.first[i]]
        }
    }

    fn at_mut(&mut self, i: usize, j: usize) -> &mut f64 {
        let k = self.start[i] + j - self.first[i];
        &mut self.values[k]
    }

    /// Fill with JᵀJ (+ λ on the diagonal) and return Jᵀr in original order.
    pub(crate) fn normal(&mut self, rows: &Rows, r: &[f64], lambda: f64) -> Vec<f64> {
        self.values.iter_mut().for_each(|v| *v = 0.0);
        let mut g = vec![0.0; self.n];
        for (row, res) in rows.iter().zip(r) {
            for &(ca, va) in row {
                let pa = self.perm[ca];
                g[ca] += va * res;
                for &(cb, vb) in row {
                    let pb = self.perm[cb];
                    if pb <= pa {
                        *self.at_mut(pa, pb) += va * vb;
                    }
                }
            }
        }
        if lambda != 0.0 {
            for i in 0..self.n {
                *self.at_mut(i, i) += lambda;
            }
        }
        g
    }

    /// Cholesky in place. With `semidefinite`, pivots that vanish relative
    /// to their diagonal become zero columns instead of failing; their
    /// positions are returned.
    pub(crate) fn factor(&mut self, semidefinite: bool) -> Option<Vec<usize>> {
        let mut zero = Vec::new();
        let diag: Vec<f64> = (0..self.n).map(|i| self.at(i, i)).collect();
        let scale = diag.iter().fold(0.0f64, |a, b| a.max(b.abs())).max(1e-300);
        for i in 0..self.n {
            let fi = self.first[i];
            for j in fi..=i {
                let fj = self.first[j];
                let lo = fi.max(fj);
                let mut s = self.at(i, j);
                for k in lo..j {
                    s -= self.at(i, k) * self.at(j, k);
                }
                if j < i {
                    let d = self.at(j, j);
                    *self.at_mut(i, j) = if d == 0.0 { 0.0 } else { s / d };
                } else if (!semidefinite && s > 0.0 && s.is_finite())
                    || s > 1e-9 * diag[i].abs().max(1e-12 * scale)
                {
                    *self.at_mut(i, i) = s.sqrt();
                } else if semidefinite {
                    *self.at_mut(i, i) = 0.0;
                    zero.push(i);
                } else {
                    return None;
                }
            }
        }
        Some(zero)
    }

    /// Solve L Lᵀ x = g after [`Profile::factor`]; `g` in original order.
    pub(crate) fn solve(&self, g: &[f64]) -> Vec<f64> {
        let n = self.n;
        let mut y = vec![0.0; n];
        for (c, v) in g.iter().enumerate() {
            y[self.perm[c]] = *v;
        }
        for i in 0..n {
            let mut s = y[i];
            for k in self.first[i]..i {
                s -= self.at(i, k) * y[k];
            }
            let d = self.at(i, i);
            y[i] = if d == 0.0 { 0.0 } else { s / d };
        }
        let x = self.back(y);
        (0..n).map(|c| x[self.perm[c]]).collect()
    }

    /// Lᵀ x = y by columns, from the last row up.
    fn back(&self, mut acc_rhs: Vec<f64>) -> Vec<f64> {
        let n = self.n;
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            let d = self.at(i, i);
            x[i] = if d == 0.0 { 0.0 } else { acc_rhs[i] / d };
            for k in self.first[i]..i {
                acc_rhs[k] -= self.at(i, k) * x[i];
            }
        }
        x
    }

    /// A null-space basis after a semidefinite factorization, in original
    /// column order: one vector per zero pivot.
    pub(crate) fn null_space(&self, zero: &[usize]) -> Vec<Vec<f64>> {
        let n = self.n;
        zero.iter()
            .map(|&p| {
                // x_p = 1, other free variables 0; solve the rest of Lᵀ x = 0.
                let mut x = vec![0.0; n];
                let mut acc = vec![0.0; n];
                for i in (0..n).rev() {
                    let d = self.at(i, i);
                    x[i] = if i == p {
                        1.0
                    } else if d == 0.0 {
                        0.0
                    } else {
                        -acc[i] / d
                    };
                    if x[i] != 0.0 {
                        for k in self.first[i]..i {
                            acc[k] += self.at(i, k) * x[i];
                        }
                    }
                }
                (0..n).map(|c| x[self.perm[c]]).collect()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_chain_system_and_finds_its_null_space() {
        // Three bodies in a chain; equations tie neighbours and pin body 0.
        // Body 1 links the others, so the reordering is not the identity.
        let adjacency = vec![vec![1], vec![0, 2], vec![1]];
        let mut rows: Rows = Vec::new();
        for k in 0..6 {
            rows.push(vec![(k, 1.0)]);
            rows.push(vec![(k, 1.0), (6 + k, -1.0)]);
        }
        // Body 2 is tied only in its first three unknowns.
        for k in 0..3 {
            rows.push(vec![(6 + k, 1.0), (12 + k, -1.0)]);
        }
        let r: Vec<f64> = (0..rows.len()).map(|i| i as f64 * 0.1).collect();
        let mut p = Profile::new(&adjacency);
        let g = p.normal(&rows, &r, 1e-3);
        p.factor(false).unwrap();
        let x = p.solve(&g);
        // Check (JᵀJ + λI) x = g densely.
        let n = 18;
        let mut a = vec![vec![0.0; n]; n];
        for row in &rows {
            for (i, vi) in row {
                for (j, vj) in row {
                    a[*i][*j] += vi * vj;
                }
            }
        }
        for (i, ai) in a.iter_mut().enumerate() {
            ai[i] += 1e-3;
        }
        let mut jtr = vec![0.0; n];
        for (row, res) in rows.iter().zip(&r) {
            for (c, v) in row {
                jtr[*c] += v * res;
            }
        }
        assert_eq!(g, jtr);
        for i in 0..n {
            let s: f64 = (0..n).map(|j| a[i][j] * x[j]).sum();
            assert!((s - jtr[i]).abs() < 1e-9, "{i}: {s} vs {}", jtr[i]);
        }

        let mut p = Profile::new(&adjacency);
        p.normal(&rows, &r, 0.0);
        let zero = p.factor(true).unwrap();
        assert_eq!(zero.len(), 3, "body 2's last three unknowns are free");
        for v in p.null_space(&zero) {
            for row in &rows {
                let s: f64 = row.iter().map(|(c, val)| val * v[*c]).sum();
                assert!(s.abs() < 1e-9);
            }
            assert!(v[..12].iter().all(|x| x.abs() < 1e-12));
        }
    }
}

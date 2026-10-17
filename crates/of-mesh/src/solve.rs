//! Sparse symmetric positive definite systems, solved by preconditioned
//! conjugate gradients, and sparse least squares through their normal
//! equations.

use std::collections::BTreeMap;

/// A sparse matrix assembled from (row, column, value) triples; repeated
/// entries add up.
#[derive(Debug, Clone, Default)]
pub struct Sparse {
    pub rows: usize,
    pub cols: usize,
    entries: BTreeMap<(usize, usize), f64>,
}

impl Sparse {
    pub fn new(rows: usize, cols: usize) -> Sparse {
        Sparse {
            rows,
            cols,
            entries: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, r: usize, c: usize, v: f64) {
        if v != 0.0 {
            *self.entries.entry((r, c)).or_insert(0.0) += v;
        }
    }

    pub fn to_csr(&self) -> Csr {
        let mut start = vec![0usize; self.rows + 1];
        let mut cols = Vec::with_capacity(self.entries.len());
        let mut vals = Vec::with_capacity(self.entries.len());
        for (&(r, c), &v) in &self.entries {
            start[r + 1] += 1;
            cols.push(c);
            vals.push(v);
        }
        for r in 0..self.rows {
            start[r + 1] += start[r];
        }
        Csr {
            rows: self.rows,
            cols: self.cols,
            start,
            col: cols,
            val: vals,
        }
    }
}

/// Compressed sparse rows.
#[derive(Debug, Clone)]
pub struct Csr {
    pub rows: usize,
    pub cols: usize,
    start: Vec<usize>,
    col: Vec<usize>,
    val: Vec<f64>,
}

impl Csr {
    pub fn mul(&self, x: &[f64]) -> Vec<f64> {
        (0..self.rows)
            .map(|r| {
                (self.start[r]..self.start[r + 1])
                    .map(|k| self.val[k] * x[self.col[k]])
                    .sum()
            })
            .collect()
    }

    pub fn mul_transpose(&self, y: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; self.cols];
        for (r, yr) in y.iter().enumerate().take(self.rows) {
            for k in self.start[r]..self.start[r + 1] {
                out[self.col[k]] += self.val[k] * yr;
            }
        }
        out
    }

    pub fn diagonal(&self) -> Vec<f64> {
        (0..self.rows)
            .map(|r| {
                (self.start[r]..self.start[r + 1])
                    .find(|k| self.col[*k] == r)
                    .map_or(0.0, |k| self.val[k])
            })
            .collect()
    }

    /// Squared norm of each column, the diagonal of AᵀA.
    pub fn column_norms(&self) -> Vec<f64> {
        let mut out = vec![0.0; self.cols];
        for k in 0..self.val.len() {
            out[self.col[k]] += self.val[k] * self.val[k];
        }
        out
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Solve the SPD system `a x = b` from `x`, to relative residual `tol`.
pub fn cg(a: &Csr, b: &[f64], x: &mut [f64], tol: f64, max_iter: usize) -> usize {
    let diag = a.diagonal();
    let apply = |v: &[f64]| a.mul(v);
    pcg(apply, &diag, b, x, tol, max_iter)
}

/// Least squares `min |a x - b|` by conjugate gradients on the normal
/// equations (CGLS-style, never forming AᵀA).
pub fn least_squares(a: &Csr, b: &[f64], x: &mut [f64], tol: f64, max_iter: usize) -> usize {
    let diag = a.column_norms();
    let rhs = a.mul_transpose(b);
    let apply = |v: &[f64]| a.mul_transpose(&a.mul(v));
    pcg(apply, &diag, &rhs, x, tol, max_iter)
}

fn pcg(
    apply: impl Fn(&[f64]) -> Vec<f64>,
    diag: &[f64],
    b: &[f64],
    x: &mut [f64],
    tol: f64,
    max_iter: usize,
) -> usize {
    let n = b.len();
    let ax = apply(x);
    let mut r: Vec<f64> = (0..n).map(|i| b[i] - ax[i]).collect();
    let pre = |r: &[f64]| -> Vec<f64> {
        r.iter()
            .zip(diag)
            .map(|(v, d)| if d.abs() > 1e-300 { v / d } else { *v })
            .collect()
    };
    let mut z = pre(&r);
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let bnorm = dot(b, b).sqrt().max(1e-300);
    for it in 0..max_iter {
        if dot(&r, &r).sqrt() <= tol * bnorm {
            return it;
        }
        let ap = apply(&p);
        let pap = dot(&p, &ap);
        if pap.abs() < 1e-300 {
            return it;
        }
        let alpha = rz / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        z = pre(&r);
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz;
        rz = rz_new;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
    }
    max_iter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_small_system() {
        let mut a = Sparse::new(3, 3);
        for (r, c, v) in [
            (0, 0, 4.0),
            (0, 1, 1.0),
            (1, 0, 1.0),
            (1, 1, 3.0),
            (2, 2, 2.0),
        ] {
            a.add(r, c, v);
        }
        let a = a.to_csr();
        let mut x = vec![0.0; 3];
        cg(&a, &[1.0, 2.0, 4.0], &mut x, 1e-12, 50);
        assert!((x[0] - 1.0 / 11.0).abs() < 1e-9);
        assert!((x[1] - 7.0 / 11.0).abs() < 1e-9);
        assert!((x[2] - 2.0).abs() < 1e-9);
    }
}

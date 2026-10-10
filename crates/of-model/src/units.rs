//! Display units. The model is always millimetres and radians; units only
//! affect how values are shown and typed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LengthUnit {
    Mm,
    Cm,
    M,
    In,
    Ft,
}

impl LengthUnit {
    /// Millimetres per unit.
    pub fn mm(self) -> f64 {
        match self {
            LengthUnit::Mm => 1.0,
            LengthUnit::Cm => 10.0,
            LengthUnit::M => 1000.0,
            LengthUnit::In => 25.4,
            LengthUnit::Ft => 304.8,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            LengthUnit::Mm => "mm",
            LengthUnit::Cm => "cm",
            LengthUnit::M => "m",
            LengthUnit::In => "in",
            LengthUnit::Ft => "ft",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AngleUnit {
    Deg,
    Rad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MassUnit {
    G,
    Kg,
    Lb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Units {
    pub length: LengthUnit,
    pub angle: AngleUnit,
    pub mass: MassUnit,
    /// Decimal places shown for lengths.
    pub decimals: u8,
}

impl Default for Units {
    /// Millimetre, gram, second.
    fn default() -> Self {
        Units {
            length: LengthUnit::Mm,
            angle: AngleUnit::Deg,
            mass: MassUnit::G,
            decimals: 2,
        }
    }
}

impl Units {
    /// Inch, pound, second.
    pub fn ips() -> Units {
        Units {
            length: LengthUnit::In,
            angle: AngleUnit::Deg,
            mass: MassUnit::Lb,
            decimals: 3,
        }
    }

    pub fn format_length(&self, mm: f64) -> String {
        format!(
            "{:.*} {}",
            self.decimals as usize,
            mm / self.length.mm(),
            self.length.symbol()
        )
    }

    /// Parse a typed length, e.g. "12.5", "1 in", "3/8in", "2cm", "=10*2".
    /// Bare numbers use the document unit.
    pub fn parse_length(&self, text: &str) -> Option<f64> {
        let t = text.trim().to_ascii_lowercase();
        let (num, unit) = split_unit(&t, &["mm", "cm", "m", "in", "\"", "ft", "'"]);
        let factor = match unit {
            "" => self.length.mm(),
            "mm" => 1.0,
            "cm" => 10.0,
            "m" => 1000.0,
            "in" | "\"" => 25.4,
            "ft" | "'" => 304.8,
            _ => return None,
        };
        eval(num).map(|v| v * factor)
    }

    /// Parse a typed angle in degrees (default) or with "deg"/"rad"/"°".
    pub fn parse_angle(&self, text: &str) -> Option<f64> {
        let t = text.trim().to_ascii_lowercase();
        let (num, unit) = split_unit(&t, &["deg", "°", "rad"]);
        let v = eval(num)?;
        Some(match unit {
            "rad" => v,
            "deg" | "°" => v.to_radians(),
            _ => match self.angle {
                AngleUnit::Deg => v.to_radians(),
                AngleUnit::Rad => v,
            },
        })
    }
}

fn split_unit<'a>(t: &'a str, units: &[&'a str]) -> (&'a str, &'a str) {
    for u in units {
        if let Some(n) = t.strip_suffix(u) {
            // "m" must not match the tail of "mm" or "cm".
            if *u == "m" && (n.ends_with('m') || n.ends_with('c')) {
                continue;
            }
            return (n.trim(), u);
        }
    }
    (t, "")
}

/// Evaluate a small arithmetic expression: numbers, + - * /, parentheses.
pub fn eval(text: &str) -> Option<f64> {
    let s: Vec<char> = text
        .trim()
        .trim_start_matches('=')
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut i = 0;
    let v = expr(&s, &mut i)?;
    (i == s.len() && v.is_finite()).then_some(v)
}

fn expr(s: &[char], i: &mut usize) -> Option<f64> {
    let mut v = term(s, i)?;
    while *i < s.len() && (s[*i] == '+' || s[*i] == '-') {
        let op = s[*i];
        *i += 1;
        let r = term(s, i)?;
        v = if op == '+' { v + r } else { v - r };
    }
    Some(v)
}

fn term(s: &[char], i: &mut usize) -> Option<f64> {
    let mut v = factor(s, i)?;
    while *i < s.len() && (s[*i] == '*' || s[*i] == '/') {
        let op = s[*i];
        *i += 1;
        let r = factor(s, i)?;
        v = if op == '*' { v * r } else { v / r };
    }
    Some(v)
}

fn factor(s: &[char], i: &mut usize) -> Option<f64> {
    if *i < s.len() && s[*i] == '-' {
        *i += 1;
        return factor(s, i).map(|v| -v);
    }
    if *i < s.len() && s[*i] == '(' {
        *i += 1;
        let v = expr(s, i)?;
        if *i < s.len() && s[*i] == ')' {
            *i += 1;
            return Some(v);
        }
        return None;
    }
    let start = *i;
    while *i < s.len() && (s[*i].is_ascii_digit() || s[*i] == '.') {
        *i += 1;
    }
    s[start..*i].iter().collect::<String>().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lengths_and_expressions() {
        let mm = Units::default();
        assert_eq!(mm.parse_length("12.5"), Some(12.5));
        assert_eq!(mm.parse_length("1 in"), Some(25.4));
        assert!((mm.parse_length("3/8in").unwrap() - 9.525).abs() < 1e-12);
        assert_eq!(mm.parse_length("2cm"), Some(20.0));
        assert_eq!(mm.parse_length("1m"), Some(1000.0));
        assert_eq!(mm.parse_length("=10*(2+1)"), Some(30.0));
        assert_eq!(mm.parse_length("abc"), None);
        assert_eq!(Units::ips().parse_length("2"), Some(50.8));
        assert!((mm.parse_angle("90").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(mm.format_length(25.4), "25.40 mm");
    }
}

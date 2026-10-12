//! Unit-checked expressions. This evaluator cannot access the filesystem, execute
//! code, or depend on rebuilt geometry. Parameter keys contain stable feature IDs.

use crate::{Document, Feature, FeatureId, FeatureKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub value: f64,
    pub length: i16,
    pub angle: i16,
}

impl Quantity {
    fn scalar(value: f64) -> Self {
        Self {
            value,
            length: 0,
            angle: 0,
        }
    }
    fn checked(self) -> Result<Self, String> {
        if self.value.is_finite() && self.length.abs() <= 32 && self.angle.abs() <= 32 {
            Ok(self)
        } else {
            Err("expression result is not finite or has excessive unit powers".into())
        }
    }
    fn same_units(self, other: Self) -> bool {
        self.length == other.length && self.angle == other.angle
    }
    fn add(self, other: Self, subtract: bool) -> Result<Self, String> {
        if !self.same_units(other) {
            return Err("incompatible units in addition/subtraction".into());
        }
        Self {
            value: self.value + if subtract { -other.value } else { other.value },
            ..self
        }
        .checked()
    }
    fn multiply(self, other: Self, divide: bool) -> Result<Self, String> {
        if divide && other.value == 0.0 {
            return Err("division by zero".into());
        }
        let sign = if divide { -1 } else { 1 };
        Self {
            value: if divide {
                self.value / other.value
            } else {
                self.value * other.value
            },
            length: self.length + sign * other.length,
            angle: self.angle + sign * other.angle,
        }
        .checked()
    }
}

fn constant(name: &str) -> Option<Quantity> {
    let (value, length, angle) = match name {
        "mm" => (1.0, 1, 0),
        "cm" => (10.0, 1, 0),
        "m" => (1000.0, 1, 0),
        "in" => (25.4, 1, 0),
        "ft" => (304.8, 1, 0),
        "rad" => (1.0, 0, 1),
        "deg" => (std::f64::consts::PI / 180.0, 0, 1),
        "pi" => (std::f64::consts::PI, 0, 0),
        _ => return None,
    };
    Some(Quantity {
        value,
        length,
        angle,
    })
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Symbol(char),
    End,
}

fn tokens(text: &str) -> Result<Vec<Token>, String> {
    if text.len() > 4096 {
        return Err("expression exceeds 4096 characters".into());
    }
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b.is_ascii_digit() || b == b'.' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i < bytes.len() && matches!(bytes[i], b'e' | b'E') {
                i += 1;
                if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let number: f64 = text[start..i].parse().map_err(|_| "invalid number")?;
            if !number.is_finite() {
                return Err("number must be finite".into());
            }
            out.push(Token::Number(number));
        } else if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'.'))
            {
                i += 1;
            }
            out.push(Token::Name(text[start..i].into()));
        } else if b"+-*/(),".contains(&b) {
            out.push(Token::Symbol(b as char));
            i += 1;
        } else {
            return Err(format!("unexpected character at byte {i}"));
        }
    }
    out.push(Token::End);
    Ok(out)
}

struct Parser<'a, F> {
    tokens: &'a [Token],
    at: usize,
    depth: usize,
    lookup: F,
    /// Validates grammar, function names and arity without values or units.
    syntax_only: bool,
}
impl<F: FnMut(&str) -> Result<Quantity, String>> Parser<'_, F> {
    fn combine(
        &self,
        a: Quantity,
        b: Quantity,
        op: impl FnOnce(Quantity, Quantity) -> Result<Quantity, String>,
    ) -> Result<Quantity, String> {
        if self.syntax_only {
            Ok(Quantity::scalar(0.0))
        } else {
            op(a, b)
        }
    }
    fn take(&mut self, symbol: char) -> bool {
        if self.tokens[self.at] == Token::Symbol(symbol) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expression(&mut self) -> Result<Quantity, String> {
        let mut q = self.product()?;
        loop {
            if self.take('+') {
                let b = self.product()?;
                q = self.combine(q, b, |a, b| a.add(b, false))?;
            } else if self.take('-') {
                let b = self.product()?;
                q = self.combine(q, b, |a, b| a.add(b, true))?;
            } else {
                break;
            }
        }
        Ok(q)
    }
    fn product(&mut self) -> Result<Quantity, String> {
        let mut q = self.atom()?;
        loop {
            if self.take('*') {
                let b = self.atom()?;
                q = self.combine(q, b, |a, b| a.multiply(b, false))?;
            } else if self.take('/') {
                let b = self.atom()?;
                q = self.combine(q, b, |a, b| a.multiply(b, true))?;
            } else {
                break;
            }
        }
        Ok(q)
    }
    fn atom(&mut self) -> Result<Quantity, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("expression nesting exceeds 64 levels".into());
        }
        let result = if self.take('-') {
            let q = self.atom()?;
            Ok(Quantity {
                value: -q.value,
                ..q
            })
        } else if self.take('+') {
            self.atom()
        } else if self.take('(') {
            let q = self.expression()?;
            if !self.take(')') {
                return Err("expected ')'".into());
            }
            Ok(q)
        } else {
            match self.tokens[self.at].clone() {
                Token::Number(n) => {
                    self.at += 1;
                    let mut q = Quantity::scalar(n);
                    if let Token::Name(unit) = &self.tokens[self.at]
                        && let Some(u) = constant(unit).filter(|u| u.length != 0 || u.angle != 0)
                    {
                        q = self.combine(q, u, |a, b| a.multiply(b, false))?;
                        self.at += 1;
                    }
                    Ok(q)
                }
                Token::Name(name) => {
                    self.at += 1;
                    if self.take('(') {
                        let first = self.expression()?;
                        let second = if self.take(',') {
                            Some(self.expression()?)
                        } else {
                            None
                        };
                        if !self.take(')') {
                            return Err("expected ')' after function arguments".into());
                        }
                        check_arity(&name, second.is_some())?;
                        if self.syntax_only {
                            Ok(Quantity::scalar(0.0))
                        } else {
                            function(&name, first, second)
                        }
                    } else if self.syntax_only {
                        Ok(Quantity::scalar(0.0))
                    } else {
                        constant(&name)
                            .map(Ok)
                            .unwrap_or_else(|| (self.lookup)(&name))
                    }
                }
                _ => Err("expected a number, variable or parenthesized expression".into()),
            }
        };
        self.depth -= 1;
        result?.checked()
    }
}

fn check_arity(name: &str, two: bool) -> Result<(), String> {
    match name {
        "min" | "max" if !two => Err(format!("{name} requires two arguments")),
        "min" | "max" => Ok(()),
        "abs" | "sqrt" | "sin" | "cos" | "tan" if two => {
            Err(format!("{name} requires one argument"))
        }
        "abs" | "sqrt" | "sin" | "cos" | "tan" => Ok(()),
        _ => Err(format!("unknown function {name}")),
    }
}

fn function(name: &str, a: Quantity, b: Option<Quantity>) -> Result<Quantity, String> {
    if matches!(name, "min" | "max") {
        let b = b.ok_or("min/max require two arguments")?;
        if !a.same_units(b) {
            return Err("min/max require matching units".into());
        }
        return Ok(Quantity {
            value: if name == "min" {
                a.value.min(b.value)
            } else {
                a.value.max(b.value)
            },
            ..a
        });
    }
    if b.is_some() {
        return Err(format!("{name} requires one argument"));
    }
    match name {
        "abs" => Ok(Quantity {
            value: a.value.abs(),
            ..a
        }),
        "sqrt" if a.value >= 0.0 && a.length % 2 == 0 && a.angle % 2 == 0 => Ok(Quantity {
            value: a.value.sqrt(),
            length: a.length / 2,
            angle: a.angle / 2,
        }),
        "sqrt" => Err("sqrt requires a nonnegative value and even unit powers".into()),
        "sin" | "cos" | "tan" if a.length == 0 && a.angle == 1 => {
            if name == "tan" && a.value.cos().abs() < 1e-14 {
                return Err("tan is undefined at this angle".into());
            }
            Ok(Quantity::scalar(match name {
                "sin" => a.value.sin(),
                "cos" => a.value.cos(),
                _ => a.value.tan(),
            }))
        }
        "sin" | "cos" | "tan" => {
            Err("trigonometric functions require an angle (deg or rad)".into())
        }
        _ => Err(format!("unknown function {name}")),
    }
}

fn evaluate(
    text: &str,
    lookup: impl FnMut(&str) -> Result<Quantity, String>,
) -> Result<Quantity, String> {
    run(text, lookup, false)
}

/// Rejects malformed formulas, unknown functions and wrong argument counts without
/// evaluating names, so missing variables and cycles stay per-field diagnostics.
pub fn check_syntax(text: &str) -> Result<(), String> {
    run(text, |_| Ok(Quantity::scalar(0.0)), true).map(|_| ())
}

/// Checks submitted formulas, naming the first variable or parameter that fails.
pub fn check_formulas<'a>(
    formulas: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Result<(), String> {
    for (name, text) in formulas {
        check_syntax(text).map_err(|error| format!("{name}: {error}"))?;
    }
    Ok(())
}

fn run(
    text: &str,
    lookup: impl FnMut(&str) -> Result<Quantity, String>,
    syntax_only: bool,
) -> Result<Quantity, String> {
    let tokens = tokens(text.trim().strip_prefix('=').unwrap_or(text.trim()))?;
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        depth: 0,
        lookup,
        syntax_only,
    };
    let value = parser.expression()?;
    if tokens[parser.at] != Token::End {
        return Err("unexpected input after expression".into());
    }
    value.checked()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub key: String,
    pub feature: FeatureId,
    pub path: String,
    pub value: Quantity,
    pub integer: bool,
}

fn param(feature: FeatureId, path: String, value: f64, angle: bool, integer: bool) -> Parameter {
    Parameter {
        key: format!("f{feature}.{path}"),
        feature,
        path,
        value: Quantity {
            value,
            length: i16::from(!angle && !integer),
            angle: i16::from(angle),
        },
        integer,
    }
}

pub fn parameters(feature: &Feature) -> Vec<Parameter> {
    if let FeatureKind::Sketch3d(sketch) = &feature.kind {
        return sketch
            .constraints
            .iter()
            .filter_map(|constraint| {
                if let of_sketch::spatial::Relation3::Distance { value, .. } = constraint.relation {
                    Some(param(
                        feature.id,
                        format!("dimension.{}", constraint.id),
                        value,
                        false,
                        false,
                    ))
                } else {
                    None
                }
            })
            .collect();
    }
    if let FeatureKind::Sketch(s) = &feature.kind {
        return s
            .sketch
            .constraints
            .iter()
            .filter(|(_, c)| c.driving)
            .filter_map(|(id, c)| {
                c.kind.value().map(|v| {
                    param(
                        feature.id,
                        format!("dimension.{id}"),
                        v,
                        matches!(c.kind, of_sketch::ConstraintKind::Angle { .. }),
                        false,
                    )
                })
            })
            .collect();
    }
    fn walk(fid: FeatureId, v: &Value, prefix: &str, out: &mut Vec<Parameter>) {
        if let Value::Object(map) = v {
            // Reference data and imported file content are never parameter targets.
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "reference" | "target" | "axis" | "plane" | "refs" | "data"
                ) {
                    continue;
                }
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if let Some(n) = value.as_f64() {
                    let angle = matches!(
                        key.as_str(),
                        "angle"
                            | "angle1"
                            | "angle2"
                            | "start_angle"
                            | "csink_angle_override"
                            | "twist"
                            | "draft"
                    );
                    let integer = key == "count";
                    let scalar = matches!(
                        key.as_str(),
                        "turns" | "position" | "turn" | "tangent_length"
                    );
                    if angle
                        || integer
                        || scalar
                        || matches!(
                            key.as_str(),
                            "x" | "y"
                                | "z"
                                | "depth"
                                | "distance"
                                | "offset"
                                | "start_offset"
                                | "thickness"
                                | "thickness2"
                                | "radius"
                                | "d1"
                                | "d2"
                                | "spacing"
                                | "pitch"
                                | "height"
                                | "diameter"
                                | "circle_diameter"
                                | "length"
                                | "diameter_override"
                                | "cbore_diameter_override"
                                | "cbore_depth_override"
                        )
                    {
                        let mut parameter = param(fid, path, n, angle, integer);
                        if scalar {
                            parameter.value.length = 0;
                        }
                        out.push(parameter);
                    }
                } else if let Value::Array(values) = value {
                    if matches!(
                        key.as_str(),
                        "translation" | "rotation" | "origin" | "direction"
                    ) {
                        for (i, value) in values.iter().enumerate() {
                            if let Some(n) = value.as_f64() {
                                let mut parameter =
                                    param(fid, format!("{path}.{i}"), n, key == "rotation", false);
                                if key == "direction" {
                                    parameter.value.length = 0;
                                }
                                out.push(parameter);
                            }
                        }
                    } else if key == "stations" || key == "variable_pitch" {
                        // Station ordering is authored, so each scalar retains its
                        // position in the variable-fillet definition.
                        for (i, value) in values.iter().enumerate() {
                            walk(fid, value, &format!("{path}.{i}"), out);
                        }
                    }
                } else {
                    walk(fid, value, &path, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(
        feature.id,
        &serde_json::to_value(&feature.kind).unwrap(),
        "",
        &mut out,
    );
    out
}

pub fn set_parameter(
    feature: &mut Feature,
    parameter: &Parameter,
    value: Quantity,
) -> Result<(), String> {
    if !value.same_units(parameter.value) {
        return Err(format!("{} has incompatible units", parameter.key));
    }
    value.checked()?;
    if parameter.integer
        && (value.value < 1.0 || value.value.fract() != 0.0 || value.value > u32::MAX as f64)
    {
        return Err(format!(
            "{} requires a positive whole number",
            parameter.key
        ));
    }
    if let Some(id) = parameter.path.strip_prefix("dimension.") {
        if let FeatureKind::Sketch3d(sketch) = &mut feature.kind {
            let id = id
                .parse::<of_sketch::Id>()
                .map_err(|_| "invalid constraint ID")?;
            let constraint = sketch
                .constraints
                .iter_mut()
                .find(|constraint| constraint.id == id)
                .ok_or("parameter no longer exists")?;
            if let of_sketch::spatial::Relation3::Distance {
                value: distance, ..
            } = &mut constraint.relation
            {
                if value.value <= 0.0 {
                    return Err("spatial distance must be positive".into());
                }
                *distance = value.value;
                return Ok(());
            }
        }
        if let FeatureKind::Sketch(s) = &mut feature.kind {
            return s
                .sketch
                .set_value(
                    id.parse().map_err(|_| "invalid constraint ID")?,
                    value.value,
                )
                .map_err(|e| e.to_string());
        }
    } else {
        let mut json = serde_json::to_value(&feature.kind).map_err(|e| e.to_string())?;
        let pointer = format!("/{}", parameter.path.replace('.', "/"));
        let slot = json
            .pointer_mut(&pointer)
            .ok_or("parameter no longer exists")?;
        *slot = if parameter.integer {
            Value::from(value.value as u32)
        } else {
            Value::from(value.value)
        };
        feature.kind = serde_json::from_value(json).map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("parameter no longer exists".into())
}

pub fn valid_variable(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && constant(name).is_none()
        && !matches!(name, "abs" | "sqrt" | "min" | "max" | "sin" | "cos" | "tan")
}

pub struct Evaluation {
    pub document: Document,
    pub errors: BTreeMap<FeatureId, String>,
    pub values: BTreeMap<String, Result<Quantity, String>>,
}

struct Evaluator<'a> {
    doc: &'a Document,
    params: BTreeMap<String, Parameter>,
    cache: BTreeMap<String, Result<Quantity, String>>,
    stack: Vec<String>,
}
impl Evaluator<'_> {
    fn get(&mut self, name: &str) -> Result<Quantity, String> {
        if let Some(value) = self.cache.get(name) {
            return value.clone();
        }
        if self.stack.iter().any(|s| s == name) {
            return Err(format!(
                "equation cycle: {} → {name}",
                self.stack.join(" → ")
            ));
        }
        if self.stack.len() >= 128 {
            return Err("equation dependency depth exceeds 128".into());
        }
        self.stack.push(name.into());
        if self.doc.equations.contains_key(name) && !self.params.contains_key(name) {
            self.stack.pop();
            return Err(format!("parameter {name} no longer exists"));
        }
        let expression = self
            .doc
            .variables
            .get(name)
            .or_else(|| self.doc.equations.get(name))
            .cloned();
        let value = if let Some(expression) = expression {
            evaluate(&expression, |key| self.get(key))
        } else {
            self.params
                .get(name)
                .map(|p| p.value)
                .ok_or_else(|| format!("unknown variable or parameter {name}"))
        };
        let value = value.and_then(|v| {
            if let Some(p) = self.params.get(name) {
                if !v.same_units(p.value) {
                    return Err(format!(
                        "{name}: incompatible units (use explicit mm or rad/deg)"
                    ));
                }
                if p.integer
                    && (v.value < 1.0 || v.value.fract() != 0.0 || v.value > u32::MAX as f64)
                {
                    return Err(format!("{name} requires a positive whole number"));
                }
            }
            Ok(v)
        });
        self.stack.pop();
        self.cache.insert(name.into(), value.clone());
        value
    }
}

/// Evaluate `text` with the document's variables and parameters, as the active
/// configuration resolves them.
pub fn evaluate_in(doc: &Document, text: &str) -> Result<Quantity, String> {
    check_syntax(text)?;
    let resolved = crate::configurations::resolve(doc)?;
    let params: BTreeMap<_, _> = resolved
        .features
        .iter()
        .flat_map(parameters)
        .map(|p| (p.key.clone(), p))
        .collect();
    let mut evaluator = Evaluator {
        doc: &resolved,
        params,
        cache: BTreeMap::new(),
        stack: Vec::new(),
    };
    evaluate(text, |name| evaluator.get(name))
}

pub fn effective(doc: &Document) -> Evaluation {
    let resolved = match crate::configurations::resolve(doc) {
        Ok(document) => document,
        Err(error) => {
            return Evaluation {
                document: doc.clone(),
                errors: doc.features.iter().map(|f| (f.id, error.clone())).collect(),
                values: BTreeMap::from([("configuration".into(), Err(error))]),
            };
        }
    };
    let doc = &resolved;
    let params: BTreeMap<_, _> = doc
        .features
        .iter()
        .flat_map(parameters)
        .map(|p| (p.key.clone(), p))
        .collect();
    let mut evaluator = Evaluator {
        doc,
        params,
        cache: BTreeMap::new(),
        stack: Vec::new(),
    };
    let mut document = doc.clone();
    let mut errors = BTreeMap::new();
    for name in doc.variables.keys() {
        let _ = evaluator.get(name);
    }
    for name in doc.equations.keys() {
        let result = evaluator.get(name);
        if let Some(param) = evaluator.params.get(name) {
            let result = result.and_then(|v| {
                set_parameter(document.feature_mut(param.feature).unwrap(), param, v)
            });
            if let Err(error) = result {
                errors.insert(param.feature, error);
            }
        } else {
            let error = format!("parameter {name} no longer exists");
            evaluator.cache.insert(name.clone(), Err(error.clone()));
            if let Some(id) = name
                .strip_prefix('f')
                .and_then(|s| s.split('.').next())
                .and_then(|s| s.parse().ok())
            {
                errors.insert(id, error);
            }
        }
    }
    // A numeric field needs its effective value just as a driven field does.
    // Resolve every registered parameter after applying configuration overrides.
    for name in evaluator.params.keys().cloned().collect::<Vec<_>>() {
        let _ = evaluator.get(&name);
    }
    Evaluation {
        document,
        errors,
        values: evaluator.cache,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn eval(text: &str) -> Result<Quantity, String> {
        evaluate(text, |n| Err(format!("unknown {n}")))
    }
    #[test]
    fn unit_arithmetic_and_domains() {
        assert!((eval("1in + 2mm").unwrap().value - 27.4).abs() < 1e-12);
        assert!((eval("sin(90deg) * sqrt(4mm * 4mm)").unwrap().value - 4.0).abs() < 1e-12);
        for text in [
            "1mm + 2deg",
            "1/0",
            "sqrt(-1)",
            "sqrt(1mm)",
            "sin(2mm)",
            "tan(90deg)",
            "1e999",
            "2;process()",
            "min(1mm, 2rad)",
        ] {
            assert!(eval(text).is_err(), "{text}");
        }
        assert!(eval(&format!("{}1{}", "(".repeat(100), ")".repeat(100))).is_err());
    }
    #[test]
    fn syntax_check_ignores_names_and_units() {
        for text in [
            "f2.end1.depth * 2 + 1mm",
            "=min(a, 3deg)",
            "sqrt(1mm) + missing",
        ] {
            assert!(check_syntax(text).is_ok(), "{text}");
        }
        for text in [
            "2 *",
            "(1",
            "1 2",
            "foo(1)",
            "min(1)",
            "abs(1, 2)",
            "1 $ 2",
            "",
        ] {
            assert!(check_syntax(text).is_err(), "{text}");
        }
    }
}

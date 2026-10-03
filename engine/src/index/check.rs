//! `seldon index --check`: validates an index against
//! `schema/index.schema.json` (with `event.schema.json` and
//! `case.schema.json`), compiled into the binary.
//!
//! The `jsonschema` crate is a test-only dependency (AGENTS.md §7), so the
//! engine carries the small Draft 2020-12 subset the contract uses — the
//! same subset as the `builtin` backend of `scripts/validate-fixtures.py`.
//! A keyword it does not implement is reported as an error, so a schema
//! change that needs more fails loudly instead of passing silently. The
//! tests hold it to the `jsonschema` crate on the sample and on the
//! must-fail fixtures.
//!
//! Only schema rules live here. A case id that appears twice (WP-057) is
//! the user's to fix, not the engine's: `commands::index` checks it on its
//! own (exit 1 under `--check`).

use std::cell::RefCell;
use std::collections::HashMap;

use chrono::{DateTime, NaiveDate};
use regex::Regex;
use serde_json::Value;

const ID_BASE: &str = "https://github.com/JohnAndrewsX/jax-seldon/schema/";

/// The contract schemas: (file name, text).
pub const SCHEMAS: [(&str, &str); 3] = [
    (
        "index.schema.json",
        include_str!("../../../schema/index.schema.json"),
    ),
    (
        "event.schema.json",
        include_str!("../../../schema/event.schema.json"),
    ),
    (
        "case.schema.json",
        include_str!("../../../schema/case.schema.json"),
    ),
];

const ANNOTATIONS: [&str; 8] = [
    "$schema",
    "$id",
    "$defs",
    "title",
    "description",
    "default",
    "$comment",
    "examples",
];

/// A validator over the compiled-in schemas.
pub struct Validator {
    docs: HashMap<String, Value>,
    patterns: RefCell<HashMap<String, Option<Regex>>>,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    pub fn new() -> Self {
        let docs = SCHEMAS
            .iter()
            .map(|(name, text)| {
                let doc: Value = serde_json::from_str(text).expect("a compiled-in schema is JSON");
                let id = doc["$id"]
                    .as_str()
                    .map_or_else(|| format!("{ID_BASE}{name}"), String::from);
                (id, doc)
            })
            .collect();
        Validator {
            docs,
            patterns: RefCell::default(),
        }
    }

    /// Errors of `instance` against `schema/<name>` (`index.schema.json`,
    /// `event.schema.json`, `case.schema.json`), one line each.
    pub fn validate(&self, instance: &Value, name: &str) -> Vec<String> {
        let id = format!("{ID_BASE}{name}");
        let mut errs = Vec::new();
        match self.docs.get(&id) {
            Some(schema) => self.walk(instance, schema, &id, "", &mut errs),
            None => errs.push(format!("no schema {name}")),
        }
        errs
    }

    fn resolve(&self, base: &str, reference: &str) -> Result<(&Value, String), String> {
        let (doc_part, frag) = reference.split_once('#').unwrap_or((reference, ""));
        let doc_id = if doc_part.is_empty() {
            base.to_string()
        } else if doc_part.contains("://") {
            doc_part.to_string()
        } else {
            let dir = base.rsplit_once('/').map_or("", |(d, _)| d);
            format!("{dir}/{doc_part}")
        };
        let mut node = self
            .docs
            .get(&doc_id)
            .ok_or_else(|| format!("unresolvable $ref {reference} (from {base})"))?;
        for part in frag.split('/').filter(|p| !p.is_empty()) {
            let part = part.replace("~1", "/").replace("~0", "~");
            node = match node {
                Value::Array(a) => part.parse::<usize>().ok().and_then(|i| a.get(i)),
                Value::Object(o) => o.get(&part),
                _ => None,
            }
            .ok_or_else(|| format!("unresolvable $ref {reference} (from {base})"))?;
        }
        Ok((node, doc_id))
    }

    fn pattern(&self, p: &str) -> Option<Regex> {
        self.patterns
            .borrow_mut()
            .entry(p.to_string())
            .or_insert_with(|| Regex::new(p).ok())
            .clone()
    }

    fn walk(&self, x: &Value, s: &Value, base: &str, path: &str, errs: &mut Vec<String>) {
        let schema = match s {
            Value::Bool(true) => return,
            Value::Bool(false) => {
                errs.push(format!("{}: not allowed", at(path)));
                return;
            }
            Value::Object(o) => o,
            _ => {
                errs.push(format!("{}: schema is not an object", at(path)));
                return;
            }
        };
        let w = at(path);
        for (k, v) in schema {
            if ANNOTATIONS.contains(&k.as_str()) || k == "then" || k == "else" {
                continue;
            }
            match k.as_str() {
                "$ref" => match v.as_str().map(|r| self.resolve(base, r)) {
                    Some(Ok((node, doc))) => self.walk(x, node, &doc, path, errs),
                    Some(Err(e)) => errs.push(e),
                    None => errs.push(format!("{w}: $ref is not a string")),
                },
                "type" => {
                    let types: Vec<&str> = match v {
                        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                        Value::String(t) => vec![t.as_str()],
                        _ => Vec::new(),
                    };
                    if !types.iter().any(|t| type_ok(t, x)) {
                        errs.push(format!("{w}: expected type {v}, got {}", type_name(x)));
                    }
                }
                "enum" => {
                    if !v.as_array().is_some_and(|a| a.contains(x)) {
                        errs.push(format!("{w}: {} not in enum", short(x)));
                    }
                }
                "const" => {
                    if x != v {
                        errs.push(format!("{w}: expected const {v}"));
                    }
                }
                "required" => {
                    if let (Some(o), Some(req)) = (x.as_object(), v.as_array()) {
                        for r in req.iter().filter_map(Value::as_str) {
                            if !o.contains_key(r) {
                                errs.push(format!("{w}: missing required '{r}'"));
                            }
                        }
                    }
                }
                "properties" => {
                    if let (Some(o), Some(props)) = (x.as_object(), v.as_object()) {
                        for (pk, ps) in props {
                            if let Some(pv) = o.get(pk) {
                                self.walk(pv, ps, base, &format!("{path}/{pk}"), errs);
                            }
                        }
                    }
                }
                "additionalProperties" => {
                    if let Some(o) = x.as_object() {
                        let known = schema.get("properties").and_then(Value::as_object);
                        for (pk, pv) in o {
                            if known.is_some_and(|k| k.contains_key(pk)) {
                                continue;
                            }
                            if *v == Value::Bool(false) {
                                errs.push(format!("{w}: unexpected property '{pk}'"));
                            } else {
                                self.walk(pv, v, base, &format!("{path}/{pk}"), errs);
                            }
                        }
                    }
                }
                "propertyNames" => {
                    if let Some(o) = x.as_object() {
                        for pk in o.keys() {
                            let name = Value::String(pk.clone());
                            self.walk(&name, v, base, &format!("{path}/{pk}(name)"), errs);
                        }
                    }
                }
                "items" => {
                    if let Some(a) = x.as_array() {
                        for (i, item) in a.iter().enumerate() {
                            self.walk(item, v, base, &format!("{path}/{i}"), errs);
                        }
                    }
                }
                "maxItems" | "minItems" => {
                    if let (Some(a), Some(n)) = (x.as_array(), v.as_u64()) {
                        let len = a.len() as u64;
                        if k == "maxItems" && len > n {
                            errs.push(format!("{w}: more than {n} items"));
                        } else if k == "minItems" && len < n {
                            errs.push(format!("{w}: fewer than {n} items"));
                        }
                    }
                }
                "maxLength" | "minLength" => {
                    if let (Some(t), Some(n)) = (x.as_str(), v.as_u64()) {
                        let len = t.chars().count() as u64;
                        if k == "maxLength" && len > n {
                            errs.push(format!("{w}: longer than {n}"));
                        } else if k == "minLength" && len < n {
                            errs.push(format!("{w}: shorter than {n}"));
                        }
                    }
                }
                "minimum" => {
                    if let (Some(n), Some(min)) = (number(x), v.as_f64())
                        && n < min
                    {
                        errs.push(format!("{w}: below minimum {v}"));
                    }
                }
                "pattern" => {
                    if let Some(t) = x.as_str() {
                        match v.as_str().and_then(|p| self.pattern(p)) {
                            Some(re) if re.is_match(t) => {}
                            Some(_) => errs.push(format!("{w}: {} does not match {v}", short(x))),
                            None => errs.push(format!("{w}: unsupported pattern {v}")),
                        }
                    }
                }
                "format" => {
                    if let (Some(t), Some(f)) = (x.as_str(), v.as_str())
                        && !format_ok(f, t)
                    {
                        errs.push(format!("{w}: {} is not a valid {f}", short(x)));
                    }
                }
                "allOf" => {
                    for sub in v.as_array().into_iter().flatten() {
                        self.walk(x, sub, base, path, errs);
                    }
                }
                "anyOf" | "oneOf" => {
                    let ok = v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|sub| {
                            let mut e = Vec::new();
                            self.walk(x, sub, base, path, &mut e);
                            e.is_empty()
                        })
                        .count();
                    if (k == "anyOf" && ok == 0) || (k == "oneOf" && ok != 1) {
                        errs.push(format!("{w}: does not match {k}"));
                    }
                }
                "not" => {
                    let mut e = Vec::new();
                    self.walk(x, v, base, path, &mut e);
                    if e.is_empty() {
                        errs.push(format!("{w}: matches a 'not' schema"));
                    }
                }
                "if" => {
                    let mut e = Vec::new();
                    self.walk(x, v, base, path, &mut e);
                    let branch = if e.is_empty() {
                        schema.get("then")
                    } else {
                        schema.get("else")
                    };
                    if let Some(b) = branch {
                        self.walk(x, b, base, path, errs);
                    }
                }
                other => errs.push(format!(
                    "{w}: schema keyword '{other}' is not implemented by the engine's checker (src/index/check.rs)"
                )),
            }
        }
    }
}

fn at(path: &str) -> &str {
    if path.is_empty() { "/" } else { path }
}

fn short(x: &Value) -> String {
    let s = x.to_string();
    match s.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s,
    }
}

fn number(x: &Value) -> Option<f64> {
    x.as_f64()
}

fn type_ok(t: &str, x: &Value) -> bool {
    match t {
        "integer" => x.is_i64() || x.is_u64() || x.as_f64().is_some_and(|f| f.fract() == 0.0),
        "number" => x.is_number(),
        "string" => x.is_string(),
        "boolean" => x.is_boolean(),
        "null" => x.is_null(),
        "array" => x.is_array(),
        "object" => x.is_object(),
        _ => false,
    }
}

fn type_name(x: &Value) -> &'static str {
    match x {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// `date` (`YYYY-MM-DD`, a real day) and `date-time` (RFC 3339 with an
/// offset); other formats are annotations.
fn format_ok(format: &str, s: &str) -> bool {
    let digits = |r: std::ops::Range<usize>| {
        s.get(r)
            .is_some_and(|p| p.bytes().all(|b| b.is_ascii_digit()))
    };
    match format {
        "date" => {
            s.len() == 10
                && digits(0..4)
                && digits(5..7)
                && digits(8..10)
                && NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
        }
        "date-time" => {
            s.len() >= 20
                && digits(0..4)
                && matches!(s.as_bytes()[10], b'T' | b't')
                && DateTime::parse_from_rfc3339(s).is_ok()
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn formats() {
        assert!(format_ok("date", "2026-10-01"));
        assert!(!format_ok("date", "2026-02-30"));
        assert!(!format_ok("date", "2026-1-01"));
        assert!(format_ok("date-time", "2026-10-01T17:05:12+02:00"));
        assert!(format_ok("date-time", "2026-10-01T17:05:12Z"));
        assert!(!format_ok("date-time", "2026-10-01T17:05:12"));
        assert!(!format_ok("date-time", "2026-10-01 17:05:12+02:00"));
    }

    #[test]
    fn refs_and_keywords() {
        let v = Validator::new();
        let event = json!({"id": "01M3V4RY8GW92AWEZ8KFTHZRAW", "ts": "2026-10-01T09:10:02+02:00",
            "source": "agent", "kind": "command", "subject": "omarchy", "actor": "agent:claude-code"});
        assert_eq!(
            v.validate(&event, "event.schema.json"),
            Vec::<String>::new()
        );
        let mut bad = event.clone();
        bad["actor"] = json!("Robot");
        bad["extra"] = json!(1);
        let errs = v.validate(&bad, "event.schema.json");
        assert_eq!(errs.len(), 2, "{errs:?}");
        let mut linked = event.clone();
        linked["kind"] = json!("resolution");
        linked["refersTo"] = json!("01M3V4RY8GW92AWEZ8KFTHZRAW");
        linked["resolution"] = json!("linked");
        assert!(
            v.validate(&linked, "event.schema.json")
                .iter()
                .any(|e| e.contains("'case'")),
            "if/then of a linked resolution"
        );
    }
}

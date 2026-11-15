//! Reads `--trace` output: `why` prints the slots a type was computed from, and `diff`
//! groups the slots whose types changed between two traces by the slot that changed first.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::Value;

pub struct Trace {
    records: BTreeMap<String, Value>,
}

impl Trace {
    pub fn load(path: &Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::from_reader(io::BufReader::new(file))
    }

    pub fn from_reader(reader: impl BufRead) -> io::Result<Self> {
        let mut records = BTreeMap::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: Value = serde_json::from_str(&line)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if let Some(id) = record.get("id").and_then(Value::as_str) {
                records.insert(id.to_string(), record);
            }
        }
        Ok(Self { records })
    }

    fn get(&self, id: &str) -> Option<&Value> {
        self.records.get(id)
    }

    fn deps(&self, id: &str) -> Vec<&str> {
        self.get(id)
            .and_then(|record| record.get("deps"))
            .and_then(Value::as_array)
            .map(|deps| deps.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }

    /// Ids matching `query` exactly, or else ids that end with it (`#name`, `@ivar`).
    pub fn find(&self, query: &str) -> Vec<&str> {
        if let Some((id, _)) = self.records.get_key_value(query) {
            return vec![id.as_str()];
        }
        self.records
            .keys()
            .filter(|id| id.ends_with(query))
            .map(String::as_str)
            .collect()
    }
}

fn str_field<'a>(record: &'a Value, key: &str) -> Option<&'a str> {
    record.get(key).and_then(Value::as_str)
}

fn location(record: &Value) -> Option<String> {
    let file = str_field(record, "file")?;
    Some(match record.get("line").and_then(Value::as_u64) {
        Some(line) => format!("{file}:{line}"),
        None => file.to_string(),
    })
}

fn evidence(record: &Value, key: &str) -> Vec<(Option<String>, String)> {
    record
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let mut text = str_field(item, "type").unwrap_or_default().to_string();
                    if let Some(method) = str_field(item, "in") {
                        text.push_str(&format!(" in {method}"));
                    }
                    (location(item), text)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Source lines quoted next to evidence, read once per file.
#[derive(Default)]
struct SourceCache {
    files: BTreeMap<String, Option<Vec<String>>>,
}

impl SourceCache {
    fn line(&mut self, location: &str) -> Option<String> {
        let (file, line) = location.rsplit_once(':')?;
        let line: usize = line.parse().ok()?;
        let lines = self.files.entry(file.to_string()).or_insert_with(|| {
            std::fs::read_to_string(file)
                .ok()
                .map(|source| source.lines().map(str::to_string).collect())
        });
        lines
            .as_ref()?
            .get(line.checked_sub(1)?)
            .map(|text| text.trim().to_string())
    }
}

/// Prints `id` and every slot it depends on as a tree. `max_depth` only limits the output.
pub fn why<W: Write>(
    trace: &Trace,
    id: &str,
    max_depth: Option<usize>,
    writer: &mut W,
) -> io::Result<()> {
    let mut printer = WhyPrinter {
        trace,
        max_depth,
        printed: BTreeSet::new(),
        sources: SourceCache::default(),
    };
    printer.print(id, "", "", 0, writer)
}

struct WhyPrinter<'a> {
    trace: &'a Trace,
    max_depth: Option<usize>,
    printed: BTreeSet<String>,
    sources: SourceCache,
}

impl WhyPrinter<'_> {
    fn print<W: Write>(
        &mut self,
        id: &str,
        head: &str,
        indent: &str,
        depth: usize,
        writer: &mut W,
    ) -> io::Result<()> {
        let Some(record) = self.trace.get(id) else {
            return writeln!(writer, "{head}{id}  (not in trace)");
        };
        let ty = str_field(record, "type").unwrap_or("?");
        let mut header = format!("{head}{id}: {ty}");
        if let Some(origin) = str_field(record, "origin") {
            header.push_str(&format!("  [{origin}]"));
        }
        if str_field(record, "kind") == Some("unresolved") {
            header = format!("{head}{id}  [unresolved]");
        }
        if let Some(location) = location(record) {
            header.push_str(&format!("  {location}"));
        }
        if !self.printed.insert(id.to_string()) {
            return writeln!(writer, "{header}  (see above)");
        }
        writeln!(writer, "{header}")?;
        let detail = format!("{indent}    ");
        if let Some(location) = location(record)
            && let Some(source) = self.sources.line(&location)
        {
            writeln!(writer, "{detail}| {source}")?;
        }
        if let Some(body) = str_field(record, "body")
            && body != ty
        {
            writeln!(writer, "{detail}body: {body}")?;
        }
        for (label, key) in [("caller", "callers"), ("write", "writes")] {
            for (location, text) in evidence(record, key) {
                let location = location.unwrap_or_else(|| "?".to_string());
                let source = self
                    .sources
                    .line(&location)
                    .map(|source| format!("  | {source}"))
                    .unwrap_or_default();
                writeln!(writer, "{detail}{label} {location} {text}{source}")?;
            }
        }
        if self.max_depth.is_some_and(|max| depth >= max) {
            return Ok(());
        }
        let deps = self.trace.deps(id);
        for (index, dep) in deps.iter().enumerate() {
            let last = index + 1 == deps.len();
            let (branch, next) = if last {
                ("└─ ", "   ")
            } else {
                ("├─ ", "│  ")
            };
            self.print(
                dep,
                &format!("{indent}{branch}"),
                &format!("{indent}{next}"),
                depth + 1,
                writer,
            )?;
        }
        Ok(())
    }
}

/// What makes a slot's own inputs different, apart from line shifts.
fn local_changes(base: &Value, head: &Value) -> Vec<String> {
    let mut changes = Vec::new();
    for key in ["origin", "body"] {
        let before = str_field(base, key);
        let after = str_field(head, key);
        if before != after {
            changes.push(format!(
                "{key}: {} -> {}",
                before.unwrap_or("-"),
                after.unwrap_or("-")
            ));
        }
    }
    for (label, key) in [("caller", "callers"), ("write", "writes")] {
        let texts = |record: &Value| -> BTreeMap<String, Option<String>> {
            evidence(record, key)
                .into_iter()
                .map(|(location, text)| (text, location))
                .collect()
        };
        let before = texts(base);
        let after = texts(head);
        for (text, location) in &after {
            if !before.contains_key(text) {
                let location = location.as_deref().unwrap_or("?");
                changes.push(format!("+ {label} {location} {text}"));
            }
        }
        for (text, location) in &before {
            if !after.contains_key(text) {
                let location = location.as_deref().unwrap_or("?");
                changes.push(format!("- {label} {location} {text}"));
            }
        }
    }
    let deps = |record: &Value| -> BTreeSet<String> {
        record
            .get("deps")
            .and_then(Value::as_array)
            .map(|deps| {
                deps.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let (before, after) = (deps(base), deps(head));
    for dep in after.difference(&before) {
        changes.push(format!("+ dep {dep}"));
    }
    for dep in before.difference(&after) {
        changes.push(format!("- dep {dep}"));
    }
    changes
}

struct TraceDiff<'a> {
    base: &'a Trace,
    head: &'a Trace,
    roots: BTreeMap<String, BTreeSet<String>>,
    visiting: BTreeSet<String>,
}

impl TraceDiff<'_> {
    fn type_changed(&self, id: &str) -> bool {
        fn field<'t>(trace: &'t Trace, id: &str) -> Option<(Option<&'t str>, Option<&'t str>)> {
            trace
                .get(id)
                .map(|record| (str_field(record, "kind"), str_field(record, "type")))
        }
        field(self.base, id) != field(self.head, id)
    }

    fn deps(&self, id: &str) -> BTreeSet<String> {
        self.base
            .deps(id)
            .into_iter()
            .chain(self.head.deps(id))
            .map(str::to_string)
            .collect()
    }

    /// The changed slots `id` reaches through changed slots only, that themselves depend on
    /// no changed slot. A changed slot with no changed dependency is its own root.
    fn roots(&mut self, id: &str) -> BTreeSet<String> {
        if let Some(roots) = self.roots.get(id) {
            return roots.clone();
        }
        if !self.visiting.insert(id.to_string()) {
            return BTreeSet::new();
        }
        let mut roots = BTreeSet::new();
        for dep in self.deps(id) {
            if dep != id && self.type_changed(&dep) {
                roots.extend(self.roots(&dep));
            }
        }
        self.visiting.remove(id);
        if roots.is_empty() {
            roots.insert(id.to_string());
        }
        self.roots.insert(id.to_string(), roots.clone());
        roots
    }
}

/// Prints changed slots grouped by root cause and returns how many slots changed.
pub fn diff<W: Write>(base: &Trace, head: &Trace, writer: &mut W) -> io::Result<usize> {
    let mut state = TraceDiff {
        base,
        head,
        roots: BTreeMap::new(),
        visiting: BTreeSet::new(),
    };
    let ids: BTreeSet<&str> = base
        .records
        .keys()
        .chain(head.records.keys())
        .map(String::as_str)
        .filter(|id| state.type_changed(id))
        .collect();
    let mut by_root: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for id in &ids {
        for root in state.roots(id) {
            by_root.entry(root).or_default().push(id);
        }
    }
    writeln!(
        writer,
        "{} changed slots, {} root causes",
        ids.len(),
        by_root.len()
    )?;
    let type_of = |trace: &Trace, id: &str| -> String {
        trace
            .get(id)
            .map(|record| {
                if str_field(record, "kind") == Some("unresolved") {
                    "(unresolved)".to_string()
                } else {
                    str_field(record, "type").unwrap_or("?").to_string()
                }
            })
            .unwrap_or_else(|| "(none)".to_string())
    };
    for (root, affected) in &by_root {
        writeln!(writer)?;
        let record = head.get(root).or_else(|| base.get(root));
        let mut header = format!("root {root}");
        if let Some(record) = record {
            if let Some(origin) = str_field(record, "origin") {
                header.push_str(&format!("  [{origin}]"));
            }
            if let Some(location) = location(record) {
                header.push_str(&format!("  {location}"));
            }
        }
        writeln!(writer, "{header}")?;
        writeln!(
            writer,
            "    type: {} -> {}",
            type_of(base, root),
            type_of(head, root)
        )?;
        if let (Some(before), Some(after)) = (base.get(root), head.get(root)) {
            for change in local_changes(before, after) {
                writeln!(writer, "    {change}")?;
            }
        }
        let others: Vec<&&str> = affected.iter().filter(|id| **id != root).collect();
        if !others.is_empty() {
            writeln!(writer, "    affects {} slots:", others.len())?;
            for id in others {
                writeln!(
                    writer,
                    "      {id}: {} -> {}",
                    type_of(base, id),
                    type_of(head, id)
                )?;
            }
        }
    }
    Ok(ids.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(lines: &[&str]) -> Trace {
        Trace::from_reader(lines.join("\n").as_bytes()).unwrap()
    }

    #[test]
    fn diff_groups_changes_by_first_changed_dependency() {
        let base = trace(&[
            r#"{"kind":"method","id":"A#a","type":"() -> Integer","deps":["B#b"]}"#,
            r#"{"kind":"method","id":"B#b","type":"() -> Integer","body":"Integer","deps":[]}"#,
            r#"{"kind":"method","id":"C#c","type":"() -> String","deps":["B#b"]}"#,
        ]);
        let head = trace(&[
            r#"{"kind":"method","id":"A#a","type":"() -> String","deps":["B#b"]}"#,
            r#"{"kind":"method","id":"B#b","type":"() -> String","body":"String","deps":[]}"#,
            r#"{"kind":"method","id":"C#c","type":"() -> String","deps":["B#b"]}"#,
        ]);
        let mut out = Vec::new();
        let changed = diff(&base, &head, &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert_eq!(changed, 2);
        assert!(out.contains("2 changed slots, 1 root causes"), "{out}");
        assert!(out.contains("root B#b"), "{out}");
        assert!(out.contains("body: Integer -> String"), "{out}");
        assert!(out.contains("A#a: () -> Integer -> () -> String"), "{out}");
    }

    #[test]
    fn diff_survives_dependency_cycles() {
        let base = trace(&[
            r#"{"kind":"method","id":"A#a","type":"() -> Integer","deps":["B#b"]}"#,
            r#"{"kind":"method","id":"B#b","type":"() -> Integer","deps":["A#a"]}"#,
        ]);
        let head = trace(&[
            r#"{"kind":"method","id":"A#a","type":"() -> untyped","deps":["B#b"]}"#,
            r#"{"kind":"method","id":"B#b","type":"() -> untyped","deps":["A#a"]}"#,
        ]);
        let mut out = Vec::new();
        assert_eq!(diff(&base, &head, &mut out).unwrap(), 2);
    }

    #[test]
    fn why_prints_each_slot_once() {
        let trace = trace(&[
            r#"{"kind":"method","id":"A#a","type":"() -> Integer","body":"<B#b> | <C#c>","deps":["B#b","C#c"]}"#,
            r#"{"kind":"method","id":"B#b","type":"() -> Integer","deps":["C#c"]}"#,
            r#"{"kind":"method","id":"C#c","type":"() -> Integer","deps":[]}"#,
        ]);
        let mut out = Vec::new();
        why(&trace, "A#a", None, &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("body: <B#b> | <C#c>"), "{out}");
        assert!(out.contains("(see above)"), "{out}");
    }
}

//! Reads `tyda trace` output: `why` prints the slots a type was computed from.

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

    /// `id` and the slots it depends on, nearest first, at most `max_depth` hops away.
    pub fn closure(&self, id: &str, max_depth: usize) -> Vec<(usize, &Value)> {
        let mut seen = BTreeSet::from([id]);
        let mut queue = std::collections::VecDeque::from([(id, 0)]);
        let mut found = Vec::new();
        while let Some((current, depth)) = queue.pop_front() {
            let Some(record) = self.get(current) else {
                continue;
            };
            found.push((depth, record));
            if depth == max_depth {
                continue;
            }
            for dep in self.deps(current) {
                if seen.insert(dep) {
                    queue.push_back((dep, depth + 1));
                }
            }
        }
        found
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

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(lines: &[&str]) -> Trace {
        Trace::from_reader(lines.join("\n").as_bytes()).unwrap()
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

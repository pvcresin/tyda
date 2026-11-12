//! wasm32-wasip1 playground entry: stdin JSON `{"ruby","rbs"}` → stdout JSON line
//! `{rbs, diagnostics, hovers, code_lens}` (1-based lines, 0-based columns).
//! With `"completion":{"line","column"}` (0-based, UTF-16 column) it prints
//! `{completion}` instead, where `completion` is null when nothing applies.
//! Non-JSON stdin = raw Ruby. stdlib RBS from `/rbs` preopen; else project-only.
//! Build: `./scripts/build-wasm.sh` (`wasm` feature).

use std::io::Read;
use std::path::PathBuf;

use tyda::analysis::{playground_analyze, playground_complete};
use tyda::rbs::stdlib_loader::LazyRbsLoader;

#[derive(serde::Deserialize)]
struct Input {
    #[serde(default)]
    ruby: String,
    #[serde(default)]
    rbs: String,
    #[serde(default)]
    completion: Option<CompletionPosition>,
}

#[derive(serde::Deserialize)]
struct CompletionPosition {
    line: usize,
    column: usize,
}

fn main() {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        eprintln!("tyda-wasm: failed to read stdin");
        return;
    }

    // Accept the JSON protocol {"ruby","rbs"}, or fall back to raw Ruby source.
    let input = serde_json::from_str::<Input>(&raw).unwrap_or(Input {
        ruby: raw,
        rbs: String::new(),
        completion: None,
    });

    let roots: Vec<PathBuf> = ["/rbs/core", "/rbs/stdlib"]
        .iter()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .collect();
    let loader = LazyRbsLoader::from_rbs_roots(roots);

    let json = if let Some(position) = input.completion {
        let completion = playground_complete(
            &input.ruby,
            &input.rbs,
            &loader,
            "stdin.rb",
            position.line,
            position.column,
        );
        serde_json::to_string(&serde_json::json!({ "completion": completion }))
    } else {
        serde_json::to_string(&playground_analyze(
            &input.ruby,
            &input.rbs,
            &loader,
            "stdin.rb",
        ))
    };
    match json {
        // Newline-terminated so line-buffered WASI stdout hosts flush it.
        Ok(json) => println!("{json}"),
        Err(err) => eprintln!("tyda-wasm: failed to serialize result: {err}"),
    }
}

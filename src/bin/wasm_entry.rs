//! wasm32-wasip1 playground entry: stdin JSON `{"ruby","rbs"}` → stdout JSON line
//! `{rbs, diagnostics, hovers, code_lens}` (1-based lines, 0-based columns).
//! Non-JSON stdin = raw Ruby. stdlib RBS from `/rbs` preopen; else project-only.
//! Build: `./scripts/build-wasm.sh` (`wasm` feature).

use std::io::Read;
use std::path::PathBuf;

use tyda::analysis::playground_analyze;
use tyda::rbs::stdlib_loader::LazyRbsLoader;

#[derive(serde::Deserialize)]
struct Input {
    #[serde(default)]
    ruby: String,
    #[serde(default)]
    rbs: String,
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
    });

    let roots: Vec<PathBuf> = ["/rbs/core", "/rbs/stdlib"]
        .iter()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .collect();
    let loader = LazyRbsLoader::from_rbs_roots(roots);

    let result = playground_analyze(&input.ruby, &input.rbs, &loader, "stdin.rb");
    match serde_json::to_string(&result) {
        // Newline-terminated so line-buffered WASI stdout hosts flush it.
        Ok(json) => println!("{json}"),
        Err(err) => eprintln!("tyda-wasm: failed to serialize result: {err}"),
    }
}

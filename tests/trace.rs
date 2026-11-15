use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const USER_RB: &str = r##"class User
  LIMIT = 10
  attr_reader :name

  def initialize(name)
    @name = name
    @age = nil
  end

  def label
    "#{name} (#{age})"
  end

  def age
    @age || default_age
  end

  def default_age
    LIMIT * 2
  end

  def self.build(raw)
    new(raw.to_s)
  end
end
"##;

const APP_RB: &str = r#"class App
  def run
    user = User.build(42)
    greet(user)
  end

  def greet(user)
    user.label
  end
end
"#;

const SIGNED_RB: &str = r#"class Signed
  extend T::Sig

  #: () -> String
  def inline = ""

  sig { returns(Integer) }
  def sorbet = 1
end
"#;

fn tyda(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tyda"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run tyda")
}

fn write_sources(dir: &Path) {
    fs::write(dir.join("user.rb"), USER_RB).unwrap();
    fs::write(dir.join("app.rb"), APP_RB).unwrap();
    fs::write(dir.join("signed.rb"), SIGNED_RB).unwrap();
}

fn read_trace(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("each trace line is JSON"))
        .collect()
}

fn record<'a>(records: &'a [Value], id: &str) -> &'a Value {
    records
        .iter()
        .find(|record| record["id"] == id)
        .unwrap_or_else(|| panic!("no record for {id}"))
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

const FILES: [&str; 3] = ["user.rb", "app.rb", "signed.rb"];

#[test]
fn trace_prints_sources_to_stdout_or_file() {
    let dir = tempfile::tempdir().unwrap();
    write_sources(dir.path());

    let mut args = vec!["trace"];
    args.extend(FILES);
    let printed = tyda(dir.path(), &args);
    assert!(printed.status.success());
    assert_eq!(
        tyda(dir.path(), &args).stdout,
        printed.stdout,
        "the trace must be the same on every run"
    );

    let mut args = vec!["trace", "-o", "trace.jsonl"];
    args.extend(FILES);
    let written = tyda(dir.path(), &args);
    assert!(written.status.success());
    assert!(written.stdout.is_empty());
    assert_eq!(
        fs::read(dir.path().join("trace.jsonl")).unwrap(),
        printed.stdout
    );

    let text = String::from_utf8(printed.stdout).unwrap();
    assert!(
        text.lines().all(|line| line.starts_with("{\"id\":")),
        "{text}"
    );

    let records = read_trace(&dir.path().join("trace.jsonl"));
    let rbs = String::from_utf8(tyda(dir.path(), &FILES).stdout).unwrap();

    let name = record(&records, "User#name");
    assert_eq!(name["type"], "def name: -> String");
    assert!(rbs.contains("  def name: -> String\n"), "{rbs}");
    assert_eq!(name["origin"], "ruby_source");
    assert_eq!(name["file"], "user.rb");
    assert_eq!(name["line"], 3);
    assert_eq!(strings(&name["deps"]), ["User@name"]);

    let ivar = record(&records, "User@name");
    assert_eq!(ivar["kind"], "ivar");
    assert_eq!(ivar["writes"][0]["line"], 6);
    assert_eq!(ivar["writes"][0]["in"], "User#initialize");
    assert_eq!(strings(&ivar["deps"]), ["User#initialize"]);

    let age = record(&records, "User#age");
    assert_eq!(age["body"], "<User#default_age> | <truthy <@age>>");
    assert_eq!(strings(&age["deps"]), ["User#default_age", "User@age"]);

    let greet = record(&records, "App#greet");
    assert_eq!(strings(&greet["deps"]), ["User#label", "User.build"]);
    assert_eq!(greet["callers"][0]["file"], "app.rb");
    assert_eq!(greet["callers"][0]["line"], 4);
    assert_eq!(greet["callers"][0]["in"], "App#run");

    let build = record(&records, "User.build");
    assert_eq!(build["callers"][0]["type"], "(42)");

    assert_eq!(record(&records, "User::LIMIT")["kind"], "constant");
    assert_eq!(record(&records, "Signed#inline")["origin"], "rbs_inline");
    assert_eq!(record(&records, "Signed#sorbet")["origin"], "sorbet_sig");
}

#[test]
fn trace_why_prints_dependencies_with_source_lines() {
    let dir = tempfile::tempdir().unwrap();
    write_sources(dir.path());
    let mut args = vec!["trace", "-o", "trace.jsonl"];
    args.extend(FILES);
    assert!(tyda(dir.path(), &args).status.success());

    let output = tyda(dir.path(), &["why", "User#name", "trace.jsonl"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("User#name: def name: -> String  [ruby_source]  user.rb:3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("write user.rb:6 <param 0> in User#initialize  | @name = name"),
        "{stdout}"
    );
    assert!(stdout.contains("caller user.rb:23"), "{stdout}");
    assert!(stdout.contains("| new(raw.to_s)"), "{stdout}");

    // Without a file, `why` reads the trace from stdin; the id may be a suffix.
    let mut piped = Command::new(env!("CARGO_BIN_EXE_tyda"))
        .current_dir(dir.path())
        .args(["why", "@name"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(
        piped.stdin.as_mut().unwrap(),
        &fs::read(dir.path().join("trace.jsonl")).unwrap(),
    )
    .unwrap();
    let piped = piped.wait_with_output().unwrap();
    let suffix = String::from_utf8(piped.stdout).unwrap();
    assert!(suffix.starts_with("User@name: "), "{suffix}");

    let missing = tyda(dir.path(), &["why", "Nope#nope", "trace.jsonl"]);
    assert_eq!(missing.status.code(), Some(1));
}

#[test]
fn trace_rejects_other_output_modes() {
    let dir = tempfile::tempdir().unwrap();
    write_sources(dir.path());
    let output = tyda(dir.path(), &["trace", "--coverage", "user.rb"]);
    assert_eq!(output.status.code(), Some(2));
}

const SHOP_RB: &str = r#"class Item
  attr_reader :price

  def initialize(price)
    @price = price
  end
end

class Cart
  def initialize
    @item = Item.new(100)
  end

  def total
    @item.price
  end

  def summary
    total
  end
end
"#;

/// Inference already knows `total` when it reads `summary` in the same file, so the body
/// keeps no reference; the trace still links them through the return it read.
#[test]
fn trace_links_methods_resolved_inside_one_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("shop.rb"), SHOP_RB).unwrap();
    assert!(
        tyda(dir.path(), &["trace", "-o", "trace.jsonl", "shop.rb"])
            .status
            .success()
    );
    let records = read_trace(&dir.path().join("trace.jsonl"));
    assert_eq!(
        strings(&record(&records, "Cart#summary")["deps"]),
        ["Cart#total"]
    );
    assert_eq!(
        strings(&record(&records, "Cart#total")["deps"]),
        ["Item#price"]
    );
    assert_eq!(
        strings(&record(&records, "Item@price")["deps"]),
        ["Item#initialize"]
    );
}

#[test]
fn trace_records_top_level_callers() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("greet.rb"),
        "class Greeter\n  def greet(name)\n    \"Hello, #{name}\"\n  end\nend\n\nGreeter.new.greet(\"alice\")\nGreeter.new.greet(:bob)\n",
    )
    .unwrap();
    let output = tyda(dir.path(), &["trace", "greet.rb"]);
    let records: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let greet = record(&records, "Greeter#greet");
    let callers: Vec<(u64, &str)> = greet["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|caller| {
            (
                caller["line"].as_u64().unwrap(),
                caller["type"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(callers, [(7, "(\"alice\")"), (8, "(:bob)")]);
}

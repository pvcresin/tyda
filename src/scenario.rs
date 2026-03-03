//! Parser for scenario-test Markdown files. See [`docs/testing.md`](../docs/testing.md) for the format.
use serde::Deserialize;

use crate::project::{RailsVersion, RubyVersion};

#[derive(Debug, Clone)]
pub struct ScenarioFile {
    pub name: String,
    pub description: String,
    pub cases: Vec<TestCase>,
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub config: ScenarioConfig,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ScenarioConfig {
    pub ruby_version: Option<RubyVersion>,
    pub rails_version: Option<RailsVersion>,
    pub include_synthetic_dsl_methods: bool,
    pub known_issue: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectFileInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub rbs_input: Option<String>,
    pub rbi_input: Option<String>,
    pub project_files: Vec<ProjectFileInput>,
    pub ruby_code: String,
    pub expected_rbs: String,
}

#[derive(Debug)]
enum BlockKind {
    Ruby,
    Sql,
    Rbs,
    Rbi,
    Routes,
    Schema,
    Yaml,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScenarioConfig {
    ruby_version: Option<String>,
    rails_version: Option<String>,
    include_synthetic_dsl_methods: Option<bool>,
    known_issue: Option<bool>,
}

enum Section {
    Update,
    Result,
}

pub fn parse_scenario_file(name: &str, content: &str) -> ScenarioFile {
    let mut file_description = String::new();
    let mut cases: Vec<TestCase> = Vec::new();

    let mut current_case_name: Option<String> = None;
    let mut current_case_config = ScenarioConfig::default();
    let mut current_steps: Vec<Step> = Vec::new();
    let mut current_block: Option<(BlockKind, String)> = None;

    let mut section = Section::Update;
    let mut rbs_inputs: Vec<String> = Vec::new();
    let mut rbi_inputs: Vec<String> = Vec::new();
    let mut project_files: Vec<ProjectFileInput> = Vec::new();
    let mut current_file_path: Option<String> = None;
    let mut current_ruby: Option<String> = None;
    let mut result_rbs: Option<String> = None;
    for line in content.lines() {
        if let Some((ref kind, ref mut buf)) = current_block {
            if line.trim() == "```" {
                let block_content = buf.clone();
                match kind {
                    BlockKind::Ruby => {
                        if let Some(path) = current_file_path.take() {
                            project_files.push(ProjectFileInput {
                                path,
                                content: block_content,
                            });
                        } else if let Some(prev) = current_ruby.take() {
                            project_files.push(ProjectFileInput {
                                path: format!("app/models/_project_{}.rb", project_files.len()),
                                content: prev,
                            });
                            current_ruby = Some(block_content);
                        } else {
                            current_ruby = Some(block_content);
                        }
                    }
                    BlockKind::Sql => {
                        if let Some(path) = current_file_path.take() {
                            project_files.push(ProjectFileInput {
                                path,
                                content: block_content,
                            });
                        }
                    }
                    BlockKind::Rbs => match section {
                        Section::Update => {
                            rbs_inputs.push(block_content);
                        }
                        Section::Result => {
                            result_rbs = Some(block_content);
                        }
                    },
                    BlockKind::Rbi => {
                        rbi_inputs.push(block_content);
                    }
                    BlockKind::Routes => {
                        project_files.push(ProjectFileInput {
                            path: "config/routes.rb".to_string(),
                            content: block_content,
                        });
                    }
                    BlockKind::Schema => {
                        project_files.push(ProjectFileInput {
                            path: "db/schema.rb".to_string(),
                            content: block_content,
                        });
                    }
                    BlockKind::Yaml => {
                        if current_case_name.is_none()
                            || !current_steps.is_empty()
                            || !rbs_inputs.is_empty()
                            || !rbi_inputs.is_empty()
                            || !project_files.is_empty()
                            || current_ruby.is_some()
                            || result_rbs.is_some()
                            || current_case_config != ScenarioConfig::default()
                        {
                            panic!(
                                "Scenario yaml config must appear immediately after a case heading"
                            );
                        }
                        current_case_config = parse_scenario_config(&block_content);
                    }
                }
                current_block = None;
            } else {
                buf.push_str(line);
                buf.push('\n');
            }
            continue;
        }

        let trimmed = line.trim();

        if trimmed.starts_with("# ") && !trimmed.starts_with("## ") && file_description.is_empty() {
            file_description = trimmed[2..].to_string();
            continue;
        }

        if trimmed == "### result" {
            section = Section::Result;
            continue;
        }

        if let Some(path) = trimmed.strip_prefix("### file:") {
            current_file_path = Some(path.trim().to_string());
            continue;
        }

        if matches!(section, Section::Update)
            && let Some(path) = inline_code_path(trimmed)
        {
            current_file_path = Some(path);
            continue;
        }

        if trimmed == "### update" || trimmed.starts_with("### ") {
            flush_step(
                &mut current_steps,
                &mut rbs_inputs,
                &mut rbi_inputs,
                &mut project_files,
                &mut current_ruby,
                &mut result_rbs,
            );
            current_file_path = None;
            section = Section::Update;
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("## ") {
            flush_step(
                &mut current_steps,
                &mut rbs_inputs,
                &mut rbi_inputs,
                &mut project_files,
                &mut current_ruby,
                &mut result_rbs,
            );
            if let Some(case_name) = current_case_name.take()
                && !current_steps.is_empty()
            {
                cases.push(TestCase {
                    name: case_name,
                    config: std::mem::take(&mut current_case_config),
                    steps: std::mem::take(&mut current_steps),
                });
            }
            current_case_name = Some(heading.to_string());
            current_case_config = ScenarioConfig::default();
            current_file_path = None;
            section = Section::Update;
            continue;
        }

        if trimmed == "```ruby" || trimmed == "```rb" {
            current_block = Some((BlockKind::Ruby, String::new()));
        } else if trimmed == "```sql" {
            current_block = Some((BlockKind::Sql, String::new()));
        } else if trimmed == "```rbs" {
            current_block = Some((BlockKind::Rbs, String::new()));
        } else if trimmed == "```rbi" {
            current_block = Some((BlockKind::Rbi, String::new()));
        } else if trimmed == "```routes" {
            current_block = Some((BlockKind::Routes, String::new()));
        } else if trimmed == "```schema" {
            current_block = Some((BlockKind::Schema, String::new()));
        } else if trimmed == "```yaml" {
            current_block = Some((BlockKind::Yaml, String::new()));
        }
    }

    flush_step(
        &mut current_steps,
        &mut rbs_inputs,
        &mut rbi_inputs,
        &mut project_files,
        &mut current_ruby,
        &mut result_rbs,
    );

    if !current_steps.is_empty() {
        let case_name = current_case_name.unwrap_or_else(|| file_description.clone());
        cases.push(TestCase {
            name: case_name,
            config: current_case_config,
            steps: current_steps,
        });
    }

    ScenarioFile {
        name: name.to_string(),
        description: file_description,
        cases,
    }
}

fn parse_scenario_config(content: &str) -> ScenarioConfig {
    let raw: RawScenarioConfig =
        serde_yaml::from_str(content).expect("Scenario yaml config must be valid");
    ScenarioConfig {
        ruby_version: raw.ruby_version.map(|value| {
            RubyVersion::parse(&value)
                .unwrap_or_else(|| panic!("Invalid ruby_version in scenario config: {value}"))
        }),
        rails_version: raw.rails_version.map(|value| {
            RailsVersion::parse(&value)
                .unwrap_or_else(|| panic!("Invalid rails_version in scenario config: {value}"))
        }),
        include_synthetic_dsl_methods: raw.include_synthetic_dsl_methods.unwrap_or(false),
        known_issue: raw.known_issue.unwrap_or(false),
    }
}

fn inline_code_path(line: &str) -> Option<String> {
    if !line.starts_with('`') || !line.ends_with('`') || line.len() < 2 {
        return None;
    }
    let inner = &line[1..line.len() - 1];
    if inner.is_empty() || inner.contains('`') {
        return None;
    }
    Some(inner.to_string())
}

fn flush_step(
    steps: &mut Vec<Step>,
    rbs_inputs: &mut Vec<String>,
    rbi_inputs: &mut Vec<String>,
    project_files: &mut Vec<ProjectFileInput>,
    current_ruby: &mut Option<String>,
    result_rbs: &mut Option<String>,
) {
    let ruby = match current_ruby.take() {
        Some(r) => r,
        None => return,
    };

    let expected = match result_rbs.take() {
        Some(e) => e,
        None => return,
    };

    let rbs_input = if rbs_inputs.is_empty() {
        None
    } else {
        Some(std::mem::take(rbs_inputs).join("\n"))
    };

    let rbi_input = if rbi_inputs.is_empty() {
        None
    } else {
        Some(std::mem::take(rbi_inputs).join("\n"))
    };

    steps.push(Step {
        rbs_input,
        rbi_input,
        project_files: std::mem::take(project_files),
        ruby_code: ruby,
        expected_rbs: expected,
    });

    rbs_inputs.clear();
    rbi_inputs.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_case() {
        let content = r#"# Simple method

## foo

### update

```ruby
def foo
  1
end
```

### result

```rbs
class Object
  def foo: -> Integer
end
```
"#;

        let file = parse_scenario_file("test", content);
        assert_eq!(file.description, "Simple method");
        assert_eq!(file.cases.len(), 1);
        assert_eq!(file.cases[0].name, "foo");
        assert_eq!(file.cases[0].config, ScenarioConfig::default());
        assert!(file.cases[0].steps[0].ruby_code.contains("def foo"));
        assert!(
            file.cases[0].steps[0]
                .expected_rbs
                .contains("def foo: -> Integer")
        );
        assert!(file.cases[0].steps[0].rbs_input.is_none());
    }

    #[test]
    fn test_parse_multiple_cases() {
        let content = r#"# Literal returns

## Integer literal

### update

```ruby
def foo
  1
end
```

### result

```rbs
class Object
  def foo: -> Integer
end
```

## String literal

### update

```ruby
def greet
  "hello"
end
```

### result

```rbs
class Object
  def greet: -> String
end
```

## Symbol literal

### update

```ruby
def status
  :ok
end
```

### result

```rbs
class Object
  def status: -> Symbol
end
```
"#;

        let file = parse_scenario_file("ruby/method/literal_return.md", content);
        assert_eq!(file.description, "Literal returns");
        assert_eq!(file.cases.len(), 3);
        assert_eq!(file.cases[0].name, "Integer literal");
        assert_eq!(file.cases[1].name, "String literal");
        assert_eq!(file.cases[2].name, "Symbol literal");
    }

    #[test]
    fn test_parse_case_level_yaml_config() {
        let content = r#"# Rails version case

## async relation methods

```yaml
rails_version: 7.0.0
```

### update

```ruby
class Post
end
```

### result

```rbs
class Post
end
```
"#;

        let file = parse_scenario_file("rails/active_record/scope.md", content);
        assert_eq!(
            file.cases[0].config.rails_version,
            Some(RailsVersion::new(7, 0, 0))
        );
        assert_eq!(file.cases[0].config.ruby_version, None);
        assert!(!file.cases[0].config.include_synthetic_dsl_methods);
        assert!(!file.cases[0].config.known_issue);
    }

    #[test]
    fn test_parse_case_level_include_synthetic_dsl_methods() {
        let content = r#"# Rails version case

## synthetic methods

```yaml
rails_version: 7.1.0
include_synthetic_dsl_methods: true
```

### update

```ruby
class Post
end
```

### result

```rbs
class Post
end
```
"#;

        let file = parse_scenario_file("rails/active_record/model.md", content);
        assert_eq!(
            file.cases[0].config.rails_version,
            Some(RailsVersion::new(7, 1, 0))
        );
        assert!(file.cases[0].config.include_synthetic_dsl_methods);
        assert!(!file.cases[0].config.known_issue);
    }

    #[test]
    fn test_parse_case_level_known_issue() {
        let content = r#"# Known issue case

## absolute class keeps lexical constants

```yaml
known_issue: true
```

### update

```ruby
module Foo
  CONST = 42
end
```

### result

```rbs
module Foo
  CONST: 42
end
```
"#;

        let file = parse_scenario_file("ruby/class/absolute_path.md", content);
        assert!(file.cases[0].config.known_issue);
    }

    #[test]
    fn test_parse_with_rbs_input() {
        let content = r#"# RBS input

## String#to_i return type

### update

```rbs
class String
  def to_i: -> Integer
end
```

```ruby
def foo(s)
  s.to_i
end
foo("42")
```

### result

```rbs
class Object
  def foo: (String s) -> Integer
end
```
"#;

        let file = parse_scenario_file("test", content);
        assert_eq!(file.cases.len(), 1);
        assert_eq!(file.cases[0].steps.len(), 1);

        let step = &file.cases[0].steps[0];
        assert!(step.rbs_input.is_some());
        assert!(step.rbs_input.as_ref().unwrap().contains("def to_i"));
        assert!(step.ruby_code.contains("s.to_i"));
        assert!(step.expected_rbs.contains("def foo:"));
    }

    #[test]
    fn test_parse_multi_step_case() {
        let content = r#"# Incremental

## Method return type change

### update

```ruby
def foo
  1
end
```

### result

```rbs
class Object
  def foo: -> Integer
end
```

### update

```ruby
def foo
  "hello"
end
```

### result

```rbs
class Object
  def foo: -> String
end
```
"#;

        let file = parse_scenario_file("test", content);
        assert_eq!(file.cases.len(), 1);
        assert_eq!(file.cases[0].name, "Method return type change");
        assert_eq!(file.cases[0].steps.len(), 2);
        assert!(file.cases[0].steps[0].expected_rbs.contains("Integer"));
        assert!(file.cases[0].steps[1].expected_rbs.contains("String"));
    }

    #[test]
    fn test_parse_with_project_blocks() {
        let content = r#"# Rails project

## route fixture

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  resources :users
end
```

```ruby
class User
  def link
    user_path(self)
  end
end
```

### result

```rbs
class User
  def link: -> String
end
```
"#;

        let file = parse_scenario_file("rails/routes/test.md", content);
        let step = &file.cases[0].steps[0];
        assert_eq!(step.project_files.len(), 1);
        assert!(
            step.project_files
                .iter()
                .any(|file| file.path == "config/routes.rb")
        );
        assert!(step.ruby_code.contains("user_path"));
    }

    #[test]
    fn test_parse_inline_schema_fixture() {
        let content = r#"# Rails project

## schema fixture

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "users", force: :cascade do |t|
    t.string "name", null: false
  end
end
```

```ruby
class User
  def label
    name.upcase
  end
end
```

### result

```rbs
class User
  def label: -> String
end
```
"#;

        let file = parse_scenario_file("test.md", content);
        let step = &file.cases[0].steps[0];
        assert!(
            step.project_files
                .iter()
                .any(|file| file.path == "db/schema.rb")
        );
        assert!(step.ruby_code.contains("name.upcase"));
    }

    #[test]
    fn test_parse_explicit_sql_project_file() {
        let content = r#"# SQL fixture

## structure sql

### update

### file: db/structure.sql

```sql
CREATE TABLE public.users (
    id bigint NOT NULL,
    name character varying
);
```

```ruby
class User
  def label
    name&.upcase
  end
end
```

### result

```rbs
class User
  def label: -> String | nil
end
```
"#;

        let file = parse_scenario_file("rails/schema/structure_sql.md", content);
        let step = &file.cases[0].steps[0];
        assert!(
            step.project_files
                .iter()
                .any(|file| file.path == "db/structure.sql")
        );
        assert!(step.ruby_code.contains("name&.upcase"));
    }

    #[test]
    fn test_parse_inline_code_path_project_file() {
        let content = r#"# Inline file fixture

## inflections

### update

`config/initializers/inflections.rb`

```ruby
ActiveSupport::Inflector.inflections(:en) do |inflect|
  inflect.irregular "person", "people"
end
```

```ruby
class Team
  def people
    []
  end
end
```

### result

```rbs
class Team
  def people: -> Array[untyped]
end
```
"#;

        let file = parse_scenario_file("rails/active_record/has_many.md", content);
        let step = &file.cases[0].steps[0];
        assert!(
            step.project_files
                .iter()
                .any(|file| file.path == "config/initializers/inflections.rb")
        );
        assert!(step.ruby_code.contains("class Team"));
    }

    #[test]
    fn test_missing_result_section_produces_no_step() {
        let content = r#"# Test

## No result section

### update

```ruby
def foo
  1
end
```
"#;

        let file = parse_scenario_file("test", content);
        assert_eq!(file.cases.len(), 0);
    }
}

use crate::types::Sym;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ruby_prism::{Node, ParseResult};

use super::inflector::{
    classify_with_project_known_classes, column_type_to_type, resolve_model_class_name,
};
use crate::registry::{MethodDef, ParamInfo, TypeRegistry};
use crate::types::{ParamKind, Type};

#[derive(Clone)]
struct ColumnDef {
    name: String,
    col_type: Type,
    nullable: bool,
}

#[derive(Clone)]
struct AssociationDef {
    name: String,
    target_class: String,
    nullable: bool,
}

struct TableDef {
    table_name: String,
    class_name: String,
    columns: Vec<ColumnDef>,
    associations: Vec<AssociationDef>,
}

pub fn load_schema(root: &Path, registry: &mut TypeRegistry) {
    let preload_timing = std::env::var_os("TYDA_PRELOAD_TIMING").is_some();
    let schema_path = root.join("db").join("schema.rb");
    let structure_path = root.join("db").join("structure.sql");
    let t = std::time::Instant::now();
    let tables = if schema_path.exists() {
        let Ok(source) = std::fs::read_to_string(&schema_path) else {
            return;
        };
        parse_schema(&source, root)
    } else if structure_path.exists() {
        let Ok(source) = std::fs::read_to_string(&structure_path) else {
            return;
        };
        parse_structure_sql(&source, root)
    } else {
        return;
    };
    let parse_tables_ms = t.elapsed().as_secs_f64() * 1000.0;
    // For namespaced models bound to a legacy table name via a `self.table_name` declaration.
    let t = std::time::Instant::now();
    let overrides = collect_table_name_overrides(root);
    let overrides_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = std::time::Instant::now();
    for table in &tables {
        register_table_methods(table, registry);
        if let Some(classes) = overrides.get(&table.table_name) {
            for class in classes {
                if class != &table.class_name {
                    register_table_methods_as(class, table, registry);
                }
            }
        }
    }
    if preload_timing {
        eprintln!(
            "TIMING rails_schema tables={} parse_tables_ms={parse_tables_ms:.3} overrides_ms={overrides_ms:.3} register_ms={:.3}",
            tables.len(),
            t.elapsed().as_secs_f64() * 1000.0,
        );
    }
}

/// Collects `self.table_name =` declarations, since inflection alone can't find namespaced + legacy tables.
fn collect_table_name_overrides(root: &Path) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut files = Vec::new();
    collect_model_files(&root.join("app").join("models"), &mut files);
    for path in files {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !source.contains("table_name") {
            continue;
        }
        let parse_result = ruby_prism::parse(source.as_bytes());
        let mut namespace: Vec<String> = Vec::new();
        collect_table_name_in_node(
            &parse_result.node(),
            &parse_result,
            &mut namespace,
            &mut map,
        );
    }
    map
}

fn collect_model_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_model_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rb") {
            out.push(path);
        }
    }
}

fn collect_table_name_in_node(
    node: &Node<'_>,
    parse_result: &ParseResult<'_>,
    namespace: &mut Vec<String>,
    map: &mut HashMap<String, Vec<String>>,
) {
    match node {
        Node::ProgramNode { .. } => {
            let program = node.as_program_node().expect("must be ProgramNode");
            for stmt in program.statements().body().iter() {
                collect_table_name_in_node(&stmt, parse_result, namespace, map);
            }
        }
        Node::StatementsNode { .. } => {
            let stmts = node.as_statements_node().expect("must be StatementsNode");
            for stmt in stmts.body().iter() {
                collect_table_name_in_node(&stmt, parse_result, namespace, map);
            }
        }
        Node::ModuleNode { .. } => {
            let module_node = node.as_module_node().expect("must be ModuleNode");
            let name = constant_path_text(&module_node.constant_path());
            namespace.push(name);
            if let Some(body) = module_node.body() {
                collect_table_name_in_node(&body, parse_result, namespace, map);
            }
            namespace.pop();
        }
        Node::ClassNode { .. } => {
            let class_node = node.as_class_node().expect("must be ClassNode");
            let name = constant_path_text(&class_node.constant_path());
            namespace.push(name);
            if let Some(body) = class_node.body() {
                collect_table_name_in_node(&body, parse_result, namespace, map);
            }
            namespace.pop();
        }
        Node::CallNode { .. } => {
            let call = node.as_call_node().expect("must be CallNode");
            if call.name().as_slice() != b"table_name=" {
                return;
            }
            if namespace.is_empty() {
                return;
            }
            let Some(args) = call.arguments() else {
                return;
            };
            let Some(first) = args.arguments().iter().next() else {
                return;
            };
            if let Some(table) = extract_string_value(&first, parse_result) {
                let class_name = namespace.join("::");
                let entry = map.entry(table).or_default();
                if !entry.contains(&class_name) {
                    entry.push(class_name);
                }
            }
        }
        _ => {}
    }
}

fn constant_path_text(node: &Node<'_>) -> String {
    let text = match node {
        Node::ConstantReadNode { .. } => {
            let cr = node
                .as_constant_read_node()
                .expect("must be ConstantReadNode");
            String::from_utf8_lossy(cr.name().as_slice()).to_string()
        }
        _ => String::from_utf8_lossy(node.location().as_slice()).to_string(),
    };
    text.trim().trim_start_matches("::").to_string()
}

fn parse_schema(source: &str, workspace_root: &Path) -> Vec<TableDef> {
    let parse_result = ruby_prism::parse(source.as_bytes());
    let root = parse_result.node();
    let mut tables = Vec::new();

    let Node::ProgramNode { .. } = &root else {
        return tables;
    };
    let program = root.as_program_node().expect("root must be ProgramNode");
    for node in program.statements().body().iter() {
        find_create_tables(&node, &parse_result, workspace_root, &mut tables);
    }
    tables
}

fn parse_structure_sql(source: &str, workspace_root: &Path) -> Vec<TableDef> {
    let mut tables = Vec::new();
    let mut current_table_name: Option<String> = None;
    let mut current_columns: Vec<ColumnDef> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if current_table_name.is_none() {
            if let Some(table_name) = parse_structure_table_start(trimmed) {
                current_table_name = Some(table_name);
                current_columns.clear();
            }
            continue;
        }

        // PostgreSQL `);` / MySQL `) ENGINE=...;` — column definition lines never start with `)`.
        if trimmed.starts_with(')') {
            let table_name = current_table_name.take().expect("table name");
            let class_name = resolve_model_class_name(workspace_root, &table_name);
            tables.push(TableDef {
                table_name,
                class_name,
                columns: std::mem::take(&mut current_columns),
                associations: Vec::new(),
            });
            continue;
        }

        if let Some(column) = parse_structure_column_line(trimmed) {
            current_columns.push(column);
        }
    }

    attach_structure_foreign_keys(source, workspace_root, &mut tables);
    tables
}

fn find_create_tables(
    node: &Node<'_>,
    parse_result: &ParseResult<'_>,
    root: &Path,
    tables: &mut Vec<TableDef>,
) {
    if let Node::CallNode { .. } = node {
        let call = node.as_call_node().expect("must be CallNode");
        let name = String::from_utf8_lossy(call.name().as_slice());
        if name == "create_table" {
            if let Some(table) = parse_create_table(&call, parse_result, root) {
                tables.push(table);
            }
            return;
        }
        if let Some(block_raw) = call.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
            && let Node::StatementsNode { .. } = &body
        {
            let stmts = body.as_statements_node().expect("must be StatementsNode");
            for stmt in stmts.body().iter() {
                find_create_tables(&stmt, parse_result, root, tables);
            }
        }
    }
}

fn parse_create_table(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    root: &Path,
) -> Option<TableDef> {
    let args = call.arguments()?;
    let first_arg = args.arguments().iter().next()?;
    let table_name = extract_string_value(&first_arg, parse_result)?;
    let class_name = resolve_model_class_name(root, &table_name);

    let mut columns = vec![
        ColumnDef {
            name: "id".to_string(),
            col_type: Type::Integer,
            nullable: false,
        },
        ColumnDef {
            name: "created_at".to_string(),
            col_type: Type::Class(Sym::new("Time")),
            nullable: false,
        },
        ColumnDef {
            name: "updated_at".to_string(),
            col_type: Type::Class(Sym::new("Time")),
            nullable: false,
        },
    ];

    if let Some(block_raw) = call.block()
        && let Some(block) = block_raw.as_block_node()
        && let Some(body) = block.body()
        && let Node::StatementsNode { .. } = &body
    {
        let stmts = body.as_statements_node().expect("must be StatementsNode");
        for stmt in stmts.body().iter() {
            if let Node::CallNode { .. } = &stmt {
                let col_call = stmt.as_call_node().expect("must be CallNode");
                let method = String::from_utf8_lossy(col_call.name().as_slice()).to_string();
                if let Some(stripped) = method.strip_prefix("t.") {
                    if let Some(parsed) = parse_column_call(stripped, &col_call, parse_result) {
                        columns.extend(parsed);
                    }
                } else if let Some(receiver) = col_call.receiver()
                    && is_t_receiver(&receiver)
                    && let Some(parsed) = parse_column_call(&method, &col_call, parse_result)
                {
                    columns.extend(parsed);
                }
            }
        }
    }

    let associations = collect_schema_associations(root, &class_name, call, parse_result);
    Some(TableDef {
        table_name,
        class_name,
        columns,
        associations,
    })
}

fn parse_structure_table_start(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("create table ") {
        return None;
    }
    let mut rest = line["CREATE TABLE ".len()..].trim();
    if rest.starts_with("ONLY ") {
        rest = rest["ONLY ".len()..].trim();
    }
    if let Some(stripped) = rest.strip_prefix("IF NOT EXISTS ") {
        rest = stripped.trim();
    }
    let table_ref = rest.split('(').next()?.trim();
    extract_structure_table_name(table_ref)
}

fn extract_structure_table_name(table_ref: &str) -> Option<String> {
    let last = table_ref.rsplit('.').next()?.trim();
    let unquoted = unquote_sql_ident(last);
    (!unquoted.is_empty()).then(|| unquoted.to_string())
}

fn unquote_sql_ident(token: &str) -> &str {
    let token = token.trim();
    for (open, close) in [('`', '`'), ('"', '"'), ('[', ']')] {
        if let Some(rest) = token.strip_prefix(open)
            && let Some(inner) = rest.strip_suffix(close)
        {
            return inner;
        }
    }
    token
}

fn parse_structure_column_line(line: &str) -> Option<ColumnDef> {
    if line.is_empty() {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        _ if lower.starts_with("constraint ")
            || lower.starts_with("primary key")
            || lower.starts_with("unique ")
            || lower.starts_with("check ")
            || lower.starts_with("exclude ")
            || lower.starts_with("like ")
            || lower.starts_with("key ")
            || lower.starts_with("key(")
            || lower.starts_with("index ")
            || lower.starts_with("fulltext")
            || lower.starts_with("spatial")
            || lower.starts_with("foreign key")
    ) {
        return None;
    }

    let trimmed = line.trim_end_matches(',');
    let (name, rest) = split_structure_column_definition(trimmed)?;
    let nullable = !rest.to_ascii_lowercase().contains("not null");
    Some(ColumnDef {
        name,
        col_type: structure_sql_type_to_type(rest),
        nullable,
    })
}

fn split_structure_column_definition(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim();
    // Quoted column names (ANSI / MySQL / SQL Server).
    let close = match trimmed.chars().next()? {
        '"' => Some('"'),
        '`' => Some('`'),
        '[' => Some(']'),
        _ => None,
    };
    if let Some(close) = close {
        let rest = &trimmed[1..];
        let end = rest.find(close)?;
        let name = rest[..end].to_string();
        let tail = rest[end + 1..].trim_start();
        return Some((name, tail));
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let name = parts.next()?.to_string();
    let rest = parts.next()?.trim_start();
    Some((name, rest))
}

fn structure_sql_type_to_type(definition: &str) -> Type {
    match structure_sql_canonical_keyword(definition) {
        Some(keyword) => {
            let base = column_type_to_type(keyword);
            if structure_sql_is_array(definition) {
                Type::Array(Some(Box::new(base)))
            } else {
                base
            }
        }
        None => Type::Untyped,
    }
}

/// Unknown types degrade to Untyped (schema / structure / DSL all converge on [`column_type_to_type`]).
fn structure_sql_canonical_keyword(definition: &str) -> Option<&'static str> {
    let lower = definition.to_ascii_lowercase();
    let lower = lower.trim();
    // MySQL `tinyint(1)` is boolean — check it before the generic `tinyint` integer case.
    if lower.starts_with("boolean") || lower.starts_with("bool") || lower.starts_with("tinyint(1)")
    {
        Some("boolean")
    } else if lower.starts_with("bigint")
        || lower.starts_with("integer")
        || lower.starts_with("int")
        || lower.starts_with("smallint")
        || lower.starts_with("tinyint")
        || lower.starts_with("mediumint")
        || lower.starts_with("serial")
        || lower.starts_with("bigserial")
    {
        Some("integer")
    } else if lower.starts_with("numeric") || lower.starts_with("decimal") {
        Some("decimal")
    } else if lower.starts_with("double") || lower.starts_with("real") || lower.starts_with("float")
    {
        Some("float")
    } else if lower.starts_with("character varying")
        || lower.starts_with("varchar")
        || lower.starts_with("char")
        || lower.starts_with("text")
        || lower.starts_with("tinytext")
        || lower.starts_with("mediumtext")
        || lower.starts_with("longtext")
        || lower.starts_with("citext")
        || lower.starts_with("uuid")
        || lower.starts_with("bytea")
        || lower.starts_with("enum")
        || lower.starts_with("set(")
    {
        Some("string")
    } else if lower.starts_with("timestamp") || lower.starts_with("datetime") {
        Some("datetime")
    } else if lower.starts_with("date") {
        Some("date")
    } else if lower.starts_with("time") {
        Some("time")
    } else if lower.starts_with("json") {
        Some("json")
    } else if lower.starts_with("hstore") {
        Some("hstore")
    } else {
        None
    }
}

/// Postgres array column: whether the type part (before any modifiers) ends with `[]`.
fn structure_sql_is_array(definition: &str) -> bool {
    let lower = definition.to_ascii_lowercase();
    let type_part = lower
        .split_once(" default ")
        .map(|(head, _)| head)
        .unwrap_or(&lower);
    let type_part = type_part
        .split_once(" not null")
        .map(|(head, _)| head)
        .unwrap_or(type_part);
    let type_part = type_part
        .split_once(" collate ")
        .map(|(head, _)| head)
        .unwrap_or(type_part);
    type_part.trim().trim_end_matches(',').ends_with("[]")
}

fn is_t_receiver(node: &Node<'_>) -> bool {
    if let Node::CallNode { .. } = node {
        let call = node.as_call_node().expect("must be CallNode");
        let name = String::from_utf8_lossy(call.name().as_slice());
        return name == "t";
    }
    if let Node::LocalVariableReadNode { .. } = node {
        let read = node
            .as_local_variable_read_node()
            .expect("must be LocalVariableReadNode");
        let name = String::from_utf8_lossy(read.name().as_slice());
        return name == "t";
    }
    false
}

fn parse_column_call(
    col_type: &str,
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
) -> Option<Vec<ColumnDef>> {
    let args = call.arguments()?;
    let first_arg = args.arguments().iter().next()?;
    let col_name = extract_string_value(&first_arg, parse_result)?;
    let nullable = column_nullable(call, parse_result);

    match col_type {
        "references" | "belongs_to" => {
            let mut columns = vec![ColumnDef {
                name: format!("{col_name}_id"),
                col_type: Type::Integer,
                nullable,
            }];
            if hash_option_bool(call, "polymorphic", parse_result).unwrap_or(false) {
                columns.push(ColumnDef {
                    name: format!("{col_name}_type"),
                    col_type: Type::String,
                    nullable,
                });
            }
            Some(columns)
        }
        _ => {
            let mut ty = column_type_to_type(col_type);
            if hash_option_bool(call, "array", parse_result).unwrap_or(false) {
                ty = Type::Array(Some(Box::new(ty)));
            }
            Some(vec![ColumnDef {
                name: col_name,
                col_type: ty,
                nullable,
            }])
        }
    }
}

fn extract_string_value(node: &Node<'_>, parse_result: &ParseResult<'_>) -> Option<String> {
    match node {
        Node::StringNode { .. } => {
            let sn = node.as_string_node().expect("must be StringNode");
            Some(String::from_utf8_lossy(sn.unescaped()).to_string())
        }
        Node::SymbolNode { .. } => {
            let sn = node.as_symbol_node().expect("must be SymbolNode");
            Some(String::from_utf8_lossy(sn.unescaped()).to_string())
        }
        Node::InterpolatedStringNode { .. } => {
            let source = parse_result.source();
            let loc = node.location();
            let raw = &source[loc.start_offset()..loc.end_offset()];
            let s = String::from_utf8_lossy(raw).to_string();
            let trimmed = s.trim_matches('"').trim_matches('\'');
            Some(trimmed.to_string())
        }
        _ => None,
    }
}

fn collect_schema_associations(
    root: &Path,
    current_class: &str,
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
) -> Vec<AssociationDef> {
    let mut associations = Vec::new();
    let Some(block_raw) = call.block() else {
        return associations;
    };
    let Some(block) = block_raw.as_block_node() else {
        return associations;
    };
    let Some(body) = block.body() else {
        return associations;
    };
    let Node::StatementsNode { .. } = &body else {
        return associations;
    };

    let statements = body.as_statements_node().expect("must be StatementsNode");
    for stmt in statements.body().iter() {
        let Node::CallNode { .. } = &stmt else {
            continue;
        };
        let col_call = stmt.as_call_node().expect("must be CallNode");
        let method = String::from_utf8_lossy(col_call.name().as_slice()).to_string();
        let assoc = if let Some(stripped) = method.strip_prefix("t.") {
            parse_schema_association_call(root, current_class, stripped, &col_call, parse_result)
        } else if let Some(receiver) = col_call.receiver() {
            if is_t_receiver(&receiver) {
                parse_schema_association_call(root, current_class, &method, &col_call, parse_result)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(assoc) = assoc {
            associations.push(assoc);
        }
    }

    associations
}

fn parse_schema_association_call(
    root: &Path,
    current_class: &str,
    col_type: &str,
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
) -> Option<AssociationDef> {
    if !matches!(col_type, "references" | "belongs_to") {
        return None;
    }
    if hash_option_bool(call, "polymorphic", parse_result).unwrap_or(false) {
        return None;
    }
    let args = call.arguments()?;
    let first_arg = args.arguments().iter().next()?;
    let assoc_name = extract_string_value(&first_arg, parse_result)?;
    Some(AssociationDef {
        name: assoc_name.clone(),
        target_class: infer_association_target_class(root, current_class, &assoc_name),
        nullable: column_nullable(call, parse_result),
    })
}

fn attach_structure_foreign_keys(source: &str, workspace_root: &Path, tables: &mut [TableDef]) {
    let mut current_table: Option<String> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(table_name) = parse_structure_alter_table_start(trimmed) {
            current_table = Some(table_name);
            continue;
        }
        let Some(table_name) = current_table.as_deref() else {
            continue;
        };
        if !trimmed.starts_with("ADD CONSTRAINT ") {
            continue;
        }
        let Some((column_name, referenced_table)) = parse_structure_foreign_key_line(trimmed)
        else {
            continue;
        };
        if let Some(table) = tables
            .iter_mut()
            .find(|table| table.table_name == table_name)
        {
            let Some(base_name) = column_name.strip_suffix("_id") else {
                continue;
            };
            let nullable = table
                .columns
                .iter()
                .find(|column| column.name == column_name)
                .is_none_or(|column| column.nullable);
            table.associations.push(AssociationDef {
                name: base_name.to_string(),
                target_class: resolve_model_class_name(workspace_root, &referenced_table),
                nullable,
            });
        }
        current_table = None;
    }
}

fn parse_structure_alter_table_start(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("alter table ") {
        return None;
    }
    let mut rest = line["ALTER TABLE ".len()..].trim();
    if rest.starts_with("ONLY ") {
        rest = rest["ONLY ".len()..].trim();
    }
    extract_structure_table_name(rest)
}

fn parse_structure_foreign_key_line(line: &str) -> Option<(String, String)> {
    let foreign_key_idx = line.find("FOREIGN KEY (")?;
    let column_rest = &line[foreign_key_idx + "FOREIGN KEY (".len()..];
    let column_name = unquote_sql_ident(column_rest.split(')').next()?.trim());
    let references_idx = line.find("REFERENCES ")?;
    let reference_rest = &line[references_idx + "REFERENCES ".len()..];
    let referenced_table = extract_structure_table_name(reference_rest.split('(').next()?.trim())?;
    Some((column_name.to_string(), referenced_table))
}

fn infer_association_target_class(root: &Path, current_class: &str, assoc_name: &str) -> String {
    let known_classes = vec![current_class.to_string()];
    classify_with_project_known_classes(root, assoc_name, &known_classes)
}

fn register_table_methods(table: &TableDef, registry: &mut TypeRegistry) {
    register_table_methods_as(&table.class_name, table, registry);
}

fn register_table_methods_as(owner: &str, table: &TableDef, registry: &mut TypeRegistry) {
    // The 15 dirty-tracking kinds per column are registered as a pattern; readers/writers/predicates are materialized (keeps RSS as a skeleton).
    let mut dirty_columns: Vec<(Sym, Type)> = Vec::with_capacity(table.columns.len());
    for col in &table.columns {
        let accessor_type = nullable_type(&col.col_type, col.nullable);
        register_attribute_methods(owner, &col.name, accessor_type, registry);
        register_predicate_method(owner, &col.name, registry);
        dirty_columns.push((Sym::new(&col.name), col.col_type.clone()));
    }
    registry.register_dirty_pattern_columns(owner, dirty_columns);
    for assoc in &table.associations {
        let assoc_type = nullable_type(&Type::Class(Sym::new(&assoc.target_class)), assoc.nullable);
        register_attribute_methods(owner, &assoc.name, assoc_type, registry);
    }
}

fn register_attribute_methods(
    class_name: &str,
    name: &str,
    accessor_type: Type,
    registry: &mut TypeRegistry,
) {
    registry.add_method_def_if_missing(
        class_name,
        MethodDef {
            name: Sym::new(name),
            param_infos: Vec::new(),
            raw_return_type: accessor_type.clone(),
            sorbet_modifier_comments: Vec::new(),
            rbs_annotated: true,
            rbs_inline_annotated: false,
            sig_annotated: false,
            attr_ivar: None,
            is_singleton: false,
            rbs_file_source: true,
            synthetic_dsl_source: true,
            rbs_method_types: Default::default(),
            extra_overloads: Vec::new(),
            loc: None,
        },
    );

    registry.add_method_def_if_missing(
        class_name,
        MethodDef {
            name: Sym::new(format!("{name}=")),
            param_infos: vec![ParamInfo {
                name: name.to_string(),
                kind: ParamKind::Required,
                default_type: Some(accessor_type.clone()),
            }],
            raw_return_type: accessor_type,
            sorbet_modifier_comments: Vec::new(),
            rbs_annotated: true,
            rbs_inline_annotated: false,
            sig_annotated: false,
            attr_ivar: None,
            is_singleton: false,
            rbs_file_source: true,
            synthetic_dsl_source: true,
            rbs_method_types: Default::default(),
            extra_overloads: Vec::new(),
            loc: None,
        },
    );
}

fn register_predicate_method(class_name: &str, name: &str, registry: &mut TypeRegistry) {
    registry.add_method_def_if_missing(
        class_name,
        MethodDef {
            name: Sym::new(format!("{name}?")),
            param_infos: Vec::new(),
            raw_return_type: Type::Bool,
            sorbet_modifier_comments: Vec::new(),
            rbs_annotated: true,
            rbs_inline_annotated: false,
            sig_annotated: false,
            attr_ivar: None,
            is_singleton: false,
            rbs_file_source: true,
            synthetic_dsl_source: true,
            rbs_method_types: Default::default(),
            extra_overloads: Vec::new(),
            loc: None,
        },
    );
}

fn nullable_type(base: &Type, nullable: bool) -> Type {
    crate::rails::nullable_column_accessor_type(base, nullable)
}

fn column_nullable(call: &ruby_prism::CallNode<'_>, parse_result: &ParseResult<'_>) -> bool {
    if matches!(hash_option_bool(call, "null", parse_result), Some(false)) {
        return false;
    }
    !has_hash_option(call, "default", parse_result)
}

fn has_hash_option(
    call: &ruby_prism::CallNode<'_>,
    key: &str,
    parse_result: &ParseResult<'_>,
) -> bool {
    hash_option_node(call, key, parse_result).is_some()
}

fn hash_option_bool(
    call: &ruby_prism::CallNode<'_>,
    key: &str,
    parse_result: &ParseResult<'_>,
) -> Option<bool> {
    let node = hash_option_node(call, key, parse_result)?;
    match node {
        Node::TrueNode { .. } => Some(true),
        Node::FalseNode { .. } => Some(false),
        _ => None,
    }
}

fn hash_option_node<'a>(
    call: &'a ruby_prism::CallNode<'_>,
    key: &str,
    parse_result: &'a ParseResult<'_>,
) -> Option<Node<'a>> {
    let args = call.arguments()?;
    for arg in args.arguments().iter() {
        match &arg {
            Node::KeywordHashNode { .. } => {
                let hash = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                for elem in hash.elements().iter() {
                    if let Node::AssocNode { .. } = &elem {
                        let assoc = elem.as_assoc_node().expect("must be AssocNode");
                        let assoc_key = extract_label_or_symbol(&assoc.key(), parse_result);
                        if assoc_key.as_deref() == Some(key) {
                            return Some(assoc.value());
                        }
                    }
                }
            }
            Node::HashNode { .. } => {
                let hash = arg.as_hash_node().expect("must be HashNode");
                for elem in hash.elements().iter() {
                    if let Node::AssocNode { .. } = &elem {
                        let assoc = elem.as_assoc_node().expect("must be AssocNode");
                        let assoc_key = extract_label_or_symbol(&assoc.key(), parse_result);
                        if assoc_key.as_deref() == Some(key) {
                            return Some(assoc.value());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_label_or_symbol(node: &Node<'_>, parse_result: &ParseResult<'_>) -> Option<String> {
    if let Node::SymbolNode { .. } = node {
        let sym = node.as_symbol_node().expect("must be SymbolNode");
        return Some(String::from_utf8_lossy(sym.unescaped()).to_string());
    }
    let raw = &parse_result.source()[node.location().start_offset()..node.location().end_offset()];
    let s = String::from_utf8_lossy(raw).to_string();
    let label = s.trim_end_matches(':');
    if label != s && !label.is_empty() {
        Some(label.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquote_sql_ident_strips_dialect_quotes() {
        assert_eq!(unquote_sql_ident("`person_id`"), "person_id");
        assert_eq!(unquote_sql_ident("\"person_id\""), "person_id");
        assert_eq!(unquote_sql_ident("[person_id]"), "person_id");
        assert_eq!(unquote_sql_ident("person_id"), "person_id");
    }

    #[test]
    fn parses_mysql_create_table_with_backticks() {
        assert_eq!(
            parse_structure_table_start("CREATE TABLE `shop_users` ("),
            Some("shop_users".to_string())
        );
        assert_eq!(
            parse_structure_table_start("CREATE TABLE `db`.`shop_users` ("),
            Some("shop_users".to_string())
        );
    }

    #[test]
    fn mysql_column_types_map_to_ruby_types() {
        assert_eq!(structure_sql_type_to_type("bigint NOT NULL"), Type::Integer);
        assert_eq!(structure_sql_type_to_type("int unsigned"), Type::Integer);
        assert_eq!(structure_sql_type_to_type("tinyint(1)"), Type::Bool);
        assert_eq!(structure_sql_type_to_type("tinyint(4)"), Type::Integer);
        assert_eq!(structure_sql_type_to_type("varchar(255)"), Type::String);
        assert_eq!(
            structure_sql_type_to_type("datetime(6)"),
            Type::Union(vec![
                Type::Class(Sym::new("DateTime")),
                Type::Class(Sym::new("ActiveSupport::TimeWithZone")),
            ])
        );
        assert_eq!(structure_sql_type_to_type("double"), Type::Float);
    }

    #[test]
    fn postgres_column_types_map_through_canonical_map() {
        // decimal / numeric map to BigDecimal, same as the DSL path.
        assert_eq!(
            structure_sql_type_to_type("numeric(10,2)"),
            Type::Class(Sym::new("BigDecimal"))
        );
        // date maps to Date; timestamp maps to the datetime union.
        assert_eq!(
            structure_sql_type_to_type("date"),
            Type::Class(Sym::new("Date"))
        );
        assert_eq!(
            structure_sql_type_to_type("timestamp(6) without time zone"),
            Type::Union(vec![
                Type::Class(Sym::new("DateTime")),
                Type::Class(Sym::new("ActiveSupport::TimeWithZone")),
            ])
        );
        // json / jsonb / hstore have no concrete type, so they degrade to Untyped.
        assert_eq!(structure_sql_type_to_type("jsonb"), Type::Untyped);
        assert_eq!(structure_sql_type_to_type("json"), Type::Untyped);
        assert_eq!(structure_sql_type_to_type("hstore"), Type::Untyped);
        // Postgres array columns become an Array wrapping the element type.
        assert_eq!(
            structure_sql_type_to_type("character varying[]"),
            Type::Array(Some(Box::new(Type::String)))
        );
        assert_eq!(
            structure_sql_type_to_type("integer[]"),
            Type::Array(Some(Box::new(Type::Integer)))
        );
        // Unknown SQL types become Untyped rather than an incorrect concrete type.
        assert_eq!(structure_sql_type_to_type("geometry"), Type::Untyped);
    }

    #[test]
    fn mysql_key_lines_are_not_columns() {
        assert!(parse_structure_column_line("PRIMARY KEY (`id`),").is_none());
        assert!(
            parse_structure_column_line("KEY `index_shop_users_on_person_id` (`person_id`),")
                .is_none()
        );
        assert!(parse_structure_column_line("UNIQUE KEY `idx` (`a`,`b`),").is_none());
        let col = parse_structure_column_line("`person_id` bigint NOT NULL,").expect("column");
        assert_eq!(col.name, "person_id");
        assert_eq!(col.col_type, Type::Integer);
        assert!(!col.nullable);
    }

    #[test]
    fn parse_structure_sql_handles_mysql_dialect() {
        // MySQL dump form: backtick names, `) ENGINE=...;` terminator, KEY lines.
        let sql = "DROP TABLE IF EXISTS `shop_users`;\n\
CREATE TABLE `shop_users` (\n\
  `id` bigint NOT NULL AUTO_INCREMENT,\n\
  `person_id` bigint NOT NULL,\n\
  `status` int DEFAULT NULL,\n\
  PRIMARY KEY (`id`),\n\
  KEY `index_shop_users_on_person_id` (`person_id`)\n\
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;\n";
        let tables = parse_structure_sql(sql, Path::new("/nonexistent"));
        assert_eq!(tables.len(), 1, "one table parsed");
        let names: Vec<&str> = tables[0].columns.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"person_id"), "person_id column: {names:?}");
        assert!(names.contains(&"status"), "status column: {names:?}");
        assert!(
            !names.contains(&"KEY"),
            "KEY line must not be a column: {names:?}"
        );
    }
}

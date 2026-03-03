use crate::types::Sym;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct ProjectInflector {
    irregular_singulars: HashMap<String, String>,
    acronyms: HashMap<String, String>,
}

pub fn classify(table_name: &str) -> String {
    flat_classify_with(table_name, &ProjectInflector::default())
}

pub fn classify_with_known_classes(name: &str, known_classes: &[String]) -> String {
    classify_with_inflector_and_known_classes(name, &ProjectInflector::default(), known_classes)
}

pub fn classify_with_project_known_classes(
    root: &Path,
    name: &str,
    known_classes: &[String],
) -> String {
    let inflector = load_project_inflector(root);
    let flat = flat_classify_with(name, &inflector);
    let Some(namespaced) = namespaced_classify_candidate_with(name, &inflector) else {
        return flat;
    };
    if known_classes
        .iter()
        .any(|candidate| candidate == &namespaced)
        || model_path_exists(root, &namespaced)
    {
        namespaced
    } else {
        flat
    }
}

pub fn resolve_model_class_name(root: &Path, table_name: &str) -> String {
    let inflector = load_project_inflector(root);
    let flat = flat_classify_with(table_name, &inflector);
    if let Some(namespaced) = namespaced_classify_candidate_with(table_name, &inflector)
        && model_path_exists(root, &namespaced)
    {
        return namespaced;
    }
    flat
}

pub fn load_project_inflector(root: &Path) -> ProjectInflector {
    let path = root
        .join("config")
        .join("initializers")
        .join("inflections.rb");
    let Ok(source) = std::fs::read_to_string(path) else {
        return ProjectInflector::default();
    };

    let mut inflector = ProjectInflector::default();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("inflect.irregular")
            .or_else(|| trimmed.strip_prefix("ActiveSupport::Inflector.inflections"))
            && rest.contains("do |inflect|")
        {
            continue;
        }
        if let Some((singular, plural)) = parse_inflection_pair(trimmed, "inflect.irregular") {
            inflector.irregular_singulars.insert(plural, singular);
        }
        if let Some(acronym) = parse_inflection_token(trimmed, "inflect.acronym") {
            inflector
                .acronyms
                .insert(acronym.to_ascii_lowercase(), acronym);
        }
    }
    inflector
}

/// A simplified singularize (approximates ActiveSupport::Inflector).
pub fn singularize(word: &str) -> String {
    singularize_with(word, &ProjectInflector::default())
}

pub fn singularize_with_project(root: &Path, word: &str) -> String {
    let inflector = load_project_inflector(root);
    singularize_with(word, &inflector)
}

fn singularize_with(word: &str, inflector: &ProjectInflector) -> String {
    if let Some(irregular) = inflector.irregular_singulars.get(word) {
        return irregular.clone();
    }
    if word.ends_with("ies") && word.len() > 3 {
        format!("{}y", &word[..word.len() - 3])
    } else if word.ends_with("sses") {
        word[..word.len() - 2].to_string()
    } else if word.ends_with("ves") && word.len() > 3 {
        format!("{}f", &word[..word.len() - 3])
    } else if word.ends_with("ices") && word.len() > 4 {
        format!("{}ex", &word[..word.len() - 4])
    } else if (word.ends_with("ses") && word.len() > 3)
        || (word.ends_with("xes") && word.len() > 3)
        || word.ends_with("ches")
        || word.ends_with("shes")
    {
        word[..word.len() - 2].to_string()
    } else if word.ends_with('s') && !word.ends_with("ss") && word.len() > 1 {
        word[..word.len() - 1].to_string()
    } else {
        word.to_string()
    }
}

/// Canonical column-type map shared by schema / structure / DSL. `None` = unmapped (e.g. a custom cast type).
pub fn column_type_keyword_to_type(col_type: &str) -> Option<crate::types::Type> {
    use crate::types::Type;
    Some(match col_type {
        "string" | "text" | "citext" | "uuid" | "binary" | "immutable_string" => Type::String,
        "integer" | "bigint" | "smallint" | "big_integer" => Type::Integer,
        "float" => Type::Float,
        "decimal" | "numeric" => Type::Class(Sym::new("BigDecimal")),
        "boolean" => Type::Bool,
        "date" => Type::Class(Sym::new("Date")),
        "datetime" | "timestamp" => Type::Union(vec![
            Type::Class(Sym::new("DateTime")),
            Type::Class(Sym::new("ActiveSupport::TimeWithZone")),
        ]),
        "time" => Type::Class(Sym::new("Time")),
        "json" | "jsonb" | "hstore" => Type::Untyped,
        _ => return None,
    })
}

pub fn column_type_to_type(col_type: &str) -> crate::types::Type {
    column_type_keyword_to_type(col_type).unwrap_or(crate::types::Type::Untyped)
}

fn flat_classify_with(table_name: &str, inflector: &ProjectInflector) -> String {
    let singular = singularize_with(table_name, inflector);
    singular
        .split('_')
        .map(|part| camelize_segment(part, inflector))
        .collect()
}

fn classify_with_inflector_and_known_classes(
    name: &str,
    inflector: &ProjectInflector,
    known_classes: &[String],
) -> String {
    let flat = flat_classify_with(name, inflector);
    let Some(namespaced) = namespaced_classify_candidate_with(name, inflector) else {
        return flat;
    };
    if known_classes
        .iter()
        .any(|candidate| candidate == &namespaced)
    {
        namespaced
    } else {
        flat
    }
}

fn namespaced_classify_candidate_with(
    table_name: &str,
    inflector: &ProjectInflector,
) -> Option<String> {
    let singular = singularize_with(table_name, inflector);
    let parts: Vec<&str> = singular
        .split('_')
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    let namespace_parts = &parts[..parts.len() - 1];
    let namespace_like = namespace_parts
        .iter()
        .all(|part| singularize_with(part, inflector) == *part);
    if !namespace_like {
        return None;
    }
    let namespace = namespace_parts
        .iter()
        .map(|part| camelize_segment(part, inflector))
        .collect::<Vec<_>>()
        .join("::");
    let model = camelize_segment(parts[parts.len() - 1], inflector);
    Some(format!("{namespace}::{model}"))
}

fn camelize_segment(part: &str, inflector: &ProjectInflector) -> String {
    if let Some(acronym) = inflector.acronyms.get(&part.to_ascii_lowercase()) {
        return acronym.clone();
    }
    let mut chars = part.chars();
    match chars.next() {
        Some(c) => {
            let upper: String = c.to_uppercase().collect();
            format!("{upper}{}", chars.as_str())
        }
        None => String::new(),
    }
}

fn parse_inflection_pair(line: &str, prefix: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix(prefix)?.trim();
    let tokens = extract_quoted_tokens(rest);
    if tokens.len() < 2 {
        return None;
    }
    Some((tokens[0].clone(), tokens[1].clone()))
}

fn parse_inflection_token(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?.trim();
    extract_quoted_tokens(rest).into_iter().next()
}

fn extract_quoted_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in input.chars() {
        match quote {
            Some(q) if ch == q => {
                tokens.push(std::mem::take(&mut current));
                quote = None;
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None => {}
        }
    }
    tokens
}

fn model_path_exists(root: &Path, class_name: &str) -> bool {
    let relative = class_name
        .split("::")
        .map(underscore)
        .collect::<Vec<_>>()
        .join("/");
    root.join("app")
        .join("models")
        .join(format!("{relative}.rb"))
        .exists()
}

fn underscore(name: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && idx > 0 {
            out.push('_');
        }
        out.extend(ch.to_lowercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify() {
        assert_eq!(classify("users"), "User");
        assert_eq!(classify("admin_users"), "AdminUser");
        assert_eq!(classify("categories"), "Category");
        assert_eq!(classify("posts"), "Post");
        assert_eq!(classify("addresses"), "Address");
    }

    #[test]
    fn test_singularize() {
        assert_eq!(singularize("users"), "user");
        assert_eq!(singularize("categories"), "category");
        assert_eq!(singularize("posts"), "post");
        assert_eq!(singularize("addresses"), "address");
        assert_eq!(singularize("buses"), "bus");
        assert_eq!(singularize("boxes"), "box");
        assert_eq!(singularize("matches"), "match");
    }

    #[test]
    fn test_column_type_to_type() {
        use crate::types::Type;
        assert!(matches!(column_type_to_type("string"), Type::String));
        assert!(matches!(column_type_to_type("integer"), Type::Integer));
        assert!(matches!(column_type_to_type("boolean"), Type::Bool));
        assert!(matches!(column_type_to_type("float"), Type::Float));
        // decimal / numeric map to BigDecimal, not Float (matches the DSL path).
        assert_eq!(
            column_type_to_type("decimal"),
            Type::Class(Sym::new("BigDecimal"))
        );
        assert_eq!(column_type_to_type("date"), Type::Class(Sym::new("Date")));
        assert_eq!(
            column_type_to_type("datetime"),
            Type::Union(vec![
                Type::Class(Sym::new("DateTime")),
                Type::Class(Sym::new("ActiveSupport::TimeWithZone")),
            ])
        );
        // json/jsonb/hstore have no concrete type, so they're Untyped; unknown column types also degrade to Untyped.
        assert_eq!(column_type_to_type("jsonb"), Type::Untyped);
        assert_eq!(column_type_to_type("nonesuch"), Type::Untyped);
    }

    #[test]
    fn column_type_keyword_distinguishes_known_untyped_from_unmapped() {
        use crate::types::Type;
        // json/jsonb/hstore are "known but with no concrete type" = Some(Untyped).
        assert_eq!(column_type_keyword_to_type("json"), Some(Type::Untyped));
        assert_eq!(column_type_keyword_to_type("jsonb"), Some(Type::Untyped));
        assert_eq!(column_type_keyword_to_type("hstore"), Some(Type::Untyped));
        // A custom cast type not in the map is None (camelized on the DSL side).
        assert_eq!(column_type_keyword_to_type("account"), None);
        assert_eq!(column_type_keyword_to_type("money"), None);
    }

    #[test]
    fn test_classify_with_known_classes_prefers_namespaced_model() {
        let known = vec!["Admin::User".to_string(), "Post".to_string()];
        assert_eq!(
            classify_with_known_classes("admin_user", &known),
            "Admin::User"
        );
        assert_eq!(classify_with_known_classes("post", &known), "Post");
    }
}

pub fn sig_source_to_rbs(sig_source: &str) -> Option<String> {
    let inner = extract_sig_body(sig_source)?;

    let chain = parse_method_chain(&inner);
    if chain.is_empty() {
        return None;
    }

    let mut params_rbs: Option<String> = None;
    let mut return_rbs = "void".to_string();

    for (method_name, arg_str) in &chain {
        match method_name.as_str() {
            "params" => {
                if let Some(args) = arg_str {
                    params_rbs = Some(convert_params_to_rbs(args));
                }
            }
            "returns" => {
                if let Some(args) = arg_str {
                    return_rbs = convert_sorbet_type_str(args.trim());
                }
            }
            "void" => {
                return_rbs = "void".to_string();
            }
            "override" | "overridable" | "abstract" | "final" | "generated" | "soft"
            | "checked" | "on_failure" | "type_parameters" | "bind" => {}
            _ => {}
        }
    }

    let return_rbs = wrap_complex_return_type(&return_rbs);

    let rbs = if let Some(params) = params_rbs {
        format!("({params}) -> {return_rbs}")
    } else {
        format!("-> {return_rbs}")
    };

    Some(rbs)
}

// Parens are required because RBS parses `-> A | B` as `(-> A) | B`.
fn wrap_complex_return_type(s: &str) -> String {
    if s.contains('|') || s.contains('&') {
        format!("({s})")
    } else {
        s.to_string()
    }
}

fn extract_sig_body(source: &str) -> Option<String> {
    let s = source.trim();
    let rest = s.strip_prefix("sig")?;
    let rest = rest.trim();

    if let Some(inner) = rest.strip_prefix('{') {
        let inner = inner.strip_suffix('}')?;
        return Some(inner.trim().to_string());
    }

    if let Some(rest) = strip_do_block(rest) {
        return Some(rest.trim().to_string());
    }

    None
}

fn strip_do_block(s: &str) -> Option<String> {
    let rest = s.strip_prefix("do")?;
    if !rest.starts_with(|c: char| c.is_whitespace() || c == '\n') {
        return None;
    }

    let mut depth = 1;
    let chars: Vec<char> = rest.chars().collect();
    let mut pos = 0;

    while pos < chars.len() && depth > 0 {
        if pos + 3 <= chars.len() {
            let word: String = chars[pos..].iter().take(3).collect();
            if word == "end" {
                let before_ok =
                    pos == 0 || !chars[pos - 1].is_alphanumeric() && chars[pos - 1] != '_';
                let after_ok = pos + 3 >= chars.len()
                    || !chars[pos + 3].is_alphanumeric() && chars[pos + 3] != '_';
                if before_ok && after_ok {
                    depth -= 1;
                    if depth == 0 {
                        let inner: String = chars[..pos].iter().collect();
                        return Some(inner);
                    }
                    pos += 3;
                    continue;
                }
            }
        }
        if pos + 2 <= chars.len() {
            let word: String = chars[pos..].iter().take(2).collect();
            if word == "do" {
                let before_ok =
                    pos == 0 || !chars[pos - 1].is_alphanumeric() && chars[pos - 1] != '_';
                let after_ok = pos + 2 >= chars.len()
                    || !chars[pos + 2].is_alphanumeric() && chars[pos + 2] != '_';
                if before_ok && after_ok {
                    depth += 1;
                }
            }
        }
        pos += 1;
    }

    None
}

fn parse_method_chain(source: &str) -> Vec<(String, Option<String>)> {
    let mut result = Vec::new();
    let s = source.trim();
    let mut pos = 0;
    let chars: Vec<char> = s.chars().collect();

    while pos < chars.len() {
        while pos < chars.len()
            && (chars[pos] == '.'
                || chars[pos].is_whitespace()
                || chars[pos] == '\n'
                || chars[pos] == '\r')
        {
            pos += 1;
        }
        if pos >= chars.len() {
            break;
        }

        let name_start = pos;
        while pos < chars.len()
            && (chars[pos].is_alphanumeric() || chars[pos] == '_' || chars[pos] == '?')
        {
            pos += 1;
        }
        let name: String = chars[name_start..pos].iter().collect();
        if name.is_empty() {
            break;
        }

        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        if pos < chars.len() && chars[pos] == '(' {
            let arg_start = pos + 1;
            let mut depth = 1;
            pos += 1;
            while pos < chars.len() && depth > 0 {
                match chars[pos] {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    '"' => {
                        pos += 1;
                        while pos < chars.len() && chars[pos] != '"' {
                            if chars[pos] == '\\' {
                                pos += 1;
                            }
                            pos += 1;
                        }
                    }
                    '\'' => {
                        pos += 1;
                        while pos < chars.len() && chars[pos] != '\'' {
                            if chars[pos] == '\\' {
                                pos += 1;
                            }
                            pos += 1;
                        }
                    }
                    _ => {}
                }
                pos += 1;
            }
            // Excludes the closing paren, and stays slice-safe even when it's missing (e.g. mid-typed `params(`).
            let consumed_end = pos.min(chars.len());
            let arg_end = if consumed_end > arg_start && chars[consumed_end - 1] == ')' {
                consumed_end - 1
            } else {
                consumed_end
            };
            let arg_str: String = chars[arg_start..arg_end].iter().collect();
            result.push((name, Some(arg_str.trim().to_string())));
        } else {
            result.push((name, None));
        }
    }

    result
}

/// Convert `x: Integer, y: String` to RBS params `Integer x, String y`.
fn convert_params_to_rbs(params_source: &str) -> String {
    let pairs = split_top_level(params_source, ',');
    let mut rbs_params = Vec::new();

    for pair in &pairs {
        let pair = pair.trim();
        if let Some(colon_pos) = find_key_colon(pair) {
            let key = pair[..colon_pos].trim();
            let value = pair[colon_pos + 1..].trim();
            let rbs_type = convert_sorbet_type_str(value);
            // Quoted param names (e.g. `"&"`) — strip quotes
            let clean_key = key.trim_matches('"').trim_matches('\'');
            rbs_params.push(format!("{rbs_type} {clean_key}"));
        }
    }

    rbs_params.join(", ")
}

/// Find the colon separating key from value in `name: Type`.
/// Must distinguish from `::` in constant paths like `T::Array`.
fn find_key_colon(s: &str) -> Option<usize> {
    let chars: Vec<char> = s.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == ':' {
            if i + 1 < chars.len() && chars[i + 1] == ':' {
                continue;
            }
            if i > 0 && chars[i - 1] == ':' {
                continue;
            }
            return Some(i);
        }
    }
    None
}

/// Convert a Sorbet type string to an RBS type string.
pub fn convert_sorbet_type_str(s: &str) -> String {
    let s = s.trim();

    if s.is_empty() {
        return "untyped".to_string();
    }

    if let Some(inner) = strip_call(s, "T.nilable") {
        let inner_rbs = convert_sorbet_type_str(&inner);
        if inner_rbs.contains('|') || inner_rbs.contains('&') {
            return format!("({inner_rbs})?");
        }
        return format!("{inner_rbs}?");
    }
    if let Some(inner) = strip_call(s, "T.any") {
        let parts = split_top_level(&inner, ',');
        let types: Vec<String> = parts
            .iter()
            .map(|p| convert_sorbet_type_str(p.trim()))
            .collect();
        return types.join(" | ");
    }
    if let Some(inner) = strip_call(s, "T.all") {
        let parts = split_top_level(&inner, ',');
        let types: Vec<String> = parts
            .iter()
            .map(|p| convert_sorbet_type_str(p.trim()))
            .collect();
        return types.join(" & ");
    }
    if s == "T.untyped" {
        return "untyped".to_string();
    }
    if s == "T.noreturn" {
        return "bot".to_string();
    }
    if s == "T.self_type" {
        return "self".to_string();
    }
    // `T.attached_class` -> keep as `instance` (collapsing to `self` in a singleton factory would incorrectly become `singleton(Sub)`).
    if s == "T.attached_class" {
        return "instance".to_string();
    }
    if s == "T.anything" {
        return "top".to_string();
    }
    if let Some(inner) = strip_call(s, "T.class_of") {
        let inner_rbs = convert_sorbet_type_str(&inner);
        return format!("singleton({inner_rbs})");
    }
    if let Some(inner) = strip_call(s, "T.type_parameter") {
        let sym = inner.trim().strip_prefix(':').unwrap_or(&inner);
        return sym.to_string();
    }
    // T.let(expr, Type) — extract Type
    if let Some(inner) = strip_call(s, "T.let") {
        let parts = split_top_level(&inner, ',');
        if parts.len() >= 2 {
            return convert_sorbet_type_str(parts[1].trim());
        }
        return "untyped".to_string();
    }
    // T.cast(expr, Type) — extract Type
    if let Some(inner) = strip_call(s, "T.cast") {
        let parts = split_top_level(&inner, ',');
        if parts.len() >= 2 {
            return convert_sorbet_type_str(parts[1].trim());
        }
        return "untyped".to_string();
    }
    // T.must(expr) — passthrough (removes nilable in Sorbet, we just return the type)
    if let Some(inner) = strip_call(s, "T.must") {
        return convert_sorbet_type_str(&inner);
    }
    // T.must_because(expr){...} — passthrough
    if s.starts_with("T.must_because") {
        return "untyped".to_string();
    }
    // T.assert_type!(expr, Type) — extract Type
    if let Some(inner) = strip_call(s, "T.assert_type!") {
        let parts = split_top_level(&inner, ',');
        if parts.len() >= 2 {
            return convert_sorbet_type_str(parts[1].trim());
        }
        return "untyped".to_string();
    }
    // T.reveal_type — not a real type, but don't crash
    if s.starts_with("T.reveal_type") {
        return "untyped".to_string();
    }

    if s.starts_with("T.proc") {
        return convert_proc_type_str(s);
    }

    // `T::Class[X]`/`T::Module[X]`: `singleton(X)` for a nameable class, otherwise bare `Class`/`Module` (checked before the generic path).
    if s == "T::Class" {
        return "Class".to_string();
    }
    if s == "T::Module" {
        return "Module".to_string();
    }
    if let Some(inner) = strip_bracket(s, "T::Class") {
        // `singleton(X)` only for a nameable class param, otherwise bare `Class`.
        let inner = inner.trim();
        if is_nameable_class(inner) {
            return format!("singleton({})", convert_sorbet_class_name(inner));
        }
        return "Class".to_string();
    }
    if strip_bracket(s, "T::Module").is_some() {
        return "Module".to_string();
    }

    // Generic bracket: only when there's a closing `]` (a mid-typed `T::Hash[` falls through to the plain path).
    if let Some(bracket_pos) = find_top_level_bracket(s)
        && s.ends_with(']')
        && s.len() >= bracket_pos + 2
    {
        let base = &s[..bracket_pos];
        let args_str = &s[bracket_pos + 1..s.len() - 1];
        let base_rbs = convert_sorbet_class_name(base.trim());
        let parts = split_top_level(args_str, ',');
        let type_args: Vec<String> = parts
            .iter()
            .map(|p| convert_sorbet_type_str(p.trim()))
            .collect();
        return format!("{}[{}]", base_rbs, type_args.join(", "));
    }

    convert_sorbet_class_name(s)
}

fn convert_proc_type_str(s: &str) -> String {
    let chain = parse_method_chain(s);
    let mut proc_params = Vec::new();
    let mut proc_return = "void".to_string();

    for (name, args) in &chain {
        match name.as_str() {
            "params" => {
                if let Some(args_str) = args {
                    let pairs = split_top_level(args_str, ',');
                    for pair in &pairs {
                        let pair = pair.trim();
                        if let Some(colon_pos) = find_key_colon(pair) {
                            let value = pair[colon_pos + 1..].trim();
                            proc_params.push(convert_sorbet_type_str(value));
                        }
                    }
                }
            }
            "returns" => {
                if let Some(args_str) = args {
                    proc_return = convert_sorbet_type_str(args_str.trim());
                }
            }
            "void" => {
                proc_return = "void".to_string();
            }
            // bind, etc. — skip
            _ => {}
        }
    }

    if proc_params.is_empty() {
        format!("^() -> {proc_return}")
    } else {
        format!("^({}) -> {proc_return}", proc_params.join(", "))
    }
}

/// Strips calls like `T.nilable(X)`, or a `Prefix[...]` bracket.
fn strip_bracket(s: &str, prefix: &str) -> Option<String> {
    let rest = s.strip_prefix(prefix)?.trim();
    let rest = rest.strip_prefix('[')?;
    let mut depth = 1;
    let chars: Vec<char> = rest.chars().collect();
    let mut pos = 0;
    while pos < chars.len() && depth > 0 {
        match chars[pos] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            pos += 1;
        }
    }
    // Require the closing bracket to be the final character so `Foo[A].bar`
    // is not mistaken for a bracketed application of `Foo`.
    if depth == 0 && pos == chars.len() - 1 {
        Some(chars[..pos].iter().collect::<String>().trim().to_string())
    } else {
        None
    }
}

/// True when `s` is a plain (optionally `::`-qualified) class name that can sit
/// inside `singleton(...)` — i.e. no unions, generics, spaces or `untyped`.
fn is_nameable_class(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
        && s.chars()
            .next()
            .is_some_and(|c| c == ':' || c.is_uppercase())
        && s != "Class"
}

fn strip_call(s: &str, prefix: &str) -> Option<String> {
    let rest = s.strip_prefix(prefix)?;
    let rest = rest.trim();
    let rest = rest.strip_prefix('(')?;
    // Find matching closing paren (handling nested parens)
    let mut depth = 1;
    let chars: Vec<char> = rest.chars().collect();
    let mut pos = 0;
    while pos < chars.len() && depth > 0 {
        match chars[pos] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            pos += 1;
        }
    }
    if depth == 0 {
        let inner: String = chars[..pos].iter().collect();
        Some(inner.trim().to_string())
    } else {
        None
    }
}

/// Split by `sep` at the top level (not inside brackets/parens/strings).
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(chars[i]);
            }
            ')' | ']' | '}' => {
                if depth > 0 {
                    depth -= 1;
                }
                current.push(chars[i]);
            }
            '"' => {
                current.push(chars[i]);
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        current.push(chars[i]);
                        i += 1;
                        if i < chars.len() {
                            current.push(chars[i]);
                        }
                    } else {
                        current.push(chars[i]);
                    }
                    i += 1;
                }
                if i < chars.len() {
                    current.push(chars[i]);
                }
            }
            '\'' => {
                current.push(chars[i]);
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    if chars[i] == '\\' {
                        current.push(chars[i]);
                        i += 1;
                        if i < chars.len() {
                            current.push(chars[i]);
                        }
                    } else {
                        current.push(chars[i]);
                    }
                    i += 1;
                }
                if i < chars.len() {
                    current.push(chars[i]);
                }
            }
            c if c == sep && depth == 0 => {
                result.push(current.clone());
                current.clear();
            }
            _ => current.push(chars[i]),
        }
        i += 1;
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Find the position of `[` at the top level.
fn find_top_level_bracket(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.chars().enumerate() {
        match ch {
            '(' | '{' => depth += 1,
            ')' | '}' => depth -= 1,
            '[' if depth == 0 => return Some(i),
            ']' if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

fn convert_sorbet_class_name(name: &str) -> String {
    match name {
        "T::Boolean" => "bool".to_string(),
        "T::Array" => "Array".to_string(),
        "T::Hash" => "Hash".to_string(),
        "T::Range" => "Range".to_string(),
        "T::Enumerable" => "Enumerable".to_string(),
        "T::Enumerator" => "Enumerator".to_string(),
        "T::Enumerator::Lazy" => "Enumerator::Lazy".to_string(),
        "T::Set" => "Set".to_string(),
        "NilClass" => "nil".to_string(),
        "TrueClass" => "bool".to_string(),
        "FalseClass" => "bool".to_string(),
        _ => name.to_string(),
    }
}

/// Check if a source line is a `sig` call.
pub fn is_sig_line(source: &str) -> bool {
    let trimmed = source.trim();
    trimmed.starts_with("sig {") || trimmed.starts_with("sig do")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Basic ──

    #[test]
    fn test_basic_returns() {
        let rbs = sig_source_to_rbs("sig { returns(String) }").unwrap();
        assert_eq!(rbs, "-> String");
    }

    #[test]
    fn test_void() {
        let rbs = sig_source_to_rbs("sig { void }").unwrap();
        assert_eq!(rbs, "-> void");
    }

    #[test]
    fn test_params_returns() {
        let rbs =
            sig_source_to_rbs("sig { params(x: Integer, y: String).returns(String) }").unwrap();
        assert_eq!(rbs, "(Integer x, String y) -> String");
    }

    // ── sig do...end ──

    #[test]
    fn test_sig_do_end() {
        let rbs =
            sig_source_to_rbs("sig do\n  params(x: Integer, y: String)\n  .returns(String)\nend")
                .unwrap();
        assert_eq!(rbs, "(Integer x, String y) -> String");
    }

    #[test]
    fn test_sig_do_end_void() {
        let rbs = sig_source_to_rbs("sig do\n  void\nend").unwrap();
        assert_eq!(rbs, "-> void");
    }

    #[test]
    fn test_sig_do_end_multiline_params() {
        let rbs = sig_source_to_rbs(
            "sig do\n  params(\n    x: Integer,\n    y: String,\n  )\n  .returns(String)\nend",
        )
        .unwrap();
        assert_eq!(rbs, "(Integer x, String y) -> String");
    }

    // ── Modifiers (should be skipped gracefully) ──

    #[test]
    fn test_override() {
        let rbs = sig_source_to_rbs("sig { override.returns(String) }").unwrap();
        assert_eq!(rbs, "-> String");
    }

    #[test]
    fn test_overridable() {
        let rbs =
            sig_source_to_rbs("sig { overridable.params(x: Integer).returns(String) }").unwrap();
        assert_eq!(rbs, "(Integer x) -> String");
    }

    #[test]
    fn test_abstract() {
        let rbs = sig_source_to_rbs("sig { abstract.returns(String) }").unwrap();
        assert_eq!(rbs, "-> String");
    }

    #[test]
    fn test_override_params() {
        let rbs = sig_source_to_rbs(
            "sig { override.params(name: String, age: Integer).returns(String) }",
        )
        .unwrap();
        assert_eq!(rbs, "(String name, Integer age) -> String");
    }

    #[test]
    fn test_checked() {
        let rbs = sig_source_to_rbs("sig { checked(:never).returns(Integer) }").unwrap();
        assert_eq!(rbs, "-> Integer");
    }

    #[test]
    fn test_override_overridable_chain() {
        let rbs = sig_source_to_rbs("sig { override.overridable.params(x: String).void }").unwrap();
        assert_eq!(rbs, "(String x) -> void");
    }

    // ── Generics / type_parameters ──

    #[test]
    fn test_type_parameters_simple() {
        let rbs = sig_source_to_rbs(
            "sig { type_parameters(:U).params(x: T.type_parameter(:U)).returns(T.type_parameter(:U)) }",
        )
        .unwrap();
        assert_eq!(rbs, "(U x) -> U");
    }

    #[test]
    fn test_type_parameters_two() {
        let rbs = sig_source_to_rbs(
            "sig { type_parameters(:U, :V).params(x: T.type_parameter(:U), y: T.type_parameter(:V)).returns(T.type_parameter(:U)) }",
        )
        .unwrap();
        assert_eq!(rbs, "(U x, V y) -> U");
    }

    #[test]
    fn test_type_parameters_do_end() {
        let rbs = sig_source_to_rbs(
            "sig do\n  type_parameters(:U)\n    .params(x: T.type_parameter(:U))\n    .returns(T.type_parameter(:U))\nend",
        )
        .unwrap();
        assert_eq!(rbs, "(U x) -> U");
    }

    // ── Type conversions ──

    #[test]
    fn test_nilable() {
        let rbs =
            sig_source_to_rbs("sig { params(x: T.nilable(String)).returns(T.nilable(Integer)) }")
                .unwrap();
        assert_eq!(rbs, "(String? x) -> Integer?");
    }

    #[test]
    fn test_t_any() {
        let rbs =
            sig_source_to_rbs("sig { params(x: T.any(String, Integer)).returns(String) }").unwrap();
        assert_eq!(rbs, "(String | Integer x) -> String");
    }

    #[test]
    fn test_t_all() {
        let rbs =
            sig_source_to_rbs("sig { params(x: T.all(Enumerable, Comparable)).returns(String) }")
                .unwrap();
        assert_eq!(rbs, "(Enumerable & Comparable x) -> String");
    }

    #[test]
    fn test_t_boolean() {
        let rbs = sig_source_to_rbs("sig { params(x: T::Boolean).returns(T::Boolean) }").unwrap();
        assert_eq!(rbs, "(bool x) -> bool");
    }

    #[test]
    fn test_t_array() {
        let rbs =
            sig_source_to_rbs("sig { params(items: T::Array[String]).returns(T::Array[Integer]) }")
                .unwrap();
        assert_eq!(rbs, "(Array[String] items) -> Array[Integer]");
    }

    #[test]
    fn test_t_hash() {
        let rbs = sig_source_to_rbs(
            "sig { params(h: T::Hash[Symbol, Integer]).returns(T::Hash[String, String]) }",
        )
        .unwrap();
        assert_eq!(rbs, "(Hash[Symbol, Integer] h) -> Hash[String, String]");
    }

    #[test]
    fn test_untyped() {
        let rbs = sig_source_to_rbs("sig { returns(T.untyped) }").unwrap();
        assert_eq!(rbs, "-> untyped");
    }

    #[test]
    fn test_class_of() {
        let rbs = sig_source_to_rbs("sig { returns(T.class_of(String)) }").unwrap();
        assert_eq!(rbs, "-> singleton(String)");
    }

    #[test]
    fn test_noreturn() {
        let rbs = sig_source_to_rbs("sig { returns(T.noreturn) }").unwrap();
        assert_eq!(rbs, "-> bot");
    }

    #[test]
    fn test_anything() {
        let rbs = sig_source_to_rbs("sig { returns(T.anything) }").unwrap();
        assert_eq!(rbs, "-> top");
    }

    // ── Proc types ──

    #[test]
    fn test_proc_void() {
        let rbs = sig_source_to_rbs("sig { params(blk: T.proc.void).void }").unwrap();
        assert_eq!(rbs, "(^() -> void blk) -> void");
    }

    #[test]
    fn test_proc_with_params() {
        let rbs = sig_source_to_rbs(
            "sig { params(blk: T.proc.params(x: Integer).returns(String)).returns(String) }",
        )
        .unwrap();
        assert_eq!(rbs, "(^(Integer) -> String blk) -> String");
    }

    #[test]
    fn test_proc_bind() {
        let rbs = sig_source_to_rbs(
            "sig { params(blk: T.proc.bind(MyClass).returns(Integer)).returns(Integer) }",
        )
        .unwrap();
        assert_eq!(rbs, "(^() -> Integer blk) -> Integer");
    }

    // ── Complex / nested ──

    #[test]
    fn test_nilable_array() {
        let rbs = sig_source_to_rbs("sig { returns(T.nilable(T::Array[String])) }").unwrap();
        assert_eq!(rbs, "-> Array[String]?");
    }

    #[test]
    fn test_any_three_types() {
        let rbs = sig_source_to_rbs("sig { returns(T.any(String, Integer, Symbol)) }").unwrap();
        assert_eq!(rbs, "-> (String | Integer | Symbol)");
    }

    #[test]
    fn test_hash_of_arrays() {
        let rbs = sig_source_to_rbs("sig { returns(T::Hash[String, T::Array[Integer]]) }").unwrap();
        assert_eq!(rbs, "-> Hash[String, Array[Integer]]");
    }

    #[test]
    fn test_nilclass() {
        let rbs = sig_source_to_rbs("sig { returns(NilClass) }").unwrap();
        assert_eq!(rbs, "-> nil");
    }

    #[test]
    fn test_trueclass() {
        let rbs = sig_source_to_rbs("sig { returns(TrueClass) }").unwrap();
        assert_eq!(rbs, "-> bool");
    }

    #[test]
    fn test_generic_class() {
        let rbs = sig_source_to_rbs("sig { returns(MyBox[String]) }").unwrap();
        assert_eq!(rbs, "-> MyBox[String]");
    }

    // ── Edge cases / robustness ──

    #[test]
    fn test_empty_sig_returns_none() {
        assert!(sig_source_to_rbs("sig { }").is_none());
    }

    #[test]
    fn test_malformed_sig_returns_none() {
        assert!(sig_source_to_rbs("not_a_sig").is_none());
    }

    #[test]
    fn test_sig_with_unknown_modifier() {
        let rbs = sig_source_to_rbs("sig { some_future_modifier.returns(String) }").unwrap();
        assert_eq!(rbs, "-> String");
    }

    #[test]
    fn test_override_abstract_do_end() {
        let rbs =
            sig_source_to_rbs("sig do\n  override\n  .params(x: Integer)\n  .returns(String)\nend")
                .unwrap();
        assert_eq!(rbs, "(Integer x) -> String");
    }

    #[test]
    fn test_quoted_block_param_name() {
        let rbs = sig_source_to_rbs("sig { params(\"&\": T.proc.void).void }").unwrap();
        assert_eq!(rbs, "(^() -> void &) -> void");
    }

    // ── Many params (keyword style in sig) ──

    #[test]
    fn test_many_params() {
        let rbs = sig_source_to_rbs(
            "sig { params(a: String, b: Integer, c: Float, d: T::Boolean, e: Symbol).returns(String) }",
        )
        .unwrap();
        assert_eq!(
            rbs,
            "(String a, Integer b, Float c, bool d, Symbol e) -> String"
        );
    }

    #[test]
    fn test_params_with_nilable() {
        let rbs = sig_source_to_rbs(
            "sig { params(name: String, email: T.nilable(String)).returns(String) }",
        )
        .unwrap();
        assert_eq!(rbs, "(String name, String? email) -> String");
    }

    #[test]
    fn test_params_with_array_type() {
        let rbs =
            sig_source_to_rbs("sig { params(items: T::Array[String]).returns(Integer) }").unwrap();
        assert_eq!(rbs, "(Array[String] items) -> Integer");
    }

    #[test]
    fn test_params_with_hash_type() {
        let rbs =
            sig_source_to_rbs("sig { params(opts: T::Hash[Symbol, String]).returns(String) }")
                .unwrap();
        assert_eq!(rbs, "(Hash[Symbol, String] opts) -> String");
    }

    #[test]
    fn test_params_with_union_return() {
        let rbs =
            sig_source_to_rbs("sig { params(x: Integer).returns(T.any(Integer, Float)) }").unwrap();
        assert_eq!(rbs, "(Integer x) -> (Integer | Float)");
    }

    #[test]
    fn test_params_with_proc_param() {
        let rbs = sig_source_to_rbs(
            "sig { params(items: T::Array[Integer], blk: T.proc.params(x: Integer).returns(String)).returns(T::Array[String]) }",
        )
        .unwrap();
        assert_eq!(
            rbs,
            "(Array[Integer] items, ^(Integer) -> String blk) -> Array[String]"
        );
    }

    #[test]
    fn test_do_end_many_params() {
        let rbs = sig_source_to_rbs(
            "sig do\n  params(\n    host: String,\n    port: Integer,\n    ssl: T::Boolean,\n    timeout: Integer,\n  )\n  .returns(String)\nend",
        )
        .unwrap();
        assert_eq!(
            rbs,
            "(String host, Integer port, bool ssl, Integer timeout) -> String"
        );
    }

    #[test]
    fn test_t_let() {
        assert_eq!(convert_sorbet_type_str("T.let(42, Integer)"), "Integer");
    }

    #[test]
    fn test_t_cast() {
        assert_eq!(convert_sorbet_type_str("T.cast(value, String)"), "String");
    }

    #[test]
    fn test_t_must() {
        assert_eq!(convert_sorbet_type_str("T.must(name)"), "name");
    }

    #[test]
    fn test_falseclass() {
        assert_eq!(convert_sorbet_type_str("FalseClass"), "bool");
    }

    #[test]
    fn test_self_type_and_attached_class() {
        // `T.self_type` is the receiver itself; `T.attached_class` is an instance
        // of the receiver's class (`instance`). Both resolve to the receiver in a singleton factory.
        assert_eq!(convert_sorbet_type_str("T.self_type"), "self");
        assert_eq!(convert_sorbet_type_str("T.attached_class"), "instance");
    }

    #[test]
    fn test_class_and_module_object_types() {
        // The overwhelmingly common tapioca form: any class / module object.
        assert_eq!(convert_sorbet_type_str("T::Class[T.anything]"), "Class");
        assert_eq!(convert_sorbet_type_str("T::Module[T.anything]"), "Module");
        assert_eq!(convert_sorbet_type_str("T::Class"), "Class");
        assert_eq!(convert_sorbet_type_str("T::Module"), "Module");
        // A concrete class parameter resolves to its singleton.
        assert_eq!(convert_sorbet_type_str("T::Class[Foo]"), "singleton(Foo)");
        assert_eq!(
            convert_sorbet_type_str("T::Class[::Foo::Bar]"),
            "singleton(::Foo::Bar)"
        );
        // Non-nameable parameters fall back to the bare class object.
        assert_eq!(
            convert_sorbet_type_str("T::Class[T.type_parameter(:T)]"),
            "Class"
        );
        // Nests inside other generics.
        assert_eq!(
            convert_sorbet_type_str("T::Array[T::Class[T.anything]]"),
            "Array[Class]"
        );
    }

    // ── Malformed / half-typed generics must not panic ──

    #[test]
    fn malformed_open_generic_does_not_panic() {
        // A half-typed `T::Hash[` (no closing `]`) used to slice past the
        // string end and panic. It must degrade gracefully instead.
        for s in [
            "T::Hash[",
            "T::Array[",
            "Foo[",
            "T::Hash[String,",
            "[",
            "]",
            "Foo]",
            "T::Hash[String, Integer", // missing closing bracket
        ] {
            let _ = convert_sorbet_type_str(s);
        }
    }

    #[test]
    fn valid_generic_still_converts() {
        assert_eq!(
            convert_sorbet_type_str("T::Hash[String, Integer]"),
            "Hash[String, Integer]"
        );
        assert_eq!(convert_sorbet_type_str("T::Array[String]"), "Array[String]");
    }

    #[test]
    fn parse_method_chain_tolerates_unbalanced_parens() {
        // A half-typed `params(` (open paren at the very end) used to slice
        // `chars[arg_start..pos - 1]` with begin > end and panic.
        for s in [
            "params(",
            "void.params(",
            "params(x: ",
            "returns(",
            "(",
            "a(",
        ] {
            let _ = parse_method_chain(s);
        }
        // The closing paren is still excluded from captured args.
        let chain = parse_method_chain("params(x: Integer)");
        assert_eq!(
            chain,
            vec![("params".to_string(), Some("x: Integer".to_string()))]
        );
    }
}

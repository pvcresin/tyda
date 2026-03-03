use std::io::{self, Write};

use crate::registry::TypeRegistry;
use crate::types::{ClassInfo, ConstantSig, MethodAliasSig, MethodSig, Param, ParamKind, Type};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderOptions {
    pub include_synthetic_dsl_methods: bool,
}

pub fn render_rbs(registry: &TypeRegistry) -> String {
    render_rbs_with_options(registry, RenderOptions::default())
}

pub fn render_rbs_with_options(registry: &TypeRegistry, options: RenderOptions) -> String {
    let mut output = Vec::new();
    render_rbs_to_writer_with_options(registry, options, &mut output)
        .expect("rendering RBS into Vec<u8> should not fail");
    String::from_utf8(output).expect("rendered RBS must be valid UTF-8")
}

pub fn render_rbs_to_writer_with_options<W: Write>(
    registry: &TypeRegistry,
    options: RenderOptions,
    writer: &mut W,
) -> io::Result<()> {
    write_rbs_from_registry(registry, options, writer, None)
}

pub fn render_rbs_to_writer_in_pool<W: Write>(
    registry: &TypeRegistry,
    options: RenderOptions,
    writer: &mut W,
    pool: Option<&rayon::ThreadPool>,
) -> io::Result<()> {
    write_rbs_from_registry(registry, options, writer, pool)
}

pub fn render_rbs_for_file(registry: &TypeRegistry, file_path: &str) -> String {
    render_rbs_for_file_with_options(registry, file_path, RenderOptions::default())
}

pub fn render_rbs_for_file_with_options(
    registry: &TypeRegistry,
    file_path: &str,
    options: RenderOptions,
) -> String {
    let mut output = Vec::new();
    write_rbs_from_classes(
        registry.build_classes_for_file(file_path),
        options,
        &mut output,
    )
    .expect("rendering RBS into Vec<u8> should not fail");
    String::from_utf8(output).expect("rendered RBS must be valid UTF-8")
}

fn write_rbs_from_classes<W: Write>(
    classes: Vec<ClassInfo>,
    options: RenderOptions,
    writer: &mut W,
) -> io::Result<()> {
    let mut first_entry = true;

    let top_level_constants: &[ConstantSig] = classes
        .iter()
        .find(|class| class.name == "Object")
        .map(|class| class.constants.as_slice())
        .unwrap_or(&[]);
    if !top_level_constants.is_empty() {
        first_entry = false;
    }
    for constant in top_level_constants {
        writeln!(writer, "{}: {}", constant.name, constant.const_type)?;
    }

    let non_empty: Vec<_> = classes
        .iter()
        .filter(|class| class_has_renderable_body(class, options))
        .collect();
    for class in non_empty {
        if !first_entry {
            writer.write_all(b"\n")?;
        }
        first_entry = false;
        write_class_body(writer, class, options)?;
    }

    Ok(())
}

fn write_rbs_from_registry<W: Write>(
    registry: &TypeRegistry,
    options: RenderOptions,
    writer: &mut W,
    pool: Option<&rayon::ThreadPool>,
) -> io::Result<()> {
    let mut first_entry = true;

    let top_level_constants = registry.build_output_top_level_constants();
    if !top_level_constants.is_empty() {
        first_entry = false;
    }
    for constant in &top_level_constants {
        writeln!(writer, "{}: {}", constant.name, constant.const_type)?;
    }

    // Chunked parallel render writes classes out in order, so output is byte-stable (the `resolve_params` cache is pure).
    const RENDER_CHUNK_SIZE: usize = 256;
    let class_names = registry.output_class_names();
    for chunk in class_names.chunks(RENDER_CHUNK_SIZE) {
        let render_chunk = || -> Vec<Option<Vec<u8>>> {
            use rayon::prelude::*;
            chunk
                .par_iter()
                .map(|class_name| {
                    let class = registry.build_output_class_info(class_name)?;
                    if !class_has_renderable_body(&class, options) {
                        return None;
                    }
                    let mut buf = Vec::with_capacity(class_render_capacity_hint(&class));
                    write_class_body(&mut buf, &class, options).ok()?;
                    Some(buf)
                })
                .collect()
        };
        let rendered = match pool {
            Some(pool) => pool.install(render_chunk),
            None => render_chunk(),
        };
        for buf in rendered.into_iter().flatten() {
            if !first_entry {
                writer.write_all(b"\n")?;
            }
            first_entry = false;
            writer.write_all(&buf)?;
        }
    }

    Ok(())
}

/// Rough upper-bound byte estimate for a class's rendered body, so the per-class buffer
/// grows once via `with_capacity` instead of repeatedly reallocating as `write!` fills it.
/// Deliberately coarse (no type-size inspection): cheap to compute per class, and a mildly
/// generous estimate wastes far less than a handful of `Vec` grow-and-copy cycles per class.
fn class_render_capacity_hint(class: &ClassInfo) -> usize {
    const HEADER_BYTES: usize = 32;
    const PER_MIXIN_BYTES: usize = 24;
    const PER_CONSTANT_BYTES: usize = 40;
    const PER_METHOD_BYTES: usize = 72;
    const PER_ALIAS_BYTES: usize = 40;
    HEADER_BYTES
        + class.mixins.len() * PER_MIXIN_BYTES
        + class.constants.len() * PER_CONSTANT_BYTES
        + class.methods.len() * PER_METHOD_BYTES
        + class.aliases.len() * PER_ALIAS_BYTES
}

fn write_class_body<W: Write>(
    writer: &mut W,
    class: &ClassInfo,
    options: RenderOptions,
) -> io::Result<()> {
    for comment in &class.sorbet_modifier_comments {
        writeln!(writer, "{comment}")?;
    }
    let keyword = if class.is_module { "module" } else { "class" };
    write_class_header(writer, keyword, class)?;
    for (kind, module_name) in &class.mixins {
        writeln!(writer, "  {kind} {module_name}")?;
    }
    let renderable_constants = renderable_constants(class);
    let renderable_methods = renderable_methods(class, options);
    if !class.mixins.is_empty()
        && (!renderable_constants.is_empty() || !renderable_methods.is_empty())
    {
        writer.write_all(b"\n")?;
    }
    for constant in renderable_constants {
        writeln!(writer, "  {}: {}", constant.name, constant.const_type)?;
    }
    if !renderable_constants.is_empty() && !renderable_methods.is_empty() {
        writer.write_all(b"\n")?;
    }
    for method in renderable_methods {
        for comment in &method.sorbet_modifier_comments {
            writeln!(writer, "  {comment}")?;
        }
        write_method_sig(writer, method)?;
    }
    for alias in &class.aliases {
        write_alias(writer, alias)?;
    }
    writer.write_all(b"end\n")
}

fn write_alias<W: Write>(writer: &mut W, alias: &MethodAliasSig) -> io::Result<()> {
    if alias.is_singleton {
        writeln!(
            writer,
            "  alias self.{} self.{}",
            alias.new_name, alias.old_name
        )
    } else {
        writeln!(writer, "  alias {} {}", alias.new_name, alias.old_name)
    }
}

// Without the generic declaration `[T]`, methods referencing `T` would produce invalid RBS.
fn write_class_header<W: Write>(
    writer: &mut W,
    keyword: &str,
    class: &ClassInfo,
) -> io::Result<()> {
    if class.type_params.is_empty() {
        match &class.superclass {
            Some(superclass) => writeln!(writer, "{keyword} {} < {superclass}", class.name),
            None => writeln!(writer, "{keyword} {}", class.name),
        }
    } else {
        let params = class.type_params.join(", ");
        match &class.superclass {
            Some(superclass) => {
                writeln!(writer, "{keyword} {}[{params}] < {superclass}", class.name)
            }
            None => writeln!(writer, "{keyword} {}[{params}]", class.name),
        }
    }
}

fn class_has_renderable_body(class: &ClassInfo, options: RenderOptions) -> bool {
    !class.sorbet_modifier_comments.is_empty()
        || !class.mixins.is_empty()
        || !renderable_constants(class).is_empty()
        || class
            .methods
            .iter()
            .any(|method| is_renderable_method(method, options))
}

fn renderable_constants(class: &ClassInfo) -> &[ConstantSig] {
    if class.name == "Object" {
        &[]
    } else {
        class.constants.as_slice()
    }
}

fn is_renderable_method(method: &MethodSig, options: RenderOptions) -> bool {
    !method.rbs_file_source || options.include_synthetic_dsl_methods && method.synthetic_dsl_source
}

fn renderable_methods(class: &ClassInfo, options: RenderOptions) -> Vec<&MethodSig> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for method in class
        .methods
        .iter()
        .rev()
        .filter(|method| is_renderable_method(method, options))
    {
        let key = (method.name.as_str(), method.is_singleton);
        if seen.insert(key) {
            deduped.push(method);
        }
    }
    deduped.reverse();
    deduped
}

/// Writes `"  "`-indented spaces without allocating (the common case is a handful of
/// spaces per overload continuation line; long ones fall back to a few extra writes).
fn write_spaces<W: Write>(writer: &mut W, mut n: usize) -> io::Result<()> {
    const SPACES: &[u8] = b"                                                                ";
    while n > 0 {
        let take = n.min(SPACES.len());
        writer.write_all(&SPACES[..take])?;
        n -= take;
    }
    Ok(())
}

fn write_method_sig<W: Write>(writer: &mut W, method: &MethodSig) -> io::Result<()> {
    // RBS has no `protected`, so it's rendered as private.
    let visibility = if method.is_private { "private " } else { "" };
    let prefix = if method.is_singleton {
        "def self."
    } else {
        "def "
    };
    write!(writer, "  {visibility}{prefix}{}: ", method.name)?;
    write_signature(
        writer,
        &method.params,
        method.block.as_ref(),
        &method.return_type,
    )?;
    writeln!(writer)?;
    if method.overloads.is_empty() {
        return Ok(());
    }
    // continuation lines align `|` under the start of the primary signature.
    let header_len = visibility.len() + prefix.len() + method.name.len();
    for overload in &method.overloads {
        write_spaces(writer, header_len + 2)?;
        writer.write_all(b"| ")?;
        write_signature(
            writer,
            &overload.params,
            overload.block.as_ref(),
            &overload.return_type,
        )?;
        writeln!(writer)?;
    }
    Ok(())
}

fn write_signature<W: Write>(
    writer: &mut W,
    params: &[Param],
    block: Option<&crate::types::HoverBlockSig>,
    return_type: &Type,
) -> io::Result<()> {
    let mut has_params = false;
    let mut first = true;
    for param in params {
        if matches!(param.kind, ParamKind::Block) && block.is_some() {
            continue;
        }
        if !has_params {
            writer.write_all(b"(")?;
        }
        has_params = true;
        if !first {
            writer.write_all(b", ")?;
        }
        first = false;
        write_param(writer, param)?;
    }
    if has_params {
        writer.write_all(b")")?;
    } else if let Some(block) = block {
        write_block_signature_trimmed(writer, block)?;
        writer.write_all(b" -> ")?;
        return write_return_type(writer, return_type, false);
    } else {
        writer.write_all(b"-> ")?;
        return write_return_type(writer, return_type, false);
    }
    if let Some(block) = block {
        write_block_signature(writer, block)?;
    }
    writer.write_all(b" -> ")?;
    write_return_type(writer, return_type, true)
}

fn write_param<W: Write>(writer: &mut W, param: &Param) -> io::Result<()> {
    let widened = param.param_type.widen();
    match param.kind {
        ParamKind::Required => {
            write!(writer, "{widened:#}")?;
            if !param.name.is_empty() {
                write!(writer, " {}", param.name)?;
            }
        }
        ParamKind::Optional => {
            write!(writer, "?{widened:#}")?;
            if !param.name.is_empty() {
                write!(writer, " {}", param.name)?;
            }
        }
        ParamKind::Rest => {
            write!(writer, "*{widened:#}")?;
            if !param.name.is_empty() {
                write!(writer, " {}", param.name)?;
            }
        }
        ParamKind::KeywordRequired => write!(writer, "{}: {widened:#}", param.name)?,
        ParamKind::KeywordOptional => write!(writer, "?{}: {widened:#}", param.name)?,
        ParamKind::DoubleRest => {
            write!(writer, "**{widened:#}")?;
            if !param.name.is_empty() {
                write!(writer, " {}", param.name)?;
            }
        }
        ParamKind::Block => write!(writer, "?{widened:#} &{}", param.name)?,
    }
    Ok(())
}

fn write_block_signature<W: Write>(
    writer: &mut W,
    block: &crate::types::HoverBlockSig,
) -> io::Result<()> {
    writer.write_all(if block.required { b" " } else { b" ?" })?;
    write_block_signature_body(writer, block)
}

fn write_block_signature_trimmed<W: Write>(
    writer: &mut W,
    block: &crate::types::HoverBlockSig,
) -> io::Result<()> {
    if !block.required {
        writer.write_all(b"?")?;
    }
    write_block_signature_body(writer, block)
}

fn write_block_signature_body<W: Write>(
    writer: &mut W,
    block: &crate::types::HoverBlockSig,
) -> io::Result<()> {
    writer.write_all(b"{ (")?;
    for (i, param) in block.params.iter().enumerate() {
        if i > 0 {
            writer.write_all(b", ")?;
        }
        write!(writer, "{:#}", param.param_type.widen())?;
    }
    writer.write_all(b") -> ")?;
    write_return_type(writer, &block.return_type, !block.params.is_empty())?;
    writer.write_all(b" }")
}

fn write_return_type<W: Write>(writer: &mut W, ty: &Type, has_params: bool) -> io::Result<()> {
    if has_params {
        write!(writer, "{ty:#}")
    } else {
        write!(writer, "{ty}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_formats_empty_tuple_as_rbs_empty_tuple() {
        let mut buf = Vec::new();
        write_signature(&mut buf, &[], None, &Type::Tuple(Vec::new())).unwrap();
        let rendered = String::from_utf8(buf).unwrap();

        assert_eq!(rendered, "-> [ ]");
        assert!(
            rbs_sys::parse_signature(&format!("class A\n  def empty: {rendered}\nend\n")).is_ok()
        );
    }
}

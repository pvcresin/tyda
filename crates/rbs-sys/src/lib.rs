#![allow(
    non_camel_case_types,
    non_snake_case,
    dead_code,
    unsafe_op_in_unsafe_fn
)]

use std::collections::{HashMap, HashSet};
use std::ffi::c_int;
use std::os::raw::c_char;
use std::ptr;

// Parser types/enums/statics and most functions come from official ruby-rbs-sys (bindgen).
mod ffi {
    pub use ruby_rbs_sys::bindings::*;

    // bindgen does not emit a `_t` alias for `rbs_hash`; supply one.
    pub type rbs_hash_t = rbs_hash;

    // attr_reader/writer/accessor, include/extend/prepend, and variable members share layouts,
    // so alias each family to one representative type.
    pub type rbs_ast_members_attr_t = rbs_ast_members_attr_reader_t;
    pub type rbs_ast_members_mixin_t = rbs_ast_members_include_t;
    pub type rbs_ast_members_variable_t = rbs_ast_members_instance_variable_t;

    // bindgen nests enum consts under ModuleConsts; re-export the ones we use as `ffi::RBS_*`.
    pub use ruby_rbs_sys::bindings::rbs_alias_kind::RBS_ALIAS_KIND_SINGLETON;
    pub use ruby_rbs_sys::bindings::rbs_attr_ivar_name_tag::{
        RBS_ATTR_IVAR_NAME_TAG_EMPTY, RBS_ATTR_IVAR_NAME_TAG_NAME,
        RBS_ATTR_IVAR_NAME_TAG_UNSPECIFIED,
    };
    pub use ruby_rbs_sys::bindings::rbs_attribute_kind::RBS_ATTRIBUTE_KIND_SINGLETON;
    pub use ruby_rbs_sys::bindings::rbs_method_definition_kind::{
        RBS_METHOD_DEFINITION_KIND_SINGLETON, RBS_METHOD_DEFINITION_KIND_SINGLETON_INSTANCE,
    };

    // Supplement parse helpers missing from ruby-rbs-sys 0.3.0's bindgen allowlist.
    // C symbols are still linked by ruby-rbs-sys; hand-written signatures are safe because
    // they stay stable across versions (unlike struct layouts).
    unsafe extern "C" {
        pub fn rbs_parse_method_type(
            parser: *mut rbs_parser_t,
            method_type: *mut *mut rbs_method_type_t,
            require_eof: bool,
            classish_allowed: bool,
        ) -> bool;

        pub fn rbs_parse_type(
            parser: *mut rbs_parser_t,
            ty: *mut *mut rbs_node_t,
            void_allowed: bool,
            self_allowed: bool,
            classish_allowed: bool,
        ) -> bool;

        pub fn rbs_parse_inline_leading_annotation(
            parser: *mut rbs_parser_t,
            annotation: *mut *mut rbs_node_t,
        ) -> bool;
    }

    // On wasm32, bindgen cannot emit bitfield-heavy `rbs_constant_pool_bucket_t`, so
    // pool/parser types and related fns drop out. tyda only needs opaque handles and a
    // tiny stable `rbs_constant_t` (start/length), so supplement those on wasm only.
    #[cfg(target_arch = "wasm32")]
    mod wasm_supplement {
        // Opaque handle; tyda only passes `*mut`, never reads fields.
        #[repr(C)]
        pub struct rbs_parser_t {
            _private: [u8; 0],
        }
        // Used for PartialParser's last field (address only) and id_to_constant args.
        // Opaque ZST is enough because offsets are fixed by preceding fields.
        #[repr(C)]
        pub struct rbs_constant_pool_t {
            _private: [u8; 0],
        }
        // Only type whose layout tyda reads (start/length); stable across versions.
        #[repr(C)]
        pub struct rbs_constant_t {
            pub start: *const u8,
            pub length: usize,
        }
        // Note: bindgen still emits `rbs_constant_id_t` on wasm, so do not redefine it.
    }
    #[cfg(target_arch = "wasm32")]
    pub use wasm_supplement::*;

    // Supplement parse/handle fns bindgen drops on wasm (host uses bindgen output).
    // Arg types like `rbs_string_t` still come from bindgen on wasm.
    #[cfg(target_arch = "wasm32")]
    unsafe extern "C" {
        pub fn rbs_string_new(
            start: *const std::os::raw::c_char,
            end: *const std::os::raw::c_char,
        ) -> rbs_string_t;
        pub fn rbs_parser_new(
            string: rbs_string_t,
            encoding: *const rbs_encoding_t,
            start_pos: std::ffi::c_int,
            end_pos: std::ffi::c_int,
        ) -> *mut rbs_parser_t;
        pub fn rbs_parser_free(parser: *mut rbs_parser_t);
        pub fn rbs_parse_signature(
            parser: *mut rbs_parser_t,
            signature: *mut *mut rbs_signature_t,
        ) -> bool;
        pub fn rbs_constant_pool_id_to_constant(
            pool: *const rbs_constant_pool_t,
            constant_id: rbs_constant_id_t,
        ) -> *mut rbs_constant_t;
    }
}

// ── Safe Rust API ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbsRecordKey {
    Symbol(std::string::String),
    String(std::string::String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbsRecordField {
    pub key: RbsRecordKey,
    pub type_: RbsType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbsType {
    Integer,
    Float,
    String,
    Symbol,
    Bool,
    Nil,
    Void,
    Untyped,
    Top,
    Bottom,
    SelfType,
    ClassType,
    InstanceType,
    Class(std::string::String, Vec<RbsType>),
    Singleton(std::string::String),
    Union(Vec<RbsType>),
    Intersection(Vec<RbsType>),
    Optional(Box<RbsType>),
    Tuple(Vec<RbsType>),
    Record(Vec<RbsRecordField>),
    Proc(Box<MethodType>),
    Variable(std::string::String),
    Alias(std::string::String, Vec<RbsType>),
    Literal(std::string::String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionParam {
    pub type_: RbsType,
    pub name: Option<std::string::String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    pub required_positionals: Vec<FunctionParam>,
    pub optional_positionals: Vec<FunctionParam>,
    pub rest_positionals: Option<FunctionParam>,
    pub trailing_positionals: Vec<FunctionParam>,
    pub required_keywords: Vec<(std::string::String, FunctionParam)>,
    pub optional_keywords: Vec<(std::string::String, FunctionParam)>,
    pub rest_keywords: Option<FunctionParam>,
    pub return_type: RbsType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockType {
    pub function_type: FunctionType,
    pub required: bool,
    pub self_type: Option<RbsType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodType {
    pub function_type: FunctionType,
    pub block: Option<BlockType>,
    pub self_type: Option<RbsType>,
    pub type_params: Vec<std::string::String>,
    pub type_param_bounds: Vec<(std::string::String, RbsType)>,
    pub type_param_lower_bounds: Vec<(std::string::String, RbsType)>,
    pub annotations: Vec<std::string::String>,
}

#[derive(Debug)]
pub enum ParseError {
    ParseFailed,
    NullPointer,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::ParseFailed => write!(f, "RBS parse failed"),
            ParseError::NullPointer => write!(f, "Unexpected null pointer in RBS AST"),
        }
    }
}

impl std::error::Error for ParseError {}

struct Parser {
    parser: *mut ffi::rbs_parser_t,
}

impl Parser {
    fn new(input: &str) -> Self {
        let start = input.as_ptr() as *const c_char;
        let end = unsafe { start.add(input.len()) };
        let string = unsafe { ffi::rbs_string_new(start, end) };
        let encoding = unsafe { ffi::rbs_encodings.as_ptr() };
        let parser = unsafe { ffi::rbs_parser_new(string, encoding, 0, input.len() as c_int) };
        Self { parser }
    }
}

impl Drop for Parser {
    fn drop(&mut self) {
        unsafe { ffi::rbs_parser_free(self.parser) };
    }
}

/// Parse a method type string like `(String, Integer) -> bool`.
pub fn parse_method_type(input: &str) -> Result<MethodType, ParseError> {
    let parser = Parser::new(input);
    let mut method_type_ptr: *mut ffi::rbs_method_type_t = ptr::null_mut();
    let ok = unsafe { ffi::rbs_parse_method_type(parser.parser, &mut method_type_ptr, true, true) };
    if !ok || method_type_ptr.is_null() {
        return Err(ParseError::ParseFailed);
    }
    unsafe { convert_method_type(method_type_ptr, parser.parser, &UseContext::default()) }
}

/// Parse a type string like `String | Integer`, `Array[String]`, etc.
pub fn parse_type(input: &str) -> Result<RbsType, ParseError> {
    let parser = Parser::new(input);
    let mut type_ptr: *mut ffi::rbs_node_t = ptr::null_mut();
    let ok = unsafe { ffi::rbs_parse_type(parser.parser, &mut type_ptr, true, true, true) };
    if !ok || type_ptr.is_null() {
        return Err(ParseError::ParseFailed);
    }
    unsafe { convert_type_node(type_ptr, parser.parser, &UseContext::default()) }
}

/// Parse a `#:` style inline annotation, returning the method type.
/// Input should be the comment body _without_ the `#` prefix, e.g. `": (String) -> Integer"`.
pub fn parse_inline_leading(input: &str) -> Result<MethodType, ParseError> {
    let all = parse_inline_all_overloads(input)?;
    all.into_iter().next().ok_or(ParseError::ParseFailed)
}

/// Parse a `#:` style inline annotation, returning all overloaded method types.
pub fn parse_inline_all_overloads(input: &str) -> Result<Vec<MethodType>, ParseError> {
    let parser = Parser::new(input);
    let mut annotation_ptr: *mut ffi::rbs_node_t = ptr::null_mut();
    let ok =
        unsafe { ffi::rbs_parse_inline_leading_annotation(parser.parser, &mut annotation_ptr) };
    if !ok || annotation_ptr.is_null() {
        return Err(ParseError::ParseFailed);
    }
    unsafe {
        let node = &*annotation_ptr;
        match node.type_ {
            ffi::rbs_node_type::RBS_AST_RUBY_ANNOTATIONS_COLON_METHOD_TYPE_ANNOTATION => {
                let annotation = &*(annotation_ptr
                    as *const ffi::rbs_ast_ruby_annotations_colon_method_type_annotation_t);
                if annotation.method_type.is_null() {
                    return Err(ParseError::NullPointer);
                }
                let mt = &*(annotation.method_type as *const ffi::rbs_method_type_t);
                let result = convert_method_type(
                    mt as *const _ as *mut _,
                    parser.parser,
                    &UseContext::default(),
                )?;
                Ok(vec![result])
            }
            ffi::rbs_node_type::RBS_AST_RUBY_ANNOTATIONS_METHOD_TYPES_ANNOTATION => {
                let annotation = &*(annotation_ptr
                    as *const ffi::rbs_ast_ruby_annotations_method_types_annotation_t);
                if annotation.overloads.is_null() {
                    return Err(ParseError::NullPointer);
                }
                let overloads = &*annotation.overloads;
                if overloads.length == 0 || overloads.head.is_null() {
                    return Err(ParseError::ParseFailed);
                }
                let mut results = Vec::new();
                let mut cur = overloads.head;
                while !cur.is_null() {
                    let entry = &*cur;
                    if !entry.node.is_null() {
                        let overload_node = &*entry.node;
                        if overload_node.type_
                            == ffi::rbs_node_type::RBS_AST_MEMBERS_METHOD_DEFINITION_OVERLOAD
                        {
                            let overload = &*(entry.node
                                as *const ffi::rbs_ast_members_method_definition_overload_t);
                            if !overload.method_type.is_null() {
                                let mt = &*(overload.method_type as *const ffi::rbs_method_type_t);
                                if let Ok(method_type) = convert_method_type(
                                    mt as *const _ as *mut _,
                                    parser.parser,
                                    &UseContext::default(),
                                ) {
                                    results.push(method_type);
                                }
                            }
                        }
                    }
                    cur = entry.next;
                }
                if results.is_empty() {
                    Err(ParseError::ParseFailed)
                } else {
                    Ok(results)
                }
            }
            _ => Err(ParseError::ParseFailed),
        }
    }
}

// ── Signature types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, Default)]
struct UseContext {
    single_aliases: HashMap<std::string::String, std::string::String>,
    wildcard_namespaces: Vec<std::string::String>,
    known_type_names: HashSet<std::string::String>,
    current_scope: Option<std::string::String>,
}

impl UseContext {
    fn with_current_scope(&self, scope: Option<std::string::String>) -> Self {
        let mut next = self.clone();
        next.current_scope = scope
            .map(normalize_type_name)
            .filter(|name| !name.is_empty());
        next
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MixinKind {
    Include,
    Extend,
    Prepend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixinDecl {
    pub name: std::string::String,
    pub args: Vec<RbsType>,
    pub kind: MixinKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfTypeDecl {
    pub name: std::string::String,
    pub args: Vec<RbsType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableKind {
    Instance,
    ClassInstance,
    Class,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableDecl {
    pub name: std::string::String,
    pub type_: RbsType,
    pub kind: VariableKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    Class {
        name: std::string::String,
        methods: Vec<MethodDecl>,
        aliases: Vec<MethodAliasDecl>,
        superclass: Option<std::string::String>,
        superclass_args: Vec<RbsType>,
        type_params: Vec<std::string::String>,
        type_param_bounds: Vec<(std::string::String, RbsType)>,
        type_param_defaults: Vec<(std::string::String, RbsType)>,
        mixins: Vec<MixinDecl>,
        variables: Vec<VariableDecl>,
    },
    Module {
        name: std::string::String,
        methods: Vec<MethodDecl>,
        aliases: Vec<MethodAliasDecl>,
        type_params: Vec<std::string::String>,
        type_param_bounds: Vec<(std::string::String, RbsType)>,
        type_param_defaults: Vec<(std::string::String, RbsType)>,
        self_types: Vec<SelfTypeDecl>,
        mixins: Vec<MixinDecl>,
        variables: Vec<VariableDecl>,
    },
    Interface {
        name: std::string::String,
        methods: Vec<MethodDecl>,
        aliases: Vec<MethodAliasDecl>,
        type_params: Vec<std::string::String>,
        type_param_bounds: Vec<(std::string::String, RbsType)>,
        type_param_defaults: Vec<(std::string::String, RbsType)>,
        mixins: Vec<MixinDecl>,
    },
    ClassAlias {
        new_name: std::string::String,
        old_name: std::string::String,
    },
    ModuleAlias {
        new_name: std::string::String,
        old_name: std::string::String,
    },
    /// `Foo: Type` constant declaration (top-level or inside a module).
    Constant {
        name: std::string::String,
        type_: RbsType,
    },
    /// `$foo: Type` global variable declaration.
    Global {
        name: std::string::String,
        type_: RbsType,
    },
    TypeAlias {
        name: std::string::String,
        type_params: Vec<std::string::String>,
        type_param_bounds: Vec<(std::string::String, RbsType)>,
        type_param_defaults: Vec<(std::string::String, RbsType)>,
        type_: RbsType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDecl {
    pub name: std::string::String,
    pub method_types: Vec<MethodType>,
    pub kind: MethodKind,
    pub attr_ivar: Option<std::string::String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodAliasDecl {
    pub new_name: std::string::String,
    pub old_name: std::string::String,
    pub kind: MethodKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodKind {
    Instance,
    Singleton,
    SingletonInstance,
}

/// Parse a full RBS signature (one or more class/module declarations).
pub fn parse_signature(input: &str) -> Result<Signature, ParseError> {
    let parser = Parser::new(input);
    let mut sig_ptr: *mut ffi::rbs_signature_t = ptr::null_mut();
    let ok = unsafe { ffi::rbs_parse_signature(parser.parser, &mut sig_ptr) };
    if !ok || sig_ptr.is_null() {
        return Err(ParseError::ParseFailed);
    }
    unsafe { convert_signature(sig_ptr, parser.parser) }
}

unsafe fn convert_signature(
    sig: *mut ffi::rbs_signature_t,
    parser: *mut ffi::rbs_parser_t,
) -> Result<Signature, ParseError> {
    let sig_ref = &*sig;
    let mut declarations = Vec::new();
    let use_context = build_use_context(sig_ref, parser);

    if !sig_ref.declarations.is_null() {
        let decls = &*sig_ref.declarations;
        let mut cur = decls.head;
        while !cur.is_null() {
            let entry = &*cur;
            if !entry.node.is_null() {
                let node = &*entry.node;
                match node.type_ {
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_CLASS => {
                        if let Ok(mut decl) = convert_class_decl(entry.node, parser, &use_context) {
                            collect_nested_declarations(
                                entry.node,
                                &mut decl,
                                &mut declarations,
                                parser,
                                &use_context,
                            );
                            declarations.push(decl);
                        }
                    }
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_CLASS_ALIAS => {
                        if let Some(decl) =
                            convert_class_alias_decl(entry.node, parser, None, &use_context)
                        {
                            declarations.push(decl);
                        }
                    }
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE => {
                        if let Ok(mut decl) = convert_module_decl(entry.node, parser, &use_context)
                        {
                            collect_nested_declarations(
                                entry.node,
                                &mut decl,
                                &mut declarations,
                                parser,
                                &use_context,
                            );
                            declarations.push(decl);
                        }
                    }
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE_ALIAS => {
                        if let Some(decl) =
                            convert_module_alias_decl(entry.node, parser, None, &use_context)
                        {
                            declarations.push(decl);
                        }
                    }
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_CONSTANT => {
                        if let Some(decl) =
                            convert_constant_decl(entry.node, parser, None, &use_context)
                        {
                            declarations.push(decl);
                        }
                    }
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_GLOBAL => {
                        if let Some(decl) = convert_global_decl(entry.node, parser, &use_context) {
                            declarations.push(decl);
                        }
                    }
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_INTERFACE => {
                        if let Ok(decl) = convert_interface_decl(entry.node, parser, &use_context) {
                            declarations.push(decl);
                        }
                    }
                    ffi::rbs_node_type::RBS_AST_DECLARATIONS_TYPE_ALIAS => {
                        if let Some(decl) =
                            convert_type_alias_decl(entry.node, parser, None, &use_context)
                        {
                            declarations.push(decl);
                        }
                    }
                    _ => {}
                }
            }
            cur = entry.next;
        }
    }

    Ok(Signature { declarations })
}

unsafe fn build_use_context(
    sig_ref: &ffi::rbs_signature_t,
    parser: *mut ffi::rbs_parser_t,
) -> UseContext {
    let mut context = UseContext::default();
    collect_known_type_names(
        sig_ref.declarations,
        parser,
        None,
        &mut context.known_type_names,
    );
    collect_use_directives(sig_ref.directives, parser, &mut context);
    context
}

unsafe fn collect_use_directives(
    directives: *mut ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    context: &mut UseContext,
) {
    if directives.is_null() {
        return;
    }
    let directives_ref = &*directives;
    let mut cur = directives_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_DIRECTIVES_USE {
                let directive = &*(entry.node as *const ffi::rbs_ast_directives_use_t);
                collect_use_clauses(directive.clauses, parser, context);
            }
        }
        cur = (*cur).next;
    }
}

unsafe fn collect_use_clauses(
    clauses: *mut ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    context: &mut UseContext,
) {
    if clauses.is_null() {
        return;
    }
    let clauses_ref = &*clauses;
    let mut cur = clauses_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            match node.type_ {
                ffi::rbs_node_type::RBS_AST_DIRECTIVES_USE_SINGLE_CLAUSE => {
                    let clause =
                        &*(entry.node as *const ffi::rbs_ast_directives_use_single_clause_t);
                    let target = normalize_type_name(resolve_type_name(clause.type_name, parser));
                    if target.is_empty() {
                        cur = entry.next;
                        continue;
                    }
                    let alias = if clause.new_name.is_null() {
                        target
                            .rsplit("::")
                            .next()
                            .unwrap_or(target.as_str())
                            .to_string()
                    } else {
                        resolve_symbol(clause.new_name, parser)
                    };
                    if !alias.is_empty() {
                        context.single_aliases.insert(alias, target);
                    }
                }
                ffi::rbs_node_type::RBS_AST_DIRECTIVES_USE_WILDCARD_CLAUSE => {
                    let clause =
                        &*(entry.node as *const ffi::rbs_ast_directives_use_wildcard_clause_t);
                    let namespace =
                        normalize_type_name(resolve_namespace(clause.rbs_namespace, parser));
                    if !namespace.is_empty() {
                        context.wildcard_namespaces.push(namespace);
                    }
                }
                _ => {}
            }
        }
        cur = (*cur).next;
    }
}

unsafe fn collect_known_type_names(
    declarations: *mut ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: Option<&str>,
    known: &mut HashSet<std::string::String>,
) {
    if declarations.is_null() {
        return;
    }
    let declarations_ref = &*declarations;
    let mut cur = declarations_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        collect_known_type_name_from_node(entry.node, parser, outer_name, known);
        cur = (*cur).next;
    }
}

unsafe fn collect_known_type_name_from_node(
    node_ptr: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: Option<&str>,
    known: &mut HashSet<std::string::String>,
) {
    if node_ptr.is_null() {
        return;
    }
    let node = &*node_ptr;
    match node.type_ {
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_CLASS => {
            let class = &*(node_ptr as *const ffi::rbs_ast_declarations_class_t);
            let name = raw_decl_name(class.name, parser, outer_name);
            known.insert(name.clone());
            collect_known_type_names_from_members(class.members, parser, &name, known);
        }
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_CLASS_ALIAS => {
            let alias = &*(node_ptr as *const ffi::rbs_ast_declarations_class_alias_t);
            known.insert(raw_decl_name(alias.new_name, parser, outer_name));
        }
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE => {
            let module_ = &*(node_ptr as *const ffi::rbs_ast_declarations_module_t);
            let name = raw_decl_name(module_.name, parser, outer_name);
            known.insert(name.clone());
            collect_known_type_names_from_members(module_.members, parser, &name, known);
        }
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE_ALIAS => {
            let alias = &*(node_ptr as *const ffi::rbs_ast_declarations_module_alias_t);
            known.insert(raw_decl_name(alias.new_name, parser, outer_name));
        }
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_INTERFACE => {
            let interface = &*(node_ptr as *const ffi::rbs_ast_declarations_interface_t);
            let name = raw_decl_name(interface.name, parser, outer_name);
            known.insert(name);
        }
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_TYPE_ALIAS => {
            let alias = &*(node_ptr as *const ffi::rbs_ast_declarations_type_alias_t);
            let name = raw_decl_name(alias.name, parser, outer_name);
            known.insert(name);
        }
        _ => {}
    }
}

unsafe fn collect_known_type_names_from_members(
    members: *mut ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: &str,
    known: &mut HashSet<std::string::String>,
) {
    if members.is_null() {
        return;
    }
    let members_ref = &*members;
    let mut cur = members_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        collect_known_type_name_from_node(entry.node, parser, Some(outer_name), known);
        cur = (*cur).next;
    }
}

unsafe fn raw_decl_name(
    name: *const ffi::rbs_type_name_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: Option<&str>,
) -> std::string::String {
    let raw = resolve_type_name(name, parser);
    // Absolute names (`::...`) stay unscoped; relative names—even path form like `HTTP::Post`—
    // get the enclosing scope prepended.
    let is_absolute = raw.starts_with("::");
    let name = normalize_type_name(raw);
    match outer_name {
        Some(outer) if !is_absolute => format!("{outer}::{name}"),
        _ => name,
    }
}

/// Walk the members list of a class / module declaration and extract any
/// nested class / module declarations into `top_level_decls`, qualifying the
/// names with the outer declaration's name.
unsafe fn collect_nested_declarations(
    outer_node: *mut ffi::rbs_node_t,
    outer_decl: &mut Declaration,
    top_level_decls: &mut Vec<Declaration>,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) {
    let outer_name = match outer_decl {
        Declaration::Class { name, .. } | Declaration::Module { name, .. } => name.clone(),
        Declaration::Interface { .. }
        | Declaration::ClassAlias { .. }
        | Declaration::ModuleAlias { .. }
        | Declaration::Constant { .. }
        | Declaration::Global { .. }
        | Declaration::TypeAlias { .. } => return,
    };
    let outer_ref = &*outer_node;
    let members = match outer_ref.type_ {
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_CLASS => {
            let class = &*(outer_node as *const ffi::rbs_ast_declarations_class_t);
            class.members
        }
        ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE => {
            let module_ = &*(outer_node as *const ffi::rbs_ast_declarations_module_t);
            module_.members
        }
        _ => return,
    };
    if members.is_null() {
        return;
    }
    let member_context = use_context.with_current_scope(Some(outer_name.clone()));
    let members_ref = &*members;
    let mut cur = members_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            match node.type_ {
                ffi::rbs_node_type::RBS_AST_DECLARATIONS_CLASS => {
                    if let Ok(mut inner) = convert_class_decl(entry.node, parser, &member_context) {
                        qualify_decl_name(&mut inner, &outer_name);
                        collect_nested_declarations(
                            entry.node,
                            &mut inner,
                            top_level_decls,
                            parser,
                            &member_context,
                        );
                        top_level_decls.push(inner);
                    }
                }
                ffi::rbs_node_type::RBS_AST_DECLARATIONS_CLASS_ALIAS => {
                    if let Some(inner) = convert_class_alias_decl(
                        entry.node,
                        parser,
                        Some(&outer_name),
                        &member_context,
                    ) {
                        top_level_decls.push(inner);
                    }
                }
                ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE => {
                    if let Ok(mut inner) = convert_module_decl(entry.node, parser, &member_context)
                    {
                        qualify_decl_name(&mut inner, &outer_name);
                        collect_nested_declarations(
                            entry.node,
                            &mut inner,
                            top_level_decls,
                            parser,
                            &member_context,
                        );
                        top_level_decls.push(inner);
                    }
                }
                ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE_ALIAS => {
                    if let Some(inner) = convert_module_alias_decl(
                        entry.node,
                        parser,
                        Some(&outer_name),
                        &member_context,
                    ) {
                        top_level_decls.push(inner);
                    }
                }
                ffi::rbs_node_type::RBS_AST_DECLARATIONS_CONSTANT => {
                    if let Some(inner) = convert_constant_decl(
                        entry.node,
                        parser,
                        Some(&outer_name),
                        &member_context,
                    ) {
                        top_level_decls.push(inner);
                    }
                }
                ffi::rbs_node_type::RBS_AST_DECLARATIONS_INTERFACE => {
                    if let Ok(mut inner) =
                        convert_interface_decl(entry.node, parser, &member_context)
                    {
                        qualify_decl_name(&mut inner, &outer_name);
                        top_level_decls.push(inner);
                    }
                }
                ffi::rbs_node_type::RBS_AST_DECLARATIONS_TYPE_ALIAS => {
                    if let Some(inner) = convert_type_alias_decl(
                        entry.node,
                        parser,
                        Some(&outer_name),
                        &member_context,
                    ) {
                        top_level_decls.push(inner);
                    }
                }
                _ => {}
            }
        }
        cur = entry.next;
    }
}

fn declaration_member_scope(name: &str, current_scope: Option<&str>) -> std::string::String {
    // Absolute names (`::...`) are top-level after normalize. Relative names—even path form
    // (`HTTP::Post`)—get the enclosing scope (`module Net` → `Net::HTTP::Post` for members/super).
    if name.starts_with("::") {
        return normalize_type_name(name.to_string());
    }
    let name = normalize_type_name(name.to_string());
    if let Some(scope) = current_scope {
        format!("{scope}::{name}")
    } else {
        name
    }
}

fn resolve_type_name_in_current_scope(
    name: &str,
    use_context: &UseContext,
) -> Option<std::string::String> {
    let mut scope = use_context.current_scope.as_deref();
    while let Some(scope_name) = scope {
        let candidate = format!("{scope_name}::{name}");
        if use_context.known_type_names.contains(&candidate) {
            return Some(candidate);
        }
        scope = scope_name.rsplit_once("::").map(|(parent, _)| parent);
    }
    None
}

unsafe fn convert_constant_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: Option<&str>,
    use_context: &UseContext,
) -> Option<Declaration> {
    let const_node = &*(node as *const ffi::rbs_ast_declarations_constant_t);
    let name = resolve_type_name(const_node.name, parser);
    if name.is_empty() {
        return None;
    }
    // Prepend enclosing scope for relative names, including path form (`HTTP::STATUS_CODES`).
    // Absolute names (`::...`) stay unscoped.
    let qualified_name = match outer_name {
        Some(outer) if !name.starts_with("::") => {
            format!("{}::{}", outer, name)
        }
        _ => name,
    };
    if const_node.type_.is_null() {
        return None;
    }
    let type_ = convert_type_node(const_node.type_, parser, use_context).ok()?;
    Some(Declaration::Constant {
        name: qualified_name,
        type_,
    })
}

unsafe fn convert_global_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Option<Declaration> {
    let global_node = &*(node as *const ffi::rbs_ast_declarations_global_t);
    let name = resolve_symbol(global_node.name, parser);
    if name.is_empty() || global_node.type_.is_null() {
        return None;
    }
    let type_ = convert_type_node(global_node.type_, parser, use_context).ok()?;
    Some(Declaration::Global { name, type_ })
}

unsafe fn convert_type_alias_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: Option<&str>,
    use_context: &UseContext,
) -> Option<Declaration> {
    let alias_node = &*(node as *const ffi::rbs_ast_declarations_type_alias_t);
    let name = resolve_type_name(alias_node.name, parser);
    if name.is_empty() || alias_node.type_.is_null() {
        return None;
    }
    // Prepend enclosing scope for relative names, including path form (`HTTP::STATUS_CODES`).
    // Absolute names (`::...`) stay unscoped.
    let qualified_name = match outer_name {
        Some(outer) if !name.starts_with("::") => {
            format!("{}::{}", outer, name)
        }
        _ => name,
    };
    let type_params = extract_type_param_names(alias_node.type_params, parser);
    let type_param_bounds =
        extract_type_param_upper_bounds(alias_node.type_params, parser, use_context);
    let type_param_defaults =
        extract_type_param_defaults(alias_node.type_params, parser, use_context);
    let type_ = convert_type_node(alias_node.type_, parser, use_context).ok()?;
    Some(Declaration::TypeAlias {
        name: qualified_name,
        type_params,
        type_param_bounds,
        type_param_defaults,
        type_,
    })
}

unsafe fn convert_class_alias_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: Option<&str>,
    use_context: &UseContext,
) -> Option<Declaration> {
    let alias = &*(node as *const ffi::rbs_ast_declarations_class_alias_t);
    let new_name = raw_decl_name(alias.new_name, parser, outer_name);
    let old_name = resolve_type_name_with_use(alias.old_name, parser, use_context);
    if new_name.is_empty() || old_name.is_empty() {
        return None;
    }
    Some(Declaration::ClassAlias {
        new_name,
        old_name: normalize_type_name(old_name),
    })
}

unsafe fn convert_module_alias_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    outer_name: Option<&str>,
    use_context: &UseContext,
) -> Option<Declaration> {
    let alias = &*(node as *const ffi::rbs_ast_declarations_module_alias_t);
    let new_name = raw_decl_name(alias.new_name, parser, outer_name);
    let old_name = resolve_type_name_with_use(alias.old_name, parser, use_context);
    if new_name.is_empty() || old_name.is_empty() {
        return None;
    }
    Some(Declaration::ModuleAlias {
        new_name,
        old_name: normalize_type_name(old_name),
    })
}

fn qualify_decl_name(decl: &mut Declaration, outer_name: &str) {
    let name = match decl {
        Declaration::Class { name, .. }
        | Declaration::Module { name, .. }
        | Declaration::Interface { name, .. }
        | Declaration::Constant { name, .. }
        | Declaration::TypeAlias { name, .. } => name,
        Declaration::ClassAlias { .. }
        | Declaration::ModuleAlias { .. }
        | Declaration::Global { .. } => return,
    };
    // Absolute names (`::...`) stay as-is. Relative path names (`HTTP::Post`) still get
    // the enclosing scope: `module Net` + `class HTTP::Post` defines `Net::HTTP::Post`.
    if name.starts_with("::") {
        return;
    }
    *name = format!("{}::{}", outer_name, name);
}

unsafe fn convert_class_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Declaration, ParseError> {
    let class = &*(node as *const ffi::rbs_ast_declarations_class_t);
    let name = resolve_type_name(class.name, parser);
    let member_context = use_context.with_current_scope(Some(declaration_member_scope(
        &name,
        use_context.current_scope.as_deref(),
    )));

    let (superclass, superclass_args) = if class.super_class.is_null() {
        (None, Vec::new())
    } else {
        let sup = &*class.super_class;
        let sup_name = resolve_type_name_with_use(sup.name, parser, &member_context);
        if sup_name.is_empty() || sup_name == "Unknown" {
            (None, Vec::new())
        } else {
            let sup_args = convert_type_list(sup.args, parser, &member_context)?;
            (Some(sup_name), sup_args)
        }
    };

    let members = extract_members(class.members, parser, &member_context)?;
    let type_params = extract_type_param_names(class.type_params, parser);
    let type_param_bounds =
        extract_type_param_upper_bounds(class.type_params, parser, &member_context);
    let type_param_defaults =
        extract_type_param_defaults(class.type_params, parser, &member_context);

    Ok(Declaration::Class {
        name,
        methods: members.methods,
        aliases: members.aliases,
        superclass,
        superclass_args,
        type_params,
        type_param_bounds,
        type_param_defaults,
        mixins: members.mixins,
        variables: members.variables,
    })
}

unsafe fn convert_module_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Declaration, ParseError> {
    let module = &*(node as *const ffi::rbs_ast_declarations_module_t);
    let name = resolve_type_name(module.name, parser);
    let member_context = use_context.with_current_scope(Some(declaration_member_scope(
        &name,
        use_context.current_scope.as_deref(),
    )));
    let members = extract_members(module.members, parser, &member_context)?;
    let type_params = extract_type_param_names(module.type_params, parser);
    let type_param_bounds =
        extract_type_param_upper_bounds(module.type_params, parser, &member_context);
    let type_param_defaults =
        extract_type_param_defaults(module.type_params, parser, &member_context);
    let self_types = extract_module_self_types(module.self_types, parser, &member_context);

    Ok(Declaration::Module {
        name,
        methods: members.methods,
        aliases: members.aliases,
        type_params,
        type_param_bounds,
        type_param_defaults,
        self_types,
        mixins: members.mixins,
        variables: members.variables,
    })
}

unsafe fn convert_interface_decl(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Declaration, ParseError> {
    let interface = &*(node as *const ffi::rbs_ast_declarations_interface_t);
    let name = resolve_type_name(interface.name, parser);
    let member_context = use_context.with_current_scope(Some(declaration_member_scope(
        &name,
        use_context.current_scope.as_deref(),
    )));
    let members = extract_members(interface.members, parser, &member_context)?;
    let type_params = extract_type_param_names(interface.type_params, parser);
    let type_param_bounds =
        extract_type_param_upper_bounds(interface.type_params, parser, &member_context);
    let type_param_defaults =
        extract_type_param_defaults(interface.type_params, parser, &member_context);

    Ok(Declaration::Interface {
        name,
        methods: members.methods,
        aliases: members.aliases,
        type_params,
        type_param_bounds,
        type_param_defaults,
        mixins: members.mixins,
    })
}

struct ExtractedMembers {
    methods: Vec<MethodDecl>,
    aliases: Vec<MethodAliasDecl>,
    mixins: Vec<MixinDecl>,
    variables: Vec<VariableDecl>,
}

unsafe fn extract_members(
    members: *mut ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<ExtractedMembers, ParseError> {
    let mut methods = Vec::new();
    let mut aliases = Vec::new();
    let mut mixins = Vec::new();
    let mut variables = Vec::new();
    if members.is_null() {
        return Ok(ExtractedMembers {
            methods,
            aliases,
            mixins,
            variables,
        });
    }

    let members_ref = &*members;
    let mut cur = members_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_MEMBERS_METHOD_DEFINITION
                && let Ok(method) = convert_method_definition(entry.node, parser, use_context)
            {
                methods.push(method);
            } else if node.type_ == ffi::rbs_node_type::RBS_AST_MEMBERS_ALIAS {
                let alias_node = &*(entry.node as *const ffi::rbs_ast_members_alias_t);
                let new_name = resolve_symbol(alias_node.new_name, parser);
                let old_name = resolve_symbol(alias_node.old_name, parser);
                if !new_name.is_empty() && !old_name.is_empty() {
                    aliases.push(MethodAliasDecl {
                        new_name,
                        old_name,
                        kind: method_kind_from_alias_kind(alias_node.kind),
                    });
                }
            } else if matches!(
                node.type_,
                ffi::rbs_node_type::RBS_AST_MEMBERS_ATTR_READER
                    | ffi::rbs_node_type::RBS_AST_MEMBERS_ATTR_WRITER
                    | ffi::rbs_node_type::RBS_AST_MEMBERS_ATTR_ACCESSOR
            ) {
                methods.extend(convert_attr_member(
                    entry.node,
                    node.type_,
                    parser,
                    use_context,
                )?);
            } else if matches!(
                node.type_,
                ffi::rbs_node_type::RBS_AST_MEMBERS_INCLUDE
                    | ffi::rbs_node_type::RBS_AST_MEMBERS_EXTEND
                    | ffi::rbs_node_type::RBS_AST_MEMBERS_PREPEND
            ) {
                if let Some(mixin) =
                    convert_mixin_member(entry.node, node.type_, parser, use_context)
                {
                    mixins.push(mixin);
                }
            } else if matches!(
                node.type_,
                ffi::rbs_node_type::RBS_AST_MEMBERS_INSTANCE_VARIABLE
                    | ffi::rbs_node_type::RBS_AST_MEMBERS_CLASS_INSTANCE_VARIABLE
                    | ffi::rbs_node_type::RBS_AST_MEMBERS_CLASS_VARIABLE
            ) && let Some(variable) =
                convert_variable_member(entry.node, node.type_, parser, use_context)
            {
                variables.push(variable);
            }
        }
        cur = entry.next;
    }

    Ok(ExtractedMembers {
        methods,
        aliases,
        mixins,
        variables,
    })
}

unsafe fn extract_module_self_types(
    list: *mut ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Vec<SelfTypeDecl> {
    let mut result = Vec::new();
    if list.is_null() {
        return result;
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_DECLARATIONS_MODULE_SELF {
                let self_type = &*(entry.node as *const ffi::rbs_ast_declarations_module_self_t);
                let name = resolve_type_name_with_use(self_type.name, parser, use_context);
                if !name.is_empty() {
                    let args = convert_type_list(self_type.args, parser, use_context)
                        .unwrap_or_else(|_| Vec::new());
                    result.push(SelfTypeDecl { name, args });
                }
            }
        }
        cur = entry.next;
    }
    result
}

unsafe fn convert_mixin_member(
    node: *mut ffi::rbs_node_t,
    node_type: ffi::rbs_node_type::Type,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Option<MixinDecl> {
    let mixin_node = &*(node as *const ffi::rbs_ast_members_mixin_t);
    let name = resolve_type_name_with_use(mixin_node.name, parser, use_context);
    if name.is_empty() {
        return None;
    }
    let args = convert_type_list(mixin_node.args, parser, use_context).ok()?;
    let kind = match node_type {
        ffi::rbs_node_type::RBS_AST_MEMBERS_EXTEND => MixinKind::Extend,
        ffi::rbs_node_type::RBS_AST_MEMBERS_PREPEND => MixinKind::Prepend,
        _ => MixinKind::Include,
    };
    Some(MixinDecl { name, args, kind })
}

unsafe fn convert_variable_member(
    node: *mut ffi::rbs_node_t,
    node_type: ffi::rbs_node_type::Type,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Option<VariableDecl> {
    let variable_node = &*(node as *const ffi::rbs_ast_members_variable_t);
    let name = resolve_symbol(variable_node.name, parser);
    if name.is_empty() || variable_node.type_.is_null() {
        return None;
    }
    let type_ = convert_type_node(variable_node.type_, parser, use_context).ok()?;
    let kind = match node_type {
        ffi::rbs_node_type::RBS_AST_MEMBERS_CLASS_INSTANCE_VARIABLE => VariableKind::ClassInstance,
        ffi::rbs_node_type::RBS_AST_MEMBERS_CLASS_VARIABLE => VariableKind::Class,
        _ => VariableKind::Instance,
    };
    Some(VariableDecl { name, type_, kind })
}

unsafe fn convert_attr_member(
    node: *mut ffi::rbs_node_t,
    node_type: ffi::rbs_node_type::Type,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Vec<MethodDecl>, ParseError> {
    let attr = &*(node as *const ffi::rbs_ast_members_attr_t);
    let name = resolve_symbol(attr.name, parser);
    if name.is_empty() || attr.type_.is_null() {
        return Ok(Vec::new());
    }
    let type_ = convert_type_node(attr.type_, parser, use_context)?;
    let method_kind = method_kind_from_attr_kind(attr.kind);
    let attr_ivar = resolve_attr_ivar_name(&name, attr.ivar_name, parser);
    let mut methods = Vec::new();
    let reader = matches!(
        node_type,
        ffi::rbs_node_type::RBS_AST_MEMBERS_ATTR_READER
            | ffi::rbs_node_type::RBS_AST_MEMBERS_ATTR_ACCESSOR
    );
    let writer = matches!(
        node_type,
        ffi::rbs_node_type::RBS_AST_MEMBERS_ATTR_WRITER
            | ffi::rbs_node_type::RBS_AST_MEMBERS_ATTR_ACCESSOR
    );
    if reader {
        methods.push(MethodDecl {
            name: name.clone(),
            method_types: vec![method_type_no_args(type_.clone())],
            kind: method_kind.clone(),
            attr_ivar: attr_ivar.clone(),
        });
    }
    if writer {
        methods.push(MethodDecl {
            name: format!("{name}="),
            method_types: vec![method_type_one_required(type_.clone(), "value", type_)],
            kind: method_kind,
            attr_ivar,
        });
    }
    Ok(methods)
}

unsafe fn resolve_attr_ivar_name(
    attr_name: &str,
    ivar_name: ffi::rbs_attr_ivar_name_t,
    parser: *mut ffi::rbs_parser_t,
) -> Option<std::string::String> {
    match ivar_name.tag {
        ffi::RBS_ATTR_IVAR_NAME_TAG_UNSPECIFIED => Some(format!("@{attr_name}")),
        ffi::RBS_ATTR_IVAR_NAME_TAG_EMPTY => None,
        ffi::RBS_ATTR_IVAR_NAME_TAG_NAME => {
            let pool = get_constant_pool(parser);
            let constant = ffi::rbs_constant_pool_id_to_constant(pool, ivar_name.name);
            if constant.is_null() {
                return None;
            }
            let c = &*constant;
            if c.start.is_null() || c.length == 0 {
                return None;
            }
            let slice = std::slice::from_raw_parts(c.start, c.length);
            Some(std::string::String::from_utf8_lossy(slice).to_string())
        }
        _ => Some(format!("@{attr_name}")),
    }
}

fn method_kind_from_attr_kind(kind: ffi::rbs_attribute_kind::Type) -> MethodKind {
    if kind == ffi::RBS_ATTRIBUTE_KIND_SINGLETON {
        MethodKind::Singleton
    } else {
        MethodKind::Instance
    }
}

fn method_kind_from_alias_kind(kind: ffi::rbs_alias_kind::Type) -> MethodKind {
    if kind == ffi::RBS_ALIAS_KIND_SINGLETON {
        MethodKind::Singleton
    } else {
        MethodKind::Instance
    }
}

fn empty_function_type(return_type: RbsType) -> FunctionType {
    FunctionType {
        required_positionals: Vec::new(),
        optional_positionals: Vec::new(),
        rest_positionals: None,
        trailing_positionals: Vec::new(),
        required_keywords: Vec::new(),
        optional_keywords: Vec::new(),
        rest_keywords: None,
        return_type,
    }
}

fn untyped_function_type(return_type: RbsType) -> FunctionType {
    FunctionType {
        required_positionals: Vec::new(),
        optional_positionals: Vec::new(),
        rest_positionals: Some(FunctionParam {
            type_: RbsType::Untyped,
            name: Some("args".to_string()),
        }),
        trailing_positionals: Vec::new(),
        required_keywords: Vec::new(),
        optional_keywords: Vec::new(),
        rest_keywords: Some(FunctionParam {
            type_: RbsType::Untyped,
            name: Some("kwargs".to_string()),
        }),
        return_type,
    }
}

fn method_type_no_args(return_type: RbsType) -> MethodType {
    MethodType {
        function_type: empty_function_type(return_type),
        block: None,
        self_type: None,
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        type_param_lower_bounds: Vec::new(),
        annotations: Vec::new(),
    }
}

fn method_type_one_required(
    param_type: RbsType,
    param_name: &str,
    return_type: RbsType,
) -> MethodType {
    MethodType {
        function_type: FunctionType {
            required_positionals: vec![FunctionParam {
                type_: param_type,
                name: Some(param_name.to_string()),
            }],
            ..empty_function_type(return_type)
        },
        block: None,
        self_type: None,
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        type_param_lower_bounds: Vec::new(),
        annotations: Vec::new(),
    }
}

unsafe fn convert_method_definition(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<MethodDecl, ParseError> {
    let method_def = &*(node as *const ffi::rbs_ast_members_method_definition_t);
    let name = resolve_symbol(method_def.name, parser);

    let kind = if method_def.kind == ffi::RBS_METHOD_DEFINITION_KIND_SINGLETON {
        MethodKind::Singleton
    } else if method_def.kind == ffi::RBS_METHOD_DEFINITION_KIND_SINGLETON_INSTANCE {
        MethodKind::SingletonInstance
    } else {
        MethodKind::Instance
    };

    let mut method_types = Vec::new();
    if !method_def.overloads.is_null() {
        let overloads = &*method_def.overloads;
        let mut cur = overloads.head;
        while !cur.is_null() {
            let entry = &*cur;
            if !entry.node.is_null() {
                let overload_node = &*entry.node;
                if overload_node.type_
                    == ffi::rbs_node_type::RBS_AST_MEMBERS_METHOD_DEFINITION_OVERLOAD
                {
                    let overload =
                        &*(entry.node as *const ffi::rbs_ast_members_method_definition_overload_t);
                    if !overload.method_type.is_null() {
                        let mt = &*(overload.method_type as *const ffi::rbs_method_type_t);
                        if let Ok(mut converted) =
                            convert_method_type(mt as *const _ as *mut _, parser, use_context)
                        {
                            converted.annotations =
                                collect_annotations(overload.annotations, parser);
                            method_types.push(converted);
                        }
                    }
                }
            }
            cur = entry.next;
        }
    }

    Ok(MethodDecl {
        name,
        method_types,
        kind,
        attr_ivar: None,
    })
}

// ── AST conversion helpers ─────────────────────────────────────────────────

unsafe fn get_constant_pool(parser: *mut ffi::rbs_parser_t) -> *const ffi::rbs_constant_pool_t {
    // The constant_pool field is at a known offset inside rbs_parser_t.
    // rbs_parser_t layout:
    //   lexer: *mut rbs_lexer_t         (8 bytes)
    //   current_token: rbs_token_t      (20 bytes)
    //   next_token: rbs_token_t         (20 bytes)
    //   next_token2: rbs_token_t        (20 bytes)
    //   next_token3: rbs_token_t        (20 bytes)
    //   vars: *mut id_table             (8 bytes)
    //   last_comment: *mut rbs_comment_t (8 bytes)
    //   constant_pool: rbs_constant_pool_t
    //
    // rbs_token_t = { type: enum (4 bytes) + range: rbs_range_t (4*4 * 2 = 32 bytes) }
    //   Actually rbs_range_t = { start: rbs_position_t, end: rbs_position_t }
    //   rbs_position_t = { byte_pos: int, char_pos: int, line: int, column: int } = 16 bytes
    //   So rbs_range_t = 32 bytes
    //   rbs_token_t = { type: 4 bytes + padding(4) + range: 32 bytes } = 40 bytes? or
    //   Actually enum is c_int = 4 bytes. rbs_range_t has rbs_position_t start (16 bytes) + end (16 bytes) = 32 bytes.
    //   rbs_token_t = { type: c_int (4 bytes), range: { start: {4*4=16}, end: {4*4=16} } = 32 } = 36 bytes.
    //   But there might be padding... Let's compute this at build time instead.

    // Alternative approach: cast to bytes and use offsetof-like calculations.
    // Actually, the parser struct is defined in parser.h and is NOT opaque - it's fully declared.
    // We can define a matching Rust struct.
    // But that's fragile. Instead, let's extract the constant pool pointer differently.

    // We stored a reference to the parser, and we can access constant_pool through the parser.
    // Since rbs_parser_t is fully defined in parser.h, let's define the Rust repr.

    // Actually, the cleanest approach: just define a subset of the parser struct that gets us
    // to the constant_pool field. We need all fields before it to be correct size.
    //
    // Let me use a simpler approach: read the parser as our PartialParser struct.

    let partial = &*(parser as *const PartialParser);
    &partial.constant_pool as *const _
}

/// Partial representation of rbs_parser_t to access constant_pool.
/// Must match the exact layout of the C struct up to and including constant_pool.
#[repr(C)]
struct PartialParser {
    lexer: *mut (),
    // current_token, next_token, next_token2, next_token3: 4 x rbs_token_t
    current_token: RbsTokenRepr,
    next_token: RbsTokenRepr,
    next_token2: RbsTokenRepr,
    next_token3: RbsTokenRepr,
    vars: *mut (),
    last_comment: *mut (),
    constant_pool: ffi::rbs_constant_pool_t,
}

/// Matches rbs_token_t = { enum type (c_int), rbs_range_t range }
/// rbs_range_t = { rbs_position_t start, rbs_position_t end }
/// rbs_position_t = { int byte_pos, int char_pos, int line, int column }
#[repr(C)]
#[derive(Copy, Clone)]
struct RbsTokenRepr {
    type_: c_int,
    range: RbsRangeRepr,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RbsRangeRepr {
    start: RbsPositionRepr,
    end: RbsPositionRepr,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RbsPositionRepr {
    byte_pos: c_int,
    char_pos: c_int,
    line: c_int,
    column: c_int,
}

unsafe fn resolve_symbol(
    sym: *const ffi::rbs_ast_symbol_t,
    parser: *mut ffi::rbs_parser_t,
) -> std::string::String {
    if sym.is_null() {
        return std::string::String::new();
    }
    let sym_ref = &*sym;
    let pool = get_constant_pool(parser);
    let constant = ffi::rbs_constant_pool_id_to_constant(pool, sym_ref.constant_id);
    if constant.is_null() {
        return std::string::String::new();
    }
    let c = &*constant;
    if c.start.is_null() || c.length == 0 {
        return std::string::String::new();
    }
    let slice = std::slice::from_raw_parts(c.start, c.length);
    std::string::String::from_utf8_lossy(slice).to_string()
}

unsafe fn resolve_type_name(
    tn: *const ffi::rbs_type_name_t,
    parser: *mut ffi::rbs_parser_t,
) -> std::string::String {
    if tn.is_null() {
        return "Unknown".to_string();
    }
    let tn_ref = &*tn;
    let mut parts = Vec::new();

    if !tn_ref.rbs_namespace.is_null() {
        let ns = &*tn_ref.rbs_namespace;
        if ns.absolute {
            parts.push(std::string::String::new());
        }
        if !ns.path.is_null() {
            let path_list = &*ns.path;
            let mut cur = path_list.head;
            while !cur.is_null() {
                let entry = &*cur;
                if !entry.node.is_null() {
                    let node = &*entry.node;
                    if node.type_ == ffi::rbs_node_type::RBS_AST_SYMBOL {
                        let sym = entry.node as *const ffi::rbs_ast_symbol_t;
                        parts.push(resolve_symbol(sym, parser));
                    }
                }
                cur = entry.next;
            }
        }
    }

    let name = resolve_symbol(tn_ref.name, parser);
    parts.push(name);

    parts.join("::")
}

unsafe fn resolve_type_name_with_use(
    tn: *const ffi::rbs_type_name_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> std::string::String {
    let name = resolve_type_name(tn, parser);
    if name.starts_with("::") {
        return name;
    }
    if !name.contains("::")
        && let Some(mapped) = use_context.single_aliases.get(&name)
    {
        return mapped.clone();
    }
    if let Some(mapped) = resolve_type_name_in_current_scope(&name, use_context) {
        return mapped;
    }
    if !name.contains("::") {
        for namespace in &use_context.wildcard_namespaces {
            let candidate = format!("{namespace}::{name}");
            if use_context.known_type_names.contains(&candidate) {
                return candidate;
            }
        }
    }
    name
}

unsafe fn resolve_namespace(
    ns: *const ffi::rbs_namespace_t,
    parser: *mut ffi::rbs_parser_t,
) -> std::string::String {
    if ns.is_null() {
        return std::string::String::new();
    }
    let ns_ref = &*ns;
    let mut parts = Vec::new();
    if ns_ref.absolute {
        parts.push(std::string::String::new());
    }
    if !ns_ref.path.is_null() {
        let path_list = &*ns_ref.path;
        let mut cur = path_list.head;
        while !cur.is_null() {
            let entry = &*cur;
            if !entry.node.is_null() {
                let node = &*entry.node;
                if node.type_ == ffi::rbs_node_type::RBS_AST_SYMBOL {
                    parts.push(resolve_symbol(
                        entry.node as *const ffi::rbs_ast_symbol_t,
                        parser,
                    ));
                }
            }
            cur = entry.next;
        }
    }
    parts.join("::")
}

fn normalize_type_name(name: std::string::String) -> std::string::String {
    name.trim_start_matches("::").to_string()
}

unsafe fn extract_type_param_names(
    list: *const ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
) -> Vec<std::string::String> {
    let mut result = Vec::new();
    if list.is_null() {
        return result;
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_TYPE_PARAM {
                let tp = &*(entry.node as *const ffi::rbs_ast_type_param_t);
                let name = resolve_symbol(tp.name, parser);
                if !name.is_empty() {
                    result.push(name);
                }
            }
        }
        cur = entry.next;
    }
    result
}

unsafe fn extract_type_param_upper_bounds(
    list: *const ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Vec<(std::string::String, RbsType)> {
    let mut result = Vec::new();
    if list.is_null() {
        return result;
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_TYPE_PARAM {
                let tp = &*(entry.node as *const ffi::rbs_ast_type_param_t);
                let name = resolve_symbol(tp.name, parser);
                if !name.is_empty()
                    && !tp.upper_bound.is_null()
                    && let Ok(bound) = convert_type_node(tp.upper_bound, parser, use_context)
                {
                    result.push((name, bound));
                }
            }
        }
        cur = entry.next;
    }
    result
}

unsafe fn extract_type_param_lower_bounds(
    list: *const ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Vec<(std::string::String, RbsType)> {
    let mut result = Vec::new();
    if list.is_null() {
        return result;
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_TYPE_PARAM {
                let tp = &*(entry.node as *const ffi::rbs_ast_type_param_t);
                let name = resolve_symbol(tp.name, parser);
                if !name.is_empty()
                    && !tp.lower_bound.is_null()
                    && let Ok(bound) = convert_type_node(tp.lower_bound, parser, use_context)
                {
                    result.push((name, bound));
                }
            }
        }
        cur = entry.next;
    }
    result
}

unsafe fn extract_type_param_defaults(
    list: *const ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Vec<(std::string::String, RbsType)> {
    let mut result = Vec::new();
    if list.is_null() {
        return result;
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_TYPE_PARAM {
                let tp = &*(entry.node as *const ffi::rbs_ast_type_param_t);
                let name = resolve_symbol(tp.name, parser);
                if !name.is_empty()
                    && !tp.default_type.is_null()
                    && let Ok(default_type) =
                        convert_type_node(tp.default_type, parser, use_context)
                {
                    result.push((name, default_type));
                }
            }
        }
        cur = entry.next;
    }
    result
}

unsafe fn convert_method_type(
    mt: *mut ffi::rbs_method_type_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<MethodType, ParseError> {
    let mt_ref = &*mt;

    if mt_ref.type_.is_null() {
        return Err(ParseError::NullPointer);
    }

    let type_node = &*mt_ref.type_;

    let function_type = match type_node.type_ {
        ffi::rbs_node_type::RBS_TYPES_FUNCTION => convert_function_type(
            mt_ref.type_ as *const ffi::rbs_types_function_t,
            parser,
            use_context,
        )?,
        ffi::rbs_node_type::RBS_TYPES_UNTYPED_FUNCTION => convert_untyped_function_type(
            mt_ref.type_ as *const ffi::rbs_types_untyped_function_t,
            parser,
            use_context,
        )?,
        _ => return Err(ParseError::ParseFailed),
    };

    let block = if mt_ref.block.is_null() {
        None
    } else {
        Some(convert_block_type(mt_ref.block, parser, use_context)?)
    };

    let type_params = extract_type_param_names(mt_ref.type_params, parser);
    let type_param_bounds =
        extract_type_param_upper_bounds(mt_ref.type_params, parser, use_context);
    let type_param_lower_bounds =
        extract_type_param_lower_bounds(mt_ref.type_params, parser, use_context);

    Ok(MethodType {
        function_type,
        block,
        self_type: None,
        type_params,
        type_param_bounds,
        type_param_lower_bounds,
        annotations: Vec::new(),
    })
}

unsafe fn convert_block_type(
    block: *const ffi::rbs_types_block_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<BlockType, ParseError> {
    let block_ref = &*block;

    if block_ref.type_.is_null() {
        return Err(ParseError::NullPointer);
    }

    let type_node = &*block_ref.type_;
    let function_type = match type_node.type_ {
        ffi::rbs_node_type::RBS_TYPES_FUNCTION => convert_function_type(
            block_ref.type_ as *const ffi::rbs_types_function_t,
            parser,
            use_context,
        )?,
        ffi::rbs_node_type::RBS_TYPES_UNTYPED_FUNCTION => convert_untyped_function_type(
            block_ref.type_ as *const ffi::rbs_types_untyped_function_t,
            parser,
            use_context,
        )?,
        _ => return Err(ParseError::ParseFailed),
    };

    Ok(BlockType {
        function_type,
        required: block_ref.required,
        self_type: if block_ref.self_type.is_null() {
            None
        } else {
            Some(convert_type_node(block_ref.self_type, parser, use_context)?)
        },
    })
}

unsafe fn convert_function_type(
    func: *const ffi::rbs_types_function_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<FunctionType, ParseError> {
    let f = &*func;

    let required_positionals = convert_param_list(f.required_positionals, parser, use_context)?;
    let optional_positionals = convert_param_list(f.optional_positionals, parser, use_context)?;
    let trailing_positionals = convert_param_list(f.trailing_positionals, parser, use_context)?;

    let rest_positionals = if f.rest_positionals.is_null() {
        None
    } else {
        let node = &*f.rest_positionals;
        if node.type_ == ffi::rbs_node_type::RBS_TYPES_FUNCTION_PARAM {
            Some(convert_function_param(
                f.rest_positionals as *const ffi::rbs_types_function_param_t,
                parser,
                use_context,
            )?)
        } else {
            None
        }
    };

    let rest_keywords = if f.rest_keywords.is_null() {
        None
    } else {
        let node = &*f.rest_keywords;
        if node.type_ == ffi::rbs_node_type::RBS_TYPES_FUNCTION_PARAM {
            Some(convert_function_param(
                f.rest_keywords as *const ffi::rbs_types_function_param_t,
                parser,
                use_context,
            )?)
        } else {
            None
        }
    };

    let required_keywords = convert_keyword_hash(f.required_keywords, parser, use_context)?;
    let optional_keywords = convert_keyword_hash(f.optional_keywords, parser, use_context)?;

    let return_type = if f.return_type.is_null() {
        RbsType::Void
    } else {
        convert_type_node(f.return_type, parser, use_context)?
    };

    Ok(FunctionType {
        required_positionals,
        optional_positionals,
        rest_positionals,
        trailing_positionals,
        required_keywords,
        optional_keywords,
        rest_keywords,
        return_type,
    })
}

unsafe fn convert_untyped_function_type(
    func: *const ffi::rbs_types_untyped_function_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<FunctionType, ParseError> {
    let f = &*func;
    let return_type = if f.return_type.is_null() {
        RbsType::Void
    } else {
        convert_type_node(f.return_type, parser, use_context)?
    };
    Ok(untyped_function_type(return_type))
}

unsafe fn convert_param_list(
    list: *const ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Vec<FunctionParam>, ParseError> {
    let mut result = Vec::new();
    if list.is_null() {
        return Ok(result);
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let param = convert_function_param(
                entry.node as *const ffi::rbs_types_function_param_t,
                parser,
                use_context,
            )?;
            result.push(param);
        }
        cur = entry.next;
    }
    Ok(result)
}

unsafe fn convert_function_param(
    param: *const ffi::rbs_types_function_param_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<FunctionParam, ParseError> {
    let p = &*param;
    let type_ = if p.type_.is_null() {
        RbsType::Untyped
    } else {
        convert_type_node(p.type_, parser, use_context)?
    };
    let name = if p.name.is_null() {
        None
    } else {
        let s = resolve_symbol(p.name, parser);
        if s.is_empty() { None } else { Some(s) }
    };
    Ok(FunctionParam { type_, name })
}

unsafe fn convert_keyword_hash(
    hash: *const ffi::rbs_hash_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Vec<(std::string::String, FunctionParam)>, ParseError> {
    let mut result = Vec::new();
    if hash.is_null() {
        return Ok(result);
    }
    let hash_ref = &*hash;
    let mut cur = hash_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.key.is_null() && !entry.value.is_null() {
            let key_node = &*entry.key;
            let key_name = if key_node.type_ == ffi::rbs_node_type::RBS_AST_SYMBOL {
                resolve_symbol(entry.key as *const ffi::rbs_ast_symbol_t, parser)
            } else {
                std::string::String::new()
            };

            let value_node = &*entry.value;
            let param = if value_node.type_ == ffi::rbs_node_type::RBS_TYPES_FUNCTION_PARAM {
                convert_function_param(
                    entry.value as *const ffi::rbs_types_function_param_t,
                    parser,
                    use_context,
                )?
            } else {
                FunctionParam {
                    type_: convert_type_node(entry.value, parser, use_context)?,
                    name: None,
                }
            };

            if !key_name.is_empty() {
                result.push((key_name, param));
            }
        }
        cur = entry.next;
    }
    Ok(result)
}

unsafe fn convert_type_node(
    node: *mut ffi::rbs_node_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<RbsType, ParseError> {
    if node.is_null() {
        return Err(ParseError::NullPointer);
    }
    let n = &*node;
    match n.type_ {
        ffi::rbs_node_type::RBS_TYPES_BASES_BOOL => Ok(RbsType::Bool),
        ffi::rbs_node_type::RBS_TYPES_BASES_NIL => Ok(RbsType::Nil),
        ffi::rbs_node_type::RBS_TYPES_BASES_VOID => Ok(RbsType::Void),
        ffi::rbs_node_type::RBS_TYPES_BASES_ANY => Ok(RbsType::Untyped),
        ffi::rbs_node_type::RBS_TYPES_BASES_TOP => Ok(RbsType::Top),
        ffi::rbs_node_type::RBS_TYPES_BASES_BOTTOM => Ok(RbsType::Bottom),
        ffi::rbs_node_type::RBS_TYPES_BASES_SELF => Ok(RbsType::SelfType),
        ffi::rbs_node_type::RBS_TYPES_BASES_CLASS => Ok(RbsType::ClassType),
        ffi::rbs_node_type::RBS_TYPES_BASES_INSTANCE => Ok(RbsType::InstanceType),

        ffi::rbs_node_type::RBS_TYPES_CLASS_INSTANCE => {
            let ci = &*(node as *const ffi::rbs_types_class_instance_t);
            let name = resolve_type_name_with_use(ci.name, parser, use_context);
            let args = convert_type_list(ci.args, parser, use_context)?;
            Ok(RbsType::Class(name, args))
        }

        ffi::rbs_node_type::RBS_TYPES_CLASS_SINGLETON => {
            let cs = &*(node as *const ffi::rbs_types_class_singleton_t);
            let name = resolve_type_name_with_use(cs.name, parser, use_context);
            Ok(RbsType::Singleton(name))
        }

        ffi::rbs_node_type::RBS_TYPES_UNION => {
            let u = &*(node as *const ffi::rbs_types_union_t);
            let types = convert_type_list(u.types, parser, use_context)?;
            Ok(RbsType::Union(types))
        }

        ffi::rbs_node_type::RBS_TYPES_INTERSECTION => {
            let i = &*(node as *const ffi::rbs_types_intersection_t);
            let types = convert_type_list(i.types, parser, use_context)?;
            Ok(RbsType::Intersection(types))
        }

        ffi::rbs_node_type::RBS_TYPES_OPTIONAL => {
            let o = &*(node as *const ffi::rbs_types_optional_t);
            let inner = convert_type_node(o.type_, parser, use_context)?;
            Ok(RbsType::Optional(Box::new(inner)))
        }

        ffi::rbs_node_type::RBS_TYPES_TUPLE => {
            let t = &*(node as *const ffi::rbs_types_tuple_t);
            let types = convert_type_list(t.types, parser, use_context)?;
            Ok(RbsType::Tuple(types))
        }

        ffi::rbs_node_type::RBS_TYPES_RECORD => {
            let r = &*(node as *const ffi::rbs_types_record_t);
            let fields = convert_record_fields(r.all_fields, parser, use_context)?;
            Ok(RbsType::Record(fields))
        }

        ffi::rbs_node_type::RBS_TYPES_PROC => {
            let p = &*(node as *const ffi::rbs_types_proc_t);
            if p.type_.is_null() {
                return Ok(RbsType::Untyped);
            }
            let type_node = &*p.type_;
            let function_type = match type_node.type_ {
                ffi::rbs_node_type::RBS_TYPES_FUNCTION => convert_function_type(
                    p.type_ as *const ffi::rbs_types_function_t,
                    parser,
                    use_context,
                )?,
                ffi::rbs_node_type::RBS_TYPES_UNTYPED_FUNCTION => convert_untyped_function_type(
                    p.type_ as *const ffi::rbs_types_untyped_function_t,
                    parser,
                    use_context,
                )?,
                _ => return Ok(RbsType::Untyped),
            };
            let block = if p.block.is_null() {
                None
            } else {
                Some(convert_block_type(p.block, parser, use_context)?)
            };
            let self_type = if p.self_type.is_null() {
                None
            } else {
                Some(convert_type_node(p.self_type, parser, use_context)?)
            };
            Ok(RbsType::Proc(Box::new(MethodType {
                function_type,
                block,
                self_type,
                type_params: Vec::new(),
                type_param_bounds: Vec::new(),
                type_param_lower_bounds: Vec::new(),
                annotations: Vec::new(),
            })))
        }

        ffi::rbs_node_type::RBS_TYPES_VARIABLE => {
            let v = &*(node as *const ffi::rbs_types_variable_t);
            let name = resolve_symbol(v.name, parser);
            Ok(RbsType::Variable(name))
        }

        ffi::rbs_node_type::RBS_TYPES_ALIAS => {
            let a = &*(node as *const ffi::rbs_types_alias_t);
            let name = resolve_type_name_with_use(a.name, parser, use_context);
            let args = convert_type_list(a.args, parser, use_context)?;
            Ok(RbsType::Alias(name, args))
        }

        ffi::rbs_node_type::RBS_TYPES_LITERAL => {
            let l = &*(node as *const ffi::rbs_types_literal_t);
            if l.literal.is_null() {
                Ok(RbsType::Literal("unknown".to_string()))
            } else {
                let lit_node = &*l.literal;
                match lit_node.type_ {
                    ffi::rbs_node_type::RBS_AST_STRING => {
                        let s = &*(l.literal as *const ffi::rbs_ast_string_t);
                        let text = rbs_string_to_rust(&s.string);
                        Ok(RbsType::Literal(format!("\"{text}\"")))
                    }
                    ffi::rbs_node_type::RBS_AST_INTEGER => {
                        let i = &*(l.literal as *const ffi::rbs_ast_integer_t);
                        let text = rbs_string_to_rust(&i.string_representation);
                        Ok(RbsType::Literal(text))
                    }
                    ffi::rbs_node_type::RBS_AST_SYMBOL => {
                        let sym = l.literal as *const ffi::rbs_ast_symbol_t;
                        let name = resolve_symbol(sym, parser);
                        Ok(RbsType::Literal(format!(":{name}")))
                    }
                    ffi::rbs_node_type::RBS_AST_BOOL => {
                        let b = &*(l.literal as *const ffi::rbs_ast_bool_t);
                        Ok(RbsType::Literal(
                            if b.value { "true" } else { "false" }.to_string(),
                        ))
                    }
                    _ => Ok(RbsType::Literal("unknown".to_string())),
                }
            }
        }

        ffi::rbs_node_type::RBS_TYPES_INTERFACE => {
            let i = &*(node as *const ffi::rbs_types_class_instance_t);
            let name = resolve_type_name_with_use(i.name, parser, use_context);
            let args = convert_type_list(i.args, parser, use_context)?;
            Ok(RbsType::Class(name, args))
        }

        _ => Ok(RbsType::Untyped),
    }
}

unsafe fn convert_type_list(
    list: *const ffi::rbs_node_list_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Vec<RbsType>, ParseError> {
    let mut result = Vec::new();
    if list.is_null() {
        return Ok(result);
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            result.push(convert_type_node(entry.node, parser, use_context)?);
        }
        cur = entry.next;
    }
    Ok(result)
}

unsafe fn convert_record_fields(
    hash: *const ffi::rbs_hash_t,
    parser: *mut ffi::rbs_parser_t,
    use_context: &UseContext,
) -> Result<Vec<RbsRecordField>, ParseError> {
    let mut result = Vec::new();
    if hash.is_null() {
        return Ok(result);
    }
    let hash_ref = &*hash;
    let mut cur = hash_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.key.is_null() && !entry.value.is_null() {
            let key_node = &*entry.key;
            let record_key = if key_node.type_ == ffi::rbs_node_type::RBS_AST_SYMBOL {
                let name = resolve_symbol(entry.key as *const ffi::rbs_ast_symbol_t, parser);
                Some(RbsRecordKey::Symbol(name))
            } else if key_node.type_ == ffi::rbs_node_type::RBS_AST_STRING {
                let s = &*(entry.key as *const ffi::rbs_ast_string_t);
                let name = rbs_string_to_rust(&s.string);
                Some(RbsRecordKey::String(name))
            } else {
                None
            };

            let value_node = &*entry.value;
            let (field_type, required) =
                if value_node.type_ == ffi::rbs_node_type::RBS_TYPES_RECORD_FIELD_TYPE {
                    let rft = &*(entry.value as *const ffi::rbs_types_record_field_type_t);
                    (
                        convert_type_node(rft.type_, parser, use_context)?,
                        rft.required,
                    )
                } else {
                    (convert_type_node(entry.value, parser, use_context)?, true)
                };

            if let Some(key) = record_key {
                result.push(RbsRecordField {
                    key,
                    type_: field_type,
                    required,
                });
            }
        }
        cur = entry.next;
    }
    Ok(result)
}

unsafe fn collect_annotations(
    list: *mut ffi::rbs_node_list_t,
    _parser: *mut ffi::rbs_parser_t,
) -> Vec<std::string::String> {
    let mut result = Vec::new();
    if list.is_null() {
        return result;
    }
    let list_ref = &*list;
    let mut cur = list_ref.head;
    while !cur.is_null() {
        let entry = &*cur;
        if !entry.node.is_null() {
            let node = &*entry.node;
            if node.type_ == ffi::rbs_node_type::RBS_AST_ANNOTATION {
                let ann = &*(entry.node as *const ffi::rbs_ast_annotation_t);
                let text = rbs_string_to_rust(&ann.string);
                if !text.is_empty() {
                    result.push(text);
                }
            }
        }
        cur = entry.next;
    }
    result
}

unsafe fn rbs_string_to_rust(s: &ffi::rbs_string_t) -> std::string::String {
    if s.start.is_null() || s.end.is_null() {
        return std::string::String::new();
    }
    let len = s.end.offset_from(s.start);
    if len <= 0 {
        return std::string::String::new();
    }
    let slice = std::slice::from_raw_parts(s.start as *const u8, len as usize);
    std::string::String::from_utf8_lossy(slice).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_method_type() {
        let mt = parse_method_type("(String) -> Integer").unwrap();
        assert_eq!(mt.function_type.required_positionals.len(), 1);
        assert_eq!(
            mt.function_type.required_positionals[0].type_,
            RbsType::Class("String".to_string(), vec![])
        );
        assert_eq!(
            mt.function_type.return_type,
            RbsType::Class("Integer".to_string(), vec![])
        );
    }

    #[test]
    fn test_parse_bool_return() {
        let mt = parse_method_type("(String, Integer) -> bool").unwrap();
        assert_eq!(mt.function_type.required_positionals.len(), 2);
        assert_eq!(mt.function_type.return_type, RbsType::Bool);
    }

    #[test]
    fn test_parse_void_return() {
        let mt = parse_method_type("() -> void").unwrap();
        assert_eq!(mt.function_type.required_positionals.len(), 0);
        assert_eq!(mt.function_type.return_type, RbsType::Void);
    }

    #[test]
    fn test_parse_method_type_with_untyped_parameters() {
        let mt = parse_method_type("(?) -> String").unwrap();
        assert!(mt.function_type.required_positionals.is_empty());
        assert_eq!(
            mt.function_type.rest_positionals.as_ref().map(|p| &p.type_),
            Some(&RbsType::Untyped)
        );
        assert_eq!(
            mt.function_type.rest_keywords.as_ref().map(|p| &p.type_),
            Some(&RbsType::Untyped)
        );
        assert_eq!(
            mt.function_type.return_type,
            RbsType::Class("String".to_string(), vec![])
        );
    }

    #[test]
    fn test_parse_union_type() {
        let ty = parse_type("String | Integer").unwrap();
        match ty {
            RbsType::Union(types) => {
                assert_eq!(types.len(), 2);
            }
            _ => panic!("Expected union type, got {:?}", ty),
        }
    }

    #[test]
    fn test_parse_generic_array() {
        let mt = parse_method_type("(Array[Integer]) -> Integer").unwrap();
        assert_eq!(mt.function_type.required_positionals.len(), 1);
        match &mt.function_type.required_positionals[0].type_ {
            RbsType::Class(name, args) => {
                assert_eq!(name, "Array");
                assert_eq!(args.len(), 1);
            }
            other => panic!("Expected Class, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_optional_type() {
        let ty = parse_type("String?").unwrap();
        match ty {
            RbsType::Optional(inner) => {
                assert_eq!(*inner, RbsType::Class("String".to_string(), vec![]));
            }
            _ => panic!("Expected optional type, got {:?}", ty),
        }
    }

    #[test]
    fn test_parse_optional_record_field() {
        let ty = parse_type("{ ?name: String, count: Integer }").unwrap();
        match ty {
            RbsType::Record(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].key, RbsRecordKey::Symbol("name".to_string()));
                assert!(!fields[0].required);
                assert_eq!(fields[1].key, RbsRecordKey::Symbol("count".to_string()));
                assert!(fields[1].required);
            }
            _ => panic!("Expected record type, got {:?}", ty),
        }
    }

    #[test]
    fn test_parse_inline_colon_annotation() {
        let mt = parse_inline_leading(": (String) -> Integer").unwrap();
        assert_eq!(mt.function_type.required_positionals.len(), 1);
        assert_eq!(
            mt.function_type.return_type,
            RbsType::Class("Integer".to_string(), vec![])
        );
    }

    #[test]
    fn test_parse_inline_all_overloads_single() {
        let results = parse_inline_all_overloads(": (String) -> Integer").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_parse_inline_all_overloads_via_multiple_calls() {
        let mt1 = parse_inline_leading(": () -> String").unwrap();
        let mt2 = parse_inline_leading(": (Integer) -> String").unwrap();
        assert_eq!(mt1.function_type.required_positionals.len(), 0);
        assert_eq!(mt2.function_type.required_positionals.len(), 1);
    }

    #[test]
    fn test_parse_method_type_with_block() {
        let mt = parse_method_type("() { (Integer) -> String } -> void").unwrap();
        assert!(mt.block.is_some());
        let block = mt.block.unwrap();
        assert_eq!(block.function_type.required_positionals.len(), 1);
        assert_eq!(
            block.function_type.return_type,
            RbsType::Class("String".to_string(), vec![])
        );
        assert_eq!(mt.function_type.return_type, RbsType::Void);
    }

    #[test]
    fn test_parse_keyword_params() {
        let mt = parse_method_type("(name: String, ?age: Integer) -> bool").unwrap();
        assert_eq!(mt.function_type.required_keywords.len(), 1);
        assert_eq!(mt.function_type.required_keywords[0].0, "name");
        assert_eq!(mt.function_type.optional_keywords.len(), 1);
        assert_eq!(mt.function_type.optional_keywords[0].0, "age");
    }

    #[test]
    fn test_parse_hash_type() {
        let mt = parse_method_type("(Hash[Symbol, String]) -> void").unwrap();
        assert_eq!(mt.function_type.required_positionals.len(), 1);
        match &mt.function_type.required_positionals[0].type_ {
            RbsType::Class(name, args) => {
                assert_eq!(name, "Hash");
                assert_eq!(args.len(), 2);
            }
            other => panic!("Expected Hash class, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_implicitly_returns_nil_annotation() {
        let sig = parse_signature(
            "class Array[Elem]\n  def first: %a{implicitly-returns-nil} () -> Elem\nend\n",
        )
        .unwrap();
        match &sig.declarations[0] {
            Declaration::Class { methods, .. } => {
                assert_eq!(methods[0].name, "first");
                assert_eq!(methods[0].method_types.len(), 1);
                assert_eq!(
                    methods[0].method_types[0].annotations,
                    vec!["implicitly-returns-nil".to_string()]
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_implicitly_returns_nil_on_index_method() {
        let sig = parse_signature(
            "class MatchData\n  def []: %a{implicitly-returns-nil} (int index) -> String\nend\n",
        )
        .unwrap();
        match &sig.declarations[0] {
            Declaration::Class { methods, .. } => {
                assert_eq!(methods[0].name, "[]");
                assert_eq!(
                    methods[0].method_types[0].annotations,
                    vec!["implicitly-returns-nil".to_string()]
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_nil_return() {
        let mt = parse_method_type("() -> nil").unwrap();
        assert_eq!(mt.function_type.return_type, RbsType::Nil);
    }

    #[test]
    fn test_parse_untyped() {
        let ty = parse_type("untyped").unwrap();
        assert_eq!(ty, RbsType::Untyped);
    }

    #[test]
    fn test_parse_proc_with_untyped_parameters() {
        let ty = parse_type("^(?) -> String").unwrap();
        match ty {
            RbsType::Proc(method_type) => {
                assert!(method_type.function_type.required_positionals.is_empty());
                assert_eq!(
                    method_type
                        .function_type
                        .rest_positionals
                        .as_ref()
                        .map(|p| &p.type_),
                    Some(&RbsType::Untyped)
                );
                assert_eq!(
                    method_type
                        .function_type
                        .rest_keywords
                        .as_ref()
                        .map(|p| &p.type_),
                    Some(&RbsType::Untyped)
                );
                assert_eq!(
                    method_type.function_type.return_type,
                    RbsType::Class("String".to_string(), vec![])
                );
            }
            other => panic!("Expected proc type, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_proc_with_self_type() {
        let ty = parse_type("^() [self: String] -> self").unwrap();
        match ty {
            RbsType::Proc(method_type) => {
                assert_eq!(
                    method_type.self_type,
                    Some(RbsType::Class("String".to_string(), vec![]))
                );
                assert_eq!(method_type.function_type.return_type, RbsType::SelfType);
            }
            other => panic!("Expected proc type, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_signature_class() {
        let sig = parse_signature("class String\n  def to_i: -> Integer\nend\n").unwrap();
        assert_eq!(sig.declarations.len(), 1);
        match &sig.declarations[0] {
            Declaration::Class {
                name,
                methods,
                superclass,
                ..
            } => {
                assert_eq!(name, "String");
                assert!(superclass.is_none());
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "to_i");
                assert_eq!(methods[0].kind, MethodKind::Instance);
                assert_eq!(methods[0].method_types.len(), 1);
                assert_eq!(
                    methods[0].method_types[0].function_type.return_type,
                    RbsType::Class("Integer".to_string(), vec![])
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_module() {
        let sig = parse_signature("module Enumerable\n  def count: -> Integer\nend\n").unwrap();
        assert_eq!(sig.declarations.len(), 1);
        match &sig.declarations[0] {
            Declaration::Module { name, methods, .. } => {
                assert_eq!(name, "Enumerable");
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "count");
            }
            _ => panic!("Expected module declaration"),
        }
    }

    #[test]
    fn test_parse_signature_with_superclass() {
        let sig = parse_signature("class Integer < Numeric[String]\n  def to_f: -> Float\nend\n")
            .unwrap();
        assert_eq!(sig.declarations.len(), 1);
        match &sig.declarations[0] {
            Declaration::Class {
                name,
                superclass,
                superclass_args,
                ..
            } => {
                assert_eq!(name, "Integer");
                assert_eq!(superclass.as_deref(), Some("Numeric"));
                assert_eq!(
                    superclass_args,
                    &vec![RbsType::Class("String".to_string(), Vec::new())]
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_type_param_defaults() {
        let sig = parse_signature(
            "class Box[T < String, U = String]\nend\n\
             interface _Each[E < String, R = void]\n  def each: () { (E value) -> void } -> R\nend\n",
        )
        .unwrap();
        match &sig.declarations[0] {
            Declaration::Class {
                type_params,
                type_param_bounds,
                type_param_defaults,
                ..
            } => {
                assert_eq!(type_params, &vec!["T".to_string(), "U".to_string()]);
                assert_eq!(
                    type_param_bounds,
                    &vec![(
                        "T".to_string(),
                        RbsType::Class("String".to_string(), Vec::new())
                    )]
                );
                assert_eq!(
                    type_param_defaults,
                    &vec![(
                        "U".to_string(),
                        RbsType::Class("String".to_string(), Vec::new())
                    )]
                );
            }
            _ => panic!("Expected class declaration"),
        }
        match &sig.declarations[1] {
            Declaration::Interface {
                type_params,
                type_param_bounds,
                type_param_defaults,
                ..
            } => {
                assert_eq!(type_params, &vec!["E".to_string(), "R".to_string()]);
                assert_eq!(
                    type_param_bounds,
                    &vec![(
                        "E".to_string(),
                        RbsType::Class("String".to_string(), Vec::new())
                    )]
                );
                assert_eq!(type_param_defaults, &vec![("R".to_string(), RbsType::Void)]);
            }
            _ => panic!("Expected interface declaration"),
        }
    }

    #[test]
    fn test_parse_signature_method_type_param_lower_bounds() {
        let sig = parse_signature("class Box\n  def fallback: [T > String] (T value) -> T\nend\n")
            .unwrap();
        match &sig.declarations[0] {
            Declaration::Class { methods, .. } => {
                assert_eq!(
                    methods[0].method_types[0].type_params,
                    vec!["T".to_string()]
                );
                assert_eq!(
                    methods[0].method_types[0].type_param_lower_bounds,
                    vec![(
                        "T".to_string(),
                        RbsType::Class("String".to_string(), Vec::new())
                    )]
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_type_alias_type_param_metadata() {
        let sig = parse_signature("type list[T < String, U = Integer] = [T, U]\n").unwrap();
        match &sig.declarations[0] {
            Declaration::TypeAlias {
                type_params,
                type_param_bounds,
                type_param_defaults,
                ..
            } => {
                assert_eq!(type_params, &vec!["T".to_string(), "U".to_string()]);
                assert_eq!(
                    type_param_bounds,
                    &vec![(
                        "T".to_string(),
                        RbsType::Class("String".to_string(), Vec::new())
                    )]
                );
                assert_eq!(
                    type_param_defaults,
                    &vec![(
                        "U".to_string(),
                        RbsType::Class("Integer".to_string(), Vec::new())
                    )]
                );
            }
            _ => panic!("Expected type alias declaration"),
        }
    }

    #[test]
    fn test_parse_signature_singleton_method() {
        let sig =
            parse_signature("class File\n  def self.read: (String path) -> String\nend\n").unwrap();
        match &sig.declarations[0] {
            Declaration::Class { methods, .. } => {
                assert_eq!(methods[0].name, "read");
                assert_eq!(methods[0].kind, MethodKind::Singleton);
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_attr_mixin_and_variables() {
        let sig = parse_signature(
            "module Named[T] : BasicObject, Enumerable[T]\n  attr_reader name: String\nend\n\
             class User\n  include Named[String]\n  @token: String\nend\n",
        )
        .unwrap();
        match &sig.declarations[0] {
            Declaration::Module {
                self_types,
                methods,
                ..
            } => {
                assert_eq!(
                    self_types,
                    &vec![
                        SelfTypeDecl {
                            name: "BasicObject".to_string(),
                            args: Vec::new()
                        },
                        SelfTypeDecl {
                            name: "Enumerable".to_string(),
                            args: vec![RbsType::Variable("T".to_string())]
                        }
                    ]
                );
                assert_eq!(methods[0].name, "name");
                assert_eq!(methods[0].attr_ivar.as_deref(), Some("@name"));
            }
            _ => panic!("Expected module declaration"),
        }
        match &sig.declarations[1] {
            Declaration::Class {
                mixins, variables, ..
            } => {
                assert_eq!(mixins[0].name, "Named");
                assert_eq!(
                    mixins[0].args,
                    vec![RbsType::Class("String".to_string(), Vec::new())]
                );
                assert_eq!(mixins[0].kind, MixinKind::Include);
                assert_eq!(variables[0].name, "@token");
                assert_eq!(variables[0].kind, VariableKind::Instance);
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_interface_global_and_type_alias() {
        let sig = parse_signature(
            "interface _Renderable\n  def render: -> String\nend\n\
             $config: String\n\
             type name = String\n",
        )
        .unwrap();
        assert!(matches!(
            &sig.declarations[0],
            Declaration::Interface { name, methods, .. }
                if name == "_Renderable" && methods[0].name == "render"
        ));
        assert!(matches!(
            &sig.declarations[1],
            Declaration::Global { name, type_ }
                if name == "$config" && type_ == &RbsType::Class("String".to_string(), vec![])
        ));
        assert!(matches!(
            &sig.declarations[2],
            Declaration::TypeAlias { name, type_, .. }
                if name == "name" && type_ == &RbsType::Class("String".to_string(), vec![])
        ));
    }

    #[test]
    fn test_parse_signature_interface_include() {
        let sig = parse_signature(
            "interface _Parent[T]\n  def value: -> T\nend\n\
             interface _Child[Elem]\n  include _Parent[Elem]\nend\n",
        )
        .unwrap();
        match &sig.declarations[1] {
            Declaration::Interface { mixins, .. } => {
                assert_eq!(mixins.len(), 1);
                assert_eq!(mixins[0].name, "_Parent");
                assert_eq!(mixins[0].args, vec![RbsType::Variable("Elem".to_string())]);
                assert_eq!(mixins[0].kind, MixinKind::Include);
            }
            _ => panic!("Expected interface declaration"),
        }
    }

    #[test]
    fn test_parse_signature_class_and_module_aliases() {
        let sig = parse_signature(
            "class Entry = Types::Item\n\
             module Helpers = Types::Helpers\n",
        )
        .unwrap();
        assert!(matches!(
            &sig.declarations[0],
            Declaration::ClassAlias { new_name, old_name }
                if new_name == "Entry" && old_name == "Types::Item"
        ));
        assert!(matches!(
            &sig.declarations[1],
            Declaration::ModuleAlias { new_name, old_name }
                if new_name == "Helpers" && old_name == "Types::Helpers"
        ));
    }

    #[test]
    fn test_parse_signature_use_single_clause() {
        let sig = parse_signature(
            "use Types::Item\n\
             class User\n  def item: -> Item\nend\n",
        )
        .unwrap();
        match &sig.declarations[0] {
            Declaration::Class { methods, .. } => {
                assert_eq!(
                    methods[0].method_types[0].function_type.return_type,
                    RbsType::Class("Types::Item".to_string(), vec![])
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_use_aliased_clause() {
        let sig = parse_signature(
            "use Types::Item as Entry\n\
             class User\n  def item: -> Entry\nend\n",
        )
        .unwrap();
        match &sig.declarations[0] {
            Declaration::Class { methods, .. } => {
                assert_eq!(
                    methods[0].method_types[0].function_type.return_type,
                    RbsType::Class("Types::Item".to_string(), vec![])
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_use_wildcard_clause_for_known_names() {
        let sig = parse_signature(
            "use Types::*\n\
             module Types\n  class Item\n  end\nend\n\
             class User\n  def item: -> Item\nend\n",
        )
        .unwrap();
        let user = sig
            .declarations
            .iter()
            .find(|decl| matches!(decl, Declaration::Class { name, .. } if name == "User"))
            .expect("User declaration");
        match user {
            Declaration::Class { methods, .. } => {
                assert_eq!(
                    methods[0].method_types[0].function_type.return_type,
                    RbsType::Class("Types::Item".to_string(), vec![])
                );
            }
            _ => panic!("Expected class declaration"),
        }
    }

    #[test]
    fn test_parse_signature_nested_lexical_type_names() {
        let sig = parse_signature(
            "class Outer\n  class Item\n  end\n  class Inner\n    class Item\n    end\n  end\n  class Source\n    DEFAULT: Item\n    def item: -> Item\n    def nested_item: -> Inner::Item\n  end\nend\n",
        )
        .unwrap();
        let source = sig
            .declarations
            .iter()
            .find(|decl| matches!(decl, Declaration::Class { name, .. } if name == "Outer::Source"))
            .expect("Outer::Source declaration");
        match source {
            Declaration::Class { methods, .. } => {
                assert_eq!(
                    methods[0].method_types[0].function_type.return_type,
                    RbsType::Class("Outer::Item".to_string(), vec![])
                );
                assert_eq!(
                    methods[1].method_types[0].function_type.return_type,
                    RbsType::Class("Outer::Inner::Item".to_string(), vec![])
                );
            }
            _ => panic!("Expected class declaration"),
        }

        let default = sig
            .declarations
            .iter()
            .find(|decl| {
                matches!(decl, Declaration::Constant { name, .. } if name == "Outer::Source::DEFAULT")
            })
            .expect("Outer::Source::DEFAULT declaration");
        assert!(matches!(
            default,
            Declaration::Constant { type_, .. }
                if type_ == &RbsType::Class("Outer::Item".to_string(), vec![])
        ));
    }

    #[test]
    fn test_parse_signature_nested_path_name_declaration() {
        // `module Net { class HTTP::Post < HTTPRequest }` defines `Net::HTTP::Post` and
        // resolves superclass `HTTPRequest` to `Net::HTTPRequest`. Guards the regression
        // that skipped qualifying relative path names just because they contained `::`.
        let sig = parse_signature(
            "module Net\n  class HTTPRequest\n  end\n  class HTTP::Post < HTTPRequest\n  end\n  HTTP::STATUS_CODES: Integer\nend\n",
        )
        .unwrap();
        let post = sig
            .declarations
            .iter()
            .find(
                |decl| matches!(decl, Declaration::Class { name, .. } if name == "Net::HTTP::Post"),
            )
            .expect("Net::HTTP::Post declaration");
        match post {
            Declaration::Class { superclass, .. } => {
                assert_eq!(superclass.as_deref(), Some("Net::HTTPRequest"));
            }
            _ => panic!("Expected class declaration"),
        }

        assert!(
            sig.declarations.iter().any(|decl| matches!(
                decl,
                Declaration::Constant { name, .. } if name == "Net::HTTP::STATUS_CODES"
            )),
            "path-form constant should be qualified with the enclosing module"
        );
    }
}

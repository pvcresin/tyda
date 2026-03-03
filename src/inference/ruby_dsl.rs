use super::*;
use crate::types::Sym;

struct AttrMethodName {
    name: String,
    loc: SourceLocation,
    hover_range: Option<(usize, usize)>,
}

pub(super) struct DynamicDefineMethodInfo<'a> {
    pub call_node: ruby_prism::CallNode<'a>,
    pub first_arg: Node<'a>,
    pub source_arg: Option<Node<'a>>,
    pub defines_singleton: bool,
}

#[derive(Clone)]
struct IteratorBinding {
    name: String,
    ty: Type,
}

impl<'a> InferenceEngine<'a> {
    fn attr_symbol_location(
        source: &[u8],
        arg: &Node<'_>,
        name: &str,
        attr_start: usize,
        attr_loc: SourceLocation,
    ) -> SourceLocation {
        let (name_offset, _) = Self::literal_name_offsets(source, arg, name);
        offset_to_location_from(source, attr_start, attr_loc, name_offset)
    }

    fn literal_name_from_type(ty: &Type) -> Option<String> {
        match ty {
            Type::LiteralSymbol(name) => Some(name.to_string()),
            Type::LiteralString(name) => Some(name.clone()),
            _ => None,
        }
    }

    fn static_name_sequence_from_type(ty: &Type) -> Option<Vec<String>> {
        let Type::Tuple(elems) = ty else {
            return None;
        };
        elems.iter().map(Self::literal_name_from_type).collect()
    }

    fn static_name_sequence_from_record_keys(ty: &Type) -> Option<Vec<String>> {
        let Type::Record(fields) = ty else {
            return None;
        };
        Some(
            fields
                .iter()
                .map(|field| match &field.key {
                    RecordKey::Symbol(name) | RecordKey::String(name) => name.clone(),
                })
                .collect(),
        )
    }

    fn static_name_sequence_from_record_values(ty: &Type) -> Option<Vec<String>> {
        let Type::Record(fields) = ty else {
            return None;
        };
        fields
            .iter()
            .map(|field| Self::literal_name_from_type(&field.value))
            .collect()
    }

    fn static_name_sequence_from_constant(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Vec<String>> {
        let const_path = self.resolve_constant_path(node, parse_result);
        if const_path == "Unknown" {
            return None;
        }
        let const_type = self.resolve_constant_type_in_scope(&const_path, class_name);
        if let Some(names) = Self::static_name_sequence_from_type(&const_type) {
            return Some(names);
        }
        self.lookup_cached_constant_name_sequence(&const_path, class_name)
    }

    fn static_name_receiver_type(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Type> {
        match node {
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                let const_path = self.resolve_constant_path(node, parse_result);
                (const_path != "Unknown")
                    .then(|| self.resolve_constant_type_in_scope(&const_path, class_name))
            }
            Node::HashNode { .. } => {
                Some(self.infer_node_type(class_name, node, parse_result, &Scope::default()))
            }
            _ => None,
        }
    }

    fn static_name_sequence_from_array_node(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Vec<String>> {
        let array = node.as_array_node().expect("must be ArrayNode");
        let mut names = Vec::new();
        for elem in array.elements().iter() {
            let elem_names = match &elem {
                Node::SplatNode { .. } => {
                    let splat = elem.as_splat_node().expect("must be SplatNode");
                    let expr = splat.expression()?;
                    self.static_name_sequence_from_node(class_name, &expr, parse_result)?
                }
                _ => vec![Self::extract_symbol_literal_name(&elem)?],
            };
            names.extend(elem_names);
        }
        Some(names)
    }

    fn static_name_sequence_from_call_node(
        &mut self,
        class_name: &str,
        call: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Vec<String>> {
        if call.block().is_some() {
            return None;
        }

        let method_name = String::from_utf8_lossy(call.name().as_slice());
        let args: Vec<_> = call
            .arguments()
            .map(|args| args.arguments().iter().collect())
            .unwrap_or_default();
        let receiver = call.receiver();

        match method_name.as_ref() {
            "+" | "concat" | "|" => {
                let receiver = receiver?;
                let mut names =
                    self.static_name_sequence_from_node(class_name, &receiver, parse_result)?;
                for arg in args {
                    let arg_names =
                        self.static_name_sequence_from_node(class_name, &arg, parse_result)?;
                    if method_name == "|" {
                        for name in arg_names {
                            if !names.contains(&name) {
                                names.push(name);
                            }
                        }
                    } else {
                        names.extend(arg_names);
                    }
                }
                Some(names)
            }
            "-" | "&" if args.len() == 1 => {
                let receiver = receiver?;
                let left =
                    self.static_name_sequence_from_node(class_name, &receiver, parse_result)?;
                let right =
                    self.static_name_sequence_from_node(class_name, &args[0], parse_result)?;
                Some(
                    left.into_iter()
                        .filter(|name| {
                            let contains = right.contains(name);
                            if method_name == "-" {
                                !contains
                            } else {
                                contains
                            }
                        })
                        .collect(),
                )
            }
            "column_names" | "attribute_names" if args.is_empty() => {
                let receiver = receiver?;
                let receiver_type =
                    self.static_name_receiver_type(class_name, &receiver, parse_result)?;
                let model = match &receiver_type {
                    Type::Singleton(name) | Type::Class(name) => name.as_str().to_string(),
                    _ => return None,
                };
                self.ensure_class_available(&model);
                self.registry.schema_column_names(&model)
            }
            "keys" | "values" if args.is_empty() => {
                let receiver = receiver?;
                let receiver_type =
                    self.static_name_receiver_type(class_name, &receiver, parse_result)?;
                if method_name == "keys" {
                    Self::static_name_sequence_from_record_keys(&receiver_type)
                } else {
                    Self::static_name_sequence_from_record_values(&receiver_type)
                }
            }
            _ => None,
        }
    }

    pub(super) fn static_name_sequence_from_node(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Vec<String>> {
        match node {
            Node::SymbolNode { .. } | Node::StringNode { .. } => {
                Some(vec![Self::extract_symbol_literal_name(node)?])
            }
            Node::ArrayNode { .. } => {
                self.static_name_sequence_from_array_node(class_name, node, parse_result)
            }
            Node::SplatNode { .. } => {
                let splat = node.as_splat_node().expect("must be SplatNode");
                let expr = splat.expression()?;
                self.static_name_sequence_from_node(class_name, &expr, parse_result)
            }
            Node::ParenthesesNode { .. } => {
                let paren = node.as_parentheses_node().expect("must be ParenthesesNode");
                let body = paren.body()?;
                let statements = body.as_statements_node()?;
                let mut nodes = statements.body().iter();
                let inner = nodes.next()?;
                nodes.next().is_none().then_some(())?;
                self.static_name_sequence_from_node(class_name, &inner, parse_result)
            }
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                self.static_name_sequence_from_constant(class_name, node, parse_result)
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("must be CallNode");
                self.static_name_sequence_from_call_node(class_name, &call, parse_result)
            }
            _ => None,
        }
    }

    fn static_name_args_scoped(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        arg_offset: usize,
        scope: &Scope,
    ) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(args) = call_node.arguments() {
            for arg in args.arguments().iter().skip(arg_offset) {
                if let Some(arg_names) =
                    self.static_name_sequence_from_node(class_name, &arg, parse_result)
                {
                    names.extend(arg_names);
                } else if let Some(arg_names) =
                    self.static_dispatch_names_from_arg(class_name, &arg, parse_result, scope)
                {
                    names.extend(arg_names);
                }
            }
        }
        names
    }

    fn static_struct_member_names(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        skip_struct_class_name: bool,
    ) -> Vec<String> {
        let mut names = Vec::new();
        let Some(args) = call_node.arguments() else {
            return names;
        };
        for (idx, arg) in args.arguments().iter().enumerate() {
            if matches!(arg, Node::KeywordHashNode { .. }) {
                continue;
            }
            if skip_struct_class_name
                && idx == 0
                && let Some(name) = Self::extract_symbol_literal_name(&arg)
                && name
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
            {
                continue;
            }
            if let Some(arg_names) =
                self.static_name_sequence_from_node(class_name, &arg, parse_result)
            {
                names.extend(arg_names);
            }
        }
        names
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_attr_methods(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        generate_reader: bool,
        generate_writer: bool,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
        is_singleton: bool,
    ) {
        let attr_start = call_node.location().start_offset();
        let attr_end = call_node.location().end_offset();
        let attr_loc = offset_to_location(parse_result.source(), attr_start);
        let rbs_lines = find_method_annotations(comments, attr_start, parse_result.source());
        let attr_type = rbs_lines.as_ref().and_then(|lines| {
            let first = lines.first()?;
            let body = first.strip_prefix("#")?;
            let trimmed = body.trim();
            if trimmed.starts_with(":") {
                let type_str = trimmed.strip_prefix(":")?.trim();
                let rbs_str = format!("#: () -> {type_str}");
                let result = parse_rbs_shorthand(&rbs_str)?;
                Some(result.return_type)
            } else {
                None
            }
        });
        let attr_type = attr_type.or_else(|| {
            let assertion = find_inline_assertion_for_node(
                &self.inline_assertion_comments,
                parse_result.source(),
                attr_start,
                attr_end,
            )?;
            match assertion {
                crate::rbs::inline::InlineAssertion::Explicit(ty) => Some(ty.clone()),
                crate::rbs::inline::InlineAssertion::NonNil => None,
            }
        });

        if let Some(args) = call_node.arguments() {
            let mut attr_names = Vec::new();
            for arg in args.arguments().iter() {
                match &arg {
                    Node::SymbolNode { .. } => {
                        let sym = arg.as_symbol_node().expect("must be SymbolNode");
                        let name = String::from_utf8_lossy(sym.unescaped()).to_string();
                        let loc = Self::attr_symbol_location(
                            parse_result.source(),
                            &arg,
                            &name,
                            attr_start,
                            attr_loc,
                        );
                        let hover_range =
                            Self::literal_definition_offsets(parse_result.source(), &arg, &name);
                        attr_names.push(AttrMethodName {
                            name,
                            loc,
                            hover_range: Some(hover_range),
                        });
                    }
                    Node::StringNode { .. } => {
                        let string = arg.as_string_node().expect("must be StringNode");
                        let name = String::from_utf8_lossy(string.unescaped()).to_string();
                        let loc = Self::attr_symbol_location(
                            parse_result.source(),
                            &arg,
                            &name,
                            attr_start,
                            attr_loc,
                        );
                        let hover_range =
                            Self::literal_definition_offsets(parse_result.source(), &arg, &name);
                        attr_names.push(AttrMethodName {
                            name,
                            loc,
                            hover_range: Some(hover_range),
                        });
                    }
                    Node::SplatNode { .. } => {
                        let splat = arg.as_splat_node().expect("must be SplatNode");
                        if let Some(expr) = splat.expression()
                            && let Some(names) =
                                self.static_name_sequence_from_node(class_name, &expr, parse_result)
                        {
                            attr_names.extend(names.into_iter().map(|name| AttrMethodName {
                                name,
                                loc: attr_loc,
                                hover_range: None,
                            }));
                        }
                    }
                    _ => continue,
                }
            }

            for attr in attr_names {
                let attr_name = attr.name;
                let attr_loc = attr.loc;
                let ivar_name = format!("@{attr_name}");
                let annotated = attr_type.is_some();
                let ret_type = attr_type.clone().unwrap_or(Type::Untyped);

                if generate_reader {
                    self.registry.add_method_def(
                        class_name,
                        MethodDef {
                            name: Sym::new(&attr_name),
                            param_infos: Vec::new(),
                            raw_return_type: ret_type.clone(),
                            sorbet_modifier_comments: Vec::new(),
                            rbs_annotated: annotated,
                            rbs_inline_annotated: annotated,
                            sig_annotated: false,
                            attr_ivar: Some(ivar_name.clone()),
                            is_singleton,
                            rbs_file_source: false,
                            synthetic_dsl_source: false,
                            rbs_method_types: Default::default(),
                            extra_overloads: Vec::new(),
                            loc: Some(attr_loc),
                        },
                    );
                }

                if let Some((start, end)) = attr.hover_range {
                    let hover_method_name = if generate_reader {
                        attr_name.clone()
                    } else {
                        format!("{attr_name}=")
                    };
                    self.push_method_definition_lookup_snapshot(
                        start,
                        end,
                        class_name,
                        &hover_method_name,
                        is_singleton,
                    );
                }

                if generate_writer {
                    self.registry.add_method_def(
                        class_name,
                        MethodDef {
                            name: Sym::new(format!("{attr_name}=")),
                            param_infos: vec![ParamInfo {
                                name: attr_name.clone(),
                                kind: ParamKind::Required,
                                default_type: None,
                            }],
                            raw_return_type: ret_type,
                            sorbet_modifier_comments: Vec::new(),
                            rbs_annotated: annotated,
                            rbs_inline_annotated: annotated,
                            sig_annotated: false,
                            attr_ivar: Some(ivar_name),
                            is_singleton,
                            rbs_file_source: false,
                            synthetic_dsl_source: false,
                            rbs_method_types: Default::default(),
                            extra_overloads: Vec::new(),
                            loc: Some(attr_loc),
                        },
                    );
                }
            }
        }
    }

    pub(super) fn collect_enum_values(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
    ) {
        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
            && let Node::StatementsNode { .. } = &body
        {
            let statements = body.as_statements_node().expect("must be StatementsNode");
            for stmt in statements.body().iter() {
                if let Node::ConstantWriteNode { .. } = &stmt {
                    let write = stmt
                        .as_constant_write_node()
                        .expect("must be ConstantWriteNode");
                    let const_name = String::from_utf8_lossy(write.name().as_slice()).to_string();
                    let full_name = crate::sym::join_scope(class_name, &const_name);
                    self.constants
                        .insert(full_name, Type::Class(Sym::new(class_name)));
                }
            }
        }
    }

    pub(super) fn collect_mixin(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        kind: MixinKind,
    ) -> bool {
        self.collect_mixin_scoped(
            class_name,
            call_node,
            parse_result,
            kind,
            0,
            &Scope::default(),
        )
    }

    pub(super) fn collect_mixin_scoped(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        kind: MixinKind,
        skip_positionals: usize,
        scope: &Scope,
    ) -> bool {
        self.collect_mixin_with_arg_offset(
            class_name,
            call_node,
            parse_result,
            kind,
            skip_positionals,
            scope,
        )
    }

    pub(super) fn collect_static_mixin_dispatch(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) -> bool {
        let method_name = String::from_utf8_lossy(call_node.name().as_slice());
        if !matches!(method_name.as_ref(), "send" | "public_send" | "__send__") {
            return false;
        }
        let Some(args) = call_node.arguments() else {
            return false;
        };
        let Some(name_arg) = args.arguments().iter().next() else {
            return false;
        };
        let Some(names) =
            self.static_dispatch_names_from_arg(class_name, &name_arg, parse_result, scope)
        else {
            return false;
        };

        let mut collected = false;
        for name in names {
            let kind = match name.as_str() {
                "include" => MixinKind::Include,
                "extend" => MixinKind::Extend,
                "prepend" => MixinKind::Prepend,
                _ => continue,
            };
            collected |=
                self.collect_mixin_scoped(class_name, call_node, parse_result, kind, 1, scope);
        }
        collected
    }

    fn collect_mixin_with_arg_offset(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        kind: MixinKind,
        skip_positionals: usize,
        scope: &Scope,
    ) -> bool {
        let Some(target_class) = self.mixin_call_target_class(class_name, call_node, parse_result)
        else {
            return false;
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let mut collected = false;
        if let Some(args) = call_node.arguments() {
            for (index, arg) in args.arguments().iter().enumerate() {
                if index < skip_positionals {
                    continue;
                }
                if let Node::CallNode { .. } = &arg {
                    self.dsl_plugin_collect_mixin_argument(
                        &target_class,
                        &kind,
                        &arg,
                        parse_result,
                    );
                }
                let Some(module_names) = self.static_mixin_names_from_arg(
                    class_name,
                    &target_class,
                    &arg,
                    parse_result,
                    &kind,
                    scope,
                ) else {
                    continue;
                };
                for module_name in module_names {
                    let self_extend = kind == MixinKind::Extend && module_name == target_class;
                    if !self_extend {
                        self.record_reference(&module_name);
                    }
                    self.registry.mark_user_defined(&target_class);
                    self.registry
                        .add_mixin(&target_class, &module_name, kind.clone());
                    collected = true;
                    if kind == MixinKind::Include && module_name == "Singleton" {
                        self.registry.add_method_def(
                            &target_class,
                            MethodDef {
                                name: Sym::new("instance"),
                                param_infos: Vec::new(),
                                raw_return_type: Type::Class(Sym::new(&target_class)),
                                sorbet_modifier_comments: Vec::new(),
                                rbs_annotated: false,
                                rbs_inline_annotated: false,
                                sig_annotated: false,
                                attr_ivar: None,
                                is_singleton: true,
                                rbs_file_source: false,
                                synthetic_dsl_source: false,
                                rbs_method_types: Default::default(),
                                extra_overloads: Vec::new(),
                                loc: Some(loc),
                            },
                        );
                    }
                    if !self_extend {
                        self.dsl_plugin_register_on_mixin(&target_class, &module_name, loc);
                        self.apply_includer_bound_dsl_from_mixin(&target_class, &module_name);
                    }
                }
            }
        }
        collected
    }

    fn static_mixin_names_from_arg(
        &mut self,
        class_name: &str,
        target_class: &str,
        arg: &Node<'_>,
        parse_result: &ParseResult<'_>,
        kind: &MixinKind,
        scope: &Scope,
    ) -> Option<Vec<String>> {
        if *kind == MixinKind::Extend && matches!(arg, Node::SelfNode { .. }) {
            return Some(vec![target_class.to_string()]);
        }

        if let Node::SplatNode { .. } = arg {
            let splat = arg.as_splat_node().expect("must be SplatNode");
            let expr = splat.expression()?;
            let ty = if let Some(local_name) = Self::extract_local_var_name(&expr) {
                scope.get(&local_name).cloned()
            } else {
                Some(self.infer_node_type(class_name, &expr, parse_result, scope))
            }?;
            return Self::static_mixin_names_from_type(&ty);
        }

        let module_name = self.resolve_constant_path(arg, parse_result);
        if module_name == "Unknown" {
            return None;
        }
        let canonical = self
            .follow_namespace_alias(&module_name, class_name)
            .unwrap_or(module_name);
        Some(vec![canonical])
    }

    fn static_mixin_names_from_type(ty: &Type) -> Option<Vec<String>> {
        let Type::Tuple(elems) = ty else {
            return None;
        };
        let names: Option<Vec<_>> = elems
            .iter()
            .map(|elem| match elem {
                Type::Singleton(name) | Type::Class(name) => Some(name.to_string()),
                _ => None,
            })
            .collect();
        let names = names?;
        (!names.is_empty()).then_some(names)
    }

    fn mixin_call_target_class(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        self.resolve_static_definition_receiver_owner(
            class_name,
            call_node.receiver().as_ref(),
            parse_result,
        )
    }

    pub(super) fn current_scope_is_module(&self, class_name: &str) -> bool {
        self.registry
            .class_data_for(class_name)
            .is_some_and(|data| data.is_module)
    }

    pub(super) fn mirror_instance_method_as_singleton(
        &mut self,
        class_name: &str,
        method_name: &str,
    ) {
        if self
            .registry
            .has_method_variant(class_name, method_name, true)
        {
            return;
        }
        let Some(mut method_def) = self
            .registry
            .lookup_method_def(class_name, method_name, false)
            .cloned()
        else {
            return;
        };
        method_def.is_singleton = true;
        self.registry.add_method_def(class_name, method_def);
        if let Some(meta) = self
            .registry
            .lookup_method_block_meta(class_name, method_name, false)
            .cloned()
        {
            self.registry
                .set_method_block_meta(class_name, method_name, true, meta);
        }
        self.module_function_mirrors
            .push((class_name.to_string(), method_name.to_string()));
    }

    pub(super) fn collect_module_function_scoped(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        arg_offset: usize,
        scope: &Scope,
    ) {
        for method_name in
            self.static_name_args_scoped(class_name, call_node, parse_result, arg_offset, scope)
        {
            self.mirror_instance_method_as_singleton(class_name, &method_name);
        }
    }

    pub(super) fn collect_module_function_define_method(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
    ) -> bool {
        let Some(args) = call_node.arguments() else {
            return false;
        };
        let mut arguments = args.arguments().iter();
        let Some(wrapped) = arguments.next() else {
            return false;
        };
        if arguments.next().is_some() {
            return false;
        }
        let Node::CallNode { .. } = wrapped else {
            return false;
        };
        let define_method_call = wrapped.as_call_node().expect("must be CallNode");
        let method_name = String::from_utf8_lossy(define_method_call.name().as_slice());
        if method_name != "define_method" {
            return false;
        }
        let Some(defined_names) = self.define_method_static_names(
            class_name,
            &define_method_call,
            parse_result,
            &Scope::default(),
        ) else {
            return false;
        };

        self.collect_define_method(
            class_name,
            &define_method_call,
            parse_result,
            comments,
            false,
        );
        for defined_name in defined_names {
            self.mirror_instance_method_as_singleton(class_name, &defined_name);
        }
        true
    }

    pub(crate) fn sync_module_function_mirrors(&mut self) {
        let mirrors = std::mem::take(&mut self.module_function_mirrors);
        let mut seen = HashSet::new();
        for (class_name, method_name) in mirrors {
            if !seen.insert((class_name.clone(), method_name.clone())) {
                continue;
            }
            let Some(mut method_def) = self
                .registry
                .lookup_method_def(&class_name, &method_name, false)
                .cloned()
            else {
                continue;
            };
            method_def.is_singleton = true;
            self.registry
                .remove_method_variant(&class_name, &method_name, true);
            self.registry.add_method_def(&class_name, method_def);
            if let Some(meta) = self
                .registry
                .lookup_method_block_meta(&class_name, &method_name, false)
                .cloned()
            {
                self.registry
                    .set_method_block_meta(&class_name, &method_name, true, meta);
            }
        }
    }

    pub(super) fn extract_alias_name(node: &Node<'_>) -> Option<String> {
        match node {
            Node::SymbolNode { .. } => {
                let sym = node.as_symbol_node().expect("must be SymbolNode");
                Some(String::from_utf8_lossy(sym.unescaped()).to_string())
            }
            Node::GlobalVariableReadNode { .. } => {
                let gvar = node
                    .as_global_variable_read_node()
                    .expect("must be GlobalVariableReadNode");
                Some(String::from_utf8_lossy(gvar.name().as_slice()).to_string())
            }
            _ => None,
        }
    }

    pub(super) fn collect_alias_method(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        is_singleton: bool,
    ) {
        self.collect_alias_method_scoped(
            class_name,
            call_node,
            parse_result,
            is_singleton,
            0,
            &Scope::default(),
        );
    }

    pub(super) fn collect_alias_method_scoped(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        is_singleton: bool,
        arg_offset: usize,
        scope: &Scope,
    ) {
        let args =
            self.static_name_args_scoped(class_name, call_node, parse_result, arg_offset, scope);
        if args.len() >= 2 {
            let new_name = &args[0];
            let old_name = &args[1];
            let loc =
                offset_to_location(parse_result.source(), call_node.location().start_offset());
            self.collect_static_alias_definition(
                class_name,
                new_name.clone(),
                old_name.clone(),
                is_singleton,
                loc,
            );
        }
    }

    pub(super) fn collect_static_alias_definition(
        &mut self,
        class_name: &str,
        new_name: String,
        old_name: String,
        is_singleton: bool,
        loc: SourceLocation,
    ) {
        let mut method_def = self
            .registry
            .lookup_method_def(class_name, &old_name, is_singleton)
            .cloned()
            .unwrap_or_else(|| MethodDef {
                name: Sym::new(&new_name),
                param_infos: Vec::new(),
                raw_return_type: Type::MethodReturnRef(class_name.into(), old_name.clone().into()),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: false,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            });
        method_def.name = Sym::new(&new_name);
        method_def.is_singleton = is_singleton;
        method_def.loc = Some(loc);
        method_def.sorbet_modifier_comments.clear();
        method_def.rbs_file_source = false;
        method_def.synthetic_dsl_source = false;
        self.registry.add_method_def(class_name, method_def);
        self.registry.record_method_alias(
            class_name,
            new_name.clone(),
            old_name.clone(),
            is_singleton,
            Some(loc),
        );
        if let Some(meta) = self
            .registry
            .lookup_method_block_meta(class_name, &old_name, is_singleton)
            .cloned()
        {
            self.registry
                .set_method_block_meta(class_name, &new_name, is_singleton, meta);
        }
    }

    pub(super) fn collect_define_method(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
        is_singleton: bool,
    ) {
        self.collect_define_method_scoped(
            class_name,
            call_node,
            parse_result,
            comments,
            is_singleton,
            0,
            &Scope::default(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_define_method_scoped(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
        is_singleton: bool,
        name_arg_index: usize,
        scope: &Scope,
    ) {
        let Some(method_names) = self.define_method_static_names_at_arg(
            class_name,
            call_node,
            parse_result,
            name_arg_index,
            scope,
        ) else {
            return;
        };
        let args = call_node.arguments().expect("define_method name checked");
        let arg_list: Vec<_> = args.arguments().iter().collect();
        let first_arg = arg_list
            .get(name_arg_index)
            .expect("define_method name checked");
        let source_arg = arg_list.get(name_arg_index + 1);

        for method_name in method_names {
            self.collect_define_method_named(
                class_name,
                call_node,
                parse_result,
                comments,
                is_singleton,
                &method_name,
                Some(first_arg),
                source_arg,
                scope.clone(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_define_method_named(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
        is_singleton: bool,
        method_name: &str,
        first_arg: Option<&Node<'_>>,
        source_arg: Option<&Node<'_>>,
        captured_scope: Scope,
    ) {
        let loc = first_arg
            .filter(|arg| Self::extract_symbol_literal_name(arg).as_deref() == Some(method_name))
            .map(|arg| Self::literal_name_location(parse_result.source(), arg, method_name))
            .unwrap_or_else(|| {
                offset_to_location(parse_result.source(), call_node.location().start_offset())
            });
        if let Some(arg) = first_arg
            && Self::extract_symbol_literal_name(arg).as_deref() == Some(method_name)
        {
            let (name_start, name_end) =
                Self::literal_name_offsets(parse_result.source(), arg, method_name);
            self.push_method_definition_lookup_snapshot(
                name_start,
                name_end,
                class_name,
                method_name,
                is_singleton,
            );
        }
        let rbs_lines = find_method_annotations(
            comments,
            call_node.location().start_offset(),
            parse_result.source(),
        );
        let rbs_sig = rbs_lines.as_ref().and_then(|lines| {
            let full = lines.join("\n");
            parse_rbs_shorthand(&full)
        });

        if let Some(sig) = rbs_sig {
            let param_infos: Vec<ParamInfo> = sig
                .param_types
                .iter()
                .enumerate()
                .map(|(i, (_ty, kind))| ParamInfo {
                    name: format!("arg{i}"),
                    kind: *kind,
                    default_type: None,
                })
                .collect();
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(method_name),
                    param_infos,
                    raw_return_type: sig.return_type,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: true,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: std::sync::Arc::new(sig.method_types),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            return;
        }

        if let Some(source_arg) = source_arg
            && let Some(mut forwarded_method) = self.define_method_from_unbound_method_arg(
                class_name,
                source_arg,
                parse_result,
                &captured_scope,
            )
        {
            forwarded_method.name = Sym::new(method_name);
            forwarded_method.is_singleton = is_singleton;
            forwarded_method.loc = Some(loc);
            self.registry.add_method_def(class_name, forwarded_method);
            return;
        }

        if let Some(source_arg) = source_arg
            && let Some((params_node, body_node)) = Self::define_method_proc_arg_parts(source_arg)
        {
            let mut scope = captured_scope;
            scope.singleton_dispatch = is_singleton;
            scope.method_name = Some(method_name.to_string());
            let (_param_names, param_infos) = match params_node
                .as_ref()
                .and_then(|p| p.as_block_parameters_node())
            {
                Some(bp) => self.collect_define_method_params_from_block_parameters(
                    class_name,
                    &bp,
                    parse_result,
                    &mut scope,
                ),
                None => (Vec::new(), Vec::new()),
            };
            let return_type = match body_node {
                Some(body) => {
                    self.infer_body_return_type(class_name, &body, parse_result, &mut scope)
                }
                None => Type::Nil,
            };
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(method_name),
                    param_infos,
                    raw_return_type: return_type,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: false,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            return;
        }

        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
        {
            let mut scope = captured_scope;
            scope.singleton_dispatch = is_singleton;
            scope.method_name = Some(method_name.to_string());
            let (_param_names, param_infos) = self.collect_define_method_block_params(
                class_name,
                &block,
                parse_result,
                &mut scope,
            );
            let return_type = if let Some(body) = block.body() {
                self.infer_body_return_type(class_name, &body, parse_result, &mut scope)
            } else {
                Type::Nil
            };

            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(method_name),
                    param_infos,
                    raw_return_type: return_type,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: false,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    fn define_method_proc_arg_parts<'n>(
        source_arg: &Node<'n>,
    ) -> Option<(Option<Node<'n>>, Option<Node<'n>>)> {
        if let Some(lambda) = source_arg.as_lambda_node() {
            return Some((lambda.parameters(), lambda.body()));
        }
        let call = source_arg.as_call_node()?;
        let name = String::from_utf8_lossy(call.name().as_slice());
        if !matches!(name.as_ref(), "proc" | "lambda") || call.receiver().is_some() {
            return None;
        }
        let block = call.block()?.as_block_node()?;
        Some((block.parameters(), block.body()))
    }

    fn define_method_from_unbound_method_arg(
        &mut self,
        class_name: &str,
        source_arg: &Node<'_>,
        parse_result: &ParseResult<'_>,
        captured_scope: &Scope,
    ) -> Option<MethodDef> {
        let source_call = source_arg.as_call_node()?;
        let source_method_name = String::from_utf8_lossy(source_call.name().as_slice());
        if !matches!(
            source_method_name.as_ref(),
            "instance_method" | "public_instance_method"
        ) {
            return None;
        }

        let source_args = source_call.arguments()?;
        let source_name_arg = source_args.arguments().iter().next()?;
        let source_names = self.static_dispatch_names_from_arg(
            class_name,
            &source_name_arg,
            parse_result,
            captured_scope,
        )?;
        if source_names.len() != 1 {
            return None;
        }

        let owner_class = if let Some(receiver) = source_call.receiver() {
            match receiver {
                Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                    let raw = self.resolve_constant_path(&receiver, parse_result);
                    let owner = raw.trim_scope_prefix();
                    if owner == "Unknown" || owner.is_empty() {
                        return None;
                    }
                    owner.to_string()
                }
                Node::SelfNode { .. } => class_name.to_string(),
                _ => return None,
            }
        } else {
            class_name.to_string()
        };

        self.ensure_class_available(&owner_class);
        self.lookup_instance_method_def_for_unbound_owner(&owner_class, &source_names[0])
    }

    fn collect_define_method_block_params(
        &mut self,
        class_name: &str,
        block: &ruby_prism::BlockNode<'_>,
        parse_result: &ParseResult<'_>,
        scope: &mut Scope,
    ) -> (Vec<String>, Vec<ParamInfo>) {
        let Some(params_node) = block.parameters() else {
            return (Vec::new(), Vec::new());
        };
        let Some(bp) = params_node.as_block_parameters_node() else {
            return (Vec::new(), Vec::new());
        };
        self.collect_define_method_params_from_block_parameters(
            class_name,
            &bp,
            parse_result,
            scope,
        )
    }

    fn collect_define_method_params_from_block_parameters(
        &mut self,
        class_name: &str,
        bp: &ruby_prism::BlockParametersNode<'_>,
        parse_result: &ParseResult<'_>,
        scope: &mut Scope,
    ) -> (Vec<String>, Vec<ParamInfo>) {
        let mut param_names = Vec::new();
        let mut param_infos = Vec::new();
        let mut positional_idx = 0usize;

        let Some(inner_params) = bp.parameters() else {
            return (param_names, param_infos);
        };

        for req in inner_params.requireds().iter() {
            if let Some(name) = Self::extract_param_name(&req) {
                param_infos.push(ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::Required,
                    default_type: None,
                });
                if !name.is_empty() {
                    scope.set(&name, Type::ParamRef(positional_idx));
                }
                param_names.push(name);
                positional_idx += 1;
            }
        }
        for opt in inner_params.optionals().iter() {
            if let Node::OptionalParameterNode { .. } = &opt {
                let opt_node = opt
                    .as_optional_parameter_node()
                    .expect("must be OptionalParameterNode");
                let name = String::from_utf8_lossy(opt_node.name().as_slice()).to_string();
                let default_type =
                    self.infer_node_type(class_name, &opt_node.value(), parse_result, scope);
                param_infos.push(ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::Optional,
                    default_type: Some(default_type),
                });
                if !name.is_empty() {
                    scope.set(&name, Type::ParamRef(positional_idx));
                }
                param_names.push(name);
                positional_idx += 1;
            }
        }
        if let Some(rest) = inner_params.rest()
            && let Node::RestParameterNode { .. } = &rest
        {
            let rest_node = rest
                .as_rest_parameter_node()
                .expect("must be RestParameterNode");
            let name = rest_node
                .name()
                .map(|name| String::from_utf8_lossy(name.as_slice()).to_string())
                .or_else(|| self.supports_anonymous_rest_params().then(String::new));
            if let Some(name) = name {
                param_infos.push(ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::Rest,
                    default_type: None,
                });
                if !name.is_empty() {
                    scope.set(
                        &name,
                        Type::Array(Some(Box::new(Type::ParamRef(positional_idx)))),
                    );
                }
                param_names.push(name);
                positional_idx += 1;
            }
        }
        for post in inner_params.posts().iter() {
            if let Some(name) = Self::extract_param_name(&post) {
                param_infos.push(ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::Required,
                    default_type: None,
                });
                if !name.is_empty() {
                    scope.set(&name, Type::ParamRef(positional_idx));
                }
                param_names.push(name);
                positional_idx += 1;
            }
        }
        for param in inner_params.keywords().iter() {
            match &param {
                Node::RequiredKeywordParameterNode { .. } => {
                    let kw = param
                        .as_required_keyword_parameter_node()
                        .expect("must be RequiredKeywordParameterNode");
                    let name = String::from_utf8_lossy(kw.name().as_slice()).to_string();
                    param_infos.push(ParamInfo {
                        name: name.clone(),
                        kind: ParamKind::KeywordRequired,
                        default_type: None,
                    });
                    if !name.is_empty() {
                        scope.set(&name, Type::KeywordParamRef(Sym::new(&name)));
                    }
                    param_names.push(name);
                }
                Node::OptionalKeywordParameterNode { .. } => {
                    let kw = param
                        .as_optional_keyword_parameter_node()
                        .expect("must be OptionalKeywordParameterNode");
                    let name = String::from_utf8_lossy(kw.name().as_slice()).to_string();
                    let default_type =
                        self.infer_node_type(class_name, &kw.value(), parse_result, scope);
                    param_infos.push(ParamInfo {
                        name: name.clone(),
                        kind: ParamKind::KeywordOptional,
                        default_type: Some(default_type),
                    });
                    if !name.is_empty() {
                        scope.set(&name, Type::KeywordParamRef(Sym::new(&name)));
                    }
                    param_names.push(name);
                }
                _ => {}
            }
        }
        if let Some(kw_rest) = inner_params.keyword_rest()
            && let Node::KeywordRestParameterNode { .. } = &kw_rest
        {
            let kw_rest_node = kw_rest
                .as_keyword_rest_parameter_node()
                .expect("must be KeywordRestParameterNode");
            let name = kw_rest_node
                .name()
                .map(|name| String::from_utf8_lossy(name.as_slice()).to_string())
                .or_else(|| self.supports_anonymous_rest_params().then(String::new));
            if let Some(name) = name {
                param_infos.push(ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::DoubleRest,
                    default_type: None,
                });
                if !name.is_empty() {
                    scope.set(
                        &name,
                        Type::Hash(Some(Box::new(Type::Symbol)), Some(Box::new(Type::Untyped))),
                    );
                }
                param_names.push(name);
            }
        }
        if let Some(block_param) = inner_params.block() {
            let name = block_param
                .name()
                .map(|name| String::from_utf8_lossy(name.as_slice()).to_string())
                .or_else(|| {
                    self.supports_anonymous_block_forwarding()
                        .then(|| "block".to_string())
                });
            if let Some(name) = name {
                param_infos.push(ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::Block,
                    default_type: None,
                });
                if !name.is_empty() {
                    scope.set(&name, Type::Untyped);
                    scope.current_block_param_name = Some(name.clone());
                }
                param_names.push(name);
            }
        }

        (param_names, param_infos)
    }

    pub(super) fn define_method_static_names(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) -> Option<Vec<String>> {
        self.define_method_static_names_at_arg(class_name, call_node, parse_result, 0, scope)
    }

    pub(super) fn define_method_static_names_at_arg(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        name_arg_index: usize,
        scope: &Scope,
    ) -> Option<Vec<String>> {
        let args = call_node.arguments()?;
        let first_arg = args.arguments().iter().nth(name_arg_index)?;
        self.static_dispatch_names_from_arg(class_name, &first_arg, parse_result, scope)
    }

    pub(super) fn collect_iterator_define_methods(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
        class_body_scope: &Scope,
        is_singleton: bool,
    ) -> bool {
        let Some(receiver) = call_node.receiver() else {
            return false;
        };
        let method_name = String::from_utf8_lossy(call_node.name().as_slice());
        if !matches!(
            method_name.as_ref(),
            "each" | "each_key" | "each_value" | "each_pair"
        ) {
            return false;
        }

        let Some(block_raw) = call_node.block() else {
            return false;
        };
        let Some(block) = block_raw.as_block_node() else {
            return false;
        };

        let Some(iterator_type) = self.static_iterator_receiver_type(
            class_name,
            &receiver,
            parse_result,
            class_body_scope,
        ) else {
            return false;
        };

        let block_param_names = Self::extract_block_param_names(&block);
        let iterator_bindings = Self::extract_iterator_bindings(
            &iterator_type,
            method_name.as_ref(),
            &block_param_names,
        );
        if iterator_bindings.is_empty() {
            return false;
        }

        let Some(body) = block.body() else {
            return false;
        };
        let Some(stmts) = body.as_statements_node() else {
            return false;
        };
        if Self::find_define_method_calls_in_block(&block).is_empty() {
            return false;
        }

        for bindings in iterator_bindings {
            let mut captured_scope = class_body_scope.clone();
            captured_scope.singleton_dispatch = is_singleton;
            for binding in bindings {
                captured_scope.set(&binding.name, binding.ty);
            }
            for node in stmts.body().iter() {
                if let Node::LocalVariableWriteNode { .. } = &node {
                    let write = node
                        .as_local_variable_write_node()
                        .expect("must be LocalVariableWriteNode");
                    let var_name = String::from_utf8_lossy(write.name().as_slice()).to_string();
                    let value_type = self.infer_node_type(
                        class_name,
                        &write.value(),
                        parse_result,
                        &captured_scope,
                    );
                    captured_scope.set(&var_name, value_type);
                    continue;
                }
                let Some(dm_info) = Self::define_method_call_info(&node) else {
                    continue;
                };
                let name_type = self.infer_node_type(
                    class_name,
                    &dm_info.first_arg,
                    parse_result,
                    &captured_scope,
                );
                for method_name in Self::literal_method_names_from_type(&name_type) {
                    self.collect_define_method_named(
                        class_name,
                        &dm_info.call_node,
                        parse_result,
                        comments,
                        is_singleton || dm_info.defines_singleton,
                        &method_name,
                        Some(&dm_info.first_arg),
                        dm_info.source_arg.as_ref(),
                        captured_scope.clone(),
                    );
                }
            }
        }

        true
    }

    fn define_method_call_info<'b>(node: &Node<'b>) -> Option<DynamicDefineMethodInfo<'b>> {
        let Node::CallNode { .. } = node else {
            return None;
        };
        let call = node.as_call_node().expect("must be CallNode");
        let name = String::from_utf8_lossy(call.name().as_slice()).to_string();
        if !matches!(name.as_str(), "define_method" | "define_singleton_method") {
            return None;
        }
        if call
            .receiver()
            .is_some_and(|receiver| !matches!(receiver, Node::SelfNode { .. }))
        {
            return None;
        }
        let args = call.arguments()?;
        let mut arg_iter = args.arguments().iter();
        let first = arg_iter.next()?;
        Some(DynamicDefineMethodInfo {
            call_node: call,
            first_arg: first,
            source_arg: arg_iter.next(),
            defines_singleton: name == "define_singleton_method",
        })
    }

    fn static_iterator_receiver_type(
        &mut self,
        class_name: &str,
        receiver: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) -> Option<Type> {
        match receiver {
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                let const_path = self.resolve_constant_path(receiver, parse_result);
                (const_path != "Unknown")
                    .then(|| self.resolve_constant_type_in_scope(&const_path, class_name))
            }
            Node::LocalVariableReadNode { .. } => {
                let name = Self::extract_local_var_name(receiver)?;
                scope.get(&name).cloned()
            }
            Node::ArrayNode { .. } | Node::HashNode { .. } => {
                Some(self.infer_node_type(class_name, receiver, parse_result, scope))
            }
            _ => None,
        }
    }

    fn extract_iterator_bindings(
        iterator_type: &Type,
        method_name: &str,
        block_param_names: &[String],
    ) -> Vec<Vec<IteratorBinding>> {
        if block_param_names.is_empty() {
            return Vec::new();
        }

        match iterator_type {
            Type::Union(types) => types
                .iter()
                .flat_map(|ty| Self::extract_iterator_bindings(ty, method_name, block_param_names))
                .collect(),
            Type::Tuple(elems) if method_name == "each" => elems
                .iter()
                .map(|elem| {
                    Self::bindings_for_iterator_yield(block_param_names, vec![elem.clone()])
                })
                .collect(),
            Type::Record(fields) => fields
                .iter()
                .filter_map(|field| {
                    let key_type = Self::record_key_literal_type(&field.key);
                    match method_name {
                        "each_key" => Some(Self::bindings_for_iterator_yield(
                            block_param_names,
                            vec![key_type],
                        )),
                        "each_value" => Some(Self::bindings_for_iterator_yield(
                            block_param_names,
                            vec![field.value.clone()],
                        )),
                        "each" | "each_pair" => Some(Self::bindings_for_iterator_yield(
                            block_param_names,
                            vec![key_type, field.value.clone()],
                        )),
                        _ => None,
                    }
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn bindings_for_iterator_yield(
        block_param_names: &[String],
        yielded_values: Vec<Type>,
    ) -> Vec<IteratorBinding> {
        if block_param_names.len() == 1 {
            let ty = if yielded_values.len() == 1 {
                yielded_values.into_iter().next().unwrap_or(Type::Nil)
            } else {
                Type::Tuple(yielded_values)
            };
            return vec![IteratorBinding {
                name: block_param_names[0].clone(),
                ty,
            }];
        }

        let expanded_values = if yielded_values.len() == 1
            && matches!(yielded_values.first(), Some(Type::Tuple(_)))
        {
            match yielded_values.into_iter().next() {
                Some(Type::Tuple(elems)) => elems,
                Some(other) => vec![other],
                None => Vec::new(),
            }
        } else {
            yielded_values
        };

        block_param_names
            .iter()
            .enumerate()
            .map(|(idx, name)| IteratorBinding {
                name: name.clone(),
                ty: expanded_values.get(idx).cloned().unwrap_or(Type::Nil),
            })
            .collect()
    }

    fn record_key_literal_type(key: &RecordKey) -> Type {
        match key {
            RecordKey::Symbol(name) => Type::LiteralSymbol(Sym::new(name)),
            RecordKey::String(name) => Type::LiteralString(name.clone()),
        }
    }

    fn extract_block_param_names(block: &ruby_prism::BlockNode<'_>) -> Vec<String> {
        let Some(params) = block.parameters() else {
            return Vec::new();
        };
        let Some(bp) = params.as_block_parameters_node() else {
            return Vec::new();
        };
        let Some(inner) = bp.parameters() else {
            return Vec::new();
        };
        inner
            .requireds()
            .iter()
            .filter_map(|param| Self::extract_param_name(&param))
            .collect()
    }

    fn find_define_method_calls_in_block<'b>(
        block: &ruby_prism::BlockNode<'b>,
    ) -> Vec<DynamicDefineMethodInfo<'b>> {
        let mut results = Vec::new();
        let Some(body) = block.body() else {
            return results;
        };
        let Some(stmts) = body.as_statements_node() else {
            return results;
        };
        for node in stmts.body().iter() {
            if let Some(info) = Self::define_method_call_info(&node) {
                results.push(info);
            }
        }
        results
    }

    fn literal_method_names_from_type(ty: &Type) -> Vec<String> {
        match ty {
            Type::LiteralSymbol(name) => vec![name.to_string()],
            Type::LiteralString(name) => vec![name.clone()],
            Type::Union(types) => types
                .iter()
                .flat_map(Self::literal_method_names_from_type)
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(super) fn collect_forwardable(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
        is_singleton: bool,
    ) {
        let Some(args) = call_node.arguments() else {
            return;
        };
        let arg_list: Vec<_> = args.arguments().iter().collect();
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let delegate_receiver = arg_list.first().and_then(|arg| {
            self.forwardable_delegate_receiver_type(class_name, arg, parse_result, is_singleton)
        });

        if method_name == "def_delegators" || method_name == "def_instance_delegators" {
            for arg in arg_list.iter().skip(1) {
                if let Node::SymbolNode { .. } = arg {
                    let sym = arg.as_symbol_node().expect("must be SymbolNode");
                    let name = String::from_utf8_lossy(sym.unescaped()).to_string();
                    self.registry.add_method_def(
                        class_name,
                        MethodDef {
                            name: Sym::new(&name),
                            param_infos: Vec::new(),
                            raw_return_type: delegate_receiver
                                .clone()
                                .map(|receiver| {
                                    Type::ReceiverMethodRef(Box::new(receiver), Sym::new(&name))
                                })
                                .unwrap_or(Type::Untyped),
                            sorbet_modifier_comments: Vec::new(),
                            rbs_annotated: false,
                            rbs_inline_annotated: false,
                            sig_annotated: false,
                            attr_ivar: None,
                            is_singleton,
                            rbs_file_source: false,
                            synthetic_dsl_source: false,
                            rbs_method_types: Default::default(),
                            extra_overloads: Vec::new(),
                            loc: Some(loc),
                        },
                    );
                }
            }
        } else if (method_name == "def_delegator" || method_name == "def_instance_delegator")
            && arg_list.len() >= 2
        {
            let delegate_name = if arg_list.len() >= 3 {
                match &arg_list[2] {
                    Node::SymbolNode { .. } => {
                        let sym = arg_list[2].as_symbol_node().expect("must be SymbolNode");
                        String::from_utf8_lossy(sym.unescaped()).to_string()
                    }
                    _ => match &arg_list[1] {
                        Node::SymbolNode { .. } => {
                            let sym = arg_list[1].as_symbol_node().expect("must be SymbolNode");
                            String::from_utf8_lossy(sym.unescaped()).to_string()
                        }
                        _ => return,
                    },
                }
            } else {
                match &arg_list[1] {
                    Node::SymbolNode { .. } => {
                        let sym = arg_list[1].as_symbol_node().expect("must be SymbolNode");
                        String::from_utf8_lossy(sym.unescaped()).to_string()
                    }
                    _ => return,
                }
            };
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(&delegate_name),
                    param_infos: Vec::new(),
                    raw_return_type: delegate_receiver
                        .map(|receiver| {
                            Type::ReceiverMethodRef(Box::new(receiver), Sym::new(&delegate_name))
                        })
                        .unwrap_or(Type::Untyped),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: false,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    fn forwardable_delegate_receiver_type(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        is_singleton: bool,
    ) -> Option<Type> {
        let scope_receiver_type = || {
            if is_singleton {
                Type::Singleton(Sym::new(class_name))
            } else {
                Type::Class(Sym::new(class_name))
            }
        };
        match node {
            Node::SelfNode { .. } => Some(scope_receiver_type()),
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                let const_path = self.resolve_constant_path(node, parse_result);
                let bare = const_path.strip_prefix("::").unwrap_or(&const_path);
                (bare != "Unknown").then(|| Type::Singleton(Sym::new(bare)))
            }
            _ => {
                let name = Self::extract_symbol_literal_name(node)?;
                if name.starts_with('@') {
                    Some(Type::IvarRef(Sym::new(name)))
                } else if name == "self" {
                    Some(scope_receiver_type())
                } else if Self::looks_like_constant_reference(&name) {
                    match self.resolve_constant_type_in_scope(&name, class_name) {
                        Type::Singleton(resolved) => Some(Type::Singleton(resolved)),
                        _ => Some(Type::Singleton(Sym::new(name.trim_scope_prefix()))),
                    }
                } else {
                    Some(Type::ReceiverMethodRef(
                        Box::new(scope_receiver_type()),
                        Sym::new(name),
                    ))
                }
            }
        }
    }

    fn looks_like_constant_reference(name: &str) -> bool {
        let bare = name.strip_prefix("::").unwrap_or(name);
        !bare.is_empty()
            && bare.split("::").all(|segment| {
                segment
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
            })
    }

    pub(super) fn collect_struct_new_members(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
        generate_writer: bool,
    ) {
        let member_names =
            self.static_struct_member_names(class_name, call_node, parse_result, generate_writer);
        if member_names.is_empty() {
            return;
        }
        let keyword_init = if !generate_writer {
            true
        } else {
            Self::extract_hash_option_bool(call_node, "keyword_init", parse_result).unwrap_or(false)
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.registry.mark_user_defined(class_name);
        if let Some(ref fp) = self.file_path {
            let fp = fp.clone();
            self.registry.set_file_path(class_name, &fp);
        }

        let mut init_param_names = Vec::new();
        let mut init_param_infos = Vec::new();

        for name in &member_names {
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Untyped,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: false,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: Some(format!("@{name}")),
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );

            if generate_writer {
                self.registry.add_method_def(
                    class_name,
                    MethodDef {
                        name: Sym::new(format!("{name}=")),
                        param_infos: vec![ParamInfo {
                            name: name.clone(),
                            kind: ParamKind::Required,
                            default_type: None,
                        }],
                        raw_return_type: Type::Untyped,
                        sorbet_modifier_comments: Vec::new(),
                        rbs_annotated: false,
                        rbs_inline_annotated: false,
                        sig_annotated: false,
                        attr_ivar: Some(format!("@{name}")),
                        is_singleton: false,
                        rbs_file_source: false,
                        synthetic_dsl_source: false,
                        rbs_method_types: Default::default(),
                        extra_overloads: Vec::new(),
                        loc: Some(loc),
                    },
                );
            }

            let kind = if keyword_init {
                ParamKind::KeywordRequired
            } else {
                ParamKind::Required
            };
            init_param_names.push(name.clone());
            // Mirrors how the reader is typed via the member type (the `@name` ivar type / call-site derived), by giving initialize's param a default_type that is a deferred ref to the same member ivar, so it resolves through the same path as the reader at render time.
            init_param_infos.push(ParamInfo {
                name: name.clone(),
                kind,
                default_type: Some(Type::IvarRef(Sym::new(format!("@{name}")))),
            });
        }

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new("initialize"),
                param_infos: init_param_infos,
                raw_return_type: Type::Void,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        let member_symbols: Vec<Type> = member_names
            .iter()
            .map(|n| Type::LiteralSymbol(Sym::new(n)))
            .collect();
        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new("members"),
                param_infos: Vec::new(),
                raw_return_type: Type::Array(Some(Box::new(Type::from_type_vec(member_symbols)))),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        if !generate_writer {
            let with_param_infos: Vec<ParamInfo> = member_names
                .iter()
                .map(|name| ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::KeywordOptional,
                    default_type: Some(Type::IvarRef(Sym::new(format!("@{name}")))),
                })
                .collect();
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new("with"),
                    param_infos: with_param_infos,
                    raw_return_type: Type::Class(Sym::new(class_name)),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
        {
            self.collect_class_body_inner(
                class_name,
                &body,
                parse_result,
                comments,
                ClassBodyCollectionOptions::new(false, Scope::default()),
            );
        }
    }

    pub(super) fn is_struct_new_call(node: &Node<'_>, parse_result: &ParseResult<'_>) -> bool {
        if let Node::CallNode { .. } = node {
            let call = node.as_call_node().expect("must be CallNode");
            let method = String::from_utf8_lossy(call.name().as_slice());
            if method == "new"
                && let Some(receiver) = call.receiver()
            {
                let recv_name = Self::resolve_constant_path_static(&receiver, parse_result);
                return recv_name == "Struct";
            }
        }
        false
    }

    pub(super) fn is_data_define_call(node: &Node<'_>, parse_result: &ParseResult<'_>) -> bool {
        if let Node::CallNode { .. } = node {
            let call = node.as_call_node().expect("must be CallNode");
            let method = String::from_utf8_lossy(call.name().as_slice());
            if method == "define"
                && let Some(receiver) = call.receiver()
            {
                let recv_name = Self::resolve_constant_path_static(&receiver, parse_result);
                return recv_name == "Data";
            }
        }
        false
    }

    pub(super) fn class_new_superclass_name(
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        let Node::CallNode { .. } = node else {
            return None;
        };
        let call = node.as_call_node().expect("must be CallNode");
        let method = String::from_utf8_lossy(call.name().as_slice());
        if method != "new" {
            return None;
        }
        let receiver = call.receiver()?;
        if Self::resolve_constant_path_static(&receiver, parse_result) != "Class" {
            return None;
        }
        let args = call.arguments()?;
        let first = args.arguments().iter().next()?;
        if !matches!(
            first,
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. }
        ) {
            return None;
        }
        let name = Self::resolve_constant_path_static(&first, parse_result);
        if name.is_empty() || name == "Unknown" {
            None
        } else {
            Some(name)
        }
    }

    pub(super) fn resolve_constant_path_static(
        node: &Node<'_>,
        _parse_result: &ParseResult<'_>,
    ) -> String {
        match node {
            Node::ConstantReadNode { .. } => {
                let cr = node
                    .as_constant_read_node()
                    .expect("must be ConstantReadNode");
                String::from_utf8_lossy(cr.name().as_slice()).to_string()
            }
            Node::ConstantPathNode { .. } => {
                String::from_utf8_lossy(node.location().as_slice()).to_string()
            }
            _ => String::from_utf8_lossy(node.location().as_slice()).to_string(),
        }
    }
}

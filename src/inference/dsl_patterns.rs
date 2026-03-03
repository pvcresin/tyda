use super::*;
use crate::types::Sym;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn add_accessor_methods(
        &mut self,
        class_name: &str,
        name: &str,
        ty: Type,
        is_singleton: bool,
        loc: SourceLocation,
    ) {
        self.add_simple_method_if_missing(class_name, name, ty.clone(), is_singleton, loc);
        self.add_method_with_param_if_missing(
            class_name,
            &format!("{name}="),
            name,
            ty.clone(),
            ty,
            is_singleton,
            loc,
        );
    }

    pub(super) fn add_simple_method_if_missing(
        &mut self,
        class_name: &str,
        name: &str,
        return_type: Type,
        is_singleton: bool,
        loc: SourceLocation,
    ) {
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(name),
                param_infos: Vec::new(),
                raw_return_type: return_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
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

    #[allow(clippy::too_many_arguments)]
    pub(in crate::inference) fn add_method_with_param_if_missing(
        &mut self,
        class_name: &str,
        name: &str,
        param_name: &str,
        param_type: Type,
        return_type: Type,
        is_singleton: bool,
        loc: SourceLocation,
    ) {
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(name),
                param_infos: vec![ParamInfo {
                    name: param_name.to_string(),
                    kind: ParamKind::Required,
                    default_type: Some(param_type),
                }],
                raw_return_type: return_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
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

    pub(in crate::inference) fn symbol_or_string_args(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
    ) -> Vec<String> {
        let mut names = Self::extract_symbol_args(call_node);
        if let Some(args) = call_node.arguments() {
            for arg in args.arguments().iter() {
                if let Node::StringNode { .. } = &arg {
                    let string = arg.as_string_node().expect("must be StringNode");
                    names.push(String::from_utf8_lossy(string.unescaped()).to_string());
                }
            }
        }
        names
    }

    pub(in crate::inference) fn first_symbol_or_string_arg(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
    ) -> Option<String> {
        self.symbol_or_string_args(call_node).into_iter().next()
    }

    pub(in crate::inference) fn hash_option_bool(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> bool {
        Self::extract_hash_option_bool(call_node, key, parse_result).unwrap_or(false)
    }

    pub(in crate::inference) fn hash_option_type(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Option<Type> {
        let args = call_node.arguments()?;
        for arg in args.arguments().iter() {
            let Node::KeywordHashNode { .. } = &arg else {
                continue;
            };
            let keyword_hash = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
            for element in keyword_hash.elements().iter() {
                let Node::AssocNode { .. } = &element else {
                    continue;
                };
                let assoc = element.as_assoc_node().expect("must be AssocNode");
                let key_name = Self::node_to_symbol_or_label(&assoc.key(), parse_result);
                if key_name.as_deref() == Some(key) {
                    return Some(self.static_node_type(&assoc.value()));
                }
            }
        }
        None
    }

    pub(super) fn has_hash_option(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> bool {
        let Some(args) = call_node.arguments() else {
            return false;
        };
        for arg in args.arguments().iter() {
            let Node::KeywordHashNode { .. } = &arg else {
                continue;
            };
            let keyword_hash = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
            for element in keyword_hash.elements().iter() {
                let Node::AssocNode { .. } = &element else {
                    continue;
                };
                let assoc = element.as_assoc_node().expect("must be AssocNode");
                let key_name = Self::node_to_symbol_or_label(&assoc.key(), parse_result);
                if key_name.as_deref() == Some(key) {
                    return true;
                }
            }
        }
        false
    }

    pub(in crate::inference) fn hash_option_names(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(args) = call_node.arguments() {
            for arg in args.arguments().iter() {
                if let Node::KeywordHashNode { .. } = &arg {
                    let kh = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                    for elem in kh.elements().iter() {
                        if let Node::AssocNode { .. } = &elem {
                            let assoc = elem.as_assoc_node().expect("must be AssocNode");
                            let key_name =
                                Self::node_to_symbol_or_label(&assoc.key(), parse_result);
                            if key_name.as_deref() != Some(key) {
                                continue;
                            }
                            match assoc.value() {
                                Node::ArrayNode { .. } => {
                                    let array =
                                        assoc.value().as_array_node().expect("must be ArrayNode");
                                    for item in array.elements().iter() {
                                        if let Some(name) =
                                            self.node_to_symbol_string_or_constant(&item)
                                        {
                                            names.push(name);
                                        }
                                    }
                                }
                                other => {
                                    if let Some(name) =
                                        self.node_to_symbol_string_or_constant(&other)
                                    {
                                        names.push(name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        names
    }

    pub(in crate::inference) fn node_to_symbol_string_or_constant(
        &self,
        node: &Node<'_>,
    ) -> Option<String> {
        match node {
            Node::SymbolNode { .. } => {
                let sym = node.as_symbol_node().expect("must be SymbolNode");
                Some(String::from_utf8_lossy(sym.unescaped()).to_string())
            }
            Node::StringNode { .. } => {
                let string = node.as_string_node().expect("must be StringNode");
                Some(String::from_utf8_lossy(string.unescaped()).to_string())
            }
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                Some(String::from_utf8_lossy(node.location().as_slice()).to_string())
            }
            _ => None,
        }
    }

    pub(super) fn attribute_declared_type(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Type> {
        let args = call_node.arguments()?;
        let declared = args.arguments().iter().nth(1)?;
        let type_name = Self::node_to_string_or_symbol(&declared, parse_result)?;
        // AR column types use the canonical map; only custom types are camelized (json/jsonb/hstore are explicitly Untyped).
        let base = crate::rails::column_type_keyword_to_type(&type_name)
            .unwrap_or_else(|| Type::Class(Sym::new(self.camelize(&type_name))));
        // `attribute :tags, :string, array: true` is an array column (Postgres array).
        if self.hash_option_bool(call_node, "array", parse_result) {
            Some(Type::Array(Some(Box::new(base))))
        } else {
            Some(base)
        }
    }

    pub(in crate::inference) fn static_node_type(&self, node: &Node<'_>) -> Type {
        match node {
            Node::IntegerNode { .. } => Type::Integer,
            Node::FloatNode { .. } => Type::Float,
            Node::StringNode { .. } => Type::String,
            Node::SymbolNode { .. } => Type::Symbol,
            Node::TrueNode { .. } | Node::FalseNode { .. } => Type::Bool,
            Node::NilNode { .. } => Type::Nil,
            Node::ArrayNode { .. } => {
                let array = node.as_array_node().expect("must be ArrayNode");
                let element_type = Type::from_type_vec(
                    array
                        .elements()
                        .iter()
                        .map(|element| self.static_node_type(&element))
                        .collect(),
                );
                Type::Array(Some(Box::new(element_type)))
            }
            Node::HashNode { .. } => {
                let hash = node.as_hash_node().expect("must be HashNode");
                let mut fields = Vec::new();
                for element in hash.elements().iter() {
                    let Node::AssocNode { .. } = &element else {
                        continue;
                    };
                    let assoc = element.as_assoc_node().expect("must be AssocNode");
                    let Some(name) = self.node_to_symbol_string_or_constant(&assoc.key()) else {
                        continue;
                    };
                    fields.push(crate::types::RecordField {
                        key: crate::types::RecordKey::String(name),
                        value: self.static_node_type(&assoc.value()),
                        optional: false,
                    });
                }
                Type::Record(fields)
            }
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => self
                .node_to_symbol_string_or_constant(node)
                .map(|name| Type::Class(Sym::new(name)))
                .unwrap_or(Type::Untyped),
            _ => Type::Untyped,
        }
    }

    pub(in crate::inference) fn camelize(&self, input: &str) -> String {
        input
            .split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        let mut out = String::new();
                        out.push(first.to_ascii_uppercase());
                        out.push_str(chars.as_str());
                        out
                    }
                    None => String::new(),
                }
            })
            .collect::<String>()
    }
}

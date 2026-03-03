use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct ActiveHash;

static MANIFEST: PluginManifest = PluginManifest {
    id: "active_hash",
    features: &[DslFeature {
        library: DslLibrary::ActiveHash,
        gem_markers: &["active_hash"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for ActiveHash {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn collect_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        _comments: &[super::RbsComment],
    ) -> bool {
        if method_name == "data=" && cx.dsl_enabled(DslLibrary::ActiveHash) {
            cx.collect_active_hash_data_dsl(class_name, call_node, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["data="]
    }
}

use super::super::*;

const ACTIVE_HASH_RELATION_CLASS: &str = "ActiveHash::Relation";

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_active_hash_scope_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.active_hash_dsl_enabled() || !self.is_active_hash_model_class(class_name) {
            return;
        }
        let Some(scope_name) = Self::extract_symbol_args(call_node).into_iter().next() else {
            return;
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.add_simple_method_if_missing(
            class_name,
            &scope_name,
            Type::Generic {
                base: Sym::new(ACTIVE_HASH_RELATION_CLASS),
                args: vec![Type::Class(Sym::new(class_name))].into(),
            },
            true,
            loc,
        );
    }

    pub(in crate::inference) fn collect_active_hash_data_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.active_hash_dsl_enabled() || !self.is_active_hash_model_class(class_name) {
            return;
        }
        let Some(args) = call_node.arguments() else {
            return;
        };
        let Some(data_node) = args.arguments().iter().next() else {
            return;
        };
        let Node::ArrayNode { .. } = &data_node else {
            return;
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let array = data_node.as_array_node().expect("must be ArrayNode");
        let mut fields: std::collections::BTreeMap<String, Type> =
            std::collections::BTreeMap::new();
        for element in array.elements().iter() {
            let Node::HashNode { .. } = &element else {
                continue;
            };
            let hash = element.as_hash_node().expect("must be HashNode");
            for item in hash.elements().iter() {
                let Node::AssocNode { .. } = &item else {
                    continue;
                };
                let assoc = item.as_assoc_node().expect("must be AssocNode");
                let Some(name) = self.node_to_symbol_string_or_constant(&assoc.key()) else {
                    continue;
                };
                let value_type = self.active_hash_field_literal_type(&assoc.value());
                fields
                    .entry(name)
                    .and_modify(|existing| {
                        *existing = existing.clone().union_with(value_type.clone())
                    })
                    .or_insert(value_type);
            }
        }

        for (name, ty) in &fields {
            self.add_accessor_methods(class_name, name, ty.clone(), false, loc);
            self.add_simple_method_if_missing(
                class_name,
                &format!("{name}?"),
                Type::Bool,
                false,
                loc,
            );
            self.add_method_with_param_if_missing(
                class_name,
                &format!("find_by_{name}"),
                name,
                ty.clone(),
                Type::Union(vec![Type::Class(Sym::new(class_name)), Type::Nil]),
                true,
                loc,
            );
            self.add_method_with_param_if_missing(
                class_name,
                &format!("find_all_by_{name}"),
                name,
                ty.clone(),
                Type::Array(Some(Box::new(Type::Class(Sym::new(class_name))))),
                true,
                loc,
            );
        }

        if let Some(id_type) = fields.get("id") {
            self.add_method_with_param_if_missing(
                class_name,
                "find",
                "id",
                id_type.clone(),
                Type::Class(Sym::new(class_name)),
                true,
                loc,
            );
        }
    }

    pub(in crate::inference) fn is_active_hash_model_class(&self, class_name: &str) -> bool {
        let bases = [
            "ActiveHash::Base",
            "ActiveFile::Base",
            "ActiveYaml::Base",
            "ActiveJSON::Base",
        ];
        if bases.contains(&class_name) {
            return true;
        }
        let mut current = self
            .registry
            .class_data_for(class_name)
            .and_then(|data| data.superclass.as_ref().map(ToString::to_string));
        let mut seen = std::collections::HashSet::new();
        while let Some(name) = current {
            if !seen.insert(name.clone()) {
                break;
            }
            if bases.contains(&name.as_str()) {
                return true;
            }
            current = self
                .registry
                .class_data_for(&name)
                .and_then(|data| data.superclass.as_ref().map(ToString::to_string));
        }
        false
    }

    fn active_hash_field_literal_type(&self, node: &Node<'_>) -> Type {
        match node {
            Node::IntegerNode { .. } => {
                let integer = node.as_integer_node().expect("must be IntegerNode").value();
                let (negative, digits) = integer.to_u32_digits();
                Self::literal_integer_type_from_digits(negative, digits)
            }
            Node::FloatNode { .. } => {
                let value = node.as_float_node().expect("must be FloatNode").value();
                Type::LiteralFloat(
                    if value.fract() == 0.0 && !value.is_infinite() && !value.is_nan() {
                        format!("{value:.1}")
                    } else {
                        value.to_string()
                    },
                )
            }
            Node::StringNode { .. } => Type::LiteralString(
                String::from_utf8_lossy(
                    node.as_string_node()
                        .expect("must be StringNode")
                        .unescaped(),
                )
                .to_string(),
            ),
            Node::SymbolNode { .. } => Type::LiteralSymbol(Sym::new(String::from_utf8_lossy(
                node.as_symbol_node()
                    .expect("must be SymbolNode")
                    .unescaped(),
            ))),
            Node::TrueNode { .. } => Type::True,
            Node::FalseNode { .. } => Type::False,
            Node::NilNode { .. } => Type::Nil,
            _ => Type::Untyped,
        }
    }
}

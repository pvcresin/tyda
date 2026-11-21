use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::prism_compat::LocationExt;
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Protobuf;

static MANIFEST: PluginManifest = PluginManifest {
    id: "protobuf",
    features: &[DslFeature {
        library: DslLibrary::Protobuf,
        gem_markers: &["google-protobuf", "protobuf"],
    }],
    base_classes: &[],
    rails_default: false,
};

const FIELD_METHODS: &[&str] = &["optional", "required", "repeated"];

impl Plugin for Protobuf {
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
        if FIELD_METHODS.contains(&method_name) && cx.dsl_enabled(DslLibrary::Protobuf) {
            cx.collect_protobuf_field_dsl(class_name, call_node, method_name, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        FIELD_METHODS
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_protobuf_field_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        // `optional <type>, :name, <tag>`: the type can be a Symbol too, so read the second argument.
        let Some(name) = call_node
            .arguments()
            .and_then(|args| args.arguments().iter().nth(1))
            .and_then(|arg| arg.as_symbol_node())
            .map(|symbol| String::from_utf8_lossy(symbol.unescaped()).to_string())
        else {
            return;
        };
        let (reader, bang) = if method_name == "repeated" {
            let field_array = Type::Class(Sym::new("Protobuf::Field::FieldArray"));
            (
                field_array.clone(),
                Type::Union(vec![field_array, Type::Nil]),
            )
        } else {
            (Type::Untyped, Type::Untyped)
        };
        self.add_simple_method_if_missing(class_name, &name, reader, false, loc);
        self.add_method_with_param_if_missing(
            class_name,
            &format!("{name}="),
            &name,
            Type::Untyped,
            Type::Untyped,
            false,
            loc,
        );
        self.add_simple_method_if_missing(class_name, &format!("{name}!"), bang, false, loc);
    }
}

use super::{DslFeature, MixinKind, Node, ParseResult, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;

pub(super) struct Shrine;

static MANIFEST: PluginManifest = PluginManifest {
    id: "shrine",
    features: &[DslFeature {
        library: DslLibrary::Shrine,
        gem_markers: &["shrine"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Shrine {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn collect_mixin_argument(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        mixin_kind: &MixinKind,
        arg: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> bool {
        if *mixin_kind != MixinKind::Include || !cx.dsl_enabled(DslLibrary::Shrine) {
            return false;
        }
        let Node::CallNode { .. } = arg else {
            return false;
        };
        let call = arg.as_call_node().expect("must be CallNode");
        if String::from_utf8_lossy(call.name().as_slice()) != "Attachment" {
            return false;
        }
        cx.collect_shrine_attachment_mixin(class_name, &call, parse_result);
        true
    }
}

use super::super::*;

const SHRINE_ATTACHER_CLASS: &str = "Shrine::Attacher";
const SHRINE_UPLOADED_FILE_CLASS: &str = "Shrine::UploadedFile";

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_shrine_attachment_mixin(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.shrine_dsl_enabled() {
            return;
        }
        let Some(name) = self.first_symbol_or_string_arg(call_node) else {
            return;
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let uploaded_file_type = Type::Class(Sym::new(SHRINE_UPLOADED_FILE_CLASS));
        self.add_simple_method_if_missing(
            class_name,
            &name,
            uploaded_file_type.clone(),
            false,
            loc,
        );
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(format!("{name}=")),
                param_infos: vec![ParamInfo {
                    name: name.clone(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Union(vec![
                        Type::Class(Sym::new("IO")),
                        Type::String,
                        Type::Hash(Some(Box::new(Type::Untyped)), Some(Box::new(Type::Untyped))),
                    ])),
                }],
                raw_return_type: uploaded_file_type.clone(),
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
        self.add_simple_method_if_missing(
            class_name,
            &format!("{name}_attacher"),
            Type::Class(Sym::new(SHRINE_ATTACHER_CLASS)),
            false,
            loc,
        );
        self.add_simple_method_if_missing(
            class_name,
            &format!("{name}_changed"),
            Type::Bool,
            false,
            loc,
        );
        self.add_simple_method_if_missing(
            class_name,
            &format!("{name}_url"),
            Type::String,
            false,
            loc,
        );
    }
}

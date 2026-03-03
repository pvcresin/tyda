use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Ams;

static MANIFEST: PluginManifest = PluginManifest {
    id: "ams",
    features: &[DslFeature {
        library: DslLibrary::ActiveModelSerializers,
        gem_markers: &["active_model_serializers"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Ams {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        synthetic_method_return(cx, receiver_type, method_name)
    }
}

const SERIALIZER_INSTANCE_METHODS: &[&str] = &[
    "object",
    "scope",
    "current_user",
    "instance_options",
    "serialization_options",
    "root",
    "read_attribute_for_serialization",
];

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    if !engine.dsl_enabled(DslLibrary::ActiveModelSerializers) {
        return None;
    }
    if !SERIALIZER_INSTANCE_METHODS.contains(&method_name) {
        return None;
    }
    engine
        .is_active_model_serializer_class(class_name)
        .then_some(Type::Untyped)
}

use super::super::*;
use crate::registry::IncluderBoundDsl;

impl<'a> InferenceEngine<'a> {
    pub(super) fn collect_active_model_serializer_belongs_to(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.is_active_model_serializer_class(class_name) {
            if self.is_collecting_concern_included() {
                let names = Self::extract_symbol_args(call_node);
                if let Some(assoc_name) = names.first() {
                    let loc = offset_to_location(
                        parse_result.source(),
                        call_node.location().start_offset(),
                    );
                    self.registry.push_includer_bound_dsl(
                        class_name,
                        IncluderBoundDsl::AmsBelongsTo {
                            name: assoc_name.clone(),
                            loc,
                        },
                    );
                }
            }
            return;
        }
        let names = Self::extract_symbol_args(call_node);
        let Some(assoc_name) = names.first() else {
            return;
        };
        let options = Self::extract_association_options(call_node, parse_result);
        let target_class =
            self.infer_association_target_class(class_name, assoc_name, &options, false);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.record_reference(&target_class);
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(assoc_name),
                param_infos: Vec::new(),
                raw_return_type: Type::Class(Sym::new(target_class)),
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

    pub(super) fn collect_active_model_serializer_has_many(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.is_active_model_serializer_class(class_name) {
            if self.is_collecting_concern_included() {
                let names = Self::extract_symbol_args(call_node);
                if let Some(assoc_name) = names.first() {
                    let loc = offset_to_location(
                        parse_result.source(),
                        call_node.location().start_offset(),
                    );
                    self.registry.push_includer_bound_dsl(
                        class_name,
                        IncluderBoundDsl::AmsHasMany {
                            name: assoc_name.clone(),
                            loc,
                        },
                    );
                }
            }
            return;
        }
        let names = Self::extract_symbol_args(call_node);
        let Some(assoc_name) = names.first() else {
            return;
        };
        let options = Self::extract_association_options(call_node, parse_result);
        let target_class =
            self.infer_association_target_class(class_name, assoc_name, &options, true);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.ensure_active_record_query_methods(loc);
        self.record_reference(&target_class);
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(assoc_name),
                param_infos: Vec::new(),
                raw_return_type: Self::active_record_collection_proxy_type(&target_class),
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
}

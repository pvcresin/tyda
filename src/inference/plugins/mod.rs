pub(super) mod aasm;
pub(super) mod action_controller;
pub(super) mod action_text;
pub(super) mod active_hash;
pub(super) mod active_model;
pub(super) mod active_record;
pub(super) mod active_resource;
pub(super) mod active_storage;
pub(super) mod active_support;
pub(super) mod ams;
pub(super) mod declarative_policy;
pub(super) mod devise;
pub(super) mod discard;
pub(super) mod doorkeeper;
pub(super) mod draper;
pub(super) mod exception;
pub(super) mod gettext;
pub(super) mod gitlab_ee;
pub(super) mod gitlab_presenter;
pub(super) mod grape;
pub(super) mod grape_entity;
pub(super) mod graphql;
pub(super) mod identity_cache;
pub(super) mod kredis;
pub(super) mod migration;
pub(super) mod oj;
pub(super) mod properties;
pub(super) mod protobuf;
pub(super) mod rails_configure;
pub(super) mod settingslogic;
pub(super) mod shrine;
pub(super) mod sidekiq;
pub(super) mod state_machines;

use super::{ClassBodyCollectionOptions, InferenceEngine, MixinKind, RbsComment};
use crate::project::DslLibrary;
use crate::registry::TypeRegistry;
use crate::types::{SourceLocation, Type};
use ruby_prism::{CallNode, Node, ParseResult};

static DSL_PLUGIN_DEBUG: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var_os("TYDA_DSL_PLUGIN_DEBUG").is_some());

pub(in crate::inference) use crate::project::{DslFeature, PluginManifest};

pub(in crate::inference) trait Plugin: Sync {
    fn manifest(&self) -> &'static PluginManifest;

    fn synthetic_method_return(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _receiver_type: &Type,
        _method_name: &str,
    ) -> Option<Type> {
        None
    }

    // Returns the gem's function type before a phantom-stub receiver falls back to Kernel; yields if a real definition exists.
    fn synthetic_method_return_override(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _receiver_type: &Type,
        _method_name: &str,
    ) -> Option<Type> {
        None
    }

    // Catch-all used only when every name-based check returns `None`; specific matches take priority.
    fn synthetic_method_return_fallback(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _receiver_type: &Type,
        _method_name: &str,
    ) -> Option<Type> {
        None
    }

    fn collect_class_body_call(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _class_name: &str,
        _method_name: &str,
        _call_node: &CallNode<'_>,
        _parse_result: &ParseResult<'_>,
        _comments: &[RbsComment],
    ) -> bool {
        false
    }

    fn consumes_class_body_call(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _class_name: &str,
        _method_name: &str,
    ) -> bool {
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &[]
    }

    fn class_body_method_prefixes(&self) -> &'static [&'static str] {
        &[]
    }

    fn register_on_class(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _class_name: &str,
        _loc: SourceLocation,
    ) {
    }

    fn register_on_mixin(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _class_name: &str,
        _module_name: &str,
        _loc: SourceLocation,
    ) {
    }

    fn collect_mixin_argument(
        &self,
        _cx: &mut PluginCx<'_, '_>,
        _class_name: &str,
        _mixin_kind: &MixinKind,
        _arg: &Node<'_>,
        _parse_result: &ParseResult<'_>,
    ) -> bool {
        false
    }
}

static PLUGINS: &[&dyn Plugin] = &[
    &grape::Grape,
    &grape_entity::GrapeEntity,
    &declarative_policy::DeclarativePolicy,
    &graphql::Graphql,
    &ams::Ams,
    &gitlab_presenter::GitlabPresenter,
    &gitlab_ee::GitlabEe,
    &migration::Migration,
    &active_record::ActiveRecord,
    &active_model::ActiveModel,
    &devise::Devise,
    &doorkeeper::Doorkeeper,
    &sidekiq::Sidekiq,
    &action_controller::ActionController,
    &rails_configure::RailsConfigure,
    &active_support::ActiveSupport,
    &gettext::Gettext,
    &exception::Exception,
    &aasm::Aasm,
    &state_machines::StateMachines,
    &active_resource::ActiveResource,
    &protobuf::Protobuf,
    &identity_cache::IdentityCache,
    &kredis::Kredis,
    &oj::Oj,
    &draper::Draper,
    &properties::Properties,
    &active_hash::ActiveHash,
    &active_storage::ActiveStorage,
    &action_text::ActionText,
    &discard::Discard,
    &shrine::Shrine,
    &settingslogic::Settingslogic,
];

pub(crate) fn builtin_plugin_manifests() -> impl Iterator<Item = &'static PluginManifest> {
    PLUGINS.iter().map(|plugin| plugin.manifest())
}

// Shortlist of just the overrides for the hot path (scanning all of PLUGINS is ~30 no-op virtual calls).
static OVERRIDE_PLUGINS: &[&dyn Plugin] = &[&oj::Oj];

const EXTRA_CLASS_BODY_DSL_METHODS: &[&str] = &[
    // Sorbet markers only apply when sorbet-runtime is enabled (gated via `is_sorbet_class_body_marker_call`).
    "accepts_nested_attributes_for",
    "after",
    "after_commit",
    "after_create",
    "after_destroy",
    "after_find",
    "after_initialize",
    "after_rollback",
    "after_save",
    "after_update",
    "after_validation",
    "all_or_none_of",
    "around_create",
    "around_destroy",
    "around_save",
    "around_update",
    "around_validation",
    "at_least_one_of",
    "before",
    "before_commit",
    "before_create",
    "before_destroy",
    "before_save",
    "before_update",
    "before_validation",
    "content_type",
    "declared",
    "declared_params",
    "default_scope",
    "delete",
    "desc",
    "detail",
    "exactly_one_of",
    "expose",
    "failure",
    "format",
    "formatter",
    "get",
    "given",
    "helpers",
    "mount",
    "mutually_exclusive",
    "namespace",
    "params",
    "patch",
    "post",
    "prefix",
    "present",
    "put",
    "requires",
    "rescue_from",
    "resource",
    "resources",
    "route_param",
    "route_setting",
    "serialize",
    "sidekiq_options",
    "sidekiq_retries_exhausted",
    "sidekiq_retry_in",
    "success",
    "summary",
    "tags",
    "validate",
    "version",
];

pub(in crate::inference) fn known_class_body_dsl_method(method_name: &str) -> bool {
    PLUGINS.iter().any(|plugin| {
        plugin.class_body_method_names().contains(&method_name)
            || plugin
                .class_body_method_prefixes()
                .iter()
                .any(|prefix| method_name.starts_with(prefix))
    }) || EXTRA_CLASS_BODY_DSL_METHODS.contains(&method_name)
}

pub(in crate::inference) struct PluginCx<'e, 'src> {
    engine: &'e mut InferenceEngine<'src>,
}

impl<'e, 'src> PluginCx<'e, 'src> {
    fn new(engine: &'e mut InferenceEngine<'src>) -> Self {
        Self { engine }
    }

    pub(in crate::inference) fn dsl_enabled(&self, library: DslLibrary) -> bool {
        self.engine.dsl_enabled(library)
    }

    pub(in crate::inference) fn any_dsl_enabled(&self, libraries: &[DslLibrary]) -> bool {
        self.engine.any_dsl_enabled(libraries)
    }

    pub(in crate::inference) fn rails_feature_enabled(&self) -> bool {
        self.engine.rails_feature_enabled()
    }

    pub(in crate::inference) fn class_matches_or_inherits(
        &self,
        class_name: &str,
        bases: &[&str],
    ) -> bool {
        self.engine.class_matches_or_inherits(class_name, bases)
    }

    pub(in crate::inference) fn class_or_ancestors_include_module(
        &self,
        class_name: &str,
        module_name: &str,
    ) -> bool {
        self.engine
            .class_or_ancestors_include_module(class_name, module_name)
    }

    pub(in crate::inference) fn is_active_record_model_class(&self, class_name: &str) -> bool {
        self.engine.is_active_record_model_class(class_name)
    }

    pub(in crate::inference) fn is_action_controller_class(&self, class_name: &str) -> bool {
        self.engine.is_action_controller_class(class_name)
    }

    pub(in crate::inference) fn is_active_model_serializers_model_class(
        &self,
        class_name: &str,
    ) -> bool {
        self.engine
            .is_active_model_serializers_model_class(class_name)
    }

    pub(in crate::inference) fn is_active_model_serializer_class(&self, class_name: &str) -> bool {
        self.engine.is_active_model_serializer_class(class_name)
    }

    pub(in crate::inference) fn registry(&self) -> &TypeRegistry {
        &self.engine.registry
    }

    pub(in crate::inference) fn external_rbs(&self) -> Option<&TypeRegistry> {
        self.engine.external_rbs
    }

    pub(in crate::inference) fn file_path(&self) -> Option<&str> {
        self.engine.file_path.as_deref()
    }

    pub(in crate::inference) fn resolve_constant_path(
        &self,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> String {
        self.engine.resolve_constant_path(node, parse_result)
    }

    pub(in crate::inference) fn record_reference(&mut self, symbol: &str) {
        self.engine.record_reference(symbol);
    }

    pub(in crate::inference) fn resolve_method_on_type(
        &mut self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Type {
        self.engine
            .resolve_method_on_type(receiver_type, method_name)
    }

    pub(in crate::inference) fn collect_class_body_inner(
        &mut self,
        class_name: &str,
        body: &Node<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
        options: ClassBodyCollectionOptions,
    ) {
        self.engine
            .collect_class_body_inner(class_name, body, parse_result, comments, options);
    }

    pub(in crate::inference) fn add_simple_method_if_missing(
        &mut self,
        class_name: &str,
        name: &str,
        return_type: Type,
        is_singleton: bool,
        loc: SourceLocation,
    ) {
        self.engine
            .add_simple_method_if_missing(class_name, name, return_type, is_singleton, loc);
    }

    pub(in crate::inference) fn add_dsl_include_mixin(
        &mut self,
        class_name: &str,
        module_name: &str,
    ) {
        let module_name = module_name.trim_start_matches("::");
        self.engine.record_reference(module_name);
        self.engine.registry.mark_user_defined(class_name);
        self.engine
            .registry
            .add_mixin(class_name, module_name, MixinKind::Include);
    }
}

#[allow(dead_code)]
impl<'e, 'src> PluginCx<'e, 'src> {
    pub(in crate::inference) fn registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.engine.registry
    }

    pub(in crate::inference) fn symbol_or_string_args(
        &self,
        call_node: &CallNode<'_>,
    ) -> Vec<String> {
        self.engine.symbol_or_string_args(call_node)
    }

    pub(in crate::inference) fn first_symbol_or_string_arg(
        &self,
        call_node: &CallNode<'_>,
    ) -> Option<String> {
        self.engine.first_symbol_or_string_arg(call_node)
    }

    pub(in crate::inference) fn extract_symbol_args(
        &self,
        call_node: &CallNode<'_>,
    ) -> Vec<String> {
        InferenceEngine::extract_symbol_args(call_node)
    }

    pub(in crate::inference) fn extract_hash_option_str(
        &self,
        call_node: &CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        InferenceEngine::extract_hash_option_str(call_node, key, parse_result)
    }

    pub(in crate::inference) fn node_to_symbol_string_or_constant(
        &self,
        node: &Node<'_>,
    ) -> Option<String> {
        self.engine.node_to_symbol_string_or_constant(node)
    }

    pub(in crate::inference) fn static_node_type(&self, node: &Node<'_>) -> Type {
        self.engine.static_node_type(node)
    }

    pub(in crate::inference) fn hash_option_names(
        &self,
        call_node: &CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Vec<String> {
        self.engine.hash_option_names(call_node, key, parse_result)
    }

    pub(in crate::inference) fn hash_option_bool(
        &self,
        call_node: &CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> bool {
        self.engine.hash_option_bool(call_node, key, parse_result)
    }

    pub(in crate::inference) fn has_hash_option(
        &self,
        call_node: &CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> bool {
        self.engine.has_hash_option(call_node, key, parse_result)
    }

    pub(in crate::inference) fn attribute_declared_type(
        &self,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Type> {
        self.engine.attribute_declared_type(call_node, parse_result)
    }

    pub(in crate::inference) fn camelize(&self, input: &str) -> String {
        self.engine.camelize(input)
    }

    pub(in crate::inference) fn rails_at_least(&self, major: u16, minor: u16) -> bool {
        self.engine.rails_at_least(major, minor)
    }

    pub(in crate::inference) fn offset_to_location(
        &self,
        source: &[u8],
        offset: usize,
    ) -> SourceLocation {
        super::offset_to_location(source, offset)
    }

    pub(in crate::inference) fn add_accessor_methods(
        &mut self,
        class_name: &str,
        name: &str,
        ty: Type,
        is_singleton: bool,
        loc: SourceLocation,
    ) {
        self.engine
            .add_accessor_methods(class_name, name, ty, is_singleton, loc);
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
        self.engine.add_method_with_param_if_missing(
            class_name,
            name,
            param_name,
            param_type,
            return_type,
            is_singleton,
            loc,
        );
    }
}

impl<'e, 'src> PluginCx<'e, 'src> {
    pub(in crate::inference) fn collect_delegate(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_delegate(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_alias_attribute(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_alias_attribute(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_class_attribute(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_class_attribute(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_mattr(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        generate_reader: bool,
        generate_writer: bool,
    ) {
        self.engine.collect_mattr(
            class_name,
            call_node,
            parse_result,
            generate_reader,
            generate_writer,
        );
    }

    pub(in crate::inference) fn collect_concern_class_methods(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
    ) {
        self.engine
            .collect_concern_class_methods(class_name, call_node, parse_result, comments);
    }

    pub(in crate::inference) fn collect_concern_included(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
    ) {
        self.engine
            .collect_concern_included(class_name, call_node, parse_result, comments);
    }

    pub(in crate::inference) fn collect_belongs_to(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_belongs_to(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_has_many(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_has_many(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_has_one(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_has_one(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_has_and_belongs_to_many(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_has_and_belongs_to_many(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_composed_of(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_composed_of(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_scope_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_scope_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_enum_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_enum_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_attribute_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_attribute_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_attributes_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_attributes_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_active_model_serializer_belongs_to(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_active_model_serializer_belongs_to(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_active_model_serializer_has_many(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_active_model_serializer_has_many(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_store_accessor_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_store_accessor_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_delegated_type_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_delegated_type_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_connects_to(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_connects_to(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_encrypted_attributes(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_encrypted_attributes(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_normalized_attributes(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_normalized_attributes(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_generates_token_for(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_generates_token_for(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_helper_method(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_helper_method(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_action_callback(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_action_callback(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn is_active_hash_model_class(&self, class_name: &str) -> bool {
        self.engine.is_active_hash_model_class(class_name)
    }

    pub(in crate::inference) fn collect_devise_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_devise_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_shrine_attachment_mixin(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_shrine_attachment_mixin(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_aasm_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_aasm_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_state_machine_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_state_machine_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_active_resource_schema_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_active_resource_schema_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_protobuf_field_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_protobuf_field_dsl(class_name, call_node, method_name, parse_result);
    }

    pub(in crate::inference) fn collect_identity_cache_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_identity_cache_dsl(class_name, call_node, method_name, parse_result);
    }

    pub(in crate::inference) fn collect_kredis_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_kredis_dsl(class_name, call_node, method_name, parse_result);
    }

    pub(in crate::inference) fn collect_draper_decorates(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_draper_decorates(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_draper_decorates_finders(&mut self, class_name: &str) {
        self.engine.collect_draper_decorates_finders(class_name);
    }

    pub(in crate::inference) fn collect_draper_delegate_all(&mut self, class_name: &str) {
        self.engine.collect_draper_delegate_all(class_name);
    }

    pub(in crate::inference) fn collect_property_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_property_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_active_hash_scope_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_active_hash_scope_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_active_hash_data_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_active_hash_data_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_active_storage_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_active_storage_dsl(class_name, call_node, method_name, parse_result);
    }

    pub(in crate::inference) fn collect_action_text_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_action_text_dsl(class_name, call_node, method_name, parse_result);
    }

    pub(in crate::inference) fn collect_secure_token_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_secure_token_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_store_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_store_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_typed_store_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_typed_store_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_secure_password_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_secure_password_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_define_model_callbacks(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_define_model_callbacks(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn add_run_callbacks_method(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        self.engine.add_run_callbacks_method(class_name, loc);
    }

    pub(in crate::inference) fn collect_validates_confirmation_of_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_validates_confirmation_of_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_validates_confirmation_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_validates_confirmation_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_validates_presence_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_validates_presence_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_current_attributes_attribute_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_current_attributes_attribute_dsl(class_name, call_node, parse_result);
    }

    pub(in crate::inference) fn collect_argument_like_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_argument_like_dsl(class_name, call_node, method_name, parse_result);
    }

    pub(in crate::inference) fn collect_field_like_dsl(
        &mut self,
        class_name: &str,
        call_node: &CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        self.engine
            .collect_field_like_dsl(class_name, call_node, method_name, parse_result);
    }

    pub(in crate::inference) fn register_current_attributes_class_methods(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        self.engine
            .register_current_attributes_class_methods(class_name, loc);
    }

    pub(in crate::inference) fn register_time_ext_class_methods(&mut self, loc: SourceLocation) {
        self.engine.register_time_ext_class_methods(loc);
    }

    pub(in crate::inference) fn register_draper_class_methods(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        self.engine.register_draper_class_methods(class_name, loc);
    }

    pub(in crate::inference) fn register_sidekiq_mixin_methods(
        &mut self,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        self.engine
            .register_sidekiq_mixin_methods(class_name, module_name, loc);
    }

    pub(in crate::inference) fn register_discard_mixin_methods(
        &mut self,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        self.engine
            .register_discard_mixin_methods(class_name, module_name, loc);
    }

    pub(in crate::inference) fn register_draper_mixin_methods(
        &mut self,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        self.engine
            .register_draper_mixin_methods(class_name, module_name, loc);
    }
}

impl<'a> InferenceEngine<'a> {
    pub(super) fn dsl_plugin_method_return(
        &mut self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let mut cx = PluginCx::new(self);
        let mut result = None;
        let mut matched: Option<&'static PluginManifest> = None;
        for plugin in PLUGINS {
            if let Some(ty) = plugin.synthetic_method_return(&mut cx, receiver_type, method_name) {
                matched = Some(plugin.manifest());
                result = Some(ty);
                break;
            }
        }
        if result.is_none() {
            for plugin in PLUGINS {
                if let Some(ty) =
                    plugin.synthetic_method_return_fallback(&mut cx, receiver_type, method_name)
                {
                    matched = Some(plugin.manifest());
                    result = Some(ty);
                    break;
                }
            }
        }
        if *DSL_PLUGIN_DEBUG {
            let matched = matched.map(|m| (m.id, m.features, m.base_classes, m.rails_default));
            eprintln!(
                "[dsl-plugin] method_return recv={receiver_type:?} m={method_name} -> {result:?} matched={matched:?}"
            );
        }
        result
    }

    pub(super) fn dsl_plugin_method_return_override(
        &mut self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let mut cx = PluginCx::new(self);
        for plugin in OVERRIDE_PLUGINS {
            if let Some(ty) =
                plugin.synthetic_method_return_override(&mut cx, receiver_type, method_name)
            {
                if *DSL_PLUGIN_DEBUG {
                    eprintln!(
                        "[dsl-plugin] method_return_override recv={receiver_type:?} m={method_name} -> {ty:?} matched={:?}",
                        plugin.manifest().id
                    );
                }
                return Some(ty);
            }
        }
        None
    }

    pub(super) fn dsl_plugin_class_body_call(
        &mut self,
        class_name: &str,
        method_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
    ) -> bool {
        if *DSL_PLUGIN_DEBUG {
            eprintln!("[dsl-plugin] class_body class={class_name} m={method_name}");
        }
        let mut cx = PluginCx::new(self);
        for plugin in PLUGINS {
            if plugin.collect_class_body_call(
                &mut cx,
                class_name,
                method_name,
                call_node,
                parse_result,
                comments,
            ) {
                return true;
            }
        }
        false
    }

    pub(super) fn dsl_plugin_consumes_class_body_call(
        &mut self,
        class_name: &str,
        method_name: &str,
    ) -> bool {
        let mut cx = PluginCx::new(self);
        for plugin in PLUGINS {
            if plugin.consumes_class_body_call(&mut cx, class_name, method_name) {
                return true;
            }
        }
        false
    }

    pub(super) fn dsl_plugin_register_on_class(&mut self, class_name: &str, loc: SourceLocation) {
        let mut cx = PluginCx::new(self);
        for plugin in PLUGINS {
            plugin.register_on_class(&mut cx, class_name, loc);
        }
    }

    pub(super) fn dsl_plugin_register_on_mixin(
        &mut self,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        let mut cx = PluginCx::new(self);
        for plugin in PLUGINS {
            plugin.register_on_mixin(&mut cx, class_name, module_name, loc);
        }
    }

    pub(super) fn dsl_plugin_collect_mixin_argument(
        &mut self,
        class_name: &str,
        mixin_kind: &MixinKind,
        arg: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> bool {
        let mut cx = PluginCx::new(self);
        let mut consumed = false;
        for plugin in PLUGINS {
            consumed |=
                plugin.collect_mixin_argument(&mut cx, class_name, mixin_kind, arg, parse_result);
        }
        consumed
    }
}

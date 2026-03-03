use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::{SourceLocation, Sym, Type};
use ruby_prism::{CallNode, ParseResult};

const CLASS_BODY_METHODS: &[&str] = &[
    "delegate",
    "class_attribute",
    "mattr_accessor",
    "cattr_accessor",
    "mattr_reader",
    "cattr_reader",
    "mattr_writer",
    "cattr_writer",
    "class_methods",
    "included",
];

pub(super) struct ActiveSupport;

static MANIFEST: PluginManifest = PluginManifest {
    id: "active_support",
    features: &[
        DslFeature {
            library: DslLibrary::ActiveSupportConcern,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveSupportCurrentAttributes,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveSupportEnvironmentInquirer,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveSupportTimeExt,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::MixedInClassAttributes,
            gem_markers: &[],
        },
    ],
    base_classes: &[],
    rails_default: true,
};

impl Plugin for ActiveSupport {
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

    fn collect_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[super::RbsComment],
    ) -> bool {
        match method_name {
            "delegate" if cx.rails_feature_enabled() => {
                cx.collect_delegate(class_name, call_node, parse_result);
                true
            }
            "class_attribute" if cx.dsl_enabled(DslLibrary::MixedInClassAttributes) => {
                cx.collect_class_attribute(class_name, call_node, parse_result);
                true
            }
            "mattr_accessor" | "cattr_accessor"
                if cx.dsl_enabled(DslLibrary::MixedInClassAttributes) =>
            {
                cx.collect_mattr(class_name, call_node, parse_result, true, true);
                true
            }
            "mattr_reader" | "cattr_reader"
                if cx.dsl_enabled(DslLibrary::MixedInClassAttributes) =>
            {
                cx.collect_mattr(class_name, call_node, parse_result, true, false);
                true
            }
            "mattr_writer" | "cattr_writer"
                if cx.dsl_enabled(DslLibrary::MixedInClassAttributes) =>
            {
                cx.collect_mattr(class_name, call_node, parse_result, false, true);
                true
            }
            "class_methods"
                if cx.dsl_enabled(DslLibrary::ActiveSupportConcern)
                    && call_node.block().is_some() =>
            {
                cx.collect_concern_class_methods(class_name, call_node, parse_result, comments);
                true
            }
            "included"
                if cx.dsl_enabled(DslLibrary::ActiveSupportConcern)
                    && call_node.block().is_some() =>
            {
                cx.collect_concern_included(class_name, call_node, parse_result, comments);
                true
            }
            _ => false,
        }
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        CLASS_BODY_METHODS
    }

    fn register_on_class(&self, cx: &mut PluginCx<'_, '_>, class_name: &str, loc: SourceLocation) {
        cx.register_current_attributes_class_methods(class_name, loc);
        cx.register_time_ext_class_methods(loc);
    }
}

const TIME_WITH_ZONE: &str = "ActiveSupport::TimeWithZone";

fn time_with_zone() -> Type {
    Type::Class(Sym::new(TIME_WITH_ZONE))
}

fn calendar_method_return(method_name: &str, receiver: &Type) -> Option<Type> {
    match method_name {
        "beginning_of_day"
        | "end_of_day"
        | "beginning_of_week"
        | "end_of_week"
        | "beginning_of_month"
        | "end_of_month"
        | "beginning_of_quarter"
        | "end_of_quarter"
        | "beginning_of_year"
        | "end_of_year"
        | "beginning_of_hour"
        | "end_of_hour"
        | "beginning_of_minute"
        | "end_of_minute"
        | "midnight"
        | "midday"
        | "noon"
        | "at_midnight"
        | "at_noon"
        | "yesterday"
        | "tomorrow"
        | "next_day"
        | "prev_day"
        | "next_week"
        | "prev_week"
        | "next_month"
        | "prev_month"
        | "next_year"
        | "prev_year"
        | "weeks_ago"
        | "weeks_since"
        | "months_ago"
        | "months_since"
        | "years_ago"
        | "years_since"
        | "days_ago"
        | "days_since"
        | "advance"
        | "change" => Some(receiver.clone()),
        "ago" | "since" | "in" => Some(receiver.clone()),
        "future?" | "past?" | "today?" | "tomorrow?" | "yesterday?" | "on_weekday?"
        | "on_weekend?" => Some(Type::Bool),
        "in_time_zone" => Some(time_with_zone()),
        "to_fs" | "to_formatted_s" => Some(Type::String),
        "seconds_since_midnight" => Some(Type::Float),
        "all_day" | "all_week" | "all_month" | "all_year" => Some(Type::Untyped),
        _ => None,
    }
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    if !engine.dsl_enabled(DslLibrary::ActiveSupportTimeExt) {
        return None;
    }
    match receiver_type {
        Type::Singleton(name) if name.as_str() == "Time" => match method_name {
            "current" => Some(time_with_zone()),
            "zone" => Some(Type::Class(Sym::new("ActiveSupport::TimeZone"))),
            "zone=" | "use_zone" => Some(Type::Untyped),
            _ => None,
        },
        Type::Singleton(name) if matches!(name.as_str(), "Date" | "DateTime") => {
            match method_name {
                "current" => Some(Type::Class(Sym::new(name.as_str()))),
                _ => None,
            }
        }
        Type::Singleton(name) if name.as_str() == "ActiveSupport::TimeZone" => match method_name {
            "all" => Some(Type::Array(Some(Box::new(Type::Class(Sym::new(
                "ActiveSupport::TimeZone",
            )))))),
            "[]" => Some(Type::Class(Sym::new("ActiveSupport::TimeZone")).union_with(Type::Nil)),
            _ => None,
        },
        Type::Class(name) if name.as_str() == "ActiveSupport::TimeZone" => match method_name {
            "now" => Some(time_with_zone()),
            "today" => Some(Type::Class(Sym::new("Date"))),
            "parse" | "at" | "local" | "iso8601" => Some(time_with_zone().union_with(Type::Nil)),
            "name" | "to_s" => Some(Type::String),
            "utc_offset" => Some(Type::Integer),
            _ => None,
        },
        Type::Class(name) if name.as_str() == TIME_WITH_ZONE => match method_name {
            "utc" | "getutc" | "to_time" | "localtime" => Some(Type::Class(Sym::new("Time"))),
            "to_date" => Some(Type::Class(Sym::new("Date"))),
            "to_datetime" => Some(Type::Class(Sym::new("DateTime"))),
            "strftime" | "iso8601" | "rfc3339" | "rfc2822" | "to_s" | "inspect" | "zone"
            | "httpdate" => Some(Type::String),
            "to_i" | "tv_sec" | "year" | "month" | "mon" | "day" | "mday" | "hour" | "min"
            | "sec" | "wday" | "yday" | "usec" | "nsec" | "utc_offset" => Some(Type::Integer),
            "to_f" => Some(Type::Float),
            "dst?" | "utc?" | "gmt?" | "blank?" | "present?" => Some(Type::Bool),
            "+" | "-" | "time" | "period" | "comparable_time" => Some(Type::Untyped),
            "<=>" => Some(Type::Integer.union_with(Type::Nil)),
            name => calendar_method_return(name, receiver_type),
        },
        Type::Class(name) if matches!(name.as_str(), "Time" | "Date" | "DateTime") => {
            calendar_method_return(method_name, receiver_type)
        }
        _ => None,
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_current_attributes_attribute_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.current_attributes_dsl_enabled() || !self.is_current_attributes_class(class_name) {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        for name in self.symbol_or_string_args(call_node) {
            self.add_accessor_methods(class_name, &name, Type::Untyped, false, loc);
            self.add_accessor_methods(class_name, &name, Type::Untyped, true, loc);
        }
    }

    pub(in crate::inference) fn register_current_attributes_class_methods(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        if self.is_current_attributes_class(class_name) {
            self.add_simple_method_if_missing(
                class_name,
                "instance",
                Type::Class(Sym::new(class_name)),
                true,
                loc,
            );
            self.add_simple_method_if_missing(
                class_name,
                "attributes",
                Type::Hash(Some(Box::new(Type::String)), Some(Box::new(Type::Untyped))),
                true,
                loc,
            );
            self.add_simple_method_if_missing(class_name, "reset", Type::Void, true, loc);
        }
    }

    pub(in crate::inference) fn register_time_ext_class_methods(&mut self, loc: SourceLocation) {
        if self.dsl_enabled(DslLibrary::ActiveSupportTimeExt) {
            self.registry.add_method_def_if_missing(
                "Time",
                MethodDef {
                    name: Sym::new("current"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Class(Sym::new("ActiveSupport::TimeWithZone")),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    fn is_current_attributes_class(&self, class_name: &str) -> bool {
        let mut current = Some(class_name.to_string());
        let mut seen: HashSet<String> = HashSet::new();
        while let Some(name) = current {
            if !seen.insert(name.clone()) {
                break;
            }
            let Some(data) = self.registry.class_data_for(&name) else {
                break;
            };
            if data.superclass.as_deref() == Some("ActiveSupport::CurrentAttributes") {
                return true;
            }
            current = data.superclass.as_ref().map(ToString::to_string);
        }
        false
    }
}

impl<'a> InferenceEngine<'a> {
    pub(super) fn collect_delegate(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());

        // Forwardable#delegate (`extend Forwardable`) hash-rocket form: `delegate [m1, m2] => :accessor` / `delegate %i[..] => :accessor`.
        // Treats an assoc whose key is an array literal as a delegation (key = method names, value = delegation target). Distinguished from ActiveSupport options like `to:` / `prefix:` (label keys) by the array key, so a single symbol key is excluded to avoid misdetection.
        let forwardable_pairs =
            self.forwardable_delegate_pairs(class_name, call_node, parse_result);
        if !forwardable_pairs.is_empty() {
            for (name, target) in forwardable_pairs {
                self.add_delegated_method(class_name, &name, &name, Some(&target), false, loc);
            }
            return;
        }

        let method_names = self.delegate_method_names(class_name, call_node, parse_result);
        let mut nested_options = Self::extract_association_options(call_node, parse_result);
        self.apply_with_options_fallback(&mut nested_options);
        let to_target = Self::extract_hash_option_str(call_node, "to", parse_result)
            .or(nested_options.delegate_to.clone());
        let explicit_prefix = Self::extract_hash_option_str(call_node, "prefix", parse_result)
            .or(nested_options.delegate_prefix.clone());
        let use_target_prefix = Self::extract_hash_option_bool(call_node, "prefix", parse_result)
            .unwrap_or(nested_options.delegate_prefix_target);
        let allow_nil = Self::extract_hash_option_bool(call_node, "allow_nil", parse_result)
            .unwrap_or(nested_options.delegate_allow_nil);
        for name in &method_names {
            let delegated_name = if let Some(prefix) = explicit_prefix.as_deref() {
                format!("{prefix}_{name}")
            } else if use_target_prefix {
                if let Some(target) = to_target.as_deref() {
                    format!("{}_{}", target.trim_start_matches('@'), name)
                } else {
                    name.clone()
                }
            } else {
                name.clone()
            };
            self.add_delegated_method(
                class_name,
                &delegated_name,
                name,
                to_target.as_deref(),
                allow_nil,
                loc,
            );
        }
    }

    fn add_delegated_method(
        &mut self,
        class_name: &str,
        delegated_name: &str,
        orig_name: &str,
        to_target: Option<&str>,
        allow_nil: bool,
        loc: SourceLocation,
    ) {
        let (_param_names, param_infos, return_type) = if let Some(target) = to_target {
            let target_obj_type = self.delegate_target_type(class_name, target);
            let (_param_names, param_infos) =
                self.delegate_param_signature(&target_obj_type, orig_name);
            let mut return_type =
                Type::ReceiverMethodRef(Box::new(target_obj_type.clone()), Sym::new(orig_name));
            if allow_nil
                && self.delegate_target_is_known(&target_obj_type)
                && self.delegate_target_may_be_nil(&target_obj_type)
            {
                return_type = return_type.union_with(Type::Nil);
            }
            (_param_names, param_infos, return_type)
        } else {
            (Vec::new(), Vec::new(), Type::Untyped)
        };
        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(delegated_name),
                param_infos,
                raw_return_type: return_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: false,
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

    fn delegate_method_names(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Vec<String> {
        let mut names = Vec::new();
        let Some(args) = call_node.arguments() else {
            return names;
        };
        for arg in args.arguments().iter() {
            if matches!(arg, ruby_prism::Node::KeywordHashNode { .. }) {
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

    fn forwardable_delegate_pairs(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        let Some(args) = call_node.arguments() else {
            return pairs;
        };
        for arg in args.arguments().iter() {
            let elements = match &arg {
                ruby_prism::Node::KeywordHashNode { .. } => arg
                    .as_keyword_hash_node()
                    .expect("must be KeywordHashNode")
                    .elements(),
                ruby_prism::Node::HashNode { .. } => {
                    arg.as_hash_node().expect("must be HashNode").elements()
                }
                _ => continue,
            };
            for elem in elements.iter() {
                let ruby_prism::Node::AssocNode { .. } = &elem else {
                    continue;
                };
                let assoc = elem.as_assoc_node().expect("must be AssocNode");
                let key = assoc.key();
                if !matches!(key, ruby_prism::Node::ArrayNode { .. }) {
                    continue;
                }
                let Some(target) = Self::extract_symbol_literal_name(&assoc.value()) else {
                    continue;
                };
                if let Some(method_names) =
                    self.static_name_sequence_from_node(class_name, &key, parse_result)
                {
                    for name in method_names {
                        pairs.push((name, target.clone()));
                    }
                }
            }
        }
        pairs
    }
    pub(super) fn collect_class_attribute(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let names = Self::extract_symbol_args(call_node);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());

        let attr_type = Self::extract_hash_option_node(call_node, "default", parse_result)
            .map(|node| self.static_node_type(&node))
            .unwrap_or(Type::Untyped);

        let instance_accessor =
            Self::extract_hash_option_bool(call_node, "instance_accessor", parse_result)
                .unwrap_or(true);
        let instance_reader = instance_accessor
            && Self::extract_hash_option_bool(call_node, "instance_reader", parse_result)
                .unwrap_or(true);
        let instance_writer = instance_accessor
            && Self::extract_hash_option_bool(call_node, "instance_writer", parse_result)
                .unwrap_or(true);

        for name in &names {
            for is_singleton in [false, true] {
                if is_singleton || instance_reader {
                    self.registry.add_method_def(
                        class_name,
                        MethodDef {
                            name: Sym::new(name),
                            param_infos: Vec::new(),
                            raw_return_type: attr_type.clone(),
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
                if is_singleton || instance_writer {
                    self.registry.add_method_def(
                        class_name,
                        MethodDef {
                            name: Sym::new(format!("{name}=")),
                            param_infos: vec![ParamInfo {
                                name: name.clone(),
                                kind: ParamKind::Required,
                                default_type: match &attr_type {
                                    Type::Untyped => None,
                                    other => Some(other.clone()),
                                },
                            }],
                            raw_return_type: attr_type.clone(),
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
                if is_singleton || instance_reader {
                    self.registry.add_method_def(
                        class_name,
                        MethodDef {
                            name: Sym::new(format!("{name}?")),
                            param_infos: Vec::new(),
                            raw_return_type: Type::Bool,
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
        }

        if self.is_active_record_model_class(class_name) {
            let relation_type = Self::active_record_relation_type(class_name);
            for name in [
                "where",
                "reorder",
                "order",
                "joins",
                "includes",
                "left_outer_joins",
                "group",
                "limit",
                "offset",
                "select",
                "merge",
            ] {
                self.registry.add_method_def_if_missing(
                    class_name,
                    MethodDef {
                        name: Sym::new(name),
                        param_infos: vec![ParamInfo {
                            name: "value".to_string(),
                            kind: ParamKind::Rest,
                            default_type: Some(Type::Untyped),
                        }],
                        raw_return_type: relation_type.clone(),
                        sorbet_modifier_comments: Vec::new(),
                        rbs_annotated: true,
                        rbs_inline_annotated: false,
                        sig_annotated: false,
                        attr_ivar: None,
                        is_singleton: true,
                        rbs_file_source: true,
                        synthetic_dsl_source: true,
                        rbs_method_types: Default::default(),
                        extra_overloads: Vec::new(),
                        loc: Some(loc),
                    },
                );
            }
        }
    }

    pub(super) fn collect_mattr(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        generate_reader: bool,
        generate_writer: bool,
    ) {
        let names = Self::extract_symbol_args(call_node);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        for name in &names {
            if generate_reader {
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
        }
    }

    fn set_collecting_concern_included(&mut self, value: bool) -> bool {
        std::mem::replace(&mut self.collecting_concern_included, value)
    }

    pub(in crate::inference) fn with_concern_included_synthetic_marking(
        &mut self,
        class_name: &str,
        f: impl FnOnce(&mut Self),
    ) {
        if !self.is_collecting_concern_included() {
            f(self);
            return;
        }
        let start = self.registry.method_defs_len(class_name);
        f(self);
        self.registry
            .mark_methods_synthetic_dsl_from(class_name, start);
    }

    pub(super) fn collect_concern_class_methods(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
    ) {
        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
        {
            self.collect_class_body_inner(
                class_name,
                &body,
                parse_result,
                comments,
                ClassBodyCollectionOptions::new(true, Scope::default()),
            );
        }
    }

    pub(super) fn collect_concern_included(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
    ) {
        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
        {
            let prev = self.set_collecting_concern_included(true);
            self.collect_class_body_inner(
                class_name,
                &body,
                parse_result,
                comments,
                ClassBodyCollectionOptions::new(false, Scope::default()),
            );
            self.set_collecting_concern_included(prev);
        }
    }
}

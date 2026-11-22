use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::prism_compat::LocationExt;
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct IdentityCache;

static MANIFEST: PluginManifest = PluginManifest {
    id: "identity_cache",
    features: &[DslFeature {
        library: DslLibrary::IdentityCache,
        gem_markers: &["identity_cache"],
    }],
    base_classes: &[],
    rails_default: false,
};

const CACHE_METHODS: &[&str] = &[
    "cache_index",
    "cache_has_many",
    "cache_has_one",
    "cache_belongs_to",
];

impl Plugin for IdentityCache {
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
        if CACHE_METHODS.contains(&method_name) && cx.dsl_enabled(DslLibrary::IdentityCache) {
            cx.collect_identity_cache_dsl(class_name, call_node, method_name, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        CACHE_METHODS
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_identity_cache_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        match method_name {
            "cache_index" => {
                self.collect_identity_cache_index(class_name, call_node, parse_result, loc)
            }
            "cache_has_many" => {
                self.collect_identity_cache_association(class_name, call_node, true, loc)
            }
            _ => self.collect_identity_cache_association(class_name, call_node, false, loc),
        }
    }

    fn collect_identity_cache_index(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        loc: SourceLocation,
    ) {
        let fields = Self::extract_symbol_args(call_node);
        if fields.is_empty() {
            return;
        }
        let field_list = fields.join("_and_");
        let model = Type::Class(Sym::new(class_name));
        let records = Type::Array(Some(Box::new(model.clone())));
        let params = fields
            .iter()
            .map(|field| ParamInfo {
                name: field.clone(),
                kind: ParamKind::Required,
                default_type: Some(Type::Untyped),
            })
            .collect::<Vec<_>>();
        let unique = self.hash_option_bool(call_node, "unique", parse_result);
        let finder_return = if unique {
            Type::Union(vec![model.clone(), Type::Nil])
        } else {
            records.clone()
        };
        let finder = format!("fetch_by_{field_list}");
        self.add_identity_cache_finder(class_name, &finder, params.clone(), finder_return, loc);
        if unique {
            self.add_identity_cache_finder(class_name, &format!("{finder}!"), params, model, loc);
        }
        self.add_method_with_param_if_missing(
            class_name,
            &format!("fetch_multi_by_{field_list}"),
            "index_values",
            Type::Array(Some(Box::new(Type::Untyped))),
            records,
            true,
            loc,
        );
    }

    fn add_identity_cache_finder(
        &mut self,
        class_name: &str,
        name: &str,
        param_infos: Vec<ParamInfo>,
        return_type: Type,
        loc: SourceLocation,
    ) {
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(name),
                param_infos,
                raw_return_type: return_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                origin: MethodOrigin::RubySource,
                attr_ivar: None,
                is_singleton: true,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    // `cache_has_many :orders` defines `fetch_orders`, and `cache_has_one` /
    // `cache_belongs_to` define a nilable `fetch_<name>`.
    fn collect_identity_cache_association(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        collection: bool,
        loc: SourceLocation,
    ) {
        let Some(name) = self.first_symbol_or_string_arg(call_node) else {
            return;
        };
        let target = self.identity_cache_association_target(class_name, &name, collection);
        let return_type = if collection {
            Type::Array(Some(Box::new(target)))
        } else {
            Type::Union(vec![target, Type::Nil])
        };
        self.add_simple_method_if_missing(
            class_name,
            &format!("fetch_{name}"),
            return_type,
            false,
            loc,
        );
    }

    fn identity_cache_association_target(
        &self,
        class_name: &str,
        name: &str,
        collection: bool,
    ) -> Type {
        let declared = self.registry.lookup_method_return_type(class_name, name);
        let target = match declared {
            Some(Type::Generic { args, .. }) if collection && args.len() == 1 => {
                Some(args[0].clone())
            }
            Some(Type::Union(members)) if !collection => {
                let mut non_nil = members.into_iter().filter(|ty| !matches!(ty, Type::Nil));
                match (non_nil.next(), non_nil.next()) {
                    (Some(ty), None) => Some(ty),
                    _ => None,
                }
            }
            Some(ty @ Type::Class(_)) if !collection => Some(ty),
            _ => None,
        };
        target.unwrap_or_else(|| {
            let options = super::super::rails_dsl::AssociationOptions::default();
            Type::Class(Sym::new(self.infer_association_target_class(
                class_name, name, &options, collection,
            )))
        })
    }
}

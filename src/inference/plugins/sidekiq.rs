use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::{SourceLocation, Sym, Type};

pub(super) struct Sidekiq;

static MANIFEST: PluginManifest = PluginManifest {
    id: "sidekiq",
    features: &[DslFeature {
        library: DslLibrary::SidekiqWorker,
        gem_markers: &["sidekiq"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Sidekiq {
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

    fn register_on_mixin(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        cx.register_sidekiq_mixin_methods(class_name, module_name, loc);
    }
}

const WORKER_MODULES: &[&str] = &["Sidekiq::Worker", "Sidekiq::Job", "ApplicationWorker"];

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    if !engine.dsl_enabled(DslLibrary::SidekiqWorker) {
        return None;
    }
    match receiver_type {
        Type::Singleton(name) if name.as_str() == "Sidekiq" => match method_name {
            "logger" => Some(Type::Class(Sym::new("Logger"))),
            "configure_server"
            | "configure_client"
            | "redis"
            | "redis_pool"
            | "options"
            | "strict_args!"
            | "default_job_options"
            | "default_configuration"
            | "server_middleware"
            | "client_middleware"
            | "schedule"
            | "set_schedule" => Some(Type::Untyped),
            _ => None,
        },
        Type::Class(name) => {
            let class_name = name.as_str();
            let is_worker = class_name.ends_with("Worker")
                || class_name.ends_with("Job")
                || WORKER_MODULES
                    .iter()
                    .any(|m| engine.class_or_ancestors_include_module(class_name, m));
            if !is_worker {
                return None;
            }
            match method_name {
                "logger" => Some(Type::Class(Sym::new("Logger"))),
                "jid" => Some(Type::String.union_with(Type::Nil)),
                "bid" => Some(Type::Untyped),
                _ => None,
            }
        }
        _ => None,
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn register_sidekiq_mixin_methods(
        &mut self,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        if self.dsl_enabled(DslLibrary::SidekiqWorker)
            && matches!(module_name, "Sidekiq::Worker" | "Sidekiq::Job")
        {
            self.add_simple_method_if_missing(class_name, "perform_async", Type::String, true, loc);
            self.add_simple_method_if_missing(class_name, "perform_in", Type::String, true, loc);
            self.add_simple_method_if_missing(class_name, "perform_at", Type::String, true, loc);
            self.add_simple_method_if_missing(
                class_name,
                "perform_bulk",
                Type::Array(Some(Box::new(Type::String))),
                true,
                loc,
            );
        }
    }
}

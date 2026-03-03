//! ! ActiveRecord migration DSL (`db/migrate`, `db/post_migrate`, ! `db/schema.rb`) plus the strong_migrations / GitLab migration extensions.
//! ! ! Migration classes inherit `ActiveRecord::Migration[7.1]` — a bracketed ! *call*, so the superclass chain is not statically walkable.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Migration;

static MANIFEST: PluginManifest = PluginManifest {
    id: "migration",
    features: &[DslFeature {
        library: DslLibrary::ActiveRecordMigration,
        gem_markers: &[],
    }],
    base_classes: &["ActiveRecord::Migration"],
    rails_default: true,
};

impl Plugin for Migration {
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

const MIGRATION_METHODS: &[&str] = &[
    "create_table",
    "create_join_table",
    "drop_table",
    "drop_join_table",
    "change_table",
    "rename_table",
    "add_column",
    "remove_column",
    "remove_columns",
    "change_column",
    "change_column_default",
    "change_column_null",
    "change_column_comment",
    "change_table_comment",
    "rename_column",
    "add_index",
    "remove_index",
    "rename_index",
    "add_reference",
    "remove_reference",
    "add_belongs_to",
    "remove_belongs_to",
    "add_foreign_key",
    "remove_foreign_key",
    "foreign_key_exists?",
    "index_exists?",
    "column_exists?",
    "table_exists?",
    "add_check_constraint",
    "remove_check_constraint",
    "add_exclusion_constraint",
    "remove_exclusion_constraint",
    "add_unique_constraint",
    "remove_unique_constraint",
    "add_timestamps",
    "remove_timestamps",
    "enable_extension",
    "disable_extension",
    "create_enum",
    "drop_enum",
    "rename_enum",
    "add_enum_value",
    "rename_enum_value",
    "execute",
    "reversible",
    "revert",
    "up_only",
    "say",
    "say_with_time",
    "suppress_messages",
    "announce",
    "connection",
    "quote",
    "quote_column_name",
    "quote_table_name",
    "disable_ddl_transaction!",
    "safety_assured",
    // GitLab migration framework
    "milestone",
    "restrict_gitlab_migration",
    "enable_lock_retries!",
    "with_lock_retries",
    "finalize_background_migration",
    "queue_batched_background_migration",
    "ensure_batched_background_migration_is_finished",
    "delete_batched_background_migration",
    "finalize_batched_background_migration",
    "add_concurrent_index",
    "remove_concurrent_index",
    "remove_concurrent_index_by_name",
    "add_concurrent_foreign_key",
    "remove_foreign_key_if_exists",
    "validate_foreign_key",
    "add_text_limit",
    "remove_text_limit",
    "index_exists_by_name?",
    "disable_statement_timeout",
    "each_batch",
    "each_batch_range",
    "define_batchable_model",
];

fn in_migration_file(engine: &PluginCx<'_, '_>) -> bool {
    engine.file_path().is_some_and(|path| {
        path.contains("/db/migrate/")
            || path.contains("/db/post_migrate/")
            || path.contains("/db/geo/")
            || path.ends_with("db/schema.rb")
            || path.contains("migration_helpers")
    })
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    if !MIGRATION_METHODS.contains(&method_name) {
        return None;
    }
    if !engine.dsl_enabled(DslLibrary::ActiveRecordMigration) {
        return None;
    }
    let class_name = match receiver_type {
        Type::Class(name) | Type::Singleton(name) => name.as_str(),
        _ => return None,
    };
    let migration_context = in_migration_file(engine)
        || class_name.contains("Migration")
        || engine.class_matches_or_inherits(class_name, &["ActiveRecord::Migration"]);
    migration_context.then_some(Type::Untyped)
}

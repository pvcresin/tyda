pub mod inflector;
mod model_tables;
pub mod parameters;
pub mod project;
pub mod relation_api;
pub mod routes;
pub mod schema;
pub mod time_with_zone;

pub use inflector::{
    CastTypeOwner, cast_type_keyword_to_type, classify, classify_with_known_classes,
    classify_with_project_known_classes, column_type_keyword_to_type, column_type_to_type,
    load_project_inflector, model_file_exists, resolve_class_name_from_owner,
    resolve_model_class_name, singularize, singularize_with_project,
};
pub use project::{detect_rails, load_project_types, load_project_types_with_activation};
pub use schema::load_schema;

use crate::types::Type;

/// ActiveSupport String extensions that return a String.
pub const RAILS_STRING_RETURNING_METHODS: &[&str] = &[
    "squish",
    "strip_heredoc",
    "underscore",
    "camelize",
    "camelcase",
    "titleize",
    "titlecase",
    "dasherize",
    "demodulize",
    "deconstantize",
    "parameterize",
    "tableize",
    "classify",
    "humanize",
    "foreign_key",
    "pluralize",
    "singularize",
    "html_safe",
    "first",
    "last",
    "truncate",
    "truncate_bytes",
    "truncate_words",
    "remove",
    "indent",
    "inquiry",
];

/// Same nullability as `load_schema` accessors: `untyped` stays untyped (union
/// reduction would otherwise collapse `untyped | nil` to `nil`).
pub fn nullable_column_accessor_type(base: &Type, nullable: bool) -> Type {
    if nullable && !matches!(base, Type::Untyped) {
        Type::Union(vec![base.clone(), Type::Nil])
    } else {
        base.clone()
    }
}

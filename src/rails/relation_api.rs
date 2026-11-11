//! Return shapes of the Active Record relation API. One table serves model class
//! delegation (`ActiveRecord::Querying`), `ActiveRecord::Relation`, and association
//! `CollectionProxy` receivers, so every dispatch path agrees on the same Rails 7.x contract.

use crate::types::{Sym, Type};

pub const RELATION_CLASS: &str = "ActiveRecord::Relation";
pub const COLLECTION_PROXY_CLASS: &str = "ActiveRecord::Associations::CollectionProxy";

/// What a relation method returns, in terms of the relation's model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationReturn {
    /// A relation over the same model: the receiver itself, or `Relation[Model]` from the model class.
    Relation,
    Model,
    OptionalModel,
    ModelArray,
    Bool,
    Integer,
    /// `ids`: the primary key values.
    PrimaryKeyArray,
    /// `pluck`: column values; call sites with literal column names refine the element.
    ColumnValues,
    /// `pick` / `minimum` / `maximum`: one column value, or nil for an empty relation.
    OptionalColumnValue,
    ModelEnumerator,
    ModelBatchEnumerator,
    BatchEnumerator,
    Untyped,
}

/// What the method returns when it is called with a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockForm {
    /// The block does not change the return type.
    Same,
    /// Rails falls back to `Enumerable` over the loaded records (`select { }`, `count { }`).
    Records,
    /// The block's own value or nil (`unscoped { }`, `find_each { }`).
    Untyped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationMethod {
    pub returns: RelationReturn,
    /// Delegated from the model class (`Post.where`), not only defined on relations.
    pub on_model: bool,
    /// Minimum Rails `(major, minor)`.
    pub since: (u16, u16),
    pub block: BlockForm,
    /// `async_*` calculations wrap the result in `ActiveRecord::Promise`.
    pub promise: bool,
    /// Call sites refine the result from the arguments or the chain (`find(1, 2)`,
    /// `pluck(:title)`, `group(:x).count`).
    pub refined_by_args: bool,
}

impl RelationMethod {
    const fn new(returns: RelationReturn, on_model: bool) -> Self {
        Self {
            returns,
            on_model,
            since: (0, 0),
            block: BlockForm::Same,
            promise: false,
            refined_by_args: false,
        }
    }

    const fn since(mut self, major: u16, minor: u16) -> Self {
        self.since = (major, minor);
        self
    }

    const fn block(mut self, block: BlockForm) -> Self {
        self.block = block;
        self
    }

    const fn promise(mut self) -> Self {
        self.promise = true;
        self
    }

    const fn refined_by_args(mut self) -> Self {
        self.refined_by_args = true;
        self
    }

    /// `pluck` / `pick` / `minimum` / `maximum`: the result is made of column values.
    pub fn reads_columns(&self) -> bool {
        matches!(
            self.returns,
            RelationReturn::ColumnValues | RelationReturn::OptionalColumnValue
        )
    }

    /// Builds the return type. `relation` is the receiver for a relation, or
    /// `Relation[Model]` for the model class; `primary_key` is the model's id type.
    pub fn return_type(
        &self,
        relation: impl FnOnce() -> Type,
        model: &Type,
        primary_key: impl FnOnce() -> Type,
    ) -> Type {
        let array = |elem: Type| Type::Array(Some(Box::new(elem)));
        let generic = |base: &str, arg: Type| Type::Generic {
            base: Sym::new(base),
            args: vec![arg].into(),
        };
        let ty = match self.returns {
            RelationReturn::Relation => relation(),
            RelationReturn::Model => model.clone(),
            RelationReturn::OptionalModel => Type::Union(vec![model.clone(), Type::Nil]),
            RelationReturn::ModelArray => array(model.clone()),
            RelationReturn::Bool => Type::Bool,
            RelationReturn::Integer => Type::Integer,
            RelationReturn::PrimaryKeyArray => array(primary_key()),
            RelationReturn::ColumnValues => array(Type::Untyped),
            RelationReturn::OptionalColumnValue | RelationReturn::Untyped => Type::Untyped,
            RelationReturn::ModelEnumerator => generic("Enumerator", model.clone()),
            RelationReturn::ModelBatchEnumerator => generic("Enumerator", array(model.clone())),
            RelationReturn::BatchEnumerator => {
                Type::Class(Sym::new("ActiveRecord::Batches::BatchEnumerator"))
            }
        };
        if self.promise {
            generic("ActiveRecord::Promise", ty)
        } else {
            ty
        }
    }
}

/// Element (model) type of a `Relation[Model]` / `CollectionProxy[Model]` receiver.
pub fn relation_element_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Generic { base, args }
            if matches!(base.as_str(), RELATION_CLASS | COLLECTION_PROXY_CLASS) =>
        {
            args.first()
        }
        _ => None,
    }
}

/// `ActiveRecord::Delegation` forwards these to the loaded records; `Enumerable`
/// methods reach the records through the relation's own `each`.
const RECORDS_DELEGATED_METHODS: &[&str] = &[
    "to_xml",
    "encode_with",
    "length",
    "each",
    "join",
    "intersect?",
    "[]",
    "&",
    "|",
    "+",
    "-",
    "sample",
    "reverse",
    "rotate",
    "compact",
    "in_groups",
    "in_groups_of",
    "to_sentence",
    "to_fs",
    "to_formatted_s",
    "as_json",
    "shuffle",
    "split",
    "slice",
    "index",
    "rindex",
];

pub fn delegates_to_records(method_name: &str) -> bool {
    RECORDS_DELEGATED_METHODS.contains(&method_name)
}

pub fn lookup(method_name: &str) -> Option<RelationMethod> {
    use BlockForm as B;
    use RelationReturn as R;
    let query = RelationMethod::new(R::Relation, true);
    let relation_only = |returns| RelationMethod::new(returns, false);
    let model = |returns| RelationMethod::new(returns, true);
    Some(match method_name {
        "all" | "none" | "where" | "rewhere" | "eager_load" | "preload" | "includes"
        | "references" | "reselect" | "reorder" | "order" | "joins" | "left_joins"
        | "left_outer_joins" | "group" | "unscope" | "and" | "or" | "having" | "limit"
        | "offset" | "lock" | "readonly" | "create_with" | "from" | "distinct" | "extending"
        | "optimizer_hints" | "reverse_order" | "annotate" | "merge" | "except" | "only" => query,
        "select" => query.block(B::Records),
        "unscoped" => query.block(B::Untyped),
        "strict_loading" => query.since(6, 1),
        "with" | "in_order_of" | "invert_where" | "excluding" | "without" => query.since(7, 0),
        "with_recursive" | "regroup" => query.since(7, 1),
        "async" | "load_async" => relation_only(R::Relation).since(7, 0),
        "not" | "associated" | "missing" | "distinct!" | "skip_query_cache" => {
            relation_only(R::Relation)
        }
        "load" | "reload" | "reset" | "merge!" | "<<" | "push" | "append" | "concat"
        | "prepend" => relation_only(R::Relation),
        "to_a" | "to_ary" | "records" => relation_only(R::ModelArray),
        "empty?" | "loaded?" => relation_only(R::Bool),
        "size" => relation_only(R::Integer),
        "delete" | "destroy" | "in_order_of_count" => relation_only(R::Integer),
        "insert" | "insert!" | "insert_all" | "insert_all!" | "upsert" | "upsert_all"
        | "replace" | "scoping" | "explain" | "each_batch" => relation_only(R::Untyped),
        "find" => model(R::Model).block(B::Records).refined_by_args(),
        "first" | "last" | "take" => model(R::OptionalModel).refined_by_args(),
        "find_by" | "second" | "third" | "fourth" | "fifth" | "forty_two" | "third_to_last"
        | "second_to_last" => model(R::OptionalModel),
        "find_by!"
        | "first!"
        | "last!"
        | "take!"
        | "sole"
        | "find_sole_by"
        | "second!"
        | "third!"
        | "fourth!"
        | "fifth!"
        | "forty_two!"
        | "third_to_last!"
        | "second_to_last!"
        | "first_or_create"
        | "first_or_create!"
        | "first_or_initialize"
        | "find_or_create_by"
        | "find_or_create_by!"
        | "find_or_initialize_by"
        | "create_or_find_by"
        | "create_or_find_by!"
        | "new"
        | "build"
        | "create"
        | "create!" => model(R::Model),
        "any?" | "many?" | "none?" | "one?" => model(R::Bool).block(B::Records),
        "exists?" | "include?" | "member?" => model(R::Bool),
        "count" => model(R::Integer).block(B::Records).refined_by_args(),
        "update_all" | "delete_all" | "touch_all" | "delete_by" => model(R::Integer),
        "destroy_all" | "destroy_by" => model(R::ModelArray),
        "ids" => model(R::PrimaryKeyArray),
        "pluck" => model(R::ColumnValues).refined_by_args(),
        "pick" | "minimum" | "maximum" => model(R::OptionalColumnValue).refined_by_args(),
        "sum" => model(R::Untyped).block(B::Records),
        "average" | "calculate" => model(R::Untyped),
        "async_count" => model(R::Integer).since(7, 1).promise(),
        "async_ids" => model(R::PrimaryKeyArray).since(7, 1).promise(),
        "async_pick" | "async_pluck" | "async_sum" | "async_average" | "async_minimum"
        | "async_maximum" => model(R::Untyped).since(7, 1).promise(),
        "find_each" => model(R::ModelEnumerator).block(B::Untyped),
        "find_in_batches" => model(R::ModelBatchEnumerator).block(B::Untyped),
        "in_batches" => model(R::BatchEnumerator).block(B::Untyped),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(name: &str) -> Type {
        let relation = Type::Generic {
            base: Sym::new(RELATION_CLASS),
            args: vec![Type::Class(Sym::new("Post"))].into(),
        };
        lookup(name).expect("relation method").return_type(
            || relation.clone(),
            &Type::Class(Sym::new("Post")),
            || Type::String,
        )
    }

    #[test]
    fn query_and_loading_methods_keep_the_relation() {
        for name in ["where", "load", "reload", "unscoped"] {
            assert_eq!(
                resolve(name),
                Type::Generic {
                    base: Sym::new(RELATION_CLASS),
                    args: vec![Type::Class(Sym::new("Post"))].into(),
                },
                "{name}"
            );
        }
    }

    #[test]
    fn record_and_key_methods_use_the_model_and_primary_key() {
        let post = Type::Class(Sym::new("Post"));
        assert_eq!(
            resolve("records"),
            Type::Array(Some(Box::new(post.clone())))
        );
        assert_eq!(resolve("first_or_create!"), post);
        assert_eq!(resolve("ids"), Type::Array(Some(Box::new(Type::String))));
        assert_eq!(
            resolve("async_ids").to_string(),
            "ActiveRecord::Promise[Array[String]]"
        );
    }

    #[test]
    fn model_class_delegation_excludes_relation_only_methods() {
        assert!(lookup("where").is_some_and(|method| method.on_model));
        assert!(lookup("load").is_some_and(|method| !method.on_model));
        assert!(lookup("to_a").is_some_and(|method| !method.on_model));
    }

    #[test]
    fn block_forms_fall_back_to_records() {
        assert_eq!(lookup("select").map(|m| m.block), Some(BlockForm::Records));
        assert_eq!(lookup("where").map(|m| m.block), Some(BlockForm::Same));
        assert_eq!(
            lookup("unscoped").map(|m| m.block),
            Some(BlockForm::Untyped)
        );
    }
}

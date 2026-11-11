//! Return shapes of the Active Record relation API. One table serves model class
//! delegation (`ActiveRecord::Querying`), `ActiveRecord::Relation`, and association
//! `CollectionProxy` receivers, so every dispatch path agrees on the same Rails contract.

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

/// Which object receives the call. `CollectionProxy` inherits `Relation` but overrides
/// some methods with a different contract (`delete` / `destroy`) and adds mutators (`<<`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receiver {
    /// The model class, through `ActiveRecord::Querying` delegation.
    Model,
    Relation,
    CollectionProxy,
}

impl Receiver {
    /// Receiver kind of a `Relation[Model]` / `CollectionProxy[Model]` type.
    pub fn of_relation(ty: &Type) -> Option<Self> {
        match ty {
            Type::Generic { base, .. } => match base.as_str() {
                RELATION_CLASS => Some(Self::Relation),
                COLLECTION_PROXY_CLASS => Some(Self::CollectionProxy),
                _ => None,
            },
            _ => None,
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Model => 1,
            Self::Relation => 2,
            Self::CollectionProxy => 4,
        }
    }
}

const ALL: u8 = 1 | 2 | 4;
const RELATIONS: u8 = 2 | 4;
const PROXY: u8 = 4;

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
    receivers: u8,
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
    const fn new(returns: RelationReturn, receivers: u8) -> Self {
        Self {
            returns,
            receivers,
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

/// The method's contract on `receiver`, or `None` when that receiver does not define it.
pub fn lookup(method_name: &str, receiver: Receiver) -> Option<RelationMethod> {
    use BlockForm as B;
    use RelationReturn as R;
    let query = RelationMethod::new(R::Relation, ALL);
    let relations = |returns| RelationMethod::new(returns, RELATIONS);
    let proxy = |returns| RelationMethod::new(returns, PROXY);
    let model = |returns| RelationMethod::new(returns, ALL);
    let method = match (method_name, receiver) {
        // `Relation` hands these to the model class (`Post.delete(id)` / `Post.destroy(id)`);
        // `CollectionProxy` removes the given records and returns them.
        ("delete", Receiver::Relation) => relations(R::Integer),
        ("destroy", Receiver::Relation) => relations(R::Model),
        ("delete" | "destroy", Receiver::CollectionProxy) => proxy(R::ModelArray),
        _ => match method_name {
            "all" | "none" | "where" | "rewhere" | "eager_load" | "preload" | "includes"
            | "references" | "reorder" | "order" | "joins" | "left_joins" | "left_outer_joins"
            | "group" | "unscope" | "or" | "having" | "limit" | "offset" | "lock" | "readonly"
            | "create_with" | "from" | "distinct" | "extending" | "reverse_order" | "merge"
            | "except" | "only" => query,
            "reselect" | "optimizer_hints" | "annotate" => query.since(6, 0),
            "and" | "strict_loading" => query.since(6, 1),
            "in_order_of" | "invert_where" | "excluding" | "without" => query.since(7, 0),
            "with" | "regroup" => query.since(7, 1),
            "with_recursive" => query.since(7, 2),
            "select" => query.block(B::Records),
            "unscoped" => query.block(B::Untyped),
            "load_async" => relations(R::Relation).since(7, 0),
            // `where.not` / `.associated` / `.missing` read as chained relations.
            "not" | "distinct!" | "skip_query_cache" => relations(R::Relation),
            "missing" => relations(R::Relation).since(6, 1),
            "associated" => relations(R::Relation).since(7, 0),
            "load" | "reload" | "reset" | "merge!" => relations(R::Relation),
            "<<" | "push" | "append" | "concat" => proxy(R::Relation),
            "replace" => proxy(R::Untyped),
            "to_a" | "to_ary" | "records" => relations(R::ModelArray),
            "empty?" | "loaded?" => relations(R::Bool),
            "size" => relations(R::Integer),
            "in_order_of_count" => relations(R::Integer),
            "scoping" | "explain" | "each_batch" => relations(R::Untyped),
            "insert" | "insert!" | "insert_all" | "insert_all!" | "upsert" | "upsert_all" => {
                relations(R::Untyped).since(6, 0)
            }
            "find" => model(R::Model).block(B::Records).refined_by_args(),
            "first" | "last" | "take" => model(R::OptionalModel).refined_by_args(),
            "find_by" | "second" | "third" | "fourth" | "fifth" | "forty_two" | "third_to_last"
            | "second_to_last" => model(R::OptionalModel),
            "find_by!"
            | "first!"
            | "last!"
            | "take!"
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
            | "new"
            | "build"
            | "create"
            | "create!" => model(R::Model),
            "sole" | "find_sole_by" => model(R::Model).since(6, 1),
            "create_or_find_by" | "create_or_find_by!" => model(R::Model).since(6, 0),
            "any?" | "many?" | "none?" | "one?" => model(R::Bool).block(B::Records),
            "exists?" | "include?" | "member?" => model(R::Bool),
            "count" => model(R::Integer).block(B::Records).refined_by_args(),
            "update_all" | "delete_all" => model(R::Integer),
            "touch_all" | "delete_by" => model(R::Integer).since(6, 0),
            "destroy_all" => model(R::ModelArray),
            "destroy_by" => model(R::ModelArray).since(6, 0),
            "ids" => model(R::PrimaryKeyArray),
            "pluck" | "minimum" | "maximum" => model(match method_name {
                "pluck" => R::ColumnValues,
                _ => R::OptionalColumnValue,
            })
            .refined_by_args(),
            "pick" => model(R::OptionalColumnValue).refined_by_args().since(6, 0),
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
        },
    };
    (method.receivers & receiver.bit() != 0).then_some(method)
}

/// `pluck` / `pick` / `minimum` / `maximum`: call sites read the literal column arguments.
pub fn reads_columns(method_name: &str) -> bool {
    lookup(method_name, Receiver::Model).is_some_and(|method| method.reads_columns())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post() -> Type {
        Type::Class(Sym::new("Post"))
    }

    fn generic(base: &str) -> Type {
        Type::Generic {
            base: Sym::new(base),
            args: vec![post()].into(),
        }
    }

    fn resolve(name: &str, receiver: Receiver) -> Option<Type> {
        let base = match receiver {
            Receiver::CollectionProxy => COLLECTION_PROXY_CLASS,
            _ => RELATION_CLASS,
        };
        lookup(name, receiver)
            .map(|method| method.return_type(|| generic(base), &post(), || Type::String))
    }

    #[test]
    fn query_and_loading_methods_keep_the_relation() {
        for name in ["where", "load", "reload", "unscoped"] {
            assert_eq!(
                resolve(name, Receiver::Relation),
                Some(generic(RELATION_CLASS)),
                "{name}"
            );
        }
    }

    #[test]
    fn record_and_key_methods_use_the_model_and_primary_key() {
        let records = Type::Array(Some(Box::new(post())));
        assert_eq!(resolve("records", Receiver::Relation), Some(records));
        assert_eq!(
            resolve("first_or_create!", Receiver::Relation),
            Some(post())
        );
        assert_eq!(
            resolve("ids", Receiver::Model),
            Some(Type::Array(Some(Box::new(Type::String))))
        );
        assert_eq!(
            resolve("async_ids", Receiver::Model).map(|ty| ty.to_string()),
            Some("ActiveRecord::Promise[Array[String]]".to_string())
        );
    }

    #[test]
    fn receivers_only_see_the_methods_they_define() {
        assert!(lookup("where", Receiver::Model).is_some());
        assert!(lookup("load", Receiver::Model).is_none());
        assert!(lookup("to_a", Receiver::Model).is_none());
        assert!(lookup("<<", Receiver::Relation).is_none());
        assert_eq!(
            resolve("<<", Receiver::CollectionProxy),
            Some(generic(COLLECTION_PROXY_CLASS))
        );
        assert!(lookup("prepend", Receiver::CollectionProxy).is_none());
    }

    #[test]
    fn delete_and_destroy_follow_the_receiver_contract() {
        let records = Type::Array(Some(Box::new(post())));
        assert_eq!(resolve("delete", Receiver::Relation), Some(Type::Integer));
        assert_eq!(resolve("destroy", Receiver::Relation), Some(post()));
        assert_eq!(
            resolve("delete", Receiver::CollectionProxy),
            Some(records.clone())
        );
        assert_eq!(resolve("destroy", Receiver::CollectionProxy), Some(records));
        assert!(lookup("destroy", Receiver::Model).is_none());
    }

    #[test]
    fn version_gates_follow_the_rails_changelog() {
        let since = |name| lookup(name, Receiver::Model).map(|method| method.since);
        assert_eq!(since("and"), Some((6, 1)));
        assert_eq!(since("in_order_of"), Some((7, 0)));
        assert_eq!(since("with"), Some((7, 1)));
        assert_eq!(since("with_recursive"), Some((7, 2)));
    }

    #[test]
    fn block_forms_fall_back_to_records() {
        let block = |name| lookup(name, Receiver::Relation).map(|method| method.block);
        assert_eq!(block("select"), Some(BlockForm::Records));
        assert_eq!(block("where"), Some(BlockForm::Same));
        assert_eq!(block("unscoped"), Some(BlockForm::Untyped));
    }
}

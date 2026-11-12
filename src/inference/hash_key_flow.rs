//! Flow facts about keys written to a hash held in a local.
//!
//! A key written through `recv[key] = v`, `recv[key] ||= v`, or `recv[key] += v` is present until
//! `recv` or a non-static `key` is used in any other way. `Hash#[]` then yields the declared value
//! type without its implicit `nil`. A local assigned `{}` also knows its full key set, so a key
//! with a different static value is absent.

use std::collections::HashMap;

use ruby_prism::{Node, Visit};

use super::{InferenceEngine, ParseResult, Scope};
use crate::types::Type;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum HashKeyIdentity {
    Value(Type),
    Local(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct HashKeyFact {
    keys: Vec<HashKeyIdentity>,
    complete: bool,
}

pub(super) type HashKeyFacts = HashMap<String, HashKeyFact>;

fn is_static_key_type(ty: &Type) -> bool {
    match ty {
        Type::LiteralSymbol(_)
        | Type::LiteralString(_)
        | Type::LiteralInteger(_)
        | Type::LiteralFloat(_)
        | Type::True
        | Type::False
        | Type::Nil => true,
        Type::Tuple(elements) => elements.iter().all(is_static_key_type),
        _ => false,
    }
}

fn local_name(node: &Node<'_>) -> Option<String> {
    node.as_local_variable_read_node()
        .map(|read| String::from_utf8_lossy(read.name().as_slice()).to_string())
}

fn key_identity(key_node: &Node<'_>, key_type: &Type) -> Option<HashKeyIdentity> {
    if is_static_key_type(key_type) {
        return Some(HashKeyIdentity::Value(key_type.clone()));
    }
    local_name(key_node).map(HashKeyIdentity::Local)
}

pub(super) fn merge_hash_key_facts(left: &HashKeyFacts, right: &HashKeyFacts) -> HashKeyFacts {
    let mut merged = HashKeyFacts::new();
    for (name, left_fact) in left {
        let Some(right_fact) = right.get(name) else {
            continue;
        };
        let keys: Vec<_> = left_fact
            .keys
            .iter()
            .filter(|key| right_fact.keys.contains(key))
            .cloned()
            .collect();
        let complete = left_fact.complete
            && right_fact.complete
            && keys.len() == left_fact.keys.len()
            && keys.len() == right_fact.keys.len();
        if complete || !keys.is_empty() {
            merged.insert(name.clone(), HashKeyFact { keys, complete });
        }
    }
    merged
}

/// Drops the facts a statement may break. The statement's own index write and index reads keep
/// them; any other use of the receiver or of a non-static key does not.
pub(super) fn invalidate_hash_key_facts(node: &Node<'_>, facts: &mut HashKeyFacts) {
    let mut walker = FactInvalidator { facts };
    if let Some(call) = node.as_call_node()
        && call.name().as_slice() == b"[]="
        && call.block().is_none()
        && call.receiver().as_ref().is_some_and(is_local_read)
    {
        if let Some(arguments) = call.arguments() {
            walker.visit_index_arguments(arguments.arguments().iter());
        }
        return;
    }
    if let Some(write) = node.as_index_or_write_node()
        && write.receiver().as_ref().is_some_and(is_local_read)
    {
        if let Some(arguments) = write.arguments() {
            walker.visit_index_arguments(arguments.arguments().iter());
        }
        walker.visit(&write.value());
        return;
    }
    if let Some(write) = node.as_index_operator_write_node()
        && write.receiver().as_ref().is_some_and(is_local_read)
    {
        if let Some(arguments) = write.arguments() {
            walker.visit_index_arguments(arguments.arguments().iter());
        }
        walker.visit(&write.value());
        return;
    }
    walker.visit(node);
}

fn is_local_read(node: &Node<'_>) -> bool {
    matches!(node, Node::LocalVariableReadNode { .. })
}

struct FactInvalidator<'a> {
    facts: &'a mut HashKeyFacts,
}

impl FactInvalidator<'_> {
    fn drop_name(&mut self, name: &[u8]) {
        let name = String::from_utf8_lossy(name);
        self.facts.remove(name.as_ref());
        for fact in self.facts.values_mut() {
            let before = fact.keys.len();
            fact.keys.retain(
                |key| !matches!(key, HashKeyIdentity::Local(local) if local == name.as_ref()),
            );
            fact.complete &= fact.keys.len() == before;
        }
    }

    /// The first argument is the key position, where reading a local cannot mutate it.
    fn visit_index_arguments<'pr>(&mut self, arguments: impl Iterator<Item = Node<'pr>>) {
        for (index, argument) in arguments.enumerate() {
            if index > 0 || !is_local_read(&argument) {
                self.visit(&argument);
            }
        }
    }
}

impl<'pr> Visit<'pr> for FactInvalidator<'_> {
    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
    fn visit_class_node(&mut self, _node: &ruby_prism::ClassNode<'pr>) {}
    fn visit_module_node(&mut self, _node: &ruby_prism::ModuleNode<'pr>) {}
    fn visit_singleton_class_node(&mut self, _node: &ruby_prism::SingletonClassNode<'pr>) {}

    fn visit_local_variable_read_node(&mut self, node: &ruby_prism::LocalVariableReadNode<'pr>) {
        self.drop_name(node.name().as_slice());
    }

    fn visit_local_variable_write_node(&mut self, node: &ruby_prism::LocalVariableWriteNode<'pr>) {
        self.drop_name(node.name().as_slice());
        self.visit(&node.value());
    }

    fn visit_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'pr>,
    ) {
        self.drop_name(node.name().as_slice());
        self.visit(&node.value());
    }

    fn visit_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'pr>,
    ) {
        self.drop_name(node.name().as_slice());
        self.visit(&node.value());
    }

    fn visit_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'pr>,
    ) {
        self.drop_name(node.name().as_slice());
        self.visit(&node.value());
    }

    fn visit_local_variable_target_node(
        &mut self,
        node: &ruby_prism::LocalVariableTargetNode<'pr>,
    ) {
        self.drop_name(node.name().as_slice());
    }

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        if node.name().as_slice() == b"[]"
            && node.block().is_none()
            && node.receiver().as_ref().is_some_and(is_local_read)
        {
            if let Some(arguments) = node.arguments() {
                self.visit_index_arguments(arguments.arguments().iter());
            }
            return;
        }
        ruby_prism::visit_call_node(self, node);
    }
}

impl InferenceEngine<'_> {
    pub(super) fn record_hash_key_write(
        &mut self,
        class_name: &str,
        receiver: &Node<'_>,
        key_node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &mut Scope,
    ) {
        let Some(receiver_name) = local_name(receiver) else {
            return;
        };
        let key_type = self.infer_node_type(class_name, key_node, parse_result, scope);
        let identity = key_identity(key_node, &key_type);
        let facts = std::sync::Arc::make_mut(&mut scope.hash_key_facts);
        let fact = facts.entry(receiver_name).or_default();
        match identity {
            Some(identity) => {
                if !fact.keys.contains(&identity) {
                    fact.keys.push(identity);
                }
            }
            None => fact.complete = false,
        }
    }

    pub(super) fn record_hash_index_assignment(
        &mut self,
        class_name: &str,
        statement: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &mut Scope,
    ) {
        let Some(call) = statement.as_call_node() else {
            return;
        };
        if call.name().as_slice() != b"[]=" {
            return;
        }
        if let (Some(receiver), Some(arguments)) = (call.receiver(), call.arguments())
            && let Some(key_node) = arguments.arguments().iter().next()
        {
            self.record_hash_key_write(class_name, &receiver, &key_node, parse_result, scope);
        }
    }

    pub(super) fn record_empty_hash_assignment(name: &str, value: &Node<'_>, scope: &mut Scope) {
        if value
            .as_hash_node()
            .is_some_and(|hash| hash.elements().iter().next().is_none())
        {
            std::sync::Arc::make_mut(&mut scope.hash_key_facts).insert(
                name.to_string(),
                HashKeyFact {
                    keys: Vec::new(),
                    complete: true,
                },
            );
        }
    }

    /// The type of `recv[key]` when the facts on `recv` decide whether the key is present.
    pub(super) fn hash_key_fact_read(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) -> Option<Type> {
        let call = node.as_call_node()?;
        if call.name().as_slice() != b"[]" || call.block().is_some() {
            return None;
        }
        let receiver = call.receiver()?;
        let fact = scope.hash_key_facts.get(&local_name(&receiver)?)?.clone();
        let arguments = call.arguments()?;
        let mut arguments = arguments.arguments().iter();
        let key_node = arguments.next()?;
        if arguments.next().is_some() {
            return None;
        }
        let receiver_type = scope.get(&local_name(&receiver)?)?.clone();
        let value_type = match &receiver_type {
            Type::Hash(_, value) => value.as_deref().cloned(),
            // `opts = given || {}`: the given value is not known yet, so the hash decides.
            Type::Union(members) => {
                let mut hashes = members.iter().filter(|m| matches!(m, Type::Hash(..)));
                let Some(Type::Hash(_, value)) = hashes.next() else {
                    return None;
                };
                let others_unknown = members
                    .iter()
                    .all(|m| matches!(m, Type::Hash(..) | Type::Untyped) || !Self::is_concrete(m));
                if hashes.next().is_some() || !others_unknown {
                    return None;
                }
                value.as_deref().cloned()
            }
            _ => return None,
        };
        if self.static_collection_has_source_method_override(&receiver_type, "[]", scope) {
            return None;
        }
        let key_type = self.infer_node_type(class_name, &key_node, parse_result, scope);
        let identity = key_identity(&key_node, &key_type)?;
        if fact.keys.contains(&identity) {
            return value_type;
        }
        let all_static = fact
            .keys
            .iter()
            .all(|key| matches!(key, HashKeyIdentity::Value(_)));
        (fact.complete && all_static && matches!(identity, HashKeyIdentity::Value(_)))
            .then_some(Type::Nil)
    }
}

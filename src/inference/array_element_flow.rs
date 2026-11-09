//! Flow facts about the elements of an array held in a local.
//!
//! A size guard such as `values.size >= 2` or `!values.empty?` gives a lower bound on the size,
//! and a truthy `values[0]` says that index holds a truthy element. `values[i]` within the bound
//! then yields the element type without the implicit `nil`, and `values[start..]` an Array.
//! A fact ends at the next statement that uses the local in any other way, and at any call,
//! `yield`, or `super`, since the array may be reachable from elsewhere.

use std::collections::HashMap;

use ruby_prism::{Node, Visit};

use super::{InferenceEngine, ParseResult, Scope};
use crate::types::Type;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ArrayElementFact {
    min_size: usize,
    truthy_indexes: Vec<i64>,
}

impl ArrayElementFact {
    fn covers(&self, index: i64) -> bool {
        if index >= 0 {
            usize::try_from(index).is_ok_and(|index| index < self.min_size)
        } else {
            usize::try_from(index.unsigned_abs()).is_ok_and(|back| back <= self.min_size)
        }
    }

    /// `values[start, length]` and `values[start..]` return nil only when `start` is past the end.
    fn covers_start(&self, start: i64) -> bool {
        if start >= 0 {
            usize::try_from(start).is_ok_and(|start| start <= self.min_size)
        } else {
            self.covers(start)
        }
    }

    fn add(&mut self, other: &Self) {
        self.min_size = self.min_size.max(other.min_size);
        for index in &other.truthy_indexes {
            if !self.truthy_indexes.contains(index) {
                self.truthy_indexes.push(*index);
            }
        }
    }
}

pub(super) type ArrayElementFacts = HashMap<String, ArrayElementFact>;

pub(super) fn merge_array_element_facts(
    left: &ArrayElementFacts,
    right: &ArrayElementFacts,
) -> ArrayElementFacts {
    let mut merged = ArrayElementFacts::new();
    for (name, left_fact) in left {
        let Some(right_fact) = right.get(name) else {
            continue;
        };
        let fact = ArrayElementFact {
            min_size: left_fact.min_size.min(right_fact.min_size),
            truthy_indexes: left_fact
                .truthy_indexes
                .iter()
                .filter(|index| right_fact.truthy_indexes.contains(index))
                .copied()
                .collect(),
        };
        if fact.min_size > 0 || !fact.truthy_indexes.is_empty() {
            merged.insert(name.clone(), fact);
        }
    }
    merged
}

/// Drops the facts a statement may break. Only `[]` reads on a tracked local keep them.
pub(super) fn invalidate_array_element_facts(node: &Node<'_>, facts: &mut ArrayElementFacts) {
    let mut walker = FactInvalidator { facts };
    walker.visit(node);
}

fn local_name(node: &Node<'_>) -> Option<String> {
    node.as_local_variable_read_node()
        .map(|read| String::from_utf8_lossy(read.name().as_slice()).to_string())
}

struct FactInvalidator<'a> {
    facts: &'a mut ArrayElementFacts,
}

impl FactInvalidator<'_> {
    fn drop_name(&mut self, name: &[u8]) {
        self.facts.remove(String::from_utf8_lossy(name).as_ref());
    }
}

impl<'pr> Visit<'pr> for FactInvalidator<'_> {
    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        if self.facts.is_empty() {
            return;
        }
        let tracked_read = node.name().as_slice() == b"[]"
            && node.block().is_none()
            && node
                .receiver()
                .and_then(|receiver| local_name(&receiver))
                .is_some_and(|name| self.facts.contains_key(&name));
        if !tracked_read {
            self.facts.clear();
            return;
        }
        if let Some(arguments) = node.arguments() {
            self.visit_arguments_node(&arguments);
        }
    }

    fn visit_yield_node(&mut self, _node: &ruby_prism::YieldNode<'pr>) {
        self.facts.clear();
    }

    fn visit_super_node(&mut self, _node: &ruby_prism::SuperNode<'pr>) {
        self.facts.clear();
    }

    fn visit_forwarding_super_node(&mut self, _node: &ruby_prism::ForwardingSuperNode<'pr>) {
        self.facts.clear();
    }

    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {
        self.facts.clear();
    }

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
}

/// `size >= n` style comparisons and the lower bound each outcome implies.
fn size_bound(operator: &[u8], size_on_left: bool, bound: i64, truthy: bool) -> Option<i64> {
    let operator = match (operator, size_on_left) {
        (op, true) => op,
        (b">", false) => b"<",
        (b">=", false) => b"<=",
        (b"<", false) => b">",
        (b"<=", false) => b">=",
        (op, false) => op,
    };
    match (operator, truthy) {
        (b">=", true) | (b"==", true) | (b"<", false) | (b"!=", false) => Some(bound),
        (b">", true) | (b"<=", false) => bound.checked_add(1),
        _ => None,
    }
}

impl InferenceEngine<'_> {
    /// Records the element facts a branch predicate gives when it has the outcome `truthy`.
    pub(super) fn apply_array_element_guard(
        &mut self,
        class_name: &str,
        predicate: &Node<'_>,
        parse_result: &ParseResult<'_>,
        base_scope: &Scope,
        target_scope: &mut Scope,
        truthy: bool,
    ) {
        if let Some(parentheses) = predicate.as_parentheses_node() {
            let Some(body) = parentheses.body() else {
                return;
            };
            let inner = match body.as_statements_node() {
                Some(statements) => {
                    let mut nodes = statements.body().iter();
                    let (Some(inner), None) = (nodes.next(), nodes.next()) else {
                        return;
                    };
                    inner
                }
                None => body,
            };
            self.apply_array_element_guard(
                class_name,
                &inner,
                parse_result,
                base_scope,
                target_scope,
                truthy,
            );
            return;
        }
        if let Some(operand) = Self::negated_operand(predicate) {
            self.apply_array_element_guard(
                class_name,
                &operand,
                parse_result,
                base_scope,
                target_scope,
                !truthy,
            );
            return;
        }
        let both = match predicate {
            Node::AndNode { .. } if truthy => predicate
                .as_and_node()
                .map(|node| (node.left(), node.right())),
            Node::OrNode { .. } if !truthy => predicate
                .as_or_node()
                .map(|node| (node.left(), node.right())),
            _ => None,
        };
        if let Some((left, right)) = both {
            for side in [left, right] {
                self.apply_array_element_guard(
                    class_name,
                    &side,
                    parse_result,
                    base_scope,
                    target_scope,
                    truthy,
                );
            }
            return;
        }
        let Some((name, fact)) =
            self.array_element_guard_fact(class_name, predicate, parse_result, base_scope, truthy)
        else {
            return;
        };
        std::sync::Arc::make_mut(&mut target_scope.array_element_facts)
            .entry(name)
            .or_default()
            .add(&fact);
    }

    fn array_element_guard_fact(
        &mut self,
        class_name: &str,
        predicate: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
        truthy: bool,
    ) -> Option<(String, ArrayElementFact)> {
        let call = predicate.as_call_node()?;
        if call.block().is_some() {
            return None;
        }
        let method = call.name();
        let method = method.as_slice();
        let arguments: Vec<_> = call
            .arguments()
            .map(|arguments| arguments.arguments().iter().collect())
            .unwrap_or_default();
        let receiver = call.receiver()?;
        match (method, arguments.as_slice()) {
            (b"empty?", []) if !truthy => {
                let name = self.guarded_array_local(&receiver, &[b"empty?"], scope)?;
                Some((
                    name,
                    ArrayElementFact {
                        min_size: 1,
                        ..ArrayElementFact::default()
                    },
                ))
            }
            (b"[]", [index]) if truthy => {
                let name = self.guarded_array_local(&receiver, &[], scope)?;
                let index = self.literal_integer(class_name, index, parse_result, scope)?;
                let min_size = if index >= 0 {
                    usize::try_from(index).ok()?.checked_add(1)?
                } else {
                    usize::try_from(index.unsigned_abs()).ok()?
                };
                Some((
                    name,
                    ArrayElementFact {
                        min_size,
                        truthy_indexes: vec![index],
                    },
                ))
            }
            (b">=" | b">" | b"<" | b"<=" | b"==" | b"!=", [other]) => {
                let (size_call, bound_node, size_on_left) =
                    match self.array_size_call_local(&receiver, scope) {
                        Some(name) => (name, other, true),
                        None => (self.array_size_call_local(other, scope)?, &receiver, false),
                    };
                let bound = self.literal_integer(class_name, bound_node, parse_result, scope)?;
                let min_size = size_bound(method, size_on_left, bound, truthy)?;
                let min_size = usize::try_from(min_size).ok().filter(|size| *size > 0)?;
                Some((
                    size_call,
                    ArrayElementFact {
                        min_size,
                        ..ArrayElementFact::default()
                    },
                ))
            }
            _ => None,
        }
    }

    /// The local in `local.size` or `local.length` when it holds an array.
    fn array_size_call_local(&self, node: &Node<'_>, scope: &Scope) -> Option<String> {
        let call = node.as_call_node()?;
        let method = call.name();
        let method = method.as_slice();
        if !matches!(method, b"size" | b"length")
            || call.block().is_some()
            || !Self::call_has_no_arguments(&call)
        {
            return None;
        }
        self.guarded_array_local(&call.receiver()?, &[method], scope)
    }

    /// A local holding `Array[T]` whose `[]` and guard methods are the core ones.
    fn guarded_array_local(
        &self,
        node: &Node<'_>,
        guard_methods: &[&[u8]],
        scope: &Scope,
    ) -> Option<String> {
        let name = local_name(node)?;
        let array_type = scope.get(&name)?;
        if !matches!(array_type, Type::Array(Some(_)))
            || scope.local_singleton_methods.contains_key(&name)
        {
            return None;
        }
        let overridden = std::iter::once(b"[]".as_slice())
            .chain(guard_methods.iter().copied())
            .any(|method| {
                self.type_has_source_instance_method_override(
                    array_type,
                    &String::from_utf8_lossy(method),
                    scope,
                )
            });
        (!overridden).then_some(name)
    }

    /// The type of `local[...]` when the facts on `local` decide that the read is in bounds.
    pub(super) fn array_element_fact_read(
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
        let name = local_name(&call.receiver()?)?;
        let fact = scope.array_element_facts.get(&name)?.clone();
        let array_type = scope.get(&name)?.clone();
        let Type::Array(Some(element)) = &array_type else {
            return None;
        };
        if self.type_has_source_instance_method_override(&array_type, "[]", scope) {
            return None;
        }
        let arguments: Vec<_> = call.arguments()?.arguments().iter().collect();
        match arguments.as_slice() {
            [range] if range.as_range_node().is_some() => {
                let range = range.as_range_node()?;
                let start = match range.left() {
                    Some(left) => self.literal_integer(class_name, &left, parse_result, scope)?,
                    None => 0,
                };
                if let Some(right) = range.right() {
                    self.literal_integer(class_name, &right, parse_result, scope)?;
                }
                fact.covers_start(start).then(|| array_type.clone())
            }
            [index] => {
                let index = self.literal_integer(class_name, index, parse_result, scope)?;
                if fact.truthy_indexes.contains(&index) {
                    return Some(Self::truthy_only(element).unwrap_or(Type::Bot));
                }
                fact.covers(index).then(|| element.as_ref().clone())
            }
            [start, length] => {
                let start = self.literal_integer(class_name, start, parse_result, scope)?;
                let length = self.literal_integer(class_name, length, parse_result, scope)?;
                (length >= 0 && fact.covers_start(start)).then(|| array_type.clone())
            }
            _ => None,
        }
    }

    fn literal_integer(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) -> Option<i64> {
        node.as_integer_node()?;
        match self.infer_node_type(class_name, node, parse_result, scope) {
            Type::LiteralInteger(value) => Some(value),
            _ => None,
        }
    }
}

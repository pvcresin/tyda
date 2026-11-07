//! Flow facts about generated attributes of an object a local exclusively holds.
//!
//! `x = Foo.new` gives the local the only reference to the object. While the local is used only
//! through generated attribute readers and writers, nothing else can reach the object, so a reader
//! returns the value of the last writer. Any other use of the local ends the facts. Values whose
//! shape can change in place keep no exact fact.

use std::collections::HashMap;

use ruby_prism::{Node, Visit};

use super::{InferenceEngine, ParseResult, Scope};
use crate::registry::TypeRegistry;
use crate::types::Type;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExclusiveReceiverFact {
    class_name: String,
    attributes: HashMap<String, Type>,
}

pub(super) type ExclusiveReceiverFacts = HashMap<String, ExclusiveReceiverFact>;

fn generated_attr_ivar<'a>(
    registry: &'a TypeRegistry,
    class_name: &str,
    method_name: &str,
) -> Option<&'a str> {
    let (owner, is_singleton) = registry
        .resolve_method_call_owners(class_name, method_name, false)
        .into_iter()
        .next()?;
    let method = registry.lookup_method_def(&owner, method_name, is_singleton)?;
    if method.has_annotation() {
        return None;
    }
    method.attr_ivar.as_deref()
}

fn is_generated_writer_call(
    registry: &TypeRegistry,
    facts: &ExclusiveReceiverFacts,
    call: &ruby_prism::CallNode<'_>,
) -> bool {
    let method = String::from_utf8_lossy(call.name().as_slice());
    let Some(attribute) = method.strip_suffix('=') else {
        return false;
    };
    let Some(fact) = call
        .receiver()
        .and_then(|receiver| local_name(&receiver))
        .and_then(|name| facts.get(&name))
    else {
        return false;
    };
    let writes_reader_ivar =
        generated_attr_ivar(registry, &fact.class_name, &method).is_some_and(|ivar| {
            generated_attr_ivar(registry, &fact.class_name, attribute) == Some(ivar)
        });
    call.block().is_none()
        && call
            .arguments()
            .is_some_and(|arguments| arguments.arguments().iter().count() == 1)
        && writes_reader_ivar
}

fn local_name(node: &Node<'_>) -> Option<String> {
    node.as_local_variable_read_node()
        .map(|read| String::from_utf8_lossy(read.name().as_slice()).to_string())
}

/// `None` when the value's shape can change in place, so no exact fact holds.
fn stable_attribute_type(ty: &Type) -> Option<Type> {
    match ty {
        Type::Array(_) | Type::Hash(_, _) | Type::Tuple(_) | Type::Record(_) => None,
        Type::LiteralString(_) => Some(Type::String),
        Type::Union(parts) => {
            let parts: Option<Vec<Type>> = parts.iter().map(stable_attribute_type).collect();
            parts.map(Type::from_type_vec_preserve_untyped)
        }
        _ => Some(ty.clone()),
    }
}

pub(super) fn merge_exclusive_receiver_facts(
    left: &ExclusiveReceiverFacts,
    right: &ExclusiveReceiverFacts,
) -> ExclusiveReceiverFacts {
    let mut merged = ExclusiveReceiverFacts::new();
    for (name, left_fact) in left {
        let Some(right_fact) = right.get(name) else {
            continue;
        };
        if left_fact.class_name != right_fact.class_name {
            continue;
        }
        let attributes = left_fact
            .attributes
            .iter()
            .filter_map(|(attribute, left_type)| {
                let right_type = right_fact.attributes.get(attribute)?;
                let joined = left_type.clone().union_with(right_type.clone());
                Some((attribute.clone(), joined))
            })
            .collect();
        merged.insert(
            name.clone(),
            ExclusiveReceiverFact {
                class_name: left_fact.class_name.clone(),
                attributes,
            },
        );
    }
    merged
}

struct ExclusiveUseWalker<'a> {
    registry: &'a TypeRegistry,
    facts: &'a mut ExclusiveReceiverFacts,
    /// A writer statement in an `if` / `case` arm updates the arm's own facts, which the join
    /// then combines. Loops, closures, and rescuable bodies re-enter or skip it, so it ends them.
    in_repeatable_flow: bool,
}

impl ExclusiveUseWalker<'_> {
    fn visit_repeatable(&mut self, visit: impl FnOnce(&mut Self)) {
        let saved = std::mem::replace(&mut self.in_repeatable_flow, true);
        visit(self);
        self.in_repeatable_flow = saved;
    }

    fn is_generated_reader_call(&self, call: &ruby_prism::CallNode<'_>) -> bool {
        let Some(fact) = call
            .receiver()
            .and_then(|receiver| local_name(&receiver))
            .and_then(|name| self.facts.get(&name))
        else {
            return false;
        };
        let method = String::from_utf8_lossy(call.name().as_slice());
        call.arguments().is_none()
            && call.block().is_none()
            && generated_attr_ivar(self.registry, &fact.class_name, &method).is_some()
    }
}

impl<'pr> Visit<'pr> for ExclusiveUseWalker<'_> {
    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
    fn visit_class_node(&mut self, _node: &ruby_prism::ClassNode<'pr>) {}
    fn visit_module_node(&mut self, _node: &ruby_prism::ModuleNode<'pr>) {}
    fn visit_singleton_class_node(&mut self, _node: &ruby_prism::SingletonClassNode<'pr>) {}

    fn visit_local_variable_read_node(&mut self, node: &ruby_prism::LocalVariableReadNode<'pr>) {
        self.facts
            .remove(String::from_utf8_lossy(node.name().as_slice()).as_ref());
    }

    fn visit_local_variable_write_node(&mut self, node: &ruby_prism::LocalVariableWriteNode<'pr>) {
        self.facts
            .remove(String::from_utf8_lossy(node.name().as_slice()).as_ref());
        self.visit(&node.value());
    }

    fn visit_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'pr>,
    ) {
        self.facts
            .remove(String::from_utf8_lossy(node.name().as_slice()).as_ref());
        self.visit(&node.value());
    }

    fn visit_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'pr>,
    ) {
        self.facts
            .remove(String::from_utf8_lossy(node.name().as_slice()).as_ref());
        self.visit(&node.value());
    }

    fn visit_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'pr>,
    ) {
        self.facts
            .remove(String::from_utf8_lossy(node.name().as_slice()).as_ref());
        self.visit(&node.value());
    }

    fn visit_local_variable_target_node(
        &mut self,
        node: &ruby_prism::LocalVariableTargetNode<'pr>,
    ) {
        self.facts
            .remove(String::from_utf8_lossy(node.name().as_slice()).as_ref());
    }

    fn visit_statements_node(&mut self, node: &ruby_prism::StatementsNode<'pr>) {
        for statement in node.body().iter() {
            if !self.in_repeatable_flow
                && let Some(call) = statement.as_call_node()
                && is_generated_writer_call(self.registry, self.facts, &call)
            {
                if let Some(arguments) = call.arguments() {
                    self.visit_arguments_node(&arguments);
                }
            } else {
                self.visit(&statement);
            }
        }
    }

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        if !self.is_generated_reader_call(node) {
            ruby_prism::visit_call_node(self, node);
        }
    }

    fn visit_while_node(&mut self, node: &ruby_prism::WhileNode<'pr>) {
        self.visit_repeatable(|walker| ruby_prism::visit_while_node(walker, node));
    }

    fn visit_until_node(&mut self, node: &ruby_prism::UntilNode<'pr>) {
        self.visit_repeatable(|walker| ruby_prism::visit_until_node(walker, node));
    }

    fn visit_for_node(&mut self, node: &ruby_prism::ForNode<'pr>) {
        self.visit_repeatable(|walker| ruby_prism::visit_for_node(walker, node));
    }

    fn visit_block_node(&mut self, node: &ruby_prism::BlockNode<'pr>) {
        self.visit_repeatable(|walker| ruby_prism::visit_block_node(walker, node));
    }

    fn visit_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'pr>) {
        self.visit_repeatable(|walker| ruby_prism::visit_lambda_node(walker, node));
    }

    fn visit_begin_node(&mut self, node: &ruby_prism::BeginNode<'pr>) {
        self.visit_repeatable(|walker| ruby_prism::visit_begin_node(walker, node));
    }
}

impl InferenceEngine<'_> {
    /// Ends exclusivity on any use other than a generated reader, then applies a top-level
    /// generated writer statement.
    pub(super) fn update_exclusive_receiver_facts_for_statement(
        &mut self,
        class_name: &str,
        statement: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &mut Scope,
    ) {
        if scope.exclusive_receiver_facts.is_empty() {
            return;
        }
        let writer = statement.as_call_node().filter(|call| {
            is_generated_writer_call(&self.registry, &scope.exclusive_receiver_facts, call)
        });
        if let Some(call) = &writer {
            let facts = std::sync::Arc::make_mut(&mut scope.exclusive_receiver_facts);
            let mut walker = ExclusiveUseWalker {
                registry: &self.registry,
                facts,
                in_repeatable_flow: false,
            };
            if let Some(arguments) = call.arguments() {
                walker.visit_arguments_node(&arguments);
            }
        } else {
            let facts = std::sync::Arc::make_mut(&mut scope.exclusive_receiver_facts);
            let mut walker = ExclusiveUseWalker {
                registry: &self.registry,
                facts,
                in_repeatable_flow: false,
            };
            walker.visit(statement);
        }
        let Some(call) = writer else {
            return;
        };
        let Some(receiver) = call.receiver().and_then(|receiver| local_name(&receiver)) else {
            return;
        };
        let Some(value) = call
            .arguments()
            .and_then(|arguments| arguments.arguments().iter().next())
        else {
            return;
        };
        let value_type = self.infer_node_type(class_name, &value, parse_result, scope);
        let method = String::from_utf8_lossy(call.name().as_slice());
        let attribute = method.trim_end_matches('=').to_string();
        if let Some(fact) =
            std::sync::Arc::make_mut(&mut scope.exclusive_receiver_facts).get_mut(&receiver)
        {
            match stable_attribute_type(&value_type) {
                Some(stable) => {
                    fact.attributes.insert(attribute, stable);
                }
                None => {
                    fact.attributes.remove(&attribute);
                }
            }
        }
    }

    pub(super) fn record_exclusive_receiver_assignment(
        &self,
        name: &str,
        value: &Node<'_>,
        value_type: &Type,
        scope: &mut Scope,
    ) {
        let Some(call) = value.as_call_node() else {
            return;
        };
        let constant_receiver = call.receiver().is_some_and(|receiver| {
            matches!(
                receiver,
                Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. }
            )
        });
        let Type::Class(class_name) = value_type else {
            return;
        };
        if call.name().as_slice() != b"new"
            || !constant_receiver
            || call.block().is_some()
            || self.registry.method_call_has_user_source_definition(
                class_name.as_ref(),
                "new",
                true,
            )
        {
            return;
        }
        std::sync::Arc::make_mut(&mut scope.exclusive_receiver_facts).insert(
            name.to_string(),
            ExclusiveReceiverFact {
                class_name: class_name.to_string(),
                attributes: HashMap::new(),
            },
        );
    }

    /// The value of the last generated writer when `node` reads the same attribute.
    pub(super) fn exclusive_receiver_attribute_read(
        node: &Node<'_>,
        scope: &Scope,
    ) -> Option<Type> {
        let call = node.as_call_node()?;
        if call.arguments().is_some() || call.block().is_some() {
            return None;
        }
        let fact = scope
            .exclusive_receiver_facts
            .get(&local_name(&call.receiver()?)?)?;
        fact.attributes
            .get(String::from_utf8_lossy(call.name().as_slice()).as_ref())
            .cloned()
    }
}

//! What code may do to the flow facts on `self` (`Scope::self_facts`).
//!
//! A fact about an ivar or a no-argument self call holds until something may write an ivar.
//! The writes come from the code itself and from the self calls it makes. Each source method's
//! body is summarized at collection, and a self call follows these summaries transitively. A
//! call on another receiver does not reach self's ivars, except through the block it takes. A
//! callee without a summary (RBS, DSL, another file) or a body using `super` or a block argument
//! may write anything.

use std::cell::RefCell;
use std::sync::Arc;

use ruby_prism::{Node, Visit};
use rustc_hash::{FxHashMap, FxHashSet};

use super::InferenceEngine;
use super::param_flow::MAX_SCAN_DEPTH;
use crate::types::{Sym, Type};

/// The writes a method body or block body makes, read off its syntax.
#[derive(Debug, Clone, Default)]
pub(super) struct SelfWrites {
    ivars: Vec<Sym>,
    /// Calls on the same self (`foo`, `self.foo = x`), followed through their summaries.
    self_calls: Vec<Sym>,
    /// Code whose writes the syntax does not show, such as `super` or `&blk`.
    opaque: bool,
}

impl SelfWrites {
    pub(super) fn of(node: &Node<'_>) -> Self {
        let mut collector = SelfWritesCollector::default();
        collector.visit(node);
        collector.writes
    }

    fn write_ivar(&mut self, name: &[u8]) {
        let name = Sym::new(String::from_utf8_lossy(name));
        if !self.ivars.contains(&name) {
            self.ivars.push(name);
        }
    }

    fn call_self(&mut self, name: &[u8]) {
        let name = Sym::new(String::from_utf8_lossy(name));
        if !self.self_calls.contains(&name) {
            self.self_calls.push(name);
        }
    }
}

#[derive(Default)]
struct SelfWritesCollector {
    writes: SelfWrites,
    depth: usize,
}

/// Stops the walk below nodes that nest arbitrarily once the code is too deep to read, which
/// leaves it opaque.
macro_rules! guarded {
    ($($method:ident, $node:ident;)*) => {
        $(
            fn $method(&mut self, node: &ruby_prism::$node<'pr>) {
                if !self.writes.opaque {
                    ruby_prism::$method(self, node);
                }
            }
        )*
    };
}

fn is_self_receiver(receiver: Option<Node<'_>>) -> bool {
    receiver.is_none_or(|receiver| matches!(receiver, Node::SelfNode { .. }))
}

impl<'pr> Visit<'pr> for SelfWritesCollector {
    fn visit_branch_node_enter(&mut self, _node: Node<'pr>) {
        self.depth += 1;
        self.writes.opaque |= self.depth > MAX_SCAN_DEPTH;
    }

    fn visit_branch_node_leave(&mut self) {
        self.depth -= 1;
    }

    guarded! {
        visit_statements_node, StatementsNode;
        visit_arguments_node, ArgumentsNode;
        visit_parentheses_node, ParenthesesNode;
        visit_array_node, ArrayNode;
        visit_hash_node, HashNode;
        visit_keyword_hash_node, KeywordHashNode;
        visit_assoc_node, AssocNode;
        visit_else_node, ElseNode;
        visit_block_node, BlockNode;
        visit_begin_node, BeginNode;
        visit_interpolated_string_node, InterpolatedStringNode;
        visit_embedded_statements_node, EmbeddedStatementsNode;
        visit_local_variable_write_node, LocalVariableWriteNode;
    }

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        if is_self_receiver(node.receiver()) {
            self.writes.call_self(node.name().as_slice());
        }
        if let Some(block) = node.block()
            && let Some(argument) = block.as_block_argument_node()
            && !argument
                .expression()
                .is_some_and(|expression| expression.as_symbol_node().is_some())
        {
            self.writes.opaque = true;
        }
        ruby_prism::visit_call_node(self, node);
    }

    fn visit_call_operator_write_node(&mut self, node: &ruby_prism::CallOperatorWriteNode<'pr>) {
        if is_self_receiver(node.receiver()) {
            self.writes.call_self(node.write_name().as_slice());
        }
        ruby_prism::visit_call_operator_write_node(self, node);
    }

    fn visit_call_or_write_node(&mut self, node: &ruby_prism::CallOrWriteNode<'pr>) {
        if is_self_receiver(node.receiver()) {
            self.writes.call_self(node.write_name().as_slice());
        }
        ruby_prism::visit_call_or_write_node(self, node);
    }

    fn visit_call_and_write_node(&mut self, node: &ruby_prism::CallAndWriteNode<'pr>) {
        if is_self_receiver(node.receiver()) {
            self.writes.call_self(node.write_name().as_slice());
        }
        ruby_prism::visit_call_and_write_node(self, node);
    }

    fn visit_call_target_node(&mut self, node: &ruby_prism::CallTargetNode<'pr>) {
        if is_self_receiver(Some(node.receiver())) {
            self.writes.call_self(node.name().as_slice());
        }
        ruby_prism::visit_call_target_node(self, node);
    }

    fn visit_index_operator_write_node(&mut self, node: &ruby_prism::IndexOperatorWriteNode<'pr>) {
        if is_self_receiver(node.receiver()) {
            self.writes.call_self(b"[]=");
        }
        ruby_prism::visit_index_operator_write_node(self, node);
    }

    fn visit_index_or_write_node(&mut self, node: &ruby_prism::IndexOrWriteNode<'pr>) {
        if is_self_receiver(node.receiver()) {
            self.writes.call_self(b"[]=");
        }
        ruby_prism::visit_index_or_write_node(self, node);
    }

    fn visit_index_and_write_node(&mut self, node: &ruby_prism::IndexAndWriteNode<'pr>) {
        if is_self_receiver(node.receiver()) {
            self.writes.call_self(b"[]=");
        }
        ruby_prism::visit_index_and_write_node(self, node);
    }

    fn visit_index_target_node(&mut self, node: &ruby_prism::IndexTargetNode<'pr>) {
        if is_self_receiver(Some(node.receiver())) {
            self.writes.call_self(b"[]=");
        }
        ruby_prism::visit_index_target_node(self, node);
    }

    fn visit_super_node(&mut self, node: &ruby_prism::SuperNode<'pr>) {
        self.writes.opaque = true;
        ruby_prism::visit_super_node(self, node);
    }

    fn visit_forwarding_super_node(&mut self, node: &ruby_prism::ForwardingSuperNode<'pr>) {
        self.writes.opaque = true;
        ruby_prism::visit_forwarding_super_node(self, node);
    }

    fn visit_instance_variable_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableWriteNode<'pr>,
    ) {
        self.writes.write_ivar(node.name().as_slice());
        ruby_prism::visit_instance_variable_write_node(self, node);
    }

    fn visit_instance_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOperatorWriteNode<'pr>,
    ) {
        self.writes.write_ivar(node.name().as_slice());
        ruby_prism::visit_instance_variable_operator_write_node(self, node);
    }

    fn visit_instance_variable_or_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOrWriteNode<'pr>,
    ) {
        self.writes.write_ivar(node.name().as_slice());
        ruby_prism::visit_instance_variable_or_write_node(self, node);
    }

    fn visit_instance_variable_and_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableAndWriteNode<'pr>,
    ) {
        self.writes.write_ivar(node.name().as_slice());
        ruby_prism::visit_instance_variable_and_write_node(self, node);
    }

    fn visit_instance_variable_target_node(
        &mut self,
        node: &ruby_prism::InstanceVariableTargetNode<'pr>,
    ) {
        self.writes.write_ivar(node.name().as_slice());
    }

    // These bodies run later or with another self.
    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}

    fn visit_class_node(&mut self, _node: &ruby_prism::ClassNode<'pr>) {}

    fn visit_module_node(&mut self, _node: &ruby_prism::ModuleNode<'pr>) {}

    fn visit_singleton_class_node(&mut self, _node: &ruby_prism::SingletonClassNode<'pr>) {}
}

/// The ivars some code may write, or `any` when it may write any of them.
#[derive(Debug, Clone, Default)]
pub(super) struct SelfFactEffect {
    any: bool,
    ivars: Vec<Sym>,
}

impl SelfFactEffect {
    pub(super) fn any() -> Self {
        Self {
            any: true,
            ivars: Vec::new(),
        }
    }

    pub(super) fn is_any(&self) -> bool {
        self.any
    }

    pub(super) fn write_ivar(&mut self, name: &str) {
        if !self.any && !self.ivars.iter().any(|ivar| *ivar == name) {
            self.ivars.push(Sym::new(name));
        }
    }

    pub(super) fn merge(&mut self, other: &Self) {
        if other.any {
            *self = Self::any();
            return;
        }
        for ivar in &other.ivars {
            self.write_ivar(ivar);
        }
    }

    /// A write to any ivar may also change what a self call returns.
    pub(super) fn invalidates(&self, facts: &std::collections::HashMap<String, Type>) -> bool {
        if facts.is_empty() || (!self.any && self.ivars.is_empty()) {
            return false;
        }
        self.any
            || facts
                .keys()
                .any(|key| !key.starts_with('@') || self.ivars.iter().any(|ivar| *ivar == **key))
    }

    pub(super) fn ivars(&self) -> &[Sym] {
        &self.ivars
    }
}

type SelfCallKey = (Sym, Sym, bool);

/// Per-engine summaries of this file's source methods and the transitive effects of self calls.
#[derive(Default)]
pub(super) struct SelfWriteSummaries {
    by_method: FxHashMap<SelfCallKey, SelfWrites>,
    /// `None` when the call may write any ivar.
    call_effects: RefCell<FxHashMap<SelfCallKey, Option<Arc<[Sym]>>>>,
}

impl SelfWriteSummaries {
    pub(super) fn record(
        &mut self,
        owner: &str,
        method: &str,
        is_singleton: bool,
        writes: SelfWrites,
    ) {
        self.by_method
            .insert((Sym::new(owner), Sym::new(method), is_singleton), writes);
        self.call_effects.get_mut().clear();
    }
}

impl InferenceEngine<'_> {
    pub(super) fn record_self_writes(
        &mut self,
        owner: &str,
        method: &str,
        is_singleton: bool,
        def_node: &ruby_prism::DefNode<'_>,
    ) {
        let writes = def_node
            .body()
            .map(|body| SelfWrites::of(&body))
            .unwrap_or_default();
        self.self_write_summaries
            .record(owner, method, is_singleton, writes);
    }

    /// Summarizes the defs directly in a class body before any of their bodies is inferred, so
    /// a body sees what a method defined after it writes.
    pub(super) fn summarize_class_body_defs(
        &mut self,
        class_name: &str,
        body: &Node<'_>,
        in_singleton_class: bool,
    ) {
        let Some(statements) = body.as_statements_node() else {
            return;
        };
        for node in statements.body().iter() {
            self.summarize_class_body_def(class_name, &node, in_singleton_class);
        }
    }

    fn summarize_class_body_def(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        in_singleton_class: bool,
    ) {
        match node {
            Node::DefNode { .. } => {
                let def_node = node.as_def_node().expect("must be DefNode");
                let is_singleton = match def_node.receiver() {
                    None => in_singleton_class,
                    Some(Node::SelfNode { .. }) => true,
                    Some(_) => return,
                };
                let method = String::from_utf8_lossy(def_node.name().as_slice());
                self.record_self_writes(class_name, &method, is_singleton, &def_node);
            }
            // `private def x`, `memoize def x`
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("must be CallNode");
                if call.receiver().is_none()
                    && let Some(arguments) = call.arguments()
                {
                    for argument in arguments.arguments().iter() {
                        self.summarize_class_body_def(class_name, &argument, in_singleton_class);
                    }
                }
            }
            Node::SingletonClassNode { .. } if !in_singleton_class => {
                let singleton_class = node
                    .as_singleton_class_node()
                    .expect("must be SingletonClassNode");
                if matches!(singleton_class.expression(), Node::SelfNode { .. })
                    && let Some(body) = singleton_class.body()
                {
                    self.summarize_class_body_defs(class_name, &body, true);
                }
            }
            _ => {}
        }
    }

    /// The effect of code with these writes, run on an instance of `class_name` (or on the
    /// class itself for `singleton_dispatch`).
    pub(super) fn self_writes_effect(
        &self,
        class_name: &str,
        singleton_dispatch: bool,
        writes: &SelfWrites,
    ) -> SelfFactEffect {
        if writes.opaque {
            return SelfFactEffect::any();
        }
        let mut effect = SelfFactEffect::default();
        for ivar in &writes.ivars {
            effect.write_ivar(ivar);
        }
        for callee in &writes.self_calls {
            effect.merge(&self.self_call_effect(class_name, singleton_dispatch, callee));
            if effect.is_any() {
                break;
            }
        }
        effect
    }

    pub(super) fn self_call_effect(
        &self,
        class_name: &str,
        singleton_dispatch: bool,
        method: &str,
    ) -> SelfFactEffect {
        let key = (Sym::new(class_name), Sym::new(method), singleton_dispatch);
        let cached = self
            .self_write_summaries
            .call_effects
            .borrow()
            .get(&key)
            .cloned();
        let ivars = cached.unwrap_or_else(|| {
            let ivars = self.collect_self_call_ivars(class_name, singleton_dispatch, method);
            self.self_write_summaries
                .call_effects
                .borrow_mut()
                .insert(key, ivars.clone());
            ivars
        });
        match ivars {
            Some(ivars) => SelfFactEffect {
                any: false,
                ivars: ivars.to_vec(),
            },
            None => SelfFactEffect::any(),
        }
    }

    /// Follows the self calls a call makes, dispatching each on the same self.
    fn collect_self_call_ivars(
        &self,
        class_name: &str,
        singleton_dispatch: bool,
        method: &str,
    ) -> Option<Arc<[Sym]>> {
        let mut ivars: Vec<Sym> = Vec::new();
        let mut visited: FxHashSet<Sym> = FxHashSet::default();
        let mut pending = vec![Sym::new(method)];
        while let Some(name) = pending.pop() {
            if !visited.insert(name) {
                continue;
            }
            let owner =
                self.registry
                    .first_method_call_owner_cached(class_name, &name, singleton_dispatch);
            let own_key = (Sym::new(class_name), name, singleton_dispatch);
            let has_own = self.self_write_summaries.by_method.contains_key(&own_key);
            // The class's own def is not registered yet while the body before it is inferred.
            let summary_key = match &owner {
                Some((owner, owner_singleton)) if !has_own || **owner == *class_name => {
                    (Sym::new(owner), name, *owner_singleton)
                }
                // A prepended module could still define it ahead of the class's own def.
                Some(_) => return None,
                None => own_key,
            };
            if let Some(writes) = self.self_write_summaries.by_method.get(&summary_key) {
                if writes.opaque {
                    return None;
                }
                for ivar in &writes.ivars {
                    if !ivars.contains(ivar) {
                        ivars.push(*ivar);
                    }
                }
                pending.extend(writes.self_calls.iter().copied());
                continue;
            }
            let attr_ivar = owner
                .as_ref()
                .and_then(|(owner, owner_singleton)| {
                    self.registry
                        .lookup_method_def(owner, &name, *owner_singleton)
                })
                .and_then(|method| method.attr_ivar.as_deref());
            match attr_ivar {
                Some(ivar) if name.ends_with('=') => {
                    let ivar = Sym::new(ivar);
                    if !ivars.contains(&ivar) {
                        ivars.push(ivar);
                    }
                }
                Some(_) => {}
                // Readers of a backing ivar, such as Active Record columns.
                None if !singleton_dispatch
                    && self
                        .registry
                        .is_pure_ivar_reader_method(class_name, &name, false) => {}
                None => return None,
            }
        }
        Some(ivars.into())
    }
}

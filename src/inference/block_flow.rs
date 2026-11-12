//! Flow summary of how a source method treats the block it receives.
//!
//! The walk assumes a block was given, follows which locals may or must still hold it, and records
//! whether normal completion implies an invocation. Any other use of the block counts as a possible
//! invocation. A method that branches on a positional parameter keeps its source, so a call passing
//! a literal there can repeat the walk with that value known.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ruby_prism::{Node, Visit};

use crate::registry::{BlockFlowSummary, BlockForward, LiteralBranchedDef};
use crate::types::SharedName;

/// An argument's value as far as branching on it can tell.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum LiteralArg {
    Nil,
    True,
    False,
    Symbol(Vec<u8>),
    Integer(bool, Vec<u32>),
    String(Vec<u8>),
    /// Truthy, but `===` against it is not decided here.
    OtherTruthy,
}

impl LiteralArg {
    pub(super) fn from_node(node: &Node<'_>) -> Option<Self> {
        match node {
            Node::NilNode { .. } => Some(Self::Nil),
            Node::TrueNode { .. } => Some(Self::True),
            Node::FalseNode { .. } => Some(Self::False),
            Node::SymbolNode { .. } => {
                Some(Self::Symbol(node.as_symbol_node()?.unescaped().to_vec()))
            }
            Node::IntegerNode { .. } => {
                let integer = node.as_integer_node()?.value();
                let (negative, digits) = integer.to_u32_digits();
                Some(Self::Integer(negative, digits.to_vec()))
            }
            Node::StringNode { .. } => {
                Some(Self::String(node.as_string_node()?.unescaped().to_vec()))
            }
            Node::FloatNode { .. } | Node::RationalNode { .. } | Node::ImaginaryNode { .. } => {
                Some(Self::OtherTruthy)
            }
            _ => None,
        }
    }

    fn is_truthy(&self) -> bool {
        !matches!(self, Self::Nil | Self::False)
    }

    /// `pattern === self`, or `None` when it depends on more than two exact literals.
    fn case_matches(&self, pattern: &Node<'_>) -> Option<bool> {
        let pattern = Self::from_node(pattern)?;
        if matches!(self, Self::OtherTruthy) || matches!(pattern, Self::OtherTruthy) {
            return None;
        }
        Some(*self == pattern)
    }
}

#[derive(Clone, Default)]
struct FlowState {
    must_hold: HashSet<Vec<u8>>,
    may_hold: HashSet<Vec<u8>>,
    invoked: bool,
    forwarded: bool,
    dead: bool,
}

impl FlowState {
    fn dead() -> Self {
        Self {
            dead: true,
            ..Self::default()
        }
    }

    fn join(self, other: Self) -> Self {
        if self.dead {
            return other;
        }
        if other.dead {
            return self;
        }
        Self {
            must_hold: self
                .must_hold
                .intersection(&other.must_hold)
                .cloned()
                .collect(),
            may_hold: self.may_hold.union(&other.may_hold).cloned().collect(),
            invoked: self.invoked && other.invoked,
            forwarded: self.forwarded && other.forwarded,
            dead: false,
        }
    }
}

#[derive(Default)]
struct BlockFlowWalker {
    positional_params: Vec<Option<Vec<u8>>>,
    branched_params: HashSet<usize>,
    literal_args: HashMap<Vec<u8>, LiteralArg>,
    select_case_arms: bool,
    closure_depth: u32,
    /// Per enclosing `begin`, the locals that held the block at every point of its body so far.
    rescue_must_hold: Vec<HashSet<Vec<u8>>>,
    state: FlowState,
    ever_held: HashSet<Vec<u8>>,
    may_invoke: bool,
    returned_uninvoked: bool,
    returned_unforwarded: bool,
    forward_target: Option<SharedName>,
    loop_exits: Vec<FlowState>,
    depth: usize,
    too_deep: bool,
}

/// Guards pathological nesting; past it, the block counts as possibly invoked.
const MAX_FLOW_DEPTH: usize = 256;

pub(super) fn summarize_block_flow(
    def_node: &ruby_prism::DefNode<'_>,
    block_param_name: Option<&str>,
) -> Option<BlockFlowSummary> {
    let body = def_node.body()?;
    let mut walker = BlockFlowWalker::for_block_param(block_param_name);
    walker.positional_params = positional_param_names(def_node);
    walker.visit(&body);
    let branched_params: Vec<usize> = walker.branched_params.iter().copied().collect();
    let mut summary = walker.finish();
    if !branched_params.is_empty() && (summary.may_invoke || summary.forward.is_some()) {
        let source = String::from_utf8_lossy(def_node.location().as_slice());
        summary.literal_branched_def = Some(Arc::new(LiteralBranchedDef {
            source: source.as_ref().into(),
            branched_params: {
                let mut sorted = branched_params;
                sorted.sort_unstable();
                sorted
            },
        }));
    }
    Some(summary)
}

/// Repeats the walk of a stored def with the literal passed to each positional parameter known.
pub(super) fn summarize_block_flow_for_literals(
    def: &LiteralBranchedDef,
    literal_args: &[Option<LiteralArg>],
    select_case_arms: bool,
) -> Option<BlockFlowSummary> {
    let parse_result = ruby_prism::parse(def.source.as_bytes());
    let root = parse_result.node();
    let program = root.as_program_node()?;
    let statements = program.statements().body();
    let first = statements.iter().next()?;
    let def_node = first.as_def_node()?;
    let body = def_node.body()?;

    let block_param_name = def_node
        .parameters()
        .and_then(|params| params.block())
        .and_then(|block| block.as_block_parameter_node()?.name())
        .map(|name| String::from_utf8_lossy(name.as_slice()).to_string());
    let mut walker = BlockFlowWalker::for_block_param(block_param_name.as_deref());
    walker.select_case_arms = select_case_arms;
    for (name, literal) in positional_param_names(&def_node)
        .into_iter()
        .zip(literal_args)
    {
        if let (Some(name), Some(literal)) = (name, literal) {
            walker.literal_args.insert(name, literal.clone());
        }
    }
    walker.visit(&body);
    Some(walker.finish())
}

/// Names by the index a positional argument binds to. Trailing required parameters shift that
/// index, so optional parameters are left out when any exist.
fn positional_param_names(def_node: &ruby_prism::DefNode<'_>) -> Vec<Option<Vec<u8>>> {
    let Some(params) = def_node.parameters() else {
        return Vec::new();
    };
    let requireds = params.requireds().iter().map(|param| {
        param
            .as_required_parameter_node()
            .map(|param| param.name().as_slice().to_vec())
    });
    let mut names: Vec<_> = requireds.collect();
    if params.posts().iter().next().is_none() {
        names.extend(params.optionals().iter().map(|param| {
            param
                .as_optional_parameter_node()
                .map(|param| param.name().as_slice().to_vec())
        }));
    }
    names
}

/// Outer or block-local names written anywhere in the block, including nested closures.
pub(super) fn block_assigned_local_names(block: &ruby_prism::BlockNode<'_>) -> HashSet<String> {
    let mut writes = LocalWrites::default();
    writes.visit_block_node(block);
    writes.0
}

/// Local names written anywhere under `node`, including nested closures.
pub(super) fn assigned_local_names(node: &Node<'_>) -> HashSet<String> {
    let mut writes = LocalWrites::default();
    writes.visit(node);
    writes.0
}

#[derive(Default)]
struct LocalWrites(HashSet<String>);

impl LocalWrites {
    fn add(&mut self, name: ruby_prism::ConstantId<'_>) {
        self.0
            .insert(String::from_utf8_lossy(name.as_slice()).to_string());
    }
}

impl<'pr> Visit<'pr> for LocalWrites {
    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
    fn visit_class_node(&mut self, _node: &ruby_prism::ClassNode<'pr>) {}
    fn visit_module_node(&mut self, _node: &ruby_prism::ModuleNode<'pr>) {}
    fn visit_singleton_class_node(&mut self, _node: &ruby_prism::SingletonClassNode<'pr>) {}

    fn visit_local_variable_write_node(&mut self, node: &ruby_prism::LocalVariableWriteNode<'pr>) {
        self.add(node.name());
        ruby_prism::visit_local_variable_write_node(self, node);
    }

    fn visit_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'pr>,
    ) {
        self.add(node.name());
        ruby_prism::visit_local_variable_or_write_node(self, node);
    }

    fn visit_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'pr>,
    ) {
        self.add(node.name());
        ruby_prism::visit_local_variable_and_write_node(self, node);
    }

    fn visit_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'pr>,
    ) {
        self.add(node.name());
        ruby_prism::visit_local_variable_operator_write_node(self, node);
    }

    fn visit_local_variable_target_node(
        &mut self,
        node: &ruby_prism::LocalVariableTargetNode<'pr>,
    ) {
        self.add(node.name());
    }
}

fn strip_parentheses<'pr>(node: Node<'pr>) -> Node<'pr> {
    let Some(body) = node.as_parentheses_node().and_then(|paren| paren.body()) else {
        return node;
    };
    if let Some(statements) = body.as_statements_node() {
        let mut body = statements.body().iter();
        return match (body.next(), body.next()) {
            (Some(only), None) => strip_parentheses(only),
            _ => node,
        };
    }
    strip_parentheses(body)
}

/// Whether the arm runs for `subject`; `None` when a pattern is not an exact literal.
fn when_matches(when: &ruby_prism::WhenNode<'_>, subject: &LiteralArg) -> Option<bool> {
    let mut undecided = false;
    for pattern in when.conditions().iter() {
        match subject.case_matches(&pattern) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => undecided = true,
        }
    }
    (!undecided).then_some(false)
}

/// A retry re-enters the body with whatever the rescue clause wrote.
fn rescue_may_retry(clause: &ruby_prism::RescueNode<'_>) -> bool {
    #[derive(Default)]
    struct Retries(bool);

    impl<'pr> Visit<'pr> for Retries {
        fn visit_retry_node(&mut self, _node: &ruby_prism::RetryNode<'pr>) {
            self.0 = true;
        }
    }

    let mut retries = Retries::default();
    retries.visit_rescue_node(clause);
    retries.0
}

fn read_name(node: &Node<'_>) -> Option<Vec<u8>> {
    node.as_local_variable_read_node()
        .map(|read| read.name().as_slice().to_vec())
}

impl BlockFlowWalker {
    fn for_block_param(block_param_name: Option<&str>) -> Self {
        let mut walker = Self::default();
        if let Some(name) = block_param_name {
            let name = name.as_bytes().to_vec();
            walker.state.must_hold.insert(name.clone());
            walker.state.may_hold.insert(name.clone());
            walker.ever_held.insert(name);
        }
        walker
    }

    fn finish(mut self) -> BlockFlowSummary {
        if self.too_deep {
            return BlockFlowSummary {
                may_invoke: true,
                ..BlockFlowSummary::default()
            };
        }
        let state = &self.state;
        let forward = self.forward_target.take().map(|name| BlockForward {
            name,
            must: (state.dead || state.forwarded) && !self.returned_unforwarded,
        });
        BlockFlowSummary {
            may_invoke: self.may_invoke,
            must_invoke: self.may_invoke
                && (state.dead || state.invoked)
                && !self.returned_uninvoked,
            forward,
            literal_branched_def: None,
        }
    }

    fn exceeded_depth(&mut self) -> bool {
        self.too_deep |= self.depth > MAX_FLOW_DEPTH;
        self.too_deep
    }

    /// The literal a read of a positional parameter yields; records the branch on that parameter.
    fn branch_on_param(
        &mut self,
        read: &ruby_prism::LocalVariableReadNode<'_>,
    ) -> Option<&LiteralArg> {
        if read.depth() != self.closure_depth {
            return None;
        }
        let name = read.name();
        let name = name.as_slice();
        if let Some(index) = self
            .positional_params
            .iter()
            .position(|param| param.as_deref() == Some(name))
        {
            self.branched_params.insert(index);
        }
        self.literal_args.get(name)
    }

    /// A condition answered by the block being present or a literal argument.
    fn known_truthiness(&mut self, node: &Node<'_>) -> Option<bool> {
        if let Some(call) = node.as_call_node() {
            let name = call.name();
            if name.as_slice() == b"!" {
                let operand = strip_parentheses(call.receiver()?);
                return self.known_truthiness(&operand).map(|truthy| !truthy);
            }
            return (call.receiver().is_none()
                && name.as_slice() == b"block_given?"
                && call.arguments().is_none())
            .then_some(true);
        }
        let read = node.as_local_variable_read_node()?;
        if let Some(literal) = self.branch_on_param(&read) {
            return Some(literal.is_truthy());
        }
        self.state
            .must_hold
            .contains(read.name().as_slice())
            .then_some(true)
    }

    /// Whether a `while` or `until` loop's first predicate check enters the body. A `begin...end
    /// while` runs its body first, so its predicate says nothing about that.
    fn loop_entry(&mut self, predicate: Node<'_>, begin_modifier: bool) -> Option<bool> {
        let known = self.known_truthiness(&strip_parentheses(predicate));
        if begin_modifier { None } else { known }
    }

    fn visit_branches(
        &mut self,
        known: Option<bool>,
        then_branch: impl FnOnce(&mut Self),
        else_branch: impl FnOnce(&mut Self),
    ) {
        match known {
            Some(true) => then_branch(self),
            Some(false) => else_branch(self),
            None => {
                let entry = self.state.clone();
                then_branch(self);
                let after_then = std::mem::replace(&mut self.state, entry);
                else_branch(self);
                let after_else = std::mem::take(&mut self.state);
                self.state = after_then.join(after_else);
            }
        }
    }

    fn visit_optional(&mut self, operand: &Node<'_>) {
        let entry = self.state.clone();
        self.visit(operand);
        let after = std::mem::replace(&mut self.state, entry);
        self.state = std::mem::take(&mut self.state).join(after);
    }

    fn visit_alternatives<'pr>(
        &mut self,
        alternatives: impl Iterator<Item = Node<'pr>>,
        no_match_continues: bool,
    ) {
        let entry = self.state.clone();
        let mut joined = if no_match_continues {
            entry.clone()
        } else {
            FlowState::dead()
        };
        for alternative in alternatives {
            self.state = entry.clone();
            self.visit(&alternative);
            joined = joined.join(std::mem::take(&mut self.state));
        }
        self.state = joined;
    }

    fn visit_loop<'pr>(
        &mut self,
        predicate: Option<Node<'pr>>,
        statements: Option<ruby_prism::StatementsNode<'pr>>,
        enters: Option<bool>,
    ) {
        if enters == Some(false) {
            return;
        }
        let outer_exits = std::mem::take(&mut self.loop_exits);
        if let Some(predicate) = &predicate {
            self.visit(predicate);
        }
        let mut after = if enters == Some(true) {
            FlowState::dead()
        } else {
            self.state.clone()
        };
        // A second pass lets an alias written late in the body reach its earlier reads.
        for _ in 0..2 {
            if let Some(statements) = &statements {
                self.visit_statements_node(statements);
            }
            if let Some(predicate) = &predicate {
                self.visit(predicate);
            }
            after = after.join(self.state.clone());
            self.state = after.clone();
        }
        for exit in std::mem::replace(&mut self.loop_exits, outer_exits) {
            after = after.join(exit);
        }
        self.state = after;
    }

    /// A closure may run any number of times, so nothing it does is guaranteed.
    fn visit_closure(&mut self, parameters: Option<Node<'_>>, body: Option<Node<'_>>) {
        let entry = self.state.clone();
        let outer_exits = std::mem::take(&mut self.loop_exits);
        self.closure_depth += 1;
        if let Some(parameters) = &parameters {
            self.visit(parameters);
        }
        if let Some(body) = &body {
            self.visit(body);
        }
        self.closure_depth -= 1;
        self.loop_exits = outer_exits;
        let after = std::mem::replace(&mut self.state, entry);
        self.state = std::mem::take(&mut self.state).join(after);
    }

    fn mark_possible_invoke(&mut self) {
        if !self.state.dead {
            self.may_invoke = true;
        }
    }

    fn visit_arguments_of(&mut self, call: &ruby_prism::CallNode<'_>) {
        if let Some(arguments) = call.arguments() {
            self.visit_arguments_node(&arguments);
        }
    }

    /// Returns whether `pass` hands over the received block.
    fn visit_block_pass(&mut self, call: &ruby_prism::CallNode<'_>, pass: &Node<'_>) -> bool {
        let Some(argument) = pass.as_block_argument_node() else {
            return false;
        };
        let held_name = argument.expression().and_then(|node| read_name(&node));
        if argument.expression().is_some()
            && !held_name
                .as_ref()
                .is_some_and(|name| self.state.may_hold.contains(name))
        {
            return false;
        }
        if self.state.dead {
            return true;
        }
        let definite = held_name
            .as_ref()
            .is_none_or(|name| self.state.must_hold.contains(name));
        let on_self = call
            .receiver()
            .is_none_or(|receiver| matches!(receiver, Node::SelfNode { .. }));
        let target = String::from_utf8_lossy(call.name().as_slice());
        let same_target = self
            .forward_target
            .as_ref()
            .is_none_or(|existing| existing.as_ref() == target);
        if on_self && definite && same_target {
            self.forward_target = Some(target.as_ref().into());
            self.state.forwarded = true;
        } else {
            self.may_invoke = true;
        }
        true
    }

    fn rescue_entry(
        &self,
        entry: &FlowState,
        body_end: &FlowState,
        must_hold: HashSet<Vec<u8>>,
    ) -> FlowState {
        FlowState {
            must_hold,
            may_hold: self.ever_held.clone(),
            invoked: entry.invoked,
            forwarded: entry.forwarded,
            dead: entry.dead && body_end.dead,
        }
    }

    fn forget_local(&mut self, name: &[u8]) {
        self.literal_args.remove(name);
        self.state.must_hold.remove(name);
        if !self.state.dead {
            for must_hold in &mut self.rescue_must_hold {
                must_hold.remove(name);
            }
        }
    }

    fn write_local(&mut self, name: &[u8], value: Option<&[u8]>) {
        self.forget_local(name);
        self.state.may_hold.remove(name);
        let Some(source) = value else {
            return;
        };
        if self.state.must_hold.contains(source) {
            self.state.must_hold.insert(name.to_vec());
        }
        if self.state.may_hold.contains(source) {
            self.state.may_hold.insert(name.to_vec());
            self.ever_held.insert(name.to_vec());
        }
    }
}

impl<'pr> Visit<'pr> for BlockFlowWalker {
    fn visit_branch_node_enter(&mut self, _node: Node<'pr>) {
        self.depth += 1;
    }

    fn visit_branch_node_leave(&mut self) {
        self.depth -= 1;
    }

    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
    fn visit_class_node(&mut self, _node: &ruby_prism::ClassNode<'pr>) {}
    fn visit_module_node(&mut self, _node: &ruby_prism::ModuleNode<'pr>) {}
    fn visit_singleton_class_node(&mut self, _node: &ruby_prism::SingletonClassNode<'pr>) {}

    fn visit_yield_node(&mut self, node: &ruby_prism::YieldNode<'pr>) {
        if let Some(arguments) = node.arguments() {
            self.visit_arguments_node(&arguments);
        }
        self.mark_possible_invoke();
        self.state.invoked = true;
    }

    fn visit_local_variable_read_node(&mut self, node: &ruby_prism::LocalVariableReadNode<'pr>) {
        if self.state.may_hold.contains(node.name().as_slice()) {
            self.mark_possible_invoke();
        }
    }

    fn visit_local_variable_write_node(&mut self, node: &ruby_prism::LocalVariableWriteNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let value = strip_parentheses(node.value());
        let source = read_name(&value);
        if source.is_none() {
            self.visit(&value);
        }
        self.write_local(node.name().as_slice(), source.as_deref());
    }

    fn visit_local_variable_target_node(
        &mut self,
        node: &ruby_prism::LocalVariableTargetNode<'pr>,
    ) {
        self.write_local(node.name().as_slice(), None);
    }

    fn visit_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'pr>,
    ) {
        self.visit(&node.value());
        self.forget_local(node.name().as_slice());
    }

    fn visit_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'pr>,
    ) {
        self.visit(&node.value());
        self.forget_local(node.name().as_slice());
    }

    fn visit_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'pr>,
    ) {
        self.visit(&node.value());
        self.forget_local(node.name().as_slice());
    }

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let name = node.name();
        let name = name.as_slice();
        let receiver = node.receiver();
        if let Some(held) = receiver.as_ref().and_then(read_name)
            && self.state.may_hold.contains(&held)
            && matches!(name, b"call" | b"yield" | b"===" | b"[]")
        {
            self.visit_arguments_of(node);
            self.mark_possible_invoke();
            self.state.invoked |= self.state.must_hold.contains(&held);
            return;
        }
        if let Some(receiver) = &receiver {
            self.visit(receiver);
        }
        self.visit_arguments_of(node);
        if receiver.is_none() && matches!(name, b"raise" | b"fail" | b"exit" | b"abort" | b"throw")
        {
            self.state = FlowState::dead();
            return;
        }
        if let Some(block) = node.block()
            && !self.visit_block_pass(node, &block)
        {
            self.visit(&block);
        }
    }

    fn visit_super_node(&mut self, node: &ruby_prism::SuperNode<'pr>) {
        self.mark_possible_invoke();
        ruby_prism::visit_super_node(self, node);
    }

    fn visit_forwarding_super_node(&mut self, node: &ruby_prism::ForwardingSuperNode<'pr>) {
        self.mark_possible_invoke();
        ruby_prism::visit_forwarding_super_node(self, node);
    }

    fn visit_forwarding_arguments_node(
        &mut self,
        _node: &ruby_prism::ForwardingArgumentsNode<'pr>,
    ) {
        self.mark_possible_invoke();
    }

    fn visit_block_node(&mut self, node: &ruby_prism::BlockNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        self.visit_closure(node.parameters(), node.body());
    }

    fn visit_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        self.visit_closure(node.parameters(), node.body());
    }

    fn visit_if_node(&mut self, node: &ruby_prism::IfNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let known = self.known_truthiness(&strip_parentheses(node.predicate()));
        if known.is_none() {
            self.visit(&node.predicate());
        }
        self.visit_branches(
            known,
            |walker| {
                if let Some(statements) = node.statements() {
                    walker.visit_statements_node(&statements);
                }
            },
            |walker| {
                if let Some(subsequent) = node.subsequent() {
                    walker.visit(&subsequent);
                }
            },
        );
    }

    fn visit_unless_node(&mut self, node: &ruby_prism::UnlessNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let known = self.known_truthiness(&strip_parentheses(node.predicate()));
        if known.is_none() {
            self.visit(&node.predicate());
        }
        self.visit_branches(
            known.map(|truthy| !truthy),
            |walker| {
                if let Some(statements) = node.statements() {
                    walker.visit_statements_node(&statements);
                }
            },
            |walker| {
                if let Some(else_clause) = node.else_clause() {
                    walker.visit_else_node(&else_clause);
                }
            },
        );
    }

    fn visit_and_node(&mut self, node: &ruby_prism::AndNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let left = strip_parentheses(node.left());
        match self.known_truthiness(&left) {
            Some(true) => self.visit(&node.right()),
            Some(false) => {}
            None => {
                self.visit(&left);
                self.visit_optional(&node.right());
            }
        }
    }

    fn visit_or_node(&mut self, node: &ruby_prism::OrNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let left = strip_parentheses(node.left());
        match self.known_truthiness(&left) {
            Some(false) => self.visit(&node.right()),
            Some(true) => {}
            None => {
                self.visit(&left);
                self.visit_optional(&node.right());
            }
        }
    }

    fn visit_case_node(&mut self, node: &ruby_prism::CaseNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let subject_literal = node.predicate().and_then(|predicate| {
            let predicate = strip_parentheses(predicate);
            let read = predicate.as_local_variable_read_node()?;
            self.branch_on_param(&read).cloned()
        });
        if let Some(predicate) = node.predicate() {
            self.visit(&predicate);
        }
        let else_clause = node.else_clause();
        let mut no_match_continues = else_clause.is_none();
        let mut arms: Vec<Node<'pr>> = Vec::new();
        let mut matched = false;
        for arm in node.conditions().iter() {
            let matches = match (&subject_literal, arm.as_when_node()) {
                (Some(subject), Some(when)) if self.select_case_arms => {
                    when_matches(&when, subject)
                }
                _ => None,
            };
            if matches != Some(false) {
                arms.push(arm);
            }
            if matches == Some(true) {
                matched = true;
                no_match_continues = false;
                break;
            }
        }
        let else_clause = else_clause.filter(|_| !matched);
        self.visit_alternatives(
            arms.into_iter()
                .chain(else_clause.map(|clause| clause.as_node())),
            no_match_continues,
        );
    }

    fn visit_case_match_node(&mut self, node: &ruby_prism::CaseMatchNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        if let Some(predicate) = node.predicate() {
            self.visit(&predicate);
        }
        self.visit_alternatives(
            node.conditions()
                .iter()
                .chain(node.else_clause().map(|clause| clause.as_node())),
            false,
        );
    }

    fn visit_while_node(&mut self, node: &ruby_prism::WhileNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let enters = self.loop_entry(node.predicate(), node.is_begin_modifier());
        self.visit_loop(Some(node.predicate()), node.statements(), enters);
    }

    fn visit_until_node(&mut self, node: &ruby_prism::UntilNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let enters = self
            .loop_entry(node.predicate(), node.is_begin_modifier())
            .map(|truthy| !truthy);
        self.visit_loop(Some(node.predicate()), node.statements(), enters);
    }

    fn visit_for_node(&mut self, node: &ruby_prism::ForNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        self.visit(&node.collection());
        self.visit(&node.index());
        self.visit_loop(None, node.statements(), None);
    }

    fn visit_begin_node(&mut self, node: &ruby_prism::BeginNode<'pr>) {
        if self.exceeded_depth() {
            return;
        }
        let entry = self.state.clone();
        self.rescue_must_hold.push(entry.must_hold.clone());
        if let Some(statements) = node.statements() {
            self.visit_statements_node(&statements);
        }
        let mut rescue_must_hold = self.rescue_must_hold.pop().unwrap_or_default();
        if node
            .rescue_clause()
            .is_some_and(|clause| rescue_may_retry(&clause))
        {
            rescue_must_hold.clear();
        }
        let body_end = self.state.clone();
        if let Some(else_clause) = node.else_clause() {
            self.visit_else_node(&else_clause);
        }
        let mut joined = std::mem::take(&mut self.state);
        let mut rescue_clause = node.rescue_clause();
        while let Some(clause) = rescue_clause {
            self.state = self.rescue_entry(&entry, &body_end, rescue_must_hold.clone());
            for exception in clause.exceptions().iter() {
                self.visit(&exception);
            }
            if let Some(reference) = clause.reference() {
                self.visit(&reference);
            }
            if let Some(statements) = clause.statements() {
                self.visit_statements_node(&statements);
            }
            joined = joined.join(std::mem::take(&mut self.state));
            rescue_clause = clause.subsequent();
        }
        self.state = joined;
        if let Some(ensure_clause) = node.ensure_clause() {
            let exits_dead = self.state.dead;
            if exits_dead {
                self.state = self.rescue_entry(&entry, &body_end, HashSet::new());
            }
            if let Some(statements) = ensure_clause.statements() {
                self.visit_statements_node(&statements);
            }
            self.state.dead |= exits_dead;
        }
    }

    fn visit_rescue_modifier_node(&mut self, node: &ruby_prism::RescueModifierNode<'pr>) {
        let entry = self.state.clone();
        self.visit(&node.expression());
        let body_end = std::mem::take(&mut self.state);
        self.state = self.rescue_entry(&entry, &body_end, HashSet::new());
        self.visit(&node.rescue_expression());
        let after_rescue = std::mem::take(&mut self.state);
        self.state = body_end.join(after_rescue);
    }

    fn visit_return_node(&mut self, node: &ruby_prism::ReturnNode<'pr>) {
        ruby_prism::visit_return_node(self, node);
        if !self.state.dead {
            self.returned_uninvoked |= !self.state.invoked;
            self.returned_unforwarded |= !self.state.forwarded;
        }
        self.state = FlowState::dead();
    }

    fn visit_break_node(&mut self, node: &ruby_prism::BreakNode<'pr>) {
        ruby_prism::visit_break_node(self, node);
        let exit = std::mem::replace(&mut self.state, FlowState::dead());
        self.loop_exits.push(exit);
    }

    fn visit_next_node(&mut self, node: &ruby_prism::NextNode<'pr>) {
        ruby_prism::visit_next_node(self, node);
        let exit = std::mem::replace(&mut self.state, FlowState::dead());
        self.loop_exits.push(exit);
    }

    fn visit_redo_node(&mut self, _node: &ruby_prism::RedoNode<'pr>) {
        self.state = FlowState::dead();
    }

    fn visit_retry_node(&mut self, _node: &ruby_prism::RetryNode<'pr>) {
        self.state = FlowState::dead();
    }
}

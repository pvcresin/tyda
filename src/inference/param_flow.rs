//! How a method body uses each required positional parameter, for rendering it as a type
//! parameter. A parameter whose every read flows into the return value is passed through
//! (`[T] (T x) -> T`). One that also has other reads, such as a method call on it, is still
//! returned but constrained by its callers (`[T < A | B] (T x) -> T`). A parameter the body
//! narrows (a condition, `case`, pattern, or type test) keeps the narrowed return instead.

use ruby_prism::{Node, Visit};

use crate::registry::ParamFlow;

/// Flows of the required positional parameters at the given indexes; parameters that are
/// reassigned, shadowed, or never returned are left out.
pub(super) fn param_flows(body: &Node<'_>, params: &[(usize, &str)]) -> Vec<(usize, ParamFlow)> {
    let mut return_values = Vec::new();
    collect_explicit_returns(body, &mut return_values);
    params
        .iter()
        .filter_map(|&(index, name)| {
            let name = name.as_bytes();
            let mut uses = LocalUses {
                name,
                reads: 0,
                disqualified: false,
            };
            uses.visit(body);
            if uses.disqualified || uses.reads == 0 {
                return None;
            }
            let mut narrowing = Narrowing { name, found: false };
            narrowing.visit(body);
            if narrowing.found {
                return None;
            }
            let returned = flow_reads(body, name)
                + return_values
                    .iter()
                    .map(|value| flow_reads(value, name))
                    .sum::<usize>();
            match returned {
                0 => None,
                returned if returned == uses.reads => Some((index, ParamFlow::PassThrough)),
                _ => Some((index, ParamFlow::Constrained)),
            }
        })
        .collect()
}

/// Reads of `name` in positions whose value becomes the value of `node`.
fn flow_reads(node: &Node<'_>, name: &[u8]) -> usize {
    match node {
        Node::LocalVariableReadNode { .. } => usize::from(
            node.as_local_variable_read_node()
                .map(|read| read.name().as_slice())
                == Some(name),
        ),
        Node::StatementsNode { .. } => node
            .as_statements_node()
            .and_then(|statements| statements.body().iter().last())
            .map_or(0, |last| flow_reads(&last, name)),
        Node::ParenthesesNode { .. } => node
            .as_parentheses_node()
            .and_then(|parens| parens.body())
            .map_or(0, |body| flow_reads(&body, name)),
        Node::ArrayNode { .. } => node.as_array_node().map_or(0, |array| {
            array
                .elements()
                .iter()
                .map(|element| flow_reads(&element, name))
                .sum()
        }),
        Node::HashNode { .. } => node.as_hash_node().map_or(0, |hash| {
            hash.elements()
                .iter()
                .filter_map(|element| element.as_assoc_node())
                .map(|assoc| flow_reads(&assoc.value(), name))
                .sum()
        }),
        Node::IfNode { .. } => node.as_if_node().map_or(0, |if_node| {
            if_node
                .statements()
                .map_or(0, |statements| flow_reads(&statements.as_node(), name))
                + if_node
                    .subsequent()
                    .map_or(0, |subsequent| flow_reads(&subsequent, name))
        }),
        Node::UnlessNode { .. } => node.as_unless_node().map_or(0, |unless_node| {
            unless_node
                .statements()
                .map_or(0, |statements| flow_reads(&statements.as_node(), name))
                + unless_node
                    .else_clause()
                    .map_or(0, |else_clause| flow_reads(&else_clause.as_node(), name))
        }),
        Node::ElseNode { .. } => node
            .as_else_node()
            .and_then(|else_node| else_node.statements())
            .map_or(0, |statements| flow_reads(&statements.as_node(), name)),
        _ => 0,
    }
}

/// Values of `return` statements, excluding nested methods and lambdas.
fn collect_explicit_returns<'pr>(body: &Node<'pr>, out: &mut Vec<Node<'pr>>) {
    struct Returns<'a, 'pr> {
        out: &'a mut Vec<Node<'pr>>,
    }
    impl<'pr> Visit<'pr> for Returns<'_, 'pr> {
        fn visit_return_node(&mut self, node: &ruby_prism::ReturnNode<'pr>) {
            if let Some(arguments) = node.arguments() {
                let mut values = arguments.arguments().iter();
                if let (Some(value), None) = (values.next(), values.next()) {
                    self.out.push(value);
                }
            }
            ruby_prism::visit_return_node(self, node);
        }
        fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
        fn visit_lambda_node(&mut self, _node: &ruby_prism::LambdaNode<'pr>) {}
    }
    Returns { out }.visit(body);
}

/// Counts reads of a local and marks writes or same-named block parameters, which would make
/// a read refer to some other value.
struct LocalUses<'a> {
    name: &'a [u8],
    reads: usize,
    disqualified: bool,
}

impl LocalUses<'_> {
    fn check(&mut self, node: &Node<'_>) {
        match node {
            Node::LocalVariableReadNode { .. } => {
                if node
                    .as_local_variable_read_node()
                    .map(|read| read.name().as_slice())
                    == Some(self.name)
                {
                    self.reads += 1;
                }
            }
            Node::LocalVariableWriteNode { .. }
            | Node::LocalVariableAndWriteNode { .. }
            | Node::LocalVariableOrWriteNode { .. }
            | Node::LocalVariableOperatorWriteNode { .. }
            | Node::LocalVariableTargetNode { .. }
            | Node::RequiredParameterNode { .. }
            | Node::OptionalParameterNode { .. }
            | Node::BlockLocalVariableNode { .. } => {
                let source = node.location().as_slice();
                let written = source
                    .strip_prefix(self.name)
                    .is_some_and(|rest| rest.first().is_none_or(|byte| !is_identifier_byte(*byte)));
                if written {
                    self.disqualified = true;
                }
            }
            _ => {}
        }
    }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

impl<'pr> Visit<'pr> for LocalUses<'_> {
    fn visit_branch_node_enter(&mut self, node: Node<'pr>) {
        self.check(&node);
    }
    fn visit_leaf_node_enter(&mut self, node: Node<'pr>) {
        self.check(&node);
    }
    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
}

/// Whether the body narrows the local: a condition, `case` subject, pattern match, or type or
/// equality test on it.
struct Narrowing<'a> {
    name: &'a [u8],
    found: bool,
}

const NARROWING_METHODS: &[&[u8]] = &[
    b"is_a?",
    b"kind_of?",
    b"instance_of?",
    b"nil?",
    b"==",
    b"!=",
    b"===",
    b"eql?",
    b"equal?",
    b"respond_to?",
    b"!",
    b"present?",
    b"blank?",
];

impl Narrowing<'_> {
    fn is_local(&self, node: &Node<'_>) -> bool {
        let node = match node {
            Node::ParenthesesNode { .. } => {
                match node
                    .as_parentheses_node()
                    .and_then(|parens| parens.body())
                    .and_then(|body| body.as_statements_node())
                    .and_then(|statements| statements.body().iter().last())
                {
                    Some(inner) => return self.is_local(&inner),
                    None => return false,
                }
            }
            node => node,
        };
        node.as_local_variable_read_node()
            .is_some_and(|read| read.name().as_slice() == self.name)
    }
}

impl<'pr> Visit<'pr> for Narrowing<'_> {
    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        let name = node.name().as_slice();
        if NARROWING_METHODS.contains(&name)
            && (node
                .receiver()
                .is_some_and(|receiver| self.is_local(&receiver))
                || node
                    .arguments()
                    .is_some_and(|args| args.arguments().iter().any(|arg| self.is_local(&arg))))
        {
            self.found = true;
        }
        ruby_prism::visit_call_node(self, node);
    }
    fn visit_if_node(&mut self, node: &ruby_prism::IfNode<'pr>) {
        self.found |= self.is_local(&node.predicate());
        ruby_prism::visit_if_node(self, node);
    }
    fn visit_unless_node(&mut self, node: &ruby_prism::UnlessNode<'pr>) {
        self.found |= self.is_local(&node.predicate());
        ruby_prism::visit_unless_node(self, node);
    }
    fn visit_while_node(&mut self, node: &ruby_prism::WhileNode<'pr>) {
        self.found |= self.is_local(&node.predicate());
        ruby_prism::visit_while_node(self, node);
    }
    fn visit_until_node(&mut self, node: &ruby_prism::UntilNode<'pr>) {
        self.found |= self.is_local(&node.predicate());
        ruby_prism::visit_until_node(self, node);
    }
    fn visit_and_node(&mut self, node: &ruby_prism::AndNode<'pr>) {
        self.found |= self.is_local(&node.left());
        ruby_prism::visit_and_node(self, node);
    }
    fn visit_or_node(&mut self, node: &ruby_prism::OrNode<'pr>) {
        self.found |= self.is_local(&node.left());
        ruby_prism::visit_or_node(self, node);
    }
    fn visit_case_node(&mut self, node: &ruby_prism::CaseNode<'pr>) {
        self.found |= node
            .predicate()
            .is_some_and(|subject| self.is_local(&subject));
        ruby_prism::visit_case_node(self, node);
    }
    fn visit_case_match_node(&mut self, node: &ruby_prism::CaseMatchNode<'pr>) {
        self.found |= node
            .predicate()
            .is_some_and(|subject| self.is_local(&subject));
        ruby_prism::visit_case_match_node(self, node);
    }
    fn visit_match_required_node(&mut self, node: &ruby_prism::MatchRequiredNode<'pr>) {
        self.found |= self.is_local(&node.value());
        ruby_prism::visit_match_required_node(self, node);
    }
    fn visit_match_predicate_node(&mut self, node: &ruby_prism::MatchPredicateNode<'pr>) {
        self.found |= self.is_local(&node.value());
        ruby_prism::visit_match_predicate_node(self, node);
    }
    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
}

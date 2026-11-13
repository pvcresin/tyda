//! How a method body uses each required positional parameter, for rendering it as a type
//! parameter. A parameter whose every read flows into the return value is passed through
//! (`[T] (T x) -> T`). One that also has other reads, such as a method call on it, is still
//! returned but constrained by its callers (`[T < A | B] (T x) -> T`). A parameter the body
//! narrows (a condition, `case`, pattern, or type test) keeps the narrowed return instead.
//! Yielding a parameter as is passes it to the block parameter, which counts like returning it.

use ruby_prism::{Node, Visit};

use crate::registry::ParamFlow;

/// Guards pathological nesting; past it, no parameter is rendered as a type parameter.
const MAX_SCAN_DEPTH: usize = 256;

/// Flows of the required positional parameters at the given indexes; parameters that are
/// reassigned, shadowed, or never returned are left out.
pub(super) fn param_flows(body: &Node<'_>, params: &[(usize, &str)]) -> Vec<(usize, ParamFlow)> {
    if params.is_empty() {
        return Vec::new();
    }
    let mut scan = ParamScan {
        names: params.iter().map(|(_, name)| name.as_bytes()).collect(),
        uses: vec![ParamUse::default(); params.len()],
        return_values: Vec::new(),
        lambda_depth: 0,
        depth: 0,
        too_deep: false,
    };
    scan.visit(body);
    if scan.too_deep {
        return Vec::new();
    }
    params
        .iter()
        .zip(&scan.uses)
        .filter_map(|(&(index, name), uses)| {
            if uses.disqualified || uses.narrowed || uses.reads == 0 {
                return None;
            }
            let name = name.as_bytes();
            let returned = flow_reads(body, name)
                + scan
                    .return_values
                    .iter()
                    .map(|value| flow_reads(value, name))
                    .sum::<usize>()
                + uses.yielded;
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
    let mut reads = 0;
    let mut pending = Vec::new();
    flow_step(node, name, &mut reads, &mut pending);
    while let Some(node) = pending.pop() {
        flow_step(&node, name, &mut reads, &mut pending);
    }
    reads
}

fn flow_step<'pr>(node: &Node<'pr>, name: &[u8], reads: &mut usize, pending: &mut Vec<Node<'pr>>) {
    match node {
        Node::LocalVariableReadNode { .. } => {
            *reads += usize::from(
                node.as_local_variable_read_node()
                    .map(|read| read.name().as_slice())
                    == Some(name),
            );
        }
        Node::StatementsNode { .. } => pending.extend(
            node.as_statements_node()
                .and_then(|statements| statements.body().iter().last()),
        ),
        Node::ParenthesesNode { .. } => {
            pending.extend(node.as_parentheses_node().and_then(|parens| parens.body()));
        }
        Node::ArrayNode { .. } => {
            if let Some(array) = node.as_array_node() {
                pending.extend(array.elements().iter());
            }
        }
        Node::HashNode { .. } => {
            if let Some(hash) = node.as_hash_node() {
                pending.extend(
                    hash.elements()
                        .iter()
                        .filter_map(|element| element.as_assoc_node())
                        .map(|assoc| assoc.value()),
                );
            }
        }
        Node::IfNode { .. } => {
            if let Some(if_node) = node.as_if_node() {
                pending.extend(if_node.statements().map(|statements| statements.as_node()));
                pending.extend(if_node.subsequent());
            }
        }
        Node::UnlessNode { .. } => {
            if let Some(unless_node) = node.as_unless_node() {
                pending.extend(
                    unless_node
                        .statements()
                        .map(|statements| statements.as_node()),
                );
                pending.extend(unless_node.else_clause().map(|clause| clause.as_node()));
            }
        }
        Node::ElseNode { .. } => pending.extend(
            node.as_else_node()
                .and_then(|else_node| else_node.statements())
                .map(|statements| statements.as_node()),
        ),
        _ => {}
    }
}

#[derive(Clone, Default)]
struct ParamUse {
    reads: usize,
    /// Reads passed as is to `yield`.
    yielded: usize,
    /// A write or same-named parameter makes a read refer to some other value.
    disqualified: bool,
    /// A condition, `case` subject, pattern match, or type or equality test on it.
    narrowed: bool,
}

/// One walk over the body for all parameters. Nested methods are skipped, and `return`
/// inside a lambda returns from the lambda, not the method.
struct ParamScan<'a, 'pr> {
    names: Vec<&'a [u8]>,
    uses: Vec<ParamUse>,
    return_values: Vec<Node<'pr>>,
    lambda_depth: usize,
    depth: usize,
    too_deep: bool,
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

impl<'pr> ParamScan<'_, 'pr> {
    fn param_index(&self, name: &[u8]) -> Option<usize> {
        self.names.iter().position(|param| *param == name)
    }

    fn local_param(&self, node: &Node<'_>) -> Option<usize> {
        match node {
            Node::ParenthesesNode { .. } => node
                .as_parentheses_node()
                .and_then(|parens| parens.body())
                .and_then(|body| body.as_statements_node())
                .and_then(|statements| statements.body().iter().last())
                .and_then(|inner| self.local_param(&inner)),
            node => node
                .as_local_variable_read_node()
                .and_then(|read| self.param_index(read.name().as_slice())),
        }
    }

    fn narrow(&mut self, node: &Node<'_>) {
        if let Some(index) = self.local_param(node) {
            self.uses[index].narrowed = true;
        }
    }

    fn count_use(&mut self, node: &Node<'_>) {
        match node {
            Node::LocalVariableReadNode { .. } => {
                if let Some(index) = node
                    .as_local_variable_read_node()
                    .and_then(|read| self.param_index(read.name().as_slice()))
                {
                    self.uses[index].reads += 1;
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
                for (index, name) in self.names.iter().enumerate() {
                    let written = source.strip_prefix(*name).is_some_and(|rest| {
                        rest.first().is_none_or(|byte| !is_identifier_byte(*byte))
                    });
                    if written {
                        self.uses[index].disqualified = true;
                    }
                }
            }
            _ => {}
        }
    }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// Stops the walk below nodes that nest arbitrarily once the depth guard has tripped.
macro_rules! guarded {
    ($($method:ident, $default:ident, $node:ident;)*) => {
        $(
            fn $method(&mut self, node: &ruby_prism::$node<'pr>) {
                if !self.too_deep {
                    ruby_prism::$default(self, node);
                }
            }
        )*
    };
}

impl<'pr> Visit<'pr> for ParamScan<'_, 'pr> {
    fn visit_branch_node_enter(&mut self, node: Node<'pr>) {
        self.depth += 1;
        self.too_deep |= self.depth > MAX_SCAN_DEPTH;
        self.count_use(&node);
    }

    fn visit_branch_node_leave(&mut self) {
        self.depth -= 1;
    }

    fn visit_leaf_node_enter(&mut self, node: Node<'pr>) {
        self.count_use(&node);
    }

    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}

    guarded! {
        visit_statements_node, visit_statements_node, StatementsNode;
        visit_arguments_node, visit_arguments_node, ArgumentsNode;
        visit_parentheses_node, visit_parentheses_node, ParenthesesNode;
        visit_array_node, visit_array_node, ArrayNode;
        visit_hash_node, visit_hash_node, HashNode;
        visit_keyword_hash_node, visit_keyword_hash_node, KeywordHashNode;
        visit_assoc_node, visit_assoc_node, AssocNode;
        visit_else_node, visit_else_node, ElseNode;
        visit_block_node, visit_block_node, BlockNode;
        visit_begin_node, visit_begin_node, BeginNode;
        visit_interpolated_string_node, visit_interpolated_string_node, InterpolatedStringNode;
        visit_embedded_statements_node, visit_embedded_statements_node, EmbeddedStatementsNode;
        visit_local_variable_write_node, visit_local_variable_write_node, LocalVariableWriteNode;
    }

    fn visit_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'pr>) {
        if self.too_deep {
            return;
        }
        self.lambda_depth += 1;
        ruby_prism::visit_lambda_node(self, node);
        self.lambda_depth -= 1;
    }

    fn visit_return_node(&mut self, node: &ruby_prism::ReturnNode<'pr>) {
        if self.lambda_depth == 0
            && let Some(arguments) = node.arguments()
        {
            let mut values = arguments.arguments().iter();
            if let (Some(value), None) = (values.next(), values.next()) {
                self.return_values.push(value);
            }
        }
        ruby_prism::visit_return_node(self, node);
    }

    fn visit_yield_node(&mut self, node: &ruby_prism::YieldNode<'pr>) {
        if let Some(arguments) = node.arguments() {
            for argument in arguments.arguments().iter() {
                if let Some(index) = argument
                    .as_local_variable_read_node()
                    .and_then(|read| self.param_index(read.name().as_slice()))
                {
                    self.uses[index].yielded += 1;
                }
            }
        }
        ruby_prism::visit_yield_node(self, node);
    }

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        if self.too_deep {
            return;
        }
        if NARROWING_METHODS.contains(&node.name().as_slice()) {
            if let Some(receiver) = node.receiver() {
                self.narrow(&receiver);
            }
            if let Some(arguments) = node.arguments() {
                for argument in arguments.arguments().iter() {
                    self.narrow(&argument);
                }
            }
        }
        ruby_prism::visit_call_node(self, node);
    }

    fn visit_if_node(&mut self, node: &ruby_prism::IfNode<'pr>) {
        if self.too_deep {
            return;
        }
        self.narrow(&node.predicate());
        ruby_prism::visit_if_node(self, node);
    }

    fn visit_unless_node(&mut self, node: &ruby_prism::UnlessNode<'pr>) {
        if self.too_deep {
            return;
        }
        self.narrow(&node.predicate());
        ruby_prism::visit_unless_node(self, node);
    }

    fn visit_while_node(&mut self, node: &ruby_prism::WhileNode<'pr>) {
        self.narrow(&node.predicate());
        ruby_prism::visit_while_node(self, node);
    }

    fn visit_until_node(&mut self, node: &ruby_prism::UntilNode<'pr>) {
        self.narrow(&node.predicate());
        ruby_prism::visit_until_node(self, node);
    }

    fn visit_and_node(&mut self, node: &ruby_prism::AndNode<'pr>) {
        if self.too_deep {
            return;
        }
        self.narrow(&node.left());
        ruby_prism::visit_and_node(self, node);
    }

    fn visit_or_node(&mut self, node: &ruby_prism::OrNode<'pr>) {
        if self.too_deep {
            return;
        }
        self.narrow(&node.left());
        ruby_prism::visit_or_node(self, node);
    }

    fn visit_case_node(&mut self, node: &ruby_prism::CaseNode<'pr>) {
        if let Some(subject) = node.predicate() {
            self.narrow(&subject);
        }
        ruby_prism::visit_case_node(self, node);
    }

    fn visit_case_match_node(&mut self, node: &ruby_prism::CaseMatchNode<'pr>) {
        if let Some(subject) = node.predicate() {
            self.narrow(&subject);
        }
        ruby_prism::visit_case_match_node(self, node);
    }

    fn visit_match_required_node(&mut self, node: &ruby_prism::MatchRequiredNode<'pr>) {
        self.narrow(&node.value());
        ruby_prism::visit_match_required_node(self, node);
    }

    fn visit_match_predicate_node(&mut self, node: &ruby_prism::MatchPredicateNode<'pr>) {
        self.narrow(&node.value());
        ruby_prism::visit_match_predicate_node(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flows(source: &str) -> Vec<(usize, ParamFlow)> {
        let parse_result = ruby_prism::parse(source.as_bytes());
        let program = parse_result.node();
        let def_node = program
            .as_program_node()
            .and_then(|program| program.statements().body().iter().next())
            .and_then(|node| node.as_def_node())
            .expect("def");
        let body = def_node.body().expect("body");
        param_flows(&body, &[(0, "x"), (1, "y")])
    }

    #[test]
    fn classifies_each_parameter_in_one_walk() {
        assert_eq!(
            flows("def f(x, y)\n  y.to_s\n  [x, y]\nend\n"),
            vec![(0, ParamFlow::PassThrough), (1, ParamFlow::Constrained)]
        );
        assert_eq!(
            flows("def f(x, y)\n  yield x\n  nil\nend\n"),
            vec![(0, ParamFlow::PassThrough)]
        );
        assert_eq!(
            flows("def f(x, y)\n  return y if x.nil?\n  x\nend\n"),
            vec![(1, ParamFlow::PassThrough)]
        );
        assert!(flows("def f(x, y)\n  x = 1\n  x\nend\n").is_empty());
        assert!(flows("def f(x, y)\n  -> { return x }\n  nil\nend\n").is_empty());
    }

    #[test]
    fn pathological_nesting_renders_no_type_parameters() {
        let depth = MAX_SCAN_DEPTH;
        let source = format!(
            "def f(x, y)\n  {}x{}\nend\n",
            "y > 0 ? 1 : (".repeat(depth),
            ")".repeat(depth)
        );
        // Debug builds need more than the default test stack to reach the guard.
        let flows = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || flows(&source))
            .expect("spawn")
            .join()
            .expect("join");
        assert!(flows.is_empty());
    }
}

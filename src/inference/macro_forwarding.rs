//! Which parameters a class method passes on to another receiverless call, as macros do
//! (`def mount_file(uploader, field: :file) = mount_uploader(field, uploader)`).

use ruby_prism::{Node, Visit};

use crate::registry::ParamInfo;
use crate::types::Sym;

struct ForwardedParams<'p> {
    params: &'p [ParamInfo],
    forwarded: Vec<Sym>,
}

impl ForwardedParams<'_> {
    fn note_argument(&mut self, node: &Node<'_>) {
        let read = match node {
            Node::SplatNode { .. } => node
                .as_splat_node()
                .and_then(|splat| splat.expression())
                .and_then(|inner| inner.as_local_variable_read_node()),
            _ => node.as_local_variable_read_node(),
        };
        let Some(read) = read else {
            if let Some(hash) = node.as_keyword_hash_node() {
                for element in hash.elements().iter() {
                    if let Some(pair) = element.as_assoc_node() {
                        self.note_argument(&pair.value());
                    }
                }
            }
            return;
        };
        let name = String::from_utf8_lossy(read.name().as_slice());
        if self.params.iter().any(|param| param.name == name)
            && !self.forwarded.iter().any(|sym| sym.as_str() == name)
        {
            self.forwarded.push(Sym::new(&name));
        }
    }
}

impl<'pr> Visit<'pr> for ForwardedParams<'_> {
    fn visit_def_node(&mut self, _node: &ruby_prism::DefNode<'pr>) {}
    fn visit_class_node(&mut self, _node: &ruby_prism::ClassNode<'pr>) {}
    fn visit_module_node(&mut self, _node: &ruby_prism::ModuleNode<'pr>) {}
    fn visit_singleton_class_node(&mut self, _node: &ruby_prism::SingletonClassNode<'pr>) {}

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'pr>) {
        let receiverless = node
            .receiver()
            .is_none_or(|receiver| matches!(receiver, Node::SelfNode { .. }));
        if receiverless && let Some(arguments) = node.arguments() {
            for argument in arguments.arguments().iter() {
                self.note_argument(&argument);
            }
        }
        ruby_prism::visit_call_node(self, node);
    }
}

/// The parameters of `def_node` its body passes as arguments to a receiverless call.
pub(super) fn forwarded_macro_params(
    def_node: &ruby_prism::DefNode<'_>,
    params: &[ParamInfo],
) -> Vec<Sym> {
    let Some(body) = def_node.body() else {
        return Vec::new();
    };
    if params.is_empty() {
        return Vec::new();
    }
    let mut visitor = ForwardedParams {
        params,
        forwarded: Vec::new(),
    };
    visitor.visit(&body);
    visitor.forwarded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ParamKind;

    fn forwarded(source: &str) -> Vec<String> {
        let parse_result = ruby_prism::parse(source.as_bytes());
        let program = parse_result.node();
        let statements = program
            .as_program_node()
            .expect("program")
            .statements()
            .body();
        let def_node = statements
            .iter()
            .next()
            .and_then(|node| node.as_def_node())
            .expect("def");
        let params: Vec<ParamInfo> = def_node
            .parameters()
            .map(|parameters| {
                let mut names: Vec<String> = parameters
                    .requireds()
                    .iter()
                    .chain(parameters.optionals().iter())
                    .chain(parameters.keywords().iter())
                    .filter_map(|param| {
                        let location = param.location();
                        let text = String::from_utf8_lossy(location.as_slice()).to_string();
                        text.split(|c: char| !c.is_alphanumeric() && c != '_')
                            .next()
                            .map(str::to_string)
                    })
                    .collect();
                if let Some(rest) = parameters
                    .rest()
                    .and_then(|rest| rest.as_rest_parameter_node())
                    && let Some(name) = rest.name()
                {
                    names.push(String::from_utf8_lossy(name.as_slice()).to_string());
                }
                names
            })
            .unwrap_or_default()
            .into_iter()
            .map(|name| ParamInfo {
                name,
                kind: ParamKind::Required,
                default_type: None,
            })
            .collect();
        forwarded_macro_params(&def_node, &params)
            .iter()
            .map(|sym| sym.as_str().to_string())
            .collect()
    }

    #[test]
    fn parameters_passed_to_another_macro_are_forwarded() {
        assert_eq!(
            forwarded(
                "def mount_file(uploader, field: :file)\n  mount_uploader(field, uploader)\nend\n"
            ),
            ["field", "uploader"]
        );
        assert_eq!(
            forwarded(
                "def secret(*names)\n  names.each { |n| self.attr_encrypted(n) }\n  define(*names)\nend\n"
            ),
            ["names"]
        );
    }

    #[test]
    fn stored_parameters_are_not_forwarded() {
        assert!(forwarded("def nullify_if_blank(*attributes)\n  self.attributes_to_nullify += attributes\nend\n").is_empty());
        assert!(forwarded("def track(name)\n  registry.add(name)\nend\n").is_empty());
    }
}

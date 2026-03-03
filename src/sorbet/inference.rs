use crate::types::Sym;
use ruby_prism::{Node, ParseResult};

use crate::inference::{InferenceEngine, Scope};
use crate::rbs::convert::convert_rbs_type;
use crate::sorbet::sig::convert_sorbet_type_str;
use crate::types::Type;

impl<'a> InferenceEngine<'a> {
    /// Detect Sorbet T.let / T.cast / T.must / T.unsafe / T.absurd / T.bind calls
    /// and return the appropriate type.
    pub(crate) fn try_resolve_sorbet_assertion(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) -> Option<Type> {
        let receiver = call_node.receiver()?;
        let receiver_name = match &receiver {
            Node::ConstantReadNode { .. } => {
                let constant = receiver.as_constant_read_node()?;
                String::from_utf8_lossy(constant.name().as_slice()).to_string()
            }
            _ => return None,
        };
        if receiver_name != "T" {
            return None;
        }

        let args = call_node.arguments()?;
        let arg_list: Vec<Node<'_>> = args.arguments().iter().collect();

        match method_name {
            "let" | "cast" | "assert_type!" => {
                if arg_list.len() >= 2 {
                    let type_arg = &arg_list[1];
                    return self.resolve_sorbet_assertion_type_arg(
                        class_name,
                        type_arg,
                        parse_result,
                    );
                }
                None
            }
            "must" | "must_because" => {
                if !arg_list.is_empty() {
                    let inner = self.infer_node_type(class_name, &arg_list[0], parse_result, scope);
                    return Some(Self::remove_nil(&inner));
                }
                None
            }
            "unsafe" => Some(Type::Untyped),
            "absurd" => Some(Type::Bot),
            "bind" => {
                if arg_list.len() >= 2 {
                    let type_arg = &arg_list[1];
                    return self.resolve_sorbet_assertion_type_arg(
                        class_name,
                        type_arg,
                        parse_result,
                    );
                }
                None
            }
            _ => None,
        }
    }

    fn resolve_sorbet_assertion_type_arg(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Type> {
        self.resolve_sorbet_type_node(class_name, node, parse_result)
            .or_else(|| {
                let raw = String::from_utf8_lossy(node.location().as_slice()).to_string();
                if let Ok(rbs_type) = rbs_sys::parse_type(&raw) {
                    return Some(convert_rbs_type(&crate::rbs::ir::RbsType::from(&rbs_type)));
                }
                let rbs = convert_sorbet_type_str(&raw);
                rbs_sys::parse_type(&rbs)
                    .ok()
                    .map(|rbs_type| convert_rbs_type(&crate::rbs::ir::RbsType::from(&rbs_type)))
            })
    }

    pub(crate) fn resolve_sorbet_type_node(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Type> {
        match node {
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                let type_name = self.resolve_constant_path(node, parse_result);
                (type_name != "Unknown")
                    .then(|| self.sorbet_type_name_to_type(&type_name, class_name))
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node()?;
                let method_name = String::from_utf8_lossy(call.name().as_slice()).to_string();

                if method_name == "[]"
                    && let Some(receiver) = call.receiver()
                {
                    let base_name = self.resolve_constant_path(&receiver, parse_result);
                    if base_name == "Unknown" {
                        return None;
                    }
                    let args = call.arguments()?;
                    let arg_types: Vec<Type> = args
                        .arguments()
                        .iter()
                        .map(|arg| {
                            self.resolve_sorbet_type_node(class_name, &arg, parse_result)
                                .unwrap_or(Type::Untyped)
                        })
                        .collect();
                    return Some(self.sorbet_generic_type(&base_name, arg_types, class_name));
                }

                let receiver_name = call
                    .receiver()
                    .map(|receiver| self.resolve_constant_path(&receiver, parse_result));
                if !matches!(receiver_name.as_deref(), Some("T") | Some("::T")) {
                    return None;
                }

                let arg_nodes: Vec<Node<'_>> = call
                    .arguments()
                    .map(|args| args.arguments().iter().collect())
                    .unwrap_or_default();

                match method_name.as_str() {
                    "nilable" => {
                        let inner = arg_nodes.first()?;
                        let inner_ty =
                            self.resolve_sorbet_type_node(class_name, inner, parse_result)?;
                        if inner_ty == Type::Untyped {
                            Some(Type::Untyped)
                        } else {
                            Some(inner_ty.union_with(Type::Nil))
                        }
                    }
                    "any" => {
                        let parts = arg_nodes
                            .iter()
                            .map(|arg| {
                                self.resolve_sorbet_type_node(class_name, arg, parse_result)
                                    .unwrap_or(Type::Untyped)
                            })
                            .collect();
                        Some(Type::from_type_vec_preserve_untyped(parts))
                    }
                    "all" => {
                        let parts: Vec<Type> = arg_nodes
                            .iter()
                            .map(|arg| {
                                self.resolve_sorbet_type_node(class_name, arg, parse_result)
                                    .unwrap_or(Type::Untyped)
                            })
                            .collect();
                        match parts.len() {
                            0 => Some(Type::Untyped),
                            1 => parts.into_iter().next(),
                            _ => Some(Type::Intersection(parts)),
                        }
                    }
                    "class_of" => {
                        let inner = arg_nodes.first()?;
                        let inner_ty =
                            self.resolve_sorbet_type_node(class_name, inner, parse_result)?;
                        Self::nominal_type_name(&inner_ty)
                            .map(|name| Type::Singleton(Sym::new(name)))
                    }
                    "type_parameter" => Some(Type::Untyped),
                    "untyped" => Some(Type::Untyped),
                    "noreturn" => Some(Type::Bot),
                    // `T.attached_class` -> the receiver's instance type (prevents collapsing to singleton in a singleton factory).
                    "self_type" => Some(Type::SelfType),
                    "attached_class" => Some(Type::InstanceType),
                    "anything" => Some(Type::Top),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn sorbet_type_name_to_type(&self, type_name: &str, class_name: &str) -> Type {
        let bare = type_name.strip_prefix("::").unwrap_or(type_name);
        match bare {
            "T::Boolean" => Type::Bool,
            "T::Array" => Type::Array(None),
            "T::Hash" => Type::Hash(None, None),
            "T.untyped" => Type::Untyped,
            "T.noreturn" => Type::Bot,
            "T.anything" => Type::Top,
            "NilClass" => Type::Nil,
            "TrueClass" => Type::True,
            "FalseClass" => Type::False,
            _ => self.resolve_type_alias_or_class(bare, class_name),
        }
    }

    fn sorbet_generic_type(&self, base_name: &str, mut args: Vec<Type>, class_name: &str) -> Type {
        let bare = base_name.strip_prefix("::").unwrap_or(base_name);
        match bare {
            "Array" | "T::Array" => {
                if args.len() == 1 {
                    Type::Array(Some(Box::new(args.remove(0))))
                } else {
                    Type::Array(None)
                }
            }
            "Hash" | "T::Hash" => {
                if args.len() == 2 {
                    Type::Hash(
                        Some(Box::new(args.remove(0))),
                        Some(Box::new(args.remove(0))),
                    )
                } else {
                    Type::Hash(None, None)
                }
            }
            "T::Boolean" => Type::Bool,
            _ => {
                let base = match bare {
                    "T::Range" => "Range".to_string(),
                    "T::Enumerable" => "Enumerable".to_string(),
                    "T::Enumerator" => "Enumerator".to_string(),
                    "T::Set" => "Set".to_string(),
                    other => (match self.resolve_type_alias_or_class(other, class_name) {
                        Type::Class(name) => name,
                        ty => return ty,
                    })
                    .to_string(),
                };
                Type::Generic {
                    base: Sym::new(&base),
                    args: args.into(),
                }
            }
        }
    }

    fn nominal_type_name(ty: &Type) -> Option<&str> {
        match ty {
            Type::Integer => Some("Integer"),
            Type::Float => Some("Float"),
            Type::String => Some("String"),
            Type::Symbol => Some("Symbol"),
            Type::Bool => Some("bool"),
            Type::True => Some("TrueClass"),
            Type::False => Some("FalseClass"),
            Type::Nil => Some("NilClass"),
            Type::Class(name) => Some(name.as_str()),
            Type::Singleton(name) => Some(name.as_str()),
            _ => None,
        }
    }

    pub(crate) fn is_type_member_call(
        &self,
        node: &Node<'_>,
        _parse_result: &ParseResult<'_>,
    ) -> bool {
        if let Some(call) = node.as_call_node() {
            let name = String::from_utf8_lossy(call.name().as_slice());
            matches!(name.as_ref(), "type_member" | "type_template")
        } else {
            false
        }
    }

    /// Detect `T.type_alias { SomeType }` and extract the aliased type.
    pub(crate) fn try_extract_type_alias(
        &self,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<Type> {
        let call = node.as_call_node()?;
        let method_name = String::from_utf8_lossy(call.name().as_slice());
        if method_name != "type_alias" {
            return None;
        }
        let receiver = call.receiver()?;
        let receiver_name = match &receiver {
            Node::ConstantReadNode { .. } => {
                let constant = receiver.as_constant_read_node()?;
                String::from_utf8_lossy(constant.name().as_slice()).to_string()
            }
            _ => return None,
        };
        if receiver_name != "T" {
            return None;
        }
        let block_raw = call.block()?;
        let block = block_raw.as_block_node()?;
        let body = block.body()?;
        let statements = body.as_statements_node()?;
        let statements: Vec<Node<'_>> = statements.body().iter().collect();
        let last = statements.last()?;
        let type_name = self.resolve_constant_path(last, parse_result);
        if type_name != "Unknown" {
            Some(Self::class_name_to_type(&type_name))
        } else {
            None
        }
    }
}

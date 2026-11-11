//! Maps tables to the model classes that read them, following Rails' `table_name`
//! computation so namespaced models get their columns as well as their associations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ruby_prism::{Node, ParseResult};

use super::inflector::{ProjectInflector, singularize_with, underscore};

/// Table name to model classes: explicit `self.table_name =` plus the name Rails computes
/// from the class (`table_name_prefix` of the enclosing module, the singular table of an
/// enclosing model class, and the pluralized demodulized class name).
#[derive(Default)]
pub(crate) struct ModelTableIndex {
    explicit: HashMap<String, Vec<String>>,
    /// Keyed by the singular table name, so plural forms resolve through `singularize`.
    computed: HashMap<String, Vec<String>>,
    inflector: ProjectInflector,
}

impl ModelTableIndex {
    pub(crate) fn load(root: &Path) -> Self {
        let mut scan = ModelScan::default();
        let mut files = Vec::new();
        collect_model_files(&root.join("app").join("models"), &mut files);
        for path in files {
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let parse_result = ruby_prism::parse(source.as_bytes());
            scan.visit(&parse_result.node(), &parse_result, &mut Vec::new());
        }
        scan.into_index(super::load_project_inflector(root))
    }

    /// Classes reading `table`, explicit declarations first.
    pub(crate) fn classes_for(&self, table: &str) -> Vec<&str> {
        let mut classes: Vec<&str> = self
            .explicit
            .get(table)
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        let singular = singularize_with(table, &self.inflector);
        for class in self.computed.get(&singular).into_iter().flatten() {
            if !classes.contains(&class.as_str()) {
                classes.push(class);
            }
        }
        classes
    }
}

struct ModelDecl {
    name: String,
    superclass: Option<String>,
    abstract_class: bool,
    explicit_table: Option<String>,
}

#[derive(Default)]
struct ModelScan {
    models: Vec<ModelDecl>,
    /// `table_name_prefix` defined on a module or class (method or class attribute).
    prefixes: HashMap<String, String>,
}

impl ModelScan {
    fn visit(
        &mut self,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        namespace: &mut Vec<String>,
    ) {
        match node {
            Node::ProgramNode { .. } => {
                let program = node.as_program_node().expect("must be ProgramNode");
                self.visit_statements(&program.statements().body(), parse_result, namespace);
            }
            Node::StatementsNode { .. } => {
                let stmts = node.as_statements_node().expect("must be StatementsNode");
                self.visit_statements(&stmts.body(), parse_result, namespace);
            }
            Node::ModuleNode { .. } => {
                let module_node = node.as_module_node().expect("must be ModuleNode");
                namespace.push(constant_path_text(&module_node.constant_path()));
                if let Some(body) = module_node.body() {
                    self.visit(&body, parse_result, namespace);
                }
                namespace.pop();
            }
            Node::ClassNode { .. } => {
                let class_node = node.as_class_node().expect("must be ClassNode");
                namespace.push(constant_path_text(&class_node.constant_path()));
                let index = self.models.len();
                self.models.push(ModelDecl {
                    name: namespace.join("::"),
                    superclass: class_node.superclass().map(|sc| {
                        String::from_utf8_lossy(sc.location().as_slice())
                            .trim()
                            .to_string()
                    }),
                    abstract_class: false,
                    explicit_table: None,
                });
                if let Some(body) = class_node.body() {
                    self.visit_class_body(index, &body);
                    self.visit(&body, parse_result, namespace);
                }
                namespace.pop();
            }
            Node::DefNode { .. } => {
                let def = node.as_def_node().expect("must be DefNode");
                if def.name().as_slice() == b"table_name_prefix"
                    && def
                        .receiver()
                        .is_some_and(|r| matches!(r, Node::SelfNode { .. }))
                    && !namespace.is_empty()
                    && let Some(prefix) = def.body().and_then(|body| single_string(&body))
                {
                    self.prefixes.insert(namespace.join("::"), prefix);
                }
            }
            _ => {}
        }
    }

    fn visit_statements(
        &mut self,
        body: &ruby_prism::NodeList<'_>,
        parse_result: &ParseResult<'_>,
        namespace: &mut Vec<String>,
    ) {
        for stmt in body.iter() {
            self.visit(&stmt, parse_result, namespace);
        }
    }

    fn visit_class_body(&mut self, index: usize, body: &Node<'_>) {
        let Some(stmts) = body.as_statements_node() else {
            return;
        };
        for stmt in stmts.body().iter() {
            let Some(call) = stmt.as_call_node() else {
                continue;
            };
            match call.name().as_slice() {
                b"primary_abstract_class" => self.models[index].abstract_class = true,
                b"abstract_class=" => {
                    self.models[index].abstract_class =
                        first_arg(&call).is_some_and(|arg| matches!(arg, Node::TrueNode { .. }));
                }
                b"table_name_prefix=" => {
                    if let Some(prefix) = first_arg(&call).and_then(|arg| single_string(&arg)) {
                        let name = self.models[index].name.clone();
                        self.prefixes.insert(name, prefix);
                    }
                }
                b"table_name=" => {
                    self.models[index].explicit_table =
                        first_arg(&call).and_then(|arg| single_string(&arg));
                }
                _ => {}
            }
        }
    }

    fn into_index(self, inflector: ProjectInflector) -> ModelTableIndex {
        let models: HashMap<&str, &ModelDecl> = self
            .models
            .iter()
            .map(|model| (model.name.as_str(), model))
            .collect();
        let resolved = Resolved {
            superclasses: self
                .models
                .iter()
                .filter_map(|model| {
                    let superclass = model.superclass.as_deref()?;
                    Some((
                        model.name.as_str(),
                        resolve_relative(&model.name, superclass, &models),
                    ))
                })
                .collect(),
            models: &models,
            prefixes: &self.prefixes,
        };
        let mut index = ModelTableIndex {
            inflector,
            ..ModelTableIndex::default()
        };
        for model in &self.models {
            if let Some(table) = &model.explicit_table {
                push_unique(
                    index.explicit.entry(table.clone()).or_default(),
                    &model.name,
                );
            } else if resolved.is_base_model(&model.name) {
                let singular = resolved.singular_table_name(&model.name);
                push_unique(index.computed.entry(singular).or_default(), &model.name);
            }
        }
        index
    }
}

struct Resolved<'a> {
    superclasses: HashMap<&'a str, String>,
    models: &'a HashMap<&'a str, &'a ModelDecl>,
    prefixes: &'a HashMap<String, String>,
}

impl Resolved<'_> {
    fn is_abstract(&self, name: &str) -> bool {
        self.models
            .get(name)
            .is_some_and(|model| model.abstract_class)
    }

    fn is_record_class(&self, name: &str) -> bool {
        let mut current = name.to_string();
        for _ in 0..self.models.len() + 1 {
            if matches!(current.as_str(), "ActiveRecord::Base" | "ApplicationRecord") {
                return true;
            }
            match self.superclasses.get(current.as_str()) {
                Some(superclass) => current = superclass.clone(),
                None => return false,
            }
        }
        false
    }

    /// A concrete record class whose superclass is abstract owns a table; STI subclasses
    /// read their superclass's table.
    fn is_base_model(&self, name: &str) -> bool {
        !self.is_abstract(name)
            && self.superclasses.get(name).is_some_and(|superclass| {
                matches!(
                    superclass.as_str(),
                    "ActiveRecord::Base" | "ApplicationRecord"
                ) || self.is_abstract(superclass) && self.is_record_class(superclass)
            })
    }

    /// Rails' `compute_table_name`, before pluralizing the last word.
    fn singular_table_name(&self, class_name: &str) -> String {
        let (parent, short) = class_name.rsplit_once("::").unwrap_or(("", class_name));
        let short = underscore(short);
        if !parent.is_empty() && self.is_base_model(parent) {
            return format!("{}_{short}", self.singular_table_name(parent));
        }
        format!("{}{short}", self.full_table_name_prefix(class_name))
    }

    /// `module_parents.detect { |p| p.respond_to?(:table_name_prefix) } || self`: a module
    /// answers only when it defines the prefix, a record class always does (class attribute).
    fn full_table_name_prefix(&self, class_name: &str) -> &str {
        let mut current = class_name;
        while let Some((parent, _)) = current.rsplit_once("::") {
            if let Some(prefix) = self.prefixes.get(parent) {
                return prefix;
            }
            if self.is_record_class(parent) {
                return self.inherited_prefix(parent);
            }
            current = parent;
        }
        self.inherited_prefix(class_name)
    }

    fn inherited_prefix(&self, class_name: &str) -> &str {
        let mut current = class_name;
        for _ in 0..self.models.len() + 1 {
            if let Some(prefix) = self.prefixes.get(current) {
                return prefix;
            }
            match self.superclasses.get(current) {
                Some(superclass) => current = superclass,
                None => break,
            }
        }
        ""
    }
}

/// Resolves a superclass reference from the class's lexical namespace outward.
fn resolve_relative(owner: &str, name: &str, models: &HashMap<&str, &ModelDecl>) -> String {
    if let Some(absolute) = name.strip_prefix("::") {
        return absolute.to_string();
    }
    let mut namespace = owner;
    while let Some((outer, _)) = namespace.rsplit_once("::") {
        let candidate = format!("{outer}::{name}");
        if candidate != owner && models.contains_key(candidate.as_str()) {
            return candidate;
        }
        namespace = outer;
    }
    name.to_string()
}

fn push_unique(classes: &mut Vec<String>, class_name: &str) {
    if !classes.iter().any(|existing| existing == class_name) {
        classes.push(class_name.to_string());
    }
}

fn first_arg<'pr>(call: &ruby_prism::CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments()?.arguments().iter().next()
}

fn single_string(node: &Node<'_>) -> Option<String> {
    if let Some(stmts) = node.as_statements_node() {
        let mut body = stmts.body().iter();
        let only = body.next()?;
        return if body.next().is_none() {
            single_string(&only)
        } else {
            None
        };
    }
    if let Some(string) = node.as_string_node() {
        return Some(String::from_utf8_lossy(string.unescaped()).to_string());
    }
    node.as_symbol_node()
        .map(|symbol| String::from_utf8_lossy(symbol.unescaped()).to_string())
}

fn collect_model_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_model_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rb") {
            out.push(path);
        }
    }
}

fn constant_path_text(node: &Node<'_>) -> String {
    let text = match node {
        Node::ConstantReadNode { .. } => {
            let read = node
                .as_constant_read_node()
                .expect("must be ConstantReadNode");
            String::from_utf8_lossy(read.name().as_slice()).to_string()
        }
        _ => String::from_utf8_lossy(node.location().as_slice()).to_string(),
    };
    text.trim().trim_start_matches("::").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_for(files: &[(&str, &str)]) -> ModelTableIndex {
        let root = tempfile::tempdir().expect("tempdir");
        for (path, source) in files {
            let path = root.path().join("app/models").join(path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, source).expect("write model");
        }
        ModelTableIndex::load(root.path())
    }

    #[test]
    fn namespaced_models_follow_rails_table_name_rules() {
        let index = index_for(&[
            (
                "organizations/organization_detail.rb",
                "module Organizations\n  class OrganizationDetail < ApplicationRecord\n  end\nend\n",
            ),
            (
                "billing.rb",
                "module Billing\n  def self.table_name_prefix\n    \"billing_\"\n  end\nend\n",
            ),
            (
                "billing/invoice.rb",
                "class Billing::Invoice < ApplicationRecord\nend\n",
            ),
            (
                "post.rb",
                "class Post < ApplicationRecord\n  class Comment < ApplicationRecord\n  end\nend\n",
            ),
            ("admin.rb", "class Admin < Post\nend\n"),
            (
                "legacy.rb",
                "module Old\n  class Thing < ApplicationRecord\n    self.table_name = 'things_v1'\n  end\nend\n",
            ),
        ]);
        assert_eq!(
            index.classes_for("organization_details"),
            ["Organizations::OrganizationDetail"]
        );
        assert_eq!(index.classes_for("billing_invoices"), ["Billing::Invoice"]);
        assert!(index.classes_for("invoices").is_empty());
        assert_eq!(index.classes_for("post_comments"), ["Post::Comment"]);
        assert!(index.classes_for("admins").is_empty());
        assert_eq!(index.classes_for("things_v1"), ["Old::Thing"]);
        assert!(index.classes_for("things").is_empty());
    }

    #[test]
    fn abstract_base_classes_pass_their_prefix_to_subclasses() {
        let index = index_for(&[
            (
                "ci/application_record.rb",
                "module Ci
  class ApplicationRecord < ::ApplicationRecord
    self.abstract_class = true

    def self.table_name_prefix
      'ci_'
    end
  end
end
",
            ),
            (
                "ci/build_need.rb",
                "module Ci
  class BuildNeed < Ci::ApplicationRecord
  end
end
",
            ),
            (
                "ci/stage.rb",
                "module Ci
  class Stage < ApplicationRecord
  end
end
",
            ),
            (
                "shop/record.rb",
                "class Shop::Record < ApplicationRecord
  self.abstract_class = true
  self.table_name_prefix = \"shop_\"
end
",
            ),
            (
                "shop/order.rb",
                "class Shop::Order < Shop::Record
end
",
            ),
        ]);
        assert_eq!(index.classes_for("ci_build_needs"), ["Ci::BuildNeed"]);
        assert_eq!(index.classes_for("ci_stages"), ["Ci::Stage"]);
        assert_eq!(index.classes_for("shop_orders"), ["Shop::Order"]);
        assert!(index.classes_for("ci_application_records").is_empty());
    }
}

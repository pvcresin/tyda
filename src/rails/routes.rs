//! Route helper generation from `config/routes.rb`.
//!
//! Models the ActionDispatch routing mapper's naming rules: each DSL call
//! derives a child [`Scope`], and helper names come from the scope level the
//! same way `Mapper#name_for_action` builds them.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::rc::Rc;

use ruby_prism::{CallNode, Node};

use super::inflector::singularize;
use crate::registry::{MethodDef, MixinKind, ParamInfo, TypeRegistry};
use crate::types::ParamKind;
use crate::types::{Sym, Type};

const ROUTE_HELPERS_MODULE: &str = "Rails::GeneratedUrlHelpers";
const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "match"];

/// Helper name (without `_path` / `_url`) to required positional args.
type Helpers = BTreeMap<String, usize>;

/// Bit set of the seven canonical resource actions, used for `only:` / `except:`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Actions(u8);

impl Actions {
    const INDEX: u8 = 1;
    const CREATE: u8 = 1 << 1;
    const NEW: u8 = 1 << 2;
    const SHOW: u8 = 1 << 3;
    const UPDATE: u8 = 1 << 4;
    const DESTROY: u8 = 1 << 5;
    const EDIT: u8 = 1 << 6;
    const ALL: Self = Self(0x7f);

    fn bit(name: &str) -> u8 {
        match name {
            "index" => Self::INDEX,
            "create" => Self::CREATE,
            "new" => Self::NEW,
            "show" => Self::SHOW,
            "update" => Self::UPDATE,
            "destroy" => Self::DESTROY,
            "edit" => Self::EDIT,
            _ => 0,
        }
    }

    fn any(self, bits: u8) -> bool {
        self.0 & bits != 0
    }

    /// `only:` / `except:` filter; `None` when neither option is given.
    fn from_options(only: Option<&[String]>, except: Option<&[String]>) -> Option<Self> {
        if only.is_none() && except.is_none() {
            return None;
        }
        let collect = |names: &[String]| names.iter().fold(0, |bits, name| bits | Self::bit(name));
        let base = only.map_or(Self::ALL.0, collect);
        Some(Self(base & !except.map_or(0, collect)))
    }
}

#[derive(Debug)]
struct Resource {
    collection: String,
    member: String,
    singular: bool,
    shallow: bool,
}

#[derive(Clone, Debug, Default)]
enum Level {
    #[default]
    Plain,
    Resources(Rc<Resource>),
    Member(Rc<Resource>),
    Collection(Rc<Resource>),
    New(Rc<Resource>),
}

/// The subset of the mapper's `@scope` that affects helper names and arity.
#[derive(Clone, Debug, Default)]
struct Scope {
    as_prefix: Vec<String>,
    args: usize,
    shallow_prefix: Vec<String>,
    shallow_args: usize,
    shallow: bool,
    action_filter: Option<Actions>,
    level: Level,
}

impl Scope {
    fn resource(&self) -> Option<&Rc<Resource>> {
        match &self.level {
            Level::Resources(res) => Some(res),
            _ => None,
        }
    }

    fn with_level(&self, level: Level) -> Self {
        Self {
            level,
            ..self.clone()
        }
    }

    fn member_scope(&self, res: &Rc<Resource>) -> Self {
        let mut scope = self.with_level(Level::Member(Rc::clone(res)));
        if res.shallow {
            scope.as_prefix.clone_from(&self.shallow_prefix);
            scope.args = self.shallow_args;
        }
        scope.args += usize::from(!res.singular);
        scope
    }

    /// Scope of routes nested under a resource (`/users/:user_id/...`).
    fn nested_scope(&self, res: &Rc<Resource>) -> Self {
        let mut scope = self.member_scope(res);
        scope.as_prefix.push(res.member.clone());
        scope.level = Level::Plain;
        scope
    }

    /// DSL calls made directly in a `resources` block are nested under it.
    fn nested_if_resource(&self) -> Self {
        self.resource()
            .map_or_else(|| self.clone(), |res| self.nested_scope(res))
    }

    /// `Mapper#name_for_action`: `leaf` is the action or `as:` name, `None` for
    /// canonical actions.
    fn name_for(&self, leaf: Option<&str>) -> Option<String> {
        let prefix = self.as_prefix.iter().map(String::as_str);
        let parts: Vec<&str> = match &self.level {
            Level::Plain => prefix.chain(leaf).collect(),
            Level::Resources(res) => prefix.chain([res.member.as_str()]).chain(leaf).collect(),
            Level::Collection(res) => leaf
                .into_iter()
                .chain(prefix)
                .chain([res.collection.as_str()])
                .collect(),
            Level::Member(res) => leaf
                .into_iter()
                .chain(prefix)
                .chain([res.member.as_str()])
                .collect(),
            Level::New(res) => leaf
                .into_iter()
                .chain(["new"])
                .chain(prefix)
                .chain([res.member.as_str()])
                .collect(),
        };
        let name = parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("_");
        (!name.is_empty()).then_some(name)
    }

    fn is_resource_method_scope(&self) -> bool {
        matches!(
            self.level,
            Level::Member(_) | Level::Collection(_) | Level::New(_)
        )
    }
}

pub fn load_routes(root: &Path, registry: &mut TypeRegistry) {
    let routes_path = root.join("config").join("routes.rb");
    let Ok(source) = std::fs::read_to_string(&routes_path) else {
        return;
    };
    for (name, required_args) in parse_route_helpers(&source) {
        register_route_helper(registry, &name, required_args);
    }
}

fn parse_route_helpers(source: &str) -> Helpers {
    let parse_result = ruby_prism::parse(source.as_bytes());
    let root = parse_result.node();
    let mut walker = RouteWalker::default();
    if let Some(program) = root.as_program_node() {
        for node in program.statements().body().iter() {
            walker.walk(&node, &Scope::default());
        }
    }
    walker.helpers
}

#[derive(Default)]
struct RouteWalker<'pr> {
    concerns: HashMap<String, Rc<Node<'pr>>>,
    active_concerns: Vec<String>,
    helpers: Helpers,
}

impl<'pr> RouteWalker<'pr> {
    fn walk(&mut self, node: &Node<'pr>, scope: &Scope) {
        let Some(call) = node.as_call_node() else {
            return;
        };
        match call.name().as_slice() {
            b"namespace" => self.namespace(&call, &scope.nested_if_resource()),
            b"scope" => self.scope(&call, scope),
            b"shallow" => {
                let mut nested = scope.clone();
                nested.shallow = true;
                self.walk_block(call.block().as_ref(), &nested);
            }
            b"resources" => self.resources(&call, &scope.nested_if_resource(), false),
            b"resource" => self.resources(&call, &scope.nested_if_resource(), true),
            b"member" | b"collection" | b"new" if scope.resource().is_some() => {
                if let Some(child) = method_scope(scope, call.name().as_slice()) {
                    self.walk_block(call.block().as_ref(), &child);
                }
            }
            b"root" => {
                let name = scope.name_for(Some("root"));
                self.insert(name, scope.args);
            }
            b"direct" => self.direct(&call),
            b"concern" => {
                if let (Some(name), Some(block)) = (first_name(&call), call.block()) {
                    self.concerns.insert(name, Rc::new(block));
                }
            }
            b"concerns" => {
                for name in positional_names(&call) {
                    self.apply_concern(&name, scope);
                }
            }
            name if VERBS.iter().any(|verb| verb.as_bytes() == name) => self.verb(&call, scope),
            _ => self.walk_block(call.block().as_ref(), scope),
        }
    }

    fn walk_block(&mut self, block: Option<&Node<'pr>>, scope: &Scope) {
        let Some(body) = block
            .and_then(Node::as_block_node)
            .and_then(|block| block.body())
        else {
            return;
        };
        if let Some(statements) = body.as_statements_node() {
            for stmt in statements.body().iter() {
                self.walk(&stmt, scope);
            }
        }
    }

    fn insert(&mut self, name: Option<String>, required_args: usize) {
        let Some(name) = name else {
            return;
        };
        self.helpers
            .entry(name)
            .and_modify(|current| *current = (*current).max(required_args))
            .or_insert(required_args);
    }

    fn namespace(&mut self, call: &CallNode<'pr>, scope: &Scope) {
        let mut nested = scope.clone();
        if let Some(name) = first_name(call) {
            let as_name = option_string(call, "as").unwrap_or_else(|| name.clone());
            let path = option_string(call, "path").unwrap_or(name);
            let segments = dynamic_segment_count(&path);
            nested.as_prefix.push(as_name.clone());
            nested.shallow_prefix.push(as_name);
            nested.args += segments;
            nested.shallow_args += segments;
        }
        self.walk_block(call.block().as_ref(), &nested);
    }

    fn scope(&mut self, call: &CallNode<'pr>, scope: &Scope) {
        let mut nested = scope.clone();
        if let Some(as_name) = option_string(call, "as") {
            nested.as_prefix.push(as_name);
        }
        if let Some(prefix) = option_string(call, "shallow_prefix") {
            nested.shallow_prefix.push(prefix);
        }
        if let Some(path) = first_name(call).or_else(|| option_string(call, "path")) {
            nested.args += dynamic_segment_count(&path);
        }
        if let Some(path) = option_string(call, "shallow_path") {
            nested.shallow_args += dynamic_segment_count(&path);
        }
        if option_is_true(call, "shallow") {
            nested.shallow = true;
        }
        if let Some(filter) = action_filter(call) {
            nested.action_filter = Some(filter);
        }
        self.walk_block(call.block().as_ref(), &nested);
    }

    fn resources(&mut self, call: &CallNode<'pr>, scope: &Scope, singular: bool) {
        let mut scope = scope.clone();
        if option_is_true(call, "shallow") {
            scope.shallow = true;
        }
        let actions = action_filter(call)
            .or(scope.action_filter)
            .unwrap_or(Actions::ALL);
        let as_name = option_string(call, "as");
        let concerns = option_strings(call, "concerns").unwrap_or_default();
        for name in positional_names(call) {
            let name = as_name.clone().unwrap_or(name);
            let res = Rc::new(Resource {
                member: if singular {
                    name.clone()
                } else {
                    singularize(&name)
                },
                collection: name,
                singular,
                shallow: scope.shallow && !singular,
            });
            let resource_scope = scope.with_level(Level::Resources(Rc::clone(&res)));
            self.canonical_routes(&resource_scope, &res, actions);
            self.walk_block(call.block().as_ref(), &resource_scope);
            for concern in &concerns {
                self.apply_concern(concern, &resource_scope);
            }
        }
    }

    fn canonical_routes(&mut self, scope: &Scope, res: &Rc<Resource>, actions: Actions) {
        let collection_actions = if res.singular {
            Actions::CREATE
        } else {
            Actions::INDEX | Actions::CREATE
        };
        if actions.any(collection_actions) {
            let name = scope
                .with_level(Level::Collection(Rc::clone(res)))
                .name_for(None);
            self.insert(name, scope.args);
        }
        if actions.any(Actions::NEW) {
            let name = scope.with_level(Level::New(Rc::clone(res))).name_for(None);
            self.insert(name, scope.args);
        }
        let member = scope.member_scope(res);
        if actions.any(Actions::SHOW | Actions::UPDATE | Actions::DESTROY) {
            self.insert(member.name_for(None), member.args);
        }
        if actions.any(Actions::EDIT) {
            self.insert(member.name_for(Some("edit")), member.args);
        }
    }

    /// `Mapper#match`: `on:` picks the resource sub-scope; otherwise a route in a
    /// plural resource is nested and one in a singular resource is a member.
    fn verb(&mut self, call: &CallNode<'pr>, scope: &Scope) {
        let target = match (option_string(call, "on"), scope.resource()) {
            (Some(on), Some(_)) => method_scope(scope, on.as_bytes()),
            (None, Some(res)) if res.singular => Some(scope.member_scope(res)),
            (None, Some(res)) => Some(scope.nested_scope(res)),
            (_, None) => Some(scope.clone()),
        };
        if let Some(target) = target {
            self.map_route(call, &target);
        }
    }

    fn map_route(&mut self, call: &CallNode<'pr>, scope: &Scope) {
        if option_is_false(call, "as") {
            return;
        }
        let path = route_path(call);
        let leaf = option_string(call, "as").or_else(|| {
            let action = path.as_deref().filter(|path| is_action_path(path))?;
            let canonical = scope.is_resource_method_scope() && Actions::bit(action) != 0;
            (!canonical).then(|| action.to_string())
        });
        let leaf = leaf.map(|leaf| normalize_name(&leaf));
        if leaf.is_none() && !scope.is_resource_method_scope() {
            return;
        }
        let args = scope.args + path.as_deref().map_or(0, dynamic_segment_count);
        let name = scope.name_for(leaf.as_deref().filter(|leaf| !leaf.is_empty()));
        self.insert(name, args);
    }

    /// `direct` helpers ignore the surrounding scope; their arity is the block's.
    fn direct(&mut self, call: &CallNode<'pr>) {
        let Some(name) = first_name(call) else {
            return;
        };
        let required = call
            .block()
            .as_ref()
            .and_then(Node::as_block_node)
            .and_then(|block| block.parameters())
            .as_ref()
            .and_then(Node::as_block_parameters_node)
            .and_then(|params| params.parameters())
            .map_or(0, |params| params.requireds().iter().count());
        self.insert(Some(name), required);
    }

    fn apply_concern(&mut self, name: &str, scope: &Scope) {
        // A concern that (transitively) uses itself would recurse forever.
        if self.active_concerns.iter().any(|active| active == name) {
            return;
        }
        let Some(block) = self.concerns.get(name).map(Rc::clone) else {
            return;
        };
        self.active_concerns.push(name.to_string());
        self.walk_block(Some(&block), scope);
        self.active_concerns.pop();
    }
}

fn method_scope(scope: &Scope, kind: &[u8]) -> Option<Scope> {
    let res = scope.resource()?;
    match kind {
        b"member" => Some(scope.member_scope(res)),
        b"collection" => Some(scope.with_level(Level::Collection(Rc::clone(res)))),
        b"new" => Some(scope.with_level(Level::New(Rc::clone(res)))),
        _ => None,
    }
}

/// A path that doubles as the action name (`Mapper#add_route`).
fn is_action_path(path: &str) -> bool {
    !path.is_empty()
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'/'))
}

/// `Mapper.normalize_name`: `"admin/report-list"` -> `"admin_report_list"`.
fn normalize_name(name: &str) -> String {
    name.trim_matches('/').replace(['-', '/'], "_")
}

fn dynamic_segment_count(path: &str) -> usize {
    path.split('/')
        .filter(|segment| segment.starts_with(':') || segment.starts_with('*'))
        .count()
}

fn arguments<'pr>(call: &CallNode<'pr>) -> impl Iterator<Item = Node<'pr>> {
    call.arguments()
        .into_iter()
        .flat_map(|args| args.arguments().iter())
}

fn first_name(call: &CallNode<'_>) -> Option<String> {
    arguments(call).next().as_ref().and_then(literal_string)
}

fn positional_names(call: &CallNode<'_>) -> Vec<String> {
    arguments(call)
        .map_while(|arg| literal_strings(&arg))
        .flatten()
        .collect()
}

/// The route path: the first positional argument, or the key of the
/// `get "path" => "controller#action"` shorthand.
fn route_path(call: &CallNode<'_>) -> Option<String> {
    let first = arguments(call).next()?;
    if let Some(path) = literal_string(&first) {
        return Some(path);
    }
    hash_elements(&first)?.into_iter().find_map(|elem| {
        elem.as_assoc_node()
            .and_then(|assoc| literal_string(&assoc.key()))
    })
}

fn hash_elements<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    if let Some(hash) = node.as_keyword_hash_node() {
        return Some(hash.elements().iter().collect());
    }
    node.as_hash_node()
        .map(|hash| hash.elements().iter().collect())
}

fn option<'pr>(call: &CallNode<'pr>, key: &str) -> Option<Node<'pr>> {
    arguments(call)
        .filter_map(|arg| hash_elements(&arg))
        .flatten()
        .filter_map(|elem| elem.as_assoc_node())
        .find(|assoc| {
            assoc
                .key()
                .as_symbol_node()
                .is_some_and(|sym| sym.unescaped() == key.as_bytes())
        })
        .map(|assoc| assoc.value())
}

fn option_string(call: &CallNode<'_>, key: &str) -> Option<String> {
    option(call, key).as_ref().and_then(literal_string)
}

fn option_strings(call: &CallNode<'_>, key: &str) -> Option<Vec<String>> {
    option(call, key).as_ref().and_then(literal_strings)
}

fn option_is_true(call: &CallNode<'_>, key: &str) -> bool {
    option(call, key).is_some_and(|value| value.as_true_node().is_some())
}

fn option_is_false(call: &CallNode<'_>, key: &str) -> bool {
    option(call, key)
        .is_some_and(|value| value.as_false_node().is_some() || value.as_nil_node().is_some())
}

fn action_filter(call: &CallNode<'_>) -> Option<Actions> {
    let only = option_strings(call, "only");
    let except = option_strings(call, "except");
    Actions::from_options(only.as_deref(), except.as_deref())
}

fn literal_string(node: &Node<'_>) -> Option<String> {
    if let Some(string) = node.as_string_node() {
        return Some(String::from_utf8_lossy(string.unescaped()).into_owned());
    }
    node.as_symbol_node()
        .map(|sym| String::from_utf8_lossy(sym.unescaped()).into_owned())
}

/// A string/symbol literal or an array of them (`:a`, `[:a, :b]`, `%i[a b]`).
fn literal_strings(node: &Node<'_>) -> Option<Vec<String>> {
    if let Some(array) = node.as_array_node() {
        return array
            .elements()
            .iter()
            .map(|elem| literal_string(&elem))
            .collect();
    }
    literal_string(node).map(|name| vec![name])
}

fn register_route_helper(registry: &mut TypeRegistry, name: &str, required_args: usize) {
    registry.add_mixin("Object", ROUTE_HELPERS_MODULE, MixinKind::Include);
    for suffix in ["path", "url"] {
        registry.add_method_def_if_missing(
            ROUTE_HELPERS_MODULE,
            build_route_helper_method(name, suffix, required_args),
        );
    }
}

fn build_route_helper_method(name: &str, suffix: &str, required_args: usize) -> MethodDef {
    MethodDef {
        name: Sym::new(format!("{name}_{suffix}")),
        param_infos: (0..required_args)
            .map(|idx| ParamInfo {
                name: format!("arg{idx}"),
                kind: ParamKind::Required,
                default_type: Some(Type::Untyped),
            })
            .collect(),
        raw_return_type: Type::String,
        sorbet_modifier_comments: Vec::new(),
        rbs_annotated: true,
        rbs_inline_annotated: false,
        sig_annotated: false,
        attr_ivar: None,
        is_singleton: false,
        rbs_file_source: true,
        synthetic_dsl_source: true,
        rbs_method_types: Default::default(),
        extra_overloads: Vec::new(),
        loc: None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_route_helpers;

    fn helpers(source: &str) -> Vec<(String, usize)> {
        parse_route_helpers(source).into_iter().collect()
    }

    fn pairs(expected: &[(&str, usize)]) -> Vec<(String, usize)> {
        let mut pairs: Vec<_> = expected
            .iter()
            .map(|(name, args)| ((*name).to_string(), *args))
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn only_and_except_drop_excluded_helpers() {
        assert_eq!(
            helpers(
                "resources :posts, only: [:index, :show]\nresources :tags, except: %i[show update destroy edit]"
            ),
            pairs(&[("post", 1), ("posts", 0), ("new_tag", 0), ("tags", 0)])
        );
    }

    #[test]
    fn scope_only_applies_to_resources_inside() {
        assert_eq!(
            helpers("scope only: :index do\n  resources :posts\nend"),
            pairs(&[("posts", 0)])
        );
    }

    #[test]
    fn shallow_members_drop_the_parent_prefix() {
        assert_eq!(
            helpers(
                "namespace :admin do\n  resources :articles, shallow: true do\n    resources :comments, only: [:index, :show] do\n      resources :replies, only: :index\n    end\n  end\nend"
            ),
            pairs(&[
                ("admin_article", 1),
                ("admin_article_comments", 1),
                ("admin_articles", 0),
                ("admin_comment", 1),
                ("admin_comment_replies", 1),
                ("edit_admin_article", 1),
                ("new_admin_article", 0),
            ])
        );
    }

    #[test]
    fn concerns_expand_in_the_using_scope() {
        assert_eq!(
            helpers(
                "concern :commentable do\n  resources :notes, only: :index\n  concerns :commentable\nend\nresources :photos, only: [], concerns: :commentable\nresources :posts, only: [] do\n  concerns :commentable\nend"
            ),
            pairs(&[("photo_notes", 1), ("post_notes", 1)])
        );
    }

    #[test]
    fn verb_routes_follow_the_resource_scope_level() {
        assert_eq!(
            helpers(
                "resources :users, only: [] do\n  get :preview\n  get :card, on: :member\n  get :search, on: :collection\n  new do\n    get :draft\n  end\nend\nresource :profile, only: [] do\n  get :avatar\nend\nget \"users/:id/profile\" => \"profiles#show\"\nget \"about-us\", as: nil\nget \"admin/stats\""
            ),
            pairs(&[
                ("admin_stats", 0),
                ("avatar_profile", 0),
                ("card_user", 1),
                ("draft_new_user", 0),
                ("search_users", 0),
                ("user_preview", 1),
            ])
        );
    }

    #[test]
    fn resources_as_and_direct_helpers() {
        assert_eq!(
            helpers(
                "resources :events, as: :happenings, only: [:index, :show]\ndirect(:homepage) { \"https://example.com\" }\ndirect :item do |model|\n  model\nend"
            ),
            pairs(&[
                ("happening", 1),
                ("happenings", 0),
                ("homepage", 0),
                ("item", 1)
            ])
        );
    }
}

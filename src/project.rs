use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RubyVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl RubyVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub const fn latest_stable() -> Self {
        Self::new(3, 4, 0)
    }

    pub fn parse(input: &str) -> Option<Self> {
        parse_version(input).map(|(major, minor, patch)| Self::new(major, minor, patch))
    }

    pub fn major_minor_string(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    pub fn full_string(self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RailsVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl RailsVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub const fn latest_stable() -> Self {
        Self::new(8, 0, 0)
    }

    pub fn parse(input: &str) -> Option<Self> {
        parse_version(input).map(|(major, minor, patch)| Self::new(major, minor, patch))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProjectVersions {
    pub ruby: Option<RubyVersion>,
    pub rails: Option<RailsVersion>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum DslLibrary {
    Aasm,
    ActionControllerHelpers,
    ActiveModelSerializers,
    ActiveModelValidations,
    ActiveRecordMigration,
    ActiveRecordPersistence,
    DeclarativePolicy,
    GitlabPresenter,
    Grape,
    GraphqlSchema,
    RailsConfigure,
    ActionMailer,
    ActionText,
    ActiveJob,
    ActiveHash,
    ActiveModelAttributes,
    ActiveModelSecurePassword,
    ActiveModelValidationsConfirmation,
    Draper,
    ActiveRecordAssociations,
    ActiveRecordColumns,
    ActiveRecordDelegatedTypes,
    ActiveRecordEnum,
    ActiveRecordFixtures,
    ActiveRecordRelations,
    ActiveRecordScope,
    ActiveRecordSecureToken,
    ActiveRecordStore,
    ActiveRecordTypedStore,
    ActiveResource,
    ActiveStorage,
    ActiveSupportConcern,
    ActiveSupportCurrentAttributes,
    ActiveSupportEnvironmentInquirer,
    ActiveSupportTimeExt,
    Config,
    Devise,
    Discard,
    Doorkeeper,
    FrozenRecord,
    Gettext,
    GrapeEntity,
    GraphqlInputObject,
    GraphqlMutation,
    IdentityCache,
    JsonApiClientResource,
    Kredis,
    MixedInClassAttributes,
    Oj,
    Protobuf,
    RailsGenerators,
    SidekiqWorker,
    Shrine,
    SmartProperties,
    StateMachines,
    UrlHelpers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DslActivationSource {
    AutoDetected,
    ForcedOn,
    ForcedOff,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DslFamily {
    Rails,
    Ruby,
    Gem,
}

/// One `DslLibrary` flag owned by a plugin, plus the Gemfile markers that enable it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DslFeature {
    pub library: DslLibrary,
    pub gem_markers: &'static [&'static str],
}

/// Detection / CLI identity for a builtin plugin. Fine-grained gates stay on `DslLibrary`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginManifest {
    pub id: &'static str,
    pub features: &'static [DslFeature],
    pub base_classes: &'static [&'static str],
    pub rails_default: bool,
}

impl PluginManifest {
    pub fn enable_from_gems(self, gems: &BTreeSet<String>, out: &mut BTreeSet<DslLibrary>) {
        for feature in self.features {
            if feature
                .gem_markers
                .iter()
                .any(|marker| gems.contains(*marker))
            {
                out.insert(feature.library);
            }
        }
    }

    pub fn enable_rails_defaults(self, out: &mut BTreeSet<DslLibrary>) {
        if !self.rails_default {
            return;
        }
        for feature in self.features {
            out.insert(feature.library);
        }
    }

    pub fn id_matches(self, name: &str) -> bool {
        let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
        self.id == normalized
    }
}

const fn dsl_feature(library: DslLibrary, gem_markers: &'static [&'static str]) -> DslFeature {
    DslFeature {
        library,
        gem_markers,
    }
}

/// Gem-only libraries that have no plugin file.
const ORPHAN_GEM_FEATURES: &[DslFeature] =
    &[dsl_feature(DslLibrary::FrozenRecord, &["frozen_record"])];

impl DslLibrary {
    pub const fn cli_name(self) -> &'static str {
        match self {
            Self::Aasm => "aasm",
            Self::ActionControllerHelpers => "action_controller_helpers",
            Self::ActiveModelSerializers => "active_model_serializers",
            Self::ActiveModelValidations => "active_model_validations",
            Self::ActiveRecordMigration => "active_record_migration",
            Self::ActiveRecordPersistence => "active_record_persistence",
            Self::DeclarativePolicy => "declarative_policy",
            Self::GitlabPresenter => "gitlab_presenter",
            Self::Grape => "grape",
            Self::GraphqlSchema => "graphql_schema",
            Self::RailsConfigure => "rails_configure",
            Self::ActionMailer => "action_mailer",
            Self::ActionText => "action_text",
            Self::ActiveJob => "active_job",
            Self::ActiveHash => "active_hash",
            Self::ActiveModelAttributes => "active_model_attributes",
            Self::ActiveModelSecurePassword => "active_model_secure_password",
            Self::ActiveModelValidationsConfirmation => "active_model_validations_confirmation",
            Self::Draper => "draper",
            Self::ActiveRecordAssociations => "active_record_associations",
            Self::ActiveRecordColumns => "active_record_columns",
            Self::ActiveRecordDelegatedTypes => "active_record_delegated_types",
            Self::ActiveRecordEnum => "active_record_enum",
            Self::ActiveRecordFixtures => "active_record_fixtures",
            Self::ActiveRecordRelations => "active_record_relations",
            Self::ActiveRecordScope => "active_record_scope",
            Self::ActiveRecordSecureToken => "active_record_secure_token",
            Self::ActiveRecordStore => "active_record_store",
            Self::ActiveRecordTypedStore => "active_record_typed_store",
            Self::ActiveResource => "active_resource",
            Self::ActiveStorage => "active_storage",
            Self::ActiveSupportConcern => "active_support_concern",
            Self::ActiveSupportCurrentAttributes => "active_support_current_attributes",
            Self::ActiveSupportEnvironmentInquirer => "active_support_environment_inquirer",
            Self::ActiveSupportTimeExt => "active_support_time_ext",
            Self::Config => "config",
            Self::Devise => "devise",
            Self::Discard => "discard",
            Self::Doorkeeper => "doorkeeper",
            Self::FrozenRecord => "frozen_record",
            Self::Gettext => "gettext",
            Self::GrapeEntity => "grape_entity",
            Self::GraphqlInputObject => "graphql_input_object",
            Self::GraphqlMutation => "graphql_mutation",
            Self::IdentityCache => "identity_cache",
            Self::JsonApiClientResource => "json_api_client_resource",
            Self::Kredis => "kredis",
            Self::MixedInClassAttributes => "mixed_in_class_attributes",
            Self::Oj => "oj",
            Self::Protobuf => "protobuf",
            Self::RailsGenerators => "rails_generators",
            Self::SidekiqWorker => "sidekiq_worker",
            Self::Shrine => "shrine",
            Self::SmartProperties => "smart_properties",
            Self::StateMachines => "state_machines",
            Self::UrlHelpers => "url_helpers",
        }
    }

    pub const fn official_builtins() -> &'static [Self] {
        &[
            Self::Aasm,
            Self::ActionControllerHelpers,
            Self::ActiveModelSerializers,
            Self::ActiveModelValidations,
            Self::ActiveRecordMigration,
            Self::ActiveRecordPersistence,
            Self::DeclarativePolicy,
            Self::GitlabPresenter,
            Self::Grape,
            Self::GraphqlSchema,
            Self::RailsConfigure,
            Self::ActionMailer,
            Self::ActionText,
            Self::ActiveJob,
            Self::ActiveHash,
            Self::ActiveModelAttributes,
            Self::ActiveModelSecurePassword,
            Self::ActiveModelValidationsConfirmation,
            Self::Draper,
            Self::ActiveRecordAssociations,
            Self::ActiveRecordColumns,
            Self::ActiveRecordDelegatedTypes,
            Self::ActiveRecordEnum,
            Self::ActiveRecordFixtures,
            Self::ActiveRecordRelations,
            Self::ActiveRecordScope,
            Self::ActiveRecordSecureToken,
            Self::ActiveRecordStore,
            Self::ActiveRecordTypedStore,
            Self::ActiveResource,
            Self::ActiveStorage,
            Self::ActiveSupportConcern,
            Self::ActiveSupportCurrentAttributes,
            Self::ActiveSupportEnvironmentInquirer,
            Self::ActiveSupportTimeExt,
            Self::Config,
            Self::Devise,
            Self::Discard,
            Self::Doorkeeper,
            Self::FrozenRecord,
            Self::Gettext,
            Self::GrapeEntity,
            Self::GraphqlInputObject,
            Self::GraphqlMutation,
            Self::IdentityCache,
            Self::JsonApiClientResource,
            Self::Kredis,
            Self::MixedInClassAttributes,
            Self::Oj,
            Self::Protobuf,
            Self::RailsGenerators,
            Self::SidekiqWorker,
            Self::Shrine,
            Self::SmartProperties,
            Self::StateMachines,
            Self::UrlHelpers,
        ]
    }

    pub fn rails_defaults() -> BTreeSet<Self> {
        let mut detected = BTreeSet::new();
        for manifest in crate::inference::builtin_plugin_manifests() {
            manifest.enable_rails_defaults(&mut detected);
        }
        detected
    }

    pub const fn is_rails_family(self) -> bool {
        matches!(
            self,
            Self::ActionControllerHelpers
                | Self::ActiveModelValidations
                | Self::ActiveRecordMigration
                | Self::ActiveRecordPersistence
                | Self::RailsConfigure
                | Self::ActionMailer
                | Self::ActionText
                | Self::ActiveJob
                | Self::ActiveModelAttributes
                | Self::ActiveModelSecurePassword
                | Self::ActiveModelValidationsConfirmation
                | Self::ActiveRecordAssociations
                | Self::ActiveRecordColumns
                | Self::ActiveRecordDelegatedTypes
                | Self::ActiveRecordEnum
                | Self::ActiveRecordFixtures
                | Self::ActiveRecordRelations
                | Self::ActiveRecordScope
                | Self::ActiveRecordSecureToken
                | Self::ActiveRecordStore
                | Self::ActiveRecordTypedStore
                | Self::ActiveResource
                | Self::ActiveStorage
                | Self::ActiveSupportConcern
                | Self::ActiveSupportCurrentAttributes
                | Self::ActiveSupportEnvironmentInquirer
                | Self::ActiveSupportTimeExt
                | Self::MixedInClassAttributes
                | Self::RailsGenerators
                | Self::UrlHelpers
        )
    }

    pub fn from_cli_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
        Self::official_builtins()
            .iter()
            .copied()
            .find(|dsl| dsl.cli_name() == normalized)
    }

    pub const fn family(self) -> DslFamily {
        if self.is_rails_family() {
            DslFamily::Rails
        } else {
            match self {
                Self::Aasm
                | Self::Config
                | Self::Devise
                | Self::Discard
                | Self::Draper
                | Self::FrozenRecord
                | Self::GraphqlInputObject
                | Self::GraphqlMutation
                | Self::IdentityCache
                | Self::JsonApiClientResource
                | Self::Kredis
                | Self::Oj
                | Self::Protobuf
                | Self::Shrine
                | Self::SidekiqWorker
                | Self::SmartProperties
                | Self::StateMachines => DslFamily::Gem,
                Self::ActiveHash | Self::MixedInClassAttributes => DslFamily::Ruby,
                _ => DslFamily::Gem,
            }
        }
    }

    pub fn gem_markers(self) -> &'static [&'static str] {
        for manifest in crate::inference::builtin_plugin_manifests() {
            for feature in manifest.features {
                if feature.library == self {
                    return feature.gem_markers;
                }
            }
        }
        for feature in ORPHAN_GEM_FEATURES {
            if feature.library == self {
                return feature.gem_markers;
            }
        }
        &[]
    }

    pub fn detected_from_gems(self, gems: &BTreeSet<String>) -> bool {
        self.gem_markers()
            .iter()
            .any(|marker| gems.contains(*marker))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DslActivation {
    pub auto_detected: BTreeSet<DslLibrary>,
    pub forced_on: BTreeSet<DslLibrary>,
    pub forced_off: BTreeSet<DslLibrary>,
}

impl DslActivation {
    pub fn enabled_libraries(&self) -> BTreeSet<DslLibrary> {
        let mut enabled = self.auto_detected.clone();
        enabled.extend(self.forced_on.iter().copied());
        for disabled in &self.forced_off {
            enabled.remove(disabled);
        }
        enabled
    }

    pub fn is_enabled(&self, library: DslLibrary) -> bool {
        if self.forced_off.contains(&library) {
            return false;
        }
        if self.forced_on.contains(&library) {
            return true;
        }
        self.auto_detected.contains(&library)
    }

    pub fn rails_mode_compat(&self) -> bool {
        self.enabled_libraries()
            .into_iter()
            .any(DslLibrary::is_rails_family)
    }

    pub fn with_auto_detected(auto_detected: BTreeSet<DslLibrary>) -> Self {
        Self {
            auto_detected,
            forced_on: BTreeSet::new(),
            forced_off: BTreeSet::new(),
        }
    }

    pub fn with_rails_mode(enabled: bool) -> Self {
        let mut auto_detected = BTreeSet::new();
        if enabled {
            auto_detected.extend(DslLibrary::rails_defaults().iter().copied());
        }
        Self::with_auto_detected(auto_detected)
    }

    pub fn apply_cli_spec(&mut self, spec: &str) {
        for token in spec
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
        {
            if token.eq_ignore_ascii_case("auto") {
                continue;
            }
            let (sign, body) = match token.as_bytes().first().copied() {
                Some(b'+') => ('+', &token[1..]),
                Some(b'-') => ('-', &token[1..]),
                _ => ('+', token),
            };
            let libraries = resolve_cli_dsl_token(body);
            match sign {
                '+' => {
                    for library in libraries {
                        self.forced_off.remove(&library);
                        self.forced_on.insert(library);
                    }
                }
                '-' => {
                    for library in libraries {
                        self.forced_on.remove(&library);
                        self.forced_off.insert(library);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn activation_source(&self, library: DslLibrary) -> DslActivationSource {
        if self.forced_off.contains(&library) {
            DslActivationSource::ForcedOff
        } else if self.forced_on.contains(&library) {
            DslActivationSource::ForcedOn
        } else if self.auto_detected.contains(&library) {
            DslActivationSource::AutoDetected
        } else {
            DslActivationSource::Disabled
        }
    }
}

impl ProjectVersions {
    pub fn detect(root: &Path) -> Self {
        Self {
            ruby: detect_ruby_version(root),
            rails: detect_rails_version(root),
        }
    }

    pub fn effective_ruby(self) -> RubyVersion {
        self.ruby.unwrap_or_else(RubyVersion::latest_stable)
    }

    pub fn effective_rails(self) -> RailsVersion {
        self.rails.unwrap_or_else(RailsVersion::latest_stable)
    }
}

pub fn detect_ruby_version(root: &Path) -> Option<RubyVersion> {
    detect_ruby_version_from_dot_ruby_version(root)
        .or_else(|| detect_ruby_version_from_tool_versions(root))
        .or_else(|| detect_ruby_version_from_gemfile(root))
}

pub fn detect_rails_version(root: &Path) -> Option<RailsVersion> {
    let lockfile = root.join("Gemfile.lock");
    let content = std::fs::read_to_string(lockfile).ok()?;
    detect_rails_version_from_lockfile_content(&content)
}

pub fn detect_dsl_activation(root: &Path) -> DslActivation {
    DslActivation::with_auto_detected(detect_dsl_libraries(root))
}

pub fn detect_dsl_libraries(root: &Path) -> BTreeSet<DslLibrary> {
    let gems = detect_declared_gems(root);
    let mut detected = detect_dsl_libraries_from_gems_inner(root, &gems);
    // For Rails, gem detection + rails_defaults already covers this, so a source scan is redundant (costs hundreds of ms on large workspaces).
    if !looks_like_rails_project(root, &gems) {
        detect_dsl_libraries_from_source(root, &mut detected);
    }
    detected
}

fn resolve_cli_dsl_token(name: &str) -> BTreeSet<DslLibrary> {
    if let Some(library) = DslLibrary::from_cli_name(name) {
        return BTreeSet::from([library]);
    }
    for manifest in crate::inference::builtin_plugin_manifests() {
        if manifest.id_matches(name) {
            return manifest
                .features
                .iter()
                .map(|feature| feature.library)
                .collect();
        }
    }
    BTreeSet::new()
}

fn detect_dsl_libraries_from_gems_inner(
    root: &Path,
    gems: &BTreeSet<String>,
) -> BTreeSet<DslLibrary> {
    let mut detected = BTreeSet::new();
    let rails = looks_like_rails_project(root, gems);

    for manifest in crate::inference::builtin_plugin_manifests() {
        if rails {
            manifest.enable_rails_defaults(&mut detected);
        }
        manifest.enable_from_gems(gems, &mut detected);
    }
    for feature in ORPHAN_GEM_FEATURES {
        if feature
            .gem_markers
            .iter()
            .any(|marker| gems.contains(*marker))
        {
            detected.insert(feature.library);
        }
    }

    detected
}

pub fn detect_dsl_libraries_from_gems(root: &Path) -> BTreeSet<DslLibrary> {
    let gems = detect_declared_gems(root);
    detect_dsl_libraries_from_gems_inner(root, &gems)
}

fn detect_ruby_version_from_dot_ruby_version(root: &Path) -> Option<RubyVersion> {
    let content = std::fs::read_to_string(root.join(".ruby-version")).ok()?;
    RubyVersion::parse(content.trim())
}

fn detect_ruby_version_from_tool_versions(root: &Path) -> Option<RubyVersion> {
    let content = std::fs::read_to_string(root.join(".tool-versions")).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        if parts.next() == Some("ruby")
            && let Some(version) = parts.next()
        {
            return RubyVersion::parse(version);
        }
    }
    None
}

fn detect_ruby_version_from_gemfile(root: &Path) -> Option<RubyVersion> {
    let content = std::fs::read_to_string(root.join("Gemfile")).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || !trimmed.starts_with("ruby") {
            continue;
        }
        if let Some(version) = extract_quoted_version(trimmed) {
            return RubyVersion::parse(version);
        }
    }
    None
}

fn detect_rails_version_from_lockfile_content(content: &str) -> Option<RailsVersion> {
    for line in content.lines() {
        let trimmed = line.trim_start();
        for prefix in ["rails (", "railties ("] {
            if let Some(version) = trimmed.strip_prefix(prefix) {
                let version = version.strip_suffix(')')?;
                if let Some(parsed) = RailsVersion::parse(version) {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

pub fn detect_declared_gems(root: &Path) -> BTreeSet<String> {
    let mut gems = BTreeSet::new();
    if let Ok(content) = std::fs::read_to_string(root.join("Gemfile.lock")) {
        collect_gems_from_lockfile(&content, &mut gems);
    }
    if let Ok(content) = std::fs::read_to_string(root.join("Gemfile")) {
        collect_gems_from_gemfile(&content, &mut gems);
    }
    gems
}

/// Constants derived from the Gemfile plus framework constants, so arbitrary gems don't trigger `unresolved_constant`.
pub fn known_external_constant_namespaces(root: &Path) -> std::collections::HashSet<String> {
    let mut names: std::collections::HashSet<String> = FRAMEWORK_CONSTANT_NAMESPACES
        .iter()
        .chain(RBS_UNSHIPPED_DEFAULT_GEM_NAMESPACES)
        .map(|s| (*s).to_string())
        .collect();
    for gem in detect_declared_gems(root) {
        names.insert(gem_to_top_constant(&gem));
    }
    names
}

/// Framework constants that can't be recovered via inflection.
const FRAMEWORK_CONSTANT_NAMESPACES: &[&str] = &[
    "Rails",
    "ActiveRecord",
    "ActiveSupport",
    "ActiveModel",
    "ActiveJob",
    "ActiveStorage",
    "ActionController",
    "ActionMailer",
    "ActionMailbox",
    "ActionView",
    "ActionCable",
    "ActionDispatch",
    "ActionText",
    "ActionPack",
    "AbstractController",
    "Arel",
    "Mime",
    "I18n",
    "Gem",
    // Sorbet runtime (`T.let`, `T.must`, `T::Struct`).
    "T",
];

/// Constants for Ruby-bundled default gems whose RBS isn't shipped by the rbs gem (won't appear in Gemfile.lock).
const RBS_UNSHIPPED_DEFAULT_GEM_NAMESPACES: &[&str] = &[
    "IRB",            // irb
    "Reline",         // reline
    "Readline",       // readline (compat constant provided by reline)
    "Fcntl",          // fcntl
    "Fiddle",         // fiddle
    "WeakRef",        // weakref
    "OpenStruct",     // ostruct
    "DRb",            // drb (default gem through Ruby 3.3)
    "Rinda",          // rinda (default gem through Ruby 3.3)
    "Racc",           // racc (default gem through Ruby 3.3)
    "GetoptLong",     // getoptlong (default gem through Ruby 3.3)
    "Syslog",         // syslog (default gem through Ruby 3.3)
    "WIN32OLE",       // win32ole
    "ErrorHighlight", // error_highlight (Ruby 3.1+)
    "SyntaxSuggest",  // syntax_suggest (Ruby 3.2+)
    "Prism",          // prism (Ruby 3.3+; RBS ships with the prism gem, not the rbs gem)
    "Bundler",        // bundler
];

fn gem_to_top_constant(gem: &str) -> String {
    gem.split('-')
        .next()
        .unwrap_or(gem)
        .split('_')
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn collect_gems_from_lockfile(content: &str, gems: &mut BTreeSet<String>) {
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, _version)) = trimmed.split_once(" (")
            && name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-'))
        {
            gems.insert(name.to_string());
        }
    }
}

fn collect_gems_from_gemfile(content: &str, gems: &mut BTreeSet<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || !trimmed.starts_with("gem ") {
            continue;
        }
        if let Some(name) = extract_quoted_token(trimmed) {
            gems.insert(name.to_string());
        }
    }
}

fn looks_like_rails_project(root: &Path, gems: &BTreeSet<String>) -> bool {
    root.join("config").join("application.rb").exists()
        || gems.contains("rails")
        || gems.contains("railties")
}

fn detect_dsl_libraries_from_source(root: &Path, detected: &mut BTreeSet<DslLibrary>) {
    // Directory traversal is sequential, but read + scan run in parallel (saves ~2.6s on a 6.5k-file workspace).
    let mut paths = Vec::new();
    collect_dsl_source_paths(root, &mut paths);
    let scanned = paths
        .par_iter()
        .fold(BTreeSet::new, |mut acc, path| {
            if let Ok(source) = std::fs::read_to_string(path) {
                detect_dsl_libraries_from_source_text(&source, &mut acc);
            }
            acc
        })
        .reduce(BTreeSet::new, |mut a, b| {
            a.extend(b);
            a
        });
    detected.extend(scanned);
}

fn collect_dsl_source_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            if should_skip_dsl_detection_dir(&path) {
                continue;
            }
            collect_dsl_source_paths(&path, out);
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "rb") {
            out.push(path);
        }
    }
}

/// Scans one analysis source. Callers union the per-file results; set union is
/// commutative/associative, so file order does not matter (same pattern as
/// `detect_dsl_libraries_from_source` above).
pub fn detect_dsl_from_source_text(source: &str, detected: &mut BTreeSet<DslLibrary>) {
    detect_dsl_libraries_from_source_text(source, detected);
}

pub fn detect_realtime_dsl_from_source(source: &str, activation: &mut DslActivation) {
    // Only `ActiveSupportConcern` is kept, so scanning for the other libraries' markers is wasted work.
    if ACTIVE_SUPPORT_CONCERN_MARKERS
        .iter()
        .any(|marker| source.contains(marker))
    {
        activation
            .auto_detected
            .insert(DslLibrary::ActiveSupportConcern);
    }
}

const ACTIVE_SUPPORT_CONCERN_MARKERS: &[&str] =
    &["included do", "class_methods do", "ActiveSupport::Concern"];

fn should_skip_dsl_detection_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "vendor"
                    | "target"
                    | "node_modules"
                    | ".git"
                    | ".bundle"
                    | "tmp"
                    | "log"
                    | "coverage"
            )
        })
}

/// Every marker `detect_dsl_libraries_from_source_text` looks for, sorted for binary search.
/// A marker missing from this table falls back to a direct substring scan, so an
/// out-of-date table can only cost speed, never change what gets detected.
const DSL_MARKERS: &[&str] = &[
    "ActiveFile::Base",
    "ActiveHash::Base",
    "ActiveJSON::Base",
    "ActiveResource::Base",
    "ActiveSupport::Concern",
    "ActiveSupport::CurrentAttributes",
    "ActiveYaml::Base",
    "Config::Options",
    "Draper::Decoratable",
    "Draper::Decorator",
    "GraphQL::Schema::InputObject",
    "GraphQL::Schema::Mutation",
    "JsonApiClient::Resource",
    "Shrine::Attachment(",
    "aasm do",
    "after_action",
    "argument :",
    "around_action",
    "attribute :",
    "authenticate_user!",
    "before_action",
    "belongs_to ",
    "cache_belongs_to",
    "cache_has_many",
    "cache_index",
    "class_methods do",
    "confirmation: true",
    "current_user",
    "decorates ",
    "decorates(",
    "delegate_all",
    "devise :",
    "devise(",
    "enum ",
    "enum :",
    "extend Discard::Model::ClassMethods",
    "field :",
    "has_many ",
    "has_many_attached",
    "has_many_rich_texts",
    "has_one ",
    "has_one_attached",
    "has_rich_text",
    "has_secure_password",
    "has_secure_token",
    "helper_method",
    "include AASM",
    "include Discard::Model",
    "include Shrine::Attachment",
    "include Sidekiq::Job",
    "include Sidekiq::Worker",
    "include SmartProperties",
    "included do",
    "kredis_",
    "optional :",
    "plugin :shrine",
    "property :",
    "repeated :",
    "required :",
    "schema do",
    "scope :",
    "scope(",
    "self.data =",
    "setting :",
    "state_machine ",
    "state_machine do",
    "store ",
    "store_accessor",
    "typed_store",
    "validates_confirmation_of",
];

/// Two-byte prefixes of `DSL_MARKERS`, so one pass over a source can reject most
/// offsets with a single bitmap probe instead of running a substring search per marker.
struct DslMarkerIndex {
    prefix_bits: [u64; 1024],
    /// `(two-byte prefix, index into DSL_MARKERS)`, sorted by prefix.
    by_prefix: Vec<(u16, u8)>,
}

impl DslMarkerIndex {
    fn build() -> Self {
        let mut prefix_bits = [0u64; 1024];
        let mut by_prefix = Vec::with_capacity(DSL_MARKERS.len());
        for (index, marker) in DSL_MARKERS.iter().enumerate() {
            let Ok(index) = u8::try_from(index) else {
                continue;
            };
            let Some(prefix) = two_byte_prefix(marker.as_bytes()) else {
                continue;
            };
            if let Some(word) = prefix_bits.get_mut(usize::from(prefix >> 6)) {
                *word |= 1u64 << (prefix & 63);
            }
            by_prefix.push((prefix, index));
        }
        by_prefix.sort_unstable();
        Self {
            prefix_bits,
            by_prefix,
        }
    }

    fn may_start_marker(&self, prefix: u16) -> bool {
        self.prefix_bits
            .get(usize::from(prefix >> 6))
            .is_some_and(|word| word & (1u64 << (prefix & 63)) != 0)
    }
}

fn two_byte_prefix(bytes: &[u8]) -> Option<u16> {
    match bytes {
        [first, second, ..] => Some((u16::from(*first) << 8) | u16::from(*second)),
        _ => None,
    }
}

fn dsl_marker_index() -> &'static DslMarkerIndex {
    static INDEX: std::sync::OnceLock<DslMarkerIndex> = std::sync::OnceLock::new();
    INDEX.get_or_init(DslMarkerIndex::build)
}

/// Which of `DSL_MARKERS` occur in one source, collected in a single pass.
struct DslMarkerHits {
    found: u128,
}

impl DslMarkerHits {
    fn scan(source: &str) -> Self {
        let index = dsl_marker_index();
        let bytes = source.as_bytes();
        let mut found = 0u128;
        for (start, pair) in bytes.windows(2).enumerate() {
            let Some(prefix) = two_byte_prefix(pair) else {
                continue;
            };
            if !index.may_start_marker(prefix) {
                continue;
            }
            let Some(rest) = bytes.get(start..) else {
                continue;
            };
            let from = index.by_prefix.partition_point(|(p, _)| *p < prefix);
            for (marker_prefix, marker_index) in index.by_prefix.get(from..).unwrap_or(&[]) {
                if *marker_prefix != prefix {
                    break;
                }
                let Some(marker) = DSL_MARKERS.get(usize::from(*marker_index)) else {
                    continue;
                };
                if rest.starts_with(marker.as_bytes()) {
                    found |= 1u128 << u32::from(*marker_index);
                }
            }
        }
        Self { found }
    }

    fn contains(&self, marker: &str, source: &str) -> bool {
        match DSL_MARKERS.binary_search(&marker) {
            Ok(index) => u32::try_from(index).is_ok_and(|index| self.found & (1u128 << index) != 0),
            Err(_) => source.contains(marker),
        }
    }
}

fn detect_dsl_libraries_from_source_text(source: &str, detected: &mut BTreeSet<DslLibrary>) {
    let haystack = source;
    let hits = DslMarkerHits::scan(haystack);
    let contains_any = |patterns: &[&str]| {
        patterns
            .iter()
            .any(|pattern| hits.contains(pattern, haystack))
    };

    if contains_any(&[
        "has_secure_password",
        "validates_confirmation_of",
        "confirmation: true",
    ]) {
        detected.insert(DslLibrary::ActiveModelSecurePassword);
        detected.insert(DslLibrary::ActiveModelValidationsConfirmation);
    }
    if contains_any(&["belongs_to ", "has_many ", "has_one "]) {
        detected.insert(DslLibrary::ActiveRecordAssociations);
    }
    if contains_any(&["scope :", "scope("]) {
        detected.insert(DslLibrary::ActiveRecordScope);
    }
    if contains_any(&["enum :", "enum "]) {
        detected.insert(DslLibrary::ActiveRecordEnum);
    }
    if contains_any(&["store_accessor", "store ", "typed_store"]) {
        detected.insert(DslLibrary::ActiveRecordStore);
        if hits.contains("typed_store", haystack) {
            detected.insert(DslLibrary::ActiveRecordTypedStore);
        }
    }
    if hits.contains("has_secure_token", haystack) {
        detected.insert(DslLibrary::ActiveRecordSecureToken);
    }
    if contains_any(&["has_one_attached", "has_many_attached"]) {
        detected.insert(DslLibrary::ActiveStorage);
    }
    if contains_any(&["has_rich_text", "has_many_rich_texts"]) {
        detected.insert(DslLibrary::ActionText);
    }
    if contains_any(&[
        "helper_method",
        "before_action",
        "after_action",
        "around_action",
    ]) {
        detected.insert(DslLibrary::ActionControllerHelpers);
    }
    if contains_any(ACTIVE_SUPPORT_CONCERN_MARKERS) {
        detected.insert(DslLibrary::ActiveSupportConcern);
    }
    if contains_any(&["ActiveSupport::CurrentAttributes", "attribute :"]) {
        detected.insert(DslLibrary::ActiveSupportCurrentAttributes);
        detected.insert(DslLibrary::ActiveModelAttributes);
    }
    if contains_any(&[
        "ActiveHash::Base",
        "ActiveFile::Base",
        "ActiveYaml::Base",
        "ActiveJSON::Base",
        "self.data =",
    ]) {
        detected.insert(DslLibrary::ActiveHash);
    }
    if contains_any(&["include Sidekiq::Worker", "include Sidekiq::Job"]) {
        detected.insert(DslLibrary::SidekiqWorker);
    }
    if contains_any(&["aasm do", "include AASM"]) {
        detected.insert(DslLibrary::Aasm);
    }
    if contains_any(&["state_machine do", "state_machine "]) {
        detected.insert(DslLibrary::StateMachines);
    }
    if contains_any(&["include SmartProperties", "property :"]) {
        detected.insert(DslLibrary::SmartProperties);
    }
    if contains_any(&["GraphQL::Schema::InputObject", "argument :"]) {
        detected.insert(DslLibrary::GraphqlInputObject);
    }
    if contains_any(&["GraphQL::Schema::Mutation", "field :"]) {
        detected.insert(DslLibrary::GraphqlMutation);
    }
    if contains_any(&["optional :", "required :", "repeated :"]) {
        detected.insert(DslLibrary::Protobuf);
    }
    if contains_any(&["cache_index", "cache_has_many", "cache_belongs_to"]) {
        detected.insert(DslLibrary::IdentityCache);
    }
    if hits.contains("kredis_", haystack) {
        detected.insert(DslLibrary::Kredis);
    }
    if contains_any(&["ActiveResource::Base", "schema do"]) {
        detected.insert(DslLibrary::ActiveResource);
    }
    if contains_any(&["JsonApiClient::Resource", "property :"]) {
        detected.insert(DslLibrary::JsonApiClientResource);
    }
    if contains_any(&["Config::Options", "setting :"]) {
        detected.insert(DslLibrary::Config);
    }
    if contains_any(&["devise :", "devise(", "current_user", "authenticate_user!"]) {
        detected.insert(DslLibrary::Devise);
    }
    if contains_any(&[
        "include Discard::Model",
        "extend Discard::Model::ClassMethods",
    ]) {
        detected.insert(DslLibrary::Discard);
    }
    if contains_any(&[
        "Draper::Decoratable",
        "Draper::Decorator",
        "delegate_all",
        "decorates ",
        "decorates(",
    ]) {
        detected.insert(DslLibrary::Draper);
    }
    if contains_any(&[
        "Shrine::Attachment(",
        "include Shrine::Attachment",
        "plugin :shrine",
    ]) {
        detected.insert(DslLibrary::Shrine);
    }
}

fn extract_quoted_version(input: &str) -> Option<&str> {
    extract_between(input, '\'').or_else(|| extract_between(input, '"'))
}

fn extract_quoted_token(input: &str) -> Option<&str> {
    extract_between(input, '\'').or_else(|| extract_between(input, '"'))
}

fn extract_between(input: &str, quote: char) -> Option<&str> {
    let start = input.find(quote)?;
    let rest = &input[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

fn parse_version(input: &str) -> Option<(u16, u16, u16)> {
    let normalized = normalize_version_token(input)?;
    let mut parts = normalized.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn normalize_version_token(input: &str) -> Option<&str> {
    let token = input
        .trim()
        .trim_start_matches("ruby-")
        .trim_start_matches('v');
    let start = token.find(|c: char| c.is_ascii_digit())?;
    let token = &token[start..];
    let end = token
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(token.len());
    let version = &token[..end];
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn dsl_marker_table_is_sorted_and_scannable() {
        assert!(
            DSL_MARKERS.windows(2).all(|pair| match pair {
                [a, b] => a < b,
                _ => true,
            }),
            "DSL_MARKERS must be sorted and duplicate-free for binary search"
        );
        assert!(DSL_MARKERS.len() <= 128, "hit set is a u128");
        assert!(
            DSL_MARKERS.iter().all(|marker| marker.len() >= 2),
            "the scan dispatches on a two-byte prefix"
        );
        // The single-pass scan must agree with a plain substring search on every marker.
        for marker in DSL_MARKERS {
            let source = format!("prefix\n{marker}\nsuffix\n");
            let hits = DslMarkerHits::scan(&source);
            assert!(hits.contains(marker, &source), "missed {marker}");
            let absent = "class Foo\nend\n";
            let empty = DslMarkerHits::scan(absent);
            assert_eq!(
                empty.contains(marker, absent),
                absent.contains(*marker),
                "disagreed on {marker}"
            );
        }
    }

    #[test]
    fn realtime_dsl_detection_matches_full_detection_for_concern() {
        for source in [
            "module M\n  extend ActiveSupport::Concern\nend\n",
            "module M\n  included do\n  end\nend\n",
            "module M\n  class_methods do\n  end\nend\n",
            "class Foo\n  belongs_to :bar\nend\n",
            "",
        ] {
            let mut full = BTreeSet::new();
            detect_dsl_libraries_from_source_text(source, &mut full);
            let mut activation = DslActivation::default();
            detect_realtime_dsl_from_source(source, &mut activation);
            assert_eq!(
                activation
                    .auto_detected
                    .contains(&DslLibrary::ActiveSupportConcern),
                full.contains(&DslLibrary::ActiveSupportConcern),
                "concern detection diverged for {source:?}"
            );
            assert!(
                activation
                    .auto_detected
                    .iter()
                    .all(|library| matches!(library, DslLibrary::ActiveSupportConcern)),
                "realtime detection must only report the concern flag"
            );
        }
    }

    #[test]
    fn parses_ruby_version_tokens() {
        assert_eq!(RubyVersion::parse("3.4.1"), Some(RubyVersion::new(3, 4, 1)));
        assert_eq!(
            RubyVersion::parse("ruby-3.3.0"),
            Some(RubyVersion::new(3, 3, 0))
        );
        assert_eq!(RubyVersion::parse("3.2"), Some(RubyVersion::new(3, 2, 0)));
    }

    #[test]
    fn detects_ruby_version_from_dot_ruby_version_first() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".ruby-version"), "3.1.4\n").expect("write .ruby-version");
        std::fs::write(dir.path().join("Gemfile"), "ruby '3.3.0'\n").expect("write gemfile");

        assert_eq!(
            detect_ruby_version(dir.path()),
            Some(RubyVersion::new(3, 1, 4))
        );
    }

    #[test]
    fn detects_ruby_version_from_tool_versions() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".tool-versions"), "ruby 3.2.2\n").expect("write tool");

        assert_eq!(
            detect_ruby_version(dir.path()),
            Some(RubyVersion::new(3, 2, 2))
        );
    }

    #[test]
    fn detects_ruby_version_from_gemfile_directive() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\nruby \"3.0.6\"\n",
        )
        .expect("write gemfile");

        assert_eq!(
            detect_ruby_version(dir.path()),
            Some(RubyVersion::new(3, 0, 6))
        );
    }

    #[test]
    fn detects_rails_version_from_lockfile() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Gemfile.lock"),
            "GEM\n  specs:\n    rails (7.1.3)\n    railties (7.1.3)\n",
        )
        .expect("write lockfile");

        assert_eq!(
            detect_rails_version(dir.path()),
            Some(RailsVersion::new(7, 1, 3))
        );
    }

    #[test]
    fn known_namespaces_include_rbs_unshipped_default_gems() {
        // Ruby-bundled default gems that don't appear in Gemfile.lock (irb / reline etc.)
        // should not be treated as "undefined" even without shipped RBS.
        let dir = tempdir().expect("tempdir");
        let names = known_external_constant_namespaces(dir.path());
        for expected in ["IRB", "Reline", "Readline", "OpenStruct", "Fiddle"] {
            assert!(names.contains(expected), "missing {expected}");
        }
        // stdlib gems that the rbs gem does ship (e.g. rdoc) are out of scope for this list.
        assert!(!RBS_UNSHIPPED_DEFAULT_GEM_NAMESPACES.contains(&"RDoc"));
    }

    #[test]
    fn parses_dsl_activation_cli_spec() {
        let mut activation = DslActivation::default();
        activation.apply_cli_spec("auto,+aasm,-protobuf,+sidekiq-worker");

        assert!(activation.forced_on.contains(&DslLibrary::Aasm));
        assert!(activation.forced_on.contains(&DslLibrary::SidekiqWorker));
        assert!(activation.forced_off.contains(&DslLibrary::Protobuf));
    }

    #[test]
    fn rails_defaults_come_from_plugin_manifests() {
        let expected = BTreeSet::from([
            DslLibrary::ActionControllerHelpers,
            DslLibrary::ActiveModelValidations,
            DslLibrary::ActiveRecordMigration,
            DslLibrary::ActiveRecordPersistence,
            DslLibrary::RailsConfigure,
            DslLibrary::ActionMailer,
            DslLibrary::ActionText,
            DslLibrary::ActiveJob,
            DslLibrary::ActiveModelAttributes,
            DslLibrary::ActiveModelSecurePassword,
            DslLibrary::ActiveModelValidationsConfirmation,
            DslLibrary::ActiveRecordAssociations,
            DslLibrary::ActiveRecordColumns,
            DslLibrary::ActiveRecordDelegatedTypes,
            DslLibrary::ActiveRecordEnum,
            DslLibrary::ActiveRecordFixtures,
            DslLibrary::ActiveRecordRelations,
            DslLibrary::ActiveRecordScope,
            DslLibrary::ActiveRecordSecureToken,
            DslLibrary::ActiveRecordStore,
            DslLibrary::ActiveRecordTypedStore,
            DslLibrary::ActiveResource,
            DslLibrary::ActiveStorage,
            DslLibrary::ActiveSupportConcern,
            DslLibrary::ActiveSupportCurrentAttributes,
            DslLibrary::ActiveSupportEnvironmentInquirer,
            DslLibrary::ActiveSupportTimeExt,
            DslLibrary::MixedInClassAttributes,
            DslLibrary::RailsGenerators,
            DslLibrary::UrlHelpers,
        ]);
        assert_eq!(DslLibrary::rails_defaults(), expected);
    }

    #[test]
    fn apply_cli_spec_accepts_plugin_id() {
        let mut activation = DslActivation::default();
        activation.apply_cli_spec("+sidekiq,+graphql,-properties");

        assert!(activation.forced_on.contains(&DslLibrary::SidekiqWorker));
        assert!(activation.forced_on.contains(&DslLibrary::GraphqlSchema));
        assert!(
            activation
                .forced_on
                .contains(&DslLibrary::GraphqlInputObject)
        );
        assert!(activation.forced_on.contains(&DslLibrary::GraphqlMutation));
        assert!(activation.forced_off.contains(&DslLibrary::Config));
        assert!(activation.forced_off.contains(&DslLibrary::SmartProperties));
        assert!(
            activation
                .forced_off
                .contains(&DslLibrary::JsonApiClientResource)
        );
    }

    #[test]
    fn properties_gems_enable_only_matching_libraries() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'config'\n",
        )
        .expect("write gemfile");

        let detected = detect_dsl_libraries(dir.path());
        assert!(detected.contains(&DslLibrary::Config));
        assert!(!detected.contains(&DslLibrary::SmartProperties));
        assert!(!detected.contains(&DslLibrary::JsonApiClientResource));
    }

    #[test]
    fn detects_orphan_frozen_record_gem() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'frozen_record'\n",
        )
        .expect("write gemfile");

        let detected = detect_dsl_libraries(dir.path());
        assert!(detected.contains(&DslLibrary::FrozenRecord));
    }

    #[test]
    fn detects_dsl_libraries_from_gems_and_source() {
        // Rails project: gem detection only (`rails_defaults()` + Gemfile.lock skip the source scan).
        let dir = tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("app/models")).expect("mkdir models");
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'rails'\ngem 'aasm'\ngem 'sidekiq'\n",
        )
        .expect("write gemfile");
        std::fs::write(
            dir.path().join("app/models/user.rb"),
            "class User\n  has_secure_password\n  include Sidekiq::Worker\nend\n",
        )
        .expect("write model");

        let detected = detect_dsl_libraries(dir.path());

        assert!(detected.contains(&DslLibrary::Aasm));
        assert!(detected.contains(&DslLibrary::ActiveModelSecurePassword));
        assert!(detected.contains(&DslLibrary::SidekiqWorker));
        assert!(detected.contains(&DslLibrary::ActionMailer));
    }

    #[test]
    fn detects_dsl_libraries_from_source_only_on_non_rails_projects() {
        // Non-Rails: DSLs without a gem declaration are only detected via the source scan.
        let dir = tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("lib")).expect("mkdir lib");
        std::fs::write(
            dir.path().join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'aasm'\n",
        )
        .expect("write gemfile");
        std::fs::write(
            dir.path().join("lib/worker.rb"),
            "class Worker\n  include Sidekiq::Worker\nend\n",
        )
        .expect("write worker");

        let detected = detect_dsl_libraries(dir.path());

        assert!(detected.contains(&DslLibrary::Aasm));
        assert!(detected.contains(&DslLibrary::SidekiqWorker));
        // Rails-family DSLs must NOT be auto-enabled on a non-Rails project.
        assert!(!detected.contains(&DslLibrary::ActionMailer));
    }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use rayon::prelude::*;

use crate::analysis::analyze_rbi_declaration_source_with_lazy_rbs;
use crate::rbs::bounded_file_cache::{BoundedFileCache, DEFAULT_FILE_CACHE_CAP};
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::registry::TypeRegistry;

type RbiFileRegistrySlot = Arc<OnceLock<Arc<TypeRegistry>>>;

type SharedShapeSlot = Arc<OnceLock<Option<Arc<TypeRegistry>>>>;

#[derive(Debug, Default, Clone, Copy)]
pub struct RbiCacheBreakdown {
    pub indexed_class_count: usize,
    pub indexed_file_count: usize,
    pub pending_file_count: usize,
    pub slot_count: usize,
    pub parsed_file_count: usize,
    pub class_count: usize,
    pub method_count: usize,
}

pub struct LazyRbiLoader {
    index_state: Mutex<LazyRbiIndexState>,
    // Keying by (path,class) reparsed the file once per class it contains, blowing up LSP RSS; keyed by path only instead.
    file_cache: Mutex<BoundedFileCache<PathBuf, RbiFileRegistrySlot>>,
    shared_shapes: Mutex<HashMap<String, SharedShapeSlot>>,
    // Avoids permanently caching a transient `None` as the shape while the lazy indexer isn't fully materialized yet.
    index_materialized: OnceLock<()>,
}

impl Default for LazyRbiLoader {
    fn default() -> Self {
        Self {
            index_state: Mutex::new(LazyRbiIndexState::default()),
            file_cache: Mutex::new(BoundedFileCache::with_cap(rbi_file_cache_cap())),
            shared_shapes: Mutex::new(HashMap::new()),
            index_materialized: OnceLock::new(),
        }
    }
}

fn rbi_file_cache_cap() -> usize {
    std::env::var("TYDA_RBI_FILE_CACHE_CAP")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or(DEFAULT_FILE_CACHE_CAP)
}

#[derive(Default)]
struct LazyRbiIndexState {
    class_to_files: HashMap<String, Vec<PathBuf>>,
    file_to_classes: HashMap<PathBuf, Vec<String>>,
    pending_files: Vec<PathBuf>,
}

pub struct LazyRbiReload {
    pub affected_classes: Vec<String>,
    pub current_classes: Vec<String>,
}

impl LazyRbiLoader {
    pub fn new(paths: &[PathBuf], excluded_dirs: &[PathBuf]) -> Self {
        Self {
            index_state: Mutex::new(LazyRbiIndexState {
                class_to_files: HashMap::new(),
                file_to_classes: HashMap::new(),
                pending_files: collect_rbi_paths(paths, excluded_dirs),
            }),
            file_cache: Mutex::new(BoundedFileCache::with_cap(rbi_file_cache_cap())),
            shared_shapes: Mutex::new(HashMap::new()),
            index_materialized: OnceLock::new(),
        }
    }

    pub fn from_indexed_file_classes(file_classes: &HashMap<String, Vec<String>>) -> Self {
        let mut class_to_files: HashMap<String, Vec<PathBuf>> = HashMap::new();
        let mut file_to_classes: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for (file_path, classes) in file_classes {
            let path = PathBuf::from(file_path);
            file_to_classes.insert(path.clone(), classes.clone());
            for class_name in classes {
                class_to_files
                    .entry(class_name.clone())
                    .or_default()
                    .push(path.clone());
            }
        }

        Self {
            index_state: Mutex::new(LazyRbiIndexState {
                class_to_files,
                file_to_classes,
                pending_files: Vec::new(),
            }),
            file_cache: Mutex::new(BoundedFileCache::with_cap(rbi_file_cache_cap())),
            shared_shapes: Mutex::new(HashMap::new()),
            index_materialized: OnceLock::new(),
        }
    }

    pub fn extend(&mut self, other: Self) {
        let mut state = self.index_state.lock().expect("lazy rbi index poisoned");
        let mut other_state = other.index_state.lock().expect("lazy rbi index poisoned");
        for (class_name, files) in other_state.class_to_files.drain() {
            state
                .class_to_files
                .entry(class_name)
                .or_default()
                .extend(files);
        }
        state
            .file_to_classes
            .extend(other_state.file_to_classes.drain());
        state.pending_files.append(&mut other_state.pending_files);
        // Newly appended pending files must be re-materialized, and any shape
        // built before the index grew is stale.
        drop(state);
        drop(other_state);
        self.index_materialized = OnceLock::new();
        self.shared_shapes
            .lock()
            .expect("lazy rbi shared shapes poisoned")
            .clear();
    }

    pub fn is_empty(&self) -> bool {
        let state = self.index_state.lock().expect("lazy rbi index poisoned");
        state.class_to_files.is_empty()
            && state.file_to_classes.is_empty()
            && state.pending_files.is_empty()
    }

    pub fn drop_file_cache(&self) {
        let mut cache = self.file_cache.lock().expect("lazy rbi cache poisoned");
        cache.clear();
    }

    pub fn cache_breakdown(&self) -> RbiCacheBreakdown {
        let mut breakdown = RbiCacheBreakdown::default();
        {
            let state = self.index_state.lock().expect("lazy rbi index poisoned");
            breakdown.indexed_class_count = state.class_to_files.len();
            breakdown.indexed_file_count = state.file_to_classes.len();
            breakdown.pending_file_count = state.pending_files.len();
        }
        let cache = self.file_cache.lock().expect("lazy rbi cache poisoned");
        for slot in cache.values() {
            breakdown.slot_count += 1;
            let Some(registry) = slot.get() else {
                continue;
            };
            breakdown.parsed_file_count += 1;
            let totals = registry.breakdown_totals();
            breakdown.class_count += totals.class_count;
            breakdown.method_count += totals.method_count;
        }
        breakdown
    }

    pub fn reload_path(&self, path: &Path) -> LazyRbiReload {
        let path = path.to_path_buf();
        let old_classes = {
            let mut state = self.index_state.lock().expect("lazy rbi index poisoned");
            let old_classes = state.file_to_classes.remove(&path).unwrap_or_default();
            state.pending_files.retain(|pending| pending != &path);
            for class_name in &old_classes {
                if let Some(files) = state.class_to_files.get_mut(class_name) {
                    files.retain(|file| file != &path);
                    if files.is_empty() {
                        state.class_to_files.remove(class_name);
                    }
                }
            }
            old_classes
        };

        self.file_cache
            .lock()
            .expect("lazy rbi cache poisoned")
            .remove(&path);

        // An `.rbi` change drops the whole shared-shape cache; coarser than per-shape dependency tracking, but such changes are rare.
        self.shared_shapes
            .lock()
            .expect("lazy rbi shared shapes poisoned")
            .clear();

        let current_classes = if path.exists() && path.extension().is_some_and(|ext| ext == "rbi") {
            let classes = std::fs::read_to_string(&path)
                .ok()
                .map(|source| collect_declared_class_names_from_rbi_source(&source))
                .unwrap_or_default();
            if !classes.is_empty() {
                let mut state = self.index_state.lock().expect("lazy rbi index poisoned");
                state.file_to_classes.insert(path.clone(), classes.clone());
                for class_name in &classes {
                    let files = state.class_to_files.entry(class_name.clone()).or_default();
                    if !files.iter().any(|file| file == &path) {
                        files.push(path.clone());
                    }
                }
            }
            classes
        } else {
            Vec::new()
        };

        let mut affected = old_classes.clone();
        for class_name in &current_classes {
            if !affected.contains(class_name) {
                affected.push(class_name.clone());
            }
        }

        LazyRbiReload {
            affected_classes: affected,
            current_classes,
        }
    }

    pub fn merge_class_into(
        &self,
        class_name: &str,
        registry: &mut TypeRegistry,
        stdlib_loader: &LazyRbsLoader,
    ) -> bool {
        #[cfg(test)]
        merge_counter::record(class_name);
        let Some(shape) = self.shared_class_shape(class_name, stdlib_loader) else {
            return false;
        };
        registry.merge_rbs_class_from(&shape, class_name);
        true
    }

    /// Canonical shape shared per FQN (turns redundant per-file re-parse/merge into a single build + copy).
    fn shared_class_shape(
        &self,
        class_name: &str,
        stdlib_loader: &LazyRbsLoader,
    ) -> Option<Arc<TypeRegistry>> {
        let slot: SharedShapeSlot = {
            let mut shapes = self
                .shared_shapes
                .lock()
                .expect("lazy rbi shared shapes poisoned");
            if let Some(existing) = shapes.get(class_name) {
                Arc::clone(existing)
            } else {
                let fresh: SharedShapeSlot = Arc::new(OnceLock::new());
                shapes.insert(class_name.to_string(), Arc::clone(&fresh));
                fresh
            }
        };

        slot.get_or_init(|| self.build_class_shape(class_name, stdlib_loader))
            .clone()
    }

    /// Canonical shape build: merges all declaring `.rbi` files, done once per FQN via `OnceLock`.
    fn build_class_shape(
        &self,
        class_name: &str,
        stdlib_loader: &LazyRbsLoader,
    ) -> Option<Arc<TypeRegistry>> {
        #[cfg(test)]
        merge_counter::record_build(class_name);
        // Fully materializes the index before building the shape (prevents caching a transient `None` from the lazy indexer).
        self.index_materialized
            .get_or_init(|| self.ensure_fully_indexed());
        let files = self.lookup_class_files(class_name)?;
        let mut shape = TypeRegistry::new();
        let mut merged = false;
        for file in files {
            let file_registry = self.load_file_registry(&file, stdlib_loader);
            if file_registry.class_data_for(class_name).is_some() {
                shape.merge_rbs_class_from(&file_registry, class_name);
                merged = true;
            }
        }
        if !merged {
            return None;
        }
        // Generated mixins are excluded from RBS render (marked `external`; lookup behavior is unchanged).
        shape.mark_mixins_external();
        shape.shrink_to_fit_after_compact();
        Some(Arc::new(shape))
    }

    /// Whether any indexed `.rbi` file declares `class_name`. Drains the lazy
    /// file index as needed but does not parse or merge the declaration.
    pub fn knows_class(&self, class_name: &str) -> bool {
        self.lookup_class_files(class_name).is_some()
    }

    fn lookup_class_files(&self, class_name: &str) -> Option<Vec<PathBuf>> {
        loop {
            {
                let state = self.index_state.lock().expect("lazy rbi index poisoned");
                if let Some(files) = state.class_to_files.get(class_name) {
                    return Some(files.clone());
                }
                if state.pending_files.is_empty() {
                    return None;
                }
            }

            let next_file = {
                let mut state = self.index_state.lock().expect("lazy rbi index poisoned");
                state.pending_files.pop()
            }?;
            self.index_file(&next_file);
        }
    }

    /// Fully materializes the index, so a transient `None` from a lazy pop is never cached in the shared slot.
    fn ensure_fully_indexed(&self) {
        let pending: Vec<PathBuf> = {
            let mut state = self.index_state.lock().expect("lazy rbi index poisoned");
            std::mem::take(&mut state.pending_files)
        };
        if pending.is_empty() {
            return;
        }
        let indexed: Vec<(PathBuf, Vec<String>)> = pending
            .par_iter()
            .filter_map(|path| {
                let source = std::fs::read_to_string(path).ok()?;
                let classes = collect_declared_class_names_from_rbi_source(&source);
                if classes.is_empty() {
                    None
                } else {
                    Some((path.clone(), classes))
                }
            })
            .collect();
        let mut state = self.index_state.lock().expect("lazy rbi index poisoned");
        for (path, class_names) in indexed {
            state
                .file_to_classes
                .entry(path.clone())
                .or_insert_with(|| class_names.clone());
            for class_name in class_names {
                let files = state.class_to_files.entry(class_name).or_default();
                if !files.iter().any(|file| file == &path) {
                    files.push(path.clone());
                }
            }
        }
    }

    fn load_file_registry(
        &self,
        path: &PathBuf,
        stdlib_loader: &LazyRbsLoader,
    ) -> Arc<TypeRegistry> {
        let slot = {
            let mut cache = self.file_cache.lock().expect("lazy rbi cache poisoned");
            if let Some(existing) = cache.get(path) {
                Arc::clone(existing)
            } else {
                let fresh: RbiFileRegistrySlot = Arc::new(OnceLock::new());
                Arc::clone(cache.insert(path.clone(), fresh))
            }
        };

        Arc::clone(slot.get_or_init(|| {
            let source = std::fs::read_to_string(path).unwrap_or_default();
            let mut registry =
                analyze_rbi_declaration_source_with_lazy_rbs(&source, None, stdlib_loader);
            registry.mark_all_methods_as_external_source();
            registry.shrink_to_fit_after_compact();
            Arc::new(registry)
        }))
    }

    fn index_file(&self, path: &PathBuf) {
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        let class_names = collect_declared_class_names_from_rbi_source(&source);
        if class_names.is_empty() {
            return;
        }
        let mut state = self.index_state.lock().expect("lazy rbi index poisoned");
        state
            .file_to_classes
            .insert(path.clone(), class_names.clone());
        for class_name in class_names {
            let files = state.class_to_files.entry(class_name).or_default();
            if !files.iter().any(|file| file == path) {
                files.push(path.clone());
            }
        }
    }
}

pub fn merge_rbi_source_into_registry(
    source: &str,
    registry: &mut TypeRegistry,
    stdlib_loader: &LazyRbsLoader,
) -> Vec<String> {
    let mut rbi_registry =
        analyze_rbi_declaration_source_with_lazy_rbs(source, None, stdlib_loader);
    let classes = rbi_registry.class_names();
    rbi_registry.mark_all_methods_as_external_source();
    registry.merge_rbs_registry(&rbi_registry);
    classes
}

pub fn merge_rbi_paths_into_registry(
    paths: &[PathBuf],
    registry: &mut TypeRegistry,
    stdlib_loader: &LazyRbsLoader,
) {
    merge_rbi_paths_into_registry_excluding(paths, &[], registry, stdlib_loader);
}

pub fn merge_rbi_paths_into_registry_excluding(
    paths: &[PathBuf],
    excluded_dirs: &[PathBuf],
    registry: &mut TypeRegistry,
    stdlib_loader: &LazyRbsLoader,
) {
    for path in paths {
        merge_rbi_path_into_registry(path, excluded_dirs, registry, stdlib_loader);
    }
}

pub fn collect_rbi_file_classes(
    paths: &[PathBuf],
    _stdlib_loader: &LazyRbsLoader,
) -> HashMap<String, Vec<String>> {
    collect_rbi_file_classes_excluding(paths, &[], _stdlib_loader)
}

pub fn collect_rbi_file_classes_excluding(
    paths: &[PathBuf],
    excluded_dirs: &[PathBuf],
    _stdlib_loader: &LazyRbsLoader,
) -> HashMap<String, Vec<String>> {
    collect_rbi_paths(paths, excluded_dirs)
        .par_iter()
        .filter_map(|path| {
            let source = std::fs::read_to_string(path).ok()?;
            let classes = collect_declared_class_names_from_rbi_source(&source);
            if classes.is_empty() {
                None
            } else {
                Some((path.to_string_lossy().to_string(), classes))
            }
        })
        .collect()
}

fn merge_rbi_path_into_registry(
    path: &Path,
    excluded_dirs: &[PathBuf],
    registry: &mut TypeRegistry,
    stdlib_loader: &LazyRbsLoader,
) {
    if path.is_file() && path.extension().is_some_and(|ext| ext == "rbi") {
        if let Ok(source) = std::fs::read_to_string(path) {
            merge_rbi_source_into_registry(&source, registry, stdlib_loader);
        }
    } else if path.is_dir()
        && !should_exclude_dir(path, excluded_dirs)
        && let Ok(entries) = std::fs::read_dir(path)
    {
        for entry in entries.filter_map(|entry| entry.ok()) {
            let child = entry.path();
            if child.is_dir() && should_skip_dir(&child) {
                continue;
            }
            merge_rbi_path_into_registry(&child, excluded_dirs, registry, stdlib_loader);
        }
    }
}

fn collect_rbi_paths(paths: &[PathBuf], excluded_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        collect_rbi_paths_from_path(path, excluded_dirs, &mut files);
    }
    files
}

fn collect_rbi_paths_from_path(path: &Path, excluded_dirs: &[PathBuf], files: &mut Vec<PathBuf>) {
    if path.is_file() && path.extension().is_some_and(|ext| ext == "rbi") {
        files.push(path.to_path_buf());
    } else if path.is_dir() && !should_exclude_dir(path, excluded_dirs) {
        files.extend(walk_rbi_dir_parallel(path, excluded_dirs));
    }
}

/// Parallel walk for `.rbi` files (on large trees, a sequential walk of non-rbi subtrees dominates init time).
fn walk_rbi_dir_parallel(dir: &Path, excluded_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let (files, subdirs): (Vec<_>, Vec<_>) = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .partition(|path| path.is_file());
    let mut found: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rbi"))
        .collect();
    let nested: Vec<Vec<PathBuf>> = subdirs
        .par_iter()
        .filter(|path| !should_exclude_dir(path, excluded_dirs) && !should_skip_dir(path))
        .map(|path| walk_rbi_dir_parallel(path, excluded_dirs))
        .collect();
    for items in nested {
        found.extend(items);
    }
    found
}

fn collect_declared_class_names_from_rbi_source(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut namespace: Vec<(usize, String)> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some((kind, rest)) = trimmed
            .strip_prefix("class ")
            .map(|rest| ("class", rest))
            .or_else(|| trimmed.strip_prefix("module ").map(|rest| ("module", rest)))
        else {
            continue;
        };
        if kind == "class" && rest.starts_with("<<") {
            continue;
        }

        let indent = line.len().saturating_sub(trimmed.len());
        while namespace
            .last()
            .is_some_and(|(existing_indent, _)| *existing_indent >= indent)
        {
            namespace.pop();
        }

        let declared_name = extract_declared_constant_name(rest);
        if declared_name.is_empty() {
            continue;
        }

        let parent_names: Vec<String> = namespace.iter().map(|(_, name)| name.clone()).collect();
        let full_name = qualify_declared_constant_name(&declared_name, &parent_names);
        names.push(full_name.clone());
        namespace.push((indent, full_name));
    }

    names
}

fn extract_declared_constant_name(rest: &str) -> String {
    rest.chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':'))
        .collect()
}

fn qualify_declared_constant_name(name: &str, namespace: &[String]) -> String {
    let bare = name.trim_start_matches("::");
    if bare.contains("::") || namespace.is_empty() {
        bare.to_string()
    } else {
        format!("{}::{bare}", namespace.last().expect("namespace not empty"))
    }
}

#[cfg(test)]
fn extract_declared_constant_source(source: &str, full_name: &str) -> Option<String> {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut namespace: Vec<(usize, String)> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let Some((kind, rest)) = trimmed
            .strip_prefix("class ")
            .map(|rest| ("class", rest))
            .or_else(|| trimmed.strip_prefix("module ").map(|rest| ("module", rest)))
        else {
            continue;
        };
        if kind == "class" && rest.starts_with("<<") {
            continue;
        }

        let indent = line.len().saturating_sub(trimmed.len());
        while namespace
            .last()
            .is_some_and(|(existing_indent, _)| *existing_indent >= indent)
        {
            namespace.pop();
        }

        let declared_name = extract_declared_constant_name(rest);
        if declared_name.is_empty() {
            continue;
        }

        let parent_names: Vec<String> = namespace.iter().map(|(_, name)| name.clone()).collect();
        let qualified_name = qualify_declared_constant_name(&declared_name, &parent_names);
        if qualified_name == full_name {
            let end_idx = find_declared_constant_end(&lines, idx, indent)?;
            let body = dedent_block(&lines[idx..=end_idx], indent);
            if declared_name.starts_with("::") || declared_name.contains("::") {
                return Some(body);
            }
            return Some(wrap_in_modules(&body, &parent_names));
        }

        namespace.push((indent, qualified_name));
    }

    None
}

#[cfg(test)]
fn find_declared_constant_end(
    lines: &[&str],
    start_idx: usize,
    target_indent: usize,
) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate().skip(start_idx + 1) {
        let trimmed = line.trim_start();
        let indent = line.len().saturating_sub(trimmed.len());
        if indent == target_indent && is_standalone_end(trimmed) {
            return Some(idx);
        }
    }
    None
}

#[cfg(test)]
fn is_standalone_end(trimmed: &str) -> bool {
    trimmed == "end"
        || trimmed == "end\n"
        || trimmed.starts_with("end #")
        || trimmed.starts_with("end\n#")
}

#[cfg(test)]
fn dedent_block(lines: &[&str], indent: usize) -> String {
    let indent_prefix = " ".repeat(indent);
    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                (*line).to_string()
            } else {
                line.strip_prefix(&indent_prefix)
                    .unwrap_or(line)
                    .to_string()
            }
        })
        .collect()
}

#[cfg(test)]
fn wrap_in_modules(body: &str, namespace: &[String]) -> String {
    let mut wrapped = body.to_string();
    for parent in namespace.iter().rev() {
        let bare = parent.rsplit("::").next().unwrap_or(parent.as_str());
        wrapped = format!("module {bare}\n{}end\n", indent_block(&wrapped, 2));
    }
    wrapped
}

#[cfg(test)]
fn indent_block(source: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    source
        .split_inclusive('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("{prefix}{line}")
            }
        })
        .collect()
}

fn should_exclude_dir(path: &Path, excluded_dirs: &[PathBuf]) -> bool {
    excluded_dirs
        .iter()
        .any(|excluded| path.starts_with(excluded))
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(should_skip_dir_name)
}

fn should_skip_dir_name(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "vendor" | "target" | "node_modules" | "tmp" | "log")
}

/// Temporary instrumentation (roadmap: plan 03 step 1). Test-only.
#[cfg(test)]
mod merge_counter {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    static COUNTS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    static BUILDS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();

    fn counts() -> &'static Mutex<HashMap<String, usize>> {
        COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn builds() -> &'static Mutex<HashMap<String, usize>> {
        BUILDS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn record(class_name: &str) {
        *counts()
            .lock()
            .expect("merge counter poisoned")
            .entry(class_name.to_string())
            .or_insert(0) += 1;
    }

    pub(super) fn record_build(class_name: &str) {
        *builds()
            .lock()
            .expect("build counter poisoned")
            .entry(class_name.to_string())
            .or_insert(0) += 1;
    }

    pub(super) fn reset() {
        counts().lock().expect("merge counter poisoned").clear();
        builds().lock().expect("build counter poisoned").clear();
    }

    pub(super) fn get(class_name: &str) -> usize {
        counts()
            .lock()
            .expect("merge counter poisoned")
            .get(class_name)
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn get_builds(class_name: &str) -> usize {
        builds()
            .lock()
            .expect("build counter poisoned")
            .get(class_name)
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze_source_with_lazy_rbs;
    use crate::types::Type;
    use std::sync::{Arc, Barrier};
    use tempfile::tempdir;

    /// Conformance audit against an external `.rbi` tree (`TYDA_RBI_SWEEP_ROOT`, opt-in via `--ignored`).
    #[test]
    #[ignore]
    fn audit_external_rbi_tree() {
        let root = std::env::var("TYDA_RBI_SWEEP_ROOT")
            .expect("set TYDA_RBI_SWEEP_ROOT to the .rbi tree root");
        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let stdlib_loader = LazyRbsLoader::new(core_dir);
        let mut files = Vec::new();
        collect_rbi_files_under(std::path::Path::new(&root), &mut files);
        files.sort();
        let mut analyze_panicked = Vec::new();
        let mut zero_classes = Vec::new();
        let mut ok = 0usize;
        for path in &files {
            let Ok(source) = std::fs::read_to_string(path) else {
                continue;
            };
            // What the indexer believes the file declares; if it lists classes
            // but analysis captures none, the type surface was lost.
            let declared = collect_declared_class_names_from_rbi_source(&source);
            let rel = path.to_string_lossy().into_owned();
            let body = source.clone();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                analyze_source_with_lazy_rbs(&body, None, &stdlib_loader).class_names()
            }));
            match caught {
                Err(_) => analyze_panicked.push(rel),
                Ok(captured) => {
                    if !declared.is_empty() && captured.is_empty() {
                        zero_classes.push(rel);
                    } else {
                        ok += 1;
                    }
                }
            }
        }
        eprintln!(
            "RBI audit: {} files, {} ok, {} analyze_panicked, {} zero_classes",
            files.len(),
            ok,
            analyze_panicked.len(),
            zero_classes.len()
        );
        for f in analyze_panicked.iter().chain(&zero_classes) {
            eprintln!("  PROBLEM {f}");
        }
        assert!(
            analyze_panicked.is_empty() && zero_classes.is_empty(),
            "RBI audit found {} analyze panics, {} zero-class files",
            analyze_panicked.len(),
            zero_classes.len()
        );
    }

    fn collect_rbi_files_under(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rbi_files_under(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rbi") {
                out.push(path);
            }
        }
    }

    struct DuplicateProneLazyRbiLoader {
        class_to_files: HashMap<String, Vec<PathBuf>>,
        file_cache: Mutex<HashMap<(PathBuf, String), Arc<TypeRegistry>>>,
    }

    impl DuplicateProneLazyRbiLoader {
        fn from_indexed_file_classes(file_classes: &HashMap<String, Vec<String>>) -> Self {
            let mut class_to_files: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for (file_path, classes) in file_classes {
                let path = PathBuf::from(file_path);
                for class_name in classes {
                    class_to_files
                        .entry(class_name.clone())
                        .or_default()
                        .push(path.clone());
                }
            }
            Self {
                class_to_files,
                file_cache: Mutex::new(HashMap::new()),
            }
        }

        fn merge_class_into(
            &self,
            class_name: &str,
            registry: &mut TypeRegistry,
            stdlib_loader: &LazyRbsLoader,
        ) -> bool {
            let Some(files) = self.class_to_files.get(class_name) else {
                return false;
            };
            let mut merged = false;
            for file in files {
                let cache_key = (file.clone(), class_name.to_string());
                let file_registry = {
                    let cache = self.file_cache.lock().expect("lazy rbi cache poisoned");
                    cache.get(&cache_key).cloned()
                };
                let file_registry = match file_registry {
                    Some(registry) => registry,
                    None => {
                        let source = std::fs::read_to_string(file).unwrap_or_default();
                        let extracted = extract_declared_constant_source(&source, class_name);
                        let analysis_source = extracted.as_deref().unwrap_or(&source);
                        let mut parsed =
                            analyze_source_with_lazy_rbs(analysis_source, None, stdlib_loader);
                        parsed.mark_all_methods_as_external_source();
                        let parsed = Arc::new(parsed);
                        let mut cache = self.file_cache.lock().expect("lazy rbi cache poisoned");
                        Arc::clone(
                            cache
                                .entry(cache_key)
                                .or_insert_with(|| Arc::clone(&parsed)),
                        )
                    }
                };
                if file_registry.class_data_for(class_name).is_some() {
                    registry.merge_rbs_class_from(&file_registry, class_name);
                    merged = true;
                }
            }
            merged
        }
    }

    fn write_heavy_rbi_file(path: &Path) {
        let mut source = String::from("class User\n");
        for idx in 0..2000 {
            source.push_str("  sig { returns(String) }\n");
            source.push_str(&format!("  def field_{idx}; end\n"));
        }
        source.push_str("end\n");
        std::fs::write(path, source).expect("write rbi");
    }

    fn bench_parallel_rbi_merge(
        merge_class_into: impl Fn() + Send + Sync + 'static,
        threads: usize,
    ) -> std::time::Duration {
        let start = std::time::Instant::now();
        let barrier = Arc::new(Barrier::new(threads));
        let merge_class_into = Arc::new(merge_class_into);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                let barrier = Arc::clone(&barrier);
                let merge = Arc::clone(&merge_class_into);
                scope.spawn(move || {
                    barrier.wait();
                    merge();
                });
            }
        });
        start.elapsed()
    }

    #[test]
    fn merge_rbi_source_registers_external_methods() {
        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();

        let classes = merge_rbi_source_into_registry(
            "class User\n  sig { returns(String) }\n  def name; end\nend\n",
            &mut registry,
            &loader,
        );

        assert!(classes.contains(&"User".to_string()));
        assert_eq!(
            registry.lookup_method_return_type("User", "name"),
            Some(Type::String)
        );
    }

    #[test]
    fn rbi_empty_body_without_sig_returns_untyped() {
        // A `def attributes; end` (empty body, no sig) in tapioca-generated gem RBI is a declaration,
        // so it becomes Untyped instead of inferring nil from the empty body.
        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();

        merge_rbi_source_into_registry(
            "class Model\n  def attributes; end\nend\n",
            &mut registry,
            &loader,
        );

        assert_eq!(
            registry.lookup_method_return_type("Model", "attributes"),
            Some(Type::Untyped)
        );
    }

    #[test]
    fn rbi_empty_body_with_sig_returns_nilclass_keeps_nil() {
        // With a `sig { returns(NilClass) }`, keep nil to stay faithful to the declaration.
        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();

        merge_rbi_source_into_registry(
            "class Model\n  sig { returns(NilClass) }\n  def clear; end\nend\n",
            &mut registry,
            &loader,
        );

        assert_eq!(
            registry.lookup_method_return_type("Model", "clear"),
            Some(Type::Nil)
        );
    }

    #[test]
    fn rbi_empty_body_with_sig_returns_string_keeps_string() {
        // With a `sig { returns(String) }`, it's String as before.
        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();

        merge_rbi_source_into_registry(
            "class Model\n  sig { returns(String) }\n  def name; end\nend\n",
            &mut registry,
            &loader,
        );

        assert_eq!(
            registry.lookup_method_return_type("Model", "name"),
            Some(Type::String)
        );
    }

    #[test]
    fn collect_rbi_classes_can_skip_excluded_subtrees() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let sorbet_rbi = root.join("sorbet").join("rbi");
        let extra_rbi = root.join("custom");
        std::fs::create_dir_all(&sorbet_rbi).expect("mkdir auto dir");
        std::fs::create_dir_all(&extra_rbi).expect("mkdir custom dir");
        std::fs::write(
            sorbet_rbi.join("user.rbi"),
            "class User\n  sig { returns(String) }\n  def name; end\nend\n",
        )
        .expect("write auto rbi");
        std::fs::write(
            extra_rbi.join("account.rbi"),
            "class Account\n  sig { returns(Integer) }\n  def id; end\nend\n",
        )
        .expect("write custom rbi");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);

        let indexed =
            collect_rbi_file_classes_excluding(&[root.to_path_buf()], &[sorbet_rbi], &loader);

        assert_eq!(indexed.len(), 1);
        assert!(
            indexed
                .values()
                .any(|classes| classes.contains(&"Account".to_string()))
        );
    }

    // Guards the shared global merge counter (plan 03 step 1 instrumentation)
    // so counter-observing tests do not interleave.
    static MERGE_COUNTER_GUARD: Mutex<()> = Mutex::new(());

    // Without the shared closure, N referencing files would each redundantly merge the same shape N times.
    #[test]
    fn merge_class_into_is_repeated_once_per_referencing_file() {
        let _guard = MERGE_COUNTER_GUARD.lock().expect("merge counter guard");
        merge_counter::reset();

        let dir = tempdir().expect("tempdir");
        let rbi = dir.path().join("shared.rbi");
        std::fs::write(
            &rbi,
            "class Shared\n  sig { returns(String) }\n  def name; end\nend\n",
        )
        .expect("write rbi");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let stdlib_loader = LazyRbsLoader::new(core_dir);
        let lazy_loader = LazyRbiLoader::new(&[rbi], &[]);

        // Simulate 50 per-file engines, each merging the same external class
        // into its own target registry (the current per-file behavior).
        const FILE_COUNT: usize = 50;
        for _ in 0..FILE_COUNT {
            let mut per_file_registry = TypeRegistry::new();
            assert!(lazy_loader.merge_class_into("Shared", &mut per_file_registry, &stdlib_loader));
        }

        assert_eq!(
            merge_counter::get("Shared"),
            FILE_COUNT,
            "same external class is re-merged once per referencing file",
        );
        // The canonical shape is built once; the rest are cached Arc copies.
        assert_eq!(
            merge_counter::get_builds("Shared"),
            1,
            "canonical shape is built once workspace-wide, not per file",
        );
    }

    // Validates FQN-only sharing: merges of the same FQN produce matching fingerprints.
    fn class_shape_fingerprint(registry: &TypeRegistry, class_name: &str) -> String {
        let data = registry
            .class_data_for(class_name)
            .expect("merged class present");
        let mut methods: Vec<String> = data
            .methods
            .iter()
            .map(|m| format!("{}#{}", m.is_singleton as u8, m.name))
            .collect();
        methods.sort();
        let mut mixins: Vec<String> = data
            .mixins
            .iter()
            .map(|m| format!("{:?}:{}", m.kind, m.module_name.as_ref()))
            .collect();
        mixins.sort();
        format!(
            "module={} super={:?} mixins=[{}] methods=[{}]",
            data.is_module,
            data.superclass.as_ref().map(|s| s.as_ref()),
            mixins.join(","),
            methods.join(","),
        )
    }

    // FQN-only sharing: merging the same FQN yields the same shape regardless of enclosing scope.
    #[test]
    fn merge_class_into_shape_is_independent_of_merge_context() {
        let dir = tempdir().expect("tempdir");
        // Even with a same-named bare/`Owner::Nested` class, the shape is stable per FQN.
        let rbi = dir.path().join("owner.rbi");
        std::fs::write(
            &rbi,
            concat!(
                "class Owner\n",
                "  module Nested\n",
                "    def from_owner_nested; end\n",
                "  end\n",
                "end\n",
                "module Nested\n",
                "  def from_top_nested; end\n",
                "end\n",
            ),
        )
        .expect("write rbi");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let stdlib_loader = LazyRbsLoader::new(core_dir);
        let lazy_loader = LazyRbiLoader::new(&[rbi], &[]);

        // Merge `Owner::Nested` alone into one registry.
        let mut registry_a = TypeRegistry::new();
        assert!(lazy_loader.merge_class_into("Owner::Nested", &mut registry_a, &stdlib_loader));

        // Merge it into a second registry that first accumulated unrelated
        // external classes, mimicking a different per-file merge order/context.
        let mut registry_b = TypeRegistry::new();
        assert!(lazy_loader.merge_class_into("Nested", &mut registry_b, &stdlib_loader));
        assert!(lazy_loader.merge_class_into("Owner", &mut registry_b, &stdlib_loader));
        assert!(lazy_loader.merge_class_into("Owner::Nested", &mut registry_b, &stdlib_loader));

        assert_eq!(
            class_shape_fingerprint(&registry_a, "Owner::Nested"),
            class_shape_fingerprint(&registry_b, "Owner::Nested"),
            "merged shape for a fixed FQN must not depend on merge context",
        );
        // The distinct top-level `Nested` keeps its own separate shape — FQN
        // disambiguates them, so sharing by FQN never conflates the two.
        assert_ne!(
            class_shape_fingerprint(&registry_b, "Nested"),
            class_shape_fingerprint(&registry_b, "Owner::Nested"),
            "distinct FQNs keep distinct shapes",
        );
    }

    // Guards output equivalence: the shared-shape path produces a byte-identical `ClassData` to legacy per-file merge.
    #[test]
    fn shared_shape_matches_legacy_per_file_merge() {
        let dir = tempdir().expect("tempdir");
        // A class split across two `.rbi` files (tapioca commonly emits a class
        // and its generated companions in separate shards).
        let rbi_a = dir.path().join("split_a.rbi");
        std::fs::write(
            &rbi_a,
            concat!(
                "class Split\n",
                "  include Helper\n",
                "  sig { returns(String) }\n",
                "  def from_a; end\n",
                "end\n",
                "module Helper\n",
                "  def helper_method; end\n",
                "end\n",
            ),
        )
        .expect("write rbi a");
        let rbi_b = dir.path().join("split_b.rbi");
        std::fs::write(
            &rbi_b,
            concat!(
                "class Split\n",
                "  sig { returns(Integer) }\n",
                "  def from_b; end\n",
                "end\n",
            ),
        )
        .expect("write rbi b");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let stdlib_loader = LazyRbsLoader::new(core_dir);
        let lazy_loader = LazyRbiLoader::new(&[rbi_a, rbi_b], &[]);

        // Fully materialize the index for the legacy comparison too (to see every declaring file of a shard-split class).
        lazy_loader.ensure_fully_indexed();

        for class_name in ["Split", "Helper"] {
            // Legacy path: merge each declaring file's class directly.
            let files = lazy_loader
                .lookup_class_files(class_name)
                .expect("class indexed");
            let mut legacy = TypeRegistry::new();
            for file in &files {
                let file_registry = lazy_loader.load_file_registry(file, &stdlib_loader);
                if file_registry.class_data_for(class_name).is_some() {
                    legacy.merge_rbs_class_from(&file_registry, class_name);
                }
            }

            // Shared-shape path.
            let mut shared = TypeRegistry::new();
            assert!(lazy_loader.merge_class_into(class_name, &mut shared, &stdlib_loader));

            assert_eq!(
                class_shape_fingerprint(&legacy, class_name),
                class_shape_fingerprint(&shared, class_name),
                "shared shape must match legacy per-file merge for {class_name}",
            );
            // Return types (from `sig`) survive the shape too, across both files.
            assert_eq!(
                legacy.lookup_method_return_type(class_name, "from_a"),
                shared.lookup_method_return_type(class_name, "from_a"),
            );
            assert_eq!(
                legacy.lookup_method_return_type(class_name, "from_b"),
                shared.lookup_method_return_type(class_name, "from_b"),
            );
        }
    }

    #[test]
    fn lazy_rbi_loader_merges_requested_class_on_demand() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let rbi = root.join("user.rbi");
        std::fs::write(
            &rbi,
            "class User\n  sig { returns(String) }\n  def name; end\nend\n",
        )
        .expect("write rbi");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let lazy_loader = LazyRbiLoader::new(&[rbi], &[]);
        let mut registry = TypeRegistry::new();

        assert!(lazy_loader.merge_class_into("User", &mut registry, &loader));
        assert_eq!(
            registry.lookup_method_return_type("User", "name"),
            Some(Type::String)
        );
    }

    #[test]
    fn class_index_tracks_nested_declarations() {
        let files = collect_rbi_paths(&[PathBuf::from("/tmp/non-existent")], &[]);
        assert!(files.is_empty());

        let names = collect_declared_class_names_from_rbi_source(
            "module Admin\n  class User\n    class Profile\n    end\n  end\nend\n",
        );
        assert!(names.contains(&"Admin".to_string()));
        assert!(names.contains(&"Admin::User".to_string()));
        assert!(names.contains(&"Admin::User::Profile".to_string()));
    }

    #[test]
    fn extract_declared_constant_source_wraps_nested_modules() {
        let source = "module Admin\n  class User\n    sig { returns(String) }\n    def name; end\n  end\n\n  class Audit\n    def id; end\n  end\nend\n";
        let extracted =
            extract_declared_constant_source(source, "Admin::User").expect("extract nested class");

        assert!(extracted.contains("module Admin\n"));
        assert!(extracted.contains("class User\n"));
        assert!(extracted.contains("def name; end\n"));
        assert!(!extracted.contains("class Audit\n"));
    }

    #[test]
    fn extract_declared_constant_source_keeps_fully_qualified_class() {
        let source = "class Aws::EC2::Resource\n  sig { returns(String) }\n  def arn; end\nend\n\nclass Aws::EC2::Instance\n  def id; end\nend\n";
        let extracted = extract_declared_constant_source(source, "Aws::EC2::Resource")
            .expect("extract qualified class");

        assert!(extracted.starts_with("class Aws::EC2::Resource\n"));
        assert!(extracted.contains("def arn; end\n"));
        assert!(!extracted.contains("class Aws::EC2::Instance\n"));
    }

    #[test]
    #[ignore = "benchmark"]
    fn bench_parallel_rbi_load_deduplicates_first_parse() {
        let dir = tempdir().expect("tempdir");
        let rbi_path = dir.path().join("user.rbi");
        write_heavy_rbi_file(&rbi_path);

        let file_classes = HashMap::from([(
            rbi_path.to_string_lossy().to_string(),
            vec!["User".to_string()],
        )]);
        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let stdlib_loader = Arc::new(LazyRbsLoader::new(core_dir));
        let threads = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
            .max(4);

        let baseline_loader = Arc::new(DuplicateProneLazyRbiLoader::from_indexed_file_classes(
            &file_classes,
        ));
        let baseline_ms = bench_parallel_rbi_merge(
            {
                let baseline_loader = Arc::clone(&baseline_loader);
                let stdlib_loader = Arc::clone(&stdlib_loader);
                move || {
                    let mut registry = TypeRegistry::new();
                    assert!(baseline_loader.merge_class_into(
                        "User",
                        &mut registry,
                        &stdlib_loader
                    ));
                }
            },
            threads,
        )
        .as_secs_f64()
            * 1000.0;

        let loader = Arc::new(LazyRbiLoader::from_indexed_file_classes(&file_classes));
        let optimized_ms = bench_parallel_rbi_merge(
            {
                let loader = Arc::clone(&loader);
                let stdlib_loader = Arc::clone(&stdlib_loader);
                move || {
                    let mut registry = TypeRegistry::new();
                    assert!(loader.merge_class_into("User", &mut registry, &stdlib_loader));
                }
            },
            threads,
        )
        .as_secs_f64()
            * 1000.0;

        eprintln!(
            "[bench] rbi parallel first-load baseline_ms={baseline_ms:.0} optimized_ms={optimized_ms:.0} speedup={:.2}x threads={threads}",
            baseline_ms / optimized_ms.max(0.1),
        );
        assert!(
            optimized_ms < baseline_ms,
            "optimized rbi loader should beat duplicate-prone baseline"
        );
    }
}

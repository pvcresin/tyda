use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::rbs::import::{
    load_rbs_definitions, load_rbs_definitions_with_classes, load_rbs_string,
};
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::registry::TypeRegistry;
use crate::sorbet::rbi::{
    LazyRbiLoader, collect_rbi_file_classes, merge_rbi_paths_into_registry_excluding,
    merge_rbi_source_into_registry,
};

// stdlib RBS root for the running binary: `TYDA_RBS_DIR` -> next to the exe -> CARGO_MANIFEST_DIR.
pub fn default_vendor_rbs_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("TYDA_RBS_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("vendor").join("rbs");
        if candidate.is_dir() {
            return candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join("rbs")
}

pub struct LoadedTypeEnvironment {
    pub user_rbs: TypeRegistry,
    pub type_file_classes: HashMap<String, Vec<String>>,
    pub lazy_rbi_loader: Option<LazyRbiLoader>,
}

pub struct LoadedCliTypeEnvironment {
    pub user_rbs: TypeRegistry,
    pub lazy_rbi_loader: Option<LazyRbiLoader>,
}

pub fn infer_workspace_root(paths: &[PathBuf]) -> PathBuf {
    let root = paths
        .iter()
        .filter_map(|path| {
            let base = if path.is_file() {
                path.parent().map(|dir| dir.to_path_buf())
            } else {
                Some(path.clone())
            }?;
            Some(find_project_root(&base).unwrap_or(base))
        })
        .next()
        .or_else(|| {
            paths
                .iter()
                .find(|path| path.is_dir())
                .cloned()
                .or_else(|| {
                    paths
                        .first()
                        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
                })
        })
        .unwrap_or_else(|| PathBuf::from("."));
    // Normalizes to `.` when climbing relative to cwd results in an empty path.
    if root.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        root
    }
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if looks_like_project_root(&dir) {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn looks_like_project_root(dir: &Path) -> bool {
    dir.join(".git").exists()
        || dir.join(".ruby-version").is_file()
        || dir.join(".tool-versions").is_file()
        || dir.join("Gemfile").is_file()
        || dir.join("Gemfile.lock").is_file()
        || dir.join("config").join("application.rb").is_file()
}

pub fn load_cli_type_registry(paths: &[PathBuf], stdlib_loader: &LazyRbsLoader) -> TypeRegistry {
    if std::env::var_os("TYDA_TRACE_CLI").is_some() {
        eprintln!("TRACE load_cli_type_registry paths={paths:?}");
    }
    load_cli_type_environment(paths, stdlib_loader).user_rbs
}

pub fn load_cli_type_environment(
    paths: &[PathBuf],
    stdlib_loader: &LazyRbsLoader,
) -> LoadedCliTypeEnvironment {
    if std::env::var_os("TYDA_TRACE_CLI").is_some() {
        eprintln!("TRACE load_cli_type_environment paths={paths:?}");
    }
    let preload_timing = std::env::var_os("TYDA_PRELOAD_TIMING").is_some();
    let t = std::time::Instant::now();
    let auto_dirs = discover_type_dirs_from_paths(paths);
    if std::env::var_os("TYDA_TRACE_CLI").is_some() {
        eprintln!("TRACE auto_dirs={auto_dirs:?}");
    }
    let rbs_paths = collect_cli_rbs_paths(paths, &auto_dirs);
    let rbi_index_paths = collect_cli_rbi_index_paths(paths, &auto_dirs);
    if std::env::var_os("TYDA_TRACE_CLI").is_some() {
        eprintln!("TRACE rbs_paths={rbs_paths:?} rbi_index_paths={rbi_index_paths:?}");
    }
    let discover_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = std::time::Instant::now();
    let mut user_rbs = load_rbs_definitions(&rbs_paths);
    let user_rbs_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = std::time::Instant::now();
    let lazy_rbi_loader = LazyRbiLoader::new(&rbi_index_paths, &[]);
    let rbi_index_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = std::time::Instant::now();
    merge_explicit_cli_rbi_files(paths, &mut user_rbs, stdlib_loader);
    let rbi_merge_ms = t.elapsed().as_secs_f64() * 1000.0;

    if preload_timing {
        eprintln!(
            "TIMING load_cli_type_environment rbs_files={} rbi_index_paths={} discover_ms={discover_ms:.3} user_rbs_ms={user_rbs_ms:.3} rbi_index_ms={rbi_index_ms:.3} rbi_merge_ms={rbi_merge_ms:.3}",
            rbs_paths.len(),
            rbi_index_paths.len(),
        );
    }

    let lazy_rbi_loader = if lazy_rbi_loader.is_empty() {
        None
    } else {
        Some(lazy_rbi_loader)
    };

    LoadedCliTypeEnvironment {
        user_rbs,
        lazy_rbi_loader,
    }
}

pub fn load_workspace_type_environment(
    root: &Path,
    stdlib_loader: &LazyRbsLoader,
) -> LoadedTypeEnvironment {
    let auto_dirs = discover_type_dirs(root);
    let mut rbs_paths = vec![root.to_path_buf()];
    rbs_paths.extend(auto_dirs.iter().cloned());

    let (user_rbs, mut type_file_classes) = load_rbs_definitions_with_classes(&rbs_paths);

    let auto_rbi_classes = collect_rbi_file_classes(&auto_dirs, stdlib_loader);
    type_file_classes.extend(auto_rbi_classes.clone());

    let lazy_rbi_loader = LazyRbiLoader::from_indexed_file_classes(&auto_rbi_classes);
    let lazy_rbi_loader = if lazy_rbi_loader.is_empty() {
        None
    } else {
        Some(lazy_rbi_loader)
    };

    LoadedTypeEnvironment {
        user_rbs,
        type_file_classes,
        lazy_rbi_loader,
    }
}

pub fn reload_external_type_file(
    file_path: &str,
    user_rbs: &mut TypeRegistry,
    type_file_classes: &mut HashMap<String, Vec<String>>,
    stdlib_loader: &LazyRbsLoader,
) -> Vec<String> {
    let old_classes = type_file_classes.remove(file_path).unwrap_or_default();

    for class_name in &old_classes {
        user_rbs.remove_external_methods_for_class(class_name);
        user_rbs.remove_class_if_external_only(class_name);
    }

    let path = PathBuf::from(file_path);
    let new_classes = if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("rbs") => {
                    load_rbs_string(&content, user_rbs);
                    parse_rbs_declared_classes_from_content(&content)
                }
                Some("rbi") => merge_rbi_source_into_registry(&content, user_rbs, stdlib_loader),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    if !new_classes.is_empty() {
        type_file_classes.insert(file_path.to_string(), new_classes.clone());
    }

    let mut affected: HashSet<String> = old_classes.into_iter().collect();
    affected.extend(new_classes);
    affected.into_iter().collect()
}

pub fn discover_type_dirs(root: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![
        root.join("sig"),
        root.join("rbs"),
        root.join(".gem_rbs_collection"),
        root.join("gem_rbs_collection"),
        root.join("sorbet").join("rbi"),
        root.join("rbi"),
    ];

    candidates.extend(discover_rbs_collection_paths(root));

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| candidate.is_dir())
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

pub fn discover_type_dirs_from_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    for path in paths {
        let base = if path.is_file() {
            path.parent().map(|parent| parent.to_path_buf())
        } else {
            Some(path.clone())
        };

        let Some(mut dir) = base else {
            continue;
        };

        loop {
            for candidate in discover_type_dirs(&dir) {
                if seen.insert(candidate.clone()) {
                    dirs.push(candidate);
                }
            }
            if dir.join(".git").exists() || !dir.pop() {
                break;
            }
        }
    }

    dirs
}

pub fn collect_rbs_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "rbs") {
            push_unique_rbs_file(path, &mut result, &mut seen);
        } else if path.is_dir() {
            collect_rbs_files_recursive(path, &mut result, &mut seen);
        }
    }
    result
}

pub fn should_skip_type_dir_name(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "vendor" | "target" | "node_modules" | "tmp" | "log")
}

fn dedupe_paths<'a>(paths: impl IntoIterator<Item = &'a PathBuf>) -> Vec<PathBuf> {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if seen.insert(path.clone()) {
            unique.push(path.clone());
        }
    }
    unique
}

fn collect_cli_rbi_index_paths(paths: &[PathBuf], auto_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let explicit_rbi_files = paths
        .iter()
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "rbi"));
    let explicit_rbi_dirs = paths
        .iter()
        .filter(|path| path.is_dir() && is_rbi_dir(path));
    let auto_rbi_dirs = auto_dirs.iter().filter(|path| is_rbi_dir(path));
    dedupe_paths(
        explicit_rbi_files
            .chain(explicit_rbi_dirs)
            .chain(auto_rbi_dirs),
    )
}

fn merge_explicit_cli_rbi_files(
    paths: &[PathBuf],
    registry: &mut TypeRegistry,
    stdlib_loader: &LazyRbsLoader,
) {
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "rbi") {
            merge_rbi_paths_into_registry_excluding(
                std::slice::from_ref(path),
                &[],
                registry,
                stdlib_loader,
            );
        }
    }
}

fn collect_cli_rbs_paths(paths: &[PathBuf], auto_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let explicit_rbs_files = paths
        .iter()
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "rbs"));
    let explicit_rbs_dirs = paths.iter().filter(|path| {
        path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_rbs_type_dir_name)
    });
    let auto_rbs_dirs = auto_dirs.iter().filter(|path| !is_rbi_dir(path));
    dedupe_paths(
        explicit_rbs_files
            .chain(explicit_rbs_dirs)
            .chain(auto_rbs_dirs),
    )
}

fn is_rbi_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "rbi")
}

#[derive(Debug, Deserialize)]
struct RbsCollectionPathConfig {
    path: Option<PathBuf>,
}

fn is_rbs_type_dir_name(name: &str) -> bool {
    matches!(
        name,
        "sig" | "rbs" | ".gem_rbs_collection" | "gem_rbs_collection"
    )
}

fn discover_rbs_collection_paths(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for config_name in [
        "rbs_collection.yaml",
        "rbs_collection.yml",
        "gem_rbs_collection.yaml",
        "gem_rbs_collection.yml",
        "rbs_collection.lock.yaml",
        "gem_rbs_collection.lock.yaml",
    ] {
        let config_path = root.join(config_name);
        let Some(collection_path) = read_rbs_collection_path(&config_path, root) else {
            continue;
        };
        if seen.insert(collection_path.clone()) {
            result.push(collection_path);
        }
    }
    result
}

fn read_rbs_collection_path(config_path: &Path, root: &Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(config_path).ok()?;
    let config: RbsCollectionPathConfig = serde_yaml::from_str(&content).ok()?;
    let path = config.path?;
    if path.is_absolute() {
        Some(path)
    } else {
        Some(root.join(path))
    }
}

fn push_unique_rbs_file(path: &Path, result: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    let path = path.to_path_buf();
    if seen.insert(path.clone()) {
        result.push(path);
    }
}

fn collect_rbs_files_recursive(dir: &Path, result: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "rbs") {
            push_unique_rbs_file(&path, result, seen);
        } else if path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| !should_skip_type_dir_name(name))
        {
            collect_rbs_files_recursive(&path, result, seen);
        }
    }
}

#[allow(dead_code)]
fn parse_rbs_declared_classes(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|content| parse_rbs_declared_classes_from_content(&content))
        .unwrap_or_default()
}

fn parse_rbs_declared_classes_from_content(content: &str) -> Vec<String> {
    if let Ok(signature) = rbs_sys::parse_signature(content) {
        signature
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                rbs_sys::Declaration::Class { name, .. }
                | rbs_sys::Declaration::Module { name, .. }
                | rbs_sys::Declaration::Interface { name, .. } => Some(name.clone()),
                rbs_sys::Declaration::ClassAlias { new_name, .. }
                | rbs_sys::Declaration::ModuleAlias { new_name, .. } => Some(new_name.clone()),
                rbs_sys::Declaration::Constant { .. }
                | rbs_sys::Declaration::Global { .. }
                | rbs_sys::Declaration::TypeAlias { .. } => None,
            })
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sorbet::rbi::collect_rbi_file_classes_excluding;
    use tempfile::tempdir;

    #[test]
    fn parse_rbs_declared_classes_reads_classes_and_modules() {
        assert_eq!(
            parse_rbs_declared_classes_from_content("class User\nend\nmodule Admin\nend\n"),
            vec!["User".to_string(), "Admin".to_string()]
        );
    }

    #[test]
    fn skip_dir_name_matches_known_large_directories() {
        assert!(should_skip_type_dir_name("vendor"));
        assert!(should_skip_type_dir_name("target"));
        assert!(should_skip_type_dir_name(".claude"));
        assert!(should_skip_type_dir_name(".git"));
        assert!(!should_skip_type_dir_name("app"));
    }

    #[test]
    fn discover_type_dirs_includes_rbs_and_collection_paths() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let rbs_dir = root.join("rbs");
        let default_collection = root.join(".gem_rbs_collection");
        let custom_collection = root.join("vendor").join("rbs_collection");
        std::fs::create_dir_all(&rbs_dir).expect("mkdir rbs");
        std::fs::create_dir_all(&default_collection).expect("mkdir default collection");
        std::fs::create_dir_all(&custom_collection).expect("mkdir custom collection");
        std::fs::write(
            root.join("rbs_collection.lock.yaml"),
            "path: vendor/rbs_collection\ngems: []\n",
        )
        .expect("write lockfile");

        let dirs = discover_type_dirs(root);

        assert!(dirs.contains(&rbs_dir));
        assert!(dirs.contains(&default_collection));
        assert!(dirs.contains(&custom_collection));
    }

    #[test]
    fn collect_rbs_files_dedupes_nested_type_dirs() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let sig_dir = root.join("sig");
        let rbs_file = sig_dir.join("user.rbs");
        std::fs::create_dir_all(&sig_dir).expect("mkdir sig");
        std::fs::write(&rbs_file, "class User\nend\n").expect("write rbs");

        assert_eq!(
            collect_rbs_files(&[root.to_path_buf(), sig_dir]),
            vec![rbs_file]
        );
    }

    #[test]
    fn infer_workspace_root_climbs_to_project_root_for_subdir() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app_dir = root.join("app");
        std::fs::create_dir_all(&app_dir).expect("mkdir app");
        std::fs::write(root.join("Gemfile"), "source 'https://rubygems.org'\n")
            .expect("write gemfile");

        assert_eq!(infer_workspace_root(&[app_dir]), root.to_path_buf());
    }

    #[test]
    fn infer_workspace_root_climbs_to_project_root_for_file() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let model_dir = root.join("app").join("models");
        let model = model_dir.join("user.rb");
        std::fs::create_dir_all(&model_dir).expect("mkdir models");
        std::fs::write(root.join(".ruby-version"), "3.3.0\n").expect("write ruby version");
        std::fs::write(&model, "class User; end\n").expect("write model");

        assert_eq!(infer_workspace_root(&[model]), root.to_path_buf());
    }

    #[test]
    fn cli_registry_keeps_auto_rbi_lazy() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let sorbet_rbi = root.join("sorbet").join("rbi");
        std::fs::create_dir_all(&sorbet_rbi).expect("mkdir auto dir");
        std::fs::write(
            sorbet_rbi.join("user.rbi"),
            "class User\n  sig { returns(String) }\n  def name; end\nend\n",
        )
        .expect("write rbi");
        std::fs::write(root.join("app.rb"), "class App; end\n").expect("write ruby");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let loaded = load_cli_type_environment(&[root.to_path_buf()], &loader);
        let lazy_loader = loaded
            .lazy_rbi_loader
            .as_ref()
            .expect("auto rbi loader should exist");
        let mut registry = loaded.user_rbs;

        assert_eq!(registry.lookup_method_return_type("User", "name"), None);
        assert!(lazy_loader.merge_class_into("User", &mut registry, &loader));

        assert_eq!(
            registry.lookup_method_return_type("User", "name"),
            Some(crate::types::Type::String)
        );
    }

    #[test]
    fn cli_registry_indexes_type_dirs_not_project_tree_rbi() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let sorbet_rbi = root.join("sorbet").join("rbi");
        let clutter = root.join(".claude").join("worktree");
        std::fs::create_dir_all(&sorbet_rbi).expect("mkdir auto dir");
        std::fs::create_dir_all(&clutter).expect("mkdir clutter");
        std::fs::write(
            sorbet_rbi.join("user.rbi"),
            "class User\n  sig { returns(String) }\n  def name; end\nend\n",
        )
        .expect("write auto rbi");
        std::fs::write(
            clutter.join("junk.rbi"),
            "class Junk\n  sig { returns(Integer) }\n  def id; end\nend\n",
        )
        .expect("write clutter rbi");
        std::fs::write(root.join("app.rb"), "class App; end\n").expect("write ruby");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let loaded = load_cli_type_environment(&[root.to_path_buf()], &loader);
        let lazy_loader = loaded
            .lazy_rbi_loader
            .as_ref()
            .expect("auto rbi loader should exist");
        let mut registry = loaded.user_rbs;

        assert_eq!(registry.lookup_method_return_type("User", "name"), None);
        assert_eq!(registry.lookup_method_return_type("Junk", "id"), None);
        assert!(lazy_loader.merge_class_into("User", &mut registry, &loader));
        assert!(
            !lazy_loader.merge_class_into("Junk", &mut registry, &loader),
            "RBI outside discovered type dirs must not be indexed"
        );
        assert_eq!(
            registry.lookup_method_return_type("User", "name"),
            Some(crate::types::Type::String)
        );
    }

    #[test]
    fn cli_registry_discovers_rbs_collection_from_file_path() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let collection_dir = root.join("vendor").join("rbs_collection");
        std::fs::create_dir_all(&collection_dir).expect("mkdir collection");
        std::fs::write(
            root.join("rbs_collection.yaml"),
            "path: vendor/rbs_collection\ngems: []\n",
        )
        .expect("write collection config");
        std::fs::write(
            collection_dir.join("collection_source.rbs"),
            "class CollectionSource\n  def label: () -> String\nend\n",
        )
        .expect("write collection rbs");
        let app = root.join("app.rb");
        std::fs::write(&app, "def label(source)\n  source.label\nend\n").expect("write ruby");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let loaded = load_cli_type_environment(&[app], &loader);

        assert_eq!(
            loaded
                .user_rbs
                .lookup_method_return_type("CollectionSource", "label"),
            Some(crate::types::Type::String)
        );
    }

    #[test]
    fn workspace_type_environment_keeps_rbi_lazy() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let sorbet_rbi = root.join("sorbet").join("rbi");
        std::fs::create_dir_all(&sorbet_rbi).expect("mkdir auto dir");
        std::fs::write(
            sorbet_rbi.join("user.rbi"),
            "class User\n  sig { returns(String) }\n  def name; end\nend\n",
        )
        .expect("write rbi");
        std::fs::write(root.join("app.rb"), "class App; end\n").expect("write ruby");

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let loaded = load_workspace_type_environment(root, &loader);
        let lazy_loader = loaded
            .lazy_rbi_loader
            .as_ref()
            .expect("workspace lazy rbi loader should exist");
        let mut registry = loaded.user_rbs;

        assert_eq!(registry.lookup_method_return_type("User", "name"), None);
        assert!(lazy_loader.merge_class_into("User", &mut registry, &loader));
        assert_eq!(
            registry.lookup_method_return_type("User", "name"),
            Some(crate::types::Type::String)
        );
    }

    #[test]
    #[ignore = "benchmark"]
    fn bench_workspace_type_environment_heavy_external_types() {
        fn build_heavy_workspace(root: &Path) {
            let sig_dir = root.join("sig");
            let rbi_dir = root.join("sorbet").join("rbi");
            std::fs::create_dir_all(&sig_dir).expect("mkdir sig");
            std::fs::create_dir_all(&rbi_dir).expect("mkdir rbi");
            std::fs::write(root.join("app.rb"), "class App; end\n").expect("write ruby");

            for idx in 0..200 {
                let rbs = format!(
                    "class Sig{idx}\n  def value: () -> String\nend\nmodule SigNs{idx}\nend\n"
                );
                std::fs::write(sig_dir.join(format!("sig_{idx}.rbs")), rbs).expect("write rbs");
            }

            for idx in 0..400 {
                let methods = (0..24)
                    .map(|method_idx| {
                        format!("  sig {{ returns(String) }}\n  def field_{method_idx}; end\n")
                    })
                    .collect::<String>();
                let rbi = format!("class Rbi{idx}\n{methods}end\n");
                std::fs::write(rbi_dir.join(format!("rbi_{idx}.rbi")), rbi).expect("write rbi");
            }
        }

        fn load_workspace_type_environment_eager_baseline(
            root: &Path,
            stdlib_loader: &LazyRbsLoader,
        ) -> LoadedTypeEnvironment {
            let auto_dirs = discover_type_dirs(root);
            let mut rbs_paths = vec![root.to_path_buf()];
            rbs_paths.extend(auto_dirs.iter().cloned());

            let mut user_rbs = load_rbs_definitions(&rbs_paths);
            crate::sorbet::rbi::merge_rbi_paths_into_registry(
                &auto_dirs,
                &mut user_rbs,
                stdlib_loader,
            );
            merge_rbi_paths_into_registry_excluding(
                &[root.to_path_buf()],
                &auto_dirs,
                &mut user_rbs,
                stdlib_loader,
            );

            let mut type_file_classes = HashMap::new();
            for path in collect_rbs_files(&rbs_paths) {
                let path_string = path.to_string_lossy().to_string();
                let classes = parse_rbs_declared_classes(&path);
                if !classes.is_empty() {
                    type_file_classes.insert(path_string, classes);
                }
            }
            type_file_classes.extend(collect_rbi_file_classes(&auto_dirs, stdlib_loader));
            type_file_classes.extend(collect_rbi_file_classes_excluding(
                &[root.to_path_buf()],
                &auto_dirs,
                stdlib_loader,
            ));

            LoadedTypeEnvironment {
                user_rbs,
                type_file_classes,
                lazy_rbi_loader: None,
            }
        }

        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        build_heavy_workspace(root);

        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);

        let eager_started = std::time::Instant::now();
        let eager = load_workspace_type_environment_eager_baseline(root, &loader);
        let eager_ms = eager_started.elapsed().as_millis();

        let lazy_started = std::time::Instant::now();
        let lazy = load_workspace_type_environment(root, &loader);
        let lazy_ms = lazy_started.elapsed().as_millis();

        eprintln!(
            "[bench] external type load eager_ms={} lazy_ms={} speedup={:.2}x eager_classes={} lazy_classes={}",
            eager_ms,
            lazy_ms,
            eager_ms as f64 / lazy_ms.max(1) as f64,
            eager.user_rbs.class_names().len(),
            lazy.user_rbs.class_names().len(),
        );
    }
}

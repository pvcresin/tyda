use std::path::{Path, PathBuf};

/// Which directories a Ruby source walk should skip.
///
/// LSP workspace scan uses [`RubyScanScope::Workspace`] so spec/test files stay
/// in the type environment (completion / hover in those files). CLI batch and
/// `--diagnostics` context use [`RubyScanScope::Production`]: tests, coverage,
/// and migration trees are not production type context (schema comes from the
/// Rails project loader, not from `db/migrate`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RubyScanScope {
    Workspace,
    Production,
}

/// Cache / VCS / generated trees. Shared by every scan.
/// Hidden dirs (`.git`, `.claude`, `.bundle`, …) are not production Ruby.
fn is_cache_dir_name(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "vendor" | "target" | "node_modules" | "tmp" | "log" | "sorbet"
        )
}

/// Test, coverage, and migration trees. Rails `db/migrate`, GitLab
/// `db/post_migrate`, and `db/migrate_*` shards are matched by convention
/// rather than a per-repo denylist.
fn is_non_production_dir_name(name: &str) -> bool {
    matches!(
        name,
        "spec" | "test" | "tests" | "features" | "coverage" | "migrate" | "post_migrate"
    ) || name.starts_with("migrate_")
}

pub fn should_skip_dir_name(name: &str, scope: RubyScanScope) -> bool {
    if is_cache_dir_name(name) {
        return true;
    }
    matches!(scope, RubyScanScope::Production) && is_non_production_dir_name(name)
}

pub fn should_skip_dir(path: &Path, scope: RubyScanScope) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| should_skip_dir_name(name, scope))
}

fn collect_rb_files_recursive(dir: &Path, scope: RubyScanScope, result: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() {
            if path.extension().is_some_and(|ext| ext == "rb") {
                result.push(path);
            }
        } else if file_type.is_dir() && !should_skip_dir(&path, scope) {
            collect_rb_files_recursive(&path, scope, result);
        }
    }
}

pub fn collect_rb_files(dir: &Path) -> Vec<PathBuf> {
    collect_rb_files_with_scope(dir, RubyScanScope::Workspace)
}

pub fn collect_rb_files_with_scope(dir: &Path, scope: RubyScanScope) -> Vec<PathBuf> {
    let mut result = Vec::new();
    collect_rb_files_recursive(dir, scope, &mut result);
    result.sort_unstable();
    result
}

pub fn collect_rb_files_from_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    collect_rb_files_from_roots_with_scope(roots, RubyScanScope::Workspace)
}

pub fn collect_rb_files_from_roots_with_scope(
    roots: &[PathBuf],
    scope: RubyScanScope,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in roots {
        if root.is_file() {
            if root.extension().is_some_and(|ext| ext == "rb") {
                files.push(root.clone());
            }
            continue;
        }
        collect_rb_files_recursive(root, scope, &mut files);
    }
    files.sort_unstable();
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_rb_files_is_sorted_and_skips_excluded_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let app = root.join("app");
        std::fs::create_dir_all(&app).expect("app dir");
        std::fs::write(app.join("z.rb"), "class Z; end\n").expect("z");
        std::fs::write(app.join("a.rb"), "class A; end\n").expect("a");
        let vendor = root.join("vendor");
        std::fs::create_dir_all(&vendor).expect("vendor dir");
        std::fs::write(vendor.join("dep.rb"), "class Dep; end\n").expect("dep");
        let rbi = root.join("sorbet").join("rbi");
        std::fs::create_dir_all(&rbi).expect("rbi dir");
        std::fs::write(rbi.join("gen.rb"), "class Gen; end\n").expect("gen");

        let files = collect_rb_files(root);
        assert_eq!(files, vec![app.join("a.rb"), app.join("z.rb")]);
    }

    #[test]
    fn collect_rb_files_from_roots_dedups_and_sorts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let a = root.join("a.rb");
        let b = root.join("b.rb");
        std::fs::write(&a, "class A; end\n").expect("a");
        std::fs::write(&b, "class B; end\n").expect("b");

        let files = collect_rb_files_from_roots(&[b.clone(), a.clone(), a.clone()]);
        assert_eq!(files, vec![a, b]);
    }

    #[test]
    fn collect_rb_files_from_roots_ignores_non_ruby_file_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let txt = root.join("note.txt");
        std::fs::write(&txt, "not ruby\n").expect("txt");

        let files = collect_rb_files_from_roots(&[txt]);
        assert!(files.is_empty());
    }

    #[test]
    fn production_scope_skips_tests_and_migration_trees_workspace_keeps_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let app = root.join("app");
        let spec = root.join("spec");
        let migrate = root.join("db").join("migrate");
        let post_migrate = root.join("db").join("post_migrate");
        let migrate_feed = root.join("db").join("migrate_feed");
        let lib = root.join("lib");
        for dir in [&app, &spec, &migrate, &post_migrate, &migrate_feed, &lib] {
            std::fs::create_dir_all(dir).expect("dir");
        }
        std::fs::write(app.join("user.rb"), "class User; end\n").expect("app");
        std::fs::write(spec.join("user_spec.rb"), "class UserSpec; end\n").expect("spec");
        std::fs::write(migrate.join("001.rb"), "class M1; end\n").expect("migrate");
        std::fs::write(post_migrate.join("002.rb"), "class M2; end\n").expect("post_migrate");
        std::fs::write(migrate_feed.join("003.rb"), "class M3; end\n").expect("migrate_feed");
        std::fs::write(lib.join("helper.rb"), "class Helper; end\n").expect("lib");

        let workspace = collect_rb_files_from_roots_with_scope(
            std::slice::from_ref(&root.to_path_buf()),
            RubyScanScope::Workspace,
        );
        let production = collect_rb_files_from_roots_with_scope(
            std::slice::from_ref(&root.to_path_buf()),
            RubyScanScope::Production,
        );

        assert_eq!(workspace.len(), 6, "workspace keeps spec and migrations");
        assert_eq!(
            production,
            vec![app.join("user.rb"), lib.join("helper.rb")],
            "production keeps app/lib only"
        );
    }

    #[test]
    fn production_skip_matches_cli_legacy_dir_names() {
        assert!(should_skip_dir_name("vendor", RubyScanScope::Production));
        assert!(should_skip_dir_name("spec", RubyScanScope::Production));
        assert!(should_skip_dir_name("migrate", RubyScanScope::Production));
        assert!(should_skip_dir_name(
            "migrate_redshift",
            RubyScanScope::Production
        ));
        assert!(should_skip_dir_name(
            "post_migrate",
            RubyScanScope::Production
        ));
        assert!(!should_skip_dir_name("app", RubyScanScope::Production));
        assert!(!should_skip_dir_name("spec", RubyScanScope::Workspace));
        assert!(!should_skip_dir_name("migrate", RubyScanScope::Workspace));
        assert!(should_skip_dir_name(".claude", RubyScanScope::Production));
        assert!(should_skip_dir_name(".claude", RubyScanScope::Workspace));
        assert!(should_skip_dir_name(".github", RubyScanScope::Production));
    }
}

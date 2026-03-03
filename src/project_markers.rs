use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectMarker {
    SorbetConfig,
}

pub fn has_project_marker_in_ancestors(path: &Path, marker: ProjectMarker) -> bool {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };

    if let Some(hit) = project_marker_cache(marker)
        .read()
        .expect("project marker cache poisoned")
        .get(start)
        .copied()
    {
        return hit;
    }

    let mut visited = Vec::new();
    let mut current = Some(start);
    let result = loop {
        let Some(dir) = current else {
            break false;
        };

        if let Some(hit) = project_marker_cache(marker)
            .read()
            .expect("project marker cache poisoned")
            .get(dir)
            .copied()
        {
            break hit;
        }

        visited.push(dir.to_path_buf());
        if marker_exists_in_dir(dir, marker) {
            break true;
        }
        current = dir.parent();
    };

    if !visited.is_empty() {
        let mut cache = project_marker_cache(marker)
            .write()
            .expect("project marker cache poisoned");
        for dir in visited {
            cache.insert(dir, result);
        }
    }

    result
}

fn project_marker_cache(marker: ProjectMarker) -> &'static RwLock<HashMap<PathBuf, bool>> {
    static SORBET_CACHE: OnceLock<RwLock<HashMap<PathBuf, bool>>> = OnceLock::new();

    match marker {
        ProjectMarker::SorbetConfig => SORBET_CACHE.get_or_init(|| RwLock::new(HashMap::new())),
    }
}

fn marker_exists_in_dir(dir: &Path, marker: ProjectMarker) -> bool {
    match marker {
        ProjectMarker::SorbetConfig => dir.join("sorbet").join("config").is_file(),
    }
}

#[cfg(test)]
pub fn clear_project_marker_cache(marker: ProjectMarker) {
    project_marker_cache(marker)
        .write()
        .expect("project marker cache poisoned")
        .clear();
}

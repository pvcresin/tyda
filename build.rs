use std::path::Path;
use std::process::Command;

// Set TYDA_VERSION (read by src/main.rs) from the single source of version
// truth, `tyda-version.txt`.
//
// `tyda-version.txt` holds `MAJOR.MINOR.YYYYMMDDHHMMSS`: humans bump the
// `MAJOR.MINOR` prefix; the patch is always a release timestamp. The literal
// `YYYYMMDDHHMMSS` placeholder just documents the scheme — the real value is
// injected per build:
//
//   - Release (CI on a main merge): `TYDA_RELEASE_VERSION` is set to the full
//     `MAJOR.MINOR.<commit timestamp>` and used verbatim, so the binary, the
//     gem, and every platform leg report the same version with no git needed.
//   - Dev build (no env): `MAJOR.MINOR.0-dev+<git describe>`, so a local binary
//     records which commit it came from. With no git it degrades to
//     `MAJOR.MINOR.0-dev`.
fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let prefix = version_prefix(&manifest);

    let version = match std::env::var("TYDA_RELEASE_VERSION") {
        Ok(release) if !release.trim().is_empty() => release.trim().to_string(),
        _ => match git_describe() {
            Some(desc) => format!("{prefix}.0-dev+{desc}"),
            None => format!("{prefix}.0-dev"),
        },
    };
    println!("cargo:rustc-env=TYDA_VERSION={version}");

    // Recompute when the version source, the release env, or HEAD changes.
    println!("cargo:rerun-if-changed=tyda-version.txt");
    println!("cargo:rerun-if-env-changed=TYDA_RELEASE_VERSION");
    for p in [".git/HEAD", ".git/logs/HEAD"] {
        if Path::new(&manifest).join(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}

/// `MAJOR.MINOR` from `tyda-version.txt` (the human-bumped prefix). Falls back
/// to `0.0` if the file is missing or malformed.
fn version_prefix(manifest: &str) -> String {
    let raw =
        std::fs::read_to_string(Path::new(manifest).join("tyda-version.txt")).unwrap_or_default();
    let mut parts = raw.trim().split('.');
    let major = parts.next().filter(|s| !s.is_empty()).unwrap_or("0");
    let minor = parts.next().filter(|s| !s.is_empty()).unwrap_or("0");
    format!("{major}.{minor}")
}

fn git_describe() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let desc = String::from_utf8(output.stdout).ok()?;
    let desc = desc.trim();
    if desc.is_empty() {
        None
    } else {
        Some(desc.to_string())
    }
}

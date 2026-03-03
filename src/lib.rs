// The minimal wasm build doesn't use CLI/LSP, so dead_code is expected there; allow it crate-wide only under the `wasm` feature.
#![cfg_attr(feature = "wasm", allow(dead_code))]

// mimalloc doesn't support wasm, so it's CLI/LSP only; the feature gate lets clippy still check the wasm path on the host.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "wasm")))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Returns cross-thread frees on rayon workers back to the OS via a pool-wide broadcast collect.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "wasm")))]
pub fn reclaim_freed_memory(pool: Option<&rayon::ThreadPool>) {
    unsafe extern "C" {
        fn mi_collect(force: bool);
    }
    if let Some(pool) = pool {
        pool.broadcast(|_| unsafe { mi_collect(true) });
    }
    unsafe { mi_collect(true) };
}

#[cfg(any(target_arch = "wasm32", feature = "wasm"))]
pub fn reclaim_freed_memory(_pool: Option<&rayon::ThreadPool>) {}

pub mod analysis;
pub mod dep_graph;
pub mod diagnostics;
pub mod inference;
// The LSP server depends on tower-lsp / tokio; only enabled under the `lsp` feature.
#[cfg(feature = "lsp")]
pub mod lsp;
pub mod parser;
pub mod project;
pub mod project_markers;
pub mod query;
pub mod rails;
pub mod rbs;
pub mod registry;
pub mod scenario;
pub mod schema_parser;
pub mod sorbet;
pub mod sym;
pub mod types;
pub mod workspace_discovery;
pub mod workspace_state;

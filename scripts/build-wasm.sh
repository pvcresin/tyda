#!/usr/bin/env bash
# Build the tyda core as a wasm32-wasip1 module.
#
# C deps (prism / rbs) are pure C and need a wasi-libc sysroot to compile,
# so point the `cc` crate at a wasi-sdk toolchain. wasi-sdk is resolved in this
# order: $WASI_SDK_PATH, then ~/wasi-sdk, then the mise-managed install
# (`mise run setup` installs wasi-sdk via mise).
set -euo pipefail

# rust-toolchain.toml does not list this target (test CI would otherwise
# download it on every cargo run). pages.yml / `mise run setup` also add it.
rustup target add wasm32-wasip1

WASI_SDK="${WASI_SDK_PATH:-}"
if [ -z "$WASI_SDK" ] || [ ! -x "$WASI_SDK/bin/clang" ]; then
  if [ -x "$HOME/wasi-sdk/bin/clang" ]; then
    WASI_SDK="$HOME/wasi-sdk"
  elif command -v mise >/dev/null 2>&1; then
    # mise pins wasi-sdk via the explicit asdf backend (the `wasi-sdk` registry
    # shortname was dropped upstream). Try that name first, then the old
    # shortname so existing local installs keep resolving.
    for _tool in "asdf:mise-plugins/mise-wasi-sdk" wasi-sdk; do
      if _dir="$(mise where "$_tool" 2>/dev/null)" && [ -n "$_dir" ]; then
        WASI_SDK="$_dir/wasi-sdk"
        break
      fi
    done
  fi
fi
if [ ! -x "$WASI_SDK/bin/clang" ]; then
  echo "error: wasi-sdk clang not found" >&2
  echo "       run 'mise run setup' (installs wasi-sdk), set WASI_SDK_PATH," >&2
  echo "       or install wasi-sdk under ~/wasi-sdk" >&2
  exit 1
fi

# ruby-prism-sys' build script reads WASI_SDK_PATH directly (it has first-class
# wasm support); rbs-sys uses the `cc` crate, which honors
# CC_<target>/AR_<target>/CFLAGS_<target>. WASI has no real mmap, so rbs'
# allocator needs the emulated-mman shim (-D at compile, -l at link below).
export WASI_SDK_PATH="$WASI_SDK"
export CC_wasm32_wasip1="$WASI_SDK/bin/clang"
export AR_wasm32_wasip1="$WASI_SDK/bin/llvm-ar"
export CFLAGS_wasm32_wasip1="--sysroot=$WASI_SDK/share/wasi-sysroot -D_WASI_EMULATED_MMAN"

# rbs-sys depends on ruby-rbs-sys (bindgen). For the wasm target, bindgen parses
# headers via libclang and needs the wasi sysroot (assert.h, 32-bit layout).
# That is separate from cc's CFLAGS, so set it for bindgen too.
export BINDGEN_EXTRA_CLANG_ARGS_wasm32_wasip1="--target=wasm32-wasip1 --sysroot=$WASI_SDK/share/wasi-sysroot"

# rbs' allocator uses mmap; link the emulated-mman shim into the wasm binary.
# Use the wasm32-wasip1-target-specific RUSTFLAGS so these wasm-only linker
# flags never leak to host build scripts / proc-macros (which made macOS-green
# builds fail on CI). The homebrew and official wasi-sdk tarballs lay the lib
# out differently, so fall back to a find pinned to the non-threads variant.
WASI_LIB="$WASI_SDK/share/wasi-sysroot/lib/wasm32-wasip1"
if [ ! -f "$WASI_LIB/libwasi-emulated-mman.a" ]; then
  WASI_LIB="$(dirname "$(find "$WASI_SDK" -path '*/wasm32-wasip1/*' -name libwasi-emulated-mman.a 2>/dev/null | head -1)")"
fi
if [ -z "$WASI_LIB" ] || [ ! -f "$WASI_LIB/libwasi-emulated-mman.a" ]; then
  echo "error: libwasi-emulated-mman.a not found under $WASI_SDK" >&2
  exit 1
fi
export CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS="-L native=$WASI_LIB -l wasi-emulated-mman"

exec cargo build \
  --target wasm32-wasip1 \
  --no-default-features \
  --features wasm \
  --bin tyda-wasm \
  "$@"

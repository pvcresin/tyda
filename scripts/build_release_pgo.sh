#!/usr/bin/env bash
set -euo pipefail

# Builds target/release/tyda with profile-guided optimization (PGO).
#
# An instrumented build analyzes a small training corpus, and the final build
# is optimized with the recorded profile. PGO changes code layout and inlining
# decisions only, so the binary behaves the same as a plain release build.
# The output path is the same as `cargo build --release`.
#
#   scripts/build_release_pgo.sh

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# Small pinned subjects (fetched by setup_subjects.sh). Each runs as RBS output and
# as diagnostics; the bundled sample runs as RBS output only, because its
# diagnostics would scan the whole repository as context.
TRAINING_SUBJECTS=(rack rake typeprof conference-app)
TRAINING_PATHS=(subject/rack subject/rake subject/typeprof/lib subject/conference-app)
PGO_DIR="$ROOT_DIR/target/pgo"

EXE=""
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) EXE=".exe" ;;
esac

# On Windows, rustc takes native paths and Git Bash takes Unix-style ones.
native_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "$1"
  else
    printf '%s' "$1"
  fi
}
shell_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -u "$1"
  else
    printf '%s' "$1"
  fi
}

rustup component add llvm-tools-preview
host="$(rustc -vV | sed -n 's/^host: //p')"
profdata="$(shell_path "$(rustc --print sysroot)")/lib/rustlib/$host/bin/llvm-profdata$EXE"
if [[ ! -x "$profdata" ]]; then
  echo "llvm-profdata not found at $profdata" >&2
  exit 1
fi

./scripts/setup_subjects.sh "${TRAINING_SUBJECTS[@]}"

rm -rf "$PGO_DIR/raw" "$PGO_DIR/merged.profdata"
mkdir -p "$PGO_DIR/raw"

echo "=== Instrumented build ==="
RUSTFLAGS="${RUSTFLAGS:-} -Cprofile-generate=$(native_path "$PGO_DIR/raw")" \
  cargo build --release --locked --bin tyda --target-dir "$PGO_DIR/build"

echo "=== Training run ==="
instrumented="$PGO_DIR/build/release/tyda$EXE"
# The instrumented binary also warns about value-profile counters on stderr.
training_log="$PGO_DIR/training.log"
: >"$training_log"
train() {
  echo "  tyda $*"
  if ! "$instrumented" "$@" >/dev/null 2>>"$training_log"; then
    tail -n 20 "$training_log" >&2
    exit 1
  fi
}
train subject/sample
for path in "${TRAINING_PATHS[@]}"; do
  train "$path"
  train --diagnostics "$path"
done

"$profdata" merge -o "$PGO_DIR/merged.profdata" "$PGO_DIR/raw"
if [[ ! -s "$PGO_DIR/merged.profdata" ]]; then
  echo "the training run recorded no profile" >&2
  exit 1
fi

echo "=== Optimized build ==="
RUSTFLAGS="${RUSTFLAGS:-} -Cprofile-use=$(native_path "$PGO_DIR/merged.profdata")" \
  cargo build --release --locked --bin tyda

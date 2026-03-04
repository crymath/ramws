#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
benchmark_cpp_vs_ramws.sh

Benchmarks a clean local C++ build against the same build inside a RAMWS workspace.

Default benchmark project: DuckDB (https://github.com/duckdb/duckdb)

Usage:
  scripts/benchmark_cpp_vs_ramws.sh [options]

Options:
  --ramws-bin PATH     Path to ramws binary (default: ./target/release/ramws)
  --work-dir PATH      Working directory for benchmark artifacts
  --repo-url URL       Git repository URL (default: DuckDB)
  --ref REF            Git ref (branch/tag/commit). Default: repository default branch HEAD
  --target NAME        Build target name (default: duckdb)
  --jobs N             Parallel build jobs (default: hw logical CPU count)
  --pool-size-gib N    Managed RAMWS pool size in GiB (default: 4)
  --generator NAME     CMake generator. Auto-detects Ninja, else Unix Makefiles
  --cleanup            Remove work directory after benchmark
  -h, --help           Show this help

Output:
  - Summary on stdout
  - Logs + result JSON in work directory
USAGE
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

RAMWS_BIN="$REPO_ROOT/target/release/ramws"
WORK_DIR=""
REPO_URL="https://github.com/duckdb/duckdb.git"
REF=""
TARGET_NAME="duckdb"
POOL_SIZE_GIB=4
CLEANUP=0

if command -v sysctl >/dev/null 2>&1; then
  JOBS="$(sysctl -n hw.logicalcpu 2>/dev/null || echo 8)"
else
  JOBS=8
fi

if command -v ninja >/dev/null 2>&1; then
  GENERATOR="Ninja"
else
  GENERATOR="Unix Makefiles"
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ramws-bin)
      RAMWS_BIN="$2"
      shift 2
      ;;
    --work-dir)
      WORK_DIR="$2"
      shift 2
      ;;
    --repo-url)
      REPO_URL="$2"
      shift 2
      ;;
    --ref)
      REF="$2"
      shift 2
      ;;
    --target)
      TARGET_NAME="$2"
      shift 2
      ;;
    --jobs)
      JOBS="$2"
      shift 2
      ;;
    --pool-size-gib)
      POOL_SIZE_GIB="$2"
      shift 2
      ;;
    --generator)
      GENERATOR="$2"
      shift 2
      ;;
    --cleanup)
      CLEANUP=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

for cmd in git cmake awk perl; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "missing required command: $cmd" >&2
    exit 1
  fi
done

if [[ "$GENERATOR" == "Ninja" ]] && ! command -v ninja >/dev/null 2>&1; then
  echo "generator 'Ninja' selected but ninja is not installed" >&2
  exit 1
fi

if [[ ! -x "$RAMWS_BIN" ]]; then
  echo "ramws binary not found or not executable: $RAMWS_BIN" >&2
  echo "build it first: cargo build --release" >&2
  exit 1
fi

if [[ -z "$WORK_DIR" ]]; then
  WORK_DIR="$(mktemp -d /tmp/ramws-cpp-bench-XXXXXX)"
else
  mkdir -p "$WORK_DIR"
fi

POOL_NAME="RAMWS_BENCH_$$"
SOURCE_CACHE="$WORK_DIR/source-cache"
NATIVE_DIR="$WORK_DIR/native-project"
RAMWS_DIR="$WORK_DIR/ramws-project"
BENCH_HOME="$WORK_DIR/bench-home"

UP_STDOUT="$WORK_DIR/ramws_up.stdout"
UP_STDERR="$WORK_DIR/ramws_up.stderr"
DESTROY_STDERR="$WORK_DIR/ramws_destroy.stderr"
NATIVE_LOG="$WORK_DIR/native_build.log"
RAMWS_LOG="$WORK_DIR/ramws_build.log"
RESULT_JSON="$WORK_DIR/result.json"

cleanup() {
  if [[ -d "$RAMWS_DIR" ]]; then
    (cd "$RAMWS_DIR" && HOME="$BENCH_HOME" "$RAMWS_BIN" destroy --all >/dev/null 2>"$DESTROY_STDERR") || true
  fi

  if [[ "$CLEANUP" -eq 1 ]]; then
    rm -rf "$WORK_DIR"
  fi
}
trap cleanup EXIT

print_step() {
  echo
  echo "==> $1"
}

parse_real_seconds() {
  awk '/^real / {print $2}' "$1" | tail -n 1
}

run_timed_to_log() {
  local log_file="$1"
  shift
  {
    /usr/bin/time -p "$@"
  } >"$log_file" 2>&1
}

print_step "Benchmark setup"
echo "work_dir: $WORK_DIR"
echo "ramws_bin: $RAMWS_BIN"
echo "repo_url: $REPO_URL"
echo "ref: ${REF:-<default branch HEAD>}"
echo "generator: $GENERATOR"
echo "target: $TARGET_NAME"
echo "jobs: $JOBS"
echo "pool_size_gib: $POOL_SIZE_GIB"

print_step "Cloning source"
if [[ ! -d "$SOURCE_CACHE/.git" ]]; then
  git clone --depth 1 "$REPO_URL" "$SOURCE_CACHE"
fi

if [[ -n "$REF" ]]; then
  git -C "$SOURCE_CACHE" fetch --depth 1 origin "$REF"
  git -C "$SOURCE_CACHE" checkout "$REF"
else
  git -C "$SOURCE_CACHE" checkout --detach >/dev/null 2>&1 || true
fi

COMMIT_SHA="$(git -C "$SOURCE_CACHE" rev-parse --short HEAD)"
FULL_SHA="$(git -C "$SOURCE_CACHE" rev-parse HEAD)"

print_step "Preparing clean source copies"
rm -rf "$NATIVE_DIR" "$RAMWS_DIR"
mkdir -p "$NATIVE_DIR" "$RAMWS_DIR"

git -C "$SOURCE_CACHE" archive --format=tar HEAD | tar -x -C "$NATIVE_DIR"
git -C "$SOURCE_CACHE" archive --format=tar HEAD | tar -x -C "$RAMWS_DIR"

print_step "Preparing isolated RAMWS config"
mkdir -p "$BENCH_HOME"
(
  cd "$RAMWS_DIR"
  HOME="$BENCH_HOME" "$RAMWS_BIN" init --force >/dev/null
)

USER_CFG="$(find "$BENCH_HOME" -type f -name config.toml | head -n 1)"
if [[ -z "$USER_CFG" ]]; then
  echo "failed to locate ramws user config under $BENCH_HOME" >&2
  exit 1
fi

perl -0777 -i -pe "s/volume_name = \"RAMWS\"/volume_name = \"$POOL_NAME\"/g; s/size_gib = 8/size_gib = $POOL_SIZE_GIB/g" "$USER_CFG"

print_step "Native build benchmark"
run_timed_to_log "$NATIVE_LOG" /bin/sh -c "cmake -S \"$NATIVE_DIR\" -B \"$NATIVE_DIR/build-native\" -G \"$GENERATOR\" -DCMAKE_BUILD_TYPE=Release && cmake --build \"$NATIVE_DIR/build-native\" --target \"$TARGET_NAME\" --parallel \"$JOBS\""
NATIVE_SECONDS="$(parse_real_seconds "$NATIVE_LOG")"

if [[ -x "$NATIVE_DIR/build-native/$TARGET_NAME" ]]; then
  NATIVE_BINARY="$NATIVE_DIR/build-native/$TARGET_NAME"
else
  NATIVE_BINARY="<not found>"
fi

echo "native_build_seconds: $NATIVE_SECONDS"

echo
print_step "RAMWS up (pool create/mount + sync-in)"
UP_START="$(date +%s)"
(
  cd "$RAMWS_DIR"
  HOME="$BENCH_HOME" "$RAMWS_BIN" up >"$UP_STDOUT" 2>"$UP_STDERR"
)
UP_END="$(date +%s)"
RAMWS_UP_SECONDS="$((UP_END - UP_START))"
WS_PATH="$(tail -n 1 "$UP_STDOUT")"

if [[ ! -d "$WS_PATH" ]]; then
  echo "failed to resolve workspace path from ramws up output" >&2
  echo "stdout:" >&2
  cat "$UP_STDOUT" >&2
  echo "stderr:" >&2
  cat "$UP_STDERR" >&2
  exit 1
fi

echo "workspace_path: $WS_PATH"
echo "ramws_up_seconds: $RAMWS_UP_SECONDS"

print_step "RAMWS workspace build benchmark"
run_timed_to_log "$RAMWS_LOG" /bin/sh -c "cd \"$WS_PATH\" && cmake -S . -B build-ramws -G \"$GENERATOR\" -DCMAKE_BUILD_TYPE=Release && cmake --build build-ramws --target \"$TARGET_NAME\" --parallel \"$JOBS\""
RAMWS_BUILD_SECONDS="$(parse_real_seconds "$RAMWS_LOG")"

if [[ -x "$WS_PATH/build-ramws/$TARGET_NAME" ]]; then
  RAMWS_BINARY="$WS_PATH/build-ramws/$TARGET_NAME"
else
  RAMWS_BINARY="<not found>"
fi

echo "ramws_build_seconds: $RAMWS_BUILD_SECONDS"

RAMWS_TOTAL_SECONDS="$(awk -v a="$RAMWS_UP_SECONDS" -v b="$RAMWS_BUILD_SECONDS" 'BEGIN { printf "%.2f", a + b }')"
BUILD_SPEEDUP="$(awk -v n="$NATIVE_SECONDS" -v r="$RAMWS_BUILD_SECONDS" 'BEGIN { if (r > 0) printf "%.3f", n/r; else print "inf" }')"
TOTAL_SPEEDUP="$(awk -v n="$NATIVE_SECONDS" -v r="$RAMWS_TOTAL_SECONDS" 'BEGIN { if (r > 0) printf "%.3f", n/r; else print "inf" }')"

cat >"$RESULT_JSON" <<JSON
{
  "project": {
    "repo_url": "$REPO_URL",
    "ref": "${REF:-<default branch HEAD>}",
    "commit": "$FULL_SHA",
    "target": "$TARGET_NAME"
  },
  "environment": {
    "generator": "$GENERATOR",
    "jobs": $JOBS,
    "pool_name": "$POOL_NAME",
    "pool_size_gib": $POOL_SIZE_GIB
  },
  "results": {
    "native_build_seconds": $NATIVE_SECONDS,
    "ramws_up_seconds": $RAMWS_UP_SECONDS,
    "ramws_build_seconds": $RAMWS_BUILD_SECONDS,
    "ramws_total_seconds": $RAMWS_TOTAL_SECONDS,
    "speedup_build_only": $BUILD_SPEEDUP,
    "speedup_with_up_overhead": $TOTAL_SPEEDUP
  },
  "artifacts": {
    "work_dir": "$WORK_DIR",
    "native_log": "$NATIVE_LOG",
    "ramws_up_stdout": "$UP_STDOUT",
    "ramws_up_stderr": "$UP_STDERR",
    "ramws_build_log": "$RAMWS_LOG"
  }
}
JSON

print_step "Benchmark summary"
echo "project_commit: $COMMIT_SHA"
echo "native_build_seconds: $NATIVE_SECONDS"
echo "ramws_up_seconds: $RAMWS_UP_SECONDS"
echo "ramws_build_seconds: $RAMWS_BUILD_SECONDS"
echo "ramws_total_seconds: $RAMWS_TOTAL_SECONDS"
echo "speedup_build_only (native/ramws_build): $BUILD_SPEEDUP"
echo "speedup_with_up_overhead (native/(up+build)): $TOTAL_SPEEDUP"
echo "native_binary: $NATIVE_BINARY"
echo "ramws_binary: $RAMWS_BINARY"
echo "result_json: $RESULT_JSON"

echo
if [[ "$CLEANUP" -eq 1 ]]; then
  echo "work_dir cleaned"
else
  echo "work_dir retained: $WORK_DIR"
fi

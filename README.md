# ramws

`ramws` provides per-project workspaces backed by fast storage pools. In v1, managed pools are macOS RAM disks created with `hdiutil ram://...` and formatted/mounted via `diskutil`.

## Features (v1)

- macOS managed RAM-disk pools with plist parsing (`hdiutil -plist` / `diskutil -plist`)
- external directory pools for non-managed storage
- per-project workspace resolution inside pools
- sync engine (`orig -> ws`, `ws -> orig`) with conflict detection
- lifecycle commands: `up`, `shell`, `run`, `sync`, `status`, `destroy`

## Install / Build

```sh
cargo build --release
```

## Quickstart

```sh
# Initialize project config in your repo root
ramws init

# Prepare workspace (auto-creates managed pool if needed)
ramws up

# Open interactive shell in workspace, then sync back on exit per policy
ramws shell

# Run one command in workspace
ramws run -- cargo test

# Explicit syncs
ramws sync in
ramws sync out

# Inspect state
ramws status

# Destroy current project workspace
ramws destroy

# Destroy all workspaces in pool and detach managed pool
ramws destroy --all
```

## Benchmark: native vs RAMWS C++ build

Use the benchmark helper to compare a clean local C++ build against the same
build inside a RAMWS workspace. The default project is DuckDB, which is large
enough to give meaningful compile-time comparison on modern laptops.

```sh
# Build ramws first
cargo build --release

# Run benchmark (defaults: DuckDB HEAD, target=duckdb)
scripts/benchmark_cpp_vs_ramws.sh
```

Useful options:

```sh
# Pin to a specific ref
scripts/benchmark_cpp_vs_ramws.sh --ref <tag-or-commit>

# Keep artifacts in a known location
scripts/benchmark_cpp_vs_ramws.sh --work-dir /tmp/ramws-bench

# Control parallelism
scripts/benchmark_cpp_vs_ramws.sh --jobs 8
```

## Configuration

### Project config (`.ramws.toml`)

Default created by `ramws init`:

```toml
version = 1

[workspace]
pool = "main"
layout = "${user}/${project}"

[sync]
delete_extra = false
respect_gitignore = true
follow_symlinks = false
conflict_policy = "abort"
hash_mode = "metadata"
exclude = [".git/**", "target/**", "build/**", ".ramws/**"]

[lifecycle]
sync_on_exit = "ask"
```

### User config (`ProjectDirs::config_dir()/config.toml`)

`ramws` initializes a default managed pool automatically if absent.

## Safety guardrails

- managed pool commands parse plist output only
- managed erase path validates virtual disk-image device metadata
- `sync_on_exit = ask` fails in non-interactive sessions
- conflict policy is explicit: `abort`, `prefer_workspace`, `prefer_original`

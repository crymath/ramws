# ramws

`ramws` is a per-project RAM workspace orchestrator for Linux. It mirrors project sources into a tmpfs-backed workspace (default `/dev/shm/ramws-${USER}/${PROJECT}`), gives you a RAM-backed shell, and lets you sync results back safely.

## Quick start

```bash
ramws init                  # create a starter .ramws.yml in the project root
ramws start                 # create and populate the RAM workspace
ramws shell                 # open a shell rooted in the RAM copy
ramws sync --back           # sync changes from RAM to disk
ramws destroy               # remove the workspace
```

Global flags include `--chdir <path>` to pick a project root and `--config <file>` to point at a specific `.ramws.yml`.

## Configuration

`ramws` reads `.ramws.yml` from the project root (searching upward from `--chdir` or CWD). The default template mirrors the entire tree, excluding common build directories. Workspace roots support `${USER}` and `${PROJECT}` expansion.

Example minimal configuration:

```yaml
workspace:
  root: /dev/shm/ramws-${USER}/${PROJECT}

sources:
  - path: .
    exclude:
      - .git/**
      - build/**
      - target/**
      - node_modules/**

build_dirs:
  - path: build
    type: scratch

sync:
  on_exit: ask
  delete: true
```

## Commands

- `ramws init` – create `.ramws.yml` in the project root (use `--force` to overwrite).
- `ramws start` – ensure the workspace exists and mirror sources into RAM.
- `ramws shell` – open an interactive shell (or run a command) in the workspace with `RAMWS_*` environment markers.
- `ramws sync` – sync files either back to disk (default) or refresh from disk with `--from`. Limit scope with `--only` or `--role` (`source|cache|scratch`).
- `ramws status` – report workspace path, filesystem stats, and pending changes.
- `ramws destroy` – remove the workspace, optionally forcing past unsynced changes.

## Notes

- Workspaces default to tmpfs; a warning is shown if the target path is not tmpfs-backed.
- Sync operations use `rsync` under the hood with optional deletion mirroring.
- Basic integration tests cover config creation and loading.

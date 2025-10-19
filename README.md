# codex-warden

`codex-warden` is a thin command-line supervisor around the official Codex CLI. It keeps the SPEC-driven workflow from our CLAUDE configuration intact by launching Codex jobs, mirroring stdin/stdout, capturing per-run logs, and maintaining a shared-memory registry that companion tools (such as the `wait` command) can inspect.

## Features

- **Transparent passthrough** - forwards every argument and stdin byte to the real `codex` binary without interpretation.
- **Shared task registry** - stores one JSON record per Codex child process inside the `codex-task` shared-memory namespace so companion processes can monitor or clean up stragglers.
- **Lifecycle management** - checks `codex --version`, spawns children, tails their output into `%TEMP%/{uuid}.txt`, mirrors exit codes, and tears down JobObjects or process groups on exit or signal.
- **Wait mode** - `codex-warden wait` blocks until the shared registry is empty, summarises finished job logs, prunes entries older than 12 hours, and lists still-running tasks.

## Quick start

```bash
# Launch a Codex job and record its output under %TEMP%/{uuid}.txt
codex-warden exec run --plan specs/my-task.plan.json

# Block until all codex-warden tasks finish, then review the logs
codex-warden wait
```

1. Keep the official `codex` binary on `PATH` (or adjust `CODEX_BIN` in `src/config.rs`).
2. Replace any direct `codex …` calls in SKILL scripts with `codex-warden …`.
3. Pair each launch with `codex-warden wait` so the shared registry is drained in the SPEC-mandated "CLI + wait" pattern.

## CLI reference

```bash
# pass-through mode
codex-warden exec run --plan path/to/spec.plan.json

# resume an existing run
codex-warden exec resume <task-id>

# wait for all registered jobs to finish
codex-warden wait
```

If the program is invoked with no arguments, it simply runs `codex --version` to validate the delegate is present and exits with the same status code.

## Shared-memory registry

- Namespace: `codex-task`
- Backing size: 4 MiB (`SHARED_MEMORY_SIZE`)
- Registry record (stored as JSON):
  ```json
  {
    "started_at": "2025-10-19T09:30:59Z",
    "log_id": "8bafb645-1d28-4c73-8718-6da35b9ebd5d",
    "log_path": "C:\Users\you\AppData\Local\Temp\8bafb645.txt",
    "manager_pid": 12345,
    "cleanup_reason": null
  }
  ```
- During start-up we sweep the map, terminating orphaned Codex processes, trimming entries older than 12 hours, and annotating removed records.

## Environment variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `CODEX_WARDEN_WAIT_INTERVAL_SEC` | Polling interval for `codex-warden wait`. | `30` |
| `CODEX_WARDEN_DEBUG` | Enables stderr debug messages (`true` / `1`). | disabled |

Legacy keys `CODEX_WORKER_WAIT_INTERVAL_SEC` and `CODEX_WORKER_DEBUG` are still honoured but will be removed in a future release.

## Working with CLAUDE.MD + SKILL

Our personal `~/.claude/CLAUDE.md` enforces the "codex CLI + wait" execution pattern for SKILL workflows (`/multi-ai-iteration`, `/spec-alignment`, `/arch-review`, …). To remain compliant while using this tool:

1. **Substitute `codex-warden` for `codex`.** Example: change `codex exec …` to `codex-warden exec …`.
2. **Always follow up with `codex-warden wait`.** The wait command reads the shared registry and mirrors the SPEC requirement for draining jobs before progressing.
3. **Update automation scripts.** Edited SKILL definitions in `~/.claude/skills/*.md` should call `codex-warden` directly so launch/cleanup semantics stay aligned.
4. **Review logs.** The wait step prints an ordered list of log files; follow them in sequence before resuming the workflow.

## Testing & CI

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

- Unit tests cover shared-memory registry operations and environment fallbacks for wait-mode intervals.
- GitHub Actions (`.github/workflows/ci.yml`) runs the commands above on every `push` and `pull_request`.
- Publishing checklist: `cargo publish --dry-run` followed by `cargo publish` once the checks succeed.

> The project targets the stable toolchain. On Windows, install the *Desktop development with C++* workload so `link.exe` is available to Rust.

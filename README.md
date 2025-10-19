# codex-warden

`codex-warden` is a thin command-line supervisor that wraps the official Codex CLI to satisfy the SPEC-driven workflow described in our CLAUDE configuration. It launches Codex jobs, mirrors their standard input, captures their combined output into per-run log files, and keeps a shared-memory task registry so that other processes can monitor, resume, or clean up outstanding work.

## What it does

- **Transparent passthrough** – forwards every argument and stdin byte to the real `codex` binary without interpretation.
- **Shared task registry** – stores one JSON record per Codex child process inside the `codex-task` shared-memory namespace so that companions (e.g. `codex-warden wait`) or other automation can see progress and clean up stragglers.
- **Lifecycle management** – validates Codex availability (`codex --version`), spawns children, tails their output into `%TEMP%\{uuid}.txt`, mirrors exit codes, and tears down JobObjects / process groups on exit or signals.
- **Wait mode** – `codex-warden wait` blocks until the shared registry drains, summarises finished job logs, trims stale >12h entries, and gives a final list of still-running tasks.

## CLI overview

```bash
# pass-through mode
codex-warden exec run --plan path/to/spec.plan.json

# resume an existing run
codex-warden exec resume <task-id>

# wait for all registered jobs to finish and show their logs
codex-warden wait
```

- The wrapper expects the official `codex` binary to be available on `PATH`; the default delegate can be changed by editing `CODEX_BIN` in `config.rs`.
- When invoked **without arguments**, `codex-warden` simply runs `codex --version` and exits with 0/1 based on success.
- When invoked with `wait`, no Codex process is launched; instead the shared registry is polled every 30 seconds (configurable).
- For every other argument set, the tool forwards the command to `codex` and writes the merged stdout/stderr stream to `%TEMP%\{uuid}.txt`. The log path and metadata are stored in the shared registry under the child PID.

## Shared-memory registry

- Namespace: `codex-task`
- Backing size: 4 MiB (`SHARED_MEMORY_SIZE`)
- Registry record (stored as JSON):
  ```json
  {
    "started_at": "2025-10-19T09:30:59Z",
    "log_id": "8bafb645-1d28-4c73-8718-6da35b9ebd5d",
    "log_path": "C:\\Users\\you\\AppData\\Local\\Temp\\8bafb645-....txt",
    "manager_pid": 12345,
    "cleanup_reason": null
  }
  ```
- On startup we sweep the map, terminating orphaned Codex processes, dropping entries older than 12 hours, and tagging cleanup reasons.

## Environment variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `CODEX_WARDEN_WAIT_INTERVAL_SEC` | Polling interval for `codex-warden wait`. | `30` |
| `CODEX_WARDEN_DEBUG` | Enables stderr debug messages (`true` / `1`). | disabled |

> Legacy compatibility: `CODEX_WORKER_WAIT_INTERVAL_SEC` and `CODEX_WORKER_DEBUG` are still honoured but considered deprecated.

## Working with CLAUDE.MD + SKILL

Our personal `~/.claude/CLAUDE.md` mandates the "`codex CLI + wait`" execution pattern for SKILL workflows such as `/multi-ai-iteration`, `/spec-alignment`, and `/arch-review`. To stay compliant while using this project:

1. **Swap in `codex-warden` wherever the spec says `codex`.** Example: replace `codex exec …` with `codex-warden exec …`.
2. **Pair every launch with `codex-warden wait`.** This fulfils the “CLI + wait” rule while letting the wait command read the shared registry our wrapper maintains.
3. **Integrate into SKILL scripts.** When editing the SKILL definitions or automation scripts referenced in `~/.claude/skills/*.md`, call `codex-warden` directly so the launch/cleanup semantics match the enforced pattern.
4. **Use the logs summarised by wait.** The wait step prints an ordered list of log files (mirroring our CLAUDE spec recommendation to review each run before proceeding).

Following the above keeps the automation fully aligned with the personal CLAUDE instructions, while retaining all SPEC guarantees around process control and logging.

## Development checklist

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

> The project targets the stable toolchain. On Windows, install the *Desktop development with C++* build tools so `link.exe` is available to Rust.

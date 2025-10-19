# codex-warden CLI SPEC

## 背景与范围
- `codex-warden` 是 Codex CLI 在非交互模式（`codex exec …`、`codex exec resume …`）下的透明代理。
- 所有传给 `codex-warden` 的参数和标准输入必须原样传递给 Codex；Codex 的标准输出与标准错误需合并写入临时日志文件，默认不回写到 `codex-warden` 的标准流。
- 依赖 `shared_hashmap` crate，在命名空间 `codex-task` 中维护跨进程共享的 Codex 任务记录。

## 关键目标
1. **透明透传**
   - `codex-warden` 不解析、不修改任何参数；收到什么就传什么，包括未知旗标或子命令。
   - Codex 子进程的标准输入原样对接；其标准输出与标准错误合并并写入日志文件，不直接写回 `codex-warden` 的标准流。
2. **跨进程任务登记**
   - 仅当透传命令是 `codex exec …` 或 `codex exec resume …` 时，才写入共享哈希表。
   - 启动 Codex 前生成全局唯一 ID（UUID 或等效方案），并以该 ID 命名日志文件：`{tmp_dir}/{id}.txt`。
   - 使用子进程 PID（十进制字符串）作为 Key，Value 为 JSON 字符串，至少包含：
     ```json
     {
       "started_at": "ISO8601 UTC",
       "log_id": "uuid",
       "log_path": "C:\\Users\\...\\Temp\\{uuid}.txt"
     }
     ```
   - 其他辅助字段可按需扩展，但必须保持向后兼容。
3. **生命周期维护**
   - 成功登记后，持续监控对应 Codex 子进程；无论正常退出、错误退出或被终止，都要及时删除共享记录，并记录完成任务对应的日志文件路径。
   - `codex-warden` 自身退出（包括接受终止信号）前，必须先结束所管理的 Codex 子进程并清理共享记录。
   - 启动时需遍历 `codex-task`，对每个 PID 检查其进程是否仍存在（Unix 使用 `kill(pid, 0)`，Windows 使用 `OpenProcess` + `GetExitCodeProcess`）；若不存在，或记录已存在超过 12 小时，则删除残留记录，并将此次删除标记为“超时清理”。
   - 若检测到共享表记录对应的 PID 仍在运行但其父进程已不再是 `codex-warden`（例如上一次执行时 worker 异常退出），应主动终止该 Codex 进程并清理记录，避免遗留孤儿任务。
4. **错误反馈**
   - 当 `codex-warden` 在无任何参数的情况下启动时，先执行 `codex --version` 验证 Codex 可用性并获取版本信息；若命令失败（找不到可执行文件或返回非零码），立即向上游输出中文错误说明并以退出码 `1` 结束。
   - 其余情况下直接按透传命令启动 Codex；若子进程启动失败或任何共享哈希表操作失败，必须输出清晰的中文错误信息，并避免留下不一致状态。
   - Codex 子进程退出后，`codex-warden` 必须以相同退出码结束。
5. **可观测性**
   - 默认不向标准输出/错误打印 Codex 日志，所有 Codex STDOUT/STDERR 内容写入临时日志文件。
   - 可通过调试开关（例如环境变量）启用额外日志，记录任务登记/删除、日志文件路径和异常细节，统一写入标准错误。
6. **平台兼容**
   - Windows 平台默认透明转发输出；若检测到标准输出或标准错误连接到控制台，可启用 `ENABLE_VIRTUAL_TERMINAL_PROCESSING` 以确保 ANSI 转义正常显示。
   - 行为必须兼容主流 Windows 与类 Unix 环境。
7. **待命模式（`wait` 命令）**
   - 当 `codex-warden` 被调用为 `codex-warden wait`（无其他参数）时，不启动 Codex，仅进入阻塞轮询流程。
   - 以固定间隔（默认 30 秒，可通过环境变量 `CODEX_WORKER_WAIT_INTERVAL_SEC` 覆盖）读取 `codex-task`。
     - 若共享表为空，则立即退出（退出码 `0`），同时汇总本轮监控期间正常完成的任务数量及其日志文件列表，并输出提示：
       ```
       当前有 N 个任务已完成，详见：
       1. {log_path_1}
       2. {log_path_2}
       ...
       请逐一查看日志并继续后续工作。
       ```
     - 若发现条目持续存在超过 12 小时（根据 Value 时间戳判断），主动删除该条目，视为“超时清理”，并在调试日志中说明；此类条目不会出现在上述汇总列表。
   - 整体阻塞等待时长不得超过 24 小时；达到上限仍未清空共享表时，输出提醒后正常退出（退出码 `0`），附带当前仍未完成任务的 PID 及日志文件路径提示上游后续处理。
8. **异常场景防护**
   - 对 `Ctrl+C`、`SIGTERM`、进程异常退出等场景设置统一清理逻辑：通过信号处理、`Drop` 守卫或 panic hook，确保无论何种退出路径都能终止 Codex 子进程并移除共享记录。
   - 建议在 Windows 上将 Codex 子进程加入同一个 `JobObject`，在类 Unix 环境中通过 `prctl(PR_SET_PDEATHSIG, SIGTERM)` 或独立进程组，让系统在父进程消失时自动向子进程广播结束信号，进一步降低孤儿进程风险。
   - 对上述机制进行专项测试，包括 panic、父进程被 kill、调用端崩溃等情况，验证清理逻辑与共享表一致性。

## 非目标
- 不解释 Codex CLI 的语义或输出内容，仅负责透传与基本可用性检查。
- 不提供 `CODEX_BIN` 等路径覆盖机制，Codex 路径由系统 `PATH` 决定。

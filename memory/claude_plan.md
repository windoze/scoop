## 当前执行计划

1. 读取 `TODO.md`，定位第一个标题未标记为 `[DONE]` 的任务。
2. 读取与该任务直接相关的上下文（必要时包含 `PLAN.md`、相关源码、测试与最新提交信息），确认依赖、验收要求与是否存在直接相关的未完成问题。
3. 在不绕过规范要求的前提下，完整实现当前任务；若出现真实阻塞，则把最小必要前置任务写回 `TODO.md` 并停止。
4. 运行当前任务要求的验证，以及必要的构建、测试、lint，修复发现的问题直到结果稳定。
5. 更新 `memory/claude_plan.md` 记录关键进展，更新 `TODO.md` 的完成状态与完成记录；仅当阶段计划实际变化时更新 `PLAN.md`。
6. 按仓库约定创建一次提交，提交本次任务涉及的全部未提交更改，然后停止，不继续下一个任务。

## 进度记录

- 已写入初始执行计划。
- 已读取 `TODO.md`，确认首个未完成任务为 `P4-T01c-pre1`：为 `P4-T01c` 引入可复用的 sysroot overlay fixture 能力。
- 已检查最新提交信息：`[P4-T01c] Track sysroot overlay prerequisite`，说明该前置已被显式记录到任务顺序中；当前执行目标仍是完成 `P4-T01c-pre1` 本身。
- 已确认实现方案：
  - `scoopc` 侧：`SessionOptions` 携带可选 sysroot overlay 路径；`Session` / `frontend` / build context 都走同一配置；`sysroot` 以“默认 sysroot + 覆盖目录按相对路径替换”的方式合并文件列表。
  - fixture 侧：每个 target 可通过同伴目录 `<target-stem>.sysroot/`（目录 case 则 `<case>.sysroot/`）声明 overlay；runner 为该 target 构造带 overlay 的 session，并在外部 `scoop run` 子进程上传递环境变量，保证 CLI 路径一致。
  - 验证侧：增加一个 `scoopc` owner test，锁定“overlay 的 `core.scoop` 若含 bodied `@Intrinsic class/struct` 会进入 `compilable_source_paths`”；再增加一个 build fixture，锁定 overlay 的 `core.scoop` 会覆盖默认 `Array` 声明，同时未覆盖的其它 sysroot 文件（如 `unsafe.scoop`）仍可见。
- 已完成实现：
  - `SessionOptions` / `Session` / `frontend` / `build` 已支持按 target 传递 sysroot overlay。
  - fixture harness 已支持 companion `.sysroot/` 目录、自动跳过 overlay 目录扫描，并把 overlay 透传到外部 `scoop run` 子进程。
  - `sysroot` 已支持按相对路径合并 overlay，并把“含 bodied `@Intrinsic struct/class` 的 overlay core”自动转入 compilable support sources。
  - 已新增 `scoopc` owner test、fixture helper tests、run-pass env 传播测试，以及一条真实 build fixture + overlay `core.scoop`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc overlay_core_with_bodied_intrinsic_nominal_method_becomes_compilable_support_source -- --nocapture`
  - `cargo test -p scoop sysroot_overlay -- --nocapture`
  - `cargo test -p scoop run_pass_single_pipeline_propagates_sysroot_overlay_env -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/sysroot_overlay_core_array_interface_bridge.scoop`
  - `cargo test -p scoopc llvm_tests -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步：检查 worktree，创建本任务提交，然后停止，不继续 `P4-T01c`。

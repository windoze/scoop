# 执行计划

## 约束与目标

- 本次调用只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在开始实现前，先检查最新提交是否提到需要优先修复的既有问题；如果有，先修复该问题。
- 任何在排查、测试、实现过程中发现的既有缺陷、规格不一致、回归或未完成边界，都必须立即纳入本次范围；若它阻塞当前任务，则需要先修复，或把它作为前置任务插入 `TODO.md` 并停止。
- 需要同步维护 `memory/claude_plan.md`、`PLAN.md`、`TODO.md`，并在完成后提交 git commit。
- 输出与工作记录使用中文。

## 初始步骤

1. 查看最新一次 git 提交信息，确认是否显式提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解已有计划与任务依赖。
4. 结合代码现状判断该任务是否能在一次调用中完整完成。
5. 如果任务过大，则把它拆成更小的前置子任务，更新 `PLAN.md` 与 `TODO.md`，提交后停止。
6. 如果任务可执行，则开始实现。

## 实现阶段计划

1. 阅读与该任务直接相关的代码、测试、规范或文档。
2. 先识别是否存在阻塞性的既有问题：
   - 若存在且可直接修复，则先修复它，再继续当前任务。
   - 若存在且无法在本次直接修复，则将其整理为当前任务的前置任务，更新 `TODO.md` 与 `PLAN.md`，提交后停止。
3. 按最小必要改动完成任务实现，避免引入与任务无关的变更。
4. 为新增行为补充或更新测试。
5. 运行相关测试，再运行必要的质量检查（至少包括与改动相关的测试；若范围允许，运行 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`）。
6. 修复测试、lint、编译中暴露的所有问题，直到结果稳定。

## 收尾步骤

1. 更新 `memory/claude_plan.md`，记录关键发现、计划调整、已完成步骤与验证结果。
2. 将已完成任务在 `TODO.md` 中标记完成。
3. 更新 `PLAN.md`，反映当前状态和后续依赖变化。
4. 检查工作区变更，确认未误改无关内容。
5. 提交 git commit，提交信息使用清晰的任务描述格式。
6. 停止，不继续处理下一个任务。

## 进度记录

- 初始计划已写入，等待开始仓库检查与任务确认。
- 已检查最新提交 `6cefe504e20bc10eb53a7fdcf32ef3185046f290`，提交标题为 `[T5000e3c] Expand main argv contract and remove scoop.process`；未发现提交说明中额外要求先修复的独立既有问题。
- 已阅读 `TODO.md` / `PLAN.md`，确认当前首个未完成任务为 `T5000e3cR Review：确认 entry-point argv contract 已替代临时 scoop.process surface`。

## 当前任务：T5000e3cR Review

### 审计目标

1. 复核 `main` 合法签名是否稳定收口为：
   - `fun main(): Unit / Pure!`
   - `fun main(): Int / Pure!`
   - `fun main(args: Array<String>): Unit / Pure!`
   - `fun main(args: Array<String>): Int / Pure!`
2. 复核 runtime / entry lowering 是否把完整 `argv`（含 `argv[0]`）直接传入 `main(args)`。
3. 复核 `Unit` / `Int` 返回的正常退出码映射是否符合 contract。
4. 复核 `scoop.process` sysroot surface、相关 runtime/lowering/fixture 是否已移除。
5. 复核 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`README.md` 与相关 fixtures/golden 是否已同步。

### 预定执行步骤

1. 检索并阅读 entry-point typecheck、HIR lowering、LLVM emit/runtime 相关实现。
2. 全文检索 `scoop.process`、`process.args`、`process.exit`、`sysroot/process.scoop` 等残留。
3. 运行与该 contract 最相关的 fixture / 测试 / lint。
4. 若发现既有问题，立即修复并补测试；若未发现，则只更新计划文档、任务状态并提交 review 结果。

### 当前发现

- 代码实现面初步复核通过：
  - typecheck 入口签名校验已收口为零参数或单个 `Array<String>` 参数，返回类型仅允许 `Unit` / `Int`；
  - LLVM 入口 `main(argc, argv)` 已通过 `scoop_entry_argv_array(argc, argv)` 把完整 native argv 注入 `main(args)`；
  - `codegen_main_exit_code` 已将正常返回 `Unit` 映射为 `0`、正常返回 `Int` 映射为返回值本身。
- 发现需要立即修复的既有文档/注释问题：
  1. `STDLIB_COMPLETENESS.md` 仍把 `scoop.process` 记为当前 `DONE` surface；
  2. `PLATFORM_API_SURFACE_AUDIT.md` 仍把 `scoop.process` 列为现行平台模块；
  3. `README.md` 尚未写出新的 executable `main` argv / exit-code contract；
  4. `STDLIB_DESIGN.md` 中 `scoop.process` 条目需要明确它是 future target，而非当前 shipped surface；
  5. `crates/scoopc/src/typecheck/expr/stmt.rs` 里关于 entry-point effect row 的注释仍口误写成 `Pure`。
- 已开始修复上述问题，并补一条 LLVM IR 单测，直接锁定 `main(args)` 接入 `scoop_entry_argv_array` 的入口路径。

### 已完成验证

- `cargo fmt --all`
- `cargo test -p scoopc minimal_main_ir_ -- --nocapture`
- `cargo run -p scoop -- run tests/fixtures/run-pass/std_process_args_exit_basic.scoop -- foo bar`
  - stdout 为：
    - `3`
    - `true`
    - `foo`
    - `bar`
- `cargo run -p scoop -- run tests/fixtures/run-pass/entry_main_args_int_exit_basic.scoop -- foo bar`
  - 退出码为 `3`
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `fixtures: ok (395)`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

### 当前结论

- `T5000e3cR` 已可标记完成。
- 已确认 entry-point argv / exit-code contract 取代了临时 `scoop.process` surface。
- 已修复本轮发现的文档/注释残留，并新增 LLVM IR 回归测试。
- 下一步仅剩收尾：更新 `TODO.md` / `PLAN.md`、检查 diff、提交 git commit。

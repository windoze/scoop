# 执行计划 / 决策记录

更新时间：2026-04-19

说明：
- 按用户要求，在开始仓库检查前先创建本文件。
- 这里记录的是可公开的执行计划、关键判断依据、进度和后续调整，不包含逐字逐句的内部推理。
- 在执行过程中，如计划变化、发现阻塞、完成关键步骤或调整任务顺序，会持续更新本文件。

初始计划：
1. 检查最新一次 Git 提交，确认提交信息中是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如果首个未完成任务过大或存在前置缺口，拆分任务并同步更新 `PLAN.md` 与 `TODO.md`，保证依赖顺序正确。
4. 实现当前应执行的首个任务或子任务。
5. 运行与改动相关的测试，并补充必要测试；同时执行格式化、lint 与无警告检查。
6. 更新文档与任务状态：
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 持续更新本文件
7. 以清晰的提交信息提交本轮改动。
8. 停止，不继续处理下一个任务。

执行约束：
- 不使用变通方案、兼容性垫片、仅为夹具通过而做的 hack。
- 如果发现规格缺口、编译器/运行时/标准库 bug、诊断不符合预期或任何阻塞当前任务的实现边界，必须先在 `TODO.md` / `PLAN.md` 中显式建模该问题，再决定是否停止本轮。
- 不回退用户已有改动；若工作树中存在无关变更，仅在不冲突时与之共存。
- 目标是一次只完成 `TODO.md` 中当前优先级最高的一个任务。

待确认信息：
- 最新提交是否包含需要优先修复的问题。
- `TODO.md` 中首个未完成任务的具体内容与依赖。
- 当前工作树是否干净，以及是否存在会影响本轮任务的未提交改动。

阶段性更新（仓库检查后）：
- 已检查最新提交 `93d3e58ed018fa1ba5591af703203be4425f9bd0`，提交信息为 `[T4008b1a] Add direct resumed-step effect summary`，未额外点名需要先修复的既有 issue。
- 当前工作树只有本文件 `memory/claude_plan.md` 的未提交修改，属于本轮要求的进度记录。
- 已确认 `TODO.md` 中按顺序的首个未完成条目是 `T4008b`，其中当前应执行的最前子任务为 `T4008b1b`：
  - 任务：为 resumed-step 补齐 arm body / `finally` / nested handle / hidden boundary 语义。
  - 依赖：`T4008b1a` 已完成。
- 已检查 `PLAN.md`，当前计划与 `TODO.md` 一致，明确下一项就是 `T4008b1b`，暂时不需要进一步拆分。

当前执行计划（细化）：
1. 阅读 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 以及相关测试，确认 direct-step summary 当前覆盖范围和缺口。
2. 为以下边界建立精确语义，并保持与现有 state-machine active/inactive 行为一致：
   - handle arm body（含 non-resuming / immediate-resume 场景）
   - `finally`
   - nested handle
   - 顶层 / 对象 once-init 等隐藏边界
3. 添加或更新回归测试，直接覆盖上述复杂 resumed-step 场景。
4. 运行定向测试；若通过，再运行更大范围验证（至少 `cargo test --all`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`）。
5. 完成后更新 `TODO.md`、`PLAN.md` 与本文件，并提交本轮改动。

阶段性更新（实现与验证后）：
- `T4008b1b` 已实现完成，未再发现需要插入到其前面的新 blocker。
- 实现位置：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
- 实现要点：
  - resumed-step summary 现在显式区分 current handle 仍处于 active body 与 arm body / `finally` / hidden init 等 inactive 区域。
  - active boundary 发出的 effect 会重新按当前 handle 的 dispatch 语义处理；inactive 区域中的 effect 不会再被误算成“仍由当前 handle 处理”。
  - 逃逸 continuation 的 direct-step API 现可覆盖：
    - immediate-resume arm body
    - 下一次 escape arm body
    - `finally`
    - nested handle boundary
    - hidden once-init boundary（顶层 immutable value / object init）
  - 为 hidden boundary summary 新增了 program side table 输入，用于读取 top-level immutable value 与 object init 的初始化步骤。
- 新增回归：
  - Rust 单测总计新增 5 条，连同原有 2 条 `direct_step_` 单测共同覆盖 `T4008b1b` 语义。
- 已完成验证：
  - `cargo test -p scoopc direct_step_`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1055)`）
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt`
- 待执行的收尾动作：
  1. 已更新 `TODO.md` 把 `T4008b1b` 标记为完成。
  2. 已更新 `PLAN.md` 把下一项切换为 `T4008b2`。
  3. 下一步只剩检查 diff、提交本轮改动并停止。

# 执行计划

## 约束说明

- 不写入内部完整推理细节；这里记录可审计的执行计划、关键决策与进度更新。
- 本次调用只处理 `TODO.md` 中第一个未完成任务；完成后更新文档、验证并提交，然后停止。

## 初始步骤

1. 读取 `TODO.md`，识别第一个标题未带 `[DONE]` 的任务。
2. 读取与该任务直接相关的上下文：必要时查看 `PLAN.md`、相关源码、测试、最近提交信息。
3. 判断是否存在阻塞当前任务的直接缺陷、缺失特性或最近提交中与该任务直接相关的未完成事项。
4. 若无阻塞，直接实现当前任务；若有阻塞，按要求在 `TODO.md` 中插入最小前置任务并停止。
5. 运行当前任务要求的验证，以及必要的 `cargo fmt`、相关测试，必要时运行 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `memory/claude_plan.md` 记录关键进展。
7. 更新 `TODO.md`：仅在任务真正完成时给标题加 `[DONE]` 并填写完成记录；如任务拆分或重排，保持其为唯一事实来源。
8. 仅在阶段计划确有变化时更新 `PLAN.md`。
9. 检查工作树，按要求提交所有相关修改，提交信息使用当前任务 ID。

## 进度

- 已创建本计划文件。
- 已读取 `TODO.md` 并确认当前首个未完成任务为 `P6-T03`：重写 `PIPELINE_GAPS.md`、active inventory 与 fixtures 到最终状态。
- 已检查最新提交；`[P6-T02] Sync frontend reject surfaces` 未记录与 `P6-T03` 直接相关的额外未完事项。

## 当前执行细化

1. 读取 `PLAN.md` 的 P6 段落与 `PIPELINE_GAPS.md` 当前全文，确认本任务需要收口的最终分类边界。
2. 审查以下 active 入口的现状与不一致项：
   - `crates/scoopc/src/mir/placeholder_inventory.rs`
   - `crates/scoopc/src/hir/lower/placeholder_inventory.rs`
   - `crates/scoopc/src/llvm/codegen_gap_inventory.rs`
   - `crates/scoopc/src/llvm/tests.rs`
   - `crates/scoopc/src/pipeline/mir_stage.rs`
   - `tests/fixtures/**` 中仍依赖 legacy reason / stale blocker / old unsupported trigger 的位置
3. 先确认是否存在阻塞 `P6-T03` 的真实实现缺口；若没有，再同步改写文档、inventory、fixtures 与断言。
4. 运行任务要求的验证命令与必要附加检查。
5. 回写 `TODO.md` 完成记录并提交。

## 当前判断

- 未发现需要为 `P6-T03` 新增前置任务的实现 blocker。
- 主要不一致集中在 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`：仍保留一批已经关闭且只剩 regression coverage 的条目，以及少量 stale owner / stale trigger。
- 预计改动：
  1. 精简 `codegen_gap_inventory.rs`，移除 regression-only / closed-id 残留条目。
  2. 更新仍保留 guard 条目的 owner / trigger。
  3. 同步 `pipeline_gap_audit.rs` 基线。
  4. 重写 `PIPELINE_GAPS.md` 中关于 active inventory 与 regression coverage 的最终说明。
  5. 刷新少量仍带旧 blocker 叙述的 fixture 头注释。

## 已完成的关键步骤

- 已精简 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`，移除 regression-only closed ids，并把剩余 active guard 的 owner/trigger 收口到当前任务线。
- 已同步 `crates/scoopc/src/pipeline_gap_audit.rs` 与 `PIPELINE_GAPS.md`，让 active inventory / 文档对“哪些编号仍保留 executable guard 语义”给出一致结论。
- 已刷新相关 fixture 注释，移除旧 CG blocker 叙述。
- 已通过以下定向验证：
  - `cargo test -p scoopc codegen_gap_inventory`
  - `cargo test -p scoopc pipeline_gap_audit`
  - `cargo test -p scoopc refactor_mir_placeholder_inventory`

## 剩余步骤

1. 已运行 `cargo test -p scoopc llvm_tests` 与 `cargo clippy --all-targets -- -D warnings`；前者命中过滤为 0 tests，后者通过。
2. `cargo run -p scoop -- test` 失败，当前出现 31 个 fixture blocker；已抽样确认其中至少一类真实根因为 typed call-site / effect-facts contract 缺失，而不是本次文档改写造成的偶发噪音。

## 当前 blocker

- `receiver_function_value_call_basic.scoop`：`call expression missing typed call-site contract`。
- `std_process_args_exit_basic.scoop`：`call expression missing typed call-site contract`。
- `string_trim_indent_basic.scoop`：effect-facts 无法为 `scoop.core.trimIndent` 构建 surface contract。
- `delegated_property_lazy_init_once_basic.scoop`：`typed call-site contract missing before MIR lowering` panic，命中 `scoop.sync.lock`。
- `mir_refactor/call_contracts.scoop` 与 `effect_lowered/dropped_continuation_abandons_remaining_work.scoop` 还额外暴露 snapshot drift。

## 处理决定

- 已在 `TODO.md` 中于 `P6-T03` 前新增前置任务 `P6-T02A`，专门修复 full regression 暴露的 typed call-site / effect-facts contract 发布回归。
- `P6-T03` 保持未完成，并改为依赖 `P6-T02A`。
- 当前调用到此停止：整理工作树后提交 blocker/task-list 更新与已完成的 inventory/doc groundwork。

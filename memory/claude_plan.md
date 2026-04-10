# 执行计划

## 约束说明

用户要求在执行任何代码或命令前，先把本次任务的思路与执行步骤写入本文件。这里记录的是可审计的高层计划、判断标准与进度更新，不包含冗长的内部推理细节。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交内容，确认是否提到了已知问题、遗留缺陷或需要先修复的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如首个未完成任务过大或依赖不清：
   - 将任务拆分成更小的可执行子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把拆分后的子任务放到正确优先级位置；
   - 本轮只执行拆分后排在最前的那个子任务。
4. 在充分理解相关代码、测试和文档后，实施该任务。
5. 运行必要的格式化、测试和质量检查，至少覆盖与变更直接相关的范围；若任务影响面较大，则补充更全面检查，包括：
   - `cargo fmt --check` 或必要时 `cargo fmt`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
6. 若发现问题，先修复问题并重新验证。
7. 更新进度文档：
   - 在 `TODO.md` 中将本轮完成的任务标记为完成；
   - 在 `PLAN.md` 中记录状态、结果和后续影响；
   - 视需要继续更新本文件，记录关键步骤完成情况或计划调整。
8. 使用清晰的提交信息提交本轮变更。
9. 停止，不继续处理下一个任务。

## 任务分解判断标准

如果首个未完成任务满足以下任一条件，则认为需要拆分：

- 涉及多个彼此独立的子系统；
- 需要跨越多个大模块并伴随明显设计决策；
- 预期无法在一次提交中稳定实现、测试并验证；
- 缺少前置基础设施，必须先补依赖项。

## 风险与处理原则

- 不回退或覆盖非本轮目标之外的已有改动。
- 若遇到无法直接完成的任务，不将其标记为阻塞，而是按依赖顺序重排 `TODO.md`，并在 `PLAN.md` 说明原因。
- 如果最新提交中已经明确指出遗留问题，这些问题优先于 `TODO.md` 当前任务。

## 当前判断

- 最新提交：`8513d0e [T0147c-3b] 收缩 ExprTypeError 超大 variant`。
- 该提交消息本身未附带额外“需先修复”的遗留问题说明。
- `TODO.md` 中当前第一个 `[TODO]` 为 `T0147c-3c`：清零剩余结构性 clippy warning，并恢复严格 gate。
- 结合 `TODO.md` / `PLAN.md` 上下文，当前工作应先以严格 `cargo clippy --workspace --all-targets -- -D warnings` 的实际输出为准，确认剩余 lint 的精确分布，再决定是否需要对任务进行进一步拆分。
- 严格 clippy 已执行；当前剩余失败共 32 类（lib），集中在：
  - `private_interfaces`：`llvm/codegen/mod.rs`
  - `dead_code`：`llvm/codegen/mod.rs` / `layout.rs` / `runtime_abi.rs`
  - `large_enum_variant`：`ast/mod.rs`（2 处）
  - `vec_init_then_push`：`cone/archive.rs`
  - `redundant_locals`、`question_mark`：`hir/lower/expr.rs`、`resolve/scopes.rs`、`typecheck/expr/call.rs`、`typecheck/expr/ops.rs`
  - `type_complexity`：`parser/decls.rs`、`rtti/type_desc.rs`、`typecheck/annotations.rs`、`typecheck/expr/entry.rs`
  - `if_same_then_else`：`parser/decls.rs` / `parser/stmt.rs`
  - `while_let_loop`：`parser/expr.rs`
  - `cloned_ref_to_slice_refs`：`cone/scoopir/tests.rs`
  - `nonminimal_bool`：`typecheck/expr/infer.rs` / `ops.rs`
  - `unnecessary_get_then_check`：`resolve/imports.rs`
  - `doc_lazy_continuation`：`typecheck/mod.rs`

## 本轮细化执行方案

1. 运行严格 clippy，记录当前全部失败项。
2. 按失败类别与模块归类，判断能否在一次提交内完成：
   - 若可完成：直接修复全部剩余 warning；
   - 若范围明显过大：将 `T0147c-3c` 拆分为更小的子任务，并更新 `PLAN.md` / `TODO.md`。
3. 对代码进行最小语义改动的 lint 修复，避免引入行为变化。
4. 运行格式化、测试与严格 clippy 复核。
5. 更新 `TODO.md` / `PLAN.md` / 本文件。
6. 提交 Git commit，然后停止。

## 进度记录

- 已完成：初始化本计划文件。
- 已完成：检查最新提交并确认未声明额外遗留问题。
- 已完成：定位当前首个未完成任务为 `T0147c-3c`。
- 已完成：执行严格 clippy，收集剩余 warning 清单。
- 当前决定：不拆分 `T0147c-3c`，直接在本轮清零全部剩余结构性 warning。
- 已完成：逐类修复 clippy 剩余失败项。
- 已完成：将 `ast::Item` / `ast::TypeMember` 的大 payload variant 改为装箱表示，消除 `large_enum_variant`。
- 已完成：`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 全量通过。
- 已完成：`cargo test --all` 通过。
- 已完成：`cargo run -p scoop -- test` 通过（`fixtures: ok (852)`）。
- 已完成：回写 `TODO.md` / `PLAN.md`，将 `T0147c-3c` 与父任务 `T0147c-3` 标记为完成。
- 已完成：确认 `TODO.md` 的下一个首个未完成任务已前移为 `T0147c`。
- 进行中：整理提交并准备结束本轮。

# 当前执行计划

说明：按要求先记录执行计划；这里记录的是可审计的工作计划、决策依据摘要和进度更新，不包含冗长的内部推理草稿。后续只要计划变化、发现阻塞、完成关键步骤，都会继续更新本文件。

## 初始目标

本轮只处理 `TODO.md` 中第一个未完成任务；如果在执行前或执行中发现已有缺陷、回归、规范不匹配或最新提交中提到的遗留问题，则先修复该问题，或在 `TODO.md` / `PLAN.md` 中插入其前置任务后停止。

## 初始步骤

1. 检查最新一次 git 提交，确认是否提到需先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划、任务依赖和可能的任务拆分空间。
4. 如果首个未完成任务过大，先拆分任务并更新 `TODO.md` 与 `PLAN.md`，然后仅执行拆分后的第一个子任务。
5. 阅读与该任务直接相关的代码、测试、规范和最近改动，确认现状及潜在既有问题。
6. 实现任务；若中途发现既有缺陷或规范不匹配，优先修复或把前置修复任务插入 `TODO.md` 后停止。
7. 运行相关测试，并补足必要测试；同时运行格式化、lint 或其他必要校验，确保没有新增告警。
8. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成情况或阻塞原因。
9. 以清晰提交信息提交本轮变更，然后停止。

## 进度

- 已完成：写入初始执行计划。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`。
  - 最新提交 `e0a16af2 [T5000b2R] Review MainCodegen construction boundary` 只包含 review 结论，没有额外声明需先修的遗留问题。
  - 当前首个未完成任务是 `T5000b3 按主题拆分 llvm/codegen/mod.rs 的独立 lowering 模块`。
- 新发现：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 当前仍有 17671 行。
  - 已识别出至少四组稳定函数簇：
    1. call dispatch / callable ABI / extern-native / vtable-itable / funptr / resume 边界；
    2. sysroot / builtin intrinsics；
    3. closure / class ctor；
    4. enum lowering / object init。
  - 原始 `T5000b3` 对单轮而言过大，应先拆成子任务，避免机械大搬家。
- 计划调整：
  1. 先把 `T5000b3` 拆成若干按主题排列的实现子任务和对应 review 任务，更新 `TODO.md` 与 `PLAN.md`。
  2. 本轮只执行拆分后的第一个子任务，优先处理 `call/` lowering 边界。
  3. 完成后运行相关测试、更新任务状态并提交。
- 下一步：回写任务拆分，然后开始实现新的第一个子任务。
# 2026-04-25 本轮续作计划（T5000b3a 收尾）

## 当前已知状态

- 最新提交 `e0a16af2 [T5000b2R] Review MainCodegen construction boundary` 的提交信息未声明需要先修的遗留问题。
- `TODO.md` / `PLAN.md` 已经把原 `T5000b3` 拆成 `T5000b3a` 到 `T5000b3d`，本轮目标是第一个未完成子任务 `T5000b3a`。
- `T5000b3a` 的主体代码已经完成：`crates/scoopc/src/llvm/codegen/mod.rs` 中的 call 主题逻辑已迁移到 `crates/scoopc/src/llvm/codegen/call/` 下的 `abi.rs`、`dispatch.rs`、`resume.rs`，`mod.rs` 中保留薄委托入口。
- 已知中途修复过两个重构边界问题：
  - `call/resume.rs` 缺少 `LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE` 导入。
  - `*_impl` 可见性过宽触发 `private_interfaces` warning，已收紧为 `pub(in crate::llvm::codegen)`。
- 已确认通过的验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
- 尚未最终确认：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 本轮执行计划

1. 先检查当前工作树状态，确认前一轮改动仍在，且没有新的意外冲突。
2. 运行完整验证：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
3. 如果验证暴露任何既有问题、warning 或回归，立即修复；修复后重新运行受影响验证，直到无 warning、无失败。
4. 若验证全部通过，检查 `codegen/mod.rs` 与新 `call/` 模块边界，确认本轮任务确实完成的是 call 主题拆分，而非引入新的混杂边界。
5. 更新项目记录：
   - 在 `TODO.md` 将 `T5000b3a` 标记完成
   - 在 `PLAN.md` 记录 `call/` 拆分完成与验证结果
   - 在本文件记录关键步骤与最终结果
6. 使用清晰提交信息提交本轮改动，然后停止，不继续处理 `T5000b3aR` 或后续任务。

## 约束提醒

- 只能完成一个任务：`T5000b3a`。
- 若遇到阻塞该任务的既有问题，必须先修复或把前置任务插入 `TODO.md` 后停止。
- 不允许带 warning 提交；`cargo clippy --all-targets -- -D warnings` 必须通过。

## 本轮执行进度更新

- 已检查工作树状态：当前未提交改动与本轮 `T5000b3a` 相关，未发现新的意外冲突文件。
- 已完成完整验证：
  - `cargo test --all`：通过；
  - `cargo clippy --all-targets -- -D warnings`：通过。
- 已完成边界复核：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中保留 `codegen_call`、`codegen_top_level_fun_call`、`emit_extern_native_call`、`try_codegen_class_vtable_call`、`codegen_funptr_value_call`、`declare_callee_resume_entry_function`、`codegen_callee_resume_dispatch` 等原入口名，但主体实现已迁到 `call/dispatch.rs`、`call/abi.rs`、`call/resume.rs` 的 `*_impl`；
  - `codegen/mod.rs` 当前降到 14972 行，`call/` 目录内部按 `dispatch` / `abi` / `resume` 三组职责分层，符合本轮的“按主题收口 call lowering 边界”目标。
- 已完成文档回写：
  - `TODO.md` 已将 `T5000b3a` 标记为 `[DONE]` 并补充完成记录；
  - `PLAN.md` 已记录 `T5000b3a` 的实现结果、验证结果，并将下一条待执行任务切换为 `T5000b3aR`。

## 待收尾步骤

1. 生成本轮变更的 git 提交。
2. 提交后停止，不继续处理 `T5000b3aR`。

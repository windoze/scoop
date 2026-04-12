# 本轮执行计划

## 说明

本文件记录本轮可审阅的执行计划、决策依据摘要、进展与结果。这里不会写入内部推理原文，但会尽量完整记录外显的检查步骤、判断条件、实施方案与后续动作，便于中途审查。

## 初始目标

按照 `TODO.md` 的顺序，只完成第一个未完成任务；如果发现被前置缺陷阻塞，则先把该缺陷整理为更靠前的任务并更新计划、提交后停止。

## 初始执行步骤

1. 在不改动业务代码之前，先检查最新一次 Git 提交：
   - 查看提交标题与正文是否提到已知问题、回归、后续修复项或未完成部分。
   - 如果最新提交明确提到遗留问题，则先定位并修复这些问题，再继续 `TODO.md` 的任务流。
2. 读取 `TODO.md` 与 `PLAN.md`：
   - 定位第一个未完成任务。
   - 判断该任务是否足够小且边界清晰，能在一轮内完整实现、测试、记录并提交。
3. 如果任务过大或依赖未满足：
   - 将任务拆分成更小子任务并更新 `PLAN.md`。
   - 调整 `TODO.md` 顺序，使最前面的未完成项成为当前真正可执行的前置子任务。
   - 本轮只处理新的第一个未完成项。
4. 实现当前目标：
   - 先读相关代码、测试、规范和现有实现边界。
   - 修改代码时优先保持模块职责清晰，避免引入临时兼容层、fixture-only hack 或偏离规范的行为。
   - 若过程中暴露规范缺口、实现缺口或现有 bug，按要求转化为更靠前的任务，而不是绕过。
5. 测试与质量保证：
   - 运行与当前改动直接相关的测试。
   - 视影响范围补充更高层验证。
   - 最终至少尝试运行格式化、测试，以及 `cargo clippy --all-targets -- -D warnings`；若受环境或耗时限制无法完成，会在本文件和最终说明中明确记录。
6. 文档与提交：
   - 更新 `TODO.md`、`PLAN.md`，记录已完成项或依赖调整。
   - 同步更新本文件的进展状态。
   - 使用清晰的 Git 提交信息提交。
   - 提交后停止，不继续处理下一个任务。

## 风险与分支处理

- 如果最新提交提到的问题无法在本轮直接修复，则需要先把该问题显式落入任务列表并前置。
- 如果第一个未完成任务依赖缺失的语言特性、运行时行为或规范修复，则不能做规避实现，必须先补依赖任务。
- 如果仓库已有未提交修改，会先确认是否与当前任务冲突；不主动回滚非本轮改动。

## 进展记录

- 已创建本文件并写入初始执行计划。
- 已检查最新提交：最近一次提交为 `[T2003c0c2b3c2-1] Modularize LLVM effect codegen`，提交说明未显式挂出需要先修的遗留 bug。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T2003c0c2b3c2-2`：抽取 effect site 扫描与 used-local/capture 分析 helper。
- 已做复杂度评估：当前任务边界清晰，暂不需要继续拆分为更小子任务；本轮目标是完成这一整项重构并跑回归。
- 已完成实现：
  - `effect/scan.rs` 新增共享的 path-state 扫描骨架 `scan_stmt_slice_with_state` / `with_scoped_scan_frame`。
  - `scan_immediate_resume_site`、`scan_mixed_escape_direct_sites`、`scan_mixed_escape_indirect_sites` 已改为复用共享扫描脚手架，而不是各自维护一套 stmt index 更新与 frame push/pop 逻辑。
  - `effect/scan.rs` 现统一提供 `collect_used_locals_in_block_static`、`collect_used_locals_in_call_args_static`、`collect_used_locals_in_handle_static`，并补齐 `perform`、`handle`、closure captures 等静态分析覆盖。
  - `escape_continuation.rs` 与 `mixed.rs` 中本地内嵌的 `collect_used_locals_in_(block|stmt|expr)` 已删除，统一复用共享 helper。
- 已完成验证：
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已完成文档同步：`TODO.md` 已将 `T2003c0c2b3c2-2` 标记为完成，`PLAN.md` 已把当前下一步调整到 `T2003c0c2b3c2-3`。
- 当前剩余动作：检查最终 diff、提交本轮改动并停止。

## 当前实施方案

1. 先收口共享的 used-local 递归分析：
   - 检查 `scan.rs`、`escape_continuation.rs`、`mixed.rs` 内各自的 `collect_used_locals_in_(block|stmt|expr)` 实现差异。
   - 以 `scan.rs` 为共享入口，补齐缺失的 HIR 形态（例如 `perform`、`handle`、`block` helper 等），然后让另外两处复用这一套共享实现。
2. 再收口 effect site 扫描的共享骨架：
   - 实际落地时没有强行合并 frame enum，而是优先抽取共享的 path-state helper，让 immediate-resume / mixed direct / mixed indirect 三套 scanner 先共用 stmt 遍历、当前 frame stmt_idx 更新、以及嵌套 frame push/pop 骨架。
   - 这样可以在不大范围改动 `matrix.rs` / lowering 消费端类型的前提下，先收掉扫描层的重复逻辑，并保持诊断边界稳定。
3. 改完后执行：
   - `cargo fmt --all --check`
   - `cargo test --all`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
4. 通过后更新 `TODO.md`、`PLAN.md`、本文件，并提交本轮改动。

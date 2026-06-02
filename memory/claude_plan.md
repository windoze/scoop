# 执行计划

## 当前状态

- 本次调用目标：按 `TODO.md` 顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 约束：不跳过 review 任务，不做开放式历史问题清扫；若发现阻塞当前任务的缺口，先修复或在 `TODO.md` 中插入最小 prerequisite 并提交后停止。
- 测试策略：先格式化，再 clippy，再按任务要求和变更范围运行相关测试；若代码有变更且任务完成，按要求运行完整 Rust 测试与 fixture 套件，除非确认仅文档变更且可复用既有绿色结果。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录任务编号、要求、依赖、验证要求。
2. 检查最新提交信息是否明确提到与该任务直接相关的未完成问题；仅在直接相关时纳入当前任务或加入 prerequisite。
3. 按任务要求检查相关代码、测试、fixture 和文档，避免先做无关历史问题清扫。
4. 若任务可直接完成，实施最小正确改动；若发现必须先修的具体 blocker，更新 `TODO.md` 插入最小 prerequisite，必要时更新 `PLAN.md`，提交后停止。
5. 对改动运行格式化、lint、相关测试以及任务要求的完整验证；对任何未被明确排期的失败测试/fixture，修复或在 `TODO.md` 中排期到当前任务完成之前。
6. 任务完成后，在 `TODO.md` 标题加 `[DONE]` 并更新 completion record；仅当阶段计划实际变化时更新 `PLAN.md`。
7. 检查 git 状态与 diff，提交所有本次任务相关变更，提交信息使用任务编号开头。
8. 停止，不继续下一个任务。

## 进度记录

- 已写入初始计划，下一步读取 `TODO.md` 定位第一个未完成任务。
- 已读取 `TODO.md` 与 `TODO-3.md`：当前第一个未完成任务为 `T3-04B`（收口 source-span / fallback / verifier / gate 残余缺口），依赖 `T3-04B0` 已完成；下一步检查最新提交是否有与 `T3-04B` 直接相关的未完成问题。
- 最新提交为 `[T3-04B0] Publish LIR source call-site identity`，未在提交说明中声明额外未完成问题；`T3-04B0` 是当前任务直接前置，继续执行 `T3-04B`。
- 定向审查确认当前实现仍有四类 `T3-04B` 残留：P6 `LlvmIntrinsicCallContract` source-span handoff、LIR intrinsic/readable_path fallback、LLVM dispatch side table、verifier owner/target 校验不足。执行顺序调整为先删除 P6 source-span intrinsic handoff，再收口 LIR fact builder/verifier，最后更新 gate 与验证。
- 已完成核心改动：删除 P6 source-span intrinsic handoff；LIR intrinsic facts 改为消费显式发布 metadata；P6 dispatch 改走 LIR `dispatches`/`physical_layout`；补充 LIR verifier 对 body-version/continuation owner 的自包含检查；dependency gate 已加入对应残留守卫并通过。
- `cargo fmt`、`cargo clippy --all-targets -- -D warnings` 和 `python3 tools/dependency_gate.py` 已通过；下一步运行完整 Rust 测试与 fixture suite。
- 完整 Rust 测试 `cargo test --all --all-targets` 已通过；下一步运行完整 fixture suite。
- 完整 fixture suite `python3 tools/run_fixtures.py` 已通过（1664 checks）。`TODO-3.md` 已将 `T3-04B` 标记为 `[DONE]` 并写入完成记录，`TODO.md` 当前活跃任务已更新为后续 `T3-04R`；下一步检查 diff 并提交本次任务变更。

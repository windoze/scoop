# Claude Execution Plan

本文件记录本次调用的可审查执行计划与进度更新；不包含私密逐步推理。

## 当前任务

`P7-B3.5：B-30 named/unsafe/FunPtr/stackmap intrinsic contract`

## 执行计划

1. 确认 `TODO.md` 中首个未完成任务为 P7-B3.5，并检查最近提交是否明确留下与 B-30 直接相关的未完成问题。
2. 阅读 B-30 的 audit category、strategy、fixture README/index 和 active inventory rows，锁定 117 个待 retire `UMB-NNNN` 的具体 source 位置。
3. 检查 named intrinsic、unsafe/FunPtr、uintPtr/funptr conversion、stackmap/statepoint 相关 codegen/typecheck/MIR contract，判断是否存在必须先记录的 spec blocker。
4. 若不存在 blocker，按 B-30 的 `InternalBugSentinel` 路径补齐签名/receiver/arity/value/return/stackmap statepoint contract；删除对应 `UnsupportedMainBody` fallback，改为 verifier 后的内部 invariant、`expect_*` 或明确 panic boundary。
5. 同步更新 `audit/UMB_inventory.csv`、`audit/UMB_retired.csv`、B-30 category/strategy/overview/spec matrix、fixture coverage/index 和 `pipeline_user_visible_failure_policy` stale count。
6. 运行要求验证：`cargo test -p scoopc audit:: -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/B-30-named-unsafe-funptr/`、`cargo run -p scoop -- test tests/fixtures/unsafe_nogc/`。
7. 运行必要补充验证：`cargo run -p scoopc --bin umb-audit -- list --bucket B-30`、`cargo run -p scoopc --bin umb-audit -- diff`、`cargo run -p scoopc --bin umb-audit -- stats`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
8. 更新 `TODO.md`：将 P7-B3.5 标记 `[DONE]` 并填写完成记录；如阶段计划未变化，不更新 `PLAN.md`。
9. 检查 git 状态和 diff，提交本任务相关所有改动，然后停止。

## 进度

- 已读取 `TODO.md`，首个未完成任务为 `P7-B3.5：B-30 named/unsafe/FunPtr/stackmap intrinsic contract`。
- 已写入本次 P7-B3.5 执行计划；下一步检查最近提交与工作区状态，仅纳入与 B-30 直接相关的问题。
- 最近提交为 `[P7-B3.4] Retire GC intrinsic UMB rows`，未发现明确指向 B-30 的未完成项；工作区初始仅有本计划文件改动。
- 已定位 B-30 的 117 个 active rows，覆盖 named/runtime intrinsic、builtin scalar/String helper、FunPtr HIR/MIR call、uintPtr/funptr conversion 与 stackmap smoke helper。
- 已将 B-30 相关 LLVM fallback 改为现有 typecheck/MIR/intrinsic contract 之后的内部 invariant、`expect_*` 或 `panic_verified_intrinsic_contract`；保留 B-10/B-12/B-13/B-24 等后续任务 rows 不动。
- `cargo check -p scoopc` 已通过；下一步同步 inventory/retired ledger、fixture coverage、stale count 和文档。
- 已同步 `audit/UMB_retired.csv`、重新生成 active inventory，并更新 B-30 category/strategy/overview、spec matrix、fixture headers/index、B-35 cross fixture 和 stale count。
- `umb-audit list --bucket B-30` 显示 0 entries；`umb-audit stats` 显示 active=203、retired=1081、initial=1284；`umb-audit diff` 已通过。
- 已完成验证：`cargo check -p scoopc`、`cargo test -p scoopc audit:: -- --nocapture`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、B-30 fixtures、`tests/fixtures/unsafe_nogc/`、B-35 cross fixtures、`cargo fmt`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 已更新 `TODO.md`：`P7-B3.5` 标记 `[DONE]`，完成记录已写入；当前统计 active=203、retired=1081。
- 下一步检查 git status/diff/log，确认变更范围后提交本任务。

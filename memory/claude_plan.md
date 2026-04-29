# Claude Plan

我不能记录或导出详细的内部思维过程，但会持续在这里维护可审阅的执行计划、关键决策与进度。

## 当前执行计划

1. 检查最新一次 Git 提交，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务或子任务。
5. 运行相关测试、修复发现的问题，并继续追踪任何既有缺陷。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或新的前置依赖。
7. 按仓库提交风格创建一次 Git 提交，然后停止。

## 进度

- 已创建初始执行计划文件。
- 已检查最新提交；提交信息未显式声明需要先修复的既有问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认当前首个未完成任务为 `T5001e2R`。

## 当前任务

- 任务：`T5001e2R Review：确认 aggregate 不再持有 post-safepoint 的旧 source-of-truth`
- 本次 review 重点：
  - 检查 aggregate copy / arg / return / payload transport 是否仍直接复用旧镜像；
  - 检查 ref 字段是否统一从 explicit-frame home slots reload，非 ref 字段是否仍来自原 storage；
  - 检查 effect / continuation / state-machine payload 是否复用同一 refresh/rebuild 合同；
  - 运行相关 LLVM 回归、fixture 与 lint，确认不存在新的或既有的 correctness 缺口。

## 已完成检查

- 已复核 `crates/scoopc/src/llvm/codegen/mod.rs` 中 `storage_slot_for_use(...)`、aggregate rebuild helper、hidden-sret reload helper 与 explicit-frame leaf-slot 映射逻辑。
- 已复核 `crates/scoopc/src/llvm/codegen/call/abi.rs`，确认 direct/indirect call arg 物化继续经 `storage_slot_for_use(...)` 收口。
- 已复核 `crates/scoopc/src/llvm/codegen/call/resume.rs`，确认 hidden-sret aggregate resume result 读取走 `load_hidden_sret_result_from_ptr(...)`。
- 已复核 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 与 `effect/state_machine_emitter.rs`，确认 effect boxed payload 与 continuation/state-machine payload transport 复用同一 aggregate refresh/rebuild 合同；heap-backed frame field 读取仍留在既有 heap trace/update 合同内。
- 已运行并通过：
  - `cargo test -p scoopc --lib`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 本轮未发现新的既有缺口；当前可将 `T5001e2R` 标记完成，并继续以下一条未完成任务为起点进行下一轮迭代。

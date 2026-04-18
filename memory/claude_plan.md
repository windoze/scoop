# 本次执行计划

说明：按安全与协作要求，这里记录精炼后的推理摘要与可审阅执行计划，不写入逐字内部思维。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在开始前发现最新提交提到的遗留问题，先修复这些问题，再处理该任务。

## 执行步骤

1. 检查最新一次 Git 提交，确认是否提到任何已知遗留问题或后续修复项。
2. 阅读 `TODO.md` 与 `PLAN.md`，识别当前排在最前面的未完成任务，并理解其上下文与依赖关系。
3. 判断该任务是否可在本轮完整完成：
   - 如果可完成：实现、补充/更新测试、运行必要验证。
   - 如果过大或存在未解决前置依赖：把任务拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
4. 在实现过程中，如发现规范不匹配、实现缺口或不能接受的临时绕过：
   - 先把缺口整理为新的前置任务写入 `TODO.md`；
   - 调整任务顺序；
   - 更新 `PLAN.md` 说明阻塞原因；
   - 提交后停止，不继续后续任务。
5. 若任务实现完成：
   - 运行相关测试；
   - 尽量运行 `cargo fmt`、相关测试集，以及 `cargo clippy --all-targets -- -D warnings`（若范围过大则至少说明实际执行范围与结果）；
   - 更新 `TODO.md`、`PLAN.md`、必要文档；
   - 使用清晰提交信息提交；
   - 停止。

## 进度

- 已创建本计划文件。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`。
- 最新提交 `c3feaf38e94c5d8406031737c50375e332c9849b` 为 `T3016kR` 复审提交，提交信息中未附带新的待修遗留问题。
- 已确认当前首个未完成任务是 `T3017`：回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线。

## 当前执行细化

1. 先运行正式 LLVM fixture runner，确认当前首个失败点是否仍是 stale expectation，而不是新的生产回归。
2. 如果 runner 已无新的生产 blocker，则批量回收 stale `EXPECT: fail`/过期 `T3006` 注释，仅保留语义上本应失败或已转记到其它任务的项。
3. 若 runner 暴露新的真实回归，则按阻塞规则：
   - 复现单个 fixture；
   - 记录 spec/实现不匹配；
   - 在 `TODO.md` / `PLAN.md` 中新增前置修复任务并调整顺序；
   - 提交后停止。
4. 若不存在新的 blocker，再运行全量验证：
   - `cargo run -p scoop --features llvm -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次单独 commit。

## 当前状态

- 已完成步骤 1，并确认 `T3017` 当前被新的真实生产回归阻塞。
- 全量 runner 首个失败点：`tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`。
- 单独复现结果：实际输出为 `caught: null` / `1`，golden 期望 `caught: cast` / `2`。
- 已定位到共享合同缺口：`crates/scoopc/src/llvm/codegen/effect/mod.rs` 的 `emit_raise_runtime_error_variant()` 当前把 Raise payload 固定写成 `0`，忽略 `_variant`，因此 synthesized `RuntimeError.ClassCastFailed` 被错误塌缩成 `RuntimeError.NullAssertionFailed`。
- 已按阻塞规则更新 `TODO.md` / `PLAN.md` / 本文件，并新增前置任务 `T3016l` / `T3016lR`。
- 下一步：提交本轮“阻塞重排”变更并停止。

# 本轮执行计划

## 说明

基于安全与协作边界，这里记录的是可审计的执行计划、依据与进度，不写入不可审计的私有推理细节。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；若发现其前置问题或规范偏差，则先按要求调整 `TODO.md` / `PLAN.md`，提交后停止。

## 步骤

1. 检查最新提交，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对任务上下文与依赖。
4. 评估该任务是否过大；如果过大，则把它拆成更小子任务，并更新 `TODO.md` 与 `PLAN.md`。
5. 实现当前应执行的第一个任务。
6. 运行相关测试，并补充/修复测试直到通过。
7. 运行格式化、必要的静态检查与无警告检查。
8. 更新 `TODO.md`、`PLAN.md` 和本文件中的进度记录。
9. 提交本轮变更，然后停止。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交：`64be9df [T3016l] Track RuntimeError variant payload blocker`。提交本身记录了一个需前置处理的真实生产回归，与 `TODO.md` 当前首个未完成任务一致。
- 已读取 `TODO.md` 与 `PLAN.md`，确认本轮首个未完成任务为 `T3016l`：恢复 synthesized `Raise.raise(RuntimeError.*)` 的具体 variant payload 合同。
- 已判断 `T3016l` 目标边界清晰，不需要再拆分子任务。
- 已定向复现 `tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`：当前输出为 `caught: null` / `1`，与任务描述一致。
- 已确认共享 effect transport 已具备 enum 值的 word/gc_ref 编解码能力；当前缺口是 `emit_raise_runtime_error_variant()` 仍把 payload 固定写成 `0`，没有复用共享 enum 编码链路。
- 已完成代码修复：
  - `emit_raise_runtime_error_variant()` 现会先合成具体的 `scoop.core.RuntimeError.<Variant>` unit enum 值；
  - 然后复用共享 `encode_effect_transport_value()` + `scoop_effect_perform_slot_write_u64_with_gc_ref(...)` 写入 TLS perform slot；
  - 不再把 synthesized runtime-error payload 固定塌缩成 `0`。
- 已新增 IR 回归测试 `runtime_raise_boundary_ir_preserves_runtime_error_variant_payload`，锁定 `ClassCastFailed` 的 variant tag 会进入统一 runtime-error transport。
- 已完成验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 结果：上述命令全部通过，`type_check_cast_is_as_asq_basic.scoop` 输出已恢复为 `caught: cast` / `2`。
- 已更新 `TODO.md` 与 `PLAN.md`：`T3016l` 标记为完成，当前 effect 主线下一项为 `T3016lR`。
- 下一步：提交本轮变更，然后停止。

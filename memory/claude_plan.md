# 执行计划与决策摘要

说明：按要求先写入计划文件。这里记录的是可审计的决策摘要与执行步骤，不包含逐字展开的内部推理。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 预定步骤

1. 检查最新一次 Git 提交，确认提交说明里是否提到仍待修复的已知问题。
2. 如最新提交暴露了未修复问题，先定位并修复这些问题，再继续后续步骤。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 如该任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，将第一个子任务作为本轮执行目标。
5. 阅读相关代码、规格、测试与文档，确认实现边界与依赖。
6. 实现该任务，必要时补充或重构测试。
7. 运行相关验证：
   - 最小相关测试
   - 必要时运行更大范围测试
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
8. 若发现规格不匹配、前置功能缺失、或不能无变通完成：
   - 在 `TODO.md` 中新增或重排前置任务
   - 在 `PLAN.md` 中记录阻塞原因
   - 保持当前任务未完成状态
   - 提交变更后停止
9. 若任务完成：
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 提交 Git
   - 停止，不继续下一个任务

## 历史进度记录

- 已完成：创建本计划文件。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`，确认上一轮第一个未完成任务是 `T2003r1d`。
- 已确认：上一轮最新提交本身没有在提交说明中附带需要先修复的独立 bug；它做的是在 `T2003r2` 前新增前置 prerequisite。
- 已确认：`T2003r1d` 可以直接实现，不需要继续拆分 `TODO.md` / `PLAN.md`。
- 上一轮已完成实现：
  1. `HandleSegmentList` 已新增统一 `frame_slots` 表与 `lifted_locals` 元数据；arm binder 改为通过 slot id 引用统一 slot 表，suspend-site locals / arm capture 继续按 symbol id 引用同一张表。
  2. `validate_builder_contract` 已扩展为校验 slot 表排序/去重、lifted-local 闭包、arm binder owner、一切 local 引用是否都能在 slot 表中解析。
  3. segment pretty dump 已新增 `frame-slots:` 可视化，并用稳定 slot name/type/owner/lifted 标记展示 metadata。
  4. 额外修复了一个直接阻塞本任务的真实缺口：此前若 outer-scope local 只在 arm body 中读取、未出现在 handle body 中，plan 不会把它写入 `frame_slots`。现已在 arm capture 收集阶段补齐这部分 slot metadata，避免 nested handle / arm-only capture 在 segment contract 中悬空。
  5. 已新增测试：slot metadata dump 覆盖 outer capture / local val / arm binder / nested handle；contract 负测覆盖缺失 lifted-local 元数据与悬空 capture 引用。
- 上一轮已完成验证：
  - `cargo test -p scoopc segment_`
  - `cargo test -p scoopc plan_dump_`
  - `cargo clippy --workspace --all-targets -- -D warnings`

## 本轮任务确认

- 已完成：检查最新提交 `ccaa7d7 [T2003r1d] Freeze segment slot metadata contract`，提交说明未提到新的待修遗留问题。
- 已完成：阅读 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T2003r2`。
- 已确认：`T2003r2` 当前可直接实现，不需要继续拆分 `TODO.md` / `PLAN.md`。
- 判断依据摘要：
  1. 当前 `HandleSegmentList` 已显式携带 state、terminator、dispatch、arm body、cleanup、suspend-site、frame-slot、lifted-local 与 nested handle 信息。
  2. `HandleStateMachinePlan` 当前额外持有的 `reads` 只是旧 HIR builder 内部用于计算 capture 的中间数据，不参与 pretty dump、simplification 与现有 emitter 选路。
  3. 因此可以新增“仅从 `HandleSegmentList` 反建 `HandleStateMachinePlan`”的统一 builder，并把主入口切到这条 round-trip 路径，再用 dump/simplification parity 锁住行为。

## 本轮实施计划

1. 在 effect 状态机构建模块中实现 segment-only builder：从 `HandleSegmentList` 递归重建 `HandleStateMachinePlan`。
2. 在主入口里改成：
   - 先产出旧 plan；
   - 投影为 segment list；
   - 校验 contract；
   - 再从 segment list 反建新的 plan；
   - 用 debug 断言校验 segment round-trip 与 simplification parity；
   - 返回反建后的 plan 给后续 emitter。
3. 新增定向测试：
   - plan pretty dump round-trip parity；
   - simplification / codegen entrypoint parity；
   - nested handle / mixed-arm representative sample parity。
4. 运行相关测试与质量检查。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，提交后停止。

## 当前状态

- 状态：`T2003r2` 已实现、验证并同步更新 `TODO.md` / `PLAN.md`，正在准备提交。
- 已完成实现：
  1. 在 `state_machine_segments.rs` 中新增 segment-only builder：`HandleSegmentList` 现在可以递归反建完整 `HandleStateMachinePlan`，包含 state、dispatch、arm、suspend-site、cleanup、frame layout 与 nested handles。
  2. `MainCodegen::build_handle_state_machine_plan` 已改为返回由 segment list 重建出的 plan，而不是直接返回旧的 HIR builder 结果。
  3. 主入口新增 debug parity 校验：重建后的 segment round-trip signature 必须与原始 segment contract 一致，mode-specific simplification signature 也必须一致。
  4. 已新增 round-trip 回归：覆盖 direct/branch/while/finally、indirect + state-machine callee、nested handle + multi-arm representative samples，验证 plan dump、simplification dump 与 codegen entrypoint parity。
- 已完成验证：
  - `cargo test -p scoopc plan_round_trip_from_segments -- --nocapture`
  - `cargo test -p scoopc segment_builder_contract_rejects -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo test -p scoopc`
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步：检查最终 diff 并创建本轮 commit，然后停止。

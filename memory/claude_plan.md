# 当前执行计划

## 约束

- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 如遇阻塞当前任务的缺失特性、规格不符或未安排的失败测试，先修复；若无法在本次完成，则在 `TODO.md` 中加入最小必要前置任务并提交后停止。
- 仅在阶段级计划变化时更新 `PLAN.md`。
- 不还原或覆盖非本次产生的用户改动。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；如有，将其纳入当前任务或作为前置任务写入 `TODO.md`。
3. 根据任务要求检查相关实现、规格、测试和 fixture。
4. 用最小正确变更完成当前任务；若发现当前任务被具体缺失能力阻塞，则更新 `TODO.md` 的依赖顺序并停止。
5. 运行格式化、lint、相关测试；需要时运行完整测试和 fixture 套件。
6. 将任务标题标记为 `[DONE]`，更新完成记录和验证记录。
7. 检查 git 状态与 diff，提交本次所有相关变更。
8. 停止，不处理下一个任务。

## 进度

- 已创建初始计划。
- 已读取 `TODO.md` / `TODO-3.md`，第一个未完成任务是 `T3-04L`：收口 T3-04R 十二次审查发现的 P6/LIR/MIR fact-only、verifier 与 gate 残余缺口。
- 最近提交为 `[T3-04R] Add T3-04L review prerequisite`，与当前任务直接相关；当前任务本体已覆盖该未完成问题。
- 已按 `T3-04L` 列出的 helper/模式搜索 P6、MIR、LIR、effect verifier 和 `dependency_gate.py` 的残余 fallback。
- 已删除 P6 class ctor 的 `source_path+span` 回退、HIR direct-call 的 source-call/source-signature root 扫描、named intrinsic 的静态 root fallback。
- 已把 LIR source call-site metadata 提升到 `LateLoweredCallable` 携带，避免 LIR facts builder 直接扫描 source body 恢复 metadata。
- 已删除 MIR backend 对 direct-call source signature 的 MIR body 扫描发布、缺 target 的 `mir_source_callable_target` 补造和 `AbiMangler.fun_symbol(&target_key)` ABI 合成，并删除 named intrinsic root helper合成。
- 已将 value-box itable 生成改为消费已发布 class itable target facts，不再扫描 source signatures 或拼装 `nominal.slot`。
- 已补齐 MIR/LIR verifier 与 dependency gate 守卫，并修复验证暴露的 source-signature/ABI、scalar intrinsic alias、class ctor source contract 和 value-box itable边界问题。
- 已更新 `TODO-3.md` 将 `T3-04L` 标记为 `[DONE]`，并将 `TODO.md` 当前活跃任务推进到 `T3-04R`。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（1664 checks）。
- 下一步检查 git diff/status，并提交本次变更。

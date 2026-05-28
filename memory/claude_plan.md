# 当前执行计划

## 约束

- 以 `TODO.md` 为任务顺序和完成状态的唯一权威来源。
- 本次只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 若发现当前任务被具体缺陷或缺失特性阻塞，优先修复该阻塞项；若无法在本次直接修复，则在 `TODO.md` 中插入最小前置任务并提交后停止。
- 不使用规避实现、弱化测试或变更任务范围来绕过规范不匹配。
- 在提交前按要求运行格式化、lint、相关测试，以及需要时的完整测试/fixture 套件。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录其依赖、验证要求和完成记录格式。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；只把直接阻塞当前任务的问题纳入范围。
3. 根据任务内容检查相关源码、测试和文档，确认需要修改的最小范围。
4. 实现当前任务；若发现规范级阻塞，更新 `TODO.md` 中的依赖/前置任务并停止。
5. 运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，随后运行任务要求的相关测试；若代码变更影响全局行为，再运行完整 Rust 测试和 fixture 套件。
6. 处理所有未明确排期的失败测试/fixture：能修则修，不能修则在 `TODO.md` 中加入最小必要前置任务。
7. 将当前任务标题加上 `[DONE]`，更新完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
8. 检查 git 状态和 diff，提交本次任务相关的所有未提交变更。
9. 停止，不继续下一个任务。

## 进度

- 已定位首个未完成任务：`P5-T03R`，review `P5-T03` 的 call surface 整合结果。
- 最近提交为 `[P5-T03] Integrate call surface overload resolution`，与当前 review 直接相关；本次将以该提交完成记录和 `OVERLOAD_RESOLUTION.md` §7-§9 为基准。
- 已确认并修正 review 范围内的三个缺口：inherited member receiver effective type 不再被调用 receiver 抹平；删除 function-value / operator / compareTo 的 exact-match 兜底；scalar operator unsafe/NoGC gate 改为唯一候选选中后执行。
- 已新增并保留 targeted fixtures 覆盖 member effect-after-selection no fallback，以及适用但非 `operator` 的同名方法不影响 operator-positioned selection。
- 已校准两个不稳定负例（inherited receiver ambiguity、function-value exact-match ambiguity）并删除，避免把当前实现未承诺的诊断形态写成错误期望。
- Targeted `tests/fixtures/typecheck` 已通过（536 个 fixture）。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- `cargo test --all --all-targets` 已通过。
- `python3 tools/spec_fixtures.py check` 已通过。
- 完整 `python3 tools/run_fixtures.py` 已通过（1606 checks）。
- 已将 `P5-T03R` 在 `TODO.md` 和 `TODO-5.md` 标记为 `[DONE]` 并填写完成记录。
- 下一步检查 git diff / status，提交本任务变更后停止。

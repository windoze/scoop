# 当前执行计划

## 约束说明

- 该文件记录可检查的执行计划、关键决策和进度更新。
- 不记录逐字内部推理链；仅记录任务选择依据、计划步骤、验证结果和阻塞事项。
- 本次调用只完成 `TODO.md` 中第一个未标记 `[DONE]` 的任务，然后提交并停止。

## 执行计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否提到与该任务直接相关的未完成问题；只把会阻塞当前任务的问题纳入范围。
3. 阅读当前任务涉及的代码、测试、规范或夹具，确认验收条件和依赖。
4. 以最小正确改动实现该任务；如果发现必须先修复的具体前置缺口，则更新 `TODO.md`、提交并停止。
5. 按要求运行格式化、lint、相关测试，并在需要时运行完整测试/fixture 套件。
6. 更新 `TODO.md`：把完成任务标题加上 `[DONE]`，并补充完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 提交本次任务涉及的全部变更，然后停止，不继续下一个任务。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 定位第一个未完成任务。
- 已定位第一个未完成任务为 `P5-T05：审计 overload diagnostics 与 user-visible failure policy`。
- 最近提交为 `P5-T04R`，未发现与当前任务直接相关的未完成提交说明；当前仓库另有既有未跟踪 `REFLECTION.md`，本任务暂不触碰。
- 下一步阅读 overload diagnostic 代码、相关 fixtures 与 audit 脚本，确定需要补强的位置。
- 已确认主要缺口：definition-time 冲突仍使用旧 `overload_conflict` 代码且错误文本不含候选 `file:line:col`；少量 operator ambiguity 路径只列签名；fixture runner 还没有诊断 forbidden-term 的负向断言。
- 调整计划：补齐 overload 诊断文本与错误码，新增 `EXPECT-NOT-ERROR` 负向断言支持，并扩展 failure-policy audit 覆盖 overload 诊断/fixture 要求。
- 已完成主要实现补丁：`conflicting_overloads` 错误码与 definition-time 候选位置、operator/protocol ambiguity 位置化、多条 `EXPECT-ERROR` 与 `EXPECT-NOT-ERROR` fixture 断言、overload audit 覆盖。
- Targeted overload diagnostic fixtures 已通过；`audit_user_visible_failure_policy.py` 目前暴露既有 internal sentinel drift，需要在完成前修复或按阻塞策略处理。
- 已修复 audit 脚本基线并完成验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo build -p scoop -p scoopc`、targeted fixtures、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`、`python3 tools/audit_user_visible_failure_policy.py`、`python3 tools/spec_fixtures.py check` 均已通过。
- 已将 `P5-T05` 在 `TODO.md` 与 `TODO-5.md` 标记为 `[DONE]` 并写入完成记录；下一步检查 git diff/status 后提交本任务变更。

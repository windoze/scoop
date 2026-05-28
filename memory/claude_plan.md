# 当前执行计划

## 范围

- 本次只处理 `TODO.md` 中第一个标题未以 `[DONE]` 开头的任务。
- 不提前处理后续任务；若遇到阻塞当前任务的实现缺口或未排期失败，将按要求更新 `TODO.md` 并停止。

## 步骤

1. 读取 `TODO.md`，确定第一个未完成任务及其验证要求。
2. 检查最近提交和当前工作区状态，确认是否存在与该任务直接相关的未完成事项或未提交变更。
3. 阅读当前任务涉及的代码、测试和文档，明确最小正确实现范围。
4. 实现任务；如发现必须先修复的具体前置问题，则更新 `TODO.md` 记录前置任务并停止。
5. 运行格式化、lint、相关测试；若代码变更影响全量验证，则按要求运行完整测试和 fixture 套件。
6. 更新 `TODO.md`：给当前任务标题加 `[DONE]`，填写完成记录和验证结果。
7. 更新本文件记录关键执行结果。
8. 检查 diff，提交所有本次任务相关变更，然后停止。

## 当前状态

- 已选定当前任务：`P5-T03：整合 member / constructor / operator / effect-after-selection 路径`。
- 当前重点：审查 call dispatch、member call、constructor call、operator call、function value call 与 effect 校验顺序，确认哪些路径仍绕过统一 overload selection。
- 工作区注意事项：已有未跟踪 `REFLECTION.md`，不是本次任务创建；除非后续确认相关，否则不修改、不提交。

## 已确认改动点

- `member_call` direct path 仍以单个 member FQN 收集签名；当子类新增同名 overload 时，会遮掉父类继承 overload，需要按静态 receiver 类型收集 child + inherited overload set，并用 child override replacement 过滤父类同签名候选。
- `ops` operator / `compareTo` 路径仍在多个 applicable operator 候选时报 `ambiguous_overload`，且 `@Unsafe` / `@NoGC` gate 在候选循环内执行；需要改为先 Phase A-C，随后 Phase D-E specificity，最后只对选中候选执行 gate/effect 记录。
- `value_call` 的 top-level function value 选择在多个可赋值 overload 时仍直接报歧义；需要复用 specificity helper。
- 计划新增/更新 targeted fixtures：member child inherited set、operator specificity、effect mismatch after selected overload no fallback、function value specificity。

## 完成记录

- 已实现 P5-T03：direct member call 收集静态 receiver 的 child + inherited overload set，operator / compareTo 在 modifier gate 后进入 specificity，顶层函数值 overload 在 expected function type 下选择最具体候选，effect gate 保持 selection 后记录。
- 已新增/更新 P5-T03 fixtures：member inherited/override/child-added overload、static receiver no runtime overload、operator specificity、effect mismatch after selected overload no fallback、top-level function value specificity。
- 已通过验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py tests/fixtures/typecheck`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。
- 已更新 `TODO.md` 与 `TODO-5.md`，将 `P5-T03` 标记为 `[DONE]` 并填写完成记录。

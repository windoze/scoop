# 当前执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 本次只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 不做开放式历史问题清扫；只处理当前任务所需或验证中发现的未排期失败。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并检查该任务的依赖、验证要求和完成记录。
2. 检查最近提交信息，判断是否有与该任务直接相关的未完成事项；若有，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 按任务要求检查相关代码、规格、测试和夹具，确认需要修改的最小范围。
4. 实现当前任务；如果发现阻塞性的规格缺口或缺失语言能力，不绕过，改为在 `TODO.md` 插入最小前置任务并停止。
5. 运行格式化、lint、相关测试；若代码发生变化，再按要求运行完整测试套件与夹具套件，处理所有未排期失败。
6. 更新 `TODO.md`：将完成的任务标题加 `[DONE]`，并补充完成记录和验证记录。仅在阶段计划变化时更新 `PLAN.md`。
7. 检查 `git status`、`git diff`、最近提交，确认只提交本次相关变更。
8. 使用清晰任务标签提交变更，然后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md` 和 `PLAN.md`。
- 第一个未完成任务是 `P5-T02R`：Review P5-T02 specificity 与 ambiguity diagnostics，任务细节位于 `TODO-5.md`。
- 已读取 `TODO-5.md` 中 `P5-T02R` 的完整要求。
- 最近提交 `7ed11d42 [P5-T02] Implement overload specificity` 与当前 review 直接相关；未在提交标题中声明未完成事项。
- 已审查 P5-T02 修改范围并发现 review blocker：泛型调用处的普通 type bound 会对 `TypeKind::Param` 延迟通过，可能用 inferred substitution 触发 specialization；constructor specificity 未使用构造器级 type params / bounds；composite 多重 bound 会退化为 `Any`；部分 ambiguity fixtures 的多条 `EXPECT-ERROR` 未真正锁住诊断细节。
- 已修复上述 blocker：收紧 generic bound satisfaction；constructor overload specificity 改为使用 owner + constructor type params / bounds；composite 多重 bound 不再退化为 `Any`，而是保留 effective alternatives；补充 targeted fixtures 覆盖 concrete-vs-generic、generic caller deferred bound、constructor specificity 与 receiver position 0。
- 已运行 targeted specificity / ambiguity fixtures、`tests/fixtures/infer` 和完整 `tests/fixtures/typecheck` 子集，均通过。
- 第一次 `cargo test --all --all-targets` 暴露 `monomorph_rewrites_external_generic_calls_to_concrete_instances` 的测试源码依赖旧的“无约束 `T` 可传给 `print<T: ToString>`”行为；已把测试中的 `wrap<T>` 改为 `wrap<T: ToString>`。
- 已在该修正后重跑 `cargo fmt` 和 `cargo clippy --all-targets -- -D warnings`，均通过。
- 已完成完整验证：`cargo build -p scoop -p scoopc`、`cargo test --all --all-targets`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py` 均通过。
- 已更新 `TODO.md` 与 `TODO-5.md`，将 `P5-T02R` 标记为 `[DONE]` 并写入完成记录。
- 下一步：检查 git diff/status，确认只提交本次相关文件，然后提交并停止。

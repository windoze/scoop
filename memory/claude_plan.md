# 执行计划

本文件记录本次调用的可审计执行计划与进度更新。

## 初始计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一项未完成任务。
2. 读取该任务相关上下文，包括 `PLAN.md`、任务描述中提到的文件、最近提交信息和必要的代码位置；仅处理与当前任务直接相关的问题。
3. 若发现当前任务被具体前置缺陷阻塞，按要求在 `TODO.md` 中插入最小前置任务、更新依赖记录、提交并停止。
4. 若不阻塞，则实现当前任务，优先做最小正确变更，避免绕过规范或夹带无关修改。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，然后运行相关测试、完整 Rust 测试和完整 fixture 套件；若只改文档且已有可复用绿色结果，则记录跳过理由。
6. 修复所有未被明确排期的失败测试或 fixture，或将其作为当前任务的前置事项写入 `TODO.md` 后提交并停止。
7. 完成后在 `TODO.md` 的任务标题加 `[DONE]` 并更新完成记录；仅当阶段级计划改变时才更新 `PLAN.md`。
8. 检查 git 状态和 diff，提交本次任务的所有相关变更，然后停止，不继续下一项任务。

## 进度

- 已创建初始执行计划，下一步读取 `TODO.md` 确认第一项未完成任务。
- 已确认 `TODO.md` 指向批 3，第一项未完成任务为 `TODO-3.md` 的 `T3-04B`。
- 当前任务范围：关闭 T3-04R 二次审查发现的 P6 source-span intrinsic/direct-call 回看、FQN/string/readable-path fallback、dispatch side-table 恢复、P4/P5 verifier 覆盖缺口，并补 dependency gate。
- 代码审查发现 `T3-04B` 存在结构性前置阻塞：P6 的 HIR/source-body call lowering 当前只携带 `hir::Expr.span`，没有 LIR-owned `SiteId` / call-site identity；若直接删除 `LlvmIntrinsicCallContract`、`published_intrinsic_call_contract` 或 `published_instantiated_call_fqn`，只能换成另一个 source-span map，仍不满足 fact-only 契约。
- 已在 `TODO-3.md` 中新增最小前置任务 `T3-04B0`，要求先贯穿 LIR-owned source call-site identity 并发布 generic/reflection/intrinsic/direct-call metadata；`T3-04B` 改为依赖 `T3-04B0`。`TODO.md` 的当前活跃任务同步更新为 `T3-04B0`。
- 本次调用将按阻塞处理停止，不标记 `T3-04B` 完成；由于只修改任务文档和本进度文件，不运行编译/fixture 套件。
- 验证：`git diff --check` 通过；未运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 或 `python3 tools/run_fixtures.py`，因为本次仅修改任务文档和进度记录。

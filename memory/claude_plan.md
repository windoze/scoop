# 当前执行计划

说明：本文件记录可审计的执行计划与进度更新，不记录私有推理链。

## 初始计划

1. 读取 `TODO.md`，按文件顺序定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项；仅在其阻塞当前任务时纳入范围。
3. 阅读当前任务要求、依赖、验证命令和完成记录，确定是否可以按原任务执行。
4. 若遇到必须先修复的具体阻塞项，更新 `TODO.md` 插入最小前置任务，记录阻塞原因，提交后停止。
5. 若无阻塞，实施当前任务的最小正确变更，避免绕过规格或削弱测试。
6. 运行当前任务要求的验证命令及相关测试；若失败，修复后重新验证。
7. 将当前任务标题加上 `[DONE]`，更新其 completion record；仅在阶段计划变化时更新 `PLAN.md`。
8. 提交所有本次任务相关变更，提交信息包含任务编号与简要说明。
9. 完成一个任务后停止，不继续处理下一个任务。

## 进度

- 已创建初始执行计划；下一步读取 `TODO.md` 定位第一个未完成任务。
- 已读取 `TODO.md`，第一个未完成任务为 `P4-T01p：锁定 @Intrinsic method 在类型成员位置同样必须省略方法体`。
- 最近提交为 `7f4b2fbb [P4-T01o] Lock intrinsic interface override bodies`，与当前任务相邻但未明确声明未完成问题；当前任务按 `TODO.md` 原条目执行。
- 已确认 `check_builtin_annotations_on_fun_decl` 已在 top-level、type member 与 object member 入口调用；本任务预计无需 production code 变更。
- 已新增 P4-T01p fixture，覆盖 class / struct / object 成员、override、generic member、`@Intrinsic("name")` member body 失败，以及成员 `@Intrinsic` 无 body 正向用例。
- 验证已通过：`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（461/0）、`cargo test -p scoopc`（863/0）、`cargo clippy --all-targets -- -D warnings`。
- 已将 `TODO.md` 中 `P4-T01p` 标记为 `[DONE]` 并写入完成记录；下一步提交本任务相关变更。

## P4-T01p 执行步骤

1. 检查 `crates/scoopc/src/typecheck/annotations.rs` 中 `IntrinsicFunMustHaveNoBody` 的覆盖入口，确认它是否已覆盖 class / struct / object member、override、generic method 和 `@Intrinsic("name")` 形态。
2. 检查既有 typecheck fixture 命名、诊断写法和 `@Intrinsic` member 语法惯例，新增最小 fixture 覆盖当前任务列出的正反用例。
3. 若现有检查没有覆盖类型成员位置，扩展相同检查链路到 method declaration 入口，复用 `intrinsic_fun_must_have_no_body` 诊断，不新增诊断 code。
4. 运行新增 fixture 和任务要求的 `cargo test -p scoopc`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo clippy --all-targets -- -D warnings`。
5. 若验证通过，将 `P4-T01p` 标记为 `[DONE]`，写入改动范围、核心决策、验证结果和计划闭合记录。
6. 提交本任务相关变更后停止。

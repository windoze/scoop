# 执行计划

说明：按要求先记录执行计划；不会记录私有推理细节，只记录可核查的步骤、决策与进度。

1. 读取 `TODO.md`，按标题是否含 `[DONE]` 找出第一个未完成任务。
2. 查看该任务的依赖、验证要求和完成记录；必要时查看最新提交是否与该任务直接相关。
3. 针对该任务检查相关代码、测试与 fixtures，确认实现范围。
4. 完成该任务；若发现阻塞当前任务的真实缺口，则只新增最小必要前置任务并停止。
5. 运行相关测试；若失败，修复后重测。
6. 更新 `TODO.md`，将完成任务标题加 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 提交本次任务相关全部变更，并在提交后停止。

当前状态：已定位首个未完成任务为 `P4-T01o`。最新提交 `4b2a5c79 [P4-T01n] Support intrinsic synthetic properties` 与当前任务顺序相邻但未声明未完成阻塞项。

本任务执行步骤：

1. 检查 `@Intrinsic` / `@Extern` method body 诊断、interface override 检查和现有 fixture 约定。
2. 补齐或修正前端约束：`@Intrinsic class/struct` 作为 interface implementer 时，显式实现/override 的 method 必须是普通 method 且有 body。
3. 新增 typecheck fixture 覆盖 `@Intrinsic override`、`@Extern override`、无 body override 报错，以及带 body 普通 override 通过。
4. 审计 sysroot 当前 `@Intrinsic class/struct` 的 interface impl 形态，并写入 `TODO.md` 完成记录。
5. 运行指定验证：相关 fixture、完整 typecheck fixture、`cargo test -p scoopc`、`cargo clippy --all-targets -- -D warnings`。
6. 标记 `TODO.md` 中 `P4-T01o` 为 `[DONE]`，提交本任务相关变更后停止。

进度更新：已在 `typecheck/interfaces.rs` 增加 `@Intrinsic` class/struct interface implementation shape 诊断，并新增 class/struct 的 `@Intrinsic` override、`@Extern` override、无 body override 失败 fixture 与带 body 普通 method 正向 fixture。已运行 `cargo fmt --all`、正向 fixture 和一条 `@Intrinsic` class 失败 fixture，均通过。

进度更新：完整验证已通过：`tests/fixtures/typecheck` 为 453 passed / 0 failed，`cargo test -p scoopc` 为 863 passed / 0 failed，`cargo clippy --all-targets -- -D warnings` clean。已把 `TODO.md` 中 `P4-T01o` 标记为 `[DONE]` 并写入完成记录；下一步检查 git diff 并提交本任务变更。

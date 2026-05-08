## 当前执行计划

1. 先读取 `TODO.md`，按标题是否带有 `[DONE]` 判断第一个未完成任务。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关且未完成的问题；若是当前任务前置阻塞，则在 `TODO.md` 中补充最小必要前置任务并停止在该处。
3. 阅读当前任务涉及的代码、测试、规范与依赖文件，确认实现边界与验收条件。
4. 以最小正确改动完成当前任务；若遇到阻塞当前任务的真实缺口或回归，优先修复，或将其作为新的前置任务写入 `TODO.md`。
5. 运行任务要求的验证命令，以及必要的回归测试；确保没有新增警告，必要时运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings` 或任务明确要求的更小范围命令。
6. 更新文档记录：
   - 在 `TODO.md` 中将当前任务标题标记为 `[DONE]` 并填写完成记录；
   - 仅当阶段计划确实变化时才更新 `PLAN.md`；
   - 在本文件补充关键进展、计划变化与验证结果。
7. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进展记录

- 已在开始执行前写入本计划文件。
- 已读取 `TODO.md`，确认首个未完成任务为 `CG-T07S0a17`：修复 `star_projection_array_read_view` 中 `Array<*>` 读视图的 `Any?` element transport trace contract 漂移。
- 已检查最近一次提交 `[CG-T07S0a16]`，未发现需要并入当前任务的直接未完成前置问题；当前按 `CG-T07S0a17` 直接执行。
- 下一步：先复现 `star_projection_array_read_view.scoop` 的 build/test 失败，并定位 `xs.get(0)` 路径上 `trace=false` 与 composite layout GC slots 不一致的 contract 发布环节。
- 已通过 `dump-ir` 定位根因：`scoop.core.get::<*>` 的结果内部类型是 `StarProjection(read_ty = Option<Any>)`；MIR transport metadata 已发布 `trace = true`，但 LLVM composite transport verifier 在重新推导时把 `StarProjection` 当成 `trace = false`，与 `read_ty` 的 GC slot 布局冲突。
- 已完成实现：
  - `crates/scoopc/src/llvm/codegen/composite_transport.rs` 现在让 `StarProjection` 继承 `read_ty` 的 trace requirement，但仍保持它不是数组底层存储的 composite layout owner，避免把 `Array<String>` 的真实 ref-like 存储误当成 `Option<Any>` 直接搬运。
  - `crates/scoopc/src/llvm/tests.rs` 新增 production codegen 回归，锁定 `firstIsSome` 中 `Option<Any>` 读视图和 traceable `Array.get::<*>` transport contract，并守护 LLVM codegen 成功。
- 当前验证结果：
  - 通过：`cargo test -p scoopc production_codegen_star_projection_array_read_view_keeps_traceable_transport_metadata`
  - 通过：`cargo run -p scoop -- build tests/fixtures/run-pass/star_projection_array_read_view.scoop -o /tmp/star_projection_array_read_view`
  - 通过：运行 `/tmp/star_projection_array_read_view`，stdout 为 `true`
  - 通过：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/star_projection_array_read_view.scoop`
  - 通过：`cargo clippy --all-targets -- -D warnings`
  - 通过且发现下一处 blocker：`cargo run -p scoop -- test` 已越过 `star_projection_array_read_view.scoop`，下一处失败转为 `tests/fixtures/run-pass/stdlib_string_basic.scoop`
- 已据此更新 `TODO.md`：
  - 将 `CG-T07S0a17` 标记为 `[DONE]` 并写入完成记录；
  - 新增前置任务 `CG-T07S0a18`，记录 `stdlib_string_basic.scoop` 中 `sysroot/string.scoop` 的 `String.byteLength()` 等 support-source intrinsic member 调用仍退化成 unresolved `MemberAccess` + `CallKind::FunValue` 的新 blocker；
  - 将 `CG-T07S0a` 的依赖与完成记录同步到新的前置任务顺序。
- 下一步：检查工作区差异，创建本次任务提交，然后停止，不继续实现 `CG-T07S0a18`。

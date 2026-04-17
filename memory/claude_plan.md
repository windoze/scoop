## 当前迭代计划

1. 检查最近一次提交信息，确认是否明确提到已有问题、回归或待补修项；如果有，优先纳入本次处理范围。
2. 阅读 `TODO.md`，定位第一个未完成任务，并同步查看 `PLAN.md`、`README.md` 与相关规范说明，确认依赖与当前状态。
3. 判断该任务是否足够收敛：
   - 如果可以在本轮完整交付，就直接实现。
   - 如果范围过大或存在前置缺口，就把任务拆分为更小的子任务，并更新 `TODO.md` 与 `PLAN.md`，随后只执行新的第一个子任务。
4. 在实现前先阅读相关模块与测试，确认现状、接口边界和潜在规格缺口；若发现阻塞当前任务的真实实现缺口，则先把缺口整理为新的前置任务并更新计划。
5. 实现本轮目标，保持改动局部、可验证，并在必要处补充测试与文档。
6. 运行与本轮改动直接相关的验证：
   - 至少运行针对性测试；
   - 如任务涉及构建、警告或全局行为，再补充 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 等适当验证。
7. 完成后更新进度文件：
   - 在 `TODO.md` 中标记本轮任务完成，或在阻塞时调整任务顺序与依赖；
   - 在 `PLAN.md` 中记录完成情况、拆分结果或阻塞原因；
   - 在本文件中补充关键进展与计划调整。
8. 检查工作区改动，确认没有误改无关内容后，创建一次清晰的 Git 提交，然后停止，不继续处理下一个任务。

## 执行约束

- 本轮只完成 `TODO.md` 中排序最靠前的一个未完成任务，除非必须先补前置缺口并据此重排任务。
- 不接受规避式实现；若实现与规范不一致，必须显式建任务并前移依赖。
- 任何关键步骤完成后，都要同步更新本文件，确保进度可追踪。

## 当前锁定任务

- 已确认首个未完成任务为 `T3009b1R`：Review `Continuation.resume(...)` 的 precise type 解析，确认修复后不再依赖被宽化的 `VarRef` HIR type。
- 本轮预期先审查最近一次提交涉及的实现与测试，再运行定向验证；若发现问题，则先修复问题并补测，然后再更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 本轮进展

- 已检查最近一次提交，未发现需要插队处理的额外提交说明问题；本轮首个未完成任务确定为 `T3009b1R`。
- 已完成生产代码复审，确认 `Continuation.resume(...)` 仍只依赖 typecheck 确认的 builtin call-site marker，没有引入按成员名 / 局部形状分流的 patch。
- 复审过程中发现并修复一个残留缺口：`resolve_expr_cg_ty()` 原先只优先读取局部 env，再回退到 `expr.ty`；而 HIR lowering 会把所有 `VarRef`（包括 top-level const）写成 `Any`。现已改为在保留局部 `CgLocal.ty` 优先级的同时复用 `resolve_expr_concrete_type()`，从而让 `Continuation.resume(...)` 的 payload fallback 也能覆盖 top-level `VarRef` 与其它 concrete-type 来源。
- 已新增回归 `tests/fixtures/run-pass/continuation_resume_top_level_const_payload.scoop`，用于锁定“top-level typed `VarRef` payload 不再退回 widened `Any`”。
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_top_level_const_payload.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步不再继续实现；文档更新并提交后停止。下一轮应从 `T3009b2` 开始。

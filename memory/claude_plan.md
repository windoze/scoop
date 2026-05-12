## 本次执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断第一个未完成任务。
2. 检查最近一次提交信息，确认是否有与该任务直接相关且尚未完成的问题需要并入当前任务或作为前置任务写回 `TODO.md`。
3. 阅读当前任务涉及的代码、测试、文档与依赖约束，只做与该任务直接相关的分析，不做开放式问题扫荡。
4. 实现当前任务；若遇到阻塞当前任务的真实缺口或缺陷，则先修复该阻塞，或按要求在 `TODO.md` 中插入最小必要前置任务并停止。
5. 运行任务要求的验证，以及必要的相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
6. 更新文档与任务记录：将完成的任务在 `TODO.md` 标记为 `[DONE]` 并填写完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
7. 检查工作区变更，按任务要求提交一次 git commit，然后停止，不继续下一个任务。

## 进度日志

- 已创建本计划文件。下一步：读取 `TODO.md` 并识别第一个未完成任务。
- 已读取 `TODO.md` 与最近一次提交；当前第一个未完成任务是 `P2-T02R`（review 任务），最近提交为 `[P2-T02] Internalize compiler-private helper linkage`，未见提交主题中显式声明的额外未完成前置问题。
- 当前 review 重点：
  1. external symbol 集中是否仍残留 compiler-private helper；
  2. `main`、runtime/native import、`@Extern` 是否仍保持 external 例外；
  3. linkage 收口后是否未引入 ABI/语义漂移。
- 下一步：阅读 `P2-T01` / `P2-T02` 相关代码与测试入口，然后运行对象符号审计、全量测试与 `clippy` 验证；若发现真实 blocker，则按要求先更新 `TODO.md`。
- 已复核的代码/测试入口：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中 `LlvmFunctionDeclarationSurface` 与统一 declaration helper；
  - `crates/scoopc/src/llvm/tests.rs` 中 external symbol 分类、object audit、raw `add_function(..., None)` inventory、`@Extern` 回归测试。
- 已完成验证：
  - 静态 grep 未发现 production 代码中残留的 `CompilerPrivateHelper + Linkage::External` 组合；
  - `cargo test -p scoopc function_declaration_ -- --nocapture` 通过；
  - `cargo test -p scoopc external_symbol -- --nocapture` 通过；
  - `cargo test -p scoopc refactor_llvm_extern_global -- --nocapture` 通过；
  - `cargo test -p scoopc` 通过（767 passed）；
  - `cargo clippy -p scoopc --all-targets -- -D warnings` 通过。
- 结论：当前未发现阻塞 `P2-T02R` 的新前置问题。下一步：更新 `TODO.md`，将 `P2-T02R` 标记为 `[DONE]` 并补全完成记录，然后提交本次 review 结果。
- 已完成 `TODO.md` 回写：`P2-T02R` 已标记为 `[DONE]`，并补充了 review 决策、固定 external 例外清单、静态审计摘要与测试/`clippy` 验证结果。
- 剩余步骤：检查变更、提交 git commit，然后停止，不进入 `P3-T01`。

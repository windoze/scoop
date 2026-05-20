# 执行计划

## 约束

- `TODO.md` 是任务顺序、完成状态、依赖和验证要求的唯一权威来源。
- 本次只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 不做开放式历史问题扫描；只处理当前任务直接需要或阻塞的事项。
- 若发现当前任务存在必须先解决的具体前置缺口，更新 `TODO.md` 插入最少前置任务、保持当前任务未完成、提交后停止。
- 不采用规避、缩小夹具、特判或偏离规范的做法。
- `PLAN.md` 仅在阶段级计划、依赖或完成标准发生真实变化时更新。

## 步骤

1. 读取 `TODO.md`，找出第一个标题未带 `[DONE]` 的任务，并记录其要求、依赖和验证命令。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；若有，将其纳入当前任务或作为前置任务记录。
3. 基于任务要求只收集必要上下文，定位相关代码、测试和夹具。
4. 按最小正确改动实现任务；编辑前后保持 `memory/claude_plan.md` 同步记录关键进展。
5. 运行任务要求的验证和相关回归测试；若失败，修复真实根因并复测。
6. 完成后更新 `TODO.md`：在任务标题前加 `[DONE]`，并填写完成记录、验证结果和关键实现说明。
7. 检查工作区差异，确保未误改无关文件；如当前任务完成，提交所有本次任务相关改动。
8. 提交后停止，不继续处理下一个任务。

## 当前状态

- 已读取 `TODO.md`，确认第一个标题未带 `[DONE]` 的任务是 `P7-T02：dependency cone C++/link-flags/linker driver 覆盖`。
- 任务要求：任一 loaded cone 有 `cxx-sources` 时默认 C++ linker driver 生效；各 cone `link-flags` 按 dependency-topological order 稳定追加；duplicate symbol 等 linker 错误不得被隐藏；新增 dependency C++ fixture 与 dependency link-flags fixture。
- 最近提交为 `[P7-T01] Extend native build to loaded cones`，无额外未完成事项说明。
- 当前实现差距：`compile_native_build_sources` 已遍历 `front.input().graph().nodes()` 编译所有 loaded cone 的 C/C++ objects；但 `run_codegen_and_link` 的 `native-build.linker` 与 `link-flags` 仍只取 consumer cone manifest。
- 调整计划：新增 graph-based native link plan，按 graph DAG 顺序收集每个 loaded cone 的 `link-flags`；显式 `linker` 从 loaded cones 统一解析并拒绝冲突；无显式 linker 且任一 loaded cone 有 C++ source 时默认使用 `clang++`；新增 dependency C++ run-pass fixture、dependency link-flags forwarding negative fixture 和 link plan 单元测试。
- 已实施：`run_codegen_and_link` 使用 `native_link_plan`；新增单元测试覆盖 DAG link flags 顺序、loaded cone linker 冲突诊断、dependency C++ 默认 `clang++`；新增 `dependency_cxx_sources_extern_call_cpp_stdlib`、`dependency_link_flags_are_forwarded`、`dependency_duplicate_native_symbol_is_link_error` fixtures。
- 验证已通过：`cargo fmt`；`cargo test -p scoop native_link_plan -- --nocapture`；`cargo run -p scoop -- test tests/fixtures/run_pass_cone/`（37 checks）；`cargo build`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（1576 checks）。
- 已更新 `TODO.md`：`P7-T02` 标为 `[DONE]`，完成记录包含改动范围、核心决策、fixture 覆盖和验证结果。
- 下一步：检查 `git status`、`git diff`、最近提交记录，确认差异只包含本任务改动后提交。

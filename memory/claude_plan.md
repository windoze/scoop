执行计划（当前调用）

1. 先读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 开头的任务。
2. 读取该任务相关上下文、最新提交和必要代码，确认任务范围、依赖和验证要求；不做无关历史问题扫查。
3. 如任务可直接完成，按最小正确改动实现；如发现阻塞当前任务的缺失特性或测试失败，按要求在 `TODO.md` 中加入最小前置任务并停止。
4. 运行与任务相关的测试；如观察到未排期失败，修复或在 `TODO.md` 中排期为当前任务前置项。
5. 更新 `TODO.md`：完成时在任务标题加 `[DONE]` 并补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
6. 检查工作区差异，提交本次调用产生的所有相关改动，然后停止，不继续下一个任务。

进度记录：

- 已写入初始执行计划；下一步读取 `TODO.md` 确认第一个未完成任务。
- 已确认第一个未完成任务为 `P9-T03`（抽出 `scoopc_codegen_llvm` crate）。最新提交 `720e3770 [P9-T02R] Review scoopc_ast extraction` 未记录直接相关的未完成事项。当前工作区已有非本次产生的未跟踪文件 `PLUGIN_ABI.md`，本次不会修改或提交它。
- 下一步聚焦读取 `scoopc` 的 LLVM 模块、`stackmap.rs`、pipeline LLVM stage、workspace Cargo 配置与 dependency gate，制定最小迁移补丁。
- 已确认直接让 `scoopc` 依赖新 backend crate 会与 P9-T03 允许的临时 `scoopc` façade 依赖形成 Cargo cycle。采用 staged extraction：把 LLVM/stackmap 源码物理迁入 `crates/scoopc_codegen_llvm`，`scoopc` 通过 `#[path]` façade 继续编译这些源文件，新 crate 暂时 re-export `scoopc::llvm` / `scoopc::stackmap`；完成记录登记 P9-T06 后切到真正 `scoopc_lir` 直依赖。
- 已完成迁移初稿：workspace 加入 `scoopc_codegen_llvm`，`llvm/` 与 `stackmap.rs` 已移到新 crate 路径，dependency gate 的 LLVM source-boundary 路径已切换。`cargo check --workspace --features llvm` 与 `cargo run -p scoop_tools -- dependency-gate` 已通过；下一步跑任务要求的完整验证。
- 验证完成：`cargo fmt`、`cargo build --workspace --features llvm`、`cargo test --all --all-targets --features llvm`（30 分钟超时重跑通过）、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets --features llvm -- -D warnings`、`cargo tree -p scoopc_codegen_llvm`、`git diff --check` 均已通过。已将 `TODO.md` / `TODO-7.md` 中 `P9-T03` 标记为 `[DONE]` 并补充完成记录；下一步检查差异并提交。

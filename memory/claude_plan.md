## 当前目标

本轮只处理 `TODO.md` 的首个未完成子任务 `T1220b`，完成其代码核对、文档收尾、质量门验证和 Git 提交后停止，不继续进入 `T4015R`。

## 当前进展

- 已完成：检查最新提交、确认未发现需要先行修复的额外既有问题。
- 已完成：核对 `T1220b` 代码改动，确认真实前端入口、fixture 路径、LLVM 单文件前端与 comptime compilation-unit 入口都已切到 compilation-unit trim 主线。
- 已完成：更新 `TODO.md`、`PLAN.md`、`ISSUES.md`，把状态从“等待修复 package-level `comptime if` 调用绑定缺口”切换为“`T1220b` 已完成，下一步是 `T4015R`”。
- 已完成：补跑并通过 `cargo test -p scoopc package_level_comptime_if_ -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 待完成：复核最终 diff，只提交 `T1220b` 相关文件，避免带入 `run_agent.sh`。
- 待完成：提交 Git，提交信息使用 `[T1220b] ...` 风格。

## 已确认实现点

- `crates/scoopc/src/comptime/interpreter.rs` 已新增 compilation-unit 级 trim 入口、`CompilationUnitTrimContext`、可见前缀 probe 与 `TopLevelFunCallBinding` override 机制。
- `crates/scoop/src/commands/build.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoopc/src/llvm/frontend.rs` 与 `eval_const_bindings_in_compilation_unit(...)` 已统一改为“先 parse 全部 AST，再按整编译单元 trim package-level `comptime if`”。
- `crates/scoopc/src/comptime/tests.rs` 与 `tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun/` 已覆盖同文件 overload、generic 显式类型实参和跨文件 import 三类验收场景。

## 已知约束

- 不回退或误提交无关改动，尤其是 `run_agent.sh`。
- 如果补跑质量门时暴露新的既有问题，必须先处理该问题，不能绕过。
- 编辑文件时统一使用 `apply_patch`。

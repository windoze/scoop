# 当前执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。开始具体实现前，先检查最近一次提交是否提到已有问题；如果有，先修复这些问题。

## 约束与执行原则

- 先处理最近一次提交中提到的既有问题，再进入 `TODO.md` 的首个未完成任务。
- 在确认任务过大时，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
- 实现后必须补充或更新测试，并运行相关验证，目标是通过 `cargo test`、相关定向测试，以及 `cargo clippy --all-targets -- -D warnings`（若作用范围允许，则优先运行与改动相关的最小充分集合，再决定是否扩大验证）。
- 完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交 Git commit，然后停止。
- 不回退用户已有改动；若遇到冲突，先读取并理解，再决定如何兼容。

## 当前步骤

1. 查看工作区状态，确认是否存在用户未提交改动以及 `memory/` 目录状态。
2. 查看最近一次提交信息，识别是否提到需要先修复的既有问题。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 评估该任务是否需要拆分；若需要，先更新 `PLAN.md` 与 `TODO.md`。
5. 实现本轮目标任务。
6. 运行格式化、测试、clippy 等验证；若失败，继续修复直到通过或明确受阻。
7. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成状态与验证结果。
8. 提交本轮改动，并停止。

## 进度记录

- 已检查工作区、最新提交、`TODO.md`、`PLAN.md`。
- 已确认最新提交额外暴露出一个既有限制：顶层 `const val` 的一般表达式在 LLVM codegen 里会落到 `top-level value ref`。
- 已用最小复现确认当前失败模式：
  - 源码：顶层 `const val BASE = 1 + 2`、`const val DOUBLE = BASE + BASE`，在 `main` 中引用 `DOUBLE`。
  - 结果：`cargo run -p scoop -- build <tmp>.scoop -o /tmp/top_level_const_val_test.out` 失败，报 `scoop::llvm::unsupported_main_body`，消息为 `top-level value ref`。
- 当前决定：先把这条既有限制补成一个显式任务，优先修复后再继续原先的 `TODO` 顺序。
- 计划中的具体实现：
  1. 在 `TODO.md` / `PLAN.md` 中插入一条位于 `T0149` 之前的新任务，描述“顶层 `const val` 一般表达式 codegen”。
  2. 在 HIR lowering 阶段为顶层 `const val` 建立 side table，保存 `fqn / source_path / span / ty / init`。
  3. 在 LLVM codegen 中，当遇到 `ValueRef::TopLevel` 且不是 object / top-level var 时，回退查找该 side table，并按声明类型内联 initializer。
  4. 为避免递归常量定义导致无限展开，加入最小循环检测。
  5. 增加单文件与多文件 run-pass fixtures，覆盖“const val 链式引用”和“helper 文件里的顶层 const val”。
  6. 运行 `cargo fmt --all`、相关定向测试、`cargo test --all`、`cargo run -p scoop -- test`、`cargo clippy --workspace --all-targets --message-format short -- -D warnings`。
- 已完成实现：
  - HIR：新增 `TopLevelConst` / `TopLevelConstIndex`，lowering 会收集顶层 `const val` 的 `fqn / source_path / span / ty / init`。
  - LLVM codegen：`ValueRef::TopLevel` 现在能回退到顶层 `const val`，并按声明类型内联 initializer；同时加入最小递归检测。
  - Reachability：扫描顶层 `const val` 时会递归进入 initializer，并继续扫描复合 callee 表达式，避免遗漏只经由顶层 const 可达的顶层函数。
  - Fixture：新增
    - `tests/fixtures/run-pass/top_level_const_val_general_expr_basic.*`
    - `tests/fixtures/run_pass_cone/top_level_const_val_multi_file_basic/**`
- 已完成定向验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/top_level_const_val_general_expr_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run_pass_cone/top_level_const_val_multi_file_basic`
- 已完成全量验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (863)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- 本轮停止点：
  - 已把该既有限制补成独立任务 `T0148d-4` 并完成。
  - `TODO.md` 中下一个未完成任务仍为 `T0149`，留待下一次调用处理。

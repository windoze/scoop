## 当前思路摘要

本轮目标是严格按照仓库流程只完成 `TODO.md` 中的第一个未完成任务，并在结束前完成测试、文档更新和提交。开始实现前，先检查最新提交是否提到任何既有问题；若存在，先修复这些问题，再进入 `TODO.md` 的任务执行。若第一个未完成任务过大或存在前置缺口，则需要先在 `PLAN.md` / `TODO.md` 中拆分、重排并记录依赖，然后只执行拆分后的第一个可落地子任务。

## 分步执行计划

1. 检查最新一次 Git 提交的提交信息与改动，确认是否显式提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的背景、依赖、当前计划状态。
4. 若任务过大、依赖不明确、或存在规范/实现缺口：
   - 在 `PLAN.md` 中补充拆分方案与阻塞原因。
   - 在 `TODO.md` 中把任务拆成更小子任务，或在前面插入必要前置修复任务。
   - 本轮只执行拆分后的第一个可执行子任务；若仅能完成重排与计划更新，则提交后停止。
5. 若任务可直接执行：
   - 阅读相关代码、测试、规范和上下文文件。
   - 实现任务所需修改，避免引入规避性方案。
6. 运行与修改范围相关的格式化、检查与测试：
   - 至少运行针对性测试；
   - 根据任务范围决定是否运行更广泛的 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 等。
7. 更新进度文档：
   - 在 `TODO.md` 标记当前任务完成；
   - 在 `PLAN.md` 反映当前状态与后续计划；
   - 如执行过程中计划发生变化，同步更新本文件。
8. 检查工作区状态，确认仅包含本轮合理修改。
9. 使用清晰提交信息创建 Git 提交。
10. 停止，不继续处理下一个任务。

## 当前检查结果（2026-04-13）

- 最新提交 `b1749bb [T2003u2] Build unified effect state-machine plan` 未在提交信息中显式声明需要先修复的既有缺陷；当前未发现“必须先于 TODO 执行”的提交内遗留 issue。
- `TODO.md` 中第一个未完成任务是 `T2003u3`。
- 进一步审计后确认：`T2003u3` 同时跨越统一 plan 的模式化简抽象、`codegen_handle_expr` 入口选路、以及代表性 LLVM 端到端验证，超出单轮安全改动范围。

## 当前调整后的执行方案

1. 先把 `T2003u3` 拆成更小子任务，并同步更新 `TODO.md` / `PLAN.md`：
   - `T2003u3a`：定义 mode-specific simplification 输出、pretty dump 与单元测试。
   - `T2003u3b`：把 simplification 接入 `codegen_handle_expr` 入口选路，并补代表性 LLVM 验证。
2. 本轮只执行 `T2003u3a`。
3. 实现内容预计包括：
   - 在统一状态机 plan 之上新增 simplification 数据结构；
   - 为 never-resume / immediate-resume / escape-continuation 记录不同的 lowering 决策；
   - 为测试提供 dump，验证同一完整 plan 可以派生出不同 mode-specific 输出；
   - 在现有 codegen 入口先构建 simplification 并消费结构签名，作为后续接线入口。
4. 完成后运行 Rust 测试与 lint，更新文档状态，提交并停止。

## 当前进度更新（T2003u3a 已完成）

- 已完成 `TODO.md` / `PLAN.md` 拆分：`T2003u3` -> `T2003u3a` + `T2003u3b`。
- 已新增 `crates/scoopc/src/llvm/codegen/effect/state_machine_simplify.rs`：
  - full plan -> simplification 的派生逻辑；
  - never-resume / immediate-resume / escape-continuation 的 lowering 决策；
  - stable signature 与 pretty dump；
  - nested handle 的递归 simplification。
- 已把 simplification 接入 `codegen_handle_expr` 的迁移前置步骤，当前会在构建 full plan 后额外构建 simplification 并消费 signature。
- 已新增单元测试：覆盖 never-resume、immediate-resume、escape-continuation，以及 mixed representative sample。
- 已完成验证：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步应由后续调用执行 `T2003u3b`，本轮到此停止。

## 执行约束

- 不接受临时规避、fixture-only hack、或偏离规范的实现。
- 如果发现任务依赖缺失特性或已有 bug，必须先把该缺口写入 `TODO.md` 并调整依赖顺序，再提交并停止。
- 不回退用户已有的无关修改。
- 所有对外说明与过程记录使用中文。

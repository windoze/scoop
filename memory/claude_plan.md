# 当前执行记录

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在开始任何仓库检查与执行前，先建立本文件，记录计划与后续进展。
- 若最新提交提到既有问题，需优先修复。
- 若在执行中发现任何既有缺陷、规格不匹配、回归、实现边界不完整或依赖缺失，必须先修复，或将其作为前置任务插入 `TODO.md` 后停止。
- 不接受通过规避路径、缩小范围、夹具特判或其他临时方案绕过问题。

## 初始执行计划

1. 查看最新一次 Git 提交信息，确认是否显式提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有计划、依赖与任务背景。
4. 结合代码现状评估该任务是否可以在本轮完整落地。
5. 若任务过大，先将其拆分为更小的前置子任务，并同步更新 `TODO.md` 与 `PLAN.md`，然后执行拆分后的第一个子任务。
6. 实现任务所需代码修改。
7. 运行相关测试、格式化、lint 与必要验证，修复执行中发现的所有既有问题。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态与任何计划调整。
9. 提交 Git commit，提交信息需清晰描述本轮完成内容。
10. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本文件，准备开始仓库检查。
- 已检查最新提交 `e6ff6bf`（`[T5000j1R] Review operator-overload target boundary`）；提交说明未显式声明新的遗留 bug 需要优先修复。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务为 `T5000j2 扩展 when / pattern 到 production MIR body / summary 主线`。
- 已完成针对 `T5000j2` 的定点探查，当前结论如下：
  - generic MIR lowering 已经把 `when` 收口为显式 CFG + `Rvalue::PatternMatch` / `Rvalue::PatternExtract`；
  - `mir::summary`、materialization、escape / inline 等 shared/middle-end 路径已经能稳定消费这两个节点；
  - 当前主要缺口集中在 `crates/scoopc/src/llvm/codegen/mir_body.rs`：raw materialized MIR body 支持检查与 production MIR bridge 仍把 `PatternMatch` / `PatternExtract` 统一判为 unsupported，导致非 effect 的 `when`/pattern body 继续退回 HIR-compatible emission；
  - probing 同时暴露了一个既有 production 缺口：`when` pattern 的 `is Type` 形状在 parser/typecheck/HIR/MIR 均已存在，但当前 production lowering 主线没有真正打通。
- 已完成 `crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/reachability.rs` 与 `crates/scoopc/src/llvm/tests.rs` 的实现与回归补齐：
  - raw materialized MIR body 现已支持 `PatternMatch` / `PatternExtract`，并覆盖 wildcard / bind / rest / or / tuple / variant / literal / `is` 等常见非 effect pattern 形状；
  - raw non-generic pattern body 现已纳入 production reachability / body emission 的 canonical materialized body 主线；
  - 已新增 production LLVM 回归，覆盖 declaration-only direct-call fallback、variant payload binder、`when is Type`、generic pattern summary 暴露与 indirect GC aggregate pattern param ABI load。
- 全量验证过程中发现并修复了一个既有回归：
  - `cargo run -p scoop -- test` 首次失败于 `tests/fixtures/run-pass/option_nested_ref_no_nested_niche_basic.scoop`；
  - 根因是 `bind_mir_params` 忽略了 ordinary param ABI 的 indirect GC aggregate 分支，把 `Option<Option<String>>` 这类 pattern 参数的 ABI 指针误当作 enum 值解释；
  - 已修为“先按 ABI load 间接参数，再进入 MIR pattern lowering”，fixture 输出恢复为 `hi / inner-none / outer-none`。
- 修复后的最终验证已全部通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc production_codegen_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1202)`）
- 当前已完成 `T5000j2` 的实现、测试与文档更新，下一步只剩提交本轮改动并停止。

## 细化实施计划

1. 直接完成 `T5000j2`，不再拆分子任务。
   - 理由：缺口已收敛到 production MIR bridge 的 pattern lowering 支持面，属于单一实现面，可在一轮内完整处理。
2. 扩展 `crates/scoopc/src/llvm/codegen/mir_body.rs`：
   - 让 raw materialized MIR body 支持检查按实际 pattern 形状与 subject/target 类型判定，而不是一律拒绝；
   - 为 `PatternMatch` / `PatternExtract` 增加 production MIR lowering；
   - 支持常见非 effect `when` pattern 形状直接走 MIR 主线：literal / wildcard / bind / rest / or / tuple / variant / `is`。
3. 如有必要，复用现有 enum/tuple/type-check lowering helper，避免把 pattern 语义判断重新塞回 backend 现场猜测路径。
4. 添加回归测试，至少覆盖：
   - production raw MIR body 中的 variant payload binder（验证 `PatternExtract`）；
   - production raw MIR body 中的 `when is Type`（验证既有 production 缺口已打通）；
   - 若实现过程中暴露其它既有缺陷，一并修复并补回归。
5. 运行格式化、相关测试、全量测试与 clippy。
6. 完成后更新 `TODO.md`、`PLAN.md`、本文件并提交。

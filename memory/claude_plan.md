# 执行计划

## 说明

按要求，我会在开始任何仓库检查或构建命令前，先把可公开的执行思路与步骤计划记录到这个文件中。这里记录的是执行摘要、决策依据和后续进展，不包含不可公开的内部推理细节。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果遇到前置缺陷、规范不匹配或任务过大，则先按要求更新 `TODO.md`/`PLAN.md`，提交后停止。

## 执行步骤

1. 检查最新一次 Git 提交信息，确认是否提到了已知遗留问题、待修复问题或明确指出的缺陷。
2. 如果最新提交提到遗留问题，先定位并修复这些问题，补充测试并验证，再继续后续步骤。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 阅读 `PLAN.md`、相关源码和测试，确认该任务的上下文、依赖和当前实现状态。
5. 判断该任务是否足够小且可在本轮完整完成。
6. 如果任务过大，则将其拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`，并以新的第一个子任务作为当前执行目标。
7. 实现当前目标任务，避免引入规避性实现，严格按规范推进。
8. 运行相关测试、格式化、lint；如果失败则立即修复并重新验证。
9. 更新 `memory/claude_plan.md`，记录关键进展、计划变化和验证结果。
10. 更新 `TODO.md` 与 `PLAN.md`，标记完成状态或记录阻塞依赖调整。
11. 使用清晰的提交信息提交本轮所有修改。
12. 停止，不继续处理下一个任务。

## 进展记录

- 已创建计划文件，尚未开始仓库检查。
- 已检查最新提交 `27ebc4af17ba4d621d55de26410c9906c80a36c2`，提交标题为 `[T4004R] 修复顶层 pattern binder 的 hidden suspend 裂缝`，未额外声明需要先补救的遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认当前第一个未完成任务为 `T4005`：把 Elvis `?:` 从静态规则推进到可执行 lowering / codegen。
- 已核对 `ISSUES.md`、规范与源码现状：
  - parser/typecheck 已支持 Elvis；
  - HIR lowering 目前仍把 `BinaryOp::Elvis` 退回 `Any` fallback；
  - LLVM codegen 目前直接报 `elvis operator` unsupported。
- 已用最小 probe 复现当前失败：
  - 输入：`val n: Int? = Some(7); println(n ?: 0)`
  - 结果：`scoop::llvm::unsupported_main_body: elvis operator`

## 当前实施方案

1. 在 HIR lowering 中为 `ast::BinaryOp::Elvis` 增加专用 desugar，优先复用现有 nullable 运算主线，而不是保留到 LLVM 二元运算节点。
2. 目标 lowering 形态为 `when (lhs) { Some(v) -> v; None -> rhs }`，保证：
   - lhs 只求值一次；
   - rhs 仅在 `None` 分支求值；
   - 结果类型与 typecheck 推断一致。
3. 如有必要，补一个 HIR golden fixture，确认 Elvis 不再以 `Binary(Elvis)` 形式残留在 lowered HIR 中。
4. 增加 run-pass 回归，覆盖：
   - `Some(v)` 返回解包值；
   - `None` 走 rhs；
   - rhs 惰性求值。
5. 运行定向 fixtures、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md`、本文件，并提交本轮变更。

## 完成记录

- 已在 HIR lowering 中把 Elvis 收口为 `when (lhs) { Some(v) -> v; None -> rhs }`。
- 已同步收口 typed nullable desugar 的结果类型写回，并让 LLVM `When` codegen 使用表达式静态类型作为结果 expected-context。
- 实现过程中额外发现并修复一个隐藏裂缝：
  - 现象：`val pair: (Any, Int) = (x ?: "fallback", 1)` 这类“Elvis 结果为 `Any` 且处于 tuple element 上下文”的代码，会在 LLVM 阶段报 `when arm type mismatch`。
  - 处理：不新增 Elvis 专用旁路，而是把 nullable desugar 与 `When` 结果合流规则统一收口。
- 已新增回归：
  - `tests/fixtures/hir/elvis_lowering.scoop`
  - `tests/fixtures/run-pass/elvis_lazy_basic.scoop`
  - `tests/fixtures/run-pass/elvis_any_tuple_context_basic.scoop`
- 已完成验证：
  - 最小 Elvis probe：build 成功，运行输出 `7`
  - `Any` / tuple probe：build 成功
  - Elvis 定向 fixtures root：`fixtures: ok (4)`
  - 既有 safe member access 回归 root：`fixtures: ok (1)`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`：`fixtures: ok (17)`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：`fixtures: ok (329)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步仅剩文档状态同步与 Git 提交；本轮不会继续执行 `T4005R`。

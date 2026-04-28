# 执行计划与进度记录

> 说明：本文件记录本次任务的可审计计划、依据、检查点和进度更新。不会记录逐字私密推理，但会尽量完整记录执行路径、关键判断和变更原因。

## 初始目标

- 按照 `TODO.md` 的顺序完成第一个未完成任务，然后停止。
- 在开始任务前检查最新提交是否提到已有问题；如有，先修复这些问题。
- 任何执行过程中发现的既有 bug、回归、规格不匹配、未完成边界或 workaround 都立即纳入范围，优先修复或添加为前置任务后停止。
- 完成一个任务后更新 `TODO.md` 和 `PLAN.md`，运行相关测试，提交 Git commit，然后停止。

## 步骤计划

1. 检查当前仓库状态，确认是否存在未提交变更，避免覆盖用户已有改动。
2. 查看最新 Git 提交信息与变更内容，判断是否提到或暴露任何既有问题。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读相关计划文件和代码上下文，确认该任务的真实范围、依赖关系和测试入口。
5. 若任务过大或依赖缺失：
   - 将任务拆解为更小的子任务；
   - 更新 `TODO.md` 和 `PLAN.md`；
   - 提交拆解/重排变更并停止。
6. 若任务可直接执行：
   - 实现功能或修复；
   - 添加或更新最小但充分的测试/fixture；
   - 运行相关测试，再根据风险运行更广测试；
   - 运行格式化和必要的静态检查。
7. 记录完成情况：
   - 将当前任务在 `TODO.md` 标记完成；
   - 更新 `PLAN.md`；
   - 必要时更新本文件进度。
8. 提交 Git commit，提交信息使用任务编号或清晰描述。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已创建本执行计划。
- 已检查工作区状态：当前只有本计划文件被本轮修改。
- 已检查最新提交：`[T5000i1P5] Default production emission to materialized MIR bodies`，提交说明未直接列出需要先修的既有问题。
- 已阅读 `TODO.md` 条目索引：第一个未完成任务是 `T5000i2 基于 escape facts 接入最小 non-escaping closure simplification`。
- 已阅读 `T5000i2` 附近的任务说明、`PLAN.md` 相关进度，以及 MIR escape facts / pass view / LLVM lowering 的现有实现。
- 发现一个必须先修的既有 MIR lowering 缺口：closure body 当前对表达式 lambda 只 lower body 表达式但没有把结果写成 `Return(Some(...))`，这会阻塞基于 body-known closure 的语义保持简化。

## T5000i2 执行方案

1. 修复 closure MIR lowering：
   - `lower_closure_fun` 在 body 表达式正常完成时显式 `return body_result`；
   - 已提前终止的 closure body 保持原 terminator，不额外生成语句。
2. 新增 backend-agnostic MIR pass（计划放在 `crates/scoopc/src/mir/closure_simplify.rs`）：
   - 只读取 `MaterializedMirPassView::escape_facts()` 中已发布的 facts；
   - 只处理 `EscapeStatus::NonEscaping` 且 direct local call 次数为 1 的 closure；
   - 只处理 `MakeClosure { env: Const(Unit), ... }`，避免尚未有明确 MIR bridge 支持的 capture/env tuple 路径；
   - 要求 closure `fn_ptr` 的 pass-visible body 存在且可按现有直线 inline 子集展开；
   - 将 `CallKind::Closure` 调用点展开成 closure body 的直线语句，然后做窄 dead-copy / dead-closure cleanup。
3. 将 pass 接到 materialization pipeline：
   - 在 summary-driven inlining 之后先运行 escape analysis；
   - 基于 escape facts 运行 closure simplification；
   - 若发生改写，重算受影响 summary，并重新运行 escape analysis，避免 side table 指向已移除 local。
4. 添加测试：
   - non-escaping / body-known / Unit-env closure 被简化；
   - escaping closure 不被简化；
   - `O0` 缺少 escape facts 时不做简化。
5. 更新受 closure return 修复影响的 MIR golden fixtures。
6. 运行格式化、相关 MIR 测试、LLVM production 相关测试、全量测试与 clippy。

## 进度更新

- 已修复 closure MIR lowering：表达式 lambda 正常完成时现在显式返回 body 结果。
- 已新增 `mir/closure_simplify.rs`，实现最小 non-escaping closure simplification：
  - 读取 pass-view escape facts；
  - 仅处理 non-escaping、单次本地 closure call、Unit env、body-known 且可直线展开的 closure；
  - 改写后移除死的 closure 构造 / copy artifact。
- 已将该 pass 接入 materialization pipeline，并在发生改写后刷新 escape facts。
- 已新增 3 个单元测试覆盖 non-escaping 简化、escaping 不简化、O0 无 facts 不简化。
- 已运行 `cargo test -p scoopc mir::closure_simplify -- --nocapture`：通过。
- 已运行 `cargo test -p scoopc mir::escape -- --nocapture`：通过。
- 已运行 `cargo test -p scoopc mir::inline -- --nocapture`：通过。
- 已运行 `cargo fmt --all`：通过。
- 已更新受 closure return 修复影响的 MIR golden fixtures：
  - `tests/fixtures/mir/closure_non_capture.mir`
  - `tests/fixtures/mir/closure_capture_val.mir`
  - `tests/fixtures/mir/closure_capture_var.mir`
  - `tests/fixtures/mir/direct_and_fun_value_call.mir`
- 已运行 `cargo run -p scoop -- test --fixtures tests/fixtures/mir`：通过。
- 已运行 `cargo test -p scoopc production_codegen -- --nocapture`：通过。
- 已运行 `cargo test -p scoopc llvm::tests -- --nocapture`：通过。
- 已运行 `cargo test -p scoopc mir:: -- --nocapture`：通过。
- 已运行 `cargo test -p scoopc --no-default-features`：通过。
- 已运行 `cargo test --all`：通过。
- 已运行 `cargo clippy --all-targets -- -D warnings`：通过。
- 运行完整 fixture suite 时发现 `tests/fixtures/run-pass/generic_fun_recursion.scoop` 退出码为 1，期望为 0。
- 下一步：复现该 fixture，检查是否由 closure return / simplification pass 改动导致 production MIR body 改写后触发后端不支持或运行语义变化；先修复该问题，再继续收尾。

## 继续执行记录（2026-04-28）

- 已复现 `cargo run -p scoop -- run tests/fixtures/run-pass/generic_fun_recursion.scoop` 失败：
  - 报错为 `scoop::llvm::unsupported_main_body`；
  - 具体 kind 为 `pass MIR branch condition`。
- 已确认该失败发生在默认 debug / `O0` 路径；`T5000i2` 新增 closure simplification 只在 escape facts 存在时运行，而 `O0` 不发布 escape facts，因此该失败不是 closure simplification 直接造成。
- 进一步用 `dump-ir` 检查 `repeat::<Int>` / `repeat::<String>` 的 materialized MIR，发现比较表达式 `n <= 0` 的结果 local 仍为 `Any`，而分支条件 lowering 需要 `Bool`。
- 判定为既有 MIR lowering 类型边界缺口：HIR/typecheck 对比较表达式能给出 `Bool`，但 MIR lowering 在部分 generic/template 路径上仍直接沿用过宽的表达式类型，导致 production MIR bridge 进入一个实际不可 lower 的 raw materialized body。
- 修复策略：
  - 在 MIR lowering 中对二元比较、相等与逻辑运算显式使用 `Bool` 结果类型；
  - 这不是 fixture 特判，也不改变语义；它把 MIR local 类型对齐到语言运算符结果类型；
  - 修复后重新跑单个 failing fixture、相关 MIR/LLVM 测试，再继续 T5000i2 收尾。

## 完成记录

- 已修复 MIR 二元表达式结果类型缺口：
  - 比较 / 相等 / 逻辑二元表达式结果 local 明确为 `Bool`；
  - 新增 `dump_mir_types_comparison_condition_as_bool_in_generic_template` 回归；
  - `generic_fun_recursion.scoop` 已单独通过。
- 已完成 T5000i2：
  - `closure_simplify` pass 已接入 materialization pipeline；
  - pass 只消费 pass-view escape facts；
  - 只处理 non-escaping、单次 direct call、Unit env、body-known、直线 body 的保守形状；
  - escaping / unknown / O0 无 facts 路径保持不改写。
- 已更新 `TODO.md` 和 `PLAN.md`，把 `T5000i2` 标记为完成并记录验证结果。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::closure_simplify -- --nocapture`
  - `cargo test -p scoopc mir::escape -- --nocapture`
  - `cargo test -p scoopc mir::inline -- --nocapture`
  - `cargo test -p scoopc mir::lower::tests::dump_mir_types_comparison_condition_as_bool_in_generic_template -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/generic_fun_recursion.scoop`
  - `cargo test -p scoopc production_codegen -- --nocapture`
  - `cargo test -p scoopc llvm::tests -- --nocapture`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`，结果为 `fixtures: ok (1202)`。
- 下一步：检查最终 diff，提交 `[T5000i2] Add MIR non-escaping closure simplification`，然后停止。

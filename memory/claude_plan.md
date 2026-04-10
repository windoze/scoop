# 执行计划

## 说明

用户要求先把思路与执行计划写入本文件，再开始执行命令。我会在这里记录可审计的决策摘要、执行步骤、关键进展和计划变更；不记录不必要的冗长内部推演。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到已知问题或待修复项。
2. 如果最新提交暴露了现存问题，先修复这些问题，并完成验证。
3. 读取 `TODO.md`，定位第一个未完成任务。
4. 读取 `PLAN.md`、必要的仓库说明文件和相关代码，建立任务上下文。
5. 判断该任务是否足够小且可以在本轮完整交付。
6. 如果任务过大：
   - 在 `PLAN.md` 中拆分为更小的子任务；
   - 在 `TODO.md` 中重写或补充为新的子任务列表；
   - 选择新的第一个子任务作为本轮目标。
7. 实现本轮目标。
8. 运行相关格式化、静态检查与测试，至少覆盖：
   - `cargo fmt --check` 或必要时先 `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - 与本次改动直接相关的测试
   - 如需要，再运行更大范围测试
9. 更新文档与计划：
   - 在 `TODO.md` 中标记任务完成，或在受阻时按依赖顺序重排；
   - 在 `PLAN.md` 中更新当前状态、后续顺序和必要说明；
   - 在本文件记录关键进度。
10. 用清晰的提交信息提交本轮改动。
11. 停止，不继续处理下一个任务。

## 执行原则

- 只处理一个任务。
- 不回退或覆盖与当前任务无关的现有修改。
- 若遇到阻塞，保持任务为待办状态，并调整 `TODO.md`/`PLAN.md` 的顺序与说明。
- 在完成实现前，优先保证编译、lint 与测试无告警无错误。

## 进度记录

- 已创建本文件并写入初始执行计划，尚未开始仓库检查。
- 已检查最新提交：`8893a28 [T0148b] 完成 Float 字面量静态语义`。提交说明未直接提到新的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，定位到当前首个未完成任务为 `T0148c`：Float 字面量 LLVM codegen。
- 当前判断：`T0148c` 规模可在本轮完整完成，暂不需要再拆分子任务。

## 当前实现计划（T0148c）

1. 审计 LLVM codegen 现状，确认 Float 已具备哪些基础设施。
2. 补齐缺失的后端路径：
   - 一元负号对 Float 生效；
   - 二元算术对同类型 Float 生效；
   - 比较与相等性对 Float 生效，并采用明确的浮点比较语义；
   - `coerce_value` 支持 `Float64 <-> Float32`，覆盖无后缀字面量吸收到 `Float32` 的后端收窄路径；
   - 如必要，补齐顶层 Float 常量初始化的最小支持。
3. 新增 LLVM 单测与 run-pass fixture，覆盖：
   - 基础算术/比较；
   - 科学计数法；
   - `Float32` 后缀与无后缀吸收到 `Float32`；
   - builtin 方法调用（至少 `toString` / `toInt` / `abs` / `isNaN` / `isInfinite` 的代表性组合）。
4. 运行格式化、clippy、相关测试与 fixture 回归。
5. 更新 `TODO.md` / `PLAN.md` / 本文件，提交本轮改动并停止。

## 当前发现

- `codegen_literal` 已能直接发射 `LiteralKind::Float64/Float32` 的 LLVM 常量。
- 现有主要缺口在：
  - `codegen_unary` 仅支持整数负号；
  - `codegen_binary` 仍只把算术/比较发往整数路径；
  - `codegen_equality` 仅覆盖 Bool/String/Int；
  - `coerce_value` 只接受同类型 Float，不支持 `Float64 -> Float32` 字面量吸收所需的后端收窄；
  - `const_initializer_for_top_level_var` 对 Float 顶层初始化直接报 `UnsupportedMainBody`。

## 已完成实现

1. LLVM 后端已补齐 Float 基础执行链路：
   - 一元负号支持 `Float64/Float32`；
   - `+ - * / %` 支持同类型 Float；
   - `< <= > >=` 使用有序浮点比较；
   - `== !=` 支持 Float，`!=` 采用 unordered-or-not-equal 语义以正确处理 NaN；
   - `coerce_value` 支持 `Float64 <-> Float32`；
   - 顶层 Float 常量初始化支持字面量与一元负号。
2. 新增验证：
   - LLVM 单测：`float_literals_lower_to_arithmetic_comparisons_and_narrowing`
   - run-pass fixture：`tests/fixtures/run-pass/float_literal_runtime_basic.*`
3. 过程中额外修复了两个阻塞本任务验收的实际缺口：
   - `scoop.core.abs/isNaN/isInfinite` 的 codegen 顶层扩展拦截改为基于真实 `CgTy` 判定，避免局部 `VarRef` 的不稳定 `expr.ty` 误导分发；
   - resolver 对 Float builtin API 的“保留为内建 member call”规则补齐短名 `Float64/Float32`，与 typecheck 保持一致。

## 验收结果

- 定向 LLVM 单测通过：
  - `cargo test -p scoopc float_literals_lower_to_arithmetic_comparisons_and_narrowing -- --nocapture`
  - `cargo test -p scoopc float_builtin_methods_lower_to_runtime_calls_and_hash_bits -- --nocapture`
- 严格 lint 通过：
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- 全仓测试通过：
  - `cargo test --all`
- fixture 全回归通过：
  - `cargo run -p scoop -- test`
  - 结果：`fixtures: ok (855)`

## 待收尾

1. 已将 `T0148c` 在 `TODO.md` 标记为完成，并补充完成说明。
2. 已在 `PLAN.md` 记录 `T0148c` 已完成及验收结果。
3. 当前只剩：提交本轮改动并停止。

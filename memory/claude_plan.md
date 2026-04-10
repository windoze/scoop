# 执行记录与当前计划

## 背景与上一轮结果
- 仓库当前基线已经完成上一轮任务拆分与首个子任务实现。
- 原始任务 `T0146c` 被拆分为两个子任务：
  - `T0146c1`：把 `Char` 作为 LLVM 运行期标量值打通。
  - `T0146c2`：补齐 `sysroot/runtime` 的 `Char` API、字符串化、哈希与打印链路。
- 上一轮已完成并提交 `T0146c1`，提交为 `19855ec [T0146c1] Lower Char as LLVM scalar`。
- 当前工作树预期应以该提交为起点，当前轮只允许继续处理 `TODO.md` 中新的首个未完成任务，也就是 `T0146c2`。

## 对用户要求的理解
- 先检查最近一次提交是否提及需要先修的既有问题；如果有，必须先修。
- 只完成 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 如果任务过大，需要拆分并同步更新 `TODO.md` 与 `PLAN.md`，但不能跨到下一个任务。
- 执行中需要持续更新本文件，记录关键进展、计划变更与验证结果。
- 完成后要更新 `TODO.md`、`PLAN.md`，提交 git commit，然后停止。

## 当前任务判断
- 根据上一轮拆分结果，当前首个未完成任务应为 `T0146c2`。
- 该任务目标是让 `Char` 具备完整运行期 API 和文本化能力，使它能像其他基础值类型一样进入 `ToString`、`print`、`println` 与 `hash` 路径。
- 这一轮不继续推进 `T0146c2` 后面的任何任务；如果实现过程中发现范围仍过大，再进一步拆分并只做新的第一个子任务。

## 本轮高层执行计划
1. 读取并确认 `TODO.md`、`PLAN.md`、最近一次提交信息以及与 `Char` 相关的 `sysroot`、runtime、LLVM codegen 现状，验证当前首个未完成任务确实是 `T0146c2`。
2. 检查最近一次提交信息是否包含需要先处理的既有问题；若提交说明没有新增待修项，则直接进入 `T0146c2`。
3. 设计 `Char` 的最小闭环实现，优先复用已有 `Int`/`String`/trait lowering 路径，避免引入额外表示层复杂度。
4. 在 `sysroot/core.scoop` 中补齐 `Char` 的接口定义，至少覆盖：
   - `struct Char : Hashable, ToString`
   - `toInt()`
   - `toString()`
   - `hash()`
   如果现有 trait/内建声明形式要求额外适配，则按仓库既有模式接入。
5. 在 `runtime/c/scoop_runtime.c` 中实现 `scoop_char_to_string(i32 codepoint)` 或等效导出函数，保证 ASCII 与一般 Unicode 标量值都能转成运行时 `String`；若现有字符串构造 API 有限制，则按现有 runtime 约定完成最小正确实现。
6. 在 LLVM codegen 中补齐 `Char.toString()` 与 `Char.hash()` 的成员访问/codegen 路径，并确认 `Char.toInt()` 与平台 `Int` 宽度转换仍然正确。
7. 检查 `print`/`println`/where-bound `ToString` 等调用链中对 `Char` 是否还存在缺口；如果 `Char: ToString` 自动走通，只补缺的地方，不做无关重构。
8. 为本任务补充最小但充分的回归：
   - 直接打印 `Char`
   - `Char.toString()`
   - `Char.hash()`
   - 如任务描述要求涉及多文件场景，则补对应 fixture
9. 运行验证：
   - 先跑与本任务直接相关的 fixture/build 验证
   - 再跑更完整的测试（至少 `cargo test --all` 与 `cargo run -p scoop -- test`）
   - 尝试 `cargo clippy --workspace --all-targets -- -D warnings`
10. 根据结果修正问题；若 `clippy` 失败来自仓库既有基线而非本轮新增问题，记录清楚，不额外扩散范围。
11. 更新 `TODO.md`、`PLAN.md`、本文件，标记 `T0146c2` 完成；然后提交一次清晰的 git commit，并停止。

## 关键风险与检查点
- `Char` 的 sysroot 声明可能已部分存在，需避免与现有语言内建定义冲突。
- runtime 字符串 API 可能要求 UTF-8，而 `Char` 运行值是 Unicode 标量值；需要确认正确编码路径。
- `print/println` 可能并不是直接走 `ToString`，也可能有内建特判；需要以现有实现为准。
- `hash()` 返回值的语义要与仓库中其他标量类型保持一致，优先遵循现有 `Hashable` 约定。
- 如发现 `T0146c2` 仍明显超出一轮可控范围，需要立即回到文档拆分，而不是半做半停。

## 开始执行前的状态
- 当前尚未读取本轮所需源码细节。
- 当前尚未确认 `T0146c2` 是否还需进一步拆分。
- 当前尚未做代码改动。

## 进展更新（本轮实现前）
- 已检查最新提交 `19855ec [T0146c1] Lower Char as LLVM scalar`；提交说明未引入需要先于 `T0146c2` 处理的额外既有问题。
- 已重新核对 `TODO.md` 与 `PLAN.md`：当前首个未完成任务确认为 `T0146c2`，范围不需要继续拆分。
- 已完成现状勘测：
  - `sysroot/core.scoop` 目前有 `Bool` / `String` / `Int` 的 `ToString` 路径，但还没有 `Char` 声明与 `Char.toString()/hash()` 扩展声明。
  - resolver/typecheck 当前只对 `Char.toInt()` 做了最小 special-case，未覆盖 `Char.toString()` / `Char.hash()`。
  - LLVM codegen 已支持 `Char` 作为运行期 `i32` 标量，以及 `Char.toInt()`；但 `codegen_to_string_method`、`codegen_sysroot_to_string_ext`、`try_codegen_tostring_iface_builtin`、`codegen_sysroot_print_like` 仍未接 `Char`。
  - runtime 目前已有 `scoop_bool_to_string` / `scoop_int_to_string` / `scoop_string_hash`，但没有 `scoop_char_to_string`。
- 由此确认的最小实现闭环：
  1. `sysroot/core.scoop` 新增 `struct Char : Hashable, ToString`，并声明 `fun Char.toInt(): Int`、`fun Char.toString(): String`、`fun Char.hash(): Int`。
  2. resolver/typecheck 把 `Char.toString()` / `Char.hash()` 加入与 `Char.toInt()` 同级的 builtin 路径。
  3. runtime 新增 `scoop_char_to_string(int32_t codepoint)`，以 UTF-8 编码一个 Unicode scalar value 到 `ScoopString`。
  4. LLVM codegen：
     - `Char.toString()` 调 runtime `scoop_char_to_string`
     - `Char.hash()` 复用现有 `Int.hash()` mixing 逻辑（对 `i32` codepoint 先 zero-extend 到 `i64`）
     - `print/println` 与 where-bound `ToString` builtin 分发都纳入 `Char`
  5. 新增两个回归：
     - 单文件 run-pass：直接打印 `Char`、`toString()`、`hash()`
     - 多文件 `run_pass_cone`：非入口文件中的 Char 字面量与 Char API

## 进展更新（实现与验证完成）
- 已完成代码实现：
  - `sysroot/core.scoop`：新增 `struct Char : Hashable, ToString`，并声明 `fun Char.toInt(): Int`、`fun Char.toString(): String`、`fun Char.hash(): Int`。
  - `resolve/scopes.rs` 与 `typecheck/expr/call.rs`：`Char.toInt()` / `Char.toString()` / `Char.hash()` 进入 builtin member 路径。
  - `typecheck/assignable.rs`：补齐 `ValueTypeKind::Char -> scoop.core.Char` 的 nominal subtype 判定，使 `Char` 能满足 `where T: ToString` / `where T: Hashable` 这类 interface 约束。
  - `runtime/c/scoop_runtime.c`：新增 `scoop_char_to_string(int32_t codepoint)`，把 Unicode scalar value 编码为 UTF-8；同时接到 `runtime_symbols.rs` / `runtime_abi.rs` / `scoop_runtime_api.h`。
  - `llvm/codegen/mod.rs`：
    - 新增 `Char.toString()` / `Char.hash()` lowering；
    - `print/println` 与 `ToString` builtin dispatch 现在能识别 `Char`；
    - 新增 body-less extension 顶层拦截：`scoop.core.toInt` / `scoop.core.toString` / `scoop.core.hash`；
    - `Char.hash()` 复用现有 `Int.hash()` mixing 逻辑（`i32` codepoint zero-extend 到 `i64`）。
- 在实现过程中额外发现并修正了两个实际缺口：
  1. `where T: ToString` 对 `Char` 最初仍报约束不满足，需要在 `assignable.rs` 中补齐 builtin Char → nominal interface 的上转。
  2. 加入 sysroot 的 body-less `Char.toInt()/hash()` 声明后，HIR 会把它们 lowering 成顶层调用 `scoop.core.toInt/hash(...)`，因此 codegen 需要像 `scoop.core.toString(...)` 一样提供专门拦截，而不能只补 member-call 路径。
- 已新增回归：
  - `tests/fixtures/run-pass/char_runtime_textual_basic.scoop` + `.stdout`
  - `tests/fixtures/run_pass_cone/char_multi_file_runtime_api/**`
- 已完成验证：
  - `cargo fmt`
  - `cargo test --all`：通过
  - `cargo run -p scoop -- build tests/fixtures/run-pass/char_runtime_textual_basic.scoop -o /tmp/char_runtime_textual_basic.out && /tmp/char_runtime_textual_basic.out`：输出与 golden 一致
  - `cargo run -p scoop -- run tests/fixtures/run_pass_cone/char_multi_file_runtime_api`：输出 `Ω / helper=中 / true`
  - `cargo run -p scoop -- test`：通过，`fixtures: ok (851)`
  - `cargo clippy --workspace --all-targets -- -D warnings`：仍失败，但失败原因为仓库既有 baseline（大量 `inkwell` deprecated `ptr_type`、长期 `too_many_arguments` / `result_large_err`），不是本轮引入
- 剩余收尾动作：
  1. 更新 `TODO.md`
  2. 更新 `PLAN.md`
  3. 提交 git commit

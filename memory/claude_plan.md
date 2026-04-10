# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。在开始实际实现前，先核对最新提交是否提到既有问题；若有，优先修复这些问题，再进入任务执行。

## 当前已知约束

- 必须先检查最新提交信息，确认是否提到需要先处理的既有问题。
- 必须读取 `TODO.md`，定位第一个未完成任务。
- 如果该任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 实现后必须补充或运行相关测试，并处理测试失败或告警。
- 必须更新 `TODO.md`、`PLAN.md`、本文件，并提交 Git commit。
- 本轮完成一个任务后立即停止，不继续处理后续任务。

## 初始步骤计划

1. 查看最新一次 Git 提交信息，确认是否存在提交中明确提到但尚未修复的问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前项目计划与该任务的上下文。
4. 判断首个未完成任务是否足够小且可在本轮完整实现。
5. 如果任务过大：
   - 将任务拆分为更小的子任务。
   - 更新 `PLAN.md` 说明拆分后的执行顺序。
   - 更新 `TODO.md`，让第一个子任务成为新的首个未完成任务。
   - 执行新的首个子任务。
6. 如果任务可直接执行：
   - 阅读相关代码、测试与文档。
   - 实现任务。
   - 运行相关格式化、lint 与测试，至少覆盖受影响范围，并尽量满足 `cargo clippy --all-targets -- -D warnings` 无告警。
   - 修复执行中发现的问题。
7. 完成后：
   - 更新 `TODO.md`，将本轮任务标记完成。
   - 更新 `PLAN.md`，记录当前状态和后续建议。
   - 更新本文件，记录关键进展与最终结果。
   - 创建一次清晰的 Git 提交。

## 说明

已完成初步核对，结论如下：

- 最新提交是 `49e2f9caa02a75c540c465b7abb519809b7c2c9f`，提交信息为 `[T0147a] Add Float builtin type plumbing`。
- 该提交及其同步更新的 `TODO.md` / `PLAN.md` 未声明“必须先修”的遗留缺陷；其中明确说明 LLVM 浮点标量映射被有意后置到 `T0147b`，这属于后续任务，不是漏修 bug。
- `TODO.md` 中第一个未完成任务是 `T0147b`：Float LLVM 标量映射（`CgTy` / LLVM type / default value / codegen 穷举补齐）。

## 本轮具体执行方案（已确认）

1. 阅读 LLVM codegen 中与 `CgTy`、类型 lowering、默认值和关键 exhaustive `match` 相关的实现。
2. 对照 `T0146c1`（Char LLVM 标量落地）已有做法，确认 Float 应接入的位置与最小闭环范围。
3. 实现 `T0147b`：
   - 在 `llvm/codegen/types.rs` 新增 `CgTy::Float64` / `CgTy::Float32`。
   - 在 `llvm/codegen/ty.rs` 中把 builtin Float 类型映射到对应 `CgTy`，并让 LLVM basic type lowering 返回 `f64` / `f32`。
   - 在 `llvm/codegen/mod.rs` 与其它关键 codegen 模块补齐 `default_value` 和必要的 `match CgTy` 分支，保证 Float builtin 走共享路径时不会 panic / unreachable。
4. 补充或调整测试：
   - 先跑受影响模块的 Rust 单测与/或定向 fixture。
   - 再跑 `cargo test --all` 与 `cargo run -p scoop -- test`。
   - 尝试 `cargo clippy --workspace --all-targets -- -D warnings`；若仍被仓库既有基线阻塞，则记录清楚。
5. 收尾：
   - 更新 `TODO.md`、`PLAN.md`、本文件。
   - 提交 Git commit。

## 当前状态

- 任务 `T0147b` 看起来边界清晰，暂时不需要继续拆分。

## 进展更新

1. 已完成 `CgTy` 扩展：
   - `crates/scoopc/src/llvm/codegen/types.rs` 新增 `CgTy::Float64` / `CgTy::Float32`。
   - `CgValue` 新增浮点构造与 `as_float()` 读取路径。
2. 已完成 LLVM 类型 lowering：
   - `crates/scoopc/src/llvm/codegen/ty.rs` 中 `cg_ty_of` / `cg_ty_of_type_fqn` 已把 builtin Float 映射到 `CgTy::Float64` / `CgTy::Float32`。
   - `llvm_basic_type_of` 已返回 `context.f64_type()` / `context.f32_type()`。
3. 已补齐主 codegen 共享标量路径：
   - 参数接收、默认值、返回值、top-level/local load、lambda 参数、函数值调用签名、operator overload 返回值、`as_llvm_arg_value`、`cg_value_from_loaded` 等均已纳入 Float 分支。
4. 已补齐与标量存储相关的共享路径：
   - `control_flow.rs` 的 `if/when` 结果槽位与 enum payload 提取。
   - `gc.rs` 的 `store_local_value`。
   - `layout.rs` 的 `cg_ty_layout`（Float64=8/8，Float32=4/4）。
   - `effect.rs` 中多处 result slot / continuation resume word / `coerce_u64_word` 路径，已让 Float 作为可编码到 `u64` word 的标量参与共享流程。
5. 已新增 LLVM 单测：
   - `crates/scoopc/src/llvm/mod.rs`：新增 `float_builtin_types_lower_to_llvm_scalars`，断言 `Float64` / `Float32` 在 IR 中落为 `double` / `float`，并验证 extern ABI 与调用签名。
6. 当前验证状态：
   - `cargo check -p scoopc --features llvm` 已通过。
   - 仍存在大量仓库既有 warning（主要是 inkwell deprecated API、少量 unused/private_interfaces/dead_code），目前尚未跑 `clippy`，后续会明确记录是否为本轮新增。

## 下一步

1. 运行 `cargo fmt` / `cargo fmt --check`。
2. 运行与本任务直接相关的测试（至少 LLVM 单测）。
3. 运行更完整的 `cargo test --all` 与 `cargo run -p scoop -- test`。
4. 尝试 `cargo clippy --workspace --all-targets -- -D warnings`，记录是否被既有 baseline 阻塞。
5. 更新 `TODO.md` / `PLAN.md` / 本文件，并提交 commit。
## 续作计划（2026-04-10，本轮接手）

### 当前判断

- 首个未完成任务仍然是 `T0147b`，上一轮已经完成主要实现与定向单测。
- 还缺少完整收尾：补跑必要验证、确认 `scoop test` 的运行方式、更新 `TODO.md` / `PLAN.md` / 本文件，并提交 commit。
- 现阶段不处理 `T0147c` 或任何后续任务。

### 本轮执行步骤

1. 重新运行工作区级验证，优先确认 `cargo test --all`。
2. 重新确认 fixture 测试运行方式，优先尝试先构建 `scoop`，再直接执行 `target/debug/scoop test`，避开 `cargo run` 触发的 `(deleted)` 可执行路径问题。
3. 运行 `cargo clippy --workspace --all-targets -- -D warnings`，确认是否仍被既有基线 warning 阻塞；若阻塞，记录为非本任务新增。
4. 若验证结果支持收尾，则更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`，把 `T0147b` 标记完成并记录测试情况。
5. 检查工作区变更，使用与任务一致的提交信息提交本轮结果，然后停止。

## 本轮执行结果（2026-04-10，本轮接手）

### 验证结果

- `cargo test --all` 已通过。
- `cargo build -p scoop` 已通过。
- `target/debug/scoop test` 已通过，结果为 `fixtures: ok (852)`。
- `cargo run -p scoop -- test` 已再次通过，结果为 `fixtures: ok (852)`。
- 先前出现的 `/target/debug/scoop (deleted)` 未再复现，结合本轮直接运行与 `cargo run` 都通过，判断更像是上一轮并发运行 / 构建时序导致的偶发现象，而不是 `T0147b` 的功能回归。
- `cargo clippy --workspace --all-targets -- -D warnings` 失败，且失败来源仍是仓库既有基线：
  - 大量 `inkwell` deprecated `ptr_type` / `ptr_sized_int_type_in_context`
  - 长期存在的 `too_many_arguments`
  - 长期存在的 `result_large_err`
  - 少量 `unused_variables` / `private_interfaces` / `dead_code`
- 本轮未对这些基线问题做扩面修复；当前任务相关改动路径未观察到新增的 task-specific clippy 失败。

### 收尾动作

- 已更新 `TODO.md`，将 `T0147b` 标记为 `[DONE]` 并补写实现/验证记录。
- 已更新 `PLAN.md`，将 `T0147b` 从 `PENDING` 改为 `DONE` 并记录验证结论。
- 下一步只剩检查工作区变更并提交本轮 commit；提交后立即停止，不进入 `T0147c`。

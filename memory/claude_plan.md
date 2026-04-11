# 本轮执行计划

## 约束说明

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在真正改代码前，先检查最新提交是否提到需要先修复的遗留问题；若有，则这些问题优先于 `TODO.md` 任务。
- 如首个未完成任务过大，需要先把它拆成更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
- 不记录或暴露内部逐词推理；这里保留的是可审计的高层分析、执行计划和进度更新。

## 初始步骤

1. 查看最新一次 git 提交信息，确认是否提到已有问题、回归、临时修复或后续待补项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关模块和测试，判断该任务是否可在本轮完整完成。
4. 若任务过大，先拆分任务并更新 `PLAN.md` / `TODO.md`；若可直接做，则进入实现。

## 实施步骤

1. 修改代码，尽量保持改动局部且模块化。
2. 为改动补充或更新测试。
3. 运行相关验证：
   - 至少运行与任务直接相关的测试；
   - 如改动范围允许，运行更完整的检查，例如 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
4. 若验证失败，先修复再继续。

## 交付步骤

1. 更新 `TODO.md`，把本轮完成的任务标记为已完成。
2. 更新 `PLAN.md`，反映当前状态、后续顺序以及必要的任务拆分/依赖说明。
3. 按需要回写本文件，记录关键进展、计划变化和验证结果。
4. 提交 git commit，提交信息使用任务编号或清晰描述。
5. 停止，不继续处理下一个任务。

## 进度记录

- 2026-04-11：已创建本轮计划文件，下一步将检查最新提交与 `TODO.md`。
- 2026-04-11：已检查最新提交 `53bbee3 [T2001] Allow mixed handle arm kinds`，提交信息本身未携带额外需先修复的遗留问题说明。
- 2026-04-11：已确认当前首个未完成任务是 `T2002`。
- 2026-04-11：已阅读 `llvm/codegen/effect.rs`、`llvm/codegen/mod.rs`、`runtime/c/scoop_runtime.c`、相关 fixtures 与 `ISSUES.md`，得到当前状态：
  - `Continuation.resume` 的 runtime ABI 已是双通道（`resume_word + resume_gc_ref`），支持 ref/复合值；
  - non-resuming custom effect 仍限制在“单参数 + `Int` payload + 单 word slot”；
  - non-resuming 的跨函数/间接 perform 分发路径已具备 flag-propagation + handler stack dispatch，但 payload 仍是 `Int`；
  - call-site suspension / CalleeSuspendState 对 escape continuation 的恢复值仍主要停留在 `resume_word` / 标量路径。
- 2026-04-11：判断原始 `T2002` 过大，准备拆分为更小子任务后再执行。本轮拟先处理“non-resuming 单 payload ABI 泛化（String/ref/aggregate，覆盖 direct + indirect perform）”，其余 escape-continuation / callee-suspend payload 泛化留给后续子任务。
- 2026-04-11：已将 `TODO.md` / `PLAN.md` 中的原 `T2002` 拆分为：
  - `T2002a`：non-resuming 单 payload ABI 泛化（本轮执行）。
  - `T2002b`：escape continuation / CalleeSuspendState 恢复值 ABI 泛化（留待下轮）。
- 2026-04-11：`T2002a` 实现已完成，关键改动：
  - `runtime/c/scoop_runtime.c`：effect perform slot 新增 `payload_gc_ref` 与 `write_u64_with_gc_ref` / `read_gc_ref`，slot 生命周期内自动 pin/unpin；
  - `crates/scoopc/src/llvm/codegen/effect.rs`：新增共享 ABI payload encode/decode helper；non-resuming perform/handler 改走 `word + gc_ref`；`Continuation.resume` 复用同一编码逻辑；
  - `crates/scoop_runtime/tests/effect_tls.rs`：补 `gc_ref` 通道可观测性测试；
  - `tests/fixtures/run-pass/`：新增 `effect_nonresuming_payload_string_direct`、`effect_nonresuming_payload_struct_indirect` 回归。
- 2026-04-11：验证结果：
  - `cargo test --all`：通过；
  - `cargo run -p scoop -- test`：通过，`fixtures: ok (908)`；
  - `cargo run -p scoop --features llvm -- test`：通过，`fixtures: ok (908)`；
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过。
# 2026-04-11 本轮执行计划（续接上一代理）

## 约束与目标

- 本轮只处理 `TODO.md` 中当前第一个未完成任务之前已经拆分并完成实现的 `T2002a` 收尾工作。
- 不继续实现 `T2002b`。
- 在任何 shell 命令之前，先把本轮计划和执行意图记录到本文件。

## 已知交接信息

- 上一代理已完成 `T2002a` 的代码实现、测试验证，以及 `TODO.md` / `PLAN.md` / 本文件的阶段性更新。
- 已通过的验证包括：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 尚未完成的唯一收尾项是：复核当前工作树后提交 git commit。

## 本轮步骤

1. 复核最新提交信息，确认是否存在必须先处理的遗留问题；若没有，则不做额外修改。
2. 复核当前工作树，仅确认未提交变更与 `T2002a` 范围一致。
3. 复核 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 的状态，确保 `T2002a` 已记录为完成，`T2002b` 仍保留待后续轮次处理。
4. 如工作树内容与交接一致，则以 `[T2002a] Generalize non-resuming effect payload ABI` 提交。
5. 提交后停止。

## 说明

- 这里记录的是可审计的执行计划与决策摘要，不包含不可审计的内部推理细节。

## 本轮进展补记

- 2026-04-11：已复核最新提交 `53bbee3 [T2001] Allow mixed handle arm kinds`，提交说明未包含需要优先插队修复的遗留问题。
- 2026-04-11：已复核当前工作树，未提交改动与 `T2002a` 范围一致：
  - `codegen/mod.rs` 仅为注释中旧函数名更新；
  - 其余代码、测试与任务文档改动均对应 non-resuming 单 payload ABI 泛化及其验证。
- 2026-04-11：下一步执行 git commit，提交后停止，不进入 `T2002b`。

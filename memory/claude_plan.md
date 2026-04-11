# 执行计划与进度记录

说明：
- 不记录不可审计的内部推理细节，但会持续记录可检查的执行计划、关键判断、操作结果与后续调整。
- 本次目标是：先处理最新提交中提到的既有问题，再完成 `TODO.md` 中第一个未完成任务，完成后测试、更新文档、提交并停止。

初始计划：
1. 检查最新一次 Git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务顺序是否一致。
4. 如果首个未完成任务过大，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本次只执行拆分后的第一个子任务。
5. 实施当前目标任务，修改代码时尽量保持改动集中、模块边界清晰，并补充必要的注释与文档。
6. 运行与改动直接相关的测试；如有必要，补充或修正测试。随后执行格式化、lint 与必要的全量/子集测试，确保无警告。
7. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态、变更原因、测试结果与遗留风险。
8. 提交本次改动，提交信息与任务编号保持清晰对应。
9. 停止，不继续处理下一个任务。

待确认事项：
- 最新提交是否包含必须优先修复的问题。
- `TODO.md` 中第一个未完成任务的范围、依赖与可执行性。
- 需要运行的最小测试集合与最终验证命令。

进度更新（已完成的审计）：
- 已检查最新提交 `834d0e780f2ba6978dae7655ac47561ca9fd2621`。该提交只改动 `PLAN.md`、`TODO.md`、`memory/claude_plan.md`，没有代码修复；提交信息指向的既有问题是“LLVM 仍不支持多 arm handle dispatch”，没有额外已提交未修的代码补丁需要抢先处理。
- 已确认 `TODO.md` 当前首个未完成任务是 `T2003c0`：LLVM 多 arm handle dispatch（mixed-arm immediate-resume 前置能力）。
- 已确认 `PLAN.md` 也把下一步指向 `T2003c0`。

关键代码发现：
- `crates/scoopc/src/llvm/codegen/effect.rs` 中 `codegen_handle_expr` 仍在 `handle.arms.len() != 1` 时直接报 `handle arm count (only 1 supported)`。
- 现有三条 lowering 链路彼此独立：
  - `codegen_handle_expr_immediate_resume`：直接扫描并手工拦截唯一 direct perform，使用栈上 state machine；
  - `codegen_handle_expr_nonresuming_single_payload`：依赖 `effect_unwind_target_stack` + `raise_target_stack` 处理 direct/indirect perform；
  - `codegen_handle_expr_escape_continuation`：对 direct perform / indirect perform 维护独立的 continuation state machine。
- 因此不能简单把多 arm `handle` 机械降成嵌套单 arm `handle`：
  - 这会让同一 source-handle 的 sibling arms 在某个 arm body 执行期间仍然保持活跃；
  - 与“arm body 在整组 sibling handler scope 之外执行，避免 sibling self-capture”的语义冲突。

范围判断：
- 原始 `T2003c0` 若一次性同时覆盖 immediate-resume + sibling non-resuming + sibling escape-continuation，多条 lowering 需要一起重构，风险过高。
- 适合先拆分：
  1. `T2003c0a`：一个 immediate-resume arm + sibling non-resuming arms 的 LLVM 多 arm dispatch；
  2. `T2003c0b`：在此基础上补 sibling escape-continuation arm，并收口其余稳定诊断。

接下来的执行计划（准备落地）：
1. 先更新 `TODO.md` / `PLAN.md`，把 `T2003c0` 拆成 `T2003c0a` 与 `T2003c0b`，并将本轮目标切换到 `T2003c0a`。
2. 在 LLVM effect codegen 中实现 shared multi-arm dispatch 的最小子集：
   - 允许一个 immediate-resume arm；
   - 允许若干 sibling non-resuming arms（含 `Raise.raise` 与单 payload custom effect）；
   - 对 sibling escape-continuation arm 给出稳定诊断，明确延后到 `T2003c0b`。
3. 补充 fixtures：
   - run-pass：mixed-arm immediate-resume + non-resuming 的最小可运行路径；
   - build-fail：mixed-arm immediate-resume + escape-continuation 仍未支持时的稳定诊断。
4. 运行格式化、相关测试、全量 lint，并在通过后更新 `TODO.md` / `PLAN.md` / 本文件，最后提交并停止。

进度更新（实现完成）：
- 已将 `TODO.md` / `PLAN.md` 中的原 `T2003c0` 拆成 `T2003c0a` / `T2003c0b`，并把本轮目标落到 `T2003c0a`。
- 已在 `crates/scoopc/src/llvm/codegen/effect.rs` 中实现 mixed-arm LLVM lowering 的第一阶段：
  - `codegen_handle_expr` 在多 arm 场景下不再直接报 arm-count 门禁，而是进入新的 mixed-arm 分流；
  - 支持“一个 immediate-resume arm + sibling non-resuming arms”的最小子集；
  - sibling non-resuming 当前覆盖 `Raise.raise` 与单 payload custom non-resuming effect；
  - sibling escape-continuation arm 当前会稳定报 `handle mixed-arm escape continuation not yet supported`，留给 `T2003c0b`。
- 关键实现策略：
  - immediate-resume 继续复用现有 direct-perform scan + state machine helpers；
  - body/resume 阶段额外挂上 mixed-arm effect dispatch，把 `Raise.raise` 与 indirect custom perform 路由到对应 sibling arm；
  - custom sibling arms 在 body/resume 阶段通过 runtime handler stack 参与 dispatch，但在任意 arm body 执行期间会从 TLS handler stack 中整体摘除，避免 sibling self-capture；resume 后再恢复 body 所需 dispatch scope。
- 已新增 fixtures：
  - run-pass：`effect_resume_mixed_custom_nonresuming_dispatch`
  - run-pass：`effect_resume_mixed_raise_dispatch`
  - build-fail：`effect_resume_mixed_escape_is_error`

测试与验证结果：
- 定向 smoke：
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_mixed_custom_nonresuming_dispatch.scoop -o /tmp/effect_resume_mixed_custom_nonresuming_dispatch` 成功，程序输出与 golden 一致。
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_mixed_raise_dispatch.scoop -o /tmp/effect_resume_mixed_raise_dispatch` 成功，程序输出与 golden 一致。
  - `cargo run -p scoop --features llvm -- build tests/fixtures/build/effect_resume_mixed_escape_is_error.scoop --emit-llvm -o /tmp/effect_resume_mixed_escape_is_error.ll` 按预期失败，并命中 `handle mixed-arm escape continuation not yet supported`。
- 全量验证：
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (921)`）。
  - `cargo run -p scoop --features llvm -- test` 通过（`fixtures: ok (921)`）。
  - `cargo clippy --workspace --all-targets -- -D warnings` 通过。

收尾动作：
1. 把 `T2003c0a` 在 `TODO.md` 标记为完成，并写入完成说明。
2. 更新 `PLAN.md`，把下一步切换到 `T2003c0b`。
3. 检查工作区差异，提交本轮改动并停止。

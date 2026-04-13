# 当前执行计划

## 约束说明

- 不记录不可审计的完整内部推理；此文件记录可执行步骤、判断依据摘要、进度与变更。
- 本轮只处理 `TODO.md` 中第一个未完成任务；若被前置缺陷阻塞，则先把阻塞项整理进 `TODO.md` / `PLAN.md`，提交后停止。

## 执行步骤

1. 检查最近一次 Git 提交信息，确认是否提到已有已知问题；若有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、必要的规范/相关代码，判断该任务是否可直接完成。
4. 如果任务过大或存在明确前置依赖：
   - 将任务拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的顺序与依赖；
   - 本轮执行拆分后的第一个子任务，或若被阻塞则提交规划调整后停止。
5. 实现当前目标任务，修改最小必要代码，避免引入规避性方案。
6. 运行相关测试，并补充/修复必要测试。
7. 运行质量检查，至少包括与改动相关的测试；若可行，执行 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
8. 更新文档与进度：
   - 在 `TODO.md` 中标记完成或调整顺序；
   - 在 `PLAN.md` 中记录当前状态；
   - 在本文件补充已完成步骤与任何计划变更。
9. 检查工作区差异，确认只包含本轮需要提交的变更。
10. 使用清晰的提交信息提交改动，然后停止，不继续下一个任务。

## 进度记录

- 已创建计划文件，待开始检查最新提交与任务列表。
- 已检查最新提交 `9fd284bbf0febff7583195e7662ba45552e683ce`；提交说明未显式提到需要先修复的遗留 issue。
- 已定位首个未完成任务：`T2003r3d2b`。
- 已阅读 `TODO.md` / `PLAN.md` 对应段落，确认 `T2003r3d2b` 已拆分得足够细，本轮直接执行，不再继续拆分。
- 已定位当前真实缺口：
  - `crates/scoopc/src/llvm/codegen/effect/nonresuming.rs` 中 unified single-resuming 入口仍对 `SingleImmediateResume` / `SingleEscapeContinuation` 执行 `unimplemented!`。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中 `resume(value)` 与 `k.resume(value)` builtin lowering 仍是 `unimplemented!`。
- 已确认 metadata / resolver 前置条件已经存在：
  - `resolve_immediate_resume_site_from_plan(...)`
  - `resolve_escape_direct_sites_from_plan(...)`
  - `resolve_escape_indirect_sites_from_plan(...)`
  - `collect_escape_capture_metas_from_plan(...)`
- 当前执行细化计划：
  1. 参考历史实现，提炼 single immediate-resume 与 single escape-continuation 的 leaf/helper。
  2. 以统一 single-resuming helper 的形式接回当前 unified 入口，避免恢复旧 shape-based route 名称或旧主选路。
  3. 接回 `resume(value)` 与 `k.resume(value)` builtin lowering。
  4. 运行定向测试与 representative LLVM fixture。
  5. 若定向验证通过，再更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 已完成 unified single-resuming 主线接回：
  - `codegen_handle_expr_unified_single_resuming(...)` 已能分流到 single immediate-resume / single escape-continuation leaf，不再对这两个 unified 形态 `unimplemented!`。
  - `resume(value)` 与 `k.resume(value)` builtin lowering 已接回 unified helper，不再保留占位实现。
  - 已补 `single_resuming.rs` / `single_escape.rs` 以及对应 shared helper，使 unified plan 的 metadata / resolver 能直接驱动 single leaf。
- 已完成的验证摘要：
  - `cargo build -p scoopc`
  - `cargo test -p scoopc unified_single_resuming_entrypoint_ -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo test --all`
  - representative LLVM 运行：
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_while_body_single_perform.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_perform_in_if_branch.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
- 当前剩余步骤：
  1. 运行 `cargo fmt --all`。
  2. 运行 `cargo clippy --workspace --all-targets -- -D warnings`，修复任何新增 warning。
  3. 更新 `TODO.md` / `PLAN.md`，把 `T2003r3d2b` 标记完成并记录验证结果。
  4. 再次更新本文件的最终状态，然后提交并停止。
- `cargo fmt --all` 已通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 首轮发现两个真实接口问题：
  - `single_resuming.rs` / `single_escape.rs` 新接回的 unified leaf 函数参数过多；
  - 收口参数后，新上下文结构的可见性不足以支撑 `pub(super)` 方法签名。
- 已修复上述问题：
  - 新增 `UnifiedSingleResumingLeafCtx`，把 single-resuming leaf 的共享输入收口为统一上下文，而不是保留 7+ 个散落参数；
  - 已把上下文结构可见性提升到与调用边界匹配的层级；
  - 重新运行 `cargo fmt --all` 与 `cargo clippy --workspace --all-targets -- -D warnings`，现已通过。
- 任务验收相关验证已重新跑完：
  - `cargo test -p scoopc unified_single_resuming_entrypoint_ -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_while_body_single_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_perform_in_if_branch.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 文档同步状态：
  - `TODO.md` 已将 `T2003r3d2b` 标记为完成，并补入完成说明与实际验收命令。
  - `PLAN.md` 已记录本轮 single-resuming leaf 接回、lint 收口与验证结果，并将下一步调整为 `T2003r3d2c`。
- 当前最终待办：
  1. 检查工作区差异，确认本轮只包含 `T2003r3d2b` 所需变更。
  2. 提交 git commit。
  3. 停止，不继续下一个任务。

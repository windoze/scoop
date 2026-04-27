# 执行计划

## 约束

- 本次只处理 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始任务前先检查最新提交是否提到已有问题；如有，优先修复所有既有问题。
- 发现任何已有 bug、规格不匹配、未完成边界或测试回归时，必须先修复；若无法立即修复，则把它作为前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。
- 不使用绕过、弱化测试、夹具专用逻辑或规格偏离来完成任务。
- 执行中每次关键进展或计划变化都更新本文件。
- 输出、记录和最终说明使用中文。

## 初始步骤

1. 查看最新提交，确认是否提到未解决问题、回归、TODO、临时方案或相关风险。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 和与该任务相关的源码、测试、规格或夹具，确认任务边界和依赖。
4. 若第一个任务过大，先拆成较小子任务，更新 `PLAN.md` 与 `TODO.md`，提交拆分结果并停止。

## 执行步骤

1. 实现第一个未完成任务或当前拆分出的第一个子任务。
2. 添加或更新最小但充分的测试覆盖。
3. 运行相关测试；若有失败，定位并修复。
4. 运行必要的质量检查，优先使用仓库既有命令；若完整检查成本过高，至少运行与改动相关的测试并记录未运行项。
5. 更新 `TODO.md`，把本次完成的任务标记为完成。
6. 更新 `PLAN.md`，记录实际完成内容、测试结果和后续状态。
7. 提交所有相关修改，使用带任务编号或清晰描述的提交信息。
8. 停止，不继续处理下一个任务。

## 当前状态

- 状态：已完成初始勘查。
- 最新提交：`817a2aead5293aa64f037a4e7e26b86cf6251a79`，标题为 `[T5000h0eR] Review pass-view production codegen consumption`。
- 最新提交检查结论：提交记录本身没有提到仍需优先修复的未解决问题；其中提到的 source-context 边界不一致已经在该提交中修复。
- 工作区状态：除本文件外暂无其他未提交改动。
- `TODO.md` 中第一个未完成任务：`T5000h 在 MIR 层实现 summary-driven inlining`。
- 当前任务验收重点：
  - 内联触发必须来自 MIR 结构、per-instance summary、`DirectCallOnly` 参数使用和 provenance，不能按函数名白名单。
  - 需要覆盖普通小 direct call 边界消除，以及高阶 wrapper 中函数值参数调用摊平。
  - inline 后 body 必须通过 `MaterializedMirPassView` 进入 production codegen，而不是只影响 dump/debug 路径。
- 已确认原 `T5000h` 横跨三个实现边界，单轮过大，已更新 `TODO.md` / `PLAN.md` 拆为：
  - `T5000h1`：在 pass-visible MIR callable body 上实现保守 small direct-call inlining；
  - `T5000h2`：让 caller-side MIR pass 能安全覆盖 request-root / non-generic callable body；
  - `T5000h3`：接入 `DirectCallOnly` + known provenance 的高阶 wrapper 摊平。
- 拆分后的本轮任务：`T5000h1`。
- `T5000h1` 执行计划：
  1. 新增 MIR inlining pass 模块，遍历 `MaterializedMirPassView` 可见 callable body。
  2. 使用 per-instance summary 做 eligibility：`body_known`、非递归、小体量，且不按函数名白名单。
  3. 先支持单块 straight-line callable 的 direct-call inline，保守跳过 CFG、todo、effect/handler 等尚未覆盖节点。
  4. 将 rewritten body 写入 `MaterializedMirPassArtifacts`，并写入保守更新后的 summary。
  5. 接到 materialization 主路径，保证 production frontend 自动拥有 pass-rewritten body。
  6. 添加 MIR/pass-view 与 LLVM production 回归，确认 `wrap<Int>` 的 pass body 不再调用 `id<Int>`，且 raw materialized MIR 不被覆盖。
  7. 运行格式化、定向测试、相关 LLVM/MIR 测试、全量测试与 clippy。

## T5000h1 进展

- 已新增 `crates/scoopc/src/mir/inline.rs`：
  - 遍历 pass-visible monomorphic callable roots；
  - 只内联 `body_known`、非递归、`size_cost <= 16`、单块 straight-line 的 direct call callee；
  - 不按函数名触发；
  - inline 后通过 `replace_callable_body(...)` / `set_instance_summary(...)` 写入 pass artifacts。
- 已在 `crates/scoopc/src/mir/materialize.rs` 的 materialization 返回前接入 `run_summary_driven_inlining(...)`。
- 已在 `crates/scoopc/src/mir/summary.rs` 添加 `summarize_pass_rewritten_fun(...)`，为 rewritten body 生成保守 summary，并保留上一版 outward-effect / recursion 上界。
- 已新增回归：
  - `mir::inline::tests::small_direct_call_inlining_rewrites_pass_body_without_mutating_raw_mir`
  - `mir::inline::tests::small_direct_call_inlining_is_not_name_based`
  - `llvm::tests::production_codegen_observes_summary_driven_mir_direct_call_inlining`
- 已通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::inline -- --nocapture`
  - `cargo test -p scoopc production_codegen_observes_summary_driven_mir_direct_call_inlining -- --nocapture`
- 第一轮扩大验证发现并修复：
  - 新自动 inliner 让旧的 pass body-presence / manual override 测试前置假设变化，已分别改为检查 `id` 不再被发射或调用，以及从 raw materialized body 构造手动 override；
  - `frontend_codegen_consumes_materialized_generic_direct_call_instances` 暴露出既有 TypeStore 边界问题：pass MIR body 的 local `TypeId` 来自 `MaterializedMir.types`，而 production MIR body lowering 原先用 HIR `lowered.types` 解码；已修正 `mir_body.rs`，MIR local type lowering 改从 `MaterializedMirPassView::materialized().types` 读取，并仅在 aggregate 需要时映射回 codegen TypeStore。
- 已继续通过：
  - `cargo test -p scoopc frontend_codegen_consumes_materialized_generic_direct_call_instances -- --nocapture`
  - `cargo test -p scoopc production_codegen_body_emission_observes_pass_view_body_presence -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_overridden_pass_mir_body -- --nocapture`
  - `cargo test -p scoopc llvm::tests -- --nocapture`
  - `cargo test -p scoopc mir:: -- --nocapture`
- 已完成全量验证：
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1201)`）
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md`，将 `T5000h1` 标记为 `[DONE]`，并记录实现、既有问题修复与验证结果。
- 已更新 `PLAN.md`，记录 `T5000h1` 完成状态、TypeStore 边界修复、测试结果和下一任务。
- 下一步：检查最终 diff，然后提交 `[T5000h1] Implement summary-driven MIR direct-call inlining`。

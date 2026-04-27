# 执行计划与进度日志

## 2026-04-27 接手 T5000hR 半成品
- 用户明确说明 `T5000hR` 仍未完成，上一个 `codex` 卡住；当前未提交文件视为半成品改动，不直接相信 `TODO.md` / `PLAN.md` 中已经标记完成的状态。
- 本次目标：审计当前 change set，先修复任何编译器 / clippy / fixture 问题，再确认 `T5000hR` 的 review 结论是否真正成立。
- 最新提交为 `121510bb [T5000h3] Flatten higher-order wrappers via MIR provenance`，提交标题没有声明额外已知问题；本轮已发现的实际问题以当前未提交 diff 与测试结果为准。

## 2026-04-27 接手执行计划
1. 查看当前未提交 diff，识别半成品改动的设计意图、接口变更和可能的编译断点。
2. 先运行最小编译/测试命令暴露真实错误；若遇到编译器问题，先修编译问题，再回到 review / fixture 验证。
3. 修复 opt-level gate、materialization options、production LLVM entry 或测试中不一致的地方，保持 `-O0` 只关闭 summary-driven MIR inlining，不关闭必要的 exact / singleton dispatch directization。
4. 运行针对性测试、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
5. 按用户要求完整运行 fixture：`cargo run -p scoop -- test`，确认没有回归。
6. 若全部通过，再更新 `TODO.md` / `PLAN.md` / `OPTIMIZATION.md` 的完成记录与实际验证结果。
7. 按 `PROMPT.md` 收尾：检查 diff，提交本轮改动并停止，不推进 `T5000i`。

## 约束说明
- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后提交并停止。
- 在执行目标任务前，先检查最新提交是否提到已有问题；若存在已知问题，优先修复。
- 遇到任何已有 bug、回归、规格不一致、未完成边界或临时绕过，立即纳入范围：能修则修；若阻塞当前任务且无法直接修复，则把前置修复任务插入 `TODO.md` 并提交后停止。
- 不采用 fixture-only hack、弱化测试、替代表示或其它绕过方式。
- 输出与进度记录使用中文。

## 初始执行计划
1. 检查最新提交信息与变更摘要，判断是否提到必须优先处理的已有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务；必要时同时查看 `PLAN.md` 获取上下文。
3. 若第一个任务过大，拆成更小子任务，更新 `TODO.md` 与 `PLAN.md`，提交拆分结果后停止。
4. 针对本轮任务阅读相关代码、规格与测试，确认正确实现路径。
5. 实现任务，保持改动范围最小且符合现有代码结构。
6. 添加或更新针对性测试；运行相关测试、`cargo test --all`，并在可行时运行 `cargo clippy --all-targets -- -D warnings`。
7. 修复测试或 lint 中暴露的真实问题；若发现阻塞性规格缺口，按规则转为前置任务并停止。
8. 更新 `TODO.md` 标记本轮任务完成，更新 `PLAN.md` 记录状态。
9. 查看 git diff，提交本轮改动，提交信息使用任务编号或清晰描述。
10. 停止，不继续下一个任务。

## 当前状态
- 已检查最新提交：`121510b [T5000h3] Flatten higher-order wrappers via MIR provenance`，提交标题未声明必须先处理的已知问题。
- 已读取 `TODO.md` / `PLAN.md` 并定位本轮第一个未完成任务：`T5000hR Review：确认 inlining 已走 summary / structure 路线`。
- 本轮执行边界：只做 `T5000hR` review；若 review 暴露真实缺陷，先修复缺陷并纳入本条完成记录；通过后更新 `TODO.md` / `PLAN.md`、提交并停止。

## T5000hR Review 执行计划
1. 阅读 `TODO.md` / `PLAN.md` 中 `T5000h0*`、`T5000h1`、`T5000h2`、`T5000h3` 与 `T5000hR` 的任务说明和完成记录，确认验收边界。
2. 复核最新提交涉及的 `crates/scoopc/src/mir/inline.rs` 与相关 pass/materialized view 接线，确认 inlining 触发基于 per-instance summary、callable family、known provenance / `DirectCallOnly` 等结构事实，而不是函数名白名单。
3. 搜索 MIR inline / pass / production codegen 路径中是否存在新的 ad-hoc FQN 特判、只服务 fixture 的绕过、或未消费 pass-rewritten body 的残留主路径。
4. 运行针对性测试覆盖 MIR inlining / production pass view / LLVM 接线；再运行 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 若发现真实缺陷，按规格修复并补充回归；若发现任务过大或依赖缺口，插入前置任务并提交后停止。
6. 若 review 通过，更新 `TODO.md` 与 `PLAN.md` 的 `T5000hR` 完成记录，提交本轮 review。

## Review 发现
- 复核 `OPTIMIZATION.md` 与当前 materialization 接线时发现真实边界问题：新的 summary-driven inlining pass 不应默认进入 `-O0`，但 `crates/scoopc/src/mir/materialize.rs` 当前在构造 `MaterializedMir` 后无条件调用 `run_summary_driven_inlining(...)`。
- 处理方向：把 summary-driven MIR inlining 的运行条件接到现有 `OptLevel`，使 debug / `-O0` 只保留 raw materialization、必要实例发现与已由 `T5000g` 建立的 exact/singleton dispatch classification；`O1+` 才运行 inlining pass。dump/debug API 可继续显式使用 pass 以支撑调试和单测，但 production build 路径必须按 opt level 控制。
- 实现进展：已新增 `OptLevel::enables_summary_driven_mir_inlining()`；`materialize_compilation_unit_from_typechecked_inputs`、single-file LLVM frontend 与 build frontend 已接收并传递 `OptLevel`；`O0` 关闭 summary-driven inlining，但保留 exact/singleton dispatch directization 以维持 build fixture 与 owner-specialized instance materialization 语义；新增 LLVM 回归锁定 O0 不覆盖 pass body、O2 会覆盖 pass body。
- 继续接手后先修复了半成品改动中的编译错误：`materialize_for_dump_with_opt_level(...)` 误调旧 wrapper，测试 helper 构造 `MirInstanceMaterializer::new(...)` 未传新增 gate。
- 修复了新增 opt-level 入口引入的 warning 风险：旧的 crate-internal 默认 wrapper 仅测试使用时加 `#[cfg(test)]`，避免非测试构建 dead-code warning 在 `clippy -D warnings` 下失败。
- 修复了 production LLVM 回归测试的 opt-level 语义：验证 inlining / devirtualization 生效的测试显式使用 `OptLevel::O2`，新增的 `production_codegen_respects_mir_inlining_opt_level_gate` 负责锁定 `O0` 不运行 MIR inlining。
- 当前已验证：`cargo fmt --all`、`cargo test -p scoopc production_codegen_respects_mir_inlining_opt_level_gate -- --nocapture`、`cargo test -p scoopc mir::inline -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture` 通过。
- 完整 fixture 初跑发现 `tests/fixtures/build/member_call_devirt_final_receiver_direct_call.scoop` 显式要求 `--opt-level 0` 下 exact receiver directization；据此修正半成品中过宽的 devirt gate，并同步更新 `OPTIMIZATION.md` 对 O0 的说明：exact/singleton dispatch directization 属于当前必要 call classification / instance discovery，不属于本轮要关闭的 summary-driven inlining。
- clippy 发现新增 request-source + opt-level API 参数过多；已改为 `MirInstanceCollectionOptions` 收口该组参数，避免用 `#[allow]` 压 warning。
- `TODO.md` 已将 `T5000hR` 标为完成，`PLAN.md` 已记录 review 结论、修复项和验证结果。
- 最终验证通过：`cargo fmt --all`、`cargo test -p scoopc production_codegen_respects_mir_inlining_opt_level_gate -- --nocapture`、`cargo test -p scoopc mir::inline -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）。

## 2026-04-27 接手后实际验证
- 已重新审计当前未提交 diff，确认半成品的实际问题是：summary-driven MIR inlining 需要由 `OptLevel` 控制，且不能把 `T5000g` 已建立的 exact / singleton dispatch directization 一并从 `O0` 关闭。
- 已确认 `cargo check -p scoopc` 通过，说明当前半成品不存在剩余编译错误。
- 已重新运行 targeted tests：
  - `cargo test -p scoopc production_codegen_respects_mir_inlining_opt_level_gate -- --nocapture`
  - `cargo test -p scoopc mir::inline -- --nocapture`
  - `cargo test -p scoopc llvm::tests -- --nocapture`
- 已重新运行收尾验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`，结果为 `fixtures: ok (1201)`
- 下一步：检查最终 diff/status，提交 `[T5000hR] Review summary-driven MIR inlining boundaries` 后停止，不进入 `T5000i`。

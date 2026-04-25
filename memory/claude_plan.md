# 本轮执行计划（T5000d3）

## 目标

完成 `TODO.md` 中当前排在最前的未完成任务 `T5000d3`，并在完成后更新 `TODO.md`、`PLAN.md`、提交 git commit，然后停止。

## 当前判断依据

- 已有交接信息表明：最新提交未显式声明需要先修的遗留问题，但在实现 `T5000d3` 时暴露出一个真实前置缺口：
  `when` 的整数字面量 / 字符串字面量模式在多个阶段仍依赖 span 回查源码，这与本任务要求的“Perform / provenance / canonicalization 入口收口”和更规范的 MIR 表达相冲突。
- 这个缺口不是可接受的绕过点，因此要作为本任务实现的一部分继续修复，而不是回避。
- 当前代码处于“结构已改一半、尚未完成收口”的状态，最可能的阻塞点是：
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/llvm/codegen/control_flow.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/hir/{mod.rs,lower/*}`
  - `crates/scoopc/src/monomorph/lower.rs`

## 执行原则

1. 先保证代码重新可编译，再判断当前 MIR 设计是否需要最小化收口。
2. 不接受把问题重新压回字符串 `Todo(...)`、span 反查、fixture 特判等“绕过去”的做法。
3. 只做本轮首个任务；若发现真正新的前置问题无法在本轮内修完，就把它插入 `TODO.md` 到当前任务之前，并更新 `PLAN.md` 后提交并停止。

## 详细步骤

1. 编译探测
   - 运行 `cargo check -p scoopc`。
   - 按报错顺序修复，优先处理类型定义、模式匹配分支、字段改名、缺失 derive、side table 透传断点。

2. 收口 HIR / LLVM 字面量 pattern 通路
   - 确认 `WhenPat::IntLit` / `WhenPat::StringLit` 的新载荷在所有使用点一致。
   - 修复 LLVM `when` codegen 中仍依赖源码 span 的路径，改为使用 HIR 携带的 `raw` / `value`。

3. 收口 MIR 结构与 lowering
   - 修复 `mir/mod.rs` 与 `mir/lower.rs` 的结构不一致问题。
   - 确认以下节点完整可用：
     - `Rvalue::TopLevelRef`
     - `Rvalue::UnresolvedName`
     - `Rvalue::Unary`
     - `Rvalue::Binary`
     - `Rvalue::TypeCheck`
     - `Rvalue::Cast`
     - `Rvalue::MemberAccess`
     - `Rvalue::PatternMatch`
     - `Rvalue::PatternExtract`
     - `Rvalue::PerformResult`
     - `TerminatorKind::Perform { op_fqn, metadata, args }`
   - 确认 provenance 从“仅 callable”扩展到通用 `value_origins` 后，call lowering / perform lowering 仍保持正确。

4. 收口 monomorph side table 透传
   - 确认 monomorph lowering 也能获得 HIR side tables，而不只是 `dump-mir` 路径。

5. 更新或新增 MIR fixtures / 测试
   - 视最终 MIR 输出更新相关 fixture，至少覆盖：
     - `when` 的 pattern match / binder extract / guard 分离
     - `perform` 的参数正规化与 metadata
   - 必要时补最小测试来防止 side table 只在单一路径生效。

6. 验证
   - 运行 `cargo fmt --all`
   - 运行与本任务直接相关的测试
   - 运行 `cargo clippy --all-targets -- -D warnings`
   - 若发现任何既有缺陷，优先修复；无法当场修复则把它前插进 `TODO.md`。

7. 文档与提交
   - 更新 `memory/claude_plan.md` 记录关键进展和计划变化。
   - 更新 `TODO.md`，把本轮完成的任务标记为已完成。
   - 更新 `PLAN.md`，反映当前状态。
   - 提交一次 git commit，然后停止。

## 当前已知风险

- 由于 MIR 结构改动较大，可能会连带影响 pretty printer、fixture 生成、LLVM codegen、monomorph 等多处调用点。
- `when` lowering 的 binder 注入/恢复如果控制流边界不严谨，容易引入 symbol shadowing 或错误类型。
- `Perform` 参数 canonicalization 如果缺少类型信息，可能需要进一步回填 side table 或补充 metadata。

## 进度记录

- 已重新整理本轮计划，准备开始第一次编译探测。
- 已完成第一次编译探测：
  - `cargo check -p scoopc` 初始只暴露出 `mir/lower.rs` 中 `TypeCheck` / `Cast` 仍使用旧字段名 `ty`；
  - 已改为对齐 HIR 的 `target_ty` 字段，并顺手去掉 `crates/scoopc/src/hir/mod.rs` 中未使用的 `LoweredFunWithSideTables` re-export；
  - 重新运行 `cargo check -p scoopc` 后已通过。
- 下一步：运行 MIR 相关 fixture / 单测，确认本轮新增 MIR 节点与 lowering 形状是否与 golden 一致，若不一致则更新实现或 fixture。
- 已完成 MIR 路径第一轮收口：
  - `cargo test -p scoopc monomorph::lower -- --nocapture` 通过，说明 HIR side table 透传没有打断 monomorph 路径；
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir` 初始暴露 3 个 golden 失配：`direct_and_fun_value_call`、`handle_perform`、`if_when`；
  - 其中 `if_when` 暴露出真实 lowering 缺口：`when` 对“无 guard 的兜底 arm”会预分配不会执行的 `next_test_bb`，并留下 `unterminated` / 多余 CFG 残块；已在 `crates/scoopc/src/mir/lower.rs` 中改为按 arm 形状懒分配 fallthrough/body block，并让无 guard arm 直接在 match block 内继续 lowering；
  - 已更新受影响的 MIR golden，并新增 `tests/fixtures/mir/when_bind_guard.{scoop,mir}`，显式覆盖 `PatternMatch + PatternExtract + guard Binary` 路径；
  - 重新跑 `cargo run -p scoop -- test --fixtures tests/fixtures/mir` 后已通过（9 个 fixture）。
- 全量验证进行中：
  - `cargo fmt --all` 已通过；
  - `cargo test --all` 首轮在 `hir::lower::tests::hir_fixture_control_flow_golden` 失败，原因是 HIR fixture 仍停留在旧的 `WhenPat::IntLit { span }` 输出，没有跟上本轮新增的 `raw` 字段；
  - 已更新 `tests/fixtures/hir/control_flow.hir`，准备重新跑全量测试与 `clippy`。
- 已完成最终验证：
  - `cargo test -p scoopc hir::lower::tests::hir_fixture_control_flow_golden -- --nocapture` 已通过；
  - `cargo test --all` 已重新通过；
  - `cargo clippy --all-targets -- -D warnings` 已通过。
- 收尾步骤：
  - 已更新 `TODO.md` / `PLAN.md`，将 `T5000d3` 标记为完成，并把下一条待执行任务推进到 `T5000d3R`；
  - 下一步只剩 git 提交，然后停止本轮执行。

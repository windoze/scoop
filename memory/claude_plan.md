# 当前执行计划

1. 读取 `TODO.md`，确认详细任务文件引用与全局顺序。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位第一个未明确记录为完成的详细任务。
3. 检查最近提交是否存在与该任务直接相关且未完成的问题；若存在且构成前置条件，则先在对应 `TODO-Px.md`/`TODO.md` 中补充并调整依赖。
4. 阅读与当前任务直接相关的代码、测试、规范与任务约束，确认最小正确修改范围。
5. 实现当前任务，避免规避性做法；若遇到阻塞，则记录最小必要前置任务并停止在该任务边界。
6. 运行与当前任务相关的验证；必要时补充或修复测试，直到结果稳定。
7. 更新 `memory/claude_plan.md` 记录关键进展或计划变化。
8. 在对应 `TODO-Px.md` 中记录任务完成情况；如任务索引、标题、顺序或依赖变更，同步更新 `TODO.md`；仅在阶段计划实际变化时更新 `PLAN.md`。
9. 检查工作区状态，确认仅提交与本次任务相关的改动，并按仓库约定创建一次 git 提交。
10. 提交后停止，不继续处理下一个任务。

## 记录规则

- 不在此文件记录隐藏推理，仅记录可执行计划、关键发现、阻塞与已完成步骤。
- 如实施过程中发现需要新增前置任务，会在此文件追加原因、受影响任务与后续动作。

## 当前进展

- 已读取 `TODO.md` 与 `TODO-P0.md`，确认首个未完成详细任务为 `P0-T04R`（`Review baseline parity 与 P0 退出条件`）。
- 已检查最新提交：`a89fe87 [P0-T04] Establish baseline parity matrix`，提交信息未声明需要在当前任务前新增直接相关的前置修复。
- 下一步：按 `P0-T04R` 要求复核 parity harness、边界清单、CLI/session/dispatcher 路径，并重新执行规定的定向验证。

## 已完成步骤

- 已复核 `crates/scoop/src/commands/parity.rs`、`EFFECT_REFACTOR_BOUNDARY_INVENTORY.md`、`crates/scoop/src/cli.rs`、`crates/scoop/src/commands/mod.rs`、`crates/scoop/src/commands/build.rs`、`crates/scoopc/src/session/mod.rs`、`crates/scoopc/src/driver_cli.rs`、`crates/scoopc/src/bin/scoopc.rs`、`crates/scoopc/src/effect_refactor_pipeline/mod.rs`。
- 结论：P0 baseline parity 已覆盖 AST / HIR / MIR / IR 代表路径，并在启用 LLVM feature 时补充 `build --emit-llvm` parity；默认 selector 仍为 `legacy`，显式 `refactor` 入口继续经 `SessionOptions` 与 dispatcher 壳层进入，不存在需要为当前任务新增的前置修复。
- 已完成定向验证：
  - `cargo test -p scoop --no-default-features parity`
  - `cargo test -p scoop --no-default-features cli`
  - `cargo test -p scoopc --no-default-features session`
  - `cargo test -p scoopc --no-default-features driver_cli`
  - `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
  - `cargo test -p scoop build_emit_llvm_cli_parity_matches_legacy_and_refactor`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 下一步：提交 `TODO-P0.md` 与 `memory/claude_plan.md` 的更新，并停止在 `P0-T04R` 边界。

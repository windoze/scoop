# Claude 执行计划

说明：本文件记录本次调用的可审计执行计划、关键进展和验证结果；不记录隐藏推理过程。

## 当前任务

- 按 `TODO.md` 的权威顺序，当前第一个未完成任务是 `U6-T02：退场标注 + 计划自检`。
- 最新提交 `025f4cc7 [U6-T01] Add UMB baseline audit tests` 是当前任务的直接前置提交，未提示与 U6-T02 相关的未完成阻塞项。
- 本次只完成 U6-T02，完成后提交并停止，不进入后续 P7/P8 production 修复。

## 执行计划

1. 读取 `TODO.md`、`PLAN.md` 和 `UnsupportedMainBody_FIX.md` 中 U6-T02 与退场判据相关段落。
2. 确认 P1-P6 交付物存在：inventory、schema、bucket 文档、strategy 文档、spec matrix、`umb_fix` fixture/index、test-only audit module。
3. 先运行 U6-T02 的核心验证：`cargo test -p scoopc audit:: -- --nocapture` 与 `cargo run -p scoop -- test tests/fixtures/umb_fix/`。
4. 在核心验证通过后，更新 `UnsupportedMainBody_FIX.md` §12 为 `[DONE]` 并追加 `PLAN-MD` 追踪说明。
5. 创建 `UnsupportedMainBody_DONE.md`，仅记录 P7/P8 handoff 头部、退场标准引用和当前 inventory count。
6. 运行完整验证：`cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`，并复跑 U6-T02 核心验证。
7. 更新 `TODO.md` 顶部状态、将 U6-T02 标为 `[DONE]`，填写完成记录。
8. 提交本次任务相关变更并停止。

## 进展记录

- 已识别当前任务为 `U6-T02：退场标注 + 计划自检`。
- 已读取 `PLAN.md` §6/§7 和 `UnsupportedMainBody_FIX.md` §9/§12，确认 `PLAN.md` 不需要更新。
- 已确认关键交付物存在：`audit/UMB_inventory.csv`、`audit/UMB_inventory_schema.md`、`audit/spec_coverage_matrix.md`、36 份 bucket 文档、36 份 strategy 文档、36 个 fixture README、`tests/fixtures/umb_fix/_index.csv`、141 个 `umb_fix` fixture，以及 `crates/scoopc/src/audit/**`。
- 已确认 `crates/scoopc/src/lib.rs` 通过 `#[cfg(test)] mod audit;` 接入 audit module。
- 核心验证已通过：`cargo test -p scoopc audit:: -- --nocapture` 通过（17 passed）；`cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（141 fixtures，runner 明确处理 `IGNORE-UNTIL-FIX` skip/pass）。
- 已更新 `UnsupportedMainBody_FIX.md` §12 为 `[DONE]`，并追加 `// PLAN-MD: see PLAN.md (this repo root) for execution tracking`。
- 已新增 `UnsupportedMainBody_DONE.md`，记录当前 `audit/UMB_inventory.csv` baseline 为 1,284 entries，并引用 P7/P8 退场标准。
- 完整验证已通过：`cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`，并已复跑核心验证。
- 已更新 `TODO.md` 顶部状态，将 U6-T02 标为 `[DONE]`，并填写完成记录。
- 剩余步骤：提交本次任务相关变更，然后停止。

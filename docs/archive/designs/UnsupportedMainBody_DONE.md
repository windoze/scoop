# UnsupportedMainBody P7/P8 退场追踪

本文件在 doc-and-test only 计划完成 P6 后，接手追踪 P7 production 修复与 P8 退场审计。

## 当前基线

- 当前 inventory count：`audit/UMB_inventory.csv` 为 header-only，active entry 0 条。
- 当前计划状态：P7 production 修复、P8 enum variant 删除、P8 归档审计均已完成。

## 退场标准引用

- P7 production 修复判据：`UnsupportedMainBody_FIX.md` §9、`PLAN.md` §7。
- P8 一次性退场动作：`UnsupportedMainBody_FIX.md` §9、`PLAN.md` §7。

## P8 最终记录

- 完成日期：2026-05-19。
- 最终状态：active=0，retired=1,284，initial=1,284；`FrontendReject`、`InternalBugSentinel`、`RealImpl` active count 均为 0。
- Production 状态：`LlvmEmitError::UnsupportedMainBody` enum variant 与 diagnostic mapping 已在 P8-T01 删除；`crates/scoopc/src/llvm/**` 不再包含 `UnsupportedMainBody` production codegen 路径。
- Audit 归档：`docs/archive/audits/unsupported-main-body/` 保存 `UMB_inventory_initial.csv`、`UMB_retired.csv`、`UMB_inventory_final_empty.csv` 与 `UMB_inventory_schema.md`。
- 计划归档：`docs/archive/plans/PLAN-unsupported-main-body-p7-p8.md` 与 `docs/archive/plans/TODO-unsupported-main-body-p7-p8.md`。
- Tooling 退场：P8-T02 删除 `umb-audit` binary 与 UMB inventory-generation tests；长期保留 `crates/scoopc/src/audit/spec_coverage.rs` 作为 `tests/fixtures/umb_fix/**` fixture coverage audit。
- Fixture 状态：`tests/fixtures/umb_fix/**` 全部 active；无 ignored/xfail 状态；fixture `COVERS` 均为 `NONE`，历史 ID 覆盖由归档 ledger 承接。
- 最终验证：
  - `cargo run -p scoopc --bin umb-audit -- stats` 在删除工具前通过，显示 active=0、retired=1284、initial=1284。
  - `cargo test -p scoopc audit:: -- --nocapture` 通过。
  - `cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（152 fixtures）。
  - `cargo test --all --all-targets` 通过。
  - `cargo run -p scoop -- test` 通过（fixtures ok，1558 checks）。
  - `cargo clippy --all-targets -- -D warnings` 通过。

# 执行计划

## 说明

按要求，我会先维护这份可审阅的执行计划与进度记录，再进行仓库检查、实现、测试与提交。
这里记录的是面向执行的计划、决策与进度，不包含不可审阅的内部推理细节。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到需要先修复的既有问题。
2. 查看 `TODO.md`，定位第一个未完成任务。
3. 查看 `PLAN.md`，确认当前计划与 `TODO.md` 是否一致。
4. 检查工作树状态，避免覆盖已有未提交改动。
5. 如果第一个未完成任务过大，则把它拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后优先执行拆分后的第一个子任务。
6. 实现当前应执行的第一个任务。
7. 运行相关测试、格式化、lint，并修复发现的问题。
8. 将完成情况同步回 `TODO.md`、`PLAN.md` 与本文件。
9. 使用清晰的提交信息提交当前轮工作。
10. 停止，不继续处理下一个任务。

## 进度记录

- 2026-04-25：已创建本文件并写入初始执行计划，尚未开始仓库检查。
- 2026-04-25：已检查最新提交、`TODO.md`、`PLAN.md` 与工作树状态。
  - 最新提交为 `[T5000b3d] Split enum and object lowering modules`，提交说明未显式挂出需要优先修复的既有问题。
  - 当前工作树仅有本文件的未提交修改。
  - `TODO.md` 中第一个明确未完成条目为 `T5000b3dR Review：确认 codegen/mod.rs 的主题拆分已收口到共享上下文与通用 helper`。
- 2026-04-25：确定本轮执行目标为 `T5000b3dR`，当前不需要再把任务细分。
  - 具体执行步骤：
    1. 审阅 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 enum/object lowering 主体是否已全部迁出。
    2. 审阅 `enum_lowering.rs`、`object_init.rs` 及相关调用面，确认接口方向是否为单向窄接口。
    3. 检查根模块剩余的大函数簇，判断它们是否属于共享上下文、通用 helper 或跨主题桥接；若发现既有边界泄漏，先修复，再继续 review。
    4. 运行相关测试与 lint。
    5. 将 review 结果同步到 `TODO.md`、`PLAN.md` 与本文件，然后提交并停止。
- 2026-04-25：`T5000b3dR` 复核过程中发现一个既有边界泄漏，需先修复。
  - 问题：`crates/scoopc/src/llvm/codegen/mod.rs` 仍残留 `codegen_sysroot_funptr_invoke`、`codegen_sysroot_funptr_to_uintptr`、`codegen_sysroot_uintptr_to_funptr` 三个 `scoop.unsafe.*` intrinsic lowering；
  - 现状：`crates/scoopc/src/llvm/codegen/call/dispatch.rs` 仍通过这三条接口做分派，说明 sysroot intrinsic lowering 尚未完全从根模块迁出；
  - 处理：先把这组三个函数迁入 `crates/scoopc/src/llvm/codegen/intrinsics/sysroot.rs`，再重新复核根模块剩余职责。
- 2026-04-25：已完成边界修复与验证。
  - 已将 `codegen_sysroot_funptr_invoke`、`codegen_sysroot_funptr_to_uintptr`、`codegen_sysroot_uintptr_to_funptr` 从 `crates/scoopc/src/llvm/codegen/mod.rs` 迁入 `crates/scoopc/src/llvm/codegen/intrinsics/sysroot.rs`；
  - 已重新复核 `codegen/mod.rs`：
    - enum / object lowering 主体已稳定留在 `enum_lowering.rs` 与 `object_init.rs`；
    - 根模块剩余内容以共享上下文、通用 helper、顶层值初始化/访问、GC/root/sret/return 桥接、单态化/具体类型恢复 helper、以及通用表达式/运算/转换 lowering 为主；
    - 未再发现需要先于 `T5000b3R` 插入的新前置缺陷任务。
  - 已完成验证：
    1. `cargo fmt --all`
    2. `cargo test -p scoopc llvm::`
    3. `cargo test --all`
    4. `cargo clippy --all-targets -- -D warnings`
- 2026-04-25：当前轮目标 `T5000b3dR` 已完成，下一步只剩提交并停止。

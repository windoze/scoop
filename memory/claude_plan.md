# 当前执行计划

## 任务约束摘要

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 如遇到阻塞当前任务的实现缺口或规格不匹配，先在 `TODO.md` 插入最小必要前置任务并提交，然后停止。
- 不做开放式历史问题扫查，不用绕过、降级或 fixture-only hack。

## 执行步骤

1. 读取 `TODO.md`，定位第一个未完成任务及其验证要求。
2. 查看最新提交信息，仅判断是否有与当前任务直接相关的未完成问题。
3. 按当前任务要求定位相关代码、测试和 fixture。
4. 以最小正确改动实现任务，期间如计划变化或关键步骤完成，更新本文件。
5. 运行任务指定验证；若未指定，运行与改动范围相关的测试，必要时补充更广验证。
6. 更新 `TODO.md`：给任务标题加 `[DONE]`，填写完成记录与验证结果。
7. 仅在阶段级计划真实变化时更新 `PLAN.md`。
8. 检查 git 状态与差异，提交本次任务全部相关改动。
9. 停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P7-T02`。
- 最新提交 `[P7-T01] Convert sysroot runtime wrappers to scoop ABI` 是当前任务的直接前置提交，未包含额外未完成事项。
- P7-T02 的工作范围：删除 `sysroot/scalar_string_bridge.scoop`、删除 `scalar_*_to_string_bridge` audited intrinsic dispatch、删除/改写旧 bridge owner 测试与 fixture，并验证 direct runtime `toString` 调用路径仍成立。
- 前置 grep 命中集中在将被本任务删除的 bridge 文件、旧 owner 测试和旧 bridge fixture；未发现业务 sysroot body 继续调用 `scoopAbi*ToString`。
- 已删除 `sysroot/scalar_string_bridge.scoop` 与旧 `sysroot_scalar_string_bridge_basic` fixture。
- 已从 `intrinsics.rs` 移除 scalar toString bridge entries；同时移除已无 sysroot 使用方的 `string_concat_bridge` audit entry，以关闭同一 audited bridge 层。
- 已从 sysroot/effect-facts support-source 列表移除 `scalar_string_bridge.scoop`，并删除旧 IR owner 测试；保留 `scalar_to_string_calls_scoop_abi_runtime_directly` 验证 direct runtime call。
- 已运行 `cargo fmt`。
- 已通过结构验证：bridge 相关 forbidden pattern 在 `crates runtime sysroot` 下无命中，且 `sysroot/scalar_string_bridge.scoop` 不存在。
- 已通过目标测试：`cargo test -p scoopc scalar_to_string_calls_scoop_abi_runtime_directly -- --nocapture`；sysroot load / core support / declaration-only audit 三个相关测试也通过。
- 有一次中间验证命令因 `cargo test` 传入两个 filter 参数而失败，随后已用独立 filter 重新执行并通过。
- `cargo test --all --all-targets` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- `cargo run -p scoop -- test` 完整执行，结果为 7 个既有 baseline 失败、1334 个通过、1371 checks 通过；旧 bridge fixture 删除后目标/检查数各减少 1。
- 已将 `P7-T02` 在 `TODO.md` / `TODO-3.md` 标记为 `[DONE]` 并写入完成记录。

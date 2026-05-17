# 当前执行计划

## 范围

- 以 `TODO.md` 为权威任务列表与完成状态来源。
- 只处理第一个未完成任务；只有标题显式带 `[DONE]` 的任务才视为完成。
- 本轮任务是 `P7-T03`：对齐 sysroot `@Extern(name = ...)` 与 runtime 导出符号，重点处理 `scoop_gc_collect_safepoint` 到 `scoop_gc_collect` 的收口。
- 不采用兼容别名、fixture-only hack 或绕过式实现。

## 执行步骤

1. 读取 `TODO.md`，定位第一个未完成任务。
2. 查看最新提交，仅判断是否存在与当前任务直接相关的未完成事项。
3. 审计 sysroot/runtime/compiler 中 print、println、panic、GC collect 的符号对应关系。
4. 用最小正确改动统一 runtime/codegen 符号。
5. 运行目标测试、构建、全量 fixture、Rust 全量测试、clippy 和 `nm` 符号检查。
6. 更新 `TODO.md` / `TODO-3.md`，把 `P7-T03` 标记为 `[DONE]` 并记录完成情况。
7. 提交本任务全部相关改动后停止。

## 进度记录

- 已读取 `TODO.md`，确认第一个未完成任务是 `P7-T03`。
- 最新提交是 `[P7-T02] Remove scalar string bridge`，是当前任务的直接前置提交，未发现额外未完成事项。
- 已确认 `__scoop_print` / `scoop_print`、`__scoop_println` / `scoop_println`、`panic` / `scoop_panic` 已一致；实际待处理项是 compiler/runtime lowering 仍使用 `scoop_gc_collect_safepoint`。
- 已删除 runtime C 侧 `scoop_gc_collect_safepoint` wrapper，并从 `scoop_runtime_api.h` allowlist 移除该旧导出。
- 已把 compiler runtime symbol、runtime ABI declaration、GC collect lowering 与相关 IR 回归断言改为 `scoop_gc_collect`。
- 目标验证已通过：scoopc late-lower/ABI 相关测试、`scoop_runtime` 导出 allowlist、最新 runtime archive `nm` 检查。
- `cargo build` 通过。
- `cargo run -p scoop -- test` 完整执行，保持既有 baseline：7 个失败、1334 个通过、1371 checks 通过；未出现新的 P7-T03 owner-path 失败。
- `cargo test --all --all-targets` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- 已更新 `TODO.md` / `TODO-3.md`，将 `P7-T03` 标记为 `[DONE]` 并写入完成记录。

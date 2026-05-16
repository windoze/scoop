# 当前执行计划

1. 读取 `TODO.md`，按文件顺序定位第一个标题未带 `[DONE]` 的任务。
2. 查看最近提交信息，只判断是否有与该任务直接相关的未完成事项或前置问题。
3. 阅读当前任务要求、依赖和验证要求；必要时查看相关源码、测试和规格文档。
4. 若任务可直接完成，则实现最小且完整的修复或功能；若发现阻塞该任务的真实前置问题，则更新 `TODO.md` 记录前置任务并停止。
5. 运行与任务相关的测试和必要的格式/检查命令，修复发现的问题。
6. 将已完成任务标题加上 `[DONE]`，更新完成记录；只有阶段级计划改变时才更新 `PLAN.md`。
7. 提交本次任务相关全部变更，提交后停止，不进入下一个任务。

## 进度记录

- 已创建本执行计划；下一步读取 `TODO.md` 定位首个未完成任务。
- 已读取 `TODO.md`；首个未完成任务为 `P5-T01：做全量稳定化、跨平台矩阵与文档收尾`。下一步核对最新提交是否包含与该任务直接相关的未完成事项。
- 最新提交为 `[P4-T02] Migrate remaining string helpers`，未显式记录与 `P5-T01` 直接相关的未完成事项。下一步阅读 `PLAN.md` / `MANAGED_ABI.md` 相关章节，确认文档收尾范围并开始验证。
- 已确认本机为 `Darwin/arm64`，Docker 可用但服务端为 `linux/arm64`；`llvm-config` 不在本机 PATH。下一步先执行 macOS/aarch64 全量验证，并评估是否能用 Docker 补跑 Linux/amd64 矩阵。
- 开始执行 macOS/aarch64 全量 Rust 测试：`cargo test --all --all-targets`。
- `cargo test --all --all-targets` 失败：主要是 `scoop` crate 单测/fixture harness 中临时工程没有加载默认 sysroot，导致 `scoop.core.*` unresolved；另有旧测试输入仍声明无 body 普通 helper，触发 `fun_must_have_body`。下一步修复测试 harness / test resources，使 P5 全量验证可执行。
- 已修复 dump/materialize/package/typecheck-cone 路径对 compilable sysroot support sources 的索引与类型环境注入，并把遗漏的 infer fixture 普通无 body helper 改为真实 body；`cargo test -p scoop --bin scoop` 现为 116 passed / 0 failed。
- 重跑 `cargo test --all --all-targets` 后，`scoop` 相关测试已通过；当前阻塞为 `scoop_runtime` export allowlist 漏登记 `scoop_array_alloc` / `scoop_array_builder_grow` / 三个测试 helper。下一步按 runtime API 头的分类补登记，保持 allowlist 测试不放宽。
- runtime export allowlist 已补齐并通过定向验证；随后 `cargo test --all --all-targets` 在 `gc_stop_the_world` 集成测试处超过 20 分钟超时，违反单测 <1 分钟要求。下一步定位 STW 测试/运行时挂起并修复。
- 定向运行 `cargo test -p scoop_runtime --test gc_stop_the_world -- --nocapture` 0 秒通过，说明前一次是全量命令总时限不足而非该单测卡死。下一步用更长超时重跑完整 `cargo test --all --all-targets`。
- 已完成 `scoopc` direct-test/sysroot support-source 修复：为 compilable sysroot 生成 signature-only index AST，补齐 dump/materialize/effect-facts support source 去重，并刷新相关 owner tests / failure-policy sentinel；`cargo test -p scoopc --lib` 现为 864 passed / 0 failed。
- `cargo test --all --all-targets` 已通过完整 Rust workspace 矩阵。下一步运行 `cargo run -p scoop -- test` 完整 fixture suite。
- `cargo run -p scoop -- test` 已通过完整 fixture suite（1367 checks）。下一步补跑 `llvm_tests` / native direct-indirect parity / managed ABI 定向验证、`clippy`，并评估 Linux/amd64 Docker 矩阵。
- 本机 `macos/aarch64` 验证完成：`cargo test --all --all-targets`、完整 fixture suite、native/managed ABI 定向套件、GC stress string helper 与 `cargo clippy --all-targets -- -D warnings` 均通过。Linux/amd64 Docker 可启动到 `x86_64`，但完整 Rust/LLVM 矩阵在 emulation 下未能在 30 分钟内完成，已停止超时容器；后续文档需明确该矩阵限制。

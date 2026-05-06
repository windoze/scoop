# 执行计划

## 当前约束
- 以 `TODO.md` 为唯一任务顺序来源，先识别第一个标题未带 `[DONE]` 的任务。
- 本次只完成第一个未完成任务，完成后停止，不推进下一项。
- 如发现当前任务被具体前置缺陷阻塞，最小化地更新 `TODO.md` 记录前置任务并提交后停止。
- 不采用规避、弱化 fixture、临时 shim 或偏离规格的实现。
- 完成任务后需要更新 `TODO.md` 的任务标题为 `[DONE]`，补充完成记录，运行相关验证并提交。

## 初始执行步骤
1. 阅读 `TODO.md`，识别第一个未完成任务及其验证要求、依赖和完成记录。
2. 查看最新提交信息，确认是否明确提到与该任务直接相关的未完成问题。
3. 根据任务内容读取最小必要的相关文件，确认实现位置和测试位置。
4. 实现当前任务；如遇规格阻塞，更新 `TODO.md` 插入最小前置任务并停止。
5. 运行任务要求的验证命令和必要的回归测试，修复当前任务引入的问题。
6. 更新 `TODO.md` 将该任务标题标记 `[DONE]` 并写入完成记录。
7. 检查工作区差异，提交本次任务所有相关改动。

## 进度记录
- 已创建初始计划，下一步读取 `TODO.md` 并识别第一项未完成任务。
- 已读取 `TODO.md`：当前任务列表 `HIR-T00` 至 `HIR-T14` 的标题均已带 `[DONE]`，未发现常规未完成任务。
- 最新提交为 `[HIR-T14] Freeze HIR completeness handoff`，与最后完成的 HIR 阶段收口任务一致，未提示新的直接未完成前置问题。
- 进入所有任务完成后的最终复核：检查文档/工作区，运行最终验证，必要时记录最终完成状态并提交、打 `v0.1.0` 标签。
- 当前 `PATH` 中没有 `llvm-config`，但发现 Homebrew LLVM 21 的 `llvm-config` 位于 `/opt/homebrew/opt/llvm@21/bin/llvm-config`；后续默认 feature 验证将临时把该目录加入 `PATH`。
- 默认 feature 的 `cargo test --all` 失败：新增 `hir::ExprKind::ClassLiteral` 未被 LLVM 相关 match 覆盖，导致 `scoopc` 默认构建失败。下一步做最小修复，使 LLVM 遍历/codegen 按 class literal 的 v0 runtime 字符串语义或不触发 effect 标记处理该变体。
- 已修复 `ClassLiteral` 的默认 LLVM feature 覆盖：主 codegen 降为 v0 类型名字符串；整数预检、可达性扫描与 effect 测试辅助遍历将其视为无子表达式叶子。
- 重新运行默认 feature 的 `cargo test --all` 后，编译问题已消失，但 `default_refactor_runs_receiver_effect_op_cli` 因 LLVM verifier 报 `Invalid InsertValueInst operands` 失败。下一步定位 refactor perform payload aggregate 构造，修复字段类型/顺序不一致。
- 已定位 receiver effect op 失败根因：HIR 已按形参顺序规范化 effect-op args，但 HIR contract/MIR metadata/旧 HIR codegen 又用 `arg_mapping` 二次重排，导致 named receiver 调用的 payload 变成 `Int, String`。已改为 payload 类型与 lowering 均消费规范化参数顺序，`arg_mapping` 仅保留源码来源索引。
- 失败路径仍会通过 legacy HIR side table 向 MIR lowering 提供 perform metadata；已将该 fallback 分支也改为消费已规范化 HIR 参数顺序，避免再次按 `arg_mapping` 重排。
- 定向回归 `cargo test -p scoop --test p7_default_pipeline default_refactor_runs_receiver_effect_op_cli` 已通过。
- 全量默认测试继续暴露 3 个陈旧测试期望：wrapper projection 中 type id 因 payload tuple 修正变更；ctor call 被 default-arg block 包裹；PerformArg raw named-arg 名称在 HIR 规范化后不再保留。已更新测试以断言当前稳定语义。
- 默认 feature `cargo test --all` 已通过（使用 `/opt/homebrew/opt/llvm@21/bin` 中的 LLVM 工具）。下一步运行 clippy、spec fixture 检查与 fixture runner。
- clippy 首次运行发现 2 个默认 feature 告警：`unwrap_or_else` 可替换为 `unwrap_or`，以及一个既有 MIR dynamic-call helper 参数数超阈值。已做最小修复/标注。
- 严格 clippy `cargo clippy --all-targets -- -D warnings` 已通过。
- `cargo run -p scoop -- test` 首次失败于 `effect_refactor_continuation_interface_full_methods.scoop`：当前 IR 已生成 resume `case0`/`case1` 的 `define`，fixture 仍期望旧 `declare` shell。已更新 fixture 期望为 `define`。
- 完整 fixture runner 随后失败于同类 `effect_refactor_dynamic_invoke_unit_payload.scoop` resume method 期望；dynamic/direct invoke 仍为 `declare`，resume `case0` 已是 `define`。已同步该 fixture。
- 本次复核确认 `TODO.md` 中 `HIR-T00` 至 `HIR-T14` 均已 `[DONE]`，最新提交已完成 HIR handoff 文档冻结。
- 按用户要求未继续运行 full fixtures；仅验证 HIR 收口相关命令。
- 已通过：`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features refactor_hir_preflight`、`cargo test -p scoop --no-default-features dump_hir`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 已执行 `rg "Todo\\(" crates/scoopc/src/hir crates/scoopc/src/effect_refactor_pipeline`，命中仍属于 legacy/debug HIR lowerer、测试注入、verifier/denylist 或 preflight 扫描路径，未发现新的 refactor production HIR placeholder 来源。
- 下一步按 `PROMPT.md` 的 all-tasks-complete 收尾要求提交最终调整并创建 `v0.1.0` 标签；`crates/scoop/target/fixtures/refactor_abi_visibility.ll` 是生成产物，保留未提交。

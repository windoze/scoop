# 本轮执行计划（T4007b）

## 任务目标

- 当前首个未完成任务是 `T4007b`：为 parameterized interface 与 `eff` 参数 target 补齐运行期匹配。
- 本轮只完成这一项，然后更新 `TODO.md` / `PLAN.md` / `memory/claude_plan.md`，提交 commit 后停止。

## 已有上下文与问题判断

- 已检查过上一轮最新提交，未发现需要先处理的额外遗留 issue；当前工作重点仍是 `T4007b`。
- 现有改动已经把 class itable 从仅保存 base `interface_id`，扩展为同时保存：
  - base dispatch 用的 `interface_id`
  - 具体 interface 实例 `interface_type_name` / `interface_type_id`
  - 运行期匹配集合 `runtime_match_type_names` / `runtime_match_type_ids`
- LLVM `is/as/as?` 针对 interface target 的匹配逻辑也已改为按 `runtime_match_type_ids` 扫描，而不是只比较 base `interface_id`。
- 目前已知新 run-pass fixture `type_check_cast_parameterized_interface_runtime_match_basic.scoop` 在单独运行时通过。
- 仍需重点验证：
  - `crates/scoopc/src/rtti/type_desc.rs` 是否编译通过且 RTTI 输出正确
  - 更广范围测试、`clippy`、全量构建是否无回归和无 warning
  - 文档状态文件是否与实现一致

## 风险点

1. `rtti/type_desc.rs` 目前的 precise metadata 合并逻辑可能仍按 base interface 名字匹配，若同一 class 对同一 base interface 存在多个具体实例，可能会丢信息或配错。
2. 运行期 itable 布局已扩展，GC / codegen / RTTI 三处都需要保持结构一致，否则可能在更大测试范围内暴露问题。
3. `TypeLowering` / `assignable` 暴露范围扩大后，可能引入 `clippy` 或可见性相关 warning。

## 执行步骤

1. 先查看当前工作区状态与相关文件，确认 handoff 内容和实际代码一致。
2. 运行编译检查，优先发现 `rtti/type_desc.rs` 或 itable 布局相关问题。
3. 运行新加的 run-pass fixture，确认运行期类型判断仍符合预期。
4. 运行 RTTI 定向验证，检查 `dump-rtti` 是否能看到 parameterized interface / `eff` target 的 precise runtime match 元数据。
5. 若 RTTI 输出不正确，修正 `crates/scoopc/src/rtti/type_desc.rs` 或相关收集逻辑，并重新验证。
6. 跑更广测试：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 必要时补充定向 fixture 测试
7. 确认任务完成后：
   - 更新 `TODO.md`，把 `T4007b` 标记完成
   - 更新 `PLAN.md`
   - 更新本文件，补充结果与测试结论
8. 提交 git commit，停止。

## 当前状态

- 已完成：
  - 核对工作区，确认当前未完成任务仍是 `T4007b`
  - 修复 `crates/scoopc/src/rtti/type_desc.rs` 中 `TypeDescError` 错误地透明包装 `miette::Report` 导致的编译失败
  - `cargo check --all-targets` 通过
  - 新增 run-pass fixture `type_check_cast_parameterized_interface_runtime_match_basic` 单独运行通过
  - `cargo run -p scoop -- dump-rtti ... --type StringReadable / PureManaged` 已验证精确 RTTI 元数据可见
  - 已补充 `dump_file_type_desc` 的 RTTI 单元回归，覆盖 parameterized interface 与 `eff` target 的 runtime match metadata
  - 新增 RTTI 单测 `dump_rtti_class_itable_entries_preserve_parameterized_runtime_match_metadata` 通过
  - 既有 `rtti::type_desc` 单测里因未走完整 typecheck 而遗留的无效源码片段已改为合法程序，整组 RTTI 单测恢复绿色
  - 新 fixture 已通过 `scoop test` runner 的 golden 校验
  - `cargo test --all` 通过
  - `cargo clippy --all-targets -- -D warnings` 通过
  - `TODO.md` / `PLAN.md` / `ISSUES.md` 已同步更新，`ISSUES.md` 第 15 条已收窄为“仅剩旧 RTTI 导出 API”
- 结论：
  - `T4007b` 已完成，无需新增 blocker 任务。
  - 下一项应推进 `T4007c`：收口旧 RTTI 导出 API 的参数化 nominal 门禁。
- 剩余动作：
  - 检查最终 diff
  - git commit 后停止

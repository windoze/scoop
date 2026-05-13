## 当前执行计划

1. 已确认第一个未完成任务为 `P7-T01`；最近提交 `[P6-T01R] Review RTTI and JSON closure` 未声明需先回插的新前置任务，因此按既有顺序继续执行 `P7-T01`。
2. 检查工作区状态，确认是否存在与本任务相关的未提交改动；若有阻塞性异常，先在此记录并同步到 `TODO.md`。
3. 阅读 `P7-T01` 涉及的测试/快照/审计 helper 入口，确定：
   - grep 审计清单如何落地
   - 受影响 fixture / snapshot 的刷新方式
   - 路径稳定性与多-cone 验证的现有测试基础
4. 补齐 `P7-T01` 所需实现或验证脚手架；若 full audit 暴露真实缺陷，则优先修复缺陷，或在必须时把最小前置任务写回 `TODO.md` 后停止。
5. 运行本任务要求的完整验证矩阵：定向 grep 审计、`cargo test -p scoopc`、`cargo test -p scoop_runtime`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、可行时 `cargo test --all`，以及 `cargo clippy --all-targets -- -D warnings`。
6. 如有 textual surface 变化，刷新相关 fixture / snapshot 并逐项确认只涉及 identity 层变化。
7. 完成后更新 `TODO.md` 的 `P7-T01` 为 `[DONE]` 并填写完成记录；仅在阶段计划本身改变时修改 `PLAN.md`。
8. 提交本次任务的全部相关改动并停止。

## 说明

- 这里记录的是可审计的执行计划与关键决策摘要，不包含逐字的内部推理展开。
- 如果实现过程中发现新的硬性前置依赖，会先更新该文件与 `TODO.md`，再停止在当前任务边界。

## 当前进展

- 已读取 `TODO.md` 并锁定当前任务：`P7-T01`。
- 已复核最近提交与 `P6-T01R` 完成记录：当前没有必须插回 `P7-T01` 之前的新前置任务。
- 已识别出 `P7-T01` 中“路径稳定性”缺少两处常驻覆盖：
  - dump path / dump label 跨 checkout 根路径稳定性
  - RTTI identity 跨 checkout 根路径稳定性
- 已在 `crates/scoopc/src/dump_support.rs` 与 `crates/scoopc/src/rtti/type_desc.rs` 补充对应回归测试，下一步进入格式化与定向验证。
- 全量矩阵首次运行在 `cargo run -p scoop -- test` 处发现 build fixture 仍锁定旧 private helper spelling；这是 `P7` 允许变化的 identity textual surface，属于 fixture 断言模型滞后，不是语义回归。
- 为保持验证力度，已把 `crates/scoop/src/fixtures` 扩展为支持 `BUILD-LLVM-REGEX`，并开始把受 stable-id 影响的 build fixtures 从旧固定字符串迁移到 hashed family regex 断言。
- build fixtures 收口后，`cargo run -p scoop -- test` 继续暴露真实编译缺陷：top-level callable-value fixture 在 `top_level_val_init` 内生成纯 direct-HIR closure object 时，没有注册 plain callable-carrier fallback，导致 callable carrier contract 误报缺少 published target entry。
- 已在 `crates/scoopc/src/llvm/codegen/closure/mod.rs` 为纯 direct-HIR closure 注册 plain fallback，并在 `crates/scoopc/src/llvm/tests.rs` 增加对应 LLVM codegen 回归；定向 run-pass fixture 已恢复通过。
- 后续全量回归又暴露 class init / ctor 路径的 generic callable 退化：
  - 一类 HIR 直调在缺少 published signature 时没有尽量回填 concrete arg type；
  - 一类 concrete materialized callable 在缺 authoritative instance key 时错误退回 generic HIR fun 计算 exported symbol。
- 已在 `crates/scoopc/src/llvm/codegen/call/lowering.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs` 修复这两条回退路径，并补了 class-init 相关回归测试/审计；`class_init_order_primary_secondary_basic` 已恢复 build/run 通过。
- 完整 `P7` 验证矩阵现已通过：`cargo test -p scoopc`、`cargo test -p scoop_runtime`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部成功。
- 已把 `TODO.md` 中的 `P7-T01` 标记为 `[DONE]`，并写回改动范围、核心决策、grep 审计分类与验证结果。下一步仅剩 git 提交并停止。

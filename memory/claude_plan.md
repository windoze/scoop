# 执行计划

## 目标
- 严格以 `TODO.md` 为准，识别首个未完成任务并只完成这一项。
- 如果遇到阻塞当前任务的真实缺陷或缺失能力，则先把该前置问题按要求写入 `TODO.md`，提交后停止。

## 执行步骤
1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交是否有与该任务直接相关且明确未完成的问题；若有，将其视为当前任务的一部分或在 `TODO.md` 中登记为前置任务。
3. 阅读与当前任务直接相关的代码、测试、规范和任务说明，确认实现边界、依赖和验收标准。
4. 实现任务要求，优先做最小且正确的修改，不引入规避性方案。
5. 运行与该任务相关的验证、测试、格式化和必要的 lint；若失败则继续修复直到满足要求，或在确认存在真实前置阻塞后回写 `TODO.md`。
6. 更新 `TODO.md`：将已完成任务标题加上 `[DONE]`，补全完成记录；仅在阶段计划真的变化时更新 `PLAN.md`。
7. 记录本文件中的关键进展与计划调整。
8. 按仓库约定创建一次 git 提交，然后停止，不进入下一个任务。

## 决策原则
- 不跳过顺序，不因为任务较大而默认拆分。
- 不用 workaround、fixture-only hack 或缩小范围来绕过规范缺口。
- 若发现阻塞当前任务的缺陷，优先修复；若本次无法直接完成，则在 `TODO.md` 中插入最小前置任务并停止。

## 进度记录
- 已创建执行计划，待读取 `TODO.md` 并确认当前任务。
- 已确认首个未完成任务为 `P2-T01`：分类 `module.add_function(..., None)` 调用点并建立统一 declaration/linkage helper。
- 最近一次提交为 `[P1-T02R] Review stable-id authoritative API`，提交摘要未显示与 `P2-T01` 直接相关的新增未完成阻塞项；继续按 `P2-T01` 执行。
- 已复核 `PLAN.md` P2 与 `STABLE_ID.md` §3.4.1/§3.4.4/§3.4.5/§7.4/§8.6，确认本任务重点是“先把 function declaration surface 显式分类”，不在本任务提前做完整 private internalize。
- 已用 `rg` 盘点 `crates/scoopc/src/llvm/**` 中全部 `module.add_function(...)` 调用点；除测试模式字符串外，当前高风险调用分为三类：
  - exported ABI / fixed external：top-level/source callable、materialized plain callable、host `main`
  - runtime/native import：`malloc`、`exit`、runtime ABI entry、`scoop_runtime_init`、`scoop_entry_argv_array`、`@Extern`
  - compiler-private helper：closure body、callee resume、effect helper/surface resume/outcome/transport thunk、object/top-level init bridge 与 init function
- 当前执行方案：先在 `llvm::codegen` 增加显式 surface/linkage helper（exported ABI、runtime/native import、compiler-private helper + linkage 参数），然后把上述 declaration path 全部改接 helper；再补测试验证 linkages/source inventory，最后跑格式化、测试与 clippy。
- 已完成 helper 接线：`codegen/mod.rs` 新增统一 declaration/linkage helper；`emit.rs`、`runtime_abi.rs`、`mir_body.rs`、`closure/mod.rs`、`object_init.rs`、`effect_lowered/{layout,body,value}.rs` 均已改为显式声明 exported/import/private surface，不再直接留下 raw `module.add_function(..., None)` 调用点。
- 已补两类测试：helper linkage 行为测试、源代码 inventory 测试（阻止 raw `add_function(..., None)` 回流）。
- 已执行 `cargo fmt`，下一步运行定向测试与全量验证。
- 定向验证已通过：`function_declaration_*`、`stable_id_audit*`、`external_symbol*`。
- 全量验证已通过：`cargo test -p scoopc`、`cargo clippy -p scoopc --all-targets -- -D warnings`。
- 额外审计：`rg -n "module\.add_function\(.*None\)" crates/scoopc/src/llvm` 已无命中；source inventory 测试也覆盖了多行 `add_function(..., None)` 回流场景。

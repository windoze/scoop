# 本轮执行计划

## 说明

按要求，先在此文件记录本轮的可执行计划、检查顺序、关键判断依据和进度更新。这里不会逐字记录内部私有推理，但会完整记录外部可审计的执行步骤、发现的问题、采取的修复动作、测试结果与计划调整。

## 初始步骤

1. 检查最近一次提交信息与提交内容，确认是否提到了尚未修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划是否与 `TODO.md` 一致。
4. 如果第一个未完成任务过大或存在前置依赖缺口，则先细化任务并更新 `PLAN.md` / `TODO.md`。
5. 实施当前应执行的第一个任务，同时在过程中留意任何既有缺陷、回归、规约不匹配或不完整实现边界；若发现，则优先修复或把它作为新的前置任务插入 `TODO.md` 并停止继续推进原任务。
6. 运行与改动相关的测试；如果任务完成后范围允许，再补充更高层验证，至少确保本次改动路径通过。
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态或新的依赖调整。
8. 提交本轮改动，提交后停止。

## 当前假设

- 目前尚未确认最近一次提交是否声明了遗留问题。
- 目前尚未确认 `TODO.md` 的第一个未完成任务是什么。
- 在读取仓库现状前，不预设任务内容或实现方案。

## 进度日志

- 已创建本文件并写入初始计划，下一步将检查最近一次提交与任务清单。
- 已检查最近一次提交：
  - `git log -1 --format=fuller` 显示提交为 `[T5000dR] Fix dump-mir generic template boundary`；
  - 提交说明中没有额外声明“尚未修复的既有问题”，因此当前无需先插入基于提交说明的额外修复项。
- 已定位 `TODO.md` 的第一个未完成任务：
  - `T5000e 在 MIR 层实现 monomorphization / instance materialization`。
- 已核对当前实现边界：
  - `crates/scoopc/src/monomorph/` 仍是早期调试/实例生成模块；
  - `crates/scoopc/src/hir/lower/mod.rs` 的多文件 lowering 仍通过 `materialize_generic_fun_instances` 和 `collect_generic_fun_instantiations(...)` 直接在 HIR 层生成 `::<...>` 实例；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 仍保留 `try_resolve_monomorphized_member_fqn` / `try_resolve_monomorphized_standalone_fun_fqn`，说明 LLVM codegen 仍在现场承担一部分单态化目标解析职责；
  - 这与 `T5000e` 的目标一致，说明任务是真实责任迁移，不是单点小修。

## 当前判断

- `T5000e` 当前过大，至少同时包含：
  1. 引入 backend-agnostic `InstanceKey`；
  2. 建立 generic MIR template -> monomorphic MIR instance 的实例化机制；
  3. reachable-driven / on-demand / per-instance cache；
  4. 把 frontend / codegen 从“HIR 预实例化 + codegen 现场猜目标”迁移到新的实例化边界。
- 预计需要先在 `TODO.md` / `PLAN.md` 中拆分 `T5000e`，再完成拆分后的第一个子任务。
- 下一步将进一步确认一个“本轮可完整交付且可测试”的首个子任务边界，并据此更新 `TODO.md` / `PLAN.md`。

## 任务拆分结果

- 已更新 `TODO.md` / `PLAN.md`，把 `T5000e` 拆为：
  1. `T5000e1`：引入 `InstanceKey`，并把 `dump-ir` 路径迁到真正的 MIR template → instance materialization；
  2. `T5000e2`：把 compilation unit frontend/build 的 instance collection / materialization 主路径迁到 MIR；
  3. `T5000e3`：让 LLVM codegen 改为消费已实例化 target identity，并删除现场猜测 monomorphized target 的主路径。
- 当前本轮执行目标已切换为：`T5000e1`。

## T5000e1 计划

1. 设计新的 `InstanceKey` 与 dump-ir materializer 的最小数据结构。
2. 复用现有 typed dump 前端事实，生成 generic MIR template，并建立模板索引。
3. 在 MIR 层实现单文件/调试路径的实例 materialization：
   - 以 typecheck 收集到的 monomorph 请求作为初始种子；
   - 对 direct generic call 做 fixed-point 发现；
   - 对 closure family 做实例化时的 FQN / fn_ptr 重写；
   - 用 per-`InstanceKey` cache 去重。
4. 把 `dump-ir` 与旧 `monomorph` 调试入口切到新 materializer。
5. 补充/迁移测试，覆盖：
   - `InstanceKey` 稳定性；
   - generic direct call 改写到实例；
   - nested closure family 在实例化后不会残留 generic fn_ptr；
   - 现有 `Virtual` / `Perform` 等 MIR 结构事实在实例体中继续保留。
6. 运行格式化、相关单测、全量测试与 clippy；若发现既有问题，优先修复。

## T5000e1 完成情况

- 已完成 `T5000e1` 的实现与验证。
- 主要实现：
  - 新增 `crates/scoopc/src/mir/materialize.rs`，引入 `TemplateKey`、`InstanceKey`、`MaterializedMir` 与 `materialize_for_dump(...)`；
  - `dump-ir` 现改为直接走 MIR materializer，并打印包含 `instance_keys` 的 `MaterializedMir` Debug 视图；
  - 旧 `crates/scoopc/src/monomorph/lower.rs` 已收口为兼容薄包装；
  - `MonomorphKey` 的注释与定位已修正为“typecheck 收集到的实例请求”，不再暗示它是最终实例身份；
  - 迁移后暴露的既有无效接口 `LoweredFunWithSideTables` 已一并清理，避免留下 warning。
- 回归测试：
  - `cargo test -p scoopc monomorph::lower -- --nocapture`
  - `cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。
- 文档/任务状态：
  - 已在 `TODO.md` 将 `T5000e1` 标记为完成；
  - 已在 `PLAN.md` 追加 `T5000e1` 的完成记录，并将下一条待执行任务更新为 `T5000e1R`。

## 本轮执行计划

### 目标
- 按照 `TODO.md` 的顺序，只完成第一个未完成任务，然后停止。
- 在开始具体实现前，先检查最新提交是否提到既有问题；若有，先修复这些问题。

### 执行步骤
1. 检查最新一次 Git 提交信息，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划、依赖关系与任务上下文。
4. 如首个未完成任务过大，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本轮只执行拆分后的第一个子任务。
5. 阅读与当前任务相关的源码、测试和规范，确认实现边界。
6. 实现任务；若遇到规格不匹配、缺失特性或前置缺陷，则按要求把问题转化为更前置的 `TODO.md` 任务，并更新 `PLAN.md`。
7. 运行相关测试、`cargo fmt`、必要的检查以及 `cargo clippy --all-targets -- -D warnings`，修复发现的问题。
8. 更新文档与任务状态：
   - 在 `TODO.md` 中标记本轮完成的任务；
   - 在 `PLAN.md` 中记录进展、调整与原因；
   - 按需要补充 `README.md`、代码注释或其他说明。
9. 查看工作区变更，确认无误后提交 Git commit，提交信息对应当前任务。
10. 停止，不继续处理下一个任务。

### 当前状态
- 已创建本文件并写入初始计划。
- 已检查最新提交、`TODO.md`、`PLAN.md` 和代码状态。
- 最新提交 `ef6cced` 的提交信息仅为 `Update plan`，未额外点名需要优先修复的既有实现问题。
- 当前工作区除本文件外无其他改动。
- 已定位首个未完成任务：`T2003r3d2`。

### 下一步
- 已完成：
  - 阅读 `T2003r3d2` 在 `TODO.md` / `PLAN.md` 中的描述与上下文；
  - 审计 `state_machine_plan.rs`、`shared.rs`、`nonresuming.rs`、`multi_escape.rs`、`codegen/mod.rs` 中与 unified resuming emitter 相关的占位；
  - 确认 `PLAN.md` 中对 `T2003r3d2` 的旧描述已过时，当前 `TODO.md` 才反映真实任务范围。
- 结论：
  - `T2003r3d2` 当前同时耦合三类工作：`plan-owned metadata`、plan-driven resolver/helper、以及 unified single/multi resuming leaf 接线。
  - 该范围过大，需先拆成更小子任务；否则一次改动会同时触及 plan 构建、capture/reads 元数据、`resume(value)` / `k.resume(value)` 调用链以及 multi-arm leaf，风险过高。

### 接下来的执行顺序
1. 已完成：更新 `TODO.md` / `PLAN.md`，把 `T2003r3d2` 按代码边界拆成 `T2003r3d2a` / `T2003r3d2b` / `T2003r3d2c`。
2. 当前执行：实现拆分后的第一个子任务 `T2003r3d2a`，优先补齐 `plan-owned metadata` 与后续 leaf 需要的 plan-driven resolver/helper。
3. 完成实现后运行该子任务对应的最小定向测试与 `cargo clippy --workspace --all-targets -- -D warnings`。
4. 回写本文件、`TODO.md`、`PLAN.md` 的进度并提交 Git。

### 最新进展
- `TODO.md` / `PLAN.md` 已对齐；`PLAN.md` 中关于 `T2003r3d2` 的旧“nested while indirect”描述已替换为新的 metadata / single-resuming / multi-resuming 三段拆分。
- 当前首个未完成任务已变更为 `T2003r3d2a`。

### 说明
- 本文件记录的是可审计的执行计划、决策摘要与进度更新，不包含冗长的内部推理展开。

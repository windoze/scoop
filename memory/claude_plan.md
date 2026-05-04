## 当前执行计划

1. 先读取 `TODO.md`，把它当作任务索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，找到第一个标题未标记 `[DONE]` 的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；如果该问题构成当前任务前置依赖，则在对应 `TODO-Px.md` 中补充前置任务并同步 `TODO.md`。
4. 在不做开放式历史问题排查的前提下，只围绕当前任务和其直接阻塞项建立实现上下文。
5. 完整实现当前任务；如果遇到无法规避且未跟踪的真实前置阻塞，则最小化新增前置任务，更新详细任务文件与 `TODO.md`，必要时才更新 `PLAN.md`。
6. 运行与该任务直接相关的测试、格式化、以及需要的质量检查；修复发现的问题，直到当前任务达到完成标准，且不依赖变通方案。
7. 在对应 `TODO-Px.md` 中将当前任务标题显式标记为 `[DONE]`，补充完成记录；若索引内容受影响，则同步更新 `TODO.md`。
8. 仅在阶段计划、依赖结构或完成标准发生变化时更新 `PLAN.md`；否则不把它当作日常执行日志。
9. 检查工作区状态，确保本次任务相关改动完整纳入一次提交；若是恢复上次失败后遗留的同一任务，也会一并提交现有未提交文件。
10. 使用清晰的任务号提交信息创建 git 提交，然后停止，不继续处理下一个任务。

## 执行记录

- 已按要求先写入本计划文件，后续如果计划变化或关键步骤完成，会继续更新这里。
- 已读取 `TODO.md` 与 `TODO-P6-part2.md`，确认首个未完成详细任务是 `P6-T03`：按 P5 state graph / boundary contract 完成 refactor LLVM body lowering。
- 已检查最新提交：`[P6-T02q] Publish resume-boundary surface route bridge`。提交信息未显式引入新的未完成直接 blocker，因此继续按 `P6-T03` 本体执行。
- 下一步：检查当前工作树状态，并阅读 `crates/scoopc/src/llvm/codegen/effect_refactor/*`、相关 late-lowered handoff、以及现有 LLVM body/codegen 入口，确定最小正确改动面。
- 已确认当前 refactor LLVM 入口仍只做 ABI materialization，并在 `emit.rs` 里对 effectful body fail-fast；`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 仍是占位文件，说明 `P6-T03` 的主体实现尚未开始。
- 已通过 `dump-effect-lowered` 验证当前 late-lowered contract 的关键形状：
  - callable body 已按 `StateId` / `BoundaryId` / `HandleDispatch` 发布完整 state graph；
  - continuation object 只捕获 heap frame 指针、resume state 和 one-shot flag，同时挂 packing vtable；
  - frame schema 已把跨 suspension 需要持久化的 source local / boundary result / resume payload / binder / system slot 显式化。
- 因此当前实现计划收敛为：
  1. 在 `effect_refactor/body.rs` 新增 refactor body emitter，负责 materialize frame、continuation、step value、boundary dispatch、handle dispatch、resume entry 与 local runtime error；
  2. 在 refactor path 下为 callable `direct_entry` / `dynamic_entry` / closure-vtable-itable carrier wrapper / surface-resume owner trampoline / resume method 生成 LLVM body；
  3. 调整 `llvm/emit.rs`：refactor 模式下不再对 late-lowered callable 走 legacy HIR/MIR body lowering，入口 `main` 改为调用 refactor direct entry；
  4. 新增/更新定向测试与 build/run fixture 验证，再补 `TODO-P6-part2.md` 完成记录并提交。
- 在真正进入 `P6-T03` 实现前又确认了一个更前置的真实 blocker：当前 handoff 对 cleanup/finally/pending-outward path 只发布了 `ResumePayloadCarrier: Any` 的槽位存在性，却没有 authoritative 发布 typed case payload 如何穿过该 carrier。
- 这意味着如果现在继续写 `P6-T03` backend，就只能现场发明 `Any` boxing / projection 或 raw transport 规则；这违反了当前 TODO 的 contract-first/no-workaround 要求。
- 因此本次计划已切换为：
  1. 在 `TODO-P6-part2.md` 中新增最小前置任务 `P6-T02qb`，专门发布 cleanup/finally pending payload carrier contract；
  2. 同步更新 `TODO.md` 索引，以及 `P6-T03` 的依赖和 blocker 记录；
  3. 提交这些任务文档改动并停止，等待下一次 invocation 先完成 `P6-T02qb`。

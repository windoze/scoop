## 当前回合执行计划

说明：这里记录可审计的执行计划、关键判断依据、进度更新与计划调整。出于安全与协作边界考虑，不记录逐字内部推理，而是记录足以复核工作的决策摘要。

### 初始目标

1. 在不跳过任何前置问题的前提下，检查最新提交是否提到已有缺陷；若有，先纳入本轮范围并修复。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否足够小且可在本轮完整实现、测试、文档化并提交。
4. 若任务过大或被真实实现缺口阻塞：
   - 细化为更小的子任务，更新 `PLAN.md` 与 `TODO.md`。
   - 若存在规范不匹配、语言特性缺失、运行时缺口或错误实现，先把该缺口作为新的前置任务写入 `TODO.md`，调整依赖顺序，然后停止。
5. 若任务可执行：
   - 完整实现。
   - 运行相关测试，并补充必要测试。
   - 运行格式化、lint 与必要的全量或定向验证，目标是不引入警告。
   - 更新 `TODO.md` 与 `PLAN.md`。
   - 提交一次清晰的 Git commit。
   - 停止，不继续下一个任务。

### 执行顺序

1. 查看最新提交信息与变更摘要，确认是否显式提到待修复问题。
2. 查看 `TODO.md` 与 `PLAN.md` 当前状态。
3. 如有必要，先查看相关规范、源码与测试位置，建立最小实现上下文。
4. 修改代码与测试。
5. 验证、更新文档、提交。

### 进度记录

- [x] 已写入本计划文件。
- [x] 检查最新提交是否包含待修复问题。
- [x] 识别 `TODO.md` 中第一个未完成任务。
- [x] 判断是否需要任务拆分或前置依赖重排。
- [ ] 实现本轮任务。
- [ ] 测试与 lint。
- [x] 更新 `TODO.md` / `PLAN.md`。
- [ ] 提交变更并停止。

### 变更记录

- 初始计划已建立，待开始仓库检查。
- 已检查 `HEAD` 提交 `5d1b90063b181a5919cd3982a2b07957b7faeb86`：提交主题为“`[T4003SR] 修复顶层 val 递归初始化读取`”，未见新的“已知但未修”的提交说明，本轮无需先补一个额外提交遗留项。
- 已读取 `TODO.md` / `PLAN.md`：首个未完成任务为 `T4004`，目标是打通顶层 `val` / `var` 的 pattern binding；其前置依赖 `T4003SR` 已完成。
- 进一步检查后确认当前不能直接执行原 `T4004`：
  - 规范约束：`SCOOP_FULL_SPEC.md` §4.2 / Appendix B.11 明确 destructuring 仅适用于 `val`，`var` 不支持 destructuring patterns；因此原任务标题中的“`val` / `var`”本身与规范不一致，必须先收窄为“顶层 `val` pattern binding”。
  - 实现缺口：最小 probe `fun main(): Int { val pair: (Int, Int) = (1, 2); val (a, b) = pair; return a + b }` 当前 `cargo run -p scoop -- build /tmp/t4004_local_destructuring_probe.scoop -o /tmp/t4004_local_destructuring_probe.out` 直接报 `scoop::llvm::unsupported_main_body: anonymous val binding`。这说明局部 `val` pattern binding 仍只有 parser/typecheck，没有可执行 lowering/codegen 主线；若直接实现顶层版本，只能新增顶层专用旁路，违反“顶层与局部复用同一套语义”的要求。
- 已据此更新任务规划：
  - 在 `T4003SR` 与 `T4004` 之间插入新的 blocker：`T4003T`（局部 `val` pattern binding lowering/codegen）与 `T4003TR`（review）。
  - 将原 `T4004` 拆分为 `T4004a -> T4004b -> T4004R`，分别处理顶层 binder 的符号/类型收集、once-init lowering/codegen 与复审。
  - `TODO.md` / `PLAN.md` 已同步更新；本轮按流程应提交这些任务重排变更后停止，不进入实现阶段。

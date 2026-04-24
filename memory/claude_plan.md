# 本轮执行计划

说明：按安全要求，这里记录的是可审计的简明推理摘要与逐步执行计划，不写不可验证的内部长链路思维。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；如果在检查最新提交、测试、实现或审阅过程中发现任何既有问题，则优先修复该问题，或把它作为前置任务插入 `TODO.md` 后停止。

## 约束与执行原则

1. 先检查最新提交，确认是否提到需要先修复的遗留问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 对当前要执行的首个任务：
   - 实现；
   - 运行相关测试；
   - 修复测试中暴露的既有问题；
   - 更新 `TODO.md` / `PLAN.md`；
   - 提交 git commit；
   - 停止，不继续做下一个任务。
5. 不接受绕过方案；若发现规范不匹配、实现缺口、回归或边界未完成，必须优先修复，或将其登记为前置任务并停止。

## 具体步骤

1. 查看最新一次提交信息与变更摘要，判断是否提到了未解决问题。
2. 检查工作区状态，避免误覆盖用户已有改动。
3. 打开 `TODO.md` 和 `PLAN.md`，确定当前任务及上下文。
4. 评估任务规模与依赖，必要时先重排任务并记录原因。
5. 在代码库中定位相关模块与测试。
6. 实施修改，并补充/调整测试。
7. 运行必要验证：
   - 优先运行与改动直接相关的测试；
   - 再运行任务要求的更完整检查（至少包括相关 `cargo test`，必要时 `cargo clippy --all-targets -- -D warnings`）。
8. 更新任务文档与计划文档。
9. 生成单个清晰提交并停止。

## 进度记录

- [x] 已写入本计划文件，尚未开始仓库检查。
- [x] 检查最新提交：最新提交为 `[T4017f] Migrate remaining effect boundaries to explicit outcome`，提交信息本身未额外声明需先修复的遗留问题。
- [x] 读取并定位首个未完成任务：当前首个未完成任务是 `T4017R`（review）。
- [x] 评估是否需要拆分：`T4017R` 为 review 条目，本轮不再拆分，按“代码审查 + 定向搜索 + 全量验证”执行。
- [ ] 审查 ordinary/effect/continuation 生产路径，确认是否仍把 TLS 当成权威语义
- [ ] 运行验证命令并确认无回归
- [ ] 更新 `TODO.md` / `PLAN.md`
- [ ] 提交并停止

## 当前审查焦点

1. ordinary direct / closure / funptr / vtable / itable / object init / top-level init / extern boundary 是否统一切到显式 `EffectCtx + EffectOutcome` contract。
2. remaining TLS 读写是否只剩：
   - 局部 transport / scratch；
   - 调试 / 测试观察口；
   而不是继续作为 production source of truth。
3. continuation capture / resume 是否只依赖 captured `ctx/frame/resume token`，而非 resuming thread 进入前的 ambient TLS。

## 已完成的审查摘要

- 已通过全文搜索确认：
  - `TODO.md` / `PLAN.md` 的下一条就是 `T4017R`；
  - 文档叙事整体已切到显式 `EffectCtx` / `EffectOutcome`；
  - `scoop_callee_suspend_state_get` 已不再被生产 codegen 调用，仅保留在 runtime / 测试接口中；
  - ordinary direct / closure / funptr / legacy boundary 代码路径已经显式调用 `consume_current_effect_outcome_into(...)` / `publish_effect_outcome_from_slot(...)`。
- 仍需最后确认：
  - `state_machine_emitter` 中剩余的 `scoop_effect_is_active` / perform-slot 读取是否都只是局部 transport，而不是重新成为 source of truth；
  - 通过全量测试验证没有遗漏回归。

## 最新进展

- 已完成 production review：
  - ordinary direct / closure / funptr / vtable / itable / object init / top-level init / extern-native boundary 已统一走显式 `EffectCtx + EffectOutcome` contract；
  - `scoop_callee_suspend_state_get()` 已不再被生产 codegen 用作恢复入口；
  - `state_machine_emitter` 中残留的 TLS 读取与文档一致，属于 direct `perform` / hidden-suspend / arm-cleanup 的局部 transport。
- 已完成验证：
  - `cargo run -p scoop -- test` -> `fixtures: ok (1174)`
  - `cargo test --all` -> 通过
  - `cargo clippy --all-targets -- -D warnings` -> 通过
- 已完成文档状态更新：
  - `TODO.md` 中将 `T4017` / `T4017R` 标记为完成，并记录 review 结论与复验命令；
  - `PLAN.md` 中将主线切换到 `T4012b3`。

## 待收尾

- [x] 审查 ordinary/effect/continuation 生产路径，确认是否仍把 TLS 当成权威语义
- [x] 运行验证命令并确认无回归
- [x] 更新 `TODO.md` / `PLAN.md`
- [ ] 检查工作区差异并提交 git commit

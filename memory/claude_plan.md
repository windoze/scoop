# 当前执行计划

## 说明

按你的要求，我会先把可审阅的工作思路摘要和执行步骤写在这里，并在后续关键进展发生时持续更新。这里记录的是可执行计划、判断依据和状态变化，不包含逐字展开的内部推理草稿。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

在进入该任务之前，先检查最新一次提交是否提到任何既有问题；如果提到了，就优先修复这些问题。随后再读取 `TODO.md` 与 `PLAN.md`，确认当前的第一项未完成工作及其依赖关系。

## 执行步骤

1. 查看最新提交信息与变更背景，确认是否存在提交中明确提到但尚未修复的既有问题。
2. 读取 `TODO.md`、`PLAN.md`，找出第一项未完成任务。
3. 判断该任务是否过大：
   - 如果可直接完成，就实施。
   - 如果过大或存在前置缺口，就先把任务拆分，更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的第一项。
4. 在实现过程中，如果探测到任何既有 bug、回归、规范不匹配、实现边界缺失或测试依赖 workaround：
   - 立即把该问题视为当前范围内事项；
   - 先修复，或者把修复任务插入到被阻塞任务之前并更新计划，然后停止。
5. 完成当前目标后，运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 如适用，运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，或在时间/作用域上采取更精准但足够覆盖的验证。
6. 更新文档状态：
   - 在 `TODO.md` 中标记已完成任务；
   - 在 `PLAN.md` 中记录当前进度与后续状态；
   - 同步更新本文件，反映关键结论和执行结果。
7. 提交本轮变更，提交信息聚焦本轮完成的单个任务。
8. 停止，不进入下一个任务。

## 当前状态

- 已完成：创建本计划文件。
- 已完成：检查最新提交，未发现提交说明里另有必须优先处理的未修问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认当前第一项未完成任务为 `T4017a`。
- 已完成：执行 `T4017a`，统一 effect / continuation 文档叙事并通过验证。
- 进行中：更新任务状态文件并提交本轮变更。

## T4017a 具体执行计划

### 任务判断

`T4017a` 是文档与注释收口任务，范围明确，可直接完成，不需要继续拆分为更小的 `TODO` 子项。

### 预定修改点

1. 更新 `CONTINUATION.md`
   - 将“设计草案”收口为实现导向文档；
   - 明确 `EffectCtx`、`EffectSignal`、`EffectOutcome`、frame、continuation 的职责边界；
   - 把 `T4017a -> T4017f` 的 staged rollout 写得更可执行。
2. 更新 `SCOOP_FULL_SPEC.md`
   - 删除把 effect propagation 与 cross-thread resume 直接表述为 TLS source-of-truth 的段落；
   - 改成以“捕获的动态 effect context + eager outcome”为规范叙事；
   - 保留“实现可暂时通过 TLS/其他内部手段承载部分过渡细节，但这不是规范语义”的边界。
3. 更新 `SCOOP_RUNTIME.md`
   - 在 continuation / effect contract 章节中对齐显式 `EffectCtx` / `EffectOutcome` 模型；
   - 说明当前运行时仍在迁移途中，但后续阶段的权威合同已经变化。
4. 更新 `docs/effect_unified_state_machine.md`
   - 把统一状态机设计与 `EffectCtx` / `EffectOutcome` 迁移路线对齐；
   - 删除或改写“继续沿用 TLS handler stack / perform slot 模型”之类表述。
5. 更新必要注释
   - 当前已定位到 `runtime/c/scoop_runtime.c` 存在注释直接把 `active flag + perform slot` 写成 effect 语义判定依据，需要改成“当前 transport / 调试骨架，不是最终 source-of-truth”。

### 验证计划

1. 先做文档一致性自检，确认四份文档的关键词和迁移顺序一致。
2. 如果改动了 `SCOOP_FULL_SPEC.md` 中 tagged fixture 代码块，则运行：
   - `cargo run -p scoop_tools -- spec-fixtures sync`
   - `cargo run -p scoop_tools -- spec-fixtures check`
3. 无论是否需要 `sync`，至少运行 `cargo run -p scoop_tools -- spec-fixtures check`。
4. 文档/注释改动不触及可执行逻辑时，优先做针对性验证；若过程中发现文档与实现存在新的既有不一致，需要先处理该问题或把它前置到 `TODO.md`。

## 已完成进展

### 已完成的修改

1. 已更新 `CONTINUATION.md`
   - 标题与状态改为 `T4017` 实施基线；
   - 补充“语义权威边界与过渡 transport”；
   - 将迁移路径细化并直接映射到 `T4017a -> T4017f`。
2. 已更新 `SCOOP_FULL_SPEC.md`
   - continuation / cross-thread resume 改为围绕捕获的动态 effect context 叙述；
   - 删除“TLS 是权威语义”式表述，改为“TLS 可能只是实现细节/过渡桥接”。
3. 已更新 `SCOOP_RUNTIME.md`
   - 明确 `EffectCtx` / `EffectOutcome` / `EffectSignal` 的职责；
   - 标注当前 runtime 中 TLS 相关状态仍属 staged migration 的 bridge machinery。
4. 已更新 `docs/effect_unified_state_machine.md`
   - 将统一状态机设计与 `T4017` 的 `ctx + outcome` 合同对齐；
   - 改写了原先直接把 handler stack / TLS 写成语义前提的段落。
5. 已更新 `runtime/c/scoop_runtime.c`
   - 把相关注释改为“当前 TLS 仅是过渡 transport / scratch，而非最终 source-of-truth”。

### 已完成的验证

1. `cargo run -p scoop_tools -- spec-fixtures check` 通过。
2. `cargo test --all` 通过。
3. `cargo clippy --all-targets -- -D warnings` 通过。

### 收尾步骤

1. 更新 `TODO.md` 与 `PLAN.md` 的当前状态。已完成。
2. 复查 diff，确认仅完成 `T4017a`，未越过到 `T4017b`。已完成。
3. 提交本轮变更并停止。进行中。

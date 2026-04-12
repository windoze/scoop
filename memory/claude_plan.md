# 本轮执行计划

## 约束说明

- 按用户要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在开始仓库检查前，先记录本计划；随着执行推进，会持续更新本文件。
- 不在实现缺口上做绕过；如果发现规范不匹配或前置依赖缺失，会先调整 `TODO.md`/`PLAN.md`，提交后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否提到已有问题需要优先修复。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务。
3. 判断该任务是否过大；若过大，则拆分为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
4. 实施当前应执行的首个任务，必要时补充或调整测试。
5. 运行相关验证，至少覆盖受影响范围；如有必要，运行更完整测试与 `clippy`。
6. 将任务完成状态回写到 `TODO.md`/`PLAN.md`，并更新本文件记录结果。
7. 生成一次 Git 提交，然后停止，不继续后续任务。

## 当前状态

- 已检查最新提交：`01f9ceb Update plan`，提交说明未直接提到需要先修复的遗留 bug。
- 已定位本轮首个未完成任务：`T2003u1`（Effect：统一状态机 pass 设计定稿与不变量收口）。
- 判断结果：`T2003u1` 是可在单轮内完成的设计/文档收口任务，不需要继续拆分子任务。

## T2003u1 执行计划

1. 审阅 `TODO.md` / `PLAN.md` 中 `T2003u1` 的目标与依赖，确认验收要求。
2. 审阅 effect 现有代码注释与规范里关于 stack/heap state machine、runtime ABI 的现状描述。
3. 在仓库中新增统一状态机设计文档，明确：
   - 输入与输出；
   - 状态表示、suspend site、cleanup edge、capture/body-lift；
   - never-resume / immediate-resume / escape-continuation 的统一关系与化简；
   - 与现有 runtime ABI、payload transport、handler stack、one-shot continuation 的对接。
4. 更新 `PLAN.md` / `TODO.md` / 相关注释，使主线表述统一为“先构建完整状态机，再做 mode-specific simplification”。
5. 运行验证命令，至少覆盖 `cargo test --all`；如无额外阻碍，补跑 `cargo clippy --workspace --all-targets -- -D warnings`。
6. 回写完成状态到 `TODO.md` / `PLAN.md` / 本文件，提交 Git commit，然后停止。

## 已完成的关键步骤

- 已确认 `T2003u1` 不需要继续拆分。
- 已审阅 `TODO.md` / `PLAN.md` 与 effect 代码注释，确定本轮输出物应为“设计定稿文档 + 主线表述收口”。
- 已新增 `docs/effect_unified_state_machine.md`，写明统一状态机 pass 的输入/输出、状态表示、不变量、化简规则与 runtime ABI 对接。
- 已更新 `TODO.md`、`PLAN.md`、`README.md`、`crates/scoopc/src/llvm/codegen/effect/mod.rs`，将主线统一为“先构建完整状态机，再做 mode-specific simplification”。
- 已完成验证：
  - `cargo test --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - 结果：均通过。

## 待执行

1. 检查工作树并提交 Git commit。
2. 提交后停止，不继续后续任务。

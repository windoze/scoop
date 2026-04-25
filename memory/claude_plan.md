# 执行计划与进度记录

## 说明

按要求，在执行任何命令前先建立本文件。出于安全与隐私限制，这里不记录完整的内部推理细节，而是记录可审计的执行计划、关键判断、实施步骤与进度更新。

## 初始执行计划

1. 检查最新一次 Git 提交，确认是否提到了需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划与任务依赖关系。
4. 结合代码与测试现状，判断该任务是否可以在本轮完整完成。
5. 如果任务过大，先将其拆分为更小的前置子任务，并更新 `TODO.md` 与 `PLAN.md`，然后只执行第一个子任务。
6. 如果在调查、测试或实现过程中发现任何既有缺陷、回归、规格不匹配、未完成边界或临时绕过逻辑，立即将其视为当前范围内问题，优先修复，或者作为前置任务插入 `TODO.md` 后停止继续推进。
7. 实现当前目标任务。
8. 运行相关检查与测试，至少覆盖：
   - 受影响模块的定向测试
   - 必要的工作区测试
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
9. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态或阻塞原因。
10. 提交 Git commit，然后停止。

## 进度

- 已创建计划文件，待开始仓库检查。
- 已检查最新提交 `b8584c0be7cc9df3538c9b5f9be1fea7523169ed`，提交说明未声明需要优先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T5000c2R Review：确认 EffectAnalysisCtx 已脱离 LLVM backend 现场取数`。

## 当前任务：T5000c2R

### Review 目标

1. 确认 `EffectAnalysisCtx` 定义本身不依赖 LLVM backend 类型或 `MainCodegen` 状态。
2. 确认 effect/state-machine 分析主路径不再要求通过 `MainCodegen` 现场取数。
3. 确认 local metadata / synthetic symbol / source-path 上下文已作为稳定输入边界存在。
4. 运行定向测试与质量检查；若审查中暴露既有问题，优先修复该问题，再决定是否能完成当前 review。

### 当前已收集证据

- `crates/scoopc/src/effect_analysis.rs` 中 `EffectAnalysisCtx` 目前只依赖 `hir`、`TypeId`、`ProgramFacts`、`PathBuf` 与标准库容器/内部可变性；
- `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中 `HandlePlanContext` 已退化为 `type HandlePlanContext = EffectAnalysisCtx;`；
- 仍需继续核对：
  - 是否还存在分析逻辑必须从 `MainCodegen` 直接读取 state 的路径；
  - `state_machine_segments.rs` / `state_machine_transform.rs` 测试 helper 是否已统一走共享 analysis context；
  - 是否存在新的 backend 边界泄漏或 review 过程中暴露的既有缺陷。

### Review 结论

1. `EffectAnalysisCtx` 本体已满足 backend-agnostic 目标：
   - 不依赖 LLVM backend 类型；
   - 统一承接 known fun/local effects、known local metadata、synthetic symbol allocator、source-path / call-site 与 `ProgramFacts`。
2. 未发现残留的“必须通过 `MainCodegen` 才能做分析”路径：
   - `MainCodegen::effect_analysis_ctx()` 仅是 backend -> shared context 的单向投影；
   - `state_machine_segments.rs` / `state_machine_transform.rs` 测试 helper 已统一通过 `collect_effect_analysis_context_for_fun(...)` 构造共享上下文。
3. 未发现需要插入到 `T5000c3` 之前的新前置缺陷：
   - `effect_step_summary.rs` 仍通过 `include!` 复用 `state_machine_plan.rs`，但这已经由 `T5000c3` 显式跟踪。

### 已执行验证

- `cargo fmt --all --check`
- `cargo test -p scoopc llvm::`
- `cargo test -p scoopc --no-default-features`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

### 当前状态

- 已完成 `T5000c2R` 文档回写，下一条应为 `T5000c3`。

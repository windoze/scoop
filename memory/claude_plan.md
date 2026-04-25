# 本轮执行计划

## 约束说明

- 我不会写入不可共享的逐字内部思维过程，但会在此文件持续记录可审计的执行计划、依据、关键决策、发现的问题与完成状态。
- 本轮目标是：先检查最新提交是否提到需要先修复的既有问题；随后读取 `TODO.md`，锁定第一个未完成任务；如果任务过大则先拆分并更新 `PLAN.md`/`TODO.md`；然后只完成当前首个任务，补齐测试、文档、任务状态与提交，最后停止。

## 初始步骤

1. 检查当前工作树状态，避免误覆盖现有改动。
2. 查看最新提交信息，确认是否显式提到必须先处理的既有问题。
3. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务以及现有计划上下文。
4. 结合代码与测试现状评估任务复杂度；若过大，则先拆分任务并更新 `TODO.md`/`PLAN.md`。
5. 实现当前首个应执行任务。
6. 运行与改动相关的验证，至少覆盖任务相关测试；若影响面较大，再补充 `cargo test` / `cargo clippy --all-targets -- -D warnings` 等检查。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，标记完成状态与关键结论。
8. 使用清晰提交信息创建一次 git commit，然后停止。

## 进度记录

- 已创建本文件并写入初始计划。
- 已检查当前工作树：仅有本文件改动。
- 已检查最新提交 `eacc54cf [T5000d3] Regularize perform and provenance MIR entry points`：
  - 提交正文没有额外说明新的已知前置缺陷；
  - 但相关实现涉及 `TODO.md`/`PLAN.md`/`memory/claude_plan.md` 更新，因此仍需按 `T5000d3R` 对该轮改动做结构复核。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务是 `T5000d3R Review：确认 generic early MIR template 的调用与 control-transfer 入口已经成型`。

## 当前任务：T5000d3R

### review 目标

1. 复核 `Perform` / `Resume` / `Direct|Closure|FunValue|Virtual|Interface` 等 call kind 是否已经统一成可扩展的 MIR 表达。
2. 复核 provenance / receiver / dispatch / perform metadata 是否足以支撑后续 monomorphization、summary、devirtualization，而不需要回退到 HIR 语法或 LLVM backend 现场补猜。
3. 检查 monomorph lowering、MIR fixture 与相关调用点是否真的消费这些结构化入口。
4. 若发现既有缺口，优先修复缺口；若确认边界成立，则更新 `TODO.md` / `PLAN.md` / 本文件并提交。

### 预定验证

- 定向阅读：
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/monomorph/lower.rs`
  - 相关 fixture 与 HIR/LLVM 消费侧
- 计划运行：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo test -p scoopc monomorph::lower -- --nocapture`
  - 如有必要，补 `cargo test --all`
  - 完成前执行 `cargo clippy --all-targets -- -D warnings`

## 执行结果

- 已完成定向代码复核，确认 `Perform` / `Resume` / `Direct|Closure|FunValue|Virtual|Interface` 已统一进入 MIR 调用 / control-transfer 表达层，且 `TopLevelRef`、`MemberAccessMetadata`、`DispatchMetadata`、`ResumeMetadata`、`PerformMetadata`、`PerformArg`、`PerformResult` 已承载后续阶段所需的最小语言级事实。
- review 过程中新增了 monomorph 回归测试 `monomorph_preserves_perform_metadata_and_arg_order_in_instantiated_body`，用于确认 generic 函数实例化后的 MIR 仍保留 `Perform` terminator、payload canonicalization 顺序、`source_arg_index` 与 `PerformResult` provenance。
- 新回归测试首次暴露了一个既有真实缺口：
  - `crates/scoopc/src/mir/lower.rs` 中 `return` / `val` / `assign` 这些 statement wrapper 会在子表达式 lowering 已通过 `Perform` 等 terminator 结束当前块后，仍继续覆盖 terminator 或追加语句；
  - 该问题会破坏 return-position / initializer-position `Perform` 的 CFG 形状，因此已在本轮立即修复，而不是延后到后续任务。
- 已完成修复：
  - 在 `return` / `val` / `assign` lowering 包装层中统一检测 `current_is_terminated()`，若子表达式已经终结当前块则立即停止。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc monomorph::lower -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上全部通过。
- 当前结论：
  - `T5000d3R` 可标记完成；
  - 未发现需要插入到 `T5000dR` 之前的新 prerequisite 任务；
  - 本轮下一步只剩更新任务文档、提交并停止。

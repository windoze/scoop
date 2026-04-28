# 执行计划

## 约束说明

- 本次只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在推进计划任务前，先检查最新提交是否提到需要优先修复的既有问题；若有，则先修复该问题。
- 任何在排查、测试、实现过程中发现的既有 bug、回归、规范不一致、未完成边界，均视为当前范围内问题，必须先修复，或在 `TODO.md` 中前置为依赖任务后停止。
- 不接受规避方案、夹层兼容、仅针对夹具的特殊处理，必须按规范正确实现。
- 需要同步维护 `TODO.md`、`PLAN.md`，并在关键步骤完成后更新本文件。
- 最终需要提交 git commit；如果任务被阻塞，则提交任务重排与计划更新；如果任务完成，则提交实现与测试结果。

## 初始步骤计划

1. 查看最新一次 git 提交，确认是否提到任何已知问题、临时修复、后续待修项，若有则优先处理。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 与相关上下文，确认该任务是否已经有细化方案、依赖说明或已知风险。
4. 评估该任务规模：
   - 如果任务可直接完成：开始实现。
   - 如果任务过大或存在明确前置依赖：先细化为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并执行第一个子任务，或在被阻塞时提交依赖重排后停止。
5. 实现当前目标任务，过程中检查相关模块、测试、规范和既有实现边界。
6. 运行与该任务相关的测试；若修改影响面较大，还需运行更广范围验证，并根据结果修复问题。
7. 运行格式化/静态检查/告警检查，至少确保与本次改动相关部分无警告；若仓库要求可行，则执行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
8. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成情况或阻塞原因。
9. 使用清晰提交信息创建 git commit。
10. 停止，不进入下一个任务。

## 当前任务识别

- 最新提交：`[T5000j2] Expand production MIR when pattern coverage`
- 最新提交未在 commit message 中显式声明需要优先修复的既有缺陷；但仍需在 review 与测试过程中继续留意是否有遗留问题。
- `TODO.md` 中首个未完成任务为：`T5000j2R Review：确认 when / pattern 覆盖扩张仍沿 MIR 结构主线推进`。

## T5000j2R 具体执行计划

1. 阅读 `TODO.md` / `PLAN.md` 中 `T5000j2` 与 `T5000j2R` 的上下文，明确 review 验收点。
2. 检查最新提交涉及的核心文件：
   - `crates/scoopc/src/llvm/codegen/mir_body.rs`
   - `crates/scoopc/src/llvm/emit.rs`
   - `crates/scoopc/src/llvm/reachability.rs`
   - `crates/scoopc/src/llvm/tests.rs`
   - 必要时补看相关 MIR 定义与 lowering 路径
3. 重点核对以下问题：
   - production MIR body 是否直接消费既有 `PatternMatch` / `PatternExtract` / provenance 等 MIR 结构；
   - reachability / summary / body emission 是否仍通过 canonical materialized MIR body / pass view 推进，而不是新增 backend 特判；
   - 是否有新的 pattern 语义判断、目标推断或覆盖逻辑被塞回 LLVM codegen；
   - 新增 fallback 是否只是保守边界，而不是重新把主路径切回 HIR 解释。
4. 运行与本任务最相关的测试与质量检查；如果 review 暴露问题，立即修复后再重新验证。
5. 若 review 通过：
   - 更新 `TODO.md` 将 `T5000j2R` 标记完成；
   - 更新 `PLAN.md` 记录复核结论与验证命令；
   - 更新本文件记录完成状态；
   - 提交 git commit 并停止。
6. 若 review 暴露阻塞性既有问题：
   - 先修复；若无法在本轮直接修复，则按要求在 `TODO.md` / `PLAN.md` 中前置依赖任务，提交后停止。

## 进行中状态

- 当前状态：`T5000j2R` 已完成。
- 复核结论摘要：
  - `when` / pattern 继续以 `Pattern` / `PatternMatch` / `PatternExtract` 作为既有 MIR 结构主线，未新增 backend 专用表示。
  - production LLVM 侧只是在 `mir_body.rs` 中直接 lower 这些既有 MIR 节点，并在 `emit.rs` / `reachability.rs` 中把 raw non-generic pattern body 纳入 canonical materialized body 选择；不支持形状仍保守回退到 HIR-compatible 边界。
  - 未发现需要前插到 `T5000j3` 之前的新缺陷任务。
- 已完成验证：
  - `cargo test -p scoopc production_codegen_ -- --nocapture`
  - `cargo test -p scoopc compare_to -- --nocapture`
  - `cargo test -p scoopc operator_overload -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1202)`）
- 下一步：更新 git 状态，提交本轮 review 结果，然后停止。

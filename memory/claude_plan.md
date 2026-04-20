# 执行计划记录

## 说明

用户要求在执行任何代码或命令前，先把计划写入本文件。由于此时尚未检查仓库状态、最新提交、`TODO.md` 与 `PLAN.md`，下面先记录一版初始执行计划。后续在读取仓库信息后，我会把实际发现、任务拆分、阻塞原因、已完成步骤和计划调整继续追加到本文件中。

出于信息安全与协作可读性考虑，本文件记录的是可审计的执行计划、依据、决策和进展，不写逐字逐句的内部推理草稿。

## 初始执行计划

1. 检查最新一次 Git 提交，确认提交说明中是否提到已知问题、未完成修复或需要优先处理的遗留事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认当前任务是否已有分解、依赖或顺序约束。
4. 如果第一个未完成任务过大或存在隐含前置依赖：
   - 将任务拆分为更小的可执行子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把子任务放到正确的依赖顺序中；
   - 本次只执行拆分后排在最前的那个子任务。
5. 在实现前检查是否存在会阻塞该任务的规范不匹配、缺失语言特性、运行时缺陷或测试基础设施问题。
6. 如果发现阻塞项：
   - 不做规避性实现；
   - 在 `TODO.md` 中加入前置修复任务并调整顺序；
   - 在 `PLAN.md` 与本文件记录阻塞原因；
   - 提交变更后停止。
7. 如果不存在阻塞项：
   - 实现当前首个未完成任务；
   - 补充或调整测试；
   - 运行相关验证，包括必要时的 `cargo test`、目标测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；
   - 修复验证阶段发现的问题。
8. 完成后更新文档：
   - 在 `TODO.md` 中把当前任务标记完成；
   - 更新 `PLAN.md` 反映当前状态；
   - 在本文件记录已完成步骤与验证结果。
9. 检查工作区差异，确保没有误改或未解释的变更。
10. 提交本次变更，提交信息应清晰描述当前完成的任务，然后停止，不继续处理下一个任务。

## 进展状态

- 当前状态：已完成首轮仓库读取，得到以下结论：
  - 最新提交为 `32ee6c8b41434c5263dab676069dcf043de5269f`，提交说明是 `[T4016c] Add shared continuation answer-return helper`。
  - 该提交说明本身未额外声明需要先修复的遗留 issue；暂未发现“提交信息里直接点名、必须先于当前任务处理”的 pre-existing issue。
  - `TODO.md` 中前置的 `T4016a1`、`T4016a2`、`T4016b1`、`T4016b2`、`T4016c` 已完成。
  - 当前顺序上第一个可执行的未完成叶子任务是 `T4016b3`：基于统一 answer-return 通道完成 `Continuation.resume(...): Answer` 的 typecheck / lowering 主线接入。
  - `PLAN.md` 也把下一步明确记录为进入 `T4016b3`。

## 当前任务理解

- 现状很可能是：
  - runtime / ABI 已经具备 answer-return helper；
  - continuation binder 的静态类型已经带 answer type；
  - 但 `Continuation.resume(...)` 在 typecheck / HIR / lowering 主线上仍可能被当作 `Unit`、语句式 builtin、或只在特定 fast-path 中返回结果。
- 本任务需要把它收口为真正的表达式：
  - `k.resume(...)` 的静态类型应为 continuation 的 answer type；
  - lowering / codegen 要使用现有 answer-return ABI；
  - 相关 effect / safe-call / tuple payload / nested handle / fresh continuation 行为需要补测试确认。

## 细化执行计划（针对 T4016b3）

1. 盘点 `Continuation.resume` 在 parser / typecheck / HIR / MIR / LLVM / runtime 中的当前实现与特判位置。
2. 确认它目前为何仍未完成 `T4016b3`：是静态类型仍为 `Unit`、HIR 节点缺失返回值建模，还是 lowering 没把 answer 通道接成表达式结果。
3. 若实现范围可控，则直接修改主线：
   - 让 `Continuation.resume(...)` 在类型系统中返回 answer type；
   - 让 expression-position resume 的 lowering 使用统一 answer-return helper；
   - 校正涉及 safe-call、tuple payload、required effects 与 hidden `Raise<RuntimeError>` 的边界。
4. 为关键语义补回归：
   - arm 内 `k.resume(...)` 后继续执行本地代码；
   - `k.resume(...)` 参与表达式求值；
   - nested handle / `finally` / early return；
   - resumed computation 再次 suspend 暴露 fresh continuation。
5. 运行相关测试与质量检查；若发现规范不匹配或新的前置阻塞，则停止当前实现，改写 `TODO.md` / `PLAN.md` / 本文件后提交。

## 本轮续做计划（2026-04-21，第二阶段）

1. 核对当前工作区差异，确认上一阶段实现已经完整落在 `T4016b3` 范围内，尤其检查是否存在仅格式化产生的无意语义修改。
2. 将 `TODO.md` 中的 `T4016b3` 标记为已完成，并把 `PLAN.md` 的“下一步”推进到 `T4016d`。
3. 同步更新 `sysroot/core.scoop` 与 `runtime/c/scoop_runtime.c` 中仍将 `T4016b3` 描述为“待接通”的过时注释，避免文档口径落后于实现。
4. 记录本轮实现摘要、关键回归与验证结果，然后做最小必要回归、自查 diff、提交并停止，不进入 `T4016d`。

## 进展更新（T4016b3 已完成）

- 已检查 `git diff -- crates/scoopc/src/typecheck/assignable.rs`，确认该文件只有 `cargo fmt` 造成的格式化变化，没有语义修改。
- `T4016b3` 的实现已落地：
  - `Continuation.resume(...)` typecheck 现返回 continuation 的 answer type；safe-call `?.resume(...)` 返回 `Option<Answer>`。
  - escape continuation arm 的 tail-resume 过渡路径不再绕开 handle 结果类型与 answer-hole 回填。
  - LLVM lowering 已为 fresh-path / replay-path 的 `Continuation.resume(...): Answer` 接通共享 answer-return helper，并在需要时解码 answer transport。
  - statement-mode safe-call `?.resume(...)` 已修复 builtin 识别顺序，避免被普通 safe member access 提前吞掉。
- 已补充回归：
  - `tests/fixtures/typecheck/continuation_resume_answer_expression_ok.scoop`
  - `tests/fixtures/run-pass/continuation_resume_answer_expression_basic.scoop`
  - `tests/fixtures/run-pass/continuation_resume_answer_replay_basic.scoop`
- 已完成验证：
  - `cargo test -p scoopc continuation_resume`
  - `cargo run -p scoop -- test --fixtures /tmp/scoop-fixtures`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt`
- 已完成收尾文档同步：
  - `TODO.md` 已将 `T4016b3` 标记为 `[DONE]`；
  - `PLAN.md` 已把下一步推进到 `T4016d`；
  - `sysroot/core.scoop` / `runtime/c/scoop_runtime.c` 已把“`T4016b3` 待接通”的过时注释更新为“已完成主线接入，`T4016d` 继续收口 task 叙事”。
- 已完成最终自查：
  - `git diff --check` 通过；
  - 最小回归再次通过：`cargo test -p scoopc continuation_resume`、`cargo run -p scoop -- test --fixtures /tmp/scoop-fixtures`。
- 当前状态：本轮只剩下 `git add` / `git commit`，提交后即停止，不进入 `T4016d`。

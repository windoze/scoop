## 当前目标

本轮只处理 `TODO.md` 中第一个未完成任务；但在开始任务前，先检查最新提交是否提到任何既有问题，并按要求优先修复。若执行过程中发现任何已存在的缺陷、规格不匹配、实现边界缺口或测试/运行回归，也必须先处理，或将其作为前置任务插入 `TODO.md` 后停止。

## 初始执行计划

1. 检查仓库当前状态，确认是否存在用户未提交改动，避免误覆盖。
2. 查看最新一次提交的提交信息与改动，判断是否明确提到需要优先处理的既有问题。
3. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并核对当前计划是否需要细化。
4. 如果该任务过大或存在前置依赖缺口：
   - 细化任务为更小子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的依赖顺序；
   - 本轮只执行细化后的第一个子任务，或在前置问题阻塞时仅提交计划调整。
5. 实施目标任务所需改动，过程中持续检查是否暴露出既有缺陷。
6. 运行与改动相关的测试；若任务涉及广泛编译/质量约束，则补充运行格式化、测试、`clippy` 等必要验证。
7. 更新文档与任务状态：
   - 在 `TODO.md` 标记本轮任务完成，或在阻塞时重排任务；
   - 在 `PLAN.md` 反映最新状态；
   - 在本文件记录关键进展与计划变更。
8. 检查 `git diff`，确认改动范围正确后提交一次清晰的 Git commit，然后停止。

## 进度记录

- 已创建本计划文件。
- 已检查工作区状态：当前存在我新增的 `memory/claude_plan.md` 修改，以及一个未跟踪文件 `OPTIMIZATION.md`；后续改动需避免干扰这些既有/用户态内容。
- 已查看最新提交 `389a5aba25c8340eea9997cd95c833f8a3bd543c`（`[T4017e2] Route continuation replay tokens through explicit outcome`）；提交信息未额外点名需要先修复的既有问题。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T4017e3`：将 ordinary indirect callee 的 `callee_suspend_state` 迁入显式 frame / resume-token metadata，并去掉 TLS resume 入口。

## 当前细化计划（T4017e3）

1. 全局搜索 `callee_suspend_state`、`scoop_callee_suspend_state_get`、ordinary indirect callee resume 相关路径，定位仍依赖 TLS 的生产代码与测试。
2. 结合 `T4017d/e1/e2` 最近改动，确认当前 state-machine / runtime / ABI 中已有的显式 `EffectOutcome`、resume token、frame metadata 能否直接承载 ordinary callee resume 状态。
3. 已确定实现路径，无需先拆前置任务：
   - 保留 ordinary callee fresh path 的 state save，但把该 state 通过显式 `EffectOutcome.signal.resume_token` / frame metadata 传播，而不是作为 resume 入口长期留在 TLS。
   - 为 ordinary top-level fun / closure 生成显式 callee-resume thunk；replay 时直接调用 thunk，不再靠 ordinary 函数入口读取 TLS 判定 fresh/resume。
   - 调整 state-machine frame layout，为 ordinary call-like suspend site 持有显式 callee resume token；suspend/replay 都围绕该 token 运作。
   - 清理生产路径里对 `scoop_callee_suspend_state_get()/clear()` 的入口依赖；TLS 若仍保留，只允许承担 fresh call -> immediate caller boundary 的短暂 transport/scratch。
4. 若实现中发现新的既有缺口或前置依赖未完成，则按要求把该缺口转成 `TODO.md` 前置任务，更新 `PLAN.md` 和本文件后停止。
5. 完成实现后，运行定向测试，再跑全量 `cargo test --all`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`（如时间/变更面允许），最后更新 `TODO.md` / `PLAN.md` 并提交。

## 当前实现决策

- 不修改所有 ordinary function 的公开/内部调用 ABI，也不把 vtable / itable / object init 一并提前收进本轮；这些仍留在后续 `T4017f`。
- 通过“显式 resume thunk + 显式 resume token”消除 ordinary callee 入口对 TLS 的依赖，范围控制在 `T4017e3` 约定的 top-level fun / closure / relevant call-like boundary。

## 约束与执行原则

- 不通过缩小规格、替换表示方式、弱化测试、局部特殊分支或其他变通方式推进任务。
- 若发现现有实现与规格不符，必须先修复，或显式加入 `TODO.md` 作为前置任务。
- 不回退或覆盖非本人改动；若发现冲突性脏改动，先评估并在必要时停止询问。
- 本轮目标是“一次只完成一个任务并提交”，不继续处理后续任务。
# 2026-04-24 当前轮执行计划补充

## 当前任务

- 继续完成 `TODO.md` 中首个未完成任务 `T4017e3`。
- 本轮目标仅限于完成 `T4017e3`、验证、更新计划文件、提交，然后停止。

## 已知前置检查

- 已检查最新提交 `389a5aba25c8340eea9997cd95c833f8a3bd543c`，提交信息为 `[T4017e2] Route continuation replay tokens through explicit outcome`。
- 该提交未在提交信息中指出必须先修复的既有问题。
- 当前工作区存在未跟踪文件 `OPTIMIZATION.md`，不属于本轮改动范围，不得改动或回退。

## 已有实现状态摘要

- `T4017e3` 的核心实现已经落地，采用“显式 ordinary-callee resume token + 显式 resume thunk”的方案。
- top-level ordinary function 与 closure 已拆分 fresh path 和独立 callee resume thunk。
- callee suspend state 已新增 resume thunk 指针字段，并通过 `EffectOutcome.signal.resume_token` 显式传播。
- state machine frame 已为 ordinary call-like suspend site 分配显式 `callee_resume_token` 槽位。
- runtime 中 TLS `__scoop_callee_suspend_state` 已降级为 propagation 边界内的 scratch transport，不再承担恢复入口职责。
- 相关局部测试已通过，剩余问题集中在 3 个 LLVM 单测断言尚未同步新 IR 结构。

## 外显执行计划

1. 先定位并修正剩余 3 个失败 LLVM 单测的断言，使其匹配新的 IR 结构。
2. 运行对应单测确认修正有效。
3. 运行 `cargo test --all` 检查是否还存在真正的既有实现问题。
4. 若全量测试通过，再运行：
   - `cargo run -p scoop -- test`
   - `cargo clippy --all-targets -- -D warnings`
5. 若验证全部通过：
   - 更新 `TODO.md`，将 `T4017e3` 标记完成；
   - 更新 `PLAN.md`，推进到后续任务；
   - 更新本文件，记录完成状态和验证结果。
6. 检查工作区改动，确认未误触 `OPTIMIZATION.md`。
7. 提交一次 git commit，提交信息预期为 `[T4017e3] Route ordinary callee resume through explicit frame tokens`，随后停止。

## 已知风险与处理原则

- 如果在全量测试、fixture 测试或 clippy 中暴露新的既有 bug、回归、spec mismatch 或未完成实现边界，则必须先修复该问题，再决定是否能完成 `T4017e3`。
- 不允许通过缩小断言覆盖面、改窄 fixture 形状、引入特判工作绕过实现缺陷。
- 如果发现必须先完成新的前置任务，需按要求更新 `TODO.md` / `PLAN.md` / 本文件，并在提交后停止。

## 当前进展更新

- 已修正剩余 3 个 LLVM 单测断言，使其匹配新的显式 ordinary-callee resume token / resume thunk IR。
- `cargo test --all` 已通过。
- `cargo run -p scoop -- test` 发现真实回归：`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop` 的 stdout 与 golden 不一致。
- 当前计划已切换为：先复现并定位该 fixture 回归，修复后重新运行 fixture 套件与 `clippy`，然后再更新 `TODO.md` / `PLAN.md` 并提交。

## 2026-04-24 收尾更新

- 上述 fixture 回归已在后续实现中修复；`T4017e3` 的代码与测试状态已经收口完成。
- 已确认本轮实现验证结果：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt`
- 已按 `PROMPT.md` 收尾要求更新任务文档：
  - `TODO.md` 已将 `T4017e3` 标记为 `[DONE]`，并同步将父条目 `T4017e` 收口为 `[DONE]`；
  - `PLAN.md` 已将主线推进为 `T4017f -> T4017R -> T4012b3 -> T4012c -> T4012R -> T4013 -> T4013R`。
- 本轮提交只包含 `T4017e3` 相关实现、计划文档与本进度记录；保留未跟踪文件 `OPTIMIZATION.md` 不纳入提交。

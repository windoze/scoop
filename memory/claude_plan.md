# 执行计划

## 说明

根据当前指令，我需要在执行任何仓库命令前先写下计划。由于此时尚未读取仓库当前状态、最新提交、`TODO.md`、`PLAN.md` 和工作区差异，下面先记录一个初始执行方案。后续一旦完成仓库检查，我会把更精确的步骤、风险和进度补充到本文件中。

我不会在这里记录不可验证的冗长内心独白，而是记录可执行、可审计的推理摘要、判断依据和后续动作。

## 初始目标

本次调用只完成一项工作：

1. 检查最新提交是否提到遗留问题；若提到，则先修复这些问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，则拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的第一项任务。
5. 运行相关测试、格式化和必要的 lint，确保没有新增问题。
6. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况。
7. 提交一个清晰的 Git commit。
8. 停止，不继续处理下一个任务。

## 初始步骤

1. 检查工作区状态，确认是否存在用户未提交改动，避免误覆盖。
2. 查看最新提交信息，判断是否明确提到了需要先处理的既有问题。
3. 打开 `TODO.md` 与 `PLAN.md`，确定当前优先级最高的未完成任务。
4. 阅读与该任务直接相关的代码、测试和规范文件，判断任务边界。
5. 若发现任务依赖未满足的语言特性、规范缺口或现存 bug：
   - 先把缺口整理成前置任务写入 `TODO.md`。
   - 在 `PLAN.md` 和本文件中说明阻塞关系。
   - 提交变更后停止。
6. 若任务可以直接完成：
   - 实施代码修改。
   - 增加或调整测试。
   - 运行 `cargo fmt`、相关测试，以及必要时运行 `cargo clippy --all-targets -- -D warnings`。
7. 完成后在 `TODO.md` 勾选对应任务，并更新 `PLAN.md` 与本文件。
8. 使用与任务编号对应的提交信息提交。

## 当前已知约束

- 必须一次只完成 `TODO.md` 中的一个任务。
- 不能以规避方案、临时兼容层或仅测试夹具通过的方式冒充完成。
- 如果发现规范与实现不匹配，必须先把缺失能力转成显式任务，再继续推进。
- 不得回退用户已有改动。
- 需要尽量保证编译、测试、lint 无警告。

## 待确认事项

- `T4016R` review 是否会暴露新的规范/实现不一致，从而需要前置新的修复任务。
- continuation answer type、one-shot 约束、`-> resume` 移除与 `Task` 去 hack 是否在生产代码中真正闭环。
- 是否存在只靠测试命名“看起来正确”，但实现仍残留双轨语义的情况。

## 进度

- 已完成：创建初始计划文件。
- 已完成：检查工作区状态。当前仅有本文件修改。
- 已完成：检查最新提交 `e3cb647907ebc014555260e3358b1f34fadcfe3a`（`[T1510c1] Fix extern native GC root reloads`）。从提交标题和摘要看，它是在修复已知的 `@Extern` + moving-GC native-roots 问题；目前未看到该提交又显式引入“尚未修复、必须先处理”的新遗留问题描述。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，确认首个未完成任务为 `T4016R`：review continuation 是否已经成为正确的单次 delimited continuation，且 `Task` 不再依赖 runtime hack。
- 已完成：静态审查 continuation / handler / `Task` 相关规范与实现。当前已确认：
  - parser 只把 `-> resume { ... }` 作为 removed-syntax diagnostic 处理；
  - `Continuation.resume(...)` 的 typecheck / lowering / LLVM codegen 统一走 answer-returning helper；
  - `Task` runtime 已通过共享 `scoop_continuation_resume_with(...)` 把 continuation answer 解释为私有 `__TaskStepResult`，不再直接回读 continuation payload/frame 私有布局。
- 已完成：运行 `cargo run -p scoop_tools -- spec-fixtures check`，结果通过。
- 已完成：运行 `cargo run -p scoop -- test`，结果失败；暴露新的 blocker：`tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop` stdout 与 golden 不一致。
- 已完成：单独复现 blocker：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop -o /tmp/stackmap_registry_statepoint_smoke.out` 成功；
  - 执行 `/tmp/stackmap_registry_statepoint_smoke.out` 输出 `-3`，而 golden 期望 `1`。
- 已完成：定位 blocker 原因：
  - `T1510c1` 已把 extern/native 三连调用改为 leaf lowering，并通过 `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 锁定“这些调用点不再生成 statepoint”；
  - 但 `stackmap_registry_statepoint_smoke.scoop` 仍假定 `@Extern("scoop_test_stackmap_statepoint_smoke")` 调用点会生成真实 statepoint record，因此在 runtime helper 中用 `__builtin_return_address(0)` 查 registry 时返回 `-3`。
- 已完成：把该 blocker 前置为新任务 `T1510c2`，并把 `T4016R` 顺延到它之后；`TODO.md`、`PLAN.md` 已同步更新依赖与顺序。
- 进行中：准备提交本轮“发现 blocker 并重排任务”的变更，然后停止。

## 当前执行方案（已细化）

1. 阅读 `T4016R` 直接涉及的规范和实现：
   - `SCOOP_FULL_SPEC.md`
   - `SCOOP_RUNTIME.md`
   - `sysroot/core.scoop`
   - continuation / handler / task 相关的 parser、typecheck、LLVM、runtime 代码与测试
2. 按 `T4016R` 的验收点逐项核对：
   - 是否仍有用户态 `-> resume` 入口或隐藏 special form
   - `k.resume(...)` 是否真的返回 delimiter answer type，而不是表面类型正确、底层仍走 `Unit`/旁路
   - resumed computation 完成后，调用点本地代码是否继续执行
   - 是否仍残留 multi-shot / replay / clone 语义通道
   - `Task` 是否仍依赖 task-private frame peek、专有 ABI 或其他 runtime hack
3. 运行定向测试与必要的全量测试/lint：
   - 先跑 continuation / task 相关 fixture 或单测
   - 若需要，再跑 `cargo test --all` 和 `cargo clippy --all-targets -- -D warnings`
4. 根据 review 结果收口：
   - 若无问题：勾选 `T4016R`，同步更新 `PLAN.md` 和本文件
   - 若发现问题：把问题前置为新任务，调整 `TODO.md` / `PLAN.md` 顺序，记录阻塞原因，然后提交并停止

## 当前结论

- `T4016R` 目前**不能**标记完成。
- 原因不是 continuation / `Task` 主线再次出现明显语义回退，而是全量回归揭示了另一个必须先清掉的真实 blocker：runtime stackmap statepoint smoke 仍依赖已经被 `T1510c1` 明确移除的 extern/native statepoint 形状。
- 正确动作不是把 golden 改成 `-3`，也不是回退 `T1510c1`，而是新增前置任务：恢复一个与现行 lowering 合同一致的真实 statepoint smoke，并在其完成后重新执行 `T4016R` 收口 review。

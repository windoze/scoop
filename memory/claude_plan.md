# 本轮执行计划

## 任务边界

- 本轮只处理 `TODO.md` 中第一个未完成任务。
- 在进入该任务前，先检查最新提交是否提到已有问题；若发现已有 bug、回归、规格不一致或未完成边界，优先修复或把必要前置任务插入 `TODO.md` 后提交并停止。
- 不采用绕过实现、削弱测试、夹带 fixture 专用逻辑或偏离规格的做法。
- 若任务过大，先把它拆成可执行子任务并更新 `TODO.md` / `PLAN.md`，提交后停止，等待下一轮处理第一个子任务。

## 步骤计划

1. 检查当前 git 工作区状态，识别是否存在用户已有改动，后续避免覆盖或回退。
2. 查看最新提交摘要和变更内容，确认是否提到预先存在的问题、已知失败或临时处理。
3. 读取 `TODO.md`，定位第一个未完成任务；同时读取 `PLAN.md` 获取任务背景和依赖关系。
4. 若最新提交暴露必须先修的问题，优先处理该问题；否则处理第一个未完成任务。
5. 研究相关代码、测试和规格文本，确定正确实现路径，发现规格不匹配时立即更新任务依赖并停止。
6. 实施最小且完整的代码或文档改动，保持与现有架构和风格一致。
7. 运行针对性测试；根据影响面补充运行更广测试，至少覆盖本次改动涉及的 fixture 或 Rust 测试。
8. 若发现编译警告或测试失败，继续修复到通过；若属于更大的前置缺口，则按要求更新 `TODO.md` / `PLAN.md` 并提交停止。
9. 完成后更新 `TODO.md` 标记本任务完成，并同步更新 `PLAN.md` 与本文件的进度。
10. 查看最终 diff，确认没有无关回退或多余改动。
11. 使用清晰任务标签提交所有本轮必要改动，然后停止，不继续下一个任务。

## 当前进度

- 已写入本计划文件。
- 已检查初始工作区状态：当前只有本文件的新增/修改属于本轮改动。
- 已读取最新提交摘要：最新提交为 `[T5000i1P2] Fix monomorph request source filtering`，需要继续查看提交正文和相关记录，确认是否提到尚未修复的既有问题。
- 已开始读取 `TODO.md` / `PLAN.md`；文件较长，下一步继续定位第一个未完成条目。
- 已确认 `TODO.md` 当前第一个未完成计划任务是 `T5000i2 基于 escape facts 接入最小 non-escaping closure simplification`。
- 最新提交相关的 `ISSUES.md` 仍记录未修复 P2 问题；根据本轮规则，这些问题优先于 `T5000i2`。
- 当前前置处理顺序：先处理 `ISSUES.md` 中第一个未修复项“request-root 当前是源文件级，不是 entry-main 可达级”。

## T5000i1P3 实施计划

1. 在 MIR instance collection 选项中加入 request-root 模式：
   - dump / 旧测试路径继续使用 source-file rooted 模式；
   - production build / single-file LLVM frontend 使用 entry-main rooted 模式。
2. 调整 materializer 的 request-root 收集：
   - source-file 模式保持当前“request source 内全部 top-level/member fun 为 roots”的行为；
   - entry-main 模式只把选定 `main`（cone 下为精确 FQN，单文件下为 name=`main`）以及显式 export entry points 作为 roots。
3. 调整 initial `MonomorphRequest` seed：
   - source-file 模式保持按 request source 过滤；
   - entry-main 模式额外要求 call-site 已被 entry 可达扫描触达，避免同源未调用 helper 的泛型请求直接成为 seed。
4. 为 production build 增加回归：
   - 单文件：同一源文件中未从 `main` 触达的 generic helper 不应 materialize；
   - cone：consumer cone 内非入口源文件的未触达 generic helper 不应 materialize，但 request source 集合仍不包含 sysroot/support。
5. 更新 `TODO.md`：
   - 插入 `T5000i1P3` 并在完成后标记 DONE；
   - 同时把 `ISSUES.md` 中剩余两个 P2 作为 `T5000i1P4` / `T5000i1P5` 插到 `T5000i2` 前，保证后续 invocation 不会越过已有问题。
6. 更新 `PLAN.md` / `ISSUES.md` / 本文件，运行针对性测试和必要的全量检查后提交。

## T5000i1P3 当前进度

- 已新增 `MaterializeRequestRootMode` 与 materializer options：
  - 默认 source-file rooted 模式保留给 dump / 旧测试路径；
  - production build 与 single-file LLVM frontend 已切到 entry-main rooted 模式。
- 已调整 materializer：
  - entry-main 模式只收集选定 `main` / export entry points 作为 request roots；
  - entry-main 模式下 initial `MonomorphRequest` 还必须对应 entry 可达扫描触达过的 call-site。
- 已新增 production build 回归，覆盖单文件同源未触达 helper 与 cone 非入口源未触达 helper 的泛型实例不会被 materialize。
- 针对性测试暴露一个既有阻塞 bug：零参数顶层调用（例如 `entry()`）在 MIR lowering 中被误判为 dispatch receiver 缺失，落成 `Todo("dispatch receiver lowering pending")`，导致 entry-main 可达扫描无法继续进入该函数体。
- 已修复 MIR call classification，使无接收者的顶层函数调用仍 lowering 为 `DirectCall`，并新增 `tests/fixtures/mir/direct_zero_arg_call.{scoop,mir}`。
- 完整 fixture 验证暴露 async task 回归：entry-root 模式不能预先只按初始 source roots 收集 HIR direct-call fallback，否则 HIR synthetic body 中的 `__task_step_ready<T>` 会漏实例。
- 已修复该回归：
  - HIR direct-call fallback 改为按实际扫描到的 reachable MIR function body 消费；
  - MIR direct-call 实例推断增加赋值目标结果类型输入，补齐只从返回类型推断 type 参数的 helper。
- 已完成 T5000i1P3 的代码与测试改动，并更新 `TODO.md` / `PLAN.md` / `ISSUES.md`。
- 已把剩余两个最新提交遗留 P2 加入 `TODO.md`，作为 `T5000i1P4` / `T5000i1P5` 排在 `T5000i2` 前。
- 已通过的验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoop build_frontend_ -- --nocapture`
  - `cargo test -p scoopc mir::materialize -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`
- 下一步：运行更广的 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`，随后检查 diff 并提交。

## T5000i1P3 确认与收尾计划

1. 复核当前未提交 diff，确认改动确实对应 T5000i1P3：entry-main rooted request roots、同源/consumer cone 未触达 helper 过滤、零参数 direct-call 修复、async synthetic helper 回归修复，以及 TODO/PLAN/ISSUES 记录。
2. 运行 `cargo fmt --all --check`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，补齐 `PROMPT.md` 要求的完整验证。
3. 若验证失败，继续修复到通过；若失败暴露更大前置问题，则按 `PROMPT.md` 把它插入 `TODO.md` 并停止。
4. 验证通过后更新本文件进度，最终检查 `git diff --stat` / `git status --short`。
5. 以 `[T5000i1P3] Fix entry-root MIR request roots` 提交本轮改动，然后停止，不继续 `T5000i1P4`。

## T5000i1P3 收尾进度

- 已复核 diff 范围，确认当前改动对应 T5000i1P3：
  - production materialization 改为 entry-main rooted request roots；
  - 保留 dump / 调试路径 source-file rooted 模式；
  - 新增 build frontend 回归覆盖同源与 consumer cone 未触达 generic helper；
  - 修复零参数顶层 direct-call lowering；
  - 修复 async synthetic helper 实例发现回归；
  - `TODO.md` / `PLAN.md` / `ISSUES.md` 已记录 T5000i1P3 完成，并把剩余两个 P2 排为 T5000i1P4 / T5000i1P5。
- 最终验证过程中 `cargo clippy --all-targets -- -D warnings` 暴露两个 `too_many_arguments` warning；已将 MIR materializer 内部 helper 参数收束为 `ReachableRvalueScanContext` 与 `DirectCallInferenceInput`，不改变行为。
- 最终已通过：
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`（`fixtures: ok (10)`）
  - `cargo run -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`
- 下一步：最终检查 `git diff --stat` / `git status --short`，提交 `[T5000i1P3] Fix entry-root MIR request roots` 后停止。

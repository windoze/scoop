# 执行计划

## 约束与工作原则

- 先读取 `TODO.md` 作为索引，再按索引顺序读取对应的 `TODO-Px.md` 详细任务文件。
- 只执行第一个未在详细任务标题中标记 `[DONE]` 的任务，完成后停止。
- 如果发现当前任务被具体实现缺口阻塞，不绕过问题；在对应 `TODO-Px.md` 中插入最小必要前置任务，同步 `TODO.md`，提交后停止。
- 完成任务时同步更新详细 TODO 与索引 TODO 的 `[DONE]` 状态和完成记录。
- 按仓库规范运行相关测试；若可行，补充更高覆盖的验证。
- 最终提交本次所有相关改动，不推进下一个任务。

## 初始执行步骤

1. 检查当前工作区状态，确认是否已有未提交改动需要保留。
2. 读取 `TODO.md`，确认索引列出的阶段文件和任务顺序。
3. 按索引顺序读取相关 `TODO-Px.md` 文件，定位第一个标题未带 `[DONE]` 的详细任务。
4. 阅读该任务的完整要求、依赖、约束、验证要求和完成记录。
5. 根据任务范围检查相关代码、测试和规格文件，识别实现位置和验证路径。
6. 实现当前任务所需的最小正确变更，不引入规避性实现。
7. 添加或更新必要测试、fixture 或文档。
8. 运行任务要求的验证命令和相关回归测试，修复发现的问题。
9. 更新 `TODO-Px.md` 的任务标题与完成记录，并同步 `TODO.md` 索引。
10. 必要时更新本计划文件以记录关键进展或计划调整。
11. 复查 diff，提交所有相关改动，提交信息使用任务编号和简明描述。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-P7.md`。
- 当前首个未完成详细任务：`P7-T02U`（修复默认 run-pass 暴露的 refactor async/task resume payload ABI 阻塞）。
- 最近提交提到 async prerequisite，和本任务直接相关；将作为当前任务上下文处理。
- 下一步：复现 `tests/fixtures/run-pass/async_await_minimal_int_basic.scoop` 的默认 refactor 失败，并定位 resume payload ABI / 注入路径缺口。

## P7-T02U 执行计划

1. 运行任务指定的最小 run-pass/run 命令，记录当前失败输出。
2. 阅读 async/task fixture、sysroot task/async helper，以及 refactor late-lowering / LLVM ABI materialization 中 resume payload 相关实现。
3. 定位 generic `Async.await<T>`、`Task<T>`、surface resume wrapper、boundary result local 之间 payload ABI 和注入路径的断点。
4. 实现最小正确修复：shared surface payload 可 erased，concrete task/continuation/boundary result 必须恢复实际 `T`。
5. 添加或更新定向测试，覆盖默认 refactor run-pass 的最小 async/await happy path。
6. 运行 `P7-T02U` 指定验证；如修复导致更早阶段测试受影响，补跑相关 Rust 单测或 fixture。
7. 更新 `TODO-P7.md` 与 `TODO.md` 的 `[DONE]` 状态和完成记录。
8. 复查 diff 并提交，提交信息使用 `[P7-T02U] ...`。

## P7-T02U 进展

- 已复现失败：默认 `scoop test` fixture 失败，默认 `scoop run` 只输出 `before` 后 exit 1；显式 legacy 可输出完整结果。
- `dump-effect-lowered` 和 emitted LLVM 显示 `main.$lambda1`、`__task_drive_waiting::<Int>` 等 resume entry 中 `resume_plain_dispatch` 的 switch 没有任何 resume state case。
- 直接原因：`resume_payload_binding_accepts_tuple` 只接受 incoming resume tuple 的 codegen 类型与 consumer local 完全一致；async/task transport 路径中 incoming `(Int, Any)` / erased surface payload 与 concrete resumed local 不一致，导致合法 resume binding 被跳过。
- 已实现修复：当 `Continuation.resume` 的 published underlying route 只能落到 self resume-boundary 且 payload 是 task transport `(Int, Any)` 时，refactor LLVM lowering 现在按 continuation object type descriptor 动态选择可接收 task transport 的 owner resume adapter；adapter 用 concrete owner frame/resume binding 恢复真实 resumed local，并把 owner `Step` 通过当前 boundary dispatch plan 消费。
- 已实现修复：resume payload 注入现在可将 `(Int, Any)` transport 解码到 concrete consumer local/home，例如 `Int` await result，而不是要求 ABI 类型完全一致。
- 已新增定向测试：`default_refactor_runs_async_await_task_resume_payload_cli` 覆盖默认 refactor `run async_await_minimal_int_basic.scoop` 输出完整 async/await happy path。
- 已验证通过：`cargo run -p scoop -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`；`cargo test -p scoop --test p7_default_pipeline`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`；`cargo check --all`；`cargo clippy --all-targets -- -D warnings`。
- 下一步：更新 `TODO-P7.md` / `TODO.md` 完成记录，然后复查并提交。

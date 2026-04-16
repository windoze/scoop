# 执行计划

说明：按安全与协作要求，这里记录的是可审阅的简明推理摘要与执行计划，不写出内部完整思维链。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；在开始实现前，先检查最新提交里是否提到已有问题，若有则先修复。

## 执行步骤

1. 检查最新一次 Git 提交信息，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`、`PLAN.md`、必要时阅读 `README.md` 和相关模块，定位第一个未完成任务及其上下文。
3. 判断该任务是否过大：
   - 若可直接完成，进入实现。
   - 若过大或被前置缺陷阻塞，先在 `PLAN.md` / `TODO.md` 中拆分、重排或补充前置任务，然后本轮只处理新的第一个可执行子任务。
4. 实现任务所需改动，过程中同步更新本文件，记录关键决策、发现的问题和当前进度。
5. 运行与改动相关的验证：
   - 优先运行最小充分测试；
   - 如任务影响范围较大，再补充运行 `cargo test`、`cargo clippy --all-targets -- -D warnings`、或其他必要命令。
6. 若发现规格不匹配、缺失特性或历史缺陷，不能绕过：
   - 在 `TODO.md` 中新增/重排前置任务；
   - 在 `PLAN.md` 和本文件中记录阻塞原因；
   - 如本轮因此无法完成原任务，则提交这些计划性调整并停止。
7. 完成后更新 `TODO.md` 与 `PLAN.md`，将本轮任务标记为已完成，并总结验证结果。
8. 使用清晰的 Git 提交信息提交本轮全部改动，然后停止，不继续做下一个任务。

## 进度记录

- 已创建本计划文件，下一步开始检查最新提交与待办列表。
- 已检查最新提交 `6d86e4f48a7c46c842de721af9c31e856dfad5ad`，提交说明未提到额外待修的历史问题。
- 已定位本轮首个未完成任务为 `T3009aR`：复审 immediate-resume lowering 是否仍会回落到 generic call/member-access。
- 本轮执行细化：
  1. 审查 `state_machine_emitter.rs` 中 immediate-resume arm 的重写入口与 `ArmResumeMatchedSite` payload 写回路径。
  2. 检索生产代码中与 `resume`、generic call、member access、placeholder local 相关的残留入口。
  3. 若发现回落或合同不一致，直接修复并补测；若未发现，则完成复审记录并运行针对性验证。
  4. 更新 `TODO.md`、`PLAN.md`、本文件，提交本轮改动并停止。
- 复审中定位到一个真实生产问题：`rewrite_immediate_resume_arm_body` 只接受 `Block`，但 `await task` 的内部 lowering 会生成非 block 的 direct `resume(join(...))` arm，当前会报 `unsupported_main_body: immediate resume arm body`。
- 已修复：将 immediate-resume dedicated rewrite 从“block arm body”收口为“顶层尾值表达式”，使 source block arm 与 synthesized expression arm 共用同一条路径。
- 已新增 emitter 单测，锁定 non-block immediate-resume arm body 也能被改写。
- 已验证：
  - `cargo test -p scoopc immediate_resume_arm_body -- --nocapture`
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop -o /tmp/t3009ar_yield_basic`
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/async_await_minimal_int_basic.scoop -o /tmp/t3009ar_async_await`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 额外发现：`/tmp/t3009ar_async_await` 运行时会在打印 `before` 后异常退出。该问题属于 structured concurrency / async 后续缺口，已在计划文档中记录，不在本轮 `T3009aR` 的 immediate-resume lowering 复审范围内。

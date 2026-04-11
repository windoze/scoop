# 本轮执行计划

## 说明

用户要求在执行命令前先写入“完整思考过程”。我不会记录不可公开的内部推理细节，但会提供足够详细、可审计的执行计划、假设、决策依据摘要与进度记录，供后续检查。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；若该任务过大，则先拆分任务并更新 `PLAN.md` 与 `TODO.md`，本轮只完成拆分后的第一个子任务。完成后测试、更新文档、提交 Git，然后停止。

## 执行步骤

1. 查看最新一次 Git 提交的信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关上下文文件，以及与该任务直接相关的源码与测试，判断任务边界与依赖。
4. 若任务过大或存在前置缺失：
   - 拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 中的任务顺序与描述；
   - 本轮只执行新的第一个子任务，或在无法继续实现时仅提交重排结果。
5. 实现本轮任务，必要时补充或整理代码结构与注释，但避免无关重构。
6. 运行相关验证：
   - 先运行与改动直接相关的测试；
   - 视影响范围运行 `cargo test --all`；
   - 运行 `cargo clippy --all-targets -- -D warnings`；
   - 如有需要运行格式化或其他项目规定检查。
7. 更新进度文件：
   - 在 `TODO.md` 中标记任务完成，或在阻塞时按要求重排；
   - 在 `PLAN.md` 中记录当前状态与后续计划；
   - 持续更新本文件，记录关键进展与计划变化。
8. 检查工作区改动，确认仅包含本轮相关更改。
9. 使用清晰提交信息提交本轮结果。
10. 停止，不继续处理下一个任务。

## 预设决策原则

- 优先修复最新提交明确提到的遗留问题。
- 不回退用户已有改动。
- 若缺少语言特性或基础能力，不强行实现上层任务，而是调整任务依赖并记录原因。
- 所有结论以实际代码、测试结果和仓库文档为准。

## 进度记录

- 已创建本计划文件，尚未开始仓库检查。
- 已检查最新提交 `40f7614`，提交标题未额外声明需要先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务是 `T2003c0b`：LLVM mixed-arm `handle` 接入 sibling escape-continuation。
- 已审阅 `crates/scoopc/src/llvm/codegen/effect.rs` 中的 mixed-arm immediate-resume lowering 与单 arm escape-continuation lowering。
- 当前判断：`T2003c0b` 原始范围过大，至少同时跨越：
  - mixed-arm dispatch 入口扩展；
  - sibling escape handler frame 的 TLS 生命周期管理；
  - direct / indirect perform 两条 escape 路径；
  - continuation captured handler stack 与 sibling self-capture 语义。
- 决策：先把 `T2003c0b` 拆成更小子任务。本轮目标改为实现第一个子任务：
  - 最小可运行子集：`immediate-resume + 单个 sibling escape-continuation arm`
  - 限定：先只覆盖 direct perform、单 perform 点、top-level 语句位置；
  - 对 indirect perform、多 perform、以及更复杂 mixed 组合给出稳定诊断。
- 已完成任务拆分并更新 `TODO.md` / `PLAN.md`：原 `T2003c0b` 现拆为 `T2003c0b1` / `T2003c0b2`。
- 已实现 `T2003c0b1` 的 LLVM lowering 子集：
  - mixed-arm 入口现在支持单个 sibling escape-continuation arm，并对“escape + sibling non-resuming”继续稳定报错；
  - 当前支持范围为：immediate site 与 escape site 都是 top-level `val = perform`；
  - continuation step 会恢复 pre-escape captures，并在 `resume(...)` 后继续执行 escape site 之后的 top-level tail。
- 已新增 / 调整 fixtures：
  - run-pass：`tests/fixtures/run-pass/effect_resume_mixed_escape_direct.scoop`
  - build-fail：`tests/fixtures/build/effect_resume_mixed_escape_is_error.scoop`（改为 multiple direct perform points 诊断）
- 已完成验证：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 当前待做收尾：
  - 复查工作区改动；
  - 提交 `T2003c0b1`；
  - 停止，等待下一轮处理 `T2003c0b2`。

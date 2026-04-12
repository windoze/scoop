# 当前执行计划

## 说明

用户要求先把思路与执行计划写入本文件，再开始任何 shell 命令或代码执行。本文件因此先记录一份初始计划；在读取仓库实际状态后，如果发现任务分解、依赖关系、阻塞项或实现路径需要调整，我会持续更新本文件。

这里记录的是可审计的分析摘要与执行步骤，不是逐字的内部隐式推理。

## 初始目标

本轮只完成一件事：处理 `TODO.md` 中第一个未完成任务，并在完成后停止。

在开始该任务前，还必须先检查最新提交中是否提到现存问题；如果有，这些问题都属于当前范围，必须先修复。

## 约束与成功条件

1. 先检查最新提交说明里是否提到遗留问题；若有，先修复并验证。
2. 读取 `TODO.md`，找出第一个未完成任务。
3. 如果该任务过大，拆成更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
4. 实现必须符合规范，不允许临时兼容、测试特判、绕过实现缺口。
5. 若遇到规范缺口、语言特性缺失、运行时/编译器 bug、诊断错误等阻塞：
   - 不能硬绕过去；
   - 必须先把阻塞问题写入 `TODO.md`，排到依赖它的任务之前；
   - 更新 `PLAN.md` 与本文件说明阻塞原因；
   - 提交后停止。
6. 完成任务后必须：
   - 更新 `TODO.md`；
   - 更新 `PLAN.md`；
   - 运行相关测试；
   - 运行高质量检查，至少包括 `cargo clippy --all-targets -- -D warnings`（如果适用）；
   - 提交 git commit；
   - 停止，不继续下一个任务。

## 计划步骤

1. 检查最近一次 git commit 信息，确认是否提到需要先修复的已知问题。
2. 读取 `TODO.md`、`PLAN.md`，确定当前最高优先级未完成任务及上下文。
3. 如有必要，补充读取相关规范/实现文件，判断任务是否可直接落地，还是需要先拆分/前置依赖。
4. 若任务需要拆分：
   - 修改 `TODO.md` 的任务顺序与子任务；
   - 修改 `PLAN.md`；
   - 更新本文件；
   - 然后执行新的首个子任务。
5. 实现任务对应代码修改，优先保持模块边界清晰，避免继续堆大文件。
6. 为变更补充或调整测试，确保覆盖关键行为与回归风险。
7. 运行格式化、测试、lint / clippy 等验证命令；若失败，立即修复直到通过，或识别出真实阻塞并回到依赖调整流程。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况与任何关键决策。
9. 检查工作树，确认只提交本轮相关修改。
10. 创建一次明确的 git commit，然后停止。

## 当前已知未知项

- 尚未读取最新 commit、`TODO.md`、`PLAN.md`，因此具体任务内容未知。
- 尚未确认是否存在 `memory/` 目录之外的文档要求（如 `README.md` 缺失、已有未提交变更、或用户本地改动）。
- 尚未确认第一项任务是否牵涉编译器、运行时、标准库或规范同步。

## 后续更新规则

- 每完成一个关键节点（如：确认首个任务、开始实现、发现阻塞、完成验证、准备提交），都会更新本文件。
- 如果计划改变，会在本文件中记录“变更原因”“新计划”“受影响任务”。

## 进展更新（2026-04-12）

### 已完成的上下文确认

- 已检查最近一次 git commit：`[T2003c0c2b2] Add no-immediate indirect escape regressions`。提交说明本身没有额外声明待修复遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务是 `T2003c0c2b3`：无 immediate-resume 的 richer escape site-matrix。
- 已检查工作区状态：目前只有本文件变更。

### 范围重判定

- 继续审计 `T2003c0c2b3` 后确认，该任务当前范围过大，不适合在一轮里直接整包实现。
- 原因：
  1. 现有 no-immediate multi-arm escape lowering 只支持 top-level direct single-site 与 top-level indirect single-site。
  2. richer matrix 需要同时打通至少四类独立问题：multiple top-level direct、nested direct、indirect site-matrix、direct+indirect mixed。
  3. 仓库里虽然已有 single-arm escape 的多 direct/nested machinery，以及 mixed-arm 的更大 site-matrix machinery，但 no-immediate multi-arm 并未直接接入这些实现；若整包推进，会把多站点 pc 状态机、nested replay、callee suspend replay、同 stmt mixed next/prev 关系一次性耦合。

### 新的执行策略

- 先把 `T2003c0c2b3` 拆成 4 个子任务，并把第一个子任务设为本轮执行目标：
  - `T2003c0c2b3a`：无 immediate-resume 的 multiple top-level direct escape sites。
  - `T2003c0c2b3b`：无 immediate-resume 的 nested direct escape sites。
  - `T2003c0c2b3c`：无 immediate-resume 的 indirect escape site-matrix。
  - `T2003c0c2b3d`：无 immediate-resume 的 direct+indirect mixed site-matrix。

### 当前正在执行

- 当前任务：`T2003c0c2b3a`
- 目标：让“一个 escape-continuation arm + 0..N sibling non-resuming arms”的无-immediate multi-arm handle 支持多个 top-level direct escape sites，并保持 `resume(...)` 后继续命中后续 direct site、body-lift、sibling dispatch / detach / cleanup 语义稳定。

### 已完成实现

- 已放宽 no-immediate escape+non-resuming 的 direct 入口分流：只要没有 indirect site，且所有 escape site 都是 top-level direct，就会进入统一的 direct lowering，而不再只放行 single-site。
- 已把 no-immediate direct continuation state 扩展为带 `pc` 的多站点状态机；`resume(...)` 后 step trampoline 会按 `pc` 恢复当前 direct site 的结果，并继续 replay 后续 top-level tail。
- 已把 top-level body-lift 分析从单 site 扩到多 site，因此第一个 direct site 产生的值可以跨第二个 suspension 保留到最终 tail。
- 已新增 run-pass 回归：
  - `effect_multi_escape_direct_multi`
  - `effect_multi_escape_custom_nonresuming_direct_multi`

### 已完成验证

- 定向运行通过：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_direct_multi.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_multi.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_single_site.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_raise_direct_single_site.scoop`
- 全量验证通过：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`

### 当前收尾步骤

- 把 `T2003c0c2b3a` 在 `TODO.md` / `PLAN.md` 中标记完成。
- 复查工作区 diff。
- 提交本轮改动并停止。

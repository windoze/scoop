# 执行计划

## 说明

按要求先记录可审计的高层推理摘要与执行步骤，不记录原始逐字思维过程。后续如计划调整、发现阻塞、完成关键步骤，都会同步更新本文件。

## 初始理解

本轮目标是：

1. 检查最新提交是否提到任何既有问题；若有，则这些问题优先于 `TODO.md` 中的任务处理。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，则拆解任务，并更新 `PLAN.md` 与 `TODO.md`。
4. 完成当前首个可执行任务的实现、测试、文档更新与提交。
5. 完成一个任务后立即停止。

## 当前任务定位

- 最新提交信息未额外声明需要先修复的既有问题。
- `TODO.md` 中首个未完成任务是 `T2003c0c2b3d3`：
  - 主题：LLVM 多 arm handle dispatch（无 immediate-resume，if branch direct + indirect same-stmt mixed）
  - 当前边界：nested block same-stmt mixed 已支持；if branch same-stmt mixed 仍由 `tests/fixtures/build/effect_multi_escape_direct_indirect_if_is_error.scoop` 锁定为失败。

## 当前实施计划

1. 读取现有 no-immediate mixed lowering、if-branch direct-only / indirect-only 子路径，以及 nested block same-stmt mixed 的实现。
2. 找到当前拒绝 if branch same-stmt mixed 的分流或扫描门禁，并确认需要复用的 replay/helper。
3. 实现 if branch same-stmt mixed lowering，优先复用已有：
   - if branch direct site replay；
   - if branch indirect site replay；
   - same-stmt next/prev mixed route；
   - sibling non-resuming dispatch / cleanup。
4. 将现有 build-fail `effect_multi_escape_direct_indirect_if_is_error` 转为 run-pass 或新增 run-pass 覆盖 if branch mixed。
5. 新增或保留一个 while mixed 边界 fixture，继续锁住 `T2003c0c2b3d4`。
6. 运行格式化、测试、LLVM fixture、clippy。
7. 更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 当前结果

- `T2003c0c2b3d3` 已完成。
- 代码层面：
  - no-immediate mixed lowering 现已支持 if branch direct + indirect same-stmt mixed；
  - initial body / continuation step 已接入 if-branch prefix、same-branch next/prev replay 与 after-if tail；
  - 同时放通了 mixed handle 内单个 if stmt 仅 direct-only / indirect-only 的分流，避免被 top-level mixed 入口误拒绝。
- fixture 层面：
  - 删除 `tests/fixtures/build/effect_multi_escape_direct_indirect_if_is_error.scoop`；
  - 新增 run-pass `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop`；
  - 保留 `tests/fixtures/build/effect_multi_escape_direct_indirect_while_is_error.scoop` 作为 `T2003c0c2b3d4` 边界。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步（不在本轮继续执行）：
  - `T2003c0c2b3d4`：while body direct + indirect same-stmt mixed

## 执行步骤

1. 检查工作区状态，避免误覆盖现有改动。
2. 查看最新一次 git 提交的提交信息与改动内容，确认是否明确提到待修复问题。
3. 读取 `TODO.md` 与 `PLAN.md`，识别首个未完成任务及其上下文。
4. 若存在更基础的阻塞问题或规范偏差，先在 `TODO.md` / `PLAN.md` 中重排任务。
5. 对当前目标任务进行代码实现。
6. 运行相关格式化、lint、单测/集成测试，修复发现的问题。
7. 更新 `TODO.md`、`PLAN.md`、必要文档以及本计划文件。
8. 使用清晰的提交信息提交本轮变更并停止。

## 风险与约束

- 不回退用户已有改动。
- 不以临时绕过方式满足任务；若遇到规范缺口，必须显式建任务并调整依赖。
- 尽量将改动控制在本轮首个任务范围内，但会修复其所暴露的前置缺陷。

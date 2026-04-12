## 当前轮次执行计划（初始）

### 目标
- 先检查最新提交是否提到既有问题；若有，优先修复。
- 读取 `TODO.md`，定位第一个未完成任务。
- 如任务过大，则拆分任务并更新 `PLAN.md` / `TODO.md`。
- 仅完成一个任务（或一个新拆分出的首个子任务），完成后测试、更新文档并提交。

### 执行步骤
1. 查看最新一次 Git 提交信息与改动摘要，确认是否明确提到需要先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，识别首个未完成任务及其上下文。
3. 结合代码现状判断任务是否足够小且可在本轮完整完成。
4. 如需拆分：
   - 在 `PLAN.md` 写明拆分后的子任务与依赖；
   - 在 `TODO.md` 中重排并把首个子任务置于当前待执行位置。
5. 实现当前目标任务。
6. 运行相关测试、格式化与静态检查，至少覆盖与改动直接相关的部分；若范围允许，再执行更完整的仓库检查。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，记录完成情况或阻塞原因。
8. 用清晰的提交信息提交本轮改动，然后停止。

### 约束与原则
- 不依赖临时绕过方案、fixture hack 或与规范不一致的实现。
- 若发现前置缺陷阻塞当前任务，先把缺陷显式加入 `TODO.md` 并调整顺序，再停止本轮。
- 不回滚用户已有改动；若工作区有额外变更，只在必要范围内协同处理。

### 待确认事项
- 最新提交是否包含“先修复”的问题说明。
- 第一个未完成任务是否需要拆分。
- 当前任务所需测试集合与可能涉及的模块边界。

### 进度
- 已创建本计划文件，等待仓库检查结果后补充。
- 已检查最新提交 `1bb45982374477b4b1cbe6643b4193274b071d96`：提交信息未声明额外必须先修复的既有问题。
- 已确认首个未完成任务为 `T2003c0c2b3b2`：`LLVM 多 arm handle dispatch（无 immediate-resume，if branch direct escape sites）`。
- 下一步：阅读 `TODO.md` 中该任务描述、对照 `PLAN.md` 的拆分原因，并检查相邻已完成任务（`T2003c0c2b3b1`）对应的 LLVM lowering，实现是否可直接扩展到 if-branch direct site。
- 已确认任务无需继续拆分：现有 mixed-site-matrix 路径已经提供 `if` 分支 direct helper，缺口主要在 no-immediate direct lowering 的入口分派与 step trampoline 仅接受 block-only path。
- 具体实现方案：
  1. 放宽 no-immediate direct 的路径白名单，允许 `if` branch direct site。
  2. 在初次执行时，按 top-level stmt 分类 direct sites：普通 site 继续走现有 top-level / nested-block 逻辑，`if` stmt 复用现有 `codegen_mixed_escape_matrix_if_stmt_direct_sites` helper 按运行时条件选择 then/else site。
  3. 在 step trampoline 中，同样对后续 top-level stmt 的 direct site 做 `if`-aware 分派，保证 `resume(...)` 后能回到命中的分支 tail，并在 after-if merge 后继续执行。
  4. 新增 if-branch run-pass 回归，并把原 `if` 负例替换为 while 边界负例，继续锁住 `T2003c0c2b3b3` 之前未支持的 while body direct site。
  5. 全量验收通过后，把 `TODO.md` / `PLAN.md` 标记到 `T2003c0c2b3b2` 完成，并准备提交。

### 当前结果
- 已完成代码修改：
  - no-immediate direct lowering 现已接受 paired if-branch direct sites；
  - 初次执行与 `resume(...)` step 都会复用 `if` helper 选择命中的 then/else site；
  - `resume(...)` 后会 replay 命中的 branch tail，再继续 after-if top-level tail。
- 已完成回归更新：
  - 新增 run-pass：`tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_if_multi.scoop`
  - 新增 build-fail：`tests/fixtures/build/effect_multi_escape_direct_while_is_error.scoop`
  - 移除旧边界负例：`tests/fixtures/build/effect_multi_escape_direct_if_is_error.scoop`
- 已完成验收：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`

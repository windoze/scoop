# 当前执行记录（本轮）

## 已知上下文

- 上一轮已经定位到当前应处理的首个未完成任务是 `T4016b4b`。
- 上一轮已经完成一组相关代码修改：
  - 移除了用户态 `Continuation<Resume>` 自动补 answer-hole 的 lowering 兼容路径。
  - 新增了 legacy pure continuation shorthand 已移除的 typecheck 诊断与回归。
  - 修复了构造器参数在 `Cell(None())` 这类场景下对 expected type 的占位与回填推断问题。
  - 已将大量 fixture 与内嵌测试字符串迁移为显式 `Continuation<Resume, Answer>`。
- 已验证通过：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 当前判断

- 编译器侧关于 legacy pure continuation shorthand 泄漏 answer-hole 到 codegen 的问题，已经在当前工作树中被修复并通过相关验证。
- 继续推进 `T4016b4b` 时，暴露出一个新的、真实的前置缺陷：
  - `tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop`
  - 该 fixture 现在可以成功 build，但在 `SCOOP_GC_STRESS=1` 下运行时，于 `workerA_resuming` 之后异常退出。
- 这属于真实 runtime / cross-thread escaped continuation / GC stress 崩溃问题，不允许用 workaround 绕过；按任务规则，必须先把它转成前置任务，再调整任务顺序。

## 已完成复核

- 已复核最新提交 `f83813077d9fae95584846721a6870c88a98d71c`，提交正文未携带额外需要顺手修复的遗留 issue。
- 已重新验证 blocker 现状：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop -o /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out` 成功。
  - `SCOOP_GC_STRESS=1 /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out` 仍在输出 `workerA_resuming` 后以 `exit code -1` 结束。

## 本轮执行计划

1. 检查当前工作树状态，并确认 `TODO.md`、`PLAN.md`、相关 fixture 注释的现状。
2. 将上面的新 blocker 正式写入任务系统：
   - 在 `TODO.md` 中为该 runtime 崩溃新增前置修复任务。
   - 保持 `T4016b4b` 为未完成状态，并将其移动到该前置任务之后，显式体现依赖。
3. 更新 `PLAN.md`，记录：
   - pure shorthand 移除与类型检查修复已完成；
   - 当前阻塞点已变为 cross-thread continuation 在 GC stress 下的 runtime 崩溃；
   - 本轮停止于任务重排，不继续实现 runtime 修复。
4. 视情况更新 `tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 顶部注释，移除已过时的“当前 blocker 是 None() 歧义”的表述。
5. 复查变更，确保文档状态与当前判断一致。
6. 按要求提交当前工作树中的相关改动，并停止，不继续下一任务。

## 当前进度

- 已完成步骤 1：确认最新提交、工作树状态、`TODO.md` / `PLAN.md` / fixture 注释现状。
- 已完成步骤 2：把新 blocker 作为前置任务插入 `TODO.md`，并让原 `T4016b4b` 显式依赖该前置任务。
- 已完成步骤 3：更新 `PLAN.md`，将当前主阻塞改写为 cross-thread escaped continuation 在 GC stress 下的 runtime 崩溃，并把下一步改为先做 `T4016b4b0`。
- 已完成步骤 4：修正 `gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 顶部注释，移除已过时的 `None()` 歧义描述。
- 已完成步骤 5：复查关键段落与 `git diff --check`，确认依赖关系、阻塞描述和补丁整洁性一致。
- 待执行步骤 6：提交并停止。

## 变更原则

- 本轮不继续尝试修复 runtime 崩溃本身，只负责把新暴露出的真实缺陷纳入 `TODO.md` / `PLAN.md` 的优先级体系。
- 不回滚当前工作树中已有改动；只在其基础上完成任务重排与记录。

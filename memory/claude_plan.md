## 当前执行计划（可审阅摘要）

### 目标

本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

### 约束

- 先检查最新提交是否提到任何既有问题；若有，先修复这些问题，再处理任务。
- 不接受规避方案、临时兼容、仅测试夹具层面的修补。
- 如果首个未完成任务过大，需要先拆分，并同步更新 `PLAN.md` 与 `TODO.md`。
- 完成后必须更新文档状态、运行相关测试、提交 Git commit，然后停止。

### 初始步骤

1. 查看最新一次 Git 提交，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划、依赖关系与任务上下文。
4. 如任务过大，先拆分为更小子任务，并更新 `PLAN.md` / `TODO.md`。
5. 实现本轮目标任务。
6. 运行相关测试与必要的质量检查；若发现问题，立即修复。
7. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`。
8. 提交 Git commit，随后停止。

### 进行中记录

- 已创建本文件，后续会在关键节点补充进展与计划调整。
- 已检查最新一次 Git 提交：提交标题为 `[T2003u5c1] Support direct-first while mixed replay`，未在提交说明中额外声明需要优先修复的既有问题。
- 已阅读 `TODO.md` / `PLAN.md`，确认本轮首个未完成任务为 `T2003u5c2`：
  - 目标：补齐无 immediate-resume 的 multiple-escape 在 while body 中 `indirect -> direct` separate-stmt mixed replay，并收口剩余 ordering matrix。
  - 当前判断：任务边界明确，暂不需要继续拆分子任务。
- 已用临时样例 `/tmp/effect_multi_escape_indirect_direct_while_tmp.scoop` 复现当前缺口。
  - LLVM 构建当前报错：
    `handle multi-arm without immediate-resume (only same-body-stmt or direct-before-indirect separate-stmt coexistence in while body supported)`
  - 说明当前缺口确实仍是 `T2003u5c2` 描述的 while mixed ordering 门禁，而不是其它无关回归。
- 下一步：
  1. 追踪 `mixed.rs` 中 no-immediate while mixed 站点分类与 `while_next_site_pc_by_pc` / `while_prev_site_pc_by_pc` 的建立逻辑。
  2. 追踪 `matrix.rs` 中 direct/indirect 当前迭代 tail 与 future-iteration re-entry helper，确认 `indirect -> direct` 是否只缺接线或还缺 replay 逻辑。
  3. 修改代码后补正式 run-pass fixture 与期望输出。
  4. 运行针对性测试，再跑任务要求的全量验收。

### 当前进展（更新）

- 已完成 `T2003u5c2` 实现，核心改动如下：
  1. `mixed.rs`
     - no-immediate top-level mixed 分类已允许 while body 中 one direct + one indirect 的双向 separate-stmt ordering。
     - earliest indirect site 的 mixed-step 恢复已改成“只恢复 lexical scope，再在写入 callee resume payload 后执行一次 call-site init”，避免重复 replay 当前 while 前缀副作用。
     - later direct site 若存在 earlier indirect 前驱，future iteration 现会回到 earlier indirect，而不是错误回到自身。
  2. `matrix.rs`
     - `codegen_mixed_escape_matrix_continue_to_next_while_site_after_indirect` 已支持 separate-stmt `indirect -> direct` continuation。
     - 新增 `codegen_mixed_escape_matrix_while_tail_after_mixed_direct_site`，用于 later direct 完成当前迭代后，在 future iteration re-entry 到 earlier indirect。
     - `codegen_mixed_escape_matrix_while_stmt_mixed_sites` 已允许 earliest site 为 indirect 的 separate-stmt while mixed 初始拦截。
  3. 回归
     - 新增 run-pass fixture：`tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
     - 已补对应 golden：`tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.stdout`

- 已完成的验证：
  - 定向样例 `/tmp/effect_multi_escape_indirect_direct_while_tmp.scoop` LLVM 构建与运行通过。
  - 新 fixture 的 build+stdout diff 通过。
  - `cargo test --all` 通过。
  - `cargo run -p scoop --features llvm -- test` 通过（`fixtures: ok (996)`）。
  - `cargo clippy --workspace --all-targets -- -D warnings` 通过。

- 剩余收尾：
  1. 更新 `TODO.md` / `PLAN.md` 为已完成状态。
  2. 检查 `git diff`，确认没有多余改动。
  3. 提交本轮 commit，然后停止。

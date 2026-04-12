# 执行计划记录

## 说明

按要求，我会在此文件持续记录可审阅的计划、关键决策、执行进度与变更说明。
出于安全与策略限制，这里不写入内部逐字推理或完整思维链；改为提供足够详细的步骤计划、判断依据、阻塞原因与后续动作。

## 初始计划

1. 检查最新一次 Git 提交，确认提交说明或相关改动中是否提到已有问题；如果发现明确的既有问题，优先修复。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，对照现有计划与任务顺序，确认依赖关系和当前阶段。
4. 判断首个未完成任务是否可以在本轮完整落地：
   - 如果可以，直接实现。
   - 如果过大或存在前置依赖缺口，则拆解为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆解后的第一个子任务。
5. 在实现过程中检查是否存在任何规范不匹配、语言特性缺失、运行时/编译器缺口或测试仅靠规避手段才能通过的情况：
   - 若存在，则将该问题显式写入 `TODO.md` 作为前置任务；
   - 调整依赖顺序；
   - 更新 `PLAN.md` 说明阻塞原因；
   - 仅提交这些计划调整并停止。
6. 对本轮目标任务完成实现后，运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 如果改动影响面较大，再补充更广的测试；
   - 按要求检查无警告构建/静态检查，优先包含 `cargo clippy --all-targets -- -D warnings`。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮任务标记完成；
   - 在 `PLAN.md` 中记录完成情况与剩余工作；
   - 如有必要补充 `README.md` 或内联注释。
8. 使用清晰的 Git 提交信息提交本轮变更，然后停止，不继续处理下一个任务。

## 立即执行的第一步

先检查最新提交与仓库任务文件，收集本轮目标任务及可能存在的既有问题。完成检查后，会回到此文件补充更具体的执行计划。

## 上下文检查结果

- 最新提交：`[T2003c0c2b3b2] Support no-immediate if-branch direct escape sites`
- 检查结果：最新提交说明没有额外标出一个必须优先修复的既有问题；当前主线仍沿着 `T2003c0c2b3*` 继续推进。
- `TODO.md` 中第一个未完成任务：
  - `T2003c0c2b3b3 [TODO] Effect：LLVM 多 arm handle dispatch（无 immediate-resume，while body direct escape sites）`
- 当前依赖状态：
  - `T2003c0c2b3b2` 已完成。
  - 后续 `T2003c0c2b3c` 依赖本任务，因此本轮只应完成 `T2003c0c2b3b3`。

## 本轮执行计划（细化）

1. 阅读 `TODO.md` 中 `T2003c0c2b3b3` 的完整描述与验收要求。
2. 检查 LLVM mixed-arm / no-immediate escape 相关实现，重点查看：
   - direct site 扫描与分类逻辑；
   - while body replay / re-entry 状态机；
   - sibling non-resuming handler scope 的摘除与恢复；
   - 现有 nested block / if direct site 的处理是否可复用。
3. 检查现有 fixtures，找出：
   - 已覆盖的无-immediate nested block / if direct escape 正例；
   - 当前用于锁 while 缺口的 build-fail 或稳定诊断用例。
4. 评估任务复杂度：
   - 若 while body direct site 只是扩展现有 nested replay 分派，可直接实现；
   - 若需要再拆任务（例如先 flat while、再 nested while），则更新 `TODO.md` 与 `PLAN.md`，本轮仅执行拆出的第一个子任务。
5. 实现代码修改，保持无 workaround：
   - 不通过放宽门禁但让 LLVM/运行时继续崩溃的方式“假支持”；
   - 若发现真实前置缺口，必须先记入 `TODO.md` 并调整顺序。
6. 新增或调整 fixtures，至少覆盖：
   - 无 immediate-resume + while body direct escape 的正向运行；
   - 如有必要，再补一个明确不支持边界的负例，避免范围漂移。
7. 运行验证：
   - 至少 `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
8. 更新 `TODO.md` / `PLAN.md` / 本文件，记录完成情况与任何新增前置问题。
9. 提交 Git commit，然后停止。

## 实施结果

- 任务未再拆分：现有 no-immediate direct lowering 已具备 while replay helper，问题主要是主入口没有接线，以及 single-site while re-entry 没有复用 intercept path，因此本轮可以完整实现 `T2003c0c2b3b3`。
- 已完成的代码修改要点：
  1. 放宽 no-immediate direct path 对 resume-path 的门禁，允许 while-body direct site 进入 lowering。
  2. 在 direct-site 分类中新增 while-body direct site 分派，不再把它统一拒绝为 unsupported。
  3. 在初次执行与 `resume(...)` continuation step 两条路径上，都接入 while replay / re-entry helper。
  4. 修正 single-site while re-entry 缺口：即使只有一个 escape site，只要存在 while re-entry，也必须走共享 `intercept_bb`，否则首次 `resume(...)` 后会落入错误的无限 replay。
- 新增/调整的 fixtures：
  - 新增 run-pass：`effect_multi_escape_custom_nonresuming_direct_while_multi`
  - 新增 build-fail：`effect_multi_escape_indirect_while_is_error`
  - 删除旧 build-fail：`effect_multi_escape_direct_while_is_error`

## 验证结果

- 定向验证：
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_while_multi.scoop -o /tmp/effect_multi_escape_custom_nonresuming_direct_while_multi.out`
  - 运行生成程序，stdout 与期望一致。
  - `cargo run -p scoop --features llvm -- build tests/fixtures/build/effect_multi_escape_indirect_while_is_error.scoop -o /tmp/effect_multi_escape_indirect_while_is_error.out`
  - 结果按预期失败，并报 `handle multi-arm without immediate-resume (escape site matrix not yet supported)`。
- 全量验证：
  - `cargo test --all`：通过
  - `cargo run -p scoop -- test`：通过（`fixtures: ok (976)`）
  - `cargo run -p scoop --features llvm -- test`：通过（`fixtures: ok (976)`）
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过

## 收尾动作

- 需要将 `TODO.md` 中 `T2003c0c2b3b3` 标记为完成。
- 需要将 `PLAN.md` 当前下一步更新为 `T2003c0c2b3c`。
- 需要提交本轮改动，然后停止。

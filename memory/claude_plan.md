# Claude Plan

## 约束说明

用户要求先把“完整思考过程”写入本文件。出于安全与协作边界，我不会记录逐字逐句的内部推理，但会完整记录可执行的分析结论、假设、步骤、决策依据和进度更新，保证你可以检查我的工作路径。

## 初始执行计划

1. 检查最近一次 git 提交信息与差异，确认是否明确提到现存问题或遗留修复项。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`、`README.md` 及与该任务直接相关的代码与测试，建立上下文。
4. 判断该任务是否能在本轮完整落地。
   - 如果可以：直接实现。
   - 如果过大或存在前置依赖：把任务拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
5. 实现任务，同时保持代码结构清晰；必要时补充模块拆分、注释和 README 更新。
6. 运行相关格式化、检查与测试，至少覆盖：
   - `cargo fmt --check` 或 `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - 与当前任务相关的最小充分测试
   - 若影响范围较大，再补充更广泛测试
7. 更新文档与计划：
   - 在 `TODO.md` 中标记本轮完成项
   - 在 `PLAN.md` 中记录当前状态和后续项
   - 在本文件中记录关键进展与必要的计划调整
8. 提交 git commit，提交信息聚焦本轮任务。
9. 停止，不继续处理下一个任务。

## 初始假设

- 当前仓库可能存在未提交变更；除非它们与本轮任务直接冲突，否则不回滚。
- “最新提交提到的 pre-existing issue” 需要以最近一次提交消息和必要的改动内容为准进行核查。
- 若用户要求与系统安全边界冲突，以安全边界为准，并在本文件中给出可审计的替代记录。

## 进度记录

- 2026-04-12: 已创建计划文件，准备开始检查最新提交与任务列表。
- 2026-04-12: 已检查最新提交 `9a27408c46c08fee6fa3a397fd925fb0e4485882`，提交消息仅为 `[T2003c0b2c1b] Support mixed-arm if-branch escape sites`，未显式提及额外 pre-existing issue，暂未发现需要在本轮任务前先独立处理的遗留修复项。
- 2026-04-12: 已定位 `TODO.md` 中首个未完成任务为 `T2003c0b2c2`：`Effect：LLVM 多 arm handle dispatch（sibling escape-continuation，while body 的 direct site）`。
- 2026-04-12: 阅读 mixed-arm matrix lowering 后确认，原 `T2003c0b2c2` 实际跨了“flat while-body direct site”与“while 内 nested block/if direct site”两个不同复杂度的问题。已按用户流程将其拆为 `T2003c0b2c2a` / `T2003c0b2c2b`，本轮只执行新的首个未完成任务 `T2003c0b2c2a`。
- 2026-04-12: 当前实现计划收口为：1) 扩展 mixed escape direct-site 扫描/used-after/body-decl 分析到 flat while body；2) 在 matrix lowering 的 `state0` / `state1` / step trampoline 中加入 while-body direct-site 的进入、恢复与 loop re-entry helper；3) 新增 run-pass fixture 覆盖 pre/post-immediate 的 while-body flat direct site，并把旧 while 负例改成新的剩余稳定诊断。
- 2026-04-12: 实现已完成。`effect.rs` 已新增 while-body flat direct site 的扫描、capture 分析与 lowering helper；`state0` / `state1` / step trampoline 现已支持 resume 后完成当前迭代尾部并在后续迭代再次命中 sibling escape site。
- 2026-04-12: 已新增 fixtures：`effect_resume_mixed_escape_pre_immediate_while`、`effect_resume_mixed_escape_post_immediate_while`；并把 `effect_resume_mixed_escape_while_is_error` 改为 nested direct 负例，作为后续 `T2003c0b2c2b` 的稳定诊断。
- 2026-04-12: 已完成验证：`cargo fmt --all --check`、`cargo test --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo run -p scoop --features llvm -- test` 均通过。

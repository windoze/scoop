# 当前执行记录

## 约束说明

我不能写出逐字逐句的完整内部推理，但会在这里持续维护一份足够详细的执行计划、决策依据、关键发现与进度记录，便于外部检查。

## 初始执行计划

1. 检查最新一次 git 提交，确认提交说明里是否提到已有已知问题；如果有，先定位并修复这些问题。
2. 阅读 `TODO.md`，找到第一个未完成任务。
3. 评估该任务是否足够小且可以在当前轮次完整完成。
4. 如果任务过大：
   - 拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 在 `TODO.md` 中替换或补充为新的子任务；
   - 仅执行拆分后的第一个子任务。
5. 实现当前要执行的任务。
6. 运行相关格式化、lint、测试，至少覆盖：
   - `cargo fmt --check` 或必要时 `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - 与变更相关的测试，必要时运行更完整的测试集
7. 更新文档与任务状态：
   - 在 `TODO.md` 中将当前任务标记为完成；
   - 在 `PLAN.md` 中反映当前状态和后续顺序；
   - 必要时补充 `README.md` 或内联注释。
8. 检查工作区差异，确认没有遗漏。
9. 提交 git commit，提交信息应清晰描述本轮完成的任务。
10. 停止，不继续处理下一个任务。

## 进度

- 已创建本文件并写入初始计划。
- 已检查最新提交：
  - 最新提交为 `22d9079 [T2003c0b2b3] Support mixed-arm pre-immediate escape sites`。
  - 提交信息正文未列出“已知遗留问题 / follow-up fix”，因此没有需要在任务前先修的显式 pre-existing issue。
- 已读取 `TODO.md` / `PLAN.md`：
  - 第一个未完成任务是 `T2003c0b2c`。
  - 该任务原始范围同时包含 sibling escape-continuation 的 nested direct site、nested indirect call site，以及 block / if / while 三类控制流形状。
- 复杂度评估：
  - 当前 mixed-arm 路径只有 top-level site matrix；nested direct 需要引入类似 single-arm escape continuation 的 resume-path / nested intercept 机制。
  - nested indirect 目前连扫描器都还是 top-level-only，依赖另一套 call-site suspension / callee-state replay 扩展。
  - 因此 `T2003c0b2c` 必须拆分，不能作为单轮可安全回归的任务直接实现。
- 拆分方案（准备同步到 `TODO.md` / `PLAN.md`）：
  1. `T2003c0b2c1`：mixed-arm sibling escape-continuation 支持 nested block / if 中的 direct site。
  2. `T2003c0b2c2`：扩展到 while body 中的 direct site。
  3. `T2003c0b2c3`：补 nested indirect call sites，并收口 direct/indirect 共存语义。
- 本轮目标：
  - 先完成任务拆分；
  - 然后只执行 `T2003c0b2c1`；
  - 完成后更新 `TODO.md` / `PLAN.md` / 本文件，跑相关测试并提交 commit。

## 进一步评估（继续细拆）

- 在阅读 `single-arm escape continuation` 的 nested direct replay 代码后，确认：
  - `nested block` 主要是顺序前缀/尾部 replay；
  - `if then/else` 需要双分支拦截、未命中分支顺序执行与 CFG 合流；
  - 两者虽然同属 direct control-flow shape，但实现复杂度仍不对称。
- 因此准备把当前的 `T2003c0b2c1` 再拆成：
  1. `T2003c0b2c1a`：nested block direct site。
  2. `T2003c0b2c1b`：if branch direct site。
- 当前轮次进一步收口为 `T2003c0b2c1a`，避免把 block 与 if 的 replay 逻辑绑在同一提交里。

## 当前结果

- `T2003c0b2c1a` 已实现完成。
- 代码侧完成项：
  - mixed-arm escape direct site 新增 block-only 扫描与 resume-path 表示；
  - mixed-arm site matrix 现可识别 statement-position nested block 中的 direct escape site，并在 pre-immediate / post-immediate 两侧 replay block tail；
  - nested block 中 perform 前声明且在 resume 后继续使用的 body local，现已纳入 capture/lift 分析与 state 恢复；
  - `if` / `while` / nested indirect 仍保持稳定诊断，不被本轮误放开。
- 新增回归：
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_pre_immediate_block.scoop`
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_post_immediate_block.scoop`
  - 现有负例 `tests/fixtures/build/effect_resume_mixed_escape_pre_immediate_nested_is_error.scoop` 继续失败，确认 if 分支仍留给后续任务。
- 已完成验收：
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 下一未完成任务将是 `T2003c0b2c1b`（if branch 的 direct site），本轮到提交后即停止。

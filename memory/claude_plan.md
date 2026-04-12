# 执行计划

说明：这里记录可执行计划、关键判断依据和进度更新，不写逐字化的内部推理。

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务，并在完成后停止。

## 执行步骤

1. 检查最新一次 Git 提交的信息，确认是否提到了现存问题。
2. 若最新提交提到需要先修复的现存问题，先定位并修复这些问题，再继续后续步骤。
3. 读取 `TODO.md`，找出第一个未完成任务。
4. 读取 `PLAN.md`，理解当前计划、依赖关系和任务背景。
5. 判断该任务是否过大或被缺失能力/规范不匹配阻塞。
6. 若任务过大：
   - 把任务拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把拆分后的子任务放到正确顺序；
   - 本次只执行拆分后的第一个子任务。
7. 若发现规范不匹配、缺失语言特性或现存实现缺陷会阻塞任务：
   - 先把阻塞问题作为更前置的任务写入 `TODO.md`；
   - 更新 `PLAN.md` 记录阻塞原因和依赖；
   - 提交这些计划调整并停止。
8. 若任务可直接实施：
   - 阅读相关代码和测试；
   - 实现改动；
   - 补充或调整测试与文档；
   - 运行相关检查，至少覆盖该任务涉及的测试，并尽量满足 `cargo clippy --all-targets -- -D warnings`。
9. 完成后更新文档：
   - 在 `TODO.md` 中标记该任务完成；
   - 在 `PLAN.md` 中更新状态与后续影响；
   - 视需要补充 `README.md` 或代码注释。
10. 检查工作区改动，确认没有误改或遗漏。
11. 使用清晰的提交信息提交本次改动。
12. 停止，不继续处理下一个任务。

## 风险与约束

- 不允许以变通方案、夹层兼容、仅夹具通过等方式绕过规范缺口。
- 如果遇到阻塞，优先把阻塞显式转成前置任务，而不是继续硬做。
- 不回滚不属于本次任务且非我产生的改动。
- 若 `PROMPT.md` 在过程中被改动，需要一并纳入提交。

## 进度

- 已创建执行计划文件，下一步将检查最新提交与任务列表。
- 已检查最新提交 `1fdc1b9 [T2003c0c2b1b] Fix indirect escape tail-perform resume path`；提交信息本身未附带新的前置遗留问题说明。
- 已定位当前第一个未完成任务：`T2003c0c2b2`（无 immediate-resume，single indirect escape site，允许 sibling non-resuming）。
- 初步判断：该任务看起来可以直接基于既有的
  - 无-immediate + single direct escape site lowering，
  - single-arm indirect escape continuation，
  - 以及 escape + sibling non-resuming mixed dispatch
  三条已完成能力收口，不先做任务拆分。
- 下一步：阅读 LLVM codegen 中无-immediate mixed-arm dispatch、single indirect escape lowering、相关 fixtures 与稳定诊断位置，确认缺口后实施。
- 已定位到 `crates/scoopc/src/llvm/codegen/effect.rs` 中现成的 `codegen_handle_expr_escape_with_nonresuming_siblings_indirect` lowering；入口已经会在“无 immediate-resume + top-level single indirect site”时分流到该路径。
- 已新增回归夹具：
  - `effect_multi_escape_indirect_single_site`
  - `effect_multi_escape_custom_nonresuming_indirect_single_site`
  - `effect_multi_escape_raise_indirect_single_site`
- 已逐个执行新增夹具，当前实现全部通过；结论是 `T2003c0c2b2` 的主要缺口不在 codegen 实现本身，而在缺少验收回归与任务状态同步。
- 已完成全量验证：
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已更新 `TODO.md` / `PLAN.md`：
  - `T2003c0c2b2` 已标记完成；
  - 下一步已顺延到 `T2003c0c2b3`。
- 下一步：检查工作区 diff，提交本次改动，然后停止。

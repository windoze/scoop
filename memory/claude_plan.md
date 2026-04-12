# 执行计划与进度记录

## 说明

本文件记录本轮可公开的执行思路摘要、步骤计划、关键判断与进度更新，用于在执行过程中持续同步状态。

## 初始目标

按仓库根目录 `TODO.md` 的顺序，只完成第一个未完成任务，然后停止。在开始具体实现前，先检查最新提交是否提到既有问题；若有，先修复这些问题，再继续当前任务。

## 初始执行步骤

1. 检查当前工作树状态，确认是否存在未提交改动，避免覆盖用户已有工作。
2. 查看最新一次提交的提交信息与改动内容，判断是否提到了尚未解决的已知问题。
3. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并理解当前计划上下文。
4. 如该任务过大，先把它拆分成更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；当前只执行拆分后的第一个子任务。
5. 实现当前目标任务，必要时补充或调整测试。
6. 运行与变更相关的验证命令；若任务触及通用基础设施，则补充更广泛验证，并确保 `cargo clippy --all-targets -- -D warnings` 无警告。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 使用清晰的提交信息创建一次 git 提交，然后停止，不继续后续任务。

## 当前已知约束

- 需要使用中文输出说明与进度记录。
- 不应回退或覆盖与当前任务无关的现有修改。
- 若遇到规范不匹配、缺失特性或已有缺陷，必须先把它们转化为明确任务并调整优先级，不能绕过。

## 进度

- 已创建本文件并写入初始计划。
- 已检查 `git status`：当前只有本文件存在未提交修改，暂无与本轮任务冲突的用户改动。
- 已检查最新提交 `afa4feb003009e7124eb1b2336075cf1ae94b965`，提交信息为 `[T2003c0c2b3c1] Support no-immediate top-level multiple indirect escape sites`；提交说明本身未额外标记需先修复的既有问题。
- 已阅读 `TODO.md` / `PLAN.md` 并定位当前首个未完成任务为 `T2003c0c2b3c2`：无 immediate-resume 的多 arm handle，在 nested block 中支持 indirect escape sites。
- 已完成任务细节与代码审计，当前判断不需要再拆分：
  - 分流入口过早把所有带 `resume_path` 的 no-immediate indirect site 一律挡在 `escape site matrix not yet supported`；
  - no-immediate indirect-matrix 内部又额外要求所有 indirect site 必须是 top-level；
  - 但仓库里已经存在可复用的 nested block indirect prefix / tail replay helper，可直接接入该路径。
- 当前实现计划：
  1. 放宽 no-immediate 多 arm 的入口分流，让“纯 indirect + top-level / block-only nested block”进入 indirect-matrix lowering。
  2. 在 no-immediate indirect-matrix 中新增按 top-level 语句分类的 simple site 索引，允许 top-level 与 block-only nested block indirect；`if` / `while` indirect 继续保留稳定诊断。
  3. 在初次执行与 continuation step 中，为 nested block indirect 站点接上 prefix replay、tail replay 与 scope pop。
  4. 新增 run-pass fixture 覆盖 nested block local capture/restore + sibling non-resuming dispatch；保留并更新 while indirect build fixture。
  5. 跑格式化/测试/LLVM fixture/clippy，随后回写 `TODO.md`、`PLAN.md` 与本文件并提交。
- 实施过程中补充确认：
  - 第一次实现后，run-pass 新 fixture 的 stdout 缺少 block tail 内容；原因是 continuation step 只会 replay 后续 top-level 语句，没有先 replay “当前正在恢复的 nested block indirect site” 自己的 block tail。
  - 修复方式：在 step trampoline 恢复当前 nested block indirect site 时先补齐 block scope，再在当前 site 的 indirect binding 返回后执行 `continue_after_indirect_site(...)`，之后才继续后续 top-level tail。
- 最终结果：
  - `T2003c0c2b3c2` 已完成。
  - 已新增回归：
    - run-pass `effect_multi_escape_custom_nonresuming_indirect_block_single_site`
    - build `effect_multi_escape_indirect_if_is_error`
  - 已完成验证：
    - `cargo fmt --all --check`
    - `cargo test --all`
    - `cargo run -p scoop -- test`
    - `cargo run -p scoop --features llvm -- test`
    - `cargo clippy --workspace --all-targets -- -D warnings`
  - 下一轮首个未完成任务将是 `T2003c0c2b3c3`（if branch indirect escape sites）。

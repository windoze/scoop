# 本轮执行计划

## 约束说明

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 先检查最新提交是否提到任何既有问题；若有，先修复这些问题，再进入任务执行。
- 如果首个未完成任务过大或被缺失特性/缺陷阻塞，则需要先调整 `TODO.md` / `PLAN.md` 的任务拆分与依赖顺序，再提交并停止。
- 我不会在这里写出逐字逐句的内部推理，但会持续记录可审计的执行计划、关键决策、阻塞原因和完成进度。

## 初始步骤

1. 查看最新提交信息，确认是否显式提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务顺序是否一致。
4. 检查工作区状态，避免覆盖已有未提交修改。
5. 评估首个未完成任务：
   - 若可直接完成，则实现、补充测试、运行相关校验、更新文档与任务状态。
   - 若过大，则拆分成更小子任务，更新 `PLAN.md` 与 `TODO.md`，本轮只做新的第一个子任务。
   - 若被规范不匹配、缺失特性或现有缺陷阻塞，则先把该阻塞项转化为更前置的任务，更新计划后提交并停止。
6. 完成后执行必要验证，至少覆盖相关测试；若任务涉及通用编译/静态检查，也运行对应命令。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，然后提交一次清晰的 Git commit。

## 待补充

## 已确认信息

- 最新提交 `e60d98cf39a82efd4230d30f589d9ab9c21e420c` 的主题是 `Update plan`，未显式引入需要优先修复的既有代码问题。
- 当前 `TODO.md` 中第一个未完成任务是 `T2003c0b2c3`：mixed-arm immediate-resume + sibling escape-continuation 的 nested indirect call site。
- 经过代码与计划审计，`T2003c0b2c3` 当前同时覆盖：
  - nested block / if / while 三类 indirect call-site suspension；
  - nested direct / indirect site 共存矩阵；
  - 相关 capture、tail replay、callee suspend state 与 handler-scope 一致性。
- 这一范围超过单轮安全实现边界，因此需要继续拆分。

## 本轮调整后的执行计划

1. 把 `T2003c0b2c3` 拆成更小的 nested-indirect 子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
2. 只实现拆分后的第一个子任务：优先支持 mixed-arm sibling escape-continuation 在 statement-position nested block 中的 indirect call site。
3. 为该子任务补充回归：
   - run-pass：覆盖 pre/post-immediate 的 nested block indirect；
   - build 负例：继续锁住 if / while 或更深层 nested indirect 的未支持边界。
4. 运行格式化、相关测试与静态检查；若通过，则更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 当前实现判断

- 现有 mixed-arm `site matrix` 已统一 direct / indirect 的 top-level 路径，但 indirect site 的扫描器 `scan_for_indirect_perform_call_sites(...)` 仍只看顶层 `val x = f(...)`。
- nested direct 已有 block / if / while 的 prefix 与 tail replay helper；nested indirect 需要补：
  - nested site 的路径表示与扫描；
  - nested block prefix 到 indirect call site；
  - resume 后从 nested block indirect site 继续 replay nested tail 与后续 top-level tail。

## 本轮结果

- 已完成任务拆分：`T2003c0b2c3` 已拆为 `T2003c0b2c3a` / `b` / `c` / `d`，并已把后续依赖改接到新的子任务链。
- 本轮已完成 `T2003c0b2c3a`：
  - mixed-arm `site matrix` 的 indirect site 现已携带 `resume_path`；
  - 新增 nested-block-only indirect scanner，并对 if / while nested indirect 给出稳定诊断；
  - state0 / state1 / continuation step 均已支持 statement-position nested block indirect 的 prefix / tail replay。
- 已新增回归：
  - run-pass：`effect_resume_mixed_escape_pre_immediate_block_indirect`
  - run-pass：`effect_resume_mixed_escape_post_immediate_block_indirect`
  - build：`effect_resume_mixed_escape_if_indirect_is_error`

## 验证结果

- `cargo fmt --all`
- `cargo test --all`
- `cargo run -p scoop -- test`
- `cargo run -p scoop --features llvm -- test`
- `cargo clippy --workspace --all-targets -- -D warnings`

以上命令已通过。本轮下一未完成任务已更新为 `T2003c0b2c3b`（if branch 的 indirect site）。

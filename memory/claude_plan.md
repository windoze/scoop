# 执行计划与进度记录

## 约束说明

- 按用户要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在执行任何 shell 命令前，先写入本文件。
- 出于安全与策略限制，这里记录的是可审计的执行决策、检查项与步骤摘要，不写入逐字内部推理。

## 初始执行计划

1. 检查最新一次 Git 提交的提交信息与变更摘要，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖与现有分解情况。
4. 判断该任务是否过大：
   - 如果可直接完成，则进入实现。
   - 如果过大或依赖不清，则先在 `PLAN.md` 中细化，并同步调整 `TODO.md` 中的任务拆分与顺序，然后只执行拆分后的第一个子任务。
5. 实现目标任务，并在实现过程中检查是否暴露出规范不匹配、缺失特性或既有缺陷。
6. 若发现阻塞当前任务的真实缺陷：
   - 不做绕过；
   - 在 `TODO.md` 中新增或前移前置修复任务；
   - 在 `PLAN.md` 记录阻塞原因与依赖关系；
   - 提交变更后停止。
7. 对完成的实现执行相关验证：
   - 先运行最小相关测试；
   - 再根据变更范围运行更广泛测试；
   - 最终至少检查 `cargo clippy --all-targets -- -D warnings` 是否通过（若时间或环境限制导致无法完成，会如实记录）。
8. 更新文档与任务状态：
   - 在 `TODO.md` 标记完成；
   - 在 `PLAN.md` 反映当前状态；
   - 必要时补充 `README.md` 或代码注释。
9. 进行 Git 提交，提交信息使用任务号与清晰描述。
10. 停止，不继续下一个任务。

## 进度状态

- 当前状态：已完成初始仓库检查，确认最新提交信息未额外声明必须先修复的既有问题。
- 已定位本轮目标：`TODO.md` 中第一个未完成任务为 `T2003c0b2c3c`，内容是“mixed-arm sibling escape-continuation 支持 while body 中的 indirect call site”。
- 任务复杂度判断：暂不需要进一步拆分。现有相邻任务已分别覆盖：
  - nested block 的 indirect site；
  - if branch 的 indirect site；
  - while body 的 direct site；
  - nested while direct site。
  当前缺口主要集中在 while-indirect 的扫描门禁与 continuation step / tail replay lowering。

## 当前执行方案

1. 阅读 `crates/scoopc/src/llvm/codegen/effect.rs` 中 mixed-arm indirect site 的扫描、prefix、tail replay 与 continuation step 相关代码。
2. 对照已有的 `if`-branch indirect 与 `while` direct 路径，确定 while-indirect 需要新增或放开的 helper 分支。
3. 实现 while body indirect 的最小正确支持：
   - 扫描阶段允许 while body 中的 flat / 受控 nested indirect site；已完成。
   - continuation step 在 `resume(...)` 后先 replay 当前迭代剩余路径，再继续当前 while body、重新检查 condition，并允许后续迭代再次命中；已完成代码接线，待测试验证。
   - 对更深层 nested while 或未纳入本阶段范围的形状继续保留稳定诊断；已完成代码与负例更新，待测试验证。
4. 新增/调整 fixtures：
   - run-pass：补一个 flat pre-immediate while-indirect，补一个 nested post-immediate while-indirect；已完成。
   - build：将既有负例改成更深层 nested while 形状，锁定稳定诊断；已完成。
5. 运行相关验证；若通过，再更新 `TODO.md` / `PLAN.md`、提交并停止。

## 当前关注的实现风险

- `scan_mixed_escape_indirect_sites` 当前对 `while` body 直接报错，需要精确定义允许的嵌套路径，不可把更深层 loop 一并放开。
- `codegen_mixed_escape_matrix_nested_tail_after_indirect_site_from_depth` 当前对 `WhileBody` 留有 dedicated lowering 占位，需要补成可重放当前迭代尾部并重入循环。
- 需要确保 loop re-entry 后的同一 indirect site 能再次拦截，而不是只支持一次性恢复。

## 当前进度更新

- 已完成 `effect.rs` 的核心代码修改：
  - `scan_mixed_escape_indirect_sites` 现已允许 while body indirect，并对 deeper nested while 保持稳定诊断。
  - 已新增 while-indirect 的 prefix / top-level stmt / tail-after-resume lowering helper。
  - mixed-arm site matrix 的分类与 `state0` / `state1` / `step trampoline` 三条路径均已接入 while-indirect 分支。
- 已完成 fixtures：
  - 新增 `effect_resume_mixed_escape_pre_immediate_while_indirect`
  - 新增 `effect_resume_mixed_escape_post_immediate_while_nested_if_indirect`
  - 更新 `effect_resume_mixed_escape_while_indirect_is_error` 为 deeper nested while 负例
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已同步任务文档：
  - `TODO.md` 已将 `T2003c0b2c3c` 标记为完成并补充完成说明。
  - `PLAN.md` 已记录本轮落地结果，并将下一步推进到 `T2003c0b2c3d`。
- 当前剩余动作：检查工作区差异，提交本轮变更，然后停止。

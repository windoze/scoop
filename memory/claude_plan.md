# 执行计划

## 约束与工作方式

- 本次只处理 `TODO.md` 中第一个未完成任务，完成后即停止。
- 在继续任务前，先检查最新提交是否提到已知遗留问题；若提到，则这些问题优先纳入本次范围。
- 若当前首个未完成任务过大，会先把它拆成更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
- 实施过程中如发现任何与规范不一致、不能通过正确实现完成、或需要依赖缺失语言特性的情况，不做规避实现；而是把缺口前置为新的待办，更新 `TODO.md` / `PLAN.md` 后提交并停止。
- 变更后需要做充分验证，至少覆盖相关测试；若适用，还会运行格式化、`clippy`、以及针对任务的最小充分测试集合。
- 完成后会更新 `TODO.md`、`PLAN.md`，提交 git commit，然后停止，不继续处理下一个任务。

## 当前阶段计划

1. 查看最新提交，确认是否提到需要先修复的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对任务背景、依赖关系和是否需要拆分。
4. 结合相关源码与测试，确认任务边界、现状和缺口。
5. 如任务过大，先拆分任务并更新 `PLAN.md` / `TODO.md`，然后以新的首个子任务为当前执行目标。
6. 实现当前目标任务，并补充或调整测试。
7. 运行相关验证；若发现问题，先修复再重复验证。
8. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或阻塞原因。
9. 提交本次变更，提交信息对应当前任务，然后停止。

## 执行记录

- 已写入初始计划，尚未开始仓库检查。
- 已检查最新提交 `fa6bf49`，未发现提交说明中额外声明的遗留问题；当前仍按 `TODO.md` 主线继续。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务是 `T2003c0b2c3d2`：在同一个 `if` 语句里支持 sibling escape-continuation 的 direct / indirect 共存。
- 已验证该任务当前的最小失败形态会在 LLVM codegen 报：
  - `handle mixed-arm escape continuation (multiple sites per top-level statement not yet supported)`
- 已完成当前任务的实现边界审计：
  - 任务已经足够具体，不再继续拆分。
  - 当前缺口主要有四处：
    1. mixed-arm site matrix 的 top-level 分类逻辑把同一个 `if` 语句里的 direct / indirect site 视为互斥类别，直接提前拒绝。
    2. body-lift 分析只有 nested block 的 direct->indirect pair 特判，没有 if-branch 对应分支。
    3. state0 / step / main-body 三条 lowering 路径都只接了 direct-only 或 indirect-only 的 `if` helper，没有 mixed helper。
    4. current-site 恢复后“继续命中同一 if 分支里的下一个 site”的 next/prev 映射只为 nested block 建了路由，没有 if 分支对应路由。
- 下一步具体执行：
  1. 为 same-`if` mixed site 增加分类与 next/prev 路由，并限制在“同分支、tail 仅为空或 block-only”这一可验证子集。
  2. 增加 if-branch 的 used-between / continue-to-next-site helper，复用已有 block replay primitive。
  3. 打通 state0 / step / main-body 中 mixed-if 的入口与恢复续跑。
  4. 新增 run-pass fixtures，至少覆盖 pre-immediate 与 post-immediate 两条 mixed-if 路径。
  5. 运行格式化、相关测试、全量测试与 `clippy`，然后更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成实现：
  - `effect.rs` 已新增 same-`if` mixed site 的分类、顺序判定、next/prev replay 路由，以及 if-branch 的 used-between / continue-to-next-site helper。
  - state0、state1 与 resumed main tail 已接入 mixed-if lowering；post-immediate direct→indirect 续跑中缺失的 branch scope 已修复。
  - 已新增 fixtures：
    - `effect_resume_mixed_escape_pre_immediate_if_indirect_direct`
    - `effect_resume_mixed_escape_post_immediate_if_direct_indirect`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 剩余收尾：
  1. 提交当前变更。
  2. 停止，本轮不继续处理下一个任务。

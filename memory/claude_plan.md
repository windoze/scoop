# 本轮执行计划

更新时间：2026-04-21

## 任务目标

按 `TODO.md` 的顺序执行**第一个未完成任务**，在完成实现、测试、文档更新和提交后立即停止。

## 思路摘要

基于当前要求，本轮需要先确认两类前置条件：

1. 最新提交是否明确提到已有缺陷、已知问题或待补修内容；若有，需要先修复这些问题。
2. `TODO.md` 中当前排在最前面的未完成任务是什么，以及它是否可以在本轮完整落地。

随后按以下原则推进：

- 如果首个未完成任务可以直接完成，就实现它并补齐测试。
- 如果该任务依赖尚未完成的语言特性、运行时能力或现存缺陷修复，则不能绕过；必须先把缺失项整理为更前置的任务，更新 `TODO.md` / `PLAN.md`，提交后停止。
- 执行过程中如发现计划判断有误、实现范围需要调整、或关键步骤已完成，会继续更新此文件。

## 分步执行计划

1. 检查最新一次 Git 提交的提交信息与变更说明，确认是否带有“已知问题 / follow-up / FIXME / TODO / bug”等需要先处理的内容。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对该任务的上下文、依赖和现有分解是否一致。
4. 评估该任务复杂度与依赖：
   - 若可在本轮完整完成，直接进入实现。
   - 若不可直接完整完成，则把任务拆解为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后执行拆解后的第一个子任务。
5. 在动手修改代码前，再次更新本文件，记录将要修改的模块与验证策略。
6. 实现目标任务，保持实现符合规范，不采用规避式 workaround。
7. 运行与改动相关的验证命令，至少覆盖：
   - 直接相关测试
   - 必要的回归测试
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 若任务影响整体可构建性，再补充 `cargo test --all` 或更合适的子集
8. 若测试暴露规格不匹配或已有缺陷：
   - 优先修复属于本任务范围内的问题；
   - 若暴露更基础的前置缺陷，则更新 `TODO.md` / `PLAN.md` 调整优先级，并在提交后停止。
9. 完成后更新文档状态：
   - 在 `TODO.md` 标记该任务完成
   - 在 `PLAN.md` 记录当前状态与后续影响
   - 在本文件记录已完成步骤与结果
10. 生成一次 Git 提交，提交信息对应当前任务，然后停止。

## 当前状态

- 已完成：初始化本轮计划文件。
- 已完成：检查最新 Git 提交；提交信息为 `Update plan`，未显式声明需要先修复的既有缺陷。
- 已完成：读取 `TODO.md` 与 `PLAN.md`。
- 已判断：`TODO.md` 中排在最前面的可执行未完成子任务为 `T4016a`；总括条目 `T4016` 已经完成任务拆分，因此本轮聚焦 `T4016a`。
- 已修正判断：在进一步核对 `sysroot` / compiler 主线约束后，原始 `T4016a` 仍偏大，已拆成 `T4016a1`（spec/runtime 设计文档）与 `T4016a2`（sysroot / 实现注释过渡合同）。
- 已完成：更新 `TODO.md` / `PLAN.md`，把顺序调整为 `T4016a1 -> T4016a2 -> T4016b -> ...`。
- 已完成：执行新的首个子任务 `T4016a1`，收口了 spec / runtime doc 中的 continuation surface、handler 语法与迁移说明。
- 已完成验证：
  - `cargo fmt --check`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
- 进行中：更新任务状态并准备本轮收尾提交。

## 针对 T4016a 的即时检查要点

1. 现有 `Continuation` 类型定义、sysroot surface、spec 与 runtime 文档是否仍停留在 `resume(...): Unit` 叙事。
2. parser / AST / HIR 是否仍保留 `-> resume` 用户态语法，相关测试与文档分布在哪里。
3. typecheck 是否已经部分携带 answer type，还是完全缺失。
4. `Task` 是否已经与 continuation answer model 解耦，还是仍依赖 runtime frame result hack。
5. 根据以上现状，判断 `T4016a` 是否可作为“文档/表面设计收口任务”独立完成，或是否必须进一步拆成更小步骤。
6. 当前实际执行项改为 `T4016a1`，因此优先修改以下位置：
   - `SCOOP_FULL_SPEC.md`
   - `SCOOP_RUNTIME.md`
7. 明确写清：
   - `Continuation` 的 answer type 语义与推荐表面模型；
   - `k.resume(...): Answer / (E + Raise<RuntimeError>)`；
   - deep handler / fresh continuation / cross-thread resume；
   - `-> resume` 已移除，迁移到 `, k ->` + `k.resume(...)`；
   - multi-shot / clone / replay 继续 deferred。
8. 完成 `T4016a1` 后，运行文档相关验证与必要的编译/测试子集，确认没有因文档更新引入构建问题。
9. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 本轮结果摘要

- `T4016a` 已拆成更小的 `T4016a1` / `T4016a2`，避免把设计定稿与 sysroot / compiler 表示改动混在同一次提交里。
- `T4016a1` 已完成：
  - `SCOOP_FULL_SPEC.md` 现已把用户态 handler surface 收口为 `->` 与 `, k ->` 两种形式。
  - continuation 文档语义已改为 answer-returning：`k.resume(payload...): Answer / (E + Raise<RuntimeError>)`。
  - 规范文字已明确 deep handler、fresh continuation、cross-thread resume、one-shot，以及 `-> resume` 的迁移方向。
  - `SCOOP_RUNTIME.md` 已同步为同一套目标合同，并明确后续由 `T4016b/c/d` 完成主线实现对齐。

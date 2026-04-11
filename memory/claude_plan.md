# 执行记录与计划

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务，并在完成后停止。

## 初始执行计划

1. 先检查最新一次 Git 提交，确认提交说明里是否提到已知问题、遗留问题或需要顺手修复的事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认当前项目计划与 `TODO.md` 是否一致。
4. 如果首个未完成任务过大，则把它拆成更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本次只执行拆分后的第一个子任务。
5. 实现本次要处理的任务，并补充必要的测试与文档。
6. 运行相关验证，至少包含与改动相关的测试；如果任务涉及通用构建质量，还要检查格式、lint 与编译告警。
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况、阻塞信息或任务重排原因。
8. 提交 Git commit，然后停止，不继续处理下一个任务。

## 说明

- 这里记录的是可审计的执行计划与关键决策，不包含内部推理细节。
- 如果后续发现计划需要调整，或完成了关键步骤，会继续更新本文件。

## 当前进展

- 已检查最新提交 `56c06bd372d2f3be18a39d3a25d173f5390333a5`（`[T2003c0b2b1] Support mixed-arm post-immediate multiple direct escape sites`）。
- 该提交说明未直接提到需要先修复的遗留问题，因此当前无需在任务前插入“提交说明明确要求”的额外修复项。
- 已读取 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T2003c0b2b2`：扩展 sibling escape-continuation 到 post-immediate indirect / direct+indirect site matrix。

## 下一步

1. 继续读取 `T2003c0b2b2` 的完整描述、验收与依赖。
2. 审计现有 mixed-arm immediate-resume + sibling escape lowering 的代码结构与现有 fixtures。
3. 判断 `T2003c0b2b2` 是否仍需进一步拆分；若需要，先更新 `TODO.md` / `PLAN.md`。
4. 若范围可控，直接实现、补测试、跑验证、更新计划与任务状态、提交 commit。

## 任务审计结论

- `T2003c0b2b2` 当前可以直接实现，不再继续拆分。
- 现状：
  - mixed-arm direct lowering 已支持 post-immediate 多个 top-level direct escape sites。
  - mixed-arm indirect lowering 仅支持单个 post-immediate indirect site。
  - dispatcher 仍把 `direct + indirect`、`multiple indirect` 作为稳定诊断直接拒绝。
- 实现方向：
  - 在 mixed-arm escape sibling lowering 中补一个统一的 top-level site matrix 路径，把 post-immediate 的 direct/indirect sites 作为同一条 continuation step 链上的不同站点处理。
  - 复用 direct 多站点路径已有的 `pc + capture-state + step trampoline` 设计，并把 indirect site 的 “resume 写回 callee suspend state + 重新调用 + 可再次逃逸” 接进去。
  - 保留本阶段已有边界：仍只支持 top-level val-bound site；pre-immediate site 与更深层 nested/control-flow 形状继续留给后续任务。

## 已完成

1. 已在 `crates/scoopc/src/llvm/codegen/effect.rs` 新增 mixed-arm post-immediate site matrix lowering：
   - 支持 multiple indirect sites。
   - 支持 direct + indirect 共存（包含 direct→indirect 与 indirect→direct 两种顺序）。
   - 继续保留 pre-immediate / nested site 的稳定诊断。
2. 已新增/调整 fixtures：
   - run-pass：`effect_resume_mixed_escape_indirect_multi`
   - run-pass：`effect_resume_mixed_escape_direct_indirect`
   - run-pass：`effect_resume_mixed_escape_indirect_direct`
   - build：`effect_resume_mixed_escape_pre_immediate_direct_indirect_is_error`
   - 删除旧的 post-immediate 负例 `effect_resume_mixed_escape_direct_indirect_is_error`
3. 已完成验证：
   - `cargo test --all`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo check -p scoopc --features llvm`

## 收尾事项

1. 更新 `TODO.md` / `PLAN.md`，把 `T2003c0b2b2` 标记为完成。
2. 检查工作区变更。
3. 提交本轮改动并停止。

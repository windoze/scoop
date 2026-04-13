# 执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 约束与执行原则

- 先检查最新一次提交是否提到已有问题；若有，则这些问题优先处理。
- 读取 `TODO.md`，确定第一个未完成任务。
- 若该任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 实现后必须补充或运行相关测试，并修复暴露的问题。
- 任务完成后更新 `TODO.md`、`PLAN.md`，再提交 git commit，然后停止。
- 若发现规格不匹配、缺失能力或被前置问题阻塞，不做绕过实现；改为更新 `TODO.md`/`PLAN.md` 记录依赖与阻塞关系，提交后停止。

## 当前步骤

1. 检查最新提交信息，确认是否包含需要先修复的已有问题。
2. 读取 `TODO.md`、`PLAN.md`，确定当前首个未完成任务及其上下文。
3. 审阅 `state_machine_segments.rs` / `state_machine_plan.rs` / 相关测试，明确 `T2003r1b` 缺口。
4. 扩展 segment metadata：
   - 增加 dispatch entry 投影；
   - 增加 arm body 元数据；
   - 给 segment 标注 `dispatch_context` 与 `cleanup_scope_stack`；
   - 给 suspend site 标注 owner segment，避免后续 builder 再回推。
5. 补定向 segment dump 单测，覆盖 mixed-arm / sibling non-resuming / cleanup context。
6. 运行相关测试与质量检查。
7. 更新文档状态并提交变更。

## 进度记录

- 已检查最新提交：提交信息未显式提到需要优先修复的遗留 issue。
- 已读取 `TODO.md` / `PLAN.md`，确认本轮首个未完成任务为 `T2003r1b`，当前不需要继续拆分。
- 已完成代码阅读：当前第一版 `HandleSegmentList` 只有 states / edges / suspend-sites / cleanup-scopes 的基本投影；multi-arm dispatch、arm body、segment 级 cleanup/dispatch context 尚未进入 segment metadata。
- 已完成实现：`HandleSegmentList` 现已补齐 `dispatch_entries` / `arm_bodies`，segment 现显式记录 `dispatch_context` / `cleanup_scope_stack`，suspend site 现记录 owner segment。
- 已完成测试：
  - `cargo test -p scoopc segment_dump_`
  - `cargo test -p scoopc plan_dump_`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 当前正在更新 `TODO.md` / `PLAN.md`，随后提交本轮变更并停止。

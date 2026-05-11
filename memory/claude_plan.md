# 执行计划

注意：此文件记录的是简明执行计划与进度摘要，不包含逐字内部推理。

## 初始计划

1. 读取 `TODO.md`，严格按标题是否带有 `[DONE]` 判断第一个未完成任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务的一部分或作为前置任务处理。
3. 阅读当前任务涉及的代码、测试、文档与约束，确认依赖与验收标准。
4. 如可直接完成，则以最小正确修改实现任务；如遇阻塞且必须新增前置任务，则只做最小必要的 `TODO.md` / `PLAN.md` 调整。
5. 运行与当前任务直接相关的验证；随后补充执行要求中的质量检查（至少包含相关测试，并尽量验证 `cargo clippy --all-targets -- -D warnings` 是否通过）。
6. 更新 `TODO.md` 的任务状态与完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
7. 按仓库提交风格创建一次提交，完成后停止，不进入下一个任务。

## 进度记录

- 已创建初始计划，下一步读取 `TODO.md` 确认首个未完成任务。
- 已确认首个未完成任务为 `G6-T07R`（Review direct/static/dynamic call lowering，确认 ABI 分流已 facts-driven）。
- 最近一次提交为 `[G6-T07] Rebuild non-legacy call lowering`，与当前 review 任务直接相关；下一步将围绕该提交涉及的 call lowering / closure / class ctor / thread / effect-lowered value 路径做定向复核，并运行 `cargo check -p scoopc` 与符号 grep。
- 已完成定向复核：
  - `cargo check -p scoopc` 前沿仍停在后续 `G7-T08` 缺口，未回退到 `G6-T07` 已补的 call lowering impl 缺失。
  - `cargo fmt --check` 通过。
  - 针对 review 范围执行 legacy bridge/TLS 名字 `rg`：无命中。
  - 人工复核结论：direct/static/vtable/itable 继续由 published callable contract 驱动显式 hidden ABI；closure body declaration 仍走同一 contract；dynamic/effect-typed closure adapter 继续消费 published dynamic-invoke layout；未发现需要在 `G6` 范围内追加修补的新问题。
- 下一步：提交 `TODO.md` 与 `memory/claude_plan.md` 的更新，完成本次调用并停止。

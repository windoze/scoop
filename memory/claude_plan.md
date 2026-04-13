# 当前执行计划

## 说明

根据系统约束，这里记录的是可审阅的执行计划、关键判断依据和进度更新，不包含逐字逐句的内部推理细节。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到任何已知遗留问题。
2. 如果最新提交提到了需要先修复的遗留问题，优先定位并修复，补充测试，再继续后续步骤。
3. 阅读 `TODO.md`，识别第一个未完成任务。
4. 评估该任务规模：
   - 如果可以在当前轮次完整落地，则直接实现。
   - 如果过大或存在明确前置依赖，则拆分任务，更新 `PLAN.md` 与 `TODO.md`，并执行拆分后的第一个子任务。
5. 实施代码修改，保证实现符合规格，不引入临时性绕过方案。
6. 运行相关测试，并补充必要测试；同时检查格式、clippy、编译告警等质量门槛。
7. 更新文档与计划文件：
   - 在 `TODO.md` 中标记本轮完成的任务。
   - 在 `PLAN.md` 中记录当前状态与后续计划调整。
   - 在本文件中补充关键进展。
8. 使用清晰的提交信息创建 Git 提交，然后停止，不继续处理下一个任务。

## 进展更新

- 已检查最新一次提交 `c6ef9bb5e9b3f23d935356370a8dac46e5629528`：
  - 提交标题为 `[T2003u5a] Support multi-escape siblings with finally`。
  - 提交说明中未额外点名需要先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`：
  - 首个未完成任务为 `T2003u5b`：`single-arm immediate-resume` 的 while-nested replay 去形状门禁。
  - `PLAN.md` 已明确这是当前下一步，暂无进一步拆分要求。

## 当前执行计划（细化到本轮任务）

1. 定位 current unified-plan immediate-resume emitter 中对 while-nested replay 的门禁与现有路径假设。
2. 读取相关 fixture / 诊断，确认当前支持与不支持的 while body nested 形状。
3. 设计并实现去门禁所需的 replay / source-path / state 恢复逻辑。
4. 为 newly-supported nested while immediate-resume 补正向回归；如需保留更深层边界，则补稳定负向回归。
5. 运行格式化、测试、LLVM fixture、clippy。
6. 更新 `TODO.md`、`PLAN.md`、本文件，并创建本轮提交。

## 当前状态

- 已完成：
  - 初始化计划文件；检查最新提交；确认当前任务为 `T2003u5b`。
  - 定位并移除 `resolve_immediate_resume_site_from_plan` 中对 while body deeper nested perform 的显式门禁。
  - 实现 plan-driven immediate-resume 的 while-nested replay：
    - nested block / nested if source-path 可进入 site；
    - while 迭代内未命中 nested branch 时会回到现有 loop condition；
    - resume 后的 nested frame tail 会正确回到 while re-entry，而不是递归展开 future iteration CFG。
  - 新增回归：
    - 解析层单测 `resolve_immediate_resume_site_from_plan_accepts_nested_while_path`
    - run-pass `effect_resume_while_nested_perform`
    - run-pass `effect_resume_while_nested_block_perform`
  - 删除旧 build-fail `effect_resume_while_nested_perform_is_error`。
  - 完成验证：
    - `cargo test --all`
    - `cargo run -p scoop -- test`
    - `cargo run -p scoop --features llvm -- test`
    - `cargo clippy --workspace --all-targets -- -D warnings`
- 备注：
  - nested block 回归当前仍使用 `@Safe { ... }` 作为临时 block 语法绕路；该迁移已由 `T2203` 明确追踪。
- 进行中：准备更新 git 状态并创建本轮提交，然后停止。

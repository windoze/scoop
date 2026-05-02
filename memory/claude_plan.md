## 当前执行计划

说明：按要求先记录计划与进展。这里记录的是可审计的执行摘要、步骤与决策依据，不包含逐字内部思维链路。

### 目标

完成 `TODO.md` 索引所指向的第一个未完成详细任务；若存在阻塞，则以最小方式补充前置任务并同步索引，然后提交并停止。

### 初始步骤

1. 读取 `TODO.md`，确认它只是索引，并找出引用的详细任务文件。
2. 按任务顺序读取相关 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否明确提到与该任务直接相关且未完成的问题；若是，则将其视为当前任务的一部分或必要前置。
4. 阅读与当前任务直接相关的源码、测试、规范和任务约束，确认验收条件与依赖。
5. 如可直接实现，则做最小正确修改；如遇到阻塞当前任务的真实缺口，则先补充前置任务到相应 `TODO-Px.md` 并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证；若任务触及通用构建/静态检查要求，再补充运行必要的 `cargo` 测试、格式化或 lint 验证。
7. 更新详细任务文件中的完成记录，并仅在需要时同步 `TODO.md` / `PLAN.md`。
8. 提交本次变更，提交信息使用当前任务号，随后停止。

### 执行约束

1. 一次只完成一个详细任务。
2. 不使用变通方案绕过规范或实现缺口。
3. 若发现阻塞，优先修复阻塞或把它登记为当前任务之前的前置任务。
4. 不回退或覆盖与当前任务无关的现有改动。

### 进展日志

- 已创建计划文件，下一步开始读取 `TODO.md` 与对应 `TODO-Px.md` 来定位当前任务。
- 已读取 `TODO.md` 索引并核对 `TODO-P5.md`；当前第一个未完成详细任务为 `P5-T06R`。
- 已检查最近一次提交：`[P5-T06] Optimize late-lowered post-lowering pipeline`。提交信息未显式声明尚未记录在 TODO 中的直接相关未完事项，因此继续按 `P5-T06R` 的 review 要求执行。
- 当前 review 计划：
  1. 阅读 `crates/scoopc/src/effect_lowered/opt.rs`、`materialize.rs` 与 P5 stage 入口，确认优化 pass 只消费 late-lowered representation。
  2. 运行任务要求的关键词搜索，检查是否存在重跑 solver / segmentation / `ImplPlan` 选择的迹象。
  3. 复跑 `P5-T06` 指定测试与静态检查命令。
  4. 若发现问题：先修复问题并补充验证；若当前任务因此被阻塞，则在对应 TODO 文件中插入最小前置任务并同步 `TODO.md`。
  5. 若 review 通过：把 `P5-T06R` 标记为 `[DONE]`，填写完成记录，必要时同步 `TODO.md`，然后提交并停止。

- review 过程中发现直接相关回归：`crates/scoopc/src/effect_lowered/opt.rs` 的 DCE 只从 `entry_state` 做普通 CFG reachability，而 dedicated `drop_state` 是通过 continuation runtime contract 进入、并不出现在普通 successor 集合里，导致 post-opt 可能错误删除 dropped-continuation path，违背 `P5-T04` 已冻结的合同。
- 已实施修复：在 late-opt 的活跃状态收集里显式从 `drop_state` 追加 reachability 根，并新增 stage 级测试，要求 `load_effect_lowered_stage_output_for_dump(...)` 的 post-opt 输出仍保留 dedicated `Drop` state，且 `Suspend` 的 `cleanup_state` 与 `drop_state` 保持分离。
- 下一步：运行 `P5-T06R` 要求的搜索、定向测试和静态检查；若全部通过，则更新 `TODO-P5.md` / `TODO.md` 并提交。
- 已完成验证：关键词搜索确认 late opt 未重跑 solver / segmentation / `ImplPlan` 选择；`refactor_late_opt`、`refactor_dropped_continuation`、`refactor_effect_lowered_stage`、`refactor_late_lowered_ir`、`refactor_step_materialization`、`refactor_boundary_lowering`、`refactor_continuation_object`、`refactor_resume_interface_completeness`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`、`cargo fmt --all --check` 全部通过。
- 已更新 `TODO-P5.md` 与 `TODO.md`：`P5-T06R` 现已标记为 `[DONE]`，完成记录中写明了 review 结论、发现并修复的 `drop_state` 回归以及验证矩阵。
- 下一步：检查工作区改动范围并创建本次任务提交，然后停止。

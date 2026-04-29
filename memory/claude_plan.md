## 当前执行计划

1. 检查最近一次提交信息，确认是否提到了需要先修复的既有问题；如果有，先处理该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否可在一次迭代内完整完成；若过大，则拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前目标任务，避免引入规避性方案；若发现既有缺陷或规格不匹配，先修复或将其作为前置任务插入 `TODO.md`。
5. 运行与本次改动相关的测试、格式化与必要检查，至少覆盖受影响范围；若用户要求或仓库规范要求，则补充更全面验证。
6. 更新 `memory/claude_plan.md`、`PLAN.md`、`TODO.md`，记录完成状态或阻塞原因。
7. 按仓库约定创建一次提交，然后停止，不继续处理下一个任务。

## 当前进度

- 已确认首个未完成任务为 `T5001c2R`。
- 已检查最近提交；提交信息未额外声明待先修复问题。
- 审查中发现一个既有文档化问题：`runtime/c/scoop_gc_backend_immix.c` 中仍有旧注释把 `InNative` 的高层 managed frame 枚举描述为依赖捕获的 unwind ctx；这与 `T5001c2` 现状不符，需先修正注释，再完成 review 结论与验证。
- 已修正上述过期注释，使其与当前合同一致：explicit-frame 路径保留 `explicit_root_frame_top`，仅 stackmap mode 依赖 captured ctx。
- 已完成验证：`cargo test --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo clippy --all-targets -- -D warnings` 通过。
- 下一步：更新 `TODO.md` / `PLAN.md` 标记 `T5001c2R` 完成，随后提交并停止。

## 说明

这里记录的是可执行计划与进度，不包含内部推理细节。

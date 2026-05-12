## 当前执行计划

说明：按要求先记录可公开的执行思路与步骤摘要；不写内部推理细节。

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，确认它是本次唯一执行目标。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的遗留事项；若有，则将其视为当前任务内容或其前置条件。
3. 阅读 `TODO.md` 中该任务的详细要求、依赖、验证标准，并检查相关代码与测试位置。
4. 如任务可直接实现：完成代码修改，并为受影响行为补充或更新测试。
5. 运行任务要求的验证命令，以及必要的 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`；若任务影响范围需要，也运行更广的测试。
6. 若遇到阻塞当前任务的真实缺陷/缺失特性：
   - 不做变通或缩小范围；
   - 在 `TODO.md` 中插入最小必要前置任务并调整依赖顺序；
   - 仅在阶段计划发生变化时更新 `PLAN.md`；
   - 提交这些调整后停止。
7. 若任务完成：
   - 在 `TODO.md` 中将该任务标题标记为 `[DONE]`，并更新 completion record；
   - 如有必要更新 `PLAN.md`；
   - 提交本次所有相关变更；
   - 停止，不继续下一个任务。

## 进度记录

- 已写入初始执行计划，下一步读取 `TODO.md` 与最近提交信息。
- 已确认首个未完成任务为 `P0-T02A`：清理剩余 stable-id 敏感 LLVM 测试中的旧 private helper 名字绑定。
- 已确认最近一次提交 `[P0-T02R] Add prerequisite for legacy helper assertions` 直接对应本任务的来源问题；`TODO.md` 已将其前置为当前执行单元，无需再新增前置任务。
- 下一步：定位 `crates/scoopc/src/llvm/tests.rs` 中该任务列出的几个测试及相关 source inventory，迁移仍直接绑定旧 private helper spelling 的断言为稳定 family / namespace / linkage / 调用关系语义断言。
- 已完成 `crates/scoopc/src/llvm/tests.rs` 的迁移：用 IR 语义 helper 替代多处旧 private helper 全字符串断言，并扩充 source inventory 以防止这些旧绑定回流。
- 已完成验证：`cargo fmt`、若干 stable-id/LLVM 定向测试、`cargo test -p scoopc`、`cargo clippy -p scoopc --all-targets -- -D warnings` 全部通过。
- 已将 `P0-T02A` 在 `TODO.md` 中标记为 `[DONE]`，并补全 completion record；`PLAN.md` 无需更新，因为阶段依赖与验收边界未变化。
- 已创建提交 `b703c6a8 [P0-T02A] Unbind LLVM tests from legacy helper spellings`。
- 当前工作区已清理完毕；本次调用到此停止。

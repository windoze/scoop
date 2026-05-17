# 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断第一个未完成任务。
2. 检查该任务相关的上下文、依赖和最新提交是否存在直接相关的未完成事项。
3. 在不绕过规格要求的前提下实现该任务；如果发现必须先修复的阻塞问题，则把最小必要前置任务写入 `TODO.md` 并停止。
4. 为实现补充或更新最小相关测试/fixture，并运行任务要求或代码变更对应的验证命令。
5. 验证通过后，将当前任务标题标记为 `[DONE]`，更新完成记录；仅在阶段级计划变化时修改 `PLAN.md`。
6. 检查工作区变更，提交本次任务涉及的全部未提交文件，然后停止，不继续下一个任务。

## 进度记录

- 已创建本轮执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已识别第一个未完成任务为 `P11-T02`：测试 helper 迁移到 test cone 或 C ABI extern 或删除。下一步读取 `TODO-5.md` 的完整任务要求，并检查最新提交是否有直接相关的未完成事项。
- 已读取 `P11-T02` 与最新提交：`03aa1548 [P11-T01] Audit GC test helper usage` 正是当前任务前置审计。执行将严格按 P11-T01 决策：新建 `scoop.runtime.test` cone，从 core 迁出四个 helper；fixture 显式 import test cone；stackmap smoke 的编译器 special-case FQN 改到 test cone。
- 已完成主要迁移编辑：`sysroot/runtime_test.scoop` 持有四个测试 helper，`sysroot/core.scoop` 与 custom sysroot overlay 不再声明它们；调用 fixture 与相关 Rust source snippets 已显式导入 `scoop.runtime.test.*`；`heap_object_count` 调用改在 unsafe block 内执行。
- 验证发现负向 owner fixture 失败：未显式 import 时仍可解析 runtime test helper。下一步定位自动 prelude / unqualified resolver 可见性路径，确保 `scoop.runtime.test` 不被默认导入。
- 负向 owner fixture 已调整到 typecheck 阶段（resolver 会延迟裸调用诊断），`tests/fixtures/typecheck/runtime_test_helper_not_in_prelude_is_error.scoop` 现在验证未导入时不可调用并已通过。
- 发现并修复 `runtime_test.scoop` 未作为 support source 编译导致 extern callable signature 缺失的问题；将其加入 compilable sysroot files 后，runtime GC 目录、HIR 目录与全量 fixture suite 均已通过。
- 验证已完成：全量 fixture suite 通过，`cargo test --all --all-targets` 通过，`cargo clippy --all-targets -- -D warnings` 通过。下一步更新 `TODO.md` / `TODO-5.md` 完成记录并提交。
- `TODO.md` 已将 `P11-T02` 标记为 `[DONE]`；`TODO-5.md` 已写入完成记录，并补充后续 P12 任务对 `runtime_test.scoop` 的文件清单/迁移目标引用。

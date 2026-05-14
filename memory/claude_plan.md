# 执行计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务；仅围绕该任务展开，不做开放式问题排查。
2. 查看最近一次提交信息，确认是否存在与当前任务直接相关且明确未完成的问题；若有，将其视为当前任务范围或作为前置依赖处理。
3. 阅读当前任务涉及的代码、测试、规范与依赖项，确认现状与验收要求。
4. 以最小且正确的改动实现当前任务；如果发现阻塞当前任务的真实缺陷或缺失能力，不采用绕过方案，而是在 `TODO.md` 中补充最小必要前置任务并停止。
5. 运行当前任务要求的验证，以及受影响范围内的测试、格式化、lint/构建检查；修复发现的问题。
6. 更新 `memory/claude_plan.md` 记录关键进展；若任务完成，则更新 `TODO.md` 的完成状态与完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 按仓库约定创建一次提交，只完成这一个任务后停止。

## 进展记录

- 初始计划已写入。
- 已读取 `TODO.md`，首个未完成任务为 `P4-T01c`：`@Intrinsic struct/class` 含 method body 完整落地（含 generic class），并锁定 non-generic 维度零编译器后门。
- 下一步：检查最近一次提交是否有与 `P4-T01c` 直接相关的未完成事项，然后阅读 `PLAN.md` 中 P4 前置部分及当前实现入口文件，确认需要打通的 parser/typecheck/HIR/MIR/codegen 路径与 gate/diagnostics。
- 已检查最近一次提交：`[P4-T01b] Unify interface dispatch metadata for body methods`，没有额外标注与 `P4-T01c` 直接相关的未完成问题。
- 已阅读 `PLAN.md` 与 `P4-T01c` 实现入口，确认当前真正的前置阻塞不在 parser/typecheck 主干，而在测试基础设施：`P4-T01c` 明确要求用 task-specific `core.scoop` fixture 重写 `scoop.core.Array` / `MutableArray`，但现有 fixture / CLI 路径始终强绑定默认 sysroot，无法 overlay 默认 `core.scoop`。
- 结论：如果继续实现 `P4-T01c` 主代码而把验证改成其他 FQN（如 `Container<T>`）或仅做 Rust owner test，会直接缩窄 `TODO.md` 规定的 fixture 形状，属于被明确禁止的 workaround。
- 处理动作：已在 `TODO.md` 中插入新的前置任务 `P4-T01c-pre1`（sysroot overlay fixture 能力），并把 `P4-T01c` 的依赖改为该新任务；本次按阻塞处理停止，不继续实现 `P4-T01c` 主体代码。

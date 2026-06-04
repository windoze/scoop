# T2-08 执行计划

## 当前任务

- 首个未完成任务：`T2-08：lowering 产出 LIR 指令（state 拥有指令）`。
- 任务目标：让 effect-lowering 阶段产出并保存 LIR 自有指令序列，替代 `LateLoweredSourceBody` / `LateLoweredStateSlice` 对 MIR body/slice 的回指；遇到 MIR 占位或未解析构造必须报错，不能在 LIR 中保留占位。
- 当前状态：发现 `T2-08` 需要先补齐 LIR-owned executable body 容器，已在 `TODO.md` 插入前置任务 `T2-08A`；本次调用将只提交该任务重排/阻塞记录并停止。

## 执行步骤

1. 检查最近提交和工作区状态，确认无直接相关未完成提交需要并入当前阻塞处理。
2. 阅读 `ir.rs`、`instruction.rs`、`segment.rs`、`builder.rs`、`materialize/*` 与 LLVM body codegen 消费路径，确认 `T2-08` 的严格删除条件。
3. 在 `TODO.md` 中新增最小前置任务 `T2-08A`，并把 `T2-08` 显式依赖它。
4. 记录阻塞原因：缺少 LIR-owned callable header/local/plain body 容器会导致删除 `LateLoweredSourceBody` 后无自包含 body 可供 codegen 消费。
5. 因本次仅修改 `TODO.md` 和 `memory/claude_plan.md`，不运行格式、clippy、测试和 fixture；在记录中说明跳过原因。
6. 检查 diff 与 git 状态，提交本次任务重排/阻塞记录，然后停止。

## 进度记录

- 2026-06-04：已确认首个未完成任务为 `T2-08`，准备检查代码与实现入口。
- 2026-06-04：已检查 `ir.rs`、`instruction.rs`、`segment.rs`、`builder.rs`、`materialize/*` 与 LLVM body codegen 消费路径。发现 `T2-08` 严格执行前缺少一个具体前置：T2-07 只定义了 state 指令外壳，尚无 LIR-owned callable body/header/local table 与 plain body 表示；如果直接删除 `LateLoweredSourceBody` / `crate::mir::Body`，plain callable 和现有 codegen 无法从 LIR program 取得 locals、params、普通 CFG/body 终结符。按任务规则，将在 `TODO.md` 中插入最小前置任务并停止。

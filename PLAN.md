# Scoop：下一轮计划（核心语言 / codegen 优先，Task 次之）

> 生成时间：2026-04-18  
> 历史归档：`PLAN-4.md` / `TODO-4.md`  
> 依据：`ISSUES.md` 当前审计结果  
> 本轮主题：先收口核心语言与 codegen 缺口，再推进 effect 完整性与 `Task` 设计；executor framework 明确留到下一阶段。

## 0. 工作原则

- 本轮严格按 `ISSUES.md` 指定顺序推进，不提前展开后续条目。
- 每个 issue 至少要完成四件事：实现、回归用例、必要的规范/文档同步、复审。
- 未完成前一条 issue 前，不开始后一条 issue 的实现任务；允许在同一条 issue 内拆子任务，但不得跨条目并行推进。
- 本轮的目标是“核心语言 / lowering / codegen 收口优先”。只有前七项完成后，才进入 effect / `Task` 两项。
- `Continuation<T, eff E>` 视为 advanced API；`Task<T>` 视为 general API。
- executor、wakeup、queueing、work-stealing、spawn scheduling 统一留到下一阶段；本轮不把它们纳入完成标准。

## 1. 顺序总览

1. `ISSUES.md` 第 5 条：泛型约束、参数化超类型与 star projection
2. `ISSUES.md` 第 3 条：lambda 推断与 receiver lambda
3. `ISSUES.md` 第 4 条：调用语义早期门禁
4. `ISSUES.md` 第 6 条：顶层 pattern binding
5. `ISSUES.md` 第 13 条：Elvis `?:` lowering / codegen
6. `ISSUES.md` 第 14 条：跨文件 / 跨包编译链路
7. `ISSUES.md` 第 15 条：RTTI 对泛型 / `eff` 参数化类型的支持
8. `ISSUES.md` 第 1 条：effect / continuation 完整性
9. `ISSUES.md` 第 2 条：`Task` 设计与 pollable object 语义

## 2. 分阶段目标

### P1. 类型系统与子类型关系收口

- 先解决参数化 nominal bound、参数化超类型、star projection。
- 目标是把 generic subtype / assignable / lowering 的基础打稳，避免后续 lambda、调用与跨文件实例化继续叠加在不稳定的类型关系上。

### P2. 表达式推断与调用语义收口

- 依次补齐 lambda 推断、receiver lambda、函数值 / funptr / constructor delegation 的调用语义缺口。
- 目标是把前端最常用的表达式与调用规则统一到同一条类型检查主线上。

### P3. 语法到 lowering 的缺口收口

- 顶层 pattern binding 与 Elvis `?:` 进入可执行 lowering / codegen。
- 目标是把“语法 + typecheck 已存在，但 lowering / codegen 不完整”的 feature 清掉，避免继续堆积半实现特性。

### P4. compilation-unit 与 runtime type info 收口

- 跨文件 / 跨包 compilation chain 与 RTTI 参数化支持放在同一阶段处理。
- 目标是先让语言规则跨 compilation unit 一致，再补运行时类型描述符对泛型 / `eff` 的覆盖。

### P5. effect 完整性收口

- 在核心语言与 codegen feature 收口后，再回头补 effect / continuation 剩余缺口。
- 目标不是扩 executor，而是把手动 stepping `Task` 所需的 effect 语义补完整，包括更自然的多 suspend 组合、continuation 类型语义与相关 lowering。

### P6. `Task` 设计定型

- 只聚焦 `Task<T>` 本体：pollable object、manual stepping、private continuation state、advanced `Continuation` 隐藏边界。
- 不在本轮定义 executor interface、wakeup API 或 `spawn` 最终调度模型。

## 3. 各阶段完成标准

### C1. 前七项核心语言 / codegen 条目

- 对应 `ISSUES.md` 条目已被关闭，或至少收缩为新的、更窄的剩余 blocker。
- 新增或更新的 fixtures 覆盖 typecheck、HIR / MIR / LLVM lowering、run-pass 或相关 regression。
- 若规范文字被实现改变或澄清，需同步 `SCOOP_FULL_SPEC.md`，必要时同步相关 runtime / sysroot 文档。

### C2. effect / continuation 条目

- 明确区分“已足够支撑 `Task` manual stepping 的能力”和“仍未覆盖的 richer effect 语义”。
- 去掉阻碍 `Task` 设计落地的主要 effect/codegen 限制，尤其是 continuation 组合与相关 lowering 缺口。

### C3. `Task` 条目

- `Task<T>` 的通用 API 形状、`Poll<T>` 合同与 manual stepping 语义要固定下来。
- raw `Continuation` 不应继续作为通用 async API 暴露；若仍需保留，只能作为 advanced API。
- executor 仍可留白，但 `Task` 本体不再依赖 executor-centric 的叙事才能成立。

## 4. 非目标

- 本轮不完成 executor framework。
- 本轮不定义 work-stealing、event loop、I/O driver、waker、queueing 或 `spawn` 的最终调度语义。
- 本轮不扩展与上述九项无直接关系的 stdlib surface。

## 5. 最终验收

- `PLAN.md` 与 `TODO.md` 中本轮任务已按顺序推进并留下明确结论。
- 相关实现通过必要的定向测试；阶段收口时复验 `cargo test --all` 与 `cargo run -p scoop -- test`。
- 若修改了 `SCOOP_FULL_SPEC.md` 中带 fixture 的代码块，还需执行 `cargo run -p scoop_tools -- spec-fixtures check`。

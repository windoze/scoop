# 当前执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新；不记录私有推理链。

## 初始步骤

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项；仅在其阻塞当前任务时纳入范围或作为前置任务记录。
3. 阅读当前任务要求、依赖、验证要求和完成记录，必要时查看相关代码与测试。
4. 若任务可直接完成，实施最小正确变更并补充或更新相关测试/fixture。
5. 运行与任务相关的验证；若发现未显式排期的失败测试/fixture，修复或在 `TODO.md` 中加入最小必要前置任务。
6. 完成后更新 `TODO.md`：在任务标题前加 `[DONE]`，并补充完成记录。
7. 仅当阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。
8. 运行最终验证，检查 git diff/status/log，提交本次任务相关变更，然后停止。

## 当前状态

- 状态：已读取 `TODO.md` 并识别首个未完成任务为 `P9-T06-a`（收窄 LIR 的 HIR/AST source payload 边界）。
- 最新提交 `bae83ccb [P9-T06] Schedule LIR boundary prerequisite` 与该前置任务直接相关；按任务顺序继续执行 `P9-T06-a`。

## P9-T06-a 执行计划

1. 读取 `TODO-7.md` 中 `P9-T06-a` 的详细要求、依赖、验证命令与完成记录格式。
2. 搜索 LIR 及相关 facts/stage 中仍直接携带 HIR/AST source payload 的类型、字段和调用点。
3. 用更窄、可跨 crate 消费的 source/location 摘要或 ID 边界替代不必要的 HIR/AST payload；避免 fixture-only 或任务私有特殊处理。
4. 更新受影响的代码、测试和文档索引，保持 crate 依赖方向符合 P9 拆 crate 目标。
5. 运行任务要求的验证和必要的补充验证；若发现未排期失败，修复或在 `TODO.md`/`TODO-7.md` 中加入最小前置任务后停止。
6. 通过后将 `P9-T06-a` 在 `TODO.md` 和 `TODO-7.md` 标记为 `[DONE]`，补充完成记录。
7. 检查工作区 diff/status/log，提交本任务全部相关变更并停止。

## 进度更新

- 已确认当前 LIR residual 集中在 `effect_lowered::ir::source` 的整包 HIR re-export，以及 `builder.rs` 中直接匹配 `crate::ast::CtorDelegationKind`。
- `crates/scoopc_mir/src` 仍按 MIR stage 设计直接依赖 HIR/AST；这不应作为 P9-T06 的 full-transitive 失败依据。本任务将把 P9-T06 的验收口径修正为 `scoopc_lir` direct dependency 不含 HIR/AST/umbrella，同时用 source-boundary gate 防止 LIR 源树重新直接命名 HIR/AST。
- 实施方向：在 `scoopc_mir::mir` 发布窄的 class ctor/source payload surface，并让 LIR 只通过该 surface 构造 `LateLoweredClassCtorInitBody`。
- 已完成实现：`scoopc_mir::mir` 发布 `source_payload` 与 class-ctor payload surface；`effect_lowered` 的 class ctor 构建改走 `class_ctor_source`；`dependency_gate` 增加 LIR direct-dependency 规则和当前/未来 LIR source-boundary 规则。
- 已验证通过：`cargo fmt`、`cargo build --workspace`、`cargo test --all --all-targets`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets -- -D warnings`，以及 LIR residual 搜索无命中。

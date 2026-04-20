# 本轮计划（2026-04-20，本次执行）

## 说明

- 这里记录的是可公开的决策摘要、约束、执行步骤和阶段性结论。
- 不记录内部逐词推理；只保留后续核查进度所需的信息。

## 目标

本轮只处理 `TODO.md` 中第一个未完成任务，并在完成后停止。当前暂定的首个未完成任务仍可能是 `T4010a2`，但必须先检查最新提交是否提到任何需优先修复的既有问题，并重新核对 `TODO.md` / `PLAN.md` / 工作树状态。

## 决策依据摘要

- 需要遵循用户流程：先修最新提交中明确提到的既有问题，再处理 `TODO.md` 首项。
- 如果首项任务过大，必须先拆分并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一项。
- 如果实施途中发现真实前置能力缺口或规格不匹配，不能绕过，必须把缺口前移成新的任务，更新计划并提交后停止。

## 执行步骤

1. 检查最新提交说明，确认是否提到了需优先修复的既有问题。
2. 核对 `TODO.md`，确认第一个未完成任务确实是 `T4010a2`；同时阅读 `PLAN.md` 获取当前任务拆分与依赖背景。
3. 检查工作树状态，确认是否存在未提交变更；若有，谨慎判断是否为当前任务相关内容，避免覆盖用户改动。
4. 阅读与 `with`、enum payload、member access、类型检查和 lowering 相关的实现与测试，明确当前语义边界：
   - parser / AST 中 `with` 路径如何表示；
   - typecheck 当前如何为 `with` 记录路径前缀的 aggregate 类型；
   - lowering 当前如何根据 aggregate 类型重建 struct/tuple；
   - enum payload 现有的字段/位置访问在 parser、typecheck、lowering 中是否已有统一主线。
5. 判断 `T4010a2` 是否可以在本轮完整闭环：
   - 如果语义可以清晰落地，则直接实现、补测试、跑验证；
   - 如果发现缺少明确前置能力或存在规格不一致，则按用户要求修改 `TODO.md` / `PLAN.md` 重新排依赖，并提交后停止。
6. 若实施：
   - 扩展类型检查，让 enum payload 的 `with` 有明确且可诊断的静态规则；
   - 扩展 lowering，在不破坏“base 只求值一次”和冲突检测的前提下，支持 enum payload 更新；
   - 增加 fixture / 单测覆盖成功路径、错误路径、嵌套路径与单次求值语义。
7. 运行必要验证，至少覆盖：
   - 相关单测；
   - 相关 fixture 套件；
   - `cargo test --all`；
   - `cargo clippy --all-targets -- -D warnings`。
8. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成情况或阻塞原因。
9. 提交本轮改动，提交信息使用任务号前缀；然后停止。

## 预期输出

- 若任务可完成：代码、测试、计划文档与任务状态一并更新，并形成单个提交。
- 若任务不可直接完成：补充前置任务、重排依赖、更新计划文档，并形成单个提交。

## 当前待确认点

- 之前的计划文件中已经记录过一次针对 `T4010a2` 的探索结论，但本轮必须重新核对它是否已被正式落到 `TODO.md` / `PLAN.md` / Git 历史中，而不能直接假定该结论仍然成立。
- 若最新提交已处理完上述 blocker，则本轮应继续推进后续首项任务；若未处理，则需要以仓库当前状态为准。

## 执行中更新

- 已检查最新提交 `3d4f542 [T4010a2a] Reorder enum with-update blockers`：
  - 提交本身没有留下“顺手待修的小问题”；它做的是把一个真实前置 blocker 正式前移到 `TODO.md` / `PLAN.md`。
  - 因此“先修最新提交提到的既有问题”在本轮等价于先执行 `T4010a2a`。
- 已核对 `TODO.md`：
  - 第一项未完成任务确认为 `T4010a2a`。
  - `T4010a2b` 明确依赖 `T4010a2a`，本轮不得越过。
- 已核对工作树：
  - 当前唯一未提交改动是本轮对 `memory/claude_plan.md` 的更新，没有发现其他用户改动需要避让。
- 下一步：
  - 阅读 `when` pattern typecheck / HIR lowering / LLVM enum payload codegen；
  - 用最小 probe 复现 `enum payload (non-scalar)`；
  - 判断缺口落在 payload 提取、binder 表示、还是 `when` 分支 codegen。
- 已复现并收窄真实根因：
  - `Ok(val point: Point)` 的最小 probe 先证实了 `enum payload (non-scalar)`；
  - 进一步分离构造/解构后确认，问题并不只在 `when`，还包括单字段 aggregate payload 的 enum ctor codegen；
  - 再继续下钻后确认，tuple payload 还暴露出第二条老假设：HIR layout 侧只保留 `ty_fqn`，而 tuple 这类字段没有稳定 FQN，LLVM 无法恢复其真实 `CgTy`。
- 已采用的实现方案：
  - 不发明新的 inline aggregate payload ABI；
  - 直接把“单字段但字段类型是 struct / tuple 的 variant”纳入现有 boxed-variant 主线；
  - 同时让 HIR `StructFieldLayout` / `EnumVariantFieldLayout` 保留字段真实 `TypeId`，供 LLVM 后端优先恢复 tuple / nullable 等无 `ty_fqn` 字段类型。

## 本轮结果

- `T4010a2a` 已完成。
- 代码层面：
  - typecheck/layout 与 LLVM enum layout 的 boxing 判定已统一；
  - 单字段 struct payload / tuple payload 不再落入 `enum payload (non-scalar)` unsupported；
  - `when` 与局部 variant binder 都能在统一主线上提取这类 payload。
- 新增回归：
  - `tests/fixtures/run-pass/enum_variant_non_scalar_payload_basic.scoop`
  - `crates/scoopc/src/llvm/mod.rs` 单测 `enum_single_field_non_scalar_payload_uses_boxed_variant_path`
- 已完成验证：
  - `cargo test -q -p scoopc enum_single_field_non_scalar_payload_uses_boxed_variant_path -- --nocapture`
  - `cargo run -q -p scoop -- test --fixtures /tmp/t4010a2a-fixtures/run-pass`
  - `cargo run -q -p scoop -- build /tmp/t4010a2a_probe.scoop -o /tmp/t4010a2a_probe.out && /tmp/t4010a2a_probe.out`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 文档状态：
  - 待提交前已同步更新 `TODO.md` 与 `PLAN.md`，当前下一项已切换到 `T4010a2b`。

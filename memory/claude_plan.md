# 当前执行计划

## 说明

根据仓库要求与本次任务约束，本文件记录的是可审计的高层推理摘要、执行计划、关键决策与进度更新，不包含逐字内部思维过程。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交信息是否提到任何已知问题、遗留修复或需先处理的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如果该任务过大或存在前置依赖缺失，先阅读相关上下文，必要时在 `PLAN.md` / `TODO.md` 中拆分任务、补充依赖并重排顺序。
4. 仅在前置问题已处理完毕后，实现当前应执行的首个任务。
5. 运行相关测试，并补足必要测试，确保实现符合规范。
6. 更新 `TODO.md`、`PLAN.md` 与本文件的状态记录。
7. 提交本轮变更，随后停止。

## 当前已知约束

- 必须先检查最新提交中提到的既有问题；如有，需优先修复。
- 不能以规避、临时兼容或仅夹具通过的方式交付任务。
- 若发现规范缺口或前置能力缺失，必须把真实问题加入 `TODO.md` 并调整依赖顺序，而不是绕过。
- 需要尽量保证编译、测试、lint 无告警。

## 待确认项

- 最新提交是否声明了未解决问题。
- `TODO.md` 中第一个未完成任务的内容、范围和依赖。
- 当前工作树是否存在未提交改动，需要在执行中避免覆盖。

## 进度记录

- 已创建本计划文件，准备开始检查提交记录与任务列表。
- 已检查当前工作树：仅本文件有未提交修改。
- 已读取最新提交与任务列表；当前第一个未完成任务为 `T3011`。
- 最新提交信息未直接声明新的未修问题；`PLAN.md` 中提到的 `T3017` stale `EXPECT: fail` 仍是已跟踪后续任务，不改变本轮首要任务顺序。
- 已完成首轮代码勘察：
  - `FrameSlot.mutable` 在 emitter / `codegen_assign_stmt` 中会直接决定是否允许赋值。
  - `HandlePlanContext::from_codegen()` 生产态只从当前 `cg.env` 收集局部元数据，不会主动并入当前 `handle` 内部声明。
  - `build_stmt(Val)` 对已有 slot 使用 `or_insert_with`，若先前已被错误占坑，则声明点不会回填 authoritative 元数据。
- 已完成首轮复现：
  - `TODO.md` 点名的验收 fixture `tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop` 当前已通过。
  - 扫描当前 `EXPECT: fail` run-pass 子集，未直接复现 `assignment to immutable local`。
- 当前判断：
  - `T3011` 更像“消除生产态 slot metadata 的潜在顺序依赖并补结构回归”，而不是单一现成 fixture 修复。
  - 下一步将补定向单测，覆盖“handle 内声明元数据进入生产 context”与“后续声明覆盖先前占坑 slot”这两条链路，然后实现对应修复。
- 已完成实现：
  - `build_unified_lowering_contract()` 现在会在生产态 `from_codegen()` context 基础上，补充当前 `handle` 自身的 local metadata。
  - `build_stmt(Val)` 遇到真实声明时会直接覆盖同 `SymbolId` 的旧 slot，消除“先占坑后永不回填”的顺序依赖。
- 已新增并通过的结构回归：
  - `declared_handle_local_overwrites_placeholder_slot_metadata`
  - `handle_context_extension_recovers_nested_handle_outer_var_mutability`
- 已完成验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop` 通过。
  - 扫描当前 `EXPECT: fail` run-pass 子集，未发现 `assignment to immutable local`。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo run -p scoop --features llvm -- test` 仍只停在既有 stale `EXPECT: fail`：`tests/fixtures/run-pass/continuation_resume_continuation.scoop`，与已跟踪的 `T3017` 一致，未出现新的更早失败点。
- 已更新 `TODO.md` / `PLAN.md`：`T3011` 标记为完成，下一项未完成任务已推进到 `T3011R`。

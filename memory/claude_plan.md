# 执行计划

说明：按安全与隐私要求，这里记录的是可审计的执行计划与进度，不包含内部推理细节。

## 初始计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并把它视为本次唯一目标。
2. 检查最近提交是否直接提到与该任务相关且未完成的问题；如果是，则将其并入当前任务范围，或在 `TODO.md` 中记录为当前任务的前置依赖。
3. 阅读任务条目中列出的要求、依赖、验证方式与完成记录，并据此定位相关代码与测试。
4. 以最小正确改动完成该任务；如果遇到阻塞当前任务的真实缺口或缺陷，不做变通，而是在 `TODO.md` 中加入最小必要前置任务并停止。
5. 运行与该任务直接相关的验证；如有需要，再运行更广泛的测试与质量检查，直到结果满足任务要求。
6. 更新 `memory/claude_plan.md` 记录关键进展；将任务在 `TODO.md` 中标记为 `[DONE]` 并填写完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 按仓库约定创建一次提交，然后停止，不继续处理下一个任务。

## 进度记录

- 已创建本文件并记录初始计划。
- 2026-05-08：读取 `TODO.md` 后确认首个未完成任务为 `CG-T07S：修复 full-suite cross-fixture transport metadata drift，解除 CG-T08 默认回归阻塞`。当前计划：先检查最近提交是否直接提到该任务相关未完成问题；随后复现 full-suite 与单 fixture 的差异，定位 transport metadata 漂移来源，再实施最小正确修复并完成任务要求的验证、文档更新与提交。
- 2026-05-08：检查 `git status` 与 `git diff` 后确认工作区已有一批与 `CG-T07S/CG-T08` 直接相关的未提交改动，包括新增 `CG-T07S` 任务条目、fixture runner session 回归测试、MIR/materialize/type canonicalization 相关代码与更新后的 snapshot。后续会在这些现有改动基础上继续完成任务，并在最终提交时一并纳入。
- 2026-05-08：已复现当前状态：单跑 `aggregate_transport.scoop` 通过，但 `cargo run -p scoop -- test` 仍在同一 fixture 的 `.mir` snapshot 首次失败；新加的 session-isolation 回归测试也失败，但失败样本落在 `generic_materialization.scoop`。综合当前代码，最可疑点是 `crates/scoopc/src/mir/materialize.rs` 中新增的 transport repair 仍会从 `local.ty` / `payload_transport.source_ty` 反推 array/handle/perform metadata，而这些字段本身可能在不同编译上下文中先发生漂移。接下来将把 repair 改为只消费 authoritative contract（`array_ty`、closure `env_ty`、handle/perform payload schema），并把 mixed-phase 回归测试聚焦到 `aggregate_transport` 场景。
- 2026-05-08：在 mixed-phase 回归测试中仍复现 `aggregate_transport.mir` 第 1939 行漂移，而且该 fixture 属于 direct-style MIR snapshot；`RefactorMirStageOutput::stable_dump()` 输出的是 `lowered_mir.file`，并不经过 `mir/materialize.rs`。因此当前主因更可能在 `crates/scoopc/src/mir/lower.rs` 或更上游的 typed-HIR/effect-contract producer，而不是 materialized MIR repair。本轮接下来会把排查重点切到 direct-style MIR transport metadata producer。
- 2026-05-08（本轮继续）：已再次确认首个未完成任务仍是 `CG-T07S`。本轮执行顺序：1）先检查最新提交与当前工作区，确认是否存在与 `CG-T07S` 直接相关且尚未完成的遗留改动；2）复现 `aggregate_transport.scoop` 在单跑与 full-suite 下的差异，并以最小范围读取相关 MIR/fixture runner/typed-HIR 代码；3）修复 cross-fixture transport metadata drift 的真实根因，要求 metadata repair/producer 只消费 authoritative contract，不依赖顺序敏感局部类型或跨 fixture 状态；4）补齐或调整最小回归测试，覆盖 fixture session 隔离与 full-suite snapshot 一致性；5）运行 `CG-T07S` 列出的验证以及必要的 `fmt`/`clippy`；6）若任务完成，则把 `CG-T07S` 标记为 `[DONE]`、填写完成记录并创建一次提交后停止。若发现新的真实前置阻塞，则只更新 `TODO.md`/必要计划并提交后停止。
- 2026-05-08：本轮复现结果更新：`cargo test -p scoop run_all_recreates_session_between_independent_fixtures` 与 `cargo test -p scoopc refactor_mir_stable_dump_canonicalizes_type_ids_by_structure` 当前均通过；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop` 和 `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor` 也通过。根目录 `cargo run -p scoop -- test` 仍失败，但失败点已转移到 `tests/fixtures/mir_refactor/generic_materialization.scoop`，首个不一致行表现为 raw `TypeId` 数字漂移。基于现状，下一步优先补完 `RefactorMirStageOutput::stable_dump()` 的 type-id canonicalization，并用更严格的单测验证“相同类型结构、不同 interning 顺序”时 stable dump 输出一致。
- 2026-05-08：已查明并修复两条与 `CG-T07S` 直接相关的问题：1）`scoop test --fixtures <mir_refactor 单文件>` 的 phase fallback 之前漏掉 `mir_refactor`，导致验证命令误走 parse phase；2）`MirLoweringFacts::with_member_value_types()` 会把 mangled generic class/struct instance clone 的具体字段类型覆盖到 base field FQN 上，导致 `generic_materialization.actual.raw.mir` 中 `holder.item` 有时被 lower 成模板 `T`、有时成具体 `Int`，进而让 `Transport(Int -> T)` 语句随机出现/消失。修复后 `tests/fixtures/mir_refactor`、`tests/fixtures/effect_facts`、`tests/fixtures/effect_lowered` 目录定向验证恢复通过。
- 2026-05-08：当前 `CG-T07S` 的最终 blocker 已变化。snapshot drift 本身已基本收口，但默认 `cargo run -p scoop -- test` 继续暴露 `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop` 的 run-pass/codegen 回归；build 诊断定位到 `CallKind::FunValue` receiver + named-arg lowering 把 `Int` 实参误送到 receiver `String` 槽位。该问题与 `CG-T07S` 不是同一 execution unit，已在 `TODO.md` 中补录为前置任务 `CG-T07S0`，本轮将在更新 TODO/记录后停止并提交。

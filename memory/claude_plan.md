# 执行计划

## 当前目标

- 以 `TODO.md` 作为任务顺序和完成状态的权威来源。
- 只完成第一个未完成任务，然后停止。
- 持续记录关键进展、计划变化、阻塞、验证结果和完成状态。

## 执行步骤

1. 先读取 `TODO.md`，找到标题未带 `[DONE]` 的第一个任务。
2. 只检查与该任务相关的近期 git 上下文，包括最新提交是否记录了直接相关的未完成工作。
3. 阅读该任务的细节、依赖、验证要求和相邻完成记录。
4. 只检查正确完成该任务所需的代码、测试、fixture 和文档。
5. 用最小且符合规格的改动完成任务，不引入 workaround。
6. 先运行格式化，再运行 lint，再运行相关测试，代码变更后执行要求的全量验证。
7. 若观察到未被计划覆盖的测试或 fixture 失败，先修复或补充最小前置任务，再标记当前任务完成。
8. 在 `TODO.md` / 子计划中给完成任务标题加 `[DONE]`，并记录实现和验证结果。
9. 更新本文件的最终状态和验证结果。
10. 用带任务编号的清晰提交信息提交所有相关变更。
11. 停止，不开始下一个任务。

## 进度记录

- 已在任务发现前初始化本执行计划。
- 已读取 `TODO.md` 和 `TODO-3.md`，确定第一个未完成任务为 `T3-04F0`。
- `T3-04F0` 范围：修复 `T3-04F` 改造后遗留的 fixture、golden 与 runtime 失败，同时不削弱 fact-only 契约。
- 最新提交 `[T3-04F] Schedule fixture follow-up` 与当前任务直接相关，作为本任务上下文处理。
- 工作区检查发现本次修改了 `memory/claude_plan.md`，另有无关未跟踪 `FACT_REFACTOR.md`；该设计笔记不属于当前任务，未修改。
- 首次完整 fixture suite 复现 4 个失败：`aggregate_transport`、`call_contracts`、`generic_materialization` 的 MIR golden mismatch，以及 `run-pass/sysroot_atomic_basic`。
- MIR mismatch 来自新增构造器元数据 `target_init_class_fqn`，确认语义后应刷新 golden。
- `sysroot_atomic_basic` 的根因是 `AtomicValue<Pair>` 构造时内部 `Atomic<Box<Pair>>` 初始化走了字符串拼接构造器 FQN，导致 result type 与 nominal owner 不匹配。
- 已修改 `call/lowering.rs`：source class ctor 检测改用 monomorphic `TypeId` 推导已注册 `ClassInstanceKey`，并删除 `scoop.core.{name}<arg>` 字符串构造 fallback。
- 已重建 `scoop` / `scoopc`，并确认 `run-pass/sysroot_atomic_basic.scoop` 单独通过。
- 已刷新 3 个受影响的 MIR lowered golden，并确认 `aggregate_transport`、`call_contracts`、`generic_materialization` 单独通过。
- `dependency_gate.py` 随后暴露 `intrinsics/builtin.rs` 中残余的 reflection HIR source-text fallback；已删除 `current_source_slice(span)` 类型实参解析，使旧 HIR reflection 路径不再解析源码文本。
- 完整 fixture suite 随后暴露 `run-pass/reflection_kind_desc_basic`；顶层 reflection initializer 仍需要发布的类型实参事实。已扩展 `EffectAnalysisFacts`，从 HIR 已发布合同复制 reflection call metadata，并让 LLVM reflection lowering 消费该事实面。
- 已重建并确认 `run-pass/reflection_kind_desc_basic.scoop` 单独通过。
- 最终验证全部通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（1664 checks）。

## 本次 T3-04F 收口

- 已识别当前第一个未完成任务：`TODO-3.md` 的 `T3-04F`。
- 最新提交为 `[T3-04F0] Fix residual fixture failures`，它是 `T3-04F` 的前置修复提交；当前收口复用其已通过的完整验证结果。
- 工作区除本计划文件外存在未跟踪 `FACT_REFACTOR.md`，暂不修改。
- 已定向检查 `T3-04F` 残留路径，并重跑 `python3 tools/dependency_gate.py` 通过。
- 已将 `TODO-3.md` 中 `T3-04F` 标记为 `[DONE]` 并补充完成记录；已将 `TODO.md` 当前活跃任务更新为下一项 `T3-04R`，但不会执行下一项。
- 因本次收口只修改任务记录和本文件，完整 Rust/fixture 验证复用最新 `[T3-04F0]` 提交记录中的全绿结果。
- 已检查差异；下一步提交本次任务记录收口。

## 本次 T3-04R 审查

- 已在执行项目命令前更新本文件，记录本次可审阅执行计划；不写入隐藏推理过程。
- 已读取 `TODO.md` 和 `TODO-3.md`，确定第一个未完成任务为 `T3-04R`（Review T3-04）。
- 最新提交为 `[T3-04F] Complete fallback closure record`，与 `T3-04R` 直接相关，因为该 review 依赖 `T3-04F`。
- 审查发现 `T3-04F` 后仍有阻塞缺口：P6 HIR source-site bridge、reflection span lookup、class ctor fallback、LIR bodyless source-signature/ABI synthesis、effect target verifier 缺口、P6 generic/FQN/dispatch/value-box fallback，以及 dependency-gate 覆盖漏洞。
- 已在 `TODO-3.md` 插入新的前置任务 `T3-04G`，将 `T3-04R` 依赖改为 `T3-04G`，并在 `TODO.md` 将当前活跃任务更新为 `T3-04G`。
- `PLAN.md` 未更新，因为批次级计划、阶段依赖和完成标准没有变化。
- 本次只修改任务记录和本计划文件，未改编译产物；不运行格式化、lint 或全量测试。

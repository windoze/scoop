# 执行计划

## 约束说明

- 本文件记录可审查的执行计划、决策依据和进度更新；不会记录隐藏推理过程。
- `TODO.md` 是任务顺序、要求、依赖和完成状态的唯一来源。
- 本次调用只完成第一个未完成任务；完成后更新 `TODO.md`、验证、提交并停止。

## 初始步骤

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务。
2. 查看最新提交信息，确认是否有与该任务直接相关的未完成事项。
3. 读取当前任务涉及的计划和源码上下文，只做与当前任务相关的调查。
4. 如果任务可直接完成，则实现、格式化、lint、测试、更新完成记录并提交。
5. 如果发现当前任务被具体缺失特性或未排期失败阻塞，则在 `TODO.md` 中插入最小必要前置任务，保留当前任务未完成，提交后停止。

## 验证顺序

1. `cargo fmt`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all --all-targets`
4. `python3 tools/run_fixtures.py`

## 进度日志

- 初始化：已创建执行计划文件，下一步读取 `TODO.md` 确认第一个未完成任务。
- 已确认当前任务：`TODO-3.md` 的 `T3-04H`。最新提交 `[T3-04R] Schedule eighth review follow-up` 与该任务直接相关；未跟踪的 `FACT_REFACTOR.md` 不属于本次已确认改动，保持不动。
- 当前执行计划：先定位 `published_print_callable_fqn`、`published_hir_generic_callable_fqn`、`class_ctor_init_bodies` fallback、`scalar_intrinsic_entry_from_fqn`、`materialized_value_box_member_impl_fqn` 等残余路径；再按 fact-only/fail-fast 要求改实现并补 gate/verifier；最后执行规定验证、更新 `TODO-3.md`/索引并提交。
- 已完成定位：残余入口集中在 LLVM direct-call lowering、class ctor lowering/MIR ctor args、MIR direct-call lowering、LLVM value-box itable materialization与 dependency gate。下一步分小 patch 删除这些 fallback，并把 value-box itable 的 impl target 发布迁到 LIR facts。
- 已完成主体改造：删除 P6 print/generic direct-call concrete FQN 合成、LLVM class ctor init base-context fallback、MIR class ctor init key fallback、MIR scalar intrinsic FQN fallback；value-box itable 改由 LIR facts 发布，P6 只按 canonical owner key 消费；同时移除 P6 direct_supertypes handoff并补 dependency gate。`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`python3 tools/dependency_gate.py` 已通过。
- Fixture 复查发现 ABI visibility 与最终 codegen 使用不同 fact 集时会漏 class ctor call-site，且 HIR direct-call 旧 root 会导致 print/mutableArrayNew 链接缺符号；已改为 ABI materialization 使用 ABI facts、最终 body codegen 使用 primary facts，并让 HIR direct-call 从已发布 source signature/ABI facts 中唯一选择 concrete root。相关定向 fixtures 已恢复通过。
- 验证完成：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（1664 checks）均通过。已将 `T3-04H` 标记为 `[DONE]`，并把 `TODO.md` 当前活跃任务推进到 `T3-04R`。

# 当前执行计划

## 目标

- 按 `TODO.md` 的顺序识别并完成第一个标题未带 `[DONE]` 的任务。
- 本次调用只完成一个任务，完成后更新任务记录、验证、提交并停止。

## 步骤

1. 读取 `TODO.md`，确认第一个未完成任务及其验证要求。
2. 查看最近提交信息，判断是否有与该任务直接相关的未完成问题需要纳入范围。
3. 根据任务内容读取相关代码、测试、文档，明确最小正确实现范围。
4. 如果发现当前任务被未跟踪的具体前置问题阻塞，更新 `TODO.md` 加入最小前置任务，提交后停止。
5. 否则实现当前任务，避免绕过规范或改变预期语义。
6. 运行格式化、lint、相关测试，并按任务要求运行必要的完整验证。
7. 若发现未安排的测试或 fixture 失败，修复它；若不能在当前任务内修复，则把最小修复任务插入 `TODO.md` 的正确位置并停止。
8. 完成后在 `TODO.md` 中给任务标题加 `[DONE]`，更新 completion record。
9. 提交所有本任务相关改动，提交信息包含任务编号和清晰描述。

## 当前状态

- 初始计划已记录。
- 已读取 `TODO.md` 与 `TODO-3.md`。
- 第一个未完成任务是 `TODO-3.md` 中的 `T3-04C`：收口 `T3-04R` 三次审查发现的 intrinsic/root/declaration ABI/reflection/verifier/gate 残余缺口。
- 最近提交 `[T3-04R] Schedule intrinsic fallback follow-up` 与当前任务直接相关，纳入本任务范围。
- 已开始实现：LIR call-site contract 增加 target-bound binding；LIR builder 删除 named intrinsic root/source-signature 扫描与 layout root ABI 补洞；LLVM intrinsic fact lookup 改为 fact-only；declaration ABI 合成改为 fail-fast；reflection HIR source-slice 解析改为缺契约报错。
- `cargo test --all --all-targets` 首次运行发现 vtable layout target `scoop.core.Bool.toString` 缺 target-bound ABI fact；正在把 layout target ABI 发布改为基于 materialized stable instance 的显式 target binding。
- 完整 fixture suite 首次运行发现 11 个 run-pass 目标失败，集中在 scalar/range/reflection intrinsic。已补充 MIR backend `intrinsic_callables` fact，由 LIR facts 消费该 upstream 显式 metadata，`run-pass/fun_call_add_basic.scoop` 已单独恢复通过。
- T3-04C 实现已完成，最终验证通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/run_fixtures.py`（1664 checks）。
- `TODO-3.md` 已将 `T3-04C` 标记为 `[DONE]`，`TODO.md` 当前活跃任务已推进到 `T3-04R`。

## T3-04C 执行计划

1. 定位 residual fallback 相关实现：`lir_facts_builder`、LIR verifier、LLVM codegen intrinsic/reflection/declaration ABI/dispatch 生产路径，以及 `tools/dependency_gate.py`。
2. 删除或替换 root/FQN/source-slice/text 恢复路径，使生产路径只消费显式发布的 LIR facts/source call-site contract，缺失时 fail-fast。
3. 补齐 verifier：KnownInstance、CandidateSet、dispatch/source-call/declaration-only/native/extern target 均校验 target key、source signature、ABI symbol 一致。
4. 补齐 dependency gate 和回归测试，锁定禁止的 helper/模式。
5. 运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/run_fixtures.py`。
6. 全部通过后更新 `TODO-3.md`、`TODO.md` 状态，提交并停止。

# 执行计划

本文件记录可检查的执行计划、关键决策和进度更新。不会记录隐藏推理细节，但会记录足够的依据、步骤和验证结果，便于审查。

## 当前计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 识别第一个未完成任务。
2. 检查最近提交，仅在其明确提到与该任务直接相关的未完成问题时纳入当前任务或补充为前置任务。
3. 阅读当前任务涉及的代码、文档和测试，确定最小正确实现范围。
4. 实施当前任务；如果发现阻塞当前任务的缺失特性、规格不匹配或未排期失败，优先修复或在 `TODO.md` 中插入最小前置任务并停止。
5. 运行格式化、lint、相关测试，并按要求运行完整测试/fixture 套件或记录可复用的绿色结果。
6. 更新 `TODO.md`：将完成任务标题加 `[DONE]`，补全完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
7. 检查 `git status`、`git diff`、近期提交，提交本次任务相关全部变更，然后停止。

## 进度

- 已创建初始执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已读取 `TODO.md` / `TODO-3.md`，第一个未完成任务是 `T3-04A`。最近提交 `T3-04A0` 是其直接前置，已作为当前任务上下文处理。
- 下一步聚焦 `T3-04A`：删除 P6 direct-call/source-site side table 与 intrinsic FQN fallback，补齐 unpublished target verifier、declaration-only ABI reachability 和 dependency gate 守卫。
- 定向探索确认主要待改点在 `llvm_codegen_stage.rs` 的 intrinsic source-span handoff、LLVM codegen 的 intrinsic contract/context 访问、LIR verifier/ABI reachability 单测覆盖，以及 `tools/dependency_gate.py` 守卫加严。
- 实施计划细化：先移除 `llvm_codegen_stage.rs` 对 `top_level_fun_call_sites` 的 intrinsic metadata 补洞；再删除 P6 三处 `legacy_scalar_named_intrinsic_entry_name` FQN fallback；随后让 LIR ABI facts 为 declaration-only call targets 发布与目标 key 绑定的 ABI fact，并移除 verifier 的 `target.readable_path()` 兜底。
- 已移除 LLVM stage 的 `top_level_fun_call_sites` intrinsic 补洞和 P6 local legacy scalar intrinsic fallback；已改为为 declaration-only target 发布 target-key 绑定 ABI fact，并让 verifier/reachability 不再依赖 readable-path 兜底。
- Fixture 失败定位为两类：旧 `target/debug` 二进制在 schema bump 后未重建，以及 LIR facts 尚未发布 root-level named intrinsic callable metadata。已将 intrinsic callable metadata 纳入 LIR facts，LLVM 从该 fact 查询 named entry；同时保留现有 source-site fact 查询并避免恢复 P6 本地 scalar FQN fallback。
- 最终验证已完成：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/run_fixtures.py`（1664 checks）均通过。`TODO-3.md` 已标记 `T3-04A` 为完成，`TODO.md` 当前活跃任务推进到 `T3-04R`。

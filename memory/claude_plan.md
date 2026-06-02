# 执行计划

## 当前约束

- 以 `TODO.md` 为任务顺序和完成状态的唯一来源；本索引指向 `TODO-3.md`，当前批次仍为进行中。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 当前第一个未完成任务是 `TODO-3.md` 的 `T3-04R`：Review T3-04。
- Review 任务不为方便拆分；只有审查发现当前任务完成条件仍不满足且必须先修复时，才在 `TODO.md` 插入最小前置任务并停止。
- 如遇阻塞当前 review 的规格不匹配、未排期失败测试或事实契约缺口，先修复或排期前置任务，不能把 `T3-04R` 标为完成。
- `PLAN.md` 只在阶段级计划、依赖或完成标准变化时更新。
- 交互和记录使用中文。

## 步骤

1. 已读取 `TODO.md` 与 `TODO-3.md`，确认 `T3-04R` 是当前第一个未完成任务，依赖 `T3-04D` 已完成。
2. 检查最近提交信息；如果它明确提到与 `T3-04R` 直接相关的未完成问题，将其纳入 review 范围或作为前置任务记录到 `TODO.md`。
3. 按 `T3-04`、`T3-04A/B/C/D` 的完成条件和阻塞记录，审查 P4/P5/P6 fact-only、fail-fast、verifier 与 dependency gate 是否仍存在残余缺口。
4. 重点搜索并核对以下类别：source-span call-site metadata 回看、ABI symbol 合成补洞、intrinsic root/FQN fallback、`DynamicFallback`/bodyless target 放行、generic/overload/dispatch 文本恢复、reachability 静默跳过 target、dependency gate 覆盖缺口。
5. 如发现必须修复的缺口，按任务规则添加最小前置任务或直接修复；修复后重新运行格式化、lint、测试和 fixture。
6. 如未发现阻塞缺口，运行 review 要求的验证；若仅文档/TODO 更新且无代码变更，可复用最近绿色结果并记录原因，否则按顺序运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关/完整测试与 `python3 tools/run_fixtures.py`。
7. 更新 `TODO-3.md`：将 `T3-04R` 标为 `[DONE]`，填写完成记录；同步更新 `TODO.md` 中 `TODO-3.md` 状态和当前活跃任务。
8. 更新本文件记录关键进展与最终验证结果。
9. 提交前检查 `git status`、`git diff`、`git log --oneline -10`，确认提交范围包含本轮必要文件且不回退他人改动。
10. 使用清晰提交信息提交本任务，然后停止。

## 当前状态

- 已确认第一个未完成任务为 `TODO-3.md` 的 `T3-04R`。
- 最近提交为 `[T3-04D] Close fourth fallback gaps`，未显式留下额外未完成事项。
- 审查发现 `T3-04D` 后仍有阻塞 `T3-04R` 完成的残余：P6 class ctor 仍携带/查询 `ctor_call_sites` source-span bridge；reflection type args 仍由 LIR facts builder 扫描 HIR `facts.source_sites.call_sites` 并按 `source_path:span` 查询；LLVM direct-call lowering 仍可用 `scalar_bodyless_intrinsic_entry_name` 从 FQN/generic/overload 文本推导 scalar intrinsic entry。
- 已运行 `python3 tools/dependency_gate.py`，当前失败：`crates/scoopc/src/pipeline/lir_facts_builder.rs:1618` 命中 `facts.source_sites.call_sites` 守卫。
- 已在 `TODO-3.md` 的 `T3-04R` 前新增前置任务 `T3-04E`，并将 `T3-04R` 依赖改为 `T3-04E`；`TODO.md` 当前活跃任务同步为 `T3-04E`。
- 本轮按规则保持 `T3-04R` 未完成；下一步检查 diff/status/log 后提交排期变更并停止。

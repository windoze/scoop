执行计划（2026-06-03）

说明：本文件记录可公开的执行计划与进展；不会记录私有推理链路。

1. 读取 `TODO.md`，按文件顺序定位第一个标题未带 `[DONE]` 的任务。
2. 读取该任务相关的上下文、依赖、验证要求，以及必要的 `PLAN.md` 内容。
3. 检查最近提交是否明确提到与当前任务直接相关的未完成问题；若有，将其纳入当前任务或按要求补入 `TODO.md`。
4. 实现第一个未完成任务；若发现阻塞当前任务的缺失特性、规格不匹配或测试失败，优先修复，或把最小必要前置任务插入 `TODO.md` 后停止。
5. 先运行格式化与 lint：`cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`。
6. 在 lint 通过后运行当前任务要求的测试；若需要全量验证，运行 `cargo test --all --all-targets` 与 `python3 tools/run_fixtures.py`，并设置足够超时。
7. 验证通过后，在 `TODO.md` 中将当前任务标题加 `[DONE]`，更新完成记录；仅当阶段级计划变化时才更新 `PLAN.md`。
8. 更新本文件记录关键进展与最终验证结果。
9. 检查 git 状态与 diff，提交本次任务相关的全部变更。
10. 提交后停止，不继续下一个任务。

当前任务：`TODO-3.md` 的 `T3-04R：Review T3-04`。

任务执行计划更新：

1. 检查 git 状态与最新提交，确认是否有直接指向 T3-04R/T3-04E 的未完成问题。
2. 审查 T3-04、T3-04A0、T3-04A、T3-04B0、T3-04B、T3-04C、T3-04D、T3-04E 的完成记录对应实现，重点检查 P4/P5/P6 是否仍存在 side-table 回看、FQN/string fallback、unpublished/missing-owner、source-span bridge、未发布 target 放行或 gate 漏洞。
3. 若审查发现阻塞完成条件的问题，按要求新增最小前置任务并停止；若未发现阻塞问题，运行验证。
4. 验证顺序：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`python3 tools/dependency_gate.py`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
5. 验证通过后，将 `T3-04R` 标记为 `[DONE]`，同步 `TODO.md` 中 `TODO-3.md` 状态为 `DONE` 并更新当前活跃任务，记录完成说明。
6. 提交本次任务相关变更后停止。

审查进展更新：

1. 已确认 `T3-04E` 后仍存在阻塞 `T3-04R` 完成的问题：P6 仍有 `current_call_site(span)` / `source_call_site_id(path+span)` 查询；class ctor/reflection facts 仍由 HIR source-site helper 发布；LIR facts builder 仍能合成 declaration/bodyless ABI symbol 与空 source signature；layout verifier 仍按 root FQN 校验 target；effect facts verifier 未校验 target 是否已发布；dependency gate 未覆盖这些等价路径。
2. 当前 review 不能标记 `[DONE]`。下一步是在 `TODO-3.md` 中新增 `T3-04F`，作为 `T3-04R` 的前置任务，记录上述六次审查阻塞项。
3. 本次只做任务拆分与阻塞记录，不修改生产代码；验证将按文档/任务清单变更处理。

任务拆分更新：已新增 `T3-04F` 并把 `T3-04R` 依赖改为 `T3-04F`；`TODO.md` 当前活跃任务同步为 `T3-04F`。本次停止在任务拆分，不继续修复 `T3-04F`。

验证记录：本次只修改 `TODO.md`、`TODO-3.md` 与 `memory/claude_plan.md`，未修改编译产物或测试代码；已运行 `git diff --check -- TODO.md TODO-3.md memory/claude_plan.md`，无空白错误。未运行 cargo/test/fixture suite。

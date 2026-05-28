# 执行计划

说明：此文件记录可公开的执行计划与进度，不包含隐藏推理链。

1. 读取 `TODO.md`，按标题是否包含 `[DONE]` 判断第一个未完成任务。
2. 检查最近提交与当前任务是否存在直接相关的未完成事项；只处理会阻塞当前任务的内容。
3. 阅读当前任务涉及的代码、规格、测试与完成要求，确认是否需要新增强制前置任务。
4. 按当前任务要求实施最小正确修改，避免变通、绕过或改变预期表示。
5. 运行格式化、lint、相关测试；若代码改动影响面较大，再运行完整测试与 fixture 套件。
6. 若发现未安排的失败测试或阻塞缺口，修复它；若无法在当前任务内正确修复，则在 `TODO.md` 中加入最小前置任务并停止。
7. 任务完成后，更新 `TODO.md`：给任务标题加 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查 git 状态与差异，提交本轮所有相关改动，然后停止，不继续下一个任务。

当前状态：已读取 `TODO.md`。第一个未完成任务是 `P3-T04R`，需要 review `P3-T04` refutable `val` pattern 的实现；最近提交 `ffbd4f81 [P3-T04] Allow refutable val patterns` 与当前任务直接相关。

本轮细化步骤：

1. 读取 `TODO-3.md` 中 `P3-T04` / `P3-T04R` 的任务正文、完成记录与验收要求。
2. 查看 `ffbd4f81` 的变更范围，确认 refutable `val` pattern、mismatch panic、diagnostic 与 fixture 覆盖点。
3. 对照 `SPEC_FIX.md` / 相关代码实现复审是否存在遗漏或 workaround。
4. 若发现问题，直接修复并补充最小相关测试；若发现无法正确修复的阻塞缺口，则按规则更新 `TODO.md` 后停止。
5. 完成后按要求运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关/完整测试和 fixture 验证。
6. 通过后更新 `TODO.md` 与 `TODO-3.md`，将 `P3-T04R` 标记 `[DONE]` 并填写完成记录，然后提交。

进度更新：复审发现 `val` variant pattern 的 `..` rest 参数在 AST/typecheck/lowering 中已有语义，但 parser 的 variant 分支未识别 `Symbol::DotDot`，会让 `val Pair(x, ..) = e` 这类 refutable pattern 解析失败。该缺口属于 P3-T04R review 直接相关范围，下一步修正 parser 并补充运行 fixture。

进度更新：已修正 variant pattern parser 对 `Symbol::DotDot` 的识别，并扩展 `destructuring_val_variant_match_basic.scoop` 覆盖 `val PairResult.Pair(first, ..) = value`。验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo build -p scoop -p scoopc`、targeted P3-T04 fixtures、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`（`fixtures: ok (1558)`）。`TODO.md` 与 `TODO-3.md` 已将 `P3-T04R` 标记为 `[DONE]` 并写入完成记录；下一步检查 git diff/status 并提交。

进度更新：git status 显示 `run_agent.sh` 与 `GC_IMMORTAL_FIX.md` 存在非本轮改动/未跟踪文件；它们与 `P3-T04R` 无关，本轮不修改、不暂存、不提交。

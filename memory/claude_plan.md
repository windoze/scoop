# 当前执行计划

## 目标

- 以 `TODO.md` 为唯一任务顺序来源，完成第一个标题未标记 `[DONE]` 的任务，然后停止。
- 不做开放式历史问题扫查；只处理当前任务直接要求、阻塞当前任务或验证过程中暴露且未被明确排期的失败。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录其要求、依赖、验证命令和完成记录格式。
2. 检查最新提交信息是否明确提到与该任务直接相关的未完成问题；如相关，将其纳入当前任务或按要求更新 `TODO.md` 为前置任务。
3. 阅读当前任务涉及的代码、测试、规范或文档，确认最小正确实现范围。
4. 如任务可直接实现，进行最小必要代码或文档修改；如存在无法绕过的缺失特性或阻塞问题，仅更新 `TODO.md` 记录最小前置任务并停止。
5. 按任务要求运行格式化、lint、测试和 fixture 验证；若发现未排期失败，修复或在 `TODO.md` 中插入必要前置任务。
6. 更新 `TODO.md`：将完成任务标题加 `[DONE]`，补全完成记录和验证结果；仅当阶段级计划变化时才更新 `PLAN.md`。
7. 检查 git 状态和差异，提交本次任务相关全部变更，提交信息使用任务编号和简短说明。
8. 停止，不继续处理下一个任务。

## 进度记录

- 已写入初始执行计划，下一步读取 `TODO.md` 定位第一个未完成任务。
- 已定位第一个未完成任务：`P3-T01R`（Review P3-T01 operator gate 语义）。最新提交为 `[P3-T01] Enforce operator modifier gate`，与当前 review 直接相关；下一步审阅该任务说明、提交差异和相关实现/fixture。
- 初步审阅结果：`ModifierSet::operator` 已进入 resolver symbol，`FunSigOwned::is_operator` 已进入 top-level/member signature；`ops.rs` 的 unary/binary/comparison operator-positioned 路径会在匹配前调用 `filter_operator_positioned_candidates`，普通 member call 未依赖 `is_operator`。下一步确认是否有遗漏入口，并运行 targeted/full fixture 验证。
- Review 发现需直接修正的缺口：二元/比较 operator 路径过滤 `operator` 后仍在本地把多个匹配候选直接判为 ambiguous，未保持现有 most-specific 选择语义。计划在 `ops.rs` 增加小型 most-specific helper，并添加 targeted typecheck fixture 覆盖 `operator` gate 后选择更具体 overload，同时确认普通 named call 仍不受 gate 影响。
- 已修正该缺口：`ops.rs` 现在会在 operator-positioned 二元/比较 overload 多匹配时选择唯一 most-specific 候选；新增 `operator_overload_most_specific_after_modifier_gate_ok.scoop`。已显式重建 fixture 二进制并通过新增 fixture 与现有 operator targeted fixtures。下一步重新运行 clippy、完整 Rust 测试与完整 fixture suite。
- 验证已完成：`cargo fmt`、`cargo build -p scoop -p scoopc`、`cargo clippy --all-targets -- -D warnings`、targeted operator fixtures、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`（`fixtures: ok (1553)`）均通过。`TODO.md` 与 `TODO-3.md` 已将 `P3-T01R` 标记为 `[DONE]` 并补全完成记录。下一步检查 git diff/status 并提交本任务变更。
- 清理排查过程中的多余分支：确认 earlier targeted failure 来自旧 fixture 二进制复用后，移除了 operator specificity helper 中不必要的“同一声明跳过”逻辑，使其与现有 member/ctor most-specific 规则保持一致。下一步重新运行格式化、构建、lint、targeted fixture，并视结果决定是否需要重跑完整验证。
- 清理后已重新验证：`cargo fmt`、`cargo build -p scoop -p scoopc`、`cargo clippy --all-targets -- -D warnings`、targeted operator fixtures、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`（`fixtures: ok (1553)`）均通过。下一步最终检查 intended diff、stage 仅当前任务相关文件并提交。

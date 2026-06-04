# 执行计划

## 当前约束
- 以 `TODO.md` 为任务来源和完成状态的唯一权威。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 不做开放式历史问题扫描；只处理当前任务直接相关或验证中暴露且未被明确排期的失败。
- 验证顺序为 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关测试、必要时全量测试和 fixture。
- 若遇到具体阻塞，不绕过实现；在 `TODO.md` 中加入最小前置任务，提交后停止。

## 步骤
1. 读取 `TODO.md`，确定第一个未完成任务及其要求、依赖、验证条件。
2. 查看最近提交信息，确认是否有与该任务直接相关的未完成事项。
3. 只围绕该任务检查必要的代码、测试、规格和文档上下文。
4. 若任务可直接完成，按最小正确改动实现；若发现必须先修复的具体前置问题，更新 `TODO.md` 并停止。
5. 添加或调整覆盖当前行为的最小相关测试或 fixture。
6. 按要求运行格式化、lint、相关测试；需要全量验证时运行完整 Rust 测试和 fixture 套件并使用足够超时。
7. 将任务标题改为 `[DONE]`，更新完成记录；仅当阶段计划真实变化时才更新 `PLAN.md`。
8. 检查 git 状态和 diff，提交本次任务涉及的所有未提交文件。
9. 停止，不继续下一个任务。

## 进度
- 已创建本计划。
- 已读取 `TODO.md`，第一个未完成任务为 `T2-06-R：Review T2-06`。
- 当前审查重点：layout 是否按 nominal 句柄索引、program 级字段是否正确、`LirFacts` 是否退出 codegen/handoff 或仅剩兼容快照、dependency gate 与基线是否通过。
- 已检查最近提交：最新提交提到 P2c TODO，不是 T2-06-R 的直接未完成项。
- 初步搜索确认 `crates/scoopc_codegen_llvm` 中 `LirFacts` 零命中。
- 初步发现 `LirPhysicalLayoutFacts` 仍使用 `BTreeMap<String, ...>` 保存 class/enum/interface/vtable/itable/layout name/ABI symbol 等 layout 数据；这可能违反 T2-06-R 的 nominal 句柄索引要求。
- 已开始直接修复：新增 `LirNominalLayoutKey`、`LirAbiSymbolKey`、`LirLayoutNameKey`，并将 `LirPhysicalLayoutFacts` 的公开 map key 改为这些显式句柄类型。
- 已同步构造、校验、dump、codegen lookup 与相关单测构造；`cargo check --all-targets` 通过。
- 已完成验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py` 均通过。
- `TODO.md` 已将 `T2-06-R` 标为 `[DONE]` 并写入完成记录。
- 已检查提交前状态和 diff；下一步提交本次任务改动后停止。

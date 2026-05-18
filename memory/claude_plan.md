# 执行计划

## 当前任务
- 首个未完成任务：`U5-T02：spec part1-6 fixture 主体`。
- 任务来源：`TODO.md` 第 613 行开始的 U5-T02 条目。
- 依赖状态：`U5-T01`、`U4-T01`、`U3-T01` 均已标记 `[DONE]`。
- 本轮只完成 U5-T02，完成后提交并停止。

## 约束摘要
- 本轮仍是 doc-and-test only，不修改 production LLVM codegen，不新增 `UnsupportedMainBody` 站点。
- 每个新增 fixture 需要同步 `tests/fixtures/umb_fix/_index.csv` 和 `audit/spec_coverage_matrix.md`。
- fixture 头部必须包含 `EXPECT`、`SPEC`、`COVERS`、`BUCKETS`；negative 还必须包含 `EXPECT-ERROR-CODE`、`EXPECT-ERROR-AT`、`EXPECT-ERROR`、`REASON`。
- C 类 happy-path fixture 在 P7 修复前必须标 `IGNORE-UNTIL-FIX:B-XX`，index 状态为 `ignore-until-fix:B-XX`。
- negative fixture 的 `EXPECT-ERROR` 文案不能包含 forbidden terms：`后端`、`backend`、`LLVM`、`codegen`、`UnsupportedMainBody`。
- 不使用 sysroot 之外未定义的库 API；fixture 之间不互相 import。
- 如发现当前任务被缺失语言能力、runner 缺陷或规格不一致阻塞，不绕开；需要在 `TODO.md` 插入最小前置任务、提交并停止。

## 执行步骤
1. 检查最新提交信息，确认是否显式提到与 U5-T02 直接相关的未完成问题。
2. 读取 U5 相关基线文件：`audit/spec_coverage_matrix.md`、`audit/strategies/B-XX.md` 摘要、`tests/fixtures/umb_fix/_index.csv`、fixture runner 头部解析能力。
3. 确定 48 组 spec-driven fixture 的最小集合，并将每组映射到 spec anchor、bucket、positive/negative 形态和 ignore/active 状态。
4. 编写 fixture 文件。优先采用能被现有 runner 稳定验证的 frontend/typecheck negative；对尚未实现或 C 类 happy-path 使用 `IGNORE-UNTIL-FIX`，不制造未标记 failing fixture。
5. 同步更新 `_index.csv`，确保每个新增 fixture 的 `fixture_path,bucket,kind,spec_anchor,umb_ids,status,notes` 与文件头一致。
6. 同步更新 `audit/spec_coverage_matrix.md`，把 U5-T02 已落地 fixture 从 planned 占位调整为真实 active 或 ignore 路径。
7. 运行 `cargo run -p scoop -- test tests/fixtures/umb_fix/`，并按失败结果修复当前任务相关问题。
8. 运行必要的补充验证；若 U6 草稿测试不存在则记录未运行原因。优先补充 `cargo test -p scoop -- fixtures -- --nocapture` 和 `cargo clippy --all-targets -- -D warnings`。
9. 更新 `TODO.md`：将 U5-T02 标题标记为 `[DONE]`，填写改动范围、核心决策、验证结果和闭合目标。
10. 若执行过程中计划有关键变化或关键步骤完成，更新本文件。
11. 按 Git 提交流程检查状态、diff、日志，提交本轮全部相关变更，然后停止。

## 当前状态
- 已读取 `TODO.md` 并定位 U5-T02。
- 已写入本执行计划。
- 已检查最新提交：`[U5-T01] Add UMB fixture skeleton`，未发现直接阻塞 U5-T02 的未完成问题。
- 已基于 `audit/spec_coverage_matrix.md` 生成 139 个 U5-T02 spec-driven fixture，并同步 `tests/fixtures/umb_fix/_index.csv`。
- 已清理 `audit/spec_coverage_matrix.md` 中 U5-T02 fixture 的 `(planned)` 状态。
- 已完成结构校验：139 个 `.scoop` 文件与 `_index.csv` 行一一对应，所有 fixture 头部包含必需字段，negative fixture 头部包含错误码、位置、错误文案和 `REASON`。
- 已完成验证：`cargo run -p scoop -- test tests/fixtures/umb_fix/`、`cargo test -p scoopc audit::spec_coverage -- --nocapture`、`cargo test -p scoop -- fixtures -- --nocapture`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoopc --bin umb-audit -- stats`、`cargo run -p scoopc --bin umb-audit -- diff`、`cargo test --all --all-targets` 均通过或无匹配测试失败。
- 已将 `TODO.md` 中 U5-T02 标记为 `[DONE]` 并填写完成记录。
- 下一步：执行 Git 提交流程并停止。

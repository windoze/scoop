# 本次执行计划

## 约束与目标

- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始实际实现前，先检查最新提交里是否提到既有问题；如果有，先修复这些问题。
- 过程中若发现规格不匹配、缺失前置能力或任务过大，需要先更新 `TODO.md` / `PLAN.md`，再决定是否继续实现。
- 我会持续更新本文件，记录当前计划、关键决策、进度与是否需要调整。

## 初始步骤

1. 查看最新一次提交的提交信息与相关变更，确认是否明确提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解该任务的上下文、依赖和既有规划。
4. 检查工作区是否存在未提交修改，避免覆盖用户已有工作。

## 执行策略

1. 如果最新提交暴露了既有问题，先定位并修复，再补充测试。
2. 如果第一个未完成任务过大：
   - 将其拆分为更小的可执行子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把当前应执行的第一个子任务放到最前。
3. 对当前任务做最小充分实现，避免引入无关改动。
4. 运行与改动直接相关的测试，再运行必要的全局质量检查：
   - 相关单元/集成/fixture 测试
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 如有必要，运行更大范围的 `cargo test --all`
5. 更新 `TODO.md` 与 `PLAN.md` 反映结果。
6. 提交本轮所有修改，提交后停止。

## 风险检查点

- 若实现过程中发现语言特性、运行时、类型系统或诊断与规范不一致，不能绕过，必须先把该问题补进 `TODO.md` 作为前置任务。
- 若测试失败，需要先判断是本次改动问题、既有缺陷，还是规范缺口；按依赖顺序处理。
- 若遇到用户未说明但工作区已存在的改动，不回退，先评估是否冲突。

## 当前状态

- 已创建计划文件。
- 已检查最新提交、`TODO.md`、`PLAN.md`、`ISSUES.md` 与工作区状态。
- 最新提交 `5ddd42941a96b9c800ee16c68c2b4130687cb66c` 没有在提交标题中额外声明必须先修的既有问题；当前工作区只有本文件被修改。
- 已确认本轮第一个未完成任务是 `T4007c`：收口旧 RTTI 导出 API 的参数化类型支持与文档同步。

## 对当前任务的理解

- CLI `scoop dump-rtti` 目前走的是 `crates/scoopc/src/rtti/type_desc.rs` 的新 API。
- 待修的是 `crates/scoopc/src/rtti/mod.rs` 中更旧的 RTTI 导出 API：它仍保留“只支持非泛型 struct”的早期边界，并在 `nominal_layout(...)` 中对带 type args / `eff` 的 nominal 直接报 `UnsupportedGenericType`。
- `T4007c` 的目标不是重做 CLI，而是让旧 API 在库层查询 generic / `eff` 参数化 nominal 时，也能输出与新 API 一致的 canonical name / type_id，不再静默拒绝。

## 当前执行计划

1. 继续阅读 `rtti/mod.rs` 与相关类型解析/布局代码，确认需要补的最小闭环：
   - 查询字符串到 `TypeId` 的解析；
   - 旧 API 对参数化 nominal 的布局/字段 RTTI 处理；
   - 与新 API 的 canonical name / type_id 对齐方式。
2. 如果确认不需要再拆分任务，则直接实现：
   - 让旧 API 接受参数化 nominal；
   - 对参数化 struct/class/interface/effect 采用正确且可解释的 RTTI 导出策略；
   - 必要时补充注释或文档，说明旧 API 的可观测边界。
3. 增加定向测试：
   - 旧 API 对 generic / `eff` nominal 的查询回归；
   - 校验 canonical name / type_id 与新 API 或同一 `TypeStore::display` 口径一致。
4. 运行格式化、定向测试、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md` / `PLAN.md` / 本文件，提交本轮修改并停止。

## 已执行结果

### 实现

- 已修改 `crates/scoopc/src/rtti/mod.rs`：
  - 旧 RTTI `dump_type_rtti` 现在通过 synthetic type query + `TypeLowering::new_with_ctx(...)` 在原文件 package/import 语境下解析类型查询，因此支持带 type args / `eff` row 的 nominal。
  - 旧 RTTI 现在会收集当前编译单元（含 sysroot）的 struct 声明头；generic struct 的字段 RTTI/布局会通过 `lower_type_ref_in_decl_file_with_scopes(...)` 重新按声明处上下文实例化，不再只支持非泛型 struct。
  - `nominal_layout(...)` 已移除对 args / `eff` 的早期硬拒绝，parameterized class/interface/effect/enum/struct 都能得到一致且可解释的 RTTI 导出结果。
- 已修改 `crates/scoopc/src/typecheck/lower.rs`：
  - 将 `lower_type_ref_in_decl_file_with_scopes(...)` 放宽到 crate 内可复用，供 RTTI 元数据路径复用现有声明处 lowering 主线。
- 已修改 `SCOOP_RUNTIME.md`：
  - 补充参数化 nominal 的 canonical name 应包含 concrete type args 与 use-site `eff` row。

### 验证

- 已通过：
  - `cargo test -p scoopc rtti:: -- --nocapture`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop --type 'StringReadable'`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop --type 'Holder<Int>'`
  - `cargo clippy --all-targets -- -D warnings`
- 已尝试但未完成：
  - `cargo test --all`
  - 阻塞原因不是 RTTI 改动本身，而是既有 runtime 测试 `crates/scoop_runtime/tests/gc_immix_compaction.rs` 中两条 compaction 用例会长时间卡在 `waiting for park: epoch=2 parked=0 need=1`。

### 计划调整

- 已在 `TODO.md` / `PLAN.md` 中把 `T4007c` 标记完成。
- 已新增 blocker `T4007S`（位于 `T4007R` 之前），专门处理 `cargo test --all` 中暴露出的既有 `gc_immix_compaction` 挂起。
- 当前下一项已切换为 `T4007S`，本轮在提交后停止。

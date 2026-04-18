# 执行记录与计划

## 约束说明

- 按用户要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在正式执行前，先建立这份计划记录；后续只记录摘要式推理、决策依据与步骤，不写私有详细思维链。
- 若发现最新提交中提到的既有问题，需先修复，再继续当前任务。
- 若遇到规范不匹配、缺失语言特性或实现边界导致无法正确推进，需要先把阻塞项写回 `TODO.md` / `PLAN.md`，提交后停止。

## 初始执行计划

1. 检查最新一次 Git 提交信息，确认是否提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关规范和受影响代码，判断该任务是否可直接完成。
4. 若任务过大，拆分为更小子任务并更新 `TODO.md` / `PLAN.md`，然后执行新的第一个子任务。
5. 实现任务，补充或调整测试。
6. 运行必要验证，优先包含：
   - 与修改范围直接相关的测试
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 如范围允许，运行更完整测试
7. 更新 `TODO.md`、`PLAN.md`、本记录文件。
8. 生成一次清晰的 Git 提交，随后停止。

## 计划更新规则

- 每完成关键步骤后，追加当前发现、风险、下一步动作。
- 若计划变更，直接在本文件中记录变更原因和新的执行顺序。

## 进度记录

### 2026-04-19 00:00 +08:00

- 已检查最新提交 `ae4d767 [T4001] 收口泛型约束、参数化超类型与 star projection`；提交信息本身未额外声明新的既有 issue，因此无需先插入单独修复任务。
- 已确认当前首个未完成任务是 `T4001R`（review 任务），不需要再拆子任务。

### 2026-04-19 00:05 +08:00

- 已完成静态复审，结论如下：
  - 参数化超类型不是靠 `Array` / `Collection` / 单个 interface 名称硬编码实现；主线为 `TypeEnv::direct_supertype_infos` 保留声明处 `TypeRef`，`TypeLowering::ensure_concrete_direct_supertypes` 在 use-site 结合 type args 做 substitution，`assignable::concrete_nominal_is_subtype` 再沿具体化后的 supertype 链 DFS。
  - star projection 在 typecheck 内保留为 `TypeKind::StarProjection`；`assignable` 通过 `is_star_projection` / `star_projection_read_view` / `is_star_projection_read_compatible` 维护“可读、禁写、值类型需显式 boxing”的语义。
  - `cone/scoopir/export.rs`、`rtti/type_desc.rs`、`llvm/codegen/layout.rs`、`llvm/codegen/ty.rs` 只在导出/布局/代码生成边界读取 `read_ty`，未把 `*` 在前端主线上回退成 `Any`。
- 发现一条过时注释：`crates/scoopc/src/typecheck/type_env.rs` 仍写着“当前只存储 FQN、不存储 type args”；计划顺手修正，避免文档误导后续实现。

### 2026-04-19 00:07 +08:00

- 已完成动态验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` 通过，结果 `fixtures: ok (326)`。
  - 因 `scoop test --fixtures` 只接受单个根目录，已在 `target/t4001r-fixtures/run-pass` 临时复制两条 review 相关 run-pass fixtures。
  - `target/debug/scoop test --fixtures target/t4001r-fixtures/run-pass` 通过，结果 `fixtures: ok (2)`。

### 2026-04-19 00:10 +08:00

- 当前收尾动作：
  1. 修正 `type_env.rs` 中的过时注释。
  2. 将 `TODO.md` 中 `T4001R` 标记为完成并写入 review 结论。
  3. 更新 `PLAN.md` 的当前状态到 `T4002`。
  4. 检查工作区、提交本轮改动，然后停止。

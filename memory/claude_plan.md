本轮执行计划：P7-T04-b-1R Review `MonoTypeId` 类型纪律基线

## 范围

- 仅处理 `TODO-6.md` 中按顺序出现的第一个未 `[DONE]` 任务 `P7-T04-b-1R`。
- 该任务是 `P7-T04-b-1` 的独立 review。任务卡定义的重点（4 项）与验证（重跑 + 额外 grep）均需当面执行；本 review **不是形式检查**：若发现前一任务未真正达成目标，必须直接修复或阻塞下一任务。
- 完成判据：review 结论明确写出 `MonoTypeId` 不变量已被 Rust 类型系统强制，或列出阻塞项并在本 review 内修复。

## 关键审查点（任务卡定义）

1. **构造唯一性**：`MonoTypeId` 不能从外部绕过 `as_mono` 构造。
   - 搜 `MonoTypeId(` 与 `pub fn ... -> MonoTypeId`：除 `as_mono` 与 `MonoTypeId::inner` 这种 accessor 外，不应有公开的、不经校验的入口。
   - `kind_mono` 内部把 children 包成 `MonoTypeId(child)` 是允许的，因为这些位置已被 `as_mono` 校验为非 `Param`，但要确认这些构造位置不暴露为公开 API。
2. **覆盖完整性**：`as_mono` 必须覆盖所有 `TypeKind` / `RefTypeKind` / `ValueTypeKind` 子位置：
   - `Ref::Nominal.args`、`Ref::Nominal.eff.terms`；
   - `Ref::Function.receiver`、`Ref::Function.params`、`Ref::Function.return_ty`、`Ref::Function.effects.terms`；
   - `Ref::Union.variants`；
   - `Value::Tuple.elements`；
   - `Value::Option(inner)`；
   - `Value::Nominal.args`、`Value::Nominal.eff.terms`；
   - `StarProjection.read_ty`；
   - 标量/builtin 终止位（`Any` / `String` / `Unit` / `Nothing` / `Bool` / `Char` / `Float64` / `Float32` / `Int` / `UInt` / `IntN` / `UIntN`）。
3. **测试覆盖**：`kind_mono` 子位置一致性是否被测试覆盖（已经在 `kind_mono_children_align_with_underlying_typekind` 测试里覆盖 Tuple / Option / Nominal / Function / StarProjection；需确认 Union / EffectRow 也被覆盖到——若缺失则补）。
4. **无 fallback**：没有任何静默把 `Param` 视为合法 codegen 类型的代码路径——这是设计 `MonoTypeId` 的核心动机，本 review 必须把整个 `crates/scoopc_types/src/` 扫一遍确认。

## 步骤

1. 写本计划到 `./memory/claude_plan.md`（本步骤）。
2. **构造唯一性审查**：
   - `rg -n 'MonoTypeId\b' crates/scoopc_types/`：列出所有引用，确认仅 `as_mono` 返回 `MonoTypeId`，公开 API 不暴露其它入口。
   - `rg -n 'pub fn .* -> .*MonoTypeId' crates/`：列出所有 `pub fn` 返回 `MonoTypeId` 的位置。
   - 检查是否有 `From<TypeId> for MonoTypeId` / `Into<TypeId> for MonoTypeId` / `unsafe`/`unchecked` 构造。
   - 检查 `MonoTypeId` 字段可见性：tuple struct field `MonoTypeId(TypeId)` 默认 private（OK），但要确认未误标 `pub`。
3. **覆盖完整性审查**：
   - 重新读 `as_mono` 实现，与 `TypeKind` / `RefTypeKind` / `ValueTypeKind` / `StarProjectionType` / `EffectRow` / `NominalType` / `FunctionType` / `UnionType` 各字段逐一对照，确认无遗漏。
   - 重新读 `kind_mono` 实现，对照同样字段集，确认 children 一一被包装为 `MonoTypeId`，无遗漏。
4. **测试覆盖审查**：
   - 检查现有 19 个 `as_mono`/`kind_mono` 单元测试是否覆盖任务卡 4 项重点；
   - 若 `kind_mono` 对 Union / `MonoEffectRow` 字段位置一致性没有测试，本 review 必须补一个 `kind_mono` 在 Union 上的测试；
   - 确认 idempotent 测试存在。
5. **fallback 路径审查**：
   - `rg -n 'Param' crates/scoopc_types/src/`：把 `Param` 出现的所有位置列出，确认 `as_mono` 只在 `Param` 节点处返回 `Err(ParamLeak)`，不存在其它路径把 `Param` 视为合法或映射为某个默认 TypeId。
   - 确认 `kind_mono` 中 `Param` 分支是 `unreachable!`（强约束）而不是返回某个 fallback。
6. 修复发现的任何阻塞项（若有）。
7. 重新运行任务卡列出的所有验证：
   - `cargo fmt`；
   - `cargo test -p scoopc_types`；
   - `cargo build -p scoopc`；
   - `cargo clippy --all-targets -- -D warnings`；
   - `git diff --check`。
8. 把 `P7-T04-b-1R` 标题前缀改为 `[DONE]`，补完成记录（明确写出 review 结论与具体审查证据）；同步 `TODO.md` 索引状态。
9. 提交（`[P7-T04-b-1R]` 前缀）。

## 完成判据（任务卡定义）

- review 结论明确写出 `MonoTypeId` 不变量已被 Rust 类型系统强制；
- 或列出阻塞项并在本 review 内修复完毕。

## 不在本轮范围内

- 任何 `cg_ty_of` / `expect_cg_ty_of` 调用点的修改（属于 b-2/b-3/b-4）；
- `hir::ClassInit` 拆分（属于 b-2）；
- `ClassInstanceKey` 引入（属于 b-3）；
- codegen 内部 token 迁移（属于 b-4）。

## 进度记录

- 已写入本计划。
- 步骤 2（构造唯一性）：✓ `MonoTypeId(TypeId)` private inner field；`#![forbid(unsafe_code)]`；`rg` 验证仅 `as_mono` 公开返回 `MonoTypeId`，无 `From`/`unsafe`/`unchecked`。
- 步骤 3（覆盖完整性）：✓ `as_mono` / `kind_mono` 与 `TypeKind` / `RefTypeKind` / `ValueTypeKind` / `StarProjectionType` / `EffectRow` / `NominalType` / `FunctionType` / `UnionType` 各子位置一一对齐。
- 步骤 4（测试覆盖）：✓ 补 `kind_mono_aligns_for_union_value_nominal_and_use_site_eff_row` 一项，覆盖原测试缺失的 Union variants / Value::Nominal / use-site EffectRow.terms。
- 步骤 5（fallback 路径）：✓ `Param` 仅在构造、`re_intern_from`、`as_mono` 拒绝、`kind_mono` `unreachable!`、Display 五类合法路径出现，无静默 fallback。
- 步骤 6：无阻塞项需修复。
- 步骤 7（验证）：`cargo fmt`、`cargo test -p scoopc_types`（25 passed）、`cargo build -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`git diff --check` 全部通过。
- 步骤 8：TODO-6.md 中 P7-T04-b-1R 标题已改为 `[DONE]`，完成记录写入；TODO.md 索引同步；TODO-6.md 头部状态行更新。
- 接下来：步骤 9（提交）。

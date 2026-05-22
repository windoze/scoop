本轮执行计划：P7-T04-b-2 拆分 `hir::ClassInit` 为 `GenericClassDecl` 与 `MonoClassInit`

## 范围

- 仅处理 `TODO-6.md` 中按顺序出现的第一个未 `[DONE]` 任务 `P7-T04-b-2`。
- 任务核心：把 codegen 视野中的"class shape 数据结构"从可含 `Param` 的 `TypeId` 收紧为 `MonoTypeId`，使 `class_inits.get(...)` 拿到的值在 *类型系统* 层就不可能含 `Param`；前置 `as_mono` 校验在 `MonoClassInit` 构造点执行，构成 monomorph driver 的硬约束。
- **不在本轮范围**：
  - body `Expr.ty` / `Block.ty` / `Stmt.ty` / `CallArg` / `CtorCallInfo` 内部的 TypeId（这些是 b-4 收紧 codegen 全面切换 MonoTypeId 时的目标）；本轮不参数化 `Expr<T>` / `Block<T>` 等，否则 cascade 到整个 HIR；
  - layout key 字符串形态（b-3 `ClassInstanceKey`）；
  - `expect_cg_ty_of` 与 `cg_ty_of` 接口（b-4）。

## 设计决策

1. **形参化内层结构**：
   - `ClassField<T> { fqn, name, mutable, ty: T }`；
   - `ClassCtorParam<T> { id, name, decl_span, ty: T, has_default, default_value, is_property, property_field_fqn }`；
   - `ClassCtor<T> { kind, span, params: Vec<ClassCtorParam<T>>, delegation: Option<ClassCtorDelegation>, body: Option<Block> }`；
   - `ClassInitStep` / `ClassCtorDelegation` 不参数化（直接 TypeId 槽位为零；body Expr 留给 b-4）。
2. **两个独立的顶层 struct（不通过 alias 共享）**：
   - `GenericClassDecl { fqn, source_path, super_class_fqn, super_ctor_args_span, super_ctor_call, super_ctor_args, this_id, fields: Vec<ClassField<TypeId>>, field_indices, steps: Vec<ClassInitStep>, ctors: Vec<ClassCtor<TypeId>> }`；
   - `MonoClassInit { fqn, source_path, super_class_fqn, super_ctor_args_span, super_ctor_call, super_ctor_args, this_id, fields: Vec<ClassField<MonoTypeId>>, field_indices, steps: Vec<ClassInitStep>, ctors: Vec<ClassCtor<MonoTypeId>> }`。
3. **索引拆分**：
   - `pub type ClassInitIndex = HashMap<String, MonoClassInit>`（重命名/换值类型，名字保留以减少全 repo 的 rename）；
   - `pub type GenericClassDeclIndex = HashMap<String, GenericClassDecl>`。
4. **`LoweredHir` 增加 `generic_class_decls: GenericClassDeclIndex` 字段**：codegen / RTTI / mir-materialize 视角的 `class_inits` 仍然只看 `MonoClassInit`；typecheck / HIR lowering / monomorph driver 视角的源声明走 `generic_class_decls`。
5. **`MonoClassInit::from_generic_decl`**：单一构造入口，输入为 `&GenericClassDecl + &TypeStore`，对每个 `ClassField` / `ClassCtorParam` 调 `types.as_mono(...)`；任一失败即返回 `MonoLeakDiag { class_fqn, slot: FieldOrParamSlot, leak: ParamLeak }`。这是 monomorph driver 把 substituted GenericClassDecl 升级为 MonoClassInit 的唯一路径。
6. **Verifier-style assertion for non-generic class**：在 `pipeline/hir_stage.rs` 把非泛型 class 直接构造为 `MonoClassInit`：先建 GenericClassDecl，调 `from_generic_decl`，失败则 typecheck 已应阻断（这里报错位置精准到字段/参数 + leak path）。
7. **`collect_generic_class_instantiation_inits`（hir/lower/util/generic_layouts.rs）**：在 substitute 完字段/参数 TypeId 后，立即调 `MonoClassInit::from_generic_decl`；失败时返回明确的 monomorph driver diagnostic。
8. **codegen 读取点**：codegen 需要的 `field.ty` / `param.ty` 现在是 `MonoTypeId`，调用 `expect_cg_ty_of(field.ty.inner(), ...)`（保留 b-4 接口语义不变）。

## 影响面（按 grep 命中分类）

A. **HIR 数据结构**：`crates/scoopc/src/hir/mod.rs`（核心定义；新增类型 + 新增 `LoweredHir.generic_class_decls`）。
B. **HIR lowering 产出 ClassInit 的位置**：
   - `crates/scoopc/src/hir/lower/util/generic_layouts.rs`（substitute_class_init_steps / substitute_class_ctor_type_params 等改为基于 `GenericClassDecl` 操作；最后调 `from_generic_decl` 产出 `MonoClassInit`）；
   - `crates/scoopc/src/pipeline/hir_stage.rs`（line ~6051 的非泛型 class 插入点；line ~3538/~3096/~3184 的 ClassInit 引用类型）；
   - `crates/scoopc/src/cone/pre_specialize.rs`（init `class_inits: HashMap::new()` 处也要 init `generic_class_decls`）。
C. **HIR completeness verifier**：`crates/scoopc/src/pipeline/hir_completeness.rs`（同时验证 `generic_class_decls` 与 `class_inits` —— 但底层 verify 函数可对内层 `ClassField<T>` / `ClassCtorParam<T>` 形参化共用一份逻辑）。
D. **MIR**：
   - `crates/scoopc/src/mir/lower/hidden_init.rs`（line 169-172：读 `class_inits` 与 `pre_specialize.class_inits`；类型从 `&hir::ClassInit` 改为 `&hir::MonoClassInit`；body `init` Expr 走 substitute 后产出 MonoClassInit）；
   - `crates/scoopc/src/mir/materialize/{instance, entry, run, generic_mir, inputs, reachable, tests, mod}.rs`（约 20 处 `class_inits` 字段；类型从 `crate::hir::ClassInitIndex` 保留名但值改为 `MonoClassInit`；test fixtures 用 `HashMap::new()` 不变）。
E. **RTTI**：
   - `crates/scoopc/src/rtti/type_desc.rs`（line 267/270/1183/1186/1191/1213/1222/1232/1234：`class_inits` reader；`flatten_class_fields` 等读 `field.ty` / `super_class_fqn`；切到 `MonoClassInit` 视图，`field.ty: MonoTypeId` 转 TypeId 通过 `.inner()`）。
F. **codegen**：
   - `crates/scoopc/src/llvm/codegen/{layout, mod, ty, gc, class_ctor, mir_body/*, effect_lowered/layout/abi, effect_lowered/value, effect_lowered/body/class_ctor, main/expr_op}.rs`：所有 `&hir::ClassInit` → `&hir::MonoClassInit`；`field.ty` / `param.ty` 调用点用 `.inner()` 喂 `expect_cg_ty_of`。
G. **emit & tests**：
   - `crates/scoopc/src/llvm/emit.rs:538`；
   - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/tests/mod.rs:197`；
   - `crates/scoopc_hir_facts/src/dump.rs`（line 41 仅 dump 计数，不动）；
   - `crates/scoopc/src/pipeline/lir_facts_builder.rs`（line 579/650：读 class shape；切换）；
   - `crates/scoopc/src/llvm/codegen/main/coerce.rs`、`stmt.rs`、`object_init.rs`、`main/globals.rs`（这些用 `expect_cg_ty_of(prop.ty)` 读非 class shape 的 ObjectField/local，**不受影响**）。

## 步骤

1. 写本计划到 `./memory/claude_plan.md`（本步骤）。
2. **设计层面注入**：
   - 在 `crates/scoopc/src/hir/mod.rs` 新增 `ClassField<T>` / `ClassCtorParam<T>` / `ClassCtor<T>` 形参化结构 + 顶层独立的 `GenericClassDecl` / `MonoClassInit` + 新 type alias `ClassInitIndex` / `GenericClassDeclIndex` + `LoweredHir.generic_class_decls` 字段；
   - 引入 `MonoLeakDiag` 与 `MonoClassInit::from_generic_decl(decl: &GenericClassDecl, types: &TypeStore) -> Result<MonoClassInit, MonoLeakDiag>`；
   - 删除原 `pub struct ClassInit` / `pub struct ClassField` / `pub struct ClassCtor` / `pub struct ClassCtorParam` 的旧形态；保留 `ClassInitStep` / `ClassCtorKind` / `ClassCtorDelegation` / `CtorCallInfo` 不变。
3. **HIR lowering 与 monomorph driver**：
   - `hir/lower/util/generic_layouts.rs` 中 substitute_class_init_* 系列函数改为 `GenericClassDecl → GenericClassDecl`（substitute 后）的 in-memory 形态；substitute 完成后由 `collect_generic_class_instantiation_inits` 调 `MonoClassInit::from_generic_decl` 产出。
   - `pipeline/hir_stage.rs` 中非泛型 class 插入点（line ~6051）：先建 GenericClassDecl，调 `from_generic_decl` 升级为 MonoClassInit 入 `class_inits`；GenericClassDecl 同时入 `generic_class_decls`。
   - 泛型 class 源声明：仅入 `generic_class_decls`，不入 `class_inits`。
4. **Reader 切换**：
   - `pipeline/hir_completeness.rs`：把 `verify_class_init` 形参化为 `for<T>(&ClassInit<T>)`（实际通过两份 thin wrapper 对 GenericClassDecl 与 MonoClassInit 复用）。验证两套索引。
   - `cone/pre_specialize.rs`：增 `generic_class_decls: HashMap::new()` 初始化。
   - `rtti/type_desc.rs`：reader 类型切到 `MonoClassInit`；`field.ty` 用 `.inner()` 取出 TypeId 喂下游。
   - `mir/lower/hidden_init.rs` & `mir/materialize/*`：类型字段 `class_inits` 值类型改为 `MonoClassInit`，使用 `.inner()` 接 `field.ty`。
   - `pipeline/lir_facts_builder.rs`：reader 类型切到 `MonoClassInit`。
   - `llvm/codegen/*`：所有 `&hir::ClassInit` → `&hir::MonoClassInit`；`field.ty.inner()` / `param.ty.inner()` 喂 `expect_cg_ty_of`。
5. **测试 & dump 修复**：
   - `mir/materialize/tests.rs` / `hir/lower/main/tests.rs` / `effect_lowered/layout/tests/mod.rs`：HashMap::new() 处保持；显式构造点 fixup。
   - `scoopc_hir_facts/src/dump.rs`：仅 count，不动。
6. 跑全套验证：
   - `cargo fmt`；
   - `cargo test -p scoopc_types`（25 passed）；
   - `cargo test -p scoopc --no-default-features`（含 hir / llvm::codegen::effect_lowered::layout）；
   - `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；
   - `cargo clippy --all-targets -- -D warnings`；
   - `git diff --check`。
7. 把 `P7-T04-b-2` 标题前缀改为 `[DONE]`，补完成记录；同步 TODO.md 索引；更新 TODO-6.md 头部状态行；更新本计划进度。
8. 提交（`[P7-T04-b-2]` 前缀）。

## 完成判据（任务卡定义）

- `hir::ClassInit` 已拆为 `GenericClassDecl` / `MonoClassInit`；
- `class_inits: HashMap<String, MonoClassInit>` 在 codegen 视野中只承载单态化条目；
- 任一含 `Param` 的字段在构造 `MonoClassInit` 时被拒绝并触发明确 diagnostic（含 class FQN、字段名、leak path）；
- 现存测试集通过（除可能因尚未引入 `ClassInstanceKey` 导致 `sysroot_atomic_basic` 在 b-3 前仍 verifier-style 错误；要求错误位置比当前 `expect_cg_ty_of` panic 更精确）。

## 进度记录

- 已写入本计划。
- 步骤 2（hir/mod.rs 数据结构定义）已完成：`ClassField<T>` / `ClassCtorParam<T>` / `ClassCtor<T>` / `ClassCtorImpl<T>` 形参化结构 + 顶层独立 `GenericClassDecl` / `MonoClassInit` + `MonoLeakDiag` + `from_generic_decl` 构造入口 + 双索引 alias + `LoweredHir.generic_class_decls` 字段全部就位。
- 步骤 3（HIR lowering 与 monomorph driver）已完成：`hir/lower/util/generic_layouts.rs` 单态化路径调 `from_generic_decl`；`hir/lower/util/decls.rs` 非泛型 class 直接构造并升级；`hir/lower/main/accessors.rs` 重构 7-tuple 返回为 `CompilationUnitInitCollectionOutputs` 结构体（修 clippy type_complexity）；`hir/lower/main/{entry,compilation_unit}.rs` 同步切换。
- 步骤 4（Reader 切换）已完成：`pipeline/hir_completeness.rs` 验证两套索引；`cone/pre_specialize.rs` init `generic_class_decls`；`rtti/type_desc.rs` reader 切 `MonoClassInit`；`mir/lower/hidden_init.rs` & `mir/materialize/*` 字段类型对齐；`pipeline/lir_facts_builder.rs` reader 切；`llvm/codegen/*` 全切。
- 步骤 5（测试 & dump 修复）已完成：tests fixtures 显式构造点对齐；effect_lowered fixture 10 文件 regenerated（移除 7 项纯泛型 class template 的 physical_layout 行）；`pipeline_user_visible_failure_policy.rs::INTERNAL_BUG_SENTINEL_HITS` 刷新为 389 entries。
- 步骤 6（验证）已完成：`cargo fmt`；`cargo test -p scoopc_types`（25 passed）；`cargo test -p scoopc --no-default-features --lib`（含 hir 86 / 全部 631 passed）；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`（10/10 passed）；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）；`cargo clippy --all-targets -- -D warnings`（clean）；`git diff --check`（clean）。
- 预存测试失败：观察到 4 项 LLVM 库测试在 clean HEAD 上即失败，按 PROMPT.md §"Test/Fixture Failure Policy" 显式排期为 `P7-T04-b-5` / `P7-T04-b-5R`，置于 `P7-T04-b-4R` 与 `P7-T04-b` 之间。
- 步骤 7（标记 [DONE] + 同步索引）已完成：TODO-6.md `P7-T04-b-2` 标题前缀改为 `[DONE]`，完成记录已填；TODO.md 索引行同步；TODO-6.md 头部状态行更新；本计划进度记录更新。
- 接下来：执行步骤 8（提交，`[P7-T04-b-2]` 前缀）。

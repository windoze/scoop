本轮执行计划：P7-T04-b-1 引入 `MonoTypeId` / `MonoTypeKind` codegen 输入类型纪律基线

## 范围

- 仅处理 `TODO-6.md` 中按顺序出现的第一个未 `[DONE]` 任务 `P7-T04-b-1`。
- 该任务由上一轮会话从 `P7-T04-b` 拆出，根本目的：在 `scoopc_types` 中引入 `MonoTypeId(TypeId)` newtype 与配套 `MonoTypeKind<'a>` 视图，使后续任务（b-2..b-4）能在类型层固定 "codegen 阶段不可能拿到含 `Param` 的 TypeId" 不变量。
- 阻塞 root cause：当前 `cg_ty_of(TypeId) -> Option<CgTy>` 与 `expect_cg_ty_of` 在签名层承认 codegen 输入 `TypeId` 可能含 `Param`，导致 codegen 必须 runtime warn + 上游 panic 替代 type-state 校验；`sysroot_atomic_basic` panic 即其表象。
- 完成判据由任务卡定义为"纯增量发布 `MonoTypeId` 基线，不修改任何现存调用点"。

## 关键设计决定

1. **`MonoTypeId` 不变量为深度不变量**：持有 `MonoTypeId` 等价于"整棵类型树（含 nominal args、function receiver/params/return/effects、union variants、tuple elements、option inner、star projection inner、nominal use-site eff row）不含 `TypeKind::Param`"。
2. **唯一构造路径**：`TypeStore::as_mono(t: TypeId) -> Result<MonoTypeId, ParamLeak>`。**不**实现 `From<TypeId>` / `Into<TypeId>` / `From<&str>` / `unsafe`/`unchecked` 等绕过路径；`inner(self) -> TypeId` accessor 仅用于 hash-cons 比较与诊断。
3. **`MonoTypeKind<'a>` 是与 `TypeKind` 同形但 children 为 `MonoTypeId` 的并行视图**：枚举形状一致，去掉 `Param` 分支。所有 inner TypeId 位置在视图中暴露为 `MonoTypeId`，调用方无需重复校验 —— `as_mono` 的深度校验保证了子位置已合法。
4. **`as_mono` 算法采用迭代 worklist**：避免递归类型导致栈溢出；`visited: HashSet<TypeId>` 防止环路；首次发现 `Param` 时返回 `ParamLeak`，含 `offending: TypeId` 与 `leak_path: Vec<TypeKindLabel>`（顶到底，描述如何从输入走到 leak）。
5. **`TypeKindLabel` 覆盖 10 个嵌套位置**：`NominalArg` / `NominalEffect` / `UnionVariant` / `FunctionReceiver` / `FunctionParam` / `FunctionReturn` / `FunctionEffect` / `TupleElement` / `OptionInner` / `StarProjectionInner`。
6. **纯增量**：不修改任何现存代码；`TypeKind` / `TypeStore::kind` 等老 API 全部保留，b-2..b-4 任务才会逐步迁移调用点。

## 步骤

1. 写本计划到 `./memory/claude_plan.md`（本步骤）。
2. 同步 `TODO.md` 索引：在 `P7-T04-a` 与 `P7-T04-b` 之间插入 8 行（`P7-T04-b-1` / `b-1R` / `b-2` / `b-2R` / `b-3` / `b-3R` / `b-4` / `b-4R`）。
3. 在 `crates/scoopc_types/src/lib.rs` 中追加：
   - `MonoTypeId(TypeId)` newtype（`Copy + Clone + Eq + Hash + Debug`，`inner()` accessor，**无** `From<TypeId>` 等隐式转型）；
   - `ParamLeak { offending: TypeId, leak_path: Vec<TypeKindLabel> }` 错误类型；
   - `TypeKindLabel` 嵌套位置标签 enum（10 种位置）；
   - `MonoTypeKind<'a>` 与并行 hierarchy（`MonoRefKind` / `MonoValueKind` / `MonoNominal` / `MonoFunction` / `MonoUnion` / `MonoEffectRow` 等）；
   - `TypeStore::as_mono(TypeId) -> Result<MonoTypeId, ParamLeak>` 实现（迭代 worklist + visited）；
   - `TypeStore::kind_mono(MonoTypeId) -> MonoTypeKind<'_>` 实现（按当前 `TypeKind` 形态包装 children 为 `MonoTypeId`）。
4. 在 `crates/scoopc_types/src/lib.rs` 内添加 `#[cfg(test)]` 单元测试模块，覆盖：
   - 单层标量/builtin 通过：`Int` / `UInt` / `Bool` / `Char` / `Unit` / `Nothing` / `Float64` / `Float32` / `IntN(N)` / `UIntN(N)` / `Any` / `String`；
   - 顶层 `Param` 拒绝（`leak_path` 为空）；
   - 嵌套 nominal `Box<T>` 拒绝（`NominalArg{fqn, index: 0}`）；
   - 嵌套 nominal `Box<Int>` 通过；
   - nominal use-site `Foo<eff Pass<T>>` 类的 `NominalEffect` 拒绝（如能用 `EffectRow` 直接构造）；
   - tuple `(Int, T)` 拒绝（`TupleElement{index: 1}`）；
   - tuple `(Int, String)` 通过；
   - option `Option<T>` 拒绝（`OptionInner`）；
   - option `Option<Bool>` 通过；
   - function `(T) -> Int / Pure` 拒绝（`FunctionParam{index: 0}`）；
   - function `(Int) -> T / Pure` 拒绝（`FunctionReturn`）；
   - function with receiver `T.(Int) -> Int / Pure` 拒绝（`FunctionReceiver`）；
   - function effect row 含 `Param` 拒绝（`FunctionEffect{index}`）；
   - union `A | B | T` 拒绝（`UnionVariant{index}`）；
   - star projection inner 含 `Param` 拒绝（`StarProjectionInner`）；
   - 自引用 nominal（如 `Box<Box<Box<Int>>>`）通过且不死循环；
   - `kind_mono` 返回的 children 与原 `TypeKind` 内嵌 `TypeId` 一一对应（`MonoTypeId::inner` 等于原 TypeId）；
   - 同一 TypeId 多次 `as_mono` 行为一致（幂等）；
   - 多次 `as_mono` 同一 leak 给出相同 `leak_path`。
5. `cargo fmt`、`cargo test -p scoopc_types`、`cargo build -p scoopc`（确认旧 `TypeId` 调用路径未被破坏）、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
6. 在 `TODO-6.md` 把 `P7-T04-b-1` 标题前缀改为 `[DONE]`、补完成记录；同步 `TODO.md` 索引把状态改 `[DONE]`。
7. 提交（`[P7-T04-b-1]` 前缀）。

## 验证清单（任务卡定义）

1. `cargo fmt`
2. `cargo test -p scoopc_types`
3. `cargo build -p scoopc`（确认旧 `TypeId` 调用路径未被破坏）
4. `git diff --check`

额外执行 `cargo clippy --all-targets -- -D warnings` 以满足 PROMPT.md "无 warning" 总纪律。

## 不在本轮范围内

- 任何 `cg_ty_of` / `expect_cg_ty_of` 调用点的修改；
- `hir::ClassInit` / `ClassInitIndex` 的拆分；
- `ClassInstanceKey` 的引入；
- `mir::Local.ty` / `MirLocalSlot.ty` 等 codegen 内部 token 的迁移；
- 删除 `monomorph miss` 警告或 `expect_cg_ty_of` 函数；
- 修复 `sysroot_atomic_basic` panic（其修复在 b-3 完成后才会发生）。

这些都属于后续 b-2 / b-3 / b-4 任务，本轮不动。

## 进度记录

- 已写入本计划。
- 接下来：先做 `TODO.md` 索引同步（小补丁），再开始实现工作。

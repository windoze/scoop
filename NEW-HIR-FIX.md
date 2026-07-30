# HIR 完善计划

> 状态：全部步骤已完成

## 已完成

### 步骤 1：derive_call_resolution 覆盖所有 call 形态
- FunValue 变体（`f()(x)` / `fns[0](1)` / lambda 调用）
- TypeApply 解包（`f<T>(args)`）
- TupleIndex → FunValue 路径
- scalar_fqn 回退：String/Any/标量类型方法调用

### 步骤 2：resolve_member_owner_fqn 支持内建类型
- `scalar_fqn` 回退：Ref(String)/Ref(Any)/Value(Int)/Value(Bool) 等内建类型
- MIR fallback 路径 0 次触发（合法程序）
- lower_infix_call/lower_index 消费 call_resolutions

### 步骤 3：Nothing 类型检查
- completeness gate 检查 TypeKind::Nothing（Walker 带 TypeStore）
- block_lenient 移除（class init block / secondary ctor body 改用严格 block()）
- verify() 只检查 User-origin 文件
- derive_ident_type 覆盖类型名 ident（构造器 callee 返回精确 nominal 类型）

### 步骤 5：inferred_type_args 在 HIR 填充
- fill_inferred_type_args 从 callee type_params + 签名 + 实参类型推断
- MIR 不再调 infer_type_args_from_call

### 步骤 6a：MIR fallback 路径删除
- lower_call 的 200 行 fallback 删除（替换为 enum variant 检测 + FunValue）
- member_call_target + resolve_owner_from_expr 删除（dead code）

### 步骤 6b：ResolvedCall 携带 param_types
- TopLevelFun 和 Method 增 param_types: Vec<TypeId>
- HIR 在所有 record_*/derive_* 函数中填充
- MIR emit_call_resolution 用 param_types 构建 overload_sig（不再查 hir.member_funs/top_level_funs）
- make_direct_call_kind_with_params 新方法

### 步骤 6c：Codegen intrinsic 分发
- intrinsic_map（@Intrinsic 注解名）为主路径
- FQN heuristic 仅对 @Intrinsic 方法生效（string_to_string 已排除）

## 验证清单

- [x] derive_call_resolution 对合法程序的所有 Call 节点返回 Some
- [x] MIR fallback 路径不再被合法程序触发（0 次 fallback hit）
- [x] 泛型调用的 inferred_type_args 在 HIR 填充
- [x] lower_infix_call/lower_index 消费 call_resolutions
- [x] completeness gate 检查 Nothing 类型（对 User-origin 文件）
- [x] block_lenient 移除
- [x] derive_ident_type 覆盖类型名 ident
- [x] MIR lower_call fallback 路径删除（200 行）
- [x] ResolvedCall 携带 param_types（MIR 不再查 hir.member_funs 构建 overload_sig）
- [x] Codegen intrinsic FQN heuristic 排除非 @Intrinsic 方法
- [x] cargo test 通过（scoop2_hir / scoop2_mir / scoop2_codegen_llvm 全绿）
- [x] run-pass fixture 不回归
- [x] no_placeholder 守卫通过

## 已知技术债

以下项目不影响正确性，但偏离理想架构。它们是声明信息查询或防御性 fallback，
不是 call resolution 泄漏到 MIR。

### MIR 中的声明信息查询（非 resolution）
- `hir.type_constraints`：查询函数的类型参数名序列（用于 stable_template_key 构建）。
  这是声明信息（类型的结构属性），不是 call resolution。
- `hir.interner.resolve()`（43 处）：Symbol → String 转换（用于 FQN 字符串构建）。
  这是 intern 表查找，不是 resolution。

### MIR 中的防御性 fallback（仅错误程序触发）
- `member_overload_sig`（2 处调用）：lower_unary 和 lower_infix_call 的 fallback，
  当 call_resolution 缺失时使用。合法程序不触发。
- `derive_enum_variant_call`：enum variant 构造检测 fallback。合法程序由 HIR EnumVariant 变体覆盖。
- `resolve_typeref`（11 处调用）：`is`/`as` 模式中的 TypeRef → TypeId 解析。
  这是类型引用解析（非 call resolution），理论上应由 HIR 在 expr_types 中预解析。

### store.nothing() 错误恢复路径（45 处）
这些是 typecheck 的错误恢复返回值（已报诊断后继续检查）。
对合法程序，completeness gate 检查到 Nothing 类型会报 `scoop::typecheck::untyped_node`。
Nothing 类型不会进入合法程序的 HIR 最终产出。

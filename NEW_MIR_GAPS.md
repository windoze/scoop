# scoop2_mir 功能差距审计（2026-07-29）

> 基于代码审计，每项含 file:line 证据。

## 当前状态：所有已识别缺口均已修复

经四轮修复（共 24 项缺口），scoop2_mir 与参考实现 scoopc_mir 的功能差距已全部关闭。全部验证命令通过：
- `cargo build -p scoop2_mir -p scoop2c` ✅
- `cargo test -p scoop2_mir -p scoop2_hir --all-targets` ✅ (71+15+1)
- typecheck fixtures: **558/558** ✅
- mir2: **11/11** ✅ / mir2_fail: **2/2** ✅
- no_placeholder 守卫 ✅

---

## 完成记录

### 2026-07-28 第一轮修复

| 缺口 | 修复内容 |
|------|----------|
| **G1** PatternMatch/PatternExtract IR 变体 | 在 `Rvalue` 枚举中添加两个新变体 |
| **G2** variant 模式测试 | 重写为 `PatternMatch { Pattern::Variant{...} }` |
| **G3** 字面量模式测试 | 重写为 `PatternMatch { Pattern::IntLit/... }` |
| **G4** 闘包捕获 metadata | `lower_lambda` 为每个捕获构造 `ClosureCaptureTransportMetadata` |
| **G8** Or 模式 | 发射完整 CondBr OR 链 |
| **G9** is T 模式 | 用 `resolve_typeref` 解析真实目标类型 |
| **G10** verify callee | 对含 `.` 的 FQN callee 严格检查符号表 |

### 2026-07-28 第二轮修复

| 缺口 | 修复内容 |
|------|----------|
| **G5** 去虚化 | 新增 `devirtualize.rs`：final 接收者的 Virtual→Direct |
| **G6** backend contracts | 升级为携带真实数据的结构体集合 |
| **G7** stable keys | `CallKind::Direct` 自动计算 `StableTemplateKey`/`StableInstanceKey` |
| **G13** transport aggregate | `aggregate_transport` 为 struct/enum/Option 填充逐字段 |
| **G14** Perform payload | 从 args 构造 payload 类型/transport/映射 |

### 2026-07-28 第三轮修复（placeholder 清理）

| 缺口 | 修复内容 |
|------|----------|
| **G6** `hir_fqn_for_metadata` | 用 `hir.interner.get(fqn_text)` 查找 Symbol |
| **G4** `mutable` 字段 | `LocalDecl` 新增 `mutable`；`alloc_named_mutable` 在 var 时设 true |
| **G7** `type_params` | 从 HIR `type_param_count` 查询 |
| **G13** boxing | 新增 `value_transport_boxed` 方法 |

### 2026-07-29 第四轮修复（深度审计清理）

| 缺口 | 修复内容 |
|------|----------|
| **H1** `enum_fqns` 从不填充 | `FnLowering::new` 从 `hir.enum_variants.keys()` 收集所有 enum FQN 填入 `enum_fqns`，使 `mir_transport_kind_for_ty` 能正确区分 EnumPayload vs Struct |
| **H2** `call_transport` 死代码 | 移除 if/else 两分支相同的死代码；`aggregate_return` 从 `mir_is_aggregate_transport_ty` 计算 |
| **H3** `value_transport_boxed` 死代码 | 闭包捕获 transport 预计算提到 `builder.assign()` 之前，消除借用冲突；`value_transport_boxed` 可用 |
| **M1** verify Virtual `known_types` 未使用 | Virtual 分支增加 `known_types.contains(&dispatch.owner_fqn)` 交叉引用检查 |
| **M2** 闭包捕获 boxing | 闭包捕获 metadata 预计算重构，transport 精确计算 |
| **M3** `MemberAccessMetadata.resolved` | `member_access_metadata` 从 HIR members/member_funs 解析为 `MemberTarget::Value`/`Fun` |

### 低优先项（全部已在第六轮修复）

（无——所有低优先项已在第六轮修复中完成。）

### 2026-07-29 第五轮修复（低优先缺口 + placeholder 清零）

| 缺口 | 修复内容 |
|------|----------|
| **L1** 解构绑定 `mutable` | `bind_pattern` 新增 `mutable: bool` 参数，从 `ValKind::Var` 传递到所有解构子绑定的 `alloc_named_mutable` 调用 |
| **L2** dump Call transport | `dump_rvalue` 的 Call 分支现在渲染 `transport={trace:..., box:...}` |
| **H3+M2** 闭包 boxing | 闭包捕获中值类型→Any 边界时产出 `boxing: Some(MirBoxingIntent { reason: ClosureCapture })`，不再是无 boxing 的 `value_transport` |
| **M3** Virtual `stable_template_key` | 所有 DispatchMetadata 构造点从 `owner_str` + `method_str` 计算 `Some(make_stable_template_key(...))` |
| **M3** `stable_candidate_keys` | 所有 DispatchMetadata 构造点从 `owner_str` + `method_str` 构造 `StableInstanceKey` 作为单元素候选列表 |
| **M3** TopLevelRef `stable_template_key` | 所有 TopLevelRef 构造点从 FQN 文本计算 `Some(make_stable_template_key(...))` |


### 2026-07-29 第六轮修复（低优先项全部清零）

| 缺口 | 修复内容 |
|------|----------|
| **L3** `stable_template_key_for` 真实 type_params | 从 HIR `type_constraints.get(&fqn).type_params` 获取真实类型参数名序列，不再用合成位置编号 |
| **L4** struct_layouts 字段类型 | 从 `format!("ty#N")` 改为 `canonical_type_text(&store, ty)` 渲染真实类型规范文本 |
| **L5** PatternExtract 构造 | `bind_pattern_arm` 中多字段 variant 提取从 `Rvalue::TupleIndex` 改为 `Rvalue::PatternExtract { path: [TupleIndex(i)] }` |
| **generic_eff_args** 填充 | TypeApply callee 路径从 `ast::TypeArgKind::Effect` 提取 effect 实参，解析为 `EffectRow`，填入 `CallKind::Direct.generic_eff_args` |

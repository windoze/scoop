# M4 设计：enum、模式匹配与 sysroot 框架

版本：0.2（实现同步）

对应 `docs/ROADMAP.md` 的 M4。目标：enum 与 `when` 扩展模式（守卫、穷尽性）、解构声明与 `..`（spec 第 4、5 章）；同时建立 **sysroot 框架**，把 M3 硬编码的 `Option` 与 M1/M2 硬编码的 `print`/`println` 正式落地为 `scoop.core` 库定义。

## 1. sysroot 框架

### 1.1 目录布局

```
sysroot/
  lib/
    scoop.core/
      Cone.toml
      src/*.scoop        # Option、print/println 等
```

`sysroot/` 承载整套编译环境：将来会有 `bin/`（工具链）等；`lib/` 专门放系统 Cone——现在是 `scoop.core`，将来可能有 `scoop.std` 等。

### 1.2 M4 的编译模型（bootstrapping）

完整的多 Cone 编译（`.slib`、依赖图）是 M12。M4 采用务实的过渡模型：

- driver 定位 sysroot（环境变量 `SCOOP_SYSROOT`，缺省为仓库内 `sysroot/`）；
- `scoop.core` 按 `Cone.toml` 读入全部 `src/*.scoop`，与用户源码**作为同一个编译单元**一起编译（同一作用域体系，core 的声明天然可见——这就是"默认导入"的临时实现）；
- `Cone.toml` 用 `toml` crate 解析（成熟库，不自己造），M4 只要求 `[cone]` 的 `group`/`name`/`version` 字段存在且 `name` 与目录名一致；
- 没有 `.slib`，没有跨 Cone 可见性检查——这些随 M12 落地，届时 core 改为预编译 `.slib` 形态，用户代码不再与 core 同单元编译。

### 1.3 注解语法与 intrinsic 登记表（最小落地）

为支撑 core 库调用 runtime，M4 落地 spec 13.1 的一个切片：

- parser 支持函数声明上的**单个注解**：仅 `@Intrinsic("name")`（其他注解一律 "annotations are not supported yet" 诊断）；
- 编译器内置 **intrinsic 登记表**（impl spec 2.10）：M4 只有一个条目族 `rt_print` / `rt_println`（签名规则"恰好一个 String/Int/Boolean 参数"、MIR 映射到对应 runtime shim、展开阶段 = HIR 识别 + MIR 符号映射）。未知 name = 编译错误（spec 13.1 已有此规则）；
- `@Intrinsic` 只允许出现在 sysroot Cone 中（用户代码使用报诊断）。M11 落地完整 FFI 注解族时放宽到 `@Extern` 等。

core 中的定义随之变为：

```
// sysroot/lib/scoop.core/src/io.scoop
@Intrinsic("rt_print")
fun print(message: String)
// println 同理
```

注意 M2/M3 的 `println(42)`（Int/Boolean）行为不变：登记表条目的签名规则覆盖三种类型，runtime shim 映射按实参类型分派（沿用 M2/M3 行为，fixtures 不变）。

## 2. 语言子集新增

### 2.1 enum 声明（spec 4.2）

```
enum Color { Red, Green, Blue }

enum Shape {
    Circle(Int),                       // 位置参数变体
    Rect(Int, Int),
    Named { w: Int, h: Int },          // block 式命名字段变体
    WithDefault(val d: Int = 0)        // 构造函数式（含默认值）
}

enum Option<T> {                       // 泛型 enum（core 库）
    Some(T),
    None
}
```

- 四种变体形式全支持；构造函数式变体的字段默认值按 spec 8.5 在调用处求值（M4 只支持常量表达式做默认值——完整缺省值机制属函数默认参数里程碑，见 5.4）；
- 泛型 enum 支持（`Option<T>` 是刚需）；泛型 **struct** 声明仍不做（见第 6 章）；
- enum 无成员函数、无构造函数、无 `init`（spec 4.2）；变体字段无可见性修饰符。

### 2.2 `when` 模式匹配（spec 第 5 章）

- 语句级 `when (subject) { arms }`（when 作为表达式不在 M4，见第 6 章）；
- 分支：模式 + 可选守卫 `if (cond)` + block；`else` 分支；
- 模式形态：单元变体（`None`）、位置参数变体（`Some(x)`、含字面量/绑定/`_`/`..`/嵌套）、命名字段变体（`Named { w, h: height, .. }`）、tuple 模式、struct 模式（位置或字段模式，类型名前缀可选）、字面量；
- 穷尽性：enum subject 必须覆盖全部变体或有 `else`（守卫分支不计入）；tuple/struct 模式按 spec 5.2/5.3 规则；不满足即诊断；
- M3 的 Option 观察手段（`?.`/`?:`/`!!`）保留，`when` 成为通用手段。

### 2.3 解构声明（spec 4.6）

- `val (a, _, ..) = tuple`（tuple 模式）；`val Point(x, _) = p` 与 `val Point { x, .. } = p`（struct 位置/字段模式）；嵌套；`..` 规则按 spec（位置模式至多一次、字段模式必须在最后、未列全必须写 `..`）；
- 解构只在 `val` 声明（不可反驳模式：tuple/struct）；enum 变体的解构只在 `when` 中（可反驳模式）；
- for 循环变量与 lambda 参数的解构不在 M4（无 for/lambda）。

## 3. 各 stage 设计

### 3.1 AST / parser

- `enum` 声明（四种变体、类型参数、变体字段默认值）；
- `when` 语句与模式语法（含守卫、`..`、嵌套、`else`）；
- 解构 `val` 声明（ValDecl 的绑定目标从标识符泛化为模式）；
- 注解：`@Intrinsic("name")` 仅函数声明、仅 sysroot；
- 模式的纯语法位置（`when` 分支、解构声明）不产生与表达式的歧义：`..` 在这些位置是 rest，区间运算符不在模式位出现（spec 4.6 消歧节）。

### 3.2 HIR

- **enum 落地**：`EnumId`（newtype arena 索引）、`EnumDecl { name, type_params, variants: Vec<Variant> }`、`Variant { name, kind: VariantKind }`、`VariantKind = Unit | Positional(Vec<Field>) | Named(Vec<Field>)`（Field 复用 struct 的）；
- `Type::Option(TypeId)` 移除，改为 `Type::Enum(EnumId, Vec<TypeId>/* 已解析的类型实参 */)`；`T?` 脱糖目标变为 core 的 `Option` enum 的 `EnumId`——HIR 在解析 core 声明后获得它（core 必须恰好定义一个名为 `Option` 的 enum，缺失/多个都是 driver 级错误）；
- 变体构造：表达式位 `Some(x)` / `Color.Red` / `Red`（上下文可确定类型时可省前缀，spec 4.2/5.1）解析为 `ExprKind::VariantConstruct { enum_id, variant, type_args, fields }`；`None` 字面量的期望类型推断逻辑（M3）保留；
- **when 语义**：subject 类型必须可模式化（enum/tuple/struct，spec 第 5 章统一规则）；分支检查（变体存在、元数、字段存在与类型、`..` 位置、守卫必须 Boolean）；模式绑定引入 Local（ty 取自变体字段）；穷尽性检查（守卫分支不计）；`else`；
- 解构声明：不可反驳模式（tuple/struct），规则同 4.6；
- **intrinsic 识别**：`@Intrinsic` 函数进入符号表，调用按登记表条目的签名规则检查；`Option` 的 M3 内建识别（`Some`/`None` 特殊名字）**删除**，由 enum 机制接管；`== None` 等比较走变体相等（见 3.3）。

### 3.3 MIR

- **单态化扩展**：enum 实例化（`Option<Int>` 等）——enum 定义随类型实参实例化（变体字段类型替换），与函数实例同一 worklist；mangling 类型编码加 enum（`E<名字>`）；
- **when 降级为决策序列**：每臂依次 = tag 比较（`EnumTag == variant index`）→ 字段提取绑定 → 守卫求值；失败进下一臂；`else` 兜底。M4 不生成跳转表/decision tree 优化；
- **enum 相等展开**（`==` 的 M2/M3 展开器扩展）：tag 相等 + 逐变体逐字段比较（复用递归展开框架）；M3 的 Option 特殊展开（IsSome/Unwrap 树）删除，统一走 enum 展开；`IsSome`/`Unwrap`/`SomeWrap`/`NoneLiteral` 这组 HIR 节点由 MIR 翻译为通用 enum 操作（tag 测试 / payload 提取 / 变体构造）——`?.`/`?:`/`!!` 的 HIR 脱糖结构不变；
- 模式绑定的局部变量在决策序列中先声明后绑定（守卫可用）。

### 3.4 LIR

- **enum 布局**：`{ i64 tag, [N x i8] payload }`（payload 起点满足最大变体对齐，N = 最大变体 payload 的自然对齐大小）；**niche 特例**（spec 7.4）：两变体、一变体无 payload、另一变体为单个 Ptr 字段 → 裸指针表示（这正是 `Option<String>`，M3 的布局 golden 必须保持通过）；
- `LirType` 增加 `Enum(EnumDefId)`；LIR meta 记录 enum 布局及每变体的递归 `RefScan`。当 tagged enum 内嵌进 struct / tuple / class / 装箱 payload 时，父布局保留 tag 相对偏移及各变体子扫描，不把各变体引用压平成无条件偏移；
- 指令新增：`EnumWrap { out, variant, fields }`、`EnumTag { out, operand }`、`EnumField { out, operand, variant, field_index }`（codegen：alloca 副本 + GEP + bitcast + load，注释说明）；niche 表示下这组指令按 M3 的 Ptr 规则翻译（null 测试等）；
- `scoop_rt_trap` 路径不变（`!!` 的 trap 仍走它）。

### 3.5 codegen

- 上述指令的机械翻译；enum 类型的 LLVM 表示（niche 或 `{i64, [N x i8]}`）；
- enum 按值使用时不单独生成 TypeDescriptor；装箱或嵌入堆对象时由外层 TypeDescriptor 携带递归扫描描述。

## 4. runtime

M4 无新增 runtime 函数（print/println 的 shim 已有；enum 不需要 runtime 支持）。

## 5. 临时决策（及退役里程碑）

1. **core 与用户代码同单元编译**（无 .slib、无 Cone 隔离）：M12 落地真正的多 Cone 后退役；届时 `Option` 的"默认导入"由 import 机制表达。
2. **注解只支持 `@Intrinsic`，且只在 sysroot**：M11（FFI 注解族）扩展。
3. **`!!` 仍 trap**：M8。
4. **构造函数式变体的默认值只支持常量表达式**：完整"定义处解析、调用处求值"（spec 8.5）随函数默认参数里程碑一起做。
5. **when 只做语句**：表达式形态的 when（产生值）在需要时单独立项。

## 6. 明确不做

- 泛型 struct 声明；`when` 表达式形态；enum 成员函数；错误恢复；lambda 与 for 的解构（无 for/lambda）；跳转表优化。

## 7. 文档同步项

- **runtime spec 2.2（已完成）**：TypeDescriptor 使用可组合的递归扫描描述；tagged enum 按 tag 选择 per-variant 子扫描，且可嵌入其他聚合布局；
- **spec 7.2 / 11.5**：`Option` 从"内建"改为"core 库真实定义"，迁移完成后把 M3 设计 5.1 的临时决策标记为已退役（在 ROADMAP M4 记录）。

## 8. 测试计划

- **独立 fixture**（`tests/fixtures/m4-enum/`）：四种变体形式的声明/构造/匹配；when 守卫与 `else`；穷尽性；tuple/struct 模式；嵌套模式；`..` 各种位置；解构声明（位置/字段/嵌套/`..`）；
- **组合 fixture**：core 的 `Option` 经 when 解构（`Some(x) ->` / `None ->`）；泛型 enum 多实例（`Option<Int>` / `Option<Point>` 共存，mangling）；`?.` 与 when 混用；enum 嵌套 struct/tuple 字段的匹配与相等；
- **迁移回归**：全部 M1–M3 fixture 必须原样通过（print/println、Option 相关行为不变；Option 布局 golden 不变）；
- **negative fixture**：非穷尽 when；`..` 两次/字段模式缺 `..`；未知变体/字段；守卫非 Boolean；绑定类型不匹配；`@Intrinsic` 未知 name；用户代码用 `@Intrinsic`；core 缺 `Option` 定义（构造一个坏 sysroot 的定向测试）。

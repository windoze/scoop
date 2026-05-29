# Static Reflection 与 Codable 设计草案

版本：草案 0.1（2026-05-29）
范围：整理现有零散的静态反射 surface，规划一套自洽的反射 + Codable（Encodable / Decodable）能力，并锁定已经决议的设计抉择。

---

## 1. 目标

静态反射应能完整覆盖以下查询：

1. 类型的名称和**泛型参数列表**（含 bounds 信息）。
2. 类型的**访问修饰符**和注解信息。
3. 类型的父类与接口列表。
4. 类型的**公共成员**列表，包括字段、方法和属性；每个成员包含访问修饰符、类型信息和注解信息。

Codable 应作为一组通用接口（`Encodable` / `Decodable`）：实现这组接口的类型可被序列化 / 反序列化，且**用户无须手写编解码代码**（auto-implementation）。

---

## 2. 现状盘点

来源：`docs/spec/language_spec-part5.md`、`sysroot/lib/scoop.core/src/core.scoop:528-1430`。

已有反射 surface：

- 类型句柄：`TypeMeta { name, kind, annotations }`，`TypeKind` 含 `Struct / Enum / Class / Interface / Effect / Tuple / Primitive`。
- 成员元数据：仅 `FieldMeta`（字段） / `VariantMeta`（enum） / `ParamMeta`（函数参数） / `PropertyMeta { name, type, owner }`（仅供 delegated property lowering，最简版）。
- intrinsic：`nameOf` / `sizeOf` / `alignOf` / `kindOf` / `descOf` / `fieldsOf` / `variantsOf` / `superTypesOf` / `annotationsOf` / `paramsOf(fn)`。
- splice 字段访问：`value.[fieldMeta]`（spec §5.4）。

对照第 1 节四点诉求的缺口：

| 诉求 | 现状 | 缺口 |
|---|---|---|
| 名称 + 泛型参数 + bounds | 仅 `name` | 缺 `typeParams: MetaList<TypeParamMeta>`，缺 `bounds` |
| 访问修饰符 + 注解 | 仅 `annotations` | 所有 meta 都缺 `visibility` 字段 |
| 父类 / 接口 | `superTypesOf<T>()` 单独 intrinsic | 没有区分直接父类与接口；建议直接挂到 `TypeMeta` 上 |
| 公共成员（字段 / 方法 / 属性） | 仅 `fieldsOf` | 缺 `methodsOf` / `propertiesOf` / `constructorsOf`；`PropertyMeta` 太薄 |

---

## 3. 元数据模型扩展

补成一组正交 struct，全部带 `annotations` + `visibility`：

```kotlin
public enum Visibility { Public, Internal, Protected, Private }

public enum Variance { In, Out, Invariant }

public struct TypeParamMeta {
    public val name: String
    public val index: Int
    public val bounds: MetaList<TypeMeta>     // upper bounds（含 interface 约束）
    public val variance: Variance
    public val hasDefault: Bool
    public val defaultType: TypeMeta?         // 仅当 hasDefault
}

public struct TypeMeta {
    public val name: String
    public val kind: TypeKind
    public val visibility: Visibility
    public val typeParams: MetaList<TypeParamMeta>
    public val superTypes: MetaList<TypeMeta>     // 直接 supertype 列表
    public val annotations: MetaList<AnnotationMeta>
}

public struct FieldMeta {
    public val name: String
    public val type: TypeMeta
    public val index: Int
    public val visibility: Visibility
    public val isMutable: Bool
    public val annotations: MetaList<AnnotationMeta>
}

public struct PropertyMeta {
    public val name: String
    public val type: TypeMeta
    public val owner: TypeMeta
    public val visibility: Visibility
    public val hasGetter: Bool
    public val hasSetter: Bool
    public val annotations: MetaList<AnnotationMeta>
}

public struct MethodMeta {
    public val name: String
    public val visibility: Visibility
    public val typeParams: MetaList<TypeParamMeta>
    public val params: MetaList<ParamMeta>
    public val returnType: TypeMeta
    public val effects: MetaList<TypeMeta>     // required effect set
    public val modifiers: Int                  // bitset: open / abstract / override / operator / ...
    public val annotations: MetaList<AnnotationMeta>
}

public struct ConstructorMeta {
    public val params: MetaList<ParamMeta>
    public val visibility: Visibility
    public val annotations: MetaList<AnnotationMeta>
}
```

四点诉求 1:1 落到 struct 字段上。`VariantMeta` / `ParamMeta` / `AnnotationMeta` 保持现状不动。

---

## 4. Intrinsic Surface

按"查类型 / 查成员 / 过滤"分层，把现有几个 intrinsic 收口：

```kotlin
// 类型本体
@Intrinsic public fun <T> typeOf(): TypeMeta              // 总入口；nameOf / kindOf 是它的快捷
@Intrinsic public fun <T> nameOf(): String
@Intrinsic public fun <T> sizeOf(): Int
@Intrinsic public fun <T> alignOf(): Int

// 成员（全集，含非 public）
@Intrinsic public fun <T> fieldsOf(): MetaList<FieldMeta>
@Intrinsic public fun <T> propertiesOf(): MetaList<PropertyMeta>
@Intrinsic public fun <T> methodsOf(): MetaList<MethodMeta>
@Intrinsic public fun <T> constructorsOf(): MetaList<ConstructorMeta>
@Intrinsic public fun <T> variantsOf(): MetaList<VariantMeta>

// 关系
@Intrinsic public fun <T> superTypesOf(): MetaList<TypeMeta>   // = typeOf<T>().superTypes
```

"只要 public 成员"做成 sysroot 普通函数，不进 intrinsic：

```kotlin
public fun MetaList<FieldMeta>.publicOnly(): MetaList<FieldMeta> =
    filter { it.visibility == Visibility.Public }
// MethodMeta / PropertyMeta / ConstructorMeta 同理
```

语言层只承担"暴露元数据"，过滤是库代码，更可演进。

---

## 5. Codable 接口

```kotlin
public interface Encodable {
    public fun encode(encoder: Encoder)
}

public interface Decodable {
    // 纯 marker：不放方法（避免引入 Self）
}

public interface Encoder {
    public fun encodeInt(name: String, value: Int)
    public fun encodeString(name: String, value: String)
    public fun encodeBool(name: String, value: Bool)
    public fun encodeNested(name: String, value: Encodable)
    public fun encodeList<T : Encodable>(name: String, value: List<T>)
    // ...
}

public interface Decoder {
    public fun decodeInt(name: String): Int
    public fun decodeString(name: String): String
    public fun decodeBool(name: String): Bool
    // ...
}
```

实现样例：

```kotlin
class User : Encodable, Decodable {
    val name: String
    val age: Int

    // auto-impl 合成：
    override fun encode(encoder: Encoder) { ... }

    companion object {
        // auto-impl 合成（返回类型是具体类型 User，不是 Self）：
        fun decode(d: Decoder): User { ... }
    }
}
```

`Decodable` 接口里**不放任何方法**，纯作为"请帮我合成 companion.decode"的标记。绕开 Self 这条决策（见 §8）使得这套形状成立。

---

## 6. Auto-implementation 机制

采用**反射 + 单态化常量折叠**路线（"路径 2"），不在 HIR 里加"合成方法体"专用阶段。

sysroot 提供两个核心 intrinsic：

```kotlin
@Intrinsic public fun <T : Encodable> encodeReflective(value: T, encoder: Encoder)
@Intrinsic public fun <T : Decodable> decodeReflective(decoder: Decoder): T
```

`encodeReflective` 的概念实现：

```kotlin
public fun <T : Encodable> encodeReflective(value: T, encoder: Encoder) {
    for (field in fieldsOf<T>().publicOnly()) {
        when (field.type.kind) {
            Primitive -> encoder.encodeInt(field.name, value.[field])
            // ...
        }
    }
}
```

`decodeReflective` 同理，遍历 `constructorsOf<T>()` + 每个参数的 `decode` 调用，折叠为构造器调用。

合成规则：

- `: Encodable` 且没有手写 `encode` → HIR 合成 `override fun encode(encoder) = encodeReflective<This>(this, encoder)`。
- `: Decodable` 且 companion 里没有 `decode(d: Decoder): ThisType` → HIR 合成（如果连 companion object 都没有，一并合成一个未命名 companion）。
- 用户手写了对应方法，合成跳过。

机制要求：

- `fieldsOf<T>()` / `constructorsOf<T>()` / `propertiesOf<T>()` 在单态化阶段必须能展开成编译期常量列表；`value.[field]` 必须能折叠为具体字段访问（这是 spec §5.4 已经承诺的）。
- 该机制可被任何"按字段遍历"的算法复用：`==`、`hashCode`、`toString`、自定义 JSON / TOML 编码器都共用同一条 lowering 路径。

替代路径（**未采用**）：

- 路径 1（HIR 合成方法体）：用户面更"魔法"，但需要新的 HIR 合成阶段、新的 "什么算缺失实现" 边界。
- 路径 3（运行期反射）：明确违背 spec §5.1，不考虑。

---

## 7. 泛型上下文的 decode

`<T : Decodable> fun decode<T>(d: Decoder): T` 这种调用没法靠接口分派到伴生 object（因为 `Decodable` 接口是空的）。统一答案：**走反射 intrinsic**。

```kotlin
val u: User = decodeReflective<User>(d)
```

伴生 object 的 `decode(d: Decoder): User` 合成体也只是 `return decodeReflective<User>(d)`，**伴生方法只是对外的稳定调用点**，实质实现统一在反射 intrinsic 里。

这意味着 `Decodable` 标记接口的唯一作用是触发 auto-impl；用户也可以**不**实现 `Decodable` 而直接调 `decodeReflective<T>(d)`，前提是 `T` 的所有字段类型都可解码。

---

## 8. 锁定的设计决议

| # | 决议 | 理由 |
|---|---|---|
| D1 | **不引入 Self type** | 静态方法 / 构造方法没有 vtable dispatch 锚点，引入 Self 等于被迫先引入 metatype 或 trait witness。Self 在输入位置破坏 LSP；继承链上的 Self 再绑定要么强制重写要么自动合成，两条都"魔法"；overload resolution / substitution / effect inference 都要新增一条隐式变量路径。代价远大于收益（"在 companion 里写具体类型"是一行字的代价）。Kotlin 没有 Self 也工作良好，作为参考。 |
| D2 | **`Decodable` 是空 marker 接口** | 配合 D1。`decode` 不是接口方法，而是合成在伴生 object 上、返回具体类型的函数。 |
| D3 | **Auto-impl 走反射 + 单态化折叠（路径 2）** | 与 spec §5.1 "静态反射"哲学对齐；复用现有 splice 字段访问 + `fieldsOf` lowering，无需新的 HIR 合成阶段；机制可被其它"按字段遍历"算法复用。 |
| D4 | **过滤公共成员是库函数，不是 intrinsic** | 语言层只暴露完整元数据，过滤策略可演进。 |
| D5 | **`@Retention` 中 `"cone"` 档暂时悬置** | `.cone` archive 功能已从主线移除（见 §9）；后续 `.cone` 重新设计时再决定注解 retention 如何与之挂接。 |

---

## 9. 已知的待清理项 / 重新设计项

- **`.cone` archive 已从主线移除**。相关代码需要清理（独立任务），暂不在本反射设计范围内。受影响：
  - spec §5 part 12.2 的 `@Retention("cone")` 档：当前没有承载介质，注解如何"导出到下游"需要随 `.cone` 重新设计一并定义。
  - 注解可见性 / 跨 cone 访问的边界条件。
  - 在新的 cross-package 元数据机制确定之前，本设计**只承诺单 package 内的反射元数据可见性**。
- 现有 sysroot 中零散的反射 intrinsic（`nameOf` / `kindOf` / `descOf` / ...）保留，但 `typeOf` 落地后这些应被视为快捷别名，文档需更新。

---

## 10. 待决议的开放问题

1. **Companion object 是否扩到 struct / enum**
   spec §2.4 当前只允许 class 声明 companion。如果 `Decodable` 要覆盖 struct（如 `struct Point : Decodable`），有两条路：
   - (a) 把 spec §2.4 扩到 struct / enum 也能有 companion。
   - (b) 仅 class 支持 `Decodable`；struct 必须直接调 `decodeReflective<Point>(d)`。
   倾向 (a)：否则"Encodable 能给 struct 用，Decodable 不能"的不对称会让人迷惑。需独立任务正式决议。

2. **`Decodable` marker 实施范围**
   是否对所有引用类型都合成 companion，还是仅在用户显式 `: Decodable` 时合成。倾向"仅显式声明"，避免编译开销与意外的 ABI 暴露。

3. **错误处理 / `Raise<DecodeError>`**
   `decodeReflective` 是否要把 decode 失败建模成 `Raise<DecodeError>` effect，还是让 `Decoder` 接口自身决定。倾向前者（与 spec §5 effect 模型一致），但需要确认 `RuntimeError` / 用户自定义 error 的定位边界。

4. **List / Map / Option / 自定义集合的 codable 形状**
   sysroot 的 `Encoder` / `Decoder` 接口需要给常用集合类型提供专用方法，还是统一靠 `T : Encodable` 递归。需独立任务讨论 API surface。

5. **MethodMeta 的 modifiers 位定义**
   bitset 的具体位分配（哪一位代表 open / abstract / override / operator / inline / ...）需要稳定下来作为 sysroot 契约。

---

## 11. 阶段化计划（建议拆分顺序）

每一步独立 commit / TODO 编号。

**前置任务**：

0. **清理 `.cone` archive 实现**：把已从主线移除的 `.cone` archive 相关残留代码、注释、spec 引用、fixture 和测试清理干净；同时把 spec §5 part 12.2 中 `@Retention("cone")` 档暂时降级为"保留 marker，无承载介质"或临时移除，待 `.cone` 重新设计后再决定。完成后本反射设计的后续步骤才能在干净的元数据基线上展开。

**主线步骤**（第 1-3 步是 schema 与 spec，越早改越省事）：

1. **Spec §5 + core.scoop**：扩 `TypeMeta`；新增 `MethodMeta` / `ConstructorMeta` / `TypeParamMeta` / `Visibility` / `Variance`；为现有 `FieldMeta` / `PropertyMeta` 加 `visibility` / `annotations` / `isMutable` / `hasGetter` / `hasSetter`。
2. **Spec §5 + core.scoop**：补 `typeOf` / `methodsOf` / `propertiesOf` / `constructorsOf` intrinsic 声明面，实现先留空让 typecheck 通过。
3. **Spec §2.4**：companion object 扩到 struct / enum（取决于 §10 问题 1 的决议）。
4. **HIR / codegen**：`fieldsOf` + splice 字段访问端到端单态化跑通（auto-impl 路径 2 的可行性 gate）。
5. **Sysroot**：`Encoder` / `Decoder` 接口 + `Encodable` / `Decodable` 接口 + `encodeReflective` / `decodeReflective` intrinsic 声明。
6. **HIR**：auto-impl 合成（`Encodable.encode` 实例方法 + `Decodable` companion.decode）。
7. **HIR / codegen**：`decodeReflective` 单态化折叠（构造器调用 + 字段写入序列）。
8. **Fixtures**：端到端回归（一个具体的 `JsonEncoder` / `JsonDecoder` 实现 + 多类型 round-trip 用例）。

`.cone` archive 重新设计是平行任务线，不挡这条主线 —— 本设计在重新设计完成前只承诺单 package 内的元数据可见性。

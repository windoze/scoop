# M38 设计：TypeInfo、结构化调试输出与 CLI 参数解析

状态：已完成并验收，2026-10-16。初稿：2026-10-11；对齐 M46 的修订：2026-10-14。实现基于 M48 最终提交 `f75704bac`，交付及三目标验证见 [ACCEPTANCE.md](ACCEPTANCE.md)，过程见 [PROGRESS.md](PROGRESS.md)。

本里程碑把 M29 的编译器内部静态类型描述开放为程序可读取的 `TypeInfo<T>`，在 core 提供面向文本 writer 的 debug printer，并以独立 `scoop.cli` Cone 提供参数解析和 help 生成。实例读取沿用普通成员访问与方法合成规则，不增加按反射调用点授予权限的机制。

本目录规定本期交付范围。类型描述与 debug 以 M37 完成后的语言能力为基础；CLI 同时采用 [M46 的 Decoder 合同](../milestone46/DESIGN.md)及[统一 Serialize／Option 默认规则](../milestone46/SERIALIZE.md)。每批先同步对应 spec，再修改实现。

## 1. 文档与交付范围

| 文档 | 内容 |
| --- | --- |
| [TYPE-INFO.md](TYPE-INFO.md) | `TypeInfo<T>`、`TypeRef`、成员、构造关系、声明处 Option 事实、注解及类型使用位置描述 |
| [DEBUG.md](DEBUG.md) | `Inspectable<T>`、只读值视图、普通访问控制、`TextWriter` 与 debug printer |
| [CLI.md](CLI.md) | 独立 CLI Cone、CLI 注解与 Serialize 映射、schema、argv 语法、稳定 Decoder、help 与错误 |

必须交付的闭环：

1. 无须构造 `T`，程序即可取得其字段名称、完整类型、注解及构造参数关系；跨 Cone 使用得到相同描述。
2. 有正常访问权限的实现产生只读值视图；core 将视图写到调用方提供的 `TextWriter`，支持嵌套值、集合和循环引用。
3. 应用只依赖普通 `scoop.cli` 库，即可用类型描述生成 help，再通过现有 `Decodable<T>` 构造参数对象。
4. library `.slib`、下游仅消费产物、泛型具体化、链接及运行均形成真实用例；不能只交付编译器内部查询测试。

## 2. 关键决定

### 2.1 类型描述与读取实例分开

`typeInfo<T>()` 的结构内容由编译器决定，结果是运行期可遍历的只读数据。它不要求 `T` 有无参构造器，不执行 `T` 的 initializer、getter、constructor、参数 default 或 companion 初始化。

`TypeInfo` 不提供通用 `FieldInfo.read(object)`、`set` 或按字段拼装对象的入口。取得一个私有成员的描述不等于取得读取该成员的函数。

实例观察采用普通 `Inspectable<T>.inspect(value)` 方法，产生 `ValueView`。调用者显式选择 inspector；debug printer 只处理该视图。方法可以手写，也可以按与现有 codec 相同的模式显式请求缺省 body 合成。合成结果是普通 typed 成员读取、调用、闭包与构造。

### 2.2 不建立新的访问控制规则

字段读取按其函数定义处的正常词法作用域、Cone、成员 visibility、owner 及 protected receiver 规则检查。合成方法使用它实际所属声明的同一上下文。编译器不得把外部 helper 移入被观察类型的内部，以使本来非法的读取合法。

因此，类型的 companion 中的实现可以合法读取宿主 private 状态；外部实现若没有相同访问权限，则按普通成员访问报编译错误。已经合法产生的读取闭包交给 printer 后，printer 调用的是普通函数，不重新取得字段访问权限。这与返回一个读取私有状态的普通 lambda 相同。

不增加 `AccessContext`、调用点权限参数、反射专用访问许可、动态 `isAccessible` 检查、core 特权或根据调用者位置变化的 `FieldInfo`。泛型具体化不改变方法定义处的访问域；`inline` 仍按 M37 忽略。

运行期从字段数组中选择一个 `FieldInfo`，不能据此让编译器按最终选中的字段产生编译错误。本设计不把普通循环扩展为编译期循环，也不赋予 `.read()` 调用特殊编译语义。

### 2.3 writer 属于 core，CLI 属于独立 Cone

| 所在位置 | 职责 |
| --- | --- |
| `scoop.core` | 类型描述、注解数据模型、`Inspectable<T>` 与值视图、`TextWriter`、`StringWriter`、`DebugPrinter` |
| `scoop.cli` | CLI 注解、schema、参数语法、归一化输入、`CliDecoder`、help 与错误展示 |
| 应用程序或 IO 库 | stdout、stderr、文件、日志等 writer；错误退出码和进程生命周期 |

`DebugPrinter` 和 help 生成只调用 `TextWriter.write(String)`，不绑定 `print`、`println`、stdout 或 stderr。core 不依赖 CLI；编译器不解释 CLI 注解的参数解析含义。

### 2.4 复用现有 Decodable

CLI 的处理过程是：

```text
TypeInfo + CLI annotations ──> CliSchema ──> help ──> TextWriter
                                  │
argv ──> token parsing ──> logical input ──> CliDecoder
                                              │
                                 explicit Decodable<T>
                                              │
                                     normal constructor ──> T
```

TypeInfo 提供解析规则的输入，Decoder 提供 M46 的 keyed/unkeyed/single-value 视图协议。CLI 直接提供已输入值，Option 的 Some 由普通 codec 构造；缺失字段不进入逻辑 record。default 求值和对象构造继续由所选 codec 完成。CLI 不通过字段偏移写内存、不先分配半成品对象、不用 setter 补齐状态，也不借 JSON 文本中转。

### 2.5 默认字段合同与 M46 对齐

CLI 自动 schema 使用 Serialize.rename／ignore／required；emitNull 是编码选项，不影响命令行的解码和 help。选项 alias、flag、位置参数及说明文字仍由 scoop.cli 的注解表达。schema 和 help 使用同一份已经构造的字段数据。

参与字段的必填性由 Serialize.required、constructor hasDefault 和成员的 isOptionAtDeclaration 决定。这个新增的只读成员属性保留原声明在 alias 展开后、宿主具体化前的外层 Option 事实；完整 TypeRef 仍展示具体类型。原字段 T 后来取 Option 不因此获得隐式 None，原字段 T? 则可以。M46 的缺省合成与 M38 描述生成复用同一已绑定类型事实，具体公式见 [CLI 默认 schema](CLI.md#51-默认-record-schema)。

TypeInfo 不描述任意 codec 正文；上述合同只对应统一的默认模型。手写 codec 或不同字段值表示使用显式 schema，不能从运行期 codec 对象推断其全部输入规则。公开 hasDefault 与原声明类型事实足够完成本期 CLI 的必填判断，不公开可执行 default 或反射构造入口。

CliDecoder 对已归一化节点提供复读与独立 unkeyed 游标。其格式没有 null，isNull 对合法值返回 false，readNull 报错；文本 "null" 保持文本。候选 codec 可对同一值重试，根入口不再检查旧的一次性 requireComplete；argv 语法、重复参数和未知 option 仍由 schema/parser 在各自边界检查。

TypeInfo、Inspectable 与 TextWriter 保留各自职责。debug 展示源码名称、真实 Option variant、惰性子值和对象 identity，不采用 Serialize 的字段投影；M46 的容器复用不改变这个观察模型。

## 3. 与 Swift Mirror 的关系

| 维度 | Swift `Mirror` | M38 |
| --- | --- | --- |
| 查询起点 | 一个运行期值 | `TypeInfo<T>` 不需要值；观察值另用 inspector |
| 字段内容 | children 中的 label 和类型擦除后的 value | 类型描述保留字段声明、完整类型、注解；读取函数另外提供 |
| enum | 主要观察当前值的 case | 类型描述列出全部 variant；值视图只读活动 variant |
| 自定义展示 | `CustomReflectable` 可自定义 mirror | 手写 inspector 可选择展示内容，不改写真实 TypeInfo |
| 泛型与访问 | Swift 自身的反射机制 | Scoop 的单态化、显式依赖、普通方法与访问检查 |

M38 不照搬 Mirror 的 `Any` children。不同类型的字段通过非泛型 `ValueView` 汇合，但产生视图的适配函数始终保留字段的实际静态类型。CLI 也无需先 `T()` 再寻找带配置的 property wrapper。

## 4. 编译器与产物落点

### 4.1 复用既有类型事实

现有入口是 [HIR 静态描述](../../compiler/hir/src/static_shape.rs)，其字段、property、constructor、注解与类型使用位置模块提供所需语义。M38 从这些已检查的事实生成库值，不另建一套语言语义检查器。

主要参照实现：

- [静态描述字段](../../compiler/hir/src/static_shape/fields.rs)与[property](../../compiler/hir/src/static_shape/properties.rs)：保留成员身份、声明顺序与实际存储关系。
- [类型使用位置](../../compiler/hir/src/static_shape/type_use.rs)与[注解](../../compiler/hir/src/static_shape/annotations.rs)：保留完整替换后的类型及 annotation target。
- [sourceLocation 具体化](../../compiler/hir-lower/src/concretize/body/source_location.rs)：编译器已能把查询变成普通库对象与常量。
- [编码方法合成](../../compiler/hir-lower/src/class/inheritance/encoding.rs)与[解码 record](../../compiler/hir-lower/src/class/inheritance/decoding/records.rs)：普通方法、访问、构造与 default 顺序的先例。
- [core Decoder](../../sysroot/lib/scoop.core/src/decoding.scoop)及[JSON Decoder](../../sysroot/lib/scoop.json/src/decoder.scoop)：CLI 直接实现相同协议。

M46 的字段类型／构造关系查询与 M38 描述生成复用已检查的 HIR 事实；成员的 isOptionAtDeclaration 从原声明类型取得，不能对具体化后的 TypeRef 重新推断。合成 encode／decode／inspect 可以复用成员访问检查、接口依赖选择和普通 body 构造 helper；各自的字段参与规则仍由相应功能负责，不把 Serialize.ignore 过滤后的字段集合直接用作 debug 视图。

### 4.2 阶段职责

| 阶段 | M38 工作 |
| --- | --- |
| AST / parser | 接受已规定的查询调用形态；不新增 static-for、field expression 或访问修饰符 |
| HIR lowering | 绑定实际 TypeInfo/annotation 查询；保持 nominal/member/annotation 的 typed 引用及原声明类型事实；正常检查 inspector requirement 与依赖 |
| 泛型 HIR / 共享模板 | 保存带 binder 的查询和定义处已绑定的 inspector body；不提前擦成字符串或消费方访问域 |
| concrete HIR | 替换完整类型实参，生成具体描述及普通对象构造；保留原成员 isOptionAtDeclaration，实例化已经选定的读取函数 |
| MIR / LIR | 消费正常的常量、分支、对象、闭包和调用；不负责 annotation 解释或成员权限决议 |
| codegen / `.slib` / linker | 沿用现有数据、函数、依赖和 ODR 规则发布、消费与合并 |
| runtime | 使用普通对象、根、闭包与 GC；不增加反射专用字段访问入口 |

实现需要分别保存“描述请求”和“实际读取函数的使用”。只查询 help 所需描述不能导致生成或调用所有类型的 inspector。

### 4.3 递归、单态化和身份

描述中的类型边引用独立 `TypeRef`，不递归复制整个类型树。构造一个类型引用不强制构造其完整描述；展开当前层也不强制展开所有子描述。因此 `Node.next: Option<Node>` 可有限表示，不形成 eager singleton 初始化环。

类型、字段、variant、property、constructor、参数与 annotation 使用各自的 typed identity。名字仅供展示与库规则匹配，不作为类型或声明身份。不同实例化沿完整 application 区分；alias 不产生新类型。

按实际使用生成描述与读取函数，复用已有实例化闭包、依赖失效、共享定义及缓存。运行期惰性描述不能掩盖编译期无限增长的类型实参；生成的描述函数同样服从既有 polymorphic recursion 检查，具体边界见 [TypeInfo 生命周期](TYPE-INFO.md#6-生成存储与生命周期)。不得添加全程序类型扫描、按名称注册所有类型、通用实例化预算或新的产物来源授权。

## 5. 规范同步范围

以下是实施时必须同步的规范范围；本次设计文档不把未实现能力写成已交付事实。

| 规范 | 需修订内容 |
| --- | --- |
| [语言规范](../specs/SCOOP-SPEC.md) 2.2、9.6、15 | 将“完全排除运行期反射”收窄为本设计的静态生成描述及显式实例观察；保留动态按名称调用、构造等排除项 |
| 语言规范 9.4、9.6 | TypeInfo/annotation 查询、完整类型引用及原成员 isOptionAtDeclaration、M37 注解 target/type-use/嵌套值的公开表示；Serialize 沿用 M46 声明 |
| 语言规范 9.5、11.13 及新增 core 小节 | `Inspectable<T>` 的显式合成、值视图、TextWriter 与 debug；不改变 9.1.5 访问规则 |
| 语言规范 11.13、12.4.4、12.6 | CLI 复用 M46 稳定 Decoder、Option 与缺省字段合同；独立 Cone，完整 argv 与参数列表入口的差别 |
| [实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.10、2.17 | 查询的 typed 登记、描述生成、inspector 合成、共享模板与跨 Cone 消费 |
| 实现规范相关 `.slib` 章节 | 按实际新增 wire 内容升级受影响 section；不预先更改不受影响的全部版本 |
| [运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 2.2、2.7、第 3 章 | 澄清 TypeInfo 是普通库数据，已有 TypeDescriptor/初始化/GC 合同保持；同步引用 |

M37 的整型枚举、嵌套 annotation、type-use 与 repeatable 数据模型是输入。M38 不再次定义一套注解常量求值规则。三份 spec 的章节交叉引用随实际修订一起更新。

## 6. 实施批次

| 批次 | 交付与退出条件 |
| --- | --- |
| M38-0 | 先完成 debug 与 CLI 共用的普通 TextWriter／StringWriter；独立、跨 Cone、异常和 moving GC 运行通过，不依赖 TypeInfo 或新的编译器 intrinsic |
| M38-1 | 同步基础 spec；完成 TypeInfo/TypeRef/TypeShape 与 typed 身份，struct、tuple、intrinsic 的源码查询可运行 |
| M38-2 | 完成 enum、class/property、构造参数、原声明 isOptionAtDeclaration、递归类型、generic/alias 与完整类型使用树；本地和上游 `.slib` 描述一致 |
| M38-3 | 完成 annotationKey、注解声明/应用/值与 M37 target；默认参数、数组、嵌套值及重复次序跨 Cone 保留 |
| M38-4 | 完成 Inspectable 与惰性值视图、显式合成、依赖组合；普通可见性正反例和跨 Cone 模板通过 |
| M38-5 | 在文本 writer 基础上完成 DebugPrinter；完成嵌套输出、redact/ignore、集合、截断与 moving GC 循环测试 |
| M38-6 | 在 M46-3 的 Serialize／默认字段合同上建立独立 scoop.cli；完成 CLI 注解、自动/显式 schema、help 与泛型必填一致性，help 不构造参数对象 |
| M38-7 | 在 M46-1／3 的稳定 Decoder／Option codec 上完成 argv、别名/位置参数/重复值归一化、复读与实际 Decodable 构造闭环；不保留 Some 输入包装或一次性消费状态 |
| M38-8 | 完成独立 `.slib` 消费、库组合、文档示例与正式 CLI 验收，记录三个 target 实际验证范围 |

每批只对实际实现声明完成；先格式化和 lint，再测试。先完成本机实现与必要验证，再统一安排 Linux 验收，不逐个小改动往返两个平台。测试失败或新变更决定后续验证范围，不反复重跑不受影响的全量验证。原始日志保存在仓库外，文档只记录结果概要。

M38-1～5 可独立于 M46 推进；M38-6／7 接入上述 M46-1／3 交付，不等待 M46 的 JsonValue、API／Cone 基线或 JSON 编码共享容器工作。里程碑编号不表示未实施部分的依赖顺序。M46 自身的合成不依赖 TypeInfo 或 M38 完成，两个方向不形成实施依赖环。

## 7. 验收重点

| 类别 | 必须覆盖 |
| --- | --- |
| 类型描述 | 字段顺序/名称/完整类型；同名不同声明；泛型实参替换与原声明 Option 事实；递归；alias；class base；intrinsic 不伪装为空 record |
| 注解 | 参数默认值、空数组类型、重复应用顺序、嵌套 annotation、整型 enum、type-use、property/getter/backing target 区分 |
| 普通访问控制 | 同 Cone internal、跨 Cone internal、file-private、companion private、protected receiver；手写与合成相同；调用者位置和 inline 不改变结果 |
| 值视图 | 活动 enum payload、惰性读取、实际静态类型、显式自定义、generic inspector 依赖、集合逻辑元素 |
| debug | 精确字符串转义、写入次序、writer 抛异常、ignore/redact 不读值、Serialize 不改源视图、真实 Option variant、共享非循环对象、真实环、GC 搬迁、base 投影不误报环 |
| CLI | argv[0]、长短选项、短名组合、负数、`--`、空值与文本 null、重复、Serialize 字段规则、Option／数组组合、enum 名称映射；help／位置布局与实际缺失结果一致 |
| Decodable | default 在容器结束后按序至多一次；错误值不回退 default；稳定复读、独立游标、isNull 恒 false；手写 scalar wrapper／候选重试；正常 constructor 异常传播 |
| 产物与组合 | model/annotation provider + core/CLI + consumer 仅用 `.slib`；re-export；generic body 再次发布；core-only debug 不依赖 CLI |
| 诊断与 stage | 新增编译错误有定位明确的 negative fixture；少量代表用例验证必要的 HIR/MIR/LIR 结构，不保存普通功能全量 dump；配置/输入错误使用运行期库测试 |

## 8. 范围边界

本里程碑不增加任意 CTFE、static-for、泛型虚方法、`reified` 语义、运行期按类型名查找/调用/构造、字段写反射或全局 inspector registry。私有访问不增加新规则。

CLI 首版围绕单个命令的 options/flags/positionals；subcommand、环境变量/配置文件合并、shell completion 和自动嵌套 record 展平留待有具体需求时独立设计。此 CLI 库不要求重写 `scoop`/`scoopc` 自身的 Rust 参数解析器。

writer 只抽象文本写入，不在本任务搭建通用 IO、flush/close、异步 writer 或日志框架。debug 输出不承诺可逆序列化、并发一致快照或稳定的持久化格式。

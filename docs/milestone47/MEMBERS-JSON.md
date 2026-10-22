# M47 类型与成员 JSON

本文件细化 [JSON 数据编码](JSON.md)中的类型及成员描述。所有引用、类型、常量、来源沿该文件，查询上下文是本次实际方法。数组保持原声明顺序；描述一次只展开一层，其他名义类型继续以完整类型引用表达。

## 类型描述

TypeDescription 为 `{type, name, parents, shape}`。type 与 parents 都是完整 Type；shape 区分 unit、any、nominal、tuple、function、pointer、nativeFunctionPointer、parameter、companion。结构类型在 type 内保留所有组成，shape 保留相应组成类型；没有成员的结构类型不能冒充名义声明。

nominal shape 包含 `declaration`、`nominalKind`、`arguments`、`parameters`、`companion`、`annotations` 和 `origin`。nominalKind 区分 struct／enum／class／object／interface／intrinsic；intrinsic 使用既有编译器 intrinsic 名称明确具体语义。parameters 使用原名义声明的 binder 和完整约束，arguments、parents、companion 属于查询上下文。名义声明来源取实际源码声明或依赖中已有的定义来源。

parameter shape 的 parameter 保留本次方法上下文中的 binder、声明引用、名称、约束和来源。companion shape 保留 host 参数及其 companion 约束，不执行 singleton 初始化。

## 成员描述

MemberDescription 为 `{type, declaration, parameters, fields, variants, properties, constructors, methods}`。声明参数属于原名义声明。非名义类型返回 null；原生名义声明可有空字段及构造器列表，但其真实方法仍可查询。Unit、Any、Ptr 等有结构表示的 core 类型仍通过实际 core 声明读取成员，不能按结构种类直接丢弃。

TypeUse 为 `{declarationType, appliedType}`。普通字段／属性／构造器的 appliedType 使用查询上下文。泛型方法另引入一层自身 binder：自身参数为 depth 0，插入的查询上下文参数提升一层；声明类型仍按原宿主和自身参数解释。方法的 applied binder 约束使用同一替换。方法声明 identity、参数引用及源码来源不因应用改变。

- 字段：`declaration`（FieldId）、`name`、`valueType`（TypeUse）、`storage`、`annotations`、`origin`。storage 区分 declared、enclosingInstance、capturedEnvironment、propertyBacking、propertyDelegate、generated；后两种 property storage 含实际 property 引用。存储描述不推导源码读写权限。
- variant：`declaration`、`name`、`style`（unit／positional／named／constructor）、`fields`、`annotations`、`origin`。
- property：`declaration`、`name`、`valueType`、`visibility`、`lookupDomain`、`getter`、`setter`（可为 null）、`overrides`、`annotations`、`origin`。是否可变由真实 setter 存在性表达。
- accessor：`declaration`、`access`、`implementation`、`body`、`annotations`、`origin`。implementation 为 storage／storageBody／constant／body／abstractSlot。
- constructor：`declaration`（CallableDeclaration）、`constructorKind`（primary／secondary／variant，variant 含真实引用）、`parameters`、`result`、`access`、`annotations`、`origin`。
- method：`declaration`（CallableDeclaration）、`name`、`receiver`、`typeParameters`、`contextParameters`、`parameters`、`result`、`effects`、`modality`、`access`、`body`、`annotations`、`origin`。typeParameters 每项为 `{declaration, applied}`，两侧都保留完整约束；宿主 binder 在外层 MemberDescription.parameters 中。modality 为 final／open／abstract／interfaceDefault。effect 字段沿待填方法签名，release 条件保留声明 binder。

值参数包含 `declaration: {owner, index}`、`name`、`valueType`、`calling`、`target`、`annotations`、`origin`。calling 与待填签名规则相同，vararg elementType 为 TypeUse，default 除应用后 arguments 还保留 declarationArguments；target 区分 plain／field／property 并携带实际关联。context 参数有独立的声明位置、label 和 TypeUse。

access 为 `{visibility, domain, accessible}`。visibility 为 public／internal／private／protected；domain 为 `{inhabited, constraints}`，constraint 区分 cone／file／lexicalOwner／subclassesOf，携带实际 cone／source／owner。空访问域与无约束的公共访问域不同；accessible 已含普通 protected receiver 限制，不能仅从 visibility 推算。

body 为 `{declaration, availability}`，availability 为 available／pendingGeneration／notPublished／noBody。读取描述不会读取或生成完整正文；完整树通过 describeBodies 查询。未知声明是查询错误，不返回虚假的 noBody。

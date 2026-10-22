# M47 查询请求与结果

本文件定义 [M47 协议](PROTOCOL.md)第 4 节的 `compiler/query` 参数和成功结果。公共值沿 [JSON 编码](JSON.md)、[成员编码](MEMBERS-JSON.md)与[正文编码](BODY-JSON.md)，不另建实体身份或类型规则。

## 请求

参数为 `{context: MethodId, query: Query}`。context 必须属于本进程本批生成任务；driver 将它映射到前端已经建立的方法上下文。共享查询和 JSON 分发只接收已解析的普通语义上下文，不负责进程任务选择。

Query 使用封闭的 `kind` 分支：

| kind | 其他字段 |
| --- | --- |
| `describeTypes` | `types: [Type]` |
| `describeMembers` | `types: [Type]` |
| `describeAnnotations` | `targets: [AnnotationTarget]`、`declarations: [AnnotationId]` |
| `describeDefaults` | `defaults: [DefaultReference]` |
| `describeBodies` | `callables: [CallableDeclaration]` |
| `typeRelation` | `relation: "equal" | "subtype"`、`left: Type`、`right: Type` |
| `lookupExport` | `provider: ConeIdentity`、`path: [string]`、`namespace: "type" | "value"` |
| `sourceLocation` | `targets: [AnnotationTarget]` |

数组可以为空；未查询的注解部分显式传空数组。所有类型输入使用完整 Type，不接收名称代替类型。path 是正常名称段，至少一项；每项使用不含反引号的规范标识符文本。provider 仅可为当前 Cone 或当前依赖视图中的直接 provider；不得以同名库替换未知 provider。

DefaultReference 为 `{owner: CallableDeclaration, parameter: u32, arguments?: [Type] | null}`。owner 与 parameter 是已有参数默认模板键；必需参数、vararg 的空数组省略以及没有该模板的引用返回未找到。合法参数本身不隐含一个可移植默认模板；查询只读取现有模板集合，不将捕获局部作用域的表达式提升为跨作用域模板。

arguments 缺省或为 null 时只读原定义。显式数组按模板 **definition root** 的宿主参数、callable 自身参数顺序给出完整实参；这正是参数描述中 default.arguments 的映射，不能改用继承方的参数数量。每个输入类型的 binder 在本次 context 中解释；定义中的 binder 仍属于原提供方。显式实参沿前端已有泛型元数、种类与 bound 规则检查，再通过共享默认值应用视图投影。查询不重新选择默认值提供方。

格式不合法、类型无法解析、应用不满足约束或导出查找歧义时，本次查询返回协议错误及前端诊断。未知声明、未知模板与正常未找到结果按下表表达。失败不会改变普通编译的诊断和生成结果；下一条独立查询可以继续。

## 成功结果

成组结果与请求数组逐项对应，保留顺序和重复项。描述中的成员仍按其原声明顺序排列。合法的空列表与未找到的 null 分别表示。

| 请求 | result |
| --- | --- |
| `describeTypes` | `{types: [TypeDescription]}` |
| `describeMembers` | `{members: [MemberDescription | null]}`；结构类型没有名义成员描述时为 null |
| `describeAnnotations` | `{targets: [[AnnotationApplication] | null], declarations: [AnnotationDeclaration | null]}` |
| `describeDefaults` | `{defaults: [{definition: DefaultBody, applied: DefaultBody | null} | null]}` |
| `describeBodies` | `{bodies: [BodyResult | null]}` |
| `typeRelation` | `{holds: boolean}` |
| `lookupExport` | `{exports: [ExportReference]}`；正常未找到时为空数组 |
| `sourceLocation` | `{locations: [SourceLocation | null]}` |

DefaultBody 沿正文编码保留 key、definitionRoot、前参、receiver、定义位置及完整表达式。应用结果与定义结果共用同一逻辑树，原声明引用、前参选择和定义位置保持；只有需替换的类型及外部引用实参使用给定映射。

BodyResult 为以下封闭分支：

- `{availability: "available", kind: "callable", body: CallableBody}`：完整普通 typed body。
- `{availability: "available", kind: "constructor", body: ConstructorBody}`：完整构造初始化计划。
- `{availability: "pendingGeneration"}`：本批待生成实现。
- `{availability: "notPublished"}`：依赖未发布正文。
- `{availability: "noBody"}`：声明本来没有普通正文，包括 variant 构造。

未知 callable 返回 null；空普通正文仍为 available。查询不会用空正文掩盖缺失实现，也不会查找依赖源码。

ExportReference 为 `{kind, id}`，kind 保持现有 BindableEntity 的具体类别：`type`、`genericType`、`objectValue`、`function`、`genericFunction`、`property`、`extensionProperty`、`typeAlias`、`enumVariant`、`annotation`。id 使用对应 typed identity；重载保留各实际声明，后续读取使用这些引用。

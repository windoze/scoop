# M23-11 可见性 fixture 的 CLI 迁移

`m23-cli-visibility` 保留 `m21-visibility` 的十三份原源码、`test:scoop-hir-lower:0.0.0` 身份和 `src/user.scoop` 路径。五项成功用例保存四阶段输出，移走源码后在无配套 compiler／LLVM 的独立 CLI 位置重新链接，然后以普通和 moving-GC 配置执行原 main。八项负例核对完整 JSON 诊断并要求不产生 binary。

只读验证全部通过：13 项、41 次进程执行、25 份阶段／link plan golden。18 条负例诊断的 message 和 byte span 与原内部前端结果全部相同；canonical Cone、source path、severity、code 和 notes 也逐字段固定。旧测试仅查找部分诊断，新声明完整包含原结果中同时出现的私有 getter 缺正文和 `ProtectedBase.expanded` 祖先槽覆盖错误，未改变成功／失败分类。

| 功能 | 正式 CLI 用例 | 进程／golden |
| --- | --- | --- |
| 独立 setter 访问域与继承槽 | `property-setter-slots`、`error-property-setter-slot-narrowing` | 7／5 |
| 方法／属性 override 不得收窄已扩大的公开槽 | `error-protected-override-public` | 2／0 |
| object／companion 的实际基类及 protected receiver | `object-protected`、`error-object-protected-receiver` | 7／5 |
| 私有 accessor、接口正文和 setter 参数类型 | `private-property-accessors`、`error-private-abstract-property`、`error-interface-property-write-type` | 9／5 |
| 嵌套词法域和私有 setter | `protected-lexical-scopes`、`error-protected-lexical-scopes` | 7／5 |
| protected 嵌套类型、成员接收者和外部类型可见性 | `protected-nested-receivers`、`error-protected-nested-receivers`、`error-protected-nested-type` | 9／5 |

三份旧摘要与仅用于文件诊断／成功编排的九个 Rust 测试已退役。六个 setter 的声明访问级别、完整 Cone／SubclassesOf 约束、继承槽和 FinalOverride 关系，五个私有 accessor 的 Final／Direct 状态，以及两个 singleton／companion 的真实基类和 owner 集合均保留为直接 typed 断言，不再格式化为文本快照。相关 Rust 代码净减少 243 行，没有内联搬运原 Scoop 文件。

清理前 22 个原可见性测试通过，并临时采集了上述 18 条诊断用于对照；采集代码已移除。清理后剩余 13 个内部测试、格式化、完整 lint 及全部 13 项 CLI 用例再次通过。此记录属于迁移分项，M23-11 的完整验收仍须运行全仓集合。

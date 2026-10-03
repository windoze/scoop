# M23-11 可见性 fixture 的 CLI 迁移

`m23-cli-visibility` 保留 `m21-visibility` 的十三份原源码、`test:scoop-hir-lower:0.0.0` 身份和 `src/user.scoop` 路径。五项成功用例保存四阶段输出，移走源码后在无配套 compiler／LLVM 的独立 CLI 位置重新链接，然后以普通和 moving-GC 配置执行原 main。八项负例核对完整 JSON 诊断并要求不产生 binary。

只读验证全部通过：13 项、41 次进程执行、25 份阶段／link plan golden。18 条负例诊断的 message 和 byte span 与原内部前端结果全部相同；canonical Cone、source path、severity、code 和 notes 也逐字段固定。旧测试仅查找部分诊断，新声明完整包含原结果中同时出现的私有 getter 缺正文和 `ProtectedBase.expanded` 祖先槽覆盖错误，未改变成功／失败分类。

| 功能 | 正式 CLI 用例 | 进程／golden |
| --- | --- | --- |
| 独立 setter 访问域与继承槽 | `property-setter-slots`、`error-property-setter-slot-narrowing` | 7／5 |

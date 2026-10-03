# M23-11 限定类型与直接依赖可见性

`qualified_dependency_types_respect_direct_package_visibility` 迁为 `m23-cli-qualified-types` 的两个正例和三个负例。保留 `m23-qualified-types` 的原源码与 Cone coordinate：origin 经两个 facade 形成 diamond，independent 另有直接依赖的同名前缀 package；源码移走后仅通过正式产物消费。

diamond、independent 的 6 份原 HIR Export／MIR／LIR 快照逐字相同。consumer 的 package 保持 `consumer.paths`，运行入口以普通 import 调用公开 `check()`，要求返回 42；独立链接与构建的 fingerprint 相同，普通与 moving GC 运行均通过。完整四阶段 golden 另保留每个 provider、facade 和 consumer 的实际输出。

三个负例继续分别检查 support package 不可见、直接声明冲突、最长 package 匹配后没有可访问类型。原全部消息、表达式和 byte span 逐项相同，完整 JSON 锁定 canonical 来源及全部诊断，失败不发布产物。

只读验证通过 **5 个用例、44 次进程、38 次阶段／计划 golden 比较**。删除旧 driver 文件测试、注册及 9 份被替代的阶段／诊断快照；原 `.scoop` 和其他 typed 单元测试保留。

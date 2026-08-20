# 源码默认值、引用与相等迁移

`tests/fixtures/m23-cli-source-defaults` 以唯一 Python schema 1 覆盖原
`m23-type-source-defaults` 的 19 份源码；20 个用例包含两种 warmup 文件顺序。
原源码保留，基线库继续使用 `test:scoop-hir-lower:0.0.0` 和原逻辑路径。
本记录仅对应这一批，不表示 M23-11 全仓验收完成。

| 功能 | CLI 用例 |
| --- | --- |
| 字面量与派生相等准备顺序 | `literal`、`derived-default-order`、`derived-default-combinations` |
| 泛型、Unit 与 tuple 默认值 | `derived-generic-defaults`、`structural-defaults`、`structural-default-combinations` |
| 结构默认值的文件顺序与访问域 | `warmup-first`、`warmup-second`、`structural-hidden` |
| 继承和定义方接收者 | `inherited-generics`、`provider-receivers`、`public-provider-receivers` |
| 数据流与原声明绑定 | `data-flow`、`declaration-binding`、`origin-binding` |
| 嵌套声明身份与类型实参 | `nested-identities`、`nested-index`、`nested-parents`、`type-envelope` |
| 函数引用、闭包、型变与 suspend | `reference-closure-combinations` |

每个成功用例先单独构建原声明并保存四阶段输出，再实际调用默认值。
可公开调用的能力由独立下游只凭 `.slib` 消费；private/protected 访问在工作副本中
加入普通检查方法，保留原访问规则。四个需要实例的跨 Cone 用例添加普通公开工厂，
没有改变原 class 构造器的可见性；内部属性通过同 Cone 的普通方法读取。
私有顶层声明与调用入口按文件可见性放在同一工作文件，原源码未移入 Rust 字符串。
全部 19 个程序移走源码后独立链接，以普通和移动 GC 执行。

`structural-hidden` 保留原主错误位置与消息，并检查实际公开 CLI 返回的完整
16 项诊断；重复类型和构造访问错误没有被过滤，不能用任意非零退出替代。
四份原摘要快照的引用数量、Universal 域、open/exact application 数量与缺失／错误
身份拒绝检查保留为 Rust 类型化断言。两种 warmup 顺序直接检查同一语义关系。
文件摘要比较和旧负例编排删除，继承接收者、binder 替换及 roundtrip 检查保留。

本批促成两项独立修复：局部泛型函数捕获非泛型父默认值，以及多 Cone 请求 Unit
派生相等时的 ODR 归属，分别见 [捕获回归](CLI-DEFAULT-PREPARATION.md) 和
[Unit 相等回归](CLI-UNIT-EQUALITY.md)。最后一次审计的 182 份既有 golden 中，
差异限于已说明的 Unit helper 身份、对应计划，以及结构组合新增的合法属性读取
方法；其余保持一致。新增五份输出覆盖该组合的实际程序与独立链接计划。

整批只读验证通过：**20 用例／20 变体／120 进程／187 份阶段与计划 golden**，
62 个产物指纹匹配。保留及新增的 7 项 Rust 类型化测试、workspace 格式化和完整
clippy、Python 格式与 lint 全部通过。原四份摘要快照和一项纯诊断 harness 已退役。

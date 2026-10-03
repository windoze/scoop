# 默认操作类型与调用签名迁移

`tests/fixtures/m23-cli-default-operations` 通过统一 schema 1 执行原
`m23-default-nominal-operations`、`m23-default-operation-signatures` 和
`m23-default-operation-types` 的六份源码。原声明先以
`test:scoop-hir-lower:0.0.0`、`src/main.scoop` 单独编译，再添加普通实例工厂；
下游从产物实际展开默认值，原文件与可见性均保留。

| 功能 | 实际组合 |
| --- | --- |
| 名义类型 | class identity、struct/enum 值相等、ZST、interface、object、泛型实参换序与静态嵌套类型 |
| 调用签名 | 普通、继承和泛型成员，getter/setter、vararg、挂起调用，class/struct/enum 构造与泛型默认构造 |
| 操作类型 | Unit、Boolean、八种整数、String、Throwable、ForeignCallbackState；Array/MutableArray/Option、同名普通类型、tuple、Ptr 和 ForeignCallback |

指针组合读取真实局部地址，callback 组合注册并释放真实 reusable callback，
协程组合通过 startCoroutine 取得实际结果。所有程序移走源码后独立链接，
普通与移动 GC 的检查结果一致。继承调用暴露的问题已沿共有成员路径修复，
独立回归见 [接收者修复记录](CLI-DEFAULT-RECEIVERS.md)。

退役四份没有剩余 Rust 注册的早期分类／签名摘要：名义类型一份、调用签名
一份、操作类型两份。原类型种类、字段位置、参数／结果、挂起性以及 intrinsic
与普通同名类型的区别，由完整阶段输出和实际下游调用继续覆盖。

只读验证通过 **6 用例／6 变体／42 进程／78 份阶段与计划 golden**，24 个
产物指纹固定。新增程序阶段补齐此前未执行的默认值路径；修复前已有输出中，
只有静态嵌套成员的 Export HIR Call 改成 MethodCall，MIR/LIR 保持不变。
格式化、完整 workspace clippy 和 Python lint 均通过。本批完成不表示
M23-11 的全仓验收已完成。

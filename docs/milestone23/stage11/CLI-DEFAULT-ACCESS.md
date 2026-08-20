# 默认参数调用域与访问规则迁移

`tests/fixtures/m23-cli-default-access` 通过唯一 Python schema 1 覆盖原
`m23-default-call-domains`、`m23-default-callable-access`、`m23-default-type-access`、
`m23-default-value-access` 和 `m23-default-source-profiles` 的 26 份源码。
这一批迁移不表示 M23-11 全仓验收完成。

| 功能 | 正例／负例 | 实际运行与保留的关系 |
| --- | --- | --- |
| 调用域 | 2／4 | diamond、值实现、继承默认值、父 binder 替换、未使用的泛型 owner 和嵌套类 |
| callable 访问 | 2／3 | 普通／泛型调用、getter/setter、lambda、匿名函数、本地函数和引用、class/interface bound、重复模板参数替换 |
| 类型访问 | 3／2 | nominal、application、tuple、函数、Ptr/FunPtr 默认值；实际局部地址读写和 NoGC callback 地址 |
| 值访问 | 2／4 | class/struct/enum 构造、字段、tuple、object、global 和访问器；修改全局后再次读取默认值 |
| 默认值发布 | 4／0 | 普通／泛型 owner、静态嵌套成员、protected/private 默认值、override 继承和 enum payload 默认值 |

八份原 reader 源码继续使用 `dev.example:standalone/combined:0.1.0` 与
`src/main.scoop`。core 保留原 stage3 输入，类型访问三个正例还保留同一
`src/default-pointers.scoop`。每份原声明先单独保存 AST/HIR/MIR/LIR；需要实例
或内部访问时，再在工作副本添加普通工厂和检查方法，保留原可见性。
下游只使用产物，13 个成功程序移走源码后独立链接，普通与移动 GC 均通过。

13 个负例检查完整的 21 项诊断，原消息和字节区间全部保留。额外诊断分别对应
两个祖先槽，以及局部变量、类型表达式和构造器的访问域；这些在原 Rust 中只做
主错误查找的内容现在按实际数组完整断言，没有过滤重复错误或改变成功／失败分类。

八份原 HIR 快照中，六份 Export 内容逐字一致。另两份只增加已在全局默认值
修复中实现的 getter/setter 正文（`44280919e`），原类型和函数行为不变。
新完整 golden 同时保留 LocalConcrete。四份早期默认值 profile 标签摘要退役，
现有共同默认值元数据、继承关系和真实下游调用承担覆盖，不恢复旧能力分档。

四项产物 producer／reader 单元测试保留：默认模板数量、继承来源、PublicSlot／
DirectOnly、类型参数映射、七类 callable 引用、六类 type 引用，以及构造、字段、
全局和 object 引用均继续直接断言。删除其中的 dump／快照编排、四个只做负例
文件循环的 HIR 模块、八份 HIR 快照及四份旧 profile 摘要。

整批只读验证：**26 用例／26 变体／109 进程／141 份阶段与计划 golden**，
57 个产物指纹匹配，4 项保留的 Rust 测试通过；格式化、完整 workspace clippy、
Python 格式与 lint 全部通过。原 fixture 源码全部保留。

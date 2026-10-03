# 源码名义声明的正式产物覆盖

`m23-cli-source-nominals` 保留原 `m23-type-source-nominals` 源码，以原
`test:scoop-hir-lower:0.0.0/src/main.scoop` 身份生产完整 provider。需要观察
内部成员时，只在明确的工作副本中加入合法宿主方法和普通工厂，并独立保存
这次物化的四阶段输出和产物指纹。下游程序移走源码后独立链接，再运行普通
和移动 GC。

| 原源码／新用例 | 验证内容 | 进程／golden／指纹 |
| --- | --- | --- |
| `binding` | 泛型方法、静态嵌套类型、成员 getter、struct 与三种 enum 变体 | 7／13／4 |
| `inheritance` | 基类与接口派发、object 覆盖、普通与泛型 enum、值类型 | 7／13／4 |
| `declarations` | 抽象泛型类、普通泛型类、接口默认方法、object、默认与次构造器 | 7／13／4 |
| `callables` | 泛型方法与嵌套类型、默认接口方法／属性、私有与 unsafe helper、object 和 enum 默认参数 | 7／13／4 |
| `properties` | 泛型 getter/setter、接口默认属性、私有常量、静态嵌套类型与 object 状态 | 7／13／4 |
| `c-layout-single` | 独立 CLayout 属性、字段读取与结构相等 | 7／13／4 |
| `c-layout` | packed/aligned 嵌套布局、泛型 CLayout、保护域中的布局与结构相等 | 7／13／4 |
| `roots` | 嵌套支持声明闭包、父接口视图、值类型与不同 object identity | 7／13／4 |
| `constructors` | 主／次构造器、各可见域、默认参数、私有委托、泛型 owner 与 singleton | 7／13／4 |
| `parameters` | required/default 参数、泛型约束、私有／protected helper、enum 与次构造器默认值 | 7／13／4 |
| `nested` | 多层保护域、泛型嵌套 owner、私有构造、接口、值类型与静态 singleton | 7／13／4 |

表中各项只读通过。声明组合发现的实际覆盖检查顺序问题由
[多文件继承修复](CLI-INHERITANCE-ORDER.md) 单独处理。
11 个普通源码用例合计 77 次进程、143 份阶段／链接 golden、44 项产物指纹，全部只读通过。原批次已有的 108 份阶段输出逐字相同。

`declarations.snap`、`nested.snap` 及其专用 Rust 摘要 formatter／文件比较已退役；
相应 HIR 单元测试保留共有声明和类型 section 的 wire roundtrip、名义声明非空、
materialized type use 验证。四项相关内部测试通过。原源码全部保留。

接口默认属性缺口由 [默认属性修复](CLI-DEFAULT-PROPERTIES.md) 处理。
另两个修改 core 的名义声明用例仍在迁移；本记录不代表 M23-11 全仓验收完成。

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

表中各项只读通过。声明组合发现的实际覆盖检查顺序问题由
[多文件继承修复](CLI-INHERITANCE-ORDER.md) 单独处理。
未列出的名义声明用例仍在迁移，本记录不代表 M23-11 全仓验收完成。

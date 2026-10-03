# M23-11 源码派发的正式 CLI 覆盖

每例先以原 coordinate 和 logical source path 独立发布原始源码，再在临时工作副本内加入合法 owner 观察入口，发布产物供下游使用；原始 fixture 不变。每例保留原始 provider、补充入口后的 provider 和下游程序的完整 AST／HIR／MIR／LIR，以及正式链接计划与四份产物指纹。移走源码后独立链接，普通与 moving GC 均运行。

| 功能 | 用例 | 只读进程／golden／指纹 | 保留的语义验证 |
| --- | --- | --- | --- |
| Suspend 与普通 callable 组合 | `callables`、`protected-callables` | 14／26／8 | unsafe、infix、operator、protected 与泛型调用；实际跨 Cone suspend 运行，见 [修复记录](CLI-SUSPEND-MEMBERS.md) |
| 虚表与接口 | `virtual`、`interfaces` | 14／26／8 | 父虚表前缀、final override、protected accessor、菱形接口顺序、默认方法及 class／object／struct／enum 表；保留 typed slot 与 wire roundtrip，删除两份专用摘要快照 |
| 构造器 | `constructors`、`protected-construction` | 14／26／8 | public／protected／internal／private、主次构造器、默认值、值类型与 object；保留九份 typed 合同的 owner／visibility／modality／参数身份断言，删除旧字符串拼装快照 |
| 参数协议 | `parameter-protocols` | 7／13／4 | 显式／省略默认实参、泛型 identity、零参数和默认字段构造；普通值 4 与显式值 9 |
| 属性派发 | `properties`、`property-direct` | 14／26／8 | 实际读写、private／protected／internal setter、接口默认属性、派生覆盖、值类型及 object 属性 |

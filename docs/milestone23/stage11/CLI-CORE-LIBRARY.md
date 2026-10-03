# Core library 用例迁移记录

`tests/fixtures/m23-cli-core-library` 复用唯一 Python schema 1。原源码保持在
`tests/fixtures/core-library`，新增文件仅提供 manifest、运行检查与实际名称绑定组合。
本记录不代表 M23-11 的最终全仓验收。

## 名称、调用与 ABI

单文件消费者保持原 `scoop:single-file` 身份。alias 的普通 library 保持原
`test:scoop-hir-lower:0.0.0` 与 `src/main.scoop`，公共接口由独立下游消费，
内部 alias 则在同 Cone 调用。core 源码先编译成产物后删除，最终 executable
从 `.slib` 独立链接，普通与移动 GC 均运行。17 个实际 artifact 指纹固定在 fixture 中。

| 功能 | case | 用例／进程／golden | 保留的验证 |
| --- | --- | --- | --- |
| 普通 core 调用 | `consumer` | 1／5／5 | 单文件编译、产物独立链接与运行 |
| 当前声明优先级 | `prelude-priority` | 1／5／5 | 当前同名函数优先于导入的 core prelude |
| core 调用组合 | `calls` | 1／5／5 | 具名、默认、显式参数、const 与默认链 |
| Unit 与混合 ABI | `abi` | 1／5／5 | Unit elision、Int32、managed String、返回类型与调用顺序 |
| core typealias | `alias-consumer`、`alias-shadow`、`alias-generic-error` | 3／14／18 | 原完整 HIR export 逐字相同；类型／值命名空间、shadow、非泛型 alias 错误 |
| 非泛型 builtin | `builtin-generic-error` | 1／2／0 | 原 Int 名称位置与完整错误逐字相同 |
| 协议与公开名称 | `builtin-binding-error` | 1／2／0 | 真实 core 保留整数协议；缺少根 Int binding 时报告原 unknown-type 错误 |

名称缺失用例将原 core 的 Int 声明原样放入 `hidden` package；
根名字由内部 typealias 保供 core 自身使用，公共 Int32/UserCoreNumber 直接指向
公开的 `hidden.Int`。消费者仍输入原 `builtin-binding-error.scoop`，完整错误为
`unknown type Int`（原消息中的代码反引号亦保留），source span 为 19–22。
该构建使用普通 package、alias、可见性规则，没有额外的 core 资格或来源机制。

本组只读验收为 9 用例／38 进程／38 份 golden，3 个错误保留原完整消息与位置。
删除原 alias HIR 文件快照和 ABI 文本快照；ABI 改为直接检查 typed 参数种类、
0/4/8 字节与 Aggregate/Scalar、managed effect、返回存储及 MIR signature。
别名选择数量、binding witness 与本地 shadow 的 Boolean 类型检查继续保留。
删除 driver 中已由 CLI 覆盖的三段纯消费者构建，保留核心 producer／reader 检查。
清理后两项 alias HIR 测试和 core 综合 producer 测试通过，格式化及 workspace lint 通过。

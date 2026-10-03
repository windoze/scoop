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

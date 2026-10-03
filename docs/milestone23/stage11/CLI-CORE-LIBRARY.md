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

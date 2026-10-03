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

## 直接输入与发布失败

| 功能 | case | 用例／进程／golden | 保留的验证 |
| --- | --- | --- | --- |
| 直接 library 输入 | `direct-standalone` | 1／8／9 | 原 manifest；显式与隐式 core 的 scoopc 产物和公开 build 逐字节一致 |
| 直接依赖组合 | `direct-combined` | 1／10／13 | 原 helper 与 combined 源码；显式依赖重排、隐式 core、实际下游消费 |
| core 发布失败 | `publication-core` | 1／6／9 | 当前 warning 与最终发布错误按原顺序保留；失败输出仍为空目录 |
| 普通 library 发布失败 | `publication-ordinary` | 1／8／13 | warm 构建仍有当前 warning；不重播预构建 core 的 warning；重建后消费运行 |

直接输入用例把实际 `scoopc build` 产生的字节交给下游，删除所有 provider 和
consumer 源码后独立链接，分别以普通与移动 GC 运行。原独立 library、helper 和
组合 library 的 manifest 与源码保持不变；隐式 core 来自已构建的 sysroot 产物。
删除原 Rust 外部进程编排模块，直接输入分类与拒绝规则继续由 typed 单元测试检查。

发布用例保留 `publication-warning.scoop` 的完整 warning、source span 108–113
和逻辑文件路径，按完整 JSON 集合检查 `SCOOP_OUTPUT_WRITE` 发布错误。
公开 `scoop build` 验证最终发布失败；原 producer 单元测试在解析成功之后
故意改变输出路径，继续检查具体 `CurrentConeProductionFailure::Publication`
及其 warning 对象。这两个实际边界不同，保留后者的内部故障检查，不再因目标缺失
提前返回。普通 library 的额外运行检查文件调用原 `describe`，验证两个 enum 分支。

本批增加 13 个 artifact 指纹；最终只读运行共 **13 用例／70 进程／82 份 golden**，
30 个产物指纹全部匹配。core 综合 producer 与发布失败单元测试通过，格式化、
Python lint 和完整 workspace clippy 通过。本批没有启用快照更新。

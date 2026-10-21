# 测试运行模式

普通功能 fixture 与内置 Cone 测试显式使用 release。Scoop 产品的默认 debug 行为保持；只在实际测试该默认值或编译器模式差异的专测中省略 profile/选择 debug。工作目录的 normal 名称仅表示一次默认变体，不表示编译 profile。

P7 移除了 1,572 个只重复普通输出的 stress 步骤及 631 个重复变体；当时的 4,435 个 fixture 均保留来源与输入归属；后续合并/迁出普通库测试后，当前集合为 4,200 项。其他环境仍继承，runner 在应用变体/步骤环境之前去掉两个 GC stress 变量。Python runner 的 55 个单测已验证环境继承、显式开启和步骤覆盖。

| 场景 | 模式 | 理由 |
| --- | --- | --- |
| 普通语言、库、CLI | release 一次 | 错误位置、运行结果和普通缓存行为不重复 debug/stress |
| IR、优化、native ABI、ODR、产物结构 | 原有必要 debug/release 或跨产物组合 | 原转换及机器结构断言依赖实际优化模式；额外 stress 已去掉 |
| 默认 profile/输出位置 | 原始省略 profile 的调用 | 验证产品默认值；不改变用户 CLI 合同 |
| `exceptions-return-paths` | debug | 断言返回值临时变量与 finally 清理的未优化 MIR；release 会合法折叠这些临时变量 |
| `executable-record` | debug → release → debug | 验证两种 profile 的缓存记录相互区分，并复用之前的 debug 结果；始终保留已有缓存产物 |
| 相反 provider/consumer profile | 保留双向组合 | 验证跨优化产物消费；不将两端同时机械改成 release |
| 57 个既有 GC 场景 | 原有 GC 矩阵，显式编入 stress 支持 | 保留根、屏障、搬迁、回收、native 生命周期和 safepoint 断言 |
| gc-stress-opt-in | 逐步骤选择普通/stress runtime 和环境 | 同时验证未编入能力时环境无效、编入能力后的运行开关及缓存差异 |

P7 检查具体变量后补入编译器模式组的用例：

- `cli-build-warm-observation-link`
- `cli-library-defaults-and-publication`
- `containers-artifacts`
- `containers-updates`
- `direct-c-artifacts`
- `interfaces-artifacts`
- `maybe-uninit-artifacts`
- `annotations-array-artifacts`
- `combinations-artifacts`
- `jumps-artifacts`
- `trailing-artifacts`
- `when-artifacts`
- `application`

其他编译器模式组与 GC 职责见 [分类记录](FIXTURES.md)。普通模式不设置 stress=0 来冒充未设置；这也使仅检查环境变量存在性的历史 native 探针不被意外触发。

逐项检查剩余步骤后，深栈/异常展开的两个 Linux fixture 因实际验证 live roots 归入 GC 专测，保留原 profile/stress 组合。测试报告的重复断言并入原步骤；JSON 协议的额外数据仍正常运行，独立链接产物仍单独执行。

`scoop doc` 不公开 profile 参数，其文档/HIR 合同按原命令覆盖；文档输入中的源码依赖准备沿用该命令现有策略，不将文档生成重复运行于 debug/release。

后续归入 GC 专测的 `lists-values-gc` 只验证普通 release 下的实际回收，保持原单一模式；归入 GC 目录不表示必须增加 stress 变体。当前 GC 组共 59 项，模式仍按具体断言选择。

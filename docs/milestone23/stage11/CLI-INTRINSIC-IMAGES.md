# M23-11 Intrinsic 源码、image 依赖与共有 LIR

这些用例保持原始源码，通过标准 schema 的文件步骤准备真实 Cone，由正式 CLI 发布、消费并独立链接。每个执行入口同时运行普通 GC 与 moving GC，固定完整四阶段 golden、链接计划和产物指纹。只读验收记录位于 `/tmp/scoop-m23-11-final-graphs/verified-*/report.json`。

| 功能 | 用例／进程／golden／指纹 | 验收与保留的内部检查 |
| --- | --- | --- |
| intrinsic-source-shapes | 2／11／22／5 | 真实 core 增加原 declarations 与 consumers；普通同名 struct/class、CLayout、泛型数组、常量、Bool vararg/spread、装箱与 gcCollect。保留 14 个 intrinsic family、nominal kind、binder bounds、实际字段、常量类型与 vararg Array application 的 typed 断言，退役 declarations.snap。 |
| image-dependencies | 1／8／17／5 | core、已选择 provider、unused provider、consumer 与 program 形成真实闭包，结果 42；未使用的直接依赖 image 仍保留。保留 Compile/Link image 相等、直接依赖集合和唯一已选择 callable 的 typed 检查，退役 image.snap。 |

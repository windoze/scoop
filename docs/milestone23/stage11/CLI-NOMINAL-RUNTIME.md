# M23-11 声明、物化和数组的正式 CLI 验收

本组 30 个用例分别发布原始源码，再通过完整产物构建独立下游。需要访问 restricted 成员时，只向工作副本加入语言允许的同 Cone 工厂或 owner 方法，并分别保留补充前后的四阶段输出与产物指纹。独立链接使用保存的产物和 runtime index，运行普通 GC 与 moving GC 两种模式。

原有短摘要和重复快照编排已退役，普通 typed 单元测试继续检查实体身份、声明归属、访问范围、字段与继承关系、ABI、依赖投影及 wire roundtrip。堆上 ZST 两例的 Export HIR 与原快照逐字相同；77 个原有 MIR 和 LIR 函数在将本地函数编号对应到实际函数后全部保持一致，仅增加已修复的派生 equals。所有新增期望均经只读复验。

| 功能 | 用例／进程／golden／指纹 | 实际覆盖与保留断言 |
| --- | --- | --- |
| `fact-providers` | 2／13／22／7 | 本 Cone 值类型、结构型支持、GC/ZST facts 与 core 基元 provider 分离；独立及组合 payload、宽度整数和引用字段。 |
| `heap-zst` | 2／12／18／6 | 零大小 heap 字段、getter/setter、closure capture、派生值与单例初始化；原 3 次阶段编译改为一次正式生产。 |
| `hir-materialized-selections` | 2／13／22／7 | signature、representation、type test、shape support、成员调用及未求值泛型默认值；保留缺失、额外与错误 provider 的拒绝测试。 |
| `nominal-requirements` | 2／14／26／8 | 主次构造、字段、enum payload、继承、槽、protected/default 成员与延迟泛型；typed visitor 核对真实 owner、字段 ID、callable 指针与完整闭包。 |
| `nominal-signature-scope` | 2／13／22／7 | 同一签名中的依赖引用、本地 aggregate 与 ZST；保留 MIR/LIR 语义签名相等、direct/indirect/elided 参数和结果布局。 |
| `materialized-type-uses` | 4／28／52／16 | 父类、签名、capture、type operand、初始化隐含 String 与未调用默认值；保留精确类型集合和 HIR 到 MIR 的实际 provider 对应。 |
| `shared-callable-declarations` | 2／14／26／8 | restricted 构造、方法、接口、访问器及 object；保留普通 metadata、support 可查但不可公开 lookup、缺失和错误 owner 拒绝。 |

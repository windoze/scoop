# M23-11 MIR 产物的正式发布和运行

每例保留原源码的独立发布，再通过普通下游消费完整产物。仅在必要时向临时工作副本加入合法同 Cone 工厂或 owner 观察入口，原文件不变。原始 provider、补充入口后的 provider 和下游各自保存完整阶段输出，另有固定产物指纹、独立链接计划以及普通／moving GC 运行。Python runner 使用公共 schema，无目录分支或逐例执行脚本。

| 功能 | 只读用例／进程／golden／指纹 | 实际组合与保留的 typed 断言 |
| --- | --- | --- |
| `mir-callable-production` | 3／19／31／10 | 普通／成员／访问器、NoGC 泛型存储、pure virtual trap、private 函数实际物化但不进入公开 binding；保留 receiver、参数、返回值、GC effect、角色与 source ID 对应。 |
| `mir-boxing-production` | 2／14／26／8 | struct／enum、菱形接口、默认方法与属性；保留实际 adjust 的唯一直接调用、目标绑定、payload 集合、itable 槽以及两例 2／24 个调整函数。 |
| `mir-constructor-production` | 2／13／22／7 | ZST、主次构造、protected／private、默认参数、managed 字段；保留语义构造签名、实际 initializer receiver、结果和字段 assembly／GC 差异检查。 |
| `mir-dispatch-production` | 2／14／26／8 | 父类前缀、final override、重新抽象、class／object／值类型派发；保留实际 slot、实现角色、object override 的独立 receiver 及完整依赖检查。 |

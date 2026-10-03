# M23-11 MIR 产物的正式发布和运行

每例保留原源码的独立发布，再通过普通下游消费完整产物。仅在必要时向临时工作副本加入合法同 Cone 工厂或 owner 观察入口，原文件不变。原始 provider、补充入口后的 provider 和下游各自保存完整阶段输出，另有固定产物指纹、独立链接计划以及普通／moving GC 运行。Python runner 使用公共 schema，无目录分支或逐例执行脚本。

| 功能 | 只读用例／进程／golden／指纹 | 实际组合与保留的 typed 断言 |
| --- | --- | --- |
| `mir-callable-production` | 3／19／31／10 | 普通／成员／访问器、NoGC 泛型存储、pure virtual trap、private 函数实际物化但不进入公开 binding；保留 receiver、参数、返回值、GC effect、角色与 source ID 对应。 |

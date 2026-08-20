# 常量分支与 GC 站点一致性

默认实参 `true`/`false` 在展开时先存入局部变量，再复制给默认正文中的条件。
LIR 原先只折叠直接的布尔字面量；LLVM 的 SROA/mem2reg 随后识别复制链，
删除死分支中的闭包分配，post-RS4GC 检查因此报告两个缺失站点。

LIR 现在在原有 CFG 清理、root plan 和 safepoint 身份分配之前，传播同块内
的布尔存储、复制、取反与比较。局部地址及其复制关联到实际存储，同类型
load/store 使用该关联；未知写入、调用和 GC 站点使可取地址存储的信息
失效。跨基本块不推测常量，普通代码求值顺序不变。死分支由已有可达性
处理删除，后端和对象检查继续精确核对全部实际站点。

| 用例 | 实际验证 |
| --- | --- |
| `m23-cli-default-captures/constant-branches` | 跨 Cone 默认闭包的 true/false 分支、显式闭包覆盖、lambda 与匿名函数 |
| `m23-cli-default-captures/constant-stores` | 局部复制、覆盖赋值、未知参数写入、取反、比较、直接地址写入、地址复制与读取、向普通函数传地址后写入 |

两个新用例通过 11 次进程执行、14 份阶段与计划 golden、5 个产物指纹，
均移走源码后独立链接，在普通与移动 GC 下运行。连同已有运行时分支用例，
整套默认捕获回归只读通过 3 项／17 进程／23 份 golden／8 个指纹。
143 项 LIR 单元测试全部通过；既有短路 OR 的 inline golden 只删除已知
不可达的右侧分支。实现单独放在 176 行的 `safepoints/constants.rs` 中。

对既有默认值、受保护域和默认操作的复验未发现本项改动引起的已有
MIR/LIR 差异；闭包引用及继承 receiver 修复涉及的 HIR 预期另行同步。
本记录不代表 M23-11 全仓验收完成。

全仓复验另覆盖已有 `core-operations-const-operators` 与 `shared-native-calls-context`。两项的 AST／HIR／MIR 不变，LIR 各只移除一处已知布尔条件的死分支：前者的 `Not true` 直接选择 else，后者的 `true` 直接选择 then。原函数身份、返回值和其余指令保留；仅同步两份 LIR golden 及 const-operators 的实际 program artifact 指纹，运行和诊断期望不变。

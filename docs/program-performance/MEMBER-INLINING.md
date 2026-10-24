# 跨 Cone 普通成员正文内联

本项相对 `15546650c` 开放普通方法与 getter 的优化正文；沿用现有通用 HIR 结构选择和 MIR 调用点成本模型，没有对 `ArrayList`、`Long` 或任何声明名称设置特殊规则。release 默认启用，debug 保留调用；所有 profile 使用相同的正文导出、实体身份和调用 ABI。

## 实现与边界

- 移除普通小正文导出／导入的顶层函数限定。相同的 96 节点结构选择同时适用于顶层函数、名义方法、属性与扩展属性访问器；当前选择仍排除 closure、字符串正文、Context、协程及局部异常协议等尚未进入这条路径的结构。
- 只有正文中 owner 与 callable 的类型参数均为空时才进入普通优化正文路径；泛型宿主与泛型成员沿用已有具体化。成员保留原 receiver 首参，访问器保留独立的 typed accessor identity。
- 同一声明的直接、虚调用和接口入口可能各有引用，按声明身份只准备一次正文。已取得正文并不授权去虚拟化未知调用；实际选定直接目标后，才由统一成本模型决定是否展开。
- 复用 shared-source 格式与独立 `ImportedInlineBody`，不复制 provider 的 Strong 定义，不修改 `.slib` wire 格式、对象／接口表布局或 runtime ABI。相容旧库没有新增优化正文时继续调用其原入口；已有提供方需要重新构建才能提供新增正文。新增正文按原契约改变产物语义指纹，更新 core 时也需重建预编译系统库的依赖闭包，不能混用新 core 与记录旧指纹的 std 等产物。
- 继续保持参数求值一次、值 receiver 快照、异常边及 GC roots。含普通 Managed helper 调用的小型成员同样可以入选；测试中 `afterCollection` 展开后，collection 调用及入口只保留返回对象参数作为 root，没有保留已不使用的 receiver。

本项没有提高执行成本阈值或调用者增长额度。`ArrayList.get` 的累计增长问题、未展开调用的 poll／TLS、native bridge 与 runtime 启动登记仍需分别处理。`Long.hash` 的成员包装被内联后仍调用原 `coreLongHash` native bridge；不将其写成所有调用已消失。

## 定向验证

- 本机格式化与两机 workspace Clippy 通过；89 项 HIR 跨 Cone 正文消费测试通过，覆盖既有泛型、继承、访问器、源码／wire 一致性及新的声明去重。既有大型 Rust 测试在默认测试线程栈上溢出，使用已有的 `RUST_MIN_STACK=33554432` 测试设置后完成，产品栈设置未变。
- Darwin、GNU、musl 均通过 `ordinary-member-inlining` 与 `inlining-artifacts`：共 12 个 variant、48 个进程。覆盖反向 provider／consumer profile、独立链接、struct／class 方法、普通／扩展 getter、显式 default、未知 virtual／interface、泛型、装箱、receiver 求值快照、除零 catch、引用返回和 moving／minor GC。
- 本机另跑 `inlining-decisions`、`devirtualization-locals`、`devirtualization-artifacts`、`local-values` 四项受影响的优化专测，共 8 个 variant、20 个进程。逐项审阅 release MIR 变化：普通成员与 `String.toString`／`Int.toString` 展开，以及后续已有的常量折叠、局部字段替换与纯整数循环 poll 分块；debug 期望未变。开发 sysroot 按真实依赖顺序重建受新 core 指纹影响的预编译库，三目标均完成。
- MIR 确认适用成员调用消失，动态调用继续分派，热点除法保留原异常出口。LIR 确认引用返回的 Managed 调用只使用 `param1` 一个 root。新编译器消费旧 core／provider 的独立构建保留外部成员调用，并匹配既有 checksum。
- [成员基准](../../tests/benchmarks/inlining/members/README.md)以用户定义的方法和 getter 为主体，保留 primitive、未知接口与标量对照；复用原有 phase 计时及 A/B 工具。三目标各验证两版、6 模式、5 种迭代次数，共 180 个预先计算的输出；18 组独立 GC 诊断的 16 项计数均一致。

## 运行结果与成本

本机 M3 Ultra／LLVM 22.1.8，Linux 为 `nuc12` i7-12700H／LLVM 22.1.2，Linux 计时绑定 CPU 4。每模式 2,000 万次迭代，一组预热、7 组 AB／BA；下表是内核墙钟中位数，单位 ms，不包含构建与进程启动。

| 负载 | Darwin 前 → 后 | GNU 前 → 后 | musl 前 → 后 |
| --- | --- | --- | --- |
| 用户 struct 方法 | 36.823 → 17.023（−53.77%） | 26.122 → 12.878（−50.70%） | 25.677 → 13.638（−46.89%） |
| 用户 class 方法 | 37.614 → 15.614（−58.49%） | 32.104 → 8.797（−72.60%） | 32.689 → 8.956（−72.60%） |
| 用户 getter | 37.233 → 15.263（−59.01%） | 27.802 → 8.877（−68.07%） | 27.859 → 8.805（−68.39%） |
| `Long.hash` | 64.170 → 61.422（−4.28%） | 51.281 → 51.258（−0.04%） | 51.303 → 51.279（−0.05%） |
| 未知接口方法 | 41.626 → 41.353（−0.66%） | 35.556 → 35.465（−0.26%） | 37.295 → 37.043（−0.67%） |
| 标量对照 | 15.239 → 15.203（−0.24%） | 9.030 → 11.127（+23.23%，见下文） | 9.100 → 8.986（−1.25%） |

GNU 标量对照两版各 74 条指令，除重定位地址外一致，函数相差 32 B 且保持相同的 32 B 对齐。原始样本中两版均从约 11 ms 转为约 9 ms，独立中位数恰好落在变化两侧；原始回退保留。仅对此项追加 2 亿次迭代、9 组交错复测，得到 **89.641 → 89.649 ms（+0.01%）**，未重跑整套验证或修改代码迁就计时。

机器码确认三个用户成员循环移除了调用；Darwin 的调用者仍有入口和循环两处完整 poll、合计 8 条 `ldar`，本项没有改动 poll 协议。收益包含被消除成员原有的 TLS／入口 poll 成本。`Long.hash` 循环改为调用 20 B 的 native bridge，原 116 B 包装及其四次 acquire 读取不再在该调用路径执行；底层 hash 计算及 bridge 调用保留，因此不预期它获得与简单 getter 相同的收益。

| 成本 | Darwin | GNU | musl |
| --- | --- | --- | --- |
| core `.slib` 增量 | 23,472 B（+0.056%） | 22,126 B（+0.044%） | 24,360 B（+0.048%） |
| provider／consumer `.slib` 增量 | 1,242／5,104 B | 1,252／2,832 B | 1,242／2,814 B |
| 程序 `.text` 增量 | 80 B | 64 B | 64 B |
| 完整程序前 → 后 | 10,597,944 → 10,596,824 B | 12,412,280 → 12,411,080 B | 12,021,352 → 12,016,056 B |
| provider／consumer 编译墙钟变化 | −2.49%／−0.78% | −0.24%／+0.61% | +0.98%／−0.11% |
| provider／consumer 编译峰值 RSS 变化 | +0.57%／+0.03% | +0.35%／+0.36% | +0.35%／+0.29% |

编译成本使用不请求 dump 的 `scoopc build`，分别编译 provider 和 consumer，排除最终链接；一组预热后取 5 组交错中位数。core 各自只测一次冷构建，三目标约 14.8～15.4 秒，不能据此宣称编译提速。更新开发 sysroot 所需的一次性依赖闭包重建不包含在这个单 Cone 对照中。普通成员正文与实际代码增长均很小，三个目标的主要适用负载收益一致，因此沿用 release 默认选择，不增加独立开关或等级。

原始输入、工具快照、IR／机器码、A/B 样本、编译成本及失败修正记录保存在两机忽略的 `tmp/program-performance/member-inline/`，不提交原始报告。

# 接口检查与视图建立共用一次查询

状态：实现与定向验证完成，release 默认启用。接续已交付的父表直接投影，以 `a3d3b0bb1` 为性能基线。

后续已加入[循环中稳定来源的查询复用](INTERFACE-QUERY-REUSE.md)：满足该条件的 Boolean／空接口检查和稳定来源的 catch 路径也可复用首次查询。本篇记录此前检查／取表合并的独立设计与测量，循环中的当前行为及成本以该后续记录为准。

## 当前问题与选择

父表投影已经使 Child 的继承调用免查表，但 Any 的接口 smartcast 仍先用 `is_instance` 判断，再用 `itable_lookup` 建立视图。两者都会扫描实际类型的接口目录。当前 `smartParent`／`smartChild` 基准每轮各有一次类型检查和一次取表。

新增一个普通 NoGC runtime leaf：`scoop_rt_itable_find(object_td, interface_td)`。返回实际静态目录项，缺失返回 null；成功时 entry 非 null，空接口的 slots 可以为 null。入口不分配、不抛异常、不登记对象，不增加 TD 字段或运行时缓存。原完整类型关系检查和保证存在的 lookup 保持其合同。

## 编译器路径

release 只融合有实际非空接口视图消费者的条件检查，包含否定条件及 `as`／`as?` 展开后的成功分支。同块 Boolean 临时值沿最后写入连接到原检查，查询仍在原位置执行一次；Boolean 取地址或检查后对象重新赋值则不复用。纯 Boolean、空接口及 debug 检查保持原路径。

MIR 的普通 CFG 上追踪同一未取地址局部值、同一 exact interface、同一次成功检查；赋值和调用结果写入使事实失效，合流取共有事实，循环求固定点，异常入口清空。没有查询的路径不能使用另一路的结果；不同查询不通过额外 phi 合并。lowering 只保留 metadata local，成功后按需加载 slots；已有 Child→Parent 投影与继承方法槽调用继续适用。

目录项在 GC 后仍有效，每次构造接口值使用当前的 managed object。普通调用的正常继续路径保留事实，原 safepoint 和异常边不变，不增加对象引用存活或 GC roots。

## 产物与验证

runtime ABI contract 15 与 NoGC runtime tag 25 表示新增入口；metadata ABI 9、父表布局 capability 17 及所有 profile 的对象／接口布局保持。正常产物和开发 sysroot 按 ABI 版本重建。

定向验证覆盖成功／失败、否定分支、受检转换、空接口、exact 泛型、装箱、局部写入、合流、循环、异常继续路径及移动 GC。使用 typed LIR 和最终机器码确认适用路径的一次 find 同时完成检查与取表。性能复用现有接口基准、预构建 AB／BA 工具和真实 checksum；记录运行、文件体积、编译成本以及任何回退。原始数据保存在忽略的 `tmp/program-performance/b1-checks`，不运行完整 fixture 验收。

## 验证结果

- 格式化及 workspace Clippy 通过。本机通过 17 项接口 lowering 测试、5 项 runtime ABI 测试、8 项 `.slib` 兼容测试及 1 项真实 C 查询测试。runtime contract 与复合 ABI 的固定向量已同步更新。
- Darwin ARM64、Linux GNU／musl x86-64 各通过三组正式 CLI 专测：`interface-view-reuse`、`interfaces-artifacts`、`conditional-interface-roots`，每个目标共 5 个 variant。包含 debug／release 反向跨 Cone、独立重链接，以及 moving／minor GC。初次新增 Option 用例误触发 `equals` 要求，改为正常 `when` 模式后闭合；没有改动该语言规则。
- 三目标的 `smartParent`／`smartChild` 最终机器码均由各一次 `is_instance` 加一次 `itable_lookup` 变为一次 `itable_find`。已有 `parentView`／`secondaryView` 与继承方法调用仍为零查询；Child 完整父表布局没有变化。
- 真实 `as`、`as?`、否定条件和循环前 guard 的 LIR 各只有一次 find，成功路径不再 lookup。原检查经 Boolean 临时值传递时仍在原位置执行。显式 GC 发生在检查和首次视图之间的用例也只有一次 find，移动后从当前 object 和静态 entry 建立视图。
- 分支合流、源值写入、取地址、循环回边及异常入口的 typed LIR 覆盖证明失效边界。代表性成功路径没有增加 managed local 或根；正常 invoke 继续路径保留 metadata。catch 中保守重建的接口视图仍 lookup，异常类型的 class 检查仍使用 is_instance。

## 运行测量

复用[接口基准](../../tests/benchmarks/interfaces/README.md)，双方预先构建，各模式 2,000 万轮，已知 stdout 为 `4287603176`。预热一对、AB／BA 交错九对，关闭 GC stress 与统计；Linux 固定逻辑 CPU 0。Darwin 使用 M3 Ultra／macOS 27.0.1／LLVM 22.1.8，Linux 使用 i7-12700H／Ubuntu 26.04.1／LLVM 22.1.2。表中是独立进程墙钟中位数及耗时下降，负数表示变慢；不混用内核计时，也不将中位数比值当成显著性检验。

| 模式 | Darwin 基线 → 候选 ms | 下降 | GNU 基线 → 候选 ms | 下降 | musl 基线 → 候选 ms | 下降 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 4：已有 Child 的继承调用，对照 | 112.923 → 113.436 | −0.45% | 80.306 → 80.879 | −0.71% | 105.656 → 110.421 | −4.51% |
| 5：Any → Parent smartcast | 160.004 → 150.488 | 5.95% | 136.941 → 101.128 | 26.15% | 164.887 → 126.843 | 23.07% |
| 6：Any → Child smartcast 后继承调用 | 173.381 → 124.488 | 28.20% | 144.862 → 103.535 | 28.53% | 173.221 → 128.806 | 25.64% |

两个目标热点的 CPU 中位数同向下降。Darwin 的 Parent 模式九对中八对变快，其余目标热点均九对变快；保留全部样本。Darwin 进程峰值 RSS 中位数减少 32～48 KiB，Linux 不变。这是相对父表投影版本的增量测量，不能与上项不同时段的中位数相乘形成累计加速比，也不代表一般程序或跨语言综合性能。

musl 对照组的回退保留为实际限制：两版 `repeatedParent` 都是 88 条指令，除重定位地址外指令及操作数相同，入口从 `0x4b5580` 移至 `0x4b5550`；没有新增查表或根。独立内核墙钟中位数也从 88.492 ms 增至 93.465 ms，不能将全部差异归为启动。布局变化已确认，具体微架构原因尚未通过硬件计数器归因；后续函数／循环布局工作应保留这个对照，不为单个基准添加补丁或报告“所有路径都变快”。

## 空间、编译成本与默认策略

三目标完整程序都保持 2,099 个表视图、547 个非空物理指针槽、1,579 个空表地址定义及 5,955 bytes 的实际表存储；相交的父表片段只计算一次。没有新增 TD 字段、方法副本、对象字段或 GC roots。

| 目标 | 完整程序 bytes：基线 → 候选 | provider `.slib` bytes：基线 → 候选 | provider 编译 ms：基线 → 候选 | 编译峰值 RSS 变化 |
| --- | ---: | ---: | ---: | ---: |
| Darwin | 12,978,024 → 12,978,072 | 1,232,038 → 1,231,680 | 994.471 → 986.519 | −1,536 KiB |
| GNU | 12,658,312 → 12,658,360 | 1,534,384 → 1,533,856 | 1,362.064 → 1,349.751 | −596 KiB |
| musl | 12,263,264 → 12,263,304 | 1,534,184 → 1,533,610 | 1,315.247 → 1,334.110 | −552 KiB |

编译测量使用同一 provider 源码，每轮独立目标目录和缓存，双方各自兼容的预编译 core；预热一对，再交错五对。墙钟差异为 −0.90%～+1.43%，CPU 差异为 −1.03%～+0.32%；没有观察到需要单独优化等级的编译或静态元数据成本。

查询合并作为 release 默认能力，仅为有实际视图需求、数据流条件成立的检查生成；debug 保持原操作，所有 profile 遵守同一 ABI。目标路径明确消除一次目录搜索，静态投影对照没有引入新指令，因此本项不增加开关，也不据单个布局敏感样本调整全局函数对齐。对象重新赋值、无共同检查的合流、异常入口、非局部来源及需要独立恢复的 adapter 继续各自查表；Any 首次接口查询仍是线性目录扫描，本项没有建立 runtime 缓存或 PGO。

# 有界引用循环的页表查询提升

本项接续[卡表幂等原子写入](CARD-STORES.md)，以 `6f5637f26` 为基线。单引用数组写入从数组对象基址定位 mapping；release 在合格的有界内层中，将四级页表及 immutable base／cards 查询移到进入边。每次实际写入后的 card store 仍保留。

下文保留首批实现与测量。后续[接口栈重取扩展](INTERFACE-CARD-REGIONS.md)已在 ARM64 release 允许保留栈 volatile 重读并提升独立查询，覆盖此前未命中的接口复制、投影与初始化；x86 试验未获稳定收益，保留本篇的区域选择规则。

## 依据与实现

一个数组对象的全部存储属于同一个 region 或 large mapping，即使元素跨越 64 KiB，也不需要逐个元素重新确定 mapping。ArraySet 的单引用槽屏障因此用已有数组基址查询，再按实际被写引用槽的地址计算 card；多引用元素继续使用原范围屏障。数组大小、元素表示与 runtime ABI 不变，所有 profile 采用同一基址规则。

先修改运行时规范 3.6 和实现规范 2.5／2.19，再复用后端已有的有界区域与稳定根槽分析：实际 CFG 必须为单入口、单 latch、去除回边后无环、无提前退出的 1～64 次内层。区域内有调用、volatile、用户原子或未知 effect 就保留原操作；待提升查询必须每轮必经，输入只能来自循环外值或内层不写、不取址的入口根槽。识别依据是 Runtime PageMap 的实际数据引用和加载链，不添加跨阶段证明标记。

只移动符合条件的页表／prefix 加载及必要地址运算。外来前驱还有其他后继时，创建专用 preheader 并修正 header phi；零次循环、未进入的分支和 checked 回退不会多做查询。前置查询发生在外层 poll／引用恢复之后，每个完整块重新取得 metadata，不带过下一次 safepoint；根恢复、逐次引用写入和 card store 留在原处。

## 定向验证

- Darwin 与 Linux 各通过 9 项有界循环后端测试，含三个目标的 LLVM safepoint 验证、物理站点及根数量检查；覆盖专用进入边、必经合流、条件写入、变化目标、根槽重赋、调用／volatile／用户原子、动态次数与超过 64 次的拒绝路径。既有整数／严格浮点向量化测试同时通过；格式化与 workspace Clippy 通过。
- Darwin ARM64、Linux GNU／musl 各完成 [runtime-cards-moving](../../tests/fixtures/gc-correctness/codegen/runtime/cards-moving.fixture.toml) 与 [reference-array-roots](../../tests/fixtures/gc-correctness/poll/reference-arrays/fixture.toml) 两组专测，每目标合计 5 个变体、11 次成功进程调用。覆盖 debug／release、跨卡与跨 64 KiB、旧数组唯一持有年轻对象、泛型父接口投影、wrapper、AtomicRef 以及并发 moving／minor。
- 三目标整个基准的 LIR 逐字节相同，235 条 TD／接口表／布局记录及 16 个函数的 poll／live 集合保持。22 种负载的分配、minor／full、晋升／复制、dirty cards、扫描、root slots 等 15 项 GC 计数相同。
- 实际 class／String／闭包／可空引用复制、wrapper 清零及普通引用初始化命中。完整块内的一组四级查询从每 64 元素执行 64 次降为一次；ARM64 `copyNodes` 内层从 27 条指令降至 11 条，GNU／musl 从 30 条降至 10 条，内层不再有页表加载，仍逐次写 card。短尾与未命中路径保留查询，引用循环没有取得 SIMD 资格。

首次本机入口误用了旧 debug 工具，musl companion 使用了相对 CC 路径；改用本次 release 工具和绝对 CC 路径后，相应定向重跑通过。没有全量验收，也没有重建未变化的系统库或 runtime。

## 运行性能

双方编译相同[引用数组基准](../../tests/benchmarks/arrays/references)，release、相同预编译 core 和 runtime。环境为 M3 Ultra／LLVM 22.1.8 与 nuc12 i7-12700H／LLVM 22.1.2，Linux 固定 CPU 4。每模式一对预热、五对 AB／BA，GC 统计另跑；长数组为 65,536 元素、128 轮，单元素与首项命中为 10 万轮。除初始化模式外，准备阶段位于内核计时之外。

下表为内核墙钟中位数变化，负数表示耗时降低。

| 操作 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| class 引用复制 | −79.84% | −47.90% | −45.26% |
| String 复制 | −79.55% | −45.95% | −48.25% |
| 闭包复制 | −79.39% | −45.88% | −48.61% |
| 可空引用复制 | −79.66% | −46.89% | −44.91% |
| MaybeUninit 清零 | −24.93% | −38.37% | −39.90% |
| class 引用分配与初始化 | −6.14% | −6.35% | −4.70% |
| wrapper 分配与空初始化 | −1.99% | −5.27% | −4.94% |
| 泛型 interface 复制 | −0.79% | −3.22% | −0.17% |
| Child 数组投影到 Parent | −1.44% | −1.73% | +2.20% |
| interface 分配与初始化 | −0.47% | +1.65% | +1.27% |

前四类复制的整进程耗时降低 Darwin 34.22%～35.35%、GNU 17.89%～19.65%、musl 12.69%～14.05%。这些是当前负载的增量结果，不代表所有程序或跨语言排名。

后三类接口操作仍有为移动 GC 保留的 volatile 引用重取，未满足本项的区域条件；页表查询仍在内层。它们只采用新的数组基址规则，静态 Child→Parent 投影继续没有接口目录查找。保留表中全部变化，不将接口路径计为查询提升的命中。

对照中，Darwin 末项身份搜索首轮 +4.90%，九对单独复测 +1.39%；musl 标量求和首轮 +3.49%，九对复测 +3.48%。两者实际内联于 `exercise`，样本范围重叠；musl 实际内联求和内层及相邻进入／尾部指令归一化后相同。保留这些回退，布局与测量因素尚未拆清。其他字段／可空求和变化为 −0.65%～+2.17%，NoGC 内核为 −0.06%～+0.04%。六类单元素模式最大绝对回退约为 musl 投影的 0.280 ns／次（+3.00%），首项命中最大回退约 0.024 ns／次。

## 空间、编译与默认策略

| 成本变化 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| text | −124 B | −256 B | −256 B |
| 完整程序文件 | 0 B | 0 B | 0 B |
| 根 Cone `.slib` | −192 B | −208 B | −208 B |
| 编译墙钟 | +0.45% | −0.61% | −0.75% |
| 编译 CPU | +0.64% | −0.39% | −0.89% |
| 编译峰值 RSS | +0.71% | −0.11% | +0.04% |

编译测量只编译根 Cone，消费相同预编译 core，关闭 dump，独立输出，一对预热后五对 AB／BA，不包含最终链接。各目标除 text 外的段大小不变；接口值、对象、static tables、roots、`.slib` 字段和全部 ABI 没有增加。

命中的写入负载在三目标均获得稳定收益，空间与编译成本低，采用 release 默认开启，不增加独立开关。已同步[开启条件清单](ENABLED-OPTIMIZATIONS.md)。含 volatile 接口视图、条件执行的查询、多引用范围屏障及更广循环仍待独立处理；同卡合并、bulk barrier 和引用 SIMD 未包含在本项中。

原始产物、样本和复测保存在两机忽略的 `tmp/program-performance/card-regions`。清理本机 81 份旧增量会话，释放约 1.16 GiB，保留各 crate 最新缓存。

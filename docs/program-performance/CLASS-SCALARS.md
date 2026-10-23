# 局部 class 与游标字段替换

状态：已完成。基线为 `3b4ac86e8`，属于 B1 抽象消除；不依赖 PGO。普通 release 默认启用，沿用已有 allocation-elision 选项，不新增等级或 CLI 开关。

## 已确认的问题

基线的分配消除只登记 Box 与 ClosureAlloc。真实源码探针中的 Point 和 Cursor 构造器、字段访问器及游标方法已被内联，最终 MIR 仍保留每轮一次 ClassAlloc、普通字段读取与写入。Cursor 在外层每轮创建，内层循环更新 current；需要保留可变字段状态。

## 实现选择

扩展现有来源与支配分析，登记普通 declared class 的创建点，包括 Assign 形式。无 release hook、来源唯一、创建点支配全部使用且对象只经纯别名和普通字段读写使用时，消除实例并为每个 flattened 字段建立 typed local。字段赋值保留原位置与值复制，读取得到当前字段值；引用字段继续进入正常 roots 与移动回写。

构造初始化就绪由语言 9.1.1 保证；优化不重放该检查、不合成非法 null 默认值。未内联调用、逃逸、身份、取地址、原子对象操作和未闭合 EH 保留对象。现有循环回边检查防止把跨创建迭代的多个实例合成一份状态。class/constructor/TD 的语义与 ABI 不改变，不增加栈对象、部分物化或通用逃逸框架。

先修订[实现规范 2.19](../specs/SCOOP-IMPL-SPEC.md)，再扩展 [allocation](../../compiler/mir-lower/src/optimization/allocation.rs)。原快照模块改为 [storage](../../compiler/mir-lower/src/optimization/allocation/storage.rs)，共同承载 box／closure 的初始化快照与 class 的可变字段；原始构造 FieldSet 是字段的定义位置。debug 保留原分配路径，所有 profile 使用相同对象／接口布局和跨 Cone 契约；无需 ABI 或 `.slib` 格式升级。

## 定向正确性与生成代码

- 格式化、workspace clippy 通过；只运行 6 项 class 测试和 16 项直接相关的 box／closure 回归，共 22 项 Rust 测试。覆盖 ValDecl／Assign、标量与引用快照、基类字段视图、内外循环、别名合流、取址、身份、未知调用、release hook 和 EH 保留。
- [class-scalars](../../tests/fixtures/compiler-correctness/optimization/lowering/class-scalars/fixture.toml) 在 debug／release 对照构造顺序与副作用、异常退出、泛型可变字段、游标和跨迭代身份；[class-scalar-roots](../../tests/fixtures/compiler-correctness/optimization/lowering/class-scalar-roots/fixture.toml) 用同一个 release 程序验证强制移动与 minor GC。包含继承、struct／tuple／String／接口字段、更新前快照及构造期间两个字段初始化之间的 GC。
- Darwin ARM64、Linux GNU／musl 各通过上述两组、三个 fixture variants；没有运行全量 fixture 或 workspace tests。复用预编译 sysroot，无全量系统库重建。
- 真实 benchmark 的 Point／Cursor 每处 ClassAlloc 从 1 降为 0，各自 4 次字段读取和 3 次字段写入全部消失。机器码中相应 TLAB 分配和慢路调用消失；Darwin 指令数为 99→54、120→71，两个 Linux 目标均为 101→64、128→83。LIR 的入口／循环 poll 数保持 2／3。
- GC 专测的 Holder<Payload> 与 Constructed 分配均实际消失。跨 `gcCollect` 的根为仍使用的 Node 与聚合值，Payload 扫描偏移为 0、8、16、24，接口 itab 不作为对象根。构造中尚未赋值的第二字段 local 不进入根集合；已初始化参数／字段值正常保活、移动回写。没有为了优化额外保留 Holder／Constructed 对象。

收尾检查另定位了既有 GC 压力循环的问题：只创建 Node 并读取字段时，新的优化正确消除该对象，等待 minorCount 增加的循环因此不再制造压力。原 gc-writes 的 collectYoung 在 MIR 中已无 ClassAlloc，并在五秒定向探针中持续等待。六组相关 GC fixture 和 minor-mark 基准改为将压力对象写入全局槽，再在下一次分配前清空；既保留实际堆分配，也避免因测试槽保活而额外晋升垃圾。

这部分只补跑受影响的验证：Darwin 的 gc-writes、containers-storage、regions-values、atomics-gc、maybe-uninit-gc、nursery-native-context-copy 共六组、13 个 variants 全部通过；GNU／musl 各补 gc-writes 的 debug／release 与正常／moving／minor 组合。三目标 minor-mark 冒烟检查均完成两轮、每轮一次 minor GC，checksum 为 1,047,552；修改后的 collectYoung 保留一次 Node 分配及原来的两个循环 poll。编译器实现和以下性能负载没有因这项测试修正而改变。

## 性能与分配

[classes.scoop](../../tests/benchmarks/allocation/classes.scoop) 复用既有 phases.c 与 compare.py。两版使用相同源码、预编译 core／std、runtime、release profile 和单个 GC worker。Point／无对象对照运行 10,000,000 轮，Cursor 运行 2,000,000 轮，ArrayList 运行 500,000 轮；后两者每轮读取 16 个值。先预热一对，再串行执行七对 AB／BA；各次均校验结果。正常计时关闭 GC stress／统计，分配诊断另跑一次。

Darwin 为 M3 Ultra／macOS 27.0.1／LLVM 22.1.8；Linux 为 i7-12700H／Ubuntu 26.04.1／LLVM 22.1.2。内核时间由程序中的 monotonic clock 记录，进程时间还包含启动／初始化／输出。以下均为中位数，负百分比表示耗时降低。

| 目标 | 模式 | 进程时间，ms | 变化 | 内核时间，ms | 变化 |
| --- | --- | ---: | ---: | ---: | ---: |
| Darwin | Point | 161.703 → 40.929 | −74.69% | 141.809 → 20.343 | −85.65% |
| Darwin | Cursor | 90.105 → 76.044 | −15.61% | 69.965 → 56.113 | −19.80% |
| GNU | Point | 176.521 → 29.782 | −83.13% | 163.083 → 17.185 | −89.46% |
| GNU | Cursor | 70.504 → 59.861 | −15.10% | 58.333 → 47.876 | −17.93% |
| musl | Point | 179.897 → 44.363 | −75.34% | 152.188 → 17.167 | −88.72% |
| musl | Cursor | 85.638 → 74.455 | −13.06% | 58.414 → 47.931 | −17.94% |

三个目标的分配结果相同：Point 总量 320,000,112→112 bytes、minor 306→0；Cursor 64,000,112→112 bytes、minor 61→0。剩余 112 bytes 来自程序初始化／输出；热点对象的分配归零。ArrayList 两版均为 24,000,296 bytes、22 次 minor；无对象对照两版均为 112 bytes。没有以统计模式的时间代替正常计时结果。

| 目标 | ArrayList 进程／内核耗时变化 | 无对象对照进程／内核耗时变化 |
| --- | ---: | ---: |
| Darwin | −0.23%／+0.04% | −0.42%／−0.69% |
| GNU | +2.27%／+1.64% | −2.34%／+0.40% |
| musl | +0.21%／+0.45% | −1.77%／−1.73% |

本项版本中，ArrayList 的 `iterator()` 构造器已展开，`next()` 虽已去虚化为 direct call，仍未内联，故保留该迭代器；后续由[字段替换指导内联](ITERATOR-INLINING.md)继续优化。两个 Linux 目标的 ArrayList／无对象对照分别为 301／64 条指令，排除重定位后的指令和符号引用一致；ArrayList 入口也未移动，但 TD 和 runtime 被调函数的位置有变化。保留本项实测回退，没有硬件计数归因，不把未命中的集合路径计为本项收益。

Point 的本机旧版内核样本为 109.344～147.143 ms，新版为 19.839～20.682 ms，说明旧版分配路径存在波动；七对样本的中位数不是显著性证明。以上是相对 Scoop 上一版的特定负载结果，不表示跨语言综合性能已经达到 Go／Java／Kotlin Native。

## 编译与空间成本

使用相同 release 宿主工具直接运行 `scoopc build`，显式消费同一个预编译 std 与 core；一对预热后计五对 AB／BA，每次产生独立 `.slib`，不计应用 cache 命中或最终链接。没有与基准计时并行构建。

| 目标 | 编译墙钟，ms | 变化 | 编译峰值 RSS 变化 | `.slib` bytes |
| --- | ---: | ---: | ---: | ---: |
| Darwin | 1,107.489 → 1,132.598 | +2.27% | −0.56% | 863,132 → 859,910 |
| GNU | 1,521.479 → 1,541.702 | +1.33% | −0.09% | 1,071,314 → 1,066,574 |
| musl | 1,537.549 → 1,532.451 | −0.33% | −0.12% | 1,071,598 → 1,066,552 |

| 目标 | 完整可执行文件 bytes | text bytes | text 变化 |
| --- | ---: | ---: | ---: |
| Darwin | 18,083,976 → 18,083,272 | 1,020,448 → 1,020,072 | −376 |
| GNU | 17,631,456 → 17,630,856 | 1,153,553 → 1,153,233 | −320 |
| musl | 17,027,504 → 17,026,904 | 1,223,230 → 1,222,846 | −384 |

比较的 TD／类型布局记录完全相同；保留语义支持定义，未引入接口表、方法机器码或对象体积的额外副本。Darwin stackmap 区另减少 152 bytes。基于热点分配确定消失、三目标收益及小幅编译成本，作为 release 默认能力交付。跨调用、复杂合流、部分物化、带 EH 的消除和真实库迭代器的后续内联仍按独立改动推进。

原始源码探针、IR／机器码、日志及样本位于两机忽略的 `tmp/program-performance/class-scalars`，不提交原始报告。

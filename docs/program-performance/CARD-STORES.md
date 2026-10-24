# 卡表置脏的幂等原子写入

本项接续[引用数组](REFERENCE-ARRAYS.md)在 GNU／musl 上带屏障写入基本持平的结果，以 `b394442e8` 为基线。生成代码和 runtime 范围屏障都改为 relaxed atomic store 1；不移动屏障，也不引入循环批量标记、缓存或新的优化等级。

## 依据与实现

当前 card storage 的每个字节只有 clean 0 与 dirty 1。映射发布前清零，收集结束的清零发生在所有 mutator 停稳、相关扫描和引用更新完成之后；没有 mutator 并发清零者，也没有其他状态位。此前每次置脏仍用原子 OR 1：x86 执行锁定读改写，Scoop 的 ARM64 单槽生成代码执行独占读写重试。

并发写入相同的 1 不丢失脏状态，因此直接使用 relaxed 原子写入即可。先修改运行时规范 3.6 与实现规范 2.5，再调整 codegen 单槽标记和 C runtime 范围标记。每个实际写入的覆盖范围、四级 acquire page-map 查询、原有局部字段复用、屏障位置以及失败 CAS 不标记的规则保持。修改不依赖数组循环识别，也适用于普通字段与其他含引用写入。

对象、card 字节、接口表、roots 和全部 ABI 保持。旧产物的 OR 1 与新 store 1 等价，可以与新 runtime 混合；测量和正式 fixture 继续消费已有预编译系统库。刷新了测试 sysroot 的普通／stress runtime，没有为本项重建全部内置 Cone。若以后引入并发清卡或更多状态，需要重新设计相应合同，不能直接沿用本项的二值证明。

## 定向验证

- Darwin 与 Linux 各通过 11 项相关 Rust／C 测试，涵盖单槽原子性／内存序、局部字段查询与同槽复用、原子发布／调用／block／局部重赋的失效边界，以及 range barrier 的相交卡范围和两个线程标记同一卡。格式化与 workspace Clippy 通过。
- Darwin ARM64、Linux GNU／musl 各通过 [runtime-cards-moving](../../tests/fixtures/gc-correctness/codegen/runtime/cards-moving.fixture.toml) 与 [reference-array-roots](../../tests/fixtures/gc-correctness/poll/reference-arrays/fixture.toml) 两组正式专测，每目标 5 个变体、11 次进程调用。覆盖 debug／release、跨卡字段、AtomicRef 发布与成功／失败 CAS、旧数组唯一持有年轻值、跨 64 KiB、泛型接口投影、wrapper 清零及并发 moving／minor。
- 三目标整个基准的 LIR 逐字节一致，235 条 TD／接口表／布局记录保持；没有新增 safepoint、根或改变 live set。各目标 22 种负载的分配、minor／full、晋升／复制、dirty cards、old reference slots、root slots、traced objects 等 15 项计数一致。
- 核对 10 个实际写入内核：原有 x86 `lock` 标记和 ARM64 `ldxrb`／`stxrb` 重试均消失，仍有必要的字节写入。C runtime 范围屏障也从 x86 `lock orb`／ARM64 `ldsetb` 变为普通字节 store；原子性由 LLVM／C atomic 操作及目标机器保证。NoGC 对照与独立标量求和的归一化机器指令相同。

只运行上述定向验证；遇到测量回退时仅复测相应负载，没有全量验收。

## 运行性能

双方编译相同[引用数组基准](../../tests/benchmarks/arrays/references)，release、基础目标、相同预编译 core，各自使用对应 runtime。每模式一对预热、五对 AB／BA，GC 统计另跑；Linux 固定 CPU 4。环境沿用 M3 Ultra／LLVM 22.1.8 与 nuc12 i7-12700H／LLVM 22.1.2。长数组为 65,536 元素、128 轮，短数组与首项命中为 10 万轮。除初始化模式外，准备阶段位于内核计时之前。

下表为内核墙钟中位数变化，负数表示耗时降低。

| 操作 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| class 引用复制 | −13.83% | −77.08% | −77.71% |
| 泛型 interface 复制 | −8.12% | −70.04% | −71.43% |
| Child 数组投影到 Parent | −8.66% | −69.54% | −70.79% |
| String 复制 | −15.88% | −77.72% | −76.84% |
| 闭包复制 | −14.97% | −77.71% | −76.91% |
| 可空引用复制 | −12.29% | −76.72% | −77.83% |
| MaybeUninit 清零 | −13.37% | −81.49% | −82.24% |
| class 引用分配与初始化 | −2.50% | −32.05% | −30.37% |
| wrapper 分配与空初始化 | −2.46% | −31.97% | −31.29% |
| interface 分配与初始化 | −3.61% | −18.56% | −17.80% |

前七项的整进程耗时降低 Darwin 4.94%～7.52%、GNU 54.85%～62.90%、musl 47.59%～55.31%。这些是当前基准的实测中位数，包含生成单槽与 runtime 标记的共同改动；没有将二者的贡献单独拆分，也不外推为其他程序或跨语言排名。

对照与回退单独保留：字段求和、可空求和及独立标量求和的内核变化为 −1.25%～+0.53%，NoGC 为 −0.20%～+0.15%。GNU 实际内联的末项身份搜索首轮为 +7.26%，九对单独复测为 +4.71%，样本范围仍明显重叠；该实际路径位于含准备代码的 `exercise` 中，不能用独立 `findNode` 指令相同替代它的归因。整进程仍因准备阶段变快而降低 14.83%，不能据此抹去搜索内核的回退。

六个单元素模式中，最大绝对回退为 musl 求和约 0.611 ns／次（+7.91%）；九对单独复测为 −4.97%，保留首轮和复测，不将其确定归因为本项优化。Darwin 初始化回退约 0.45 ns／次（+2.75%）。GNU／musl 的短引用写入普遍获益。首项身份命中变化为 −4.88%／−0.06%／+0.04%。

## 空间、编译与默认策略

| 成本变化 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| text | 0 B | −64 B | 0 B |
| 完整程序文件 | 0 B | 0 B | 0 B |
| 根 Cone `.slib` | +8 B | −80 B | +1,346 B |
| 编译墙钟 | +0.25% | +1.45% | +0.87% |
| 编译 CPU | +0.22% | +0.79% | +0.85% |
| 编译峰值 RSS | −0.71% | +0.14% | +0.19% |

编译测量只编译根 Cone，消费相同预编译 core，关闭 dump，独立输出，一对预热后五对 AB／BA；不包含最终链接。完整程序大小相同不表示内容相同；机器码、relocation 与布局位置有变化，静态表与 GC 数据表示没有增加。`.slib` 的最大增量约 0.037%，没有引入新元数据字段。

本项去掉了当前 GC 不需要的原子读改写，三目标写入获益，编译与空间成本低，作为所有 profile 和 runtime 的默认实现，不增加独立开关。已同步[开启条件清单](ENABLED-OPTIMIZATIONS.md)。每次写入的 card store 和页表查询仍存在；同卡合并、批量屏障、metadata 提升与引用 SIMD 继续依据实际成本独立推进。原始产物、样本及复测保留在两机忽略的 `tmp/program-performance/card-stores`。

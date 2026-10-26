# minor 的脏卡组清单

相对 `00b272145`，minor 的 remembered 枚举和收尾清卡只处理本轮登记的脏卡组，不再枚举全部 region／卡表；full 专用 region 标志也只在 full 后重置。所有 profile 使用同一 runtime ABI contract 17，metadata ABI 9、对象头、TLAB 和精确 roots 保持。旧 ABI 16 产物必须重建。

## 布局和热路径

每张卡仍为 512 B 的二值标记，每 64 张卡组成一个 32 KiB 组。组节点为 16 B，保存 atomic next 和不可变 owner region；region 的共享 prefix 在原 base／size／cards 后增加 offset 24 的组数组指针。首次写入用 CAS 取得节点并 release 加入 heap 私有链，其他写入者看到已登记状态后跳过；首次登记线程在发布完成前不能 park。STW acquire 遍历清单，引用更新结束后只清本组 64 B 和链接，清链先于 region 撤销。

生成代码先保留完整的 card store／登记对，让原有有界循环合并同时移动二者；随后用临时 LLVM helper 和标准 always-inliner 展开 relaxed 状态读取，仅首次登记调用 NoGC runtime。临时 helper 不成为最终机器入口，未增加 TLS、锁、分配或诊断代码。runtime 范围屏障采用相同登记合同。

组 metadata 占映射容量的 0.0488%，每个 region 另增加 8 B，heap 全局状态增加 8 B。节点均为 metadata 地址，不成为 managed root，不增加对象体积或用户引用存活范围。

## 定向验证

本机格式化、workspace Clippy、22 项 LIR runtime ABI 与 24 项相关 codegen／runtime 测试通过；Linux 通过相关 Clippy、7 项卡区域 IR 和新增 O2 native 测试。覆盖组边界、重复登记、8 个并发写入者、真实稀疏大数组、无引用大对象、晋升预留失败转 full、重复回收和 region 撤销。

Darwin／GNU／musl 各通过 reference-array-kernels、reference-array-roots、pointer-free-large-remembered 三组正式专测，共每目标 6 个变体。旧 large-remembered 用例的临时 Node 分配在基线和本次版本中都已被合法消除，不能触发它所断言的 minor；改为让旧数组实际持有分配对象后，新旧版本均通过。没有放宽 collector 检查或运行完整验收。

IR 和真实机器码确认完整登记随批量卡标记移动，已登记路径只有普通／relaxed 读取和分支；最终程序没有 `scoop.card_group.fast` 入口，慢边直接调用 runtime。用户调用、poll、stackmap 记录与原有内联结果保持。

## 测量与代价

原始 trees（depth 20）、strings、strmap 使用相同源码、各自 ABI 相容的预编译系统库和 runtime。普通计时一对预热、三对交错进程，中位数如下；另跑一对 GC 诊断。String API 改名没有混入本次对照。

| 程序 | Darwin ARM64，秒 | Linux GNU，秒 | Linux musl，秒 |
| --- | --- | --- | --- |
| trees | 12.8762 → 12.7636（−0.87%） | 17.5695 → 18.3773（+4.60%） | 20.6837 → 20.3889（−1.43%） |
| strings | 0.48944 → 0.48082（−1.76%） | 0.50600 → 0.50408（−0.38%） | 0.93904 → 0.91703（−2.34%） |
| strmap | 3.27219 → 3.24127（−0.95%） | 3.75797 → 3.77645（+0.49%） | 5.62315 → 5.65237（+0.52%） |

GNU trees 三对都回退，Darwin trees 两对回退、一对改善；不能宣称三个真实程序都稳定加速。trees 的 remembered 扫描累计从 Darwin 59.72 ms／GNU 62.69 ms 降至 0.50／0.86 ms，但每轮对象标记、晋升、复制等成本仍主导。strings／strmap 有真实脏引用需要扫描，单纯跳过空目录收益有限。分配量保持；trees 的 full 次数随堆布局／并行次序略有变化。

隔离旧堆空扫的 native 用例保留一个脏 owner，额外持有 0／64／256 MiB 的静默旧数组，连续执行 600 次 minor。初始化和 full 在计时外，关闭自动 full 阈值以单独测量显式 minor；每组一对预热、三对交错。

| 额外静默旧堆 | Darwin，毫秒 | GNU，毫秒 | musl，毫秒 |
| --- | --- | --- | --- |
| 0 MiB | 4.255 → 2.721 | 5.038 → 6.254 | 10.104 → 9.148 |
| 64 MiB | 7.731 → 2.230 | 6.473 → 3.666 | 16.124 → 7.033 |
| 256 MiB | 15.008 → 2.212 | 14.167 → 3.634 | 39.882 → 6.990 |

64→256 MiB 时，本次 remembered 扫描累计保持在约 0.11／0.20／0.17 ms，原版随卡表容量增长。GNU 零额外旧堆的短样本回退也保留在表中；这组结果说明消除了空扫的增长项，不代表普通程序获得相同倍数的收益。

另测 800 万次重复字段写入、65,536 元素 class／interface 复制，各五对加一对预热。字段整进程变化为 +1.32%／+1.13%／−5.48%；两种复制的实际计时阶段分别变化 +0.73%／+0.77%／+1.73% 和 +0.01%／+1.50%／+1.47%。musl 两种复制的整进程中位数回退 17.47%／5.67%，阶段回退较小，保留启动与短样本波动，不将其抹去。

三个程序代码段增加 Darwin 23,380～24,520 B、GNU 25,280～26,496 B、musl 25,280～26,560 B；完整文件分别增加 16,704～33,200 B、24,728～32,920 B、28,792 B。相关静态元数据在 Darwin 减少 8 B，Linux 保持；core／std／test 完整 `.slib` 增幅都不超过 0.16%。consumer 独立编译一对预热加三对交错，墙钟变化 −1.10%～+3.45%，峰值 RSS 最大增加约 1.63%。

本项作为统一 runtime 的默认实现，交换的是每次写屏障的小幅固定成本和约 0.049% 的 side metadata，以去除稀疏大旧堆下无界增长的 minor 空扫；不增加多套布局或开关。真实程序中的回退已记录，nursery 自适应、survivor／晋升年龄继续处理当前更大的 GC 成本。

原始输入来自用户提供的 `/tmp/scoop-bench`，所有副本、产物与日志保存在两机仓库忽略的 `tmp/program-performance/dirty-cards/`。本轮另清理 154 份过时的已完成 Rust 增量会话，保留各 crate 最新缓存。

# ArrayList 与 StringBuilder 优化

2026-10-29，分支 `codex/arraylist-stringbuilder`，对照基线 `726a38a96c81adce600a58b658a23fe7adf453d5`。HashMap 实现留待后续，前期分析保留在 [基础容器调研](CORE-CONTAINERS-RESEARCH.md)。

## 实现

ArrayList 的普通 core 正文将每次追加的容量检查与冷扩容函数分开，追加保留本次逻辑长度。扩容先分配清零 backing，再只复制有效前缀；增长循环不再把旧数据复制与空尾部初始化混在一起。插删和 clear 捕获稳定 backing／计数，沿既有范围检查、poll 分块和引用写屏障处理；删除与 clear 仍清零退出有效范围的槽。没有新增列表专用布局、GC 扫描长度或 intrinsic。

MIR 构造复用数组分配器的清零结果：仅当 initializer 是已知直接 lambda／匿名函数或其不可变局部副本，且正文只返回全零值时，省去逐项初始化循环。支持 MaybeUninit.uninit、零整数、false、Unit 及全零 tuple／wrapper 组合。保留实参求值、负数检查和分配；可变函数变量、含用户调用或写入的 initializer 继续逐项执行。实际 release MIR 已确认 ArrayList 构造及扩容不再生成重复清零循环，追加循环中仅扩容分支调用 grow。

StringBuilder 改为 `MutableArray<UInt8>` 与 byteCount，公开 `initialCapacity: Long = 0L`，容量按字节计；clear 只清有效长度，toString 复制有效前缀为独立快照。八种整数、Char、Boolean 使用普通 append 重载。整数复用 core 十进制 writer，先格式化到 20 B 的 GC-free 栈缓冲再复制有效区间；没有中间 String。Char 写入 1～4 个 UTF-8 字节，Boolean 复用常量文本。Float／Double 继续走 generic ToString，格式规则保持。

generic append 先完成唯一一次用户 ToString，再读取当前 builder 状态追加；转换期间的 append、clear、扩容和抛出保留普通调用顺序。短 f-string 的现有展开没有修改，超过八段的后备通过普通重载解析使用新 builder。

四项内部复制 intrinsic 沿 HIR／MIR／LIR 的既有 String 操作通道传递。数组前缀到新 String 保留源到分配后并重新计算地址；String、数组前缀和稳定原生缓冲到已有数组使用无 safepoint 的 ByteCopy，显式保留目标写副作用，不为每次 append 建立 pin／callback。退休 `scoop_rt_string_join_parts`、C 声明和构建清单项，同时删除 ArrayList 的内部 backing getter。runtime ABI contract 从 26 更新到 27，metadata ABI 9 与对象布局不变；配套 core／依赖产物需重建。

## 本机对照

Apple M3 Ultra、macOS 27.0.1、LLVM 22.1.8，普通 release。改前和改后来自同一份探针；所有模式各预热一次，然后各测七次，按轮反转模式及改前／改后顺序。计时期间无并行构建。表中为函数内阶段墙钟中位数，单位 ms；输入准备与最终输出在阶段外。每次运行验证内容或校验和，两个版本的输出一致。

| 场景 | 次数／元素数 | 改前 ms | 改后 ms | 改前／改后 |
| --- | ---: | ---: | ---: | ---: |
| ArrayList<Long> 从零增长追加 | 1,048,576 | 4.659 | 2.475 | 1.88× |
| ArrayList<Long> 预留容量后追加 | 1,048,576 | 2.864 | 1.998 | 1.43× |
| MutableArray<Long> 直接初始化（参照） | 1,048,576 | 0.532 | 0.524 | 1.02× |
| ArrayList<Long> 顺序求和 | 1,048,576 | 0.558 | 0.561 | 约不变 |
| ArrayList<Long>.clear | 1,048,576 | 0.529 | 0.157 | 3.37× |
| ArrayList<Cell>.clear | 1,048,576 | 1.138 | 0.299 | 3.81× |
| ArrayList<Long> 头部插入／移除 64 轮 | 65,536 | 4.459 | 4.410 | 1.01× |
| ArrayList<Long>.toArray | 1,048,576 | 0.694 | 0.693 | 1.00× |
| StringBuilder 追加已有 8 B 文本 | 32,768 | 0.283 | 0.148 | 1.91× |
| StringBuilder 追加递增 Long | 32,768 | 1.239 | 0.351 | 3.53× |
| StringBuilder 追加 Char 雪 | 32,768 | 0.859 | 0.255 | 3.37× |
| StringBuilder 追加交替 Boolean | 32,768 | 0.321 | 0.185 | 1.74× |
| 每追加一个 x 就 toString | 4,096 | 17.686 | 0.314 | 56.32× |
| 固定 1,024 段文本，反复 toString | 2,048 | 5.782 | 0.569 | 10.16× |
| 每次仅追加一个已有 8 KiB String 并 toString | 2,048 | 0.109 | 1.257 | 慢 11.53× |
| 同一 builder 每轮追加 256 个 Long、取快照后 clear | 1,024 | 3.958 | 1.226 | 3.23× |

头部插删模式的“次数／元素数”为初始长度，每轮右移再左移；快照模式为快照次数，clear 复用模式为轮数。顺序求和、数组快照与插删基本不变，收益集中在追加、增长初始化和 clear。

GC 统计另行运行，没有混入上述计时。以下 allocated_bytes 为整进程分配，包含输入准备和最终打印：

| 场景 | 改前 B | 改后 B | minor GC 次数 |
| --- | ---: | ---: | ---: |
| StringBuilder 追加递增 Long | 1,726,056 | 677,512 | 1 → 0 |
| StringBuilder 追加 Char 雪 | 1,671,624 | 360,912 | 1 → 0 |
| 同一 builder 每轮追加 256 个 Long、取快照后 clear | 9,097,504 | 706,832 | 8 → 0 |
| 每次仅追加一个已有 8 KiB String 并 toString | 417,984 | 33,841,344 | 0 → 42 |

Long 追加的整进程分配减少约 61%，Char 追加减少约 78%，clear 复用负载减少约 92%。已有文本／Boolean 的分配量基本不变，主要减少引用槽写入、片段遍历与小段复制。快照仍分配独立 String，重复快照没有加入缓存。

单个已有大 String 是明确的退化场景：旧实现直接返回唯一片段，新实现先复制进 builder，再复制为结果，2,048 次 8 KiB 构建从 0.109 ms 增至 1.257 ms。该代价随现有大段文本的比例增加；clear 也会保留历史高水位字节容量。本轮保留简单的字节缓冲实现，没有引入单片段延迟存储或快照缓存。上述本机微基准不代表任意应用或其他目标的加速比。

## 定向验证

- Rust fmt、workspace Clippy 与改动 Scoop 文件的正式 formatter 通过。
- 6 项编译器单测通过：String intrinsic 声明／签名、原 String copy 与新增字节复制的机器 IR、分配后源重定位与 LIR 根计划、runtime ABI 编码／指纹。
- 定向选择 core consumer 的 17 项测试通过，覆盖列表增长／引用值／插删／快照／零初始化副作用、整数位宽与十进制边界、UTF-8、Boolean／浮点回退、容量、clear、重入／异常、自身追加和长短插值。
- 同时启用 moving 与 minor GC stress，仅重跑 StringBuilder 的 6 项和列表增长的 2 项，全部通过。
- 本机开发 sysroot 按准备脚本重建，用于消费 ABI 27 的预编译系统库；这不运行各库测试。
- 使用该预编译 sysroot 单独构建现有插值 fixture 的 release 版本，IR 检查及 moving／minor GC stress 运行通过。

遵照本次要求，没有运行 `cargo test --workspace` 或完整 fixture 套件，也没有进行 Linux／x86_64 验证。原始计时、GC 数据、编译日志、探针源码与脚本保存在仓库外 `scoop-performance-results/2026-10-29/arraylist-stringbuilder/`，不提交逐次数据。

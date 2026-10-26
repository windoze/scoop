# Runtime 职责收敛

基线 `01047d8c6`。普通算法使用 Scoop 正文，已知表示操作消费 typed IR／现有 codegen，C 保留 GC、线程、平台与确有消费者的共享慢路径。没有新的清理开关；旧产物沿现有 ABI／输入更新机制重建。

## 实际消费者与分批结果

| 入口组 | 核对结果 | 处理 |
| --- | --- | --- |
| bool equals/hash/toString、long/ulong hash/toString、Char toString、String eq、long/boolean print/println | 13 个旧入口均无生产消费者；仅 stage 测试替身、旧文档和 Char GC fixture 显式引用。生产正文已经迁移。 | 删除 C 实现、声明、Char 构建项、重复整数 writer 及其专属测试。stage 测试使用明确的 synthetic extern；GC 用例继续验证真实 Char 正文。 |
| String 长度／定位／UTF-8 转换／长 hash | 原 core extern 是实际消费者；单字节读取与复制已有生成路径。 | 已迁普通 core 扫描、编码、strict／lossy 校验及共享长 hash；退役旧 C 算法、聚合结果 shim 与测试依赖。 |
| String 有界 leaf、复制／concat、Array clone、box/unbox | 字符串长比较／解码及动态 concat 仍有真实共享调用；clone 和已知 box 已有完整布局。 | clone／box 统一使用生成路径并退役旧入口；动态拆箱检查及实际共享字符串后备保留。 |
| Context 操作 | runtime callback、try_get 和初始线程任务仍有 native 消费者。 | fork／current／enter／leave／restore 使用 typed 生成操作；保留 ensure_root、snapshot、try_get、push 的实际 C 职责。 |
| Float／Double | Ryu 核心有实际消费者，C 还承担文本适配；解析有逐次 malloc。 | 保留成熟数字算法，迁移格式规则并消除不必要的解析堆缓冲。 |
| compiler／caller root frame | 编译器已提供完整 entries，C 仍重放 entries 与链表检查。 | 本批收敛生成发布／撤销；native 根边界仍承担自身检查。 |

第一批 runtime ABI contract 升至 20，metadata ABI 9 与对象布局不变。不为历史公开头文件保留无人消费的重复实现。删除死代码只计维护与静态体积收敛，不计运行性能收益。

StringBuilder／join_parts，以及异常记录生命周期、calloc、全局锁、LSDA 与 libunwind 查询优化按约定后置。原始日志和逐次数据保存在仓库外 `scoop-performance-results/2026-10-26/runtime-responsibilities`。

## 第一批验证

Darwin ARM64：release Rust Clippy、C 警告检查、Python Ruff 通过；4 项相关 HIR／MIR／LIR 单测、5 项真实 core 标量行为、String copy 的 debug／release moving GC 两变体，以及重建／重定位 core 的 8 次编译、链接与运行通过。产物测试同步修正旧 helper import 和 String 定义剪切边界，私有 hash helper 随实际定义一起移动。准备出的 91 个 runtime 对象均不再定义这 13 个符号。

同参数 `-O2` 编译的 `rt.c` 加 Char 翻译单元，`__text` 从 2,524 B 降到 1,040 B，减少 1,484 B；含只读数据和 unwind 的各 section 合计减少 2,122 B。没有热路径改动，不声称运行加速。x86_64 的最终 ABI／运行验证随后续批次一起执行。

## 第二批：字符串算法与表示操作

`string_scans.scoop` 扫描前导字节实现字符计数、定位及切片边界；计数按至多 64 B 分块，在块内使用 UByte 累加后转 Long，保留 managed poll，避免每个字节先扩为 64 位。`utf8_encoding.scoop` 共享 Char 宽度与编码；`utf8_validation.scoop` 共享至多四字节的普通 NoGC 校验 helper，保留首错偏移、maximal subpart 和全部 Unicode 标量。fromChars／lossy 两遍确定精确长度，一次分配结果，无临时托管输出数组。原 List 快照、getter 顺序和作用域借用保持。

三个内部 typed 操作分别表达已知长度的零字节 String 分配，以及已建立范围的字节 load／store。分配复用 StringCopy／TLAB／慢路径，写入限定为尚未发布的私有构造对象；不在 codegen 复制 UTF-8 算法，跨 GC 使用更新后的对象。普通 extern 的协议不变。现有 poll 分块接纳有界 String 访问，复制 call destination 时同步重命名临时值；专测固定此前长循环中的错误复制。

退役 `string_length`、`string_get`／`get_storage`、`string_slice_bounds`／`slice_bounds_storage`、`string_hash`、`string_from_utf8_unchecked`、旧非 leaf `string_byte_length`／`byte_at`／`character_at_byte`、`string_slice_bytes`／`from_bytes`／`from_chars`、`string_decode_utf8`／`decode_utf8_storage`／`decode_utf8_lossy`（均为 `scoop_rt_` 前缀），以及旧 `scoop_utf8_*` 编码／扫描导出。删除专属源文件、头、ARM64／x86_64 字符串 shim 和仅验证旧 C 路径的测试。保留三项真实有界 leaf，供 invoke、受检读取失败及单步标量解码使用；单步解码的少量位运算作为同一 C 文件的私有 helper 保留。长比较继续复用 memcmp；concat、box、Context 的真实后备由下一批逐项收敛。

三目标均通过 14 项 core 字符串行为测试（含 1,112,064 个有效 Unicode 标量往返）、debug／release 复制与读取、moving／minor GC，以及输入紧邻不可读页的严格解码验证。Darwin 另通过重建／重定位 core 的 8 次产物消费；两机的 native 表示回归及 x86_64 GNU／musl 静态／动态 anchor ABI 回归通过。本机 10 项 poll 分块回归、String intrinsic 签名与分配后源重读 IR 回归通过。这里的应用 profile 组合消费预编译 release core；完整 debug provider 在后续统一收尾中验证。Linux native 回归首次因隔离源码目录缺少 Ryu 输入失败，补齐原 vendor 输入后通过，未改变生产实现。

固定源码，正常 release，预热一次后五对 AB／BA，单位 ns／次；Linux 固定 CPU 4：

| 路径 | ARM64 Darwin | x86_64 GNU | x86_64 musl |
| --- | ---: | ---: | ---: |
| 1024 B ASCII charLength | 135.84 → 56.82 | 200.68 → 50.01 | 283.64 → 79.92 |
| 160 B Unicode charLength | 32.43 → 19.30 | 40.51 → 24.74 | 68.30 → 34.67 |
| 动态字符定位 | 107.54 → 43.25 | 91.32 → 43.92 | 136.67 → 55.46 |
| 字符切片 | 122.10 → 55.32 | 101.75 → 57.27 | 188.53 → 123.73 |
| fromChars，64 项 | 385.75 → 405.43 | 700.46 → 654.41 | 669.81 → 620.19 |
| strict UTF-8，160 B | 537.01 → 435.74 | 685.28 → 557.64 | 708.06 → 588.71 |
| lossy UTF-8，160 B | 1051.20 → 781.63 | 1120.69 → 921.20 | 4858.36 → 864.47 |

lossy 输入约每七个字节包含一个非法字节，其余为 ASCII；结果包含减少逐段 helper／小 memcpy 调用等组合收益，不能外推为完整应用收益。33～128 B hash 分别降低约 24%～42%、21%～50%、30%～57%；4096 B 降低约 2%～3%，8／32 B 原有路径基本保持。ARM64 fromChars 的 5.1% 回退保留；职责迁移不设额外开关。

七项负载各目标的分配字节、minor／full GC 次数均与基线一致。ARM64 机器码出现 16 B 向量读取和字节归约，无扫描 C 调用；短标量解码继续使用真实 leaf。两个程序的 text 分别减少 ARM64 192／916 B、GNU 1,600／2,432 B、musl 2,048／2,880 B；完整文件分别增加 10,704／8,336 B、21,160／14,216 B、21,240／10,200 B。单次应用构建墙钟变化为 −1.0%～+3.6%（约 −0.03～+0.15 s），CPU 成本同量级；未给清理工作新增运行配置或测量代码。

## 第三批：浮点适配

`floating_text.scoop` 以明确的 GCLeaf C 声明调用现有 Ryu buffered_n，普通有界 NoGC helper 处理 Scoop 的小数／科学计数法格式，再由 typed StringCopy 构造一次 String。输入与输出均为 GC-free 栈缓冲，不持有移动对象的内部地址。删除 `scoop_rt_float_to_string`／`scoop_rt_double_to_string`、`floating.c`、声明和构建项；Ryu 的数字核心及来源版本不变。

JSON／CLI 继续通过实际的 Scoop ABI Option 适配调用 libc `strtof_l`／`strtod_l`；保留完整消费、有限值、次正规数、带符号零、舍入和 C locale。小于 256 B 的输入使用 256 B 栈缓冲补 NUL，其他长度使用原动态缓冲，不增加合法输入限制。短输入每次省去适配层的一次 malloc／free，不能据此声称 libc 内部完全不分配。长度接口候选 fast_float v8.3.2 的原生内核约 10～11 ns，现有 libc 的栈缓冲约 18 ns，旧堆缓冲约 32 ns；两个候选 wrapper 的代码与常量约 44 KiB，并需补充 C++／musl 构建路径。本批采用栈缓冲，未引入新依赖或配置。

ARM64 Darwin、x86_64 GNU／musl 的 core 文本规则及极值、JSON 的 2,048 组 Float／Double 随机位型往返、CLI 数值解码和 254／255／256／257／1024 B 边界通过；debug／release 的 Scoop ABI、moving／minor GC 组合通过。机器码直接调用 Ryu，旧两个 C 包装符号不在产物中。

固定浮点源码、一百万次、预热一次后五对 AB／BA，ARM64 的 Double／Float toString 为 68.51→36.51 ns、62.84→28.82 ns；Double／Float parse 为 50.53→33.61 ns、49.14→33.20 ns。每项的托管分配及 GC 完全一致：两个文本负载分别 35,000,336 B／33 次 minor、34,000,336 B／32 次 minor，两个解析负载仅固定 352／344 B、无 collection。与本轮原始基线比较，完整文件 3,005,000→3,023,848 B，text 321,104→320,604 B；静态变化含前一字符串批次，不能全部归因于浮点迁移。本机候选应用构建墙钟 3.12 s、CPU 4.79 s，原基线未保存可比编译计时。普通职责迁移所有 profile 生效，release 复用既有内联与分配优化。

Linux 空闲时固定 CPU 4，以相同源码及预编译二进制补齐五对 AB／BA。GNU 的 Double／Float toString 为 68.01→37.42 ns、60.95→28.71 ns，Double／Float parse 为 62.97→58.53 ns、59.10→56.58 ns；musl 分别为 194.14→143.24 ns、179.55→126.37 ns、210.66→160.80 ns、234.76→180.55 ns。四项分配字节及 minor／full 次数与上述 ARM64 数值相同，前后保持。GNU／musl 的 text 分别减少 2,304／2,816 B，完整文件增加 10,472／6,456 B，同样包含字符串批次的累计影响；Linux 候选构建单次墙钟 7.78／7.74 s，受到并行编译环境影响，不据此宣称编译加速。


## 第四批：生成操作与实际后备

所有 profile 的 BoxValue 直接消费既有实例布局，复用 managed allocation；分配后从已更新的源根复制 payload，随后执行必要屏障。实例不超过 128 B 时使用 LLVM memcpy，较大实例调用共享 NoGC memcpy，避免冷装箱适配器反复展开宽复制。debug、大对象、ZST、引用和 padding 使用同一合同；动态拆箱检查保留。退役 box_zst／box_value、Array clone、Context fork／restore／current／enter／leave 和旧 write／println C 包装及编译器符号目录，原生 snapshot、ensure_root、try_get、push 保留给实际 callback／线程／绑定树消费者。runtime contract 21，metadata 9。

Context current／enter／leave 使用 TaskContextLoad／TaskContextStore，snapshot 和 restore 复用普通字段读取／写入与屏障。无新增对象字段、managed 根或访问器 safepoint。宽引用装箱回归还固定了晚期 aggregate 分解中已有的支配错误：嵌套 GC-free projection 必须插在选中分量的定义之后；旧实现会产生先使用后定义的 LLVM SSA，ARM64 debug 的宽 payload 读到错误值。最小 IR 在修改前 verifier 失败，修复后与真实 moving／minor GC 一起通过。

ARM64 Darwin、x86_64 GNU／musl 均通过装箱、接口 SSA、跨 profile 小值、native ABI、Context moving／minor、协程出口、callback、普通 native 链接及重建 core 十组定向组合。最终共享复制另跑两 profile 的宽引用装箱；两机格式化、release Clippy、相关 IR／native 测试通过。真实普通 extern 仍按其声明调用，生成的 memcpy 不产生 native transition 或额外 statepoint。没有全量验收。

相对 `0921d90a2`，固定源码和各自配套 runtime，ARM64 一次预热、五对 AB／BA：144 B 实例装箱 23.54→13.24 ns（−43.77%），Context scope 47.68→44.96 ns（−5.71%）；ZST、Long、引用值和 class 对照分别为 +1.46%、+3.23%、−0.06%、+0.01%，未将这些小差异当作稳定收益。全部三目标五种装箱和 Context 的分配字节、minor／full 次数逐项相同。全展开候选虽然大 box 为 12.29 ns，却增加约 51 KiB text；最终采用共享复制，无新配置。

最终两个程序的 text 分别减少 ARM64 1,120／1,012 B、GNU 2,304／2,112 B、musl 2,368／2,240 B；文件分别减少 880／896 B、8,928／4,832 B、4,832／736 B。ARM64 构建单次墙钟为 2.78／2.28 s，基线 2.77／2.23 s；Linux 候选构建 3.91～4.58 s，受到其他编译任务影响，运行计时延后，不从 ARM64 外推 Linux 收益。删除失效 C 入口只计职责与静态收敛，不能把它单独算作运行收益。

compiler／caller 根帧生成发布仍待下一批；当前先按新增任务推进编译速度与最终符号体积。StringBuilder／join_parts 和原定 EH 生命周期等事项继续后置。

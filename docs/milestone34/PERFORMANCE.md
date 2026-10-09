# M34 性能记录

## M33 基线

2026-10-09，revision `e97b40f5c9c408e2ff785e5d09c6bf9222c78979`。使用原有 `tests/benchmarks/measure.py`，Scoop release、默认 nursery、开启 GC 诊断，各程序执行五次，无单独 warmup。编译／缓存构建与计时串行；表内为包含进程启动的运行中位数，不能解释成单个操作延迟。两台主机分别作后续同机对照，不跨宿主计算加速比。

- Darwin：Apple M3 Ultra，512 GiB RAM，macOS 27.0.1，LLVM／Clang 22.1.8；[完整环境](measurements/environment-darwin.json)、[五次样本与 GC 统计](measurements/m33-darwin-release.json)。
- Linux GNU：NUC12／i7-12700H，LLVM 22.1.2；C 工具及系统信息见[完整环境](measurements/environment-linux-gnu.json)，[五次样本与 GC 统计](measurements/m33-linux-gnu-release.json)。

| 程序 | Darwin 中位数 ms | Linux GNU 中位数 ms | 预期输出 |
| --- | ---: | ---: | ---: |
| aggregate | 36.061 | 32.657 | 2000000 |
| allocation | 13.851 | 14.275 | 1599960000 |
| arrays | 24.448 | 26.408 | 393216 |
| collections | 30.663 | 25.744 | 312600 |
| old-graph-large | 45.554 | 49.143 | 199680 |
| old-graph | 23.673 | 32.262 | 199680 |
| primitive | 126.672 | 130.914 | 79999971 |
| survivors | 63.780 | 72.159 | 196614 |

原始报告保存每个样本、最小／最大值、冷／热构建时间、可执行文件与 slib 大小，以及分配／晋升／复制字节、minor／full 次数、root／旧代槽扫描、GC 停顿与 histogram。此处尚未测量当前／峰值 RSS、各个 GC 子阶段、内联开关或多 worker；这些对照随对应批次补充，不能由本表推断。

基线的源码、runtime 与配套工具保存在两地 `tmp/m34/baseline-source`、`tmp/m34/baseline-tools`，后续专门用例可用同一基线重新构建。Darwin 工作目录为 `tmp/m34/baseline-darwin-release-llvm22`，Linux 为 `tmp/m34/baseline-linux-gnu-release`。复跑命令：

```sh
TMPDIR="$PWD/tmp" python3 tests/benchmarks/measure.py \
  --tools tmp/m34/baseline-tools --sysroot tmp/m34/baseline-source/sysroot \
  --runtime tmp/m34/baseline-source/runtime --work tmp/m34/baseline-repeat \
  --revision e97b40f5c9c408e2ff785e5d09c6bf9222c78979 \
  --profile release --gc-stats --runs 5
```

Linux 追加 `--build-arg=--target --build-arg=x86_64-unknown-linux-gnu` 与实际 `--unwind-prefix` 参数，完整 argv 已保存在报告。Darwin 必须显式选用 `/opt/homebrew/opt/llvm@22/bin`；首次误用 LLVM 23 的运行被项目版本检查拒绝，没有计入样本。

## M34-3：MIR 内联

使用 [`mir-inlining.scoop`](../../tests/benchmarks/mir-inlining.scoop)，两千万次 xorshift 与常量 false 检查分支组合，输出均为 `4062365736`。两套工具均为 release 并启用去虚拟化；关闭内联的工具来自 `0fbd84911`（M34-3a），开启组为本批实现。kernel 和 helper 都是 NoGC，避免尚未轻量化的完整 poll 主导结果。raw sysroot、runtime、输入、目标及环境保持相同，工具各用独立 core cache；每组七次串行运行，无单独 warmup。

| 主机 | 关闭内联 ms | 开启内联 ms | 中位数比值（关／开） | kernel 机器码 bytes（关 → 开） | kernel 栈使用 bytes（关 → 开） |
| --- | ---: | ---: | ---: | ---: | ---: |
| Darwin／M3 Ultra | 47.308 | 44.077 | 1.073 | 92 → 76 | 48 → 16 |
| Linux GNU／NUC12 | 36.306 | 35.481 | 1.023 | 73 → 70 | 40 → 8 |

栈使用从函数入口到 prologue 后的 SP 差值读取，GNU 不含 caller 的返回地址。两目标的循环内均由两次 helper 调用变为直接标量指令；开启组没有循环内的调用或 spill。Darwin 少保存四个 callee-saved 寄存器，GNU 少保存三个；保留目标要求的 frame pointer。原始机器码：[Darwin 关闭](measurements/m34-inline-off-darwin.asm)、[开启](measurements/m34-inline-on-darwin.asm)，[GNU 关闭](measurements/m34-inline-off-linux-gnu.asm)、[开启](measurements/m34-inline-on-linux-gnu.asm)。

| 主机／配置 | 冷构建 s | 热构建 s | 可执行文件 bytes | 根 slib bytes |
| --- | ---: | ---: | ---: | ---: |
| Darwin／关闭 | 52.960 | 3.121 | 6,038,920 | 241,536 |
| Darwin／开启 | 47.074 | 3.028 | 5,983,848 | 245,398 |
| GNU／关闭 | 86.317 | 3.215 | 4,512,504 | 260,636 |
| GNU／开启 | 84.469 | 3.164 | 4,461,632 | 263,966 |

冷构建含完整 core、runtime 和链接，热构建经过普通缓存路径；每项只观测一次，不将该差异解释为内联 pass 自身的编译耗时收益。可执行文件包含同样按对应配置优化的 core；根 slib 增长还包括内联局部身份目录。两组分配均为 112 bytes，没有 minor／full collection；因此本组不评估 GC 吞吐或停顿。

全部样本与构建记录：[Darwin 关闭](measurements/m34-inline-off-darwin.json)、[开启](measurements/m34-inline-on-darwin.json)，[GNU 关闭](measurements/m34-inline-off-linux-gnu.json)、[开启](measurements/m34-inline-on-linux-gnu.json)。Darwin 两组第一次启动分别为 550.45／527.61 ms，明显高于后六次，原始样本完整保留；表格使用七次中位数。GNU 的收益较小，且两组样本范围重叠，本次结果只支持该工作负载的有限改进，不代表普遍加速。

实际成本模型按正文求和：direct call 6、动态调用 10、分配 12、Context／原子操作 6、aggregate 构造 4、字段／数组／指针读取 2、其他标量操作 1，local／literal 0；赋值和参数副本另计 1 及值复制成本，分支 2、异常出口 8。极小阈值为 12；普通阈值为 20，循环加 16、已证明的常量化简加 12、实际 receiver 后续派发机会加 8、小值复制机会加 4，最高 48。caller 累计额度为原成本的一半加 96、最多 384，每次展开至少消耗 1；嵌套链最多 6。

这些参数结合正式决策 fixture 保持初值：冷／热调用分别保留／展开，常量化简后较大的 helper 可以展开；连续 64 次小调用仍保留 26 次，递归 SCC 不展开。当前证据包含正确性、两个 target 的代码大小和 spill 观察及一组运行对照；未声称得到通用最优阈值。后续 M34 总验收继续观察组合程序。

## M34-4a：条件 poll

使用原有 [`primitive.scoop`](../../tests/benchmarks/primitive.scoop)，一千万次 managed 整数循环，输出 `79999971`。对照组为 `444d2bd52`（M34-3b）的完整 runtime poll；两组均启用相同的 MIR 优化和 LLVM release 优化。开启组仅在 poll 慢分支调用 runtime；稳定的线程状态指针在函数入口读取一次，每个 poll 保留四次 acquire 状态读取。每组七次串行运行，单独的 core cache，无单独 warmup；运行时间包含进程启动。

| 主机 | 完整 poll ms | 条件 poll ms | 中位数比值（完整／条件） | main 机器码 bytes | main 栈使用 bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Darwin／M3 Ultra | 134.987 | 17.203 | 7.847 | 108 → 236 | 48 → 80 |
| Linux GNU／NUC12 | 123.986 | 18.756 | 6.611 | 116 → 228 | 40 → 56 |

两目标的循环正常路径没有 call 或 spill；slow 分支仍保留 safepoint call。Darwin 入口执行一次 TLV 解析，循环保留四条 `ldar`；GNU 入口解析 TLS，循环用符合 x86 acquire 合同的普通 `mov` 读取四个原子位置。两组都有正常 ABI 的 callee-saved 保存；条件 poll 增加了活跃地址寄存器和代码，不能把该优化描述成零栈开销。原始机器码：[Darwin 完整](measurements/m34-poll-off-darwin.asm)、[条件](measurements/m34-poll-on-darwin.asm)，[GNU 完整](measurements/m34-poll-off-linux-gnu.asm)、[条件](measurements/m34-poll-on-linux-gnu.asm)。

| 主机／配置 | 冷构建 s | 热构建 s | 可执行文件 bytes | 根 slib bytes |
| --- | ---: | ---: | ---: | ---: |
| Darwin／完整 | 49.637 | 4.852 | 5,978,280 | 204,946 |
| Darwin／条件 | 48.536 | 4.958 | 6,028,120 | 208,620 |
| GNU／完整 | 86.277 | 5.801 | 4,456,992 | 219,336 |
| GNU／条件 | 87.896 | 6.052 | 4,510,280 | 223,318 |

全部样本：[Darwin 完整](measurements/m34-poll-off-darwin.json)、[条件](measurements/m34-poll-on-darwin.json)，[GNU 完整](measurements/m34-poll-off-linux-gnu.json)、[条件](measurements/m34-poll-on-linux-gnu.json)。Darwin 首次启动分别为 603.59／501.37 ms，完整保留在七次样本中；其余样本范围分别为 132.17～136.00／16.41～17.41 ms。GNU 完整组为 122.94～128.58 ms，条件组为 13.30～20.29 ms，后者有明显波动，表内不选择最快样本。每组构建只测量一次，不解释为稳定的编译速度变化。

两组均分配 104 bytes，未发生 minor／full collection，因此本对照只衡量无请求 poll 的成本。慢路径停顿、移动根和 pending 激活由正式 fixture 与 runtime 线程回归验证，不从本程序的零 GC 结果推断。最初保留每轮 TLV 解析的 Darwin 实现在同样七次测量中为 22.30 ms；根据机器码改为仅复用线程地址后，重新执行结构测试和三个 target 的组合验收，再得到上表的最终结果。

## M34-4b：线程累计分配计数

对照为 `56343447b`（M34-4a），两组都使用条件 poll。单 mutator 使用原有 `allocation.scoop`，分配 40,000 个 Small 和 40,000 个 Medium；四 mutator 使用新 [`parallel-allocation/program.scoop`](../../tests/benchmarks/parallel-allocation/program.scoop)，每线程分配 200,000 个 Item。C companion 只启动和 join pthread，循环与分配均来自真实 Scoop 生成代码。两类工作负载各自比较旧／新实现，不跨负载相除。

使用同一宿主的普通编译缓存复用未变的 core；两组独立指定 runtime 源码并保存实际构建 argv。每个组合七次串行测量，无单独 warmup。诊断开关只改变退出时的 GC 报告，计数始终维护。复跑入口与构建说明见 [README](../../tests/benchmarks/parallel-allocation/README.md)。

| 主机 | mutator | GC 诊断 | 全局计数 ms | 线程计数 ms | 中位数比值（全局／线程） |
| --- | ---: | --- | ---: | ---: | ---: |
| Darwin | 1 | 关 | 10.734 | 11.067 | 0.970 |
| Darwin | 1 | 开 | 10.571 | 10.750 | 0.983 |
| Darwin | 4 | 关 | 93.708 | 21.138 | 4.433 |
| Darwin | 4 | 开 | 89.255 | 20.396 | 4.376 |
| GNU | 1 | 关 | 12.725 | 12.201 | 1.043 |
| GNU | 1 | 开 | 14.071 | 12.045 | 1.168 |
| GNU | 4 | 关 | 91.587 | 28.468 | 3.217 |
| GNU | 4 | 开 | 91.381 | 27.167 | 3.364 |

开启诊断的每次单线程运行均分配 4,160,112 bytes、执行 3 次 minor；四线程均分配 19,200,224 bytes、执行 18～19 次 minor。两组均无 full collection，stdout 分别为 `1599960000` 和 `2000000`。完整 JSON 保留分配、晋升／复制、根扫描、暂停直方图及所有样本，验证没有因线程退出而丢失累计字节。

机器码中，旧 Darwin nursery 完成路径有四条对共享 heap state 的 `ldadd`，GNU 有四次带 `lock` 前缀的 RMW；新路径只对当前线程字段进行普通 load／store，以实现 relaxed atomic 合同。对象起点 bitmap 的必要原子发布仍保留。新的 TLAB helper 需要取得当前线程，线程状态也保留独立对齐的计数区域；Darwin 单线程中位数稍慢，两组样本范围重叠。GNU 单线程波动亦较大，不将其小幅比值解释为稳定的普遍收益。四线程样本则支持消除共享计数争用的效果。

原始样本：[Darwin 全局](measurements/m34-counters-off-darwin.json)、[线程](measurements/m34-counters-on-darwin.json)，[GNU 全局](measurements/m34-counters-off-linux-gnu.json)、[线程](measurements/m34-counters-on-linux-gnu.json)。首次启动的 Darwin 高值全部保留，未删去离群样本。对应机器码：[Darwin 全局](measurements/m34-counters-off-darwin.asm)、[线程](measurements/m34-counters-on-darwin.asm)，[GNU 全局](measurements/m34-counters-off-linux-gnu.asm)、[线程](measurements/m34-counters-on-linux-gnu.asm)。实际构建记录保存在同名前缀的 `-builds.json`，这批共享缓存测量不作为冷构建速度对照。

## M34-5a：小值参数与结果

对照为 `430a69c71`（M34-4b），两组均保留条件 poll、线程分配统计和相同的优化配置。[用例与复跑说明](../../tests/benchmarks/small-values/README.md) 提供两个工作负载：普通跨 Cone 源码调用，以及独立 LLVM object 中的 Scoop native 调用。每组循环一千万次，每次递增 16-byte Pair 的第一个字段，输出 `10000010`。callee 均不会被 consumer 的 MIR／LLVM 内联删除；每个组合七次串行测量，无单独 warmup，时间包含进程启动。

| 主机 | 调用边界 | 间接 ABI ms | 小值 ABI ms | 中位数比值（间接／小值） | main 机器码 bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| Darwin | 普通跨 Cone | 58.196 | 17.947 | 3.243 | 264 → 232 |
| GNU | 普通跨 Cone | 65.110 | 15.840 | 4.110 | 231 → 209 |
| Darwin | Scoop native | 187.180 | 226.641 | 0.826 | 308 → 304 |
| GNU | Scoop native | 176.290 | 182.943 | 0.964 | 325 → 291 |

普通跨 Cone 的 caller 不再逐轮构造 byval 参数和隐藏结果槽，Pair 在寄存器中跨循环传递；callee 直接在返回寄存器中递增字段。Darwin main 栈从 144 降至 80 bytes，正常循环没有值复制或 spill，poll 慢路仍按需要保存值。coercion 的临时存储被 LLVM 优化消除；独立 place 的语义由正式取址 fixture 验证。原始样本及机器码：[Darwin 间接](measurements/m34-small-source-off-darwin.json)／[小值](measurements/m34-small-source-on-darwin.json)，[GNU 间接](measurements/m34-small-source-off-linux-gnu.json)／[小值](measurements/m34-small-source-on-linux-gnu.json)；同名 `.asm` 保存 caller 与 provider 代码。

native 组合保留现有 NativeBorrowed 的 roots 发布、进入／退出协议；小值减少了按值存储，但整体没有获得收益。Darwin 除首次启动外，旧组为 186.28～188.03 ms、新组为 202.67～236.79 ms，观察到实际变慢。GNU 新组为 176.12～280.77 ms，范围与旧组重叠且有明显高值；本轮不足以认定稳定的 4% 回退。机器码显示 ABI 值已直接传递，仍有 native 协议调用与对应活跃寄存器保存；本次没有进一步把时间差归因于某条指令或缓存因素。这组限制同样保留：[Darwin 间接](measurements/m34-small-native-off-darwin.json)／[小值](measurements/m34-small-native-on-darwin.json)，[GNU 间接](measurements/m34-small-native-off-linux-gnu.json)／[小值](measurements/m34-small-native-on-linux-gnu.json)，同名 `.asm` 给出完整调用路径。

所有运行均分配 104 bytes，minor／full 次数均为零；本对照不推断 GC 吞吐。Darwin 首次启动的 360～719 ms 高值全部保留在七次样本中。普通跨 Cone 的两组复用各自 core cache：provider 构建分别为 Darwin 2.325／2.549 s、GNU 3.177／3.139 s，consumer 首次构建为 4.959／5.361 s、6.495／6.238 s；缓存构建为 5.106／5.129 s、6.212／6.220 s。这些各一次的构建观测不代表稳定的编译速度变化。

普通跨 Cone 的可执行文件大小在 Darwin 两组均为 6,094,552 bytes，在 GNU 均为 4,554,120 bytes；根 slib 分别为 179,678 → 179,698 和 192,582 → 192,478 bytes。native 的可执行文件为 Darwin 6,035,960 → 6,035,960、GNU 4,519,720 → 4,519,736 bytes。Darwin 初次 native 探针缺少 deployment 导致 Mach-O 装载信息检查失败，修正 LLVM triple 后复用已经构建的 core；该组 JSON 的 `first_build_cache_reused` 标明此事，其首次构建时间不能当作冷构建时间。失败阶段没有产生运行样本。

## M34-5b：C aggregate 直接调用

对照为 `dfc609f83`（M34-5a），[既有 C FFI workload](../../tests/benchmarks/ffi/README.md) 的源码、独立 C native objects、MIR 优化和 production runtime 配置相同。每个线程调用返回 16-byte Pair 的 C 函数 2,000,000 次并核对完整校验和；在 1、4 线程下，各按轮交替 C／NativeSafe／GCLeaf 三条路径，保留七轮全部样本。表中单位为 ns/call，四线程值是墙钟时间除以总调用数，表示聚合吞吐。

| 主机 | 线程 | 协议 | StorageBridge | DirectC | 中位数比值（旧／新） |
| --- | ---: | --- | ---: | ---: | ---: |
| Darwin | 1 | NativeSafe | 16.987 | 16.046 | 1.059 |
| Darwin | 1 | GCLeaf | 1.494 | 1.000 | 1.494 |
| Darwin | 4 | NativeSafe | 4.788 | 4.710 | 1.017 |
| Darwin | 4 | GCLeaf | 0.418 | 0.282 | 1.482 |
| GNU | 1 | NativeSafe | 23.171 | 22.424 | 1.033 |
| GNU | 1 | GCLeaf | 2.000 | 1.797 | 1.113 |
| GNU | 4 | NativeSafe | 6.343 | 6.095 | 1.041 |
| GNU | 4 | GCLeaf | 0.636 | 0.462 | 1.377 |

独立 C 循环的前后中位数分别为 Darwin 0.749／0.753（1 线程）、0.210／0.210（4 线程），GNU 0.997／0.995、0.252／0.249。GCLeaf 在 Darwin 和 GNU 四线程的样本有一致改善；GNU 单线程旧组为 1.717～2.502，新组为 1.779～1.822，存在明显重叠，表中的 1.113 不作为稳定的整体加速结论。NativeSafe 各组前后分布也有重叠，保留七次观测及其小幅中位数差异，不将它们解释为已证明的稳定收益。

机器码确认原 C storage bridge 已消除，Pair 直接经目标寄存器传递；GCLeaf 路径不包含 native 状态切换或 caller roots，正常循环保留条件 poll。NativeSafe 继续调用原有 enter／leave 协议，返回寄存器的值按需要跨 leave 保存。两组的运行分配量相同：C 控制路径 80 bytes，Scoop 1／4 线程分别为 104／176 bytes，均为 callback 建立时的一次性分配；minor／full 次数均为零。

全部样本、C／LLVM 版本、完整构建 argv 与记录保存在 [Darwin 旧](measurements/m34-direct-c-off-darwin.json)／[新](measurements/m34-direct-c-on-darwin.json)、[GNU 旧](measurements/m34-direct-c-off-linux-gnu.json)／[新](measurements/m34-direct-c-on-linux-gnu.json)，同名 `.asm` 保存 caller、原 bridge 和独立 C callee。native compiler 为 Apple clang 21.0.0／GCC 15.2.0；Scoop LLVM 为 22.1.8／22.1.2。

构建复用现有 core cache，首次／重复构建分别为 Darwin 旧 8.441／5.238 s、新 4.960／5.313 s，GNU 旧 6.203／6.192 s、新 6.134／6.173 s，均非冷构建。每项只有一次构建观测，不据此推断编译速度。可执行文件为 Darwin 6,153,256 → 6,152,760 bytes、GNU 4,613,280 → 4,612,888 bytes；根 slib 为 934,722 → 931,180、1,017,674 → 1,012,704 bytes。

## M34-6：多 region 与稀疏地址查询

对照为 `6f6950575`（M34-5b），两版使用相同源码、release 优化与 GC 配置。复用 `allocation`、`old-graph-large` 和 `survivors`，新增 [region-stores](../../tests/benchmarks/region-stores.scoop) 在两个保活对象之间执行八百万次单槽写入；它保留条件 poll，循环中不分配。每轮交替旧／新版本，共七轮，计时期间同宿主没有编译、其他测试或 target 清理。表中为整次程序的墙钟中位数，单位 ms。

| 主机 | 工作负载 | 单 arena | 多 region | 中位数比值（旧／新） |
| --- | --- | ---: | ---: | ---: |
| Darwin | allocation | 11.098 | 10.527 | 1.054 |
| Darwin | old-graph-large | 38.019 | 38.425 | 0.989 |
| Darwin | survivors | 56.653 | 59.038 | 0.960 |
| Darwin | region-stores | 24.533 | 35.463 | 0.692 |
| GNU | allocation | 9.644 | 10.515 | 0.917 |
| GNU | old-graph-large | 39.650 | 44.386 | 0.893 |
| GNU | survivors | 61.976 | 79.411 | 0.780 |
| GNU | region-stores | 52.707 | 48.947 | 1.077 |

Darwin 单槽写入观测到明确成本：除首轮外旧组为 24.125～24.857 ms，新组为 34.724～35.524 ms。机器码显示四级 acquire 查询及 region 内 card offset 已内联，之后仍是原来的 byte atomic OR；条件 poll 和慢路 roots 未改变。GNU 单槽组的总体区间重叠，7.7% 的中位数差异不作为稳定加速结论。GNU 的旧图和高存活图分别从 38.311～40.749、61.435～64.466 ms 变为 43.171～49.819、76.122～89.842 ms，记录实际回退，不将本批描述为普遍的吞吐优化。完整对象查询也改走同一稀疏索引；后续存活集合复用和并行 mark 继续使用这些 workload 进行独立对照。

四个 workload 的全部旧／新样本具有相同的分配字节、minor/full 次数、复制字节、脏卡数和 trace 对象数。allocation／旧图／高存活图／单槽循环分别分配 4,160,112／9,021,592／6,947,000／200 bytes；对应 minor/full 次数为 3/0、8/2、6/1、0/1。因此性能表没有通过减少真实 GC 工作或改变输入规模取得结果。Darwin 两版每个可执行文件的首轮启动均有高值，最高 652.881 ms，七次样本全部保留；其余小幅变化和 GNU 分配样本的重叠不另作稳定收益声明。

全部构建 argv、记录、每轮 stdout/stderr、GC 指标与产物大小保存在 [Darwin 旧](measurements/m34-regions-off-darwin.json)／[新](measurements/m34-regions-on-darwin.json)、[GNU 旧](measurements/m34-regions-off-linux-gnu.json)／[新](measurements/m34-regions-on-linux-gnu.json)。同名 `.asm` 保存单槽循环、TLAB 完成入口、范围屏障，以及新版本的 runtime 地址查询。环境沿用本节之前的两台主机与 LLVM 22；runtime C compiler 为 Apple clang 21.0.0／GCC 15.2.0。

所有构建复用 core cache，JSON 明确记为 first_build_seconds 和 warm_build_seconds，不能当作冷构建。Darwin 首次为 5.026～6.402 s、重复为 4.920～5.274 s，GNU 分别为 5.970～6.236 s、5.963～6.357 s；每项只观测一次，不推断编译速度变化。可执行文件增量为 Darwin 304～16,800 bytes、GNU 4,880～8,984 bytes，具体值与根 slib 大小保留在 JSON。

新统计的 mapped_bytes 是当前 managed 映射的虚拟字节数，区别于活跃 block 字节和 RSS。Darwin 的 allocation／单槽程序保留一个 16 MiB region，旧图与高存活图还保留其独立数组 mapping；本批未声称 ordinary region 已向 OS 归还。超过 1 GiB 的真实大对象、三 region 存活图及大对象死亡后的 unmap 由功能测试验证，不以性能数字替代容量与回收正确性。

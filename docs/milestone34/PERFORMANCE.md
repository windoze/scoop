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

## M34-7：region 集中与物理内存归还

对照为 `3cc277490`（M34-6），沿用相同 release 编译和 GC 配置。复用 allocation／old-graph-large／survivors，新增 [region-churn](../../tests/benchmarks/regions/region-churn.scoop)：两轮各建立 180 万个 32-byte Node，再缩为 64 个根，最后退出持有这些根的 scope。相同 binary 的 region-pins 变体在稀疏阶段 pin 首尾两个对象，退出前 unpin；两种变体输出均为 `113400000`。配套 [C 观测入口](../../tests/benchmarks/regions/region-memory.c) 在每轮的完整图、稀疏图和 scope 退出后记录 region／映射和 OS RSS，共六个阶段。

每个工作负载七轮，每轮先旧版再新版；同宿主计时期间没有编译、其他测试或 target 清理。下表为最终实现的墙钟中位数，单位 ms。两版输出、分配字节、minor／full 次数和 trace 对象数均逐样本核对一致。

| 主机 | 工作负载 | M34-6 | M34-7 | 中位数比值（旧／新） |
| --- | --- | ---: | ---: | ---: |
| Darwin | allocation | 10.886 | 10.567 | 1.030 |
| Darwin | old-graph-large | 37.259 | 37.140 | 1.003 |
| Darwin | survivors | 58.018 | 57.948 | 1.001 |
| Darwin | region-churn | 1366.253 | 1374.392 | 0.994 |
| Darwin | region-pins | 1360.579 | 1370.158 | 0.993 |
| GNU | allocation | 9.035 | 11.848 | 0.763 |
| GNU | old-graph-large | 47.154 | 44.862 | 1.051 |
| GNU | survivors | 77.587 | 76.824 | 1.010 |
| GNU | region-churn | 1750.545 | 1746.406 | 1.002 |
| GNU | region-pins | 1747.087 | 1749.428 | 0.999 |

本批主要收益是收缩后的映射和驻留量，表中小幅吞吐差异的样本范围均有重叠，不解释为稳定加速。GNU allocation 的墙钟范围为旧 7.846～12.101、新 9.304～12.121 ms，GC 停顿总和中位数为 0.953 → 1.306 ms，保留这一剩余成本。补充的同 CPU 0 七轮诊断为 0.892 → 0.946 ms，仍有约 6% 差异；该诊断不替换未绑核的主测量，也不足以把全部差异归因于调度，原始数据见 [固定 CPU 样本](measurements/m34-reclamation-pinned-linux.json)。

初版分拆清扫后，机器码出现每个死亡小对象一次的 `retire_small_object` 调用，Darwin／GNU 的 allocation GC 停顿中位数分别从 0.566／1.002 增至 0.821／1.525 ms。将热 helper 保持内联后，重新构建并执行整组测量；Darwin 最终为 0.579 → 0.582 ms。修复前全部样本和机器码保留为 `m34-reclamation-pre-inline-{off,on}-{darwin,linux-gnu}.{json,asm}`，不能与最终样本混合取中位数。最终机器码确认额外 helper 调用已消失，source 筛选、旧代空洞、页建议和 region unmap 路径均在同名汇编中可查。

| 主机／变体 | 稀疏阶段 region 数 | 稀疏阶段映射 MiB | 第一轮稀疏阶段当前 RSS MiB | 第二轮 scope 退出后当前 RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| Darwin／churn | 4 → 2 | 64 → 32 | 96.188 → 65.391 | 96.188 → 65.484 |
| Darwin／pins | 4 → 3 | 64 → 48 | 96.188 → 81.391 | 97.641 → 65.453 |
| GNU／churn | 4 → 2 | 64 → 32 | 95.188 → 40.000 | 95.188 → 39.992 |
| GNU／pins | 4 → 3 | 64 → 48 | 95.191 → 40.059 | 95.191 → 39.977 |

完整图阶段两版均为 4 个 ordinary region 和 77.75 MiB 总映射。pin 阶段保留额外一个区域，unpin／scope 退出后新版回到 2 个 ordinary region、32 MiB 映射；此时程序仍有其他存活对象，不能称为整个堆为空。无 core 的 C 回归单独确认全空时仅保留一个 region。Darwin churn 峰值 RSS 为 109.922 → 111.219 MiB，GNU 为 108.609 → 108.711 MiB，未降低峰值。GNU current RSS 来自 statm，peak 来自 getrusage，两种 OS 统计口径可能有小幅差异。

churn／pins 的两轮累计 unmap 均为 95,944,704 bytes，含 64 MiB ordinary region 和 27.5 MiB large mapping。成功 discard 建议量分别为 Darwin 50,741,248／84,885,504 bytes、GNU 51,040,256／85,651,456 bytes，建议失败均为零；这些是累计建议字节，不能视为同时新增的物理内存归还量。Darwin 的 `MADV_DONTNEED` 与 GNU 的 RSS 变化不同，表中保留实际观测。旧版缺少这些统计字段，因此标记未知，不能填零；旧版已经能 unmap 死亡 large mapping。

allocation／旧图／高存活图／两种增长图分别分配 4,160,112／9,021,592／6,947,000／144,001,344 bytes，minor/full 为 3/0、8/2、6/1、104/12，trace 对象为 1／196,363／408,832／11,610,207。复制字节则因普通 full 的选择性策略改变：allocation 两版均为 24，旧图 2,097,424 → 2,089,136，高存活图 6,365,264 → 6,234,168，churn 108,747,072 → 108,629,552，pins 108,747,072 → 108,628,232。该变化属于实际搬迁策略，没有缩小源程序或减少收集次数。

最终全部 stdout／stderr、GC／六阶段内存数据、构建 argv 和工具版本保存在 [Darwin 旧](measurements/m34-reclamation-off-darwin.json)／[新](measurements/m34-reclamation-on-darwin.json)、[GNU 旧](measurements/m34-reclamation-off-linux-gnu.json)／[新](measurements/m34-reclamation-on-linux-gnu.json)，同名 `.asm` 保存主入口和收集／归还路径。Darwin 新 binary 首次执行仍有高值，allocation／旧图／高存活图／churn 分别达到 502.256／381.513／404.079／2128.514 ms；全部保留，pins 复用已运行的 churn binary。工具沿用 Apple clang 21.0.0／GCC 15.2.0、LLVM 22.1.8／22.1.2。

所有构建复用 core cache，旧版构建记录来自首次对照，新版记录来自内联修复后的重建；均非冷构建。Darwin 首次范围为 5.045～6.247 s、重复为 4.924～5.468 s，GNU 分别为 5.991～6.395 s、5.964～6.259 s，每项只有一次构建观测，不推断编译速度。最终可执行文件增加 Darwin 544～16,944 bytes、GNU 688～4,784 bytes，根 slib 在同 target 两版间逐项相同。

## M34-8：并行 mark 与存活集合

对照为 `66abed0c8`（M34-7）。新增 [mark-graphs](../../tests/benchmarks/parallel/mark-graphs.scoop) 与 [阶段观测入口](../../tests/benchmarks/parallel/mark-phase.c)：wide 为完整二叉树，array 为含共享环节点的大引用数组，chain 为单后继长链。每种图 131071 个主节点，每进程主动 full 五次，输出均为 `8589737985`。每种 worker 数七轮，每轮固定依次执行旧版、新版 1／2／4／8 worker 与当时默认八 worker；同宿主计时不与编译、其他测试或 target 清理并发。每个样本核对完整输出、每轮 live／trace 对象数和 GC histogram，并确认各 worker 首次标记数之和等于存活数。

下表 mark／pause／CPU 为 35 次 explicit full 的中位数，单位 ms；CPU 是该次所有 marker 线程 CPU 之和，不是墙钟。worker 数包含 coordinator。旧版没有分阶段计时，不能把缺失的 mark 值视为零。

| 主机／图 | mark，1 worker | 2 worker | 4 worker | 8 worker | pause，1 → 4 → 8 | CPU，1 → 4 → 8 |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| Darwin／wide | 4.091 | 2.170 | 1.244 | 1.380 | 9.155 → 6.327 → 6.480 | 4.090 → 4.865 → 9.582 |
| Darwin／array | 5.635 | 2.970 | 1.705 | 1.617 | 13.938 → 10.045 → 10.053 | 5.634 → 6.680 → 11.613 |
| Darwin／chain | 3.950 | 3.948 | 3.990 | 4.011 | 8.972 → 9.045 → 9.146 | 3.949 → 3.989 → 4.075 |
| GNU／wide | 4.003 | 2.235 | 1.478 | 1.003 | 11.147 → 8.908 → 8.261 | 4.001 → 5.402 → 6.243 |
| GNU／array | 6.657 | 3.466 | 1.956 | 1.474 | 19.771 → 15.032 → 14.381 | 6.655 → 7.395 → 9.632 |
| GNU／chain | 4.157 | 4.313 | 4.317 | 4.452 | 11.540 → 11.720 → 11.996 | 4.155 → 4.243 → 4.527 |

四 worker 相对同实现的一 worker，wide／array 的 mark 加速为 Darwin 3.29／3.31 倍、GNU 2.71／3.40 倍；总 pause 只加速 1.45／1.39、1.25／1.32 倍。四 worker 下 reference update 中位数分别为 Darwin 4.048／7.300 ms、GNU 6.110／11.699 ms，仍是串行阶段。array 每次产生 128 个区间任务；CPU 与对象分布确认宽图和数组由多个线程实际参与。chain 没有可利用的图并行性，多数轮由 coordinator 完成全部标记，不能据此声称多核加速。

初版 16383-node、三次 full 的单轮定位样本曾出现 chain 的八 worker mark 中位数 17.826 ms，而一 worker 为 0.828 ms。原因是每个后继重新排队并广播；改为在一个最多 64 对象的任务内继续扫描尚未发布的后继，仅存在一个后继时不广播，最后完成仍唤醒等待者。同规模定位复测为 0.544／0.596 ms。这些诊断保留在 `m34-mark-pre-batch-*` 和 `m34-mark-batched-*`，它们不是七轮性能结论，未混入上表。

Darwin 八 worker 没有明显改善总 pause，线程 CPU 接近翻倍；GNU 八 worker 的 wide／array 总 pause 比四 worker再缩短约 7%／4%，但 CPU 增加。因此最终默认上限为四，minor 和活跃 block 不足 4 MiB 的 full 仍为一；显式 1..8 保留。上述固定 worker 样本来自默认上限为八时的同一算法，显式设置覆盖默认策略，故无需重跑。默认八的原始结果单独保留为 `m34-mark-default8-*`。

对最终默认四的 binary 另外执行七轮旧／新交替测量，下表墙钟为七个完整进程的中位数，pause 为其中 35 次 explicit full 中位数，单位 ms。长链相对 M34-7 的改善主要来自存活集合复用和标记实现变化，不能归于并行。

| 主机／图 | 墙钟，M34-7 → M34-8 默认 | explicit full pause，旧 → 新 | 新 mark |
| --- | --- | --- | ---: |
| Darwin／wide | 92.695 → 62.054 | 11.782 → 6.529 | 1.262 |
| Darwin／array | 148.634 → 89.945 | 21.938 → 11.002 | 1.778 |
| Darwin／chain | 96.105 → 74.539 | 12.673 → 9.404 | 4.052 |
| GNU／wide | 118.447 → 77.747 | 14.871 → 8.594 | 1.488 |
| GNU／array | 192.419 → 113.460 | 28.671 → 14.470 | 2.192 |
| GNU／chain | 116.809 → 91.605 | 15.124 → 11.596 | 4.312 |

两版 wide／chain 的 managed 映射均为 16 MiB，array 为 17.0625 MiB。末轮当前 RSS 中位数（旧 → 新）为 Darwin wide 16.359 → 17.547 MiB、array 17.359 → 18.547 MiB；GNU 分别为 14.680 → 14.828、16.676 → 15.816 MiB。Darwin wide 峰值从 17.344 增为 18.484 MiB，GNU wide 为 17.852 → 17.852 MiB。存活清单在 update 后释放，线程池／队列保留；这些 RSS 样本不证明 OS 已归还全部临时分配，也不宣称峰值下降。

复用的三个 workload 同样七轮旧／新交替，各样本 stdout、分配字节、minor／full 次数、trace 对象数一致。下表为进程墙钟与整个进程累计 GC pause 的中位数，单位 ms；这些程序默认 minor 单 worker，并不全部触发并行 full。

| 主机／工作负载 | 墙钟，旧 → 默认四 | 累计 GC pause，旧 → 默认四 |
| --- | --- | --- |
| Darwin／allocation | 11.022 → 10.817 | 0.581 → 0.586 |
| Darwin／old-graph-large | 37.951 → 34.590 | 21.738 → 18.012 |
| Darwin／survivors | 58.851 → 44.883 | 44.185 → 29.767 |
| GNU／allocation | 8.548 → 11.800 | 0.941 → 1.106 |
| GNU／old-graph-large | 46.168 → 38.312 | 31.624 → 23.328 |
| GNU／survivors | 76.840 → 56.440 | 63.592 → 43.102 |

GNU allocation 的未绑核结果偏慢；补充 CPU 0 上七轮对照为墙钟 8.489 → 8.243 ms、GC pause 0.945 → 0.722 ms，方向相反。固定 CPU 诊断完整保存于 `m34-mark-regressions-default4-cpu0-*`，不替换主表，也不足以断言稳定收益或稳定回退。Darwin 新 binary 首次 wide 墙钟为 537.185 ms，后续 61.763～67.557 ms；全部样本保留，未删除首次运行高值。

原始固定 worker 报告为 `m34-mark-{off,on-1,on-2,on-4,on-8,default8}-{darwin,linux-gnu}.json`，最终默认对照为 [Darwin 旧](measurements/m34-mark-default4-off-darwin.json)／[新](measurements/m34-mark-default4-on-darwin.json)、[GNU 旧](measurements/m34-mark-default4-off-linux-gnu.json)／[新](measurements/m34-mark-default4-on-linux-gnu.json)；既有 workload 使用 `m34-mark-regressions-*`。报告保留每轮 stderr、分阶段时间、worker CPU／对象数、RSS、完整构建 argv 和产物大小。`m34-mark-{off,on}-{darwin,linux-gnu}.asm` 保存标记、队列、终止、移动计划与存活集合遍历；新机器码包含 AArch64 原子置位／amd64 lock 指令，并直接遍历保留集合进行引用更新。

工具仍为 Apple clang 21.0.0／GCC 15.2.0、LLVM 22.1.8／22.1.2。构建复用 core cache，first／warm 不是冷／热完整重建；旧版记录来自首次基线构建，新版来自默认四重建。最终新版首次／重复构建为 Darwin 5.171～5.685／5.202～5.562 s，GNU 5.941～6.312／5.910～6.247 s，每项仅一次观测。可执行文件增加 Darwin 1,184～17,712 bytes、GNU 每项 9,952 bytes，同 target 的根 slib 大小逐项不变。

## M34-9：紧凑容器存储

对照使用相同的编译器、release 配置和默认 GC，仅切换 ArrayList／StringBuilder 的 core 实现。旧版为 `9176404ba` 的 Option backing；两份 runtime 都包含原生 Option 返回 ABI 修复。每组七轮，逐轮交换旧／新顺序，计时期间没有构建、测试或清理。源码与 phase 定义见 [容器基准](../../tests/benchmarks/containers/README.md)。四份 [Darwin 旧](measurements/m34-containers-off-darwin.json)／[新](measurements/m34-containers-on-darwin.json)、[GNU 旧](measurements/m34-containers-off-linux-gnu.json)／[新](measurements/m34-containers-on-linux-gnu.json) 报告保留全部样本、实际输出、GC、CPU、RSS、构建命令和产物大小；同名 `.asm` 保存六个热点的机器码及 MIR／LIR 符号映射。构建记录是当时缓存状态下的实际耗时，不作为冷构建加速比。

实测 Int backing stride 从 16 降到 4，capacity 262144 的数组分配大小从 4,194,328 降到 1,048,600 bytes；ZST 从 8 降到 0，同容量数组从 2,097,176 降到 24 bytes。引用保持 8、接口保持 16 bytes。这里删除的是容量状态的 Option tag／padding，没有增加容器专用 shape 或改变正常数组初始化。

下表为整个进程墙钟中位数，单位 ms，包含程序启动；各操作的独立计时在原始 phase 数据中。

| 工作负载 | Darwin，旧 → 新 | GNU，旧 → 新 |
| --- | --- | --- |
| Int 262144 项 | 18.929 → 17.632 | 16.906 → 13.456 |
| interface 131072 项 | 33.658 → 34.779 | 37.825 → 37.334 |
| ZST 262144 项 | 20.169 → 19.651 | 17.015 → 15.896 |
| capacity 1024、有效项 1 | 12.548 → 12.358 | 9.451 → 8.767 |
| capacity 1048576、有效项 1 | 65.718 → 64.271 | 44.578 → 42.060 |
| StringBuilder 32768 段 | 21.540 → 22.114 | 19.343 → 18.696 |
| JSON 8192 个 Int | 61.658 → 62.345 | 56.664 → 56.704 |

Int 容量初始化在 Darwin／GNU 分别为 0.582 → 0.430／1.258 → 0.398 ms，读取并修改为 3.985 → 3.302／3.600 → 3.000 ms。clear 仍逐槽执行普通 setter，分别为 0.383 → 0.379／0.212 → 0.212 ms，没有获得同幅度加速；不能把内存缩小比例当作全部操作的吞吐比例。ZST 容量初始化从 0.485 → 0.241／0.753 → 0.120 ms，追加的实际副作用次数不变。

引用和接口原先已使用 niche，因此不节省元素字节。Darwin 的接口追加（包含 Cell 分配）由 18.864 增至 20.196 ms，整个进程约慢 3.3%；GNU 整体略快。StringBuilder 和 JSON 组合没有一致的墙钟收益，Darwin 分别约慢 2.7%／1.1%，保留这些回退样本。JSON 的编码／解码分配字节分别从 6,494,168／12,870,672 降至 6,297,432／12,346,368；收集时点随分配量改变，部分阶段扫描槽数上升，报告未把它解释为更少的 GC 工作。

低占用测试每进程预热一次 full 后再计五次 full。capacity 1024 和 1048576 的实际 `mark_reference_slots` 分别为 1026 和 1048578，两版完全一致。大容量 full pause 的 35 次样本中位数在 Darwin 为 8.133 → 7.918 ms，在 GNU 为 4.012 → 4.023 ms；新实现对应进程 CPU 为 10.673／7.887 ms，mark 为 0.965／1.378 ms。即使只有一个有效元素，全零空闲槽仍有按 capacity 访问的成本，当前实现没有按 size 扫描的优化。

Darwin 的 executable／root `.slib` 为 8,923,112／12,307,088 → 8,939,048／12,393,038 bytes，`__text` 从 437,332 降至 433,992 bytes。GNU 对应为 6,676,112／13,447,850 → 6,682,688／13,534,934，`.text` 从 507,569 降至 504,241。机器码略减而类型／产物描述略增，两者分别记录。

## M34-10：完整变更的同机对照

这里的 off／on 分别是完整 M33／完整 M34，不能把总收益归给某一个优化。M33 为 `e97b40f5c9c408e2ff785e5d09c6bf9222c78979`；最终编译器实现为 `fbd98ad0d`，之后的 fixture／记录提交不改变编译器或 runtime。复用八个原有工作负载，新增跨 Cone 接口和纯计算 GC 响应两套程序，按模式形成 14 组比较。源码、构建和计时入口见 [复跑说明](../../tests/benchmarks/m34/README.md)。

每组分别 warmup 一次，正式运行七轮并交替 M33／M34 顺序，开启相同的 GC 统计，使用默认 nursery／worker 策略。计时不与同宿主的编译、CLI 验收或 target 清理并发。程序输出、接口的独立 UInt checksum、各线程整数和及 GC histogram 总数全部检查；CPU 为进程及线程累计 CPU 时间，墙钟包含进程启动。两机环境沿用本文件开头的记录，未固定 CPU affinity，GNU 的混合核调度存在可见波动。

Linux 样本最初使用 `96323e2b0` 构建。后续 non-integral pointer 与 intrinsic receiver 修复完成后，两机各重新构建全部十个可执行文件，与对应旧候选逐字节相同，热点机器码大小也相同，因此保留并复用 Linux 已完成的七轮；[Darwin 对照](measurements/m34-final-binary-comparison-darwin.json)和[GNU 对照](measurements/m34-final-binary-comparison-linux-gnu.json)同时记录路径、字节数与 SHA-256。原始样本与修复后构建记录分别保存，不把重新计算摘要当作重新计时。

### 原有工作负载

八个程序的输入与 M33 基线一致。表内时间为七次中位数及最小／最大值，单位 ms；不能把整进程时间当作单个操作延迟。全部墙钟、CPU、stdout、GC 及 binary 大小见 [Darwin](measurements/m34-final-standard-darwin.json)／[GNU](measurements/m34-final-standard-linux-gnu.json)原始报告。

| 主机／工作负载 | M33 墙钟 ms，中位数 [范围] | M34 墙钟 ms，中位数 [范围] | CPU ms，旧 → 新 |
| --- | ---: | ---: | ---: |
| Darwin／aggregate | 36.092 [35.404, 37.745] | 10.510 [10.197, 10.840] | 34.003 → 8.529 |
| Darwin／allocation | 14.238 [13.896, 14.628] | 11.853 [11.476, 12.918] | 12.251 → 9.770 |
| Darwin／arrays | 26.581 [25.772, 26.822] | 19.422 [19.254, 19.625] | 24.397 → 17.340 |
| Darwin／collections | 32.005 [30.843, 34.502] | 18.926 [18.679, 19.107] | 29.811 → 16.735 |
| Darwin／old-graph-large | 49.197 [48.752, 50.392] | 36.865 [36.075, 37.719] | 46.744 → 34.568 |
| Darwin／old-graph | 25.763 [25.195, 25.970] | 18.596 [18.085, 20.212] | 23.498 → 16.421 |
| Darwin／primitive | 139.522 [136.218, 175.711] | 17.802 [17.291, 18.637] | 137.043 → 15.650 |
| Darwin／survivors | 68.389 [67.965, 69.672] | 47.563 [47.390, 47.809] | 66.201 → 51.007 |
| GNU／aggregate | 37.463 [32.160, 40.961] | 6.371 [5.197, 8.847] | 37.248 → 6.241 |
| GNU／allocation | 13.786 [12.614, 15.776] | 9.682 [8.578, 11.915] | 13.633 → 9.575 |
| GNU／arrays | 25.222 [24.131, 27.076] | 16.067 [15.224, 18.335] | 25.090 → 15.906 |
| GNU／collections | 26.224 [24.500, 28.687] | 15.631 [13.023, 18.515] | 26.042 → 15.530 |
| GNU／old-graph-large | 49.815 [47.533, 54.614] | 35.964 [35.333, 38.988] | 49.525 → 35.787 |
| GNU／old-graph | 23.677 [23.021, 26.485] | 16.238 [14.009, 21.848] | 23.546 → 16.088 |
| GNU／primitive | 131.647 [124.128, 135.278] | 13.491 [12.686, 15.587] | 131.396 → 13.387 |
| GNU／survivors | 75.656 [70.645, 87.778] | 55.491 [53.410, 62.109] | 74.950 → 62.240 |

原有 aggregate、primitive 的分配仍均为 104 bytes，没有 collection。allocation／arrays／old-graph 的工作量与分配量保持一致；collections 因紧凑容器减少 9,600 bytes。Darwin allocation 的累计 GC pause 中位数为 0.863 → 0.627 ms，survivors 为 44.258 → 31.557 ms；GNU 分别为 2.293 → 0.776 ms、51.658 → 41.980 ms。survivors 新版进程 CPU 为 62.240 ms，超过 55.491 ms 墙钟，反映并行 GC 使用额外核心，不是计时错误。

八个程序的可执行文件大小在 Darwin 变化 −0.098%～+0.239%，GNU 增加 0.559%～0.688%；根 `.slib` 的变化与 executable 分开记录。修复后实际构建：[Darwin](measurements/m34-final-standard-on-darwin-build.json)／[GNU](measurements/m34-final-standard-on-linux-gnu-build.json)。构建期间存在同机验收活动，首次构建也复用了缓存，所存首次／重复构建时间仅作执行记录，不据此宣称稳定编译速度改善；受控内联编译时间对照见 M34-3。

### 接口调用与转换

provider 中的 Cell.next 执行 UInt xorshift，四种模式各循环 2,000,000 次；consumer 独立构建，所有输出均为 `3383936671`。known 使用已知 Cell，unknown 接收 Reader，converted-once 在循环前把 Any 转为 Reader，converted-each 每轮重新转换。完整样本：[Darwin](measurements/m34-final-interfaces-darwin.json)／[GNU](measurements/m34-final-interfaces-linux-gnu.json)。

| 主机／工作负载 | M33 墙钟 ms，中位数 [范围] | M34 墙钟 ms，中位数 [范围] | CPU ms，旧 → 新 |
| --- | ---: | ---: | ---: |
| Darwin／known | 180.841 [172.347, 184.478] | 16.342 [14.995, 17.839] | 178.671 → 14.179 |
| Darwin／unknown | 185.849 [178.025, 191.173] | 15.583 [15.118, 15.623] | 183.622 → 13.483 |
| Darwin／converted-once | 177.242 [171.614, 179.087] | 15.743 [15.219, 18.180] | 174.896 → 13.634 |
| Darwin／converted-each | 236.566 [231.249, 239.289] | 133.816 [132.695, 134.225] | 234.110 → 131.599 |
| GNU／known | 99.964 [98.872, 107.697] | 9.645 [9.000, 13.806] | 99.814 → 9.541 |
| GNU／unknown | 101.299 [99.615, 107.455] | 9.397 [9.121, 13.573] | 101.136 → 9.161 |
| GNU／converted-once | 107.419 [99.357, 111.858] | 13.555 [9.135, 15.395] | 107.161 → 13.449 |
| GNU／converted-each | 130.611 [129.568, 131.831] | 74.457 [74.104, 77.667] | 130.496 → 74.307 |

C companion 只在普通 Scoop 循环外记录时间；去掉进程启动后的 kernel 如下，仍保留 Managed poll 和真实调用。

| 主机／模式 | M33 kernel ms | M34 kernel ms |
| --- | ---: | ---: |
| Darwin／known | 171.850 [163.083, 175.332] | 7.323 [6.469, 8.788] |
| Darwin／unknown | 176.660 [168.941, 182.095] | 6.440 [6.321, 6.546] |
| Darwin／converted-once | 168.586 [162.514, 170.077] | 6.596 [6.163, 8.780] |
| Darwin／converted-each | 227.309 [222.267, 229.926] | 124.755 [123.531, 124.954] |
| GNU／known | 94.479 [93.151, 100.755] | 4.842 [4.716, 7.376] |
| GNU／unknown | 95.697 [94.446, 100.714] | 4.871 [4.709, 7.048] |
| GNU／converted-once | 100.998 [94.121, 104.787] | 7.024 [4.723, 8.627] |
| GNU／converted-each | 126.107 [125.085, 127.054] | 69.886 [69.795, 71.112] |

机器码中，known 已去虚拟化为 Cell.next 直接调用，但没有将 next 整体内联；next 内的 salt getter 已内联。unknown 直接从保存的 itab 取 slot，循环中没有 itable lookup。converted-once 在循环外检查／查表一次；converted-each 保留每轮检查／查表，仍明显慢于前两种模式。旧版 known／unknown 每轮均查表。两目标原始机器码与源码符号映射见 [Darwin M33](measurements/m34-final-interfaces-off-darwin.asm)／[M34](measurements/m34-final-interfaces-on-darwin.asm)、[GNU M33](measurements/m34-final-interfaces-off-linux-gnu.asm)／[M34](measurements/m34-final-interfaces-on-linux-gnu.asm)，同名 JSON 保存完整构建和 section 数据。

GNU converted-once 的波动较大，不能据其一次中位数声称循环前转换使每次接口调用本身更慢。四种模式分配均为 136 bytes，没有 GC，表中收益包含条件 poll、getter 内联和接口表示的共同变化。

### 纯计算循环的 GC 响应

每个工作线程在普通 Managed Scoop 函数中执行 100,000,000 次整数循环；独立 collector 重复请求 full GC，并在请求之间以 NativeSafe 等待 100 μs。比较 1／4 个工作线程，C runner 核对每个线程的完整整数和。报告：[Darwin](measurements/m34-final-poll-darwin.json)／[GNU](measurements/m34-final-poll-linux-gnu.json)。

| 主机／工作负载 | M33 墙钟 ms，中位数 [范围] | M34 墙钟 ms，中位数 [范围] | CPU ms，旧 → 新 |
| --- | ---: | ---: | ---: |
| Darwin／1-thread | 1936.320 [1901.175, 1954.302] | 122.824 [122.258, 125.339] | 1991.572 → 124.786 |
| Darwin／4-threads | 2389.429 [2346.569, 2507.524] | 155.280 [153.333, 156.020] | 8647.807 → 545.658 |
| GNU／1-thread | 2727.635 [2691.932, 2740.281] | 73.858 [66.760, 78.586] | 2756.073 → 74.593 |
| GNU／4-threads | 3526.523 [3507.025, 3555.251] | 92.267 [85.575, 96.935] | 13857.512 → 326.601 |

每个进程内部按实际请求记录停稳等待及 pause 的 p50／p99／max。下表是七个进程各自分位数的中位数和范围，不是把全部请求合并后的分位数；collection 数为七次正式运行之和。

| 主机／线程 | 总 collection，旧 → 新 | stop-wait p50 μs，旧 → 新 | stop-wait p99 μs，旧 → 新 | pause p99 μs，旧 → 新 |
| --- | ---: | ---: | ---: | ---: |
| Darwin／1-thread | 73201 → 5074 | 6.000 [6.000, 6.000] → 6.000 [6.000, 6.000] | 19.000 [19.000, 22.000] → 18.000 [16.000, 21.000] | 78.000 [71.000, 80.000] → 41.000 [37.000, 50.000] |
| Darwin／4-threads | 73035 → 5087 | 29.000 [28.000, 30.000] → 28.000 [26.000, 29.000] | 98.000 [89.000, 113.000] → 84.000 [77.000, 101.000] | 178.000 [169.000, 195.000] → 137.000 [121.000, 140.000] |
| GNU／1-thread | 72417 → 2691 | 0.727 [0.685, 0.903] → 0.900 [0.806, 4.009] | 2.346 [1.047, 2.411] → 6.385 [5.782, 7.457] | 125.045 [117.159, 152.063] → 31.588 [31.169, 38.311] |
| GNU／4-threads | 81194 → 3120 | 8.384 [8.114, 8.492] → 7.765 [7.495, 8.458] | 22.673 [22.022, 23.483] → 26.981 [21.589, 28.704] | 226.111 [180.512, 233.880] → 61.756 [55.542, 67.745] |

循环变快后，collector 在程序结束前能发出的请求数量随之减少。因此累计 pause 与整个程序时间不能解释为相同请求数量或固定 GC 压力下的单独 poll 加速。Darwin 的停稳等待 p99 为 19 → 18 μs、98 → 84 μs，记录精度为整 μs；GNU 则从 2.346 → 6.385 μs、22.673 → 26.981 μs，吞吐提高并不意味着等待尾部也下降；全部极值保留，但两版请求数不同，不把观察到的最大值差异解释为稳定极值保证。

两目标的 compute 循环仍在，正常回边是条件检查，GC 请求走保留根与 relocation 的慢分支。[Darwin M33](measurements/m34-final-poll-off-darwin.asm)／[M34](measurements/m34-final-poll-on-darwin.asm)、[GNU M33](measurements/m34-final-poll-off-linux-gnu.asm)／[M34](measurements/m34-final-poll-on-linux-gnu.asm)记录实际机器码；同名 JSON 保存构建与函数映射。

### 代码大小、内存与结果边界

| 热点 | Darwin bytes，M33 → M34 | GNU bytes，M33 → M34 |
| --- | ---: | ---: |
| Cell.next | 80 → 164 | 71 → 136 |
| salt getter 的独立入口 | 44 → 136 | 34 → 109 |
| known | 180 → 300 | 169 → 279 |
| unknown | 176 → 312 | 169 → 274 |
| convertedOnce | 372 → 540 | 380 → 517 |
| convertedEach | 388 → 544 | 382 → 525 |
| compute | 80 → 200 | 66 → 171 |

条件 poll 增加分支、状态地址与慢路径，热点机器码可以增长；完整 executable 的变化远小于这些局部比例。独立 getter 入口仍存在；next 内已经内联 getter，并不表示链接结果中的独立 getter 正文也消失。

当前与峰值 RSS、mapped／committed／discarded／unmapped、GC 子阶段、pin 和再次增长成本在 M34-7 的阶段实验中记录，1／2／4／8 worker 与 worker CPU 在 M34-8 中记录。M33 普通统计没有新增 RSS／子阶段字段，因此这里不补造跨版本字段。M34-9 的容器对照显示 Int stride 16 → 4、ZST 8 → 0，但引用／接口仍扫描完整 capacity，StringBuilder／JSON 没有一致墙钟收益；这些取舍继续保留。

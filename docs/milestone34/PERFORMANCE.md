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

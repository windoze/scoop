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

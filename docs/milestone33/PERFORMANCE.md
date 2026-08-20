# M33 性能记录

## M33-1a：NativeSafe / NativeBorrowed

2026-10-08 测量。基线为 `670a45477` 的 runtime，修改后为 M33-1a；两者使用同一份 [benchmark](../../runtime/tests/native_transition_benchmark.c) 和独立编译的 [native callee](../../runtime/tests/native_transition_leaf.c)。此处隔离 runtime transition 成本；完整 Scoop FFI、GCLeaf 和 DirectC 的测量在对应功能完成后追加。

- Darwin：Apple M3 Ultra，AArch64，LLVM/Clang 22.1.8，动态系统库。
- Linux GNU：NUC12 / Intel i7-12700H，x86-64，Clang 22.1，动态系统库。
- C11、`-O2 -fno-omit-frame-pointer -fno-optimize-sibling-calls -pthread`；runtime 各 translation unit 使用 `-DNDEBUG`，测试程序保留断言。native callee 单独编译，不使用 LTO、builtin 替换或仅针对直接 C caller 的内联。所有 caller 执行同一空 native 操作。
- 不加 GC 的各组合取三次样本的中位数；直接 C 每线程 2,000,000 次，其余每线程 200,000 次。计时包含线程开始/结束；这些微基准不代表真实业务延迟。
- GC 压力使用真实 collector、空 heap 和合成 stackmap；每线程 10,000,000 次 NativeSafe 调用，每次之后执行真实 poll。两轮 collection 请求之间等待 100 微秒。根移动正确性由受控交错测试及正式 CLI fixture 另行验证。
- collector 的超时复查参数为 **50 微秒**，条件等待期间释放 world lock。测试构建才启用 STOPPING、停稳、恢复前的观测点；production 不含观测回调。

原始样本与环境保存在 [Darwin](measurements/native-transition-darwin.json) 和 [Linux GNU](measurements/native-transition-linux-gnu.json)。

### 无 GC：每次调用的聚合耗时

单位 ns，计算方式为总墙钟时间 / 全部线程的总调用次数。只有单线程列可近似理解为调用延迟；多线程列用于比较聚合吞吐，不能当作每个线程的调用延迟。

| 平台 / 路径 | 版本 | 1 线程 | 2 线程 | 4 线程 | 8 线程 |
| --- | --- | ---: | ---: | ---: | ---: |
| Darwin / 直接 C | 前 | 1.444 | 0.758 | 0.394 | 0.188 |
| Darwin / 直接 C | 后 | 2.088 | 0.707 | 0.363 | 0.180 |
| Darwin / NativeSafe | 前 | 27.610 | 53.650 | 85.080 | 80.168 |
| Darwin / NativeSafe | 后 | 16.000 | 8.245 | 9.410 | 5.951 |
| Darwin / NativeBorrowed | 前 | 24.565 | 61.495 | 86.981 | 79.059 |
| Darwin / NativeBorrowed | 后 | 15.810 | 7.910 | 4.919 | 7.173 |
| GNU / 直接 C | 前 | 1.358 | 1.227 | 0.374 | 0.300 |
| GNU / 直接 C | 后 | 1.173 | 0.598 | 0.308 | 0.252 |
| GNU / NativeSafe | 前 | 43.177 | 132.144 | 160.872 | 186.083 |
| GNU / NativeSafe | 后 | 25.823 | 13.197 | 6.648 | 4.964 |
| GNU / NativeBorrowed | 前 | 52.711 | 119.104 | 154.126 | 183.334 |
| GNU / NativeBorrowed | 后 | 23.775 | 11.767 | 6.015 | 4.146 |

NativeSafe 的 8 线程聚合吞吐在这组样本中提高约 13.5 倍（Darwin）和 37.5 倍（GNU）。短样本会受到启动成本、调度和频率变化影响，尤其是直接 C 与多线程列；这不是对整段程序加速比的承诺。

### GC 压力：停稳等待与完整停顿

下表为微秒，单元格依次为 p50 / p99 / 最大值。等待测量从 STOPPING 发布到全部目标停稳；完整停顿从 STOPPING 到恢复 RUNNING 前，不使用 collector 原有的局部 pause 指标代替。

| 平台 / 线程 | 版本 | 记录的 GC 数 | 停稳等待 | 完整停顿 |
| --- | --- | ---: | ---: | ---: |
| Darwin / 1 | 前 | 3,371 | 6 / 20 / 95 | 28 / 46 / 217 |
| Darwin / 1 | 后 | 2,420 | 0 / 8 / 70 | 25 / 33 / 198 |
| Darwin / 2 | 前 | 11,125 | 7 / 30 / 222 | 28 / 68 / 245 |
| Darwin / 2 | 后 | 2,465 | 0 / 16 / 82 | 22 / 45 / 278 |
| Darwin / 4 | 前 | 39,936 | 13 / 55 / 816 | 37 / 90 / 878 |
| Darwin / 4 | 后 | 2,476 | 12 / 51 / 117 | 34 / 87 / 190 |
| Darwin / 8 | 前 | 48,645 | 62 / 193 / 460 | 96 / 263 / 486 |
| Darwin / 8 | 后 | 4,046 | 42 / 267 / 401 | 81 / 345 / 532 |
| GNU / 1 | 前 | 3,332 | 0.657 / 4.355 / 72.748 | 89.737 / 114.743 / 1,628.281 |
| GNU / 1 | 后 | 2,213 | 0.677 / 101.599 / 110.395 | 89.926 / 195.507 / 1,498.492 |
| GNU / 2 | 前 | 18,997 | 1.419 / 7.264 / 152.976 | 94.299 / 104.215 / 1,525.575 |
| GNU / 2 | 后 | 2,305 | 4.089 / 101.801 / 103.722 | 99.363 / 197.928 / 1,150.563 |
| GNU / 4 | 前 | 49,216 | 4.618 / 12.806 / 179.560 | 106.920 / 170.237 / 1,523.251 |
| GNU / 4 | 后 | 2,501 | 6.797 / 101.928 / 102.429 | 110.974 / 204.698 / 1,605.792 |
| GNU / 8 | 前 | 100,000* | 10.938 / 32.954 / 548.491 | 123.048 / 205.511 / 1,456.213 |
| GNU / 8 | 后 | 2,935 | 12.795 / 80.480 / 132.221 | 131.204 / 213.772 / 1,584.805 |

Darwin 时钟在此测量中约有 1 微秒分辨率，0 表示低于计时分辨率。`*` 表示只保存最先发生的 100,000 次 GC 分布，benchmark 的调用总时长仍覆盖全部迭代；其余组合没有达到记录上限。

去掉每次 transition 的广播后，collector 有时需要等超时复查，因此 **停稳等待的尾部并非普遍改善**：GNU 的 1/2/4 线程 p99 约为 102 微秒，Darwin 8 线程完整停顿 p99 从 263 增至 345 微秒。条件等待的实际唤醒包含系统调度延迟，不承诺等于 50 微秒。当前取舍是显著减少高频 native 调用的全局争用，同时保留可推进的停稳等待；本记录不把吞吐改进写成全部 GC 延迟改进。

初版不间断发起 GC 的测量在 Linux 两线程上超时；它会反复抢先取得 world lock。正式前后对照均使用上述 100 微秒间隔，超时记录不作为成功样本。

## M33-1b/1c：真实 Scoop GCLeaf / DirectC

2026-10-08 测量，硬件与 LLVM 版本同上。源码、计时与复现步骤见 [FFI benchmark](../../tests/benchmarks/ffi/README.md)，原始样本见 [Darwin](measurements/ffi-darwin.json) 与 [Linux GNU](measurements/ffi-linux-gnu.json)。native callee 位于独立的 C 对象，所有 caller 采用 O2、同一链接方式与同一实际操作，无 LTO、内联或 builtin 替换；每个返回值都进入校验和。每个工作线程只通过一次 foreign callback 进入 Scoop，计时包含该次 attach/detach。

无 GC 配置采用 production runtime；每线程 2,000,000 次调用，三轮交替执行六条路径。下表单位 ns，仍是总墙钟时间 / 总调用数。scalar 为整数传参并返回，aggregate 为两个 Long 的 C-layout struct 传参和返回；两类工作负载分别与自己的 C baseline 比较。

| 平台 / 路径 | 1 线程 | 2 线程 | 4 线程 | 8 线程 |
| --- | ---: | ---: | ---: | ---: |
| Darwin / C scalar | 0.776 | 0.418 | 0.210 | 0.108 |
| Darwin / NativeSafe DirectC | 31.281 | 16.299 | 8.371 | 6.234 |
| Darwin / GCLeaf DirectC | 12.557 | 6.826 | 3.473 | 1.781 |
| Darwin / C aggregate | 0.751 | 0.418 | 0.210 | 0.108 |
| Darwin / NativeSafe bridge | 28.904 | 15.418 | 7.739 | 7.283 |
| Darwin / GCLeaf bridge | 13.752 | 8.043 | 3.841 | 2.247 |
| GNU / C scalar | 0.930 | 0.460 | 0.223 | 0.137 |
| GNU / NativeSafe DirectC | 30.977 | 15.990 | 8.904 | 5.803 |
| GNU / GCLeaf DirectC | 16.237 | 7.882 | 4.622 | 2.379 |
| GNU / C aggregate | 0.996 | 0.604 | 0.302 | 0.188 |
| GNU / NativeSafe bridge | 35.256 | 17.786 | 9.511 | 6.753 |
| GNU / GCLeaf bridge | 15.906 | 8.910 | 4.664 | 2.640 |

GCLeaf DirectC 单线程耗时为 Darwin 12.557 ns、GNU 16.237 ns，普通 NativeSafe 分别为 31.281 ns、30.977 ns。它们仍明显高于直接 C 循环：优化机器码表明 Scoop 每轮保留正常的 `scoop_rt_safepoint` poll，C baseline 没有这项语言运行时义务。GCLeaf 的实际 native 调用已经直接传值，没有额外 bridge、根发布或状态切换；这里不将整段循环的差距归为 GCLeaf 调用协议，也不声称整段代码与 C 等价。aggregate 的构造、storage bridge 与拷贝成本单独保留，跨负载的微小排序不能证明某一种调用一定更快。

### GC 压力下的停稳与完整停顿

使用同一 Scoop 程序与启用测试观测点的 runtime 副本，工作线程各执行 20,000,000 次 scalar 调用，独立 collector callback 在每次 full GC 后 NativeSafe 等待 100 微秒。compiler 生成真实 stackmap、native transitions 和回边 poll。观测点自身会增加此测试配置的开销；无 GC 表格的 production runtime 不含这些调用。collector 超时复查仍为 50 微秒。

下表单位为微秒，单元格依次为 p50 / p99 / 最大值。所有样本均未达到 100,000 条记录上限。

| 平台 / 线程 | 调用 | GC 数 | 停稳等待 | 完整停顿 |
| --- | --- | ---: | ---: | ---: |
| Darwin / 1 | NativeSafe | 4,089 | 0 / 11 / 87 | 43 / 71 / 275 |
| Darwin / 1 | GCLeaf | 1,791 | 6 / 21 / 159 | 55 / 80 / 257 |
| Darwin / 2 | NativeSafe | 4,239 | 1 / 19 / 114 | 55 / 74 / 226 |
| Darwin / 2 | GCLeaf | 1,785 | 13 / 27 / 80 | 68 / 87 / 313 |
| Darwin / 4 | NativeSafe | 4,502 | 14 / 63 / 153 | 75 / 139 / 300 |
| Darwin / 4 | GCLeaf | 1,799 | 29 / 63 / 100 | 90 / 128 / 313 |
| Darwin / 8 | NativeSafe | 6,582 | 32 / 220 / 377 | 117 / 338 / 663 |
| Darwin / 8 | GCLeaf | 2,299 | 148 / 408 / 578 | 253 / 550 / 774 |
| GNU / 1 | NativeSafe | 4,623 | 0.946 / 101.801 / 135.836 | 116.575 / 225.623 / 1574.895 |
| GNU / 1 | GCLeaf | 1,864 | 0.91 / 3.882 / 107.269 | 109.572 / 245.755 / 1617.321 |
| GNU / 2 | NativeSafe | 5,005 | 4.204 / 101.868 / 103.407 | 124.191 / 227.657 / 1640.115 |
| GNU / 2 | GCLeaf | 1,828 | 5.662 / 9.885 / 107.24 | 126.057 / 150.888 / 1862.071 |
| GNU / 4 | NativeSafe | 5,194 | 4.964 / 102.074 / 149.019 | 143.032 / 241.72 / 1395.746 |
| GNU / 4 | GCLeaf | 1,955 | 8.274 / 24.412 / 159.316 | 148.604 / 226.201 / 1146.368 |
| GNU / 8 | NativeSafe | 6,159 | 10.158 / 102.241 / 144.701 | 171.483 / 276.33 / 1363.536 |
| GNU / 8 | GCLeaf | 2,307 | 16.912 / 45.481 / 132.695 | 179.461 / 293.788 / 1377.457 |

GCLeaf 的线程保持 managed，collector 必须等真实 poll；普通 C 调用已发布 NativeSafe 时可直接视为停稳。Darwin 8 线程下，GCLeaf 的等待 p99 为 408 微秒，高于 NativeSafe 的 220 微秒；GNU 的 GCLeaf 等待尾部在本次样本中较低，但完整停顿并非始终更短。吞吐、停稳和整轮 GC 是不同指标，不能用单次调用加速推导全部 GC 延迟改善。errno 捕获的独立测量见下一节。

## M33-5：errno 捕获的独立成本

2026-10-09 补测。复用同一 [FFI benchmark](../../tests/benchmarks/ffi/README.md)，增加 C、NativeSafe 与 GCLeaf 三条捕获路径，以 `--errno-only` 只执行本组，不重复既有 aggregate 与 GC 压力测量。三条路径调用同一个独立编译的 `bench_scalar`，调用前清零 errno，返回后立即读取，并将 native 返回值和 errno 一起计入校验和；callee 的操作仍是整数加一，errno 保持零。每线程 2,000,000 次调用，1/2/4/8 线程分别取三轮交替样本的中位数，共 36 个样本。

Darwin 使用上述 Apple M3 Ultra，native callee 与 C caller 为 Clang 22.1.8 O2，Scoop 为 LLVM 22.1.8 release，bridge 使用目标 SDK 工具链。Linux GNU 使用上述 NUC12，native callee 与 C caller 为 Clang 22.1.2 O2，Scoop 为 LLVM 22.1.2 release，目标 C ABI 工具链为 GCC 15.2.0。两者采用 production runtime、动态系统库、独立对象和禁用 builtin／LTO 的相同构建条件。计时期间被测机器不执行其他编译、fixture 或目录清理。原始样本见 [Darwin](measurements/errno-darwin.json) 和 [Linux GNU](measurements/errno-linux-gnu.json)。

下表单位为 ns，计算方式为总墙钟时间除以总调用数，表示聚合吞吐；只有单线程列可近似理解为调用延迟。

| 平台 / 路径 | 1 线程 | 2 线程 | 4 线程 | 8 线程 |
| --- | ---: | ---: | ---: | ---: |
| Darwin / C errno | 2.350 | 1.248 | 0.633 | 0.319 |
| Darwin / NativeSafe errno bridge | 31.620 | 16.015 | 9.257 | 7.718 |
| Darwin / GCLeaf errno bridge | 17.284 | 8.556 | 4.149 | 2.234 |
| GNU / C errno | 1.183 | 0.600 | 0.300 | 0.190 |
| GNU / NativeSafe errno bridge | 32.349 | 16.469 | 9.258 | 6.408 |
| GNU / GCLeaf errno bridge | 17.833 | 9.259 | 5.399 | 2.778 |

这组结果包含 libc errno 访问、必要的 StorageBridge 及结果存储。Scoop 循环保留正常回边 poll；GCLeaf 捕获调用自身没有 NativeSafe 进出和 caller-root 操作。它们与无捕获的 DirectC 是不同调用路径，不能把整段循环相对 C 的差距都归为状态切换，也不以不同批次的微小耗时差异推断固定的 errno 增量成本。

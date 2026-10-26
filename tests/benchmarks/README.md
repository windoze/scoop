# 程序性能基准

## 预构建程序的 A/B 对照

`compare.py` 用于比较两个已经构建并验证输出的程序，支持 Darwin 和 Linux。构建不进入计时区间；同一组输入参数分别传给 baseline 和 candidate，先预热，再按 AB／BA 次序交替启动独立进程。它直接服务编译器、runtime 和库优化的前后对照，也可比较同一程序的不同运行配置。

```sh
mkdir -p tmp/program-performance
export TMPDIR="$PWD/tmp"
python3 tests/benchmarks/compare.py \
  --baseline "$PWD/tmp/program-performance/baseline/app" \
  --candidate "$PWD/tmp/program-performance/candidate/app" \
  --baseline-label "before, release" --candidate-label "after, release" \
  --runs 7 --warmups 1 \
  --work "$PWD/tmp/program-performance/compare-01"
```

工作目录必须尚不存在。每次运行的 stdout／stderr、耗时、user／system CPU 和峰值 RSS 都保存于该目录；`report.json` 记录可执行文件、参数、显式环境覆盖、主机、实际运行顺序和原始样本。峰值 RSS 按每次子进程的资源统计读取，在 Darwin／Linux 上统一为 bytes；它不是整个进程树的内存曲线。耗时包含进程启动、输出和退出，应用内部稳态区间、GC 暂停或请求尾延迟需用对应专项测量。

所有预热和正式运行必须正常退出且 stdout 完全相同。可用 `--expected-stdout <file>` 核对已知正确输出，避免两个版本恰好产生相同错误；普通 stderr 保留供检查。`--arg=<value>` 可重复指定双方相同的参数。`--baseline-env NAME=VALUE`／`--candidate-env NAME=VALUE` 可重复指定不同运行配置，报告只保存显式覆盖及工具控制的 GC 设置，不保存整个继承环境。构建版本与选项写入对应 label。

工具默认清除继承的 GC stress、full-only 和统计开关。`--gc-stats` 显式开启诊断，保留每次 GC 原始记录并核对暂停直方图；它与正常发布形态分别测量。例如通过 `--candidate-env SCOOP_GC_FULL_ONLY=1 --gc-stats` 对照 nursery 与 full-only。

报告汇总双方正式样本的中位数、baseline／candidate 比值和每对运行的耗时比；大于 1 表示 candidate 的该项耗费更少。这些是描述统计，不能据此宣称统计显著性，也不能将程序启动测量当作 Java JIT 稳态结果。预热样本不进入汇总。出现失败或输出不一致时保留原始文件并停止，不发布成功报告。

工具自身的进程、交替次序、输出及 GC 记录测试：

```sh
TMPDIR="$PWD/tmp" python3 -m unittest discover -s tests/benchmarks/tests
```

按需机器符号映射与 macOS `sample`／Linux `perf` 的使用见[函数级采样归因](PROFILING.md)。
它保留原始采样含义，与普通 A/B 计时分别测量，不作为 PGO 编译输入。

[普通字符串拼接](scalars/concat.scoop)复用 scalars 的显式 phases helper，四个模式为热二元、五段链、共享中间结果和全常量链；设置 `SCOOP_BENCH_KIND=0..3` 与 `SCOOP_BENCH_COUNT`。输入构造在计时外，GC 统计另行运行，IR 与机器码核对实际分配／复制；结果和边界见[拼接记录](../../docs/program-performance/STRING-CONCAT.md)。

[实际装箱](scalars/boxing.scoop)用相同 helper 和选项，模式 0～3 分别为 ZST、Long、含引用小值和较大值；结果写入外部数组并在计时后读取，避免被非逃逸优化消除。GC 统计与正常运行分开，范围和结果见[装箱记录](../../docs/program-performance/BOX-ALLOCATION.md)。

[数组复制与初始化](arrays/copies.scoop)复用 [scalars 的 phases helper](scalars/README.md)，将每次产生的新数组存入 256 项循环数组，输入在计时前建立。模式 0～7 分别为八项引用字面量、八项引用 clone、两端追加元素的 vararg spread、接口 clone、含两引用的聚合 clone、Long clone、ZST clone，以及 2,048 项聚合 clone；对应输出为 `92 / 215 / 94 / 215 / 215 / 17 / 8 / 4295`。设置正数 `SCOOP_BENCH_COUNT`，普通模式建议一千万次，大数组模式两万次；GC 统计另行运行。小数组用于测量初始化屏障，大数组保留慢分配路径，无引用模式作为对照。

[ArrayList 快照](arrays/list-snapshots.scoop)从容量 4,096、逻辑长度 1,025 的 `ArrayList<Long>` 连续取得十万对不可变／可变数组，修改可变结果后读取两份快照，预期输出为 `5307150000`。用相同 runtime 分别构建基线、仅整字存储和新增快照成员三组，普通计时与 GC 统计分开；检查实际复制循环、分配量及编译成本。

## M31 性能用例

M33 的真实 C FFI 调用与 collector 等待基准见 [ffi/README.md](ffi/README.md)。

M34 的专项用例包括 [接口调用与转换](interfaces/README.md)、[纯计算循环的 GC 请求响应](poll-response/README.md)、[小值 ABI](small-values/README.md) 与 [容器存储](containers/README.md)。具体样本及取舍见 [M34 性能记录](../../docs/milestone34/PERFORMANCE.md)。

[资源作用域基准](abstractions/resources/README.md)对照 `Disposable.use` 和直接写法，测量闭包、正常清理、字段副作用及最后读取的完整成本；复用阶段计时与 A/B 工具，GC 诊断单独运行。

[跨 Cone 普通成员基准](inlining/members/README.md)覆盖用户定义的值方法、引用方法、getter 和 primitive hash，并保留未知接口调用及直接标量循环作对照；检查正文选择、稳态调用成本和产物大小。

[真实短命对象分配](allocation/nursery/README.md)对照 16～136 字节对象、数组、带 release hook 的共享旧代对象与无分配循环，显式发布对象以保留实际分配；既可对照 runtime，也可使用配套工具与 sysroot 对照生成代码。

这些普通 Scoop 程序分别覆盖 primitive 循环、aggregate/enum、短命小/中对象、标量与引用数组、长寿旧图的少量写入，以及 String/List/泛型组合。输出用于确认工作负载确实完成，不以耗时阈值作为正确性条件。

`measure.py` 只调用正式 `scoop build` 与所生成的程序；每次报告保存冷/热构建、各次运行时间、产物大小和实际输出。使用独立工作目录，默认重复运行五次。传入配套工具、sysroot 和 runtime 路径可复跑 M30 保存的基线；Linux 通过重复的 `--build-arg` 指定现有 target、C compiler 与 unwind 参数。

```sh
mkdir -p tmp
export TMPDIR="$PWD/tmp"
python3 tests/benchmarks/measure.py --tools target/release \
  --sysroot sysroot --runtime runtime --work tmp/m31-benchmark \
  --revision "$(git rev-parse HEAD)" --profile debug
```

报告同时记录各次样本、中位数和最小/最大值。GC 统计由 runtime 的诊断输出提供时保留在每次 stderr；没有统计的旧 runtime 不推测扫描量或收集次数。数据与实际参数归档到 `docs/milestone31/`，源码使用同一输入规模比较各阶段。

M31 使用 `--gc-stats --compare-full`：每个 profile 只构建一次，每轮交替运行默认 nursery 和 `SCOOP_GC_FULL_ONLY=1`，记录两组时间、实际 GC 计数及停顿分布。脚本清除继承的 GC stress 开关，检查两种模式输出一致、full-only 没有 minor，以及直方图与实际收集次数一致。分别运行 debug/release 得到四组控制；共同的 ABI、分配布局和必要 lowering 不计入 debug→release 因子。原有六个程序保持原输入，`old-graph-large` 将旧图增大八倍，`survivors` 另外覆盖高存活率、跨代引用和 pin。

[survivor-cache.scoop](survivor-cache.scoop) 保留 8192 个最近创建的记录，连续替换八百万次，用于观察中等寿命对象的提前晋升、full 次数与内存占用，输出为 `127738132421632`。使用普通 release 构建，以 `compare.py --baseline-env SCOOP_GC_SURVIVOR=0 --candidate-env SCOOP_GC_SURVIVOR=1` 对照同一程序，GC 统计单独运行；不要把减少晋升直接等同于缩短耗时。长期保留对象的 `survivors`／trees 仍作为独立对照。

```sh
python3 tests/benchmarks/measure.py --tools target/release \
  --sysroot sysroot --runtime runtime --work tmp/m31-benchmark-debug \
  --revision "$(git rev-parse HEAD)" --profile debug --gc-stats --compare-full
```

Linux 例如追加 `--build-arg=--target --build-arg=x86_64-unknown-linux-gnu`，以及 `--build-arg=--unwind-prefix --build-arg=/absolute/sysroot/lib/x86_64-unknown-linux-gnu/unwind`。M30 runtime 不提供这些 GC 统计，复跑旧基线时省略 `--gc-stats --compare-full`。容器和原生宿主分别记录环境，各自比较，不跨环境计算加速比。

当前 Linux 复现使用 `ssh nuc12`，仓库为 `~/repos/scoop`；工作目录和 `TMPDIR` 都放在该仓库的 `tmp/` 下。先完成工具构建与功能验证，再串行计时，避免并行编译或清理目录干扰测量。

M34 的 `mir-inlining.scoop` 用两千万次 xorshift 组合比较同一 release 配置下的 MIR 内联开关。kernel 与 helper 为 NoGC，使现有完整 poll 不掩盖调用和常量分支的差异；最终 checksum 为 `4062365736`。用 `--case mir-inlining --runs 7 --gc-stats` 单独运行，关闭内联的工具保存自 M34-3a。两套工具分别使用独立 cache，结果及热点机器码见 [M34 性能记录](../../docs/milestone34/PERFORMANCE.md)。

M34 的 `region-stores.scoop` 在保活对象之间执行八百万次单槽写入，单独记录四级 page map 内联屏障的成本；输出为 `9`。配合 `allocation`、`old-graph-large` 与 `survivors` 测量分配、稀疏旧区写入和存活图的组合影响。对照使用相同 release 编译优化与源码，每组七次、逐轮交替旧／新 heap，保留 GC 计数与机器码；不把引入可扩展堆所需的屏障成本隐藏在整体平均值中。

M34-7 的 `regions/region-churn.scoop` 连续两轮分配 180 万个 Node，分别在保留整个图、保留 64 个稀疏对象、全部离开作用域后执行 full 并采样；输出为 `113400000`。先将同目录的 `region-memory.c` 编译为 `libm34_memory.a`，再用普通 `scoop build --library-path` 链接。C companion 只读取两版 runtime 共有的映射计数，并用相同 OS API 记录进程当前／峰值 RSS；六条 phase JSON 写入 stderr，最终 GC 统计仍由 `SCOOP_GC_STATS=1` 输出。设置 `SCOOP_BENCH_PIN=1` 会在稀疏阶段 pin 首尾两个对象，再在退出本轮前 unpin，用相同程序对照 pin 对集中回收的影响。phase 0／3 为完整图，1／4 为稀疏图，2／5 为作用域退出后的状态；实际 RSS、成功 discard 和 unmap 是不同的测量值。

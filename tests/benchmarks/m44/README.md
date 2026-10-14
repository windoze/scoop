# M44 GC 对照

复用 `../parallel/mark-graphs.scoop` 的 wide／array／chain 程序。先构建固定
binary，再串行测量；计时期间同机不运行构建、测试或清理。临时文件使用仓库的
`tmp/m44`，输出使用 `target/m44`。

```sh
export TMPDIR="$PWD/tmp/m44"
python3 tests/benchmarks/m44/build-graphs.py \
  --tools target/m44/tools-baseline \
  --work target/m44/graphs-baseline-darwin \
  --cache target/m44/benchmark-cache --target aarch64-apple-darwin
```

构建脚本保留当前 runtime／sysroot 的副本、工具摘要、源码 commit／已跟踪差异、
C／LLVM 版本、构建命令与输出，以及 LLVM/object dump。runtime 副本启用线程测试
观测点，原 runtime 不受修改；编译器工具应预先构建并固定。Linux 指定实际 target，
必要时传 `--unwind-prefix`。`cc` 应与 target 对应；GNU／musl 使用各自工具环境。

优化后换一个 `--work` 构建 candidate，然后在同机交替测量：

```sh
python3 tests/benchmarks/m44/measure-graphs.py \
  --binary baseline=target/m44/graphs-baseline-darwin/graphs \
  --binary candidate=target/m44/graphs-candidate-darwin/graphs \
  --output target/m44/graphs-comparison-darwin.json --runs 7 --workers 4
```

每种图先对每个 binary 预热一次，再交替运行七次进程，每次主动收集五轮；保留预热
数据但不纳入汇总。默认 131071 个节点，可用 `--nodes`、`--rounds`、`--graph` 选择
输入；`--workers` 固定同组 worker 数。未传第二个 binary 时可单独记录当前基线，
但收益结论使用同机交替对照。脚本清除继承的 GC／benchmark 环境开关，显式启用统计
和指定 worker，检查 checksum、记录数量、phase 顺序、时间差及 GC histogram。

逐轮记录实际 moved_objects、原有阶段指标、worker CPU、映射量和 RSS，以及紧随
STOPPING／COLLECTING／RUNNING 发布的 T0／T1／T2。三个差值分别表示停稳等待、world
停止阶段、请求到允许恢复的区间。最后一个区间不包含随后广播／解锁／OS 调度，不能
代替线程实际停止推进到恢复推进的测量。测试回调和时钟读取本身有成本，两侧均开启；
既有 pause_ns／stop_wait_ns 口径不变。

报告保留逐次进程与收集原始数据，汇总中位数、p95、最小和最大值；七次运行、每图
35 次主动收集的 p95 仅作样本描述，不代表稳定尾延迟。process_seconds 包含构图、
收集及校验，nodes_per_second 只是整个工作负载吞吐；不能当成纯分配速率。worker
CPU 与阶段墙钟分开，阶段可能重叠，不把阶段之和称为总暂停。

## 普通 full 的选择性搬迁

`--graph compact --nodes 1800000 --rounds 5` 使用同一图程序的稀疏保留场景：先分配
180 万个 node／GC-free leaf 对，再从全部存活降为每 16 个保留一个；保留节点形成
引用环，并由 aggregate 外部的真实数组根持有。每进程执行五次独立增长／收缩周期，
只围绕收缩后的主动 full 记录逐轮阶段，普通策略决定 source 和 destination。
测量器检查每轮发生实际搬迁且 source region 数多于 target，输入太小或没有集中
收益时失败；不得将这样的运行当作移动路径性能样本。checksum 验证保留的值和完整
引用环，输出为五轮保留值之和。此场景未开启 moving stress。

构建方式相同，使用独立 `--work` 固定两个版本，再给测量命令传上述参数。
各轮 source／target 数和真实 moved_objects 保存在报告中；生命周期内还存在未单列
阶段的自动 minor 和初始 full，其总计仍保留在进程 GC 统计，不能把阶段样本当成
进程全部收集。

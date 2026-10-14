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

## 年轻引用图与 minor

`--graph minor --nodes 4096 --rounds 5` 先把可空引用数组放入旧代，再构造 4096 个
年轻节点和同数 GC-free leaf，组成反向引用环；固定第一个节点，让它引用最后一个
可搬迁节点。普通分配压力触发一次 minor，未增加主动 minor 的生产 API，也不启用
stress。核对整环、leaf 值和 unpin 结果，记录晋升字节、脏卡与旧代槽访问。

每轮必须恰好一次 minor、无 full／fallback、确有移动与 remembered 工作。pin 所在
block 按既有策略原地晋升，因此不要求除单个 pinned 对象外全部移动。构图不得提前
触发 minor；该默认节点数留出 nursery 余量，任意调大输入可能使这一前提不成立。

构建脚本可传 `--runtime`／`--sysroot`，用同一份基准源码配合两套固定工具和对应
runtime。分别用 `--workers 1` 与 `--workers 4` 完成七次交替进程，每进程五轮；
逐对核对实际 traced／moved／promoted／dirty-card／old-slot 和分配计数再比较时间。
阶段时间只计 collector；进程吞吐还包含准备 full、构图、压力分配与校验。

## 编译器 GC 边界整理

`build-codegen.py` 复用现有 allocation、arrays、old-graph 和 primitive 四个程序，
分别覆盖分配、引用数组写入、字段写入与纯计算循环 poll。两个固定工具集使用同一份
runtime／sysroot 副本，保留源码、工具摘要、构建输出和 IR dump；该步骤不进行计时。

跨 runtime 版本的整体对照可分别传 `--runtime`／`--sysroot`，每次只构建与该源码
匹配的工具集，使用不同的 `--work`。报告保存实际来源与 runtime 逐文件摘要；合并
两份报告时保留各自完整构建记录。整体对照不能使用下述要求完整 binary 相同的
`inspect-codegen.py` 作为通过条件，应分别检查生成函数与 runtime 的实际机器码差异。

```sh
python3 tests/benchmarks/m44/build-codegen.py \
  --tools baseline=target/m44/tools-baseline --tools boundary=target/m44/tools-codegen \
  --work target/m44/codegen-darwin --cache target/m44/codegen-benchmark-cache \
  --target aarch64-apple-darwin
python3 tests/benchmarks/m44/measure-codegen.py \
  --build target/m44/codegen-darwin/build.json \
  --output target/m44/codegen-comparison-darwin.json --runs 7 --workers 4
python3 tests/benchmarks/m44/inspect-codegen.py \
  --build target/m44/codegen-darwin/build.json \
  --output target/m44/codegen-machine-darwin.json
```

先结束同机的全部构建／测试，再运行第二步。测量器核对 binary 摘要和独立计算的
checksum，预热一次后交替执行至少七次，保存每个进程的墙钟、CPU、完整 GC 统计与
中位数／p95／范围。iterations_per_second 是整个程序的工作负载吞吐，包含启动和
收集，不能解释为单条 store 或 poll 的成本；小程序的进程计时只作回归观察。
正常 GC 配置和 worker 数固定，原始统计保留收集数、映射量、RSS 与分配量。
结构检查确认两侧根节点由不同编译缓存键实际编译，核对 LIR、产物、完整 binary 和
所有根 Cone 函数的机器码，并保存反汇编与 section 大小。任何差异都需先审阅，不能
通过更新固定摘要隐藏变化；本检查专门用于保持生成代码不变的边界整理。

## 多线程实际进度与分配

见 [progress](progress/README.md)。`build-progress.py` 接受明确的工具、runtime 与
sysroot，生成带测试观测的固定 binary；`measure-progress.py` 对计算／分配两种模式、
1／4 个 mutator 分别预热并交替执行至少七次进程。记录每个线程确实推进后的时间，
不把 RUNNING 发布替代为已经恢复执行。

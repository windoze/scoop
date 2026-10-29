# Scoop / Go / GraalVM 21 本机基准

三套普通程序实现同一批工作负载，Java 同时运行 Graal JIT 和 Native Image。
源码、复跑脚本和结果都保存在此目录；`build/`、`logs/` 与编译中间产物由
[.gitignore](.gitignore) 排除。最终分析见 [2026-10-29 报告](results/2026-10-29/REPORT.md)。

## 工具与复跑

默认直接使用 `~/.scoop/bin/scoop`，由它选择相邻安装根的预编译标准库和 runtime。
`BENCH_SCOOP` 可以指定另一套完整安装；`BENCH_JAVA_HOME` 可以指定另一套 GraalVM。
Go 使用 PATH 中的 `go`。脚本目前在 macOS 上记录主机信息，进程计时复用仓库已有
`tests/benchmarks/process_metrics.py` 的 `wait4` 统计。

```sh
# 在仓库根运行；先完成构建，随后串行计时。
python3 benchmark/build.py --native --output results/new-run/build.json
python3 benchmark/run.py verify --output benchmark/results/new-run
caffeinate -i python3 benchmark/run.py measure --output benchmark/results/new-run
python3 benchmark/validate_results.py benchmark/results/new-run
python3 benchmark/run.py startup --output benchmark/results/new-run
python3 benchmark/diagnose.py benchmark/results/new-run
python3 benchmark/build_cost.py --output benchmark/results/new-run/build-cost.json \
  --work benchmark/build/new-run-cost
uv run --with matplotlib==3.11.2 python benchmark/summarize.py benchmark/results/new-run
```

`measure` 拒绝覆盖已有 `measurements.json`。`--case`、`--language` 可重复指定子集；
`--forks`、`--samples`、`--sample-ms`、`--jit-warmup-ms` 控制采样。完整记录包含命令、
输入、源码 SHA-256、执行顺序、每个样本、校验和、CPU 时间、峰值 RSS 与主机信息。
散列仅用于记录本次测量所用源码版本。

格式化与检查：

```sh
~/.scoop/bin/scoop fmt benchmark/scoop
~/.scoop/bin/scoop check benchmark/scoop --cache-dir benchmark/build/check-cache
gofmt -w benchmark/go
go vet benchmark/go/*.go
uv tool run --from ruff==0.16.10 ruff format benchmark
uv tool run --from ruff==0.16.10 ruff check benchmark
```

Java 构建使用 `javac -Xlint:all -Werror`；C helper 使用 C11，只有读取配置、读取时钟
和输出结果的函数。计算内核全部由三种被测语言实现。

## 测量口径

- Scoop：release（LLVM 22.1、O2）、普通 runtime；Go：默认优化；Java：本机
  GraalVM 21 JIT，以及 `native-image --no-fallback -O2 --parallelism=6`。
  Native Image 使用默认 Serial GC，未提供训练 profile；构建器自己的 ML-inferred
  profile 保持默认。Java 两种模式都设置 `-Xms64m -Xmx1g`；Scoop、Go 使用默认 GC，
  因此这不是等堆大小实验。
- 每项 3 个独立进程，按固定随机顺序交错执行四种模式。每进程 Java JIT 预热 5 秒，
  AOT 预热 500 ms；再测 5 个至少 200 ms 的批次。汇总先取每个进程的中位数，再取
  3 个进程的中位数。进程间最小、最大值也保留；15 个批次不冒充 15 个独立样本。
- 每次 kernel 使用 `42..49` 循环变化的 seed，返回可校验的结果。计时前计算 8 个 seed
  的值；每个计时批次的调用数和累计 XOR 都要与这些结果一致。四种执行方式的完整
  规模结果互相核对；缩小输入和完整规模均另与独立 Python 公式／实现核对。
- 单次 kernel 包含该任务的数据构造、初始化、分配和结果消费；没有把 Java 的初始化
  留在计时外而把 Scoop 的初始化放入。GC 正常开启，不调用显式 full GC，不使用 stress。
  小数组初始化中的零填充、对象布局、容器扩容和库算法属于各实现的实际成本。
- 时钟检查在 kernel 调用之间，每个调用至少约亚毫秒至数毫秒；进程启动、JIT 预热及
  结果输出不进入 kernel 时间。RSS 是整个进程（包含预热）的峰值，不等于存活堆大小，
  也不是每个 kernel 的净分配量。CPU 时间同样包括预热，不能直接跨语言相除。
- `startup` 测整个基准可执行程序的空工作分支，包括进程创建、加载、输出与退出。
  先丢弃每种实现 3 次，再测 30 次；OS 文件缓存保持温热，不声称磁盘冷启动。
- 单线程任务只有一个应用工作线程；GC／JIT 可以使用辅助线程。并行任务把相同的
  8 个 shard 分给 1／4／8 个 worker，每次包含启动与 join。Scoop 使用平台 Thread，
  Java 使用平台 Thread，Go 使用 goroutine 和 WaitGroup；不测虚拟线程或协程调度。

## 工作负载与比较边界

固定规模见 [cases.py](cases.py)。20 项覆盖整数依赖链、Mandelbrot、浮点数组、
数据分支、随机访存、同算法排序、值／引用记录、逃逸对象、递归对象图、动态分派、
动态数组、哈希表、十进制格式化／解析、字符串构建、Unicode 标量扫描和多核扩展。

`value-records` 在 Scoop／Go 中是连续值数组，在 Java 21 中是 `record[]` 引用数组；
`reference-records` 则三者都使用引用数组。`list-grow` 与 `hash-map` 中 Java 的
`Long` 会装箱，Scoop 泛型单态化、Go 原生值容器保留直接数值表示。这些项测常见
语言表示和标准库的组合，不作为同一机器布局的编译器后端对照。Unicode 项统一
计算 Unicode 标量之和，Java 正确跳过 UTF-16 surrogate pair，Scoop／Go 遍历 UTF-8。
字符串构建项都显式先生成十进制文本，未使用各自特殊的数字直写优化。

没有联网、数据库、真实 HTTP 服务、异步 I/O、磁盘持久化、异常抛出或尾延迟负载；
这里的结论只覆盖这些确定性程序。桌面背景任务和 macOS 对性能核／能效核的调度
未被隔离，不以小幅差异宣称统计显著性，也不使用一个总分给语言排名。

## 本次 Scoop 更新

先用 LLVM 22.1.8 执行：

```sh
LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22 CARGO_INCREMENTAL=0 \
  cargo build --release -p scoop -p scoopc -p scoop-linker -p scoop-lsp -p scoop-pm --bins
LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22 \
  python3 scripts/prepare_sysroot.py --target aarch64-apple-darwin \
    --scoop target/release/scoop --work-dir benchmark/build/sysroot
```

随后复制五个配套命令、25 个生产 Cone、公开头文件和四种 runtime 至 `~/.scoop`，
通过已有 `scripts/package_native.py` 整理私有 LLVM 动态依赖并重新签名。
安装的版本为 `ae13036d9ce6740517df96152707a30934ed4c3e`；所有基准实际调用安装中的
工具，未使用旧安装或 `target/release` 编译应用。此次是本机工具链更新，不是重新制作
整套发行归档；VS Code 扩展、API 文档及调试器不参与本次测量。

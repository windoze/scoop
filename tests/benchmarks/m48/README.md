# M48 构建测量

先构建当前 release 工具，再顺序测量；测量期间不重写工具二进制。

```sh
CARGO_INCREMENTAL=0 cargo build --release -p scoop -p scoopc -p scoop-linker --bins
python3 tests/benchmarks/m48/measure.py --output /tmp/scoop-m48-baseline
```

默认每个场景五组冷／暖命令，Scoop profile 固定 debug；`--case` 可以多次传入 core、std、empty、application、native。冷指每组使用新构建缓存，不刷新 OS 文件缓存。所有实际源码和工具保持存在；临时缓存与 target 在该组结束后清理，原始命令输出和 report.json 保留到指定输出目录。

application 复用 M42 的真实 std／JSON 多 Cone 应用，以源码 locator 连接 provider 和 consumer；native 复用 M33 的平台 C／头文件／runtime 组合，并在冷暖后修改头文件再次构建。每次 executable 构建后实际运行并检查结果。

报告保存每次时间、抽样进程树 RSS 峰值、精确 compiler build child 数和原始构建 JSON，结束时给出中位数与范围。ps 抽样每轮后等待 20 ms，RSS 是同一轮所有存活后代之和；短命工具可能漏采，抽样进程列表不是精确 native 或 linker 调用计数。测量脚本不修改生产构建路径，不提供运行时跟踪或预算机制。

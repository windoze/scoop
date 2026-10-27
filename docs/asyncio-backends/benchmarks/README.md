# 后端测量 Cone

使用普通 `scoop.test` 测量同步 `run` 的空闲等待、timer 偏差、文件吞吐与收尾。只有正确性断言，没有性能通过阈值；结果和比较口径见[性能记录](../PERFORMANCE.md)。宿主接入的指标来自 `scoop.asyncio` 中的 `EventLoopHostProgressTest`，使用真实 poll fd-watch。

先按仓库脚本说明准备匹配工具版本的开发 sysroot。在仓库根执行：

```sh
measure_dir="$HOME/scoop-work/asyncio-measure"
mkdir -p "$measure_dir/temp"
export TMPDIR="$measure_dir/temp"
target/release/scoop check docs/asyncio-backends/benchmarks --tests --release \
  --sysroot "$PWD/sysroot" --cache-dir "$measure_dir/cache"
target/release/scoop test docs/asyncio-backends/benchmarks --release \
  --sysroot "$PWD/sysroot" --cache-dir "$measure_dir/cache" \
  --target-dir "$measure_dir/target"
```

Linux 目标分别传 `--target x86_64-unknown-linux-gnu` 或 `--target x86_64-unknown-linux-musl`；musl 使用带独立 Linux UAPI 头搜索路径的 `--cc` 包装器。不要把 glibc 头目录加入 musl 路径。

后续测量复用生成的测试程序，通过 `SCOOP_TEST_RUN_DIR` 为每次运行指定仓库外的新输出目录。`scoop.test` 将指标写入测试 stdout，`events.jsonl` 保存对应的正确性结果。比较时先完成编译，关闭 GC stress，同机交错运行基线／平台版各三次并取中位数；保留源码、工具与原产物，日志不提交。

基线使用 `eb81be354` 的 asyncio、libuv 及兼容的 platform 资源；使用相同 release 编译器与此测量正文，仅把构造入口改为 `LibuvEventLoop()`。迁移前没有 `attach` API，宿主 post／turn 指标只能测当前实现，不能构造同接口的旧后端对照。

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

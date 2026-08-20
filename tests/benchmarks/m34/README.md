# M34 最终同机对照

这三个脚本保存 M34 最终验收实际使用的固定工作负载、构建步骤与交替测量方法。从仓库根目录运行，先完成构建和清理，再独占该机的测试时段计时。Darwin 使用 LLVM 22.1 的 `llvm-config`／`llvm-objdump`，Linux GNU 使用对应的 LLVM 22.1 工具与 sysroot 中的 unwind；不跨主机计算加速比。

脚本沿用验收工作目录 `tmp/m34`，需要以下已有输入：

- `baseline-source`：M33 revision `e97b40f5c9c408e2ff785e5d09c6bf9222c78979` 的源码、core 与 runtime；`baseline-tools` 是该 revision 配套的 release CLI。
- `baseline-darwin-release-llvm22` 或 `baseline-linux-gnu-release`：使用上级 [measure.py](../measure.py) 对八个普通程序保存的 M33 构建、缓存、可执行文件与 `report.json`。
- `target/release`：当前 M34 的三个配套 CLI；`index-darwin/cache`、`index-gnu/cache` 保存当前普通缓存。Darwin 如果已有 `acceptance-darwin/cache`，则复用该目录。
- `sysroot/native/x86_64-unknown-linux-gnu/unwind`：Linux GNU 的既有链接工具依赖。

`build.py` 为每个 mode 建立新的 `final-<kind>-<mode>-<host>` 目录，已存在时拒绝覆盖；`off` 表示完整 M33，`on` 表示完整 M34。它不是某个优化的单独开关。同步到另一台机器但没有同步 Git 历史时，使用 `M34_COMPILER_REVISION` 明确记录构建源码的本机提交号。

```sh
python3 tests/benchmarks/m34/build.py standard on
python3 tests/benchmarks/m34/build.py interfaces off
python3 tests/benchmarks/m34/build.py interfaces on
python3 tests/benchmarks/m34/build.py poll off
python3 tests/benchmarks/m34/build.py poll on
python3 tests/benchmarks/m34/structure.py interfaces
python3 tests/benchmarks/m34/structure.py poll
```

构建记录包含完整 argv、实际 compile/link 输出、首次与重复构建时间、产物字节数和 revision；复用缓存时的首次构建不是冷构建。接口保留独立 provider／consumer，poll 使用启用现有线程观测点的 runtime 副本，详见 [接口](../interfaces/README.md)和[纯计算 GC 响应](../poll-response/README.md)。

确认同机没有构建、测试或清理后依次运行：

```sh
python3 tests/benchmarks/m34/measure.py standard
python3 tests/benchmarks/m34/measure.py interfaces
python3 tests/benchmarks/m34/measure.py poll
```

每个模式先分别预热 M33／M34 一次，再运行七轮，逐轮交换先后顺序。脚本校验程序输出、接口独立计算的 UInt checksum，以及 GC histogram 总数；poll 的 C runner 独立校验各线程整数和。报告保留预热、全部正式样本、stdout／stderr、GC、墙钟／CPU、binary SHA-256 与大小。接口另记 kernel 时间；poll 的分位数是每个进程内真实 collection 样本的统计，不能把七个 p99 的中位数称为所有请求合并后的 p99。

结果写入 `tmp/m34/final-<kind>-<host>.json`；结构脚本保存实际 MIR／LIR 符号映射、代码字节数、目标 section 大小与 `.asm`。归档结果与解释见 [M34 性能报告](../../../docs/milestone34/PERFORMANCE.md)。

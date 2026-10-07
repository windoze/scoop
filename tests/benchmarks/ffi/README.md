# C FFI 调用基准

`program.scoop` 使用真实的 foreign-thread callback 执行 Scoop 循环；`runner.c` 提供相同的直接 C 循环。六种路径为 C scalar、NativeSafe DirectC、GCLeaf DirectC、C aggregate、NativeSafe StorageBridge、GCLeaf StorageBridge。每次调用的返回值都进入校验和，native callee 在独立的 `leaf.o` 中，禁止 LTO、内联和 builtin 替换。C/Scoop caller 都使用 O2，Scoop 循环保留正常回边 safepoint poll。

无 GC 测量使用 production runtime，各线程执行 2,000,000 次调用，按轮交替六条路径，保存三次样本和中位数。计时包含一次 callback attach/detach 及线程启停收尾；每线程只进入一次 Scoop callback。多线程结果为总墙钟时间除以总调用数，表示聚合吞吐，不能作为单线程调用延迟。

GC 测量使用独立 runtime 副本，在该副本的 `src/thread/testing.h` 顶部添加 `#define SCOOP_THREAD_TESTING`，启用已有测试观测点。collector callback 每轮 full GC 后 NativeSafe 等待 100 微秒，工作线程各执行 20,000,000 次 scalar 调用。分别记录 STOPPING→全部停稳与 STOPPING→恢复的 p50/p99/最大值；不使用现有局部 pause 指标代替。记录上限为 100,000 次，仅限制保存的样本，不限制 GC。观测回调会增加测试配置的开销；production 无观测点调用。

下面在仓库根目录构建。LLVM 使用项目要求的 22.1；Linux 在两次 `scoop build` 中另加 `--target x86_64-unknown-linux-gnu --unwind-prefix <GNU unwind 路径>`。工具路径可替换为配套构建，`--cache-dir` 可使用已有测试缓存。

```sh
export TMPDIR="$PWD/tmp"
mkdir -p tmp/m33/ffi-benchmark
clang -std=c11 -D_POSIX_C_SOURCE=200809L -O2 -fno-omit-frame-pointer \
  -fno-optimize-sibling-calls -fno-builtin -pthread \
  -c tests/benchmarks/ffi/leaf.c -o tmp/m33/ffi-benchmark/leaf.o
clang -std=c11 -D_POSIX_C_SOURCE=200809L -O2 -fno-omit-frame-pointer \
  -fno-optimize-sibling-calls -fno-builtin -pthread \
  -c tests/benchmarks/ffi/runner.c -o tmp/m33/ffi-benchmark/runner.o
ar rcs tmp/m33/ffi-benchmark/libffi_benchmark.a \
  tmp/m33/ffi-benchmark/leaf.o tmp/m33/ffi-benchmark/runner.o

target/release/scoop build tests/benchmarks/ffi/program.scoop --profile release \
  --sysroot sysroot --cache-dir tmp/m33/ffi-benchmark/cache \
  --target-dir tmp/m33/ffi-benchmark/target \
  --library-path tmp/m33/ffi-benchmark -o tmp/m33/ffi-benchmark/program

python3 - <<'PY'
from pathlib import Path
import shutil
copy = Path('tmp/m33/ffi-benchmark/runtime')
shutil.copytree('runtime', copy)
header = copy / 'src/thread/testing.h'
header.write_text('#define SCOOP_THREAD_TESTING\n' + header.read_text())
PY

target/release/scoop build tests/benchmarks/ffi/program.scoop --profile release \
  --sysroot sysroot --runtime-root tmp/m33/ffi-benchmark/runtime \
  --cache-dir tmp/m33/ffi-benchmark/cache \
  --target-dir tmp/m33/ffi-benchmark/target-gc \
  --library-path tmp/m33/ffi-benchmark -o tmp/m33/ffi-benchmark/program-gc

python3 tests/benchmarks/ffi/measure.py \
  --binary tmp/m33/ffi-benchmark/program \
  --gc-binary tmp/m33/ffi-benchmark/program-gc \
  --output tmp/m33/ffi-benchmark/report.json
```

先完成构建和功能测试，再运行计时，期间不要并行编译、清理目录或执行其他基准。结果归档到 `docs/milestone33/measurements/`。errno 捕获的测量在其功能实现后补充，不能将 aggregate 或 errno bridge 的成本归为 GCLeaf 状态切换。

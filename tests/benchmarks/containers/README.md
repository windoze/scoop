# M34 容器存储对照

同一个程序通过 `SCOOP_BENCH_KIND` 选择工作负载，通过 `SCOOP_BENCH_COUNT` 设置元素数或容量。退出前检查结果并输出 checksum；不以耗时作为正确性门槛。

| kind | 工作负载 | 正式测量 count | phase |
| --- | --- | ---: | --- |
| 0 | Int 列表 | 262144 | 0 预留容量，1 追加，2 逐项读取和修改，3 clear |
| 1 | interface 列表 | 131072 | 0 预留容量，1 分配并追加 Cell，2 接口读取，3 clear |
| 2 | ZST 列表 | 262144 | 0 预留容量，1 带副作用的追加，3 clear |
| 3 | 只有一个 Cell 的列表 | 1024／1048576 | 0 预留容量；一次预热 full 后，五次 phase 4 full |
| 4 | StringBuilder 拼接整数文本 | 32768 | 1 转换并追加，2 build |
| 5 | JSON 编码和解码 Int 列表 | 8192 | 1 追加，2 encode，3 decode |

`phases.c` 使用两版共有的 runtime 诊断计数器，记录各阶段墙钟、进程 CPU、分配字节、GC pause、mark、实际扫描槽数及当前／峰值 RSS。列表布局行读取普通 ArrayList 的两个实际字段及 backing 的 InlineArray 描述，记录 capacity、count、stride、alignment 和实际分配大小；不向生产代码加入容器观测入口。Int 工作负载显式执行 get 和 set，在同一阶段测量读取与修改。

先将 `phases.c` 以 C11／O2 编译并打包为 `libm34_container_bench.a`。使用同一套编译器、runtime 与优化配置，对照迁移前后的 core；两份 runtime 均包含 M34 原生 Option 返回 ABI 修复。以下命令中的路径分别指向所选工具、源码和独立输出目录：

```sh
cc -std=c11 -O2 -Wall -Wextra -Werror -I "$source/runtime/include" \
  -c tests/benchmarks/containers/phases.c -o "$work/phases.o"
ar rcs "$work/libm34_container_bench.a" "$work/phases.o"
"$tools/scoop" build "$source/sysroot/lib/scoop.core" \
  --scoopc "$tools/scoopc" --sysroot "$source/sysroot" \
  --runtime-root "$source/runtime" --profile release --cache-dir "$work/cache" \
  -o "$work/artifacts/scoop/scoop.core/0.1.0/cone.slib"
"$tools/scoop" build "$source/sysroot/lib/scoop.json" \
  --scoopc "$tools/scoopc" --sysroot "$source/sysroot" \
  --runtime-root "$source/runtime" --profile release --cache-dir "$work/cache" \
  --cone-path "$work/artifacts" -o "$work/artifacts/scoop/scoop.json/0.1.0/cone.slib"
"$tools/scoop" build tests/benchmarks/containers \
  --scoopc "$tools/scoopc" --sysroot "$source/sysroot" \
  --runtime-root "$source/runtime" --profile release --cache-dir "$work/cache" \
  --cone-path "$work/artifacts" --library-path "$work" -o "$work/containers"
SCOOP_GC_STATS=1 SCOOP_BENCH_KIND=0 SCOOP_BENCH_COUNT=262144 "$work/containers"
```

Linux 各个构建命令增加 `--target x86_64-unknown-linux-gnu` 和当前工具链的 `--unwind-prefix`。正式对照每组七轮，逐轮交换旧／新顺序；关闭继承的 stress、full-only 和 worker 覆盖，只保留默认 GC 策略。构建、测试与清理均在计时之外。原始构建命令、全部样本和结果见 [M34 性能记录](../../../docs/milestone34/PERFORMANCE.md)。

# 标量转换基准

`decimal.scoop` 测量整数转字符串，包含实际 String 分配，并消费结果的字节长度。模式 0 是六位以内 ULong，1 是完整位宽 ULong，2 是完整位宽 Long，3 是常量 `1234L`。前三种输入由确定的 LCG 生成；正确性由独立 Cone 测试覆盖，不以计时阈值断言。

复用 `interfaces/phases.c` 的阶段计时入口；只在该基准程序中链接，不向普通程序或 runtime 增加测量代码。例如，在仓库根执行：

```sh
mkdir -p tmp/scalar-bench
cc -O2 -c tests/benchmarks/interfaces/phases.c -o tmp/scalar-bench/phases.o
ar rcs tmp/scalar-bench/libscoop_scalar_phases.a tmp/scalar-bench/phases.o
target/release/scoop build tests/benchmarks/scalars/decimal.scoop --release \
  --sysroot sysroot --scoopc target/release/scoopc \
  --library-path tmp/scalar-bench --target-dir tmp/scalar-bench/build \
  -o tmp/scalar-bench/decimal
SCOOP_BENCH_KIND=1 SCOOP_BENCH_COUNT=2000000 tmp/scalar-bench/decimal
```

跨目标时给 C 编译器和 Scoop 同时选择相应目标与 unwind 资源。比较前保存旧程序和 sysroot，固定输入，先预热，再交替运行新旧程序；阶段 JSON 输出到 stderr，校验和输出到 stdout。原始输出和逐次数据保存在仓库外。`SCOOP_GC_STATS=1` 应另行运行，用于比较分配字节和 collection 次数，不混入普通计时。

`operations.scoop` 使用相同的 phases helper。模式 0 测动态 Long hash，1 测常量 ULong hash，2 测 Boolean equals 与 hash，3 测 Boolean 字符串化和结果长度。构建时将上例源码和输出名替换为 `operations`；计时建议五百万次以上，极短的无分配内核需要较长批次，Linux 可固定 CPU 后单独复核。

`strings.scoop` 使用同一 helper，模式 0 为动态 String 相等，1 为与八字节常量比较，2／3 为八／六十四字节 hash，4 为常量 hash，5／6 为 ASCII／混合 Unicode 遍历。输入在计时前构建；建议每轮三百万次，短路径与共享长路径分别报告。遍历包含真实 iterator／接口分派，不能将整段收益归给单步解码。

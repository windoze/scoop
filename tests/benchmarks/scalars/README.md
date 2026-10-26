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

`string_hash.scoop` 单独对照 hash 的长度范围。模式 0～13 的动态输入依次为 0、1、7、8、9、15、16、17、31、32、33、64、128、4096 字节，包含 NUL；模式 14／15 为十六／三十二字节常量。输入在计时前按字节构造，动态路径交替选择两个不同字符串，完整消费 hash。短路径建议两百万次，4096 字节建议三万次；同时检查边界两侧、常量调用点和完整程序，不能只用新覆盖的长度判断收益。该文件使用同一显式链接的 phase helper，普通程序不包含其测量代码。

`string_algorithms.scoop` 对照 Runtime 算法迁移，仍使用相同 phase helper。模式 0／1 为 1024 B ASCII／160 B 混合 Unicode 的字符计数，2 为动态字符定位，3 为字符切片，4 为 64 项 Char 输入的 fromChars，5／6 为严格／lossy UTF-8 转换。所有模式交替选择两份动态输入；前四项建议三十万次，其余十五万次。构造输入在计时前完成，输出消费长度或首字符；fromChars／decode 包含原有 List 快照、借用及结果消费，不能把整个差值单独归给编码内循环。

`floating_adapters.scoop` 使用同一 helper，模式 0／1 对照 Double／Float 的动态 toString，2／3 对照 JSON／CLI 实际使用的 Scoop ABI 解析后备。每轮交替八个普通、小数、科学计数法或带符号零输入；建议一百万次，分别消费文本长度或结果 bits。文本构造包含一次 String 分配，解析不分配托管结果；native 适配缓冲的分配要与 GC 统计分开报告。

`boxing.scoop` 使用同一 helper，将实际分配的值放入 256 项循环数组。模式 0／1／2／3 分别为 ZST、Long、含两个引用的值和 128 B 宽值装箱；模式 4 为与模式 2 字段相同的普通 class 初始化。模式 2／4 的对象实例均为 40 B，输入 Node／String 在计时前建立，写入旧数组仍需屏障；模式 3 的 box 实例为 144 B，保留共享后备。建议每轮一千万次，分别记录内核与整程序时间；零引用模式作为对照，不用它们推断引用初始化的收益。

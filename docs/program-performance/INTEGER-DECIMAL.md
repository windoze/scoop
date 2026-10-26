# 整数十进制格式化与普通 helper 正文

## 选择与边界

基线为 `4eead1c59`。定宽整数继续经 Long／ULong 实现十进制 `toString`，普通 core 正文向 20 字节 GC-free 局部缓冲区反向写入一次，返回实际长度；新的普通 Scoop-ABI extern 只负责按该长度分配 String 并复制。零、负数、Long 最小值、ULong 最大值、无 locale 依赖和无 NUL 的表示保持。负数幅度在 ULong 上按位宽回绕计算，避免最小有符号数取负溢出。

算法采用八位分块、32 位余数和两位数字表，复用仓库 vendored Ryu 的 digit table／数字写入思路。C ABI 的 `scoop_rt_long_to_string`／`scoop_rt_ulong_to_string` 保留，使用同一算法替代 `snprintf`；没有引入新依赖或修改浮点 Ryu 与解析。String 的表示、既有分配器、不可变身份和 GC 契约保持。raw pointer 复制入口要求调用期间地址稳定、长度非负、内容为合法 UTF-8；这里来源始终是栈缓冲区，不保留 managed 内部指针或新增 root。

普通可移植正文增加循环和 typed pointer 操作，沿现有声明队列递归导出普通 helper；消费方只继续处理新发现的使用，并接受已绑定的 imported callable application。跨 Cone 的实际函数身份、局部 place、loop target、字符串常量归属及原调用 effect 保持。没有修改内联评分或按名称特判：动态输入的 207 单位正文仍因收益不足保留共享 Scoop helper；常量 `1234L` 的剩余正文降到 87 单位后被接受，其两位 helper 也被展开。原有 byteLength／byteAt 生成路径通过完整正文得以复用，不算作新实现。

所有 profile 使用同一 core 算法和可移植格式，release 默认按调用点评分内联；没有新开关或 ABI 版本。普通 extern 的名称不参与优化识别。分配／复制继续使用共享 runtime，本批没有把动态完整扫描算法强制复制到每个调用点。

## 验证与对照

先完成 Rust 格式化、相关 crate 的 Clippy、C 警告检查和 fixture inspector 的 Ruff，再进行定向验证：

- native writer 对照 `snprintf`，覆盖边界、十进制位数转换、十万连续值和十万伪随机位型；每个位型检查有符号／无符号输出及缓冲区哨兵。本机与 Linux 通过。
- 三目标各 2 项 core Cone 测试，覆盖窄整数、动态接口、绑定方法和 collection 前后保存的结果；边界使用明确 typed 字面量。早期 const-property receiver 测试触发的 source-point 错误在旧基线也能复现，未扩大本批范围修复。
- 5 项 HIR 普通正文回归、既有泛型正文消费回归通过。三目标各一组成员内联 fixture（2 个 profile 变体、8 次进程调用），验证跨 Cone 私有 helper、循环、独立局部缓冲区、intrinsic application 及 moving GC。
- 最终 MIR 与机器码确认：三个转换内核均无旧整数 C 格式化入口／`snprintf`；常量内核没有十进制 helper 调用，动态内核保留共享 helper，分配复制入口和必要 poll 保持。byteLength 已成为直接长度读取，byteAt 的失败后备保持。

采用一次预热、三对交替运行，计时不打开 GC 统计；另用十万次转换检查 GC 计数。下表为含真实 String 分配的 ns／次，旧 → 新：

| 输入 | Darwin ARM64 | Linux GNU x86-64 | Linux musl x86-64 |
| --- | ---: | ---: | ---: |
| 六位以内 ULong | 83.462 → 52.872 | 56.846 → 45.691 | 219.480 → 79.026 |
| 完整位宽 ULong | 92.391 → 62.861 | 77.028 → 50.972 | 212.658 → 77.611 |
| 完整位宽 Long | 96.230 → 63.422 | 73.566 → 54.258 | 211.989 → 76.755 |
| 常量 1234L | 80.936 → 49.429 | 55.508 → 36.453 | 207.833 → 62.997 |

原有 strings 程序耗时降低 21.65%／16.53%／37.27%，strmap 降低 9.36%／10.72%／31.38%。这些是格式化和正文可见性共同变化的结果，不全部归因于一条 `snprintf` 调用。早期 Darwin 测量有明显系统波动，复测与最终产物数据分开保留；不采用那轮夸大的加速比例，也不以耗时作正确性断言。

三目标四种输入的分配字节、minor／full 和 root 扫描计数分别保持相同。十万次六位／常量转换为 3,200,104 B、3 次 minor；完整无符号为 4,799,712 B、有符号为 4,799,632 B，均 4 次 minor、0 次 full。每次仍产生一个结果 String，局部 scratch 不分配 GC 对象。

## 成本与未采纳方案

独立 C writer 对照了 `snprintf`、逐位除 10、逐两位除 100、八位分块。完整位宽 ns／次如下：

| 算法 | Darwin | GNU | musl |
| --- | ---: | ---: | ---: |
| printf | 43.660 | 40.573 | 140.098 |
| 除 10 | 18.131 | 18.776 | 19.594 |
| 除 100 | 8.489 | 11.069 | 11.014 |
| 八位分块 | 4.265 | 6.021 | 6.202 |

printf 微测适配了反向输出接口，包含额外的至多 20 字节复制；上面的 Scoop 端到端对照直接使用旧实现，避免依赖这一微测高估收益。两位以内输入的分块方案比除 100 慢约 0.4～0.6 ns；六位输入也略慢，但完整位宽和混合宽度获益。统一采用分块，没有为分布另设运行时选择或多套策略。

Darwin 的独立 writer text 为 64／148／332 B（除 10／除 100／分块），两位方案与分块均使用 200 B 表；各自编译中位数约 50.3／51.2／54.0 ms。因运行成本较高，较小的除 10、除 100 实现未采纳为默认。

三个代表程序的最终 text 增加 512～1,984 B，完整文件增加 1,488～8,680 B；core `.slib` 增加约 0.14%～0.15%，四个重建库的增幅为 0.14%～2.27%，包含新增可移植 helper。三对冷 consumer 编译的墙钟增加 1.10%／1.38%／1.18%，CPU 增加 2.29%／1.39%／1.56%，峰值 RSS 增加 2.53%／1.18%／1.19%。两处受改 C 翻译单元单次编译增加约 0.7～10.9 ms；这是 TU 对照，不是完整 runtime 冷构建。未增加生产产物的诊断或测量代码。

原始日志、IR、机器码和逐次数据保存在仓库外 `scoop-performance-results/2026-10-26/runtime-visible/integer-format`；实验源码／构建位于仓库忽略的 `tmp/program-performance/integer-format`。基准源码见 `tests/benchmarks/scalars/`。

# 标量计算改用普通 core 正文

基线为 `2dba829fe`。Boolean equals／hash 改为普通条件选择；Long／ULong hash 使用普通 `@NoGC` helper，在 ULong 上完成原来的 xor、逻辑右移和两次回绕乘法。窄整数继续先按原符号规则扩展，因此历史 hash 位型保持。Boolean.toString 选择已有的 `"true"`／`"false"` immortal 字符串，遵守 String 的不可变身份合同。

没有添加 intrinsic、编译器函数名特例、内联阈值或新开关。所有 profile 使用同一正文，release 按现有调用点评分展开。普通动态 hash 调用点的净增长为 28、收益为 24，继续调用共享 Scoop helper；热循环接受展开，常量热调用随后被折叠。旧的 C ABI 后备和调用合同保留，新 core 不再引用对应 native wrapper；本项不扩大现有 native 对象文件的裁剪范围。

完成格式检查和 Ruff 后，三目标各 3 项 core Cone 测试通过，覆盖符号／位宽边界、窄整数、Boolean 真值、接口、绑定方法及 GC 后保存的字符串。成员内联 fixture 每目标 2 个 profile 变体、8 次进程调用通过；普通 hash 调用点保留共享正文，新增热循环实际展开。初始 inspector 错误要求普通点也必须内联，已按真实调用点决策修正；Linux 传输意外生成的 AppleDouble 文件已删除，仅重跑受影响 fixture。

三个目标的四个热内核在 MIR 中没有标量外部调用，机器码仅保留必要的 safepoint 慢边；常量 hash 无运行时混合计算，Boolean 字符串化无分配。下表为一次预热、三对交替运行、每次五百万迭代的 ns／次，旧 → 新：

| 内核 | Darwin ARM64 | Linux GNU x86-64 | Linux musl x86-64 |
| --- | ---: | ---: | ---: |
| 动态 Long hash | 1.626 → 0.830 | 1.276 → 1.316 | 1.218 → 0.863 |
| 常量 ULong hash | 1.623 → 0.203 | 1.109 → 0.228 | 1.637 → 0.230 |
| Boolean equals＋hash | 2.704 → 0.604 | 2.878 → 0.691 | 3.284 → 0.512 |
| Boolean.toString＋byteLength | 47.497 → 0.539 | 37.473 → 0.699 | 68.223 → 0.635 |

GNU 动态 hash 初测的 +3.12% 约为 0.04 ns／次。为复核该小幅回退，单独固定 CPU 4、延长到五千万迭代并运行七对，得到 1.178 → 0.843 ns；两组结果都保留，不能把几毫秒的短批次当作稳定吞吐承诺。没有重复整套性能或正确性验收。

十万次 Boolean 字符串化在三目标均减少 3,200,000 B 分配，minor 3→0；剩余 104 B 来自基准启动／结果输出。其他三个内核的分配计数保持。新普通程序没有测量代码，基准显式链接既有 phases helper，GC 统计另行运行。

最终基准 text 增加 232／256／256 B，完整文件减少 2,384／7,512／7,512 B，core `.slib` 减少 10,506／10,466／21,002 B。三对冷 consumer 编译墙钟增加 2.05%／0.32%／1.06%，CPU 增加约 2.18%／0.27%／1.20%，峰值 RSS 增加约 1.19%／0.29%／0.37%。runtime 未修改或新增对象字段、GC roots、ABI 及诊断机制。

原始数据、IR 和机器码位于仓库外 `scoop-performance-results/2026-10-26/runtime-visible/scalar-bodies`；实验构建在 `tmp/program-performance/scalar-bodies`，基准源码为 `tests/benchmarks/scalars/operations.scoop`。

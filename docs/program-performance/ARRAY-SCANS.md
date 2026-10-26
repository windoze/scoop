# 固定引用布局的数组扫描

基线为 `a3caa13e8`，包含既有接口 SSA 和 32 B String hash 优化。新 strmap 主线程采样中，remembered 标记及回写占 1,185／3,489 个样本，约 34%；这里只用于定位，不换算为预期加速。脏卡已按实际范围扫描，剩余问题是每个数组元素递归进入 scan 解释器，重复执行描述符分派和函数边界操作。

runtime 适配层在数组入口识别实际 `References` child，直接沿元素和引用 offset 枚举槽，复用原 scan。没有按集合／元素名称特判，没有新增描述符或另一套扫描程序。仍逐槽检查半开地址范围，保持元素及 offset 顺序；每个 managed leaf 仍执行原标记／搬迁 visitor，itab、tag、padding 不参与扫描。Sequence 和递归 Array child 保留原解释路径。三目标机器码确认固定引用数组不再逐元素递归调用解释器，其他 child 仍有递归调用。

两机先格式化、C 警告检查和 release Clippy，再各通过 4 项定向单测（6 次 native 执行）。新 O0／O2 回归覆盖单／多引用、首尾部分元素、指针字内部切分、空范围、空数组、嵌套 Array／Sequence、访问顺序及真实槽写入；复用稀疏脏卡、survivor、并行回写与 native root 别名测试。三目标各通过 interface-ssa 和 reference-array-roots 的三个变体、五次程序运行，覆盖 debug／release、moving／minor／survivor、父投影、niche 值、复制重赋、EH 和 callback。单独统计确认每目标接口用例均实际 50 次 full、复制 10,648 B；数组 minor 分别 189／193／194 次，复制 3,287,960／3,148,296／3,518,960 B。没有全量验收或耗时正确性断言。

正常 release，同一源码、编译器、接口布局和四个预编译系统库；四种 runtime 资源各 92 个对象中只有 scan 对象变化。M3 Ultra／LLVM 22.1.8 与 nuc12 i7-12700H／LLVM 22.1.2 实机各一对预热、五对 AB／BA，未固定 CPU affinity。下表为墙钟中位数变化，包含启动和输出；八个程序的 MIR／LIR 在每个目标均逐字节相同。

| 程序 | Darwin ARM64 | GNU x86_64 | musl x86_64 |
| --- | ---: | ---: | ---: |
| strmap | −5.21% | −3.29% | −2.68% |
| strings | −3.46% | −1.49% | −1.51% |
| trees，depth 20 | +0.79% | +0.27% | −1.36% |
| dispatch | +0.59% | −0.44% | −0.87% |
| lambdas | −0.93% | +0.21% | +0.39% |
| List<Long> | +1.01% | −2.10% | −0.36% |
| 接口数组／父投影 | −0.13% | +8.71% | +1.85% |
| class 跨调用使用 | −1.53% | −5.55% | +2.02% |

strmap 的绝对时间分别为 0.983797→0.932550 s、1.427418→1.380523 s、2.015609→1.961616 s。另跑一对 GC 诊断，remembered 阶段降低 13.21%／10.86%／10.39%，引用更新降低 29.24%／12.73%／12.38%；这些单对阶段数据不作统计显著性结论。八个程序在各目标的分配字节、minor／full 次数、复制字节、脏卡、旧代扫描槽及 root slots 均保持。strmap 均为 777,411,704 B 分配、259 次 minor、9 次 full、12,695,360 B 复制、33,414,853 个旧代扫描槽；收益来自减少解释开销，没有少扫描必要引用。

GNU 接口数组初测的五对差异为 −1.43%／+9.04%／+0.57%／+9.48%／+9.85%。实际自然循环两侧都是 63 条指令、9 条栈访存，poll 快路径 5 条。只做一次定向定位：固定 CPU 4（未锁频），同样一对预热、五对交替，原程序 146.807→145.341 ms；临时 phase 探针仅包围 copies 调用，为 143.038→143.372 ms（+0.23%）。原始回退仍保留；固定 affinity 和增加计时边界改变了运行条件，不替代原结果，也不把未改动循环的时差归为 GC 扫描收益。probe 仅在忽略的临时基准中显式链接，没有加入普通产物。

| 成本 | Darwin ARM64 | GNU x86_64 | musl x86_64 |
| --- | ---: | ---: | ---: |
| 八个程序各自 text 增量 | +112 B | +192 B | +256 B |
| 完整文件增量 | 0 | 0 | 0 |
| runtime 冷构建墙钟／CPU | −2.75%／−0.82% | −0.26%／−0.47% | −1.46%／−1.19% |
| 两 consumer 冷编译总墙钟／CPU | +0.26%／−6.39% | +0.89%／+0.78% | +3.87%／+0.84% |
| consumer 每轮最大 RSS | +1.49% | −0.03% | −0.02% |

runtime 与两个 consumer（strmap、接口数组）各测三对独立冷 cache，不宣称编译加速。静态 scan、Scoop stackmap、接口表与系统库保持；Linux 原生 `.eh_frame` 减少 16 B。解释器本身的 native 栈帧增加 ARM64 16 B、x86_64 32 B，固定引用数组则省去了逐元素解释器帧；没有新增 managed 对象、引用、持久 metadata 或 GC roots。

本项作为所有 profile 的默认 runtime 实现，重建 runtime 资源并重新链接即可，无需重建 core。runtime contract 19、metadata 9、对象和 scan ABI 保持，无新选项或诊断代码。复杂 scan 继续共享递归后备，真实根恢复、引用 visitor、边界判断、写屏障及保留对象的扫描仍存在。

原始采样、逐次计时、IR／机器码、GC 和构建记录在两机仓库外 `scoop-performance-results/2026-10-26/remembered-ranges`；源码副本与构建在 `tmp/program-performance/remembered-ranges`。Linux 汇总另复制到本机该记录目录的 `linux-results`，不把原始日志提交到文档目录。

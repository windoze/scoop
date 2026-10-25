# minor 的局部块遍历与连续清卡跳过

本项对应实际 trees／strings／strmap 调研中的全堆枚举问题。新版 runtime 在所有 profile 默认采用；没有新增开关、对象字段、GC roots 或诊断代码。runtime ABI 16、metadata ABI 9、TLAB 与 `.slib` 均保持。

## 实现范围

refill 在原 heap lock 内将年轻 block 加入私有清单，普通对象的 bump／登记不增加操作。minor 的标记准备、worker 存活字节汇总和搬迁 source 选择只枚举年轻块；新建的 evacuation target 单独记录，收尾只处理年轻块和这些目标。预留失败时先释放本轮目标，再恢复 source，仍从完整原图转 full。

具有未使用 block 的普通 region 单独记录，补充 nursery／搬迁目标不再为找到一个可用 block 扫描全部已满 region。full 按原 source 排除规则选择并重建可用清单，保持目录顺序与保留空 region 的优先复用。大对象与 release-hook 对象仍预晋升，minor 不产生需要归还的大对象 mapping，因此不再执行 region 回收／VM 归还枚举。

full 仍遍历完整堆、执行 release 与回收。年轻清单和目标清单在引用更新、晋升／退役完成后清空，早于 region metadata 撤销。活动年轻块／目标块与空闲块按互斥状态复用同一个链接字段，退役前取得后继；block 大小及原有热字段偏移保持。region 的 generated-code prefix 仍为 base／size／cards，偏移 0／8／16。

卡表和写屏障的布局与语义保持。remembered 扫描遇到清卡时，用 `memchr` 找到后续首个值为 1 的卡，批量跳过连续清卡；脏卡顺序、包含跨卡对象起点的扫描范围、计数和引用回写保持。卡字节仍只有 0／1，扫描发生在 STW，没有并发写入。

remembered 扫描及收尾清卡仍枚举 region／卡表，不能据此宣称 minor 已完全消除与旧堆大小成正比的工作。survivor／年龄阈值也尚未加入。

## 定向正确性验证

格式化、workspace Clippy 与改动 C 文件的 `-Wall -Wextra -Werror` 检查通过。本机和 Linux 各运行同一组 21 项 runtime 测试：11 项 nursery、8 项 region，加并行标记和 moving collector。覆盖多 region、真实大对象、pin、旧代脏引用、重复块复用、部分晋升预留失败后转 full、并发 mutator、callback／冻结栈段 roots、region 压缩和 VM 归还。新增清单测试在测试进程内交叉核对年轻／可用 region 成员，检查完整回收后无遗留成员；没有给生产 collector 增加交叉扫描或计数。

首次对照发现 GNU strmap 回退后，从机器码确认 144→152 B 的 block stride 使 x86 block 寻址由 LEA／移位变成乘法。最终复用互斥状态的链接，恢复 144 B。Darwin 的逐字节清卡循环在代码位置变化后的版本中出现约两倍扫描耗时，引用回写时再扫一次放大了差异；同一组循环指令的反汇编与阶段数据将问题收窄到清卡空扫，随后改为批量跳过。

收尾修改只复验受影响的清单生命周期、晋升回退、region 压缩和写屏障测试；清卡跳过复用已有稀疏数组、无引用大对象与并发 mutator 覆盖，确认相隔较远的两张脏卡、连续清卡及重复收集仍保留并回写年轻引用。没有重复完整验收或新增生产诊断代码。

## 实际程序对照

使用用户原始 trees（`maxDepth = 20`）、strings、strmap；较短的 depth 16 版本仅用于前期排查，不替代下表原始负载。每个程序、目标先预热一对，再交错测量三对，独立核对输出。普通计时不启用 GC 诊断，另运行一对诊断。表中为墙钟秒数中位数及相对变化，负值表示耗时减少。

| 程序 | Darwin ARM64 | Linux GNU x86_64 | Linux musl x86_64 |
| --- | --- | --- | --- |
| trees | 14.117 → 12.996（−7.94%） | 18.648 → 16.408（−12.01%） | 23.481 → 20.719（−11.76%） |
| strings | 0.477 → 0.481（+0.98%） | 0.494 → 0.468（−5.35%） | 0.955 → 0.935（−2.10%） |
| strmap | 3.411 → 3.337（−2.20%） | 3.889 → 3.818（−1.81%） | 5.834 → 5.790（−0.75%） |

trees 在三个目标的每一对测量均获益。Darwin strings 保留约 1% 的回退；Darwin strmap 有一对回退，其小幅中位数改善不视为稳定收益。未对特定程序或函数选择布局／开关。

独立诊断中，trees 的 remembered 扫描分别从 0.561／0.548／0.569 秒降到 0.059／0.060／0.116 秒；GC 总暂停累计分别从 12.301／16.513／21.279 秒降到 10.529／14.153／18.304 秒。分配量相同，但原始树负载的 full 次数与晋升量随布局／并行次序略有变化，不将全部差值归因于某一个循环。strings、strmap 的 minor／full 次数和晋升量与各自基线保持。原始 trees 峰值 RSS 的中位数变化约为 +0.29%／−0.32%／−8.43%，不据此承诺普遍内存降幅。

## 空间与兼容性

64-bit 目标的 block 元数据仍为 144 B，不增加每块存储；每个 region 增加 8 B，可用 region 清单复用原 allocation-region 指针，heap 全局状态增加两个清单头，共 16 B。对象头仍为 16 B，接口值及所有生成代码共享视图不变。

基准使用同一编译器、相同应用和预编译系统库，只替换 runtime 重新链接；应用与依赖 `.slib` 身份和指纹相同，应用缓存目录因构建记录更新而不同。不增加应用编译分析，也不要求重编系统库。四种既有 runtime 配置沿正常准备流程更新。

三个原始程序的 text 增量分别为 Darwin −28 B、GNU +768 B、musl +704～768 B；静态接口／stackmap 等应用记录不变。完整文件在 Darwin 不变，GNU 增加 16 B，musl 的 trees 增加 24 B、strings／strmap 因布局对齐增加 4,120 B。

相同 runtime builder、工具链与 vendor 输入使用独立空输出缓存，交错冷构建三对普通 runtime，墙钟中位数分别为 4.549→4.421 秒（−2.80%）、4.958→5.024 秒（+1.34%）、5.474→5.516 秒（+0.77%）。应用编译阶段未改变；上述小幅构建差异不视为编译优化收益。三目标均发布普通、stress、startup-stats 及两者组合的四份 runtime，默认 sysroot 消费者复用同内容应用产物，运行输出正确。

原始输入只读自用户的 `/tmp/scoop-bench`；副本、初版与最终布局的产物、对照日志和报告保存在两机仓库忽略的 `tmp/program-performance/minor-blocks/`。

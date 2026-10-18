# M56 实施记录

状态：实施中，2026-10-19 开始。范围与完成条件以 [DESIGN.md](DESIGN.md) 第 9、10 节为准；未完成项不得以设计或历史性能数据代替验收。

## 交付检查表

| 项目 | 状态 | 验证／取舍 |
| --- | --- | --- |
| A：当前 release 工具、开发 sysroot 与真实程序基线 | 进行中 | 基于 `28fab1812` 构建；设计阶段的机制实验不作为应用基线 |
| B1：内部 TD 验证结果复用、native 边界保留 | 待完成 | 尺寸、溢出、对齐与非法输入；完整分配路径对照 |
| B2：unbox／clone 单次对象定位 | 待完成 | 类型匹配、对象边界、移动与含引用值 |
| B3：String 有界表示 intrinsic | 待完成 | byteLength、byte-at、cursor 解码；UTF-8、越界与 GC 组合 |
| C1：NoGC／无 site 循环优化及 Managed 保守优化 | 待完成 | 实际向量化、唯一 site／roots、EH／原子与响应 |
| C2：局部 card 查询／mark 复用 | 待完成 | 同 card／跨 card、同步边界、多 mutator、minor／moving |
| D1：nursery／refill 对照与策略取舍 | 待完成 | 1／4／8／16 MiB，存活率与 1–8 mutator；吞吐／内存／停顿 |
| D2：有搬迁 GC 并行 update 候选 | 待完成 | 串行 claim、槽分区、完整阶段屏障；单／多 worker 同义性及净收益 |
| E：声明构建、局部 box／协程消除、跨 Cone 内联 | 待评估 | 逐项记录设计规定的进入条件与实测，不以无依据延期替代调查 |
| 本机阶段验证 | 待完成 | 每批先格式化／lint，相关 crate／runtime 与正式 CLI fixtures |
| Linux 完整验收 | 待完成 | nuc12：workspace、fixture runner 规则和完整 CLI；GNU／musl 合同 |
| 逐功能提交、临时资料与 target 清理 | 进行中 | 只提交本任务变更，原始日志在仓库外，不使用绝对 `/tmp` |

## 工作环境与验证策略

本机 Apple M4 Pro／macOS 27.0.1，LLVM 22.1.8；Linux 使用 `nuc12:~/repos/scoop`。开始时远端工作区干净，HEAD 为 `3cbc339f8`，完整验收前同步本次实现。开始时本机 target 为 4.8 GiB、远端 target 为 1.6 GiB；按阶段清理无用构建产物，保留当前有效增量结果，避免每个小改动反复全量构建。

仓库外工作目录为 `../scoop-work/m56-20261019`，包含 scratch、基线／候选测量与原始日志；命令显式设置 TMPDIR。性能样本先完成构建，再串行交替运行。正常应用使用显式准备的开发 sysroot，runtime 专测继续用已有源码构建测试入口。

已有的 M47 两份文档和 M55 设计不属于本次提交。初始内容摘要保存在仓库外，结束时核对；不使用全仓 reset／checkout，也不删除／隐藏源码或工具来验收缓存。

## 实施与验收记录

尚无功能提交或验收结果。每项完成后在此补充具体实现、相关验证、性能结论与剩余限制；原始逐次输出不进入仓库。

# M58 LLVM codegen 优化

## 基线与范围

基线 `bc806046b`，本机 `codex/m58`。已有 codegen 按 callable 拆 LLVM module；machine release O2 与普通 IR 管线分别配置。NoGc 资格同时要求 typed effect 和实际 safepoint 集为空。M58 已完成的 MIR 装箱／closure／tail suspend 优化保留，避免把前端收益算入新增 LLVM pass。

Managed 已有受限 GVN（关闭 PRE、load PRE、backedge load PRE）、BDCE、DSE、ADCE、MemorySSA early-CSE、SCCP；NoGc 原来只有初始表达式化简、循环规范化、LICM、indvars、loop／SLP vectorization 及收尾。两者均缺少 constraint-elimination、correlated-propagation、memcpyopt、aggressive-instcombine、vector-combine。

## 分批实施

1. 两条管线补范围推导；NoGc 复用既有受限 GVN 等标量优化。顺序为 SROA／mem2reg → InstCombine／early-CSE／reassociate → constraint-elimination／correlated-propagation／SCCP → InstCombine／受限 GVN／BDCE／DSE／ADCE／MemorySSA early-CSE／不可达清理。NoGc 再运行原有循环与向量阶段。范围推导后明确清理常量分支与失败路径，避免整条管线重复迭代。
2. 评估 memcpyopt 与 SROA、DSE 的配合，先覆盖 by-value 中间副本。含引用复制必须保持 AS1 provenance、独立值存储、根恢复、屏障和发布；若缺少可证明的效果信息，仅补局部属性，不把 NoGc 当 pure。
3. aggressive-instcombine 只评估 NoGc；vector-combine 放在既有 loop／SLP 后。浮点仍严格执行 NaN、signed zero 与原有舍入／结合顺序。
4. loop-idiom、loop-load-elim、小规模 unroll、jump-threading、simple-loop-unswitch 逐项测量。只在 typed NoGc 且实际无 safepoint 的函数运行；收益不足或编译时间／体积回退时保持关闭。

Managed 不新增会复制 site 的 CFG／循环 pass；普通优化先于最终根计划和 RS4GC。D 的有界内层必须另有真实结构分析，不能仅依赖 loop metadata，不能把整个 Managed callable 改标 NoGc。本项不增加 LTO、跨过程摘要或通用 pass 配置框架。

## 验证与对照

少量源码 fixture 覆盖分支范围、整数边界、集合访问、按值副本、严格浮点；精确 LLVM 测试只锁定实际优化、唯一 site 和 roots 合同。Managed 组合真实移动 GC、含引用聚合、异常和 poll 回归。先 fmt／clippy，后相关 Rust／CLI 测试。

在同机、同输入、同 compiler 构建配置、同 app release 与 sysroot 下交替运行前后可执行文件，记录耗时分布、编译时间和 text／文件大小。编译测量使用独立输出并复用同一预备 sysroot，应用构建缓存命中不计作编译加速。纯 IR 变化与端到端收益分开报告；本批不改变 poll 位置，若 D 改变频率再测 stop-wait 尾部。原始日志和临时实验放仓库外，最终移除实验开关及产物。

实现与测量结果随独立提交补充；未通过验证或没有收益的候选不写成默认启用。

## 第一批：范围与 NoGc 标量补齐

已按上述顺序接入两条 release 管线。保留原受限 GVN，不扩大 PRE；Managed 不新增 SimplifyCFG。后续循环 pass 仍只供 typed NoGc 且无实际 site 的 callable 使用。

本机 fmt／workspace clippy、16 项 statepoint 专测、新范围源码三变体、M56 循环七变体通过。精确测试确认传递范围删除不可达失败 site、活跃 site 唯一且保留一条根及 volatile restore；NoGc 的已知小整数范围消除多余算术移位。既有专测同时覆盖三目标的 byval、AS1 聚合复制、invoke、原子引用和终结 poll 的实际机器输出。源码移动模式覆盖 GC 后快照与 catch，以及既有原子发布／循环异常路径。

实际内核的首轮运行时间没有超出噪声的收益，详见 [性能记录](PERFORMANCE.md#5-llvm-管线对照)。此批确认局部范围推导能力及 NoGc 标量覆盖，不宣称程序普遍加速，也不据此扩大 GVN 配置。

实现核对使用本机 LLVM 22.1.8 headers 及对应版本的 [ConstraintElimination](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp)、[CorrelatedValuePropagation](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp)。两者使用支配条件／范围事实化简；新管线不主动附加 fast-math 或无溢出假设。

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

## Native 辅助符号的链接边界

loop-idiom 的 Darwin 实验生成了 `bzero`，暴露出 `.slib` 生产路径要求所有 native 引用预先登记的错误限制。删除该拒绝分支；未匹配已知分类的引用直接保存符号名与重定位，由最终 linker 解析。已有 helper 表保留已知 ABI 分类用途，不作为白名单，也不添加 `bzero` 特判。正文编码保留 native 目标名字。测试验证普通引用可保存、读回及区分目标；不要求编译器预判辅助函数是否存在。

本机 fmt／workspace clippy 与 141 项对象产物相关测试通过；原触发 Darwin `bzero` 的实际程序已正常编译、链接，fill／prefix／small／branch 四模式输出均与基线一致。动态 undefined 表保留 `_bzero`，由系统库正常解析。

## 第二批：按值复制

两条 release 标量管线在 BDCE 后增加 memcpyopt，随后由 DSE／ADCE 清理，前置 SROA 保持。没有新增内存效果属性。精确测试覆盖 byval 中间副本转发，以及源值在消费前被修改时必须保留快照；Managed 同时保留实际 site。已有跨三目标测试继续覆盖含 AS1 引用的聚合复制、volatile 根恢复、invoke 和机器 site 唯一性。

fmt／workspace clippy、17 项 statepoint 专测与新源码 debug／release／moving 三变体通过。源码组合大 struct 参数取址与独立值副本、tuple、old→young 数组写入、clone、GC 后返回和 catch。程序对照的主要收益是综合内核 text 减少 616 bytes；同 Cone 值复制及跨 Cone byval 的运行时间均没有可信提升，详见 [性能记录](PERFORMANCE.md#6-复制与循环候选)。不把单条 memcpy 消失解释为通用运行加速。

## 第三批：NoGc 循环

仅在原有 typed NoGc 且无实际 site 的分支接入三个候选：loop-rotate／LICM → simple-loop-unswitch（trivial／nontrivial，保留默认增长阈值）→ indvars → loop-load-elim → 最多四次的完整 unroll → 既有 loop／SLP vectorization → 原收尾。unroll 使用 O2 的局部成本选择，关闭 partial、runtime、upper-bound 和两类 peeling，不采用完整 default<O2> 管线。严格浮点与 Managed 管线保持。

逐项和组合的实际程序均有相应收益：递推加载约三倍、固定四步处理约两倍、不变分支约 20% 加速。组合 text 增加 256 bytes，循环程序编译时间中位数约 +0.4%；数据、规模与局限见性能记录。jump-threading 虽加速两个标量内核，却使分支内核回退约 8%；与尚无足够收益的 aggressive-instcombine、vector-combine、loop-idiom 一起保持关闭。Darwin helper 已可正常交给 linker，因此 loop-idiom 的关闭只依据当前性能结果。

fmt／workspace clippy、18 项 statepoint 专测通过。新增精确测试检查循环中的 load 被转到非循环块、四次回边消失、不变 flag 分支移出循环，并对照 Managed 定义保留这些结构；不依赖块名或完整 IR 快照。源码覆盖零／一次／短／较长循环、边界外哨兵、四步变换、两种分支及移动后的对象内容。M58 三项源码九变体与 M56 原有循环七变体通过，包含 alias、严格浮点、真实强制移动 GC、minor、异常和原子引用。临时 pass 环境变量已从生产代码删除。Linux 全量验收随 M58 后续功能统一完成。

## 最终根计划前的常量控制流收尾

重建完整 sysroot 时，`scoop.http.asyncio` 在 Darwin／GNU 同时暴露 MissingStackmap。隔离后的实际函数在 GVN／MemorySSA early-CSE 后有两个常量 false 分支；单独 unreachableblockelim 没有折叠其终结指令。最终根计划仍登记 64 个 site，而 RS4GC 随后的死路径清理只留下 60 个，导致描述与对象不一致。

Managed release 在全部普通优化（含合格内层 SIMD）之后、最终根计划之前追加一次 SCCP／unreachableblockelim；NoGc 的既有 CFG 收尾与 debug 保持。不放开一般 SimplifyCFG，不在 roots／RS4GC 后增加普通优化，不删除产物对实际 stackmap 的必要一致性检查。该次 SCCP 的消费者是后段标量变换新暴露的常量分支，不重跑整套标量管线。

精确 IR 以同一对象的两个不相交字段复现：第一个字段决定分支，写第二字段后再读第一个；后段 GVN 才将失败条件折为 false。三目标的 debug／release 同时断言剩余 site、volatile 根恢复、gc.relocate 和机器 stackmap 数量。源码扩展 ranges fixture，组合相邻字段、真正改变被检查字段后的异常和实际移动 GC。21 项 statepoint 测试通过，原完整 HTTP asyncio 库已成功构建；平台整体验收继续执行。诊断用 LLVM dump 改动已恢复。

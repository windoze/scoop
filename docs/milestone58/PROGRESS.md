# M58 实施与验收记录

状态：实施中。分支 `codex/m58`，主仓库 `/Users/chenxu/repos/scoop`；保留工作区中其他任务的 M47／M57 未提交内容，不纳入 M58 提交。按 [DESIGN.md](DESIGN.md) 完成全部 A–D 后再做整体验收；当前结果不代表里程碑完成。

## 分项状态

| 功能 | 当前状态与证据 |
| --- | --- |
| A：年轻 small block 整块退役 | 已实现；本机 GC 专测 32 项、新源码 fixture 四个变体通过，Linux 待验证 |
| A：无引用大对象跳过 remembered scan | 已实现；本机 nursery 专测 10 项、新源码 fixture 三个变体通过，Linux 待验证 |
| A：full 必要容量早判 | 已实现；本机 region 专测 7 项、新源码 fixture 三个变体通过，Linux 待验证 |
| A：starts bitmap 枚举 | 已实现；本机 GC 专测 35 项、M58 runtime 四个源码 fixture／13 变体通过，Linux 待验证 |
| B：统一优化迭代 | 已实现三轮上限与无变化早停；本机 MIR 单测 122 项、新旧优化源码回归通过，Linux 待验证 |
| B：函数内来源／逃逸分析与 box 消除 | 已实现，接入同一迭代；本机 typed MIR 专测与源码／移动 GC 回归通过，Linux 待验证 |
| B：closure 实例消除 | 已实现，复用来源／循环分析；本机五项 typed MIR 专测、源码与移动 GC 回归通过，Linux 待验证 |
| B：closure 私有支持产物清理 | 已实现 LIR 准备时的实际引用选择；本机 160 项 LIR 单测、源码／跨库／callback 回归通过，Linux 待验证 |
| C：tail completion 转交 | 已实现 release 本地直接调用的整函数无帧转交；typed MIR、普通／移动 GC／跨线程源码回归通过，Linux 待验证 |
| C：状态机生成前同步内联 | 已实现本地同步正文与 call-site 同步改写；19 项协程、23 项优化单测及源码组合通过。四层包装运行约 −32%、分配约 −74.5%、text −21 KB，Linux 待验证 |
| D：外层 poll 循环分块、受限内层 SIMD | 已实现标量分块及实际结构限定的整数 SIMD；默认 64 次，本机并发 moving／minor、三目标精确 IR／site／根／EH 通过。checksum 约 −48%，text +212 bytes，stop-wait p99 未见明显回退；Linux 待运行验收 |
| LLVM codegen：范围与 NoGc 标量补齐 | 本机实现与相关验证通过；实际内核暂无可信运行时间收益，后续候选逐项评估 |
| LLVM codegen：按值复制 | 两条 release 管线增加 memcpyopt；17 项 statepoint 专测及源码三变体通过，text −616 bytes，运行与编译时间基本持平 |
| LLVM codegen：NoGc 循环 | 启用 load elimination、最多四次完整 unroll、受限 unswitch；18 项 statepoint 专测、源码九变体及旧循环七变体通过。实际递推／四步处理／不变分支分别约 3×／2×／20% 收益，循环 text +256 bytes，编译约 +0.4% |
| native helper 链接边界 | 已删除未登记外部符号的编译期拒绝，交由 linker 解析；141 项对象产物回归与真实 Darwin loop-idiom 程序通过 |
| runtime C → LLVM IR 小函数 | String 有界读取已实现，本机精确 IR 与源码十一变体通过；重建两版 core 后字符遍历约 −7.6%，byteLength 单项未见明确收益。静态 unbox 已完成，值／含引用扫描分别约 −19.4%／−28.7%；数组 clone 已完成，小数组约 −15%，stop-wait 未见稳定尾部回退；见 [迁移记录](RUNTIME-IR.md) |
| 性能与跨平台 | A 的本机生产前后矩阵已记录于 [PERFORMANCE.md](PERFORMANCE.md)；A–D 本机性能矩阵已完成；GNU／musl 完整验收及最终清理待完成 |

## 2026-10-20：LLVM 范围推导与 NoGc 标量补齐

先核对 `bc806046b` 的现有管线及 M58 完成范围，再修改实现规范 2.19。两条 release 管线增加 constraint-elimination／correlated-propagation，并安排 SCCP／InstCombine 与不可达清理；NoGc 补齐既有受限 GVN、BDCE、MemorySSA early-CSE，随后才执行原循环／向量阶段。没有打开 default<O2>、PRE 或 Managed 循环复制。

fmt、workspace clippy、16 项 statepoint 专测、三个新源码变体及七个既有循环变体通过。真实 moving GC、含引用快照、catch、原子引用、严格浮点和唯一 site 均包含在覆盖内。原始日志位于仓库外；首轮运行对照未发现可信加速，范围推导能力与端到端结果分别记录于 [LLVM-CODEGEN.md](LLVM-CODEGEN.md) 和 [PERFORMANCE.md](PERFORMANCE.md)。后续 memcpy／向量及循环候选仍待评估，M58 整体未完成。

## 2026-10-20：年轻 block 退役

先补 runtime 规范 3.9，再在收尾阶段为普通 minor 的全死／已搬空年轻 small block 使用现有 block 退役流程。先统计晋升量；源存储已完成回写与验证才释放 forwarding，复用依赖现有 activation 清空元数据。含 pin 的存活 block、evacuation target、release-hook 旧对象和 moving stress 保持原路径。

新增 C 专测反复检查全死块与混合存活源块的退役／复用、旧起点不可见、晋升计数和链内容；新增正式源码 fixture 以持续分配和 old→young 链覆盖 debug／release 及普通／stress minor。运行参数显式选择当前 runtime 源码，避免误测旧 sysroot runtime。

本机验证：C 格式化与 `-Wall -Wextra -Werror` 语法检查、`cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；`scoop-codegen` 的 GC 专测 32 项通过，包含 pin／handle、release、移动 stress、并行回写、晋升失败和线程握手；新 fixture 1 项／4 变体／12 进程通过。最初 fixture 使用 Option 结构比较触发 Node 缺少 equals 的诊断，已改为模式匹配并闭合。

构建与原始验证输出位于仓库外 `/Users/chenxu/repos/scoop-work/m58-validation`；临时目录显式使用仓库的 `tmp/m58/tmp`，不使用 `/tmp`。开始时定向 `cargo clean -p scoop-codegen` 释放约 1.4 GiB，保留可复用依赖。后续继续按实际磁盘占用清理，Linux 尚未开始。

## 2026-10-20：跳过无引用大对象的卡表

在 shared remembered 枚举入口读取已发布旧 large head 的 TD；没有 object scan 时不遍历其卡表。标记与回写共用此路径，卡表分配／屏障／清零和对象本身的保活规则保持原样。

新增 C 专测将 1 MiB 无引用对象全部标脏，与真正含引用的 large array 同轮收集；四轮均只计入两个必要脏卡、128 个旧引用槽，两个年轻对象正常搬迁、内容和晋升统计保持。正式源码 fixture 组合大值数组的 scoped pointer borrow、native 范围屏障和旧引用数组，使用当前 runtime 源码。

格式化、C 严格语法检查、workspace clippy 通过；nursery 专测 10 项、新 fixture 1 项／3 变体／9 进程通过。fixture 的回调补上显式 unsafe 作用域；无引用存储使用 Array 的 scoped borrow，避免当前显式 MutableArray 借用重载解析到 Array 的无关诊断。没有修改语言或重载行为，目标运行时路径由 C 专测和源码闭环共同覆盖。性能测量留到 GC 改动完成后统一对照。

## 2026-10-20：full 容量早判

在普通 full 的单 source 情况下，使用现有 region 存活统计与普通 block 容量，计算已占用目标 region 的可用容量上界。明显不足时恢复 source 状态，直接做非移动收尾；通过时仍使用实际预留／回滚。没有新增堆列表、对象扫描、公开参数或 ABI。

C 专测在真实 STW、mark 与 plan 边界检查：两个 block 的 source 无法放入仅一个 block 的容量时，目标死亡对象仍未 release、起点仍存在；正常收尾只 release 一次。额外释放两个目标 block 后，同一 source 的两个对象成功搬迁，引用和内容保持。正式源码 fixture 用 55 万个含引用节点、共享尾节点和大引用数组，覆盖密集全存活、稀疏回收及再次 full 的图正确性。

C 格式化／严格语法检查、Rust fmt／workspace clippy 通过；region 专测 7 项通过，包含失败回滚、pin、discard、region 压缩；源码 fixture 1 项／3 变体／3 进程通过，覆盖 debug、release 与四 worker。此处确认正确性和提前退出路径，尚未给出暂停加速比。清理约 16 GiB 的 `target/debug/incremental`，保留依赖与正式 CLI 工具。

## 2026-10-20：按 starts 位图枚举

small block 收尾按 starts 置位枚举对象；目标准备直接枚举 `starts & ~marks`。每次取得 bitmap word 快照后清位，维持地址顺序与原有 release／pin／forwarding／size／line 逻辑。没有增加对象列表或持久索引。

新增 C 专测覆盖同 word 多起点、相邻 word、空 word、block 末尾的 24-byte 对象、含 pin 的目标与另一 source region；压缩前后的存活资源内容保持，全部资源只 release 一次。独立源码 fixture 以 512 个 release 资源保留稀疏子集，覆盖普通 full 与移动 stress 的生命周期。

C 格式化／严格语法检查、Rust fmt／workspace clippy 通过；全部 GC 专测 35 项通过。M58 runtime 的四个源码 fixture 共 13 变体／33 进程通过，包含前面三项改动的组合回归。A 的四项实现已分别闭合本机正确性验证；生产性能对照与 Linux 验证尚未完成。

## 2026-10-20：A 的生产对照

固定编译器，仅替换 `c3b8942db`／`8df65ae17` runtime，使用真实 Scoop 编译运行的 full、minor 与连续分配程序。五对独立进程、事件级 full 100 样本／minor 250 样本，另做预热后的七对连续分配；方法、阶段、RSS、原始离群值处理和限制见 [PERFORMANCE.md](PERFORMANCE.md)。四 worker 下，90 万节点的失败压缩准备 plan 5.495 → 0.074 ms、总暂停 p50 20.788 → 11.729 ms；真实稀疏压缩 27.764 → 20.480 ms，仍有长暂停。四 worker、128 MiB 无引用旧数组的 minor p50 814 → 413 µs。连续分配总时间 105.234 → 67.775 ms，主要由累计暂停减少解释。

全部测试输出与临时 benchmark 副本位于仓库外；已清理约 1.8 GiB 的已完成 fixture 产物。Linux、多个 mutator 的响应尾部、GUI 帧循环及 B–D 性能仍待完成。

## 2026-10-20：统一普通优化迭代

release 使用最多三轮 devirt／inline／simplify，实际无变化时早停。内联的稳定模板、原始成本、caller 剩余增长及展开祖先跨轮保留；当前直接调用图每轮刷新递归判断，隐藏递归另受实际展开祖先约束。只重新分析上轮改变的函数，LocalValueRegistry 在整个阶段内共用，结束时一次写回。没有新增持久 IR／产物字段或通用 pass 框架。

新增 typed MIR 专测区分一轮仍间接、两轮已展开、第三轮观察到固定点；12 轮测试验证累计增长与六层深度不重置、开放 receiver 中隐藏的递归保留，以及新 exact receiver 暴露的直接递归触发重新判断。独立源码 fixture 组合 interface helper、两层 closure 包装、参数副作用、可变字段、实际递归与移动 GC；此时 closure 分配仍保留，环境消除属于下一项。

Rust fmt、workspace clippy、MIR lowering 全部 122 项单测通过；8 项直接覆盖普通优化。新源码 fixture 3 变体／3 进程；M34 内联五项 10 变体／44 进程、M31 优化五项 12 变体／38 进程通过。M34 去虚拟化中，跨库与 slots 两项先通过；locals 的 release MIR 因新增展开和常量分支化简更新已有优化专测期望，debug 期望保持，复跑两变体／8 进程通过。隐藏递归单测原先预期停在 method，实际由展开祖先停在 helper；修正预期并单独增加新直接 SCC 的专测，二者均闭合。

这笔仅完成有实际去虚拟化／内联消费者的迭代功能，不代表 B 的分配消除或 C 的协程前置内联已完成。GC benchmark 的临时源码、基线 runtime、可执行文件与缓存已清理；保留文档概要和仓库外的小型验证日志，完成的优化 fixture 产物继续定期清理。

## 2026-10-20：函数内来源分析与 box 替换

先补实现规范 2.19，在每轮 inline／simplify 后加入普通局部 box 消费者。只登记 ValDecl 根部的 typed Box 创建点，以稀疏队列传播 Local／Retype 的来源并集；外来值、取地址和多个候选汇合均保留已有候选。创建支配检查和回边 liveness 区分循环内多个动态实例与循环外一次创建。所有引用使用均有明确替换时，才在原创建点保存 payload 快照、替换 exact 拆箱、折叠 boxed 继承／itable 决定的检查，并删除引用别名。含引用快照正常进入 LIR roots，不增加持久 IR、栈对象或 GC ABI。

未内联调用、identity、存储／聚合嵌入、地址、非 exact 拆箱及 EH 保守保留；整型 enum 的父整数拆箱保留实际对象。类型检查可以先化简失败／成功路径，再在下一轮依据剩余使用消除 box。MIR visitor 复用原 block 遍历，拆出实际需要的 statement／terminator 只读入口；分析、控制流与改写按职责分模块。

Rust fmt／workspace clippy 通过；MIR 247 项、MIR lowering 131 项全量单测通过，追加 interface 交替专测后装箱专测共 10 项通过。专测覆盖快照顺序、来源合流、未知使用、地址、创建支配、循环、失败路径，以及第一轮仍保留接口调用／box、后续 devirt／inline 后实际删除。新源码两个 fixture 共六变体／六进程通过，覆盖 debug／release／moving、值重赋值、参数副作用、interface、enum、ZST identity、嵌套引用、泛型与跨 GC 的根。既有 M58 迭代三变体、M34 去虚拟化六变体／28 进程／六 golden、M37 enum boxing 两变体／六进程、M56 unbox／clone 四变体均通过。

最初测试按未规范化的 return 编写结构断言，已改为检查实际返回临时值；源码用例按现行 immutable struct、泛型函数语法及 smart cast 规则修正。所有失败均闭合，没有改变语言规则或更新既有 golden。清理约 6 GiB incremental 与已完成 fixture 产物，日志仍放仓库外。B 的生产性能对照、closure 消除、协程与循环分块继续实施；本节不宣称新的分配吞吐或暂停倍率。

## 2026-10-20：closure 捕获替换与实例消除

同一 pass 以明确的 Box／Closure 两种候选共用来源、创建支配、循环携带与保守使用判断。invoke 展开后，匹配 exact closure class 与捕获字段类型，以原创建点的 typed 捕获快照替换 ClosureCapture。快照按原 captures 求值顺序发出，独立记录物理 field index；无捕获环境直接删除，不生成占位局部值。只折叠已知的 Any／exact 函数类型检查，其余函数关系保留。没有删除 class、invoke、bridge 语义记录，物理支持清理仍是下一项。

新增五项 typed MIR 专测验证：一轮 inline 后环境仍在、第二轮 devirt／inline 后删除；捕获 box 在环境删除后第三轮继续消失；无捕获环境、identity 保留与反转字段顺序。测试 helper 补齐实际 lambda lowering 的 hidden receiver 后全部闭合。fmt／workspace clippy、23 项普通优化专测及 MIR lowering 全部 137 项单测通过。

新源码覆盖零／一／17 次高阶调用、普通／泛型 scope 包装、含引用值捕获、循环体内 GC、bound receiver 单次求值、同 closure class 的逃逸 sibling、外层 scope 与被保存的内层回调，以及 identity。M58 优化四个 fixture 共 12 变体／12 进程通过；M37 局部类型 closure 与 inner closure 各两变体／六进程、M34 内联语义两变体／八进程均通过。本机闭环未出现根或运行结果回归；Linux、生产分配／耗时／footprint 矩阵仍待统一验证，不将实例消失等同于私有 TD／layout 已不发射。

## 2026-10-20：closure 私有物理支持选择

先修订实现规范 2.3／2.19。在 LIR 准备中，以实际发射的普通函数、入口、初始化、dispatch、callback 与协程协议为根，沿调用和环境引用选择普通 closure 的支持。仅允许省略关联的生成 invoke／bridge；同 class 的剩余实例、捕获读取和直接 helper 引用保留支持，无根的 helper 环不自行保活。未选环境的 TD、对象／值 layout、扫描描述及私有 callable 不进入 LIR，函数类型检查仍保留所需签名 TD；MIR 完整目录保持。suspend closure 暂保留，不扩展为任意源码定义 DCE。

新增六项 typed LIR 专测覆盖消失、剩余实例、invoke／bridge 直接引用、签名检查独立存活、dispatch 和 suspend 保留。旧的 nominal descriptor 身份测试增加真实 closure 创建，继续验证六个同显示名／不同 exact identity 的描述符。测试发现通用 generated descriptor 补齐也需尊重筛选，已修正；测试 helper 的 boxed Boolean 支持补齐后全部闭合。

fmt／workspace clippy、LIR 全部 160 项单测通过，正式 CLI 重建后，M58 优化五个 fixture／13 变体／15 进程、跨库 callable 组合一项／五进程、managed native callback 两变体／八进程通过。新的小源码专测同时检查 LIR 无私有 lambda 环境，并执行普通／移动 GC 路径。清理约 2.7 GiB 的本轮 fixture 产物，保留仓库外日志。此处确认物理支持确实不发射，B 的生产分配／耗时／footprint 对照仍待完成。

## 2026-10-20：tail suspend completion 转交

先补实现规范 2.3。release 在完整 suspend ABI 建立后、状态机生成前，检查整函数的全部 site：只接受同结果 exact type、同 completion 类型的本地直接调用，调用后只经未取地址的局部复制和无条件边返回结果；EH、pending cleanup、结果处理及不闭合目标保留原路径。合格调用追加原 completion，使用新的 typed step local 原样返回；普通值分支继续返回 Completed，不生成自己的 frame／adapter。新增 TailForwarding lowering 分支，非尾部 caller 仍将其视为可能挂起。

五项 typed MIR 专测覆盖四层尾包装、Unit、debug 保留、非尾部前序／结果处理，以及调用无帧 wrapper 的非尾部 caller 仍有状态机。新 local 保留完整身份。fmt／workspace clippy 通过；MIR 247 项、MIR lowering 141 项全量单测通过，追加非尾部 caller 专测后五项 tail 专测通过。正式 CLI 重建后，两个新源码 fixture 共五变体／13 进程通过；补充 Unit 与结果丢弃反例后，普通／release／moving 三变体复跑通过。

源码覆盖多分支尾调用、直接普通返回、泛型、参数单次求值、立即完成、真正挂起后的成功／失败及异常对象身份、同步恢复、register 完成后抛错、重复完成、catch／finally／Context restore。独立 native 组合通过真实线程恢复和移动 GC，验证尾包装前分配的 payload 在 child frame 中保活、失败只完成一次。测试 helper 的 local owner 断言按现有 typed API 修正，未留下失败。清理约 1.6 GiB 的本轮 fixture 产物；保留仓库外日志。前置同步内联、B–D 性能及 Linux 完整验证仍待完成，不将无帧直接等同于暂停时长改善。

## 2026-10-20：静态拆箱 IR 快路径

先修订 runtime 2.3 和实现规范，复用 LIR 已选 boxed shape 的 payload offset，TD 相等时按完整大小复制到独立 typed local；保留 AS1、packed alignment、padding、引用根和 ZST。非 exact 的 enum 父整数及错误情况复用原 C leaf，源码 null／cast 检查不变。审查中将 aggregate load/store 调整为完整 memcpy，避免遗漏可观察 padding。

fmt、workspace clippy、LIR descriptor 两项、codegen boxing 五项通过。新源码三变体与既有 roots／unbox-clone 七变体通过，覆盖实际移动、minor、嵌套引用、packed、ZST、enum 父整数、异常和 padding 字节。对照 `564be9489`，4096 个逃逸 box 扫描 1024 次的两个程序分别约 −19.4%／−28.7%，text +116 bytes，文件大小不变，编译时间基本持平；详见性能记录第 8 节。相关源码、精确断言与 benchmark 保留，临时缓存和应用产物清理，原始日志保存在仓库外。

## 2026-10-20：静态数组 clone IR 展开

先修订 runtime 2.4 与实现规范，复用现有数组 sizing、源 root、TLAB／慢分配和范围屏障。原 site 仅用于分配慢调用，分配后重读源根再形成 AS1 copy 地址；复制完整 storage，保留目标 TD、count、ZST 和独立身份。没有重复实现分配器或添加 shape 验证层。

fmt／workspace clippy、八项数组精确测试、十八项 statepoint 专测及新旧源码八变体通过；覆盖真实 moving／minor、large old-to-young references、padding、packed、异常与 ZST。fixture 的 Option 写入补上显式 Some；observer 构建补齐私有 sysroot 的现有 vendor 输入，失败均已闭合。

生产小数组两个场景约 −14.9%，大数组持平，text +380 bytes、文件不变、编译时间基本持平；独立 observer runtime 的一／四 worker 各百万次 clone、七对运行补足 stop-wait 样本，未见稳定尾部回退。完整方法和 max 离群值见性能记录第 9 节。首批 runtime C→IR 三项已完成本机验证，后续 C2／D、统一性能矩阵与 Linux 验收继续。

## 2026-10-20：状态机生成前同步内联

先补实现规范 2.3，在原 source-signature CFG／call-site 完成后、hidden ABI 前，沿本地直接依赖传播同步事实。稳定模板复用普通 clone／cost／累计增长及六层祖先；普通 GC、参数单次求值和副作用保留，原语／动态／外部 suspend、递归及 callee 自身 EH／pending cleanup 不作为同步内联来源。克隆后更新私有 site 的 block、statement、顺序和 caller cleanup；实际剩余 site 仍产生尾转交或状态机，没有新的 ABI／摘要。

组合测试发现内联返回汇合的 Assign 会妨碍尾转交，已按规范接受未取地址、只复制同一结果的普通 local，并保留其他副作用／EH 拒绝条件。19 项协程单测及 23 项普通优化单测通过；原 tail 专测使用超过内联阈值的同步 leaf，继续独立验证尾转交。M58 协程三项源码共八变体／16 进程通过，涵盖移动 GC、真实挂起、跨线程、异常身份、递归、generic、pending return/finally 和共享位置的 deferred suspend；补充 Unit 副作用后，三个源码变体复跑通过。所有代码先 fmt／workspace clippy。

固定 `915e1f5d0` 基线、同 core／当前生产 runtime，50,000 次四层非尾同步包装的运行中位数 31.963 → 21.678 ms，总分配 40,800,296 → 10,400,296 bytes，minor 39 → 9，text −21,216 bytes。应用编译 6.419 → 6.192 s，编译峰值 RSS 约 528.7 → 509.3 MiB。完整矩阵、启动／协议分配及暂停限制见性能记录第 10 节；未据同步分析删除未内联的调用点。

## 2026-10-20：嵌套循环的局部分配与 B 性能

实际 each 对照暴露出过度保守的回边检查：外层创建的 closure 虽已完成 helper／invoke 内联，仍因在内层回边存活而被保留。先补实现规范 2.19，再按每条回边的支配关系区分内层复用和外层新实例；不提高内联阈值或轮数，不改变不可约回边和跨创建迭代旧别名的保护。

fmt／workspace clippy、26 项普通 MIR 优化专测通过；新增 nested box 精确断言，已有上一迭代别名保留测试继续通过。源码 closure fixture 增加 17 轮 each、每个元素实际 gcCollect 与含引用捕获，M58 优化五个 fixture 共 13 变体／15 进程通过。

B 功能开／关矩阵见性能记录第 13 节：局部装箱与 scope 闭包分别约 −80%／−50% 耗时，循环分配消失。嵌套 each 相比关闭 B 约 −13.5%，但相比回边修正前增加约 15.6% 耗时；换得 8 MB 分配／7 次 minor 消失、RSS 约 −1.37 MiB。保留并公开该 CPU／footprint 取舍，不把分配消失等同于总耗时改善。基线开关已恢复，Linux 完整验收随后执行。

## 2026-10-20：C1 尾转交的生产对照

新增实际 suspendCoroutine leaf 的四层尾包装 benchmark，区分 register 同步完成与真正返回 Suspended 后恢复；保留 C2 和其他优化，仅关闭／开启 C1。两模式各十一对输出与唯一完成计数一致，整程序时间约 −34%／−42%、分配约 −67%、minor 48 → 16，text −21940 bytes。累计暂停减少，单次最大暂停没有明确改善；编译／RSS 和限制见性能记录第 14 节。临时源码开关已恢复并完成 fmt／workspace clippy／工具重建；既有跨线程、移动 GC 和异常 fixture 承担协议正确性验证。

## 2026-10-20：闭合后段常量分支与 safepoint 计划

最终 sysroot 重建在 Darwin／GNU 的 HTTP asyncio 库复现同一 MissingStackmap。定位到后段 GVN／MemorySSA 产生的常量分支：根计划仍包含死路径的四个 site，RS4GC 随后才删除它们。先补实现规范 2.19，再在 Managed release 的全部普通优化之后追加 SCCP／不可达清理，早于最终根计划。没有放宽 stackmap 一致性检查或打开一般 CFG 复制。

fmt／workspace clippy、21 项 statepoint 专测通过；新精确回归覆盖三目标、debug／release、真实机器 stackmap 与保留根。原失败的 HTTP asyncio 库本机成功重建；ranges 源码增加相邻字段与真／假异常路径，随整组 M58 的 fresh-core／moving GC 验证执行。临时函数 dump 已从生产源码撤销。

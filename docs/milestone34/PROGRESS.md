# M34 实施记录

开始日期：2026-10-09。基线为 `e97b40f5c9c408e2ff785e5d09c6bf9222c78979`（M33）；工作分支为 `codex/m34`。

本文件区分目标规范与已实现能力；设计和路线图的原始 M34 草稿是本轮输入。各功能完成必要验证后分别提交，不以文档修订代替实现。

| 批次 | 状态 | 实际出口 |
| --- | --- | --- |
| M34-1 规范与基线 | 已完成 | 三份规范先行修订；两地 M33 基线、混合 AS1／metadata 返回及目标 C ABI 实验完成，详见下文。 |
| M34-2 双字接口 | 已完成 | 表示、slot receiver、niche、GC、AtomicRef、native、closure／coroutine、跨 Cone；三 target 的六组 CLI fixture 通过。 |
| M34-3 MIR 优化 | 已完成 | 实际类型传播、去虚拟化、三档调用点内联、常量化简及累计增长控制；三 target 的 13 组 CLI、两机开关性能对照完成。 |
| M34-4 poll 与分配 | 已完成 | 条件 poll／pending 激活、线程累计计数／detach 归并、三 target 验收与两机性能对照完成。 |
| M34-5 小值与 DirectC | 已完成 | 小值 DirectParts、正向 C aggregate DirectC、三 target ABI／GC／artifact-only 验收与两机性能对照。 |
| M34-6 多 region | 待实施 | 稀疏地址索引、扩容、large mapping、cards。 |
| M34-7 搬迁与归还 | 待实施 | source／target 规划、完整回写、discard／unmap。 |
| M34-8 并行 mark | 待实施 | 首次标记、分块任务、全局终止、存活集合复用。 |
| M34-9 容器 | 待实施 | MaybeUninit 与 ArrayList／StringBuilder。 |
| M34-10 总验收 | 待实施 | 三 target 的功能、CLI、ABI、GC、性能与实际版本清单。 |

## 验证与磁盘使用

优先运行受影响 crate／fixture；最终汇总必要覆盖，不因小改动重复全量。基线运行与其他编译／清理串行，三个 target 分别记录，不跨宿主相除。

本机初始无 target，Linux 初始 target 为 46 GiB。分批清理已链接的中间对象及过期 incremental，保留仍有效的库、CLI、测试产物和构建缓存；记录实际回收字节。构建、运行和清理不并发操作同一 target。

本机默认 LLVM 已升级为 23.1.3；首次基线运行按项目合同拒绝，未计为测量。随后显式使用 `/opt/homebrew/opt/llvm@22`（22.1.8）；Linux scoopc 链接 `libLLVM.so.22.1`。每次 Rust 构建保留显式 LLVM 22 prefix，避免误选其他已安装版本。

## ABI 与格式迁移

目标为 runtime contract 12／metadata ABI 8，旧 ABI 11／7 产物需重建。当前规范已描述目标，代码和 section versions 随实际批次逐项迁移；开发中间批次不作为稳定发布 ABI。persistent 声明身份不因机器表示或优化而改变。

## M34-1：规范、基线与后端可行性

三份规范先行规定双字接口／slot object projection、DirectParts／目标 C 分类、MIR 优化顺序、条件 poll、多 region／cards／归还、并行 mark、线程局部统计和 MaybeUninit；同步移除旧单字／全 aggregate 间接／full 必须搬迁等冲突条文。代码仍以 M33 ABI 11／7 运行为基线，后续实际 ABI 迁移单独记录。

两地各完成八个原有工作负载 × 五次运行，实际输出与 GC 直方图均由原脚本检查；[性能记录](PERFORMANCE.md) 保留全部样本和测量限制。[独立 C 实验](experiments/README.md) 确认整数、混合、浮点 HFA 与寄存器耗尽的两套分类；没有将编译器实验写成尚未实现的 DirectC 功能。

LLVM 22.1 的最小实验确认，双字结果必须经过已有的精确 managed leaf 回写协议；只拆分再组合原 SSA aggregate，可能被 InstCombine 合并并遗漏 relocation。实验结合已有 codegen 路径保存接口存储，跨 site 只对 object 发射根标记和 volatile leaf 回写，再通过 volatile object reload 与原 metadata 重建；仅有 volatile store 不能阻止 LLVM 转发旧值。PHI／select 同样经过这一普通路径，M34-2 将落实对应接口发射。invoke 保留原有零 gc-live／显式根协议，不引入另一套 aggregate SSA 优化或降低验证标准。

验证：工作区 fmt／all-targets clippy 通过；Darwin 与 nuc12 各 24 项 statepoint 定向回归通过，包含原子引用、GC 根合并、循环派发及 amd64 固定调用栈。新增实验覆盖三个 target × debug／release × PHI／select，执行 Scoop 根验证、LLVM safepoint verifier 和真实目标 object 生成。实验代码分为独立 Rust／LLVM 文件，各不足 110 行；本批不重复运行无关工作区测试或完整文件 fixture。

已清理本机 target 96 项、3,132,545,645 bytes；nuc12 52 项、1,997,877,943 bytes，保留库、CLI 和有效缓存。详细记录位于两地 `tmp/m34/batch1-target-cleanup.json`。

M34-2 开始启用 runtime contract 12／metadata ABI 8。MIR `cross-cone-type-bridge` 为 18，记录具体对象 receiver 的 interface adapter；LIR `cross-cone-param-free-bridge` 为 2、`cross-cone-layout-abi` 为 13、`cross-cone-layout-link-closure` 为 6，保存 DirectParts 与双字 interface exact layout。目标 ABI 分类合同同步区分 interface parts；其他未改变编码的 section 保留原版本。

## M34-2：双字接口

普通 interface 与同构 niche enum 已使用 16-byte／8-byte 对齐的 `{AS1 object, AS0 itab}`；参数展开为两个 physical parts，结果直接返回两分量。动态调用直接读取携带的 slot table，隐藏 receiver 只传 object。默认方法和装箱方法使用真实生成的 typed adapter，直接 class 实现保持原入口；object 的源码与 backing 视图共用 adapter。普通参数、结果、字段、数组、枚举、捕获和 suspend ABI 均保留完整值。

转换按完整目标 interface identity 选择表，已知具体类型复用实际 descriptor 目录顺序，未知转换在边界查询。None 清零两个分量，空接口仍按 object 判断存在；引用比较、pin、handle 与抛异常只使用 object。GC 只扫描 offset 0，跨 statepoint 从回写后的 object 与原 metadata 重建；LLVM 优化所需的 volatile leaf store／load 已落实，invoke 延续显式 roots。

AtomicRef 的原子槽保持单 object，load／exchange／CAS 结果在 NoGC 路径恢复接口表。Context 内部 binding 使用独立 generated managed-pointer 类型。协程 receiver slot 使用真实 box identity，普通 nominal 与泛型 application 分别保留 Strong／ODR 归属；内部私有 slot 不扩大发布闭包。消费者复用普通依赖类的 descriptor 和 adapter，本地子类及实际泛型实例生成自己的表。

新增六组正式 CLI fixture：值与转换、原子操作、Scoop native ABI、四线程发布与 CAS、挂起／恢复、跨 Cone 产物。覆盖 debug／release、普通／full-moving／minor-stress；跨 Cone fixture 交叉使用 provider／consumer profile，并删除源码后仅凭产物重新链接。挂起用例同时包含 lambda、普通私有值类型和公开值类型；跨库 object 验证共享 default adapter。macOS、Linux GNU 和 Linux musl 的六组均通过；GNU／musl 每个 target 为 12 variants、60 processes、42 stage goldens。两地共用的 14 份 HIR／MIR 快照逐份校验一致，全部 target-specific LIR 快照均已保存。

受影响 IR、ABI、codegen、metadata 与 native 边界的定向回归已通过；最近的 HIR→MIR 生产路径 26 项、协程 lowering 10 项均通过。另运行原有接口 default／super／属性 helper 和函数变型／移动 GC 文件 fixture，3 variants、6 processes、12 stage goldens 通过。新实现按 interface lowering、ABI parts、receiver adapter 和身份记录分文件，未将新逻辑继续堆入已有大型测试文件。

本批两次清理中间对象与 incremental：本机共 38,752,299,448 bytes，Linux 共 2,075,523,097 bytes（按删除文件的逻辑大小记录）。最近一次分别删除 19,386 项与 119 项；有效库、CLI、测试程序及 M33 基线保留。记录在两地 `tmp/m34/batch2-target-cleanup*.json`。


## M34-3a：实际类型传播与去虚拟化

独立优化模块在完整 MIR 形成后、现有输出验证前执行，由 driver 显式传入 debug／release 配置。分析从 allocation、box、closure 和 final 参数取得事实，沿局部复制、引用转换、正常及异常边传播并求 CFG 不动点。合流有不同实际类型时退回未知；可变字段、未知返回、取址 local 不提供过期事实。构造测试继续遵守初始化 receiver 不可提前调用的语言规则，检查完整构造链后保留最派生类型。

virtual／interface／closure／function bridge 调用从已有 typed slot 或 invoke 条目选择实际目标，同时改写 callee 和 receiver 类型；完整签名不一致或表项不可用时保留动态派发。普通外部类的 vtable 可以引用已经选中的 external callable；不为优化重建依赖类的 itable 或 adapter。原调用 effect、GC、EH 和 Context 路径不变。

新增三组正式 CLI fixture，覆盖 final／open 参数、未知返回、同类型与冲突分支、稳定与变化循环、异常合流、可变字段、构造链、默认方法与菱形继承、重载 slot、装箱、函数变型和跨库继承。三 target 的 debug／release 与普通／full-moving／minor-stress 运行均通过；跨库用例删除源码后重新链接。每个 target 的新 fixture 为 6 variants、28 processes、24 stage goldens，两地 33 份共用 HIR／MIR 快照一致。

接口 fixture 的 MIR golden 按 profile 分开；debug 的七份逐字保持上一提交内容，release 锁定真实优化结果。本机 HIR→MIR 生产路径 26 项及六组接口 CLI 回归通过，Linux GNU／musl 的六组接口回归亦通过，各为 12 variants、60 processes、42 stage goldens。实现拆为优化入口、数据流和目标解析三个文件，分别为 114／210／117 行。

本机清理 1,805 项已链接中间对象及 incremental，回收逻辑大小 3,590,215,643 bytes；Linux 清理 224 项、1,216,699,824 bytes。两地均保留有效库、测试程序与 CLI，并保存仅去虚拟化的工具用于内联性能对照。

## M34-3b：调用点内联与局部化简

优化在同一原始 concrete 函数目录上选点，不依赖先前 caller 的展开顺序。极小正文阈值为 12；普通正文基础阈值为 20，循环、已证明的常量化简、已知 receiver 和小值复制机会可提高到最多 48。每个 caller 的累计展开额度为原成本的一半加 96、最多 384，嵌套深度最多 6。成本只决定是否保留调用，不影响程序合法性或产物兼容性；具体权重与测量见性能记录。

布尔与精确位宽整数的常量事实复用 receiver 分析，代入实参后删除已证明不可达分支；纯标量运算可折叠，内存、分配、检查和调用保留效果。callee 的形参和局部值取得 caller 的独立身份与存储，实参各求值一次；多个 return 先写临时值，再在 continuation 初始化原调用结果，保持 variant test／payload projection 所需的稳定 local。Managed 循环保留 poll，NoGC 循环展开后不增加 poll。普通 throw 和可能抛出的操作继承调用点出口；callee 自身 EH 协议、callback、协程入口与递归 SCC 保留调用。

MIR `identity-foundation` 升至 6，其实际局部值目录进入 Code／LinkValidationOnly sink；依赖语义仍由现有可消费 bridge sections 决定。新增指纹测试确认内联产生的局部身份不使相反 profile 的依赖失效，公开 bridge 变化仍改变语义。已选外部声明可以保留未使用项和原索引，移除“目录项未被引用即非法”的重复遍历；最终链接继续由实际 undefined references 驱动。

新增四组 CLI fixture：调用点决策、累计增长、效果与 GC、跨产物。快照显示循环 helper 被展开而冷调用保留，常量 false 删除大检查分支，未知分支和互递归保留；8 层 wrapper 在 6 层停止；64 个连续小调用中 38 个展开、26 个保留。语义组合实际展开了分配、throw、取址参数、InteriorMutable 副本、闭包捕获与枚举返回；caller 的 catch／finally／Context 仍生效。外部泛型的本地 concrete 实例被展开，普通外部正文保持调用；交叉 profile 与删除源码后的重新链接均已验证。

工作区 fmt／all-targets clippy、内联身份／递归／NoGC 循环三项结构测试、外部声明索引回归与 scoop-slib 的 569 项单测通过。新增实现分为常量、CFG、选点、成本、克隆与简化模块，最长的优化文件为 260 行。HIR 快照按 profile 分开，记录依赖 identity table 的真实临时索引；既有 11 份 debug MIR 逐字保持 M34-3a 内容。

三个 target 的四组新 fixture 与九组接口／去虚拟化回归全部通过，每个 target 为 26 variants、124 processes、96 stage goldens。本机最终关闭 snapshot 更新重新验收；Linux 复核保留输出与按 profile 分开的 96 份快照逐字一致，两地 64 份 HIR／MIR 文件的 SHA-256 相同。两机内联开关分别运行七次并保留机器码、代码大小、构建时间及全部样本；本机中位数比值为 1.073，Linux GNU 为 1.023，限制见性能记录。

本机本批清理 target 6,382 项、8,215,828,137 bytes；Linux 清理 224 项、1,219,044,107 bytes。保留有效库、CLI、测试程序，并在两地保存 M34-3b 的配套工具和源码用于下一批对照。清理记录位于两地 `tmp/m34/batch3b-target-cleanup.json`。

## M34-4a：条件 poll

现有 LIR ManagedPoll 保留逻辑位置、唯一 site 和完整 live set，codegen 为它发射快／慢分支。快路径分别 acquire 读取 epoch、world phase、真实线程 mode 和 observed epoch；只有 RUNNING／MANAGED／epoch 一致时继续。MANAGED_PENDING 必须进入既有有锁激活路径，NativeSafe 返回和 callback 握手保持原职责。线程状态只保留一份；私有 TLS 指针指向其中的 poll 字段，attach 设置、detach 清空。

根物化、runtime poll call、relocation 回写均在慢分支发射，后续使用沿现有 canonical storage 合流。线程地址在函数调用期间稳定，因此 TLS 指针只在入口读取一次，四次原子检查仍逐 poll 执行。runtime ABI 目录新增 PollState／WorldPhase／GcEpoch 数据符号（tags 29～31）；TLS relocation 与 runtime 导出检查直接使用既有机器合同的数据种类，移除原先仅识别两个数据符号的硬编码。

新增 roots 与 threads 两组 CLI fixture，覆盖 interface、array、tuple、niche Option、closure、continue 回边，以及三个纯计算线程等待另一线程执行四轮 GC。两台主机的 moving-GC 发射／改写 10 项和 runtime 收集器／线程 21 项回归通过；多 poll 结构测试验证一份 TLS 读取、各自的 acquire 检查与慢路径根。符号 relocation 三项和 GNU／musl runtime 完整导出检查通过。

三个 target 均完成两组新用例及接口并发、协程、artifact-only、内联语义组合验收，每个 target 为 12 variants、60 processes、42 stage goldens，包含 debug／release 与普通／full-moving／minor-stress。最终运行关闭 snapshot 更新；八份新 HIR／MIR 跨主机逐字一致。poll 发射与新增结构测试分别为 165／89 行。

整数循环的两机七次开关对照及机器码已记录在性能报告：正常循环不再每轮调用协调入口或解析 TLS，原子检查保留；入口栈和机器码增量如实记录。两地保存 `tmp/m34/poll-tools` 与 `poll-source` 供分配统计对照。本机清理 target 5,622 项、6,273,385,579 bytes；Linux 清理 4,362 项、4,987,043,809 bytes，记录为两地 `tmp/m34/batch4a-target-cleanup.json`。

## M34-4b：线程分配统计

TLAB 完成、普通慢分配和大对象分配改为更新本线程的累计对象数、nursery 对象数及两种分配字节数。单写者使用 relaxed atomic load／store；对象起点登记的原子发布保持原职责。计数区域独占 128-byte 对齐区域，线程状态使用相同对齐的 `posix_memalign` 分配，覆盖本机实际的 128-byte cache line。

detach 在 world lock 内先归并累计值再移除目录项；查询按 world → heap 顺序保护目录和 collection 基线。collector 在已停止的稳定目录上直接汇总，累计值不清零；当前对象数由回收后存活数与随后分配数得到，minor 同时扣除新增 nursery 数以保留旧代。主线程退出后的最终报告仍包含全部分配。

两机 22 项 runtime 回归和两套 Linux runtime 的完整 ABI 导出检查通过。新增 120 行 C 测试覆盖内联 TLAB、普通／old／large 分配、minor／full 基线、并发查询、32 次 attach／detach 与主线程退出后的统计；先在旧实现上确认已有行为，再迁移计数。新增 accounting／threads 两组正式 CLI fixture，组合数组、Option、GC、callback、重复线程退出与 gcStats；三个 target 的四组定向验收各为 8 variants、40 processes、24 goldens，最终关闭 snapshot 更新，八份 HIR／MIR 跨主机一致。

两机分别测量单／四 mutator、GC 诊断关／开，每组七次。两种实现的每次累计分配字节严格相同，minor 次数在原有范围内，未发生 full collection；机器码确认四次全局统计 RMW 已替换为线程字段的 load／store。四线程收益明确，单线程结果与波动、TLS 读取成本一并记录在性能报告，不声称所有场景加速。

生产改动保持在现有 allocation、statistics、thread 与 collection 结算模块；最长相关文件为 thread.c 的 323 行。本批回收本机 target 520 项、444,825,719 bytes；Linux 264 项、380,448,508 bytes。配套工具和源码保存为两地 `tmp/m34/allocation-tools`／`allocation-source`，清理记录为 `batch4b-target-cleanup.json`。

## M34-5a：小值 Scoop ABI

GC-free、size ≤ 16、alignment ≤ 8 的 struct／tuple／tagged enum 按目标分类直接传递，含 GC leaf、大值与过高对齐保留间接值协议。双字接口继续使用专门的 AS1 object／AS0 metadata 两分量。参数与返回分别分类：Darwin 保留 HFA 和连续整数 carrier，SysV 按完整签名计算 INTEGER／SSE 余量，含隐藏 sret，余量不足时整个聚合参数回退。

物理计划保存 carrier、byte offset、有效 extent 与 alignment；三个 target 的分类共用已有 scalar／layout 数据。发射时逐字段物化已清零的源 storage，只复制有效范围，较宽 carrier 的高位补零，保留浮点位模式。callee 的取址参数是独立副本；普通返回、invoke、函数值及 interface slot 共用相同转换，receiver projection 后重新确定完整签名的寄存器分配。

canonical callable／native contract 保存完整计划，产物读入后核对 exact signature、引用、GC 与布局，并复用已保存的物理分类；删除 metadata 消费端另行重放源码 ABI 分类的实现。LIR `cross-cone-param-free-bridge` 升为 3、`cross-cone-layout-abi` 为 14、`cross-cone-layout-link-closure` 为 7；Scoop target classifier 使用新 tag 3。runtime contract 12／metadata ABI 8 不变，C classifier 留待下一批。

新增三组正式 fixture：普通值与效果、独立 LLVM native 入口、跨 Cone artifact-only。覆盖整数／浮点／混合／嵌套／packed／tuple／tagged enum、大值与含引用值回退、参数取址、寄存器耗尽、函数值／interface slot、try／finally、GC 和交叉 profile。native 用例单独检查 signaling NaN、负零、3-byte 高位与 padding 补零。迁移原有 16-byte Scoop native 浮点探针，24-byte 间接探针保持原合同。

三个 target 的新用例及接口 native／artifact-only、内联效果、浮点 FFI 回归均通过；各为 14 variants、78 processes、56 goldens，最终运行关闭 snapshot 更新。37 份共用 HIR／MIR／AST 跨主机逐字一致。受影响 identity、分类、codegen、native 布局、产物版本和两组真实 core 共享 ABI／普通跨库关系测试通过；EH 测试桩同步修正了条件 poll 后的线程字段位置。

新 carrier、classifier 和 coercion 发射文件分别为 204／221／261 行，canonical storage 从签名模块分出；原 1593 行 ABI 校验文件按 metadata、调用点、目标、runtime 和测试拆分，最大模块为 433 行。整理后 15 项 codegen ABI 校验回归通过。本机回收 target 19,276 项、22,542,764,718 bytes；Linux 298 项、1,984,919,128 bytes，保留有效库、CLI、测试程序及对照工具。两地清理记录为 `batch5a-target-cleanup.json`。

两台主机各完成普通跨 Cone 与 Scoop native 两种小值调用的七次前后对照，保留全部原始样本、构建记录与 caller／callee 机器码。普通跨 Cone 在 Darwin／GNU 的中位数比值分别为 3.243／4.110；native 边界组合未获收益，Darwin 变慢、GNU 波动较大，详见性能报告，不将这组数据删去或更改已有 native 协议。两地配套工具与源码保存为 `tmp/m34/small-value-tools`／`small-value-source`，供 DirectC 对照使用。

## M34-5b：正向 C aggregate DirectC

固定 cdecl、无 errno 捕获的合法 C signature 全部使用 DirectC。物理计划保存 C scalar、coercion parts、SysV byval／Darwin caller copy 和 sret，复用 M34-5a 的目标 carrier 分类及 typed call storage；小整数扩展按展开后的物理位置发射。SysV 的 MEMORY 参数副本至少 8-byte 对齐，packed 类型布局保持原值；Darwin 的大结构使用独立副本指针，HFA 支持四个 Double。模块边界检查 canonical C 类型、存储和所选计划，调用目录不再误套 Scoop 的 16-byte 门槛。errno、callback、FunPtr 与 native global/TLS 保留各自既有协议。

C lowering profile 由退役 tag 2 迁为 tag 3，进入现有 target fingerprint 和构建缓存；runtime contract 12／metadata ABI 8、M34-5a 的 section versions 不变。通用间接结果约定在代码中改称 Sret，明确与 C bridge 的普通结果指针不同，未引入另一套 carrier 或 GC 协议。新增分类和 C 属性发射模块为 93／156 行，C call plan 为 149 行。

新增三组 DirectC fixture 覆盖 3-byte 值、整数／浮点／混合、32-byte HFA、nested／packed／aligned 布局、尾 padding、Char／Boolean／nullable data/code pointer、小整数扩展、两类寄存器耗尽、隐藏结果参数、独立副本和 signaling NaN／负零位型。NativeSafe callback 与两个 foreign threads 在 GC 中保活对象，GCLeaf 循环直接调用 native symbol。跨 Cone 用例组合泛型 C-layout、interface／函数值、相反构建 profile，并删除双方源码后使用正式 linker 重链运行。

FFI 回归发现并修复 M34-3b 的构造发布问题：保留 release-ready 紧邻的最外层 initializer 调用，允许包含完整构造序列的函数整体内联，维持构造异常不发布。独立 `m34-inlining-release` fixture 锁定空构造、泛型、异常与 GC；原两组含 release 的 FFI 用例在 release 模式恢复通过。

Darwin、GNU、musl 均在关闭 snapshot 更新后通过九组正式 CLI 验收，各为 18 variants、108 processes、68 goldens。43 份共用 AST／HIR／MIR 跨主机逐字一致；实际不同的 debug/release HIR／MIR 分别保存。两机 workspace／all-targets clippy 通过，相关 Rust ABI 99 项及 MIR 内联 3 项测试通过。前后性能以 `dfc609f83` 为对照，复用既有 FFI workload 的 C／NativeSafe／GCLeaf aggregate 路径，详见性能记录。

本机回收 target 324 项、1,812,724,794 bytes；Linux 回收 147 项、767,645,502 bytes，保留有效库、CLI 和测试程序。记录为两地 `batch5b-target-cleanup.json`；配套工具与 runtime／sysroot 保存为 `tmp/m34/direct-c-tools`／`direct-c-source`，供后续 runtime 批次对照。

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
| M34-6 多 region | 已完成 | 16 MiB region、独立 large mapping、稀疏地址索引、内联 cards、三 target 验收与两机性能对照。 |
| M34-7 搬迁与归还 | 已完成 | region 选择性搬迁、旧代空洞复用、完整预留回滚、discard／unmap；三 target 验收与两机内存／性能对照。 |
| M34-8 并行 mark | 已完成 | 原子首次标记、分块任务、全局终止、存活集合复用；三 target 回归、两机 TSan 与 1／2／4／8 worker 七轮对照完成，默认上限为 4。 |
| M34-9 容器 | 已完成 | MaybeUninit 类型／操作／GC、ArrayList 紧凑 backing 与 StringBuilder 拼接边界；三 target 验证、两机容器性能对照完成。 |
| M34-10 总验收 | 实施中 | 功能批次已交付；正在汇总工作区／正式 CLI 回归、性能与实际版本清单。 |

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

## M34-6：多 region 与地址索引

普通堆按需映射 16 MiB region，保留 32 KiB block／128-byte line；block metadata、free run、TLAB owner 与搬迁计划使用稳定的 block 指针及所属 region，移除全局裸 block index、固定 arena 边界和 large tail 表。先复用已释放 block，large object 独立按 64 KiB 取整映射。OS 边界新增 release_mapping，对齐预留的首尾页立即解除映射；死亡的大对象在完整引用更新后撤销索引并 unmap，不再受 1 GiB 上限限制。

地址目录覆盖完整 uintptr_t：64 KiB chunk、四级各 12-bit 的稀疏 radix，metadata/cards 初始化后 release 发布，reader acquire 读取。单槽写屏障内联完成四级查找、region 内相对 card 寻址和原子 OR；native／aggregate 范围屏障按映射边界覆盖完整范围。remembered set 遍历各 region 的脏旧区，large object 跨 chunk 的卡片仍归属同一对象。STW 撤销后回收空 radix 路径；普通 region 的选择性搬迁及整区归还仍属于下一批。

runtime registry 使用 PageMap tag 32，退役 CardTable tag 6；根数据为 32768-byte／8-byte 对齐，generated-code prefix 的偏移由静态断言锁定。保留 runtime contract 12／metadata ABI 8，删除无调用者的单 arena 调试地址入口，统计补充普通 region 数、large mapping 数与实际映射字节。TLS cursor 的既有两字 ABI 不变，所属 block 保存在 runtime 私有线程字段，收集时与 cursor 一起失效。

新增 runtime 测试实际分配超过 1 GiB 的对象，验证清零、精确大小、pin、跨 chunk 范围标卡及死亡后撤销映射；1200 个 30000-byte 对象跨三个普通 region 保活。稀疏索引测试覆盖全部 64 个地址位、部分删除、空路径回收与重新发布。晋升失败用真实 block 填充及 fake OS 的映射失败复现，确认预留回滚后从完整原图执行 full。两机收集器原有 22 项及新增两项通过；Linux GCC 15 的原子数组声明／typedef 组合问题由统一 typedef 解决，未放松原子或警告合同。

新增 values／threads 两组正式 CLI fixture，正常规模保留 600000 个普通对象并创建超过一个 region 的接口数组，四线程各保留 180000 个对象，组合链式写入、nullable interface、数组复制、AtomicRef、tuple、closure、callback、minor 与 full。stress 使用较小图，minor 模式仍实际触发 minor collection。回归包含原有 nursery/native/Context/范围复制、AtomicRef 和 artifact-only 入口；原 M33 atomic GC 的 HIR/MIR 按实际 debug/release 输出分别保存。

Darwin、GNU、musl 均在关闭 snapshot 更新后通过八组验收，各为 19 variants、73 processes、36 goldens；24 份共用 HIR/MIR 跨主机逐字一致。五项写屏障结构测试和四项 runtime ABI 目录测试通过，保留完整 pointer publication、slot 原子性和原有 poll/roots 合同。两机 workspace/all-targets clippy 通过；新增 region、page map、range barrier、block retirement 和 codegen card 模块分别为 114／83／27／49／94 行，相关最长 C 文件为 allocation.c 的 264 行。

四个 workload 各七次、每轮交替旧／新版本的性能对照已经归档。可扩容堆的地址查询存在实际成本：Darwin 单槽循环与 GNU 旧图／高存活图变慢，完整样本和机器码均保留，未以 GC 工作量变化掩盖结果；详见性能记录。两机保存 `tmp/m34/region-tools`／`region-source` 供后续搬迁与回收对照。

本批清理 target：Darwin 删除 320 项、1,512,187,367 bytes，Linux 删除 117 项、616,166,385 bytes，保留有效库、CLI、测试程序和缓存。记录为两机 `tmp/m34/batch6-target-cleanup.json`。

## M34-7：选择性搬迁与 OS 归还

普通 full 保留最密集的普通 region，将其他不超过四分之一容量且无 pin 的非空区域作为候选 source。先复用非 source 旧代 block 的空闲 line，再使用空 block／未用 block；单 source 不新增映射。完整预留后，新占用的空 region 数必须小于 source 数，包括已有的空缓存，避免往返搬迁却没有集中收益。minor 和显式 moving stress 继续使用原来的 block 级策略。

目标旧代 block 在复用前完成死亡对象 release 并移除起点，保留原有存活 mark；预留失败只退还新激活的目标 block，不破坏已有活对象，也不重复 release。成功后才复制并发布 forwarding，完成 roots／对象引用回写后才退休 source。映射失败测试先实际占用目标空间，再使下一次 OS 映射失败，验证原对象图、根和 pin 地址均保持有效；恢复映射后可以继续收集。

full 按完整 OS 页检查 live-line 覆盖，Darwin／Linux 使用 `madvise(MADV_DONTNEED)` 建议归还物理页；失败保留映射与待归还状态，下次 full 重试。空 ordinary region 保留一个，其余撤销地址索引后 unmap；large mapping 继续单独回收。free block 链在 region 撤销后重建，TLAB 在收集前失效。统计分别记录建议成功／失败次数、累计建议字节、累计 unmap 字节和当前／峰值 RSS，避免混淆虚拟映射、建议性归还与实际驻留量。

新增三个 C 测试覆盖跨 region 环图、旧代空洞、pin 阻止释放、连续增长收缩、release 恰好一次、跨页存活对象、discard 失败重试、重新分配清零、完整预留失败和空缓存无收益回滚。原有三个移动协议测试显式在精确根已发布的收集阶段启用 stress，普通 full 不再被错误要求搬迁单 region。两机 27 项 collector 测试通过。

新增 growth／pins 两组正式 CLI fixture，普通规模为 160 万／180 万个对象；组合双字接口、数组复制、AtomicRef、closure、tuple、old→young 写入与 scoped pin。debug／release、普通／moving／minor 三种模式均覆盖。Darwin、GNU、musl 的最终 12 组定向验收均在关闭 snapshot 更新后通过，各为 25 variants、106 processes、60 goldens；39 份共用快照跨主机逐字一致。补修清扫函数内联后，5 项 C 回归及三个 target 各 4 组 CLI 再次通过，每 target 为 8 variants、32 processes、16 goldens。

两机 workspace/all-targets clippy 与 C 警告检查通过。清扫、source 选择和 OS 归还拆为独立模块，分别为 183／75／116 行；规划 160 行、收集结束入口 55 行，相关最长 C 文件 allocation.c 为 267 行。没有增加生产测试钩子、资源预算或重复的产物验证层。

五个 workload 各七轮、每轮交替 M34-6／M34-7 的性能结果与机器码已经归档。初版清扫拆分产生每个死亡对象一次的额外函数调用，修复为内联后重新测量，并保留全部修复前样本。无 pin 的稀疏图映射从 64 MiB 降至 32 MiB，GNU 当前 RSS 约从 95 MiB 降至 40 MiB、Darwin 从 96 MiB 降至 65 MiB；吞吐、峰值 RSS 和剩余成本如实记录于性能文档。两机保存 `reclamation-tools`／`reclamation-source`，修复前源码另存为 `reclamation-pre-inline-source`。

本批清理 target：Darwin 删除 72 项、151,091,917 bytes，Linux 删除 144 项、156,877,294 bytes；有效库、CLI、测试程序和缓存保留。记录为两机 `tmp/m34/batch7-target-cleanup.json`。

## M34-8：并行标记与存活集合复用

STW 后发布不可变 heap／roots 视图，内部 pthread marker 不登记为 mutator。small mark 位与 large mark 状态采用原子首次置位，只有赢家登记对象；line-live 原子 OR，block 统计在 worker 私有数组中累积，结束后归并。存活清单按 1024 个指针分块，发布后地址稳定。普通任务扫描最多 64 个对象，大引用数组按最多 1024 个元素拆分，GC-free 数组不生成扫描任务。全局 pending 包含队列、在途任务与子任务发布；父任务必须先发布全部剩余子任务再完成，不能用一次窃取失败判定结束。

coordinator 与后台线程运行同一任务循环；线程懒创建，失败时停止并 join 已建线程，后续收集使用同一实现的单 worker 路径，正常 shutdown 全部 join。默认 minor 和活跃 block 不足 4 MiB 的 full 使用一个 worker，其余按 CPU 数选择最多四个，显式 `SCOOP_GC_WORKERS=1..8` 保留。线程池覆盖先串行、首次并行、再次 minor 和后续 full 的切换。

mark 产生的唯一存活集合直接用于移动计划和每个当前副本的引用更新，删除原 full 更新阶段的第二次可达图发现。复制、引用回写、release hook、reclaim 和 VM 归还仍由 coordinator 完成。新增停稳等待、roots、remembered、mark、plan、copy、update、reclaim、VM 归还阶段计时，以及各 worker CPU／标记对象数、数组任务、窃取、region mapping 与 source／target／pin 阻塞指标；它们不进入产物身份或程序合法性规则。

新增两项 C 测试，一次编译分别运行 1／2／4／8 worker，覆盖共享子图、环、GC-free 与复合值数组、pin／搬迁、old→young、长链、全部回收及 coordinator 上恰好一次的 release；另一项在测试链接边界令第三次 pthread_create 失败，验证已建线程 join、完整串行回退且不重试，没有生产故障钩子。两机 29 项 collector 测试通过。两机 TSan 覆盖四种显式 worker 配置和默认线程池切换，未报告数据竞争；fake stack fixture 使用 `-fno-inline` 保持其合成 frame 边界，不放松真实栈范围校验。

新增 values／threads 两组正式 CLI，组合双字接口、含引用 tuple／struct 数组、复制、AtomicRef、closure、pin、四个 native pthread callback 和冻结的 NativeSafe roots。debug／release 在 1／2／4／8 worker 下执行，并补 moving／minor stress。Darwin、GNU、musl 最终严格验收各通过 14 组、29 variants、144 processes、72 goldens；默认上限收敛为四后，仅补默认路径的 threads／pins 各两组，每 target 4 variants、26 processes、12 goldens。既有固定 worker 的正确性与性能样本无需因默认参数改变重跑。

两机 workspace/all-targets clippy、C 警告检查通过。实现拆为 mark／objects／pool／queue 四个 C 模块，分别为 140／171／168／78 行，collector 为 210 行。新测试共享 fixture 为 156 行，两个测试为 87／38 行。没有新增通用配额、授权、重复语义校验或第二套 collector。

两机完成三种 131071-node 图的 1／2／4／8 worker 七轮对照，每进程五次 full；另对最终默认四 worker 补七轮 baseline 对照，以及三个既有 workload 的回归测量。初版长链逐对象队列与广播开销已修正为批内继续扫描后继，并保留修复前诊断样本。完整样本、阶段结果、CPU、RSS、构建与机器码见 [性能记录](PERFORMANCE.md)。两机保存 `marker-tools`／`marker-source`，初始默认八 worker 的源码和 binary 另存，供后续容器批次做精确对照。

本批清理 target：Darwin 删除 148 项、337,525,691 bytes，Linux 删除 76 项、190,134,062 bytes；保留有效库、CLI、测试程序和缓存。记录为 `tmp/m34/batch8-target-cleanup-{darwin,linux}.json`。

## M34-9a：MaybeUninit 存储原语

core 通过实际的 intrinsic nominal 与三个方法声明提供 invariant `MaybeUninit<T>`。每个 application 有独立 exact identity，与 T 等 size／alignment；零构造清除完整存储，initialized 按值包装，assumeInit 按现有 unsafe context 检查。wrapper 不提供普通构造、payload 字段、隐式转换、派生相等／字符串化或 T 的 niche，GC-free 条件递归取决于实际 payload。

HIR、默认实参模板、MIR、LIR 与产物均保存完整 typed 操作和 payload 身份。安全方法引用在普通 closure adapter 内展开，显式 companion receiver 按原调用／引用规则求值。LIR 操作直接写入独立 local place；LLVM 使用包含全部字节和精确 AS1 引用槽的值表示，包装、取值与 aggregate local 复制保留 padding 和浮点 bits。layout／scan 复用 payload，字段／数组写屏障和跨 safepoint roots 沿用普通路径，不增加 initialized 标记或 GC 协议。

实际 section 版本为 HIR interface 69／type semantics 27、MIR type bridge 19、LIR layout ABI 15／layout link closure 8；MIR representation tag 13、LIR value representation tag 9、默认表达式 tag 73。runtime contract 12／metadata ABI 8、bootstrap 14、param-free MIR 2／LIR 3 与两套 target classifier 3 保持不变。产物消费实际覆盖泛型默认参数、具体函数与接口值，provider／consumer 使用相反 profile，删除源码后仅凭产物重新链接。

新增生产模块为 13～90 行，closure adapter 与 body lowering 共用 typed 操作构造；未把新逻辑堆入既有大型文件，也未建立授权、初始化证明或通用预算机制。两机 workspace/all-targets clippy 通过；Darwin 的 HIR／产物 intrinsic 定向单元测试 42 项通过。

新增 18 组正式 CLI fixture：值布局与复制、GC／native roots、跨 Cone 产物、安全方法引用，以及 14 项语言错误。组合覆盖 Int／Float bits、ZST 求值、padding 跨函数与 GC 保留、引用／双字接口、含引用 struct／enum、tuple、Option 与嵌套 wrapper，旧数组写入年轻引用及清零。Scoop native 入口使用独立 LLVM byval／sret shim，C companion 在收集期间为实际 byval 槽注册 roots。三个 target 的新增用例与四组小值 ABI／poll 回归各通过 22 组、30 variants、98 processes、60 goldens；debug／release 均执行普通、full-moving 和 minor-stress。跨主机共用 HIR／MIR 快照一致。最后补强的 padding 跨调用用例在三个 target 分别重跑通过，不重复无关全量测试。

Darwin 清理中间对象／incremental 4,349 项、5,548,604,841 bytes；Linux 本批无新增可删中间对象，另清理已停用的 debug 与 M33 build 目录 8,654 项、18,326,714,520 bytes，保留源码目录、有效 release 工具和 M33 性能基线。记录为 `tmp/m34/batch9a-target-cleanup-darwin.json`、`batch9a-old-target-cleanup-linux.json`。两机保存 `maybe-tools`／`maybe-source`，使后续容器对照可以固定编译器及 GC，仅改变库的存储实现。

## M34-5c：core 原生结果 ABI 补齐

容器的 JSON 组合验证发现，core 的字符定位与浮点解析入口仍使用旧的 sret 适配。现在 `Option<Char>` 由公共 C 后备按两个整数分量返回；`Option<Float>`／`Option<Double>` 的平台适配直接返回 tag 与 payload bits，避免套用 C 的混合浮点 struct 返回分类。24-byte slice 结果和含引用的 UTF-8 decode 结果继续间接返回。修复对齐既有 runtime contract 12，不增加格式版本或另一套调用协议。

新增 core-string／core-floating 两组 fixture，覆盖 Unicode 索引、负数／越界、slice、UTF-8 成功／失败、正常浮点值、signed zero、最小 subnormal 和解析失败；结合原有 JSON 浮点用例，三个 target 各通过 3 组、5 variants、23 processes、15 goldens。Linux LLVM frame 探针同步采用 DirectParts 字符结果，GNU、musl static／dynamic 的 O0／O2 六种组合通过，保留真实 relocation 与间接 slice 回归。容器前后性能对照的两份 runtime 同步包含这一修复，避免让旧 ABI 故障污染比较。

## M34-9b：ArrayList 与 StringBuilder 迁移

ArrayList 使用普通 `MutableArray<MaybeUninit<T>>` 保存容量，初始化与扩容尾部全零；有效槽按值包装，读取在索引检查后使用局部 unsafe 取值。insert／remove 移动完整 wrapper，remove／clear 清零失效槽，完整元素写入后才增加 size。StringBuilder 继续复用 ArrayList，原生拼接边界读取实际 String 前缀并在分配后重读已发布的 backing root，没有增加专用扫描形状或中间数组复制。

新增 storage／builder／artifacts 三组正式 fixture，覆盖 Int、引用、双字接口、含引用值、Option、ZST，扩容／插入／删除／clear、尾部全零、old→young 跨卡写入、删除后不再保活、Unicode／重入／重复 build 与 JSON。跨 Cone 使用相反 profile，删除双方源码后仅凭产物重新链接；debug／release 均执行普通、full-moving 和 minor-stress。三个 target 的三组新增及十组既有列表／拼接回归全部通过；musl 首轮两项冷构建超时后单独补跑通过，没有放宽测试时限。

五份旧公共 LIR 的 16-byte tagged 返回在 Darwin 与 x86_64 的物理 carrier 确有差异，改为 target-specific 快照，HIR／MIR 继续共用。最终关闭快照更新，在 Darwin、GNU、musl 各验证七组受影响入口，均为 10 variants、52 processes、41 goldens。原有浮点 JSON 组合的三个 target 快照同步记录新 core，实际运行通过。

两机 workspace/all-targets clippy 与 C 警告检查通过。生产实现保持 ArrayList 95 行、StringBuilder 19 行、原生拼接 44 行；新增测试及基准按实际功能分文件。容器性能在固定编译器／runtime 下七轮交替对照，Int stride 16 → 4、ZST 8 → 0，引用／接口大小保持原值。大容量低占用列表仍扫描完整 capacity，部分应用组合略慢，全部样本、阶段成本、机器码与产物大小见 [性能记录](PERFORMANCE.md)。本批测量结束后再次检查两地 target，无新增可删除的中间对象；记录为 `batch9b-target-cleanup-{darwin,linux}.json`。

## M34-9c：索引更新复用真实实参

容器基准暴露一个迁移前已经存在的问题：复合赋值按 `$argument.N` 名字回查索引临时值，依赖调用使用另一命名，getter 缺省表达式中的嵌套调用也会覆盖查找结果。现在在选中 getter 的实参物化完成后保存实际 typed local，写回 set 时复用这些原始显式索引；内层 place 和候选探测沿既有上下文保存／恢复，不改变 HIR 或产物格式。索引 lowering 拆为 60 行模块，原 places 文件减少 56 行。

新增跨 Cone updates fixture 在旧编译器上复现了 Long 索引误取 Int 的错误；修复后检查本地／依赖 getter 默认参数、set 自身默认参数、vararg、多索引、MutableList／ArrayList、嵌套 prefix／postfix、异常与 GC 的单次求值顺序。三个 target 的该用例和五组旧运算符回归均通过，各为 8 variants、21 processes、24 goldens；新用例再关闭更新严格执行，各为 2 variants、12 processes、12 goldens。两机运算符 HIR 单测与 workspace/all-targets clippy 通过。旧两份 LIR 按实际 carrier 差异分 target，公共 HIR／MIR 保持跨主机一致。

## M34-10a：总验收发现的边界修复

Darwin 的一次完整 Rust 工作区运行完成 5,404 项测试，初跑 5,343 项通过、61 项失败；保留初跑结果，按失败项定向补跑，不重复整个工作区。公共 fixture runner 的 44 项测试通过。ABI／core 预期迁移及正式文件 fixture 总验收仍在继续。

多文件 HIR 测试暴露了 companion 延迟登记时沿用上一源文件上下文的问题。`declare_singleton` 现在先恢复其已有 `file` 参数对应的上下文，再登记注解与成员，MaybeUninit 的 core intrinsic 因而取得正确源文件归属。18 项真实 core／用户源码组合回归在 Darwin、Linux 通过，未放宽用户声明 intrinsic 的规则。

原生对象确定性测试的预处理字节完全一致，Mach-O 的调试信息却包含随机 `/private/var/.../scoop-native-*` 目录。编译临时目录现在先取得物理路径，使传给编译器的路径与既有 prefix map 一致；继续保留完整对象字节比较。两机定向测试通过，未对测试输出做摘要或路径归一化。两机工作区 all-targets clippy 通过；本批生产文件分别为 283／141 行。

## M34-10b：LLVM 内存 helper 的普通链接要求

Linux 的跨 Cone nominal 签名和完整 core 产物测试暴露了小值 coercion 清零产生的 `memset` relocation 未登记。实现规范先补充其既有 target-support tag 8、C 机器签名与 NoGC／不抛异常合同，再由普通 target-support registry 保存真实 libc 引用；没有增加新的链接机制。对象读取共用 66 行 memory-call 模块识别 `memcpy`／`memset` 的真实直接调用，普通外部调用与 Managed invoke 仍按各自合同处理。

新增 57 行对象测试在三个 target 的 debug／release 下分别生成真实机器对象，验证两种内存 helper 与普通外部调用的区别。两机 workspace/all-targets clippy 通过；Darwin 的 13 项 helper／runtime ABI／产物检查通过，Linux 的 18 项定向检查通过，包含原来失败的 nominal 签名、完整 core 导出、初始化单元、dispatch 表和精确根计数。Linux 三个配套 CLI 已重新构建。

## M34-10c：既有 Rust 回归迁移

按真实新 ABI 和 core 定义更新旧测试：小值检查完整 DirectParts storage／coercion，原 sret 用例改用 24-byte 值以继续覆盖间接返回；接口 default 表项检查必需的 receiver adapter；容器检查 MaybeUninit 实例化和新增 companion 初始化单元。原子指令测试将用户操作与 page-map 的原子 metadata load 分开计数，条件 poll 的精确根测试继续同时核对实际 stackmap 与注册记录。

更新 target／C bridge、产物 capability、语义与缓存的完整固定向量，保持字节和摘要断言。Darwin 初跑的 61 个失败项已经逐项通过定向补跑闭合；Linux 对同一清单及新增 helper 回归完成复验，其中两项真实产物错误由上一批修复。没有再次运行完整 Rust 工作区，也没有降低格式、ABI、引用或 GC 检查要求。

## M34-10d：最终对照程序与复跑脚本

新增跨 Cone 接口基准，将已知／未知 receiver、循环前转换一次／每轮转换四种模式分开；纯计算 GC 响应基准复用已有 callback runner 和线程观测点，工作线程执行普通 Managed 整数循环。两台主机的 M33／M34 版本均已构建，Darwin 的 MIR／LIR 与机器码映射已提取；Linux 七轮交替测量完成，独立 checksum、整数和与 GC histogram 检查全部通过。Darwin 计时等待该机完整 CLI 验收结束。

实际使用的构建、测量与结构提取脚本保存在 [最终对照基准](../../tests/benchmarks/m34/README.md)，分别为 185／143／59 行；新 C 辅助代码 39 行，通过格式和严格警告检查，Python 通过指定版本 ruff。完整样本与解释随总验收归档。Linux 此时再次检查 target，没有新增可删除的 rcgu 对象或 incremental 目录，未删除有效工具和基线。

## M34-10e：复制值中的 managed pointer provenance

Linux 的 release 接口协程用例在 unbox 后复制含引用 struct，再以 byval 参数调用挂起适配器。LLVM 将复制降为整数 load／store，随后把 GC leaf 的 pointer load 合并为新引入的 `inttoptr`，触发既有 provenance 校验。最小 LLVM 复现确认是缺少 managed 地址空间的 non-integral 声明；module 现在从已有目标 data layout 派生 `ni:1`，保持原指针大小、对齐与原生 ABI，也不放宽显式转换检查。

新增 22 行 LLVM／59 行 Rust 回归在三个 target 的 debug／release 下执行优化、RS4GC、Scoop 根验证、LLVM safepoint verifier 和真实对象生成，保留一个正确 relocation root，禁止优化器引入整数重建。两机 workspace/all-targets clippy 通过；Darwin 的 362 项、Linux 的 377 项 codegen 测试通过，数量差异来自既有平台条件测试。原接口协程正式 fixture 已在 Darwin／GNU 完成普通、full-moving、minor-stress 的 debug／release 运行；两机关闭快照更新复验通过。临时 LLVM 诊断输出已从生产代码移除。

## M34-10f：intrinsic class 的 default receiver

完整 CLI 初跑发现六组修改 core 后再从产物继承成员的用例失败：String 的接口 default adapter 使用了普通 `Class` 类型，而其 source exact registry 正确保存的是 intrinsic `String`。adapter 的参数、局部变量、receiver 表达式和 MIR 检查现在统一使用类声明已有的 `physical_type`，同时覆盖 Any；没有增加别名、名称回退或重复身份登记。

两机 20 项 MIR 引用类型回归通过。六组 core 重建／消费用例及上一批的接口协程在 Darwin、GNU 均完成验收，各为 8 variants、60 processes、22 goldens；包括源码删除后的链接与 moving GC，以及原有 NoGC／错误实参诊断。两机对这七组关闭快照更新再验一次通过。生产修改限于已有 receiver lowering 和相应结构验证。

这两个修复后，两台主机各自重新构建了十个最终基准可执行文件。与修复前逐字节比较全部相同，热点机器码大小也相同，因此复用已经完成的 Linux 七轮样本；原始测量、修复后构建与 binary 对照分别保留，不重复计时。Darwin 的首次最终计时仍等待该机 CLI 验收结束。

## M34-10g：core 非法声明的诊断位置

完整 CLI 的预期迁移中，六组缺失或非法 Any／Nothing 声明用例保留了旧 ArrayList 文件范围。错误文本、所属 Cone、文件和起点都未改变；终点按真实源码大小从 3036 更新为 3336 bytes，继续比较完整诊断 JSON。Darwin 对六组关闭快照更新复验通过，未扩大测试时限或删除位置断言；相同预期已同步到正在执行的 GNU 选择中。

## M34-10h：既有 native 与产物损坏用例

四组 Scoop ABI native 用例在普通 GC 后强制要求对象地址变化，与 M34 的选择性搬迁不符。101 行 C companion 现在在普通模式检查内容和根回写，在明确开启 moving-stress 时继续要求每次实际搬迁；24-byte 含引用值的 Scoop byval／sret shim 保持原约定。两机 C 格式／严格 warning 检查通过；Darwin 的四组普通／moving、源码构建／删除源码后重链接均通过，随后关闭快照更新复验通过。

Darwin 专用产物损坏用例按真实新 `.slib` 的 158558 bytes，将截断长度更新为 158557，继续只删除最后一个字节。完整错误文本、损坏 member／magic、缺失和错误文件种类、依赖错误，以及所有失败后保留既有 executable 字节的断言均通过严格复验，没有用宽泛诊断匹配替代原检查。

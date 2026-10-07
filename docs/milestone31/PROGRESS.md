# M31 实施记录

日期：2026-10-07。基线：`6e62514da`（M30）。设计见 [DESIGN.md](DESIGN.md)。

## 实施顺序

1. 配置与基线：保存 M30 可复跑数据；贯通 debug/release、child 请求、producer 与缓存。
2. ODR 与 image：删除正文判等，按身份和 ABI 选择物理实现，迁移 metadata ABI 5。
3. 首批优化：局部传播、LLVM 函数内 pass、最终 GC 发射计划。
4. 分配与屏障：跨 line 普通分配、精确 size、引用范围写屏障。
5. nursery：年轻代 TLAB、minor、首次存活晋升、pin 与 full fallback。
6. 三目标验收及性能对照。

每项功能完成后单独提交；先格式化与 lint，再执行受影响的单元测试、正式 CLI fixture 和必要平台组合。复用未变化的通过结果，不逐批重跑全量。阶段快照保留实际类型、ABI、GC 与引用结构，不固定无关代码摘要。

## 当前状态

- 已读取设计与对应规范；工作区原有 M31 文档作为实施基准保存。
- Linux `nuc12:~/repos/scoop` 留有 M30 测试变更；Linux 验证将使用独立目录，保留原目录内容。
- M31-1～M31-5 的主要功能已实现；正在完成三 target 总验收与性能报告。Darwin 的独立链接组合及 GNU 混合优化 ODR 已通过，musl 收尾验收进行中。各批实际改动、版本与验证结果记录如下。

## 性能基线与构建清理

- 从 `6e62514da` 重建 release 配套命令，连同 M30 runtime/core 保存到 `/tmp/scoop-m31-baseline/`，可在后续变更后独立复跑。
- 新增六个普通 Scoop 性能程序与 100 行内的 CLI 测量脚本。Darwin 每项运行五次，核对确定输出，记录冷/热构建时间、执行时间、可执行文件和 `.slib` 大小；数据见 [M30-DARWIN.json](M30-DARWIN.json)。旧 runtime 没有完整 GC 统计，不填造分配/扫描/停顿数据。
- Python 按 Ruff 0.16.10 格式化与 lint；六项真实编译、链接及运行通过。用例准备阶段的 `gc.collect()` 改用现有公开测试入口 `gcCollect()` 后，相关四项重新执行，报告仅保留成功运行。
- 清理闲置 `target/debug/incremental` 与 M28/M29 专用目录约 11 GB；保留配套 release 命令与可复用依赖。
- Linux glibc/musl 各完成同一组六个程序、每项五次运行，记录见 [M30-LINUX-GNU.json](M30-LINUX-GNU.json) 与 [M30-LINUX-MUSL.json](M30-LINUX-MUSL.json)。Linux 工具来自原 M30 `9e4c411f2` 构建，runtime/core 源码来自 `6e62514da`；报告保留这两个 revision，后续比较使用相同环境。

## M31-1：实际优化配置

- 增加封闭的 `OptimizationMode::{Debug, Release}`，沿 umbrella、graph、child 协议、普通 `scoopc` 请求和 producer 贯通。Scoop machine 使用 O0/O2，generated-C 使用 O0/O2；runtime 保持现有 O2。LLVM 普通 IR pass 与最终 GC 计划尚属 M31-3。
- child protocol 升为 4；single-cone production capability 升为 4，manifest field 12 保存模式并进入 Code/Artifact 内容摘要；compile-cache domain 升为 `scoop-cone-compile-cache-v2`。优化模式不进入语言身份或共享 ABI。
- LLVM O2 实际产生 Mach-O `LC_LINKER_OPTIMIZATION_HINT`；reader 接受此标准命令并验证长度、文件范围及与已占用数据的非重叠，新增合法／越界／重叠测试。将 request 解码与 Mach-O 表读取按职责拆成子模块。
- Darwin 正式 profile 用例的两个变体通过，覆盖 C bridge、冷／热构建、普通与 child 产物一致、release full-moving GC；旧 CLI 用例与四份阶段快照通过。
- `nuc12:~/repos/scoop-m31` 使用 LLVM 22.1.2 构建，glibc/musl 同一 profile 用例各两个变体、12 个进程均通过。原 `~/repos/scoop` 源码未修改；复用其 target 与 native sysroot。
- 格式化及 workspace clippy 通过；定向 protocol 24、C bridge 16、manifest production 6、cache key 5、scoopc CLI 7、Mach-O optimization hint 1 项通过。后续批次复用这些结果，不重复全量测试。

## 宽值参数的 EH 检查

- LLVM 为大结构体 `byval` 生成的标准 `memcpy` 可以位于已有 LSDA 保护区间。根据直接调用指令及外部 `memcpy` 重定位识别该不展开调用，避免误报额外 LIR invoke；未知调用、managed site 和 landing pad 的检查保留。
- 定向 EH 19 项通过，包含额外复制调用、未知调用拒绝和 managed site 缺失拒绝。528-byte 值的真实 `.slib` 编译通过；跨卡复制组合用例在 Darwin debug、release、full-moving 三个变体通过。

## M31-3 局部清理：整数条件

- 扩展原有局部布尔清理，传播 typed integer 常量与复制，并折叠 `IntegerCompare`、语言比较 lowering 使用的 `IntegerCompareTo`。按声明宽度及有无符号解释边界值，沿原取址和调用失效规则处理；不新增跨过程分析。
- 已知条件的死边在 poll、root plan 和 site identity 定稿前删除，修复 release 删除不可能的异常路径后站点清单仍残留的问题。
- 定向边界与复制测试 2 项通过；独立整数条件 fixture 在三 target 的 debug/release 下均通过，实际运行启用 full-moving，HIR/MIR/LIR 快照锁定删除死分支后的结构。数组长度与跨卡复制作为组合覆盖。完整 M31-3 的 LLVM pass 和最终 GC 发射计划仍待完成。

## M31-4：普通分配与范围屏障

- 普通对象上限改为 32640 bytes，TLAB 和 evacuation 连续跨 line bump；精确 size 使用 uint16 的 8-byte units，相交 line 全部标记。refill 保留放不下当前对象的短 free run，对不同 mutator 共用 side bitmap 的位操作使用 atomic。
- GC-free/scalar heap store 不再标卡；单个引用保留内联 atomic OR，多引用 value、enum、数组初始化/组装/复制、box 和 Context 使用完整目标引用范围。新增 GC-leaf `scoop_rt_gc_write_barrier`，runtime ABI contract 6→7，更新必要的编码向量。
- 分配、屏障及既有运行时组合定向测试 46 项通过；后补数组初始化/组装屏障的 codegen 14 项、runtime ABI 4 项、compatibility 4 项通过。C 使用严格告警语法检查，Rust fmt/clippy 通过。
- 两个正式 CLI 用例在 Darwin、glibc、musl 各通过 debug、release、full-moving，合计 18 个运行变体。覆盖 80-byte 连续对象、跨 card 的 528-byte value/enum、引用数组、array clone、box/unbox 和 full GC 后内容保持。
- 本批尚未消费 remembered set；其真实 old→young 存活性由后续 nursery 批次验收。再次清理约 10 GB 的 debug 增量产物，后续构建关闭增量编译。

## M31-5：nursery 与晋升

- 按 block 保存 Young/Old；普通对象通过真实 TLAB 进入 1 MiB nursery，large 和 release-hook 对象直接进入旧代。generated fast path 检查 null hook，runtime ABI contract 7→8。
- minor 共享完整根访问，递归追踪仅限年轻对象；旧代沿脏卡、object-start/size 和精确 scan 访问相交引用，大数组按 card 裁剪元素区间。年轻存活对象首次晋升，含 pin 的 block 原地转旧，旧代垃圾留给 full。
- minor/full 共用先预留、后复制的 evacuation plan。部分预留失败会释放目标且保持原图，在同一 STW 转 full；normal full 无 to-space 时可原位保留。normal 不再重复全堆 verification walk，显式 metadata 验证与 full stress 保留。
- 拆分根访问、范围 scan、remembered set、空间规划与统计，修改后各 GC C 文件均约 350 行以内。新增实际分配/晋升/扫描/停顿计数与退出 JSON；计数不参与语言合法性或兼容性。
- 首轮定向 48 项通过；随后新增多线程 native/callback/冻结段根和同一卡并发写入 2 项通过，native recursive region 补测通过。真实空间不足测试在已预留一个目标 block 后失败，再 full 并继续分配，数据保持。
- Darwin 三个正式用例共 9 个变体通过；新增构造期间晋升、ready/unready release hook、宽值/数组/box/Context 组合的三个变体通过，包含真实 minor stress 与正常容量触发。runtime ABI 与 compatibility 各 4 项通过，JSON 统计已实际运行并解析。
- Linux 本批验证待补：`nuc12w.0d0a.com:22` 连续连接超时，未把前一批 Linux 结果当作本批通过。继续其余实现后重试。清理新增的约 1.1 GiB debug 增量目录。

## M31-2 物理边界：正文附属登记

- 每个 callable 对象包含自己的 callable/safepoint registration、Context key 表与 cell，以及原有 EH/stackmap/私有 scan；无 Context 的登记同样随正文。公共 metadata 对象不再定义正文登记。
- 分区直接使用已有 typed body/site owner，保持所有 definition 的完整、不重叠覆盖；未增加新身份或分组框架。原 Context 专用发射模块改为统一正文登记发射，公共发射代码减少。
- Rust fmt 与 workspace clippy 通过；真实 object、NoGC、登记、patch 侧表定向 15 项通过。Darwin nursery 的 debug/release/normal 三变体、跨 Cone Context 的 artifact-only link/run 与九份阶段快照通过，合计 19 个正式进程。
- 本批仅建立可同选的正文物理边界；独立 ODR 数据分区、显式 primary 选择、最终 image 和 definition 字段删除继续实施。

## M31-2 物理边界：独立 metadata 与候选 image

- 候选 image 及六类表独占一个对象；非 callable 的 ODR 定义各自与 associated atoms 成组，普通 Strong metadata 继续共用对象。统一发射一次 metadata LLVM，再按已有 definition/atom 索引投影；分区复用索引，不增加新的后端或对象解析框架。
- 跨分区的类型描述、interface 和派发表引用复用已经解析的当前 Cone typed relocation binding，不再要求目标恰好位于同一 native member。保留实际身份、角色与范围检查。
- Rust fmt 与 workspace clippy 通过；codegen 物理分区 6 项、reader 类型登记与引用闭包 26 项通过。Darwin 跨 Cone Context 的 artifact-only link/run 与九份快照通过；TLS 的 normal/full-moving 两变体、源码移除后独立链接与运行、八份快照通过，合计 22 个正式进程。
- TLS 的 Darwin LIR 快照同步已实现的整数条件清理，只删除已知非零除数对应的不可达除零分支。Linux 快照留待实际目标运行更新，未以 Darwin 结果代替。
- Context 用例拆分后的首轮耗时约 58 秒，复用符号索引后约 48 秒；前一批约 27 秒。这是物理分区的实际编译开销，后续性能报告继续记录。清理新增的约 0.90 GiB debug 增量目录，保留可复用依赖与 release 命令。

## M31-2 物理选择与最终 image

- program-link 按 coordinate 的 group/name/version 顺序，在已有 typed 物理索引上选择 ODR primary。正文附属登记与原始 stackmap/EH 同选，独立 metadata 分别选择；定义、未定义引用与实际对象输入均过滤落选 member。Darwin 的最终 link map 逐一核对所选 ODR primary 与有引用的 atom 归属。
- 原候选 image 对象退出 native 输入；startup C/native-object 路径生成每个逻辑 Cone 的最终 image、六类 pointer 表与既有 trap-message 符号。复用 RuntimeImage 编码计算实际所选登记的摘要，输入 artifact 不改写。链接计划域升级为 `scoop-resolved-link-plan-v3`，直接覆盖所选对象和新 startup 源码/对象。
- 最终 image 检查针对新生成的 prefix、coordinate、依赖、摘要与登记表/count/binding；复用已有目标映射和权限读取。修复通用 Mach-O 对象库将标准 `__DATA_CONST,__const` 定义标成 Unknown 时的 native data 识别，其他未知节和指令属性仍拒绝。
- Rust fmt 与 workspace clippy 通过。4 项定向测试通过，包含真实 C metadata、最终可执行文件的 image 摘要/表项数量损坏拒绝和实际选择与 link map 不一致拒绝。Darwin 跨 Cone Context 用例通过；新增四 Cone 独立用例的 debug/release、artifact-only link、倒序依赖输入、normal/full-moving/minor stress、六份阶段快照通过，合计 20 个正式进程。
- 本批建立实际选择与最终 image，旧 ODR 内容判等和公共 definition 字段将在下一批删除，混合优化验收随之补齐。Linux SSH 仍超时，未声明本批 Linux 通过。清理新增的约 0.69 GiB 闲置增量产物。

## M31-2 兼容实现与依赖语义

- 跨产物 ODR 合并只比较完整 group/member key 与共享 ABI，删除 canonical LIR、机器码与 stackmap 的内容判等及专用差异诊断。原先绕过普通依赖图、把不同源码当作同一定义的三个测试，改为同一模板的 debug/release 真实对象测试；重复 artifact 仍拒绝。
- LIR dependency semantic 改用已有 layout/descriptor/scan/dispatch/callable/shape-support 导出与普通 callable bridge。本地 foundation 和完整 production 不再贡献依赖语义，backend 配置移出该摘要；target 与 runtime/identity ABI 保留。域为 `scoop-lir-semantic-v2`，不新增正文摘要或 ABI 副本。摘要实现按编码与测试职责拆分，主文件 426 行。
- 正式 ODR fixture 覆盖两种相反的混合优化组合、缓存键变化、消费者零次重编译、最终链接失效及重复输入稳定；移除源码后倒序传入产物链接，并运行 minor/full moving。真实泛型模板增加 GC 调用后，旧消费者仍以 StaleDependency 拒绝且不发布输出。
- workspace fmt/clippy 通过；摘要与 registry 定向检查 8 项、真实同 key/ABI 不同机器对象测试 1 项通过；Darwin 正式 fixture 两变体共 20 个进程、6 项 stage golden 通过（37.30s，复用编译缓存）。Linux SSH 仍超时，本批 Linux 验证待补。
- metadata ABI 5、公共 definition 字段及仅服务这些字段的摘要节点/正文编码将在下一批迁移，本批不声明 M31-2 全部完成。

## M31-2：metadata ABI 5 与专用正文摘要退役

- 六类 registration 删除公共 definition fingerprint，公共 identity 由 136 bytes 缩为 104 bytes；其余实际 body、descriptor、layout、scan、stackmap 和 gateway 字段保留。C/LLVM/reader/final-image 布局统一为 ABI 5，runtime ABI contract 为 9，RuntimeImage 使用 `scoop-runtime-image-v2`。
- 删除 LirDefinition、OdrDefinition、StrongRegistration 的类型、节点、补丁、正文编码及只服务这些字段的对象摘要。callable/shape 表只记录既有 typed owner 和共享 ABI；manifest field 5 的 Strong registration 摘要表退出。受影响 capability 为 LIR foundation 6、strong production 20、cone production 9、layout ABI 10、link identity closure 15、manifest production 5；普通物理引用 closure 保持 5，退役 tag 不复用。
- 六类读取路径复用已验证的语义和物理结果，删除重复的 digest graph 重放；immortal String 的长度、UTF-8、padding 和 TD relocation 检查移入对象读取边界，保留必要负例。实际字段回填及最终对象内容仍按普通格式和引用规则验证。
- 最终回填模块按覆盖、回填、记录和内容检查拆分，主文件约 350 行；原 2798 行 Mach-O fixture 和 1054 行语义 fixture 按职责拆分，主文件分别为 169、380 行，子模块不超过约 650 行。本批净删除约 1.3 万行专用摘要代码和旧测试。
- Rust fmt、workspace clippy 通过；identity 340 项、LIR production/canonical 189 项及 runtime ABI 定向检查、slib 全部 563 项、C/LLVM metadata 与 image 40 项、GNU/musl ELF 交叉对象检查、真实泛型对象回填与关联 EH/stackmap 内容检查均通过。修正了正文登记同放一个对象后旧测试取第一个 definition 的错误假设，按 typed role 查找实际正文。
- Darwin 正式混合优化、nursery/native/Context/跨卡复制、两种真实模板冲突共 7 个变体、57 个进程、54 份 stage golden 通过。冲突用例继续检查独立链接结果一致性、真实运行和 stale 拒绝，移除与冲突规则无关的整份 core startup 源码快照。混合优化本轮冷缓存用时 108.09s，未与前一轮热缓存耗时直接比较。
- Linux 原生运行仍待补：本批 SSH 再次连接超时。交叉发射只证明 ELF 对象生成及读取，不替代 GNU/musl 上的 nursery、启动、链接和运行验收。

## M31-3 局部清理：整数运算与已知 variant

- 沿基本块和唯一前驱直线链传播已经求值的整数、Boolean 与完整 typed variant；在合流、回边和相关未知内存写入处失效。整数按实际宽度 wrapping，转换保留源符号性，shift 只消费归一化计数；除零和 MIN/-1 不折成合法 primitive。
- 已知 enum/Option 构造与普通复制可以消除匹配失败路径，payload 求值、复制和 GC 保活保留。现有 codegen variant 支配检查支持未改写的构造及复制，覆盖写入和逃逸存储的未知指针写入仍拒绝无依据的投影。
- 常量分析拆分为主模块、整数和已知值模块，分别约 183、179、110 行；variant 结构检查与支配分析分别约 247、315 行。未增加跨过程分析或通用优化框架。
- Rust fmt、workspace clippy 通过；LIR lowering 154 项、enum codegen 21 项通过，包含整数边界、精确/未知指针写入、构造复制支配及失效负例。同步修正一个 ABI 5 遗留测试，以真实 metadata 字段检查 RuntimeImage 输入集合。
- Darwin 正式 local-values 与 local-effects 用例通过 debug/release、full-moving/minor stress，共 5 个变体、10 个进程、15 份 HIR/MIR/LIR 快照；无快照更新复测用时 22.92s。快照确认整数和已知 variant 的死边已删除，运行覆盖 payload 副作用、取址别名、循环合流、除零、MIN/-1 和 finally。
- Linux SSH 仍超时，本批 GNU/musl 原生用例及快照待补。LLVM 函数内优化与最终 GC 发射计划继续实施。

## M31-3：LLVM 优化与最终 GC/EH 发射计划

- release 使用 SROA、mem2reg、InstCombine、EarlyCSE、DSE、ADCE、末次 EarlyCSE、SCCP 和 UnreachableBlockElim，再执行 RS4GC；debug 使用 SROA、mem2reg、SCCP 和不可达块清理。末次 CSE 合并死存储删除后暴露的相同加载，SCCP 在计划定稿前清除常量分支；没有启用跨函数内联、调用复制或完整 `default<O2>`。
- 删除仅用于钉住旧 leaf 身份的专用 alloca 和 volatile load。真实 canonical root 回写保持 volatile，其前方的零机器码 `llvm.fake.use` 携带 typed source/offset；普通优化后按 SSA 等值分组，常量不占 relocation pair。RS4GC 后按 relocate token/index 核对这些组，并保留旧值使用、derived 地址及实际 stack-only 检查。
- 循环旧值检查复用已有 CFG 路径规则，区分跨回边重新定义的 SSA 动态值与实际旧值跨 safepoint 使用。终止调用的保活标记放在真实回写之前，避免 LLVM 清理 `unreachable` 前的尾部 intrinsic；两种情况都有正反或实际对象回归。
- 先准备 callable，形成实际 GC/EH 计划，再投影 production 和发射候选 image。删除不可达 site 的登记、私有 ODR member、definition/atom 和摘要输入；无剩余 invoke 的 body 删除 personality 及相关 EH atom。其余类型、扫描和初始化数据复用原结果。
- backend contract 新增 field 28=1；strong production 20→21、cone production 9→10。runtime ABI 9、registration ABI 5 和 stackmap v3 保持；普通编码向量和不兼容旧版本检查同步更新。发射、EH 投影、SSA 分组及 CFG 路径代码按职责拆分，新模块均约 300 行以内。
- Rust fmt、workspace clippy 通过；codegen 定向 62 项、LIR backend/foundation/production 227 项、slib profile/compatibility 23 项通过。真实交叉对象覆盖 Darwin、GNU、musl 的别名根 2→1、常量根 1→0、release 删除死 invoke/登记及 image/COMDAT 关联。
- Darwin 新增 SSA roots 正式用例通过 debug/release/full-moving/minor、源码移除后的独立链接与运行，共 3 个变体、12 个进程、9 份阶段快照（56.86s）。组合回归的浮点、Context、协程、nursery、跨卡复制及混合优化 ODR 已通过；另修正冷缓存用例与共享 core 缓存的冲突，以及协程用例遗留的完整 startup 快照，保留真实链接计划一致性和运行检查。两个修正用例单独复测通过，共 4 个变体、20 个进程和 8 份阶段快照。
- 使用 Cargo 按包清理闲置 debug 产物 2.4 GiB，保留 release 工具。NUC SSH 继续超时；已建立独立 Debian 13/amd64 容器，LLVM 22.1.8，实际执行 GNU PIE、musl static/PIE cleanup/catch/delete 探针均通过。GNU 的七个 M31 用例通过，包含 SSA roots、局部值、nursery/native 和跨卡复制；首次混合 ODR 运行发现 ELF 匿名只读数据缺少 atom 归属，继续修正，冷缓存用例亦需在新声明下复测。容器结果不作为 NUC 原生性能数据。

## M31-6：可重复的 GC 性能对照

- 增加默认关闭的 `SCOOP_GC_FULL_ONLY=1`，在原 nursery refill 压力点请求普通 full；分配容量、布局与屏障不变，不使用 full-moving stress 充当单代性能。内部统计增加 minor/full 暂停总量与八档分布，不改变公开 `ScoopGcMetrics` 或 runtime ABI。
- 测量脚本每个 profile 只构建一次，同一 executable 交替运行 nursery/full-only，检查输出、实际收集种类与暂停计数。新增八倍旧图和高存活率/pin 两个程序，原六个基线工作负载不变。
- Rust fmt、workspace clippy、C 严格告警与 Ruff 0.16.10 通过。五项 nursery 定向测试通过，包含 refill/full 对照、脏区与 pin、晋升失败、多 mutator 和 callback/frozen roots；两个新程序在 Darwin debug 的两种收集模式实际编译运行通过，输出分别为 `199680`、`196614`。该轮与构建并行，仅用于功能验证，不计入性能结果。
- 最终报告分别比较同 revision 的 debug/release × nursery/full-only，以及整个 M31 与同环境 M30；实际性能测量与 Linux 验收继续进行。

## 物理分区后的空 span

- ELF 可把没有重定位的全零空哨兵放入独立只读节；对象读取接受这种填充，继续拒绝未归属的非零数据和可写/代码节。空 span 保留元素范围、alignment、只读权限及不与实际 atom 重叠的检查。
- 删除不同记录必须使用同一 sentinel 地址、不同 sentinel 必须有不同地址的额外检查。空数组没有语言身份，不同物理成员可各自携带全零数据；修复泛型 delegate 的多个独立 storage 分区被误拒绝的问题。
- fmt 与 Darwin/Linux workspace clippy 通过；符号/填充 6 项、静态存储 15 项通过。Linux 三个真实 ELF 测试均覆盖 GNU/musl 的静态初值、独立全零节、前缀和损坏数据；修正旧测试 helper 在多 member 下未按 intent 排序 patch site 的假设。Darwin delegate-siblings 的正式独立链接与运行复测通过。

## Darwin 大量对象的链接参数

- 独立 metadata member 增多后，较长工作目录中的协程用例超过 Darwin 的进程参数长度限制。对象路径改经系统 linker 的标准 response file 传递，实际顺序、内容、link map 与 trace 检查保持。
- fmt、workspace clippy 通过；原生输入替换/独立链接测试新增含空格、逗号、两种引号和反斜杠的目录，真实系统 linker 接受路径，运行结果与链接计划保持一致。触发长度限制的 coroutine-lifecycle 正式用例复测通过。

## 链接 fixture 迁移

- 移除 27 份声明中的 85 个旧版完整 startup/link-plan 快照，保留本次构建与独立链接的实际 fingerprint 一致、原生输入/Cone 关系、所有阶段结构和运行结果。源码/运行时变化仍检查链接失效及输出；重建 core 继续核对实际 String owner。
- 损坏用例按当前 Mach-O 产物确认 payload 修改位置与尾部截断长度，保留完整诊断、失败位置和原子输出。普通函数源码变化不再改变 LIR 的共享 ABI 摘要；旧 stale 负例按现有 HIR 观察面继续拒绝，并锁定实际摘要相等关系。
- 36 个独立链接 fixture 全部通过，合计 57 个变体、272 个正式进程、280 份阶段快照；先执行整组，之后仅复测失败项。结果见 [M31-DARWIN-LINK.json](M31-DARWIN-LINK.json)。公共 runner 的 44 项测试通过。

## musl 暂停计时的 POSIX 声明

- collector 在所有系统头之前声明 `_POSIX_C_SOURCE=200809L`，使正式 runtime 的严格 C11 编译可以使用 `clock_gettime(CLOCK_MONOTONIC)`。此前单元测试命令已有该宏，正式 musl 构建暴露了缺项；计时语义、公开结构和 ABI 不变。
- Darwin cc 与 musl-gcc 均在不额外传入 feature macro 的情况下通过 `-std=c11 -Wall -Wextra -Werror` 语法检查。musl 九个 M31 fixture 的实际构建、链接与运行全部通过，23 个变体、73 个进程；本轮同时生成目标快照，普通模式复验与平台组合随后继续。

## Rust workspace 总验收

- 完整执行 `cargo test --workspace --no-fail-fast`，共 5325 项，5321 项首次通过、4 项旧断言失败，无忽略项。新增 mode/backend 字段的两个固定向量、诊断中的 cone-production major，以及物理分区后的旧对象数量假设分别迁移；保留受控编码向量，物理对象改为核对 LLVM dump/实际对象的完整 typed unit 集合，以及写入时/重新读取后的实际成员数量。
- fmt 与 Darwin/Linux workspace clippy 通过。仅定向复测失败项：runtime ABI 向量 1 项、依赖诊断 1 项、core/layout 端到端 2 项均通过；最后两项 396.62 秒。当前全部 5325 项均有有效通过结果，不再重跑已通过的 workspace。

## Linux 栈边界与完整 codegen 验收

- Linux 完整 codegen 运行 351 项，350 项通过；旧栈增长测试独立链接 thread.c 时漏了 M30 浮点环境入口。测试现链接真实 floating.c 与 libm，并用函数节回收去掉此独立 probe 不使用的格式化/分配函数，没有补空实现。
- 模拟环境的 musl 在 attach 时已给出整段主线程栈范围，原先假定自然增长必然刷新四次的断言失败，实际观察刷新次数为零。probe 现在每段深递归前以实际 OS 范围内的地址设置窄缓存，强制 managed entry、managed anchor、native transition、callback 四条发布路径重新查询真实范围；四次刷新要求也适用于 glibc。
- C 严格告警与 workspace clippy 通过；该测试的 GNU PIE、musl static/PIE × O0/O2 六种组合全部通过，18.32 秒。保留其余 350 项结果，Linux codegen 351 项验收闭合。
- Rust 验收闭合后按包清理闲置 HIR/HIR lowering/parser 的 debug 产物 19.7 GiB，保留活跃的正式 release 工具、fixture 缓存与报告。

## GC 实际复制量

- 性能 JSON 增加内部 `copied_bytes`，在已有 evacuation 执行真实 `memcpy` 时累加；公开 `ScoopGcMetrics` 和 runtime ABI 保持。晋升量继续包含 pin block 原地转代，复制量只记录实际搬迁，能够分别观察高存活率的两种成本。
- C 严格告警与五项 nursery 回归通过。Darwin 高存活率/pin 程序在 nursery/full-only 下均输出 `196614`，两组实际复制量为 6,365,264 / 917,672 bytes，晋升量均为 6,815,768 bytes，收集次数分别为 minor/full 6/1 与 0/7。
- 上述运行与其他编译并行，仅确认统计字段和行为；正式耗时仍由后续串行测量记录。

## 优化后的站点与异常边界

- foundation reader 接受删除不可达站点后留下的稀疏 ordinal，保留原 typed identity、owner/role/ordinal 唯一性及 runtime mapping 的一一关系，不重新编号。优化前完整 LIR 的连续编号约束与实际 stackmap/return PC/根检查保持。对应关系测试覆盖 `[1, 3, u32::MAX]` 的读取及原样编码。
- NoGC invoke 添加标准 `nomerge` call-site 属性，避免 machine O2 合并多个异常抛出调用而破坏最终 EH 清单。只约束有 unwind 边的调用，不把 NoGC 改为 nounwind；Context/协程、debug/release 与 moving 组合在 Darwin、GNU、musl 普通模式均通过。
- release 的 InstCombine 使用默认一轮及 `no-verify-fixpoint`。LLVM 22.1 的默认单轮可能仍留有后续优化机会，固定点自检不能作为合法程序的资格条件；LLVM IR、GC/EH 与对象 verifier 继续运行。GNU/musl 的整数矩阵和 release 迭代程序已实际编译运行成功，快照生成轮与后续普通复验分别记录。
- 三批 Rust 均先 fmt 与 workspace release clippy，再定向验证 foundation 12 项、EH 19 项、普通优化与最终根计划 6 项，全部通过。新增代码位于既有模块，相关实现文件为 72、165、451 行；删除了 50 多行不再适用的最终连续性检查。
- 复验使用固定的配套工具。第二套临时工具完成任务后，按其实际 Cargo 生成目录清理 2257 个文件、1.9 GiB；当前工具与 fixture 缓存继续保留。

## 最终 image 的委托初始化回归

- sibling consumer 的 native probe 按最终选择后的两个 image 检查 2+3 个初始化单元，逐个核对 ODR linkage，并要求五个语义身份及实际 cell/storage/failure root 地址互异。旧的 2+5 断言重复登记了已经选入另一 image 的两个单元，已按最终 image 合同迁移。
- 源码继续验证两个 sibling 访问同一委托的值、单次初始化、失败缓存和 moving GC。Darwin、GNU、musl 的完整正式 fixture 均以普通模式通过；符号期望与各自保留的实际程序一致，没有用 Darwin 的 stackmap 集合替代 Linux 集合。
- probe 保持 38 行，没有新增抽象或资格状态。实际 native 编译继续使用 `-Wall -Wextra -Werror`。

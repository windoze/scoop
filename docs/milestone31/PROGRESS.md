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
- M31-1、M31-4 已完成；nursery 已实现并通过 Darwin 验证，Linux 待补。ODR/image 与完整优化继续实施。各批实际改动、版本与验证结果记录如下。

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

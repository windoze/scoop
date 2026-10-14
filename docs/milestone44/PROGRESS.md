# M44 实施记录

依据：[设计](DESIGN.md)。实施起点为 M43 完成提交 `4f3ec3be`，不把旧 M34 性能数据当作当前基线。

## 当前状态

| 批次 | 状态 |
| --- | --- |
| M44-1 基线、计时观测与 spec | 发布边界、实际 mutator 进度与本机各场景同条件对照已记录；Linux 同机对照待补 |
| M44-2 零搬迁更新分支 | 实现与本机定向验收、图对照完成；Linux 组合待后续统一验证 |
| M44-3 引用定位、写回和无引用对象 | 实现、本机定向验收及普通 full／minor 搬迁和零搬迁对照完成；Linux 组合待最终验收 |
| M44-4 编译器 GC 代码生成边界 | 实现与本机定向验收完成；四类程序的独立编译产物和机器码一致，Linux 组合待最终验收 |
| M44-5 runtime GC 操作与 Scoop 适配 | 根／stackmap／scan、mutator、对象信息与普通 runtime 堆操作边界已实现，通过本机定向验收与完整边界性能对照；Linux 组合待验收 |
| M44-6 组合、平台与性能验收 | 本机各场景性能对照、工作区测试与 49 项 fixture runner 公共规则通过；正式 CLI 全量运行中，Linux 功能与性能验收待执行 |

## 工作约定

- 延续每个完整功能先规范、再实现、格式化／lint、验证及提交的顺序。优化与机械迁移分别提交和测量。
- 工具临时目录为仓库 tmp/m44，生成输出为 target/m44；基线可执行文件和配套工具固定保存，已无用途的生成输出定期清理。
- 本机验证后再执行 nuc12 的 Linux GNU／musl 验证。计时前完成同机编译、测试和清理；对照在同一主机交替运行，至少七次进程样本，不跨主机计算加速比。
- 共享 GC/codegen 路径集成完成后，按设计完成工作区、正式 fixture 全量及三目标适用组合；开发各批先执行受影响范围，不反复重跑无关全量。

## 2026-10-14：实施前核对

- 当前 collector 在规划／复制后无条件重扫 roots、remembered set 与 retained live set；verify_heap_object 又要求 scanned 状态。零搬迁分支必须同时调整这两处职责，不能只加一个条件跳过调用。
- 已有线程测试观测点覆盖 STOPPING、COLLECTING 后以及 RUNNING 发布前的 RESUMING。后者尚不等于设计要求的 T2；计时观测需明确发布边界，保留旧 stop_wait_ns/pause_ns 的定义。
- ScoopGcMetrics 的快照按 world／heap 顺序加锁，后续计时实现须保持同步与锁序。现有 benchmark 已有 parallel 图、region、allocation、poll 与 FFI 工作负载，优先复用真实输入及原始输出检查。
- 当前只登记实施和建立基线准备，不宣称任何 GC 优化或性能收益已完成。

## 2026-10-14：collection 发布边界观测

- 运行时规范 3.5／3.9 明确三个发布边界及既有 stop_wait_ns／pause_ns 的不同口径。新增 RUNNING 发布后的线程测试观测点，原 RESUMING 点继续位于发布前；生产构建仍完全移除测试回调，不变更公共 ABI 或快路径。
- 图基准可选记录 STOPPING／COLLECTING／RUNNING 原始单调时间及三个差值，并直接记录本轮 moved_objects。观测仅用于专用测试 runtime；不能据此宣称测得 mutator 实际恢复时间。
- 格式化、workspace clippy 和 C 的两种宏配置语法检查通过。NativeSafe 握手回归在 full／minor × O0／O2 四种组合各执行 60 次受控交错，验证发布顺序、phase 和移动后的真实根，见[验证日志](validation/observation-test.log.gz)与[lint 日志](validation/observation-lint.log.gz)。
- M43 完成后的 release 工具构建成功并固定于 target/m44/tools-baseline。尚未执行基线计时，不报告优化收益。

## 2026-10-14：当前版本图基线

- 新增两个小型基准脚本，复用现有图程序及普通 CLI；构建与测量分开，固定 runtime／sysroot 副本、工具摘要和源码版本。测量支持同机交替执行，保存逐进程、逐 collection 原始记录并检查输出、三个时间差及 GC histogram。
- Ruff 0.16.10 格式化／检查通过。正式 release CLI 构建成功，wide／array／chain 各预热一次、正式运行七次，每次五轮主动 full；105 轮有效 full 的实际搬迁数全部为零。当前仍执行 update，其中位数分别为 3.442、6.607、3.289 ms；这只是基线，不是已实现收益。环境、限制和原始数据见 [性能记录](PERFORMANCE.md)。
- 保留基线可执行文件、LLVM/object dump 和 runtime 副本，供零搬迁优化后交替对照。年轻图、选择性搬迁、多线程推进与代码生成快路场景尚未完成，不据此关闭 M44-1 或 M44。

## 2026-10-14：零搬迁跳过 relocation

- 先修订 runtime spec 3.7／3.9。collector 在最终规划／复制或 minor 转 full 完成后检查本轮 moved_objects；零搬迁不扫描用于 relocation 的 roots、remembered 和 retained live set，不累计 update 时间。验证仍扫描必要引用，只在实际有搬迁时要求 relocation 的 scanned 覆盖；其余晋升、统计、release、回收、清卡、TLAB／nursery 重置及恢复保持共用。
- 独立 C 回归覆盖空 full、带死亡对象／ready 与 unready release 的非移动回收、pin 阻止 minor 移动、old→young 循环、清卡、连续有／无搬迁计数，以及全 pin 的 moving stress。分别在普通／metadata 验证 runtime 的 O2 下通过；full 预留回滚和 minor 晋升失败转 full 的既有回归新增 update 未执行断言。
- 首轮发现并修正两处测试假设：并行标记专测原来无条件要求 update 时间增加，现按本轮搬迁数检查；新增测试的模拟栈边界在 O2 下可能落在发布区间内，改用已有 OS stack-high 查询。原始失败保留在[首轮日志](validation/no-move-runtime-initial.log.gz)，未修改 production 栈边界检查。
- 格式化／workspace clippy 后，30 项 runtime GC 测试通过，覆盖移动、精确根、并行 worker／启动失败回退、nursery、region、release、Context、pin 和 NativeSafe 交错，见[完成日志](validation/no-move-runtime.log.gz)。随后新增全 pin stress 断言的独立两配置复验也通过，见[日志](validation/no-move-stress.log.gz)。
- 正式 CLI 的 7 项 fixture、15 个 debug／release／minor 变体、89 个步骤通过，包含新增 m44-no-move（真实机器根及含引用 aggregate 数组环）和 m31 nursery／Context、m34 parallel／regions／reclamation 的组合，见[报告](validation/no-move-fixtures-darwin.json.gz)及[日志](validation/no-move-fixtures-darwin.log.gz)。配套工具固定，未在测试中重建。
- 同机交替对照各 105 轮零搬迁 full，candidate update 每次为零；请求到 RUNNING 中位从 wide 5.605→1.878 ms、array 9.431→2.504 ms、chain 7.547→3.903 ms。详见[性能记录](PERFORMANCE.md)及其适用限制。Linux、移动路径优化与模块隔离仍待后续批次。

## 2026-10-14：普通 full 搬迁基线

- 复用现有图基准增加 compact 场景：180 万对 node／GC-free leaf，保留 1/16 并连接成环，五次独立增长／收缩。现有 region-churn 每轮只保留 64 个对象，不足以反映引用更新工作量；新增场景保持普通选择性搬迁策略，不使用 stress 计时替代实际策略。
- 先完成 Ruff 0.16.10、C 格式化及 workspace clippy，再通过正式 release CLI 构建。初次使用不存在的 Int.toLong 导致基准源码诊断，修正为已有 toInt64 后构建成功；原始失败保留在[构建记录](validation/compact-build-initial.json.gz)。没有修改数值转换语义。
- 当前优化前版本各预热一次、正式七次进程，每次五轮；全部 35 个有效阶段均检查到普通 full 的实际集中搬迁并通过引用环／值校验。update 中位 14.122 ms，请求到 RUNNING 中位 55.683 ms，见[原始基线](validation/compact-baseline-darwin.json.gz)。candidate 尚待验证和同机交替对照，不能据此推定收益。
- 前批已核对归档并清理 target/m44/no-move-fixtures/cases，释放约 114 MB，保留工具、cache、sysroot 和基线 binary。

## 2026-10-14：引用位置复用与出站扫描精简

- 先修订 runtime spec 3.7／3.9。slot visitor 一次解析 block／word，再直接读代际并把已解析位置传给 forwarding；仅新旧地址不同时写回。retained object 未移动时复用该位置，移动时才解析目标副本，保留当前副本检查。删除无其他消费者的重复查询入口，私有 forwarding／scan 操作收回 heap_internal。
- object_scan 为 null 的对象在 retained-set update 中直接返回，不再定位、claim 或解释空扫描；mark 存活清单、搬迁规划、复制、统计及回收保持原路径。验证只要求含引用对象的扫描覆盖。collector／evacuation 主体分别为 216／135 行，没有增加持久位置缓存或第二套数据结构。
- 复核发现 forwarding 指回 source 自身时，旧实现由后续 scan claim 拒绝；GC-free 对象省去该步骤后，在 forwarding 处显式保留此失败出口。新专测在子进程中对真实分配对象注入四种损坏 metadata，检查非 collection、未标记、缺失 forwarding 和 self-forwarding 的实际失败信息，不增加 production 测试接口。
- 格式化与 workspace clippy 通过。原有 30 项 runtime GC 回归在最终 runtime 通过；新增专测最初因引用了未使用的 fixture helper 未通过 C 的 Werror，补充真实正常收集后再注入损坏，单独复验通过四种失败出口。见[原有组及初次编译诊断](validation/relocation-validation-initial.log.gz)、[专项完成日志](validation/relocation-validation-fixed.log.gz)与[lint](validation/relocation-lint.log.gz)。
- 正式 CLI 初组 10 项 fixture、21 个变体、121 个步骤通过，涵盖零／有搬迁、固定的含引用 aggregate 数组、GC-free leaf／大数组、MaybeUninit、原子引用、Context、region、并行标记及 release，见[初组报告](validation/relocation-fixtures-initial-darwin.json.gz)。补上 self-forwarding 检查后，对最终 runtime 再执行两个 M44 专项，4 个 debug／release 变体、12 个步骤通过，见[最终专项报告](validation/relocation-final-fixtures-darwin.json.gz)。
- [最终 AArch64 反汇编](validation/relocation-machine-code.txt.gz)确认 slot visitor 只有一次 object_meta，forward helper 不重新定位；地址比较后的相等分支绕过 str，GC-free retained object 在 claim 前返回；目标副本定位只位于地址改变的分支。未增加逐引用生产计数器。
- 最终同机交替普通搬迁对照保持每对进程的 moved／source／target／live／traced 数一致，update 中位 14.082→11.388 ms，请求到 RUNNING 56.155→53.894 ms；七次进程耗时范围也分离。三类零搬迁图各 35 轮仍每次 update 为零，未观察到整体回退。完整数据与限制见[性能记录](PERFORMANCE.md)。本批不替代 Linux、minor 性能或后续边界整理验收。

## 2026-10-14：编译器 GC 代码生成边界

- 先补充实现规范 2.4／2.5 与运行时规范 3.1：通用 LIR 保存完整语义，Immix 分配策略和完整托管写操作归 codegen GC 模块，保持共享 ABI 与快路径成本。普通对象上限从 LIR runtime ABI 文件移至 GC allocation；实际 ABI 符号、布局与摘要目录不变。
- 新增 `function/gc/`，收拢 TLAB 分配、card/page-map、poll、release-ready，以及数组分配／构造／拼接／clone。字段与数组值写入调用完整 heap_store，原子 store／exchange／CAS 调用完整操作；屏障 helper 仅在 GC 模块内部可见。保留批量构造的单次范围屏障与失败 CAS 的无写入分支，没有增加每元素屏障、外部热路调用或第二套接口目录。通用地址／数组布局辅助、根与 statepoint、调用 ABI 继续复用原模块。
- Rust 格式化与 [workspace clippy](validation/codegen-clippy.log.gz)通过，[372 项 codegen／相关 runtime 测试](validation/codegen-rust.log.gz)通过；包括三个 target 的原子内存序、条件 poll 的慢边根物化与 TLS 查询次数、单槽 card、GC-free 无屏障、分配、数组与原有 collector／握手回归。固定 [release 配套工具](validation/codegen-tools.log.gz)后再运行正式 CLI。
- 新增 [m44-gc-writes](../../tests/fixtures/m44-gc-writes/)：旧对象字段、大数组中的含引用 struct／interface、AtomicRef store／exchange／成功与失败 CAS、实际 minor、构造中 GC、数组 literal／vararg 拼接／clone、boxing 与 moving 组合。首轮使用了不合法的数组 literal 展开，保留 [首轮报告](validation/codegen-fixtures-initial-darwin.json.gz)及 [日志](validation/codegen-fixtures-initial-darwin.log.gz)；改为普通 vararg 展开后，debug／release 各自的普通、moving stress、minor stress 全部通过，见 [复验报告](validation/codegen-writes-darwin.json.gz)与 [日志](validation/codegen-writes-darwin.log.gz)。没有改变语言规则迁就测试。
- 首轮另外五项已有 fixture 均通过：M31 nursery/native/Context/copy、M33 atomics GC、M34 region values、M44 no-move 和 moving-references。与新增用例的成功复验合计六项、13 个变体、57 个进程步骤；首轮失败步骤不计入成功集合，不把定向组称为全量验收。
- 在相同当前 runtime 下用整理前／后的固定编译器分别编译 allocation、arrays、old-graph、primitive。已核实每个根实际触发编译且两侧编译缓存键不同；LIR、根产物、完整 executable 与全部根 Cone 函数的机器指令均相同。原始反汇编和 section 大小见 [结构记录](validation/codegen-machine-darwin.json.gz)。完成构建／测试后各预热一次，再交替七次，checksum 与 GC 统计通过；见 [性能记录](PERFORMANCE.md)和 [原始对照](validation/codegen-comparison-darwin.json.gz)。本批不宣称性能提升。
- 新增基准构建、交替测量与机器码核对脚本均通过固定 Ruff 格式化／lint，并以本批真实构建和测量验证。代码按完整操作划分，最长迁移文件为既有 346 行数组拼接；新原子操作模块 109 行，没有额外 GC crate 或策略插件。
- 已核对压缩归档再清理本批 fixture cases 约 99 MB，保留工具、缓存、基线与对照 binary；上一批 final 证据归档后清理的失败／过期构建及 cases 约 206 MB 也已完成。runtime 适配／mutator 边界、线程实际恢复与其余性能场景、Linux 及最终全量验收继续推进，不宣称 M44 完成。

## 2026-10-14：Scoop 根与扫描适配

- 先在运行时规范 3.3 明确根表示与扫描解释归 Scoop 适配层，复用冻结目录并访问真实槽位。新增 `gc_binding/` 承接 image/process 根、native root frames、线程根链、LLVM stackmap／精确栈根、扫描程序和 managed/native 入口适配；GC 私有头不再导出 stackmap 或 image registry。
- `gc_binding.h` 保留实际使用的访问器与锁合同，扫描解释器支持原有整对象及 card 半开范围。marker 从内联对象／数组视图取得 scan、元素起点、数量与步长，不再读取 scan 编码；保留原有数组分段、任务批次与首次标记检查。pin frame 由适配层枚举真实槽位，GC 的显式 pin 计数和解除临时固定规则保持。
- 普通对象是否已发布的完整分类操作移回 GC 对象实现；锁顺序仍为 heap → roots。构建源目录、平台测试的显式源清单及现有文档链接同步迁移，runtime ABI、metadata 编码和生成代码布局不变。
- 首轮 [372 项测试](validation/binding-scans-rust-initial.log.gz)中 341 项通过，失败均来自拆除间接 include 后的声明或测试链接源缺失；[头文件修正记录](validation/binding-scans-rust-headers.log.gz)保留后续失败与精确栈根成功结果。为初始化／普通 runtime 显式包含实际使用的 image 声明，为测试包含 binding 声明和入口源后，[31 项 collector 定向复验](validation/binding-scans-rust-final.log.gz)全部通过。按名称核对初轮要求集合，372 项均取得通过证据，见 [覆盖索引](validation/binding-scans-rust-coverage.json.gz)，不把首轮失败报告当作整组通过。
- C 格式化、Rust 格式化与 [workspace clippy](validation/binding-scans-clippy.log.gz)通过，固定 [release 工具](validation/binding-scans-tools.log.gz)后执行真实 runtime 构建与 CLI。八项 fixture、17 个变体、93 个步骤全部通过，见 [报告](validation/binding-scans-darwin.json.gz)与 [日志](validation/binding-scans-darwin.log.gz)：M34 parallel threads／values、MaybeUninit GC、M31 nursery/Context/copy、native Scoop ABI moving roots，以及三个 M44 独立回归。
- 压缩证据已逐字节核对，清理本批 cases 输出并保留当前工具／缓存。此批完成根与扫描适配；allocation／collector 的 mutator 字段访问、统计与普通 runtime 的私有堆查询组合尚待迁移。完整边界的性能与 Linux／全量组合在后续完成，不以目录迁移单独宣称 M44-5 或 M44 完成。

## 2026-10-14：GC mutator 状态与线程协调

- 先修订运行时规范 3.1。TLAB cursor／limit、block 所有权和独立对齐的分配计数迁入 GC 私有 mutator；ThreadState 只持有 opaque 引用。沿用现有 allocation-context TLS，以首字段取得同一状态，没有增加 TLS、线程目录或逐分配全局 RMW。线程 attach／managed exit／callback exit／detach 通过具体生命周期操作接入。
- `gc_binding/threads.c` 适配原 world／registry 握手、停稳后的分配区失效、统计汇总和 resident-memory 查询。GC 核心不再包含线程私有头或访问 ThreadState。detach 仍在 world 锁内归并计数后移除目录；collection 沿用 world → heap → roots 锁序，直接遍历冻结目录，不复制快照。
- C／Rust 格式化与 [workspace clippy](validation/mutator-clippy.log.gz)通过，[31 项 collector 回归](validation/mutator-runtime.log.gz)全部通过，包含并发统计与反复 attach/detach、nursery refill 竞争、初始化协调、NativeSafe 返回、精确根、pin／release 和移动。测试的手写 TLAB 分配使用与 generated code 相同的 TLS 数据合同；独立线程测试源清单也加入实际 mutator 实现。
- 固定 [release 工具](validation/mutator-tools.log.gz)后，七项正式 CLI fixture、15 个变体、79 个进程步骤全部通过，见 [报告](validation/mutator-darwin.json.gz)和 [日志](validation/mutator-darwin.log.gz)。覆盖三个 M44 专项、M34 parallel threads／values、M31 nursery/native/Context/copy 和 native Scoop ABI moving roots。
- 证据压缩归档后逐字节核对；本批没有改变公开／generated-code ABI 或正常分配策略。对象信息适配、普通 runtime 完整堆操作及完整边界的性能／Linux 验收继续推进，不以本批定向组代替最终全量验收。

## 2026-10-14：对象信息与普通 runtime 堆操作

- 先修订运行时规范 3.3／3.6。分配归一化、类型对齐与初始化、release hook 读取由 `gc_binding/object.h` 的内联访问器适配；GC 保留 shared header 的私有状态位、分配登记、代际与回收策略。GC 核心不再读取 TypeDescriptor 字段或直接调用 shape 操作，也没有另一份类型数据。
- 普通 boxing／数组克隆的 heap lock、对象起点、exact type 和精确大小检查收进 `gc_binding/values.c` 的两个实际操作。新 `gc.h` 只向普通 runtime 暴露其使用的分配、检查、已发布对象分类、shutdown／统计，以及包含 memcpy 和既有单次范围屏障的内联复制操作；异常、Context、初始化、字符串和启动不再包含 GC 私有堆接口。没有放宽错误条件、去掉拆箱锁或改变 ready／release 语义。
- C／Rust 格式化与 [workspace clippy](validation/operations-clippy.log.gz)通过。[首轮 372 项 codegen/runtime 测试](validation/operations-codegen-initial.log.gz)中 371 项通过；初始化专测依赖被移除的间接私有声明，改用完整 `scoop_rt_gc_init` 后[专项复验](validation/operations-initialization.log.gz)通过。按名称核对的[覆盖索引](validation/operations-coverage.json.gz)包含全部 372 项成功证据。既有 representation 专测覆盖错误 exact type、interior pointer、非法 count／side size、ZST、boxed subtype 和移动后的读取。
- [本机 O2／无 LTO 反汇编对照](validation/operations-machine.json.gz)以本批之前的 `6bd5d6ec` 为基线，allocation、evacuation_plan 与 block_sweep 的指令及 relocation 完全相同，确认对象适配没有新增逐分配／逐对象调用。boxing 与 array clone 的实际 memcpy／范围屏障仍在调用方内联展开；clone 源检查新增一次边界调用，不对数组元素逐个调用。该结果只证明这些路径的结构，不代替完整 runtime 边界计时。
- 固定 [release 工具](validation/operations-tools.log.gz)后，七项正式 CLI fixture、15 个变体、79 个步骤全部通过，见[报告](validation/operations-darwin.json.gz)与[日志](validation/operations-darwin.log.gz)。范围与 mutator 批次相同，验证最终组合的真实 runtime 构建、生成代码、native roots、并行线程、对象复制和 moving／minor stress。
- 本批和上一批各约 114 MB 的 fixture cases 在证据归档核对后清理；保留固定工具、缓存、基线及机器码证据。M44-5 的实现边界已接通，完整边界的分配／写入／收集性能、实际 mutator 恢复观测和 Linux／最终全量验收仍未完成。

## 2026-10-14：完整 runtime 边界对照与数组 mark 回退修正

- 对完整 runtime 边界重建四个既有程序及带边界观测的图程序。观测版 runtime 首次构建暴露 binding 的测试宏在定义函数前尚未启用，见[原始诊断](validation/boundary-build-initial.json.gz)；调整 threads.c 的 include 顺序，使适配层先读取既有测试配置。生产快路径不受影响，带全局测试宏的 nursery retry [专项复验](validation/boundary-retry.log.gz)也通过。
- 首轮同机交替对照发现引用数组 mark 的稳定回退，不能用其他图正常来忽略：mark 中位 1.422→1.646 ms，请求到 RUNNING 2.341→2.566 ms，见[首轮零搬迁记录](validation/boundary-zero-initial-darwin.json.gz)；[普通搬迁记录](validation/boundary-compact-initial-darwin.json.gz)没有同样回退。检查实际 runtime cache 中的 object，确认视图封装改变 marker 的内联和指令布局，见[机器码证据](validation/boundary-marker-machine.json.gz)。
- 最终把 null scan 分支保留在 GC 调用方；适配层只解释非空描述，数组视图保存 count／elements，stride 在原有分段任务发布处读取。保持扫描语义、分段大小和任务数，不用强制 noinline／always_inline 或新的队列策略调参。仅恢复 null 分支的[中间复验](validation/boundary-guard-zero-darwin.json.gz)仍有回退，随后恢复分段读取位置的[数组专项](validation/boundary-array-check-darwin.json.gz)才恢复到基线范围。
- 最终 C／Rust 格式化与 [workspace clippy](validation/boundary-clippy.log.gz)通过，[31 项 collector 回归](validation/boundary-runtime.log.gz)全部通过；三个 M44 正式 CLI fixture、六个变体、24 个步骤全部通过，见[报告](validation/boundary-darwin.json.gz)和[日志](validation/boundary-darwin.log.gz)。
- 最终四程序与三类零搬迁／普通搬迁图各执行预热及七次交替进程；同一输入的 moved／live／traced／source／target 计数逐轮相同。数组请求到 RUNNING 中位 2.321→2.332 ms，普通搬迁 update 11.457→11.381 ms；其余结果与适用限制见[性能记录](PERFORMANCE.md)。本批不宣称边界拆分带来加速，也不替代 minor、实际 mutator 恢复及平台验收。

## 2026-10-14：实际 mutator 进度与多线程分配

- 新增真实 Scoop callback／pthread 基准，覆盖 1／4 个 mutator 的计算与分配。每 1024 次实际迭代发布进度，以 T1 前最后进度与 T2 后更大迭代数对应的时钟记录恢复；明确包含采样批次、解锁／广播和 OS 调度，不将 RUNNING 发布替代为已执行。旧／新 runtime 均启用 nursery retry 观测，生产快路未加入时钟或逐对象统计。
- 六项[记录校验测试](validation/progress-tests.log.gz)通过，拒绝缺失、顺序错误或没有实际迭代增长的恢复记录，允许连续 GC 共享后续进度点。C 格式化、Python Ruff 0.16.10 与 [workspace clippy](validation/progress-clippy.log.gz)通过。构建初次遇到 Unsafe 块返回 Unit、之后遇到 Apple C common symbol；分别修正基准 lambda 的返回表达式与 helper 编译的 `-fno-common`，保留[失败构建记录](validation/progress-build-failures.json.gz)，未改变编译器规则。
- 两版四配置的[功能核对](validation/progress-smoke.json.gz)通过；正式分配量增为每线程两百万次。随后串行交替完成八次预热与 56 次正式进程，checksum、实际分配字节、进度次序、GC histogram 全部核对，见[原始对照](validation/progress-darwin.json.gz)。逐文件[源码核对](validation/progress-source-audit.json.gz)确认基线 `44a3c0bf` 和当前 `f300090b` 的 runtime 副本匹配对应提交。
- 两侧四线程分配字节完全相同，minor／retry 数量和吞吐存在调度波动；实际进度间隔中位 400→397 微秒，T2 到下一进度中位 27→28 微秒。完整中位、p95、最大值和进程范围见[性能记录](PERFORMANCE.md)，不声称稳定吞吐改善，也不把 retry 数量解释成锁等待时间。
- 完成进程和证据归档核对后，清理失败／中间 binary 的构建目录约 28 MB，保留正式两版 binary、工具与缓存。年轻存活图／old→young／pin 的 minor 测量、Linux 与最终全量验收继续推进，本批不宣称 M44 完成。

## 2026-10-14：年轻存活图与实际 minor 搬迁对照

- 扩展已有 graph 基准，旧代数组持有 4096 个年轻节点和同数 GC-free leaf，构成反向引用环；pin 首节点，使其出站引用指向另一个可移动 block 中的尾节点。普通分配压力触发 minor，测试同一轮的旧代槽、年轻根与环更新、固定对象出站引用和原地／移动晋升；无新增生产 API 或 stress。构图提前触发 minor、额外 full 或 fallback 均失败，不混入不同场景的样本。
- C 格式化、Python Ruff 0.16.10 和两版 helper 的 Werror 构建通过。初版测量断言错误地把 pin 当作只阻止单个对象移动；按既有 block source 选择规则修正后，原有 wide／compact 与 minor 功能核对通过。最终反向环在两版、1／4 worker 下均通过，见[功能及初始诊断](validation/minor-functional.json.gz)。没有修改 collector 以满足基准。
- 基线 `44a3c0bf` 与当前 runtime 在两种 worker 配置下各完成预热和七次交替进程，每进程五轮。全部 140 个正式 minor 样本均处理相同的 8192 个年轻对象，实际搬迁 7027 个、晋升 229376 bytes，65 张脏卡与 4096 个旧代引用槽；每对样本的完整工作量计数一致，见[源码／计数核对](validation/minor-audit.json.gz)。
- 1／4 worker 的 update 中位分别为 343→172、334→178 微秒；同组更新时间范围分离，实际移动与引用工作量保持。整体暂停、进程波动、内存与限制见[性能记录](PERFORMANCE.md)，原始数据为 [1 worker](validation/minor-1-darwin.json.gz)和 [4 worker](validation/minor-4-darwin.json.gz)。
- 证据核对后清理约 28 MB 初版构建，保留正式 binary、固定工具和缓存。本机共享路径工作区测试、fixture runner 公共规则与正式 CLI 全量验收随后执行，再推进 Linux GNU／musl；M44 仍未完成。

## 2026-10-14：跨 runtime 版本的基准构建来源

- 为 Linux 的旧／新版本整体对照，四程序构建脚本增加可选 runtime／sysroot 来源，保存实际路径与 runtime 逐文件摘要。每次构建使用与源码目录匹配的固定工具，避免旧工具的源目录清单与已拆分的新 runtime 混用；原有同 runtime 的双工具对照保持。
- Python Ruff 0.16.10 [格式与检查](validation/codegen-source-selection-lint.log.gz)通过。以 `44a3c0bf` 的 220 个 runtime 文件和当前的 231 个文件分别构建、运行 allocation／arrays／old-graph／primitive，八个进程的预期输出与 GC 统计均核对，源码摘要与各自输入逐项一致，见[构建及功能记录](validation/codegen-source-selection-darwin.json.gz)。准备命令首轮误向 Darwin 传入仅供 Linux 的 unwind prefix，改用 SDK 默认来源后通过；原始拒绝诊断保留。
- 本批只验证构建来源与程序功能，不在其他本机测试运行期间计量性能。证据归档核对后清理本批构建副本约 74 MB，保留共享缓存。工作区测试和正式 CLI 全量仍在运行；49 项 fixture runner 公共规则已通过，最终覆盖与 Linux 结果仍待闭合。

## 2026-10-14：工作区验收与既有测试期望修正

- 首轮 `cargo test --workspace` 执行 25 个目标，5619 项通过、2 项失败。最长的 core 产物测试包含多组实际发布与损坏产物拒绝检查，最终通过；未因观察等待到期而重启或跳过。原始失败、定向诊断与修复日志均保留在[完整记录](validation/acceptance-rust-darwin.json.gz)。
- 两个失败均为旧测试期望：诊断路径仍写 cone-production/11，更新为 M43 既定的 12；bootstrap 类型集合漏掉 M42 的五个 Collection 实例，以及 Float／Double 位转换经 Ptr 引入的四个 Equals 实例。逐项对照现有 lists.scoop、floating.scoop 和 ffi.scoop 后补齐，未改变编译器行为或放宽产物检查。
- 格式化与 workspace clippy 通过，两个失败项用独立 release 测试程序复验通过。根据 Cargo 编译目标清单，补跑首轮因失败未执行的 driver CLI 7 项和 machine-capability 1 项；另执行完整 workspace doctest，21 个目标中 2 项测试通过。按目标及测试名称核对，27 个测试目标共 5629 项均有成功证据，另有 2 项 doctest，见[覆盖索引](validation/acceptance-rust-coverage-darwin.json.gz)；没有再次执行已通过的完整工作区套件。
- [49 项 fixture runner 公共规则](validation/acceptance-fixture-rules-darwin.log.gz)通过。正式 CLI `--all` 仍在运行，Linux GNU／musl 的适用功能组合与同机性能尚待验证；本批不宣称 M44 完成。

## 2026-10-14：Linux 固定工具与对照程序准备

- nuc12 同步至 `af49910f`，格式检查、workspace clippy 与 release 配套工具构建通过；复制到独立 tools-final 后固定摘要。GNU 的旧／新版 graphs、progress 和四程序构建共六组均成功，尚未进行性能计时或平台功能验收，见[准备记录](validation/linux-preparation.json.gz)。
- 对六份 runtime 副本逐文件核对 Git 来源：基线 `44a3c0bf` 为 220 个文件，当前为 231 个文件；仅允许对应观测头增加既定测试宏。四个微基准均由两套工具分别实际编译，根 `.slib` 逐字节相同，LIR、函数集合、最终函数指令数与长度也相同。最终 binary 和已链接指令字节不同，保留完整反汇编及产物摘要；本期 runtime 已变化，因此不以最终 binary 相同作为整体对照条件，也不据构建结果宣称 runtime 性能保持。

## 2026-10-14：Linux 整体性能与边界回退定位

- nuc12 完成整体优化和独立 runtime 边界的同机交替对照，每场景各七次正式进程，图每进程五轮。零搬迁 update 为 0；整体 compact update 16.596→12.021 ms，minor 约 550→274 µs。实际 mutator 进度、分配／重试、CPU／RSS 与四程序结果均保留，见 [Linux 性能记录](PERFORMANCE-LINUX.md)及其原始归档。
- 独立边界对照暴露 minor update 约 252→273 µs 的回退，重复及固定 P-core 后仍存在。八个更新热函数的指令字节相同，图与 native helper 也相同；代码布局改变，但没有把具体 CPU 原因当作已证实结论。
- 隔离副本的 64-byte callback 对齐恢复 minor，却令 full update 11.672→12.858 ms；遍历前选 full/minor callback 的另一实验也未改善两类场景。两者均不合入生产，不利样本和机器码完整归档。三份完整矩阵共 18 组图对照的 15 个工作量字段逐对相同，归档后逐字节核对。
- 当前生产 runtime 保持 `f300090b`。本机正式 CLI 全量仍在运行；Linux 性能修复和 GNU／musl 最终功能验收仍需完成，不以整体收益掩盖模块拆分的回退。

## 2026-10-14：内联 forwarding 的隔离验证

- 新候选仅将既有 forwarding 函数体移到 GC 私有头并使用普通 static inline，保留全部检查、原签名与唯一实现，不改变 metadata、ABI 或策略。GNU 实际机器码消除了槽访问／claim 到该 helper 的外部调用；minor update 恢复到约 251 µs，full 为 11.672→10.904 ms。零搬迁、4-marker minor、实际 mutator 进度与四程序对照均已完成，工作量逐轮相同；完整数据和尾部波动见 [Linux 性能记录](PERFORMANCE-LINUX.md)。
- [Darwin](validation/forward-validation-darwin.json.gz) 31 项 collector 测试通过，[Linux](validation/forward-codegen-gnu.json.gz) 387 项 codegen/runtime 测试通过。现有测试驱动及断言不变，临时 cc wrapper 只重定向 runtime 源/include 路径；完整编译命令和源码摘要已核对归档。三个正式 M44 fixture 在 Darwin、GNU、musl 各通过六个变体、24 个执行步骤；后两者见[隔离 fixture 记录](validation/isolated-fixtures-linux.json.gz)。
- Darwin 候选图／进度／四程序已构建，尚未计时；本机完整 CLI 仍使用未修改的生产 runtime。本批不合入候选、不把隔离验证当作最终平台覆盖，待全量进程结束后进行本机同条件对照。
- 归档未采用的对齐实验在本机的[构建及三项 fixture 记录](validation/alignment-preparation-darwin.json.gz)，其 GNU／musl 结果也保留于隔离 fixture 记录。确认相关进程结束并核对归档后，清理本机约 104 MB、Linux 约 134 MB 的对齐／分流 binary 与已结束 cases，保留基线、当前候选及可复用缓存。

## 2026-10-14：产物损坏用例的固定偏移修复

- Darwin 全量 CLI 中，program-link-artifact-corruption 首轮预期 member 摘要不匹配，实际先报 manifest 编码损坏。核对原始产物后确认：manifest 已增长至 53982 bytes，旧的绝对偏移 20734 落在 manifest 内；HIR payload 的 magic 位于 78833。reader 正确拒绝了实际损坏的输入。
- 改用已有文件 replace 操作，唯一匹配 HIR payload 的 `SCOOPHIR` magic 并等长替换为 `BROKENIR`，消除对 manifest 长度的依赖；不增加用例脚本、runner 框架或生产实现改动。摘要错误的完整 JSON／human 期望、成员身份、截断及其他错误断言保持。
- 文档／TOML 格式与配置检查通过，复用六份已发布缓存，仅复验该项：1 个 fixture、1 个变体、13 个进程步骤通过，包含失败后旧程序的普通与 moving GC 执行。原始失败、字节位置分析、缓存来源及复验报告保留于[记录](validation/artifact-corruption-repair-darwin.json.gz)。完整 CLI 仍在运行，最终应合并该项成功复验，不能将原始失败报告改写为成功。

## 2026-10-14：本机全量闭合与候选性能复核

- 正式 CLI `--all` 完成，首轮 4036 项通过、16 项不适用、1 项失败。以已完成的产物损坏复验替换该项后，4037 项适用用例全部通过，共 4561 个变体、17020 个进程步骤；按当前发现的全部 4053 个名称和适用性逐项核对，无缺项或额外替代。保留[首轮完整记录](validation/acceptance-cli-darwin.json.gz)和[覆盖索引](validation/acceptance-cli-coverage-darwin.json.gz)，未重跑整套。
- 全量进程结束后，Darwin forwarding 候选完成六组交替性能对照，六个图组的 15 项工作量计数逐对一致。full／minor update 改善，实际进度与四程序没有稳定退化，但数组 mark 出现持续回退；加入当前生产版的专项复测确认该差异，见[性能记录](PERFORMANCE.md#内联-forwarding-的-darwin-对照与数组回退)。
- 准备采用时的 C 格式检查与 workspace clippy 通过，但性能复核后恢复生产 runtime，候选未提交。继续在隔离副本定位数组 mark 回退；GNU／musl 最终组合验收等待修复确定，M44 尚未完成。

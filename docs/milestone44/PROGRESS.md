# M44 实施记录

依据：[设计](DESIGN.md)。实施起点为 M43 完成提交 `4f3ec3be`，不把旧 M34 性能数据当作当前基线。

## 当前状态

| 批次 | 状态 |
| --- | --- |
| M44-1 基线、计时观测与 spec | 发布边界与三类图的当前基线完成；其余场景继续随批次补齐同条件对照 |
| M44-2 零搬迁更新分支 | 实现与本机定向验收、图对照完成；Linux 组合待后续统一验证 |
| M44-3 引用定位、写回和无引用对象 | 实现、本机定向验收及普通搬迁／零搬迁对照完成；minor 性能与 Linux 组合继续补齐 |
| M44-4 编译器 GC 代码生成边界 | 待实施 |
| M44-5 runtime GC 操作与 Scoop 适配 | 待实施 |
| M44-6 组合、平台与性能验收 | 待实施 |

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

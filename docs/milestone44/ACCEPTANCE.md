# M44 验收核对

状态：验收进行中，M44 尚未完成。日期：2026-10-14。

范围依据 [设计](DESIGN.md)第 8～9 章。实施及失败修复历史见 [PROGRESS](PROGRESS.md)，
性能原始记录与测量限制见 [PERFORMANCE](PERFORMANCE.md)。本文件区分已有的本机定向
证据和仍缺少的最终集成／平台证据，不把开发期定向通过当作三目标验收完成。

## 交付项

| 项目 | 当前实现与证据 | 剩余验收 |
| --- | --- | --- |
| M44-1 基线与时间口径 | 运行时规范 3.5／3.9 保留既有 pause／stop_wait 含义，增加 STOPPING／COLLECTING／RUNNING 观测；[握手测试](validation/observation-test.log.gz)覆盖发布顺序，本机各场景已有同条件基线与对照 | Linux 同机基线与对照 |
| M44-2 无搬迁跳过更新 | [collector](../../runtime/src/gc/collector.c)在规划／复制及 fallback 完成后检查本轮 moved_objects；只跳过 relocation，回收／晋升／release／分配状态失效／线程恢复共用后续路径；[独立专测](../../runtime/tests/no_move_update_test.c)覆盖零搬迁及移动后下一轮，见[最终 collector 组](validation/boundary-runtime.log.gz) | 最终集成与 Linux 组合 |
| M44-3 减少实际更新工作 | [slot visitor](../../runtime/src/gc/collector.c)复用本次 object_meta 结果并条件写回；[forwarding／claim](../../runtime/src/gc/evacuation.c)只为新地址重新定位，GC-free 对象跳过出站扫描；[损坏 forwarding 专测](../../runtime/tests/forwarding_validation_test.c)与[反汇编](validation/relocation-machine-code.txt.gz)保留必要失败出口和一次定位证据；full／minor 实际搬迁对照已完成 | 最终集成与 Linux 组合 |
| M44-4 完整 GC 代码生成 | [function/gc](../../compiler/codegen/src/function/gc/)持有分配、字段／数组／aggregate 写入、原子操作、发布和 poll；失败 CAS 只在成功分支执行屏障；[372 项测试](validation/codegen-rust.log.gz)、[真实写入 fixture](validation/codegen-writes-darwin.json.gz)与[机器码对照](validation/codegen-machine-darwin.json.gz)通过 | Linux 生成代码与运行组合 |
| M44-5 runtime 边界 | [gc_binding](../../runtime/src/gc_binding/)解释根、stackmap、对象 scan 与线程字段；[mutator](../../runtime/src/gc/mutator_internal.h)拥有分配状态并保留原 TLAB 前缀；[普通 runtime 操作](../../runtime/src/gc.h)收拢实际分配／复制／对象检查，[操作回归](validation/operations-darwin.json.gz)通过；完整边界的本机性能与机器码已核对 | 最终集成与 Linux 生命周期组合 |
| M44-6 组合与平台 | 本机定向正式 CLI、collector 组、各性能场景已有记录；[工作区覆盖](validation/acceptance-rust-coverage-darwin.json.gz)包含 27 个目标的 5629 项成功测试及 2 项 doctest，[49 项公共规则](validation/acceptance-fixture-rules-darwin.log.gz)通过 | 正式 CLI 全量、Linux GNU／musl 适用组合、最终覆盖核对 |

## 正确性矩阵

以下为设计第 9 章各范围的具体落点；列出的现有证据证明本机开发阶段覆盖，不免除最终集成。

| 范围 | 专测／组合与已归档证据 |
| --- | --- |
| 零搬迁、回收与历史计数 | no_move_update_test：无 source 的 full、死亡对象、ready／unready release、pin 的 minor、nursery／TLAB 失效、移动后连续零搬迁；[no-move CLI 组](validation/no-move-fixtures-darwin.json.gz) |
| 晋升回退、预留回滚与空洞复用 | evacuation_fallback_test、nursery_fallback_test 与现有 nursery／region 组；最终 [31 项 collector 测试](validation/boundary-runtime.log.gz)包含对应 Rust 驱动 |
| 完整引用更新与精确表示 | [moving-references](../../tests/fixtures/m44-moving-references/)和 [gc-writes](../../tests/fixtures/m44-gc-writes/)覆盖 roots、环、旧代字段／数组、未移动含引用对象、GC-free leaf／数组；[引用组合组](validation/relocation-fixtures-initial-darwin.json.gz)另含 MaybeUninit、region、Context 与并行值 |
| 根、FFI、线程与生命周期 | collector 根与握手专测；[mutator 组](validation/mutator-darwin.json.gz)及[普通 runtime 操作组](validation/operations-darwin.json.gz)各 7 项、15 变体、79 步，含 native-scoop-abi-moving-roots、nursery/native/Context/copy、parallel-threads／values |
| pin／handle／release | collector 的 pins、moving、reclamation、no-move、fallback 组；[回收与 pin 的 CLI 组](validation/no-move-fixtures-darwin.json.gz)；实际 minor 基准核对 pinned 节点到移动节点的引用 |
| 托管写入与发布 | gc-writes 在 debug／release 下运行普通、moving stress、minor stress；字段、数组、含引用值复制、构造、clone、AtomicRef store／exchange／成功与失败 CAS；codegen 专测检查内存序、屏障与 poll |
| 显式验证及错误出口 | collector 驱动的 verify_metadata 配置、moving stress，损坏 forwarding 的真实对象注入／失败消息，初始化与对象表示专测；[操作测试覆盖索引](validation/operations-coverage.json.gz)保留首轮失败与成功复验对应关系 |
| 产物消费与平台 | 最终工作区测试已完成并逐项核对，见[覆盖索引](validation/acceptance-rust-coverage-darwin.json.gz)；正式 CLI 的无源码消费和 artifact-only 链接仍需合并最终结果。GNU／musl 尚未执行本期验收，不能使用 M43 的平台结果代替 |

## 性能与边界核对

- 无搬迁 wide／array／chain：[最终边界对照](validation/boundary-zero-darwin.json.gz)每图每侧 35 轮实际 moved_objects=0、update_ns=0；collector 后续必要工作仍执行，独立正确性测试不以计时为分支依据。
- 普通 full 选择性搬迁：[引用优化对照](validation/relocation-compact-darwin.json.gz)与[最终边界对照](validation/boundary-compact-darwin.json.gz)保持逐对图和工作量计数。引用数组 mark 的初轮回退、机器码定位与修复后的完整对照均保留，未删除不利样本。
- minor／old→young／pin：[1 worker](validation/minor-1-darwin.json.gz)、[4 worker](validation/minor-4-darwin.json.gz)各每侧 35 轮，实际移动、晋升和 remembered 工作量逐对相同，见[核对](validation/minor-audit.json.gz)。
- 多线程分配与实际进度：[56 个正式进程](validation/progress-darwin.json.gz)覆盖计算／分配及 1／4 mutator。记录 T2 后实际增长的迭代数，明确 1024 次采样粒度、调度影响与重试计数的限制，不把 RUNNING 发布当作已经恢复。
- 编译器／runtime 边界：[生成代码](validation/codegen-machine-darwin.json.gz)和[普通 runtime 操作](validation/operations-machine.json.gz)反汇编与四程序对照证明对应已测路径没有额外热路成本；GC 核心不直接解释 thread 私有字段或 scan 编码，普通 runtime 不组合 GC 私有锁／对象查询。

这些证据均为 Darwin／AArch64 同机结果。GNU／musl 的适用功能与 Linux 的同机性能
对照仍缺失；也不能由上述小样本推导所有程序的固定加速比或稳定尾延迟。

## 尚未闭合的完成条件

1. 完成仍在运行的正式 CLI `--all`，按完整发现集合核对通过／不适用／失败，修复失败后保留准确的复验映射。工作区首轮的两项旧期望已修正，定向复验、遗漏目标与 doctest 均通过；[完整记录](validation/acceptance-rust-darwin.json.gz)保留首轮失败。
2. 在 nuc12 运行 Linux GNU／musl 的 GC／ABI／根／写入／生命周期、真实库与产物消费适用组合，并完成同机性能与机器码对照；不跨主机计算加速比。
3. 合并最终平台覆盖、原始失败与修复记录，逐项核对本文件和设计后，同步设计、实施记录及路线图的完成状态。

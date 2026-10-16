# M50 前置调研：编译器自身性能

记录日期：2026-10-17。测量日期：2026-10-16。代码版本：`5845250043b7e0697c1d441bea8ac56097fc379c`。

状态：前置测量已归档，M50 职责清理后的同机复测见第 5 节；普通性能候选仍未作为新增功能实施。范围是编译器及构建编排的耗时与内存，包含编译 runtime 源码的成本，不包含生成物的运行性能。本记录为 [M50 设计](DESIGN.md) 提供依据；普通性能优化先作为候选方向记录，不自动成为 M50 的新增交付要求。

## 1. 测量口径

- 本机 Apple M3 Ultra，32 个逻辑 CPU、512 GiB 内存，macOS 27.0.1，aarch64 Darwin。
- 从上述版本构建 release 版 `scoop`、`scoopc`、`scoop-linker`；`scoopc` 实际链接 LLVM 22.1.8。构建时设置 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`。初次误选 LLVM 23 导致的失败未计入结果。
- 被编译的 Scoop 程序使用 debug profile。使用现有 [M48 测量脚本](../../tests/benchmarks/m48/measure.py)，对 core、std、空程序和 M42 多 Cone 应用各做 3 轮冷／暖配对，共 24 次 build。
- 冷构建每轮使用新的 Scoop cache 和 target 目录，未清空操作系统文件缓存；源码与工具始终存在。下表为普通 build 的墙钟时间，程序输出的功能验证在计时之外。
- RSS 为进程树 RSS 总和的观测峰值，每轮取峰值后再取三轮中位数。采样循环每次执行 `ps` 后等待约 20 ms，可能漏掉短进程，对暖构建的毫秒级数值也有影响。
- 另外用 `/usr/bin/sample` 对一次完整应用构建的 6 个 `scoopc` 子进程逐个采样，间隔 1 ms，进程退出即停止。采样构建耗时 91.708 s，不计入无采样基准。下文百分比均以对应子进程的主线程样本为分母，包含子调用和可能的 I/O 等待；不同层级的百分比不能直接相加或换算成提速承诺。

## 2. 已完成的测量

| 构建 | 冷构建中位数 | 全命中中位数 | 冷构建峰值 RSS 中位数 | 冷／暖编译子进程数 |
| --- | ---: | ---: | ---: | ---: |
| core | 29.278 s | 0.085 s | 2.78 GiB | 1 / 0 |
| std，含 core | 49.193 s | 0.086 s | 2.80 GiB | 2 / 0 |
| 空程序，含 core、runtime 编译和链接 | 37.360 s | 0.094 s | 2.74 GiB | 2 / 0 |
| M42 多 Cone 应用，含依赖、runtime 编译和链接 | 82.782 s | 0.089 s | 2.81 GiB | 6 / 0 |

全部暖构建均未再次启动编译子进程。当前主要开销在需要实际编译的路径。

另在应用与全部依赖已完成构建后，仅向临时副本的应用 `src/main.scoop` 追加一行注释，进行 3 次输入变化构建：**12.214、12.279、12.432 s**，中位数 **12.279 s**。每次只启动 **1 个**编译子进程，依赖没有重编译；峰值 RSS 中位数约 **2.11 GiB**。源码、工具与原产物均保持存在。

## 3. 已确认的热点与候选方向

### 3.1 LLVM 全量声明与大量小对象

**采样依据：** `emit_object_set_v2` 占 core 主线程样本的 **74.3%**、std 的 **38.9%**、应用自身的 **51.6%**。core 中 `declare_function` 占 **12.2%**，元数据对象分区与输出占约 **16.1%**。

**实现依据：** [emit.rs](../../compiler/codegen/src/emission/objects/emit.rs) 为每个 callable 新建 LLVM Context/module 并生成独立对象，但 [emission.rs](../../compiler/codegen/src/emission.rs) 又在每个 module 中声明整个 Cone 的函数、类型描述符、外部 callable 和全局数据。若有 F 个函数，仅全量函数声明准备就形成 F×F 的重复规模；这不是整个编译过程已经测得平方复杂度的结论。[ExpectedSafepoints::for_function](../../compiler/codegen/src/statepoint/manifest.rs) 还会扫描全部 site，并复制完整函数表。

实际 core `.slib` 包含 **9,688 个 ar 成员，其中 9,684 个是 Mach-O 对象**，总大小 **78.35 MiB**；对象载荷约 **17.79 MiB**，其余 4 个成员载荷约 **60.00 MiB**。std 有 **5,154 个对象**，总大小 **58.65 MiB**。数量与大小通过普通 ar 目录和成员魔数读取确定。core 的主线程叶子样本中，`__open` 约占 **7.9%**。

当前 [metadata 分区](../../compiler/codegen/src/emission/metadata_partition/partitions.rs) 已采用二分递归投影；仍需关注最终分区数量、每个分区的 LLVM pass、对象写出、复读和修补，不能沿用“每个对象都克隆整个原始 metadata module”的旧描述。

**候选方向：** 按实际引用缩小每个 module 的声明集合，复用已有索引；评估物理对象分区粒度；尽可能传递已有对象字节，减少小文件落盘与重复解析。对象选择及 ODR/ABI 的实际要求仍需满足。

### 3.2 自产输出的重复复核

**采样依据：** core 的 `LLVMVerifyModule` 占 **15.45%**，自定义 `statepoint::verifier::verify_rewritten` 占 **5.11%**。这些包含在上面的 backend 时间内。整个 artifact assembly 另占约 **11.5%**，其中也包含必要的打包与连接工作，不能全算成可删除开销。

**实现依据：** [prepare.rs](../../compiler/codegen/src/emission/prepare.rs) 在 LLVM 变换前后执行完整 module 验证；当时的 `statepoint::verifier`（M50 已删除） 明确用于防御性检查编译器／工具链输出，当时的 `verifier::policies`（M50 已删除） 又遍历完整函数表。对象生成后，[layout verification](../../compiler/driver/src/object_production/layout/verification.rs) 和 [registration finalize](../../compiler/driver/src/object_production/layout/registrations.rs) 继续处理对象、生产记录和指纹，并多次复制 patch-site／stackmap 数据。

**M50 清理方向：** 按设计删除仅用于证明自身或 C/C++ 工具输出正确的复核、封存与凭证；实际需要的 stackmap、符号、地址和补丁位置提取一次后复用。必要的语言检查及实际 GC/ABI 数据生成继续保留。具体清理范围不由样本占比决定。

### 3.3 HIR Lowerer 状态复制与释放

**采样依据：** std 的 HIR lowering 占主线程样本约 **42.2%**；`Lowerer::clone` 占整个进程的 **22.92%**，`drop_glue<Lowerer>` 占 **14.32%**。调用栈命中 ClassConstructor、导入 callable 选择表、PreparedImportedGeneric、属性源声明及 SourceProvider 等 Vec/Map 的深复制与释放。

**实现依据：** [重载候选探测](../../compiler/hir-lower/src/overload/probe.rs) 在检查参数形状之前就复制 Lowerer；[receiver_has_property](../../compiler/hir-lower/src/expr/members/resolution.rs) 仅判断属性是否存在，也复制整个 Lowerer。[成员探测](../../compiler/hir-lower/src/expr/members/selection.rs) 和 [导入 callable 探测](../../compiler/hir-lower/src/expr/named_calls/imported_dependency/probe.rs) 有相同模式。

当前函数 Arena 已采用 SnapshotArena，部分声明和类型使用 Shared/Rc；剩余成本来自 [Lowerer 字段](../../compiler/hir-lower/src/lib.rs) 中仍被复制的大量普通 Vec、Arena、HashMap 和导入选择状态，不能将其概括为所有字段都在深复制。

**候选方向：** 共享稳定的声明、导入和已解析数据；存在性查询避免复制整个 Lowerer；候选形状先过滤，实际推导只保存相关变化。围绕已命中的字段简化，不另建通用事务框架。

### 3.4 依赖元数据与实体图的重复处理

**采样依据：** M42 consumer 的 `LoadedExplicitDependencyInputs::validate_inner` 占主线程样本 **69.36%**，应用自身为 **19.19%**。consumer 中仅 `register_external_graph_authorities` 就占 **25.53%**。

**实现依据：** [依赖入口](../../compiler/driver/src/request/preflight/dependencies/validate.rs) 对整个闭包进入 [layout closure reader](../../compiler/slib/src/layout_compile_closure/read.rs)，解码多套 HIR/MIR/LIR 与物理产物数据，再逐个依赖构造身份图。[外部实体图合并](../../compiler/identity/src/validation/external.rs) 已用 Arc 共享 key，但每次仍收集、排序、查表和插入全图，合并后的图还经过完成检查。不同 `scoopc` 进程会再次消费同一批 core/std 数据。

[decode_canonical](../../compiler/wire/src/cbor/decode.rs) 在完整解码后还重编码整个记录并比较字节；其 primitive 已有局部格式检查，这里仍额外生成一份完整编码。重复遍历和分配有源码依据，本次未单独量化重编码的独占耗时。

**处理方向：** M50 清除整包重编码比较及来源／资格复核；普通读取优化可按本次实际需要的接口、泛型模板和布局读取数据，在同一闭包内共享身份索引，减少全图展开。不能把所有依赖读取、实体归属或引用处理都归为额外安全校验。

### 3.5 增量复用粒度为整个 Cone

**实测与实现依据：** 上述叶子源码变化仍需 **12.279 s**，且只编译了应用自身。[Cone cache input](../../compiler/scoop/src/snapshot/prepared/model/cache_key.rs) 将当前 Cone 的源输入放入一个编译键，变化后执行当前 Cone 的完整流水线；当前没有复用其中未改动函数／文件的前端和 codegen 结果。

**候选方向：** 在降低已有重复工作后，按实际编辑负载评估 Cone 内的函数／文件复用。这是增量能力的粒度限制，依赖缓存已经正常复用；本次没有验证更细粒度的实现方案或收益。

### 3.6 构建图与 native 编译串行

**实测与实现依据：** [构建调度](../../compiler/scoop/src/schedule.rs) 逐个完成 dependency-first 节点；本次 6 个编译子进程也依次运行。应用中 json 与 std 可同时编译，但其他节点存在真实依赖，不能将全部耗时视为可并行时间。

[runtime C 编译](../../compiler/scoop/src/runtime_build/compile.rs) 按源文件串行执行，每个单元先运行 `-E -dI -v -MD`，读写预处理文件，再调用编译器生成对象。本次 Darwin runtime 生成 **94 个编译单元**的对象，按该循环对应 **188 次预处理／编译命令**，不含工具探测和链接；这个次数由输出数量与实现推得，并非 `ps` 捕获的完整进程计数。

[native objects](../../compiler/toolchain/src/native/objects.rs) 也逐单元编译，以整个配置源集保存记录，观测输入失效后重新准备该组单元。本次未另测大型 C++ 项目或 native 头文件变化，该部分保留为源码层面的扩展性发现。

**候选方向：** 对已就绪的 Cone 和独立 native/runtime 单元使用有界并行；评估编译时产生 depfile 能否满足现有输入观察需要，以减少独立预处理调用；按 native 单元的实际依赖复用。并行度需结合单次编译的数 GiB 内存成本选择。

## 4. 后续顺序与限制

先按 M50 既定边界清理重复输出复核；普通性能优化优先评估 Lowerer 热点复制、LLVM 全量声明与对象粒度，再收窄依赖读取，随后评估并行及更细的增量复用。这是候选工作的相对顺序，尚未形成新的实施计划或性能验收阈值。

本次没有修改编译器、测试或 runtime 实现，没有执行全仓库测试，也没有做专门的堆分配跟踪、Scoop release profile 编译测量或其他目标平台测量。RSS 不足以精确归因到某个字段。M48 历史性能表使用不同机器和代码版本，不据此直接断言性能回退或计算倍数。

原始日志、逐轮 JSON、调用栈、临时输入与分析脚本均保留在仓库外，不作为源码或验收归档提交。本机位置为 `/var/folders/0s/mcfxhz813ps4mky0c1sr7rz00000gn/T/scoop-compiler-perf-20261016-6u41ycwy/`，主要记录是 `baseline-llvm22/report.json`、`incremental-report.json`、`profile-application/profile-report.json` 和 `profile-application/samples-summary.json`。这些临时路径只用于定位本次原始材料，文档中的结论不依赖其长期存在。

## 5. M50 清理后的同机复测

2026-10-17 在同一台 M3 Ultra、LLVM 22.1.8 上复测，工作树版本为 `e84786437`，生产工具包含至 `87beaaeb0` 的实现（后续两项 Rust 测试修正不改变生产工具）。沿用同一脚本、四组源码与 3 轮冷／暖配对，Scoop 为 debug profile，工具为 release profile。原始报告保留在 `~/.cache/scoop-m50/performance-after/report.json`，工作目录通过 `TMPDIR=~/.cache/scoop-m50/tmp` 放在仓库外。

| 构建 | 清理前冷构建 | 清理后冷构建 | 冷构建耗时降低 | 全命中中位数 | 冷构建峰值 RSS 中位数 | 冷／暖编译子进程数 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| core | 29.278 s | 18.842 s | 35.6% | 0.057 s | 2.27 GiB | 1 / 0 |
| std，含 core | 49.193 s | 33.818 s | 31.3% | 0.059 s | 2.36 GiB | 2 / 0 |
| 空程序，含 core、runtime 和链接 | 37.360 s | 25.084 s | 32.9% | 0.063 s | 2.29 GiB | 2 / 0 |
| M42 多 Cone 应用 | 82.782 s | 58.077 s | 29.8% | 0.061 s | 2.34 GiB | 6 / 0 |

24 次 build 全部成功，可执行样例均在计时外通过输出检查。全部暖构建未启动编译子进程，源码与工具始终存在。该结果是整批清理后的整体观测，不能单独归因给某个删除项；本次未重做调用栈采样或叶子源码变化测量，也不扩展成新的性能验收阈值。

该测量早于最终工具链探测输出复核的收尾清理；该批只补验功能，未追加性能测量。

# M56 设计：生成代码、GC 与 runtime 性能

状态：实现与验收完成，2026-10-19。A–D 实现及策略取舍已交付，E 和桌面后续候选完成进入调查；候选不等于已启用功能。原始代码调研基于 `28fab1812dbba96f9299686ec648ba3de2506877`；逐项记录见 [PROGRESS.md](PROGRESS.md)，最终范围、性能、失败闭合与清理见 [ACCEPTANCE.md](ACCEPTANCE.md)。第 2、11 节保留设计调研阶段的实验及清理记录。

现行合同以[语言规范](../specs/SCOOP-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)为准。下文区分现状、拟采用方案和仍需实验的候选；涉及合同的实施必须先按第 8 节同步规范，不能把设计目标当成现有能力。

## 0. 目标与范围

M56 面向以 PC／桌面应用为主的 Scoop 程序，含 60 fps GUI，并兼顾低配置个人电脑。优先降低分配成本和 GC 暂停尾延迟，其次控制实际内存占用，再考虑大批量 GC 吞吐；同时关注编译时间与代码体积。优先处理可以从现有代码定位、可用独立 fixture 验证的成本。桌面场景的时间口径、局部优化顺序与增量 GC 取舍见[短暂停补充设计](DESKTOP-GC.md)。

范围包括 MIR／LIR／LLVM 优化、safepoint、对象与值表示操作、runtime 小操作、分配、写屏障及现有分代 Immix collector。String 表示操作属于本范围；JSON、集合算法、HTTP、TLS、异步 I/O 的库级重构不纳入本期，它们只可作为组合工作负载。M55 泛型约束设计不作为前置条件。

本期主线依次为：复用已有验证结果与消除微小 runtime 操作的边界成本；扩展安全的 release 优化；测量并优化现有分配与搬迁后引用回写。编译侧重复声明构建、跨 Cone 内联及协程对象消除按第 9 节的证据条件进入，不能成为推迟前三项交付的前置框架。

不新增通用资源预算、产物授权或防伪机制，不把优化事实变成凭证／收据，也不新建平行验证器。必要的类型、边界、ABI、精确根和并发发布检查继续保留。性能计数只帮助选择实现，不决定程序是否合法。

## 1. 证据与优先级

“代码确认”表示成本路径确实存在，不等于已证明它支配真实应用。下表保留实施前定位；第 2 节是原始调研实验，实施期补测与已落地结果见对应章节及 PROGRESS。M34／M44 数字均为历史记录，不能充当当前版本基线。

| 优先级 | 已定位的成本 | 影响场景与证据 | 方案入口 |
| --- | --- | --- | --- |
| P1 | release 的 LLVM IR pass 主要为局部化简，没有一般循环优化／向量化管线 | 数值计算；[policy.rs](../../compiler/codegen/src/statepoint/policy.rs)，本次机制实验 | 第 3.1 节，先区分无 site 与 Managed 代码 |
| P0 | 每条 Managed 回边执行四次 acquire load 和条件慢路径 | 很短的热循环；[poll.rs](../../compiler/codegen/src/function/gc/poll.rs)，本次机制实验 | 第 3.2 节，保留握手合同，批量 poll 单独评估 |
| P0 | 分配尺寸查询重复执行 TD 地址成员查找；TLAB 完成路径也经过它 | 小对象高频分配；[value_shape.c](../../runtime/src/value_shape.c)、[allocation.c](../../runtime/src/gc/allocation.c)，本次子操作实验 | 第 4.1 节，复用已验证的不可变 shape |
| P0 | `String.byteLength` 等常数时间操作经过 Scoop native transition | String／UTF-8 热路径；[strings.scoop](../../sysroot/src/scoop.core/src/strings.scoop)、[strings.c](../../runtime/src/strings.c)、[native.rs](../../compiler/codegen/src/function/call/native.rs)，代码确认，未测当前端到端占比 | 第 4.2 节，有限、类型化的表示 intrinsic |
| P1 | unbox／clone 检查中，锁内重复查询对象起点、大小与 TD | Any／interface／装箱值密集代码；[values.c](../../runtime/src/gc_binding/values.c)、[heap_objects.c](../../runtime/src/gc/heap_objects.c)，代码确认 | 第 4.3 节，单次定位及局部表示优化 |
| P0 | 1 MiB 共享 nursery、32 KiB TLAB block refill 取得 heap lock；release 对象走旧代分配锁 | 多 mutator 与短命对象；[allocation.c](../../runtime/src/gc/allocation.c)、[heap_internal.h](../../runtime/src/gc/heap_internal.h)，代码确认 | 第 5 节，优先分配成本与短暂停，已测后保留 1 MiB |
| P0 | mark 可并行，但 plan／copy／reference update／reclaim 仍串行 | 有大量搬迁的 full GC；[collector.c](../../runtime/src/gc/collector.c)，M44 历史 update 约 11.6 ms | 第 6 节，D2 已并行 update；桌面调研继续分解其余串行成本 |
| P1 | 普通 card mark 每次四级 page-map acquire 查找再执行原子 OR | 密集引用写入；[card.rs](../../compiler/codegen/src/function/gc/card.rs)，代码确认 | 第 5.3 节，无 safepoint 区间内复用与去重 |
| P2 | 每个 callable 独立创建 LLVM context／module，重新声明全 Cone 类型、global 和 callable | 大 Cone 的冷编译；[emit.rs](../../compiler/codegen/src/emission/objects/emit.rs)、[emission.rs](../../compiler/codegen/src/emission.rs)，尚无阶段耗时测量 | 第 3.3 节，按实际引用构造声明 |
| P2 | 普通外来 strong callable 不进入现有 MIR 内联候选，逐 callable LLVM module 也看不到其正文 | 小库函数层层包装；[inline/policy.rs](../../compiler/mir-lower/src/optimization/inline/policy.rs)、[prepare.rs](../../compiler/codegen/src/emission/prepare.rs)，代码确认 | 第 7.1 节，有限正文导入候选 |
| P2 | 有 suspend site 的 caller 在调用前保存 live values、发布状态并构造 adapter，即使本次调用同步完成 | 多层 suspend 包装；[coroutine/sites.rs](../../compiler/mir-lower/src/coroutine/sites.rs)，代码确认 | 第 7.2 节，静态不挂起与尾部转交候选 |

以下已有优化不能重复列为待修缺陷：interface 值已携带 itab，普通接口调用不再每次查表；分配统计已为线程独立计数；零搬迁 collection 已跳过 relocation；无引用对象不扫描出站引用；GC-free 小值直接返回与 compact `MaybeUninit` 表示已存在；没有 suspend site 的函数已跳过协程拆分。

## 2. 本次实验及其限制

### 2.1 方法与环境

主机 Apple M4 Pro，macOS 27.0.1（26A434），Homebrew LLVM／Clang 22.1.8。全部实验位于仓库外的临时目录，未修改生产源码或开发 sysroot。当前工作区没有可用的已准备 `sysroot/lib`，因此没有使用旧 `target/release` 二进制冒充当前 Scoop 版本的应用基线，也没有进行完整 CLI／GC 验收。

两组实验均先编译全部变体，再串行计时；每变体一次预热、七次独立进程样本，正反顺序交替，报告 kernel 中位数与最小／最大值。不固定 CPU affinity，未测 p95／p99；七个样本不足以给出可靠尾延迟结论。检查计算结果一致，计时区间不含进程启动和输入准备。

临时 C 文件先运行 LLVM 22.1.8 `clang-format`，再以 `clang -O2 -Wall -Wextra -Werror` 编译，未开启 LTO；LLVM IR 使用 `opt -verify-each`。这些检查只覆盖实验本身。

### 2.2 LLVM 管线与 poll 的机制对照

kernel 对 262,144 个 `uint32_t` 元素原地计算 `x = x * 1664525 + 1013904223`，按 32 位回绕；重复 256 遍，每进程共 67,108,864 次元素更新。输入为 `data[i] = i`，末尾对全部元素求和，四个变体均得到 `562954449584128`。

手写 LLVM IR 有两种：无 poll；每回边读取 epoch、phase、mode、observed 四个 acquire 原子值，不满足快路径条件时调用外部 `bench_slow()`。全程处于快路径，slow call 计数均为零。每种分别运行当前 release 的精确 pass 串和 `default<O2>`：

```text
function(sroa,mem2reg,instcombine<no-verify-fixpoint>,early-cse,dse,adce,early-cse<memssa>,sccp,unreachableblockelim)
```

两者随后均使用 `llc -O2 --disable-tail-duplicate --tail-dup-placement=false`，目标 `arm64-apple-macosx27.0.0`，不指定本机 CPU 特性。harness 与 kernel 独立编译。

| IR 形态 | 当前 pass，ns／元素 | `default<O2>`，ns／元素 | 优化后形态 |
| --- | ---: | ---: | --- |
| 无 poll | 0.256 [0.253, 0.259] | 0.051 [0.050, 0.052] | 当前为标量；O2 有 `<4 x i32>`，机器码有 `mla.4s` |
| 四次 acquire＋条件调用 | 1.007 [0.995, 1.015] | 1.005 [0.986, 1.042] | 两者均为标量，均保留四次 acquire 读取 |

结论：当前局部 pass 留有明显的循环优化空间；这一个循环中的 poll 形态同时限制了向量化，完整 O2 没有消除其成本。不能由此得出“所有 Scoop 程序可加速五倍”，也不能将两行差值解释成孤立的四次 load 延迟。

限制：数组是普通地址空间的 native 存储；实验不含 Scoop 数组边界检查、AS1、真实 root／relocation、TLS 布局、并发 collector 或 RS4GC。无 poll 变体只是机制对照，不是合法删除 Managed poll 的实现证明。

LLVM 官方区分普通 IR 优化管线与机器代码生成优化；默认 O2 IR 管线由 `buildPerModuleDefaultPipeline` 构造，不能仅根据 TargetMachine 的 O2 推断已经执行全部 IR 优化。[New Pass Manager](https://llvm.org/docs/NewPassManager.html) 也因此只能用于解释管线差异，实际结论以上述 LLVM 22.1.8 实验为准。

### 2.3 TypeDescriptor 查找的子操作对照

编译真实的 `runtime/src/value_shape.c`、`image/lookup.c` 和 `image/active.c`。候选只在临时副本中移除 `operation_shape()` 的 `scoop_image_require_type(td)`，保留尺寸、对齐、kind 和溢出计算。harness 预建地址有序的 registry，构造 minimum size 32、alignment 8 的 fixed-object TD，发布后调用 `scoop_shape_normalize_allocation(td, 32)` 五百万次。

64／4096 个 TD 是用于观察查找规模影响的合成输入，不是已测应用规模。固定模式总访问数组第一个 TD；轮转模式按索引顺序循环。每轮返回值总和均为 `160000000`。

| TD 数／访问模式 | 当前实现，ns／次 | 去掉重复查找，ns／次 |
| --- | ---: | ---: |
| 64／固定 | 10.859 [10.411, 11.073] | 1.156 [1.124, 1.194] |
| 64／轮转 | 8.461 [8.288, 8.705] | 1.164 [1.143, 1.215] |
| 4096／固定 | 26.316 [26.135, 26.994] | 1.171 [1.154, 1.290] |
| 4096／轮转 | 23.975 [23.847, 24.266] | 1.176 [1.161, 1.202] |

结论：地址成员查找足以支配这个尺寸计算子操作，值得从真实分配热路径移除重复工作。现有实现是 `bsearch` 成员查询，不能描述为每次重放完整 metadata 语义验证。

限制：registry 为实验直接构造，未执行真实 image 装载／完整启动验证；候选仅说明边界外复用结果的潜力，不是公开 ABI 可接受任意 TD 的依据。计时没有 bump、清零、对象头／起点发布、锁竞争、分配或 GC，因此这些倍数不能当成 TLAB 或应用加速比。

### 2.4 历史证据如何使用

[M44 性能记录](../milestone44/PERFORMANCE.md)的最终 Darwin 数据中，compact full 的 update 中位为 11.637 ms，请求到 RUNNING 为 55.258 ms；wide／array／chain 的 update 均为零。它说明有搬迁的图值得单独评估回写，而不是所有 GC 都有同一份 update 成本。不同指标的中位数不能相加或相减推出其余阶段耗时。

[M34 性能记录](../milestone34/PERFORMANCE.md)中，Darwin 两百万次接口调用的 converted-once kernel 为 6.453 ms、converted-each 为 126.691 ms；当时没有 GC，普通 unknown 模式已直接使用 itab。这支持检查重复转换本身，不支持重新设计普通接口调用。两份记录都需在 M56 实施基线上复测后才能决定收益。

## 3. 编译器与生成代码

### 3.1 分层扩展 release 优化

现状：MIR release 执行去虚拟化、局部内联、再去虚拟化；LLVM 使用第 2.2 节的局部 pass。machine O2 已开启，并非整个后端以 O0 编译。现有根定稿在普通 LLVM 优化之后、`rewrite-statepoints-for-gc` 之前执行。

拟采用方案：

1. **真正无 safepoint 的 NoGC callable**：基于现有 typed effect 与实际调用合同选择 O2 候选管线，验证循环规范化、归纳变量化简、LICM、loop／SLP vectorization 的收益。不要通过扫符号名或只检查“当前没有 `statepoint` 指令”推断 NoGC。
2. **Managed callable**：先增加不复制 site、不改变握手位置的标量与局部优化；每组 pass 通过精确 GC／异常／原子用例后再加入固定 release 管线。继续复用现有最终根计划，不添加第二套全图语义复核。
3. **会复制 CFG／调用的变换**：在物理 site 与 roots 产生前的 typed MIR／LIR 变换中完成，给新增实体分配正常 typed ID，再生成完整根计划。不能让 loop unroll／unswitch／inlining 复制 LLVM 上已有的 `statepoint-id` 后，仅靠重编号修补。
4. 保留严格浮点、整数回绕／异常、求值次数、Context、初始化和 release-ready 的既有语义。memory attributes 只表达实际实现事实，不能把 poll 或访问 managed 存储的调用标为无外部状态以诱导向量化。

现有 [finalization.rs](../../compiler/codegen/src/statepoint/finalization.rs)要求每个保留 site 的 owner 与 root identities 完整，拒绝重复 site、重复 root 和残留恢复。LLVM 默认 O2 包含的变换不自动满足这一 Scoop 合同，因此本设计不批准直接全局替换 pass 字符串。机器阶段禁止 tail duplication、Linux 固定 outgoing argument area 等约束保持。

LLVM 官方建议尽量在抽象 GC 形式上完成优化、较晚执行 RS4GC；这支持当前总体顺序，但不替 Scoop 验证 site identity 和异常边。[Statepoints](https://llvm.org/docs/Statepoints.html)；循环向量化是否成功也取决于内存依赖和调用效果，不能把“加入 vectorizer”等同于“已向量化”。[Vectorizers](https://llvm.org/docs/Vectorizers.html)

验收除运行结果外，保留少量专门的 IR／机器码断言：应向量化的 NoGC kernel 确实出现向量操作；Managed call／invoke 的唯一 site、活跃引用、正常与 unwind relocation 正确。普通语言 fixture 不增加全量 IR 快照。

### 3.2 Poll 的吞吐与响应合同

当前入口与每条回边（含 continue）都有 poll，快路径读取 epoch、phase、线程 mode 和 observed epoch；TLS poll-state 指针已经在函数内缓存。不能把四次读取提升到循环外，也不能只删除“没有分配”的循环中的 poll；另一线程仍可能请求收集。

首轮保留此合同，优化 poll 周围的冗余计算与控制流，并测量入口 poll、回边 poll、真实慢路径分别占比。不将 poll 数量减少本身作为正确性或性能验收结果。

**候选：有界分块循环（strip-mining）。** C1 的真实 Managed 数组循环仍没有稳定改善，满足继续评估的进入条件。将单层循环分成外层块循环与内层循环，外层每块进入前 poll，内层最多执行编译期选定的常数 N 次。整个循环的 trip count 可以在运行期才知道；编译器需要确定的是每块上界与单次迭代的工作有界性，不是推断整个循环的精确执行时间。

对于已证明 `0 <= i <= limit`、步长为 1、limit 不变且循环体无异常出口的计数循环，可采用以下示意；`limit - i` 避免直接计算 `i + N` 溢出：

```text
while i < limit:
    poll()                         // 现有入口 poll 仍保留
    base = relocated_managed_base  // 慢路径返回后使用更新后的引用
    count = min(N, limit - i)
    for offset in 0..<count:
        body(base, i + offset)
    i += count
```

这不是把任意 while 改写成只在外层检查条件。原条件若必须逐次求值，内层仍逐次求值且次数、顺序不变。实际 CFG 中所有普通回边和 `continue` 必须经过扣减块计数的 latch；`break` 退出原循环，不能只退出内层。未知步长、整数回绕可能改变终止性、循环内修改界限、有副作用的条件等不符合首版计数循环条件时，保留原循环；优化失败不产生编译错误。

首版候选限于能证明每次迭代工作有界的计算／内存访问：排除分配、挂起、未知或可能阻塞的调用、不受控内层循环、原子等待和未处理的异常出口。`NoGc` effect 本身不证明执行时间有界，不能仅凭它允许任意调用。数组快速路径必须已有合法范围证明，或以不改变原异常及部分写入行为的 guard 选择；不能为向量化提前触发本应较晚发生的异常。包含 managed 数据的内层只是无 safepoint 区间，不改变源码 `@NoGC` 的含义；每块之后丢弃 derived address，从已更新的 base 重新计算。

变换应在 typed MIR／LIR 的 site、liveness 和 roots 定稿前完成，外层保留显式 loop poll target；不能在 LLVM statepoint 生成后直接删掉内层 poll。C1 当前只对真正 NoGc callable 启用循环管线，因此分块还需要仅优化经证明无 site 的内层区域；不把整个 Managed 函数交给可复制外层 site 的默认 O2，再靠重编号修补。该后端接入及完整 CFG／移动根测试仍未实现，以下实验不是已交付功能。

N 是局部优化参数，可从少量候选中按循环体形态选择，再以吞吐与响应实测校准，不引入按逻辑指令收费的通用预算。启动测量用 16／64／256／1024；达到吞吐平台后优先较小值。迭代数上界不等于硬实时上界，缺页、调度、缓存未命中仍可能放大一块耗时。这会改变“每条回边”的现行合同，正式启用前必须先修订运行时规范 3.2 和实现规范 2.19；当前生产合同保持不变。

#### 3.2.1 分块机制与真实握手实验

2026-10-19，在本机 M4 Pro／LLVM 22.1.8 上，用仓库外 C 原型表达相同的四次 acquire poll 与内层分块，使用当前真实 runtime、线程／VM 实现及测试用合成 stack map。一个可搬迁对象包含两组 2049 个 UInt32，执行十万轮 `destination[i] = source[i] * 3 + 7 + round`，包含尾块；每块慢路径后从更新的根重算地址。Clang O2 的计算循环在 N=1 时保持标量，在 N>=16 时有 `<4 x i32>`。一轮预热、七轮交替独立进程，完成构建后串行测量，checksum 均为 `211206822`：

| N | 无 GC kernel 中位 [最小, 最大] ms | 64 次搬迁 GC 的 kernel 中位 ms | 停稳 p95 的样本中位 µs | 请求到 RUNNING p95 的样本中位 µs |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 215.935 [213.981, 224.031] | 217.943 | 9 | 40 |
| 16 | 18.206 [17.602, 18.540] | 20.333 | 8 | 40 |
| 64 | 12.544 [11.780, 13.583] | 14.857 | 9 | 38 |
| 256 | 12.417 [11.647, 13.625] | 14.856 | 9 | 41 |
| 1024 | 12.378 [11.876, 12.727] | 14.566 | 8 | 40 |

并发组另一线程每次休眠至少 50 µs 后发起收集，每个进程均验证 64 次收集与 64 次根地址变化，最终逐元素检查源／目的值。停稳从 STOPPING 记录至 STOPPED；请求到 RUNNING 从 collector 调用前记录至真实 RUNNING 测试点，不以阶段耗时之和代替。表中是各进程内部 p95 的七样本中位数；观测到停稳最大值可达 37 µs、请求到 RUNNING 最大值 151 µs，不能把中位／p95 写成上界。

该数据说明分块能同时降低 poll 频率并打开向量化机会，且此用例在 N=64 附近已达到吞吐平台；不能外推整个应用加速比例或直接选定生产默认值。对象工作集在 L1 内，原型使用 C 地址与合成 stack map，不包含 Scoop 数组检查、真实 AS1／RS4GC 生成及自动循环变换。正式接受还需真实 Scoop 编译产物、nuc12、缓存外工作集与更重单次迭代的吞吐／响应对照，以及 continue／break／零迭代／异常／尾块的语义覆盖。

### 3.3 编译耗时：避免逐 callable 重建全量声明

现有 emitter 串行遍历 callable；每次新建 context／module，并声明全 Cone 的 TD、global、callable 和外来 callable。若有 F 个 callable、D 个描述符和 G 个 global，仅这部分声明工作就有约 `F × (F + D + G)` 的重复构建趋势；这不是完整编译复杂度或实测耗时结论。

先分别记录普通 release 冷编译中的 IR 声明构建、优化、machine emission 和对象处理时间，并按 100／1000／5000 个小函数以及不同类型／调用边数量测量增长。完整 CLI 冷编译与缓存命中耗时另列，不能用此内部阶段替代用户可见耗时。

若占比显著，复用完整 LIR 已有的 typed references，预先索引当前 callable 所需的函数签名、TD、global、异常 personality、初始化单元、callback 和 metadata/support atoms，再只声明所需闭包。前向引用仍先声明后定义，同一 typed ID 只产生一个声明；不使用 FQN 回退，不机械地把完备输出改成到处 `Option`。

保留逐 callable object 的物理定义／ODR／链接选择合同，不为避免重复工作先改成全 Cone 单一 object。只在声明重复消除后再评估有限并行 codegen；LLVM Context 不跨 worker 共享，峰值 RSS 与确定性要同时测量。缓存命中仍在读取源码、探测工具和准备链接前返回，不恢复命中产物审计。

本机补测已确认声明构建随小函数数量快速增长：1000／5000 个简单 NoGC 函数约花 1.20／31.25 s 构造声明。C1 新管线与原管线的完整冷编译时间在样本波动范围内接近；详见[实施记录 E](PROGRESS.md#e有条件优化的进入调查)。声明裁剪作为有证据的独立后续改进，先限当前 callable 实际需要的函数声明，再处理 TD／global 闭包；不把全面 emitter 重写加入本期 A–D 的完成门槛。

## 4. Runtime 热路径

### 4.1 将不可变 TD 验证留在真实边界

启动已经收集并验证不可变 metadata。`operation_shape()` 又通过 active registry 和 `bsearch` 查询成员资格，分配尺寸计算、TLAB finish 及多种表示操作重复付费。

拟采用方案：梳理 generated-code、runtime internal 和 native FFI 的调用角色。在已经由编译器／装载入口确定合法 TD、且期间没有变化的内部路径，直接使用 `instance_shape`；需要验证外来输入的公开边界只做一次必要检查，再调用同一份 shape／size 实现。没有新增运行期“已认证 TD”对象、令牌或另一个 reader。

保留实际需要的 requested size／count 检查、整数溢出、最大对象大小、对齐、abstract 类型不可分配及 fixed-size 一致性。不能把“TD 已验证”推导成“动态 requested size 必然正确”。共享计算 helper 避免 checked／internal 两份尺寸算法漂移。

TLAB cursor 更新之后仍需按现行合同完成清零、对象头、精确大小、起点与统计发布；候选只消除重复查询，不直接删除这些工作。真正的程序基准需测整条分配路径，不能用第 2.3 节替代。

### 4.2 常数时间表示操作走 typed intrinsic

`String.byteLength` 的 C 实现只读取 `len`，但 core 包装经普通 `@Extern(abi="scoop")` 调用。LIR／codegen 为它建立 NativeBorrowed transition、发布 roots、进入并退出 native 状态。正常 borrowed 快路径不必总取得全局 mutex，成本描述应以真实 transition 为准。

拟采用方案：先将 `byteLength`、有界的 byte-at 与按已有效字节 cursor 解码一个字符的操作，接入现有 intrinsic 目录和 typed lowering。byteLength 可直接加载；访问字节保留索引检查；单字符解码最多消费一个 UTF-8 标量，保持既有合法输入与失败行为。由普通、可校验的声明身份绑定 intrinsic，不能按 extern 符号拼写偷换任意函数。

core 对应内部声明与 compiler 一起调整，公开 String API 与值不变。生成的 managed load 或内部 GC leaf 使用正确 AS1／借用作用域，不生成 native transition；必要的 trap／异常出口按原合同处理。native FFI 的旧入口是否保留按现有 ABI 兼容规则决定，不能无说明删符号。

源码 `@NoGC` 禁止 managed ref 参数／访问，不能给 String 方法贴该注解规避检查；普通 Scoop extern 的 NoGC effect 也不自动取消 NativeBorrowed 边界。本方案不增加公开的“允许 ref 的 NoGC”注解。

`String.length` 当前按 UTF-8 字节扫描，随机 `get(i)` 从头寻找第 i 个标量；循环依次调用 `get(i)` 可形成二次复杂度，而当前 iterator 持字节 cursor，本身是线性扫描。前述 intrinsic 改善每步固定成本，不改变随机访问复杂度。缓存标量长度／索引需要对象表示或库策略变更，暂不纳入；无界 length／hash／compare 扫描也不能整体标为不合作的 leaf。若测出 GC 等待，应另评估带 roots 的分块扫描或合法 pin／NativeSafe 路径。

### 4.3 单次定位与局部装箱消除

现有 unbox 先在 heap lock 内验证 object start，再为读取精确大小重复做 `object_meta` 查找，并进行 TD 与 shape 检查。先把这一操作收敛为一次定位得到 block／word／exact size，在同一受保护区间内复用；保留实际类型匹配、对象边界与必要动态检查。不能将短暂定位结果缓存到 safepoint 之后，也不因单线程实验快就删除多 mutator 所需同步。

在 MIR 已知具体值类型、转换／装箱只供紧邻消费且不逃逸时，评估直接传值、去虚拟化后消除临时 box，或复用同一引用的既有 interface conversion。位置放在 LIR safepoint／root 生成前，避免 runtime 为本可消除的表示付费。

只使用现有类型、数据流和别名事实，首版限定局部模式；`Any`／引用身份、可变值复制、`this` copy、内部可变状态、地址取得、pin／handle、异常与跨挂起使用不能受影响。不能把有状态的 boxed receiver 错改为每次重建的值副本，也不能把重复转换查找与普通 itab 分派混为一谈。

## 5. 分配与写屏障

### 5.1 先拆开分配成本

实际快路径不只有 bump：finish helper 还执行尺寸归一化、清零、对象头初始化、side metadata 登记与线程统计；小对象起点和 line 占用使用原子更新。refill 每次取得一个 32 KiB block，并在 heap lock 下处理共享 nursery。超过 32,640 bytes 的普通对象走大对象 mapping；带 release hook 的小对象预置于旧代并使用旧代分配锁，不能归入普通 nursery 吞吐。

按普通 GC-free／含引用／boxed／release／large 五类测量：TLAB 内分配、refill、完整分配＋回收分别报告。复用现有计数，必要的 refill 次数或锁等待测量只放显式诊断／实验模式，避免在每次生产分配上增加时钟读取。

优先落地第 4.1 节。只有剩余成本被确认后才考虑减少 helper 往返、合并同一 line 的登记或 owner-only 写入；必须证明其他线程和 collector 如何观察已发布 metadata。计数已为线程独立 relaxed 发布，不能再以“消除全局统计 RMW”为新收益。

不全局取消清零：当前返回前清零是 GC／对象构造合同。若后续要消除确定会覆盖的字节，必须先定义部分初始化状态与异常／GC 可观察边界；这不作为 M56 首版前置。release 对象旧代 TLAB 也先测锁占比，不直接套用年轻代资格判断。

### 5.2 Nursery 与 TLAB 策略

当前 nursery 固定 1 MiB，多线程共享；minor 默认一个 marker，当前实现对 young survivor 作晋升。短命对象高吞吐时，小 nursery 可能增加 collection 频率；提高容量又可能提高单次复制量、暂停与驻留内存，不能仅凭容量常量判定错误。

使用 1／4／8／16 MiB 候选，分别测试低存活率、跨数轮存活和高存活率，以及 1／2／4／8 个 mutator。记录完成相同业务工作所需时间、minor／full 次数、晋升与复制字节、请求到 RUNNING、峰值 RSS／committed bytes、TLAB 未用空间和 refill 等待。nursery 容量与整体 collection threshold 分别报告，不能扩大两者后将所有收益归因于 nursery。

先选择有证据支持的简单默认值；保留小堆和单线程表现。若 refill lock 仍明显，评估每次领取少量 block 或复用已领取区间，但区间归属、STW 退役、detach 回收和未用容量必须完整处理。不能把竞争中被其他 mutator 领走 nursery 误报成 OOM，也不能按固定重试次数放弃合法分配。

容量选择是 GC 策略，不进入语言合法性、对象 ABI 或产物兼容性。没有稳定优势时保留现值并记录结果，不为“自适应”建立通用调参框架。survivor space、多级年龄晋升、pretenuring 预测先留后续，需要真实寿命分布再设计。

本机完成完整矩阵及无额外插桩复测后，M56 选择保留 1 MiB：大容量在全存活批次中减少 minor 搬迁，但短／中寿命用例没有稳定全面改善，单次停顿与峰值 RSS 增长明显。临时计数器对多线程时序有可见影响，吞吐取舍以无插桩数据为准；详见 [实施记录 D1](PROGRESS.md#d1nursery-容量与-refill-取舍)。不追加批量 refill 或自适应策略。

### 5.3 无 safepoint 区间内优化 card mark

当前 card mark 先以 shifts 52／40／28／16 查询四级 page-map，再读取 region base／cards，按 512-byte card 执行 relaxed atomic OR。region 不是按自身大小对齐，不能用 16 MiB 掩码代替真实映射。

首版只处理可证明的局部模式：同一对象、连续且中间没有 safepoint／调用／原子访问／同步发布的普通引用字段写入，可复用一次映射结果。同一对象完整地属于一个 region／large mapping，因此跨 card 的字段也可复用 base／cards，但必须按各自实际地址计算并标记 card；相邻静态字段不必然同 card。冗余 mark 首版仅消除精确相同字段槽的重复写入，不增加动态 card 比较分支。局限单 LLVM basic block，local 写入等可能改变 receiver 的操作使缓存失效；异常出口前仍完成已发生写入所需的 mark，保持 store 与标记的必要顺序。

metadata 指针不跨 safepoint 缓存。不同 mutator 可同时写同一 card，保留原子置脏，不能用非原子 store 替换 OR；“先读 dirty 再决定是否 OR”是否划算须实测。AtomicRef 的 release／acquire 与 CAS 成败语义单独保留，首版不跨这类同步操作合并。

仅凭“刚分配”不能省屏障：分配后可能已经 GC／晋升，release／large 对象也不一定在年轻代。进一步省略年轻对象屏障必须证明当前 allocation class、尚未经历 safepoint、未逃逸及初始化可见性；该扩大不作为首版要求。

## 6. GC：优先处理有搬迁时的回写

### 6.1 保留现有阶段边界

现有 collector 的 roots／remembered 生产初始工作、plan、copy、update、reclaim 与 VM 归还由 coordinator 执行；mark 有常驻 worker。默认大堆按 CPU 数最多四个 worker，minor／小堆默认一个，`SCOOP_GC_WORKERS=1..8` 已可做同义性对照。

M56 优先评估 **copy 完成后，对存活副本的出站引用并行更新**。不同时引入并发 GC、read barrier、并行搬迁、第二套 collector 或通用任务系统；先保持 forwarding 与对象存储在 update 期间稳定，减少正确性变量。

### 6.2 并行 update 的具体方案

1. coordinator 完成 mark、搬迁规划与复制，forwarding 结果只读；所有 mutator 仍已停稳，heap／roots 生命周期保护持续有效。
2. roots 与 minor remembered slots 先按现有顺序串行更新，完成后才发布 live-object update 工作，避免同一 slot 由 root／remembered 与对象任务并发改写。
3. 复用 mark 得到的稳定 live-object 清单。普通对象各归一个任务；大引用数组可按互不重叠的元素区间拆分，GC-free 对象不产生扫描任务。
4. 发布任务前由 coordinator 完成对象 relocation claim 与当前副本定位。现有 `scanned` bitmap 使用普通 bit set，**不能只因为对象不同就让 worker 并发改写同一 bitmap word**；首版由 coordinator 写该 bookkeeping，worker 只扫描分配给自己的槽。数组也只 claim 一次再拆分。
5. worker 只读稳定的 region／对象／forwarding 视图，不重新取得 coordinator 持有的 heap lock。每个目标槽恰归一个 worker；指向同一目标的不同槽可独立改写。必要计数按 worker 累计后归并。
6. 在现有内部 worker pool 增加专用 update phase；完整完成屏障之后才能验证、释放 source 存储、执行 release hook、reclaim、归还 VM 或恢复 mutator。创建 worker 失败沿用串行回退，任务存储失败走既有明确失败出口。

任务颗粒度按实际引用槽数／扫描量测量，避免一个巨大数组只占用一个 worker，也避免把每个 slot 都排队。首次只选择有足够工作量的 collection，零搬迁仍直接跳过 update；小 minor 不强制启线程。

分别测量 claim／任务发布、槽扫描、等待和整个请求到 RUNNING；串行 claim 可能成为下一上限，不能仅报告 worker scan 加速。全程复用同一 scan 解释器与转发规则，后续并行 copy 需要单独论证空间预留、目标分配与 forwarding 发布，不顺带加入。

本机原型及最终实现已验证该取舍：4 worker 的普通稀疏 region full 整体中位从 41.874 ms 降到 34.297 ms，单 worker 未测到明确退化；详细工作集、时间边界和限制见 [实施记录 D2](PROGRESS.md#d2搬迁后的并行引用回写)。因此采用上述专用 update phase，沿用本轮 mark 的 worker 数，不新增独立调参策略；Linux 仍需复验。

### 6.3 扫描量与停顿口径

容量很大的引用 backing array，即使上层容器 size 很小，GC 仍按数组自身的真实 length 扫描。运行时不知道某个容器的逻辑 size；不能按 `ArrayList` 等类型名字跳过数组尾部。若要 live-prefix 表示，须由对应库与对象 scan 合同共同维护，本期只用此类输入观察扫描成本。

持续区分无搬迁 full、有搬迁 full、minor、长链、大数组、宽图、pin 与 release。已存在的零搬迁跳过、无引用对象跳过、stable live set 和显式 heap 验证保持；普通运行不启用全堆重型交叉验证。

沿用运行时规范 3.5／3.9 的时间边界：stop wait、已停稳阶段、请求到 RUNNING、业务实际进度分别报告。阶段可能重叠，阶段和或中位数差不能代替整体停顿；mark 扩展性不代表总 GC 扩展性。诊断开关、worker 数与 stress 状态必须随样本记录。

### 6.4 桌面交互与低配置机器

按用户补充的桌面用途，短暂停和分配速度优先于批量 GC 吞吐；不能仅靠扩大 nursery、长期推迟 full 或增加 worker 换取表面成绩。新增[补充设计](DESKTOP-GC.md)分解 minor 回收、旧堆遍历、side metadata、VM 归还、首次 worker 创建及 native／release 停顿，给出低复杂性候选、实验边界与是否进入增量 major 的条件。现有 runtime 合同保持，新增调研不自动启用增量／并发 collector。

## 7. 有条件进入的编译优化

### 7.1 跨 Cone 小函数内联

逐 callable module 有利于现有物理选择，但 LLVM 看不到被调用的普通 strong body；现有 MIR 内联候选只含本地／单态化正文。泛型实例在下游已有优化机会，不能笼统声称所有跨库调用都不能内联。

候选方案是在正常 `.slib` 消费中按 typed callable identity 选择少量可用 MIR 正文，进入既有 MIR 内联器，先限无 EH、非协程、可闭合依赖的小函数。复用现有局部成本／增长策略，只影响优化选择。缺少可用正文时保持普通调用，不产生编译错误。

导入必须保留定义方身份、实际 private 引用、泛型实例归属、初始化与 shared state 唯一性、Context 及 ODR 选择，不把调用方可见性扩展为定义方权限，也不靠 FQN 找替代定义。若现有 metadata 不包含所需正文或引用，先明确正常格式扩展及兼容版本，不增加正文授权凭证。

进入条件：跨 Cone benchmark 证明此调用边界仍有显著成本；同时给出编译时间、峰值内存、`.slib`／机器码大小的变化。源码 `inline` 当前是兼容性修饰符，本方案不静默赋予新的语义，也不要求全程序 LTO。

### 7.2 协程 frame／adapter 消除

当前有 suspend site 的 wrapper 建 frame；site 在调用前保存 live values、发布 suspended state 并分配 adapter，再判断返回的 `CoroutineStep`。同步完成不自动免除这些分配；没有 site 的函数已走普通实现，不再为它重复设计优化。

先测一层／多层包装的同步完成、真实挂起、跨线程恢复、异常与取消，记录每次操作的 frame／adapter 数和字节。仅对两个局部模式提出候选：

- 静态可确定被调用正文不会挂起且 continuation 不逃逸：在 coroutine lowering 前选择等价同步实现或消除该 site，让现有无 site 路径生效。不能依据某次运行同步完成就做此判断。
- 尾部 suspend 转交：返回结果无需加工、无调用后 live 状态、无待执行 catch／finally／Context 恢复、无独立完成状态义务时，直接传递既有 continuation。父 frame 的唯一完成／失败与 Context 行为必须仍然成立。

复用完整 typed MIR 数据流和原 CoroutineStep ABI，不新增可复用对象池、栈上逃逸 continuation 或另一套调度器。保留 release／acquire、单次获胜恢复、TaskContext、异常引用与取消语义。MIR 的尾部转交不是解除 LLVM 机器层禁止 tail duplication／tail call 的理由。

不能证明这些条件的 site 保持原实现；若有限模式收益不足，记录结果后留后续，不把全程序逃逸分析变成本期交付前置。

## 8. 规范、ABI 与实现边界

实施时先更新真正受影响的合同，再改代码；只改变私有实现和测量选择时，不虚构语言行为变更。

| 改动 | 必须核对／同步的权威合同 | 兼容性处理 |
| --- | --- | --- |
| release pass 与物理根计划 | 实现规范 2.3、2.4、2.5、2.19；运行时规范 3.2 | 普通优化不改源码语义或公共 ABI；若 backend/root 格式实际变化，按现有版本规则更新 |
| TD 验证边界、分配／unbox | 运行时规范 2.2、2.3、3.1、4.1；实现规范 2.4、2.5 | 内部 helper 无需制造新 artifact 身份；公开 FFI 检查和符号变化单独列明 |
| String 表示 intrinsic | 语言规范 11.4、13.1、13.2；运行时规范 2.4、第 6 章；实现规范 2.10 | 同步实际 core 声明与 typed intrinsic 表，沿用现有工具链／sysroot 兼容流程 |
| card mark 合并 | 运行时规范 3.6；实现规范 2.4、2.5 | 保持共享快路径布局与原子语义，首版不改对象 ABI |
| nursery／refill／并行 update | 运行时规范 3.1、3.5、3.7、3.9 | 属于 runtime 策略；线程／转发／扫描合同写清，容量不成为产物兼容条件 |
| 分块 poll（后续候选） | 运行时规范 3.2、3.5；实现规范 2.19 | 明确改变回边响应合同，不能仅作为 pass 参数悄悄启用 |
| 跨 Cone 内联（候选） | 实现规范 2.3、2.13、2.19；语言规范 8.4 | 仅在确需 metadata 扩展时变更格式，不改变 `inline` 源码含义 |
| 协程消除（候选） | 语言规范 8.2、8.3、11.9；运行时规范第 8、9 章；实现规范 2.3、2.4 | 保持对外 Continuation／CoroutineStep 与跨线程恢复合同 |

各 stage 仍只通过 IR／meta crate 交换结构完备的结果；driver 只编排。正常优化事实在产生位置计算一次，未改变时复用，不要求后续 stage 重跑整套分析来证明前一步做过。平台特定选择放在已有平台边界，不把 LLVM 或 collector 私有策略扩散到通用 IR。

## 9. 实施顺序与完成条件

| 步骤 | 交付内容 | 完成条件 |
| --- | --- | --- |
| A：建立真实基线 | 准备当前开发 sysroot；固定 release、目标、输入、诊断状态和 workload；复测当前 CLI 程序 | 结果可重复，区分运行／编译时间，已有失败先闭合 |
| B：低风险热路径 | TD 验证结果复用、unbox 单次定位、String 有界表示 intrinsic | 对应边界与组合 fixture 通过；整条操作收益可测，必要检查与异常不退化 |
| C：代码生成 | 无 site／NoGC 的循环优化；Managed 保守优化；局部 card lookup／mark 复用 | 目标 kernel 的实际代码形态改善，site／root／EH／原子合同通过，编译耗时与体积可接受 |
| D：分配与 GC | nursery／refill 对照；有搬迁 collection 的并行 update 候选 | 完成正确性及跨目标性能对照；只有稳定净收益的策略进入默认，负结果明确保留现值并关闭该候选 |
| E：按证据追加 | 声明构建重复、局部 box／协程消除、跨 Cone 小函数内联 | 分别满足第 3.3、4.3、7 节条件；未进入者明确记录原因与后续范围，不阻塞 B–D |

M56 的实现验收至少覆盖 A–D；其中 D 必须完成实验和取舍，并不预先规定 nursery 新容量、强制启用并行 update 或承诺统一加速比例。批量 poll、并行 copy、并发 GC、全程序逃逸分析、String 新索引表示和容器 live-prefix 不属于本期首版必交付项。

## 10. 正确性与性能验收

### 10.1 正确性

每项优化要有独立 fixture 和与其他能力组合的 fixture；变换不适用的输入应继续正确编译执行。必要的非法输入诊断、格式／边界错误与失败出口仍覆盖，但不以大量假想 helper 使用者扩大接口。

- LLVM／poll：循环嵌套、continue／break、零迭代、严格浮点、原子、call／invoke、异常合流、移动 GC 后的引用与 derived address；并发 collector 请求期间核对业务结果与进度。
- TD／分配：fixed／inline bytes／array／ZST、对齐、溢出、abstract、native 输入边界、部分构造异常、TLAB refill／detach、release、large 和 moving stress。
- String／表示：空串、ASCII／多字节 UTF-8、边界／越界、Iterator、异常、装箱含引用值、Any／interface 身份、可变值复制、pin／handle 与 GC 组合。
- card／update：旧到新引用、同 card 多写、跨 card、多个 mutator、AtomicRef 成功／失败 CAS、minor／full、无搬迁与强制搬迁、共享目标、循环图、巨大引用数组、pin、immortal、native／Context roots、release hook。
- collector 使用已有单 worker 与多 worker 开关核对等价结果，显式验证模式检查全部槽；普通运行保持相同轻量路径。worker 创建失败、任务完成与 shutdown 生命周期要有对应覆盖。

实施时按仓库要求先格式化与 lint，再运行相关 crate／runtime 测试、fixture runner 规则测试和正式 CLI fixture；完整里程碑还需 `cargo test --workspace` 与 `python3 tests/run_fixtures.py --all`。普通 fixture 使用显式准备的开发 sysroot，不隐式触发发行打包。

### 10.2 性能矩阵与决策

| 工作负载 | 主指标 | 必须同时记录 |
| --- | --- | --- |
| 算术／数组 kernel，无 poll 机制对照和正常 Managed 程序分别测 | ns／元素、实际向量化 | GC 请求响应、结果 checksum、代码体积；不能关闭 poll 后充当产品成绩 |
| String byteLength／逐字节／iterator；unbox／interface conversion | ns／操作、分配量 | 内部 transition／查找次数、短串与长串、异常与真实组合程序 |
| 小对象／含引用／release／large，1–8 mutator | objects／s、bytes／s | refill／GC 次数、晋升、停顿、RSS、TLAB 浪费及实际业务工作量 |
| wide／chain／array／compact full／minor | 请求到 RUNNING、业务进度间隔 | stop wait、各 GC 阶段、搬迁量、引用槽数、worker CPU、诊断开关 |
| 多层 suspend／跨 Cone 包装 | ns／操作、frame／adapter／box 数 | 同步／挂起／跨线程、代码大小、编译时间 |
| 小／大 Cone 构建、普通应用 cache hit | 总编译／链接时间、峰值 RSS | 各阶段、声明数量、产物大小、真实输入更新与缺失输入 |

在 Darwin/AArch64 与 Linux/amd64 分别比较同一目标的基线和候选；GNU／musl 至少覆盖现有目标合同，不能用一个平台的内联／原子成本推导另一个。正式测量采用多进程、交替或随机顺序、预热、固定输入与正确性结果；应用基准保留实际 GC 和代表性的组合特征。

吞吐使用足够多的独立运行报告中位数与离散程度；尾延迟另外收集足够样本。一个进程内多轮 GC 是相关样本，报告进程数与每进程轮数，不伪装成独立进程数。有限样本的差异落在噪声内时，结论为未证明改善。

每个候选同时报告收益、编译成本、内存与延迟回退。只在目标负载有可重复净收益且关键控制组无不可接受回退时启用；不预设“全部至少快 20%”之类与证据无关的门槛。负结果也可关闭候选，不能为保住假设扩大工作范围。

缓存验收保持源码、工具与原产物存在，用真实调用和输出验证复用；保留输入变化、缺失输入、失败不发布与普通并发协调测试。不删除／隐藏工具或源码，不读取命中产物追加生产过程审计。

## 11. 设计调研阶段的交付与清理

本轮只保留本设计及路线图引用。两组实验的源码、候选 C 副本、IR、汇编、objects、可执行文件和逐次 JSON 均在仓库外创建；结果已按第 2 节提炼，整个实验目录及其中全部临时文件已删除。未改动生产源码、sysroot、构建配置与缓存，因此不需要用全仓 reset／checkout 恢复。

开始工作时已有的 `docs/milestone47/DESIGN.md`、`docs/milestone47/PROTOCOL.md` 和未跟踪的 `docs/milestone55/DESIGN.md` 均属于用户工作；已按开始时的内容摘要核对，全部保持不变。设计及路线图的本地链接和空白检查通过；本轮没有运行完整编译器／runtime／CLI 验收。后续真正实施的原始测量资料按仓库约定放仓库外，只在验收文档保存方法、结果概要、失败闭合与限制。

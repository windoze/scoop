# 带外层 poll 的循环分块：M58 调研

本文属于 [M58 设计](DESIGN.md)。2026-10-19 的调查基线为 `04d28e763`；在 `c3b8942db` 上复核相关合同。自动标量分块与受限整数内层 SIMD 现已实现，范围与验证见第 6／7 节；下文的 AS1 数组机制实验不等同于生产数组支持。

## 1. 优化形态与收益来源

对于边界稳定、循环体无调用／分配／异常边的计数循环，可把原来的每轮 poll 改为每个有界小块 poll：

```text
// 原循环：i 从 0 到 n-1，body 不修改 i 或 n。
i = 0
while i < n:
    poll
    body(i)
    i += 1

// 候选：N 是编译期常数，末块可以少于 N 轮。
i = 0
while i < n:
    poll
    count = min(N, n - i)
    repeat count times:
        body(i)
        i += 1
```

这里编译期确定的是**内层最多 N 轮**，不是整个循环的次数，也不要求最后一块填满。对满足 `0 <= i <= n` 的非负计数，使用 `n - i` 限制剩余量，避免先算 `i + N` 再取 min 所带来的溢出。保留函数入口 poll。上例只是适用模式的伪代码，不是任意 while 的等价替换规则。

收益有两部分：降低 poll 快路径的原子读取／分支频率；给无 poll 内层提供向量化、归纳变量和局部移动优化的机会。调查基线的 [policy.rs](../../compiler/codegen/src/statepoint/policy.rs) 按整个 callable 的 typed GC effect 选择管线：NoGc 且无 site 才有 loop vectorizer，Managed 使用保守标量管线。因此**只移走内层 poll，不会自动获得 NoGc 的循环优化收益**；生产实现另加第 7 节的实际结构分析，只对合格整数内层请求向量化。

[OpenJDK C2 的 loop strip mining 设计](https://cr.openjdk.org/~roland/loop_strip_mining.pdf)使用同类外层 safepoint／有界内层结构，并讨论了循环变换期间的结构维护和短循环成本。这里采用其机制思路，不照搬 JVM 的节点类型、JIT profiling 或参数默认值。

## 2. 当前 Scoop 的插入位置

MIR 的 loop lowering 记录 `LoopHeaderPollTarget`，continue 经 cleanup 后跳向相应 header；普通内联会重映射这些目标。LIR 的 [complete_function](../../compiler/lir-lower/src/safepoints/mod.rs) 顺序为常量分支折叠／不可达块清理 → `insert_polls()` → root plan → safepoint identity。实际 poll 插在 header 入口，不能仅按源码末尾的“回边”想象条件求值顺序。

建议在**普通 MIR 优化完成后、LIR poll／root 生成前**做分块：此时算术与调用形态较清楚，又没有物理 safepoint ID。先限一个可识别的自然计数循环，建立正常 typed block／local，外层继承 poll 要求，内层明确带有编译器已证明的迭代上限。复用现有循环信息并只补当前消费者必需的数据，不设计通用循环证明系统。

不能只把内层的 header 从 poll 列表中删除而不保留为何可以省略的结构事实；后续 CFG 简化／复制必须维持这个事实，失去适用形态时保守保留 poll。新增的外层进入正常 LIR site 和 root 生成，不在最终 LLVM stackmap 上修补重复编号。

当前运行时规范 3.2 要求 Managed 入口与循环回边 poll。正式实现应先同步该条款、实现规范 2.19 以及相关 lowering 描述，明确“经编译器证明有界的无 safepoint 内层由外层 poll 覆盖”的允许条件。它是优化选择，不影响程序合法性、不进入运行时 ABI，也不逐操作计算通用 CPU 配额。

## 3. 初版适用范围与控制流边界

| 条件 | 初版选择 | 原因 |
| --- | --- | --- |
| 非负单调归纳变量、稳定上界、无溢出的步进 | 接受能由现有 typed 算术证明的简单形式 | 同时给出循环终止和末块边界 |
| 数组长度、简单整型算术、已证明合法的索引 | 优先候选 | 常见 GUI 像素／数值处理可受益；严格保持原整数与浮点语义 |
| 循环体含调用、分配、外部访问或潜在挂起 | 初版保留原循环 | `NoGC` 只约束 GC 行为，不证明调用会很快返回 |
| 原子等待、自旋、volatile、pointer chase、未知嵌套循环 | 初版保留 | 不能仅用外层迭代数证明每轮工作有界；原子也限制合法重排 |
| EH、finally／Context cleanup、break／continue、多出口 | 初版保留 | 先验证无这些边的基本形态，避免一次扩大 CFG 语义范围 |
| 总工作量很小或已有充分 poll 的循环 | 保留或按已有信息不选优化 | 分块本身增加外层控制开销；不要求动态 profiling 才能工作 |

之后若扩展 continue，必须让每个实际执行的原迭代都消耗一个 chunk 名额，continue 仍经过原本的更新／条件／cleanup 边，不能跳过 chunk 计数变成无限无 poll 内层。break 必须退出整个原循环；带标签跳转要维持目标身份。条件含副作用时不能提前多求值或把最后一次失败判断挪动；因此初版只接受稳定、无副作用的上界。

分块不自动授权消除边界检查。比如 `n` 可能大于数组长度时，原程序可能先写入若干合法元素才抛错；将检查提到块前会改变可观察副作用。优先处理 `n` 来自该数组不可变长度、索引可直接证明在界内的模式，不能仅为向量化制造提前异常。

## 4. 移动 GC 与 LLVM 管线

外层 poll 可能移动整个数组或其他 managed 对象。每块所用的数组对象必须来自该 poll 的 relocation；元素地址在 poll 后由更新基址重新计算。上一块的 derived pointer、借用裸地址或 vector pointer 不能绕过 root plan 带到下一块。payload 含多个 managed 引用时，每个跨 poll 活值都继续遵守既有精确根规则。

实施可以分两步验收：先在现有 Managed 管线下确认 poll 频率与语义；再仅为满足条件的无 site 内层启用必要循环变换。这样能够区分减少 poll 本身与向量化的收益，避免把二者混为一个 pass 字符串改动。

后一步需要限制作用范围：外层不做 unroll／unswitch／调用复制，内层才允许 vectorization／必要规范化；正常 root 定稿仍在 LLVM 普通优化后、RS4GC 前。loop metadata 可以参与选择，但不能代替真实结构分析或 Scoop 的现有 site／root 检查。单独给函数加 `NoGC`、把 poll 标为无副作用、或直接对所有 Managed 函数运行默认 O2 都不是本方案。

### 4.1 本次 LLVM 22.1.8 机制实验

手写一个 `gc "statepoint-example"` 函数，数组参数为 `ptr addrspace(1)`，运行时长度 `n`，内层上限 64；每元素计算 `x = x * 1664525 + 1013904223`，按 UInt32 回绕。入口与外层各有一个 poll，明确指定不同的 `statepoint-id` 101／102。外层禁用向量化、两层禁用 unroll，内层允许向量化。

实际执行：

```text
opt -verify-each \
  -passes='function(loop-simplify,lcssa,loop-vectorize),rewrite-statepoints-for-gc,verify'
llc -O2 --disable-tail-duplicate --tail-dup-placement=false
```

在 AArch64／Mach-O 上得到：

- 内层出现 `<4 x i32>` load／multiply／add／store，机器码包含 `mla.4s`；函数保持 Managed GC strategy。
- 两个静态 statepoint 均保留，ID 各出现一次；两个位置都有数组根的 `gc.relocate`，后续计算经更新基址进行。
- `llvm-readobj --stackmap` 显示两条记录、ID 101／102，各含数组的 base／derived 间接根位置。
- 链接 C harness，运行长度 0、1、3、15、63、64、65、2049、262144，逐元素值、数组两侧 guard 和 poll 次数 `1 + ceil(n/64)` 均正确。

该实验补上了旧 C 原型缺少的 **AS1＋vectorizer＋RS4GC 组合证据**。但 poll 的 C 实现仅计数，没有触发移动；手写 IR 没有 Scoop 的 logical root restore、异常边、真实数组检查和 site manifest。因此它不等于当前 Scoop codegen 已通过该管线，也不替代源码级强制移动验收。未以这份 harness 计时报吞吐收益。

### 4.2 与已有真实 runtime C 原型如何互补

原 [M56 原始实验记录](../milestone56/PROGRESS.md) 的 2049 元素、100,000 轮实验用真实 runtime 与 synthetic stackmap，对照 N=1／16／64／256／1024。无 GC kernel 为 215.935／18.206／12.544／12.417／12.378 ms；有 64 次强制 moving GC 为 217.943／20.333／14.857／14.856／14.566 ms。N=64 附近已接近该轻量、L1 工作集的吞吐平台。

那个实验验证了有界 poll 与实际搬迁的组合，但不包含真正 Scoop AS1／RS4GC 产物；本次 LLVM 实验验证另一半，尚未把两半连成正式编译器实现。两者都不能推断自动识别任意循环正确，也不能承诺 GUI 握手 p99。

## 5. N 的选择与短 STW 取舍

第一轮实现以 16／64／256 对照，64 可作为本例的起始候选；选达到吞吐平台的较小 N，而不是最大块。N 只决定编译期迭代上限，不根据一次平均 ns／iteration 推导硬时限，不需要添加 runtime 定时器、轮询计费或可调整状态。

poll 变少会延长另一个线程请求 GC 后等待当前块结束的时间，因此必须看 **stop-wait p95／p99／max**，不能只看 collector 已停稳的 `pause_ns`。cache miss、缺页、抢占及低配置 CPU 都会扩大墙钟间隔；[LLVM safepoint 文档](https://llvm.org/docs/Statepoints.html)同样区分正常条件下的及时到达与严重 paging 等最坏情况。编译期常数不是实时保证。

实现后的性能矩阵应至少包含：

| 维度 | 必要对照 |
| --- | --- |
| 生成策略 | 原逐轮 poll；分块但保守标量；分块且内层向量化 |
| 块大小／长度 | N=1／16／64／256；0、1、N−1、N、N+1、非整倍数、很长循环 |
| 工作集／算术 | L1、超出缓存、大数组；轻 body 与较重但有界 body；原有浮点／溢出行为 |
| GC | 无请求；并发线程反复请求 minor／full；强制移动；多个跨 poll 根及内部地址 |
| 桌面环境 | 一／多 mutator、两核限制或实际低配置机、其他线程有负载、空闲后突发 |
| 结果 | 每元素耗时、机器码体积、poll 次数、stop-wait 与请求到恢复分布、实际漏帧率 |

正确性先覆盖原退出条件、值与别名、移动后地址、重复／遗漏迭代；初版不接受的 EH／跳转／调用模式确认仍走原路径。少量优化专测检查向量指令、site 唯一性及根位置，普通 fixture 仍只锁定语言结果。GNU／musl 和真实 compiled stackmap 的移动测试通过之前，保留为实验候选。

## 6. 标量实现与第一轮验证

在 `417e661f2` 后实现 MIR 普通优化结束时的一次分块。识别已有显式 poll header、唯一 body／回边与独立初始化前驱；Int／Long 从非负常量开始，稳定上界为形参或整数常量，唯一更新为加一，条件为严格小于。支持 core 运算展开产生的局部副本及 compareTo 形态。body 只接受整数／布尔局部表达式；数组 getter／setter、调用、EH、非局部访问、取地址和复杂跳转仍保留原循环。已知最多 64 次的循环不转换。

原 header 保持其 poll 身份；新 CFG 计算 `min(64, n-i)`，正倒计数控制内层回边。新 counter 使用 caller 的正常 synthetic local identity，不增加 MIR／LIR wire 字段。内层无需 poll 的依据是实际倒计数 CFG；没有 LLVM metadata 或函数 effect 伪装。

本机 fmt／workspace clippy、25 项 MIR 优化单测、18 项 statepoint 专测通过。两个新源码 fixture 共五变体、九次程序运行，覆盖 Int／Long、负／零上界、1／15／63／64／65／129／1025 次、Long 最大值附近、回绕算术、break／continue、普通／moving／minor 并发 GC，以及 class／interface／tuple／Option 引用跨循环存活。并发测试使用 32 次真实收集，避免测试 collector 无间隔持续请求造成无关的 mutator 饥饿。

标量 64 相对逐轮 poll 的 checksum 程序约快 38%，递推程序约快 4%；text 增加 64 bytes，编译时间约增加 0.5%。stop-wait p99 基本不变，但单线程出现一次 1.99 ms 最大值异常，不能据此宣称尾延迟完全不回退。详见 [性能记录第 11 节](PERFORMANCE.md#11-外层-poll标量有界内层)。块大小选择、受限 SIMD 和 Linux 验收继续独立推进。

## 7. 受限整数内层 SIMD 与默认块大小

在标量提交 `219a1a958` 之后，把完整块与尾部标量块分开：剩余量至少为 64 时进入固定 64 次的 body 副本，否则执行剩余次数；两个内层都回到原 poll header。完整块为自身的 ValDecl 分配新的 caller local identity，循环携带的源变量继续共享；若 body 内临时值在外部使用，则不转换。该形式用少量代码复制让 LLVM 端无需再次求解 `n-i` 的全套范围关系。

LLVM 普通标量阶段后检查真实 CFG：单块自循环、计数 phi 唯一外来初值是 1～64 常量、唯一回边减一并在零退出、正文全部为整数 SSA 运算。call／invoke、内存读写、原子、浮点、未知次数均排除。仅对通过检查的 loop 设置向量化选择；只追加 loop-simplify、LCSSA、`loop-vectorize<interleave-forced-only;vectorize-forced-only>`、InstCombine／ADCE。外层、尾部与其他 Managed 循环没有选择标记；不运行完整 NoGc 管线、unroll、unswitch 或 LICM。metadata 是这次结构判断的输出，不是资格证明本身。相关 LLVM 控制语义见 [LoopVectorizeOptions](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/include/llvm/Transforms/Vectorize/LoopVectorize.h)。

资格允许尝试，不保证向量化发生。源码递推循环暴露出 LLVM missed-optimization remark 会污染现有 child 协议 stderr；局部 pass 期间忽略 remark，结束后恢复原 handler，其他诊断沿原路径保留。源码 fixture 验证无法向量化时仍正常编译、运行，不把它变为新的语言限制。

本机 fmt／workspace clippy 通过；25 项 MIR 优化与 20 项 statepoint 专测通过，追加了 body 副本 local 身份检查、三个目标的向量指令／两条唯一机器 stackmap／AS1 根恢复、变量上限与原子路径的排除，以及同函数 invoke／异常出口组合。源码覆盖补充 checksum、循环后的真实异常，并与先前并发 moving／minor 根用例共同通过。Linux 真实运行仍在最终跨平台验收中执行。

同一轮五策略对照中，64 次 SIMD 相对逐轮 poll 的 checksum 总耗时约 −48%，相对标量 64 再减约 14%；递推程序没有稳定加速，因此不沿用首轮的小幅收益作通用结论。256 次只比 64 再快约 5.5%，四 worker stop-wait p99／max 为 70／309 μs，64 为 64／139 μs；保留较小的 64。text 相对逐轮增加 212 bytes，编译时间无明显变化。首轮 1.99 ms outlier 在本轮未复现，但原样保留；结果不是墙钟暂停上界。完整方法与数据见 [性能记录第 12 节](PERFORMANCE.md#12-整数内层-simd-与块大小选择)。临时 16／256 编译器修改已逐字恢复，最终代码没有实验开关。

# f-string 就地展开与 StringBuilder 后续优化

状态：短 f-string 已实现，三目标验证和实际编译器对照见第 6.1 节；本轮仅交付这一阶段，第 7 节 StringBuilder 字节缓冲留待后续。2026-10-27 对照基线 `d90538338`；第 5 节保留实现前的可行性探针，不能与正式验收数据混用。

## 1. 结论与实施范围

先实施少量片段的 f-string 就地展开。这项收益不依赖 StringBuilder 改成字节缓冲，可以独立完成、测量和交付。

- HIR 脱糖按源码顺序求值每个插值表达式，立即完成该值的 ToString 转换，把 String 结果保存到不可变临时变量。
- 不超过八个有效片段时，在这些转换之后生成已有的 typed String 拼接链。release 复用现有 MIR 融合器，将连续单用链降为一个 `StringConcat`，最后一次分配结果。
- 超过八段时继续生成当前真实 core StringBuilder 的构造、append、toString 调用；不改变其存储或 runtime 后备。
- 整数、Char 等的临时 String 暂时保留。直接将标量格式化进最终结果属于后续工作。

首版选择已有 HIR `PrimitiveBinaryKind::StringConcat`，不新增 HIR 多元拼接节点、插值 IR、core intrinsic 或产物格式。两个动态片段的冷调用点可能继续使用已有二元 runtime concat；它同样只分配一个结果。debug 采用相同的源码脱糖，但保留现有二元后备，三个及以上片段不承诺融合或一次分配。这是生成策略的区别，不改变语言语义。

如果以后要求所有 profile 的多段拼接都直接生成单次分配，再单独考虑 HIR 多元 StringConcat 及其可移植正文表示；本阶段无需承担该格式变更。

## 2. 实施前已核实的实现基础

| 位置 | 基线行为及对设计的影响 |
| --- | --- |
| [HIR 插值下降](../../compiler/hir-lower/src/expr/interpolation.rs) | 无论片段数，先构造隐藏的 core StringBuilder，再逐段调用 append，最后调用 toString。绑定来自实际 core 声明，保留源位置。 |
| [core StringBuilder](../../sysroot/src/scoop.core/src/string_builder.scoop) | 持有 `ArrayList<String>`；generic append 立即转换；toString 调用 `coreStringJoinParts`。 |
| [parts 后备](../../runtime/src/string_parts.c) | 多片段时已是 checked 求和、一次结果分配及按序复制；单片段直接返回原 String。因此 f-string 本轮主要消除 builder、parts 存储与追加调用，不应将收益描述为把原来的多次结果拼接改成一次。 |
| [HIR 拼接操作](../../compiler/hir/src/body.rs)、[MIR 下降](../../compiler/mir-lower/src/body/operators/primitive.rs) | 已有 typed 二元 StringConcat，不必从普通方法名或 extern 符号猜测操作。 |
| [MIR 拼接融合](../../compiler/mir-lower/src/optimization/strings.rs) | 最多八段；同块、稳定局部值、单用中间结果；其他调用和可变写入会结束融合。两次及以上连续 concat 可以融合；单个二元 concat 只对热调用点或全短常量展开。 |
| [StringConcat codegen](../../compiler/codegen/src/function/instruction/strings.rs) | 已实现长度检查、一次分配、GC 后重读输入和精确复制；短常量直接写入结果。 |
| [可移植运算表示](../../compiler/hir/src/cross_cone_interface/default_templates/body/operators.rs) | 已保存 StringConcat primitive；现有 generic/default/普通优化正文通道可复用该操作。仍需验证此次生成的完整正文可以导出和消费。 |

不能把 f-string 改写成一边转换、一边执行 `result = result + part`：ToString 调用会隔断现有融合分析，还会产生逐步增长的中间 String。也不需要先生成 builder，再期待普通内联和逃逸分析完整识别它。

## 3. f-string 脱糖

### 3.1 片段选择与展开

有效片段指非空文本段及每个 `${expression}` 段。移除空文本段；相邻短文本可以按既有不超过 64 bytes 的规则合并。每个表达式仍算一段，不根据其运行期是否返回空串决定路径，也不为计算长度提前调用 ToString。长文本仍是一段，超过短常量阈值时引用原 String 常量。

八段是当前局部展开的代码体积选择，不是 f-string 合法性或输出长度限制。九段及以上始终有普通 builder 路径。两条路径使用相同的类型、名称绑定与求值规则；以后调整阈值须同时核对现有融合上限。

例如：

```scoop
val text = f"<${makeLeft()}:${makeRight()}>"
```

展开的执行顺序等价于：

```scoop
val first = makeLeft().toString()
val second = makeRight().toString()
val text = "<" + first + ":" + second + ">"
```

上述源码仅说明顺序。实际 HIR 使用 hygienic locals、已绑定的 ToString callable 和 typed StringConcat，不重新执行用户作用域中的 `.toString` 或 `+` 名称查找。所有转换均在拼接链之前完成，链内只读取 String 常量与已经保存的结果，中间 concat 结果只供下一个 concat 使用。

多片段在全部转换完成后，再把转换结果引用保存到拼接所在块的不可变局部变量。协程恢复可能在多条路径写回先前保存的转换局部；这组最后的引用快照使拼接输入仍是单次定义，从而复用已有融合条件。快照不调用 ToString、不分配 String，也不保存 bytes 指针；普通直线正文中的冗余复制由既有优化消除。

| 有效片段数 | 脱糖与结果 |
| --- | --- |
| 0 | 返回空 String；仍先完成 f-string 自身的合法性检查。 |
| 1，纯文本 | 使用文本 String。 |
| 1，表达式 | 求值并完成一次 ToString 转换，直接使用转换结果。 |
| 2～8 | 先完成按序转换，再发射已有二元 typed 拼接链；release 达成一次最终结果分配。 |
| 9 及以上 | 沿现有 StringBuilder 脱糖，不分组创建额外 builder 或中间大 String。 |

不增加空串拼接来诱导融合，不无条件扩大普通二元拼接的展开范围，不给 StringBuilder 或用户的同名类添加编译器特殊表示。

### 3.2 ToString 绑定与原语义

转换必须复用普通 conformance、bound-member 和 implementation 选择，绑定真实 core `ToString` 的成员。保留输入的完整静态类型与实际 callable 身份；不先转换为 `Any` 或为方便调用而统一装箱成 `ToString`。

- 用户类型的表达式恰好求值一次，其 ToString 恰好调用一次；前一转换完成后才开始下一表达式。String 的转换为自身，可由现有优化消除无作用的调用。
- 只有同名 `toString` 方法而没有实际 conformance 的类型仍报错；静态 `Any`、无 bound 的泛型、Unit 等沿当前规则检查。用户的同名 StringBuilder、ToString、extension、alias 和 import 不替换脱糖目标。
- 每次转换后立即保存返回的 String。不能延迟读取一个可变 String 变量，也不能先收集所有原始值再统一转换；后续表达式可能修改它们。
- 任一表达式或转换抛出时，后续表达式、转换和最终拼接都不执行。已产生的用户副作用保留。
- `${...}` 的挂起合法性保持。已转换的 String 按普通局部变量保存在协程 frame 中，恢复后继续下一段；不跨挂起保存内部 bytes 指针。
- 嵌套 f-string、函数实参、tuple/array 元素及默认值中的 setup 必须留在原求值位置，不能被 hoist 到前一个实参之前或另一控制流分支。
- raw、多行、转义、Unicode 和 U+0000 规则保持；即使零段或纯文本，f-string 仍不是 `const val` 常量表达式。NoGC、unsafe 和 suspend 合法性不因结果可化简而放宽。

删除隐式 builder 会改变临时分配次数、GC 次数和资源耗尽时点；这些不是需保留的用户求值事件。保留真实的长度溢出与分配边界，不能把 fatal allocation failure 改成可捕获的格式化异常。

### 3.3 阶段边界与生成要求

HIR lower 负责语义检查、转换调用和临时变量的顺序，输出已经完整绑定的普通 typed 正文。MIR lower 只消费 HIR，通过已有 primitive 下降及 release 融合产生 StringConcat。LIR/codegen 复用现有目标 TD、根计划和复制生成。

不让 HIR lower 调用 MIR lower 的实现，也不按源文本、FQN 或 runtime 符号重新识别。generic/default/跨 Cone 正文保存相同的局部变量、调用身份、String 常量 owner 和 primitive 操作，不增加平行序列化通道。

拼接执行前发生的任何分配、用户调用或挂起，都必须保活此前已得到的 String；最终结果分配期间也要保活全部输入。复制地址在分配后从重定位引用重新计算。复用现有 codegen 的这个合同，不新增 pin、borrow callback 或手写 payload 偏移。

验收应检查正常 release 下降的 2～8 段最终只有一次结果分配，尤其覆盖最后一个表达式包含分支、异常或挂起的情形。若 setup 的组织阻碍连续拼接，应修复具体的控制流组织问题，不通过放宽所有调用的融合屏障来绕过。

## 4. 第一阶段的 spec 与实现清单

实现前先同步以下规范。第一阶段不需要改变 runtime ABI、StringBuilder 布局或 `.slib` 容器与 HIR 正文格式。

| 文档 | 修改内容 |
| --- | --- |
| [语言规范 §6、§6.2](../specs/SCOOP-SPEC.md#6-字符串插值) | 将 builder 调用序列作为说明插值行为的基准，不再规定必须创建 builder；明确允许保持逐段求值/转换顺序的等价拼接展开。保留 ToString bound、hygiene、异常、挂起、const 与源位置规则。八段阈值放在实现规范。 |
| [语言规范 §11.6](../specs/SCOOP-SPEC.md#116-stringbuilder) | 将“字符串插值的脱糖目标”改为通用字符串累积 API 及插值的后备实现；本阶段其 API、parts 存储、clear 和快照合同保持当前定义。 |
| [实现规范 §2.15](../specs/SCOOP-IMPL-SPEC.md#215-集合char-与字符串) | 补充短 f-string 的临时变量与拼接链下降、片段计数、超过上限的后备、release/debug 差异；修订“该局部融合不涉及插值”及“f-string 一律绑定 builder”的描述。保留普通 `+` 的现有融合限制。 |
| [实现规范 §2.19](../specs/SCOOP-IMPL-SPEC.md#219-优化与物理定义选择) | 如列举生成来源或 profile 范围，同步说明插值复用原拼接融合；不新增优化开关。 |
| [运行时规范 §6](../specs/SCOOP-RUNTIME-SPEC.md#6-核心类型的运行时后备) | 现有 concat 的分配/重定位合同已经适用，本阶段无需改 ABI 或删除 join_parts。只有新增交叉引用需要同步文字。 |
| [路线图](../ROADMAP.md)、[性能进展](PROGRESS.md) | 实施验收后记录短 f-string 能力与实际测量；既有 M26 和 STRING-CONCAT 记录是历史成果，不改写成当时已经支持插值。 |

主要实现入口为 `compiler/hir-lower/src/expr/interpolation.rs`。先复用现有转换绑定、临时局部变量及 typed primitive 生成工具；MIR/LIR/codegen 以验证既有路径为主，不为此次优化新增扫描、格式化或容器算法。

## 5. 本机可行性探针

2026-10-27 使用同一个现有 release 编译器和 sysroot，对照当前 f-string 与手写的等价展开。两种五片段形式为 `f"<${left}:${right}>"` 和 `f"[${value}:${index}]"`；后一项每次更新 Long 输入并转换 Int 索引，两边都保留相同的数值 ToString。结果字节长度进入输出校验和，输入与最终打印在计时区间外。

命令版本为 `scoop/scoopc 0.0.0-dev (eb81be3540657142e584cedfcee7c2834bae220b)`，宿主为 Darwin ARM64。这不是当前工作区编译器补丁的测试：未重新构建编译器，也未替换 sysroot。源码先经 `scoop fmt` 和 `fmt --check`；复用 [phase 计时 helper](../../tests/benchmarks/interfaces/phases.c)，以 `-O2 -Wall -Wextra -Werror` 编译。

每个模式预热一次，随后五对交替 AB/BA、每次一百万轮；GC 统计另以十万轮运行。每一对输出校验和一致。

| 五片段负载 | 当前 f-string，中位 ns/次 | 等价展开，中位 ns/次 | 十万次 allocated_bytes，当前 → 展开 | minor 次数 |
| --- | ---: | ---: | ---: | ---: |
| 已有 String 加文本 | 110.865 | 10.883 | 21,600,176 → 4,800,176 | 20 → 4 |
| Long、Int 加文本 | 149.862 | 48.561 | 30,397,840 → 13,597,840 | 29 → 13 |

两种展开的最终 MIR 都有一个 StringConcat，拼接处没有二元 runtime concat；当前 f-string 的 MIR 仍有 StringBuilder 分配及 ArrayList 访问。两种负载每十万次均减少 16,800,000 B，即本探针每次减少 168 B；整数行仍包含两个转换结果 String 的分配。这直接支持先消除 builder 路径的判断。

数据只证明这个展开方向在两个本机微负载上可行；没有完成新前端实现、三目标对照、全部语义/GC 验收、代码体积或编译成本评估。尤其不能把上述比值视为任意 f-string 或完整应用的承诺。

探针源码与构建位于仓库忽略的 `tmp/program-performance/f-string-desugar-yl0lpnxg/`；原始结果和四段 MIR 位于仓库外 `/var/folders/6k/02kz7z0s7nxc1m0gg3xj99c40000gn/T/scoop-f-string-research-qh44wgsq/`。这些临时目录仅是本次调查记录，不作为正式验收依赖。

## 6. 第一阶段验证与交付

普通行为放入 core 的独立 [consumer 测试 Cone](../../sysroot/src/scoop.core/consumer-tests/Cone.toml)，默认 release；编译错误、IR/ABI/产物与定向 GC 才使用相应 fixture。现有插值行为 fixture 可作为复用入口，不继续把普通行为复制成更多 fixture。

| 验证层 | 必须覆盖 |
| --- | --- |
| 普通行为 | 空、纯文本、单表达式；2/3/7/8/9 段；空转换结果、长文本、Unicode、NUL、raw/多行和嵌套；generic 与接口输入；重复 String；后续表达式改写先前读取的变量。 |
| 顺序与异常 | 明确断言 `E1,T1,E2,T2`；表达式失败和转换失败的副作用前缀；前一段抛出则后一段不执行；用户 ToString 内分配、GC 或修改普通对象。 |
| 语言诊断 | 复用 interpolation/negative 的真实 ToString bound、fake-toString、Any、Unit、无 bound 泛型、const、NoGC、非法 suspend；断言源位置和必要消息。 |
| 编译器正确性 | 少量代表例检查 HIR 无隐式 builder、MIR 的 release 单次 StringConcat、两段冷点的一次二元后备、9 段 builder；debug 正常运行而不要求融合。短常量/长常量的表示及原 owner 保持。 |
| 跨 Cone | 实际 provider 的 generic/default/普通可移植正文进入 consumer；遮蔽和 alias 不替换 core 目标；预编译 release core 与必要的 debug/release consumer 组合。输入源码和工具保持存在。 |
| 定向 GC/协程 | 早期 String 在后续转换的分配/GC 后有效；所有输入在最终拼接分配后正确重定位；跨挂起/恢复/异常的 String 局部保活；小对象、较大结果及 minor/moving。仅这些用例开启 stress。 |

优先复用 [order](../../tests/fixtures/language/interpolation/order/program.scoop)、[aggregate-order](../../tests/fixtures/language/interpolation/aggregate-order/program.scoop)、[suspend](../../tests/fixtures/language/interpolation/suspend/program.scoop) 与 [concat 编译器专测](../../tests/fixtures/compiler-correctness/strings/concat/fixture.toml) 的已有场景，补齐这次改变的行为和结构断言，不保存普通功能的完整阶段快照。

性能验收使用同一输入、配套工具与系统库，对照真正修改前后的 f-string；手写展开只作为上限参考。覆盖冷点/热循环，1/2/5/8/9 段，String、整数、Char、自定义 ToString 和长文本。分别报告墙钟、GC 分配、minor/full、text/完整文件与冷 consumer 编译成本；计时与 GC 统计分开。保持九段后备及普通 String `+` 的对照，确认没有把成本转移到它们。

实施顺序为：同步 spec → HIR 脱糖与相关测试 → Rust 格式化/Clippy → 定向语义、产物及 GC 检查 → 三目标性能和成本对照。修改 Python inspector 时先运行仓库规定的 Ruff。正式源码编译、产物消费、链接与运行闭环通过后交付本阶段，不等待 StringBuilder 重构或整数融合。

### 6.1 实施验收（2026-10-27）

实现限于 HIR lower，沿真实 core ToString slot 绑定转换，保留具体 receiver、泛型 bound 和动态 override。已同步语言规范 §6／§6.2／§11.6 与实现规范 §2.15；MIR／LIR／codegen、runtime、StringBuilder 和产物格式未改动。先独立修复 HIR 测试 core 中已失效的浮点 helper 引用，再通过 2,064 项 HIR 单元测试；最终补齐引用快照后，两项专门的 HIR 回归再次通过。Rust fmt／workspace Clippy、Scoop 格式检查及 Python Ruff 通过。

Darwin ARM64、Linux GNU x86_64、Linux musl x86_64 均通过 29 项 interpolation fixture（含诊断、求值／转换／异常顺序、遮蔽、跨 Cone generic/default 和挂起），两项 core consumer 行为测试，以及三个字符串编译器专测的六个 profile 变体。新增专测精确断言短链、含分支的最后一段、普通挂起和默认参数挂起在 release 各生成一个 StringConcat；debug 保留相应二元后备，九段保留一个 builder。release 专测使用 moving／minor stress，另复用现有挂起异常用例检查跨恢复后的保活。没有运行全量测试套件。

正式对照使用同一份 [benchmark](../../tests/benchmarks/strings/README.md)，修改前工具构建于 `3600e70f0`（生产代码与 `d90538338` 相同），修改后使用本阶段实现。每目标两边消费同一份匹配 ABI 的预编译 sysroot；Linux 在独立 checkout 准备系统库，未改动原工作区。十个模式分别预热一次，再运行七对 AB／BA、每次一百万轮；逐对校验 stdout。Linux 固定 CPU 4；计时与十万轮 GC 统计分开。

| 负载 | Darwin，ns/次，前 → 后 | GNU，ns/次，前 → 后 | musl，ns/次，前 → 后 |
| --- | ---: | ---: | ---: |
| 单个 String | 66.036 → 0.187 | 64.423 → 0.136 | 84.551 → 0.159 |
| 两个 String | 102.105 → 11.325 | 92.981 → 9.604 | 121.863 → 25.890 |
| 五段 String／文本 | 117.642 → 11.442 | 119.061 → 10.342 | 251.650 → 74.056 |
| 八个 String | 136.682 → 18.037 | 146.837 → 22.796 | 360.487 → 225.963 |
| 九个 String，builder 后备 | 193.411 → 194.469 | 192.376 → 187.953 | 454.935 → 451.268 |
| Long／Int 与文本 | 155.242 → 50.554 | 158.578 → 47.323 | 342.649 → 158.369 |
| Char 与文本 | 132.818 → 12.581 | 142.814 → 16.937 | 295.673 → 45.926 |
| 自定义 ToString／Int | 135.194 → 24.915 | 140.130 → 24.695 | 294.716 → 117.068 |
| 长文本 | 106.440 → 9.222 | 107.691 → 12.377 | 178.560 → 54.420 |
| 普通 String `+` 对照 | 11.665 → 11.462 | 10.548 → 10.577 | 74.075 → 74.127 |

这些短插值负载每次均减少 168 B 临时分配，三目标一致。五段 String 的十万轮 allocated_bytes 为 21,600,176 → 4,800,176，minor 为 20 → 4；整数为 30,397,840 → 13,597,840、29 → 13，仍保留数值转换 String。单段只有固定 176 B 启动分配，minor 16 → 0；八段 minor 25 → 9。九段和普通 `+` 的分配及 minor 保持，全部模式 full 为零。

单段返回已有 String 后循环可继续简化，不能将亚纳秒结果解释为实际调用成本；其他数值也只是这些微负载的阶段时间。初次 GNU 未绑定 CPU 的普通 `+` 对照回退约 8.8%，固定 CPU 后为 +0.3%；musl 与构建测量重叠的一轮已单独保留，表中采用无并发构建的复测。Darwin 九段 +0.5% 与 Linux 对照的微小变化均保留，不据此声称后备或普通拼接得到优化。最终引用快照补丁生成的三目标基准程序与已测候选逐字节相同，因此复用运行数据；编译成本另用最终编译器重测。

完整文件与代码段均缩小；冷 consumer 构建以每边三个全新 cache／target 目录交替测量，保留全部源码和工具。具体成本与原始记录位置见[性能进展](PROGRESS.md#b1短-f-string-就地展开)。本阶段已清理本地不再使用的 dev target（约 1.7 GiB），保留 release 工具与需要复用的缓存。

## 7. 后续 StringBuilder 设计结论

字节缓冲方向也合适，但作为独立第二阶段。以下结论用于接续实施，不成为第一阶段交付条件。

### 7.1 存储、容量与公开行为

采用普通 `MutableArray<UInt8>` 和 `byteCount: Long`，数组 size 表示容量。只有 `[0, byteCount)` 是有效 UTF-8；其后可有旧内容，不能整体当作 String。有效前缀只由合法 String、Char 编码、标量格式化及其连接产生，toString 不重复校验 UTF-8。

容量按倍数增长，至少满足实际 required。checked 累计 required；倍增接近表示上界时改用实际 required，沿现有数组/String 分配边界处理失败，不引入任意大小预算。扩容完成后只复制旧的有效前缀，再替换 backing；不复制未使用容量。普通数组构造仍产生合法 UInt8，不新增公开未初始化数组。

建议支持 `initialCapacity: Long` 的构造参数并保留零参数调用，语义是初始字节容量；负数抛 IllegalArgumentException，零合法。具体默认容量、首次增长量为库实现取舍，实施时通过短 builder 与复用负载选择，不承诺公开的精确增长序列。

`clear()` 只将 byteCount 置零、保留容量并返回自身，O(1)。UInt8 不含引用，不需要像 ArrayList<String> 一样逐槽清除引用。toString 复制有效前缀到独立 String，不转移或共享可变 backing；非空结果一次分配、一次复制，空串可以复用，快照身份不作额外保证。

generic append 必须先执行 `val text = value.toString()`，然后再读取 builder 当前长度/存储并追加。用户转换期间对同一个 builder 的 append、clear、扩容按调用顺序生效；转换失败保留其重入副作用，不追加外层结果。现有 [reentry 测试](../../sysroot/src/scoop.core/consumer-tests/tests/string-builder/reentry/program.scoop) 是这项合同的直接回归基础。

### 7.2 typed 复制与 GC

`append(String)` 需要无回调的 String → 已分配 MutableArray<UInt8> 字节复制。扩容需要旧数组有效前缀 → 新数组，toString 需要数组有效前缀 → 新 String；每项都有实际消费者，不建立通用 sink/allocator/plugin 框架。

现有 [coreStringCopyUtf8Unchecked](../../sysroot/src/scoop.core/src/string_copies.scoop) 的参数是 `Ptr<Byte>, Long`，其分配期间要求来源地址稳定。不能直接从可移动的 MutableArray 取一个裸地址再调用它。现有 StringCopy 的 `FromBytes` 也只接受完整的 exact Array<Byte>，不能把 MutableArray<UInt8> 的容量误当作有效字节数。

因此需要扩展既有 typed StringCopy 的托管数组前缀来源，显式携带实际数组类型与有效长度；复用分配/复制发射，在结果分配后从更新的根取得源数组地址。源与目标都为已分配对象的复制，先完成实参求值，在无 safepoint 的复制区间内取得两侧 payload 地址并完成 memcpy，不保留地址。写目标数组的操作必须登记真实写副作用，不能因它不分配就被当作纯操作删除或跨写入移动。

这些内部操作不走 `withDataPointer`/`withUtf8Bytes` callback，不引入每次 append 的闭包或 pin 成本；也不放宽源码 NoGC 对 managed 引用的限制。UInt8 payload 无引用屏障，builder 替换 backing 字段仍遵守普通引用写入合同。必要的范围与容量前提在建立它们的边界完成，不重复扫描已合法的 UTF-8。

### 7.3 标量重载的首版选择

添加 Long、Int、ULong、UInt、Char、Boolean 六个普通 append 重载，保留 generic append 和显式泛型调用。不能把任意 ToString 输入按运行期类型改写成这些重载，也不引入隐式整数转换。

这里建议对“直接写入”作一次收敛：

- Char 复用 `coreUtf8Word`/`coreUtf8Width`，预留实际 1～4 字节后写入；Boolean 复用固定的 `true`/`false` 内容，不分配临时 String。
- 整数首版复用 [现有十进制 writer](../../sysroot/src/scoop.core/src/integer_decimal.scoop)，格式化一次到最多 20 B 的 GC-free 栈缓冲，再通过无回调 typed 复制追加有效区间。这消除了临时 String，但仍有一次很小的复制，应如实区分于完全直接写入 builder。
- 当前 writer 反向写入 `Ptr<Byte>`，且是普通 managed helper；不能将未固定的数组内部地址直接传给它，也不能假定“最多 20 B”就保证调用链没有 poll。为消除这一次小复制新建借用机制、复制一套整数算法或把格式化迁入 codegen，都不值得作为首版前提。
- 有符号幅度仍在无符号域计算，保留 Long 最小值、ULong 最大值、零及十进制位数边界。Int/UInt 经已有精确转换复用对应 writer。

Float/Double 先走 generic ToString。当前 [floating_text.scoop](../../sysroot/src/scoop.core/src/floating_text.scoop) 已将 Ryu 输出转换成 Scoop 的小数/科学计数法格式；后续接入 builder 应复用这层规则，不能把 Ryu 原始文本直接作为语言结果。

### 7.4 代价、spec 与退役范围

字节缓冲提高碎片追加与 clear 复用效率，减少保留的 String 引用和 GC 扫描；但已有大 String 需要先复制到 builder，再复制到最终结果，扩容还会复制有效前缀。旧 parts 实现的单片段快照能直接返回原 String，新方案会增加复制。clear 后也会保留历史高水位字节容量。大段 String、单片段、重复快照和大容量低占用必须单独测量，不能宣称全场景更快。

第二阶段实现前同步语言规范 §11.6 的重载、capacity、有效 UTF-8 前缀、clear 与 O(byteCount) 快照复杂度；§11.16 StringWriter 继续复用 builder，无需改 TextWriter 接口。实现规范 §2.10/§2.15 补齐必要复制 intrinsic 的 typed 合同及 wire 项，删除 StringBuilder 必须持有 ArrayList 的描述。运行时规范 §2.4/§6 删除 StringBuilder 复用 parts 容量扫描与 join_parts 的条款，并补充托管前缀复制的重定位规则。StringWriter 行为与用户已返回快照保持。

生产消费者完成迁移后，删除 `coreStringJoinParts`、`runtime/src/string_parts.c`、公开 C 声明和 [runtime 构建清单](../../compiler/toolchain/src/runtime/sources.rs) 的对应项；ArrayList 的内部 backing getter 当前只有 builder 使用，届时一并删除。保留仍有消费者的二元 `scoop_rt_string_concat`。更新 [Runtime 职责收敛](RUNTIME-RESPONSIBILITIES.md) 的后置事项说明。

builder 私有布局、构造/重载与复制 intrinsic 的改变需要配套 core 和依赖 Cone 正常重建；删除 join_parts 属于实际 runtime ABI 入口删除，应更新既有 ABI contract。String/数组对象布局与 metadata ABI 无需随之改变。不保留仅服务旧测试的适配层，不通过删除或隐藏源码/工具验收产物消费，也不引入缓存内容复核。

## 8. 再后续的标量插值融合

第一阶段先消除 builder，第二阶段改善手写 builder 及长插值。只有它们完成并测量后，再考虑把整数/Char 插值直接写入最终 String。

这时仍需先解决各片段长度与一次结果分配的配合，并保留原表达式顺序。可复用普通 core 的有界格式化结果和实际长度，避免一次测长再完整格式化第二次。只优化已静态确定的真实 core 标量实现；用户 ToString、generic bound 或动态接口输入继续走原转换，不能跳过用户代码。不得把“少分配临时 String”扩成另一套后端格式化算法或通用格式 DSL。

# Scoop 实现路线图

版本：0.1（草案）

配套文档：`docs/specs/SCOOP-IMPL-SPEC.md`（pipeline 与各 stage 职责）、`AGENTS.md`（编码准则）。

## 1. 策略：纵向主线 + 显式语言子集

不采用"逐 stage 完整实现"的横向推进，而是先打通最小端到端主线，再逐里程碑扩大语言子集。理由：

- **风险前置**：statepoint/stackmap、landingpad、GC pin/handle、suspend 状态机是本项目的新链路，越晚碰代价越大；
- **IR 设计需要下游反馈**：每个 stage 输出的真实需求由其消费者发现，孤立地"完整"实现某个 stage 几乎必然返工；
- **测试基建一次到位**：fixture runner、golden dump、negative fixture 断言在第一个里程碑建好，后续特性直接落入现成框架。

与 AGENTS.md"不留占位符"准则的调和：每个里程碑定义一个**显式的语言子集**，子集内的规则完整实现（结构完备、无 TODO 分支）；子集外的语法由 parser/HIR 报**正式的不支持诊断**（有位置、有信息）——这是面向用户的错误报告，不是代码中的占位分支。随着里程碑推进，这些诊断逐一消除。

## 2. 里程碑

每个里程碑都是全链路可运行的（parser → HIR → MIR → LIR → codegen → 可执行文件）。

### M0 技术 spike ✅（2026-08-21 完成）

- ~~独立一次性程序验证 inkwell 的 statepoint + stackmap + landingpad 全链路（不进主线代码）~~——`spikes/llvm-gc/`（独立 workspace），inkwell 0.10 + LLVM 22.1.8 验证通过：`rewrite-statepoints-for-gc` 正常改写、`.o` 含 `__llvm_stackmaps` 与 `gcc_except_tab`。该历史spike把pointer发射为address space 0，只证明metadata/EH通道打通，没有验证AS1 root、`gc.relocate`或root machine location；这些契约由M15的专用LLVM 22.1 qualification与artifact测试建立；
- ~~建立 fixture runner、golden dump 设施与 CLI 骨架~~——`compiler/driver/tests/fixtures.rs`（insta 快照），`scoopc` CLI 拆为 lib + 薄 bin（clap），冒烟 fixture `tests/fixtures/m0-smoke/hello.scoop` 端到端通过。

### M1 hello world ✅（2026-08-22 完成，设计见 `docs/milestone1/DESIGN.md`）

顶层函数、`String` 字面量、`fun main`、调用 runtime 的 print。GC 用 always-leak 实现（分配即 malloc、不回收）；单 Cone；不插 statepoint。

### M2 值类型基础 ✅（2026-08-22 完成，设计见 `docs/milestone2/DESIGN.md`）

struct / tuple、字段访问、`val` / `var`、if / while、结构相等。

### M3 泛型与 Option ✅（2026-08-22 完成，设计见 `docs/milestone3/DESIGN.md`）

单态化、`Option<T>`、`T?`脱糖与`?.` / `?:` / `!!`（spec第7章）。M3首版将Option作为编译器内建，M4已迁移为`scoop.core`的真正泛型enum；M13边界修订后，export generic template、HIR生成的local concrete实例与MIR实体使用三类独立typed id，MIR不再直接读取template完成单态化。

### M4 enum 与模式匹配 ✅（2026-08-22 完成，设计见 `docs/milestone4/DESIGN.md`）

enum变体、when扩展模式、守卫、穷尽性、解构声明与`..`（spec第4、5章）。同时建立了sysroot框架（`sysroot/lib/scoop.core`），`Option`与`print`/`println`的硬编码定义正式迁移入core库；M13将tagged enum的扫描表示修订为pure-value共享payload、ref-bearing独占slot及不读取tag的固定ref偏移，可递归嵌入struct / tuple / class。

### M5 数组 ✅（2026-08-27 完成，设计见 `docs/milestone5/DESIGN.md`）

`Array<T>` / `MutableArray<T>`（M5暂为编译器内建，M14迁移为core generic intrinsic class）、字面量与推导规则、下标读写、`size`、构造函数形式互转（memcpy 快照）；越界 trap（M8 改异常）。数组 TD 携带递归元素扫描，支持含引用的 struct / tuple 与 tagged enum 内联元素。

### M6 引用类型层级 ✅（2026-08-28 完成，设计见 `docs/milestone6/DESIGN.md`）

class / 继承 / interface / 方法、vtable / itable 分派、装箱（spec 3、4.4、9.1；impl spec 2.9）。方法级 final / open / abstract 语义完整落地：类开放不隐式开放成员，final 调用 direct，open / abstract 调用 virtual，final override 保留继承槽。落地后顺带解锁：数组的 `toArray` / `toMutableArray` 方法形式、core 的 `class StringBuilder` 声明、`add<T>` 依赖的 `toString` 分发基础。

### M7 函数重载 ✅（2026-08-28 完成，设计见 `docs/milestone7/DESIGN.md`）

顶层函数与方法的 overload resolution（候选集分层 + 可应用性 + MSC，按 Kotlin 规范）；`print` / `println` 已迁移为 `scoop.core` 的普通重载定义。M7 首版由三个 `@Intrinsic` 原语支撑，M12 已把 `write` 迁为 Scoop ABI extern，只保留临时的值到字符串转换 intrinsic。

### M8 异常 ✅（2026-08-30 完成，设计见 `docs/milestone8/DESIGN.md`）

try / catch / finally / throw，landingpad 落地（runtime spec 第 5 章）。四条 trap 路径已全部改接真实异常：`!!` → `UnwrapException`、数组越界 → `IndexOutOfBoundsException`、`as` → `ClassCastException`、整数除零 → `ArithmeticException`（spec 11.7 已同步新增后者）。`scoop_rt_throw` 按 `__cxa_allocate_exception` + 拷贝的 ABI 正确形态实现。

### M9 真 GC ✅（2026-08-30 完成，设计见 `docs/milestone9/DESIGN.md`）

真 GC 替换 always-leak：Immix 核心（32KB block / 128B line、bump 分配、free-line 复用、标记-区域回收，1 GiB mmap arena）；statepoint 打开（GC strategy + `rewrite-statepoints-for-gc` + safepoint poll + stackmap，runtime v1 暂用经对象起点校验的保守栈扫描）；对象头扩为 16B（td + gc_word）；递归 TD 扫描描述；pin（对象头标志位）与 GcHandle 表落地；`scoop.core.gc` 包；写屏障卡片表（预偏置指针，为分代预留）；M1–M8 全部 fixture 在真 GC 下原样通过。M12 已把公开 pin/handle API 迁为 `@Unsafe` 普通 core 函数，仅保留 raw runtime boundary intrinsic。

### M10 协程 ✅（2026-08-31 完成，设计见 `docs/milestone10/DESIGN.md`）

命名`suspend`函数/方法、完全类型化的suspend状态机变换、`Continuation`与最小启动/挂起原语（spec 8.2、11.9；impl spec 2.3）。已完成MIR CFG化、HIR concrete化后再做MIR状态机变换、direct / virtual / interface与型变bridge的hidden ABI、真实挂起/同步完成/失败恢复、异常物化及跨挂起`catch` / `finally`，并以强制GC验证嵌套frame/adapter链和递归扫描。高层协程构建器与调度器仍属标准库；M10以core的`SuspendTask` / `SuspendRegistration`适配器打通无lambda前置依赖的端到端闭环。

### M11 函数类型、函数值与 closure ✅（2026-08-31 完成，设计见 `docs/milestone11/DESIGN.md`）

已完成ordinary / suspend function type、lambda、匿名函数、局部函数、callable reference与捕获closure的全链路实现（spec 8.1；impl spec 2.2–2.4）。跨stage固定“HIR concrete化 → MIR closure conversion → coroutine transform”顺序；generic callable value、静态/动态函数型变adapter与suspend hidden ABI均保持完全类型化。M11采用类似Java lambda的保守capture边界，但以Scoop显式声明的不可变性为准，不推导effectively final：只允许捕获`val`、参数、`this`等不可重新绑定的binding。captured value type按concrete layout内联于closure，不隐式生成shared cell或boxing。closure/adapter都有独立TypeDescriptor与完整GC扫描描述，并已覆盖异常、真实挂起和强制GC。core已在M10的`SuspendTask` / `SuspendRegistration`协议上增加函数值形态适配重载。

M11 同时补齐 M7 预留的局部函数候选层，并把 managed 函数值与 native `FunPtr` 明确分开：只有下一里程碑在 `FunPtr<F>` 期望位置处理合格的顶层 `::name`，lambda/closure 不自动变成 native callback。

### M12 FFI 注解族 ✅（2026-08-31 完成，设计见 `docs/milestone12/DESIGN.md`）

已完成 `@Extern` / `@NoGC` / `@Unsafe` / `@Safe` / `@CLayout` / `@CallingConvention` / `@Global` / `@ThreadLocal` / `@InteriorMutable`、`value` / `ref` kind bound、显式调用类型实参以及 `Ptr` / `FunPtr` 的全链路实现（spec 第 13、14 章）。C ABI 通过编译器生成且带静态布局断言的 C bridge 交给 host C compiler分类；Scoop ABI 保持普通Scoop typed signature与direct ref，不经过C storage bridge（M15在不改变该ABI的前提下把caller实现修订为typed native-borrowed transition）。extern function/global/TLS、GC-free本地存储、CLayout struct双向传值、raw pointer操作、同线程同步静态 `@NoGC` callback、native root frame和 core GC/output boundary迁移均已有独立 IR golden与端到端 fixture。

M12 只实现普通、非挂起的 FFI：`@Extern` 与 `suspend` 互斥，挂起函数也不能在 `FunPtr` 上下文中解析为原生地址。两种情况都由 HIR 直接诊断；不生成 wrapper，也不向外暴露 M10 hidden continuation ABI。`FunPtr<F>` 复用 M11 的正式函数类型与中性 `::name` 语法，但通过期望类型选择独立的 native ABI resolution。C ABI 只接受 GC-free C-FFI-safe值并经过 C bridge；Scoop ABI复用 typed managed ABI直接传 ref，跨 safepoint由 native root slot保活与更新。M12 callback只支持同步同线程的静态 `@NoGC` target；managed closure保活、异步调用和foreign-thread入口明确延后到M13。

### M13 多线程 GC 与 foreign-thread managed callback ✅（2026-09-01 完成，设计见 `docs/milestone13/DESIGN.md`）

M13 已将 M9 的单 mutator runtime升级为**多 mutator、stop-the-world、collector仍单线程且不移动**的基线，并补完 M12 明确延后的 GC-aware managed closure反向回调。它不引入语言级线程库，而是让 `pthread_create` 等带 opaque context的 C API 能安全地在 foreign thread上执行普通 Scoop closure。HIR输出同时完成`ExportHir` / `LocalConcreteHir`的typed-id隔离；fully specialized type及每个enum variant携带完备`gc_free: bool`，tagged enum采用GC-free共享slot、ref-bearing独占slot，niche仅用于与`Option<ref/Ptr/FunPtr>`结构同构的enum。

- runtime 建立显式 thread attach/detach、TLS thread state、栈边界、per-thread TLAB/native-root链和全局线程登记表；主线程、runtime创建的线程及 foreign thread使用同一套注册实体；
- GC 请求通过入口/回边 safepoint poll、native/runtime入口和线程状态完成 STW handshake。C ABI outbound call在多线程模式下发布 caller roots并进入 native-safe状态；Scoop ABI direct-ref callee只有在登记 native roots的显式入口参与协调，collector不得在其普通 native指令区间移动或扫描未知裸指针；
- 分配器、Immix block/free-line元数据、card table、handle/pin表、全局根与 native-root登记改为多线程安全。M13 只要求单线程 collector完成 STW tracing/sweep；精确根与moving compaction进入M15，parallel/concurrent marking和分代仍留 backlog；
- 新增 managed callback registration协议：Scoop侧以 ordinary、非 suspend closure和编译器生成的 typed invoke adapter注册一个 runtime-owned opaque token；token内部用 `GcHandle` 保活 closure，C侧只持有静态 C ABI trampoline与 GC-free context pointer；
- foreign callback入口执行 attach-if-needed → enter managed → invoke adapter → leave managed → detach-if-owned。参数/返回值必须是 C-FFI-safe，closure body可正常分配、触发 GC和调用 managed代码；这条路径独立于 M12 的静态 `FunPtr` / `@NoGC` target路径，不得通过放宽 `FunPtr` 来源规则实现；
- callback token具有显式 retain/release和一次性 ownership transfer规则；异常在反向边界内捕获并转换为 status/受管异常handle，绝不穿越 C frame。首个端到端 fixture用 `pthread_create`：registration分别保留worker ownership与join-observer ownership；创建失败释放两者，成功后新线程调用捕获 closure、释放worker ownership并detach，`join`侧以仍存活的observer读取完成/异常、按约定重新抛出后执行最终release；
- 任意 closure导出首版只支持具有显式 `void *user_data` / context槽的 C API；没有 context参数的 callback API仍只能使用 M12 静态 `FunPtr`，动态 executable trampoline/slot registry不在本里程碑；
- continuation完成/恢复状态改为原子协议，使普通 managed callback在 foreign thread中恢复既有 continuation时不产生数据竞争；调度器、`launch` / `async`、结构化并发、取消和 suspend FFI仍不属于 M13；
- 验收使用确定性barrier同时覆盖：多个mutator分配并强制GC、一个线程阻塞在native-safe C ABI调用、一个Scoop ABI direct-ref callee跨runtime入口使用native root、foreign thread反复attach/invoke/detach，以及callback token的retain/release、create失败和异常路径；
- M13只保证runtime/GC与callback token本身的线程安全，不把未同步的普通managed可变状态竞争定义为安全行为；跨线程共享数据必须由native同步原语或后续标准库memory model约束。

### M14 泛型类型、上界约束与接口化 ✅（2026-09-02 完成，设计见 `docs/milestone14/DESIGN.md`）

- generic nominal type统一模型：普通用户class/struct/enum/interface使用各自的ExportHir template → typed application → LocalConcreteHir specialization身份；既有generic struct/enum/interface迁出`declaration id + type args`旧表示，新增invariant generic class的构造推导、generic base/interface、宿主成员解析及单态化layout/TD/vtable/itable完整闭环；`is`/`as`/`as?`按完整application identity工作，不擦除type argument或接受裸generic目标；
- non-virtual generic method闭环：class/struct/enum method可声明自己的类型参数与bound；class generic method必须final，任何generic method都不能open/abstract/override、实现dispatch slot或进入vtable/itable。宿主参数前缀与method参数后缀使用不同typed identity，共同参与推导、callable reference、单态化及跨Cone template输出；
- 泛型上界约束：`T : Interface` 与 `where` 子句（spec 2.1/3.2 的既有语法落地）、有界类型参数上的方法解析（bounded method resolution）；
- `@Intrinsic`扩展到compiler-represented core type：Int/UInt/Boolean/String在core源码中显式声明ToString/Hash/equals等nominal能力；`Array<T>` / `MutableArray<T>`迁移为使用普通generic class身份的generic intrinsic representation family，删除独立built-in array type identity。固定表示与表示族都由typed kind/application提供，不伪装成零字段普通类型。生产模式只允许sysroot provider；compiler test可通过内部`CompileOptions`按input/Cone allowlist授权，且只放宽来源检查；
- `ToString` / `Hash` 接口落地（spec 11.11）：所有类型都通过普通implements/override显式adopt，不生成值类型派生conformance；`print` / `println` 改造为 `fun <T : ToString> print(v: T)`（单态化静态分发，退役 M7 的 `Any.toString()` 分发形态）；
- equals 的 operator fun 化（成员限定，spec 11.11）：class 的 `==` 走 `equals` 运算符，值类型的条件派生 `==`；vtable 前三槽（Any 方法）拆除；
- 受益方：M16 字符串插值的 `add<T : ToString>`；同时退役现有按对象地址实现的过渡 `Any.hashCode` / `toString`，避免把地址稳定性带入M15 moving collector。
- 附加完成M13后发现的IR完备性整改：MIR expression携带非可选类型；intrinsic、compiler-generated exception与function type canonical mapping全部类型化；LIR call完整携带target/signature/result/effect，pointer null保留provenance，layout/TypeDescriptor/dispatch只用typed identity连接；用sum type消除native global、foreign callback、caller root与enum field中的非法组合。所有信息由上游结构化地产生，删除下游按context、arena反扫、FQN/symbol或并行字段猜测/补齐的路径。Any typed method/fixed-slot问题随本里程碑主线拆槽自然消失，不作为独立附加项重复实现。

### M15 精确根、statepoint relocation 与 moving compaction（设计见 `docs/milestone15/DESIGN.md`）

M15在M13的多mutator STW与M14清理后的对象语义之上，把GC从“只会mark、地址永远不变”推进为**单代、STW、单线程collector的moving Immix**。本里程碑首先是正确性门：所有managed ref都必须来自可枚举、可更新的root/field slot，不能继续依赖“旧地址碰巧还能用”。分代、parallel/concurrent collection不与moving一起引入。

- M15首个且唯一强制target为macOS/AArch64（Apple Silicon、Mach-O、LLVM stack map v3）；其他target在codegen前明确拒绝，不允许退回非移动/保守模式。opaque typed target profile从完备capability选择runtime platform bundle；Mach-O image、Darwin thread/VM及AArch64 frame/anchor分为可组合组件，通用stackmap parser、root visitor与collector不得包含平台分支。新增平台复用已有维度，只登记profile并补缺失组件；
- 编译器后端固定为与当前stable Rust一致的LLVM 22.1，本路线不包含LLVM升级。DarwinAArch64 profile固定使用22.1标准SelectionDAG/TargetMachine pipeline：GC pointer不进入vreg，post-RA `FixupStatepointCallerSaved`禁止callee-saved register root，`DeoptLiveIn`与透传原始LLVM backend option均禁止；因此managed GC root的产出契约是可写`Indirect [SP/FP + offset]`，object检查只是验证该后端不变量的防御性断言，不负责从任意LLVM产物中猜测能力；
- runtime从dyld已fixup的进程内`__LLVM_STACKMAPS,__llvm_stackmaps`精确解析并登记record，以每个parked线程的return address和受检stack location定位root；moving collection不允许回退到保守栈扫描。全局根、immortal/stable external object、native/compiler root slot、`GcHandle`、对象字段、数组/enum/tuple/closure/coroutine frame中的引用都必须通过统一的可改写slot visitor更新；
- runtime入口按类型隔离为generated managed薄入口、Scoop ABI native-borrowed入口、runtime internal实现及foreign callback gateway：只有managed薄入口捕获直接caller的PC/SP/FP；native-borrowed入口只扫描冻结caller roots和callee登记slot，不能把C frame冒充managed frame；
- outbound native-safe/native-borrowed在transition薄入口保留唯一零`gc-live`/relocate statepoint并捕获冻结segment的精确anchor，实际native machine call为普通nounwind调用。LIR完备caller-root plan只负责transition所在的顶层generated frame及含ref result slot；冻结segment的外层managed frame从transition anchor按stack map更新，不能要求callee反推任意外层caller liveness。返回managed后顶层值全部从caller-root/result slot reload，不能同时消费普通`gc.relocate`；
- `Managed` pointer发射为LLVM address space 1，raw/code/metadata保持address space 0。函数入口及每条循环回边由LIR显式输出带`SafepointId + StatepointLiveSet`的poll，codegen不得补插或重算roots；普通call的集合还必须包含LLVM 22.1会自动列入`gc-live`的可移动direct实参载体，并为RS4GC不会递归发现的aggregate实参拆叶。codegen验收必须检查`rewrite-statepoints-for-gc`之后的IR，而不只检查`gc.statepoint`和stackmap section存在：跨普通poll/call存活的managed ref必须产生并使用正确的`gc.relocate`结果。LLVM当前不可表达的exceptional relocation不作为依赖：managed invoke把两个后继活跃ref与可移动实参spill到带edge-role的显式compiler root frame，由codegen直接发射零`gc-live`的statepoint invoke，normal/unwind各自reload并pop，不得携带exceptional `gc.relocate`；
- M13建立的managed/raw/code/metadata pointer provenance继续保留；未pin的interior/derived pointer不得跨safepoint，必要时从relocated base重新计算。每个statepoint使用确定的typed id，post-RS4GC verifier与Mach-O/AArch64 artifact测试共同锁定root count、relocate dominance、return PC和受支持location kind；
- collector为被移动对象建立forwarding关系，将未pin存活对象evacuate到新line/block，再重写全部roots与heap引用。`PinnedPtr`指向的对象地址保持不变，pinned对象的出站引用仍须更新；`GcHandle`的generation/identity不变但slot内容更新到新地址。解除pin后，对象可在后续collection移动；
- Scoop永久不支持GC finalizer、析构回调或对象复活；M15在不可达判定、moving与reclaim中不调用用户代码。未来只预留处理GC-free typed payload的release hook，用于兜底释放native resource；它不属于M15，也绝不能扩展为managed finalizer；
- block header、free-block/free-line node、object-start、精确allocation size与forwarding等collector元数据全部迁出GC arena；可变长String/Array及large object不得从TypeDescriptor fixed size或block span反推复制长度；
- 增加专用**moving GC stress mode**：禁用TLAB/threshold绕过，使每次managed allocation都进入slow path并在分配新对象前执行一次完整moving compaction；collector内部的evacuation allocation不得递归触发stress collection。除pinned对象外，每个可移动存活对象都应在该轮取得不同地址，避免启发式evacuation因“这次没搬”掩盖悬空引用；
- stress mode完成全部root/heap slot重写并验证forwarding闭包后，清除旧object-start记录并用固定非法pattern poison完整旧副本。只要一个源block在本轮evacuation后不再含任何live/pinned对象，就立即`mprotect(PROT_NONE)`并在该stress进程余下生命周期内隔离、不重新交给allocator；为此block header、链表和free-list节点等collector元数据必须移到block外。含pinned对象而不能整块保护的block仍poison其中已迁出的旧副本；
- 验收强制覆盖：普通local/parameter/phi、含ref aggregate、递归对象图、数组/tagged enum、异常catch/materialize、closure与interface dispatch、协程挂起frame、`GcHandle`、pin/unpin、Scoop ABI native root reload，以及M13 foreign-thread callback/多mutator组合。测试必须断言未pin对象地址确实改变、所有合法引用仍指向同一identity，并以poison/`PROT_NONE`使故意保留的旧裸地址确定性失败；
- M1–M14全部fixture必须在普通moving模式下回归；选定的GC/FFI/closure/coroutine组合fixture必须在“每次allocation compact”的stress mode下通过。只有能生成stackmap、但runtime不消费或不更新root，不算完成M15。

### M16 字符串插值

f-string 与 `StringBuilder` 脱糖（spec 第 6 章，设计见 `docs/milestone16/DESIGN.md`）。低优先级语法糖；`add<T : ToString>` 由 M14 支撑。

### M17 多 Cone 与 `.slib`

`Cone.toml`、依赖图、`.slib` 打包与reader、三层meta（impl spec 2.6）、re-export（`public import`）。在接入多Cone之前先落实HIR消费者边界：`ExportHir`供下游HIR，包含导出的concrete语义接口与generic template；`LocalConcreteHir`只供本Cone MIR，包含完全特化的类型与函数体。两侧使用不同的typed id，`.slib`不得打包或暴露`LocalConcreteHir`。

## 3. 备注

- 里程碑内的特性验收标准：独立 fixture + 组合 fixture + 相关编译错误规则的 negative fixture + 各 stage 的 golden dump（见 AGENTS.md 编码准则）。
- 里程碑顺序可按实现中发现的依赖调整，但 M0 不推迟、M3 不晚于任何依赖 `Option` 的特性。
- 2026-08-28 顺序调整：字符串插值由 M6 后移至 M12（低优先级语法糖）；引用类型层级提前为 M6，新增 M7 函数重载；原 M8–M12 顺延为 M8–M13。其后（同日）再调整：新增 M12"泛型上界约束与接口化"（ToString/Hash/equals，spec 11.11 已定稿），字符串插值顺延为 M13、多 Cone 顺延为 M14。
- 2026-08-31 顺序调整：在 FFI 前新增 M11“函数类型、函数值与 closure”，先完成 lambda/callable reference/closure conversion，使 FFI 直接复用正式函数类型；原 M11–M14 顺延为 M12–M15。
- 2026-08-31 顺序调整：在 FFI 后新增 M13“多线程 GC 与 foreign-thread managed callback”，补完 managed closure反向回调和 foreign-thread runtime入口；原 M13–M15 顺延为 M14–M16。
- 2026-09-01 顺序调整：在接口化之后新增M15“精确根、statepoint relocation与moving compaction”，以强制relocation stress mode前置验证managed ref/root契约；字符串插值与多Cone顺延为M16/M17。

## 4. 待补齐清单（backlog）

各里程碑"涵盖但只实现了部分"的事项，按来源里程碑整理。标注→的为目标里程碑（已知时）；未标注的待排期。

### 来自 M1

- ~~always-leak GC → M9 替换~~（已完成）；
- ~~parser 错误恢复~~（已完成：lexer 收集多个可恢复词法错误；parser 按顶层声明、类型成员和块内语句同步，存在诊断时丢弃残缺 AST）；
- 单文件单 Cone 编译 → M17。

### 来自 M2

- struct 字段默认值、命名参数调用、次构造函数（spec 4.1）；
- 副本更新表达式 `s.{ f: v }`（spec 4.5）；
- `break` / `continue` / `for` 循环（`for` 与区间见 M5 行）；
- 定宽整数族 `Int8/16/32/64`、`UInt*`（spec 11.2；`Int` 已固定 i64）；
- 整数溢出语义（spec 未定，需先回 spec 补充）；
- 内建 print 重载 → M7 转为 core 普通重载（设计已含）。

### 来自 M3

- ~~`!!` 失败 trap → `UnwrapException`~~（M8 已完成）；
- `while` 条件中禁用 `?.`/`?:`（诊断拒绝；待 `break` 或循环重组方案，需先回 spec 讨论）；
- ~~`f(None, 1)` 式"先 None 后绑定"的推断~~（已完成：函数重载、泛型 enum/struct 构造统一按整组实参固定点推导，延迟上下文实参且保持源码求值顺序）；
- ~~显式类型实参 `f<Int>(x)`~~（M12 已完成：parser以事务式 probe保持与比较运算符消歧，HIR按完整实参列表定型函数、构造、变体与成员调用）；
- ~~`value` / `ref` 类型约束（spec 13.9）~~（M12 已完成：所有 generic声明保留类型化 kind bound，定义点与具体实例化点均检查）；
- ~~非 Unit 函数返回的分支穷尽分析~~（已完成：按顺序块、`if`、穷尽 `when`、`try/catch/finally` 组合分析可落空路径）；
- ~~`if` 作为表达式~~（已完成：分支尾表达式定型并写入隐藏结果 local；无期望类型时计算可表达 LUB，无 `else` 的值位置诊断拒绝）；
- ~~泛型 **struct** 声明~~（已完成：字段/方法类型形参作用域、构造推断与嵌套应用全链路落地；M13边界修订要求HIR生成独立的fully specialized local-concrete struct实体后再交给MIR）。

### 来自 M4

- core 与用户代码同单元编译 → M17 的 `.slib` 与 Cone 隔离；
- ~~注解仅 `@Intrinsic` 且仅 sysroot~~（M12 已扩展为类型化 FFI 注解族，并统一完成参数、目标和共存检查）；
- 构造函数式变体的默认值只支持常量表达式（完整 spec 8.5"定义处解析、调用处求值"随函数默认参数一起做）；
- ~~`when` 的表达式形态（产生值）~~（已完成：模式绑定、守卫与穷尽检查沿用语句形态，正常分支尾值统一定型，支持嵌套控制表达式）；
- 命名字段模式的子模式（`S { f1: 0, .. }` 字面量匹配——ast::FieldPattern 需扩展）；
- 表达式位的裸变体名解析推广到所有 enum（当前仅 `Option` 的 `Some`/`None`；spec 4.2/5.1 的"上下文可确定类型时可省略前缀"在表达式位只对 Option 生效）；
- tuple/struct 的穷尽性按"穷尽模式组合"判定（当前要求 catch-all 或 `else`；spec 5.2/5.3 的组合判定是保守简化）；
- ~~tagged enum嵌入struct/tuple/class字段时精确扫描~~（M13修订：pure-value variant共享payload，含ref variant使用独占slot及固定ref偏移，扫描不读取tag）；
- `for` 循环变量与 lambda 参数的解构（随 `for`/lambda）。

### 来自 M5

- ~~`toArray` / `toMutableArray` 方法形式~~（已完成：与构造函数形式共用 `ArrayClone`，保持 memcpy 独立快照语义）；
- `for` 循环与区间 `IntRange` 等（spec 11.8；含 `..` 区间运算符与 rest 的共存验证）；
- `String` 下标/切片；
- 数组 `==` 语义（spec 缺口，需先回 spec 第 10 章补充）；
- ~~数组字面量混合引用类型的 LOB 推导~~（已完成：唯一可表达最小上界；多个互不可比较的最小共同上界退化为 `Any`；数组元素位禁止值类型 auto-box）；
- ~~数组越界 trap → 异常~~（M8 已完成）。

### 来自 M6

- 次构造函数、`init` 块、body 属性（非构造函数属性）、`super` 调用；这些声明自身拥有的初始化体固定为非挂起上下文（spec 8.2、9.1.1；M10 设计已锁定）；
- interface 的属性与默认实现；
- ~~泛型 interface 与声明点 `in` / `out` 变型~~（已完成：接口应用类型贯穿 AST/HIR/MIR，位置合法性与变型子类型关系在 HIR 检查；MIR 按具体实参生成独立接口 TypeDescriptor，并为引用/值 ABI 生成变型 itable bridge）；
- ~~`equals` / `hashCode` / `toString` 的用户覆写~~（M14 已按接口化设计完成：`equals` 走成员 `operator fun`，`ToString` / `Hash` 显式adopt，vtable不再保留Any固定前三槽）；
- companion object、`object` 声明、`sealed`、委托（`by`）；object/companion 的初始化与属性委托协议不得隐式挂起（spec 8.2、9.1.1）；
- 顶层属性与 object/companion 的精确初始化时机、跨文件顺序及循环初始化诊断（M12 只设计 GC-free 常量初始化的显式 `@Global` / `@ThreadLocal` 存储与无 initializer 的 extern global；通用属性语义仍需按 spec 9.1.1 在实现前定稿）；
- `const val`（仅顶层/object/companion，HIR 编译期常量求值与依赖环检查，不生成 runtime initializer；spec 9.1.2）；
- 可见性修饰符（`internal` 语义 → M17 多 Cone 前）；
- `?.` 后随方法调用（`a?.foo()`）；
- smart cast 完整 flow analysis（当前简化：仅不可变局部变量、仅 `is`/`!is` 与 `&&`）；
- 基类构造委托实参不可引用构造函数属性（`class B(val x: Int) : A(x)` 中 `x` 暂不可用于委托实参——hir-lower 在空作用域降级）；
- ~~class 字段按 8 字节槽索引的约定与连续 sub-8 字段布局冲突~~（已修复：LIR `HeapLoad` / `HeapStore` 携带自然布局的字节偏移，连续 `Boolean` 不再被错误扩为槽）；
- ~~泛型成员函数~~（M14 已补齐class/struct/enum的non-virtual generic method、两组typed argument identity、bound/推导/callable reference与单态化闭包；interface method-level generic在定义处拒绝，未来动态分派ABI另列backlog）；
- `Any` 的 core 库形态（spec 11.1；当前编译器内建）。

### 来自 M7

- ~~两个泛型重载推导出相同类型实参时，单态化实例按符号错误合并~~（已修复：以 `GenericFunctionId + concrete type args` 为实体键，重载实例符号带定义 discriminator）；
- 候选集分层的完整层级：局部函数层 → M11；显式 import / 星号 import 分层随 import 机制落地后插入（当前为“成员 → 调用点同侧顶层 → 对侧隐式导入”三层）；
- 泛型候选的MSC比较改用Kotlin式fresh-variable约束系统（当前为“推断后类型实参参与比较”的简化）；与postponed argument、projection参与的LUB/overload一起汇总到“M14设计预留”的完整constraint solver项；
- ~~`write` 的 `@Intrinsic` 退役~~（M12 已直接声明 `@Extern(abi = "scoop") fun write(String)`，作为 managed ABI direct-ref入口）；
- ~~`print` / `println` 的 `Any.toString()` 过渡分发~~（M14 已改为 `fun <T : ToString> ...` 的普通generic bound调用，并拆除Any固定槽）；
- 歧义/无匹配诊断的候选明细展示（首版只报主消息）；
- 默认参数/vararg 的决议规则、运算符重载（`operator fun`）、`context` 参数（spec 8.3）——随各自特性落地时补齐。

### 来自 M8

- ~~try 的表达式形态（`val x = try {...}`）~~（已完成：try body / catch body 共同定型，结果写入发生在 finally 前，finally 值丢弃且 return / throw 仍覆盖待定结果）；
- catch 遮蔽降级为警告（当前为错误；待警告级别诊断基础设施）；
- ~~finally 内的路径分析~~（已完成：必退出的 finally 覆盖 try/catch 的返回、异常与正常继续路径；可落空 finally 保留原路径结果）；
- 异常穿越 Scoop ABI FFI frame 的规则（维持 runtime spec 第 5 章的暂定“初版禁止”）。

### 来自 M9

- 分代（nursery、晋升、remembered set 消费卡片表、代间引用检查）；
- 精确消费statepoint stackmap、root relocation与Immix evacuation/defragmentation → M15；
- arena扩容、多段arena与普通模式下的长期碎片率/compaction启发式调优仍待后续；
- ~~多 mutator STW协调、线程注册/握手与线程安全分配/根表 → M13~~（已完成：pthread registry、合作式epoch握手、per-thread TLAB及同步heap/root/handle/pin元数据；parallel/concurrent collector仍待后续）；
- ~~tagged enum的精确扫描描述发射~~（M13修订：移除`SCOOP_REFS_ENUM`按tag分派，独占ref-bearing slot的固定偏移可与`SCOOP_REFS_SEQUENCE`及数组元素扫描组合）；
- ~~hir-lower 的泛型 struct 字段类型形参作用域~~（已完成：移除 core GC struct 按名识别 stopgap，泛型定义本身不进入 MIR，仅发射具体实例）；
- 其余定宽整数族（Int8/16/32、UInt8/16/32，spec 11.2；UInt/UInt64 已落地）。

### 来自 M10

- ~~suspend function type、函数引用与 lambda/closure~~（已由 M11 完成；core 保留 `SuspendTask` / `SuspendRegistration` 最小协议，并已增加函数值形态适配 overload）；
- `launch` / `async`、dispatcher、事件循环、结构化并发与取消属于标准库设计，不进入编译器最小 core；
- ~~continuation 的跨线程恢复状态协议 → M13~~（已完成：生成的adapter/frame以acquire/release/CAS发布并竞争完成；调度器和线程切换策略仍由后续标准库定义）；
- ~~suspend FFI ABI（→ M12 实现既定禁令）~~（已决策：现阶段不支持；`@Extern` 与 `suspend` 互斥，挂起函数的声明引用不能在 `FunPtr` 上下文中解析为原生地址，不得生成 wrapper 或暴露 M10 hidden continuation ABI）；
- frame elision、栈上 fast path、共享 adapter 代码等优化；当前优先保留完全类型化、可由 GC 扫描的显式 frame/adapter。

### 来自 M11（设计预留）

- 显式 mutable/reference/move capture 或 capture list：当前只允许按值捕获 `val`、参数、`this` 等不可重新绑定的 binding，不做 effectively-final 推导，不为外层词法局部 `var` 隐式 boxing。未来方案必须以源码可见的新语法明确 lifetime、identity、并发与 ABI 成本，不能直接放宽 M11 的 `var` 诊断。

### 来自 M12（设计预留）

- ~~managed closure导出、callback token、类型化 invoke adapter、foreign-thread attach/detach及 `pthread_create` 组合闭环 → M13~~（已完成；M12 的裸 `FunPtr` 仍只表示同步同线程的静态 `@NoGC` callback地址，managed callback必须使用配对的trampoline与opaque context）；
- 无显式 context/user-data槽的 C callback API所需动态 trampoline或有限 slot registry仍待后续；M13 不通过泄漏 closure或把 managed ref伪装成裸指针支持它们。

### 来自 M14（设计预留）

- non-interface generic type（class/struct/enum）的声明点`in`/`out`及其position检查、subtyping、表示转换和必要的分派bridge；M14只实现invariant nominal application。class的参数可能接收不同layout的value type，struct/enum本身又具有不同concrete value layout，不能未经表示设计直接照搬interface的声明点型变；
- use-site `in` / `out` projection：补齐projection type AST/HIR、subtyping、member读写签名变换、overload/inference与capture conversion；
- star projection：它表示一个捕获的未知application，不是省略实参，也不能擦除成`Any`。后续必须定义`C<*>`/`I<*>`的RTTI与cast、itable/vtable key、返回或接收未知value type时的ABI、跨Cone metadata；generic struct/enum等unboxed value application需要明确禁止projection还是采用显式existential boxing；
- class upper bound；M14的upper bound只接受完整interface application；
- 可作为普通表达式静态类型的交叉类型；M14的多个interface bound只构成type parameter能力集合；
- interface方法自身的type parameter及其跨Cone specialization/itable ABI；未来实现必须保证每个合法interface application仍可作为普通reference type，并同时支持concrete、interface与bounded receiver调用，不得引入`Self`、trait object或object-safety分类；
- generic `typealias`：type parameter/bound、透明展开、递归alias诊断、可见性和跨Cone export；alias不产生新的nominal application、layout、TypeDescriptor或单态化身份；
- generic extension property；普通member/top-level property自身不允许method式type parameter。该能力随extension property基础语义落地，并须定义receiver参数如何参与推导及getter/setter单态化；
- nested/inner generic type与generic class companion的参数作用域：static nested type不隐式继承外层参数，`inner` type必须携带outer application与outer ref；object/companion声明自身没有type parameter、不按每个宿主application复制，也不能隐式使用宿主type parameter，其中的generic method仍必须non-virtual；
- 显式type argument中的`_`占位及部分推断；当前只允许“整组省略并推断”或“整组完整写出”；
- 更一般的polymorphic recursion。M14只接受generic callable递归SCC中环上参数替换合成为identity的可判定子集，并在参数增长/变化的递归环上定义处诊断；未来放宽必须提供结构化termination proof，不能以worklist深度、实例数或超时充当语义；
- 完整Kotlin fresh-variable/postponed-argument constraint system，以及projection参与后的MSC、LUB与overload比较；M7“推断后实参比较”的简化backlog并入此项；
- runtime generic dictionary、witness参数、反射式bound调用或共享generic body；M14仅实现单态化，后续只有在代码体积、动态加载或其他明确需求出现时再设计，不能作为缺失concrete信息的fallback；

### 来自 M15（设计预留）

- GC-free release hook：用于遗漏显式`release`/`close`时兜底清理native resource。未来设计只允许具有唯一managed identity的`ref` owner使用，值类型因复制语义禁止；hook只接收编译器验证为GC-free的typed payload副本，只可调用`@NoGC` native release primitive，不接收managed对象/`this`，不能分配、抛异常、挂起、回调managed代码、操作root/handle/pin或复活对象；显式释放可原子disarm，collector至多claim一次，moving只转移armed状态。执行时机、顺序及正常/异常退出时执行均不保证，shutdown不做全堆finalization pass。全功能GC finalizer永久不支持，不是本backlog的一部分。

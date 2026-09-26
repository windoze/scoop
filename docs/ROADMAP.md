# Scoop 实现路线图

运行时类型转换的失败构造使用前端解析的实际异常类型与 constructor 引用，并沿共有的类型、callable、ABI 和 Link 路径消费。删除由 Cast 反向投影的独立 CastFailure call-site、RuntimeOperationDependency role，以及 reader 对同一目标再按 compiler protocol 进行资格判断的通道；普通源码调用的位置、参数、结果与 typed 引用检查保留。共有 HIR 格式更新为 `hir/cross-cone-interface/27`，原 call-site reason tag 2 与 external-reference role tag 9 退役，不复用；旧产物、profile fingerprint 与缓存重建。该调整不改变转换失败抛出 ClassCastException 的语言行为、runtime C ABI 或 String 表示。

引用上行转换在 HIR 中用显式 `ReferenceUpcast` 节点保存内部表达式及目标类型，不能直接改写构造、调用或局部读取的原始类型。MIR 使用已有 `Retype`，不分配对象、不改变引用身份；构造器仍按实际所属 class 分配。默认值正文使用新 expression tag 58 保存同一操作，tag 44 继续退役；共有 HIR 格式更新为 `hir/cross-cone-interface/27`，旧产物与缓存重建，不改变 runtime C ABI。

M23-6 的实际消费覆盖外来接口类型、父接口、class 虚方法、接口默认方法与动态覆写；真实 provider 源码产生方法和 dispatch 表，下游只消费完整产物，独立与组合 fixture 锁定 HIR/MIR/LIR 并链接运行。

删除仅由测试实现的默认值operation-typing、nested ABI及root/origin语义工厂和其证明数据、平行验证入口与专用测试。正式reader继续使用共有声明表、完整typed模板、类型与binder检查、来源位置、局部数据流及真实引用一致性检查。局部数据流直接借用模板与共有字段查询，删除重复body input及authority适配器；nested descriptor保留实际类型化身份、parent/path与binder数据，删除独立Standalone证明模式。语言操作规则由前端负责，不在IR/meta crate再复制实现。此清理不改变wire字段、profile版本、runtime C ABI或String表示。

参数自由依赖class通过共有名义声明取得真实身份、modality、直接父类型和声明序字段，引用类型按GC契约传播，递归引用字段不展开成递归值布局。HIR保留完整声明及解析后的字段、父类型，不重建同名本地声明。外来class构造仍是对真实constructor的typed调用；HIR→MIR消费已选MIR绑定中的ClassInitializer角色，使用共有class分配路径创建对象，再将该对象作为receiver调用实际provider的初始化实现。构造表达式返回分配的对象，物理initializer返回Unit；普通返回class的函数不走构造分支。本地与外来initializer共用Callee表示、参数求值顺序和GC处理。 core默认导入层的构造调用和静态限定名同时查询普通Type/Value binding；用户新增的class、enum和typealias与其他依赖使用相同声明路径，不限于预设内建名字。此能力不增加来源凭证、core分支或runtime ABI，沿用现有类型、MIR绑定及LIR布局格式。 M23-6的真实class运行在单个最终镜像中链接实际产物及runtime；LLVM stackmap段按各对象贡献的完整v3 blob逐个读取，长度由已有count和对齐决定，保留每份格式、记录唯一性与精确PC检查，不增加展开预算。该读取不要求M23-8的多镜像启动或M23-9的program-link。

M23-6 清理默认值逐引用访问证明及仅测试使用的平行完整验证入口。保留前端定义处与继承后的可见性检查、完整 typed 正文和引用索引；producer 直接投影，reader 在边界检查真实引用和跨表关系。默认引用 field 3 退役，`hir/cross-cone-interface/27` 要求旧 `/24` 及更早产物、profile fingerprint 和缓存重建；跨 Cone 默认值实际展开与运行仍是验收要求。

共有导出绑定包含 enum 变体的真实 typed ID，nominal 的 `nested_bindings` 同时列出其静态命名空间中的嵌套类型、object value 与 enum 变体；变体归属由实际声明确定，不能误作包级值。`hir/cross-cone-interface/27` 更新该格式语义，旧 `/23` 及更早产物与缓存重建；既有 tag 不复用，不保留双轨 reader，runtime C ABI 和 String 表示保持。

M23-6 的 enum 模式验收覆盖实际跨 Cone 的 unit／payload 模式、嵌套 tuple、guard、穷尽性、类型别名和定义处默认值，并由第三个 Cone 消费后链接运行。模式与变体测试保留正文中的真实声明引用，不追加构造器访问资格；引用集合更新使用 `hir/cross-cone-interface/27`，旧 `/22` 及更早产物重建。

canonical ABI 导出复用同次完整 IR 的实际签名和 callable definition，删除重复逐参数 layout ID 表及只为该表存在的重放入口。共有 reader 以真实声明和 MIR lowered signature 核对 ABI，dispatch 按实际 receiver 查询表示。`lir/cross-cone-layout-abi/3` 退役 callable field 5，其余字段编号保持；旧 `/2` 产物与缓存重建，profile 和内容 fingerprint 按格式正常更新。tuple 的字段与参数使用完整结构身份和已有存储算法，不为嵌套值补造独立 nominal、layout/TD definition 或来源记录。

M23-6 的结构签名与字段验收包含实际 tuple 参数／结果、嵌套 tuple 字段、全 ZST 字段、enum tuple payload、成员／getter 及默认值的跨 Cone 编译、发布与运行。普通 callable 已生成的记录直接用于后续 MIR 组装、boxing 和 dispatch，不再生成一份重复绑定后丢弃。

共有 nominal 声明直接保存 struct 主构造器的 typed declaration ID，供前端按实际语言角色检查 `@NoGC` 调用；不能从参数形状、字段布局或 provider 身份推断主构造器。主构造器继续保留源码 Managed、物理 NoGC 的既有合同；值构造本身不分配，`@NoGC` 的参数、结果与局部值仍须 GC-free，managed 次构造器仍禁止调用。`hir/cross-cone-interface/27` 在 `NominalDeclarationDetailsV1` 新增 field 8：空数组表示无值主构造器，单元素数组保存其 constructor ID；有构造器的 struct 必须明确该引用，引用必须属于同一 nominal 的声明集合，其他 nominal 不得填写。旧 `/21` 及更早格式退役并要求重建，既有 tag 不复用，profile 与内容 fingerprint 正常更新；MIR/LIR callable 格式和 runtime ABI 不变。

M23-6 的源码消费验收包含跨 Cone struct 主／次构造器、命名／默认参数、ZST 与大值返回 ABI 的独立及组合用例。验收需由真实源码生成 provider 产物、下游生成完整产物并完成对象链接与运行；构造器按真实 typed target 消费既有 MIR/LIR 定义。

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

M23-6 的实际跨 Cone 验收包含 struct 类型、普通 final 成员、字段与计算属性读取、默认参数（含定义处字段引用及下游再次发布）、命名参数、operator、ZST 和含引用值。独立与组合 fixture 通过真实源码编译发布，再由下游仅凭产物消费；三个 Cone 的实际对象必须完成适用的链接和运行。共有 callable 查询支持 dispatch 引用普通成员，不能用历史 metadata 分区或来源资格限制合法使用。其余 M23-6 能力与清理仍按 [阶段设计](milestone23/stage6/DESIGN.md) 完成后验收，不以这部分通过代替整个里程碑。

M23-6 的框架清理以生产调用链为依据：移除未被共有 reader 使用的 HIR foundation/declaration transcript、来源绑定层及专用证明测试，保留实际声明查询、默认参数实例化和正常产物错误回归。真实源码发布与下游消费继续作为验收依据。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 候选和已选声明只保存实际 provider ID，不逐项复制 artifact 坐标/fingerprint 凭证；HIR→MIR 使用同次编译的依赖快照及完整声明，核对实际定义和签名，不再次比较来源凭证。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

版本：0.2（草案）

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

try / catch / finally / throw，landingpad 落地（runtime spec 第 5 章）。四条 trap 路径已全部改接真实异常：`!!` → `UnwrapException`、数组越界 → `IndexOutOfBoundsException`、`as` → `ClassCastException`、整数除零 → `ArithmeticException`（spec 11.7 已同步新增后者）。M8首版的`scoop_rt_throw`使用`__cxa_allocate_exception` + 拷贝；M25已在不改变语言语义和显式CFG的前提下用Scoop自有exception record与Level-I personality替换该实现。

### M9 真 GC ✅（2026-08-30 完成，设计见 `docs/milestone9/DESIGN.md`）

真 GC 替换 always-leak：Immix 核心（32KB block / 128B line、bump 分配、free-line 复用、标记-区域回收，1 GiB mmap arena）；statepoint 打开（GC strategy + `rewrite-statepoints-for-gc` + safepoint poll + stackmap，runtime v1 暂用经对象起点校验的保守栈扫描）；对象头扩为 16B（td + gc_word）；递归 TD 扫描描述；pin（对象头标志位）与 GcHandle 表落地；`scoop.core.gc` 包；写屏障卡片表（预偏置指针，为分代预留）；M1–M8 全部 fixture 在真 GC 下原样通过。M12 已把公开 pin/handle API 迁为 `@Unsafe` 普通 core 函数，仅保留 raw runtime boundary intrinsic。

### M10 协程 ✅（2026-08-31 完成，设计见 `docs/milestone10/DESIGN.md`）

命名`suspend`函数/方法、完全类型化的suspend状态机变换、`Continuation`与最小启动/挂起原语（spec 8.2、11.9；impl spec 2.3）。已完成MIR CFG化、HIR concrete化后再做MIR状态机变换、direct / virtual / exact interface hidden ABI、真实挂起/同步完成/失败恢复、异常物化及跨挂起`catch` / `finally`，并以强制GC验证嵌套frame/adapter链和递归扫描。高层协程构建器与调度器仍属标准库；M10以core的`SuspendTask` / `SuspendRegistration`适配器打通无lambda前置依赖的端到端闭环。

### M11 函数类型、函数值与 closure ✅（2026-08-31 完成，设计见 `docs/milestone11/DESIGN.md`）

已完成ordinary / suspend function type、lambda、匿名函数、局部函数、callable reference与捕获closure的全链路实现（spec 8.1；impl spec 2.2–2.4）。跨stage固定“HIR concrete化 → MIR closure conversion → coroutine transform”顺序；generic callable value、静态/动态函数型变adapter与suspend hidden ABI均保持完全类型化。M11采用类似Java lambda的保守capture边界，但以Scoop显式声明的不可变性为准，不推导effectively final：只允许捕获`val`、参数、`this`等不可重新绑定的binding。captured value type按concrete layout内联于closure，不隐式生成shared cell或boxing。closure/adapter都有独立TypeDescriptor与完整GC扫描描述，并已覆盖异常、真实挂起和强制GC。core已在M10的`SuspendTask` / `SuspendRegistration`协议上增加函数值形态适配重载。

M11 同时补齐 M7 预留的局部函数候选层，并把 managed 函数值与 native `FunPtr` 明确分开：只有下一里程碑在 `FunPtr<F>` 期望位置处理合格的顶层 `::name`，lambda/closure 不自动变成 native callback。

### M12 FFI 注解族 ✅（2026-08-31 完成，设计见 `docs/milestone12/DESIGN.md`）

已完成 `@Extern` / `@NoGC` / `@Unsafe` / `@Safe` / `@CLayout` / `@CallingConvention` / `@Global` / `@ThreadLocal` / `@InteriorMutable`、`value` / `ref` kind bound、显式调用类型实参以及 `Ptr` / `FunPtr` 的全链路实现（spec 第 13、14 章）。C ABI 通过编译器生成且带静态布局断言的canonical C bridge交给已验证system C compiler profile分类；Scoop ABI保持普通Scoop typed signature与direct ref，不经过C storage bridge（M15在不改变该ABI的前提下把caller实现修订为typed native-borrowed transition）。extern function/global/TLS、GC-free本地存储、CLayout struct双向传值、raw pointer操作、同线程同步静态 `@NoGC` callback、native root frame和core GC/output boundary迁移均已有独立IR golden与端到端fixture。

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
- 受益方：M26 字符串插值可直接使用普通`add<T : ToString>`；同时退役现有按对象地址实现的过渡`Any.hashCode`/`toString`，避免把地址稳定性带入M15 moving collector。
- 附加完成M13后发现的IR完备性整改：MIR expression携带非可选类型；intrinsic、compiler-generated exception与function type canonical mapping全部类型化；LIR call完整携带target/signature/result/effect，pointer null保留provenance，layout/TypeDescriptor/dispatch只用typed identity连接；用sum type消除native global、foreign callback、caller root与enum field中的非法组合。所有信息由上游结构化地产生，删除下游按context、arena反扫、FQN/symbol或并行字段猜测/补齐的路径。Any typed method/fixed-slot问题随本里程碑主线拆槽自然消失，不作为独立附加项重复实现。

### M15 精确根、statepoint relocation 与 moving compaction ✅（2026-09-03 完成，设计见 `docs/milestone15/DESIGN.md`）

M15在M13的多mutator STW与M14清理后的对象语义之上，把GC从“只会mark、地址永远不变”推进为**单代、STW、单线程collector的moving Immix**。本里程碑首先是正确性门：所有managed ref都必须来自可枚举、可更新的root/field slot，不能继续依赖“旧地址碰巧还能用”。分代、parallel/concurrent collection不与moving一起引入。

- M15首个且唯一强制target为macOS/AArch64（Apple Silicon、Mach-O、LLVM stack map v3）；其他target在codegen前明确拒绝，不允许退回非移动/保守模式。请求级opaque typed target selection从完备capability选择runtime platform bundle；M23-2进一步把其中LIR-target、backend、C-bridge、runtime-build与final-link证明拆为五个不可互相推导的projection。Mach-O image、Darwin thread/VM及AArch64 frame/anchor分为可组合组件，通用stackmap parser、root visitor与collector不得包含平台分支。新增平台复用已有维度，只登记profile并补缺失组件；
- 编译器后端固定为与当前stable Rust一致的LLVM 22.1，本路线不包含LLVM升级。DarwinAArch64 profile固定使用22.1标准SelectionDAG/TargetMachine pipeline：GC pointer不进入vreg，post-RA `FixupStatepointCallerSaved`禁止callee-saved register root，`DeoptLiveIn`与透传原始LLVM backend option均禁止；因此managed GC root的产出契约是可写`Indirect [SP/FP + offset]`，object检查只是验证该后端不变量的防御性断言，不负责从任意LLVM产物中猜测能力；
- runtime从dyld已fixup的进程内`__LLVM_STACKMAPS,__llvm_stackmaps`精确解析并登记record，以每个parked线程的return address和受检stack location定位root；moving collection不允许回退到保守栈扫描。全局根、immortal/stable external object、native/compiler root slot、`GcHandle`、对象字段、数组/enum/tuple/closure/coroutine frame中的引用都必须通过统一的可改写slot visitor更新；
- runtime入口按类型隔离为generated managed薄入口、Scoop ABI native-borrowed入口、runtime internal实现及foreign callback gateway：只有managed薄入口捕获直接caller的PC/SP/FP；native-borrowed入口只扫描冻结caller roots和callee登记slot，不能把C frame冒充managed frame；
- outbound native-safe/native-borrowed在transition薄入口保留唯一零`gc-live`/relocate statepoint并捕获冻结segment的精确anchor，实际native machine call为普通nounwind调用。LIR完备caller-root plan只负责transition所在的顶层generated frame及含ref result slot；冻结segment的外层managed frame从transition anchor按stack map更新，不能要求callee反推任意外层caller liveness。返回managed后顶层值全部从caller-root/result slot reload，不能同时消费普通`gc.relocate`；
- `Managed` pointer发射为LLVM address space 1，raw/code/metadata保持address space 0。函数入口及每条循环回边由LIR显式输出带`SafepointId + StatepointLiveSet`的poll，codegen不得补插或重算roots；普通call的集合还必须包含LLVM 22.1会自动列入`gc-live`的可移动direct实参载体，并为RS4GC不会递归发现的aggregate实参拆叶。codegen验收必须检查`rewrite-statepoints-for-gc`之后的IR，而不只检查`gc.statepoint`和stackmap section存在：跨普通poll/call存活的managed ref必须产生并使用正确的`gc.relocate`结果。LLVM当前不可表达的exceptional relocation不作为依赖：managed invoke把两个后继活跃ref与可移动实参spill到带edge-role的显式compiler root frame，由codegen直接发射零`gc-live`的statepoint invoke，normal/unwind各自reload并pop，不得携带exceptional `gc.relocate`；
- M13建立的managed/raw/code/metadata pointer provenance继续保留；未pin的interior/derived pointer不得跨safepoint，必要时从relocated base重新计算。每个statepoint使用确定的typed id，post-RS4GC verifier与Mach-O/AArch64 artifact测试共同锁定root count、relocate dominance、return PC和受支持location kind；
- collector为被移动对象建立forwarding关系，将未pin存活对象evacuate到新line/block，再重写全部roots与heap引用。`PinnedPtr`指向的对象地址保持不变，pinned对象的出站引用仍须更新；`GcHandle`的generation/identity不变但slot内容更新到新地址。解除pin后，对象可在后续collection移动；
- Scoop永久不支持GC finalizer、析构回调或对象复活；M15在不可达判定、moving与reclaim中不调用managed用户代码。M24在不放宽这条边界的前提下增加同步GC-free release hook，专门用于兜底释放native resource；
- block header、free-block/free-line node、object-start、精确allocation size与forwarding等collector元数据全部迁出GC arena；可变长String/Array及large object不得从TypeDescriptor fixed size或block span反推复制长度；
- 增加专用**moving GC stress mode**：禁用TLAB/threshold绕过，使每次managed allocation都进入slow path并在分配新对象前执行一次完整moving compaction；collector内部的evacuation allocation不得递归触发stress collection。除pinned对象外，每个可移动存活对象都应在该轮取得不同地址，避免启发式evacuation因“这次没搬”掩盖悬空引用；
- stress mode完成全部root/heap slot重写并验证forwarding闭包后，清除旧object-start记录并用固定非法pattern poison完整旧副本。只要一个源block在本轮evacuation后不再含任何live/pinned对象，就立即`mprotect(PROT_NONE)`并在该stress进程余下生命周期内隔离、不重新交给allocator；为此block header、链表和free-list节点等collector元数据必须移到block外。含pinned对象而不能整块保护的block仍poison其中已迁出的旧副本；
- 验收强制覆盖：普通local/parameter/phi、含ref aggregate、递归对象图、数组/tagged enum、异常catch/materialize、closure与interface dispatch、协程挂起frame、`GcHandle`、pin/unpin、Scoop ABI native root reload，以及M13 foreign-thread callback/多mutator组合。测试必须断言未pin对象地址确实改变、所有合法引用仍指向同一identity，并以poison/`PROT_NONE`使故意保留的旧裸地址确定性失败；
- M1–M14全部fixture必须在普通moving模式下回归；选定的GC/FFI/closure/coroutine组合fixture必须在“每次allocation compact”的stress mode下通过。只有能生成stackmap、但runtime不消费或不更新root，不算完成M15。

### M16 统一约束系统与重载决议 ✅（2026-09-03 完成，设计见 `docs/milestone16/DESIGN.md`）

- 统一普通/local/member/extension函数、generic nominal构造、enum variant、operator与callable reference的候选及applicability入口；single-candidate不再绕过统一检查；
- 以candidate-local fresh variables、结构化equality/subtyping/kind/interface bound及postponed arguments替换M3/M7/M14分散的固定点绑定；lambda/callable reference/`None`/空数组/嵌套generic构造在同一session完成；
- MSC改为与本次actual inference隔离的pairwise fresh-variable forwarding constraint system，删除“比较推断后concrete type arguments”的简化；
- 失败候选不产生永久HIR实体，winner原子地产生唯一typed callee、完整owner/callable concrete arguments和argument adaptation；`LocalConcreteHir`不得含inference variable、constraint或export placeholder；
- 完成invariant nominal generic application、function type variance与bound范围；整数literal widen在对应语言能力落地时扩展同一solver。原context parameter规划由M27的运行期exact-match模型取代，不进入solver。

### M17 命名参数、默认参数与 `vararg` ✅（2026-09-04 完成，设计见 `docs/milestone17/DESIGN.md`）

- AST/parser正式区分位置、命名、spread与尾随lambda实参，以及required/default/vararg parameter；所有callable/constructor按候选独立映射；
- 默认表达式作为callable source interface在定义处完成绑定、类型检查及调用域覆盖检查，导出default只能引用export/re-export实体而不携带private/internal hidden dependency closure；winner及完整type arguments确定后，只在实际缺省处经同一实例化器hygienic展开。receiver先求值，显式实参按源码顺序求值，随后default按声明顺序求值；所有concrete expression都具有完备、类型隔离的definition/evaluation origin，default机制不识别具体intrinsic；
- `vararg T`的实际参数类型是`Array<T>`；位置element/spread产生fresh array，命名whole-array直接使用；当前invariant Array要求spread精确匹配`Array<T>`；
- 重载补齐候选参数名shape filter、“更少实际default”与“无vararg”优先规则；default/empty vararg不为generic inference虚构约束；
- `ExportHir`完整携带parameter calling shape、hygienic default template、definition origin及包含真实typed目标与定义位置的kind-specific引用；定义处与继承后的调用域覆盖检查由前端完成，产物不附加证明记录；local/export default使用不同id且不向下游stage泄漏，winner commit后`LocalConcreteHir`/MIR只见完整位置参数与普通typed array assembly；
- 覆盖普通/local/member/extension/generic function与method、class/struct主构造、constructor-style enum variant、abstract/interface/default inheritance及Scoop ABI extern；C `...`仍不支持。

### M18 callable 表面补齐（设计见 `docs/milestone18/DESIGN.md`）

- 完整operator声明角色、表达式映射、infix优先级与property-like `invoke`，全部复用M16/M17的candidate-local constraint、参数映射、MSC与求值协议；primitive/String/Array/Ptr能力由普通core operator声明提供，winner后才正规化为typed intrinsic；
- `++`/`--`、复合赋值与多参数下标使用typed place plan保证receiver/index/右值各求值一次；`LocalConcreteHir`前消除source operator与place计划；
- `?.method()`按Option分支lower，实参/default/vararg只在Some分支执行；结果始终再包一层Option，不展平`Option<Option<T>>`；
- `componentN`支持class位置解构并导出typed role；`iterator`与range operator表面进入M18，`for`/range core类型仍由M22消费。属性委托operator随M21定义reflection-free协议。

### M19 构造与初始化（设计见 `docs/milestone19/DESIGN.md`）

- class primary constructor补齐普通参数；class body加入带显式类型/initializer的stored property与按源码交错执行的`init`，并以field readiness和受限`InitializingThis`禁止读取未初始化字段或发布半初始化对象；
- class/struct secondary constructor复用M16/M17的候选、generic host推导与完整参数协议，委托图在HIR验证唯一typed target、termination和cycle；
- class构造改为一次exact allocation后在同一receiver上依次执行base、primary field、body initializer / `init`与secondary body；废除递归拼接继承字段的flattened constructor捷径；
- `super.method()`只在direct base member层决议并强制direct dispatch；constructor / initializer保持非挂起，普通suspend caller的显式构造实参仍可挂起；
- moving GC下initializing receiver和已写ref字段沿普通root/relocation传播，未写payload在首个safepoint前全零。

### M20 泛型类型系统第二阶段（设计见 `docs/milestone20/DESIGN.md`）

- 正式固定class/struct/enum/interface的全部nominal type parameter为invariant；删除interface旧variance bridge，不提供declaration/use-site `in`/`out`、star projection或capture conversion；
- generic算法通过callable自身type parameter与bound表达；application变换使用显式`map`/重建，未知application使用非generic interface或显式erased wrapper；expected type支持直接构造目标`Option<T>`等exact application；
- upper bound扩展为至多一个class加多个interface，bound均为参数完整的exact application；bounded receiver在实例化后解析为普通direct/virtual/interface target；
- 调用显式type argument list允许用`_`逐项继续推断；`_`复用M16同一candidate-local constraint/MSC内核，并在LocalConcrete HIR前完全消失；runtime、RTTI、dispatch和跨Cone metadata保持exact-only。

### M21 属性、对象与可见性（设计见 `docs/milestone21/DESIGN.md`）

- 统一logical property/accessor模型，覆盖stored/computed/extension/interface/delegated property、自定义getter/setter、override与typed place；property不再等同于field；
- 永久删除`lateinit`与隐藏未初始化状态。无accessor的`var p: Option<T>`/`var p: T?`可省略initializer，语义精确等价于在该初始化位置写`= None`；其他stored property仍必须完整初始化；
- reflection-free delegate协议不传`KProperty`/名称：可选`provideDelegate()`与必需`getValue(thisRef)`/`setValue(thisRef, value)`形成独立typed role；
- top-level/static nested `object`与non-generic companion使用线程安全exactly-once gate，只在完整初始化后发布；top-level runtime property在`main`前初始化，失败记忆、直接/间接循环与moving-GC root契约一次锁定；
- interface function/property accessor支持default body，按class hierarchy优先与唯一most-specific interface选择；冲突要求显式override，`super<I>`只direct调用direct superinterface default；
- 默认visibility改为`internal`，对外API逐项显式写`public`；四种visibility以typed access domain贯穿候选、override、signature exposure、M17 default的前端检查及`.slib`中的实际声明可见性。M12的raw `@Global`/`@ThreadLocal`与普通managed top-level property正式分离；
- 支持static nested nominal/object声明；`inner`/anonymous/local object、generic delegated extension及class/interface delegation仍不在本里程碑。

### M22 循环、值模式与定宽整数 ✅（2026-09-07 完成，设计见 `docs/milestone22/DESIGN.md`）

- `for`、不带标签的`break`/`continue`与typed loop target；统一while/for header，控制转移按目标cleanup深度穿越catch/finally/suspend状态，所有回边继续满足M15 poll契约；
- public exact `Iterator<T>`/`Iterable<T>`协议、Array/MutableArray迭代器，以及四个不同nominal identity的普通core `IntRange`/`UIntRange`/`LongRange`/`ULongRange`、`until`/`downTo`/`step`；
- struct命名字段副本更新，固定base只求值一次、RHS源码顺序与声明序重建；任何enum目标都是稳定编译错误，必须通过`when`匹配后显式重建；
- binding pattern与match pattern分流、命名字段递归subpattern、sound pattern-matrix完备性/witness，以及由import或唯一expected enum application驱动的通用裸variant；
- 八种signed/unsigned定宽整数、candidate-local literal fit、显式转换与全宽layout/C ABI；`Int`/`UInt`固定32位且`Int32`/`UInt32`为alias，`Long`/`ULong`固定64位且`Int64`/`UInt64`为alias；无上下文literal采用`Int → Long`/`UInt → ULong`默认阶梯，算术采用定义良好的wrapping并显式处理LLVM division/shift边界；既有64位source/core契约整体迁名为`Long`/`ULong`，包括Array size/index（上限仍`INT64_MAX`）、Hash、integer及String的compareTo、integer shift count、SourceLocation、`@CLayout`的aligned/packed与其他M22触及的API，内部machine metadata继续使用独立typed scalar；String length/index/slice尚未进入实现子集，由M26首次以`Long`表面引入；
- 当前可执行profile仍要求64位data/code pointer、全零null carrier及合法地址逐bit往返；`Ptr<T>`/`FunPtr<F>`迁为无公开representation field的compiler-represented family，阻断解构/copy update伪造；`Ptr`的raw/to与`sizeOf`/`alignOf`暂用`ULong`，element offset暂用`Long`，且只保留typed unsafe nonzero-ULong入口并要求pointee GC-free；`FunPtr`不提供源码constructor或integer转换，裸pointer固定非零、null只由`Option`的niche表示；platform-native integer及这些临时底层surface的最终迁移留待后续设计；
- 为固定宽度/Kotlin整数拼写及普通用户别名提供top-level非generic透明`typealias`；alias只有声明/可见性身份，不产生第二个类型/layout/RTTI/ABI。四种range本身不是alias；generic alias及真实跨Cone编码仍留后续。

### M23-1 source表面、parser与当前编译单元lookup ✅（2026-09-09 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage1/DESIGN.md`。

- 实现`package`、exact/star `import`、`as`和`public import`的AST/parser、文件头恢复与当前编译单元内package lookup；跨Cone target尚未开放时必须显式诊断，不伪造artifact或留下半成品AST节点。
- 完成门：新语法的parser golden、negative/组合fixture与旧fixture全量回归。

### M23-2 persistent identity与`.slib` wire基础

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage2/DESIGN.md`。

- 冻结Cone及kind-specific persistent identity、exact/source/unit/callable application/body/runtime/safepoint id、当前generic body所需的最小ODR group/member identity、`PersistentV1` mangler与session remap。callback source site使用允许binder的`PersistentCallbackRegistrationId`，fully concrete materialization使用`{ registration, CallableMaterializationContext }`的`PersistentCallbackApplicationId`；generic delegated initializer/ensure的local generic owner使用`EnclosingInitializationApplication { unit }`。Initialization generated template只引用声明级unit，application unit只进入materialization context并决定实际body root。box/coroutine step/slot/shell/start统一按`ExactOwnerRoot`归属；该root不豁免helper依赖的generic nominal application/ODR能力。M23-3/6先预物化param-free source nominal的非callable有限shape-support closure，shell/start到M23-7随完整ODR proof加入；consumer不得替定义Cone发Strong定义。
- 统一冻结`ByteSpan`、`DomainSeparatedCborHash`与仅供callable body使用的`RuntimeEncode`；`.slib`冻结deterministic container、三层schema-1 metadata envelope、typed member directory、资源上限、基础Graph/Compile reader与schema演进规则。三条`identity-foundation/1`保存可重算的exact canonical payload；native witness只覆盖extern/callback边界的source nominal闭包，Scoop extern的`GcEffect::{Managed, NoGc}`与ordinary/suspend effect分离，不把foundation冒充通用layout服务。
- M23-2只持久化`ValidatedLirTargetSelection { lir_target, backend }`；请求级registry原子解析`ResolvedTargetProfile`的`lir_target/backend/c_bridge_toolchain/runtime_build/final_link`五个projection。canonical C signature只描述generated-C source storage，不持久化完整target C classifier；LLVM candidate绑定`ValidatedBackendProfile`，generated-C candidate绑定`ValidatedCBridgeToolchainProfile`。bridge recipe使用producer-independent `GeneratedBridgeUnitId`，实际定义使用producer-specific `GeneratedBridgeAtomId`，LIR/ODR relocation引用unit并由object verifier从atom规范化回unit。
- member envelope从本阶段起允许任意数量、任意已登记producer的`LinkObject`以及opaque/required blob；成员用途不依赖文件名、扩展名、顺序或object数量。后续语义payload按独立section/capability version加入，不在尚无verifier时宣称最终Link view完成。

### M23-3 single-Cone artifact与core分离 ✅（2026-09-16 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage3/DESIGN.md`。

- 引入`Cone.toml`、source discovery、library/executable entry sum及manifest/protocol边界；`scoopc`每次只消费显式上游`.slib`闭包并产生当前Cone `.slib`，不搜索、递归、构建runtime或最终链接。
- `scoop.core`成为trusted独立library artifact；先闭环core-only编译，其他Cone dependency稳定拒绝到M23-5。实现compiler侧per-Cone image producer、strong-only六类registration/image digest、member-aware definition/undefined requirement、当前Scoop/generated `LinkObject` verifier与基础Link view，使本阶段成功artifact已通过Compile/Link双view；core中可跨Cone引用的param-free source nominal同时验证并物化M23-2冻结的有限shape-support closure。production profile必须拒绝任意ODR group/member/body/symbol，直到M23-7具备完整证明。single-file synthetic request固定为一个source、executable、core-only Cone dependency，其`.slib`只能作为local executable root artifact。

### M23-4 resolved build graph与调度 ✅（2026-09-16 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage4/DESIGN.md`。

- `compiler/scoop` orchestration library解析exact locator和静态无环、同`group:name`单版本的DAG，拥有cache与dependency-first子进程调度。M23-6 将原 Compile/Link 双重完成门禁收缩为父进程的归档/摘要一致性检查，完整 typed IR 与对象由实际编译器消费者读取。
- 以recording artifacts和core-only真实节点完成chain/diamond/cycle、ambiguous locator、stale dependency、cache失效与child失败传播；本阶段不提前开放跨Cone源码名称。

### M23-5 多Cone名称语义 ✅（2026-09-19 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage5/DESIGN.md`。

- 落地direct/support closure、cross-Cone exact/star/alias import、re-export、public/internal/private access provenance、default、non-generic alias与selected HIR/MIR/LIR metadata；新增cross-Cone strong profile，成功machine-use只开放已解析基础类型构成的 const 及签名完全由这些参数自由类型构成的非generic top-level/extension callable或property accessor。凡需尚未具备的cross-Cone layout/dispatch或物化实现的使用稳定拒绝到M23-6，不由consumer临时发Strong定义；receiver-dependent protected、generic或direct source-extern能力分别拒绝到M23-6、M23-7或M23-10，不产生残缺IR。
- 完成direct/transitive可见性、split package、链式re-export、negative lookup observation及semantic cache失效矩阵；每个成功用例仍产生双view有效artifact。

### M23-6 跨Cone layout、typed ABI与ZST

清理 concretizer 入口的整模块迭代语义重放及其独立 IR/meta 验证实现，删除仅服务该通道的伪造 HIR 测试和修改工厂。保留真实源码的 iterator 选择、类型、effect、默认值、解构和循环控制回归；正常格式与引用检查仍由实际产物边界负责。

跨 Cone 的 `throw`、语句及表达式形式的 `catch` 使用前端已解析的实际 `Throwable` 声明，沿共有依赖类型查询取得继承关系并执行同一子类型规则；导入协议不允许跳过检查。同名普通 class 不能替代该实体。异常构造、默认参数、调用、布局、TD 和展开使用共有路径，保留调用求值顺序、catch 顺序与 finally 语义；此能力不增加协议资格、独立证明、wire 字段或 runtime ABI。 公开存储属性的 getter 与可公开调用的 setter 必须按实际 owner 和声明类型生成普通 callable body，即使 provider 的正文没有引用该属性；名义类型、顶层属性与 core 使用同一规则。参数自由且可物化的属性自动导出实际 body。泛型或 source-only owner 仅用于签名和表示查询时，不自动实例化其普通方法或 accessor；实际调用仍通过同一 typed 请求生成完整实例，dispatch 所需的方法继续随其实际类型物化。

跨 Cone 的强制 `as` 沿实际 `ClassCastException` 声明查询完整异常类型，并选择该声明的零参数 constructor 或默认参数适配入口。成功检查保留原对象身份，失败通过普通 class 分配、外部 initializer 调用和 throw 执行；别名、interface 与装箱值沿同一类型与 ABI 路径处理。默认参数中的转换在实际展开时进入相同路径，未求值模板不触发机器物化。异常类的表示仍依赖后续泛型能力时，前端在输出 LocalConcrete HIR 前对实际执行点给出已有 layout-required 诊断；不输出缺少所需表示的成功 IR。MIR 直接消费完整协议中的 typed 声明引用，不再建立丢弃外来声明信息的 Core/Imported 资格投影。所选 constructor 使用共有依赖 callable 集合，保留真实逻辑/物理签名、GC effect、ABI 和 relocation，不增加转换调用的证明记录、wire 字段或 runtime ABI。零参数默认值 adapter 以已有 GeneratedCallable 实体及 ClassInitializer 表示进入共有 callable 导出，不能只发布协议引用而遗漏定义。MIR/LIR selected callable 记录本身是机器依赖引用；reader 按 provider、typed target、定义、签名和传递依赖检查其完整性，不要求隐式调用另附一份 HIR 操作资格记录。

共有 HIR 类型位置的结构、foreign nominal 分发、真实 provider 与定义/求值位置在 HIR reader 边界检查一次。后续物化查询和 MIR/LIR 消费同一未变化的 typed 记录，不重新完整检查这组 HIR 关系，不建立额外验证状态或凭证；实际类型表示、签名、ABI、对象与传递引用仍由相应边界检查。外部字节重新读入或相关数据发生变化时重新验证受影响部分。这一职责调整不改变 wire、内容 fingerprint 的字段组成或 runtime C ABI。

M23-6 删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试，不把它们改名后保留。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。实际发射的 image、root entry、type/callable、storage、immortal、initialization 和 safepoint 记录继续使用既有格式；这项删除不改变实际 runtime C ABI、String 表示或其 fingerprint。M23-8/9 的多 image 启动与 program-link 仍留在后续阶段，按实际入口和引用需要定义数据，不提前冻结新的 program record 或另建 String 授权表。

协议数据清理删除重复 String source/exact capability 和独立 definitions 外层，`hir/core-bootstrap-interface/4` 直接保存完整 typed 角色。旧 section field 4 退役，field 5 保存实际本地产物定义；类型身份、初始化服务与 ABI 继续通过共有路径消费，旧 `/1`～`/3` 产物与缓存重建。

String descriptor 使用完整 MIR 中实际声明的 source exact identity，沿共有 descriptor 查询、layout selection、physical import、registration 和 Link relocation 消费。删除独立 String bridge 与固定角色的 descriptor 恢复通道，不以 provider 坐标或协议来源豁免普通引用检查。Strong production `/7`、`/8` 退役原服务表中的 TD tag 2；旧产物与缓存重建，String 表示及 runtime C ABI 不变。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示升级为 `/9`、`/10`，删除初始化专用 ABI field 11 和外部服务表 field 12，section 只保留 field 2～9 的八字段 product。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

总体设计见 [M23 设计](milestone23/DESIGN.md)，详细设计见 [M23-6](milestone23/stage6/DESIGN.md)，清理范围见 [清理设计](milestone23/stage6/CORE-AUTHORITY-CLEANUP.md)。

- 完成共有 `ValueStorageLayout`、canonical Scoop ABI、scan/TypeDescriptor、ZST payload elision、boxing/address/static token、Array/Ptr 与 C 边界；完成 param-free 跨 Cone 类型、constructor/member/object、继承、dispatch/protected 和有限 shape-support 的实际发布与消费。
- core 作为普通 library 使用相同构建、缓存、依赖、类型查询和产物消费。完成 canonical ABI、native boundary、compiler protocol、String/初始化及 Link 路径中的专用资格清理。已完成部分核对生产调用链并复用。
- 外来 enum 构造直接使用共有声明、参数与默认值，覆盖限定名、直接导入、别名、期望类型及第三 Cone 默认值展开；不要求额外外部机器构造函数。
- 默认值源码位置复用共有 HIR 接口的完整正文和实际使用记录；删除为位置收集保留的旧默认值来源生产器、平行表与专用测试，不新增替代工厂。
- 清除 compiler/slib/runtime 的通用资源预算、计费与配额，删除成本策略与 profile/fingerprint/runtime ABI 的绑定。保留实际范围、整数溢出、非法环与 GC 契约检查。
- 清除额外来源授权、防伪、重复证明和测试专用来源工厂；默认参数、名称解析和语言规则由前端负责，IR/meta 与 reader 不再另建语言语义实现。Compile/Link 共享同一不可变语义结果；发布与 runtime 热点不反复完整重放。
- producer、reader、linker、wire/profile、版本、fingerprint、fixture、golden 和文档同步。退役 tag 不复用，不兼容产物重建。保留实际 C ABI、String 表示和 typed identity。
- 共有默认值和受限声明只保存一份 typed 协议与正文；退役 type-semantics field 5～7 及嵌套源码 payload 的 field 3，capability /5 要求旧产物重建。
- 正式 `scoopc` 发布与 `scoop` 依赖消费接入共有 `CrossConeLayoutStrong` reader；语义会话直接接收完整类型、ABI 与真实对象，移除旧 M23-5 的重复提交和 Link 消费路径，旧依赖重建。
- 正式发布直接保存同次编译完成的归档和普通摘要，回读核对 bytes；删除旧 M23-5 writer 与双版本发布包装，外部产物继续通过共有 reader 检查。

- 构建完成节点、prebuilt 与 cache 复用普通 manifest 摘要和不可变归档；父进程不重新读取完整语义/对象或全部依赖。编译器消费边界保留完整类型、ABI、格式与 Link 检查，缓存失效和 child 结果核对继续执行。
- 真实源码生成完整 `.slib`，由本阶段跨 Cone Compile/Link 路径消费，并完成适用链接与运行；覆盖 core 修改、扩展、重建、下游使用，以及类型、成员、dispatch、ABI、ZST 的独立与组合场景。手工 metadata 和证明反例不能替代验收。
- 原有可复用实现与测试继续核对；只有实际功能、上述清理和必要验收全部完成且按功能提交，才完成 M23-6。当前仍在实施。ODR、multi-image startup、artifact-only program-link 按 M23-7/8/9 的原阶段安排，不提前承接。

### M23-7 跨Cone generic、ODR与generic delegated extension

- 消费M23-2已冻结的group/member key，完成consumer-side concretization、generic hidden support closure、完整ODR member closure、ABI/definition digest DAG、root provenance与跨Cone member-set/definition一致性验证、collection-based object materialization，以及exact receiver application唯一的lazy delegated storage；在同一完整ODR proof下为param-free source nominal加入依赖`Continuation<R>`/`SuspendTask<R>` application的`ContinuationShell`与`CoroutineStart` callable shape-support closure。
- sibling Cone的相同specialization必须产生相同member/fingerprint，冲突由artifact/object合并验证器拒绝；真正地址coalesce与全程序exactly-once留给真实link/runtime阶段。

### M23-8 runtime multi-image registry与启动

- 消费已有 image descriptor，按实际生产调用确定启动所需 C 数据与登记契约；实现六类registration table、全image登记、canonical eager/lazy初始化、no-throw gateway与连续LLVM v3 stackmap blob消费。
- 先由typed synthetic program descriptor驱动3+ image、moving GC、exception、failure/cycle、ODR重复与损坏metadata fatal测试，不依赖生产linker或weak-symbol扫描。

### M23-9 基础artifact-only program-link

- `compiler/runtime-build`只消费`lir_target + c_bridge_toolchain + runtime_build`并产出`ValidatedRuntimeArtifact`；`compiler/linker`只消费`ValidatedArtifactClosure<Link>`、该runtime artifact、`lir_target + final_link`及其他已验证产物，生成并验证program object和无用户native requirement的真实多Cone binary。`final_link`必须显式闭合startup/support/default system provider及target-synthetic输入，基础plan/evidence不得依赖linker隐式default；backend/C-bridge/runtime-build信息只通过已验证producer产物进入link plan。源码、locator、cache和额外raw object不进入stage。
- 完成真实ODR coalesce、multi-object stackmap、初始化、moving GC与exception gateway；带尚未处理native requirement的程序稳定拒绝。

### M23-10 general native requirement闭包与link evidence hardening

- 把M23-9固定target/runtime slice推广到完整用户native extern contract，完成direct object/archive/general dynamic provider验证、snapshot/TOCTOU hardening、完整link plan/evidence、trace核对、可选link-cache重验与最终artifact verifier。
- native候选只能经`.slib`已有typed requirement和显式`--library-path`解析，不能直接注入无来源raw `.o`、archive、linker option或script；任意多个`LinkObject`按typed directory参与，非link blob永不误入。

### M23-11 umbrella CLI、single-file mode与总验收

- 正式启用`cargo`式`scoop`：`scoop build <root>`按DAG调用配套`scoopc`，`scoop run <root> -- ...`仅在同一build/program-link成功后执行。文件root使用reserved synthetic identity、logical `main.scoop`、唯一source与core-only Cone dependency；不发现相邻manifest、Scoop/C/C++源码或blob。FFI只能通过已有typed requirement加显式library search root解析。
- M1–M22及M25历史fixture全部迁到正式`scoop build <file>` orchestration并保留stage dump/诊断/运行结果覆盖；完成多Cone、corruption/reproducibility、cache和moving-GC/exception/closure/coroutine/FFI全量回归后，删除core/source拼接、`scoopc`最终链接和固定object名称/数量旁路。final-link cache是可选优化，不是完成门。

### M24 GC-free release hook（设计见 `docs/milestone24/DESIGN.md`）

- 普通`final class`可声明至多一个不可调用、不可继承的`release { ... }` block；它不是method/finalizer，源码没有managed `this`，只可只读同owner的GC-free backing field；
- HIR到LIR以独立typed id、`ReclaimingReceiver`、`ReleaseSafe` call graph与完备`None | SynchronousGcFree` policy保证hook不能分配、抛异常、挂起、进入safepoint、操作root/handle/pin或回调managed代码；
- exact TypeDescriptor追加静态hook thunk；hook-bearing对象仅在完整构造成功后设置内部`RELEASE_READY`位，构造失败对象不运行hook；
- collector只在逻辑死亡对象真正reclaim前同步claim并调用hook；不复制payload、不建立执行队列。moving只转移ready状态，from-space旧副本绝不触发；
- best effort不保证GC时机、对象间顺序、执行线程或shutdown调用；但正常collection一旦决定回收ready对象，就必须在poison、复用或unmap其存储前尝试一次；
- 显式`close`/`release`仍是主路径，并应先把owner字段置为inert state以避免后续hook重复释放。M24不新增公开arm/disarm API、full finalizer、对象复活、ByteBuffer或external-memory accounting。
- `.slib` container仍为v1，但HIR/MIR/LIR outer schema必须同步升为2，foundation capability分别改为`org.scoop-lang.hir/identity-foundation/4`（承接 M23-6 的 `/3`）、`org.scoop-lang.mir/identity-foundation/2`、`org.scoop-lang.lir/identity-foundation/2`，artifact profile改为`org.scoop-lang.slib-profile/identity-foundation/3`（承接 M23-6 删除成本字段后的 `/2`）；callable body改用v2 key/domain但继续使用runtime metadata encoder ABI `RuntimeEncode`，旧v1 artifact整体重建，不能以outer schema升级代替capability/profile major升级。

### M25 自有异常 ABI 与 libc++abi 退役 ✅（2026-09-05 完成，设计见 `docs/milestone25/DESIGN.md`）

- 以 Scoop 私有 exception record、稳定 `exception_class`、per-thread caught 栈和 begin/end/rethrow 协议替换 `__cxa_*`，异常 payload 继续按值复制并作为 stable external object root 接受 moving GC 更新；
- 实现只接受 LLVM 22.1 catch-all/cleanup 封闭 LSDA 子集的 `scoop_eh_personality`，直接通过 Itanium Level I `_Unwind_*` 完成 search、landing-pad install、resume 与 record 删除，不借用 C/C++ personality；
- HIR/MIR/LIR 的异常语义与显式 CFG 保持不变，codegen只替换personality和runtime symbol；suspend handler继续先物化为managed `Throwable`并结束native catch，record不可跨线程或挂起点；
- Darwin/AArch64最终链接删除`-lc++abi`且不添加显式`-lunwind`，由默认`libSystem`解析unwind接口；C ABI/Scoop ABI FFI的异常边界不扩大；
- 以decoder/runtime/GC/协程组合测试及Mach-O依赖/导入符号检查验收，最终程序不得出现`__cxa_*`、gxx/gcc personality、C++ terminate或`libc++abi.dylib`依赖。

### M26 字符、off-heap ByteBuffer 与字符串插值（待设计）

原M24字符串里程碑整体移至M26，并补齐其实际前置范围：落地`Char`；定义`MutableArray<Char>`与`String`的safe互转、`MutableArray<Byte>`与`String`的unsafe byte互转；设计可增长且以off-heap storage为主的通用ByteBuffer及I/O使用边界；在此基础上以普通core class实现`StringBuilder`，再落地f-string desugar。M26依赖M24 release hook；容量增长、borrow/view、失败原子性、external-memory pressure accounting（含hook路径只扣减、不触发GC的release-safe入口）及字符编码细节仍在M26设计中一次定稿。ByteBuffer仍须提供确定性的显式close，不能依赖hook及时回收。原M16字符串设计已删除，不作为实现依据。

### M27 Task-local Context（设计见 `docs/milestone27/DESIGN.md`）

- 保留Kotlin-like的`context(name: T)` contextual declaration与compiler-known `context(value) { ... }`结构化表达式；后者是block而非lambda/普通函数调用，首版一次绑定一个non-null managed ref，多个binding通过嵌套scope表达；
- canonical exact static type就是Context key。binding与requirement必须exact match；静态类型为derived type的binding不会隐式满足base/interface requirement，调用者须先把表达式显式定型为目标父类型。lookup缺失时抛可捕获的`MissingContextException`；context requirement进入导出metadata与override contract，但不参与overload、MSC、类型推断、函数类型、mangle或普通函数ABI，M27不引入静态effect row；
- contextual declaration在每次activation入口按源码顺序lookup一次，把结果snapshot为普通不可变local；同一activation内后来安装的同型binding不改变已取得的参数；
- binding属于logical coroutine task而非OS thread：普通调用与direct suspend调用共享，`startCoroutine`从当前有效binding fork独立child context；挂起不退出scope，resume在所属TaskContext下运行并在离开driver时严格恢复调用者context；
- `context(value) { ... }`按结构化LIFO语义覆盖normal、return、break、continue与exception cleanup；普通closure不隐式捕获Context，现有`foreignCallback`在registration处捕获binding snapshot，并为每次invocation建立相互隔离的调用Context；同步FFI不切换logical task；
- key从跨Cone的`PersistentExactTypeId`确定，并使用独立的kind-specific typed identity；artifact显式携带与machine-code owner/ODR关系一致的key-use与只写一次cell metadata，程序登记期把同key解析为同一个进程内slot。slot值不是语义identity，不进入`.slib`或program fingerprint，也不以FQN、symbol或arena ordinal回退；
- binding、undo、snapshot、scope mark/execution guard、coroutine frame、thread current-context root及callback handle-registry root中的全部managed ref都必须参与M15精确扫描、relocation与checked write barrier，TLS只定位`ScoopThreadState`。物理索引结构保持runtime-private；设计文档给出适配当前64B small-object上限的小节点persistent radix tree作为参考实现，但不把fanout、节点布局或helper命名固化为语言契约；
- HIR/MIR/LIR以互不混用的typed key/context/mark实体表达完整语义，MIR在coroutine transform前生成scope cleanup CFG，LIR完整携带managed provenance、safepoint与root plan；验收覆盖shadow/missing、全部退出边、真实挂起与跨线程resume、child隔离、同步FFI、并发callback、跨Cone identity及moving-GC stress。

## 3. 备注

- 里程碑内的特性验收标准：独立 fixture + 组合 fixture + 相关编译错误规则的 negative fixture + 各 stage 的 golden dump（见 AGENTS.md 编码准则）。
- 里程碑顺序可按实现中发现的依赖调整，但 M0 不推迟、M3 不晚于任何依赖 `Option` 的特性。
- 2026-08-28 顺序调整：字符串插值由 M6 后移至 M12（低优先级语法糖）；引用类型层级提前为 M6，新增 M7 函数重载；原 M8–M12 顺延为 M8–M13。其后（同日）再调整：新增 M12"泛型上界约束与接口化"（ToString/Hash/equals，spec 11.11 已定稿），字符串插值顺延为 M13、多 Cone 顺延为 M14。
- 2026-08-31 顺序调整：在 FFI 前新增 M11“函数类型、函数值与 closure”，先完成 lambda/callable reference/closure conversion，使 FFI 直接复用正式函数类型；原 M11–M14 顺延为 M12–M15。
- 2026-08-31 顺序调整：在 FFI 后新增 M13“多线程 GC 与 foreign-thread managed callback”，补完 managed closure反向回调和 foreign-thread runtime入口；原 M13–M15 顺延为 M14–M16。
- 2026-09-01 顺序调整：在接口化之后新增M15“精确根、statepoint relocation与moving compaction”，以强制relocation stress mode前置验证managed ref/root契约；字符串插值与多Cone顺延为M16/M17。
- 2026-09-03 顺序调整：撤回原M16字符串插值设计并延后原M17多Cone；新M16先统一fresh-variable constraint solving与overload resolution，新M17再落地命名/default/vararg完整实参协议。其余M2/M4/M6/M7/M14基础backlog及多Cone、字符串能力按上节后续顺序重新排期。
- 2026-09-04 编号确定：原“后续顺序”七项依次编号为 M18 callable表面、M19构造与初始化、M20泛型类型系统第二阶段、M21属性/对象/可见性、M22基础语言能力、M23多Cone与`.slib`、M24字符串底层与插值；各项详细范围仍须spec先行并单独设计。
- 2026-09-04 M20设计决定：nominal generic统一为invariant，撤销既有interface variance并明确不引入projection/star/capture；M20只新增class upper bound与调用点`_`部分类型实参，泛型抽象由generic callable、exact interface及显式转换表达。
- 2026-09-04 M21设计决定：Scoop不提供`lateinit`；延后初始化必须显式使用`Option<T>`/`T?`，其中无accessor的`var Option<T>`省略initializer等价于`None`。同时固定logical property/accessor、reflection-free delegate、interface default、singleton/global exactly-once初始化与typed access domain。
- 2026-09-04 M21可见性修订：默认visibility由Kotlin式public改为internal；public API、public interface contract、public override与public constructor都要求源码显式标记，`main`仍可internal。sysroot不享有默认public特权，计划导出的core API也必须显式标记。
- 2026-09-04 新增M25“自有异常ABI与libc++abi退役”：保留LLVM landingpad与Level I unwinder，以Scoop record/personality/catch状态替换C++ ABI层；M8–M10对应实现选择由M25设计取代。
- 2026-09-05 完成M25：Scoop runtime自有record/personality/caught栈与moving-GC external payload生命周期落地；object与最终Mach-O门禁锁定LLVM 22.1封闭LSDA、八个Level-I导入及`libSystem` provider，生成程序不再链接`libc++abi`。
- 2026-09-05 M22设计决定（后续修订）：`Int`/`UInt`永久固定32位并以`Int32`/`UInt32`为alias，`Long`/`ULong`永久固定64位并以`Int64`/`UInt64`为alias；无上下文literal采用32位优先、越界升至64位的默认阶梯。为避免同时改变既有API值域，原来使用64位`Int`/`UInt`的array、String、Hash、compareTo、shift、SourceLocation、`@CLayout`参数、pointer/size等source/core契约整体迁名为`Long`/`ULong`，Array上限仍为`INT64_MAX`，internal machine metadata不伪装成源码integer。四种整数range均为真实nominal type。当前target仍须有64位data/code pointer及对应内部carrier逐bit往返能力，裸`Ptr`/`FunPtr`固定非零并由`Option`唯一承载null；platform-native integer留待后续。M22其余范围仍为无标签break/continue与for、exact Iterator协议、递归pattern matrix和typed cleanup target；label、do-while、CharRange与generic alias继续留后续。
- 2026-09-05 M23设计决定：Cone与source package分离，以canonical `group:name:version`及按kind隔离的persistent typed id表达跨artifact identity，v1只消费exact、静态、无环resolved graph。`package`、exact/star/alias/public import与re-export只扩展既有typed resolver层；target-specific确定性`.slib`显式打包三层metadata、native object与闭合export surface，下游完成generic concretization并以完整ODR group coalesce。跨artifact ABI同时固定ZST为零payload/typed elision/显式address token，并为Array采用zero-sized element分支而非零stride通用路径。最终静态程序通过唯一program descriptor登记全部Cone image及runtime metadata后按canonical dependency order初始化；package registry/版本求解、动态加载、generic typealias、interface方法级泛型与跨版本ABI不属于M23。
- 2026-09-07 M23工具/容器边界补充：新增`cargo`式umbrella binary `scoop`负责多Cone图、cache和依赖顺序，`scoopc`收缩为只消费显式上游`.slib`闭包的single-Cone compiler，最终binary由独立artifact-only program-link产生。`.slib`改以typed member directory表达任意多个link object、C/C++ object或其他opaque blob，不再把`code.o`/`bridge.o`、扩展名或object数量写成格式假设；所有可链接object统一走`LinkObject`，全体link object只共同提供一个typed image descriptor，Graph/Compile/Link view与runtime/final-input provenance分别闭合。
- 2026-09-08 M23拆分与单文件模式：原M23按依赖拆为M23-1…M23-11，依次落实source语法、persistent identity/`.slib` container、single-Cone/core边界、build graph、多Cone名称语义、ZST/ABI、generic/ODR、runtime registry、基础program-link、native闭包/hardening与最终CLI。正式`scoop build/run <file>`把指定文件作为固定reserved identity、唯一source、core-only Cone dependency的synthetic executable Cone；不发现旁边manifest/源码，native FFI只由artifact已有typed requirement经显式library search root解析。历史fixture在M23-11切到该正式路径并删除旧`scoopc`直编直链/core拼接旁路。
- 2026-09-07 顺序调整：M24改为GC-free release hook，采用“完整构造后ready、逻辑死亡且真正reclaim前同步调用TypeDescriptor hook”的直接模型，不采用payload复制或异步queue；原M24字符串范围整体移至M26，并补入Char、safe字符互转、unsafe byte互转及off-heap growable ByteBuffer。既有已完成M25编号保持不变。
- 2026-09-07 新增M27“Task-local Context”：保留Kotlin-like的`context(name: T)`/`context(value) { ... }`表面，以canonical exact static type为key，结合结构化动态binding和logical-task传播重写原spec 8.3 context parameters。首版不做子类型兼容解析或静态effect row；ordinary ABI保持不变。实现细节区分架构不变量与参考方案，物理索引布局不作为长期契约。

## 4. 待补齐清单（backlog）

各里程碑"涵盖但只实现了部分"的事项，按来源里程碑整理。标注→的为目标里程碑（已知时）；未标注的待排期。

### 来自 M1

- ~~always-leak GC → M9 替换~~（已完成）；
- ~~parser 错误恢复~~（已完成：lexer 收集多个可恢复词法错误；parser 按顶层声明、类型成员和块内语句同步，存在诊断时丢弃残缺 AST）；
- 正式单文件Cone编译/运行模式 → M23-11 `scoop build/run <file>`。

### 来自 M2

- struct字段默认值与命名参数调用 → M17；次构造函数 → M19；
- struct副本更新表达式 `s.{ f: v }`（spec 4.5）→ M22；
- 不带标签的`break`/`continue` jump statement、`for`循环与区间 → M22；`do-while`与带标签的控制流仍待后续；
- 源码可命名的底类型`Nothing`（含signature、generic application与cast）及一般jump expression（例如`value ?: break`、argument/initializer中的jump）→ 后续里程碑；M22只以`ControlOutcome`表达jump路径的semantic bottom，不物化`Nothing` expression/type；
- 定宽整数族 `Int8/16/32/64`、`UInt*`（spec 11.2；M22修订为`Int`/`UInt`固定i32、`Long`/`ULong`固定i64）→ M22；
- 整数溢出语义 → M22（spec 11.2已固定wrapping、除法与shift边界）；
- 内建 print 重载 → M7 转为 core 普通重载（设计已含）。

### 来自 M3

- ~~`!!` 失败 trap → `UnwrapException`~~（M8 已完成）；
- ~~`while` 条件中禁用 `?.`/`?:`~~（M17 已完成：HIR 条件 setup 区域在首次检查及每条回边前重新执行）；
- ~~`f(None, 1)` 式"先 None 后绑定"的推断~~（已完成：函数重载、泛型 enum/struct 构造统一按整组实参固定点推导，延迟上下文实参且保持源码求值顺序）；
- ~~显式类型实参 `f<Int>(x)`~~（M12 已完成：parser以事务式 probe保持与比较运算符消歧，HIR按完整实参列表定型函数、构造、变体与成员调用）；
- ~~`value` / `ref` 类型约束（spec 13.9）~~（M12 已完成：所有 generic声明保留类型化 kind bound，定义点与具体实例化点均检查）；
- ~~非 Unit 函数返回的分支穷尽分析~~（已完成：按顺序块、`if`、穷尽 `when`、`try/catch/finally` 组合分析可落空路径）；
- ~~`if` 作为表达式~~（已完成：分支尾表达式定型并写入隐藏结果 local；无期望类型时计算可表达 LUB，无 `else` 的值位置诊断拒绝）；
- ~~泛型 **struct** 声明~~（已完成：字段/方法类型形参作用域、构造推断与嵌套应用全链路落地；M13边界修订要求HIR生成独立的fully specialized local-concrete struct实体后再交给MIR）。

### 来自 M4

- core与用户代码同单元编译 → M23-3 single-Cone artifact与core分离；
- ~~注解仅 `@Intrinsic` 且仅 sysroot~~（M12 已扩展为类型化 FFI 注解族，并统一完成参数、目标和共存检查）；
- 构造函数式变体的默认值只支持常量表达式 → M17改为完整spec 8.5“定义处解析、调用处实例化”；
- ~~`when` 的表达式形态（产生值）~~（已完成：模式绑定、守卫与穷尽检查沿用语句形态，正常分支尾值统一定型，支持嵌套控制表达式）；
- 命名字段模式的子模式（`S { f1: 0, .. }` 字面量匹配——ast::FieldPattern 需扩展）→ M22；
- 表达式位的裸变体名解析推广到所有 enum（当前仅 `Option` 的 `Some`/`None`；spec 4.2/5.1 的"上下文可确定类型时可省略前缀"在表达式位只对 Option 生效）→ M22；
- tuple/struct 的穷尽性按"穷尽模式组合"判定（当前要求 catch-all 或 `else`；spec 5.2/5.3 的组合判定是保守简化）→ M22；
- ~~tagged enum嵌入struct/tuple/class字段时精确扫描~~（M13修订：pure-value variant共享payload，含ref variant使用独占slot及固定ref偏移，扫描不读取tag）；
- `for`解构，以及lambda/`val`既有解构与M18 class `componentN`能力的共享binding plan → M22。

### 来自 M5

- ~~`toArray` / `toMutableArray` 方法形式~~（已完成：与构造函数形式共用 `ArrayClone`，保持 memcpy 独立快照语义）；
- `for` 循环与区间 `IntRange` 等（spec 11.8；含 `..` 区间运算符与 rest 的共存验证）→ M22；
- `String` 下标/切片 → M26；
- 数组 `==` 语义（spec 缺口，需先回 spec 第 10 章补充）；
- ~~数组字面量混合引用类型的 LOB 推导~~（已完成：唯一可表达最小上界；多个互不可比较的最小共同上界退化为 `Any`；数组元素位禁止值类型 auto-box）；
- ~~数组越界 trap → 异常~~（M8 已完成）。

### 来自 M6

- 次构造函数、`init` 块、body 属性（非构造函数属性）、`super` 调用 → M19；这些声明自身拥有的初始化体固定为非挂起上下文（spec 8.2、9.1.1；M10 设计已锁定）；
- interface 的属性与默认实现 → M21；
- ~~泛型 interface 与声明点 `in` / `out` 变型~~（已完成：接口应用类型贯穿 AST/HIR/MIR，位置合法性与变型子类型关系在 HIR 检查；MIR 按具体实参生成独立接口 TypeDescriptor，并为引用/值 ABI 生成变型 itable bridge）；
- ~~`equals` / `hashCode` / `toString` 的用户覆写~~（M14 已按接口化设计完成：`equals` 走成员 `operator fun`，`ToString` / `Hash` 显式adopt，vtable不再保留Any固定前三槽）；
- companion object 与 `object` 声明 → M21；`sealed`、class/interface delegation（`class C : I by impl`）仍待排期，property delegation由M21覆盖；object/companion初始化与property delegate协议不得隐式挂起（spec 8.2、9.1.1、9.2）；
- 顶层属性与 object/companion 的精确初始化时机、跨文件顺序及循环初始化诊断 → M21（M12 只设计 GC-free 常量初始化的显式 `@Global` / `@ThreadLocal` 存储与无 initializer 的 extern global；通用属性语义仍需按 spec 9.1.1 在实现前定稿）；
- `const val`（仅顶层/object/companion，HIR 编译期常量求值与依赖环检查，不生成 runtime initializer；spec 9.1.2）→ M21；
- 可见性修饰符 → M21（`internal` 必须先于 M23-5 多Cone名称语义完成）；
- `?.` 后随方法调用（`a?.foo()`）→ M18；
- smart cast 完整 flow analysis（当前简化：仅不可变局部变量、仅 `is`/`!is` 与 `&&`）；
- 基类构造委托实参不可引用构造函数属性（`class B(val x: Int) : A(x)` 中 `x` 暂不可用于委托实参——hir-lower 在空作用域降级）→ M19；
- ~~class 字段按 8 字节槽索引的约定与连续 sub-8 字段布局冲突~~（已修复：LIR `HeapLoad` / `HeapStore` 携带自然布局的字节偏移，连续 `Boolean` 不再被错误扩为槽）；
- ~~泛型成员函数~~（M14 已补齐class/struct/enum的non-virtual generic method、两组typed argument identity、bound/推导/callable reference与单态化闭包；interface method-level generic在定义处拒绝，未来动态分派ABI另列backlog）；
- `Any` 的 core 库形态（spec 11.1；当前编译器内建）。

### 来自 M7

- ~~两个泛型重载推导出相同类型实参时，单态化实例按符号错误合并~~（已修复：以 `GenericFunctionId + concrete type args` 为实体键，重载实例符号带定义 discriminator）；
- 候选集分层：局部函数层已由M11落地；M16统一当前local/member/extension/top-level/core层的applicability，显式import/星号import层由M23-1建立本Cone结构、M23-5接入跨Cone surface；
- 泛型候选MSC改用fresh-variable约束系统、统一postponed argument与候选诊断 → M16；M20的`_`部分实参继续复用同一solver；
- ~~`write` 的 `@Intrinsic` 退役~~（M12 已直接声明 `@Extern(abi = "scoop") fun write(String)`，作为 managed ABI direct-ref入口）；
- ~~`print` / `println` 的 `Any.toString()` 过渡分发~~（M14 已改为 `fun <T : ToString> ...` 的普通generic bound调用，并拆除Any固定槽）；
- 歧义/无匹配诊断的候选明细展示 → M16；
- 默认参数/vararg的决议规则 → M17；完整运算符重载 → M18；原`context`参数语义 → M27按task-local Context重写。

### 来自 M8

- ~~try 的表达式形态（`val x = try {...}`）~~（已完成：try body / catch body 共同定型，结果写入发生在 finally 前，finally 值丢弃且 return / throw 仍覆盖待定结果）；
- catch 遮蔽降级为警告（当前为错误；待警告级别诊断基础设施）；
- ~~finally 内的路径分析~~（已完成：必退出的 finally 覆盖 try/catch 的返回、异常与正常继续路径；可落空 finally 保留原路径结果）；
- libc++abi异常实现依赖、自有Level I personality与catch状态 → M25；
- 异常穿越 Scoop ABI FFI frame 的规则（维持 runtime spec 第 5 章的暂定“初版禁止”）。

### 来自 M9

- 分代（nursery、晋升、remembered set 消费卡片表、代间引用检查）；
- 精确消费statepoint stackmap、root relocation与Immix evacuation/defragmentation → M15；
- arena扩容、多段arena与普通模式下的长期碎片率/compaction启发式调优仍待后续；
- ~~多 mutator STW协调、线程注册/握手与线程安全分配/根表 → M13~~（已完成：pthread registry、合作式epoch握手、per-thread TLAB及同步heap/root/handle/pin元数据；parallel/concurrent collector仍待后续）；
- ~~tagged enum的精确扫描描述发射~~（M13修订：移除`SCOOP_REFS_ENUM`按tag分派，独占ref-bearing slot的固定偏移可与`SCOOP_REFS_SEQUENCE`及数组元素扫描组合）；
- ~~hir-lower 的泛型 struct 字段类型形参作用域~~（已完成：移除 core GC struct 按名识别 stopgap，泛型定义本身不进入 MIR，仅发射具体实例）；
- 其余定宽整数族与固定宽度alias → M22；既有64位`Int`/`UInt`实现迁为canonical `Long`/`ULong` identity，新canonical `Int`/`UInt`为32位，`Int32`/`UInt32`与`Int64`/`UInt64`分别作为对应透明alias。

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

- class upper bound → M20；M14的upper bound只接受完整interface application；
- 可作为普通表达式静态类型的交叉类型；M14的多个interface bound只构成type parameter能力集合；
- interface方法自身的type parameter及其跨Cone specialization/itable ABI；未来实现必须保证每个合法interface application仍可作为普通reference type，并同时支持concrete、interface与bounded receiver调用，不得引入`Self`、trait object或object-safety分类；
- generic `typealias`：M22只落地top-level非generic透明alias；带type parameter/bound的alias、递归generic alias及跨Cone打包/re-export仍待后续。alias不产生新的nominal application、layout、TypeDescriptor或单态化身份；
- generic extension property；普通member/top-level property自身不允许method式type parameter。该能力随extension property基础语义落地，并须定义receiver参数如何参与推导及getter/setter单态化；
- static nested generic type与generic class companion参数作用域 → M21：二者不隐式继承宿主参数，object/companion不按host application复制；`inner` type及outer application/ref捕获仍待后续；
- 显式type argument中的`_`占位及部分推断 → M20；当前只允许“整组省略并推断”或“整组完整写出”；
- 更一般的polymorphic recursion。M14只接受generic callable递归SCC中环上参数替换合成为identity的可判定子集，并在参数增长/变化的递归环上定义处诊断；未来放宽必须提供结构化termination proof，不能以worklist深度、实例数或超时充当语义；
- 当前完整application范围内的fresh-variable/postponed-argument constraint system与MSC → M16；M20的`_`部分实参沿用该solver；
- runtime generic dictionary、witness参数、反射式bound调用或共享generic body；M14仅实现单态化，后续只有在代码体积、动态加载或其他明确需求出现时再设计，不能作为缺失concrete信息的fallback；

### 来自 M15（设计预留）

- ~~GC-free release hook → M24~~（已完成设计：仅普通final class可声明受限release block；完整构造后设置ready位；collector在逻辑死亡对象真正reclaim前同步调用TypeDescriptor hook。无payload副本、异步queue、managed `this`、公开arm/disarm或对象复活；实现与验收范围见M24设计）。

### 来自 M22（设计预留）

- platform-native integer另行设计：决定`ISize`/`USize`与`IntPtr`/`UIntPtr`是否分立、data/code pointer表示资格及target-dependent const/layout规则，再逐项决定M22临时使用`Long`/`ULong`的Ptr raw/to/offset、`sizeOf`/`alignOf`、Array/String size/index及其他相关surface是否迁到相应native type；普通`Int`/`UInt`与`Long`/`ULong`自身的固定宽度不随target改变，迁移范围不能在本里程碑预判或静默扩大。

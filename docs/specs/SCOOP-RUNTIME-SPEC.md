# Scoop Runtime 规范

版本：0.2（草案）

配套文档：`SCOOP-SPEC.md`（语言规范）。本文引用其章节号。

## 1. 概述与范围

Runtime 是编译产物的支撑层，职责包括：

- 对象模型与内存布局（对象头、TypeDescriptor、装箱）；
- GC 与编译器生成代码之间的**契约**（safepoint、根集合、handle 表）；
- Scoop ABI FFI 的 runtime functions（第 4 章，本文重点）；
- 异常抛出与展开；
- 核心类型（String / Array / StringBuilder 等）的运行时后备实现；
- 进程启动、线程注册与终止。

明确**不在**本文范围：

- 第 3 章所定单代、STW、单线程 moving Immix 基线之外的替代 GC 算法及优化；
- 协程调度器、集合、IO 等标准库内容；
- 普通 outbound C ABI callee 的内部实现；spec 14.3 的 managed callback 是经 runtime gateway 重新进入 Scoop 的独立反向边界；
- 编译器 intrinsic（`@Intrinsic`，spec 13.1）由编译器生成实现，不经 runtime。

---

## 2. 对象模型与内存布局

### 2.1 引用对象

每个引用类型对象带有对象头 `ScoopObjectHeader`，其中包含指向 `TypeDescriptor` 的指针（类似 vpointer，spec 10.4）。对象头中其余字段（标记位、哈希缓存等）由 GC 实现决定。

### 2.2 TypeDescriptor

每个具体类型（含每个单态化实例，spec 3.2）有一份编译器生成的 `TypeDescriptor`，至少包含：

- 类型标识（`is` / `as` 检查用）；
- 稳定的 UTF-8 类型名（与 TypeDescriptor 同生命周期，用于未捕获异常等运行时诊断）；
- 实例大小与对齐；
- **递归引用扫描描述**（GC 扫描对象内部引用用）：普通节点记录相对当前值起点的引用字节偏移；sequence 节点把多个扫描作用于同一起点；array 节点记录元素 stride 与单个内联元素的子扫描。tagged enum允许完全不含managed ref的variant复用pure-value payload区；每个直接或间接含managed ref的variant拥有互不重叠的连续slot，构造时把inactive slot清零，因此所有ref-bearing variant中的ref leaf直接合并为固定偏移，不存在按tag分派的扫描节点。该描述可任意组合，覆盖struct / tuple / class / 装箱payload及含引用聚合数组元素；
- **enum 扫描不读取 tag**：tagged enum的所有潜在ref位置都位于ref-bearing variant的独占slot，按普通固定偏移检查；inactive独占slot必须为全0，pure-value共享区不进入扫描。niche表示的managed-ref enum整体是一个普通引用位置，`Ptr` / `FunPtr` niche不是managed root。没有出站引用的节点可用空指针表示。LIR meta 的`RefScan`提供该信息（见 impl spec 2.4）；
- 父类型信息（接口、父类）；
- 虚分派结构：内嵌 **vtable 指针**与 **itable 数组**（itable 以接口 TypeDescriptor 指针为键）。vtable只含实际需要virtual dispatch的class方法，slot从0开始且允许为空；`Any`没有方法或固定前缀。`ToString` / `Hash`及声明operator equals的interface均走普通itable；装箱值类型的表项指向this调整thunk（impl spec 2.9）。TypeDescriptor的类型名只用于诊断，runtime不得据此合成用户可见字符串、哈希或相等语义。

### 2.3 装箱

值类型装箱为堆对象：对象头 + 按值存储的 payload（spec 4.4.4）。拆箱取回 payload。装箱赋予对象identity但不赋予通用`==`：相等按表达式静态引用类型声明的成员operator equals分派；`Any`或未声明equals的interface不能使用`==`。TypeDescriptor没有通用结构相等入口。

所有 Scoop 方法 receiver 都按值传递（spec 3.3）：ref-type receiver 复制 managed ref value，因而仍指向同一对象；value-type receiver 复制完整值。装箱值经 interface 分派调用值类型实现时，box 只是 payload 的存储来源；dispatch thunk 用 payload 的值初始化 value-type `this`。thunk 可以在不可观察时借用 payload 地址作为 ABI 优化；若 unsafe/interior-mutable 路径能观察或修改存储，必须先复制 payload，不得把 box 内部地址暴露为 `this`。

### 2.4 `String` / `Array` 布局

- `String`：对象头 + 长度 + 内联字节数据（UTF-8，spec 11.4）。
- `Array<T>` / `MutableArray<T>`：对象头 + `size` + 对齐填充 + 内联元素区；元素区起点为 `alignUp(24, alignOf<T>())`。`T` 为值类型时元素不装箱且按 `sizeOf<T>()` stride 连续布局（满足 pack/align 约束，spec 10.1）。数组 TypeDescriptor 的扫描描述以元素 stride 重复执行 `T` 的递归子扫描，因此 `T` 可以是含引用或 tagged enum 的 struct / tuple；tagged enum元素的inactive variant slot同样保持全0。

### 2.5 `Option` 的 niche 表示

引用类型的全 0 机器字表示 `None`；`Ptr` / `FunPtr` 以 `_rawPointer == 0u` 表示 `None`（spec 7.4）。GC 扫描时必须识别 niche 编码，不得把全 0 当作有效引用追踪。

### 2.6 函数值与 closure

managed 函数值是普通引用对象，不是原生函数指针。每个 concrete closure 实例包含编译器控制的 invoke entry 与零个或多个不可变捕获字段；语言只允许捕获不可重新绑定的 binding，不存在 compiler-generated shared cell。closure、函数类型型变 adapter 与 suspend closure frame 都必须有各自的 TypeDescriptor 和完备的递归引用扫描描述。

- invoke entry、TypeDescriptor 及其他代码/metadata 指针不是 managed 引用，不进入 GC 扫描描述；captured value type按 concrete layout直接内联在 closure对象中，其中的引用必须按普通字段递归扫描；该内联存储没有独立对象头、identity或额外TypeDescriptor，不构成 boxing；
- concrete closure 的 TypeDescriptor 记录其 exact function type descriptor；function type descriptor 保留挂起性、参数与返回类型 identity。编译器可以按实际使用登记型变 bridge，运行期 `is` / `as` 不得仅把不同签名按同一个“closure”根类型处理；
- 无捕获 closure 可以由编译器放入静态只读对象或复用单例，但其对象头和 TypeDescriptor 仍须满足普通引用对象契约；
- closure 的分配、调用与回收不需要新增 runtime API，走现有 managed 分配、statepoint 与动态分派设施；
- 本节对象不得直接当作 `FunPtr` 交给原生代码。spec 13.10 的 `FunPtr` callback 是独立的 GC-free 原生地址；spec 14.3 的 GC-aware closure 回调则必须先通过 runtime 注册/保活协议。

---

## 3. GC 契约

参考实现由M9的单代非移动Immix和M13的多mutator STW演进而来；M15基线为单代、STW、单线程collector的moving Immix。以下是编译器与runtime共同遵守的长期契约；后续分代或parallel/concurrent实现可以替换算法，但不得破坏root、safepoint、native借用、pin与handle语义。M15首个runtime target为macOS/AArch64；其他target在拥有等价的精确frame/location adapter前不得退回保守扫描运行moving collector。

### 3.1 分配入口

分配分为两条通道：

- **managed 快速通道**：编译器生成的代码**不使用 handle**，直接取得裸指针。标准形态是 TLAB 碰撞指针（bump pointer）内联序列；成功后由不含safepoint、不取得heap/world锁的GC-leaf `scoop_runtime_finish_tlab_alloc`完成清零、对象头与object-start/精确allocation-size side metadata登记，缓冲区耗尽时才落入 slow path（`scoop_runtime_alloc_slow`，可能触发 GC）。编译器必须在 safepoint保留完整live root信息；M15 moving collector精确消费statepoint stack map并更新其location，生成代码在调用后只使用`gc.relocate`结果（spec 14.2）。managed 分配是最高频操作，其成本必须保持摊销 O(1)，不经 handle 表。
- **Scoop ABI native 通道**：外部实现没有 stack map；它可以直接借用传入的 managed ref，但在调用可能触发 GC 的 runtime入口前，必须先把仍需使用的引用登记为 native root slot（见 4.2）。需要把新对象长期带出 native frame时可使用 handle；需要稳定裸地址时使用 pinned allocation。

M13 起每个已attach线程持有独立TLAB；slow path在同步的heap元数据下从Immix free-line run或新block切出互不重叠区间。STW开始后全部TLAB失效，GC结束后各线程在下一次分配时重新refill。内联bump与上述GC-leaf finish helper共同构成fast path，并必须在返回前清零完整对象、初始化对象头，并以原子方式同时登记object-start与精确normalized allocation size，保证多mutator分配与构造中途safepoint都可安全扫描。只登记start而没有size不是合法的已发布对象状态。TLAB具体尺寸可调，但managed分配返回裸指针、摊销O(1)、不经handle表的契约不变。

### 3.2 safepoint 模型

- managed（编译器生成）代码：statepoint poll（spec 14.2 的 LLVM 策略）。函数入口及每条循环回边在LIR中已经是带完备live-root plan的显式poll，不由codegen补插；managed ref在LLVM中使用address space 1，普通poll/call的活跃ref leaf由statepoint/`gc.relocate`描述。含ref aggregate必须先拆为独立leaf，不能把aggregate alloca当作隐式stack region；
- managed `invoke`：LLVM当前异常边relocation不能作为语言实现基础。编译器在invoke前把normal/unwind后继仍活跃的ref leaf写入显式compiler root frame，两个后继都从slot reload并在继续控制流前pop；该invoke仍有statepoint frame record，但不得产生exceptional `gc.relocate`；
- outbound native transition：native-safe/native-borrowed调用前发布完备caller-root frame，machine call保留statepoint record但`gc-live`/relocate为空；冻结managed segment不扫描该record。返回值含ref时使用调用前已清零并登记的result storage，在native-borrowed返回后、leave可能park前写入，回到managed后连同其他live root只从slot reload；
- Scoop ABI FFI：函数体内**无** safepoint poll，GC 只能在其显式调用 runtime或回调 managed代码时于该入口内部发生；跨越这些入口的 direct ref必须位于 native root slot（spec 14.3）；
- C ABI callee不接收 managed ref，也不得直接调用 Scoop GC；M12 单 mutator实现可把它视为纯 GC leaf。M13启用多mutator后，outbound caller必须在调用前发布live roots并进入native-safe，使其他线程发起的STW GC无需等待一个可能阻塞的C调用。C代码若持有4.3注册所得的静态trampoline与cookie，可以经这个**独立反向边界**进入managed callback；这不使原C callee获得direct ref或Scoop ABI能力，外层caller roots继续由native-safe transition保活。

M15的macOS/AArch64 runtime只更新stack-resident managed roots。固定的LLVM 22.1 backend profile通过SelectionDAG默认spill与post-RA statepoint fixup保证每个GC base/derived location都是可写的8-byte `Indirect [SP/FP + offset]`；runtime不保存、unwind或改写register root。stackmap前三个constant location、deopt location与live-out不属于GC root pair；通用parser必须能区分这些区域，DarwinAArch64 profile只在GC root位置拒绝`Register`/`Direct`/constant等非约定形态。

### 3.3 根集合

- 栈根：managed caller在statepoint处的live root location、managed invoke显式compiler root frame、Scoop ABI调用点保持的caller roots，以及native callee显式登记的可更新root slot链。runtime以精确return PC查stack-map record并把每个location解析为可写slot；不存在保守栈扫描fallback；
- 全局根：`object` 单例等引用类型全局状态（全局 `var` 按 spec 13.6 必须 GC-free，不构成根）。LIR/codegen为每个可能含ref的managed global输出非可选递归scan描述，runtime在managed执行前登记其可写storage；
- stable/immortal对象：活跃的ABI exception buffer作为动态stable external object登记，其内部ref slot可更新；编译器生成的静态String等只读immortal object必须有精确地址/size/TD登记。M15只允许GC-free payload的只读immortal对象；未登记的heap外地址不能出现在managed slot；
- handle 表与 pinned 对象（`GcHandle` 保活其引用对象；pinned 对象作为根被扫描，见 3.4）。

单image ABI固定导出`ScoopManagedGlobalDescriptor scoop_image_managed_globals[]`、`u64 scoop_image_managed_global_count`、`ScoopImmortalObjectDescriptor scoop_image_immortal_objects[]`、`u64 scoop_image_immortal_object_count`。两个record分别为`{ void *writable_base; const u64 *scan; }`与`{ const void *object_start; u64 object_size; const TypeDescriptor *td; }`。count是唯一权威；count为零时数组仍含一个全零sentinel。runtime在GC heap初始化前验证并登记两张表；重复storage、重叠immortal range、TD/header不匹配、含managed出站引用的只读immortal object均为fatal metadata error。

### 3.4 保活机制（两级）

- **pin（对象头标志）**：保活且阻止移动。pin 标志位于对象头，`PinnedPtr.raw` 即对象地址（spec 14.1），pin/unpin 是 O(1) 的对象头读写，**不经 handle 表**。带 pin 标志的对象作为 GC 根被扫描（其出站引用必须被追踪），但自身不移动。
- **`GcHandle`（handle 表）**：保活，不阻止移动。handle 表是 runtime 私有结构；64位host使用带generation的`(generation, slot)` opaque编码，0保留为null niche，slot复用时generation递增，避免已释放handle错误命中新对象。GC 移动对象后负责更新表项。

只在一次 Scoop ABI 调用内、且不跨 safepoint使用的 direct ref不需要 pin/handle。需要稳定裸地址时优先 pin；需要长期保活且允许移动时使用 `GcHandle`；仅需跨 native callee内的 safepoint时优先使用 native root slot，避免改变对象移动属性或建立长期 handle。

### 3.5 线程

线程创建/销毁时向 GC 注册/注销。M13建立的多 mutator协议使用带单调GC epoch的合作式 stop-the-world handshake，并至少区分 managed、native-safe、native-borrowed、parked与collector状态：managed线程在入口/回边 poll或managed runtime入口发布平台定义的top managed anchor后停顿；C ABI outbound call发布 caller roots并冻结当前managed栈段后进入 native-safe，collector扫描已发布roots且无需等待其返回；持有 direct ref的 Scoop ABI native code属于 native-borrowed，只在登记 native roots的显式 runtime/managed入口或返回边界参与协调。每次managed/native重入都在LIFO transition链中划分managed栈段。M15只对当前活动managed段从anchor按精确stack map walk；冻结段完全由进入native前发布的caller roots表示，不扫描native-call statepoint location、其中的普通内存，也不跨native frame反向猜测。collector 不得在未握手的普通 native-borrowed指令之间移动对象或扫描仍在变化的native栈。进入协调点时，当前线程anchor、compiler caller-root frame与native root slot链必须可扫描、可更新。compiler caller-root entry由value storage地址和递归scan descriptor组成；tagged enum的固定ref偏移可直接扫描，因为inactive variant slot按spec 7.4恒为全0。含ref的Scoop ABI返回storage从push前的全零值开始登记，在native返回后、leave可能park前写入结果，并在恢复managed后reload。

进入managed、离开native-safe或离开native-borrowed必须与epoch发布形成无丢失握手：转换方发布mode后复查phase/epoch，观察到`Stopping`或epoch变化时在执行第一条managed指令前park；collector先release发布`Stopping`再以acquire读取线程mode。不得把线程按native-safe计为quiescent后又允许它未经复查进入managed。

foreign thread在进入任何 managed代码前必须 attach，建立 TLS thread state、栈边界、TLAB和空 native-root链；离开最后一个 managed callback后由拥有本次 attachment的入口 detach。重复使用的长期 foreign thread可以显式保持 attachment，但不得在 runtime shutdown后重新进入。

M15的platform bundle由object-image、OS thread/VM与architecture/ABI frame三个完备组件组成；通用runtime只经该bundle把top anchor与每一帧return PC/stack-map location转换为可写root slot，不读取Mach-O/x29等平台细节。macOS/AArch64基线强制generated/runtime frame pointer、禁止managed tail call，并要求return PC原值精确命中record；不得使用`PC-4`、最近函数、symbol或地址范围猜测。任何缺失record、越界location、未知DWARF register或当前target不支持的GC-root location kind都是fatal metadata错误；生产编译器应已在object验证阶段把这种情况报告为compiler/toolchain invariant failure，runtime检查只防御错误链接、损坏或非Scoop产物。每个由generated managed code直接调用且可park/collect的managed runtime symbol必须通过薄入口先发布`{ return_pc, callsite_sp, frame_pointer }` opaque anchor，再调用平台无关实现；runtime内部不得重入该入口并把C frame冒充managed frame。Scoop ABI native实现调用的是另一组native-borrowed入口：它验证已发布的caller/native roots并参与握手，但不捕获C caller或伪造managed anchor。

### 3.6 屏障

是否需要写/读屏障取决于 GC 算法；编译器侧预留插桩点（具体形式随 GC 方案确定）。M15的单代collector仍不消费card table，但多mutator对card的标记必须使用atomic monotonic store/RMW；多个线程写入同一个普通byte即使值都为1也不能视为无数据竞争。

### 3.7 moving collection与side metadata

M15 collector为选中的from-space对象建立arena外forwarding关系，把未pin live object复制到独立to-space，再通过统一可改写slot visitor更新全部root和heap字段。普通模式可按block占用率选择source，但只要存在可移动live object且to-space足够，一次显式full collection至少选择一个eligible source，不能退化为永久不移动的mark/sweep；stress模式必须移动全部未pin live object。pinned对象不移动但其出站ref仍更新；`GcHandle`只更新table entry，generation/slot identity不变。

block state、object-start、每个对象的精确normalized allocation size、line/free-run信息与forwarding均位于GC arena外。block header、free-block node或hole node不得存放在可能poison/保护的arena内。可变长String/Array及large object的复制长度直接读取allocation metadata，不能从TypeDescriptor fixed size、block span或payload内容反推。

collection在释放world前必须重新验证所有合法slot不再指向forwarded旧地址。stress mode在每次mutator-visible managed allocation前执行full moving collection；evacuation allocation不可递归触发collection。验证完成后poison旧副本；不再含live/pinned对象的source block整块`PROT_NONE`并永久quarantine，partial pinned block中的已搬span同样poison且在stress进程中不复用。

### 3.8 finalizer与资源释放

Scoop**永久不支持全功能GC finalizer**。不可达判定、mark/sweep、evacuation、旧副本poison或block quarantine都不得调用对象方法、lambda/closure或任意managed代码；对象不能在回收阶段取得`this`、重新发布自身、建立root/handle/pin或以其他形式复活。该限制不是当前实现缺口，也不能通过runtime内部开关、core专用能力或FFI旁路放宽。

非GC资源首先必须由程序通过显式`release`/`close`及`try/finally`管理。未来可以增加一个仅用于遗漏显式释放时兜底的**GC-free release hook**，但其语法、core API与落地里程碑尚未确定，并且必须满足以下长期边界：

- hook是静态、non-suspend、non-throw且GC-free的清理入口；它只可操作payload并调用经验证为`@NoGC`的native resource release primitive（例如`free`/`close`）。不得分配managed对象、触发GC、调用managed代码或callback，也不得操作root、`GcHandle`或pin；
- hook只能附着在具有唯一managed identity的`ref`对象上；struct、enum、tuple及其他值类型会按值复制，不能携带隐式release ownership。需要兜底的native handle值必须由引用型owner封装；
- hook不接收managed对象或`this`，只接收编译器验证为GC-free的typed release payload副本，例如raw native handle、整数与其他GC-free值。payload中直接或间接出现managed ref均为编译错误；
- collector在逻辑对象死亡时至多claim一次armed release记录，并在回收对象存储前把payload复制到runtime拥有的非GC内存；hook不通过旧对象地址读取字段；
- 显式资源释放必须能够原子地disarm对应记录；claim/disarm保证同一资源至多清理一次。moving只转移armed状态，from-space旧副本失效不表示逻辑对象死亡，不能触发hook；
- hook的执行时间、不同hook之间的顺序以及进程正常或异常退出时是否执行均不保证。runtime shutdown不遍历全部live/uncollected对象执行finalization pass；程序正确性、锁释放、事务完成和稀缺资源的及时回收不能依赖hook。

release policy在未来IR/runtime中必须是完备sum（概念上为`None | GcFreeRelease { payload layout, hook }`），不能用可缺失hook指针、类型名或对象字段形状让collector猜测。该受限机制不具备finalizer语义，也不得逐步扩展为finalizer。

---

## 4. Scoop ABI FFI runtime functions

对应 spec 第 14 章。以下固定 runtime 必须提供的功能与语义；M13 已定稿的 thread state、handle 与 callback token 编码继续按本章执行，M15增加精确root更新与moving语义；尚未列出的导出 C 函数签名在实现时由 runtime header 锁定。

### 4.1 对象分配

- runtime导出入口在类型和symbol上区分managed generated-code entry、Scoop ABI native-borrowed entry、runtime-only internal implementation与foreign-callback entry；不得由单一入口在运行期猜direct caller kind。以下本节分配API属于native-borrowed入口，managed TLAB slow path使用3.1所述独立managed入口；
- `scoop_runtime_alloc_pinned(type_desc, size) -> ScoopObjectHeader*`：**分配即固定**，直接返回裸指针。它用于确实要求稳定裸地址的 native / C ABI 场景，不是 direct-ref Scoop ABI 的默认通道。pin 标志在分配返回前已设置，因此即使分配过程触发了 GC，返回的指针也可以直接使用；其所有权与 `unpin` 时机必须由具体 API 契约明确（spec 14.1）。
- `scoop_runtime_alloc(type_desc, size) -> handle`：返回 GC handle（可移动），用于需要把新对象长期带出当前 native root frame的场景。
- FFI 代码没有 stack map，无法使用 managed 快速通道（见 3.1）。若新对象只在本次 native调用内使用，可把当前引用写入 native root slot；pinned 分配与 handle仍分别服务于稳定地址和长期保活。
- native-borrowed分配入口在第一个可能GC的动作前验证当前thread transition及root链。它触发collection时只扫描冻结managed segment的caller-root frame和native实现已登记的root slots，不捕获当前C return address，也不跨native frame反向unwind；返回后native代码从slot reload。

### 4.2 native root、pin 与 handle 操作

- runtime 提供 `push_native_roots(slots, count)` / `pop_native_roots()` 等价能力：把当前线程上一组 `void **` root slot按栈帧登记/移除。具体 C 结构可以内联携带 previous/count/slots，但必须是类型化 runtime API，不能依赖 C 栈保守扫描猜测。
- push/pop 本身不得分配、触发 GC或回调 managed代码；root frame严格 LIFO。collector 扫描 slot当前值，并在移动对象后写回新地址。native代码跨 safepoint后必须从 slot reload。
- direct ref只在无 safepoint的同步借用区间内可以作为普通 C pointer缓存；不得把该副本保存到 root frame之外、全局存储或调用返回之后。

- pin / unpin：直接读写对象头的 pin 标志（`scoop_runtime_pin(ptr)` / `scoop_runtime_unpin(ptr)`），O(1)。
- `scoop_runtime_pin_handle(handle) -> ptr`：把传入的（可移动）handle 解析为当前地址并固定，用于 FFI 收到 `GcHandle` 参数又需要裸指针的场景。
- handle 校验：runtime 必须校验generation、slot与live状态；非法或stale handle按4.4视为fatal runtime ABI error。
- Scoop 侧的 `pin` / `unpin` / `getGcHandle` / `releaseGcHandle`（spec 14.1）本身就是以 Scoop ABI 实现的 extern 函数，映射到上述能力。

### 4.3 回调 Scoop closure

- M13 提供 managed callback registration协议。概念入口为 `scoop_runtime_callback_register(closure, adapter, signature, mode) -> cookie`：注册函数按 Scoop ABI直接接收 ordinary、非 suspend closure，为其建立 `GcHandle`，并在runtime registry中创建 opaque token slot。64位host的GC-free cookie在目标ABI保证可往返且保持canonical的非零payload位内编码generation和slot，只做`uintptr_t`/`void *`往返、从不解引用；超出该位预算的slot/generation不得分配。slot复用递增generation，因而可检测stale/use-after-final-release而无需永久泄漏C heap tombstone。
- token slot至少保存closure handle、首个异常handle、typed adapter、静态signature descriptor、`Reusable`/`OneShot` mode、owner/active计数和完成/失败状态；C侧不得读取这些字段，也不得把cookie当地址解引用。
- 编译器为每个实际导出的 concrete函数类型生成 managed invoke adapter，并按`(C signature, context index)`生成/复用静态 C ABI trampoline。trampoline按真实 C签名收参，移除被token占用的context参数，把其余值写入 C-FFI-safe args/result storage，再调用 C-callable `scoop_runtime_callback_invoke(cookie, signature, args, result)`；runtime不能用未类型化可变参数直接猜 managed invoke ABI。
- `scoop_runtime_callback_invoke` 执行 attach-if-needed → enter managed → 从 handle取得 closure并登记为root → 调用 adapter → leave managed → detach-if-owned。closure调用期间使用普通 managed ABI、statepoint和异常处理，可以分配及触发 GC；跨调用保存的不是 closure裸指针，而是 token中的 handle。
- token提供 `retain` / `release` 等价能力并明确 ownership transfer。普通值复制不增加owner；owner与active lease都为零后撤销handles并回收slot，之后调用属于 ABI错误。`Reusable`调用只增减active lease，持久owner由unregister后的调用者释放；`OneShot`入口原子claim并把一份worker owner转为active worker lease，出口消费。需要在`join`侧观察完成/异常时，observer必须预先retain并持有到读取状态后最终release；创建失败路径释放所有尚未转移的ownership。
- callback adapter必须在返回 C前捕获所有 Scoop异常，物化成managed对象并以status/受管异常handle报告失败；异常不得展开穿越 trampoline/C frame。token以first-wins保存首个失败，trampoline按真实C返回类型返回全零值；由API-specific同步点后的observer决定重新抛出。invalid/stale cookie、signature不匹配、one-shot重复调用或shutdown后调用无法安全映射为任意C API错误，统一视为fatal runtime ABI error。
- 首版只支持原生API具有显式 `void *` context/user-data槽的 callback；静态 trampoline和token分别占据 function pointer与context。缺少context槽的API不能导出任意closure，只能使用 spec 13.10 的静态 `FunPtr`，直到后续实现动态 executable trampoline或有限slot registry。

### 4.4 错误处理

- 无法分配runtime/GC元数据、非法或stale handle、native-root LIFO破坏、非法thread transition、callback cookie/signature错误及shutdown后重新进入均为fatal runtime ABI error，打印稳定诊断后终止。普通callback抛出的 Scoop异常不属于runtime内部失败，按4.3转为token失败状态。

### 4.5 线程状态

- M12 的 Scoop ABI outbound调用不切换线程状态（不插 `enter_native` / `leave_native`，spec 14.2）；native callee仍属于当前已注册 managed thread。M13 多 mutator实现按 3.5 区分 C ABI native-safe与 Scoop ABI native-borrowed，runtime/managed入口读取该线程的 managed/native root frame链并完成 safepoint握手。
- foreign callback是反向边界：未注册线程必须先经 `scoop_runtime_attach_foreign_thread` 等价入口建立 thread state，再由 callback gateway进入 managed；detach只能由拥有attachment且已退出所有 managed frame/native root frame的代码执行。
- 边界转换必须可嵌套并按LIFO恢复previous mode；只使用一个`native_depth`不能区分native-safe/native-borrowed，也无法正确处理“managed → C → same-thread managed callback → C”的重入链。native-safe返回、native-borrowed返回和callback enter在切入managed前都必须检查当前GC epoch。

---

## 5. 异常

- 抛出入口 `scoop_rt_throw(obj)`（M8 起）：从对象头的 TypeDescriptor 读 `size`，调 `__cxa_allocate_exception(size)` 分配 ABI 异常缓冲并把对象内容拷贝进去，再对该缓冲调 `__cxa_throw(buffer, NULL, destructor)`（throw-by-value：catch 侧取得的是缓冲指针，拷贝即异常对象本尊）。`__cxa_throw` 会把异常头写到抛出指针紧前方，因此用户对象绝不可原地抛出。ABI 缓冲作为外部对象根登记；其 destructor 在异常生命周期结束时移除该根，原对象无需 pin。
- 栈展开机制为 landing pad + personality function（`scoop_eh_personality`，M8 最小实现委托 `__gxx_personality_v0`）。landing pad 先捕获 ABI record/raw pointer，再由普通 dispatch 块调 `__cxa_begin_catch` 取得异常对象；每条正常离开 handler 的路径必须调一次 `__cxa_end_catch`。handler 内的新异常及 `__cxa_rethrow` 先进入 cleanup chain：保存替代异常、结束当前 catch，再转交同函数外层 handler/cleanup；没有同函数外层时以 `resume` 继续传播，保证 begin/end 严格配对且重抛保持异常身份。
- 内置异常的抛出点：除零（`ArithmeticException`）、`as` 失败（`ClassCastException`）、`!!` 失败（`UnwrapException`）、数组越界等（spec 10.5、11.7）。
- M10 的 suspend handler 不允许把 `__cxa_begin_catch` 建立的原生 EH 状态跨挂起点保存。选中 catch 或进入可能挂起的 finally 前，生成代码调用 `scoop_rt_materialize_exception(caught)`：按 caught 的 TypeDescriptor 分配 managed 对象，保留新对象已初始化的 `td` / `gc_word`，只复制对象头之后的 payload。复制期间 ABI 缓冲仍登记为外部根。随后立即 `__cxa_end_catch`，catch local / pending exception 改指向 managed 副本。恢复失败时重新从该 managed 对象抛出，因此在源码语义上仍等价于异常发生在原挂起调用点。
- **边界规则**：异常不得穿越 C ABI frame（行为未定义）；能否穿越 Scoop ABI FFI frame 取决于实现（FFI 函数无 landing pad，穿越意味着跳过外部语言代码——初版建议禁止，行为定为终止进程）。

## 6. 核心类型的运行时后备

以 Scoop ABI FFI 函数形式实现；spec 14.4 的 `write(String)` 是不跨 safepoint直接借用 ref的最小范例，涉及分配的函数则按 4.2 登记 native roots：

- `String`：创建、拼接、内容比较、内容hash、长度、索引/切片；
- `Array` / `MutableArray`：分配（按 spec 10.1 的元素布局）、`size`、越界检查与抛异常、`toArray` / `toMutableArray` 的浅拷贝转换（spec 10.4）。转换入口显式接收编译器已选定的目标concrete application TypeDescriptor，以该descriptor分配并保留新对象头，只复制对象头之后的`size`、padding与inline elements；不得从来源对象、元素布局或类型名推断目标类型，也不得沿用来源descriptor；
- `StringBuilder`：`add` / `build`（spec 11.6）；
- 基本类型的具体`ToString` / `Hash` / operator equals后备（不提供`Any`或地址fallback）；
- 类型测试与装箱辅助：`is` / `as` 的 TypeDescriptor 比较、装箱/拆箱。

## 7. 启动、线程与终止

- 进程启动：初始化 GC 与 handle 表，注册主线程，调用 `main`；
- 线程：主线程启动时注册；runtime创建的线程及 foreign thread在首次进入 managed代码前 attach，在退出最后一个 managed入口且不再持有 runtime thread state时 detach（配合 3.5、4.3）；
- 终止：`main` 返回后runtime先进入`ShuttingDown`并拒绝新attach/registration；只有主线程以外无attachment、无活动callback且token ownership均已释放时才销毁GC状态。存在迟到线程/token时报告计数并终止，不在其仍可能进入时释放runtime。shutdown不运行GC finalizer，也不为未来release hook遍历全部live/uncollected对象；native resource仍须在正常控制流中显式释放。

## 8. 协程

`suspend` 的状态机变换、`CoroutineStep<T>`、frame 与各挂起点的 `Continuation<T>` adapter 全部由编译器生成（spec 8.2、11.9；impl spec 2.3）。这些实体都是普通 managed 对象/值：

- frame 与 continuation adapter 必须有普通 TypeDescriptor 和完备的递归引用扫描描述；frame 链由 GC 自然保活，不登记额外的 runtime root；
- continuation 的完成状态与 frame 的当前恢复状态存于 managed 对象字段。M10–M12 的最小实现是单线程协议；M13 把adapter claim/完成与frame `running/suspended/completed`转换升级为64位对齐原子状态机：winner以acq_rel CAS取得完成/驱动权，先写payload再release发布终态，读取方以acquire消费；等待短暂`Completing`状态的循环必须包含safepoint/backoff。已attach线程可安全恢复既有continuation，重复完成仍抛`IllegalStateException`。调度器、队列和恢复后在哪个线程继续执行仍由后续标准库规定；
- hidden continuation ABI 仅存在于编译器生成的 Scoop 托管调用之间。runtime 不提供 suspend FFI 入口、extern trampoline 或 callback wrapper；`@Extern` 与 `suspend` 的互斥，以及挂起函数声明引用不能在 `FunPtr` 上下文中解析为原生地址，由 HIR 保证（spec 8.2、13.4、13.10）；
- runtime 只提供第 5 章所述的 ABI 异常物化辅助，不参与状态分派、恢复、队列或线程切换；
- 调度器、事件循环与取消属于标准库。永不恢复的 continuation 只会按普通不可达对象被 GC 回收，runtime 不替它执行 cleanup / `finally`。

---

## 9. TBD 清单

仍待后续里程碑补充：

- macOS/AArch64以外target的精确frame/location adapter；分代/晋升、parallel/concurrent collector及相应屏障消费策略仍待后续；
- GC-free release hook的源码/API设计、typed payload描述、原子claim/disarm及非GC执行队列；全功能GC finalizer明确不在TBD中；
- runtime functions 的完整签名表与错误处理矩阵；
- 异常穿越 Scoop ABI frame 的最终规则。

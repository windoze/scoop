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

- GC 算法的具体实现（留待后续；第 3 章只定义它必须满足的契约）；
- 协程调度器、集合、IO 等标准库内容；
- C ABI 一侧（无 runtime 介入，spec 13.8）；
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
- **递归引用扫描描述**（GC 扫描对象内部引用用）：普通节点记录相对当前值起点的引用字节偏移；sequence 节点把多个扫描作用于同一起点；tagged-enum 节点记录 tag 的相对偏移及每个变体的子扫描；array 节点记录元素 stride 与单个内联元素的子扫描。扫描描述可任意组合，因此 struct / tuple / class / 装箱 payload 中嵌套的 tagged enum，以及含引用的聚合数组元素，均不会被压平成无条件引用偏移；
- **enum 的引用扫描按 tag 分派**：扫描 tagged enum 值时先按节点记录的偏移读 tag，再执行对应变体的递归子扫描。niche 表示的 enum（spec 7.4）整体就是一个引用，使用普通引用节点；没有出站引用的节点可用空指针表示。LIR meta 的 `RefScan` / `LayoutKind::Enum` 提供该信息（见 impl spec 2.4）；
- 父类型信息（接口、父类）；
- 虚分派结构：内嵌 **vtable 指针**与 **itable 数组**（itable 以接口 TypeDescriptor 指针为键）。`Any` 的 `equals` / `hashCode` / `toString` 是 vtable 的固定前三个槽位；装箱值类型的表项指向 this 调整 thunk（impl spec 2.9）。

### 2.3 装箱

值类型装箱为堆对象：对象头 + 按值存储的 payload（spec 4.4.4）。拆箱取回 payload。装箱对象的 `==` 仍为结构相等（经 TypeDescriptor 分发）。

所有 Scoop 方法 receiver 都按值传递（spec 3.3）：ref-type receiver 复制 managed ref value，因而仍指向同一对象；value-type receiver 复制完整值。装箱值经 interface 分派调用值类型实现时，box 只是 payload 的存储来源；dispatch thunk 用 payload 的值初始化 value-type `this`。thunk 可以在不可观察时借用 payload 地址作为 ABI 优化；若 unsafe/interior-mutable 路径能观察或修改存储，必须先复制 payload，不得把 box 内部地址暴露为 `this`。

### 2.4 `String` / `Array` 布局

- `String`：对象头 + 长度 + 内联字节数据（UTF-8，spec 11.4）。
- `Array<T>` / `MutableArray<T>`：对象头 + `size` + 内联元素区；`T` 为值类型时元素不装箱且连续布局（满足 pack/align 约束，spec 10.1）。数组 TypeDescriptor 的扫描描述以元素 stride 重复执行 `T` 的递归子扫描，因此 `T` 可以是含引用或 tagged enum 的 struct / tuple。

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

## 3. GC 契约（算法实现留待后续）

GC 算法未定时，以下契约先固定，编译器与 runtime 双方据此实现：

### 3.1 分配入口

分配分为两条通道：

- **managed 快速通道**：编译器生成的代码**不使用 handle**，直接取得裸指针。标准形态是 TLAB 碰撞指针（bump pointer）内联序列：线程本地缓冲区内移动分配指针即完成，缓冲区耗尽时落入 slow path（`scoop_runtime_alloc_slow`，可能触发 GC）。安全性由 statepoint 保证：GC 在 safepoint 移动对象后，由 statepoint rewrite 更新栈上的引用（spec 14.2）。managed 分配是最高频操作，其成本必须是摊销 O(1) 的几条内联指令，不经 handle 表。
- **Scoop ABI native 通道**：外部实现没有 stack map；它可以直接借用传入的 managed ref，但在调用可能触发 GC 的 runtime入口前，必须先把仍需使用的引用登记为 native root slot（见 4.2）。需要把新对象长期带出 native frame时可使用 handle；需要稳定裸地址时使用 pinned allocation。

TLAB 的有无、尺寸与 slow path 细节随 GC 方案确定；契约只要求：managed 分配返回裸指针、摊销 O(1)、不经 handle 表。

### 3.2 safepoint 模型

- managed（编译器生成）代码：statepoint poll（spec 14.2 的 LLVM 策略）；
- Scoop ABI FFI：函数体内**无** safepoint poll，GC 只能在其显式调用 runtime或回调 managed代码时于该入口内部发生；跨越这些入口的 direct ref必须位于 native root slot（spec 14.3）；
- C ABI callee不接收 managed ref，也不得调用 Scoop GC；M12 单 mutator实现可把它视为纯 GC leaf。启用多 mutator后，outbound caller仍必须在调用前发布 live roots并进入 native-safe状态，使其他线程发起的 STW GC无需等待一个可能阻塞的 C调用；callee本身仍不参与 GC。

### 3.3 根集合

- 栈根：statepoint stack map（managed caller）、Scoop ABI 调用点保持的 live roots，以及 native callee显式登记的可更新 root slot链；
- 全局根：`object` 单例等引用类型全局状态（全局 `var` 按 spec 13.6 必须 GC-free，不构成根）；
- handle 表与 pinned 对象（`GcHandle` 保活其引用对象；pinned 对象作为根被扫描，见 3.4）。

### 3.4 保活机制（两级）

- **pin（对象头标志）**：保活且阻止移动。pin 标志位于对象头，`PinnedPtr.raw` 即对象地址（spec 14.1），pin/unpin 是 O(1) 的对象头读写，**不经 handle 表**。带 pin 标志的对象作为 GC 根被扫描（其出站引用必须被追踪），但自身不移动。
- **`GcHandle`（handle 表）**：保活，不阻止移动。handle 表是 runtime 私有结构；handle 值是表项的 opaque 编码；GC 移动对象后负责更新表项。

只在一次 Scoop ABI 调用内、且不跨 safepoint使用的 direct ref不需要 pin/handle。需要稳定裸地址时优先 pin；需要长期保活且允许移动时使用 `GcHandle`；仅需跨 native callee内的 safepoint时优先使用 native root slot，避免改变对象移动属性或建立长期 handle。

### 3.5 线程

线程创建/销毁时向 GC 注册/注销。M13 的多 mutator基线使用 stop-the-world handshake，并至少区分 managed、native-safe与 native-borrowed状态：managed线程在入口/回边 poll或runtime入口停顿；C ABI outbound call发布 caller roots后可进入 native-safe，collector无需等待其返回；持有 direct ref的 Scoop ABI native code属于 native-borrowed，只在登记 native roots的显式 runtime/managed入口参与协调。collector 不得在未握手的普通 native-borrowed指令之间移动对象。进入协调点时，当前线程的 managed roots与 native root slot链必须可扫描、可更新。

foreign thread在进入任何 managed代码前必须 attach，建立 TLS thread state、栈边界、TLAB和空 native-root链；离开最后一个 managed callback后由拥有本次 attachment的入口 detach。重复使用的长期 foreign thread可以显式保持 attachment，但不得在 runtime shutdown后重新进入。

### 3.6 屏障

是否需要写/读屏障取决于 GC 算法；编译器侧预留插桩点（具体形式随 GC 方案确定）。

---

## 4. Scoop ABI FFI runtime functions

对应 spec 第 14 章。以下是 runtime 必须提供的功能图景，具体函数签名与编码在 GC 方案定型后细化。

### 4.1 对象分配

- `scoop_runtime_alloc_pinned(type_desc, size) -> ScoopObjectHeader*`：**分配即固定**，直接返回裸指针。它用于确实要求稳定裸地址的 native / C ABI 场景，不是 direct-ref Scoop ABI 的默认通道。pin 标志在分配返回前已设置，因此即使分配过程触发了 GC，返回的指针也可以直接使用；其所有权与 `unpin` 时机必须由具体 API 契约明确（spec 14.1）。
- `scoop_runtime_alloc(type_desc, size) -> handle`：返回 GC handle（可移动），用于需要把新对象长期带出当前 native root frame的场景。
- FFI 代码没有 stack map，无法使用 managed 快速通道（见 3.1）。若新对象只在本次 native调用内使用，可把当前引用写入 native root slot；pinned 分配与 handle仍分别服务于稳定地址和长期保活。

### 4.2 native root、pin 与 handle 操作

- runtime 提供 `push_native_roots(slots, count)` / `pop_native_roots()` 等价能力：把当前线程上一组 `void **` root slot按栈帧登记/移除。具体 C 结构可以内联携带 previous/count/slots，但必须是类型化 runtime API，不能依赖 C 栈保守扫描猜测。
- push/pop 本身不得分配、触发 GC或回调 managed代码；root frame严格 LIFO。collector 扫描 slot当前值，并在移动对象后写回新地址。native代码跨 safepoint后必须从 slot reload。
- direct ref只在无 safepoint的同步借用区间内可以作为普通 C pointer缓存；不得把该副本保存到 root frame之外、全局存储或调用返回之后。

- pin / unpin：直接读写对象头的 pin 标志（`scoop_runtime_pin(ptr)` / `scoop_runtime_unpin(ptr)`），O(1)。
- `scoop_runtime_pin_handle(handle) -> ptr`：把传入的（可移动）handle 解析为当前地址并固定，用于 FFI 收到 `GcHandle` 参数又需要裸指针的场景。
- handle 校验：runtime 应校验 handle 合法性；非法 handle 的行为由实现定义（spec 14.3）。
- Scoop 侧的 `pin` / `unpin` / `getGcHandle` / `releaseGcHandle`（spec 14.1）本身就是以 Scoop ABI 实现的 extern 函数，映射到上述能力。

### 4.3 回调 Scoop closure

- M13 提供 managed callback registration协议。概念入口为 `scoop_runtime_callback_register(closure, adapter, flags) -> ScoopCallbackToken*`：注册函数按 Scoop ABI直接接收 ordinary、非 suspend closure，为其建立 `GcHandle`，并在 native稳定内存中创建 opaque token。token至少保存 handle、typed adapter、函数类型/参数编组描述、引用计数和完成/失败状态；C侧不得读取这些字段。
- 编译器为每个实际导出的 concrete函数类型生成 managed invoke adapter和静态 C ABI trampoline。trampoline按真实 C签名收参，把值写入 C-FFI-safe args/result storage，再调用 C-callable `scoop_runtime_callback_invoke(token, args, result)`；runtime不能用未类型化可变参数直接猜 managed invoke ABI。
- `scoop_runtime_callback_invoke` 执行 attach-if-needed → enter managed → 从 handle取得 closure并登记为root → 调用 adapter → leave managed → detach-if-owned。closure调用期间使用普通 managed ABI、statepoint和异常处理，可以分配及触发 GC；跨调用保存的不是 closure裸指针，而是 token中的 handle。
- token提供 `retain` / `release` 等价能力并明确 ownership transfer。release到零后撤销handle并销毁token；之后调用属于 ABI错误。一次性回调（如 `pthread_create` entry）由worker消费一份 ownership；需要在`join`侧观察完成/异常时，observer必须另持一份ownership直到读取状态并最终release。创建失败路径释放所有尚未转移的ownership。
- callback adapter必须在返回 C前捕获所有 Scoop异常，`invoke`以 status和受管异常handle报告失败；异常不得展开穿越 trampoline/C frame。由API-specific wrapper决定终止、返回错误或在 `join`/完成观察点重新抛出。
- 首版只支持原生API具有显式 `void *` context/user-data槽的 callback；静态 trampoline和token分别占据 function pointer与context。缺少context槽的API不能导出任意closure，只能使用 spec 13.10 的静态 `FunPtr`，直到后续实现动态 executable trampoline或有限slot registry。

### 4.4 错误处理

- runtime 内部失败（分配失败、非法 handle 等）的处理方式——抛 Scoop 异常还是终止进程——需要逐函数规定，随 GC 方案一同细化。

### 4.5 线程状态

- M12 的 Scoop ABI outbound调用不切换线程状态（不插 `enter_native` / `leave_native`，spec 14.2）；native callee仍属于当前已注册 managed thread。M13 多 mutator实现按 3.5 区分 C ABI native-safe与 Scoop ABI native-borrowed，runtime/managed入口读取该线程的 managed/native root frame链并完成 safepoint握手。
- foreign callback是反向边界：未注册线程必须先经 `scoop_runtime_attach_foreign_thread` 等价入口建立 thread state，再由 callback gateway进入 managed；detach只能由拥有attachment且已退出所有 managed frame/native root frame的代码执行。

---

## 5. 异常

- 抛出入口 `scoop_rt_throw(obj)`（M8 起）：从对象头的 TypeDescriptor 读 `size`，调 `__cxa_allocate_exception(size)` 分配 ABI 异常缓冲并把对象内容拷贝进去，再对该缓冲调 `__cxa_throw(buffer, NULL, destructor)`（throw-by-value：catch 侧取得的是缓冲指针，拷贝即异常对象本尊）。`__cxa_throw` 会把异常头写到抛出指针紧前方，因此用户对象绝不可原地抛出。ABI 缓冲作为外部对象根登记；其 destructor 在异常生命周期结束时移除该根，原对象无需 pin。
- 栈展开机制为 landing pad + personality function（`scoop_eh_personality`，M8 最小实现委托 `__gxx_personality_v0`）。landing pad 先捕获 ABI record/raw pointer，再由普通 dispatch 块调 `__cxa_begin_catch` 取得异常对象；每条正常离开 handler 的路径必须调一次 `__cxa_end_catch`。handler 内的新异常及 `__cxa_rethrow` 先进入 cleanup chain：保存替代异常、结束当前 catch，再转交同函数外层 handler/cleanup；没有同函数外层时以 `resume` 继续传播，保证 begin/end 严格配对且重抛保持异常身份。
- 内置异常的抛出点：除零（`ArithmeticException`）、`as` 失败（`ClassCastException`）、`!!` 失败（`UnwrapException`）、数组越界等（spec 10.5、11.7）。
- M10 的 suspend handler 不允许把 `__cxa_begin_catch` 建立的原生 EH 状态跨挂起点保存。选中 catch 或进入可能挂起的 finally 前，生成代码调用 `scoop_rt_materialize_exception(caught)`：按 caught 的 TypeDescriptor 分配 managed 对象，保留新对象已初始化的 `td` / `gc_word`，只复制对象头之后的 payload。复制期间 ABI 缓冲仍登记为外部根。随后立即 `__cxa_end_catch`，catch local / pending exception 改指向 managed 副本。恢复失败时重新从该 managed 对象抛出，因此在源码语义上仍等价于异常发生在原挂起调用点。
- **边界规则**：异常不得穿越 C ABI frame（行为未定义）；能否穿越 Scoop ABI FFI frame 取决于实现（FFI 函数无 landing pad，穿越意味着跳过外部语言代码——初版建议禁止，行为定为终止进程）。

## 6. 核心类型的运行时后备

以 Scoop ABI FFI 函数形式实现；spec 14.4 的 `write(String)` 是不跨 safepoint直接借用 ref的最小范例，涉及分配的函数则按 4.2 登记 native roots：

- `String`：创建、拼接、比较、`hashCode`、长度、索引/切片；
- `Array` / `MutableArray`：分配（按 spec 10.1 的元素布局）、`size`、越界检查与抛异常、`toArray` / `toMutableArray` 的 memcpy 转换（spec 10.4）；
- `StringBuilder`：`add` / `build`（spec 11.6）；
- 基本类型的 `toString` / `hashCode` / `equals`；
- 类型测试与装箱辅助：`is` / `as` 的 TypeDescriptor 比较、装箱/拆箱。

## 7. 启动、线程与终止

- 进程启动：初始化 GC 与 handle 表，注册主线程，调用 `main`；
- 线程：主线程启动时注册；runtime创建的线程及 foreign thread在首次进入 managed代码前 attach，在退出最后一个 managed入口且不再持有 runtime thread state时 detach（配合 3.5、4.3）；
- 终止：`main` 返回后的清理（GC 关闭、线程汇合）由实现定。

## 8. 协程

`suspend` 的状态机变换、`CoroutineStep<T>`、frame 与各挂起点的 `Continuation<T>` adapter 全部由编译器生成（spec 8.2、11.9；impl spec 2.3）。这些实体都是普通 managed 对象/值：

- frame 与 continuation adapter 必须有普通 TypeDescriptor 和完备的递归引用扫描描述；frame 链由 GC 自然保活，不登记额外的 runtime root；
- continuation 的完成状态与 frame 的当前恢复状态存于 managed 对象字段。M10–M12 的最小实现是单线程协议；M13 为 foreign-thread callback把完成/恢复转换升级为带明确内存序的原子状态机，使已attach线程可以安全恢复既有continuation。调度器、队列和恢复后在哪个线程继续执行仍由后续标准库规定；
- hidden continuation ABI 仅存在于编译器生成的 Scoop 托管调用之间。runtime 不提供 suspend FFI 入口、extern trampoline 或 callback wrapper；`@Extern` 与 `suspend` 的互斥，以及挂起函数声明引用不能在 `FunPtr` 上下文中解析为原生地址，由 HIR 保证（spec 8.2、13.4、13.10）；
- runtime 只提供第 5 章所述的 ABI 异常物化辅助，不参与状态分派、恢复、队列或线程切换；
- 调度器、事件循环与取消属于标准库。永不恢复的 continuation 只会按普通不可达对象被 GC 回收，runtime 不替它执行 cleanup / `finally`。

---

## 9. TBD 清单

随 GC 方案定型后补充：

- GC 算法、分代/并发策略、屏障插桩形式；
- handle 的编码与校验细节；
- callback token的最终二进制编码、status与异常handle错误矩阵（概念协议见4.3，M13设计时定稿）；
- runtime functions 的完整签名表与错误处理矩阵；
- 异常穿越 Scoop ABI frame 的最终规则。

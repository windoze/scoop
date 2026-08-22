# Scoop Runtime 规范

版本：0.1（草案）

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
- 实例大小与对齐；
- **引用字段位图/描述**（GC 扫描对象内部引用用）；
- **enum 的引用扫描按 tag 分派**：enum 的 TypeDescriptor 携带 per-variant 的引用偏移表（LIR meta 的 `LayoutKind::Enum`，见 impl spec 2.4）；扫描 enum 值时先读 tag，再按对应变体的偏移表扫描。niche 表示的 enum（spec 7.4）整体就是一个引用，无表；
- 父类型信息（接口、父类）；
- 虚分派结构：内嵌 **vtable 指针**与 **itable 数组**（itable 以接口 TypeDescriptor 指针为键）。`Any` 的 `equals` / `hashCode` / `toString` 是 vtable 的固定前三个槽位；装箱值类型的表项指向 this 调整 thunk（impl spec 2.9）。

### 2.3 装箱

值类型装箱为堆对象：对象头 + 按值存储的 payload（spec 4.4.4）。拆箱取回 payload。装箱对象的 `==` 仍为结构相等（经 TypeDescriptor 分发）。

### 2.4 `String` / `Array` 布局

- `String`：对象头 + 长度 + 内联字节数据（UTF-8，spec 11.4）。
- `Array<T>` / `MutableArray<T>`：对象头 + `size` + 内联元素区；`T` 为值类型时元素不装箱且连续布局（满足 pack/align 约束，spec 10.1）。

### 2.5 `Option` 的 niche 表示

引用类型的全 0 机器字表示 `None`；`Ptr` / `FunPtr` 以 `_rawPointer == 0u` 表示 `None`（spec 7.4）。GC 扫描时必须识别 niche 编码，不得把全 0 当作有效引用追踪。

---

## 3. GC 契约（算法实现留待后续）

GC 算法未定时，以下契约先固定，编译器与 runtime 双方据此实现：

### 3.1 分配入口

分配分为两条通道：

- **managed 快速通道**：编译器生成的代码**不使用 handle**，直接取得裸指针。标准形态是 TLAB 碰撞指针（bump pointer）内联序列：线程本地缓冲区内移动分配指针即完成，缓冲区耗尽时落入 slow path（`scoop_runtime_alloc_slow`，可能触发 GC）。安全性由 statepoint 保证：GC 在 safepoint 移动对象后，由 statepoint rewrite 更新栈上的引用（spec 14.2）。managed 分配是最高频操作，其成本必须是摊销 O(1) 的几条内联指令，不经 handle 表。
- **FFI 通道**：`scoop_runtime_alloc` 返回 handle（见 4.1），供没有 stack map 的 Scoop ABI FFI 代码使用。FFI 调用本身已是重操作，handle 表开销相对可忽略。

TLAB 的有无、尺寸与 slow path 细节随 GC 方案确定；契约只要求：managed 分配返回裸指针、摊销 O(1)、不经 handle 表。

### 3.2 safepoint 模型

- managed（编译器生成）代码：statepoint poll（spec 14.2 的 LLVM 策略）；
- Scoop ABI FFI：函数体内**无** safepoint poll，GC 只能在其显式调用 runtime 时于 runtime 内部发生（spec 14.3）；
- C ABI：与 GC 完全无交互（spec 13.8）。

### 3.3 根集合

- 栈根：statepoint stack map（managed）与 conservative root spill（Scoop ABI 调用点）；
- 全局根：`object` 单例等引用类型全局状态（全局 `var` 按 spec 13.6 必须 GC-free，不构成根）；
- handle 表与 pinned 对象（`GcHandle` 保活其引用对象；pinned 对象作为根被扫描，见 3.4）。

### 3.4 保活机制（两级）

- **pin（对象头标志）**：保活且阻止移动。pin 标志位于对象头，`PinnedPtr.raw` 即对象地址（spec 14.1），pin/unpin 是 O(1) 的对象头读写，**不经 handle 表**。带 pin 标志的对象作为 GC 根被扫描（其出站引用必须被追踪），但自身不移动。
- **`GcHandle`（handle 表）**：保活，不阻止移动。handle 表是 runtime 私有结构；handle 值是表项的 opaque 编码；GC 移动对象后负责更新表项。

短期持有一律优先 pin（更省）；只有需要长期保活且允许移动时才用 `GcHandle`。

### 3.5 线程

线程创建/销毁时向 GC 注册/注销。并行 GC 的线程协调方式（stop-the-world / handshake）由实现决定，但契约是：Scoop ABI FFI 函数在其指令流中不停顿（spec 14.3），GC 的线程协调点只能落在 runtime 调用内部。

### 3.6 屏障

是否需要写/读屏障取决于 GC 算法；编译器侧预留插桩点（具体形式随 GC 方案确定）。

---

## 4. Scoop ABI FFI runtime functions

对应 spec 第 14 章。以下是 runtime 必须提供的功能图景，具体函数签名与编码在 GC 方案定型后细化。

### 4.1 对象分配

- `scoop_runtime_alloc_pinned(type_desc, size) -> ScoopObjectHeader*`：**分配即固定**，直接返回裸指针——这是 FFI 分配的推荐形态（spec 14.4 的示例即此模式）。pin 标志在分配返回前已设置，因此即使分配过程触发了 GC，返回的指针也可以直接使用，并可以直接交还 Scoop 侧（由 `unpin` 收尾，spec 14.1）。
- `scoop_runtime_alloc(type_desc, size) -> handle`：返回 GC handle（可移动），用于需要长期保活且允许移动的场景。
- FFI 代码没有 stack map，无法使用 managed 快速通道（见 3.1）；但 pinned 分配同样不经 handle 表，开销与 managed 慢路径相当。

### 4.2 pin 与 handle 操作

- pin / unpin：直接读写对象头的 pin 标志（`scoop_runtime_pin(ptr)` / `scoop_runtime_unpin(ptr)`），O(1)。
- `scoop_runtime_pin_handle(handle) -> ptr`：把传入的（可移动）handle 解析为当前地址并固定，用于 FFI 收到 `GcHandle` 参数又需要裸指针的场景。
- handle 校验：runtime 应校验 handle 合法性；非法 handle 的行为由实现定义（spec 14.3）。
- Scoop 侧的 `pin` / `unpin` / `getGcHandle` / `releaseGcHandle`（spec 14.1）本身就是以 Scoop ABI 实现的 extern 函数，映射到上述能力。

### 4.3 回调 Scoop closure

- runtime 提供从 FFI 代码回调 Scoop closure 的能力（spec 14.3）：closure 是 managed 代码，其中 GC 重新生效。
- 需要的功能：注册 closure（以其 handle 保活）、从 C 侧发起调用的 trampoline 入口、调用结束后的状态恢复、closure 注册解除。
- 具体 API 形态（trampoline 的创建/销毁、参数编组）在 runtime 实现时定义。

### 4.4 错误处理

- runtime 内部失败（分配失败、非法 handle 等）的处理方式——抛 Scoop 异常还是终止进程——需要逐函数规定，随 GC 方案一同细化。

### 4.5 线程状态

- Scoop ABI 调用不切换线程状态（不插 `enter_native` / `leave_native`，spec 14.2）；runtime 函数内部（alloc、回调）自身是 GC 感知代码，自行维护所需状态。

---

## 5. 异常

- 抛出入口（如 `scoop_throw`）与栈展开机制（landing pad / personality function，具体方案由实现定）。
- 内置异常的抛出点：除零（`ArithmeticException`）、`as` 失败（`ClassCastException`）、`!!` 失败（`UnwrapException`）、数组越界等（spec 10.5、11.7）。
- **边界规则**：异常不得穿越 C ABI frame（行为未定义）；能否穿越 Scoop ABI FFI frame 取决于实现（FFI 函数无 landing pad，穿越意味着跳过外部语言代码——初版建议禁止，行为定为终止进程）。

## 6. 核心类型的运行时后备

以 Scoop ABI FFI 函数形式实现（spec 14.4 的 `scoop_concat_string` 即范例）：

- `String`：创建、拼接、比较、`hashCode`、长度、索引/切片；
- `Array` / `MutableArray`：分配（按 spec 10.1 的元素布局）、`size`、越界检查与抛异常、`toArray` / `toMutableArray` 的 memcpy 转换（spec 10.4）；
- `StringBuilder`：`add` / `build`（spec 11.6）；
- 基本类型的 `toString` / `hashCode` / `equals`；
- 类型测试与装箱辅助：`is` / `as` 的 TypeDescriptor 比较、装箱/拆箱。

## 7. 启动、线程与终止

- 进程启动：初始化 GC 与 handle 表，注册主线程，调用 `main`；
- 线程：创建时注册到 GC（配合 3.5），退出时注销；
- 终止：`main` 返回后的清理（GC 关闭、线程汇合）由实现定。

## 8. 协程

`suspend` 的状态机变换由编译器完成（spec 8.2），`Continuation` 是普通对象；runtime 无需专门的协程设施。调度器属于标准库。

---

## 9. TBD 清单

随 GC 方案定型后补充：

- GC 算法、分代/并发策略、屏障插桩形式；
- handle 的编码与校验细节；
- 回调 trampoline 的 API；
- runtime functions 的完整签名表与错误处理矩阵；
- 异常穿越 Scoop ABI frame 的最终规则。

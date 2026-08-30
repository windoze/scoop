# M9 设计：真 GC（Immix 核心 + statepoint）

版本：0.2（实现同步）

对应 `docs/ROADMAP.md` 的 M9。目标：用真正的 GC 替换 always-leak——打开 statepoint、落地 Immix 核心（块/行结构、bump 分配、标记-区域回收）与 pin/handle 设施（runtime spec 第 3、4 章）。**分代（nursery/remembered set/晋升）与 evacuation/defrag 不在本里程碑**（见 5.1 与第 6 章）。

## 0. 范围说明（分阶段的理由）

完整"Immix 分代式"包含三件大事：精确栈根、Immix 堆组织与回收、分代+写屏障+晋升。本里程碑交付 Immix 回收闭环与 statepoint / stackmap 产出通道；runtime 精确消费 stackmap、分代、evacuation / defrag 与并行协调转入 backlog（第 6 章）。关键约束（ROADMAP）：M1–M8 的全部 fixture 必须在真 GC 下原样通过。

- **不移动对象**（v1 无 evacuation/copying）→ 栈根不需要更新。codegen 已产出 statepoint / stackmap；runtime 当前使用经过堆对象起点校验的保守栈扫描，精确消费 stackmap 是明确的替换点（见 3.2）；
- **对象头从 8B 扩为 16B**（`td` + `gc_word`）：GC 标记位与 pin 标志的载体（runtime spec 2.1"对象头其余字段由 GC 实现决定"的落地）。这是本里程碑最大的结构性涟漪（所有堆布局偏移 +8，见 3.1）。

## 1. 语言与库子集

无新语法。新增 core 库设施（spec 14.1 的 M9 形态）：

```
// sysroot/lib/scoop.core/src/gc.scoop
struct PinHandle<T>(val raw: UInt)
struct GcHandle<T>(val raw: UInt)

@Intrinsic("rt_pin")        fun <T> pin(v: T): PinHandle<T>
@Intrinsic("rt_unpin")      fun <T> unpin(h: PinHandle<T>): T
@Intrinsic("rt_get_handle") fun <T> getGcHandle(v: T): GcHandle<T>
@Intrinsic("rt_release_handle") fun <T> releaseGcHandle(h: GcHandle<T>): T
```

- `UInt64`/`UInt` 类型本里程碑一并落地（spec 11.2 已有定义；当前只有 Int——加一个 `UInt`（= `UInt64`）基本类型即可，其余定宽类型进 backlog）；
- 四个函数 M9 走 `@Intrinsic`（spec 的 `@Unsafe` 形态等 M11 的 FFI 注解族，见 5.2）；T 实际只接受引用类型（spec 14.1 的 `T : ref` 约束在 M12 上界落地前按"引用类型才合法"诊断实现）；
- 测试钩子：`scoop_rt_gc_collect()`（强制回收）与 `scoop_rt_gc_stats()`（分配/存活计数），经 `@Intrinsic("rt_gc_collect")`/`@Intrinsic("rt_gc_stats")` 暴露给 core，供 fixture 断言回收行为（见 5.3）。

## 2. runtime（C，新增 `runtime/src/gc.c`）

### 2.1 堆组织（Immix 核心）

- **block = 32KB，line = 128B**：堆按 block 向 OS 申请（`aligned_alloc`/`mmap`），block 内按 line 切分；
- **bump 分配**：当前 block 的 cursor 推进；每线程（v1 单线程）一个 TLAB（当前 block + cursor），分配即指针推进 + 写对象头（`td` + 清零的 `gc_word`）；
- **free-line 复用**：回收后把完全空闲的 line 记入 block 的 free-line list，后续分配优先复用（Immix 的 line 级碎片处理）；
- **大对象**（> line 的一半）：独占 block；
- **慢路径**：TLAB 耗尽 → 取新 block / free-line；堆占用超阈值（v1 固定 16MB 或按上次回收后存活量的 2 倍）→ 触发回收。

### 2.2 回收（标记-区域，单代，不移动）

- **根**：(a) 栈根——v1 保守扫描并用堆对象起点校验，精确 stackmap 通道已由 codegen 产出（见 3.2）；(b) runtime 侧注册的全局引用与在途 ABI 异常缓冲（见 3.4）；(c) handle 表（GcHandle）；(d) pinned 对象（header pin 位）。
- **标记**：从根出发按 TD 的递归描述精确扫描对象内部——普通节点按相对偏移追踪，sequence 组合多个子扫描，tagged enum 按 tag 选择变体子扫描，数组按 stride 对每个内联元素执行元素子扫描。该结构覆盖 tagged enum 嵌入 class / struct / tuple 与含引用的聚合数组元素。mark 位写在 `gc_word`（block/line 侧表亦可，v1 用对象头）。
- **区域回收**：逐 block 检查——无存活对象的 block 归还 OS；有存活但含空闲 line 的 block 把空闲 line 入 free-line list。**不移动对象**（v1 不做 evacuation）。
- **终结行为**：always-leak 的语义改变是用户可观察的——fixture 不依赖泄漏语义（M1–M8 全部通过即可验证）。

### 2.3 pin / handle（runtime spec 3.4、第 4 章）

- **pin**：`gc_word` 的 pin 位；pinned 对象作为根被扫描但不被回收；O(1)；
- **GcHandle**：runtime 的全局 handle 表（增长数组），表项 = 对象指针；handle 值 = 表项索引 +1（0 保留给 niche）；回收时**不**更新表项（v1 不移动，表项稳定；移动式回收落地时改为更新表项，注释注明）；
- handle 校验：索引在表范围内且未释放，否则 abort（runtime spec 4.2）。

## 3. 各 stage 设计

### 3.1 LIR / codegen

- **对象头 16B 化**（最大的机械变更）：`HeapLoad` / `HeapStore`、class 构造、装箱 payload、数组对象（`{td, size, elems}` → `{td, gc_word, size, elems}`）、`scoop_rt_box`、`scoop_rt_alloc` 的写头代码、全部布局表——统一改为 `{td, gc_word, ...}`，字段区从偏移 16 起；TD 的 `size` 与递归扫描描述由 lir-lower 集中计算；
- **扫描描述发射**：LIR `RefScan` 保留普通引用、sequence、tagged enum 与数组元素扫描；codegen 递归发射常量描述树，runtime 用同一解释器扫描对象、装箱 payload 与数组元素；
- **statepoint 打开**（spec 14.2 的 managed 部分）：
  - codegen 给每个函数设置 GC strategy（inkwell `set_gc("statepoint-example")`，M0 spike 已验证），发射前跑 `rewrite-statepoints-for-gc` pass（调用点自动 statepoint 化）；
  - safepoint poll：函数入口与回边（while/for 循环头）插入 `gc.safepoint` poll（M0 spike 验证过的另一形态；poll 做成 runtime 的空操作符号 `scoop_rt_safepoint`，回收请求时经它握手）；
  - `.o` 的 `__llvm_stackmaps` section 由 LLVM 自动产出；
- **写屏障插桩点**（为分代预留，spec 3.6）：`HeapStore`/`ArraySet`（含数组元素 store）的 store 之后插入 card 标记（`card_table[addr >> 9] = 1`，runtime 提供卡片表；v1 的回收忽略卡片）。

### 3.2 runtime 的栈扫描

- codegen 给函数设置 `statepoint-example` GC strategy、执行 `rewrite-statepoints-for-gc`，目标文件已含 `__llvm_stackmaps`；
- runtime v1 在单线程回收时用 `setjmp` 溢出寄存器，并保守扫描当前帧到启动时记录的栈顶；每个候选值必须通过 arena、block 与对象起点三级校验后才会标记，保证内存安全但可能短暂过度保活；
- 后续精确实现解析 stackmap，并以返回地址定位各帧的指针槽；移动式回收前必须完成该替换。

### 3.3 driver / 链接

无变化（runtime 仍是单个 C 库；`mmap`/`aligned_alloc` 不需要新链接参数）。

### 3.4 异常对象在 GC 下的保活（M8 遗留）

`scoop_rt_throw` 的 ABI 缓冲拷贝不受 GC 管理，但其内容（异常对象拷贝及其引用）必须保活：拷贝完成后把缓冲登记为外部对象根；传给 `__cxa_throw` 的 destructor 在 ABI 异常生命周期结束时移除该根。缓冲本身由 C++ ABI 释放，原对象无需 pin。生成代码以 `begin_catch` / `end_catch` 和 cleanup landing pad 的结构化配对保证 destructor 可在最后一个 handler 结束后及时执行。

## 4. 测试计划

- **runtime C 测试**：分配/回收闭环、free-line 复用、大对象、pin / handle、handle 校验 abort，以及 plain / sequence / tagged-enum / 聚合数组的递归扫描；
- **fixture（Scoop 级）**：所有既有 fixture 在真 GC 下通过；`m5-arrays/recursive-reference-scans.scoop` 覆盖动态字符串、聚合数组元素与内嵌 tagged enum 的组合；
- **golden / codegen 测试**：对象头 16B 布局、statepoint 与 stackmap、写屏障，以及递归扫描常量树的 LLVM 发射。

## 5. 临时决策（及退役里程碑）

1. **单代、不移动、无 evacuation**：分代（nursery/晋升/remembered set 消费卡片表）、Immix 的 defrag evacuation、并行/并发回收协调 → 全部进 backlog（届时卡片表与 statepoint 通道已就位）。
2. **`pin` 等四函数走 `@Intrinsic`**：M11 转 spec 14.1 的 `@Unsafe` 普通函数/FFI 形态；`gcCollect`/`gcStats` 是测试专用 intrinsic，不进 spec（标注为内部设施）。
3. **回收触发阈值**：v1 固定值；自适应阈值随后续。
4. **保守栈根是 v1 过渡方案**：精确消费 stackmap 在移动式回收前完成；当前不绑定单一 CPU 架构。
5. **statepoint GC strategy 用 `statepoint-example`**：这是我们自己的 GC 策略注册名（LLVM 内置名，M0 已验证）；若后续需要自定义策略名（`scoop`）需向 LLVM 注册，当前不需要。

## 6. 明确不做（进 backlog）

- 分代（nursery、晋升、remembered set 消费卡片表、代间引用检查）；
- evacuation / defragmentation（Immix 的碎片整理）；
- 并行/并发回收与 STW 协调（线程注册/握手）；
- runtime 精确解析 statepoint stackmap 并扫描栈根（含所需的平台寄存器/帧支持）；
- `scoop.std` 的 GC 调优 API；

# M9 设计：真 GC（Immix 核心 + statepoint）

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M9。目标：用真正的 GC 替换 always-leak——打开 statepoint、落地 Immix 核心（块/行结构、bump 分配、标记-区域回收）与 pin/handle 设施（runtime spec 第 3、4 章）。**分代（nursery/remembered set/晋升）与 evacuation/defrag 不在本里程碑**（见 5.1 与第 6 章）。

## 0. 范围说明（分阶段的理由）

完整"Immix 分代式"包含三件大事：statepoint 精确栈根、Immix 堆组织与回收、分代+写屏障+晋升。本里程碑交付前两个加一个可用的回收闭环（单代、不移动），分代/evacuation/defrag/并行协调转入 backlog（第 6 章）。关键约束（ROADMAP）：M1–M8 的全部 fixture 必须在真 GC 下原样通过。

- **不移动对象**（v1 无 evacuation/copying）→ 栈根不需要更新，但**找到**栈根仍需精确——statepoint/stackmap 通道按 spec 14.2 落地（这也是为将来移动式回收铺路）；
- **对象头从 8B 扩为 16B**（`td` + `gc_word`）：GC 标记位与 pin 标志的载体（runtime spec 2.1"对象头其余字段由 GC 实现决定"的落地）。这是本里程碑最大的结构性涟漪（所有堆布局偏移 +8，见 3.1）。

## 1. 语言与库子集

无新语法。新增 core 库设施（spec 14.1 的 M9 形态）：

```
// sysroot/lib/scoop.core/src/gc.scoop
struct PinHandle<T>(val raw: UInt64)
struct GcHandle<T>(val raw: UInt64)

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

- **根**：(a) 栈根——statepoint stackmap 精确扫描（见 3.2）；(b) 全局根——runtime 侧注册的全局引用（v1 主要是 thrown-exceptions 列表，见 3.4）；(c) handle 表（GcHandle）；(d) pinned 对象（header pin 位）。
- **标记**：从根出发按 TD 精确扫描——Plain 布局用 `ref_offsets`、enum 按 tag 选 per-variant 表（runtime spec 2.2 的 M4 契约）、数组按 `element_is_ref` 扫整个元素区。mark 位写在 `gc_word`（block/line 侧表亦可，v1 用对象头）。
- **区域回收**：逐 block 检查——无存活对象的 block 归还 OS；有存活但含空闲 line 的 block 把空闲 line 入 free-line list。**不移动对象**（v1 不做 evacuation）。
- **终结行为**：always-leak 的语义改变是用户可观察的——fixture 不依赖泄漏语义（M1–M8 全部通过即可验证）。

### 2.3 pin / handle（runtime spec 3.4、第 4 章）

- **pin**：`gc_word` 的 pin 位；pinned 对象作为根被扫描但不被回收；O(1)；
- **GcHandle**：runtime 的全局 handle 表（增长数组），表项 = 对象指针；handle 值 = 表项索引 +1（0 保留给 niche）；回收时**不**更新表项（v1 不移动，表项稳定；移动式回收落地时改为更新表项，注释注明）；
- handle 校验：索引在表范围内且未释放，否则 abort（runtime spec 4.2）。

## 3. 各 stage 设计

### 3.1 LIR / codegen

- **对象头 16B 化**（最大的机械变更）：`ExtractValue` 堆读约定、HeapStore、class 构造、装箱 payload、数组对象（`{td, size, elems}` → `{td, gc_word, size, elems}`）、`scoop_rt_box`、`scoop_rt_alloc` 的写头代码、全部布局表——统一改为 `{td, gc_word, ...}`，字段区从偏移 16 起；TD 的 `size`/`ref_offsets` 同步重算（lir-lower 的布局计算集中修改）；
- **statepoint 打开**（spec 14.2 的 managed 部分）：
  - codegen 给每个函数设置 GC strategy（inkwell `set_gc("statepoint-example")`，M0 spike 已验证），发射前跑 `rewrite-statepoints-for-gc` pass（调用点自动 statepoint 化）；
  - safepoint poll：函数入口与回边（while/for 循环头）插入 `gc.safepoint` poll（M0 spike 验证过的另一形态；poll 做成 runtime 的空操作符号 `scoop_rt_safepoint`，回收请求时经它握手）；
  - `.o` 的 `__llvm_stackmaps` section 由 LLVM 自动产出；
- **写屏障插桩点**（为分代预留，spec 3.6）：`HeapStore`/`ArraySet`（含数组元素 store）的 store 之后插入 card 标记（`card_table[addr >> 9] = 1`，runtime 提供卡片表；v1 的回收忽略卡片）。

### 3.2 runtime 的栈扫描

- 解析 `__llvm_stackmaps`（进程启动时从自身 Mach-O 读 section；格式见 LLVM StackMap 文档，M0 spike 已验证其存在性）；
- 回收时暂停当前线程（v1 单线程 = 回收线程即被停线程），沿帧链（frame pointer）逐帧取返回地址 → 查 stackmap 得本帧的指针槽位 → 标记；
- **register root 用平台汇编**（impl spec 技术路线）：帧链遍历与寄存器快照用少量 aarch64/x86_64 汇编（v1 仅 aarch64，x86_64 进 backlog）。

### 3.3 driver / 链接

无变化（runtime 仍是单个 C 库；`mmap`/`aligned_alloc` 不需要新链接参数）。

### 3.4 异常对象在 GC 下的保活（M8 遗留）

`scoop_rt_throw` 的 ABI 缓冲拷贝不受 GC 管理，但其内容（异常对象拷贝及其引用）必须保活：v1 简化为 **`scoop_rt_throw` 在拷贝完成后对原对象 pin**（对象头 pin 位），并把拷贝缓冲登记到 runtime 的 thrown-exceptions 列表（全局根）；列表项在每次回收末尾尝试清除（无人引用的缓冲由 C++ ABI 自行释放——v1 只做根登记不做主动释放，注释注明这是已知的保守近似，见第 6 章）。

## 4. 测试计划

- **runtime C 测试**：分配/回收闭环（造垃圾 → `scoop_rt_gc_collect` → stats 显示存活回落）、free-line 复用、大对象路径、pin 后不被回收、handle 保活与释放、handle 校验 abort；
- **fixture（scoop 级）**：`tests/fixtures/m9-gc/`——分配循环（大量临时字符串/数组/对象，中间插 `gcCollect()` 并断言 `gcStats()` 的存活数）、pin/handle 的 scoop 层使用、异常抛出后继续分配、所有既有 fixture 原样通过（真 GC 下的全量回归是主验收）；
- **golden dump**：对象头 16B 的布局、statepoint 插入（函数 GC 属性）、写屏障形状。

## 5. 临时决策（及退役里程碑）

1. **单代、不移动、无 evacuation**：分代（nursery/晋升/remembered set 消费卡片表）、Immix 的 defrag evacuation、并行/并发回收协调 → 全部进 backlog（届时卡片表与 statepoint 通道已就位）。
2. **`pin` 等四函数走 `@Intrinsic`**：M11 转 spec 14.1 的 `@Unsafe` 普通函数/FFI 形态；`gcCollect`/`gcStats` 是测试专用 intrinsic，不进 spec（标注为内部设施）。
3. **回收触发阈值**：v1 固定值；自适应阈值随后续。
4. **aarch64 only**（栈扫描汇编）：x86_64 进 backlog。
5. **statepoint GC strategy 用 `statepoint-example`**：这是我们自己的 GC 策略注册名（LLVM 内置名，M0 已验证）；若后续需要自定义策略名（`scoop`）需向 LLVM 注册，当前不需要。

## 6. 明确不做（进 backlog）

- 分代（nursery、晋升、remembered set 消费卡片表、代间引用检查）；
- evacuation / defragmentation（Immix 的碎片整理）；
- 并行/并发回收与 STW 协调（线程注册/握手）；
- x86_64 栈扫描汇编；
- `scoop.std` 的 GC 调优 API；
- 异常缓冲的精确释放（v1 为保守近似，见 3.4）。

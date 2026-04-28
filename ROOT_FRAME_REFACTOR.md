# Root Frame / Root Map 重构设计

> 生成时间：2026-04-29  
> 目的：为后续 root map 抽象与 explicit root frame 落地提供统一设计说明，明确编译器与 runtime 的职责边界；并确认本轮 refactor 之后，explicit root frame 将成为新的默认 explicit mode，而 LLVM stackmap 退为未来可选优化方案。

## 1. 背景

当前 moving GC 的 managed roots 主要依赖两类来源：

- LLVM statepoint + stackmap 产出的 roots metadata
- `enter_native` / `leave_native` 使用的 `native_roots` slot 指针数组

现状里的关键约束已经比较明确：

- runtime 最终真正消费的是一串**可写回的 `void** slot`**
- moving GC 只支持更新这些 slot，不支持直接读写 register roots
- 编译器当前只能通过 LLVM 选项、pass 顺序、保守 spill/writeback 等手段，尽量避免 LLVM 在 stackmap roots 中留下 register location

这条路线的问题是：

1. LLVM stackmap 仍然可能包含 register location，或者需要依赖更复杂的机器层 fixup 才能成立。
2. 当前 GC 不支持读写 register root，因此一旦 managed root 留在寄存器里，就可能在 compaction 后变成 stale value。
3. 即使 runtime 只接受 stack/FP-based spill slots，编译器也还需要与 LLVM statepoint 的 `gc-live` / `gc.relocate` 合同保持一致，心智复杂度较高。

因此，本次重构的目标不是从设计上否认 stackmap，而是引入一个更稳定、更可控的 correctness-first 路线：`explicit root frame`。

并且需要明确：

- **本轮 refactor 完成后，默认 explicit mode 将切到 explicit root frame**
- **在这个默认 explicit mode 中，不使用，也不生成 LLVM stackmap**
- stackmap 仅作为未来潜在的可选优化模式保留

## 2. 设计目标

### 2.1 主目标

- 引入一个统一的 `root map` 抽象，同时兼容：
  - 当前 LLVM stackmap 实现
  - 新的 explicit root frame 实现
- 让现有 GC 通过统一的 `root map` 接口扫描和更新 managed roots。
- 把 explicit root frame 作为新的默认 explicit mode 实现，用来规避 LLVM stackmap 里可能出现 register location 的问题。
- 明确默认 explicit mode 的实现策略：
  - 不使用 LLVM stackmap 作为 managed roots source of truth
  - 不生成 LLVM stackmap section / records
- 把 LLVM stackmap 保留为未来可选优化方案，而不是默认路径。
- 通过不生成 `.llvm_stackmaps` / `__llvm_stackmaps` section 及其相关 metadata，争取获得一定的 object / binary size 收益。

### 2.2 explicit root frame 的定位

`explicit root frame` 可以视为：

- 一种 **precise shadow stack**
- 或一种 **自定义的、精简版的 LLVM stackmap**

它与传统 shadow stack 的关键区别是：

- 传统 shadow stack 常只保存一组运行时值
- 本设计中的 explicit root frame 由：
  - 每 activation 的 frame object
  - 每函数的常量 descriptor
  共同定义出一组稳定的 root home slots

运行时虽然不再动态保存 slot 地址数组，但仍然可以通过：

- `frame base + descriptor offsets`

恢复出每个 `void** slot`，从而在 moving GC 后直接更新 `*slot`，并让后续代码从该 slot reload 新地址。

### 2.3 非目标

- explicit root frame **本身**并不能解决 register root 问题。
- 它的作用是保证“所有跨 safepoint 的 managed root 都有一个稳定、可写回的 home slot，并可被 root map 看到”。
- 真正消除 stale register / stale SSA 的关键，仍然是编译器 contract：
  - safepoint 之后必须从 root frame 对应的 slot reload ref pointer
  - safepoint 之前的 GC ref SSA / register 值在语义上不再可信

### 2.4 重构范围

这次重构允许做较大范围的工程调整，但边界应保持清晰：

- 主要改动集中在：
  - LLVM codegen
  - runtime / GC roots 枚举与更新路径
  - explicit mode 对应的 runtime ABI / lowering contract
- 尽量不改动：
  - 语言表面语义
  - HIR / MIR 的核心语义模型
  - 非 GC roots 相关的中端表示

换句话说，这次重构的复杂度主要集中在 codegen/runtime 一侧，而不是语言前中端本身。

## 3. 设计原则

### 3.1 统一抽象应围绕 `void** slot`，而不是 stackmap record

stackmap 的自然入口是：

- `return_address -> record -> locations -> roots slots`

explicit root frame 的自然入口是：

- `thread-local frame chain -> roots slots`

两者真正的公共部分不是 record lookup，而是：

- **枚举一串可读写的 `void** slot`**

因此，root map 抽象的核心能力应是：

- 遍历当前线程 / 指定线程的 managed root slots
- 把每个 root 以 `void** slot` 的形式交给 GC visitor

这样：

- mark 阶段可读取 `*slot`
- moving / compaction 阶段可直接原地更新 `*slot`
- verify-roots 也能复用同一条遍历路径

### 3.2 correctness-first，优化 second

本次重构先追求：

- roots 一定可见
- roots 一定可更新
- safepoint 后一定可 reload

而不是先追求：

- 每个 safepoint 只扫描 live subset
- 最小扫描开销
- 完全依赖 LLVM statepoint metadata

stackmap 的 live-set 精度优势是后续优化点，不应阻塞 correctness 路线。

### 3.3 不做“半 stackmap、半 explicit”的细粒度混搭

第一阶段应避免：

- 同一个函数一部分 roots 走 explicit frame，另一部分 roots 仍依赖 stackmap
- safepoint 前后同时依赖 `gc.relocate` 和 explicit frame reload

原因是这会显著增加编译器与 runtime 的推理复杂度。

更合理的模式是：

- **explicit mode**：managed frame roots 统一走 explicit root frame
- **stackmap mode**：managed frame roots 统一走 stackmap

必要时可以按编译单元 / 函数级策略切换，但同一条 lowering 主线内不混用两套 source of truth。

本轮 refactor 的明确决策是：

- 默认 explicit mode = explicit root frame mode
- 默认 explicit mode 下不使用 stackmap，也不生成 stackmap

## 4. Root Map 抽象

### 4.1 概念边界

`root map` 是对“managed frame roots 来源”的统一抽象。

它不负责：

- 决定哪些值在语言语义上是 live
- 直接解决 register root 的 SSA 生命周期问题

它负责：

- 把当前线程（或被 GC 暂停的线程）中的 managed roots 暴露为一串 `void** slot`

### 4.2 抽象后的两种实现

#### A. Stackmap root map

- 定位：未来可选优化实现，不属于本轮默认 explicit mode
- 输入：stack walking ctx / frame `(sp, ra, fp)`
- 过程：
  - unwind walk
  - stackmap registry lookup
  - record locations 解释成 `void** slot`
- 优点：
  - 每个 safepoint 理论上可以只枚举 live roots
  - 扫描开销更小
- 风险：
  - 依赖 LLVM stackmap / statepoint 合同
  - 可能遇到 register location
  - runtime 目前不支持 register root 读写

#### B. Explicit root frame root map

- 定位：本轮 refactor 之后的默认 explicit mode 实现
- 输入：TLS 上的 explicit frame chain
- 过程：
  - 从 TLS top 开始沿 `prev` 链遍历每个 managed activation
  - 读取该 activation 对应的函数级 frame descriptor
  - 依据 descriptor 中的固定 offsets 计算每个 `void** slot`
- 优点：
  - 不依赖 stackmap record 解释
  - 不需要把 roots 暴露成 register location
  - 不需要在每次 activation 时动态构造 slot 指针数组
  - 和当前 GC 的 `void** slot` 语义天然一致
- 代价：
  - 可能扫描比 stackmap 更多的 slot
  - 需要编译器先为每个函数规划固定 frame layout
  - 需要编译器保证 slot 初始化 / 清空 / safepoint reload contract

### 4.3 `native_roots` 的位置

当前 `native_roots` 本质上也是：

- 一个 `void**` 指针数组

因此可以把它视为 root map 世界里的一个现有特例：

- stackmap mode 下：`native_roots` 仍用于 `InNative` call-site roots
- explicit mode 下：`native_roots` 可以继续保留给 native 过渡期的临时 roots

但对于“更高层 managed caller frames”的 roots，explicit mode 不再需要依赖 `enter_native` 时捕获 unwind ctx 去回找 stackmap frames。

默认 explicit mode 里仍可继续保留 `native_roots`：

- 它服务的是 native 过渡边界
- 不是为了继续维持 managed frames 的 stackmap roots 枚举

## 5. Explicit Root Frame 设计

### 5.1 总体形态

每个 managed 函数在 explicit mode 下都拥有两部分相关结构：

1. **每函数常量 frame descriptor**
2. **每 activation 栈上 frame object**

其中：

- frame descriptor 描述“这个函数的 roots 布局”
- frame object 承载“这次 activation 当前的 roots 值与链表链接关系”

这两者的职责不同：

- descriptor 是**函数级常量**
- frame object 是**activation 级动态对象**

在 descriptor-based 设计中：

- 不再为每个 activation 动态构造 `void**` slot 指针数组
- 而是让 GC 通过“frame base + 常量 offsets”计算出每个 `void** slot`

这意味着 explicit root frame 的 source of truth 变成：

- frame object 中的固定 root home fields

而不是：

- 一组运行时临时收集出来的 slot 地址数组

### 5.2 推荐布局

推荐的逻辑布局如下：

```c
typedef struct ScoopRootFrameDesc {
  uint32_t slot_count;
  const uint32_t *slot_offsets;
} ScoopRootFrameDesc;

typedef struct ScoopRootFrameHeader {
  struct ScoopRootFrameHeader *prev;
  const ScoopRootFrameDesc *desc;
} ScoopRootFrameHeader;

typedef struct FooExplicitFrame {
  ScoopRootFrameHeader hdr; // 必须放在首字段
  void *root_0;
  void *root_1;
  void *root_2;
} FooExplicitFrame;
```

并为每个函数生成常量 descriptor：

```c
static const uint32_t foo_root_offsets[] = {
  offsetof(FooExplicitFrame, root_0),
  offsetof(FooExplicitFrame, root_1),
  offsetof(FooExplicitFrame, root_2),
};

static const ScoopRootFrameDesc foo_root_desc = {
  .slot_count = 3,
  .slot_offsets = foo_root_offsets,
};
```

其中：

- `slot_offsets[i]` 记录的是：
  - **相对于 explicit frame object 基址的固定偏移**
- `hdr.prev`：前一帧 activation
- `hdr.desc`：当前 activation 对应的函数级 descriptor
- `root_i`：这次 activation 的固定 root home slot

说明：

- 这里的 offset 必须相对于**我们自己定义的 explicit frame object**
- 不能定义成相对于“机器意义上的 SP/FP/函数栈底”的偏移
- GC 最终仍然通过 `frame_base + offset` 得到 `void** slot`
- 若 `slot_count == 0`，实现上可以选择：
  - 不为该函数建立 explicit frame
  - 或保留统一路径，建立一个 `slot_count = 0` 且 `slot_offsets = NULL` 的空 descriptor / 空 frame

第一阶段更建议允许“无 roots 函数跳过 frame 构造”，把空 frame 作为实现细节可选项，而不是语义要求。

相比“slot 指针数组版”的实现，这个 descriptor-based 布局的关键好处是：

- per-activation 不再需要把 `root_slots[i] = &home_slot_i` 逐个写到数组里
- 栈上不再需要额外的 `void**[N]` 指针数组 payload
- 二进制里主要新增的是“每函数一份 descriptor”，而不是“每 activation 反复构造数组”的代码

### 5.2.1 explicit frame 不是“函数所有 locals 的大 struct”

需要明确：

- explicit frame 的职责是承载 **managed root home slots**
- 它不是“把函数里所有 ordinary locals 打包成一个大 struct”
- 它更接近一层 **GC shadow layout**

因此：

- frame layout 不应按“源码里有几个局部变量”来定义
- frame layout 应按“有几个需要进入 root map 的 stable home slots”来定义
- `Int` / `Bool` / 纯 value-only aggregate / 不跨 safepoint 的临时值，并不因为 explicit frame 的存在而必须进入 frame descriptor

这也意味着：

- explicit frame 的布局，与值类型本身的 canonical 内存布局是两回事
- `struct` / `tuple` / `enum` 自己的字段顺序、padding、alignment、调用 ABI 表示，不应因为 explicit root frame 而改变
- explicit frame 只是额外增加在函数 activation 上的一块 GC-visible 辅助存储

例如，一个函数中若只有：

```scoop
struct Named(val name: String, val score: Int)
val named: Named
```

并且 `named` 会跨某个 safepoint 存活，那么：

- root frame 最小只需要为 `named.name` 准备一个 home slot
- `named.score` 不是 GC root，不需要进入 descriptor
- 是否还保留一个 ordinary local `Named named` 副本，是 lowering 选择，不是 explicit frame 的语义要求

### 5.2.2 包含 ref 的复合值按 leaf root slots 展开

对于“包含 ref 的复合值类型”，explicit frame 的规划粒度应是：

- **按可写回的 ref leaf slot 建模**
- 而不是“每个 aggregate local 对应 frame 里的一个大字段”

例如：

```scoop
struct Named(val name: String, val score: Int)
struct Outer(val left: Named, val tag: String, val n: Int)
```

若 `outer: Outer` 跨 safepoint 存活，则更合理的逻辑 frame layout 是：

```c
typedef struct FooExplicitFrame {
  ScoopRootFrameHeader hdr;
  void *outer_left_name;
  void *outer_tag;
} FooExplicitFrame;
```

而不是：

```c
typedef struct FooExplicitFrame {
  ScoopRootFrameHeader hdr;
  Outer outer;
} FooExplicitFrame;
```

换句话说：

- descriptor 记录的是 leaf ref home slot 的 offset
- 非 ref 字段不进入 root frame descriptor
- nested aggregate 中的 ref 字段也应被展开成各自独立的 stable home slot

这条规则与 runtime 的 `void** slot` 语义更一致：

- GC 真正需要的是“一串可读写的 ref slot”
- 而不是“一个 aggregate 变量对象本体”

### 5.3 frame object 的生命周期与布局约束

每个 activation 的 explicit frame object 必须是：

- **entry-block alloca**
- 生命周期覆盖整个 activation

原因：

- 只有这样它的地址才稳定
- 只有这样 GC 才能在任意深层 callee 触发的 safepoint 中仍看到上层 caller frame 的 roots
- 只有这样 descriptor 中的 offsets 才能始终映射到同一个 frame object 布局

这与当前 codegen 的局部变量模型是一致的：

- locals 本来就统一使用栈上 alloca 承载
- `create_entry_alloca_raw` 已经在 entry block 放置 alloca

另外需要一个明确的布局约束：

- `ScoopRootFrameHeader` 必须是 explicit frame object 的首字段

这样 runtime 在只拿到 `ScoopRootFrameHeader*` 时，就可以把它直接视为：

- 当前 activation 的 frame base

从而用 `base + slot_offsets[i]` 计算 slot 地址。

### 5.4 链接到 TLS

每个线程维护一个 TLS root frame top：

- `__scoop_explicit_root_frame_top`

函数进入时：

1. 读取 TLS top
2. 为当前函数选定的 frame object 做 entry-block alloca
3. 初始化 frame object 中的 root home fields 为 `NULL`
4. 写入 `frame.hdr.desc = &foo_root_desc`
5. 写入 `frame.hdr.prev = tls_top`
6. TLS top = `&frame.hdr`

函数退出时：

1. TLS top = `frame.hdr.prev`

因此：

- 所有 managed activation 组成一条精确的 per-thread frame chain
- GC 扫描时无需 unwind 即可枚举 managed frame roots

## 6. 编译器 Contract

explicit root frame 的成败主要取决于编译器 contract，而不是 frame 结构本身。

### 6.1 每个跨 safepoint 的 GC ref 都必须有 stable home slot

编译器必须保证：

- 任何可能跨 safepoint 存活的 GC ref，都有一个稳定的 home slot
- 这些 home slots 位于当前函数的 explicit frame object 中
- 当前函数的 frame descriptor 为这些 home slots 提供固定 offsets

这意味着 explicit mode 不应依赖：

- “某个值大概率还留在 SSA / register 里”
- “LLVM 之后会帮我把它 spill 成可更新的 location”

而应明确要求：

- home slot 先于 safepoint 已存在，并且属于显式 frame layout 的固定字段
- safepoint 期间 GC 只更新 home slot

### 6.2 safepoint 是 GC ref 的 clobber 边界

这是最关键的 contract。

对任何 GC ref 而言：

- safepoint 之前的 SSA / register 值，在 safepoint 之后都不应再被认为是可信值
- safepoint 之后的所有继续使用，都必须从 home slot reload

也就是说：

- explicit root frame 并不直接“修寄存器”
- 它修的是“真实可更新的 source of truth”

然后由编译器强制：

- post-safepoint 只从这个 source of truth reload

### 6.3 descriptor 存的是 slot offset，frame 里存的是对象值

descriptor-based explicit frame 的关键约定是：

必须满足：

- descriptor 记录的是 `offsetof(FrameTy, root_i)`
- frame object 中的 `root_i` 字段本身就是 stable home slot
- GC 通过 `frame_base + offset` 得到 `void** slot`
- 代码在 safepoint 后重新从 `frame.root_i` reload ref value

不能改成：

- descriptor 直接依赖机器意义上的 `SP/FP/函数栈底`
- 或把 roots 分散在任意 alloca 上，再试图用“不稳定的栈偏移”回推它们

否则：

- 正确性会重新依赖目标 ABI / backend frame layout
- 我们就失去了“由编译器自己定义 stable frame object 布局”的主要收益

还需要进一步明确一个容易混淆的边界：

- 这里说的“frame 里存的是对象值”，指的是 **root home slot 中存放对象头指针值**
- 它不等于“frame 必须原样承载整个 aggregate local 的物理布局”

例如 `Named { name: String, score: Int }`：

- root frame 里需要的是 `name` 对应的 home slot
- `score` 不是 GC root，不属于 descriptor 管理范围
- 即使编译器额外保留一个 ordinary `Named` 栈副本，该副本也不是 GC update 的 source of truth

### 6.3.1 旧 aggregate 副本不是 post-safepoint source of truth

对于“包含 ref 字段的值类型副本”，需要明确：

- safepoint 之前存在的 ordinary aggregate 副本，在 safepoint 之后不应默认继续被视为可信
- moving GC 只保证更新 home slot 本身，不保证同步更新任意旧 aggregate 副本中的 ref 字段镜像

因此：

- 若一个 ordinary local `Named named` 在 safepoint 前持有 `{ name = p, score = 42 }`
- safepoint 期间若 `name` 对应对象搬迁到 `p'`
- 则 GC 只承诺把 root frame 中的 `named_name` slot 更新为 `p'`
- `named.name` 这个旧镜像若未显式刷新，可能仍然是 stale 的 `p`

所以 post-safepoint contract 应明确为：

- 继续使用 ref 字段时，必须从对应 home slot reload
- 不得把旧 aggregate 副本中的 ref 字段当作 source of truth
- 非 ref 字段仍可继续来自 ordinary local / SSA

### 6.3.2 aggregate 的复制、传参与重组

上面的约束并不意味着“包含 ref 的 aggregate 以后不能按值复制/传参”。

真正的规则是：

- **不能盲目复制一个可能已经 stale 的旧 aggregate 副本**
- 但可以复制一个“已经按最新 home slots 刷新过”的 aggregate 值

因此，在 safepoint 之后若需要：

- 把 `Named` 按值传给 callee
- 把 `Named` 写入另一个栈槽
- 把 `Named` 作为返回值 / payload / 临时 aggregate 继续传播

更稳妥的 lowering contract 应是：

1. 从 root frame 的 home slots reload 所有相关 ref 字段
2. 从 ordinary local / SSA 读取其余非 ref 字段
3. 用这些最新字段值重组出一个 fresh aggregate
4. 再按目标 ABI 做直接传值、间接传址、store、或必要时的 `memcpy`

也就是说：

- “对 fresh aggregate 做复制”是允许的
- “对 pre-safepoint 的旧 aggregate 副本盲 `memcpy`”是不允许依赖其正确性的
- 这条规则同样适用于 direct call arg、indirect arg、sret result、effect/continuation payload 等路径

### 6.3.3 SSA promotion / mem2reg 的允许边界

在默认 explicit mode 不再依赖 stackmap/statepoint roots metadata 之后：

- 编译器不必再因为“stackmap record 可能产出 register location”而一刀切压制所有 promotion
- 但 promotion 的边界仍必须围绕“谁是 managed roots 的 source of truth”来划定

更准确地说，应该把函数内存储分成三类：

#### A. 明确禁止 promotion 的槽位

以下对象应默认视为 **must-remain-materialized stack storage**：

- explicit frame object 本体
- explicit frame 中的所有 root home fields
- 任何地址会暴露给 TLS / runtime / callee，且正确性依赖其 stable address 的 stack slot
- 任何被选作 post-safepoint reload source-of-truth 的 spill slot / frame field

这些槽位的共同特点是：

- GC 或 lowering contract 需要它们在栈上有稳定地址
- 它们不能退化成“只剩 SSA / register 语义”

#### B. 优先可考虑 promotion 的槽位

以下对象更适合作为 explicit root frame 落地后的首批 mem2reg 候选：

- 不含 GC refs 的标量 locals
- 不含 GC refs、且地址不逃逸的 aggregate locals
- 不跨 safepoint 的短生命周期 temporaries
- 仅参与 pre-safepoint 计算、不会作为 post-safepoint source-of-truth 的中间值

这些槽位即使被 promotion：

- 也不会改变 root map 可见的 managed root 集合
- 也不会让 moving GC 失去唯一可写回点

#### C. 需要单独审计后再决定的槽位

以下对象不应在第一阶段直接放开：

- 含 ref 的 aggregate locals，即使它们不是最终 source-of-truth，也可能在复制/传参路径上与 post-safepoint 重组发生耦合
- indirect arg / sret / return alloca 这类按地址传递的 ABI scratch 槽位
- effect / continuation / state-machine lowering 生成的辅助槽位
- 任何 address-taken local，尤其是地址被传给 helper/runtime/callee 的路径

对这类槽位，需要逐条证明：

- promotion 不会让 post-safepoint reload 消失
- promotion 不会把 fresh aggregate 重组重新折叠成 stale 副本
- promotion 不会破坏间接调用 / 返回 ABI 对 stable address 的期望

换句话说：

- explicit root frame 允许我们重新引入 mem2reg
- 但这个“重新引入”必须是 **按 storage class 分层放开**
- 而不是把所有 alloca 一次性重新交给 LLVM promotion

#### D. promotion 之后仍必须保持的语义不变量

无论是否启用 mem2reg，以下 contract 都不能退化：

1. managed root 的唯一 source-of-truth 仍是 root home slot。
2. root home slot 在 root live 区间内始终有稳定地址。
3. root 变 live 后，对应 home slot 的写入必须先于相关 safepoint。
4. post-safepoint 的 managed ref 使用必须来自 home slot reload。
5. fresh aggregate 的重组不得被优化回 pre-safepoint 的 stale 副本。

### 6.4 固定长度 frame 不等于固定 live set

每个函数的 frame layout 与 descriptor 长度可以是编译期固定的，但这不表示所有 root fields 在整个 activation 中一直 live。

因此编译器必须保证：

- 所有 root home fields 在函数入口先初始化为 `NULL`
- 当某个 field 当前不承载有效 GC ref 时，field 值必须为 `NULL`
- dead / inactive root 不能把旧对象地址长期残留在 field 里

这点很重要，因为 explicit mode 下 GC 会在任意 safepoint 扫描整帧：

- stackmap 模式是“按 safepoint live subset 扫”
- explicit mode 是“按 activation frame superset 扫”

如果 dead slot 不清空，就会退化为“安全但过度保活”，甚至在未初始化时直接变成错误 root。

### 6.5 临时 / 编译器内部 roots 也必须纳入 contract

不仅源码级 local 需要 home slot，编译器生成的 GC-sensitive 临时值也需要。

典型例子包括：

- hidden sret result roots
- deferred spill roots
- 当前 `extra_gc_root_slots` 一类的额外内部槽位
- call lowering / effect lowering 过程中跨 safepoint 的临时 GC 值

在 explicit mode 下，这些临时 roots 不能继续依赖“运行时动态 register/unregister 某个 root slot id”。

更稳妥的方向是：

- 把这些内部 roots 也映射到函数 frame object 的固定 fields 中
- 或在 lowering 阶段把它们规范成 frame layout 里的稳定 home slots

### 6.6 只纳入 stack-backed roots，不纳入 heap-backed 槽位

root frame 只应该表示：

- 当前 activation 的 explicit frame object 中那些可写回 root home fields

不应该把以下内容也塞进 explicit frame：

- effect frame / continuation frame 等已经位于 traced heap 中的字段
- heap object 自身的引用字段

原因：

- heap-backed 槽位本来就会在 heap tracing / runtime object fixup 中被更新
- 如果再把“调用前旧值的镜像”塞进 explicit frame，容易形成双重 source of truth，甚至把旧值写回覆盖新值

## 7. Runtime / GC Contract

### 7.1 GC 只关心 `void** slot`

无论 root 来源是：

- stackmap
- explicit root frame
- native_roots

GC 最终看到的接口都应统一为：

- visitor(`void** slot`)

这样当前的 mark / update / verify 逻辑都可以复用。

### 7.2 explicit mode 的 roots 枚举

在 explicit mode 下：

- managed frames roots：来自 TLS explicit frame chain
- native 过渡 roots：来自 `native_roots`（如仍需要）
- pinned / handles / global roots：沿用现有逻辑

这意味着：

- 对 managed frames 的 roots 枚举，不再依赖 unwind ctx + stackmap registry lookup
- 对 moving GC 的 roots 更新，也不再依赖 stackmap record locations 解释
- 默认 explicit mode 的 runtime 不再以 LLVM stackmap section 为运行前提
- runtime 会按 `header -> desc -> offsets` 直接计算每个 slot 地址

### 7.3 InNative 线程会被简化

当前 stackmap 路线下，`enter_native` 之所以要捕获 `stack_walking_ctx`，是因为：

- 仅凭当前 call-site 的 `native_roots` 不足以覆盖更高层 managed caller frames
- GC 还需要靠 unwind 回到 caller frames，再查 stackmap

在 explicit mode 下：

- 更高层 managed caller frames 已经稳定地挂在 TLS explicit frame chain 上
- `enter_native` 不再需要为“找回 caller managed roots”而捕获 unwind ctx

这样：

- `native_roots` 可以只保留给 native call-site 自己的临时 roots
- runtime 的 `InNative` 协议会更简单

### 7.4 moving GC 的更新语义

moving / compaction 时：

1. GC 从 TLS top 拿到当前 activation 的 `ScoopRootFrameHeader*`
2. 读取 `hdr->desc`
3. 对每个 `offset` 计算 `slot = (void**)((uint8_t*)hdr + offset)`
4. 读取 `old = *slot`
5. 若对象发生搬迁，则写回 `*slot = new`
6. mutator 在 safepoint 之后 reload `*slot`

这里默认了一个布局 contract：

- `hdr` 是 frame object 的首字段

因此 `(uint8_t*)hdr` 同时也是 frame base。

这和当前 stackmap spill-slot update 的语义一致，只是 roots source 不再来自 stackmap record。

## 8. 与 LLVM Stackmap 的关系

### 8.1 本轮默认 explicit mode 的实现决策

本轮 refactor 完成后，默认 explicit mode 采用以下策略：

- managed roots 统一来自 explicit root frame
- runtime 不以 LLVM stackmap 作为 managed roots 的扫描/更新依据
- 编译器不为默认 explicit mode 生成 LLVM stackmap section / records
- 默认 explicit mode 不再把 statepoint/stackmap 视为 correctness contract 的一部分

因此：

- 默认 explicit mode 的 source of truth 是 explicit root frame
- `.llvm_stackmaps` / `__llvm_stackmaps` 不再属于默认产物的一部分
- runtime stackmap registry 对默认 explicit mode 不是必需路径

### 8.2 explicit mode 的主要价值

explicit root frame 的最大现实价值是：

- **回避 LLVM stackmap 中可能出现 register location 的情况**

当前 GC 不能读写 register root，因此：

- 只要 roots 仍可能被 LLVM 表达成 register location，runtime correctness 就不能完全建立在 stackmap 上

explicit mode 的策略则是：

- 根本不把 managed root 的可更新性寄托在 stackmap location kind 上
- 直接要求它们都先拥有 stable stack slot home

### 8.3 二进制尺寸收益

默认 explicit mode 不生成 LLVM stackmap，还带来一个附带收益：

- 产物中不再包含 `.llvm_stackmaps` / `__llvm_stackmaps` section
- 不再需要为默认 explicit mode 的每个 safepoint 生成 stackmap records 及其相关 metadata
- 默认显式 roots metadata 主要变成“每函数一份 frame descriptor”，而不是“每 safepoint 一批 stackmap records”
- 在 managed call-site / safepoint 数量较多的程序里，通常能获得一定的 object / executable size 收益

需要注意：

- 这个收益不一定压过所有新增代码
- explicit root frame 自身会引入 frame push/pop、slot 初始化、post-safepoint reload 等指令
- 因此净收益取决于程序里 safepoint / managed call-site 的数量，以及编译器实现细节

从方向上看：

- 默认 explicit mode 至少消除了 stackmap section 本身的尺寸开销
- 对大量 managed 调用的二进制，尺寸通常有机会比当前默认 stackmap 路线更小

### 8.4 LLVM stackmap 保留为未来可选优化

本设计不要求永久删除 stackmap。

保留 stackmap 的原因：

- 它仍然是一个合理的优化方向
- 它可以在 safepoint 粒度只枚举 live roots
- 相较 explicit frame，理论上可以减少扫描开销

但 stackmap 不再承担默认 explicit mode 的 correctness 路线。

### 8.5 stackmap abstraction 不应继续绑定在 `return_address -> record`

为了兼容 explicit mode，root map 抽象的上层接口不应长成：

- “给我 return address，我给你 stackmap record”

而应长成：

- “给我当前线程 / 当前 GC 周期的 root source，我给你一串 `void** slot`”

stackmap 只是其中一种实现细节。

## 9. 正确性约束清单

explicit root frame 落地时，至少需要满足以下约束：

1. 每个 managed activation 都在 entry block 建立 frame。
2. frame 必须在所有离开 activation 的路径上正确 pop。
3. frame 中的 slot 地址在整个 activation 内稳定不变。
4. frame 中的每个 home slot 在函数入口必须初始化为 `NULL`。
5. dead / inactive root slot 必须被清回 `NULL`。
6. slot 中只能存对象头指针或 `NULL`，不能存 interior / derived pointer。
7. safepoint 之后不得继续使用 safepoint 前的 GC SSA / register 值。
8. post-safepoint 使用必须 reload 对应 home slot。
9. heap-backed traced fields 不进入 explicit frame。
10. moving GC 对 managed roots 的唯一写回点是 home slot 本身。
11. explicit frame 只表示 managed root home slots，不等于“函数所有 locals 的大 struct”。
12. 值类型的 canonical 内存布局 / 调用 ABI 不因 explicit root frame 而改变。
13. 对包含 ref 的 aggregate，frame layout 按 ref leaf home slots 建模；非 ref 字段不进入 descriptor。
14. safepoint 后不得把旧 aggregate 副本中的 ref 字段当作 source of truth；若要按值复制/传参，必须先从最新 home slots 刷新或重组 aggregate。
15. explicit frame object、本身承载 source-of-truth 的 root home slots，以及其它依赖 stable address 的关键槽位，不得被 promotion 成仅 SSA / register 形式。

## 10. 实现顺序建议

### Phase 1：抽象 runtime root map 入口

- 把 runtime 中“managed frame roots 来源”的入口从 stackmap 专用逻辑抽出来
- 上层统一只看 `void** slot visitor`
- 为“默认 explicit mode 不走 stackmap”预留干净接口

### Phase 2：引入 explicit root frame runtime 结构

- 增加 TLS top
- 增加 frame descriptor / frame header 定义
- 增加按 `header -> desc -> offsets` 遍历 explicit frame chain 的逻辑
- 让 GC 能以 explicit mode 扫描 managed frame roots

### Phase 3：编译器发射 explicit root frame

- 为每个 managed 函数计算固定 frame layout
- 为每个 managed 函数生成常量 frame descriptor / offset table
- entry block 分配 frame object，并把所有跨 safepoint roots 收敛到 frame fields
- 在统一函数入口 push，在统一退出路径 pop
- 把跨 safepoint 的所有 GC ref 规范到 stable home slot
- 明确 aggregate flattening 规则：frame 按 ref leaf slots 建模，而不是按 local 个数建模
- 切换默认 explicit mode 到 explicit root frame

### Phase 4：收紧 safepoint reload contract

- 明确所有 safepoint 都是 GC ref clobber 边界
- post-safepoint use 必须 reload
- 杜绝继续依赖 safepoint 前的旧 SSA / register 值
- 为 post-safepoint 的 aggregate 复制 / 传参 / 返回补齐“先刷新/重组，再按 ABI 传递”的 lowering contract
- 为后续重新评估 mem2reg 准备可证明的 clobber / reload barrier 语义

### Phase 5：默认 explicit mode 停止使用和生成 LLVM stackmap

- 默认 explicit mode 的 managed roots 完全由 explicit root frame 支撑
- 默认 explicit mode 不再使用 LLVM stackmap 作为 runtime roots 输入
- 默认 explicit mode 不再生成 `.llvm_stackmaps` / `__llvm_stackmaps`
- 观察 object / executable size 与 steady-state / GC pause 的变化

### Phase 6：保留 stackmap 作为未来优化路线

- 在 correctness-first 的 explicit mode 稳定后
- 再决定哪些函数 / 场景可以切回 stackmap mode
- stackmap mode 应被视为优化，不再是默认 explicit mode 的基础

### Phase 7：重新评估 SSA promotion / mem2reg

- 以 explicit root frame 为前提，审计哪些 ordinary locals 可以安全 promotion
- 验证 explicit frame object 与 root home slots 不会被 promotion / scalar replacement 破坏
- 验证 post-safepoint reload 不会被 mem2reg/GVN/CSE 重新折叠回旧 SSA
- 在测试与 GC stress 足够覆盖后，再决定默认是否放开、以及放开的范围
- 先形成 allowlist/denylist：
  - allowlist：ordinary non-root locals、GC-free temporaries、地址不逃逸的非 root aggregates
  - denylist：explicit frame object、root home slots、依赖 stable address 的 ABI/runtime scratch
- 为 effect/continuation/state-machine 路径单独建立 mem2reg audit 清单

## 11. 风险与待确认事项

### 11.1 LLVM 优化器是否会复用 safepoint 前 load

即使源码层面显式写了 reload，也仍需确认：

- LLVM 不会把 safepoint 前的 load/CSE 结果错误地复用到 safepoint 后

因此 explicit mode 可能还需要额外约束：

- 更强的 lowering barrier
- 更清晰的 memory-clobber 语义
- 或调整 pass / call attributes，确保 safepoint 真正成为 GC ref clobber 边界

### 11.1.1 explicit root frame 落地后，mem2reg 也不能“自动无条件打开”

需要单独澄清：

- 当默认 explicit mode 不再依赖 stackmap / statepoint roots 合同时，
  “LLVM 可能把 roots 表达成 register location” 这条旧 blocker 会显著弱化
- 因而，相比当前 stackmap 路线，explicit root frame 确实更有希望重新放开一部分
  SSA promotion / mem2reg 优化

但这不等于：

- refactor 一完成，就可以对所有相关 alloca 无条件打开 mem2reg

原因至少有两类：

1. root frame 自身的地址稳定性仍然是 correctness 前提
2. safepoint 的 clobber / reload contract 仍可能被 mem2reg、GVN、CSE 等优化削弱

具体来说：

- explicit frame object 必须保留为 entry-block alloca，且地址在整个 activation 内稳定
- 任何真正承担 managed root source-of-truth 的 home slot，都不能被优化成“只存在于寄存器里”
- 即使 ordinary locals 允许 promotion，post-safepoint 对 managed ref 的使用也仍必须体现为：
  从 home slot reload，而不是复用 safepoint 前的旧 SSA

因此，更合理的策略应是：

- 把“打开 mem2reg”视为 explicit root frame 稳定之后的**单独优化任务**
- 先证明 safepoint 具备足够强的 memory-clobber / barrier 语义
- 再验证 root frame / root home slots 不会被 promotion 破坏 source-of-truth
- 最后再按范围逐步放开：
  - 优先 ordinary non-root locals
  - 审慎处理 aggregate copies / temporaries
  - 对 explicit frame object 与 root slots 保持更强保护

换句话说：

- **去掉 stackmap 后，可以重新评估 mem2reg**
- **但不能把“stackmap 被拿掉了”直接等价成“mem2reg 已经无条件安全”**

### 11.1.2 mem2reg rollout 的验证门槛

在真正放开 mem2reg 之前，建议至少具备以下验证门槛：

1. IR 级不变量检查

- explicit frame object 仍可在 IR 中观测到 entry-block alloca
- root home slots 仍通过稳定 GEP/offset 从 frame base 可达
- 关键 post-safepoint reload 仍保留在 safepoint 之后

2. 行为级回归覆盖

- alloc-heavy + GC stress 场景
- 含 ref aggregate 的 direct/indirect call arg 路径
- sret / return alloca / payload transport 路径
- effect / continuation / resume / state-machine 路径

3. 分阶段放开策略

- 先只放开 ordinary non-root locals
- 再评估 GC-free aggregates
- 最后才考虑更复杂的 address-taken / ABI scratch / effect lowering 槽位

4. 可回退的开关

- 在默认放开之前，最好先提供编译开关或 pipeline 级开关
- 便于在 GC stress / CI / 性能对比中快速 bisect

### 11.2 effect / continuation / resume 路径

需要单独审计：

- ordinary early return
- effect propagation
- continuation resume / suspend
- state-machine step / dispatch

这些路径是否都能正确 push/pop explicit frame，以及是否都遵守 post-safepoint reload contract。

### 11.3 frame 开销

explicit mode 的代价包括：

- 栈空间开销更固定
- 每次 GC 可能扫描比 stackmap 更多的 slot
- 编译器需要先规划每函数固定 frame layout

这是 correctness-first 路线的预期代价，应接受为短中期 tradeoff。

### 11.4 二进制尺寸 tradeoff

默认 explicit mode 停止生成 LLVM stackmap 后，收益方向包括：

- 去掉默认产物中的 stackmap section
- 去掉与 stackmap record 直接相关的对象文件内容

但代价也包括：

- 入口/出口 frame 管理代码
- slot 初始化代码
- post-safepoint reload 代码
- 每函数 frame descriptor / offset table 本身的常量开销

因此：

- 对 safepoint 很多、stackmap records 很多的程序，尺寸收益通常更明显
- 对 roots 很少、函数又很碎的小程序，净收益未必总是显著

### 11.5 “precise” 的含义

本设计把 explicit root frame 称为 precise shadow stack，其精确性来自：

- frame 中记录的是编译器选定的 root slot 集合
- slot 中保存的是对象头指针或 `NULL`

但它与 stackmap 的“每个 safepoint 仅 live subset”并不完全等价。

因此：

- 若要保持足够精确，必须依赖 NULL discipline
- 若 dead slot 不清空，它会退化为“安全但更保守”的 roots 表示

## 12. 结论

本次重构的核心判断如下：

- `root map` 的统一抽象应该围绕 **`void** slot` 枚举/更新** 建立，而不是围绕 stackmap record 建立。
- `explicit root frame` 是一个 correctness-first 的 managed roots 表示，可视为 precise shadow stack 或自定义精简 stackmap。
- 它的最大现实价值，是**回避 LLVM stackmap 中可能出现 register location 的问题**，从而不再把 managed roots 的可更新性建立在 runtime 目前无法处理的 register roots 之上。
- explicit root frame 是一层 **GC shadow layout**：它不等于把一个函数的所有 locals 合并成大 struct，也不应改变值类型自己的 canonical 内存布局 / 调用 ABI。
- 对包含 ref 的 aggregate，root frame 应按 ref leaf home slots 建模；post-safepoint 若要继续复制/传值，必须基于从 home slot 刷新后的 fresh aggregate。
- 本轮 refactor 完成后，`explicit root frame` 将成为新的默认 explicit mode；这个 mode 不使用，也不生成 LLVM stackmap。
- 因为默认 explicit mode 不再生成 `.llvm_stackmaps` / `__llvm_stackmaps`，它还有机会带来一定的 object / binary size 收益。
- explicit root frame 本身并不自动解决 stale register / stale SSA；真正的关键是编译器 contract：**safepoint 之后必须从 root frame 对应的 home slot reload ref pointer**。
- 因此，去掉 stackmap 之后可以重新评估 mem2reg / SSA promotion，但它仍应被视为后续优化任务，而不是 explicit root frame 落地后的自动结论。
- LLVM stackmap 仍值得保留，但应退到“未来可选优化方案”的位置，而不是继续承担默认 explicit mode 的 correctness 路线。

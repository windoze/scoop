# M15 设计：精确根、statepoint relocation 与 moving compaction

状态：设计稿

依赖：M9 的 Immix 堆与 statepoint 产出、M13 的多 mutator STW / native root / callback、M14 的完备 typed LIR 与无地址语义核心库。

## 0. 结论与平台范围

M15 把现有单代、STW、单线程 collector 从非移动 mark-region 实现升级为 moving Immix。完成后，任何未 pin 的存活引用对象都可能在一次 GC 中改变地址；所有合法 managed ref 都必须由精确、可改写的 slot 表示，旧地址不再具有任何隐含有效期。

首个且本里程碑唯一必须支持的平台是：

- OS：macOS；
- CPU：AArch64（Apple Silicon，LLVM triple 规范化为 `aarch64-apple-darwin`，接受 LLVM/工具链输入中的 `arm64` 同义拼写）；
- object format：Mach-O 64；
- LLVM：固定为与当前stable Rust一致的22.1（当前qualification使用22.1.8），stack map format v3；
- 异常 ABI：现有 Itanium C++ ABI 路径。

其他 OS/CPU 不能静默退回 M13 的保守扫描或非移动 collector。driver 必须在 codegen 前给出明确的 unsupported-target 诊断。`arm64e`、pointer-authenticated code pointer、cross compilation、动态装载的 Scoop image 均不在 M15 范围。

LLVM 22.1是当前编译器的固定组成部分，不是可在PATH中任意替换的外部优化器。选择该版本是因为它与项目采用的当前stable Rust后端一致，可以把IR、statepoint lowering和机器码行为锁在同一已验证边界。M15及当前路线不设计LLVM升级；构建和driver启动必须验证实际链接的LLVM major/minor恰为22.1，其他版本在处理用户module前失败。未来升级必须作为独立工作重新qualification，不能在本里程碑中准备多版本fallback。

平台范围虽窄，代码边界必须从第一版就模块化：stack map 解析、root visitor、forwarding、evacuation 和 Immix side metadata 都与平台无关；Mach-O image、Darwin thread/VM 与 AArch64 frame/anchor 是三个可组合的平台组件。新增平台只能登记 target profile，并补充尚不存在的 object-format、OS 或 architecture/ABI 组件，不得修改 collector 算法或在通用代码中散布 `#if defined(...)`。

## 1. 当前实现审计与必须修正的事实

M15 不能把 M9 已有的“statepoint section 存在”当作可移动 GC 基础。当前实现有以下已验证事实：

1. codegen 把 `Managed` / `Raw` / `Code` / `Metadata` 全部发射为 LLVM address space 0 的 opaque pointer。`statepoint-example` 只把 address space 1 识别为 GC managed pointer，因此现有 stack map 每条记录只有 calling convention、flags、deopt count 三个常量，没有任何 root location，也没有有效的 `gc.relocate`。M0历史spike沿用了这个AS0模型，只证明RS4GC、stackmap section与EH section通道存在，不能作为精确根验证；
2. `rewrite-statepoints-for-gc` 不会递归发现 LLVM aggregate 中的 managed pointer leaf。一个跨 safepoint 存活的 `{ i64, ptr addrspace(1) }` 整体不会自动进入 `gc-live`；
3. LLVM 当前仍不能为 Scoop 使用的普通 Itanium `{ ptr, i32 } landingpad` 正确表示异常边 relocation。带活跃 managed ref 的 `invoke` 经 RS4GC 后会产生无法通过 LLVM verifier 的 `gc.relocate(landingpad ...)`；
4. 使用LLVM 22.1.8对AS1 live root执行的独立`opt`/`llc`及inkwell `TargetMachine::write_to_file` qualification表明：macOS/AArch64的statepoint root在O0/O2下均成对生成为`Indirect [SP + offset]`，stack map的`instruction_offset`精确指向`bl`后的return address；不需要也不允许`PC - 4`猜测。显式打开GC vreg与callee-saved register root后会稳定产生`Register` location，重新关闭后post-RA fixup恢复为`Indirect`，因此测试确实覆盖了相关开关而非偶然观察一次产物；
5. 当前 Scoop 函数会保存 x29/x30，但没有建立可遍历的 x29 frame chain；M15 必须强制 `frame-pointer=all`；
6. 链接后的 Mach-O 使用 dyld chained fixup 编码 stack map 中的 function address。磁盘文件里的 64 位字不是运行期代码地址；runtime 必须从已被 dyld fixup 的进程内 section 读取，离线测试则按 Mach-O fixup 语义验证；
7. 当前 block header、free-block node 和 free-line hole node 位于 GC arena 内。stress mode 对空源 block 执行 `mprotect(PROT_NONE)` 后，这些地址不能再作为 collector 元数据访问；
8. 当前对象起点只记录 bit，不记录精确 allocation size。可变长 String/Array 及 large object 不能只靠 TypeDescriptor 固定 size 正确复制；
9. 当前全局 managed storage 没有编译器生成的完备 root 描述登记，静态 String 等 immortal managed object 也没有精确地址域登记；保守扫描会掩盖其中一部分问题；
10. runtime 中会再次分配的 Scoop ABI/native helper 必须在分配前发布输入 direct ref 或含 ref payload，并在 GC 后从 slot reload。把 C local 中碰巧存在的旧指针当作保活手段在 M15 中非法；
11. LLVM 22.1 RS4GC 会把普通call的AS1直接实参加入`gc-live`，即使该source在caller continuation中已经死亡；它不会递归拆解含AS1 leaf的aggregate实参。LIR必须显式给出post-site live source与可移动实参载体的并集，post verifier不能只拿普通live-after数量猜最终root count；
12. 同一行为使带AS1实参的普通LLVM `invoke`经RS4GC必然获得`gc-live`并触发不可用的exceptional relocation。managed invoke必须由codegen直接发射零`gc-live`的显式`gc.statepoint` invoke，不能先发射普通invoke再期待RS4GC自动得到设计要求的形态。

这些不是独立 backlog，而是 M15 正确性闭环的一部分。

## 2. 目标与明确不做

### 2.1 完成目标

- 普通模式使用精确根和 moving compaction；不再执行任何 conservative stack word scan；
- LLVM managed ref 使用 address space 1，跨普通 safepoint 的每个活跃 leaf 生成并使用 `gc.relocate`；
- managed `invoke` 使用异常安全的显式 root spill 协议，绕开 LLVM 不可表达的 exceptional relocation；
- root、heap field、外部异常对象、handle、pin、closure、coroutine frame、callback 等都通过同一个可改写 slot visitor；
- 普通 collector 可按 block occupancy 选择 evacuation source；stress collector 每轮搬动全部未 pin 的存活对象；
- stress mode 在每次 mutator-visible managed allocation 前 GC，poison 旧副本，并对空源 block 永久 `PROT_NONE`；
- M1–M14 全量 fixture 在普通 moving 模式通过，指定组合 fixture 在 stress mode 通过。

### 2.2 本里程碑不做

- 分代、晋升、remembered-set 消费；现有 card mark 保留但本里程碑仍不用于 tracing；
- parallel/concurrent marking 或 evacuation；collector 仍单线程；
- concurrent compaction/read barrier/Brooks pointer；
- weak reference、ephemeron；
- GC-free release hook；它是后续用于兜底释放native resource的受限机制，不属于M15；
- 全功能GC finalizer永久不支持，不是后续backlog。尤其不允许以`finalize`/析构方法、release hook或runtime内部开关执行任意managed代码、访问对象图或复活对象；
- pinned interior pointer 的新语言能力；`PinnedPtr.raw` 继续只由既有 pin 契约产生；
- 新异常 ABI、suspend FFI 或异常跨 C frame；
- Linux/x86_64/Windows runtime backend；
- M17 之前的多 Cone、动态 library image stack-map discovery。

## 3. 不变量

### 3.1 引用与地址域

每个运行期 pointer value 属于以下一种 provenance：

| LIR provenance | LLVM 表示 | 含义 |
|---|---|---|
| `Managed` | `ptr addrspace(1)` | null 或对象起点；由 GC 跟踪、可移动 |
| `Raw` | `ptr addrspace(0)` | native 数据地址；GC 不检查、不改写 |
| `Code` | `ptr addrspace(0)` | 函数入口/trampoline |
| `Metadata` | `ptr addrspace(0)` | TypeDescriptor、scan program 等 immortal metadata |

`Managed` 与其他三类不能因 LLVM opaque pointer 而合并。所有涉及AS1的`addrspacecast`、`ptrtoint`与`inttoptr`都必须来自完备的typed来源；M15正常LIR不提供managed-to-raw cast或managed `PtrToInt`。codegen自身只有两个需要pointer-conversion witness的封闭internal boundary：TLAB地址发布为新object，以及write-barrier计算card index；它们分别携带不同的非可选metadata witness，post verifier按operation kind精确匹配。C++ EH的`BeginCatch`是独立的typed operation，其ABI声明直接产生已登记、稳定的AS1 exception ref，不经过address-space cast。witness不能由源码/LIR构造，也不能把转换开放成通用pointer escape。pin返回的raw地址经现有`UInt`/`Ptr` boundary取得，不以任意addrspacecast冒充。

managed ref 的非 null 值只能指向：

- 当前 GC heap 中已登记的对象起点；
- 已登记的 stable external managed object（当前为活跃的 ABI exception buffer）；
- 编译器登记的 immortal managed object（当前主要为静态 String/无捕获静态 closure）。

精确 slot 中出现其他非 null 值是 compiler/runtime corruption，必须 fatal；不能像保守扫描那样忽略。

### 3.2 可改写 slot

collector 的 tracing 原语不是“查看一个指针值”，而是访问 `ManagedSlot`：一个包含 managed ref 的可写 8 字节位置。scanner在mark和relocate阶段都只提交slot地址，不复制出一个脱离来源的pointer value。统一visitor有两个封闭mode：`Mark`只验证/标记，`Relocate`查询或建立forwarding并写回；不能为某类root另写只读扫描旁路。`Relocate`对slot执行：

1. 读取旧值并验证地址域；
2. 对 heap object 查询或建立 forwarding；
3. 把新地址写回同一 slot；
4. 把首次到达的目标对象加入 scan queue。

所有来源必须最终降为此接口：

- statepoint stack location；
- managed invoke 的 compiler root frame；
- native root / native-safe caller root；
- managed global storage；
- `GcHandle` table entry；
- stable external object内部字段；
- pinned object及普通 heap object的字段；
- Array 元素和 struct/tuple/tagged enum/closure/coroutine frame 中递归展开的 ref leaf。

只有 pinned root 本身和 stable/immortal object base 不是可改写 slot：它们的地址固定，但其内部 managed slot 仍走同一 visitor。

### 3.3 forwarding 与完成状态

- 每个 from-space 对象至多建立一次 forwarding；forwarding table 位于 arena 外；
- 普通模式未被选为 evacuation source 的对象可留在原址；被选对象必须移动，除非 pin；
- stress 模式中每个存活、未 pin 的对象都必须满足 `new != old`；
- pinned 对象地址不变，出站引用必须更新；unpin 后可在后续 collection 移动；
- `GcHandle` 的 generation/slot identity 不变，只改写 table entry；
- collection 结束前，任何合法 slot 都不能再指向已 forwarding 的旧对象；
- 旧 object-start 记录只能在 forwarding 闭包与 slot 验证全部成功后清除。

## 4. 目标平台与模块边界

### 4.1 compiler target profile

codegen 不再直接使用一个无约束的 `host_target_machine()`。driver 先把 host triple 规范化为 opaque typed `TargetProfileId`，再从唯一 target registry 取得完备 `TargetProfile`；M15 registry 只有 `DarwinAarch64`。除 registry 与该 profile 的构造测试外，下游不得 exhaustive match `TargetProfileId` 来反推能力。profile 完整给出：

- canonical triple 与 data layout；
- 已解析的CPU/features、relocation/code model及macOS deployment/link配置；
- managed pointer address space（1）；
- stack map version（3）与支持的 root location capability；
- frame-pointer / tail-call policy；
- object format 与 runtime platform bundle/source set。

profile 是 codegen 和 runtime build 的唯一平台选择入口。其字段是非可选 capability，不以 `Option` 表示“该平台也许支持”；registry 不能构造能力残缺的 profile。新增平台在 registry 增加 canonical triple 映射，复用已有组件并只实现缺失组件；pointer lowering、collector、thread handshake 等消费者只读取 profile 能力，不增加 target 判断。

### 4.2 runtime platform interface

建议模块结构：

```text
runtime/src/
  gc/
    collector.c          forwarding、evacuation、collection phases
    heap.c               arena、block side metadata、mutator/GC allocator
    roots.c              scan program与统一ManagedSlot visitor
    stackmap.c           平台无关LLVM v3 parser/index
    gc_internal.h
  platform/
    platform.h                   唯一通用接口与完备PlatformBundle
    profiles/darwin_aarch64.c    只负责组合下列三个组件
    image/macho.c                loaded image与stack-map section发现
    os/darwin.c                  pthread stack bounds、page size与VM保护
    arch/aarch64.c               frame chain与stack-map location解释
    arch/aarch64_anchor.S        捕获managed caller anchor
  thread.c               STW状态机，持有opaque platform anchor
runtime/tests/platform/
  fake.c                   通用collector/root测试使用的确定性假平台
```

这同时把当前 1200 行以上的 `gc.c` 按真实职责拆开，不做按行数机械切割。`thread.c` 中现有 macOS/Linux 栈边界条件分支迁入 OS 组件。

`platform.h` 不暴露 Darwin/Mach-O 类型。它定义一个由三个非空 operation table 构成、静态只读的 `PlatformBundle`：`MetadataImageOps`、`ThreadVmOps` 与 `ManagedFrameOps`。profile 必须一次性提供完整 bundle；通用 runtime 不能在调用时判断某个 function pointer 是否为空，也不能用 host macro 自选实现。概念能力至少包括：

```text
PlatformMetadataImages() -> [MetadataImage]
PlatformThreadStackBounds() -> StackBounds
PlatformCaptureManagedAnchor() -> ManagedAnchor
PlatformWalkManagedFrames(anchor, boundary, visitor)
PlatformResolveStackMapLocation(frame, location) -> ManagedSlot
PlatformProtectNone(page_aligned_base, size)
PlatformPageSize() -> usize
```

通用 `stackmap.c` 解释 v3 字节格式；image 组件只发现进程内 metadata span，frame 组件只把 DWARF register number 与当前 frame 的 SP/FP/保存上下文映射为地址，OS 组件只提供 thread/VM primitive。通用 collector 不读取 x29、Mach-O header 或 `mprotect`。

这种分解避免平台笛卡尔积复制：Linux/AArch64可复用AArch64 frame组件并新增ELF image/Linux OS组件；macOS/x86_64可复用Mach-O/Darwin组件并新增x86_64 frame组件。若某个ABI使同一CPU的frame规则不同，应新增architecture/ABI组件，而不是在已有组件中读取OS名称分支。

新增target的固定扩展面仅包括：target registry中的一条规范化映射、一个完整profile组合、此前不存在的平台组件、对应构建source set与artifact测试。`gc/`、LIR root model及通用codegen pass不是扩展点。测试用fake bundle可注入metadata image、frame序列和page-protection结果，使collector/stackmap/root单元测试不依赖Darwin；若通用模块必须include Apple/ELF header、读取具体register或match target identity，视为模块边界失败。

### 4.3 macOS/AArch64 实现

- 通过 `<mach-o/getsect.h>` 与主 executable header 取得进程内 `__LLVM_STACKMAPS,__llvm_stackmaps` 的地址和长度；必须读取 dyld 已应用 chained fixup 后的内存，不直接 mmap/解析 executable 文件；
- metadata image API 从第一版即返回 span 列表。M15 Darwin 实现只有主 image 的一个 span；未来动态 image 或多 image 只扩展 platform discovery；
- Scoop generated function 强制 `"frame-pointer"="all"` 与 `"disable-tail-calls"="true"`；runtime C 以 `-fno-omit-frame-pointer -fno-optimize-sibling-calls` 构建；
- managed runtime entry 在任何可能 park/collect 前发布含`resume_pc`、callsite `sp`与`fp`的opaque `ManagedAnchor`。entry wrapper直接观察managed caller，不能在进入多层C helper后反推；嵌套runtime helper复用最外层活跃anchor；
- 当前 frame 的 stack pointer 按该 function stack-map record 的 `stack_size` 和 AArch64 frame record 关系精确求出；外层 frame 从 `[fp]` 的 previous FP 与 `[fp + 8]` 的 saved LR 继续；所有地址必须位于该线程已登记 stack bounds 和当前 managed segment boundary 内；
- return PC 必须原值命中 stack-map index。禁止 `PC-4`、范围最近匹配或函数名回退；
- stackmap前三个location依次编码calling convention、statepoint flags与deopt count，它们不是GC root；M15不产生deopt输入并禁止`DeoptLiveIn`。跳过这三个header location和零个deopt location后，DarwinAarch64只接受size=8、base=derived且位置为`Indirect [SP/FP + checked offset]`的GC root pair。GC root位置出现`Register`、`Direct`、constant、未知DWARF register、动态stack size或不相同的base/derived pair是compiler/toolchain invariant failure；通用parser仍保留全部location枚举，以正确跳过非root区域并供未来平台声明自己的能力；
- page size 从系统查询。整块保护前同时检查 GC block 对齐、page 对齐与 size 整除；`mprotect` 失败立即 fatal。

stack-only不是由runtime根据最终产物猜测出的能力。LLVM 22.1 SelectionDAG的`max-registers-for-gc-values`默认且本项目固定为0；标准TargetMachine machine pipeline在register allocation之后运行`FixupStatepointCallerSaved`，其`fixup-allow-gcptr-in-csr`保持false时会把GC register operand改写为frame-index spill。Scoop不调用LLVM command-line parser接收未经筛选的用户参数，也不提供透传LLVM backend option的CLI；codegen固定使用SelectionDAG和包含该fixup的标准pipeline，不能切到GlobalISel或自定义删减machine pass。

object验证仍必须逐个检查root location，但它只是后端不变量的防御性断言：失败表示编译器配置、链接LLVM或LLVM 22.1 qualification被破坏，应报告internal compiler/toolchain error并丢弃object，而不是把某段普通Scoop源码诊断为“不支持register root”。runtime初始化重复检查是对错误链接、损坏或非Scoop image的最后防线。当前路线不升级LLVM；未来若升级，先建立新的backend profile和qualification，再决定继续stack-only还是实现register-root unwind/update，不能让下游runtime临时补猜。

### 4.4 managed runtime entry 协议

runtime registry与C header把入口分为互不兼容的四类：

- `ManagedEntry`：只由generated managed code直接调用，LIR target为`ManagedTargetRef`，有statepoint；
- `NativeBorrowedEntry`：只由Scoop ABI native实现调用，要求当前TLS mode、caller-root frame及callee native-root链已经发布，没有managed statepoint/anchor；
- `RuntimeInternal`：只供runtime内部调用的`*_impl`，不导出给codegen或FFI author；
- `ForeignCallbackEntry`：C trampoline反向进入managed的独立gateway，按M13执行attach/transition并建立新的managed segment。

不同入口使用不同typed id/API声明，不能导出一个“自动判断direct caller是managed还是native”的万能symbol。codegen、runtime内部与FFI header都只能取得各自允许的入口集合。

每个可 park/collect 的 `ManagedEntry` 都是一个直接观察 generated managed caller的、`noinline`薄入口；它不是通用 C 实现本体。入口必须在覆盖managed caller寄存器或建立普通C helper frame之前，经architecture/ABI组件捕获`{ return_pc, callsite_sp, frame_pointer }`，把anchor push到TLS transition链，然后调用平台无关的`*_impl`，最后在所有正常出口pop。runtime内部只调用`*_impl`，不得再次经过该入口并把C caller误记为managed caller。

AArch64入口锚点用受控汇编实现并由反汇编测试锁定，不能依赖未经验证的`__builtin_return_address`/`__builtin_frame_address`优化行为。不同签名的入口可以共享汇编prologue模板，但其ABI转发必须由runtime header/生成表驱动，不按symbol猜参数。runtime ABI仍禁止异常穿过该薄入口；发生fatal metadata错误时不执行恢复性pop。

最外层薄入口拥有当前active managed segment的top anchor。回调或native transition产生新的managed segment时，按M13 LIFO transition协议各自push新的anchor/boundary；collector只扫描每个active segment的anchor，冻结segment只扫描已发布caller root frame。

`NativeBorrowedEntry`在第一个可能GC/park的动作前验证当前transition确为native-borrowed且native-root链结构合法，然后参与epoch握手。该入口触发collection时，外层managed segment仍是冻结segment，只枚举进入native前的caller-root frame与callee登记的native-root slots；禁止捕获当前C return address、跨C frame unwind或回退到栈扫描。入口返回后，native实现必须从root slots reload。

## 5. LIR 与 statepoint 完备信息

### 5.1 callsite root plan

LIR lowering 在 CFG、类型、layout 与 liveness 均完整后，为每个调用点输出非可选、按 effect 隔离的调用实体：

```text
ManagedPollSite {
    target: ManagedTargetRef,
    safepoint: SafepointId,
    live: StatepointLiveSet,
}

ManagedCallSite {
    target: ManagedTargetRef,
    call: TypedCall,
    safepoint: SafepointId,
    live: StatepointLiveSet,
}

ManagedInvokeSite {
    target: ManagedTargetRef,
    call: TypedCall,
    safepoint: SafepointId,
    roots: ExceptionalRootSet,
    normal: BlockId,
    unwind: BlockId,
}

NoGcCallSite { target: NoGcTargetRef, call: TypedCall }
NoGcInvokeSite { target: NoGcTargetRef, call: TypedCall, normal, unwind }
NativeSafeCallSite {
    target: NativeSafeTargetRef,
    call: TypedCall,
    safepoint: SafepointId,
    roots: NativeSafeRootSet,
}
NativeBorrowedCallSite {
    target: NativeBorrowedTargetRef,
    call: TypedCall,
    safepoint: SafepointId,
    roots: NativeBorrowedRootSet,
}
```

四种 target ref 使用不同 typed id；不能保留一份 `CallEffect` enum，再配一份可能矛盾或缺失的 `Option<RootPlan>`。这是M15对M14 `CallEffect`存储形态的显式修订：M14已经解析完成的四分语义保持不变，但LIR输出改为按protocol隔离的sum，不能同时保留旧enum供codegen任选。void/direct/indirect-result 的现有 typed return convention 作为各 callsite 内部互斥 sum 保留。native call 仍不允许 unwind 回 managed code。

函数入口与每条循环回边的poll由LIR lowering在CFG上显式插入为`ManagedPollSite`，并在同一次backward liveness中生成root plan；poll target是runtime registry给出的typed managed target。codegen不得自行寻找回边、添加poll或重新计算其live roots。入口poll的live set包含仍由callee持有的managed参数，因此caller只传入而call后不再活跃的参数仍在callee到达第一个park点前获得精确根。

`SafepointId` 是 deterministic、非零、image 内唯一的 typed id。codegen 将它写入 callsite 的 `statepoint-id` attribute；runtime 仍以实际 return PC 为主键，ID 用于 artifact 对照、诊断和测试，不能替代 PC。

### 5.2 managed leaf 展开

`StatepointLiveSet` 由 LIR backward liveness与同一callsite的typed operand信息共同产生，不由 codegen 扫描 LLVM use 或根据变量名补齐。它是以下两组source的去重并集：

- 在statepoint后仍活跃、必须消费relocated值的source；
- 普通managed call的可移动managed实参载体。LLVM 22.1会把直接AS1实参自动列入`gc-live`；aggregate实参则必须由本集合显式暴露其managed leaf。immortal常量地址不属于可移动载体；managed global storage中的值若先load为temp，则按该temp进入集合。

每项同时携带：

- typed source（parameter/local/temp）；
- concrete LIR type/layout；
- 已扁平化、去重且按 offset 排序的 managed leaf path；
- leaf 的 `PointerKind::Managed`。

对含 ref aggregate，codegen 在 call 前把每个 leaf load 为独立 `ptr addrspace(1)` SSA value，并在 call 后形成对该值的显式使用，再写回原 storage/rebuild aggregate。这样 RS4GC 必须为每个 leaf 创建 relocate，并把调用后的 use 改为 relocated value。不能把 aggregate alloca 地址作为 stack region 交给 LLVM，也不能假设 RS4GC 会扫描 struct。

普通 managed ref 是 offset 0 的单 leaf。tagged enum 直接消费 LIR 的固定 ref offset；不读取 tag。inactive slot 已由 M13/M14 契约保证为 0。codegen的tagged-enum LLVM物理类型必须把每个固定ref offset表达为真正的AS1 pointer field，只有GC-free间隙可用opaque byte array；把整个payload降成`[N x i8]`会使SROA把managed pointer拆成`ptrtoint`/字节片段，属于非法的provenance擦除。

### 5.3 普通 managed call与poll

普通 `ManagedCallSite` 与`ManagedPollSite`经以下顺序发射：

1. materialize `StatepointLiveSet` 中的独立 AS1 leaf，并让call实参消费同一materialized source；
2. 给原始 call 设置唯一 `statepoint-id`；
3. 发射 call；
4. 在 call 后使用每个 leaf 重建 source storage/SSA value；
5. 完成 SSA/SROA/mem2reg 后运行 RS4GC；
6. verifier 确认每个预期 leaf 有 `gc.relocate`，且调用后不再使用原地址值。

direct ref / aggregate 参数即使只被callee使用而不在caller continuation活跃，也属于该call的`StatepointLiveSet`。这是LLVM 22.1实际statepoint模型的一部分：direct AS1参数会被RS4GC列入caller的`gc-live`，而含ref aggregate需要Scoop主动拆叶。callee入口poll仍建立callee自己的参数根，但它不能替代callsite参数根。

含 ref 的 indirect return storage 遵守“callee 本地构造、无 safepoint epilogue 一次发布”规则：callee 在最后一个可能 safepoint 后才把完整结果复制到 caller storage，此后直接返回；异常路径不发布部分结果。不能让 partially initialized、未登记的 caller return storage 跨 callee safepoint。

### 5.4 managed invoke 与异常边

不得把 LLVM exceptional relocation 当作可用能力。`ManagedInvokeSite` 在进入 invoke 前：

1. 为 normal/unwind 任一后继仍活跃的全部 managed leaf及可移动实参leaf建立 addressable storage；
2. 按 `ExceptionalRootSet` 写入 compiler root frame并 push；
3. 调整调用后数据流，使原 AS1 SSA value在 invoke 后没有直接 use，两个后继都只从 frame storage reload；
4. codegen直接发射带`SafepointId`的显式`gc.statepoint` invoke，并把原callee与实参放在statepoint的actual-call operand区；`gc-live`必须为空，因此 stack map保留 frame/callsite定位记录而没有 exceptional `gc.relocate`。RS4GC只跳过这个已显式化的invoke，不能负责从普通invoke生成该形态；
5. normal edge reload并 pop；unwind landingpad 的第一段无 safepoint cleanup同样 reload并 pop，然后才进入 catch/finally/`resume`；
6. 任何 rethrow/resume 路径都不得携带尚未 pop 的 compiler root frame。

compiler root frame 使用与 native caller root 相同的递归 scan descriptor和统一 slot visitor，但用独立 frame kind，不能伪装成 native-safe transition。LIR 的 `ExceptionalRootSet` 是 normal/unwind successor liveness与可移动实参载体的并集；每个entry还完备标出`normal_live`与`unwind_live`，使两个后继只reload自己需要的source，纯实参根可在两边都不reload。result 在 normal edge定义，不进入 pre-call root set。共享landingpad在捕获exception record后按动态LIFO栈pop；NoGc invoke可发布一个无entry的compiler frame，使同一landingpad不必根据前驱猜测是否存在frame。

这样既保留每个 managed frame 的 statepoint record供精确 frame walk，又不生成 LLVM 当前不可验证的异常边 relocation。

### 5.5 native-safe与native-borrowed调用

两类outbound native call都划分冻结managed segment，不能再把它们当作普通`ManagedCallSite`。其root plan分别使用不同类型，但都完整列出调用后仍活跃的managed leaf；`NativeBorrowedRootSet`还包含全部direct-ref实参。若Scoop ABI返回值直接或间接含ref，root plan按return-convention sum携带一个调用前全零并一同发布的result storage及其scan，不使用`Option<ReturnRoot>`让codegen补齐。

发射协议为：

1. materialize并发布caller-root frame；含ref返回storage先清零；
2. 进入`native-safe`或`native-borrowed`transition；
3. 给native machine call设置`SafepointId`并发射为零`gc-live` statepoint；所有跨调用AS1 SSA value在call后都不得再使用；
4. `native-safe`的C ABI结果必为GC-free；`native-borrowed`若返回含ref值，在仍处于borrowed且collector不能把本线程视为quiescent时立即写入已发布result storage；
5. leave transition并执行epoch握手；此时caller-root/result slot保持发布状态，若park/collect可被改写；
6. 回到managed后只从这些slot reload，再pop caller-root frame。

collector不扫描冻结segment中的native-call statepoint location；零`gc-live` record只保留调用边界的LLVM语义与artifact可审计性。让RS4GC同时生成普通relocate、再从另一份caller-root slot reload属于两个互相冲突的真相来源，必须由post-RS4GC verifier拒绝。

`@Extern(abi = "scoop") @NoGC`只保证native函数体不会进入GC/runtime/managed callback，不把该边界改成`NoGcCallSite`：多mutator moving模式仍须进入native-borrowed、发布caller roots，并在返回epoch握手后reload。真正的`NoGcCallSite`只用于不离开generated managed segment且由typed target保证不触发GC/transition的调用。

Scoop ABI callee若在内部进入`NativeBorrowedEntry`，仍必须先发布自己的native roots并在返回后reload；caller root只能保活调用链，不能修复callee持有的旧C local。

### 5.6 derived/interior pointer

AS1 value 只表示 object start。字段/数组元素地址是短生命周期 derived address，只能在同一无 safepoint instruction region 内用于 load/store。跨 managed call/invoke、poll、allocation slow path 或 native transition存活时，LIR lowering必须保留 base managed ref，并在 relocate/reload后重新计算地址。

post-RS4GC verifier 拒绝：

- base/derived index不同的 `gc.relocate`；
- AS1 GEP结果跨 safepoint；
- 没有与operation kind匹配的internal typed witness的AS1 `ptrtoint`/`inttoptr`/`addrspacecast`，以及任何由普通LIR `PtrToInt`产生的managed转换；
- raw/code/metadata pointer进入 `gc-live`；
- aggregate 内含 AS1 leaf却没有对应 `StatepointLiveSet` leaf。

### 5.7 codegen pass 与 verifier

IR pass顺序固定为：完成LLVM IR → verifier → 必需的SSA construction/SROA → `rewrite-statepoints-for-gc` → M15 post-RS4GC verifier → object emission。RS4GC必须在SSA construction后运行；不得继续依赖“只跑RS4GC，LLVM也许会顺手处理alloca”。object emission使用LLVM 22.1标准SelectionDAG/TargetMachine machine pipeline；其中register allocation后的`FixupStatepointCallerSaved`是stack-only root契约的一部分，不是可选优化pass。

post-RS4GC verifier 至少检查：

- managed function具有 GC strategy、frame-pointer与disable-tail-calls属性；`@NoGC`函数无 strategy/poll；
- 每个 `SafepointId` 恰好对应预期 statepoint；
- statepoint没有deopt输入且未设置`DeoptLiveIn`；编译器不解析或透传可改变GC root machine location的LLVM command-line option；
- 普通call及入口/回边poll的root count/leaf identity/relocate dominance完整；
- managed invoke 的 gc-live/relocate均为空，显式 root frame在两个edge对称清理；
- native-safe/native-borrowed call的gc-live/relocate均为空，跨边界value与含ref result只从caller-root storage reload；
- AS1/AS0 provenance、tagged-enum typed ref slot与封闭internal pointer-boundary witness规则；
- 不存在未处理的 managed aggregate live-through。

object-level test再解析Mach-O relocation与stack map，先验证前三个header location及deopt count，再锁定`3 + 2 * root_count` location数、每个GC pair的AArch64 stack-indirect location、stack size与return-PC offset。该测试必须经inkwell 0.10实际使用的LLVM 22.1 `TargetMachine::write_to_file`路径，在O0与项目优化配置下分别覆盖零/单个/多个AS1 root；仅搜索section name或只调用独立`llc`不算主线验收。若任一GC root成为`Register`，测试应将其归类为backend invariant failure，并证明不会把前三个constant或live-out误判为root。

## 6. 精确 stack map 消费

### 6.1 parser 与索引

`stackmap.c` 使用 checked cursor 解析 LLVM v3：header、function records、constant pool、callsite records、location与 live-out。所有 count/size/alignment 算术必须检查 overflow 和 section bounds；version、reserved field、record总数、function record count合计或 location kind不符合 profile时，在 runtime init 阶段 fatal。

初始化后构建不可变索引：

```text
return_pc -> {
    SafepointId,
    function_start,
    stack_size,
    managed_root_pairs,
}
```

key 是 dyld fixup 后的 `function_address + instruction_offset`。重复 PC、PC 不在 executable text、stack size非静态/非16字节对齐、root offset越出 frame都非法。原始 LLVM section初始化后只读；collector使用紧凑索引，不在每轮重新解析。

### 6.2 managed frame walk

每个 parked managed thread 发布包含top managed callsite PC/SP/FP的opaque `ManagedAnchor`。collector从 top frame开始：

1. 以 anchor return PC精确查 record；
2. platform adapter求出该 frame SP/FP；
3. 为每个 root pair解析出可写 `ManagedSlot`并访问；
4. 沿 AArch64 frame record取得外层 FP与 return PC；
5. 到当前 managed segment boundary停止。

每个外层 managed frame都必须命中 stack map。未知 PC不是“可能是无根帧”，而是 metadata/codegen错误。runtime/native frame不在这条起点之后：managed runtime entry负责直接发布其 managed caller anchor。

native-safe/native-borrowed transition冻结的外层 managed segment在 M15 不再扫描内存，也不再尝试从 native frame反向 unwind。进入 native前由 codegen发布的 caller root frame就是该段跨 boundary 的完整 live set；callback重新进入 managed后，活动 callback segment走 stack map，外层冻结段仍只走显式 root frame。

`jmp_buf` 不能继续充当 opaque register/root描述；M15 thread state改持有 profile定义的 anchor/context。保守 `gc_scan_range` 与 heap-start过滤路径删除，而不是保留为 debug fallback。

## 7. managed global、immortal 与 external object

LIR 的 managed global storage直接携带完整 `RefScan`；codegen不能根据 initializer或symbol判断是否为root。单 image产物发射：

- managed global storage descriptor table `scoop_image_managed_globals`及其`u64` count `scoop_image_managed_global_count`，record为`{ writable_base, scan }`；
- immortal managed object table `scoop_image_immortal_objects`及其`u64` count `scoop_image_immortal_object_count`，record为`{ object_start, object_size, td }`。

count是表长度的唯一权威；零长度表仍发射一个全零sentinel record，使每个image都从结构上定义四个symbol，而不是让runtime用weak symbol或“symbol不存在”猜测表是否存在。

runtime init在任何 managed代码执行前登记两张表。GC-free global保留显式 `RefScan::None`，不进入 visitor；不能用缺失字段表达“可能无root”。M17 多 Cone时把同一记录并入 image registration，不改变 collector接口。

immortal object地址稳定且不进入 forwarding。M15只允许没有 managed出站引用的只读 immortal object；若未来要支持含 ref 静态 closure/object，必须先增加可写 relocation storage或启动期堆物化，不能把只读 section中的 ref slot交给 collector写入。

ABI exception buffer是动态 stable external object：注册期间其 base可作为合法 managed ref，object内部字段按TD scan更新；结束 catch/物化后注销。未登记的外部地址出现在 managed slot中为 fatal。

## 8. arena 外 side metadata

GC arena 中不再存放任何 collector必须读取的元数据。以 arena block index定位的 side metadata至少包含：

- block state：never-used / mutator / evacuation-source / evacuation-target / pinned-partial / free / quarantined；
- small/large kind与 large span；
- object-start及每个对象的精确 normalized allocation size；
- line occupancy/live/pinned信息；
- allocator free-run信息；
- collection期间的 forwarding/status索引。

all-block list、free-block list和hole list同样使用 arena 外 node或block index。通过 object address masking只能得到 block index，随后查 side table；不能把 block base cast为 header。

object allocation commit顺序为：完整清零 → 写对象头 → 写 exact size/meta → release发布 object-start。collector看到 start时，TD、size和payload必须已经可安全扫描。large object的实际size不能由block span或TD fixed size猜测。

这项重构也修复stress保护后的元数据可达性，并为 ordinary compaction区分from-space/to-space角色。

## 9. moving collector

### 9.1 collection phases

一次 collection 严格按以下阶段执行：

1. **Stop**：沿用 M13 epoch handshake停住managed/native-borrowed线程；native-safe线程已发布caller roots。retire所有TLAB；
2. **Exact mark**：在world stopped且root/heap锁顺序固定时，以`Mark` visitor从全部root source建立本轮live object/line与pin占用；扫描入口仍是可改写slot，不能读取保守word；
3. **Plan**：普通模式按本轮live占用率/碎片选择 source block；stress选择所有含未pin live object的block。destination block在本轮不得同时为source；
4. **Forward roots**：以`Relocate` visitor访问所有root slot；首次遇到source中的未pin对象时，以exact size从GC内部evacuation allocator取得新空间、复制完整对象、登记to-space object meta并建立forwarding；
5. **Trace/update**：按queue扫描to-space对象、未移动对象、pinned对象及external root object的字段；每个ref leaf再次走同一 `Relocate` visitor，直到闭包；
6. **Update side roots**：handle table entry已作为slot更新；pinned registry自身不变；重置card table；
7. **Verify**：重新枚举所有root与所有存活对象slot，确认只指向null、current heap start或已登记stable/immortal object；forwarded old address不得残留；stress确认每个live movable object地址变化；
8. **Retire source**：清除旧start/size/forwarding状态，poison或回收source空间；
9. **Resume**：重建普通allocation free runs，保持TLAB为空，release world。

任何阶段失败均 fatal，不允许恢复到旧堆继续执行。

M15在判定对象不可达或回收from-space时不调用用户代码。evacuation、旧副本poison与block quarantine只是同一逻辑对象的存储迁移/回收步骤，绝不构成析构事件。未来GC-free release hook若加入，armed状态必须随forwarding转移，不能因一次移动对同一资源重复执行清理。

### 9.2 evacuation allocator

evacuation allocator是collector私有路径：

- 不读取/修改mutator TLAB；
- 不检查threshold，不发起递归collection；
- 只从本轮destination block分配；
- 失败时报告“to-space exhausted”并fatal，不能覆盖from-space做未验证的原地压缩；
- copy后保留TD与pin以外的合法header状态；未pin对象不会在复制中意外获得pin。

普通模式可在cycle完成后把全空source block还给free pool；有pin对象的source block留下，已搬走区域形成新free run。只要存在可移动live object且to-space足够，一次显式full collection至少选择一个eligible source，不能长期退化成永不搬动的mark/sweep。large object按exact size迁移到独立destination span。

## 10. stress mode

runtime-only测试开关命名为 `SCOOP_GC_STRESS_MOVE=1`；它不是语言、稳定CLI或程序可观察API。runtime启动时解析一次并保存不可变配置。

stress mode规则：

- 禁用TLAB fast allocation：thread allocation cursor/limit始终为空；
- 每次 mutator-visible managed heap allocation（含box、Array、closure、coroutine frame及runtime helper分配）在创建新对象前完成一次full moving collection；pinned allocation同样触发collection，但新分配对象自身按请求保持pin；
- collector内部evacuation allocation设置不可重入状态，不触发stress；
- 本轮所有live movable object必须移动，不能因occupancy heuristic留在原址；
- 完成slot验证后，以固定pattern覆盖旧对象的完整exact size；
- 若source block不再含live/pinned object，整块设为`PROT_NONE`并标记quarantined，在该进程余下生命周期永不解除保护、永不重新分配；
- 含pin对象的partial block不能整块保护，但所有已搬旧副本仍poison，相关span在stress进程中不复用，避免旧指针偶然命中新对象；
- quarantine耗尽1 GiB测试arena时给出明确stress exhaustion fatal；stress fixture应控制规模，普通模式不承担永久quarantine成本。

故意读取旧裸地址的测试必须在子进程执行，并在 macOS/AArch64 上断言因受保护block得到确定的异常终止；不能在同一测试进程内捕获未定义行为后继续。

## 11. runtime/native boundary审计

M15逐项审计所有`ManagedEntry`与`NativeBorrowedEntry`及其内部调用图。任何 C/runtime helper只要在持有managed输入时可能再次分配或park，就必须：

1. 在第一个可能safepoint前把direct ref写入native root slot；含ref payload使用完整recursive scan descriptor；
2. push严格LIFO root frame；
3. 调用可能GC的入口；
4. 返回后从slot reload，旧C local副本不可再用；
5. 在所有正常出口pop。runtime ABI不允许异常穿过该C frame。

至少覆盖 String concat/toString分配、Array clone、box payload、异常materialize、callback invoke临时closure/exception以及所有公开allocation wrapper。每个wrapper还必须在registry/header中属于上述唯一入口类，并有错误入口类的link/compile negative。需要扫描box/aggregate payload的入口必须显式接收typed scan descriptor或由callsite发布完整root frame；不得按TD名称、payload size或symbol猜布局。

Scoop ABI外部实现继续遵守runtime spec 4.2：native-borrowed代码在调用GC入口前自行登记slot，并在返回后reload。M15测试必须使用旧地址poison证明漏登记不能再“碰巧工作”。

## 12. 测试与验收

### 12.1 IR / artifact

- target registry把macOS的`aarch64`/`arm64`输入规范化为同一`DarwinAarch64` profile，拒绝`arm64e`和其他OS/CPU；profile所有capability与bundle operation均完备；
- 构建/driver拒绝实际链接LLVM major/minor不是22.1的编译器；测试环境显式选择22.1而不接受PATH中碰巧优先的其他`llvm-config`；
- 通用codegen/GC模块的依赖测试禁止导入具体target/profile实现；fake platform bundle运行stackmap/root/collector unit，证明平台实现可替换；
- AS1 direct ref跨call或poll产生`gc-live`与`gc.relocate`；post-site只使用relocated value；
- 函数入口与每条循环回边都已有LIR `ManagedPollSite`及完备live set，codegen不会自行插入或漏掉poll；
- struct/tuple/tagged enum/closure/coroutine aggregate每个ref leaf单独relocate；GC-free leaf不进入；
- managed invoke由codegen直接发射唯一ID的显式statepoint、零gc-live、零exceptional relocate，normal/unwind按各自edge flag reload并pop显式frame；
- native-safe/native-borrowed call有唯一statepoint ID、零gc-live/relocate，transition前后root/result publication与reload顺序完整；
- derived address跨safepoint、无typed witness或witness kind错误的managed pointer转换为verifier negative；
- Mach-O v3 parser golden覆盖所有location编码、alignment、constant pool和truncated/corrupt输入；Darwin profile对unsupported root shape给出确定错误；
- linked executable runtime检查dyld fixup后的function address和return PC均落在预期代码范围；离线测试用`dyld_info`/Mach-O parser确认chained fixup，而不把磁盘编码误读为指针；
- AArch64反汇编锁定frame chain、return address等于stack-map PC、root为8-byte indirect SP/FP slot；O0与项目优化配置的真实AS1 fixture均不得产生GC register root。

### 12.2 collector/runtime unit

- forwarding唯一性、环/共享子图、self-reference；
- side metadata exact size覆盖短/长String、Array、small/large object；
- global、handle、pin、external exception、native root、compiler invoke root逐类更新；
- pinned partial block只移动其他对象且更新pinned出站ref；unpin后下一轮可移动；
- stress每轮地址变化、poison、empty block `PROT_NONE`与永久quarantine；
- malformed root、旧from-space root、interior pointer、未登记external/immortal address全部fatal；
- 对象不可达、ordinary/stress evacuation及quarantine都不会调用managed finalizer或其他用户代码；
- 多mutator parked/native-safe/native-borrowed及foreign callback同时存在时精确更新；
- 删除保守扫描后，伪装成heap地址的普通整数不会保活对象。

### 12.3 fixture

普通moving模式运行M1–M14全部fixture。专门组合至少包括：

- local/parameter/loop phi与多层managed call；
- 含ref struct/tuple/tagged enum/Array与interface dispatch；
- catch/finally/rethrow、external exception materialize；
- closure capture、函数型变adapter、挂起coroutine frame及跨线程resume；
- `GcHandle`、pin/unpin、Scoop ABI native root reload；
- pthread foreign callback与另一个mutator同时请求GC；
- managed global与静态String；
- stale raw address子进程崩溃。

选定的GC/FFI/exception/closure/coroutine/callback组合还要在stress mode执行。普通输出正确但对象地址未按模式改变，或runtime没有实际消费stack map，均视为失败。

## 13. 实现顺序

1. 同步runtime/impl spec与ROADMAP，固定LLVM 22.1与DarwinAarch64唯一profile、AS1、stack-only exact root及moving契约；
2. 拆出codegen target/statepoint模块和runtime gc/platform模块，先保持M13行为等价；
3. LIR加入按effect类型隔离的managed poll/call/invoke root plan、唯一`SafepointId`、managed global scan与niche pointer provenance，并由LIR lowering显式插入入口/回边poll；
4. codegen完成AS1映射、frame属性、aggregate leaf materialization、invoke显式root frame、native transition caller-root/result协议和post-RS4GC verifier；
5. 实现通用v3 parser与DarwinAarch64 section/frame/location adapter，先用非移动collector精确枚举所有root并与测试期影子结果对照；
6. 删除conservative scan，加入global/immortal/external精确地址域和runtime managed-input root审计；
7. 把heap block/object/free-run元数据迁出arena并记录exact allocation size；
8. 实现forwarding、evacuation、slot update与ordinary moving；
9. 实现stress pre-allocation collection、poison、`PROT_NONE` quarantine；
10. 完成artifact/unit/negative/组合fixture和M1–M14全量回归。

每一步先完成对应数据结构和反向检查，再允许下游消费；不得临时让codegen/runtime通过名称、地址范围或保守扫描补齐缺失信息。

## 14. 完成条件

M15只有在以下条件同时满足时完成：

- LLVM 22.1及其SelectionDAG/标准TargetMachine stack-only statepoint pipeline是显式backend profile，版本不符或GC root出现`Register`均作为compiler/toolchain invariant failure；
- macOS/AArch64是显式target profile，其他平台明确拒绝；平台细节仅存在于target registry/profile与runtime image/OS/arch组件，通用模块可在fake bundle下测试；
- stack map中的每个预期root真实存在、runtime按精确return PC消费且可写回；无保守fallback；
- 每个跨普通managed call或poll的AS1 leaf生成并使用`gc.relocate`；入口/回边poll在LIR中显式且root plan完备，aggregate、invoke与两类native transition分别按本设计处理；
- 全部root/heap路径统一经可改写slot visitor，非法地址确定失败；
- object exact size、block/free-run/forwarding元数据全部位于arena外；
- ordinary模式确实执行evacuation，stress模式每次allocation前移动全部未pin live object；
- M15没有GC finalizer或release hook执行路径，moving/poison/quarantine不会被误当成析构；
- poison与`PROT_NONE`测试能确定暴露旧地址使用；
- pin、handle、异常、native boundary、callback与coroutine在移动后保持语义；
- M1–M14 fixture普通moving全量通过，指定stress矩阵通过。

参考：[LLVM 22.1 Statepoints](https://releases.llvm.org/22.1.0/docs/Statepoints.html) 与 [LLVM 22.1 Stack Maps](https://releases.llvm.org/22.1.0/docs/StackMaps.html) 定义RS4GC、address space 1、`statepoint-id`及v3二进制格式；[LLVM 22.1.8 StatepointLowering](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/CodeGen/SelectionDAG/StatepointLowering.cpp)、[FixupStatepointCallerSaved](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/CodeGen/FixupStatepointCallerSaved.cpp)与[TargetPassConfig](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/CodeGen/TargetPassConfig.cpp)共同构成当前stack-only machine lowering的qualification依据。项目固定使用LLVM 22.1；升级不属于当前路线，未来决定升级时必须先建立新的完整backend profile并重跑IR、machine与artifact契约测试。

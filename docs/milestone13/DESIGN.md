# M13 设计：多线程 GC 与 foreign-thread managed callback

版本：1.0（已实现，2026-09-01）

对应 `docs/ROADMAP.md` 的 M13。目标：把 M9 的单 mutator、单线程 runtime 升级为**多 mutator、stop-the-world、单线程 collector、非移动**的正确性基线，并实现带显式 `void *context` 槽的 GC-aware managed closure 反向回调。完成后，`pthread_create` 一类 C API 可以在 foreign thread 上执行普通 Scoop closure；closure 内可以分配、触发 GC、抛出 Scoop 异常或恢复既有 continuation，而不会把 managed ref、Scoop 异常或未登记线程暴露给 C ABI。

## 0. 范围与关键决策

M13 交付以下闭环：主线程注册普通、非挂起 closure，runtime 用 `GcHandle` 保活它并返回“静态 C trampoline + opaque context cookie”；C 创建 foreign thread并把 cookie传入；新线程经 callback gateway自动 attach，进入 managed状态并调用 closure；closure可以与其他 mutator并发分配并触发 STW GC；返回或抛出后 gateway恢复 native状态、消费约定的 callback ownership并按需detach；原线程在 `join` 后读取完成/失败状态，重新抛出受管异常并释放最后一份 ownership。

- M13 不新增 Scoop 语言级 `Thread`、线程池或调度器。线程创建、`join`、mutex和barrier均通过 M12 C FFI或runtime测试设施提供；
- collector仍为单线程、单代、非移动的 Immix mark-region collector。M13只让多个mutator安全共享它；精确stackmap、root relocation与evacuation进入M15，parallel/concurrent marking、分代和晋升仍待后续；
- STW采用合作式握手，不用异步signal在任意指令处抢停线程。managed线程在函数入口、循环回边和runtime入口poll；native-safe线程已经发布roots，不阻塞GC；native-borrowed线程必须返回或进入显式runtime/managed入口后才能确认停顿；
- C ABI outbound与Scoop ABI direct-ref outbound是两种不同状态。前者进入`native-safe`，collector不等callee返回；后者进入`native-borrowed`，普通native指令区间不被扫描或移动，只有遵守native-root协议的显式入口能参与握手；
- M13继续使用经过heap-object-start校验的保守栈扫描。被停住的managed栈可以保守扫描；native区间只扫描进入native前冻结的managed栈段、编译器发布的caller roots和显式native-root链，不扫描仍在运行的未知native栈。因为collector不移动，过度保活不破坏正确性；M15必须以精确stackmap消费替换该过渡路径后才能移动对象；
- managed callback与M12 `FunPtr`严格分离。`FunPtr`仍只来自合格的顶层`@NoGC ::name`；M13不会把closure转换为`FunPtr`，而是为每个concrete C callback signature生成可复用的静态trampoline，并以context cookie选择closure实例；
- managed callback只接受ordinary、非`suspend` closure。callback body可以调用普通managed代码，也可以通过已存在的普通`Continuation<T>.resume`恢复协程；它本身不能是suspend callback，`@Extern suspend`禁令不变；
- 首版只支持C API显式提供一个`void *context` / `user_data`参数的形态。context槽由runtime token独占，不作为源码closure参数传入；没有context槽的API继续只能使用M12静态`FunPtr`；
- callback异常在managed adapter内全部捕获并物化为managed异常对象，再由token中的异常`GcHandle`保活。C trampoline按签名返回全零值，异常绝不展开穿越C frame；
- M13保证runtime、GC元数据、callback token和编译器生成的continuation状态机无数据竞争。普通managed对象字段的并发读写不因此自动安全；没有native同步或未来memory model约束的冲突访问仍不受支持。

### 0.1 HIR消费者边界前置修正

M13新增的enum GC布局与callback concrete signature使早期单一`hir::Module`同时容纳generic template和concrete实例的问题不可继续保留。按impl spec 2.2，HIR输出必须先按消费者拆为互不兼容的`ExportHir`与`LocalConcreteHir`：

- `ExportHir`供下游Cone的HIR使用，包含导出的非generic语义接口、generic template、const/default metadata及template依赖闭包；
- `LocalConcreteHir`只供本Cone MIR使用，所有type parameter均已替换，所有non-generic/instantiated body与type均为concrete；
- 两侧的type/function/callable/variant id使用不同Rust类型。MIR API不能接收`ExportHir`，`.slib`也不打包`LocalConcreteHir`；
- 每个`ConcreteTypeId`必有`gc_free: bool`。concrete enum的每个variant也必有`gc_free: bool`，enum自身flag为逐variant AND。禁止`Option<bool>`、默认值、后续全模块扫描或LIR字段递归补齐；
- MIR创建closure、coroutine frame、adapter等synthetic concrete type时，创建实体的同一操作必须生成完整flag。这是MIR自有新实体的完备构造，不是对HIR缺失信息的补丁。

本节是enum fixed-slot布局、caller-root scan和foreign callback signature lowering的输入前提；未完成该隔离前不得把generic节点直接送入MIR以推进M13。

## 1. 线程与 mutator 模型

### 1.1 `ScoopThreadState`

每个已注册OS线程恰好对应一个runtime-owned `ScoopThreadState`，TLS只保存指向它的指针。主线程、未来runtime创建的线程和foreign thread使用同一结构，不再保留“主线程全局栈顶 + 若干零散TLS”的双轨状态。

概念字段如下；真实C布局属于runtime私有实现，只有分配fast-path所需的最小ABI前缀可以暴露给codegen：

```
ScoopThreadState {
    thread_id
    atomic mode
    atomic observed_gc_epoch
    stack_low / stack_high
    parked_sp / register_spill
    managed_stack_boundary
    native_root_head
    current_transition
    tlab
    managed_depth / callback_depth
    attachment_kind
    registry_links
}
```

- `stack_low/high`在attach时从平台线程API取得，不再由单一`gc_stack_base`全局变量表示；
- `native_root_head`取代当前独立的`_Thread_local gc_native_roots`，由当前线程独占修改，STW后由collector读取；
- TLAB属于thread state。collector开始扫描前使所有TLAB失效，回收完成后由各线程重新refill；
- thread state的地址在detach前稳定。全局线程登记表保存这些稳定地址，并由world lock保护；
- M13支持的host基线是64位POSIX pthread。Linux使用`pthread_getattr_np`取得栈边界，macOS使用`pthread_get_stackaddr_np` / `pthread_get_stacksize_np`；Windows线程、SEH和cross target仍不在本里程碑。

### 1.2 attach / detach

runtime提供等价于以下概念入口的C API：

```
bool scoop_runtime_attach_foreign_thread(void)
void scoop_runtime_detach_foreign_thread(void)
```

`attach`返回本次调用是否新建了attachment。callback gateway只在返回`true`时拥有对应detach责任；已由主线程启动、长期显式attach或外层callback attach的线程可以嵌套进入callback，但内层入口不能误删外层thread state。

attach顺序固定为：

1. 检查runtime处于`Running`；
2. 取得当前线程栈边界并分配/初始化thread state；
3. 在world lock下等待正在进行的STW结束，登记线程；
4. 发布TLS，初始状态为不持有managed ref的`native-safe`；
5. callback gateway随后显式进入`managed`。

detach只允许在`native-safe`、`managed_depth == 0`、`callback_depth == 0`、native-root链为空且没有活动TLAB时执行。它先在world lock下从登记表移除，再清TLS并释放thread state；STW的线程快照因此不会观察到半注销实体。违反LIFO、带root detach或double detach属于runtime ABI错误并终止。

主线程启动改为`runtime init → attach main → enter managed → scoop_main → leave managed → detach main → shutdown`。shutdown先进入`ShuttingDown`，拒绝新attach和callback注册；若仍有foreign attachment、活动callback或未释放token，给出计数诊断并终止，而不是释放GC状态后允许迟到callback进入悬空runtime。

### 1.3 线程状态

| 状态 | 含义 | STW处理 |
|---|---|---|
| `managed` | 正在执行有GC strategy的Scoop代码或managed runtime入口 | 必须在poll/runtime入口保存寄存器和SP后park |
| `native-safe` | 不持有未发布的managed ref；典型为C ABI call、`@NoGC`区间或已attach但尚未进入managed的foreign thread | 已是quiescent；扫描冻结managed栈段和发布root，不等待native返回 |
| `native-borrowed` | Scoop ABI native code正借用direct ref | 不能在普通指令间抢停；等待其返回或进入显式runtime/managed入口 |
| `parked` | 已对当前GC epoch确认停顿 | collector按`parked_from`选择根扫描方式 |
| `collector` | 发起本轮GC并执行mark/sweep的线程 | 扫描自身roots后执行单线程collector |
| `detaching` | 已退出执行状态、正与登记表解除关系 | 与STW在world lock下串行化 |

状态写入使用release，collector用acquire读取。每次边界转换使用位于native栈上的`ScoopThreadTransition`记录previous mode、当前managed栈段边界和上一冻结段，TLS保存LIFO链。重入managed callback会建立一个新的当前managed段；collector沿transition链扫描各managed段并跳过夹在其中的native栈。这样可正确覆盖：

```
managed -> native-safe C call
    -> same-thread managed callback
        -> nested native-safe C call
        -> managed
    -> native-safe
managed
```

不能只用`native_depth`计数，因为`native-safe`和`native-borrowed`的GC语义不同，且callback需要恢复到准确的外层状态。

### 1.4 native-safe 与 native-borrowed

进入两种native状态前，codegen都记录当前managed栈边界，并把当前函数内所有**跨该调用仍活跃**的managed reference leaf发布到编译器生成的root frame。含引用的struct/enum/tuple按LIR递归扫描描述枚举leaf；SSA临时值先spill到隐藏slot。未初始化local、raw `Ptr`、`FunPtr`、TypeDescriptor和其他metadata pointer不得混入root列表。

compiler caller-root frame中的每个entry由“可寻址value storage + 静态递归scan descriptor”组成；普通managed ref使用`refs[0]`，aggregate复用与heap field相同的`References / Sequence`扫描程序。tagged enum按spec 7.4让pure-value variant复用共享payload区，并只为每个直接或间接含managed ref的variant保留独立slot；构造时清零inactive ref-bearing slot，因此caller-root与heap扫描都直接使用所有ref-bearing variant ref leaf的固定偏移，不读取tag。Scoop ABI调用若返回含ref的值，codegen还要在push前把对应result storage清零并作为entry发布；native call写入结果后、`leave_native_borrowed`可能park之前，该storage已经是有效root。返回managed后从storage reload结果。这样caller frame从发布起没有未初始化扫描字节，并覆盖“callee已返回、caller尚未恢复managed”的窗口。

- C ABI call以及从managed代码进入可能无界执行的`@NoGC` callable：push caller-root frame → enter native-safe → call → leave native-safe（先检查GC epoch）→ pop frame并reload roots；
- Scoop ABI managed extern：push caller-root frame → enter native-borrowed → direct typed call → leave native-borrowed（先检查GC epoch）→ pop/reload；
- Scoop ABI callee若跨runtime入口继续持有direct ref，仍必须自己push native roots并在入口返回后reload。caller发布root只保证managed调用链保活，不能替代callee对移动后地址更新的责任；
- native-safe线程执行的native栈不扫描。collector只扫描进入native前冻结且不会再被该线程修改的managed栈段、caller-root frame和native-root链；若native代码重入managed callback，则另扫描callback的当前managed段，仍跳过两段之间的native frame；
- native-borrowed线程在普通native代码中不确认GC。它到达runtime入口时，入口先验证/发布当前root链并参与握手；到达managed callback入口时按同一规则切入managed。未登记root而跨入口保存direct ref是native ABI错误，runtime不通过隐式pin来兜底。

M13继续非移动，因此native-borrowed造成的等待只影响延迟，不涉及对象地址变化。M15引入moving collector时仍必须保留该状态差异并完成精确root更新，不能把所有direct ref预先pin住来规避协议；未来concurrent collector也复用同一边界。

## 2. Stop-the-world 协议

### 2.1 GC epoch 与握手

runtime维护单调递增的`gc_epoch`、`world_phase = Running | Stopping | Collecting`、world mutex/condition variable和collector mutex。任何managed slow allocation或测试强制入口都可请求GC，但同一时刻只有一个线程成为collector；其他同时请求者参加当前epoch并等待结果。

一轮GC按以下顺序进行：

1. requester在collector mutex下建立新epoch，在world lock下把phase改为`Stopping`，发布请求并取得已注册线程快照；
2. requester把自身切到collector状态并保存roots；
3. `native-safe`线程立即计为quiescent；`managed`线程在下一poll/runtime入口park；`native-borrowed`线程在返回或进入显式协调点后park；
4. 每个park线程发布SP、register spill、root head和observed epoch后唤醒collector；
5. 全部目标确认后phase进入`Collecting`。attach/detach、TLAB refill和root registry结构变更在此期间不能与快照交错；
6. collector使全部TLAB失效，扫描根、mark、sweep并重建free-line元数据；
7. 发布完成epoch，把phase恢复`Running`，广播唤醒park线程；线程以acquire读取完成状态，恢复原mode并按需reload root slots。

M13不使用超时把仍在`native-borrowed`的线程强制视为安全。测试可提供debug timeout并打印阻塞线程id/mode，生产语义则是合作式等待；阻塞native-safe调用不应出现在等待集合中。

所有“进入managed / 离开native-safe / 离开native-borrowed”转换都必须与epoch发布形成无丢失握手：转换方读取`world_phase/epoch`、发布新mode后再次确认epoch未变；若观察到`Stopping`或epoch变化，就在执行第一条managed指令前park。collector先release发布`Stopping`再读取线程mode。不能存在collector已把某线程按native-safe计为quiescent，而该线程同时未经复查进入managed的窗口。

### 2.2 safepoint

`scoop_rt_safepoint`不再是空函数。fast path只读取TLS与全局epoch；未请求GC时立即返回，请求存在时进入slow path：

- 用`setjmp`等平台机制把callee-saved寄存器溢出到thread state；
- 记录当前SP并以release发布`parked`；
- 等待本轮epoch完成，再恢复先前mode；
- poll调用本身继续作为LLVM statepoint，使现有stackmap通道保持有效。

poll仍位于每个managed函数入口和循环回边。所有可能分配、修改root/handle/pin表或从native切回managed的runtime入口也先执行等价epoch检查，避免只依赖用户循环碰到poll。`@NoGC`函数没有poll；从managed进入可能长时间运行的NoGC区间必须按1.4进入native-safe，否则一个阻塞NoGC C call会使STW永久等待。

### 2.3 根扫描

M13按线程停顿来源选择扫描集合：

- `managed -> parked`：扫描register spill、当前SP到本次managed-entry边界的当前managed段、transition链保存的外层冻结managed段、native-root链和runtime全局roots；不把夹在managed段之间的native frame纳入保守扫描；
- `native-safe`：扫描冻结managed栈段、编译器caller-root frame和native-root链，不扫描当前正在变化的native栈；
- `native-borrowed -> parked`：只在显式入口确认后扫描冻结managed栈段、caller roots和callee登记的native roots，不把未知native局部当managed ref；
- collector自身：在进入collector前spill寄存器，按managed requester规则扫描；
- callback token中的closure/异常经全局`GcHandle`表进入根集合，token native内存本身不按对象扫描。

保守扫描的每个word仍先通过arena、block和object-start三级验证；只允许把对象起点当managed ref。该方案可能因managed栈中的陈旧word过度保活，但不会解引用foreign地址。M13验收不宣称“精确GC”或“可移动”；runtime消费`__llvm_stackmaps`及root relocation由M15完成。

## 3. 并发分配与GC元数据

### 3.1 per-thread TLAB

当前全局`gc_bump_pos/end`改为每线程TLAB。slow path在heap lock下从free-line run或新block中切出互不重叠的整line区间；线程只在自己的cursor/limit内分配。大对象、block table、arena free-block list、hole/refill list和collection threshold仍走全局slow path。

LIR保留类型化`ManagedAlloc`；codegen针对runtime公开的最小TLS allocation context发射cursor/limit碰撞检查，失败才调用`scoop_runtime_alloc_slow`。成功的内联bump之后调用不含safepoint的GC-leaf `scoop_runtime_finish_tlab_alloc`完成清零、对象头和start bitmap；这仍属于fast path，不进入heap/world锁。fast path整体必须：

- 保证小对象不跨Immix line并按8字节对齐；
- 以atomic OR登记object-start bitmap，避免两个TLAB虽不重叠却更新同一bitmap word时形成数据竞争；
- 清零完整对象存储，再写`td`和当前allocation mark color，使构造期间尚未写入的引用字段为null；
- 在对象完成初始化并经普通managed store发布前，不向其他线程暴露它。

collector在STW内退休全部TLAB；未使用tail与其他空line一起重建为hole。GC结束后不把旧cursor重新交还线程，下一次分配统一refill。M13不做per-thread nursery或work-stealing allocator。

### 3.2 元数据同步

至少分离以下同步域，且在runtime内部固定锁顺序，不能用一个递归global mutex掩盖重入问题：

1. world/registry：线程登记、phase、epoch、park/wakeup；
2. heap：arena、block/hash table、free line、large allocation和sweep元数据；
3. roots：全局root、外部异常root、handle和pinned registry；
4. callback registry：opaque token slot、owner/active计数和失败状态。

申请GC时不得持有heap/roots/callback lock；callback registry lock也不得跨managed adapter调用持有。建议锁顺序为`world → heap → roots → callback`，反向需要的数据先做带transient lease的快照再释放锁。

具体并发规则：

- `GcHandle`升级为带generation的64位`(generation, slot)`编码，slot复用时generation递增，0仍为null niche；表操作在roots lock下完成，避免旧handle在slot复用后错误指向新对象；
- pin bit使用atomic RMW，pinned registry在roots lock下更新。现有pin非引用计数语义不变：多个业务owner必须自行协调unpin；
- native-root链由owner线程按LIFO修改；collector只在该线程quiescent后读取，不需要每次push/pop获取全局锁；
- object mark bit只由STW collector写，allocation color由TLAB快照初始化；
- card table即使M13 full collection仍不消费，也改为`atomic i8 monotonic store 1`。多个mutator对同一card做普通store在LLVM内存模型中仍是数据竞争，不能以“写入值相同”豁免；
- runtime全局统计使用atomic或在相应锁下维护；测试hook不得读取撕裂的计数。

## 4. Managed callback 源码协议

### 4.1 core 形态

M13在`scoop.core.ffi`增加以下概念API；具体声明由HIR验证为唯一core contract：

```
enum ForeignCallbackMode {
    Reusable,
    OneShot
}

enum ForeignCallbackState {
    Registered,
    Active,
    Completed,
    Failed
}

struct ForeignCallback<F>(
    val function: FunPtr<F>,
    val context: Ptr<Unit>
)

@Unsafe
@Intrinsic("foreign_callback_register")
fun <F> foreignCallback(
    callback: Any,
    contextIndex: Int,
    mode: ForeignCallbackMode
): ForeignCallback<F>

@Unsafe
@Intrinsic("foreign_callback_retain")
fun <F> retainForeignCallback(callback: ForeignCallback<F>): ForeignCallback<F>

@Unsafe
@Intrinsic("foreign_callback_release")
fun <F> releaseForeignCallback(callback: ForeignCallback<F>)

@Unsafe
@Intrinsic("foreign_callback_state")
fun <F> foreignCallbackState(callback: ForeignCallback<F>): ForeignCallbackState

@Unsafe
@Intrinsic("foreign_callback_failure")
fun <F> foreignCallbackFailure(callback: ForeignCallback<F>): Throwable?
```

这里的`callback: Any`只是core源码无法写出“从F删除context参数后得到的函数类型”的声明占位，不产生`Any`装箱或运行期类型猜测。HIR在普通调用lower之前识别该已验证intrinsic，根据显式`F`和`contextIndex`构造managed closure的精确expected function type，再按M11规则双向检查lambda/callable value。下游只收到类型化registration实体。

规则如下：

- 调用`foreignCallback`必须显式给出`F`；`F`是ordinary、非suspend、完全具体化的C callback函数类型，所有参数/返回值必须C-FFI-safe；
- `contextIndex`必须是编译期Int常量，落在参数范围内，且该参数类型精确为`Ptr<Unit>`。从`F`的参数列表删除该位置后，得到managed closure的ordinary concrete函数类型；返回类型保持不变；
- `mode`必须是编译期可确定的`Reusable`或`OneShot`。HIR把它写入`ForeignCallbackRegistration`，不能把未知runtime enum值推给adapter猜测；
- closure可以是lambda、匿名函数、局部函数引用、绑定引用或已有managed函数值；仍遵守M11只捕获不可重新绑定binding的规则。它不要求`@NoGC`，但不得是suspend函数类型；
- `ForeignCallback<F>`是GC-free值，两个字段分别传给C API；它自身不是`@CLayout`聚合，不作为单个C参数传递；
- `ForeignCallback` core contract把`F`登记为deferred callback-signature参数，因而允许其字段写`FunPtr<F>`；每个实际应用仍必须把`F`具体化为合法函数类型。这是已验证core类型的局部规则，不新增一般性的`function` kind bound，也不允许用户generic struct用未约束类型参数绕过`FunPtr`检查；
- 用户不能直接构造有效的`ForeignCallback`。与非null `FunPtr`相同，HIR只允许registration intrinsic产生其非空function/context组合；
- registration、retain/release和状态观察均为unsafe：语言当前没有affine/linear owner，复制一个`ForeignCallback`值只是借用别名，不增加owner count。只有`foreignCallback`返回值和`retainForeignCallback`返回值各代表一份可release的逻辑ownership。

`foreignCallbackFailure`返回token保存的首个未处理异常引用；无失败时为`None`。token仍由调用者持有，所以该引用在返回后按普通managed root保活。core可以在它之上提供普通Scoop helper `rethrowForeignCallbackFailure`，但最终`release`仍应放在调用方`finally`中。

### 4.2 ownership mode

`Reusable`适用于会多次调用同一callback的C API：

- trampoline每次进入时取得一份transient active lease，返回时释放；
- 持久owner count不因调用改变；
- 用户必须先按C API执行unregister/stop并完成同步，确认以后不再调用，再release最后一份owner；
- 多个foreign thread可以并发调用同一token。runtime/GC和token状态安全不等于closure捕获的用户对象可无同步并发修改。

`OneShot`适用于`pthread_create` entry等恰好调用一次的API：

- token只有一次从`Registered`原子claim到`Active`的机会，第二次invoke是ABI错误；
- 传给C API的那一份owner视为待转移。C创建成功后Scoop侧不能再release该owner；callback claim时把一份持久owner原子转成active worker lease，callback出口再消费该lease；
- 若C创建失败，ownership未转移，调用者必须release worker owner；
- 需要在`join`侧观察状态时，调用C之前先`retain`一份observer owner。worker完成后token因observer仍存活，状态为`Completed`或`Failed`；join侧读取/重抛后最终release observer；
- callback活动期间另有内部active lease，所以并发错误地提前release外部owner不会立刻释放正在使用的adapter/handle，但callback结束后token会失效；这不把提前release变成合法用法。

### 4.3 `pthread_create` 组合示例

下例省略平台`pthread_t`布局差异，fixture使用一个薄C wrapper稳定它；ownership顺序是验收契约的一部分：

```
@Extern(lib = "m13_fixture", name = "start_worker")
fun startWorker(
    entry: FunPtr<(Ptr<Unit>) -> Ptr<Unit>>,
    context: Ptr<Unit>
): Int

@Extern(lib = "m13_fixture", name = "join_worker")
fun joinWorker(): Int

fun runOnWorker(work: () -> Unit) {
    @Unsafe {
        val worker = foreignCallback<(Ptr<Unit>) -> Ptr<Unit>>(
            callback = fun(): Ptr<Unit> {
                work()
                return Ptr<Unit>(0u)
            },
            contextIndex = 0,
            mode = OneShot
        )
        val observer = retainForeignCallback(worker)
        val createStatus = startWorker(worker.function, worker.context)
        if (createStatus != 0) {
            releaseForeignCallback(worker)
            releaseForeignCallback(observer)
            return
        }

        joinWorker()
        try {
            when (foreignCallbackState(observer)) {
                Completed -> { }
                Failed -> {
                    val failure = foreignCallbackFailure(observer)
                    when (failure) {
                        Some(exception) -> throw exception
                        None -> throw IllegalStateException()
                    }
                }
                Registered -> throw IllegalStateException()
                Active -> throw IllegalStateException()
            }
        } finally {
            releaseForeignCallback(observer)
        }
    }
}
```

成功创建后不能再release `worker`：它已转移给one-shot trampoline并在worker出口自动消费。create失败则callback不会执行，两份ownership都仍由caller持有，必须全部释放。

## 5. Callback 编译与 ABI

### 5.1 类型化实体

HIR为每个registration表达式建立独立`ForeignCallbackRegistrationId`，至少保存：

```
ForeignCallbackRegistration {
    native_function_type: FunctionTypeId
    managed_function_type: FunctionTypeId
    context_index
    mode
    closure_expression
    c_signature
}
```

它不复用M12 `FunctionAddress` / `CallbackBridgeId`，也不把closure先转成`Any`、`Ptr<Unit>`或`FunPtr`。同一源码closure在普通managed上下文和foreign callback注册上下文中仍是同一个concrete函数类型，但registration identity、adapter identity和native trampoline identity各自类型化。

跨HIR/MIR顺序保持“HIR生成`LocalConcreteHir` → MIR closure conversion → callback adapter生成 → coroutine transform”。对每个实际registration，在closure conversion完成后生成typed storage adapter：

```
status adapter(
    closure_ref,
    result_storage,
    arg_storage[],
    exception_root_slot
)
```

- `arg_storage`只包含删除context槽后的参数，adapter按`managed_function_type`逐项typed load后调用closure；
- 非`Unit`正常结果写入`result_storage`；
- adapter是managed entry，带GC strategy/statepoint，可以分配和调用任意普通managed代码；
- 整个closure调用位于catch-all内。捕获异常后先按M10同类规则物化成managed对象、结束native catch，再写入已登记的`exception_root_slot`并返回`Threw`；adapter对C侧表现为`nounwind`；
- adapter绝不能标为`@NoGC`，也不能复用M12静态callback的storage bridge。

### 5.2 静态 C trampoline

codegen按`(C signature, context index)`去重生成静态C trampoline；不同closure实例不生成可执行内存：

```
C_R trampoline(C_A0 a0, ..., void *context, ...) {
    C_R result = {0};
    const void *args[] = { &a0, ... };   // 不含 context
    status = scoop_runtime_callback_invoke(
        context, signature_descriptor, &result, args
    );
    if (status is fatal boundary error) runtime_fatal(...);
    return result;
}
```

- `Unit`返回映射C `void`，不建立result storage；其他C-FFI-safe返回值都可按字节清零，异常时返回该零值；
- `signature_descriptor`是静态只读metadata。token注册时保存同一descriptor；gateway验证cookie没有与错误的trampoline/signature配对；
- C compiler继续负责scalar extension、struct register classification和aggregate return，与M12 outbound bridge采用同一`CType`树和`_Static_assert`布局；
- `ForeignCallback.function`指向这个静态trampoline，`context`是token cookie。函数地址相同不代表closure identity相同；
- 该function值不是对`::name`做native-address resolution的结果，不放宽spec 13.10的`FunPtr`来源规则。

### 5.3 LIR 表示

M13补充分离GC含义所需的LIR结构：

- raw pointer/code pointer/managed reference不能继续全部只靠同一个无provenance `Ptr`猜测；至少以`PointerKind::{Managed, Raw, Code, Metadata}`或等价类型信息保留到root liveness和codegen完成；
- native call effect至少区分`ManagedSafepoint`、`NoGc`、`NativeSafe`、`NativeBorrowed`，不能把它们压成`GcLeaf: bool`；
- 增加compiler root-frame描述：live value storage及其递归scan、零初始化的含ref native result storage、冻结managed栈边界、进入/离开transition；
- foreign adapter是普通managed function加明确的`ForeignManagedEntry` origin；trampoline、C args/result storage和signature descriptor进入独立`ForeignCallbackBridge` arena；
- continuation原子状态操作降为类型化`AtomicLoad/Store/CompareExchange`，MIR之后不靠函数名识别。

## 6. Runtime callback token

### 6.1 opaque cookie 与registry

C侧`context`不是closure指针，也不要求是可解引用的C allocation。M13 64位host把它实现为由`uintptr_t`往返`void *`的opaque cookie：在目标ABI保证可往返且保持canonical的低位payload中编码`slot+1`和generation，0保留为非法/null，其余高位保持目标要求的固定值。runtime必须为每个支持目标明确payload位预算并在启动测试round-trip；slot或generation超出预算时不再分配，generation耗尽的slot永久retire。

这种设计同时满足：

- C只复制和回传一个GC-free word；
- callback入口先查registry再接触token内容，可确定性检测stale/use-after-final-release；
- 释放token不需要永久泄漏一个C heap tombstone，也避免malloc地址复用造成ABA；
- cookie从不作为指针解引用。当前只承诺M12/M13的64位POSIX host；其他pointer model需另定编码。

token slot至少包含：

```
generation / live
signature_descriptor
typed_adapter
closure_handle
first_failure_handle
mode
owner_count / active_count
one_shot_claimed
state
```

registry中只保存`GcHandle`整数，不保存可能移动的closure/exception裸地址。GC从全局handle表追踪这些对象。

### 6.2 invoke gateway

`scoop_runtime_callback_invoke`的顺序固定为：

1. 在callback lock下校验cookie generation、live、signature和mode；one-shot执行claim，并把一份持久owner转成active worker lease；增加`active_count`，快照adapter/handle后释放lock；
2. `attach-if-needed`，保存attachment ownership；若线程原为native-safe/native-borrowed，建立可恢复的transition；
3. 检查GC epoch后enter managed；从closure handle解析当前地址，把closure local和exception output slot放入native-root frame；
4. 调用typed managed adapter；该调用可以任意分配、GC、嵌套FFI或恢复continuation；
5. adapter正常返回后，若为`Threw`，在exception root仍有效时创建`GcHandle`并以first-wins规则记录首个失败；并发后续失败的临时异常不覆盖首个失败；
6. pop roots，leave managed并恢复先前native mode；
7. 在callback lock下减少active count，更新`Completed/Failed`。one-shot在此消费claim时保留的worker lease；owner和active都为0时释放closure/failure handles并回收registry slot；
8. 仅当本入口拥有attachment时detach。

callback lock不能跨第2–6步持有。active lease保证在锁外执行adapter时token和两个handle槽不会被最终销毁。若runtime已shutdown、cookie无效、signature不匹配或one-shot重复调用，gateway无法安全地向任意C API返回Scoop异常，统一打印边界诊断并终止。

### 6.3 状态与异常

- one-shot：`Registered → Active → Completed | Failed`，终态以release发布；observer查询以acquire读取；
- reusable：无active调用时保持`Registered`，一个或多个并发调用时为`Active`；首个异常后`Failed`保持sticky，但后续已合法进入的调用仍完成自己的清理；
- `first_failure_handle`由token持有到最终release。若多个callback并发抛出，只有第一个成功安装，其他managed异常在移除临时root后按普通不可达对象回收；
- adapter异常对应C返回全零值。C调用者若要区分错误，必须通过API-specific同步点后由仍存活的observer查询token；runtime不把私有status强塞进用户C callback的返回类型；
- callback之外的foreign/C++异常仍受runtime spec第5章边界规则约束，M13不实现跨语言unwind翻译。

## 7. Continuation 跨线程恢复

M13不新增suspend callback；跨线程场景是ordinary managed callback捕获一个`Continuation<T>`，并在foreign thread中调用其普通`resume` / `resumeWithException`方法。为此，M10生成的普通字段状态机改为原子协议。

### 7.1 safe continuation adapter

现有`Registering / Waiting / LatchedSuccess / LatchedFailure / Consumed`增加内部claim状态，避免winner尚未写完payload时另一线程读取：

1. `resume`以acq_rel CAS从`Registering`或`Waiting`抢到对应`Completing*`；失败即重复完成，抛`IllegalStateException`；
2. 从`Registering`获胜表示同步完成：winner写latch payload，再release-store `LatchedSuccess/Failure`，不重入frame；registration线程acquire读取后消费；
3. 从`Waiting`获胜表示真实异步恢复：winner写frame结果/异常，acq_rel把adapter置`Consumed`，再驱动frame；
4. `register`正常返回时CAS `Registering → Waiting`。若观察到`Completing*`，以包含safepoint poll与退避的acquire循环等待winner发布短临界区；随后消费latched结果。若观察到latched结果直接继续；
5. `register`抛出时CAS `Registering → Consumed`决定原异常是否胜出；若同步完成已被claim/发布，则按M10规则改报registration协议错误；
6. payload普通store发生在release状态发布之前，读取发生在acquire之后。

### 7.2 frame lifecycle

frame的`running / suspended(k) / completed`字段同样改为64位对齐原子状态：

- 保存跨挂起值后，以release发布`suspended(k)`；
- 对应adapter必须以acq_rel CAS从`suspended(k)`取得`running`所有权，状态不匹配即`IllegalStateException`；
- 整个driver执行只允许一个线程持有running；再次挂起或完成时release发布新状态；
- continuation在哪个已attach线程被恢复，后续Scoop代码就在哪个线程inline继续，直到再次挂起或完成。M13不自动投递回原线程，也不建立dispatcher队列；
- 用户自行同步的publication必须保证producer能看到continuation引用。callback token、pthread创建/join和runtime原子状态只覆盖各自协议，不为任意managed字段建立隐含全局顺序。

这些原子字段仍是普通managed对象payload的一部分，TypeDescriptor扫描只关心其中的引用字段。STW会停住所有managed写入者，所以collector无需与frame字段写入并发扫描。

## 8. 各 stage 与 runtime 文件职责

### 8.1 AST / parser

M13不新增语言语法。显式type argument、函数类型、lambda、`FunPtr`、unsafe block和enum都复用M11/M12。parser只需正常解析core新增声明与用户调用；`contextIndex`、mode和callback签名合法性不在parser按名字检查。

### 8.2 HIR

- 验证唯一`ForeignCallbackCore`：两个enum、`ForeignCallback<F>`字段和五个intrinsic声明；失败是core配置错误；
- 对registration做第4.1节的特殊双向类型检查，输出`ForeignCallbackRegistrationId`；
- 复用C-FFI-safe classifier生成native signature，并派生删除context后的managed `FunctionTypeId`；
- 诊断suspend callback、非具体F、非法context index/type、runtime mode、C-unsafe参数/返回、直接构造token和非unsafe调用；
- 不改变`FunPtr` native-address resolution候选或closure capture规则。

### 8.3 MIR

- closure conversion后生成typed adapter、异常catch/materialize路径和registration runtime call；
- token操作保留独立typed节点，不改写成raw UInt/Ptr操作；
- 把M10 adapter/frame状态读写改为原子状态机，保持每个resume point的精确结果类型；
- dump锁定registration、adapter、native/managed signature映射、context index、mode和atomic transition。

### 8.4 LIR / codegen / driver

- LIR完成managed/raw/code pointer分类、native transition effect、caller-root liveness和atomic instruction lowering；
- codegen发射TLS TLAB fast path、atomic card mark、safepoint slow path调用、native-safe/borrowed prologue/epilogue和callback managed entry；
- C bridge生成器复用M12 CType/layout设施，新增foreign trampoline和signature descriptor；
- driver/runtime构建加入pthread编译与链接选项，fixture继续把API-specific wrapper编成独立native archive，不把测试函数塞入runtime；
- callback adapter/trampoline描述必须进入对应IR/meta；跨Cone实际导出的closure注册在M17前仍只验证单Cone，但实体格式不能依赖源码文件局部索引。

### 8.5 runtime

runtime把`gc.c`中的单线程global mutator状态拆成thread registry、per-thread roots/TLAB和STW coordinator；callback registry可单独放入`callback.c`，平台栈边界/线程辅助放入`thread.c`，避免继续扩大单个`gc.c`。公共header只暴露ABI结构、transition/root frame和callback gateway；registry slot、锁和thread state完整布局保持私有。

## 9. 测试计划

### 9.1 runtime C 单元测试

- 主线程及多个pthread attach/detach、nested attach ownership、带root或managed depth时拒绝detach；
- deterministic barrier控制managed/native-safe/native-borrowed/parked状态，断言GC只等待正确线程；
- 多线程TLAB refill、small/large allocation、free-line复用、threshold触发和arena/block table一致性；
- 并发handle create/release与generation stale检测、pin/unpin原子bit、global/external root增删；
- callback registry retain/release、active lease、slot reuse generation、one-shot double invoke、signature mismatch、shutdown late invoke；
- ThreadSanitizer构建覆盖runtime元数据路径；平台不支持TSan时仍保留普通确定性测试，不以随机sleep作为同步。

### 9.2 parser / HIR negative

- 未显式给F、F为suspend/generic未具体化/C-FFI-unsafe函数类型；
- contextIndex非常量、越界或对应参数不是`Ptr<Unit>`；删除context后closure参数/返回不匹配；
- suspend lambda、managed/C function signature混淆、直接构造`ForeignCallback`；
- safe context注册/retain/release/query；runtime值mode；
- 确认lambda/已有closure不能赋给`FunPtr`，M12诊断不因M13放宽。

### 9.3 IR / codegen golden

- HIR `ForeignCallbackRegistrationId`同时锁定两个FunctionTypeId、context index和mode；
- MIR adapter是managed、可抛并有完整catch/materialize路径；M12 callback bridge仍为NoGC且id种类不同；
- LIR区分managed/raw/code pointer及`NativeSafe/NativeBorrowed` call effect，caller roots只含live managed leaf；
- LLVM锁定atomic continuation CAS、atomic card store、TLS TLAB、transition顺序和adapter GC strategy；
- generated C snapshot锁定真实C prototype、context参数位置、args数组排除context、零值异常返回和signature descriptor。

### 9.4 端到端 fixture

1. **多mutator GC**：至少4个foreign thread经managed callback同时分配含嵌套引用/enum/array的对象；barrier使一个线程触发GC，全部结果在join后可验证；
2. **native-safe阻塞**：一个已注册线程阻塞在C ABI函数，另一线程强制GC；GC必须完成而不等待前者返回，阻塞线程的caller roots保持存活；
3. **native-borrowed协调**：Scoop ABI C函数持有direct ref，push native root后跨一次runtime分配/强制GC并reload；collector必须等其到达入口，不能扫描普通native区间；
4. **attach循环**：重复创建/加入短生命周期pthread，每次自动attach/invoke/detach；registry最终只剩主线程，无TLS/thread state泄漏；
5. **one-shot pthread**：成功路径、create失败路径、worker closure捕获值类型与ref、worker/observer两份ownership、join后完成状态；
6. **callback异常**：closure抛带managed引用payload的异常，C frame正常返回零值，join observer取得同类型managed异常并重新抛出，finally释放token；
7. **reusable/concurrent callback**：同一token由多个foreign thread调用，active lease与最终unregister/release正确；用户共享结果通过native mutex/barrier同步；
8. **continuation**：ordinary callback捕获已发布的`Continuation<Int>`，foreign thread恢复成功/失败；另测两线程竞争resume，恰好一个成功，另一个得到`IllegalStateException`；
9. **组合回归**：callback内再调用C ABI和Scoop ABI runtime入口、主动GC后返回；M1–M12全部fixture原样通过。

测试不得依赖`usleep`碰运气。每个状态转换使用native barrier/condition variable准确停在指定阶段，并以debug hook读取thread mode、epoch、token owner/active count和registry size。

## 10. 实现顺序与验收门

1. **线程登记基线**：引入thread state/TLS/attach/detach，把单线程主线程迁入同一模型；GC行为暂不并发，M1–M12全回归；
2. **STW握手**：managed poll、epoch、park/wakeup、保守多栈扫描；两个纯managed mutator可确定性强制GC；
3. **native状态**：LIR pointer provenance、caller-root publication、native-safe/borrowed transition及阻塞/explicit-root fixture；
4. **线程安全allocator/roots**：per-thread TLAB、heap/handle/pin/root同步、atomic card mark，完成runtime并发压力与TSan门；
5. **callback类型链**：core/HIR registration实体、MIR typed adapter、LIR bridge和静态C trampoline；先在已注册同线程回调验证；
6. **foreign gateway/token**：cookie registry、retain/release、one-shot/reusable、attach-if-needed、异常handle与pthread闭环；
7. **原子continuation**：safe adapter/frame CAS状态机和foreign-thread resume；
8. **组合与回归**：所有确定性barrier fixture、negative/golden、M1–M12回归和shutdown错误路径。

每一门先完成runtime/unit与IR golden，再进入端到端；只让pthread执行`@NoGC FunPtr`不算managed callback完成，只给heap加mutex而没有线程状态/STW不算多mutator GC完成，只在无GC的closure上成功不算验收通过。

## 11. 明确不做与后续 backlog

1. **不做语言级并发库**：`Thread`、atomic/mutex包装、dispatcher、`launch`/`async`、structured concurrency、取消与线程亲和性属于后续标准库；
2. **不做suspend FFI**：`@Extern suspend`、suspend `FunPtr`、suspend managed callback和hidden continuation外部ABI继续禁止；
3. **不做无context任意closure导出**：dynamic executable trampoline、JIT closure stub和有限slot registry继续留backlog；
4. **不做移动/并发collector**：精确stackmap解析、root relocation与evacuation进入M15；parallel/concurrent mark和分代仍待后续；
5. **不定义完整语言memory model**：M13只固定runtime atomics、continuation publication和callback ownership所需的happens-before；普通对象竞争后续单独规范；
6. **不自动修复native ABI错误**：direct ref漏登记、token提前释放、错误signature/context配对、shutdown后invoke均不隐式pin、泄漏或吞错，统一按边界错误处理；
7. **不保证无界NoGC/native-borrowed代码的STW延迟**：native-safe阻塞不影响GC；native-borrowed和未到poll的managed区间仍按合作式协议决定暂停延迟。

## 12. 文档同步结果

- **spec 13.10 / 14.3**：已补`ForeignCallback<F>` core形态、context删除规则、mode/ownership、异常零返回和它与`FunPtr`的隔离；
- **runtime spec 3.1–3.6 / 4.2–4.5 / 7–9**：已定稿thread state/STW、native-safe/borrowed、generation handle、callback cookie/status/error matrix与shutdown规则，并从TBD移除callback token编码；
- **impl spec 2.2–2.5**：已补registration core contract、pointer provenance、native call effect、compiler root frame、managed adapter/C trampoline和continuation atomic IR职责；
- **ROADMAP M13/backlog**：已链接本文并勾除多mutator、managed callback与continuation跨线程恢复；精确stackmap/移动GC转入M15，无context callback和语言memory model继续保留。

# M27 设计：Task-local Context

状态：实现进行中；按第 9 节逐批完成并提交，最终功能验收尚未完成。

日期：2026-10-04。

依赖：M10/M13 的协程、完成权与 managed callback，M15 moving GC，M22 的结构化 cleanup，M23 的共有 HIR/跨 Cone/ODR/正式 CLI，以及 M25 的异常 ABI。以已完成 M24/M26 的 core、runtime 和产物格式为当前基线。

对应 [路线图 M27](../ROADMAP.md)、[语言规范](../specs/SCOOP-SPEC.md) 8.3、11.7、11.9、14.3、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)第 9 章和[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.16。本文取代 2026-09-07 的旧草案，不保留“设计完成但规范仍采用 Kotlin 静态解析”的分叉。

## 0. 目标与当前基础

M27 交付运行期按精确类型查找的动态 Context，属于逻辑任务，支持源码编译、独立产物消费、链接与运行：

```scoop
class Request(val id: Long)

context(request: Request)
fun currentId(): Long = request.id

fun execute(request: Request): Long =
    context(request) {
        currentId()
    }
```

`context(request: Request)` 声明入口 requirement 并引入正文 local；`context(request) { ... }` 安装一个结构化 binding。两者使用同一 canonical exact static type 作为 key。中间普通调用不需要传隐式参数；缺失时由被调用实现抛可捕获的 MissingContextException。

设计基线尚无 Context 专用 AST、TaskContext 或该异常；M27-1 已接通同步路径。其余批次沿下列已有基础推进：

| 已有实现 | M27 使用方式 |
| --- | --- |
| 共有 HIR 的声明、类型表达式、具体化和接口 contract | 保存有序 requirement，复用 substitution、可见性与 override 检查 |
| MIR 的 PendingTransfer、CleanupCursor、EndCatch 与 coroutine transform | 加入恢复 binding 的 cleanup，保留 exact result/mark 跨挂起 |
| CoroutineFrame、start helper、resume adapter 的原子协议 | frame 持有 task；真正驱动时切换 Context |
| ThreadState、ManagedEntry、native roots 与 collector thread scanner | 保活 current/previous task，沿已有入口分配和 relocation |
| callback token 的 closure/failure handles 与 active lease | 增加不可变 binding-root snapshot handle |
| callable registration、associated atoms、Strong/ODR、object reader | 保存 body 使用的 key cell，沿现有链接关系合并 |
| 64-byte small-object 上限、32-KiB large-object block | 使用小节点树，不为 Context 改写 allocator |

源码范围覆盖现有合法的命名函数、计算/abstract property、泛型、协程、closure 与 managed callback。Context 不引入静态 effect row、scheduler、取消或公开 Context 容器 API。

## 1. 关键决策

| 问题 | 决策 |
| --- | --- |
| key | alias 展开后的完整静态引用类型；不按对象动态类型或兼容父类型查找 |
| 值 | 非空 managed ref；已有普通装箱/转换可先把值变为引用 |
| 入口取得 | 每次 source implementation activation 按声明顺序取得一次 |
| 缺失 | 普通 MissingContextException；caller 可以 catch |
| 作用域 | compiler-known block；所有真实退出恢复，仅挂起不恢复 |
| 调用契约 | requirement 参与导出、override 与内容 fingerprint，不参与重载、推断、函数类型或普通 ABI |
| task | 普通/direct suspend 调用共享；startCoroutine fork child |
| closure | 只捕获显式使用的词法 local，不隐式保存整个 Context |
| callback | 注册时 snapshot；每次 invocation 使用独立 TaskContext |
| 存储 | 可变 TaskContext root + immutable 四叉 radix node；scope 保存旧 root |
| key 物化 | 每 body/key 一个 associated slot cell，登记到现有 callable record |
| 验证 | 在实际类型、CFG、格式、对象和运行时边界检查，复用未变化的结果 |

与旧草案相比，删除独立 undo 链、undo depth、snapshot 堆对象、ContextKey 的第二份 hash、独立 key-use record family 和 program support descriptor。旧 root 已能完成恢复，已有 exact type 与 callable identity 已能表达 key 和 cell 归属。旧稿规定了 undo-top/depth 与 LIFO 检查，并未定义独立 mark registry；本版继续保留 typed scope mark、必要的 owner 检查和 cleanup 验证，不新增登记表。

## 2. 源码与类型规则

### 2.1 声明前缀

```text
context-parameter-list ::= "context" "(" context-parameter
                           ("," context-parameter)* ")"
context-parameter      ::= (Identifier | "_") ":" type
contextual-declaration ::= context-parameter-list eligible-declaration
context-scope          ::= "context" "(" expression ")" block
```

前缀非空、至多一个，位于 annotation/modifier 之前，可以换行，不跨显式分号。参数不接受默认值、vararg 或修饰符。

允许的 target：

- 顶层、成员、扩展及局部命名函数；ordinary、suspend、generic、abstract/interface/default body 沿既有合法形态使用。
- 没有 backing storage、initializer 或 delegate 的计算/abstract property。property list 同时约束 getter 与已有 setter；不增加 accessor 自己的 list。

名义类型、constructor、init、release block、lambda、匿名函数、函数类型、stored/delegated property 及 Extern/Intrinsic/NoGC 声明不能带前缀。Context 不放宽 generic virtual、interface method-level generic、generic extension property 等原有实现边界，也不让 getter/setter 挂起。

具名项在 list、普通值参数及实际 setter 参数中必须唯一。`_` 可以重复，但不建立名称，诊断使用从 1 开始的源码序号。context name 只在函数或 accessor 正文中作为 immutable local 可见，不进入 annotation、类型子句或 caller-side default。正文中的普通词法 shadow/capture 继续遵守现有规则。

### 2.2 ref 类型与 exact key

String、Array/MutableArray、class/interface/object、Any 和 ordinary/suspend 函数值均可作为 key。Option、struct、enum、tuple、Ptr、FunPtr 不能直接绑定；不因正文需要某个接口就自动装箱或转换 binding value。

```scoop
interface Service
class ServiceImpl : Service

context(service: Service)
fun useService() {}

fun needsServiceBinding(impl: ServiceImpl) {
    context(impl) {
        useService() // ServiceImpl does not supply Service.
    }
}

fun withServiceBinding(impl: ServiceImpl) {
    val service: Service = impl
    context(service) {
        useService() // Exact Service binding.
    }
}
```

key 使用表达式完成普通类型检查和 smart cast 后的静态类型。别名与目标同 key；不同 nominal application 或完整函数类型不同 key，函数型变不会自动增加另一个 binding。同型多角色使用不同 nominal wrapper，透明 alias 不能充当新 key。不同 key 可以共存，安装 ServiceImpl 不遮蔽外层 Service；只有后者也不存在时 needsServiceBinding 才抛缺失异常。

泛型直接使用已有 type expression：

- `Array<T>`、`(T) -> T` 等始终为 ref，可以作为符号化 key。
- 裸 T 需要 `T : ref` 或 class 上界；无约束或仅 interface 上界的 T 可能为 value，定义处即拒绝。
- 保留 13.9 的 kind bound 与 nominal bound 互斥规则，不为 Context 发明新的 bound 组合。
- 定义处拒绝已经 canonical 相同的 key；完整替换后再拒绝新合并的 key。例如 `context(a: A, b: B)` 的 A/B 各自满足 ref 条件，A=B 的具体 application 仍非法。
- 具体化检查不能只在有 body 的调用处执行；只有 abstract/interface contract 的 owner application 也须检查。

公开/protected requirement 遵守普通签名暴露规则。类型只出现在 requirement 中也属于真实接口依赖，按实际 provider 导出和查询。

### 2.3 取得时点与声明契约

source implementation 的 prologue 按 list 顺序完成全部 lookup，再进入用户 body。named 项建立 local，anonymous/未使用项也执行 presence check；第一个 missing 决定异常。用户函数内部的 try 尚未进入，只有 caller 能捕获 prologue 失败。

```scoop
context(current: Request)
fun observe(next: Request): Long =
    context(next) {
        current.id + currentId()
    }
```

上例 current 始终是本次入口取得的对象，currentId() 取得 next。suspend activation 只在 initial state 查找，resume 使用已保存 local。this-adjust thunk、callable-reference adapter、跨 Cone bridge 不重复插入 prologue；abstract/interface 无 body 声明只保存 contract。

override/interface implementation 在完成 owner/callable substitution 后必须保持 requirement 的数量、顺序和 exact type，名称可不同。继承同一普通签名却带不同 contract 是 owner application 冲突。property 的 getter/setter 分别承接同一 list；stored/delegated/constructor property 的隐式 accessor 不能实现 contextual property obligation。

只差 context list 的函数是重复声明。context list 不参与 candidate applicability、MSC、类型推断或函数类型，也不转成机器参数。无 requirement 的中间函数可以调用 contextual function；不推导传递 effect。函数引用创建时不 lookup/snapshot，在真正调用目标 body 时取得。普通 receiver、实参和 default 先按 M17 规则求值；default 可以调用别的 contextual function，但不能引用尚未建立的 callee context local。

### 2.4 parser 消歧与诊断

无 receiver 的完整 `context(value) { ... }` 形态保留给专用表达式，在生成普通 Call/trailing Lambda 前识别。qualified `obj.context(...) { ... }` 及其他不匹配该形态的 identifier 用法仍按普通规则。括号与 block 间使用既有 control-flow header 的换行规则。

scope body 是原 callable 内的词法 block，可使用原有 return/break/continue，也继承挂起能力；没有 non-local lambda return 或额外 suspend overload。value 独立求类型，scope 的 expected result type 只传给 body，body 的 contextual calls 不能反向给 value 提供类型约束。

非法 target、空/重复前缀、缺类型、多 binding value、非法参数修饰和错误 block 形态在声明/表达式边界报错，不退化成“找不到 context 函数”。名称、exact key、ref 类别、可见性、override 与 effect 错误由 HIR 给出原 source span。

## 3. 作用域、异常与 cleanup

### 3.1 求值与恢复

1. 在外层 Context 完整求值一次 value；抛出或挂起时，新 scope 尚未安装。
2. 构造新的 binding root，成功后提交并取得 mark。
3. 执行 body；同 key 的内层 binding shadow 外层。
4. 真实退出按词法 cleanup 恢复旧 root，保留原结果或控制转移。

MIR 示意：

```text
value = evaluate_in_outer_context()
mark = ContextPush(key, value)
activate_cleanup(RestoreContext(mark))
body_outcome = execute_body()

normal_value:
    store_exact_result_if_non_unit()
real_scope_exit:
    run_existing_cleanup_chain()
    continue_original_transfer_or_unwind()
suspend:
    save_live_mark_and_values()
    return_suspended_without_scope_restore()
```

push 失败时没有激活 cleanup。ContextRestore 与 finally、EndCatch 使用同一个 cleanup stack，不增加另一套 pending tag、异常存储或运行期解释器。

| 嵌套关系 | 实际退出顺序 |
| --- | --- |
| context 内含 try/finally | 内部 finally 仍看到该 binding，之后 restore |
| try 内含 context，外部有 finally | 先 restore，外部 finally 看到恢复后的 binding |
| context 内含 active catch | 退出 catch 的 EndCatch 先于外层 restore |
| active catch 内含 context | 内层 restore 先于 EndCatch |
| 内部 catch 处理异常且未退出 scope | 保留当前 binding |
| 只挂起 | 不消费 scope mark，只退出本次 driver execution entry |

非 Unit normal result、return/pending payload 和含 ref aggregate 先进入既有 exact place/root，再执行 cleanup。throw/rethrow 沿 M25 的 EH record 规则；需要跨挂起时沿已有 materialization 生成 managed Throwable，不能把 native record 放进 frame。

### 3.2 MissingContextException

core 增加 `public final class MissingContextException(message: String?) : Exception`，普通可构造、可捕获。缺失分支以实际 core constructor 和 Some(message) 构造，再走现有 throw 路径。

消息包含声明 diagnostic path、named label 或匿名序号、canonical type；可以在具体化时生成不可变字符串常量，不需要 runtime 反射。消息不使用进程 slot、对象地址或 host 路径作为类型身份。第一个 missing 项稳定，不受 hash table 或登记顺序影响。

lookup hit 是无分配 leaf，miss 的异常构造属于 managed CFG。ContextPush 可分配；因此 contextual declaration 和 binding scope 均不属于源码 NoGC，也不能用于 const/release-safe body。OOM 仍沿现有 fatal allocation failure，不改成 MissingContextException。

## 4. 逻辑任务与控制边界

### 4.1 传播矩阵

| 边界 | 有效 Context |
| --- | --- |
| 普通/virtual/interface/closure 调用 | 当前 task |
| 同一协程的 direct suspend 调用 | 当前 task，不 fork |
| startCoroutine 的 ordinary 实参及适配器构造 | caller task |
| 真正进入 startCoroutine task body | 从当前有效 binding fork 的 child |
| task 的最终 completion 通知 | child task，body scope 已按正常/异常路径退出 |
| REGISTERING 时投递同步 completion payload | 不驱动 frame，不切换 task |
| 实际 resume driver | frame 持有的 task；退出后恢复 resumer |
| 同步 outbound C/Scoop ABI 调用 | 保持当前 task |
| foreignCallback 注册 | 保存当前不可变 binding root |
| 每次 callback invocation | 从 registration root fork 的独立 task |

parent、child 与 callback invocation 只共享 immutable node 和 bound 对象；后续重绑定不互相写回，对象 mutation 仍是普通共享对象行为。Context 不提供同步或深拷贝。

### 4.2 启动与线程根

ThreadState 增加可回写 current_task_context root。主线程 attach 仍是 native-safe；第一个 eager/root gateway 先执行原 mandatory poll，再分配空 TaskContext 并安装，之后才执行源码 initializer/main。后续 gateway 复用同一个 task；没有 eager initializer 时由 root gateway 完成首次建立。

允许没有 task 的阶段仅为 native attachment、gateway 的内部准备前缀及最终 teardown。准备前缀只分配/安装 Context，不运行用户 body 或 requirement。无 binding 的 initializer 调用 contextual function 得到普通 MissingContextException。

root 在 gateway 之间的 native-safe 区间仍被扫描。main 完成、不再进入 managed 代码后，在现有 thread/registry 同步下清空；foreign callback 恢复外层 task 或空值后再 detach。不得额外缓存未扫描的 TLS TaskContext 指针。

### 4.3 frame、resume 与 completion

每个 resumable frame 有非可选、发布后不重新赋值的 TaskContext field，GC relocation 除外；direct suspend 创建的新 frame 取得同一 task。跨挂起的入口参数和 scope mark 使用已有 exact CoroutineSlot。

resume adapter 继续先做原 atomic claim。只写 latched payload 的路径不 enter；真正取得驱动权后才取得 frame task、保存 resumer Context 并 enter。Suspended、Completed、失败通知和 unwind 出口均 leave，previous root 活到恢复完成。

同一 task 的 binding 更新使用 M13 已有驱动串行性；发布可恢复状态后，原 driver 不再修改已交出的 task，只恢复自己的线程入口。不得只因“单个 frame 有 CAS”就忽略 direct suspend 链的交接；真实跨线程组合 fixture 必须覆盖这一顺序。无需另加全局 task 锁。

completion 自身抛出仍执行 leave，不能将它误作 task body 失败而再次调用 completion。callback A 恢复 coroutine B 的正常嵌套顺序为 A → B → A → callback 外层。

### 4.4 closure 与 callback

普通 closure 没有隐式 TaskContext field：

```scoop
context(request: Request)
fun captureRequest(): () -> Request = { request }

fun deferredRead(): () -> Long = { currentId() }
```

第一种捕获入口 local；第二种在 invocation 时由 currentId 的 prologue 查找。

callback token 增加 `Empty | RootHandle` 快照存储。注册成功前取得 current root，非空时沿既有 handle 表保活；retain 共享它，不重新采样。每次 invocation 的 active lease 同时保活 closure、snapshot 和 failure 所需状态；owner/active 归零后一起释放，OneShot 与 Reusable 沿原协议。

C gateway 在已进入的 managed 段内 resolve/root closure、snapshot 和 exception，再调用 generated adapter。adapter 的入口 poll 后 fork snapshot、enter invocation task、调用用户 closure，正常和 catch/status 出口都 leave。快照作为 typed nullable managed 输入加入私有 adapter；C trampoline 的外部签名、contextIndex 与 ForeignCallback<F> 表示不变。

MissingContextException 在 callback 内沿原 catch-all/status/failure-handle 路径处理。同步重入也使用 registration snapshot；当时调用线程的 outer Context 只用于恢复。并发 invocation 各有独立 mutable task。

M27 不承诺取消或 abandoned continuation cleanup；永不恢复的 frame 与 task 不可达后由普通 GC 回收。

## 5. Runtime 表示与 GC

### 5.1 两种内部对象

```text
TaskContext                         // 24 bytes with the current header
    bindings: ContextNode?

ContextNode                         // 48 bytes with the current header
    child[4]: NullableManagedRef

ContextScopeMark                    // compiler aggregate, not a heap object
    owner: TaskContext
    previousRoot: ContextNode?

ContextExecutionGuard               // distinct compiler/native aggregate
    previousTask: TaskContext?
```

node 中间层存 child node，最后一层存实际 payload，全部槽有 managed provenance 和精确扫描。null 表示内部空边，不是源码 nullable payload。published node 除 GC 更新指针外不可修改。

TaskContext 提供同一逻辑任务的共享可变入口：该任务的多个 frame 都通过它观察最新 binding root；fork 则建立另一个入口，共享当前不可变节点。snapshot 没有独立的源码 identity 或其他状态，一个 root 引用已能表示同一版本；callback 的非空 root 仍由 GcHandle 保活。scope 的历史版本保存在各自 mark 中，跨挂起保留在 frame，不是取消恢复状态或 GC root。

TaskContext 与 ContextNode 由实际 core provider 以两个封闭 generated nominal role 生成，key 使用既有 `(core provider identity, role)` 关系，进入普通 exact type/Strong TD/scan 导出。consumer 使用原定义；不按用户 Cone 重复生成，也不恢复已删除的 program/core descriptor。角色、typed layout 与 C struct 必须对应，helper 的 TD 通过显式 typed metadata 参数取得。

这两种内部类型不公开为源码 Any、Context 容器或 intrinsic API。C 与 LLVM 共享本版固定 layout，必要的 ABI 测试检查 size/offset/scan；其 release hook 为 None。字段和表示发生真实改变时更新 ABI/layout fingerprint，不追加“表示来源”证明。

### 5.2 索引与成本

全程序登记的实际 key 数为 K。`K > 0` 时选择覆盖 `0..K-1` 的最小非零树高，最多 16 层；K=0 保持空树。slot 固定为进程内 u32，超出实际可表示容量是分配/metadata 错误，不是新的语言配额。

- lookup 按 2-bit digit 访问，无节点则 miss；成本 O(H)，H≤16，与动态 scope 深度无关。
- push 只 path-copy 目标路径，成本 O(H) 小对象；新路径全部完成后提交一次 task.bindings。
- restore 直接恢复 mark.previousRoot，O(1)，不分配、不 unwind。
- snapshot 只取得 immutable root，O(1)，不分配。
- fork 只分配新的 TaskContext，O(1)，无需复制 undo 或所有 binding。

mark 通过普通局部/frame root 保活旧树，足以恢复被 shadow 的值。去掉 undo 对象后，task 本身不保留历史 scope；消费 mark 后结束 liveness/清空 frame slot。不要使用 64-ref managed page：当前 allocator 会把约 528-byte page 放入至少一个 32-KiB large block。allocator 优化另行评估。

fanout、树高、节点字段和 slot 排序属于本版 runtime 实现，不是源码可观察值或长期产物身份；首版固定这一条实现，不同时提供 hash map、COW page 等平行路径。

这项选型优先满足低成本 snapshot/fork、无分配恢复，以及不随 scope 深度增长的查找。保存整份旧 root 能正确恢复，依赖绑定只按结构化 LIFO 修改、同一 task 串行驱动、child 使用独立 TaskContext；它只恢复 binding 映射，不回滚 payload 对象的 mutation。

代价是每次 push 复制 H 个节点并增加 GC 压力，lookup 也需要 H 次路径访问。例如全程序有 1000 个 key 时 H=5，一次完整路径复制约为 5×48=240 byte 的对象存储，尚未包含分配器元数据。这是按布局计算的成本，不是基准结果；当前没有数据证明 radix tree 是性能最优。可变 slot 表可以降低 push/lookup 成本，但需要为 snapshot/fork 复制存储或维护 COW；链式 binding 可以便宜地追加和保存快照，但 lookup 随链深度增长。后续实际性能数据可以支持替换表示，不改变本章的源码、GC 与恢复契约。

### 5.3 根与 effect

| 持有位置 | 扫描/恢复方式 |
| --- | --- |
| ThreadState.current_task_context | 所有 thread mode 下的可回写 root |
| execution guard 的 previous | compiler root 或明确登记的 native slot |
| scope mark 的 owner/previousRoot | 普通 exact aggregate；跨挂起进入 frame slot |
| task/root/node/payload | 普通 TD 与 RefScan |
| push/fork 分配中间值 | 已有 ManagedEntry/native root，GC 后 reload |
| named context local | 普通 local/capture/frame liveness |
| callback snapshot | handle-registry 的 object slot，token 只保存整数 handle |

所有 ref store 服从现有 checked barrier，包括 task root 替换和新节点字段初始化。push 的 input value、旧 root、新建部分及 task 在每个 allocation slow path 都有完整 root；不能保留 GC 前的裸地址继续构造路径。

try-get、restore、snapshot、enter/leave 为 no-allocation/no-GC/nounwind runtime leaf；仍读写 managed ref，不等于源码 NoGC。push/fork/初始 task 构造是 may-GC 操作，复用已有薄 ManagedEntry/NativeBorrowedEntry。无需让 native-safe 状态直接分配，不新建 Context allocator。

这些 leaf 访问 ThreadState/tree，首版保持保守 LLVM memory effect，不能标为 readnone/argmemonly。lookup/snapshot 不得跨 push/restore/enter/leave 错误 hoist 或合并；没有 safepoint 不代表没有内存 effect。

仅保留实际动态状态的局部检查，例如 unresolved cell、错误入口或 restore owner 不匹配。LIFO/退出正确性由既有 typed cleanup 构造和结构验证负责；不在每次 restore 重放 CFG、扫描整棵树或维护 mark 凭证。

## 6. 编译器分层与代码落点

### 6.1 数据边界

```text
ContextContract
    ordered requirements:
        label: Named | Unnamed
        refTypeExpr
        sourceSpan

ContextLookup
    declarationRequirementRef + ordered parameter index
    complete expression type (generic type expression / concrete exact type)
    immutable local initializer or standalone presence-check expression

ContextScope
    value + exact/symbolic ref type
    typed body + result/ControlOutcome
```

declaration requirement 使用原 callable/property typed identity 加独立 parameter index 引用；不保存 body local。generic 复用共有 type expression，concrete application 才有 exact key；不重复保存另一份 key recipe/hash。空 contract 与缺失信息不同，必要字段不能用 Option 待下游补齐。

入口按声明顺序生成普通的 immutable local initializer 或 presence-check expression；每项使用专用 ContextLookup，完整类型给出 key，不另设平行的入口指令列表。MIR scope 的 push 先于结构化 try，restore 放入该 try 的 generated finally，复用已有 CleanupCursor/EndCatch/PendingTransfer 的次序；不新增与 finally 重复的 cleanup 状态机。

mark/guard 以不同的逻辑 variant 保持类型区分，物理存储使用已有 exact aggregate/nullable-ref 和 scan。Task、Node、erased binding ref、mark、switch guard 使用 compiler-owned Context storage role；只有 Task 和 Node 是新增的可分配堆对象；其余 role 复用普通值布局、abstract-reference 或 boxed-value descriptor 表示存储与扫描，不生成额外 Context 堆对象。各 role 的持久名义身份由实际 core Cone 与 role 导出，源码不能命名这些类型；Task/Node 的内部 ref 可以为空，binding ref 不解释为源码 Any。跨挂起 mark 的 CoroutineSlot 携带这份完整 storage type，不用 opaque bytes，也不为它增加 Context 堆对象。

| 层 | 负责的变化 |
| --- | --- |
| AST/parser | 前缀、参数 label/span、独立 scope/block、恢复边界与 dump |
| HIR lowering | ref 类别、key、名称域、default 边界、signature exposure、override、具体化重复、入口绑定、scope 类型/ControlOutcome |
| HIR/export | 同一声明与共享正文的 requirement/scope codec、原 provider 与必要支持引用 |
| MIR lowering | prologue、missing CFG、ContextRestore cleanup、exact mark/result、frame task、start/resume/adapter enter/leave |
| MIR | 完整操作/类型、frame field 和 callable contract；复用现有结构与引用验证 |
| LIR lowering | managed/null provenance、helper ABI、TD 参数、statepoint/root、cell atom 与 registration |
| codegen | 机械发射已有选择、共享 C layout、callable cell 表与 relocation |
| slib/linker/runtime-build | section/profile、object 引用与 ODR、缓存失效；不重做源码语义 |

普通函数及 hidden continuation signature 不添加 Context 参数。私有 callback adapter 的新增 snapshot 输入必须同时进入 C typedef、MIR/LIR physical signature、root plan 与 runtime 调用。

### 6.2 现有文件与模块

实施沿现有模块扩展，不新增 stage crate：

- `compiler/ast/src/declarations.rs`、`expressions.rs`；`compiler/parser/src/decl/`、`expr/primary.rs` 及 dump。
- `compiler/hir/src/entities/core_protocols.rs`、共有声明/正文/codec，以及 `compiler/hir-lower/src/` 的声明、表达式、substitution 与 override 入口。
- `compiler/mir-lower/src/cfg/`、`coroutine/`、`coroutine_registry/start.rs`；`compiler/mir/src/module.rs` 的 CoroutineFrame 与相关 typed metadata。
- `compiler/lir-lower/src/callbacks.rs`、production、root lowering；`compiler/codegen/src/` 的操作、frame、callback 和 registration 发射。
- `runtime/src/thread.h`、`thread/`、`gc/collector.c`、`callback.c`、`image/` 及新建 `runtime/src/context.c` 的实际 Context 操作。
- `sysroot/lib/scoop.core/src/throwable.scoop`、core 协议生产、identity/generated nominal、slib profile/object reader 和对应 schema 1 fixture。

大型模块按已有职责拆分。实现前先阅读具体落点；上表指定职责，不要求机械创建与表项一一对应的文件或工厂。

## 7. key cell、跨 Cone 与格式迁移

### 7.1 只增加实际 cell

`ContextKey(PersistentExactTypeId)` 是不同于 type/slot 的 newtype，没有第二份 digest。cell 使用 `(PersistentCallableBodyId, ContextKey)` 的 typed associated-atom key；machine code 经 cell 取得进程 slot，不烘焙 artifact-local ordinal。

每 body/key 只发射一份 cell 与使用项，多个 lookup/push site 复用。抽象 contract 没有实际操作时不发射 cell，入口 local 的后续读取也不产生新 use。

使用项加入**现有 callable registration**的有序 cell 列表，保存 exact key 与 cell relocation。含 key 的 body 将其现有 callable registration、表和 cell 一起输出到该 body 的 LinkObject；无 key 的 registration 继续放在普通 metadata 对象。这是 codegen 的对象分片选择，不产生新的 record family。表、cell 继承 body 的 Strong/ODR associated-atom linkage。同一 ODR body 的这些 atom 一起 coalesce；不同 body 的同 key cell 可以并存。不给 cell 新建 ODR member、key declaration 或独立 registration family。

编译期 key 的 ref 类型、alias 和 identity 已完成检查，runtime 不用对象 TD 重新检查绑定资格。跨 Cone 只比较同一个 PersistentExactTypeId，不用 FQN、symbol、参数名、arena index 或诊断文本回退。

### 7.2 启动发布与 fingerprint

slot cell 为 u64，canonical 初值 0，运行期写入 `u64(slot) + 1`；这可表示全部 u32 slot，包括 0。登记全部 callable 后按 exact key 分组，选定 slot 并一次发布到各 cell；全部输入检查成功前不写入，发布完才调用 gateway。

runtime 检查新增 span/cell 的范围、可写性、零初态、实际 owner 及必要的重复关系，复用既有 ODR 去重。它不重新计算 type hash 或整份 object fingerprint。slot 在本进程内固定，不回收、不复用；首版无动态 image 登记。

内容 fingerprint 包含 contract、操作、key、cell canonical zero bytes、owner 与 typed relocation，继续沿原 ObjectDefinition/registration/RuntimeImage 路径计算。运行期 cell 值、slot 分配顺序和树高不参与持久 fingerprint。只有 fingerprint 自身字段沿既有规则归零，不添加第二个回填/重验状态机。

### 7.3 当前基线与迁移面

2026-10-04 代码基线如下；这是实施依据，不表示 M27 已升级代码：

| 项目 | 当前基线 | M27 迁移 |
| --- | --- | --- |
| runtime metadata | ABI 3；image 六类 table，callable record exact-sized | ABI 4，更新 callable cell-list 字段与 size；image table 集合和启动参数保持 |
| HIR | core-bootstrap-interface /7、cross-cone-interface /48、type-semantics /15 | core-bootstrap-interface /8、cross-cone-interface /50、type-semantics /16（/49 是 M27-1 共享 body 节点的中间版本） |
| MIR | identity-foundation /2、type-bridge /8 | identity-foundation /3、type-bridge /9，保留现有表示表 |
| LIR | foundation /3、layout-abi /7、layout-link-closure /5、cone-production /5、strong-production /17、link-identity-closure /10 | foundation /4、cone-production /6、strong-production /18、link-identity-closure /11；布局字段未改变的 section 保留版本，更新内容 fingerprint |
| profile | 两个 Strong /4、cross-cone-generic /3 | Strong /5、generic /4，required inventory 同批切换 |
| outer/container/identity | HIR/MIR/LIR outer schema 2，callable-body-v2，persistent-v1 mangler | 保留既有域与规则，新增封闭 generated nominal/atom variant |
| runtime 构建缓存 | 当前源集与 ABI/toolchain fingerprint | 包含 Context 源文件、共享 layout、adapter 与 registration ABI 的变化 |

共有 callable 声明在 field 11 保存独立的有序 context 参数数组（label/type），不改声明 identity 和普通 value parameters；portable callable body 的 field 11 同样保留 context 参数，涵盖没有公开声明记录的 local function；inheritance signature 在 field 3 保存替换后的有序 ContextKey。context 类型引用沿原 signature reference/visibility 入口收集，正文 lookup 的声明和 ordinal 沿原 body 验证边界解析。

section 的具体新编号/字段号在对应 codec 变更时登记；只有内容变化、编码不变的 section 使用新的内容 fingerprint，不无差别升级所有 schema。core/provider/consumer/runtime 同步重建；旧 ABI 在 prefix 或 capability 边界拒绝，不为旧 frame/callback/record 保留兼容实现。

不恢复已删除的 ProgramDescriptor/CoreBindings，也不为 Context 建立单独授权、来源证明、预算或通用登记框架。

## 8. 测试矩阵

新测试使用 [fixture schema 1](../../tests/fixture_runner/README.md) 与正式 scoop/scoopc/scoop-linker 命令；本次设计不创建未实现语法的通过 fixture。实施时每项都有独立正例、相关 negative 和组合覆盖，诊断断言原位置与信息。

| 组 | 独立覆盖 | 组合覆盖 |
| --- | --- | --- |
| 声明/parser | 合法 function/property/anonymous requirement；prefix 与普通 context method 消歧；非法 target/list | local capture、operator/extension、default、setter 名称、三层 dump |
| 类型/key | ref/Any/函数类型、alias、不同 exact application、derived 不隐式提供 base、不同 key 共存 | smart cast、显式 upcast、M16 推断不受 body 影响、generic substitution 后重复 |
| contract | duplicate name/key、顺序、override/继承冲突、可见性、NoGC/const/release 禁令 | abstract owner 仅用作类型、跨 Cone property obligation、default 中无 context local |
| 入口 | first missing、anonymous/未使用项、caller catch、entry local 稳定 | callable reference 创建/调用分离、interface default/adjust thunk 只有一次 prologue |
| scope | value 求值一次、求值失败/挂起时尚未安装、嵌套 shadow、结果与 normal/return/break/continue/throw | 内外 finally/EndCatch、M25 rethrow、含 ref aggregate 与 M26 插值 |
| coroutine | immediate/真实挂起、resume failure、direct suspend 共享、start child 隔离 | 同型 binding 与旧 entry local 跨挂起、finally 内挂起和 pending transfer、跨线程/两 child/completion throw |
| callback | 注册内/调用外、空 snapshot、OneShot/Reusable、exception status | 同步重入、并发 invocation、callback→resume、retain/release 和 foreign detach |
| GC | 只有 binding 持有 payload、current/previous/task/node/old root 全部移动 | 每个 push/fork 分配点 stress、native-safe 往返、跨挂起 mark 清空、callback handle roots |
| artifact | HIR/MIR/LIR round-trip、旧版本拒绝、cell/relocation 损坏、缓存重建 | A→B→C、去掉 provider 源码消费、重复 generic/ODR cell 合并、re-export 与独立 body 同 key |

补充验收要求：

- runtime 单测覆盖空树、分支边界、较高 slot、嵌套恢复、分配中间值 relocation；实现测试可以检查本版节点，源码 fixture 不锁定具体 slot 数字。
- negative fixture 覆盖 2.1–2.4 的每个编译错误类别，包括只有 interface bound 的 T、具体化后重复、stored property 实现 contextual obligation、仅靠当前 binding 推断 T 失败。
- 跨 Cone fixture 分别从 A 声明类型/requirement，B 包装或物化，C 只凭产物链接运行；同一 generic body 在 B/C 重复物化时 cell 跟随原 ODR member 合并。
- AST/HIR golden 锁定 scope 是 block、ordered contract、exact/symbolic key 与 entry action；MIR 锁定 prologue、cleanup、mark/result/frame 和 enter/leave；LIR 锁定 provenance、root、cell relocation、ordinary ABI 参数未增加。
- GC stress 验证恢复后不再保活已消费 mark 的旧树；可以组合 M24 既有 release-hook fixture 观察显式测试 collection 后的不可达性，不引入 weak-ref API 或新回收承诺。
- 产物损坏在现有 reader/runtime 真实边界测试，不建立另一套正常语言语义 verifier、通用故障框架或测试专用编译器。

## 9. 实施顺序与完成门

每批以能运行的源码路径推进，不先完成全部 metadata 再等待语言入口。

| 批次 | 交付内容 | 批次完成依据 |
| --- | --- | --- |
| M27-1 同步纵向闭环 | core exception/内部类型、key cell、root gateway、lookup/push/restore、普通声明与 scope | 正式 CLI 运行独立/嵌套/missing/GC scope，含各 stage dump |
| M27-2 完整源码契约 | 所有合法命名函数与 property、入口 local/capture、defaults、override、effect 和退出边 | 语言规则 negative 与 finally/EndCatch/loop/result 组合通过 |
| M27-3 泛型与独立产物 | symbolic→concrete、abstract contract、共有 HIR 导出、A→B→C、Strong/ODR 与缓存 | 删除 provider 源码后消费，实际 key/cell 合并和必要损坏用例通过 |
| M27-4 协程任务传播 | frame task、start fork、实际 resume enter/leave、mark/local exact slots | immediate/真实挂起、跨线程、direct 链、completion 抛出与 moving GC 通过 |
| M27-5 callback 传播 | snapshot handle、private adapter ABI、独立 invocation task、全出口恢复 | native 重入、并发 Reusable、OneShot、失败与 retain/release 组合通过 |
| M27-6 总验收 | 文档/codec/profile/runtime 一致，全部正式 CLI、golden 与回归 | 下列命令无更新模式全部通过，另记录结果到 ACCEPTANCE.md |

内部批次不代表已完成 M27，也不允许把暂未接通的形态输出为成功但缺字段的 IR。迁移期间沿路线图既有正式诊断报告未进入该批的能力，全部 M27 范围在总验收前关闭。

按仓库约定，每批代码完成后先格式化、lint，再测试。最终至少执行：

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build -p scoop -p scoopc -p scoop-linker --bins
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --all
```

如修改 Python runner，先按 AGENTS.md 执行固定 Ruff 版本的 format/check。fixture 不用 --update-snapshots 作为验收；新增预期先独立审阅。runtime 相关 C 测试沿现有仓库构建/测试入口执行，不把 cargo test 当作正式 CLI 覆盖。

只有以下条件同时满足才标记 M27 完成：spec、core、三个 IR 与 runtime 一致；source 和 artifact-only 链接均可运行；lookup 无动态类型搜索；普通 ABI 无 hidden Context 实参；全部真实退出/挂起/回调边界正确恢复；moving GC 可更新每个 root；旧格式可靠拒绝且缓存重建；没有 TODO、空分支或只在测试存在的替代实现。

## 10. 明确的后续边界

以下不进入 M27：静态 effect row/可用性推断、compatible/subtype lookup、context function type、显式 context 实参、公开 contextOf/capture/runWithContext、first-class/generative key、多 value 同层 binding、直接 value/nullable payload、非结构 set/remove、反射/枚举 Context、普通 closure 隐式 snapshot、scheduler/launch/async/取消、多次 continuation 或 native stack capture。

Context 索引优化、lookup hoist、slot constant folding、frame/task elision 以及 allocator 改造需要实际性能证据另行设计。它们不成为本里程碑源码、链接与运行闭环的前置门槛。

# M24 GC-free release hook 设计

状态：设计完成，待实现  
日期：2026-09-07  
依赖：M15 moving GC、M19 class 构造、M23-10 artifact/runtime/link闭包、M25 自有异常 ABI

## 0. 结论

M24 引入一个受限的 `release { ... }` class member，作为遗漏显式 `close` / `release` 时释放 unmanaged resource 的最后兜底。它不是对象方法，也不是 GC finalizer：源码不能调用、覆写或取得它的引用；collector 只在一个已经完整构造的对象被证明逻辑死亡、且其存储即将真正回收之前，同步调用 exact `TypeDescriptor` 中登记的静态 hook thunk。

本里程碑固定以下边界：

- hook 只属于普通 `final class`，不允许值类型、interface、object/companion、可继承 class、intrinsic class、异常对象或 compiler-generated class；
- hook thunk 接收一个 runtime 私有 raw object pointer，但源码没有 `this`。源码只能只读本 class 自己声明的、具有 backing storage 且 exact 类型满足下述 `ReleaseValue` 的字段；
- hook 必须 GC-free、non-suspend、non-throw、不可复活对象，不得分配 managed object、进入 safepoint、操作 root/handle/pin、回调 managed code或执行线程状态转换；
- hook 在 collector 的同步 reclaim 路径中运行，不建立 payload 副本，不进入 cleaner/finalizer queue，也没有后台执行器；
- hook 是 best effort：不承诺何时发生 GC、对象之间的调用顺序、执行线程、进程退出时执行，也不承诺及时释放；但只要一次正常 collection 决定回收一个 release-ready 对象，就必须在复用、poison、unmap其存储前尝试调用一次；
- 显式 `close` / `release` 仍是主路径。显式释放必须先把 owner 字段改成合法的 inert state，再释放资源，使以后可能发生的 hook 成为 no-op；M24 不提供公开的 arm/disarm API；
- hook-bearing 对象在完整构造成功后才由生成代码设置内部 `release-ready` 位。构造失败的半初始化对象不会运行 hook；构造过程中已经取得的 unmanaged resource仍须由构造代码显式清理；
- moving 只搬运逻辑存活对象及其 `release-ready` 位。from-space 旧副本的失效绝不触发 hook；
- 全功能 finalizer、对象复活、异步 cleaner、thread-affine cleanup、外部内存压力记账及 ByteBuffer 本身不属于 M24。原字符串里程碑整体移至 M26。

这组约束使 release hook 能安全支撑 M26 的 off-heap growable byte buffer，同时不把 managed execution重新带进 GC reclaim 路径。

## 1. 源码表面

### 1.1 语法

`release` 是 class member 位置的 contextual keyword：

```text
class-member  ::= ... | release-block
release-block ::= "release" block
```

示例：

```scoop
@Extern(name = "free")
fun nativeFree(address: Ptr<UInt8>)

final class NativeBytes public constructor(
    private var allocation: Option<Ptr<UInt8>>,
    public val size: Long,
) {
    public fun close() {
        val owned = allocation
        allocation = None
        when (owned) {
            Some(value) -> @Unsafe { nativeFree(value) }
            None -> {}
        }
    }

    release {
        when (allocation) {
            Some(value) -> @Unsafe { nativeFree(value) }
            None -> {}
        }
    }
}
```

`fun release()`仍是普通合法方法名，与无 `fun` 的 `release { ... }` 不冲突。release block：

- 没有名称、参数、返回类型、visibility、annotation或modifier；
- 每个合法 owner 至多一个；
- 不是 declaration lookup candidate，不进入 overload、override、vtable/itable、callable reference或反射表面；
- 不能显式调用，也没有隐式的正常控制流调用点；
- 正常完成结果固定为 `Unit`，`return`、`throw`及挂起均非法。

`@Unsafe` 不由 release block 隐式获得。调用 C ABI release primitive、解引用 raw pointer等操作仍须位于显式 `@Unsafe { ... }` 中；`@Safe` 的现有嵌套规则不变。

### 1.2 合法 owner

M24 只允许普通、可实例化、最终的 class 声明 release block：

- class 必须为 `final`；默认 final 的普通 class满足此条件；
- `open`、`abstract`、`sealed` class一律拒绝，避免继承、hook chain与“哪个 exact owner负责资源”的歧义；
- interface、struct、enum、tuple、annotation class、`object`、companion、intrinsic class及 compiler-generated closure/coroutine/adapter均拒绝；
- owner 不得是 `Throwable` 的子类型。M25 exception record会按值复制异常 payload，release ownership不能随该物理副本复制；
- owner 可以是 static nested ordinary class，也可以是 generic final class；
- owner 可以继承一个不带 release block 的普通 base class并实现interface，但 release block只能读取本 owner 自己声明的 backing field，不形成 base/derived hook chain；
- release-bearing class不能被编译器改写为 immortal、singleton、栈对象或其他绕过普通 GC heap reclaim 的表示。

generic owner 的 release body在 template中保存。若它读取的字段表示依赖类型参数，HIR导出结构化的“该实参必须满足 `ReleaseValue`”条件；每个 fully specialized application必须证明条件，失败即为该 application 的编译错误。未使用的 managed field不妨碍同一对象拥有 release block。

### 1.3 reclaiming receiver

release block 没有普通 `this`。HIR为它建立独立的 `ReclaimingReceiver` 能力，仅提供对同 owner backing field 的 direct read：

- 可以读取本owner的primary-constructor property及body stored property的实际backing storage；release语义始终绕过getter直接load，即使该stored property另有explicit accessor；
- field 的fully resolved exact value type必须满足下节的`ReleaseValue`；
- 读取结果是普通release value，可以绑定local、解构、做模式匹配、整数/指针运算及传给合法release-safe target；
- field 只读；assignment、`++` / `--`、复合赋值、取址和形成可写place均非法；
- computed/accessor-only property、delegated property、任何getter/setter调用、inherited backing field、method、extension、`super`及任何需要ordinary receiver的表达式均不可用；
- 裸 `this`、隐式或显式把 receiver 传参、返回、存储、捕获、装箱、cast、identity比较、取得 `GcHandle`/pin或 callable reference均非法；
- 不能通过 raw pointer把当前对象重新解释成managed ref，不能读取本对象中的managed field，也不能把指向GC heap的地址藏入 unmanaged storage。

这一能力不是缩水的 class method receiver。它只在 release body 的 typed lowering 中存在，进入 MIR 后已经被正规化为对已验证 field identity与静态offset的 `ReleaseFieldLoad`。

### 1.4 release-safe effect

release block 隐式满足比普通 `@NoGC` 更窄的 `ReleaseSafe` effect；`@NoGC` annotation本身仍不允许标在 class或release block上。合法 body必须满足：

`ReleaseValue(T)`要求fully resolved exact `T`首先满足`gc_free == true`，并且其递归表示不含compiler registry标记的GC/root/callback capability；M24封闭排除`PinnedPtr`、`GcHandle`、`FunPtr`、`ForeignCallback`及其任何aggregate/`Option`包装。它们虽然物理上GC-free，却可能保活、固定或回调managed对象。普通integer、`Ptr<Unit>`、pointee也满足`ReleaseValue`的`Ptr<T>`、相应`Option<Ptr<T>>`及只由release value组成的struct/enum/tuple仍合法。该判断按typed core identity完成，不能按FQN或字段形状猜测。

- 所有local、temporary、expression、field read、参数与非`Unit`结果均为`ReleaseValue`；
- 不分配managed对象、不读写managed ref、不触发safepoint/poll/collection，不使用boxing、String、Array、closure、coroutine、exception或managed callback；
- 不调用 ordinary managed call、Scoop ABI extern、virtual/interface/indirect dispatch、native-safe/native-borrowed transition或runtime root/handle/pin/thread/GC入口；
- 可以调用经过传递验证、目标静态可确定的 `@NoGC` Scoop helper，包括top-level function与GC-free value-type的direct method；该调用使用专用 `ReleaseSafeScoopCall`，class/virtual/interface dispatch不在其中。callee的既有NoGC机器body及完整调用图都必须为 `ReleaseSafe`，不能包含C extern或runtime transition；M24不为普通helper另生成release-context clone；
- release block可以直接调用签名为C-FFI-safe、且全部参数/非`Unit`结果同时满足`ReleaseValue`的C ABI extern release primitive。该调用在release thunk中降低为raw `ReleaseNativeLeafCall`，不经过managed/native线程状态转换；需要C storage bridge时直接复用同一contract生成且验证为纯native leaf的bridge，不能在bridge内补transition。foreign unwind、回调Scoop或进入GC/root/handle/pin/thread runtime入口违反契约；
- 可以使用纯scalar、GC-free aggregate与raw-pointer intrinsic；unsafe要求仍按13.3检查；
- `@ThreadLocal`访问非法，因为hook执行线程不属于源码契约。GC-free const及经类型检查的普通native global访问仍服从既有可见性、unsafe与数据竞争规则。

编译器在整个 release-call graph 上求固定点并输出封闭的 `ReleaseSafeCallableRef`；不得仅看到 `@NoGC` 拼写就放行，也不得让codegen按symbol allowlist猜测。循环和native调用虽然可以是GC-free，但长时间运行会延长完整STW停顿；等待mutator、等待GC或形成无界阻塞违反release hook运行契约。

## 2. 生命周期语义

### 2.1 构造完成与 `release-ready`

每个带release policy的exact class对象使用 `gc_word` 中一个版本化的 `RELEASE_READY` 位：

1. 分配时该位为0，其他header与完整payload仍按M19清零；
2. base、common initialization及全部secondary body按M19执行；
3. 最外层constructor正常返回后、构造表达式把receiver作为普通值公开前，生成代码执行一次 `PublishReleaseReady`；
4. 该操作以 `memory_order_release`、不产生safepoint的原子RMW设置该位，并保留mark/pin/其他GC位；
5. constructor任一路径抛出时没有该操作，失败对象以后按普通垃圾回收但不运行hook。

这不是通用field initialization bitmap，也不改变M19的字段就绪规则。它只证明release block可以观察一个已经完整构造的对象。由于 `InitializingThis` 不可发布，设置该位之前没有其他源码线程可以取得该对象。

lazy取得资源的owner在构造完成后同样ready；其字段起初应为 `None`等inert value，随后普通方法可更新。release-ready只表示“允许在死亡时执行block”，不表示“当前一定拥有资源”。

### 2.2 显式释放

M24 不生成 `close()`，不定义 `Closeable`，也不公开arm/disarm intrinsic。一个资源owner若提供显式释放方法，应采用：

1. 读出当前owned handle；
2. 先把对象字段写成该类型的合法inert state；
3. 再释放刚才读出的handle。

这样重复显式调用及未来GC hook都能观察inert state并成为no-op。并发调用的线性化、多个owner别名同一native handle及native API本身的ownership仍由库设计负责；release hook只保证每个对象最多尝试一次，不证明每项外部资源全局唯一。

### 2.3 何时调用

一次正常collection对每个已登记object-start作封闭分类：

- marked且留在原位：逻辑存活，不调用；
- marked且已evacuate：from-space副本只是转发/旧存储，不调用；to-space对象继承ready位；
- pinned或被root/handle保活：逻辑存活，不调用；
- unmarked且确实将被sweep、复用、poison、quarantine或unmap：逻辑死亡，进入release判断。

逻辑死亡对象同时满足已验证的 `TypeDescriptor.release_hook != NULL` 与 `RELEASE_READY == 1` 时，collector必须：

1. 以 `memory_order_acq_rel` 对header执行原子clear-and-test；只有RMW返回的旧值带ready位的执行者取得本对象的release claim，不建立第二套claimed状态；
2. 除ready位已经clear外，保持TypeDescriptor pointer及全部payload field bytes完整可读；
3. 以对象起点raw pointer同步调用descriptor中的hook；
4. hook正常返回后，才允许poison、删除object-start、复用line/block或解除映射。

collector不复制field payload，不建立GC或native队列，不释放world后再执行。ready位本身就是一次性claim；clear发生在调用前，确保runtime内部重试、验证或错误路径不会对同一逻辑对象调用两次。当前collector单线程且STW；未来parallel collector也必须复用同一原子clear-and-test语义。

claim的acquire部分与构造完成时的release publish配对，保证初始字段可见；构造后的普通field更新则继续依赖语言数据竞争规则和M13既有STW handshake的mutator-release/collector-acquire边。一个显式`close`在mutator park前完成的inert-state写入必须对随后reclaim hook可见；存在源码数据竞争时release hook不额外提供同步保证。

### 2.4 best effort 的精确定义

best effort只削弱调度与生命周期保证，不允许collector一边正常回收ready对象一边静默跳过hook：

- 不保证程序会再次GC，也不保证对象在进程生命周期内被判死；
- 不保证多个hook之间、同一对象图内或不同Cone之间的顺序；
- 不保证运行在哪个线程，因此不能用于线程亲和资源；
- 正常或异常shutdown都不进行全堆遍历，也不为仍live、尚未collect或未ready对象调用hook；
- native release失败没有重试、返回值传播或异常报告通道；
- 进程abort、硬件fault、foreign unwind、无限阻塞或违反GC-free ABI时不保证继续回收；可检测的契约破坏是fatal runtime/compiler invariant error。

因此锁、事务、flush、协议消息、稀缺fd及时归还及其他程序正确性都不能依赖hook。正常控制流仍必须使用显式释放与 `try/finally`。

## 3. Typed IR

### 3.1 AST 与 HIR

AST新增独立 `ReleaseBlock`，不能用特殊名字的方法节点表示。HIR完成owner、唯一性、字段能力、effect、generic条件、`Throwable`继承闭包及visibility检查，并输出：

```text
ReleasePolicy<Target> = None
                      | SynchronousGcFree { hook: Target }
```

`ExportHir`中的generic owner保存typed release template、其 `ReleaseFieldRef` 集合、`ReleaseSafe` call graph edge及`RequiresReleaseValue` type-argument条件；param-free owner保存完整hook body。`LocalConcreteHir`只含fully substituted `ConcreteReleasePolicy`，其中hook target、每个field exact type及所有call target都已确定；不能保留源码名称、nullable body或“稍后检查ReleaseValue”的flag。

release block使用独立的stage-local `ExportReleaseHookId` / `ConcreteReleaseHookId`。它们不能与function、method、constructor、accessor或ordinary callable id混用，但也**不是**persistent identity：M24不定义`PersistentReleaseHookId`，不为它分配新mangler kind。跨Cone template body进入generic hidden support closure但不进入public lookup surface；具体machine body只由第4.2节`PersistentCallableBodyId`标识，generic ODR归组复用M23已冻结的`OdrMemberRole::ReleaseHook=16`。

### 3.2 MIR

HIR进入MIR前把reclaiming receiver正规化。MIR新增独立 `MirReleaseHookBody`，其唯一隐含输入是 `RawObjectStart`，不是managed `this`：

- field读取使用checked `MirReleaseFieldRef`和 `ReleaseFieldLoad`；
- ordinary heap load/store、write barrier、managed call/invoke、throw、suspend、boxing与GC root operation在该body variant中不可构造；
- call只允许 `ReleaseSafeScoopCall` 或 `ReleaseNativeLeafCall`；
- CFG可以包含普通GC-free branch/loop/match，但每个block的值闭包都由validator证明GC-free；
- body返回固定 `Unit`，没有unwind successor或exceptional root set。

普通class construction在其正常完成edge上新增typed `PublishReleaseReady { owner: ConcreteReleaseHookId, object }`。无hook owner、失败edge、base/inner delegation返回点均不能携带该operation；同一construction恰好一个publish。

### 3.3 LIR

LIR把每个release body降为 `LirReleaseHook`：

- ABI固定为 `void (RawObjectStart)`；
- function没有GC strategy、statepoint、stackmap record、personality、LSDA、landingpad、native-root frame或thread transition；
- direct field load使用exact class layout的静态byte offset与GC-free `LirType`，不得从field name、property accessor或runtime reflection取得offset；
- native extern调用保留M23的完整 `NativeExternalContractRecord`，但callsite variant为release-only raw leaf，不复用managed侧 `NativeSafe`；
- `PublishReleaseReady`降低为专用header RMW，不能通过普通源码field/pointer store伪造；
- module validator重算完整release-call graph并拒绝managed pointer provenance、AS1 value、safepoint、unwind edge或不允许的runtime symbol。

concrete type的release policy使用完备sum；物理nullable函数指针只在最后的C metadata编码中出现，不能倒灌为IR语义。

## 4. TypeDescriptor、ABI 与跨Cone

### 4.1 TypeDescriptor扩展

M24 bump runtime ABI fingerprint与`.slib`的HIR/MIR/LIR三层wire schema；旧M23 `.slib`必须重建，不做兼容读取。M23的program/image/entry/core及六类registration record继续使用原`abi_version == 1`、原字段顺序与精确size，不增加nullable尾字段；TypeDescriptor本身没有prefix，它在现有字段末尾追加：

`.slib` 仍使用 M23 的 deterministic-ar container schema 1、bootstrap manifest schema 1、persistent identity schema 1 与 persistent-v1 mangler。M24 将 compatibility 中的 HIR/MIR/LIR schema 与各 outer envelope 全部升至 2；混代或 outer/compatibility 不一致均拒绝。foundation capability 分别使用 `org.scoop-lang.hir/identity-foundation/4`、`org.scoop-lang.mir/identity-foundation/2`、`org.scoop-lang.lir/identity-foundation/3`：HIR 承接 M23-6 的 /3，LIR 承接 [M23-7](../milestone23/stage7/DESIGN.md) 的 /2。正式生产 profile 承接 `cross-cone-generic/1`，升至 `/2` 并列出完整新 required section；不恢复 identity-only profile。同一 capability/profile 的旧、新 major 不得混入同一 artifact 或依赖 world，outer 升代不能代替 section 检查。

各层新 major 的 identity foundation 沿用 M23-7 delta table 的 kind 与未退役 field tag，LIR callable-body table整体改存body-v2 runtime key；HIR release template、MIR release body/publish relation、LIR release policy/definition与object proof分别由major 2的closed payload/required capability承载，不能给`/1` product加nullable field。runtime ABI没有可协商兼容模式，最终程序中的compiler、core、所有Cone与runtime必须提供同一新fingerprint。

M24不另造`LanguageAbiContractV2`、`RuntimeAbiContractV2`或新hash domain；它沿用M23已冻结的V1 contract product与`DomainSeparatedCborHash`，只为既有closed enum分配新tag：

```text
LanguageAbiContractV1 {
    revision: M24ReleaseHook = 2,                         // field 1
}

RuntimeAbiContractV1 {
    object_and_gc_contract: M24ReleaseReadyImmixV1 = 2, // field 1
    runtime_metadata_record_abi: u32 = 1,                // field 2
    type_descriptor_contract: M24ReleaseHookV1 = 2,     // field 3
}

LanguageAbiFingerprint =
    DomainSeparatedCborHash("scoop-language-abi-contract-v1",
                            LanguageAbiContractV1)

RuntimeAbiFingerprint =
    DomainSeparatedCborHash("scoop-runtime-abi-contract-v1",
                            RuntimeAbiContractV1)
```

`M24ReleaseReadyImmixV1=2`承诺本设计的header ready bit、publish/搬迁保留、logical-death分类及nonnull hook claim/reclaim顺序；`M24ReleaseHookV1=2`承诺TypeDescriptor追加的hook字段、精确布局与policy/nullability映射。两项必须同时取2，不能只提升TD而让collector仍按M23 header/GC contract解释，或只提升GC contract却按无hook TD读取。六类prefixed runtime metadata record的字段/size未变，因此field 2保持1。

两份deterministic Wire CBOR与fingerprint golden精确为：

```text
LanguageAbiContractV1 CBOR = a10102
LanguageAbiFingerprint =
    634ec02192ba1541f603b8b56f8c9e63dfc31d86ca5d4443a626f6afc9005391

RuntimeAbiContractV1 CBOR = a3010202010302
RuntimeAbiFingerprint =
    7d317378bb7cb0ac337127f541e46496c8affcd9666b23ac9d32faed09692ece
```

compatibility field 12仍以M23的`IdentityAbiDescriptorV1`和原domain重算。M24 descriptor的完整值如下；除language/runtime leaf、callable body与三层wire schema外，其余字段逐项保持M23值：

```text
IdentityAbiDescriptorV1 {
    language_abi: 634ec02192ba1541f603b8b56f8c9e63
                  dfc31d86ca5d4443a626f6afc9005391, // field 1
    runtime_abi:  7d317378bb7cb0ac337127f541e46496
                  c8affcd9666b23ac9d32faed09692ece, // field 2
    identity_schema: 1,                               // field 3
    wire_cbor_schema: 1,                              // field 4
    hash_framing_schema: 1,                           // field 5
    callable_body_schema: 2,                          // field 6
    mangling_schema: "persistent-v1",                 // field 7
    runtime_type_id_derivation_schema: 1,             // field 8
    safepoint_id_derivation_schema: 1,                // field 9
    runtime_metadata_encoder_abi: 1,                  // field 10
    hir_schema: 2,                                    // field 11
    mir_schema: 2,                                    // field 12
    lir_schema: 2,                                    // field 13
}

CompositeIdentityAbiFingerprint =
    DomainSeparatedCborHash("scoop-composite-identity-abi-v1",
                            IdentityAbiDescriptorV1)
```

其canonical CBOR与fingerprint golden为：

```text
ad015820634ec02192ba1541f603b8b56f8c9e63dfc31d86ca5d4443a626f6afc9005391
0258207d317378bb7cb0ac337127f541e46496c8affcd9666b23ac9d32faed09692ece
0301040105010602076d70657273697374656e742d7631080109010a010b020c020d02

CompositeIdentityAbiFingerprint =
    ecf6e26c3a9b045961065acd4bcb82d2156485d30ce95062cd4ebe98c0ed9724
```

上面的CBOR换行只为排版，实际输入是三行hex无分隔符拼接后的bytes。writer、reader、runtime registry与program-link必须使用这些exact values；兼容检查不能只比较outer schema或producer version，也不能接受M23 language/runtime leaf与M24 descriptor字段的任意混搭。

```c
typedef void (*ScoopReleaseHookV1)(void *object_start);

struct ScoopTypeDescriptor {
    uint64_t type_id;
    ScoopTypeInstanceShapeV1 instance_shape;
    const uint64_t *object_scan;
    const ScoopTypeDescriptor *parent;
    const void *const *vtable;
    const ScoopItableEntryV1 *itables;
    uint64_t itable_count;
    ScoopByteSpanV1 diagnostic_name;
    ScoopReleaseHookV1 release_hook;
};
```

物理编码固定为：

- `ReleasePolicy::None` → `release_hook == NULL`；
- `ReleasePolicy::SynchronousGcFree` → 非null且指向该exact type唯一的已登记release thunk；
- 非 `FixedObject` shape、非普通final class及`Throwable`继承闭包的descriptor必须为null；
- runtime registry、link verifier与object verifier逐项检查policy、pointer relocation、callable registration及owner exact type一致性。

collector只读已经验证的physical field，不从type name、field layout、方法名或注解猜测是否存在hook。

### 4.2 持久身份与fingerprint

M24不增加release专用persistent id，只把所有machine body统一切换到一个新的runtime-encoded key代际：

```text
CallableBodyKeyV2 =
    Strong { owner: StrongCallableDefinitionOwner }                 // tag 1
  | Odr { member: CallableOdrMemberId }                              // tag 2
  | RootGateway { root_cone: ConeIdentity, main: MainCallableBodyId }// tag 3
  | InitializationStartupGateway { unit: PersistentInitializationUnitId } // tag 4
  | ReleaseHook { owner: PersistentExactTypeId }                     // tag 5

PersistentCallableBodyId =
    SHA-256(ByteSpan("scoop-callable-body-v2") ||
            RuntimeEncode(CallableBodyKeyV2))
```

`RuntimeEncode`沿用M23 runtime metadata encoder：tag为little-endian `u32`，product按声明序编码，typed id写固定32 bytes；这里绝不是Wire CBOR，也不在key bytes外再套一层CBOR或host struct。所有五个variant都使用v2 domain，旧v1 artifact整体重建。`ReleaseHook` payload只有owner exact type id，因此runtime可从type registration的`PersistentExactTypeId`重算唯一合法body id，再要求该callable entry与TypeDescriptor函数指针逐bit一致。

param-free source class不构造ReleaseHook ODR member；其hook body使用上述精确`ReleaseHook(owner)` body key，但definition plan走Strong owner并与同一个exact source subject的TypeDescriptor、callable registration一起由定义Cone发射，默认`ConeStrong`。若M23-7已经对整个subject取得完整template-support hidden proof，它们可以整体继承`TemplateSupportHidden`，但仍属于Strong侧而不是ODR；不得只把hook、registration或TD中的一个改成hidden。

generic exact application使用M23已冻结的nominal specialization group，且恰有一个`OdrMemberKey { group=owner的Nominal组, role=ReleaseHook(16), discriminator=ExactType(owner) }`。validator逐字段证明group的origin/arguments等于owner的`ExactTypeKey::NominalApplication`；hook body仍是`CallableBodyKeyV2::ReleaseHook(owner)`，其`cb` symbol与ObjectDefinitionPlan primary由该ReleaseHook member拥有并取`OdrWeak`。同组另有且只有一个`RegistrationRecord/CallableBody(body id)` member供`cr`使用；不能再制造`CallableBody` role member、`od`第二primary、safepoint或`sr`。TD的hook relocation必须命中该`cb` entry。多个consumer materialize同一application时逐member证明并coalesce为同一TD、registration和hook地址。

hook调用verified pure C leaf bridge时，canonical LIR definition、object relocation fingerprint与ODR digest只引用M23的`GeneratedBridgeSemanticTarget { unit }`；各producer实际指向本Cone `PrimaryEntry(unit)` atom，object verifier必须再规范化回unit。producer-local atom id不能进入generic hook fingerprint，否则两个consumer会为同一release specialization得到不同definition。

`PersistentExactTypeId`、其他declaration identity、`PersistentCallableApplicationId`、`OdrGroupId`、M23已分配的ReleaseHook role 16及`persistent-v1` mangling schema不改变；以CallableApplication/GeneratedCallable为discriminator的primary callable member identity也不因body schema改变。manifest的composite identity-ABI fingerprint必须改为包含callable-body-v2并与M23值不同。由body id派生的link symbol、safepoint owner/site identity、以CallableBody/SafepointSite为discriminator的派生`OdrMemberId`、registration symbol/member set、ODR fingerprint及缓存key全部重新生成，不能只给新增ReleaseHook使用v2而保留其他body的v1 id。

canonical source/HIR/MIR/LIR fingerprint覆盖release body、读取的typed field、`ReleaseValue`条件和call graph；`CanonicalLirDefinition`覆盖release policy与hook body id。TypeDescriptor的object definition覆盖physical hook relocation role及对应body definition fingerprint；RuntimeImage type/callable record双向关联owner exact type、entry与producer。修改hook body、field type/offset、callee或policy必须使相应artifact和下游缓存失效。

### 4.3 artifact验证

在任何managed代码运行前，M23-8 registry流程额外验证：

- 每个非null `release_hook`恰好命中一个`CallableBodyKeyV2::ReleaseHook(owner exact type)`的`cb` executable entry；param-free owner要求同producer Strong plan，generic owner要求同nominal ODR group唯一的ReleaseHook/ExactType member，不能把“同producer或同group”当作任选其一；
- callable body id的owner exact type等于TypeDescriptor registration的exact type；
- pointer为正确ABI/alignment的函数入口，不与ordinary managed/native gateway共址或混用registration；
- TD physical nullability与typed release policy完全一致；
- hook body产物不含statepoint/stackmap/LSDA/personality、managed pointer relocation、禁止runtime入口或未声明native symbol；
- generic ODR重复的policy、唯一ReleaseHook member、唯一RegistrationRecord/CallableBody member、body/layout/descriptor fingerprint及最终地址全部一致；param-free则禁止出现ReleaseHook ODR member。

runtime不反汇编任意函数来重新证明GC-free；该证明由typed IR validator与object verifier建立，runtime只消费经过完整program registry commit的数据。

## 5. Runtime 与 GC

### 5.1 header位

M24不改变16-byte object header大小。`gc_word`的版本化位布局固定新增 `GC_RELEASE_READY_BIT = UINT64_C(4)`（bit 2）；现有`GC_PIN_BIT = UINT64_C(2)`（bit 1）不变，bit 0与其他位继续保留给既有/后续GC用途。compiler/runtime共享该常量并纳入runtime ABI fingerprint，所有mark、pin、forwarding辅助操作以mask/RMW保留不属于自己的位。合法状态：

- TD无release hook时ready必须始终为0；
- collector在未forward的普通对象上观察到null hook与ready=1时必须报告fatal ABI invariant error，不能静默跳过；
- 分配hook owner时ready为0；
- 只允许生成的 `PublishReleaseReady` 以release RMW把0置1；
- collector claim在调用hook前以acq_rel clear-and-test把1清0，旧值中的ready位决定唯一winner；
- mutator源码与FFI API不能直接读取或修改该位。

forwarding关系继续位于arena外side metadata，不占用或覆盖ready。evacuation复制header时必须保留ready，随后对from-space分类时依据marked/forwarded状态禁止调用旧副本hook。

### 5.2 reclaim集成

release调用应集中在唯一runtime helper中，使small object、large object、普通sweep、evacuation source清理及stress poison共同使用同一分支顺序：

```text
classify logical death
  -> load and validate TypeDescriptor.release_hook
  -> if hook == NULL:
         require RELEASE_READY == 0
         skip claim and call
     else:
         atomically clear-and-test release-ready
         if old ready == 1:
             call hook(raw object start)
         else:
             skip call
  -> retire object-start / poison / free / unmap
```

null分支绝不能先clear一个不应存在的ready位再静默继续；观察到`hook == NULL && ready == 1`是fatal ABI invariant error。non-null分支也只有clear-and-test返回旧ready=1的唯一winner可以调用，旧值为0时直接回收。hook执行期间world保持stopped，collector metadata处于不可重入状态。hook不得请求collection、attach/detach线程或重新进入managed代码。对象存储不是root，也不能被hook发布；允许读取它只是reclaim helper提供的短暂raw lifetime，该lifetime在hook返回时立即结束。

调用顺序不按地址、type、allocation时间、引用图或Cone排序。循环引用中的多个死对象各自至多调用一次，但一个hook不能读取或依赖另一个managed对象仍然存在。

### 5.3 shutdown

M23-8/M25既有shutdown协议保持不变：停止新attach/registration并等待活动入口后直接销毁runtime状态，不运行“最后一次GC”，不遍历live object，也不补调release hook。需要确定性关闭的core/IO组件必须在正常控制流中显式关闭。

## 6. 诊断与测试

### 6.1 稳定诊断

至少覆盖：

- 非final/非法owner、Throwable subtype、重复release block；
- annotation/modifier/visibility/参数/返回类型等非法语法；
- `this` / `super` / inherited、computed或delegated property访问；
- managed、GC capability或其他非`ReleaseValue` field read，包括generic application实例化后失败；
- field write、receiver逃逸、capture、boxing、ordinary managed call、Scoop ABI extern/dispatch、native transition、root/handle/pin/thread/GC API、throw/suspend；
- 仅 `@NoGC` 但不满足传递 `ReleaseSafe` 的callee；
- `@ThreadLocal`依赖与不合法C ABI extern；
- malformed `.slib` / object中的policy、hook pointer、body owner、fingerprint、registration及ODR不一致；
- outer schema `1/2/1`混代、`/2` outer搭配foundation/profile `/1`、同world混入body-v1，以及ReleaseHook role/discriminator或额外CallableBody/`od` primary错误。

诊断必须指向release block、具体field/callsite或generic application来源；不能在MIR/codegen以“unsupported”兜底。

### 6.2 正向与组合测试

独立fixture与IR golden至少锁定：

- raw pointer、fd integer、`Option<Ptr<T>>`及GC-free aggregate field清理；
- 显式close先置inert state，GC后不发生double free；重复close保持no-op；
- lazy resource acquisition在构造完成后仍由hook观察；
- constructor成功时publish一次，constructor失败时不publish、不调用hook；
- null hook且ready=0直接回收，null hook且ready=1 fatal；non-null hook且ready=0跳过，non-null hook且ready=1只有clear-and-test winner调用一次；
- live root、pin、`GcHandle`与active native root保活期间不调用，解除后回收时调用；
- normal moving与每次allocation compaction stress下，live对象搬迁不调用、from-space旧副本不调用、最终死亡对象调用一次；
- small/large object、partial source block、整块quarantine/unmap均保证hook先于存储失效；
- 多个互相引用的dead owner各调用一次且测试不依赖顺序；
- generic owner跨Cone materialize、钻石复用与ODR coalesce得到唯一TD/hook；
- param-free owner使用Strong plan且无ReleaseHook ODR member；generic owner恰有ReleaseHook/ExactType与RegistrationRecord/CallableBody两个所需member、无第二CallableBody/`od`/`sr`；
- hook内直接C extern leaf及传递ReleaseSafe Scoop helper；
- hook目标object code不含statepoint、stackmap、LSDA、managed pointer或thread transition；
- shutdown不补调仍live对象的hook。

测试native资源使用带原子计数和地址有效性断言的C fixture helper：hook进入时对象字段和native block仍可读，返回后计数恰加一；不得依赖真实fd耗尽或malloc地址复用制造偶然结果。

## 7. 实现顺序与完成门

实现按以下批次推进，每批均先更新IR/meta schema，再实现相邻lowering：

1. parser/AST语法、owner与基础negative fixture；
2. Export/LocalConcrete HIR的release policy、reclaiming receiver、`ReleaseValue`条件及ReleaseSafe call graph；
3. MIR专用body/field load/publish operation及结构validator；
4. LIR layout、hook ABI、header bit operation、fingerprint/ODR/meta schema；
5. codegen、callable/type registration、object/link verifier及`.slib` reader损坏矩阵；
6. runtime header位、mark/move/sweep/large-object/reclaim集成；
7. 普通与stress端到端组合回归。

M24只有在以下条件同时满足时完成：源码不能构造出managed finalizer能力；每个成功构造的hook owner恰好publish一次；构造失败与from-space旧副本从不调用hook；每个真正回收的ready对象在存储失效前必须发起且最多发起一次hook调用；hook机器代码从结构和产物上均GC-free/release-safe；跨Cone identity、fingerprint与ODR闭合；shutdown与显式释放语义无歧义；M1–M25既有fixture在普通及相关moving-GC stress模式下无回归。

## 8. 明确不在 M24

- `Char`、String/`MutableArray<Char>`/`MutableArray<Byte>`互转；
- growable off-heap ByteBuffer、native allocator core API及external-memory pressure accounting；
- ordinary class `StringBuilder`与f-string desugar；
- `Closeable`/RAII/`defer`/using语法或编译器自动生成close；
- 公开arm/disarm、手动触发hook或查询ready状态；
- 异步cleaner/finalizer queue、payload copy、后台线程或用户选择executor；
- full finalizer、destructor、对象复活、weak/phantom reference；
- thread-affine cleanup、hook异常传播、重试、顺序或deadline保证；
- parallel/concurrent/generational collector本身。

前三项与字符串插值统一进入M26重新设计；M26的off-heap ByteBuffer可以依赖本里程碑的release hook，但必须另行确定容量增长、失败原子性、borrow/view、外部内存记账及I/O边界。若external-memory accounting需要在hook路径扣减计数，M26还必须把该扣减入口证明为`ReleaseSafe`的纯native leaf；它不能触发或请求GC。

# Scoop Runtime 规范

版本：0.5（草案）

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

对象分配与语言构造是两个层次。M19 class construction只分配一次exact concrete对象；header从分配完成起始终指向最派生TypeDescriptor，base/`this` constructor initializer在同一对象上执行，不分配base子对象或替换header。runtime不运行constructor、不在对象header/payload保存“field正在初始化”状态位，也不根据TypeDescriptor补字段默认值；初始化顺序与访问限制完全由编译器静态保证。M21的object/global exactly-once cell是独立的runtime side metadata，只管理完整initializer的并发执行与发布，不改变任一property的源码type或field表示（见2.7）。

分配入口在返回managed pointer前必须清零完整payload并原子登记header、object-start与精确size。constructor中途发生safepoint时，collector按完整class扫描描述处理该对象：已写field包含合法值，未写managed leaf保持全0并不形成引用。构造失败后对象只按普通可达性回收，不调用析构、rollback或用户代码。

### 2.2 TypeDescriptor

每个**runtime-materialized concrete exact type**有且只有一份编译器生成的`TypeDescriptor`。materialized精确定义为该exact type进入某Cone的LIR layout/type closure或param-free exported LIR bridge；只存在于尚未替换的Export HIR template/binder中的type不提前产生descriptor。它包括每个单态化exact nominal实例，也包括进入LIR的tuple、managed function、raw/native pointer等结构exact type；后者即使layout相同也按各自`PersistentExactTypeId`区分。descriptor至少包含：

- 类型标识（`is` / `as` 检查用）；
- 稳定的 UTF-8 类型名（与 TypeDescriptor 同生命周期，用于未捕获异常等运行时诊断）。producer、artifact reader与final-link verifier必须从已验证exact-type key按M23设计3.1重算`CanonicalExactTypeDiagnosticName`，并以memoized checked展开成本在分配前执行同一16 MiB semantic-text上限；transparent alias、import/re-export alias、使用点qualifier、mangle或consumer本地display spelling均不得改变bytes。最终runtime metadata不携带可反演的exact-type key，因此runtime本身只验证该span非空、只读、不超过同一上限，并以线性有界parser拒绝非ASCII、非法tag/delimiter/percent escape及不完整source/generated atom；它依赖已验证的descriptor/object/ODR fingerprint关联语义，不能假装从`PersistentExactTypeId` digest反推名称或按名称判断类型；
- 版本化`ScoopTypeInstanceShapeV1`：只描述带对象头的managed instance，并以封闭`FixedObject | BoxedValue | InlineBytes | InlineArray | AbstractRef`区分固定对象、装箱值、String、array与不可分配的reference identity；未装箱值的`ValueStorageLayout`是独立LIR契约；
- **递归引用扫描描述**：`RefScan`普通节点记录相对调用方明确给出的base之字节偏移，sequence节点把多个扫描作用于同一base；array节点是自包含的`Array { length_offset, first_element_offset, stride: NonZeroU64, element: NonEmptyRefScan }`，从同一object base读取length并定位首元素。TypeDescriptor的`object_scan`永远相对managed object start；shape中的`inline_scan`只相对box payload或单个array element起点。BoxedValue必须把非空inline scan按payload offset平移成object scan；InlineArray固定使用`length_offset == 16`、`first_element_offset == inline_offset`和shape中的nonzero stride，不能假设首元素总在24。ZST或其他GC-free payload/element的两个scan都为`None`，不得编码`stride == 0`后按length重复空扫描。tagged enum允许完全不含managed ref的variant复用pure-value payload区；每个直接或间接含managed ref的variant拥有互不重叠的连续slot，构造时把inactive slot清零，因此所有ref-bearing variant中的ref leaf直接合并为固定偏移，不存在按tag分派的扫描节点；
- **enum 扫描不读取 tag**：tagged enum的所有潜在ref位置都位于ref-bearing variant的独占slot，按普通固定偏移检查；inactive独占slot必须为全0，pure-value共享区不进入扫描。niche表示的managed-ref enum整体是一个普通引用位置，`Ptr` / `FunPtr` niche不是managed root。没有出站引用的节点可用空指针表示。LIR meta 的`RefScan`提供该信息（见 impl spec 2.4）；
- 父类型信息（接口、父类）；
- 虚分派结构：内嵌 **vtable 指针**与 **itable 数组**（itable 以接口 TypeDescriptor 指针为键）。vtable只含实际需要virtual dispatch的class方法，slot从0开始且允许为空；`Any`没有方法或固定前缀。`ToString` / `Hash`及声明operator equals的interface均走普通itable；装箱值类型的表项指向this调整thunk（impl spec 2.9）。TypeDescriptor的类型名只用于诊断，runtime不得据此合成用户可见字符串、哈希或相等语义。

generic nominal application始终invariant。每个fully specialized application以自身TypeDescriptor及替换完成的exact base/interface closure参加`is`/`as`；runtime不按nominal template或type argument subtyping合成其他application关系，不解析UTF-8类型名、比较mangled symbol前缀或把同template的application擦除为相等。Scoop没有projected/star generic descriptor、wildcard lookup或runtime generic dictionary。

M23以后TypeDescriptor地址是program-wide materialized exact type identity的一部分。同一个generic/structural specialization被多个Cone发射时，其descriptor、vtable/itable、scan/layout constant与相关adapter必须属于同一个canonical ODR group并由linker真正coalesce；同persistent exact type/group id出现两个地址、或不同exact type id共享一个地址都是fatal metadata error，runtime不得按结构相等、类型名、相同layout或“先登记者”合并。param-free nominal类型由定义Cone强定义；含consumer-local type argument的新application及跨Cone重复materialize的structural exact type由consumer发射ODR定义。

同一`PersistentExactTypeId`的canonical diagnostic bytes必须完全相同。TypeDescriptor的canonical LIR definition按值包含该span，descriptor `ObjectDefinition`/`descriptor_fingerprint`因此覆盖其length与bytes；span指向的只读diagnostic atom也必须由对应strong definition plan覆盖或作为type ODR group显式member进入object/ODR fingerprint。artifact reader与final linker从exact key重算并验证；runtime把通过验证的span视为诊断用opaque bytes，只做生命周期、只读range及同identity逐byte一致性的防御检查，绝不从它反推type identity。

M22的八种integer value按1/2/4/8 byte精确存储并使用target自然对齐；嵌入aggregate/array/CLayout或装箱时不扩成统一8-byte payload。canonical identity固定为`Int8`/`Int16`/`Int`/`Long`的i8/i16/i32/i64与`UInt8`/`UInt16`/`UInt`/`ULong`的u8/u16/u32/u64；`Byte`/`Short`/`Int32`/`Int64`与`UByte`/`UShort`/`UInt32`/`UInt64`分别是对应identity的transparent alias，不能产生多份TypeDescriptor或按alias名称选择布局。C ABI精确映射为`int8_t`/`int16_t`/`int32_t`/`int64_t`与`uint8_t`/`uint16_t`/`uint32_t`/`uint64_t`；因此`Int`不是C `long`，`Long`也不随平台C `long`变宽。当前runtime target profile同时要求data/code pointer恰为64位、两类null carrier全零，并保证合法非null地址与各自内部64位raw carrier逐bit往返；build/C bridge作宽度静态断言，target artifact test验证null表示与往返能力。M22暂不引入platform-native integer：原有64位pointer/size源码surface暂时用`Long`/`ULong`，即`Ptr(raw: ULong)`及`toULong(): ULong`承担raw往返、元素offset用`Long`，`sizeOf`/`alignOf`返回`ULong`；`FunPtr`仍不开放integer构造或accessor。不满足profile者不是可运行target，runtime不能截断/扩展pointer伪造结果。runtime metadata中的版本、tag、id、offset、size、count、epoch与state等`u32`/`u64`/`size_t`标量是各自独立的内部typed carrier，不是源码`Int`/`UInt`/`Long`/`ULong`值，不得为复用源码operator、boxing或FFI规则而伪装成任一源码integer。

ZST的`ValueStorageLayout`保存logical size 0、严格大于0的alignment与空value scan；不同exact ZST即使layout相同也保留不同persistent identity与TD地址。它的TypeDescriptor使用`BoxedValue { ZeroSized }`：`inline_size == 0`且两个scan为空，但`inline_offset = alignUp(16, inline_alignment)`，managed box的`minimum_size`仍是按instance alignment规范化后的非零值。runtime不得把ZST解释为缺失layout、未知size或“无需descriptor”。需要地址identity的source/static place遵守spec 4.7与2.8的1-byte token规则；token既不进入value layout size，也不充当box payload。

M23起`ScoopTypeDescriptor`的精确字段顺序为`type_id, instance_shape, object_scan, parent, vtable, itables, itable_count, diagnostic_name`，其中shape顺序为`u32 instance_kind, u32 inline_storage_kind, u64 minimum_size, u64 instance_alignment, u64 inline_offset, u64 inline_size, u64 inline_stride, u64 inline_alignment, inline_scan*`（完整C spelling见M23设计6.1）。合法矩阵如下：`FixedObject`的minimum是含header和尾padding的exact allocation、alignment至少8且inline字段全零/null；`BoxedValue`按`inline_offset = alignUp(16, inline_alignment)`与`minimum_size = alignUp(inline_offset + inline_size, max(8, inline_alignment))`保存未装箱value layout，stride为0，object scan是inline scan的checked平移（References offset相加、Sequence递归、Array的length/first offset相加而element child不变）；`InlineBytes`固定minimum/offset 24、alignment 8及1-byte inline size/stride/alignment；`InlineArray`固定minimum/offset为`alignUp(24, inline_alignment)`，非ZST element要求`inline_size == inline_stride > 0`，ZST element要求size/stride为0，object scan只在element scan非空时使用`Array { length_offset: 16, first_element_offset: inline_offset, stride: inline_stride, element: inline_scan }`；`AbstractRef`的minimum/alignment/inline字段与两个scan全零/null且任何allocation均fatal。`minimum_size == 0`因此只表示AbstractRef，不表示ZST。target profile给出非零`maximum_managed_alignment`与`maximum_managed_object_size`；layout超过alignment上限必须在LIR阶段拒绝，任意allocation超过object-size上限必须失败，allocator不得降级、wrap或截断。descriptor/scan fingerprint、C/LLVM `sizeof/offsetof`测试及runtime验证锁定整个矩阵。

scan program v1的word编码固定为：null表示empty；普通节点为`[count, offset...]`且`0 < count < UINT64_MAX-1`；sequence为`[UINT64_MAX-1, child_count, child_ptr...]`且`child_count >= 2`；array为`[UINT64_MAX, length_offset, first_element_offset, nonzero_stride, nonempty_element_scan_ptr]`。canonical `RefScan` normal form规定：References offset按无符号byte offset严格递增且无重复；Sequence不含None或直接Sequence child，先把同层References合并为一个offset并集，再将包含该合并节点在内的全部已规范化child统一按`(child ScanFingerprint, canonical child bytes)`严格递增、去重，零/单child分别折叠为None/该child；Array的element是已规范化的NonEmptyRefScan。producer在fingerprint/发射前规范化到fixed point，reader/runtime拒绝非canonical、越界、错对齐、过深、过大或成环graph。v1共享ABI常量固定为root depth计1的`SCOOP_SCAN_V1_MAX_DEPTH = 64`、每个top-level scan最多`SCOOP_SCAN_V1_MAX_NODES = 65536`个不同node、最多`SCOOP_SCAN_V1_MAX_WORDS = 1048576`个不同node storage word、按展开每条parent→child路径计数的`SCOOP_SCAN_V1_MAX_EXPANDED_NODES = 1048576`，以及同样按路径重计的`SCOOP_SCAN_V1_MAX_CANONICAL_BYTES = 16777216`。node/word预算对物理共享child按地址去重，depth按active path计数，expanded-node/canonical-byte cost则用memoized checked subtree cost在继续展开/比较/编码前限流，active recursion stack仍检测cycle，因此共享DAG不能产生指数工作或重复大leaf的二次工作。这五个常量进入runtime ABI fingerprint。

scan的canonical typed bytes使用2.8的scalar/count规则且没有pointer：`None = u32(0)`；`References = u32(1) || u64(count) || each u64(offset)`；`Sequence = u32(2) || u64(child_count) || each canonical child`；`Array = u32(3) || u64(length_offset) || u64(first_element_offset) || u64(stride) || canonical element`。`ScanFingerprint = SHA-256(ByteSpan("scoop-scan-v1") || canonical typed bytes)`；共享物理child按semantic tree路径重复编码但受expanded预算限制。static storage的None scan使用canonical `[0]`非null sentinel；该sentinel不是合法Recursive root，TypeDescriptor empty scan按shape矩阵使用null。

### 2.3 装箱

值类型装箱为堆对象：对象头 + 对齐填充 + 按值存储的payload（spec 4.4.4）。分配、复制和拆箱只消费TypeDescriptor的`BoxedValue` shape，不能固定假设payload在`+16`：payload位于`inline_offset`，allocation使用`minimum_size`，inline scan相对payload而object scan相对object start。compiler/runtime内部ABI以两个互斥入口实现LIR的`BoxPayload::{ZeroSized, NonZero { source_place }}`：`scoop_rt_box_zst(td)`只接受`BoxedValue/ZeroSized`且没有payload pointer；`scoop_rt_box_value(td, source_place)`只接受`BoxedValue/Inline`及满足其size/alignment的non-null readable、跨该入口地址稳定的caller place。NonZero value的inline scan非空时，caller必须先把完整value写入该place，再以TD的inline scan push覆盖它的native recursive-region root；push本身NoGC，且必须发生在调用入口及其managed-entry handshake/park之前。该root保持到entry handshake、allocation和从经collector更新后的同一place复制payload全部完成，`scoop_rt_box_value`返回后才由caller按LIFO pop；runtime验证所需region root已经活跃，不能等进入入口后补登记。空scan可省略region frame。不能在push前后缓存payload中的AS1 ref或改从旧副本复制。对应拆箱入口为不接收result pointer的`scoop_rt_unbox_zst(obj, expected_td)`与接收non-null writable destination的`scoop_rt_unbox_value(obj, expected_td, destination)`；两者都先验证exact TD identity与shape。size、alignment和scan只从已验证TD读取，调用者不得再传一份可能矛盾的size/scan。旧的`box(td, payload, size, scan)`形态不属于M23 ABI。ZST payload不占字节，但box仍分配普通非零对象、登记object start/size并获得新的ref identity，不能返回共享singleton或ZST address token；ZST unbox只产生typed logical value，需要可观察的value-type `this` place时另行物化token，不能把位于对象末尾的零字节payload地址暴露出去。装箱赋予对象identity但不赋予通用`==`：相等按表达式静态引用类型声明的成员operator equals分派；`Any`或未声明equals的interface不能使用`==`。TypeDescriptor没有通用结构相等入口。

所有 Scoop 方法 receiver 都按值传递（spec 3.3）：ref-type receiver 复制 managed ref value，因而仍指向同一对象；value-type receiver 复制完整值。装箱值经 interface 分派调用值类型实现时，box 只是 payload 的存储来源；dispatch thunk 用 payload 的值初始化 value-type `this`。thunk 可以在不可观察时借用 payload 地址作为 ABI 优化；若 unsafe/interior-mutable 路径能观察或修改存储，必须先复制 payload，不得把 box 内部地址暴露为 `this`。

### 2.4 `String` / `Array` 布局

- `String`：对象头后在offset 16保存独立typed `u64` physical byte count，内联UTF-8字节数据从offset 24开始（spec 11.4）。源码可见length、index与slice boundary均使用`Long`（i64）；实际length限于`0..=INT64_MAX`，index/boundary实参仍可取任意`Long`值并由普通bounds语义拒绝负数或越界值。physical count为allocation/copy/runtime使用的machine scalar，不是源码`ULong`值。
- `Array<T>` / `MutableArray<T>`：对象头后在offset 16保存一个独立typed `u64` physical count，offset 24是未考虑元素过对齐时的data boundary，实际元素区起点为`alignUp(24, alignOf<T>())`，object allocation alignment至少为`alignOf<T>()`。源码可见`size`、`get`/`set` index与iterator index均为`Long`（i64）；实际logical size限于`0..=INT64_MAX`，index实参仍可取任意`Long`值并由普通bounds语义拒绝负数或越界值。physical count只是为allocation、copy和runtime scan服务的machine scalar，不是源码`ULong`字段。非ZST值元素不装箱且以nonzero `sizeOf<T>()` stride连续布局；ref元素使用pointer stride。ZST元素使用封闭`ZeroSized { alignment }`分支，任意logical size的exact allocation size都等于元素区起点，不计算`size * 0`、不物化逐元素token，也不调用payload `memcpy`；clone/互转仍分配fresh目标对象并复制logical size。bounds与整数index迭代仍按spec 10章执行。只有元素子扫描非空时TypeDescriptor才使用带length/data offset及nonzero stride的array scan；ZST/GC-free元素直接为`None`。因此collector工作量不随ZST array length增长，也不会反复访问同一地址；tagged enum元素的inactive variant slot同样保持全0。

variable allocation公式固定为：String `alignUp(24 + len, 8)`；Inline array `alignUp(inline_offset + count * inline_stride, instance_alignment)`；ZeroSized array恰为`minimum_size == inline_offset`。M23没有接收任意signed length的public array constructor：literal/assembly/vararg的logical count只能由非负component count经checked加法产生并位于`0..=INT64_MAX`，再以checked zero-extension形成内部typed `u64` count；clone/互转必须先验证source exact array TD、offset 16的physical count与side-metadata allocation一致，且count不超过`INT64_MAX`，再以该count和target refined InlineArray TD分配。内部入口观察到未经证明的signed count、超过`INT64_MAX`的physical count或layout不一致都是fatal compiler/runtime invariant error，不按名称构造`IllegalArgumentException`。全部乘加/alignUp使用checked `u64`，结果还须可转target `size_t`并不超过target profile的`maximum_managed_object_size`；算术溢出、对象过大或资源耗尽沿用fatal allocation failure，不能wrap/截断。未来普通core API若接收signed length，其源码异常由该API自行实现。collector扫描array前从offset 16读取typed `u64` count，并用side metadata exact size验证count、first offset和stride覆盖范围；不一致是fatal invariant error。

### 2.5 `Option` 的 niche 表示

引用类型的全0机器字表示`None`；`Ptr`/`FunPtr`的内部data/code-pointer carrier为全0时同样只表示对应`Option`的`None`（spec 7.4）。裸reference/`Ptr`/`FunPtr`值均维持非零不变量，因而`Some(payload)`不可能与`None`碰撞。GC扫描时必须识别niche编码，不得把全0当作有效引用追踪。

### 2.6 函数值与 closure

managed 函数值是普通引用对象，不是原生函数指针。每个 concrete closure 实例包含编译器控制的 invoke entry 与零个或多个不可变捕获字段；语言只允许捕获不可重新绑定的 binding，不存在 compiler-generated shared cell。closure、函数类型型变 adapter 与 suspend closure frame 都必须有各自的 TypeDescriptor 和完备的递归引用扫描描述。

- invoke entry、TypeDescriptor 及其他代码/metadata 指针不是 managed 引用，不进入 GC 扫描描述；captured value type按 concrete layout直接内联在 closure对象中，其中的引用必须按普通字段递归扫描；该内联存储没有独立对象头、identity或额外TypeDescriptor，不构成 boxing；
- concrete closure 的 TypeDescriptor 记录其 exact function type descriptor；function type descriptor 保留挂起性、参数与返回类型 identity。编译器可以按实际使用登记型变 bridge，运行期 `is` / `as` 不得仅把不同签名按同一个“closure”根类型处理；
- 无捕获 closure 可以由编译器放入静态只读对象或复用单例，但其对象头和 TypeDescriptor 仍须满足普通引用对象契约；
- closure 的分配、调用与回收不需要新增 runtime API，走现有 managed 分配、statepoint 与动态分派设施；
- 本节对象不得直接当作 `FunPtr` 交给原生代码。spec 13.10 的 `FunPtr` callback 是独立的 GC-free 原生地址；spec 14.3 的 GC-aware closure 回调则必须先通过 runtime 注册/保活协议。

### 2.7 M21 initialization unit与singleton发布

每个需要runtime求值的top-level property及每个object/companion拥有独立、GC-free的`ScoopInitializationCell` side metadata；它不位于managed object内，也不是TypeDescriptor字段。状态为`Uninitialized`、`Initializing(owner_thread, dependency_stack)`、`Initialized`或`Failed`。cell只控制整个generated initializer entry恰好执行一次：ordinary stored property在Initialized状态下始终已有声明type的合法值，`var p: T?`省略initializer时写入的就是普通`None`；runtime不得为property另建late-init bit或读取检查。

singleton winner线程先按2.1/3.1普通分配对象，只把initializing receiver保存在generated managed frame的精确root中；base/common initializer全部成功后，才把ref以release语义写入已登记的published global root slot并把cell转为Initialized。loser线程以acquire观察terminal state后从published slot重新读取；不得在Initializing时读取临时ref、pin对象或把旧地址缓存在cell。moving collector只更新普通root slot。

initializer抛异常时，singleton不发布，top-level storage不被视为可读；runtime把managed Throwable写入单独登记的failure root slot、转为Failed并唤醒waiter。本次和以后ensure都重新抛出该failure且不重试；已发生副作用不回滚，未发布object按普通可达性回收。

同线程重入由dependency stack检测并产生带稳定unit path的cycle结果。跨线程等待由init coordinator登记wait edge；加入新edge形成cycle时，该访问产生同类结果。generated ensure把path交给编译器已解析的core constructor，抛出`message`包含完整cycle的`IllegalStateException`；它若未被initializer内的普通`try`捕获，才使当前unit失败并逐步唤醒其他waiter。runtime不按名称查找或构造core对象。无环等待必须使用与3.5 epoch/STW handshake兼容的park；不能持有未登记managed pointer睡眠，不能busy-spin阻止safepoint。

每个Cone image在2.8的typed descriptor中提供init-unit span；unit identity与初始化排序键只使用kind-specific `PersistentInitializationUnitId`，ODR去重另使用其group/member，不再另造“Cone coordinate前缀字符串key”。该persistent id的definition key已经包含origin `ConeIdentity`或specialization identity；coordinate/declaration path只放在独立`diagnostic_path`用于cycle/错误显示，不参与unit identity或排序，但仍是descriptor definition、RuntimeImage hash及ODR逐字段判等的record内容。descriptor还必须关联cell、generated initializer/ensure entry、no-throw startup gateway以及其published/property与failure storage registration。所有storage都进入2.8的static-storage descriptor span；`scan_kind == Recursive`者才按3.3成为GC root，init descriptor中的opaque状态不能代替root登记。

generic delegated extension specialization是lazy unit，其storage、cell、failure root、initializer/ensure与descriptor属于同一ODR group。多个image可以各自引用linker已coalesce的同一record；registry只有在stable group/unit id、fingerprint及全部关键地址均相同时去重一次。相同id但地址不同必须fatal，不能运行两次或扫描两个storage掩盖错误。

### 2.8 M23 program与Cone image descriptor

M23的私有metadata ABI版本为1，精确C声明为`extern const ScoopProgramDescriptorV1 scoop_program_descriptor;`，最终可执行文件必须且只能定义这一个default-visible strong symbol。所有top-level record以固定`{ u64 magic; u32 abi_version; u32 struct_size; }`开头；program/image/entry/core/storage/immortal/init/type/safepoint/callable的magic依次为`0x53434f4f50505247`、`0x53434f4f50494d47`、`0x53434f4f50454e54`、`0x53434f4f50434f52`、`0x53434f4f5053544f`、`0x53434f4f50494d4d`、`0x53434f4f50494e49`、`0x53434f4f50545950`、`0x53434f4f50535054`与`0x53434f4f5043414c`。`abi_version == 1`、`struct_size`必须精确等于共享header中的v1 `sizeof`，reserved field全零；runtime只在先验证固定16-byte prefix后读取其余字段。当前Darwin/AArch64 profile固定little-endian 64-bit pointer与natural 8-byte struct alignment；runtime C header与LLVM codegen必须以`sizeof/offsetof`产物测试锁定，不能靠双方“通常相同”。

program字段顺序固定为`prefix, runtime_abi[32], target_profile[32], graph_fingerprint[32], root_image*, entry*, core*, images**, image_count`。image字段顺序固定为`prefix, canonical {group bytes, name bytes, version bytes, ConeIdentity}, runtime_image_fingerprint[32]`，随后依次为direct dependency identity及static storage、immortal object、initialization unit、type registration、safepoint registration、callable registration六组pointer/count pair；callable pair追加在safepoint pair之后。runtime必须重验coordinate grammar、重算ConeIdentity并用coordinate bytes验证canonical topological tie-break；coordinate既是排序/identity验证输入，也可用于诊断。

六类registration record均内嵌固定`{ u32 linkage_kind; u32 zero; semantic_id[32]; odr_group_id[32]; odr_member_id[32]; definition_fingerprint[32] }`。外层record type把`semantic_id`分别解释为`PersistentStaticStorageId`、`PersistentImmortalObjectId`、`PersistentInitializationUnitId`、`PersistentExactTypeId`、`PersistentSafepointSiteId`或`PersistentCallableBodyId`；strong记录的ODR字段为零，ODR记录的group/member非零。static-storage record另按声明序含`scan_kind, initial_state_kind, writable_base, byte_size, allocation_extent, required_alignment, scan_program, scan_fingerprint, layout_fingerprint, initial_template byte span, initial_relocations*, initial_relocation_count`；child relocation record固定为`{ u64 pointer_offset; const ScoopImmortalObjectDescriptorV1 *target; }`。`byte_size`是logical payload size，`allocation_extent`必须恰为`max(byte_size, 1)`。immortal record含`object_start, object_size, required_alignment, type_registration*`；init record含`schedule, diagnostic_path, cell, storage registration*, failure-root registration*, initializer/ensure callable id与entry、startup gateway callable id/definition fingerprint/pointer`；type registration含`runtime_type_id, TypeDescriptor*, descriptor/layout fingerprint`；safepoint registration含`SafepointId, site role, root-pair count, owner callable id, normalized stackmap fingerprint`；callable registration含`body_definition_fingerprint, entry`，其function-pointer typedef只作地址identity，不能用该擦除类型发起调用。完整字段顺序与C spelling由共享`scoop_runtime_metadata_v1.h`及M23设计6.1锁定，不允许加可空尾字段模拟版本协商。

这些digest字段不各自定义临时算法：per-Cone source/layout/scan字段分别由typed DAG的`SourceSignature`/`Layout`/`Scan`节点写入；TypeDescriptor的`descriptor_fingerprint`与每个callable record的`body_definition_fingerprint`由对应atom的`ObjectDefinition`节点写入；root/Eager-init gateway fingerprint是同一gateway callable的同一个ObjectDefinition digest在另一patch site的逐byte镜像，必须等于其callable record中的body digest；Lazy-init gateway fingerprint是固定全零tagged encoding，不是graph slot；safepoint normalized字段由`StackmapRecord`写入；registration identity中的definition字段由strong record的`StrongRegistration`或ODR record所属`OdrDefinition`写入；image字段由`RuntimeImage`写入。strong registration精确使用M23设计3.4的`scoop-strong-registration-v1`公式：对应canonical record强制Strong linkage、零ODR group/member并只把own definition slot归零，其他声明的上游digest保留，direct input按`DigestKind tag + DigestNodeId`排序编码。`LirDefinition`与`ObjectSupport`可以只存在verifier index。每个object slot固定32 bytes、无relocation、初始为零且只有一个writer；同一node写入多个镜像slot时最终bytes必须一致。

每个image只列出由该Cone实际发射的strong或ODR producer record，普通external reference不重复登记；table是**record pointer** span而不是by-value record。两个Cone产生同一ODR specialization时，其pointer经link relocation必须指向同一coalesced record。runtime为六种semantic id分别建立`id -> record address`及反向map；另为callable建立`PersistentCallableBodyId <-> entry address`双向map，entry必须non-null、位于最终程序某个executable segment且同一entry不能属于两个body。callable全集精确等于各Cone LIR在`code.o`中声明的`RegisteredCallableBody`：普通managed/NoGC Scoop body、compiler adapter/trampoline与root/init gateway即使没有safepoint也必须登记；只有声明的native extern、runtime archive函数及C compiler生成的`bridge.o` storage bridge不在该集合。所有registered callable atom都是address-significant：不得使用`unnamed_addr`、跨不同body id的MergeFunctions/function alias folding或function ICF；只有完整验证为同一ODR callable member的winner可以共址。TypeDescriptor地址也只能对应一个`PersistentExactTypeId`，`runtime_type_id`必须非零且等于`descriptor->type_id`，64-bit id/full key一对一。strong重复、同id不同地址、不同type id同TD地址、body/entry非一一对应、ODR group/member/fingerprint或关键storage/cell/entry/TD地址不一致均为fatal，不能靠登记两份、constant merge或ICF掩盖。

safepoint还必须独立建立非零`SafepointId <-> PersistentSafepointSiteId`双向map；这里与上一段runtime type一样要求u64/full key全程序一一对应。不同full site key映射到同一u64时，即使owner、ODR group或return PC不同，也必须在用该ID匹配raw stackmap record之前fatal，不能让后续PC判等消歧。

所有pointer/count pair即使count为0也指向类型正确的addressable sentinel；runtime对非零span验证count上限、乘加溢出、alignment、non-null及所在loaded segment权限。v1没有per-Cone code-range字段；entry/gateway pointer必须经callable map解析为声明的body、位于最终程序任一executable segment，并且该callable record出现在要求的producer image表（strong）或同一已验证ODR owner闭包的producer表中，不能声称验证一个不存在的“Cone text range”。storage的整个allocation range必须落在writable segment，descriptor/coordinate/scan/initial-template/relocation metadata必须为只读。零尺寸storage要求`scan_kind == None`和canonical空scan，但仍通过独立1-byte writable identity token提供non-null地址；除已验证的同一ODR member外，任意两个storage allocation range不得重叠。

static初值的C tag封闭为`ZeroedForRuntimeUnit=1`与`EncodedStaticValue=2`。Zeroed分支要求template为长度0的typed sentinel、relocation count为0及typed sentinel，并在registry commit与任一managed代码前逐byte验证完整`allocation_extent`为零；只有initialization-unit的ordinary/delegate/published storage、init/root-entry failure storage可用该分支。不能以undef、未初始化BSS假设或“initializer很快会运行”代替。Encoded分支要求没有initialization/root-entry descriptor把它作为storage/failure slot，`initial_template.length == allocation_extent`，template/data range只读且每个padding、ZST token byte和Recursive managed-pointer leaf均为零；实际storage中非pointer-leaf bytes逐byte等于template。即使Encoded template恰好全零也不得改用Zeroed tag。

runtime在semantic phase先建立并验证全部immortal-object provisional map，再验证Encoded relocation sequence。记录按`pointer_offset`严格递增且无重复；offset须8-byte对齐、checked落在logical `byte_size`内并恰好命中Recursive scan展开出的一个leaf。`scan_kind == None`禁止任何relocation且实际storage必须逐byte等于template；`scan_kind == Recursive`要求每个实际非null leaf恰有一项record、每项record也恰对应该leaf，record target必须解析到本程序已登记且位于只读segment的immutable immortal object，leaf bits须逐bit等于其精确`object_start`。其余leaf必须为null；immortal interior pointer、未登记静态地址及任意heap ref一律fatal。link/object verifier还要求writable atom在每个非null leaf恰有一条exact-width、zero-addend、指向同一typed immortal object-start的relocation，且child record的target pointer relocation指向同一immortal registration；runtime不因object verifier已经通过而省略上述post-ASLR逐byte检查。显式raw `@Global`/`@ThreadLocal`与只读immortal object不属于这两个storage初值分支。

`RuntimeImageFingerprint`的canonical record不是raw struct bytes，而是封闭`StaticStorage | ImmortalObject | InitializationUnit | TypeRegistration | SafepointRegistration | CallableRegistration` sum。最外层sum tag就是record kind的唯一编码；每个variant payload先编码不含kind的`{linkage, semantic id, ODR group/member-or-zero, definition fingerprint}`，再依次编码：storage的scan kind、`OwnStorageAtom`、logical size/allocation extent/alignment、`EmptyScan | ScanProgram(scan fingerprint)`、scan/layout fingerprint，以及`ZeroedForRuntimeUnit | EncodedStaticValue { allocation-extent template byte span, sequence<{pointer offset, target PersistentImmortalObjectId}> }`初值sum；immortal的`OwnImmortalAtom`、size/alignment与target exact type registration id；init的schedule、diagnostic path bytes、`OwnInitializationCell`、storage/failure storage id、initializer/ensure callable id及其`CallableEntry` role、`None | UnitGateway(startup gateway body id, gateway fingerprint)`；type的runtime type id、`TypeDescriptorAtom(exact type id)`、descriptor/layout fingerprint；safepoint的64-bit id、site role、root-pair count、owner callable id与normalized stackmap fingerprint；callable的body definition fingerprint与`OwnCallableEntry(body id)`。canonical encoder统一使用`u32/u64`小端、32-byte id/digest原字节、`u64 length + bytes` byte span、`u64 count + elements` sequence、无padding的声明序product和`u32 tag + payload` sum。固定tag为：record kind按上述顺序为1..6；linkage `Strong=1, Odr=2`；scan `None=0, Recursive=1`；static initial state `ZeroedForRuntimeUnit=1, EncodedStaticValue=2`；schedule `Eager=1, Lazy=2`；scan-program choice `Empty=0, Program=1`；gateway choice `None=0, Unit=1`；typed role `OwnStorageAtom=1, OwnImmortalAtom=2, OwnInitializationCell=3, CallableEntry=4, UnitGateway=5, TypeDescriptorAtom=6, RootGateway=7, OwnCallableEntry=8`；site role `ManagedPoll=1, ManagedCall=2, ManagedInvoke=3, NativeSafeTransition=4, NativeBorrowedTransition=5`。Encoded canonical bytes内联template内容并把每个C target pointer替换为typed immortal semantic id；template/relocation-array地址、target record地址和storage中ASLR后的pointer bits均不进入key。每张表按`(semantic id, linkage, ODR group, ODR member)`byte序严格递增并分别编码count，direct dependency identity也按digest bytes递增、去重并编码count；unknown/reserved tag一律拒绝。StrongRegistration fingerprint复用own-definition置零后的这份完整sum bytes，不在前面再写第二份record-kind tag。

每个machine body的persistent identity为`SHA-256(ByteSpan("scoop-callable-body-v1") || canonical(CallableBodyKey))`。`CallableBodyKey`同样使用`u32`小端tag与声明序字段：`Strong=1 { owner }`、`Odr=2 { member }`、`RootGateway=3 { root_cone, main_body_id }`、`InitializationStartupGateway=4 { unit_id }`；unknown tag拒绝。main body、root gateway、initializer、ensure与startup gateway因此都是不同body。root/init gateway及每个safepoint的owner都必须解析到callable registration；所有raw function address只用于验证/调用，不进入canonical identity或hash。

`normalized_stackmap_fingerprint`也不能hash raw LLVM section slice。每个已匹配registration的record规范化为声明序`{format_version=3, site_id, safepoint_id:u64, owner_callable_id, site_role:u32, root_pair_count:u32, instruction_offset:u32, stack_size:u64, locations, live_outs}`；location是`Register=1 {size:u32, dwarf_register:u32}`、`Direct=2 {size, dwarf_register, signed_offset_bits:u64}`、`Indirect=3 {size, dwarf_register, signed_offset_bits:u64}`或`Constant=4 {size, value_bits:u64}`，live-out为`{dwarf_register:u32, size:u32}`。raw顺序和两个sequence count都保留；i32 offset/constant先符号扩展为i64 two's-complement bits，`ConstantIndex`必须有效并用pool原始u64值规范化成同一个Constant tag，因此pool index、pool顺序及未引用entry不进入hash。Register offset、Constant register、reserved/flags/padding必须为零，unknown kind拒绝。fingerprint精确为`SHA-256(ByteSpan("scoop-stackmap-record-v1") || canonical(record))`。function address经`owner_callable_id`的callable map取得并必须逐bit等于raw function table address；该ASLR地址及`return_pc`不入hash，但`instruction_offset`与`stack_size`进入。当前Darwin/AArch64 profile另要求location数为`3 + 2 * root_pair_count`、前三项是8-byte Constant且每对root是相同的8-byte SP/FP Indirect可写slot；registration中的site/body/role/root count逐项相等。

所有pointer在canonical key中都替换为上述own atom/scan/cross-registration/callable role，绝不输入ASLR地址：跨record pointer必须解析到目标kind-specific registration，scan内容必须与fingerprint一致；link verifier另用relocation证明pointer指向typed symbol，runtime重验地址映射、segment/producer关系及ODR duplicate相等。TypeDescriptor内部pointer语义由descriptor fingerprint承诺，runtime只作shape、scan和已登记type relation的防御性检查。RuntimeImage hash stream依次编码`ByteSpan("scoop-runtime-image-v1")`、runtime ABI、target profile、canonical Cone record、sorted direct dependencies及按record-kind顺序的六个独立table sequence；自身slot、raw `code.o` bytes、ASLR地址与program-level core binding排除。typed digest DAG要求runtime-image node显式依赖record key中出现的definition/layout/scan/descriptor/gateway/callable/stackmap节点。

program graph另定义`RootEntryKey = { owner ConeIdentity, main callable id, source-signature fingerprint, gateway callable id, gateway-definition fingerprint, failure-root PersistentStaticStorageId, RootGateway(gateway callable id) }`与`RuntimeCoreBindingsKey = { core ConeIdentity, SCOOP_CORE_STRING_CAPABILITY_ID_V1, String exact/type-registration id, header/length/data/minimum/alignment/scan fields }`；entry的failure-root/gateway及core的core-image/String-registration四个raw pointer必须先经provisional registry解析成typed identity并验证owner关系。core binding的String scan字段由final-program builder复制已验证core `Scan` digest，不另造hash。Graph hash stream依次编码`ByteSpan("scoop-program-graph-v1")`、runtime ABI、target profile、root Cone identity、两个key及canonical-topological `{ConeIdentity, RuntimeImageFingerprint}` sequence；其SHA-256为`GraphFingerprint`。它不属于Cone digest DAG，由final-program builder在program graph slot为零时唯一写入，只排除该slot。semantic/object leaf -> ODR/strong registration definition -> runtime image -> finalized code/artifact -> program graph是单向DAG；每个节点只归零自己和非传递依赖的digest slot，不能以“尚未计算”定义视图。Rust producer/link verifier与C runtime必须共享逐bytegolden vectors；manifest、object/link verifier与runtime可重算的层必须使用相同domain、字段顺序、tag、count、排序与归零集合。

root entry不是裸Scoop function pointer。`ScoopRootEntryDescriptorV1`保存root Cone、persistent main body、ordinary `() -> Unit` source signature fingerprint、由root image producer表拥有的已登记failure root，以及gateway body id/fingerprint和精确`uint32_t(void)` C-callable gateway。gateway id必须由`RootGateway { root_cone, main_body_id }`重算，pointer逐bit等于该body的callable registration entry，两个位置的ObjectDefinition digest逐byte相等。gateway以自己的body identity登记内部managed call/异常路径全部safepoint，再调用namespaced main：成功返回0；未捕获异常必须在generated landing pad内物化并发布failure root、结束native catch后返回1；其他值fatal。

init schedule在IR中是封闭`EagerStartup { gateway } | LazyAccess`。Eager的startup gateway body id由`InitializationStartupGateway { unit_id }`重算，fingerprint非零且等于对应callable body digest，pointer逐bit等于该entry；Lazy的gateway id/fingerprint全零且pointer为null。两种schedule的cell、storage/failure registration及initializer/ensure body id/entry都必须non-null/nonzero并由当前strong producer或unit同一ODR group拥有；initializer/ensure pointer逐bit等于各自callable registration entry。eager init只能由no-throw startup gateway从C调用，普通initializer/ensure entry仅供generated managed代码；lazy访问仍经普通ensure抛向Scoop caller。由此任何startup异常都不会跨C ABI frame。

主线程attach后处于native-safe且没有活动managed栈段。C startup coordinator对**每次**init/root gateway调用都执行独立native→managed transition wrapper：在本次wrapper栈底push `NativeToManagedBoundary`，LIFO保存previous mode/boundary，发布封闭`EntryPending`状态（gateway尚未执行，因此段内没有managed frame）；发布managed mode后按3.5复查GC phase/epoch，需要时凭empty-segment证明park，不能把C PC作为managed anchor。compiler的typed `NativeGatewayEntry`强制gateway入口poll以自己的body/site id发布准确anchor，并将段切为`ActiveManagedSegment`，随后才执行普通managed body。walker必须在本次boundary停止，不扫描C coordinator。gateway在managed段内完成异常物化、failure-root发布及EndCatch，只返回GC-free status；wrapper返回后以release语义发布native-safe，按同一epoch/snapshot协议移除boundary并恢复外层记录，collector可能读取的record不可提前销毁。每个eager调用和root调用各自enter/leave，不能用attach时的一条boundary覆盖多个调用；嵌套callback继续独立建段并LIFO恢复。

init `failure_root`与root-entry `failure_root`是封闭role的专用static storage，不是任意合法root：当前profile精确要求8-byte logical/allocation size、8-byte alignment、`Recursive`、canonical `References { offsets:[0] }`（word program `[1,0]`）、`StaticInitialState::ZeroedForRuntimeUnit`，并在任何managed代码前保存全零null ref且没有template/relocation payload。它不能兼作property/published/ZST token或不同unit/root的failure slot；只有同一已验证ODR identity可以重复引用。init `cell`固定size 16/alignment 8，必须位于writable segment并初始恰为`{state=Uninitialized(0), owner_thread=null}`；不同unit的cell互不重叠，也不与任何storage range重叠。普通init storage的persistent key必须绑定该unit及property/published role，不得与另一unit共享registration或range，也不能等于本unit failure root。runtime在coordinator或gateway可能写入前建立并验证`cell address -> unit`与`storage id/address -> unit`反向map。

`ScoopRuntimeCoreBindingsV1`至少保存reserved core image、String persistent exact type id与其type registration，以及String header/length/data/min-size/alignment/scan contract。linker先把该id与trusted core Export HIR/LIR的`RuntimeCoreCapability::String` well-known relation精确比对。共享ABI header的`SCOOP_CORE_STRING_CAPABILITY_ID_V1[32]`精确为`SHA-256(ByteSpan("scoop-runtime-core-capability-v1") || ByteSpan("String")) = d69647675041ab4e7a2550b9a5a27b49741c08b30ab07b259d998e606f218b77`；它只标识versioned String capability kind，本身不编码runtime ABI，Graph另把`runtime_abi`作为独立必需输入。runtime直接使用这32 bytes，不按NUL字符串、host endian或symbol地址重算。runtime还要求core image coordinate精确为`scoop:scoop.core:0.1.0`，String registration由该image自身producer表拥有、id/TD地址一致，并验证object header 16 bytes、length offset 16、inline bytes minimum/offset 24、alignment 8、TD为`InlineBytes`且inline size/stride/alignment均为1、object/inline scan均为空；runtime不再按`scoop_td_String`等源码符号或diagnostic name查找能力。

program descriptor强引用全部image，image再强引用六类metadata producer table。M23的Cone image是最终可执行文件内的logical image，不是独立Mach-O/dylib；不支持运行期静态metadata注册、`dlopen`、卸载或global destructor。Darwin linker会把各object的`__LLVM_STACKMAPS,__llvm_stackmaps`串接成多个完整v3 blob而非合成一个header；parser必须逐blob按count/对齐算出精确消费长度并解析至section末尾。ODR重复site只在full site/body owner/group/member、最终链接/ASLR relocation后的**原始return PC**、canonical location/live-out payload与fingerprint全等时去重，不做`PC-4`、nearest-symbol或range修正。M23 Darwin final-link profile禁止`-dead_strip`，因为descriptor引用不能保活无符号stackmap section；缺失任一safepoint或callable registration对应record的artifact在startup前fatal。

上述去重不放宽固定宽度碰撞门禁：非ODR或不同full site key复用同一`SafepointId`或原始return PC均fatal。

---

## 3. GC 契约

参考实现由M9的单代非移动Immix和M13的多mutator STW演进而来；M15基线为单代、STW、单线程collector的moving Immix。以下是编译器与runtime共同遵守的长期契约；后续分代或parallel/concurrent实现可以替换算法，但不得破坏root、safepoint、native借用、pin与handle语义。M15首个runtime target为macOS/AArch64；其他target在拥有等价的精确frame/location adapter前不得退回保守扫描运行moving collector。

### 3.1 分配入口

分配分为两条通道：

- **managed 快速通道**：编译器生成的代码**不使用 handle**，直接取得裸指针。标准形态是 TLAB 碰撞指针（bump pointer）内联序列；成功后由不含safepoint、不取得heap/world锁的GC-leaf `scoop_runtime_finish_tlab_alloc`完成清零、对象头与object-start/精确allocation-size side metadata登记，缓冲区耗尽时才落入 slow path（`scoop_runtime_alloc_slow`，可能触发 GC）。编译器必须在 safepoint保留完整root信息，包括post-site live leaf与普通managed call的可移动实参leaf；M15 moving collector精确消费statepoint stack map并更新其location，生成代码在调用后只使用`gc.relocate`结果（impl spec 2.4/2.5）。managed 分配是最高频操作，其成本必须保持摊销 O(1)，不经 handle 表。
- **Scoop ABI native 通道**：外部实现没有 stack map；它可以直接借用传入的 managed ref，但在调用可能触发 GC 的 runtime入口前，必须先把仍需使用的引用登记为 native root slot（见 4.2）。需要把新对象长期带出 native frame时可使用 handle；需要稳定裸地址时使用 pinned allocation。

本章未细分时出现的“native root”“root slot（链）”只是4.2封闭root frame链的简称，不表示只支持direct slot：直接ref使用`DirectSlots`，含managed leaf的内联value/aggregate必须使用`RecursiveRegion`。collector及每个可能GC的native-borrowed入口都必须穷尽匹配两种variant。

M13 起每个已attach线程持有独立TLAB；slow path在同步的heap元数据下从Immix free-line run或新block切出互不重叠区间。STW开始后全部TLAB失效，GC结束后各线程在下一次分配时重新refill。内联bump与上述GC-leaf finish helper共同构成fast path，并必须在返回前清零完整对象、初始化对象头，并以原子方式同时登记object-start与精确normalized allocation size，保证多mutator分配与构造中途safepoint都可安全扫描。只登记start而没有size不是合法的已发布对象状态。TLAB具体尺寸可调，但managed分配返回裸指针、摊销O(1)、不经handle表的契约不变。

### 3.2 safepoint 模型

- managed（编译器生成）代码：statepoint poll（impl spec 2.4/2.5）。函数入口及每条循环回边在LIR中已经是带完备root plan的显式poll，不由codegen补插；M22的普通循环尾与continue edge都必须经过对应header poll。managed ref在LLVM中使用address space 1，普通poll/call的post-site live leaf及可移动实参leaf由statepoint/`gc.relocate`描述。含ref aggregate必须先拆为独立leaf，不能把aggregate alloca当作隐式stack region；
- managed `invoke`：LLVM当前异常边relocation不能作为语言实现基础。编译器在invoke前把normal/unwind后继仍活跃的ref leaf及可移动实参leaf写入独立compiler root frame，每项携带normal/unwind reload角色；codegen直接发射零`gc-live`的显式statepoint invoke，两个后继只从各自所需slot reload并在继续控制流前pop，不得产生exceptional `gc.relocate`；
- outbound native transition：native-safe/native-borrowed调用前发布只描述transition顶层generated frame的完备caller-root frame，并以唯一`SafepointId`把transition入口发射为零`gc-live`/relocate statepoint；平台薄入口在C prologue前捕获该record的PC/SP/FP。transition入口本身可能park并使root被改写，所以入口返回后必须先从caller-root storage重新构造含ref实参，不能把入口前求值的AS1 SSA值传入native callee。顶层frame只从caller-root storage reload，冻结segment的外层managed frame从该anchor按精确stack map扫描；实际native machine call为nounwind普通调用，不跨native frame反向unwind。返回值含ref时使用调用前已清零并登记的result storage，在native-borrowed返回后、leave可能park前写入，回到managed后连同其他顶层live root只从slot reload；
- Scoop ABI FFI：函数体内**无** safepoint poll，GC 只能在其显式调用 runtime或回调 managed代码时于该入口内部发生；跨越这些入口的 direct ref必须位于 native root slot（spec 14.3）；
- C ABI callee不接收 managed ref，也不得直接调用 Scoop GC；M12 单 mutator实现可把它视为纯 GC leaf。M13启用多mutator后，outbound caller必须在调用前发布live roots并进入native-safe，使其他线程发起的STW GC无需等待一个可能阻塞的C调用。C代码若持有4.3注册所得的静态trampoline与cookie，可以经这个**独立反向边界**进入managed callback；这不使原C callee获得direct ref或Scoop ABI能力，外层caller roots继续由native-safe transition保活。

M15的macOS/AArch64 runtime只更新stack-resident managed roots。固定的LLVM 22.1 backend profile通过SelectionDAG默认spill与post-RA statepoint fixup保证每个GC base/derived location都是可写的8-byte `Indirect [SP/FP + offset]`；runtime不保存、unwind或改写register root。stackmap前三个constant location、deopt location与live-out不属于GC root pair；通用parser必须能区分这些区域，DarwinAArch64 profile只在GC root位置拒绝`Register`/`Direct`/constant等非约定形态。

### 3.3 根集合

- 栈根：managed caller在statepoint处的live root location、managed invoke显式compiler root frame、Scoop ABI调用点保持的caller roots，以及native callee显式登记的可更新root slot链。M19的initializing receiver从class allocation返回起就是普通managed root，跨base/`this` initializer、property/`init` managed call与异常边时按同一规则relocate；不存在constructor专用handle或不可移动区。runtime以精确return PC查stack-map record并把每个location解析为可写slot；不存在保守栈扫描fallback；
- 静态storage root：ordinary top-level property/delegate storage、object/companion published singleton slot、root entry及initialization failure slot。LIR/codegen为每个compiler-owned static storage输出2.8的typed descriptor、非可选`scan_kind`与封闭`StaticInitialState`；`ZeroedForRuntimeUnit`在任一managed代码前使完整extent为零，`EncodedStaticValue`则只允许Recursive leaf为null或经typed relocation指向已登记immutable object-start，两者都在registry commit后携带完备scan登记为可写root。`None`明确表示GC-free且禁止relocation，只参与storage/init identity验证。零尺寸payload强制为`None`，但仍拥有独立的1-byte writable identity token；该token在两种variant中的canonical byte均为零，GC绝不扫描。spec 13.6显式`@Global`/`@ThreadLocal` raw storage仍是另一种FFI representation；
- stable/immortal对象：活跃的Scoop exception record中按值复制的对象payload作为动态stable external object登记，其内部ref slot可更新；编译器生成的静态String等只读immortal object必须有精确地址/size/TD登记。M15只允许GC-free payload的只读immortal对象；未登记的heap外地址不能出现在managed slot；
- handle 表与 pinned 对象（`GcHandle` 保活其引用对象；pinned 对象作为根被扫描，见 3.4）。

M23以后不再引用固定`scoop_image_*`符号；runtime只遍历2.8 program descriptor强引用的全部image span。static-storage/immortal/type/init/safepoint/callable全部使用2.8带prefix、typed registration identity及kind-specific字段的v1 record；不能再接受旧`{base, scan}`、`{start, size, td}`by-value table、裸TypeDescriptor/SafepointId span或未经登记的function address。count是唯一权威，空span使用addressable sentinel。runtime必须先对全部image完成DAG、table、range、TD与callable双向identity、ODR与duplicate验证并建立static metadata registry，才初始化GC heap或执行任一managed initializer；不能验证一个image后立即运行其副作用。

普通不同storage/unit semantic id必须全程序唯一；不同identity的storage allocation range与immortal range分别不得重叠，TD/header必须匹配，只读immortal object不得含managed出站引用。ODR重复先按record variant验证后去重：storage要求record地址、group/member/id、definition、base、logical size/allocation extent/alignment、scan/layout、initial-state tag、template span地址/长度/逐byte内容、relocation-array地址/count及每项offset/target registration全同；immortal要求record地址、identity/definition、start/size/alignment及type registration全同；init要求record地址、identity/definition、schedule/path、cell、storage/failure、body id/entry/gateway全同；type要求record/TD地址、identity/definition、runtime id、descriptor/layout全同；safepoint要求record地址、identity/definition、site/body owner/root count/stackmap fingerprint全同；callable要求record地址、body id、registration/body definition fingerprint与entry全同。只有已经这样确认的同一ODR record才可共享range/address；同id不同地址、不同body id共址或同一body id对应不同entry都是fatal metadata error，不能同时登记两个slot来掩盖链接失败。**静态program/image metadata registry**在进入managed startup前冻结，M23不接受后续静态image/static-storage/immortal/type/init/safepoint/callable登记；native root frame、active exception stable external root、handle与pin仍按既有API动态增删，不受该冻结影响。

### 3.4 保活机制（两级）

- **pin（对象头标志）**：保活且阻止移动。pin 标志位于对象头，当前64位源码surface上的`PinnedPtr.raw: ULong`即对象地址（spec 14.1），pin/unpin 是 O(1) 的对象头读写，**不经 handle 表**。带 pin 标志的对象作为 GC 根被扫描（其出站引用必须被追踪），但自身不移动。这个源码`ULong`是本版本的临时carrier，后续与native integer的归属关系另行决定；不允许runtime将任意`ULong`当作已验证的pinned object地址。
- **`GcHandle`（handle 表）**：保活，不阻止移动。handle 表是 runtime 私有结构；64位host使用带generation的`(generation, slot)` opaque编码，0保留为null niche，slot复用时generation递增，避免已释放handle错误命中新对象。源码`GcHandle.raw: ULong`只是该固定64位opaque token的ABI carrier；generation、slot与live state仍是runtime内部typed metadata，不获得源码`ULong`算术语义。GC 移动对象后负责更新表项。

只在一次 Scoop ABI 调用内、且不跨 safepoint使用的 direct ref不需要 pin/handle。需要稳定裸地址时优先 pin；需要长期保活且允许移动时使用 `GcHandle`；仅需跨 native callee内的 safepoint时优先使用 native root slot，避免改变对象移动属性或建立长期 handle。

### 3.5 线程

线程创建/销毁时向 GC 注册/注销。M13建立的多 mutator协议使用带单调GC epoch的合作式 stop-the-world handshake，并至少区分 managed、native-safe、native-borrowed、parked与collector状态：managed线程在入口/回边 poll或managed runtime入口发布平台定义的top managed anchor后停顿；C ABI outbound call发布顶层caller roots并冻结当前managed栈段后进入 native-safe，collector无需等待其返回；持有 direct ref的 Scoop ABI native code属于 native-borrowed，只在登记 native roots的显式 runtime/managed入口或返回边界参与协调。每次managed/native重入都在LIFO transition链中划分managed栈段。活动managed段从top anchor按精确stack map walk；每个冻结段的顶层frame由caller-root storage表示，外层frame从进入native前由零root transition statepoint捕获的anchor继续精确walk到该段boundary。collector不扫描native stack，也不从当前native PC或C frame反向猜测managed入口。collector 不得在未握手的普通 native-borrowed指令之间移动对象或扫描仍在变化的native栈。进入协调点时，当前线程anchor、transition anchor、compiler caller-root frame与native root slot链必须可扫描、可更新。compiler caller-root entry由value storage地址和递归scan descriptor组成；tagged enum的固定ref偏移可直接扫描，因为inactive variant slot按spec 7.4恒为全0。含ref的Scoop ABI返回storage从push前的全零值开始登记，在native返回后、leave可能park前写入结果，并在恢复managed后reload。

进入managed、离开native-safe或离开native-borrowed必须与epoch发布形成无丢失握手：转换方发布mode后复查phase/epoch，观察到`Stopping`或epoch变化时在执行第一条managed指令前park；collector先release发布`Stopping`再以acquire读取线程mode。不得把线程按native-safe计为quiescent后又允许它未经复查进入managed。

M23的C startup coordinator遵守2.8逐次gateway transition协议。attach本身不构成永久managed段；首次managed指令前的`EntryPending`是已证明为空的新段，允许epoch握手但不允许扫描native frame。gateway首次poll建立自己的精确anchor后才转为活动managed段，返回时先退出该段再继续native-safe coordinator。

foreign thread在进入任何 managed代码前必须 attach，建立 TLS thread state、栈边界、TLAB和空 native-root链；离开最后一个 managed callback后由拥有本次 attachment的入口 detach。重复使用的长期 foreign thread可以显式保持 attachment，但不得在 runtime shutdown后重新进入。

M15的platform bundle由object-image、OS thread/VM与architecture/ABI frame三个完备组件组成；GC arena reservation、page protection与线程栈边界都只经OS/VM组件，通用heap/collector不直接调用`mmap`/`mprotect`。通用runtime只经该bundle把top anchor与每一帧return PC/stack-map location转换为可写root slot，不读取Mach-O/x29等平台细节。macOS/AArch64基线强制generated/runtime frame pointer、禁止managed tail call，并要求return PC原值精确命中record；不得使用`PC-4`、最近函数、symbol或地址范围猜测。任何缺失record、越界location、未知DWARF register或当前target不支持的GC-root location kind都是fatal metadata错误；生产编译器应已在object验证阶段把这种情况报告为compiler/toolchain invariant failure，runtime检查只防御错误链接、损坏或非Scoop产物。每个由generated managed code直接调用且可park/collect的managed runtime symbol必须通过薄入口先发布`{ return_pc, callsite_sp, frame_pointer }` opaque anchor，再调用平台无关实现；runtime内部不得重入该入口并把C frame冒充managed frame。Scoop ABI native实现调用的是另一组native-borrowed入口：它验证已发布的caller/native roots并参与握手，但不捕获C caller或伪造managed anchor。

### 3.6 屏障

是否需要写/读屏障取决于 GC 算法；编译器侧预留插桩点（具体形式随 GC 方案确定）。M15的单代collector仍不消费card table，但多mutator对card的标记必须使用atomic monotonic store/RMW；多个线程写入同一个普通byte即使值都为1也不能视为无数据竞争。

### 3.7 moving collection与side metadata

M15 collector为选中的from-space对象建立arena外forwarding关系，把未pin live object复制到独立to-space，再通过统一可改写slot visitor更新全部root和heap字段。普通模式可按block占用率选择source，但只要存在可移动live object且to-space足够，一次显式full collection至少选择一个eligible source，不能退化为永久不移动的mark/sweep；stress模式必须移动全部未pin live object。pinned对象不移动但其出站ref仍更新；`GcHandle`只更新table entry，generation/slot identity不变。

block state、object-start、每个对象的精确normalized allocation size、line/free-run信息与forwarding均位于GC arena外。block header、free-block node或hole node不得存放在可能poison/保护的arena内。可变长String/Array及large object的复制长度直接读取allocation metadata，不能从TypeDescriptor fixed size、block span或payload内容反推。

collection在释放world前必须重新验证所有合法slot不再指向forwarded旧地址。stress mode在每次mutator-visible managed allocation前执行full moving collection；evacuation allocation不可递归触发collection。验证完成后用固定字节`0xA5` poison旧副本；不再含live/pinned对象的source block整块`PROT_NONE`并永久quarantine，partial pinned block中的已搬span同样poison且在stress进程中不复用。

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
- native-borrowed分配入口在第一个可能GC的动作前验证当前thread transition anchor及完整root frame链。它触发collection时扫描冻结managed segment顶层的caller-root frame、native实现登记的全部`DirectSlots | RecursiveRegion` frame，并从预先捕获的transition anchor精确扫描外层managed frame；不捕获当前C return address，也不跨native frame反向unwind；返回后native代码分别从slot或region base reload。

### 4.2 native root、pin 与 handle 操作

- native root链的frame kind是封闭`DirectSlots | RecursiveRegion`。`push_native_roots(slots, count)` / `pop_native_roots()`等价能力登记一组`void **` direct-ref slot；`push_native_region_root(base, byte_extent, scan)` / `pop_native_region_root()`等价能力登记`{non-null base, nonzero byte_extent, NonEmptyRefScan}`，供含内联managed leaf的caller/value storage使用。具体C结构可以内联携带`previous + kind + payload`，但必须是类型化runtime API，不能依赖C栈保守扫描猜测，也不能用`count == 0`或null scan伪装另一variant。
- 两种push/pop本身都不得分配、触发GC或回调managed代码；所有root frame共享一条严格LIFO链。RecursiveRegion的base/extent在frame生存期内地址稳定，scan必须是已canonical且通过2.2资源限制的只读nonempty program；push先验证所有静态offset/alignment，collector再用同一recursive slot visitor相对base扫描并在移动后原地更新每个leaf，动态Array节点还须使length/乘加落在extent内。用于2.3 boxing时，extent/alignment/scan精确来自已验证BoxedValue TD的inline layout，`scoop_rt_box_value`要求链顶region frame与`source_place`及该TD三者逐项相符。
- generated caller在进入可能park的transition/managed-runtime入口前以release语义发布完整frame，入口/collector在把线程视为可扫描后以acquire观察；pop只能发生在入口返回并从更新后的region/slot reload之后。direct native代码跨safepoint后必须从slot reload，aggregate/value代码必须从region base重新读取，不能缓存旧managed leaf。
- generated managed caller使用的region push/pop经compiler-private `GeneratedNoGcLeaf`入口或等价内联序列完成；该入口只操作当前TLS root链，必须NoGC、nounwind、有限时间且不得执行allocation、park、handshake或callback，因此它不建立新的native transition，也不需要stackmap。源码与普通extern不能取得该symbol；generated CFG在随后的managed entry statepoint之前完成push，并在所有非fatal出口匹配pop。
- direct ref只在无 safepoint的同步借用区间内可以作为普通 C pointer缓存；不得把该副本保存到 root frame之外、全局存储或调用返回之后。

- pin / unpin：直接读写对象头的 pin 标志（`scoop_runtime_pin(ptr)` / `scoop_runtime_unpin(ptr)`），O(1)。
- `scoop_runtime_pin_handle(handle) -> ptr`：把传入的（可移动）handle 解析为当前地址并固定，用于 FFI 收到 `GcHandle` 参数又需要裸指针的场景。
- handle 校验：runtime 必须校验generation、slot与live状态；非法或stale handle按4.4视为fatal runtime ABI error。
- Scoop 侧的 `pin` / `unpin` / `getGcHandle` / `releaseGcHandle`（spec 14.1）本身就是以 Scoop ABI 实现的 extern 函数，映射到上述能力。

### 4.3 回调 Scoop closure

- M13 提供 managed callback registration协议。概念入口为 `scoop_runtime_callback_register(closure, adapter, signature, mode) -> cookie`：注册函数按 Scoop ABI直接接收 ordinary、非 suspend closure，为其建立 `GcHandle`，并在runtime registry中创建 opaque token slot。64位host的GC-free cookie在目标ABI保证可往返且保持canonical的非零payload位内编码generation和slot，只做`uintptr_t`/`void *`往返、从不解引用；超出该位预算的slot/generation不得分配。slot复用递增generation，因而可检测stale/use-after-final-release而无需永久泄漏C heap tombstone。
- token slot至少保存closure handle、首个异常handle、typed adapter、静态signature descriptor、`Reusable`/`OneShot` mode、owner/active计数和完成/失败状态；C侧不得读取这些字段，也不得把cookie当地址解引用。
- callback ABI中的mode/state整数只是固定wire code，不具有Scoop nominal enum identity：`scoop_runtime_callback_register`的`uint32_t mode`中`0/1`分别表示header常量`SCOOP_FOREIGN_CALLBACK_REUSABLE`/`SCOOP_FOREIGN_CALLBACK_ONE_SHOT`及经core-contract验证的`Reusable`/`OneShot`；其他mode输入是fatal ABI error。`scoop_runtime_callback_state`的`uint32_t`返回中`0/1/2/3`分别表示header常量`SCOOP_FOREIGN_CALLBACK_REGISTERED`/`ACTIVE`/`COMPLETED`/`FAILED`及经验证的`Registered`/`Active`/`Completed`/`Failed`。compiler metadata必须原子保存每个code对应的exact typed variant并在边界穷尽转换；runtime返回其他state值属于ABI破坏，generated code必须fatal/trap，不能把该整数直接作为语言enum tag。
- 编译器为每个实际导出的 concrete函数类型生成 managed invoke adapter，并按`(C signature, context index)`生成/复用静态 C ABI trampoline。源码`contextIndex: Long`在HIR已验证为非负且可索引该签名，进入runtime/bridge metadata后是独立typed callback-parameter index，不是源码`Long`。trampoline按真实 C签名收参，移除被token占用的context参数，把其余值写入 C-FFI-safe args/result storage，再调用 C-callable `scoop_runtime_callback_invoke(cookie, signature, args, result)`；runtime不能用未类型化可变参数直接猜 managed invoke ABI。
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

### 4.6 Initialization coordinator

- 任何`enter`前，descriptor必须已经由2.8/3.3的全程序registry验证并intern。eager startup顺序是dependency Cone先于dependent Cone；互不依赖的ready Cone按canonical coordinate byte order；Cone内按`PersistentInitializationUnitId` bytes严格递增，coordinate/declaration path只用于显示。runtime独立验证program给出的canonical order。直接ensure仍可按普通依赖提前运行目标unit，cell/wait-edge语义不因此改变；generic delegated extension unit始终lazy，不进入eager loop。
- M21 的generated ensure先调用typed managed入口`scoop_rt_init_enter(descriptor)`。入口以acquire语义观察cell并返回封闭结果：当前线程获胜为`RunInitializer`，已经Initialized为`Ready`，既有失败为`Failed(rooted Throwable)`，同线程重入或跨线程wait-for cycle为`Cycle(GC-free stable unit path)`。无环等待按2.7、3.5进入safepoint-aware park，醒来后重新观察状态；生成代码不得自行spin、缓存cell字段或绕过coordinator。
- generated ensure对`Failed`从已登记failure root取得managed异常并重新`scoop_rt_throw`；对`Cycle`调用HIR已经绑定的`IllegalStateException(message)` concrete constructor，把path复制为普通managed String后抛出。enter返回所需的managed ref/result storage与GC-free path metadata必须由typed call/result plan完整描述；path的runtime side storage至少存活到复制完成。coordinator不认识core名称、constructor symbol或对象layout。
- `RunInitializer`分支执行ordinary managed initializer body。成功后，生成代码先完成property storage写入，或以release语义写入singleton published root slot，再调用`scoop_rt_init_succeed(descriptor)`；该入口发布Initialized、移除dependency/wait edge并唤醒waiter。`Ready`分支只从已经登记的storage/root slot重新读取结果。
- initializer抛出的native exception record不能保存进cell，也不能跨线程共享。generated catch-all在catch仍active时调用5.4的`scoop_rt_materialize_exception(caught)`得到普通managed `Throwable`并结束native catch，再调用`scoop_rt_init_fail(descriptor, throwable)`；该入口把throwable写入已登记的failure root slot，以release语义发布Failed，移除dependency/wait edge并唤醒waiter。随后当前访问从该managed对象重新`scoop_rt_throw`；以后所有访问从failure root读取并重新抛出同一managed对象，但每次都会建立新的native unwind record。
- `enter` / `succeed` / `fail`是编译器与runtime之间的typed内部ABI，不是源码FFI API。descriptor决定unit identity和对应root/storage，调用者不能提交任意裸地址；非法transition、descriptor/image不匹配或未持有winner资格属于4.4的fatal runtime ABI error。

---

## 5. 异常

### 5.1 Scoop exception record 与抛出

- M25起异常runtime只建立在Itanium Level I unwind接口上，不使用C++ ABI。runtime私有的`ScoopExceptionRecord`包含恰好一个满足目标对齐要求的`_Unwind_Exception`、catch/rethrow/lifetime元数据，以及按对象TypeDescriptor大小和对齐保存的Scoop对象payload；各部分的具体offset不属于生成代码ABI，raw unwind pointer与payload之间只能经runtime入口转换。
- 每条新异常记录使用Scoop专属且稳定的`exception_class = 0x53434f4f50000000`（`"SCOOP\0\0\0"`）。`_Unwind_Exception.exception_cleanup`负责在记录最终删除时先撤销payload的stable external object root，再释放整条记录；unwinder私有字段只由unwind library读写，runtime和personality不得挪作catch状态。
- 抛出入口`scoop_rt_throw(obj)`要求对象头的TypeDescriptor shape为`FixedObject`，从其`minimum_size/instance_alignment`读取精确allocation size/alignment，分配record并把完整对象按值复制到payload，随后在任何可能展开或触发GC的动作前登记该payload为stable external object root，再调用`_Unwind_RaiseException`。`Throwable`层次若出现其他shape是fatal type/runtime invariant error。catch取得的payload copy是本次异常对象本尊，原managed对象无需pin；绝不能把原managed对象或普通GC heap地址直接解释成`_Unwind_Exception`抛出。
- `_Unwind_RaiseException`只会在没有handler或unwind错误时返回。runtime必须在仍可读取payload时打印稳定的`uncaught exception: <type name>`或unwind-failure诊断，删除异常记录并终止进程；不注册或调用C++ `std::terminate`。

### 5.2 Personality 与 landing pad

- 栈展开使用LLVM `invoke` / `landingpad` / `resume`和runtime自有`scoop_eh_personality`。M25的personality直接消费LLVM 22.1为当前Itanium target生成的LSDA，只接受编译器封闭输出的两类action：Scoop catch-all与cleanup；Scoop源码catch类型继续由landing pad之后的普通`scoop_rt_is_instance`分派完成，LSDA不携带C++ RTTI或Scoop TypeDescriptor类型表。
- search phase只允许Scoop `exception_class`的catch-all action成为handler；cleanup-only action不能提前终止search。cleanup phase在匹配call-site range时按ABI设置exception/selector数据寄存器与landing-pad IP：中间cleanup进入cleanup pad，`_UA_HANDLER_FRAME`进入已选中的catch pad。`resume`保持LLVM生成的`_Unwind_Resume`，不能用它实现源码重抛。
- personality必须验证version、action flag、LSDA header、pointer encoding、call-site范围和action链；当前target profile没有声明的encoding、typed catch/filter、损坏或越界表项均为fatal unwind错误，不能退回`__gxx_personality_v0`、`__gcc_personality_v0`或把任意非零action当作合法Scoop handler。
- 非Scoop exception不得被Scoop catch解释为managed对象。它进入生成Scoop EH区域属于不受支持的foreign unwind边界，runtime必须终止而不是交给`BeginCatch`；本条不改变5.5的FFI边界限制。

### 5.3 Catch 状态、结束与重抛

- landing pad先捕获opaque exception record/raw pointer，再由普通dispatch块调`scoop_rt_begin_catch(raw)`验证`exception_class`、把对应record压入当前`ScoopThreadState`的caught-exception栈并返回其payload managed ref。该ref指向heap外稳定对象，但其对象头、TypeDescriptor和出站引用遵守普通Scoop对象与3.3 stable external root契约。
- 每条正常离开handler的路径必须调一次`scoop_rt_end_catch()`。入口只操作当前线程栈顶并弹栈：普通caught record立即调`_Unwind_DeleteException`，标记为rethrow的record则恢复为in-flight而不删除；同一active record不得重复`BeginCatch`。cleanup callback是撤销root和释放record的唯一最终出口；runtime以active registry/state保证只调用一次Delete，callback在解引用record前也按raw地址检查active membership并拒绝重复回调。释放后的raw header不再是private ABI或Level-I API的有效输入。begin/end/rethrow都不得分配managed对象、触发GC或把异常记录地址暴露给Scoop源码。
- `scoop_rt_rethrow()`只在存在active catch时合法。它在当前record上标记rethrow并对同一个`_Unwind_Exception`重新调用`_Unwind_RaiseException`；随后原handler的cleanup chain调用`EndCatch`时只弹出catch状态而不得删除record，外层`BeginCatch`重新接管同一payload identity。handler内抛出另一异常时，cleanup chain先结束旧catch，再以LLVM `resume`传播新record。
- Scoop不提供`exception_ptr`、跨线程exception record共享或C++式公开引用计数；语言可跨线程/挂起保存的是managed `Throwable`，不是native unwind record。

### 5.4 跨控制边界的异常物化

- M10 的suspend handler不允许把`scoop_rt_begin_catch`建立的native catch状态跨挂起点保存。选中catch或进入可能挂起的finally前，生成代码调用`scoop_rt_materialize_exception(caught)`：按caught payload的TypeDescriptor分配managed对象，保留新对象已初始化的`td` / `gc_word`，只复制对象头之后的payload。复制期间exception payload仍登记为stable external root；复制完成后立即`scoop_rt_end_catch()`，catch local / pending exception改指向managed副本。
- 恢复失败或物化后的继续传播从该managed对象重新调用`scoop_rt_throw`，因而创建新的native record。Scoop的throw-by-value语义不承诺两个native record相同；同一次未物化rethrow则按5.3保持原record/payload identity。
- M21 initializer failure也必须在离开winner线程的catch、写入共享Failed状态之前调用同一物化入口；coordinator只保存已登记为global root的managed `Throwable`，绝不保存`_Unwind_Exception`、catch payload地址或active catch状态。物化、`scoop_rt_init_fail`与重新抛出的顺序遵守4.6。

### 5.5 依赖与边界

- 生产runtime与生成对象的异常依赖只允许Itanium Level I `_Unwind_*`接口；不得导入`__cxa_*`、`__gxx_personality_v0`、`__gcc_personality_v0`或C++ terminate符号。每个target profile必须显式选择兼容的unwind provider并以产物级符号检查验收；Darwin/AArch64由系统`libSystem`重导出libunwind接口，不添加`-lc++abi`，也不要求当前SDK不提供的独立`-lunwind`链接名。
- 内置异常的抛出点：除零（`ArithmeticException`）、`as`失败（`ClassCastException`）、`!!`失败（`UnwrapException`）、数组越界等（spec 10.5、11.7）由generated managed CFG构造异常并调用runtime-only no-return throw入口；不能藏进可能把异常展开出native frame的Scoop ABI FFI helper。range的非正step由普通Scoop core body抛`IllegalArgumentException`，不需要runtime专用入口。
- **边界规则**：异常不得穿越C ABI frame（行为未定义）；能否穿越Scoop ABI FFI frame取决于实现（FFI函数无landing pad，穿越意味着跳过外部语言代码——初版禁止，行为定为终止进程）。M25只替换Scoop进程内的异常runtime，不扩大FFI可展开边界。

## 6. 核心类型的运行时后备

以 Scoop ABI FFI 函数形式实现；spec 14.4 的 `write(String)` 是不跨 safepoint直接借用 ref的最小范例，涉及分配的函数则按 4.2 登记 native roots：

- `String`：创建、拼接、内容比较、内容hash、长度、索引/切片；
- `Array` / `MutableArray`：按spec 10.1的元素布局分配、读取`size`，以及`toArray` / `toMutableArray`的浅拷贝转换（spec 10.4）。源码可见bounds check、`IndexOutOfBoundsException`构造与throw都在generated managed CFG中完成，native helper不抛异常；若仅供受检代码使用的helper收到越界index则是fatal compiler/runtime invariant error。转换入口显式接收编译器已选定的目标concrete application TypeDescriptor，以该descriptor分配并保留新对象头，先验证source exact array TD/side metadata与logical size，再复制该size；`Inline`非ZST分支复制目标data offset之后的inline element payload，padding保持分配时的canonical zero；`ZeroSized`分支不调用payload `memcpy`。不得从来源对象、元素布局或类型名推断目标类型，也不得沿用来源descriptor；
- `StringBuilder`：`add` / `build`（spec 11.6）；
- 八种定宽integer的具体`ToString`/`Hash`后备，以及其他基本类型所需的`ToString`/`Hash`/operator equals后备（不提供`Any`或地址fallback）。`Hash.hash()`与所有integer `compareTo`的源码结果都是`Long`（i64），不随operand宽度改成`Int`；窄signed/unsigned值可由core按规则sign/zero extend后复用64位后备。integer equals由typed intrinsic直接生成比较，既有`scoop_rt_int_equals`/`scoop_rt_uint_equals`在M22迁移调用点后退出公开runtime契约。alias在进入runtime前已经展开，runtime不按`Byte`/`Int64`等alias名称分派；
- `Iterator`/`Iterable`、四个独立nominal type `IntRange`/`UIntRange`/`LongRange`/`ULongRange`、Array iterator、range终止与非法step全部由普通Scoop core与生成代码实现；`LongRange`/`ULongRange`不再是前两者的alias，M22不新增range/iteration runtime ABI；
- 类型测试与装箱辅助：`is` / `as` 的exact TypeDescriptor比较、普通装箱/拆箱。

## 7. 启动、线程与终止

主线程attach仅建立native-safe状态；每次eager gateway及最后root gateway分别按2.8建立boundary、完成epoch enter握手、执行mandatory gateway入口poll，并在GC-free status返回后完成leave与boundary回收。两次调用之间的C coordinator没有活动managed段，不能跨C frame扫描或展开。

- 进程启动严格分阶段：先建立纯native runtime/thread/callback基础状态；再执行envelope phase，只验证唯一program以及image/entry/core/六类record的固定prefix、exact size、pointer/count算术、alignment与segment权限；随后遍历全部producer table建立尚未公开的kind-specific semantic-id/address、callable body/entry、object address及dependency DAG provisional index。semantic phase在该index上解析全部连续stackmap v3 blob，并验证TypeDescriptor/scan/layout、ODR、range、init cell与专用failure root、initializer/ensure/gateway、root entry及core String binding；只有这些cross-record relation全都成立后，才从已类型化record key重算每个RuntimeImage fingerprint，再验证canonical dependency-first order并重算Graph fingerprint。全部成功才一次性commit/freeze registry；失败丢弃整个provisional state，不能留下半登记副作用。之后才初始化GC heap/handle状态、attach主线程，按spec 9.1.3及第12章的Cone依赖/typed unit id顺序通过各unit的no-throw startup gateway运行eager initializer，最后通过program descriptor的no-throw root gateway调用typed `main`。commit前不得分配managed对象、运行initializer/callback或构造Scoop异常，也不得为了提前hash而解引用尚未通过semantic phase的nested pointer；gateway返回Failed时从已登记failure root输出普通未捕获异常并终止，dependent initializer与`main`不执行，任何Scoop exception都不得穿越C startup frame；
- object/companion不因metadata登记或其const读取而初始化；第一次非const访问通过2.7 gate同步ensure。initializer entry是编译器生成的ordinary managed callable，可以分配、触发GC或抛异常但不能挂起；runtime只协调状态、wait与发布，不按名称反射调用property/accessor，也不补initializer body；
- 线程：主线程启动时注册；runtime创建的线程及 foreign thread在首次进入 managed代码前 attach，在退出最后一个 managed入口且不再持有 runtime thread state时 detach（配合 3.5、4.3）；
- 终止：`main` 返回后runtime先进入`ShuttingDown`并拒绝新attach/registration；只有主线程以外无attachment、无活动callback且token ownership均已释放时才销毁GC状态。存在迟到线程/token时报告计数并终止，不在其仍可能进入时释放runtime。shutdown不运行GC finalizer，也不为未来release hook遍历全部live/uncollected对象；native resource仍须在正常控制流中显式释放。

## 8. 协程

`suspend` 的状态机变换、`CoroutineStep<T>`、frame 与各挂起点的 `Continuation<T>` adapter 全部由编译器生成（spec 8.2、11.9；impl spec 2.3）。这些实体都是普通 managed 对象/值：

- frame 与 continuation adapter 必须有普通 TypeDescriptor 和完备的递归引用扫描描述；frame 链由 GC 自然保活，不登记额外的 runtime root；
- continuation 的完成状态与 frame 的当前恢复状态存于 managed 对象字段。M10–M12 的最小实现是单线程协议；M13 把adapter claim/完成与frame `running/suspended/completed`转换升级为64位对齐原子状态机：winner以acq_rel CAS取得完成/驱动权，先写payload再release发布终态，读取方以acquire消费；等待短暂`Completing`状态的循环必须包含safepoint/backoff。该状态机的64位字段是compiler/runtime共享的独立typed atomic state carrier，不是源码`Long`属性，不得经普通integer operation读写。已attach线程可安全恢复既有continuation，重复完成仍抛`IllegalStateException`。调度器、队列和恢复后在哪个线程继续执行仍由后续标准库规定；
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

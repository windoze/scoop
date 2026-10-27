# Scoop 运行时规范

本文规定 Scoop 对象表示、GC、异常、线程、初始化与 native 调用的运行时契约。语言行为见 [语言规范](SCOOP-SPEC.md)，编译阶段、产物与链接规则见 [编译器与产物规范](SCOOP-IMPL-SPEC.md)。

## 1. 职责与目标

运行时提供 managed 对象分配、精确移动 GC、类型检查与动态分派所需的数据、异常展开、一次性初始化、Task-local Context 和 FFI 支持。运行时不得重新进行源码名称解析、重载选择或泛型实例化。构建缓存与产物更新属于实现规范 2.6～2.8；移除缓存内容复核不增加运行时补充验证或新 ABI。实际 image 注册、GC 根更新、对象边界和 native 调用契约保持。

函数条件与成员调用要求属于编译期合同（语言规范3.2.2～3.2.4、实现规范2.11、2.13），具体化后只产生已有的普通调用、构造与 singleton 访问。不增加 runtime 约束检查、dictionary、TypeDescriptor 字段或按调用点作用域分派的 ABI。

语言9.1.8的条件interface实现同样在具体HIR前落实。每个exact type的普通接口表仅包含适用关系，接口转换、is/as、装箱和分派共同使用该结果；runtime不执行guard或选择实现块。字段表示、GC扫描、基类前缀及companion的原singleton状态和ensure协议保持，同一完整宿主跨Cone共享原对象。关系查询不执行初始化，不增加TD字段、额外codec参数或新的运行期分派ABI。

M42 的平台无关标准基础库按 [标准基础库规范](SCOOP-STDLIB-SPEC.md) 完全使用普通 Scoop Cone 实现，只通过公开 core API 使用既有运行时能力。集合、排序、文本和字节处理、数值文本转换、RNG、日期／时间运算均不增加 runtime helper、native 后备或专用 ABI；Clock／TimeSource 只有普通库接口，没有系统取时实现。库代码触发的实际 GC、异常或 ABI 缺陷必须先在对应底层修复并建立独立回归，不能将缺陷处理转移成库专用绕行。

受支持目标使用 little-endian、64-bit data/code pointer 和 natural 8-byte metadata alignment。所有大小、数量、对齐和地址范围计算必须检查溢出及实际存储边界；不满足目标表示或 ABI 的输入必须拒绝。

## 2. 对象模型

### 2.1 Managed 对象

managed reference 是对象起点的直接指针；对象可能移动。对象头固定为 16 bytes，依次包含 TypeDescriptor 指针与 GC 状态字，各占 8 bytes。null 仅用于允许空状态的内部存储，不能作为普通非空 Scoop 引用。

普通 interface value 是 `{ object: managed pointer, itab: metadata pointer }`，size 16、alignment 8，offset 分别为 0、8；itab 指向对象实际类型对静态 exact interface 的完整 slots 视图。TD 目录的 `ScoopItableEntryV1.slots` 保存一个规范片段；从其他菱形路径投影得到的等价片段可以具有不同地址，但具有相同槽序、最终实现和调用 ABI，不能以 itab 地址判断接口值身份。class、Any、String、Array 和普通函数值仍为单字引用。空接口的合法 slot table 可以为 null，引用有效性只由 object 决定。身份比较、pin 和 handle 只使用 object；值转接口仍按既有语义装箱。

转换在建立接口视图时取得目标 exact interface 的表。每个具体实现类型的子接口表完整内嵌同一类型的父接口表；单继承为完整前缀，多继承按声明序具有固定偏移，菱形保留父表完整片段。Child → 静态已知 Parent 直接复用或调整 itab，不扫描对象 TD 目录；TD 的父接口目录项复用内嵌片段。其物理槽及只读表符号遵循实现规范 2.9、所有构建 profile 使用同一 ABI。已知实际类型时可直接引用静态表；相同 exact interface 的复制保留两个分量。接口调用直接读取已有 itab 的 slot，隐藏 receiver 只传 object，其余参数／结果遵守完整 Scoop ABI。default、boxed value 和变型入口按实际 typed adapter 适配；普通 class 实现不为相同签名额外增加转发调用。

`scoop_rt_itable_find(object_td, interface_td)` 是 NoGC、无异常的查询入口，两个参数均为有效的静态 metadata 指针，目标必须是 exact interface。它查询实际对象 TD 的完整接口目录，返回匹配的 `const ScoopItableEntryV1 *`；缺失返回 null。匹配空接口时返回非 null entry，entry 的 slots 仍可为 null。返回指针只引用不可变 metadata，不引用对象，不成为 GC root，可在对象移动后继续用于同一对象值。编译器可用 entry 是否为 null 完成接口 `is`／`as` 的检查，并在成功路径加载该 entry 的 slots，避免再查目录；源码 Boolean 及失败／异常语义不变。其他类型关系继续使用 `scoop_rt_is_instance`，保证存在表的未检查转换继续可用 `scoop_rt_itable_lookup`。

动态查询按 TD 指针身份逐项比较目录项，不以 diagnostic name、slots 地址或不完整的泛型类型代替。读取任何项前必须保证其下标小于 `itable_count`；空目录不解引用 itables，短目录也不能读取目录外的项。find、lookup 和完整类型关系中的接口查询共用此规则。查询不重排静态目录，也不保存 runtime 缓存；所有 profile 使用同一目录与调用 ABI。编译器可以按实现规范 2.19 对已证明稳定的局部对象值惰性保存查询结果，包含 null 未命中；这仅保存当前函数调用的不可变 metadata，不保存 managed 对象、改变首次查询时机或新增 GC root。

GC 状态字的 pin bit 为 `UINT64_C(2)`（bit 1），release-ready bit 为 `UINT64_C(4)`（bit 2）；操作这些位时必须保留其他位。

class 的基类对象布局是派生对象的完整前缀，包括基类尾部 padding；派生字段不得复用该 padding。构造最派生对象时只分配一次，整个构造期间对象头都指向最派生 exact type 的 TypeDescriptor，不随基类构造改变。

对象在可被 GC 观察前必须具有有效对象头、精确分配大小与已初始化的 payload。语义上 payload 从全零开始；3.1 允许消除被确定初始化覆盖且覆盖前不可观察的无引用尾部零写入。对象头、引用槽、padding 和未证明覆盖的字节仍按原规则清零。含引用的字段在初始化完成前保持 null 或合法 managed reference。构造期间的 receiver 是普通精确 root；未完成的实例不能发布为可用对象。

带 release hook 的对象初始 `RELEASE_READY` 为 0。完整的最外层构造成功后、对象发布前，以 release 语义置为 1；构造失败时保持 0。其回收规则见 3.8。

### 2.2 TypeDescriptor 与扫描描述

最终程序中每个实际保留的 materialized exact type 具有唯一 TypeDescriptor 及完整类型登记；同一 ODR 类型在最终程序中具有同一地址。不可达的类型及其元数据可按实现规范 2.19 一起删除，但保留 TD 的父类型、接口、相关类型、布局和扫描引用仍须闭合。不同声明或不同泛型 application 不因名称、表示或布局相同而共享类型身份。

TypeDescriptor 按以下顺序保存字段：

```text
type_id: u64
instance_shape: ScoopTypeInstanceShapeV1
object_scan: scan pointer
parent: TypeDescriptor pointer
vtable: function pointer table
itables: ScoopItableEntryV1 pointer
itable_count: u64
diagnostic_name: byte span
relation_kind: u32
related_type_count: u32
function_result: TypeDescriptor pointer
release_hook: function pointer
related_types: TypeDescriptor pointer[related_type_count]
```

固定部分为 152 bytes，随后为 `8 * related_type_count` bytes 的尾表。TypeDescriptor 不带 2.8 的 descriptor prefix。`type_id` 与 persistent exact type identity 在整个程序中一一对应，固定宽度碰撞是错误。

语言规范 9.6 的 TypeInfo、成员及注解描述是普通 core struct/class/Array/String 和闭包，不添加 TypeDescriptor 字段、全局名称索引或 runtime 查询 ABI。描述内的 managed 引用按普通 rooting/relocation 规则处理；语义相等不依赖描述对象或 TypeDescriptor 的地址。

语言规范 11.17 的值视图、读取闭包和 debug 路径同样使用普通 managed 引用。循环检测保留原对象引用，GC 搬迁后以更新的引用执行既有 identity 比较；不缓存裸地址或把临时值装箱作为被观察对象的 identity。访问检查在普通函数定义处完成，runtime 不提供反射专用读写入口或动态权限检查。

`diagnostic_name` 是由完整 exact type 确定的非空、稳定 UTF-8 名称；alias、re-export 或使用处的拼写不改变其 bytes。同一 exact type 的名称逐 byte 相同，存储只读且与 TD 同生命周期。该名称只用于诊断，不参与类型判断、用户字符串化、相等或哈希。

`ScoopTypeInstanceShapeV1` 依次为两个 `u32` 字段 `instance_kind, inline_storage_kind`，六个 `u64` 字段 `minimum_size, instance_alignment, inline_offset, inline_size, inline_stride, inline_alignment`，以及 `inline_scan` 指针。即 shape 共 64 bytes：2 个 `u32`、6 个 `u64`、1 个指针。

| instance kind | tag | 大小与内联表示 |
| --- | --- | --- |
| FixedObject | 1 | `minimum_size` 为完整对象分配大小，alignment 至少 8；inline 字段为 0，scan 为 null。 |
| BoxedValue | 2 | `inline_offset = alignUp(16, inline_alignment)`；对象 alignment 为 `max(8, inline_alignment)`；minimum 为 payload 末端按对象 alignment 向上对齐；stride 为 0。 |
| InlineBytes | 3 | minimum 与 offset 为 24，对象 alignment 为 8，inline size／stride／alignment 均为 1。 |
| InlineArray | 4 | minimum 与 offset 为 `alignUp(24, inline_alignment)`，对象 alignment 为 `max(8, inline_alignment)`；非 ZST 元素的 size 与 stride 相等且非零；ZST 二者为 0。 |
| AbstractRef | 5 | 大小、对齐、inline 字段均为 0，scan 为 null，不可分配。 |

inline storage tag 为 `None=0, Inline=1, ZeroSized=2`。FixedObject 与 AbstractRef 使用 None，InlineBytes 使用 Inline；BoxedValue 与 InlineArray 按 payload／元素是否为 ZST 使用 ZeroSized 或 Inline。`minimum_size == 0` 只用于 AbstractRef，不表示零尺寸 box 或数组对象。所有实际 alignment 均为正的 2 次幂，大小和 offset 必须满足目标 ABI。

target profile 的 `maximum_managed_alignment` 与 `maximum_managed_object_size` 均非零；布局超过前者时拒绝，分配超过后者时失败，不得截断或降低对齐。

`object_scan` 的 offset 相对对象起点，`inline_scan` 相对 value payload 或数组元素起点。BoxedValue 的 object scan 等于 inline scan 按 payload offset 平移；含引用数组的 scan 使用 length offset 16，first-element offset 与 shape 相同。GC-free 或 ZST payload／元素的这两种 scan 均为 null。Ptr、FunPtr 和整数不是 managed root。

tagged enum 的 managed 引用槽由各 variant 独占，inactive variant 的引用槽必须为 null；扫描无需读取 enum tag。scan 只能访问实际表示内对齐的 managed pointer 槽。

接口的 value scan 只包含 offset 0；itab 不标记、不搬迁。struct、enum、数组、closure、Context、native region 和 coroutine frame 内的接口同样递归扫描这一 leaf，移动后保留原 itab。

语言值的冗余局部复制可以在根计划之前消除；同一未变更值不因经过多个局部名称而获得额外保活要求。复制仍具有快照语义，源重赋、取址、控制流合流和异常出口须按真实使用保留所需旧值。物理 roots 根据变换后的值流建立，不能仅删除存储而留下指向已删除副本的根来源。

未取址的接口和接口 niche 值可按实现规范 2.5 以独立 object／itab 分量参与 SSA 优化；真实取址、根发布与 ABI 所需存储继续保留。跨 safepoint 的实际使用必须消费 relocate 后的 object，条件路径和循环按当前分量合流，metadata 保持。普通优化结束后的 aggregate 分解保证返回、存储和继续传参不会复用移动前的旧 aggregate；SSA 分解自身不构成额外 GC root，也不能替代精确保活及必要的发布后重读。

扫描程序使用 `u64` word：

| 种类 | word 表示 |
| --- | --- |
| Empty | TypeDescriptor 中为 null；需要 addressable 空程序的静态存储使用 `[0]`。 |
| References | `[count, offsets...]`，`0 < count < UINT64_MAX - 1`，offset 严格递增。 |
| Sequence | `[UINT64_MAX - 1, child_count, child_pointer...]`，child count 至少 2。 |
| Array | `[UINT64_MAX, length_offset, first_element_offset, stride, element_scan_pointer]`，stride 非零，element scan 非空。 |

canonical scan 中 References 去重排序；Sequence 展平、合并 References，按 `(child fingerprint, child canonical bytes)` 排序去重，空或单元素序列折叠；Array 的 child 非空。canonical bytes 的整数均为 little-endian，编码为：

```text
None       = u32(0)
References = u32(1) || u64(count) || each u64(offset)
Sequence   = u32(2) || u64(child_count) || each canonical child
Array      = u32(3) || u64(length_offset) || u64(first_element_offset)
             || u64(stride) || canonical element
```

scan fingerprint 为 `SHA-256(ByteSpan("scoop-scan-v1") || canonical_scan_bytes)`，ByteSpan 为 `u64 length + bytes`。共享 child 的物理地址不改变其语义编码。

编译器完成 scan 的静态格式、类型布局、对齐和引用规则。runtime 登记实际 scan 的可读地址范围和 child 引用，直接使用已生成的布局，不重新比较 inline/object scan、重算类型形状或另设完整 metadata 复核模式。动态 Array 的 length 与乘加、实际分配及对象访问继续遵守相应范围。

数组元素 scan 为固定 References 时，适配层直接按元素步长和引用 offset 枚举槽，复用原描述符，不逐元素重新分派递归解释器。整对象、并行数组任务和脏卡扫描共享这一规则；半开地址范围只访问槽起点位于范围内的 managed leaf，首尾不完整元素仍须裁剪，不能因元素与范围相交而读取其余槽。保持元素顺序和元素内 offset 顺序，空数组不访问元素；Sequence 或递归 Array child 保留递归解释。此优化适用于所有 profile，不改变 scan 编码、引用集合、itab／tag／padding、根发布、写屏障或搬迁后原位回写契约，不建立第二份扫描布局。

`parent`、itable 和相关类型引用必须解析到实际登记的 TypeDescriptor。空 vtable 为 null；空接口仍保留以其真实 TD 为键的 itable entry，slot table 可以为 null。slot 与 callable ABI 由产物合同确定。

装载后 TD 与 shape 不再变化。generated-code 的 typed TD 引用、已发布对象头中的 TD 和 runtime 内部实际类型引用直接复用登记结果，尺寸计算、装箱及 collector 扫描不反复查询地址成员表。公开 native 分配入口在消费外来 TD 时检查登记一次，再进入同一内部尺寸与分配实现；动态 requested size／count、对齐、溢出、abstract 类型和对象实际边界检查不因此省略。不新增验证凭证或第二套 shape 实现。

`relation_kind` 为 0 时 related count 为 0、function result 为 null；1 和 2 分别表示普通与 suspend 函数类型，related types 按参数顺序保存、result 保存结果；该关系中 null TD 表示 Any。3 表示 interface，related types 按 exact identity 排序保存直接父接口，result 为 null。4 表示源码 intrinsic Nothing 的底类型关系，related count 为 0、result 为 null，实例形状必须为 AbstractRef；它是所有类型的子类型，其他类型不能成为它的子类型。其他 tag 非法。

`Any` 与 `Nothing` 的类型身份来自 core 的实际源码声明（语言规范 11.1）。二者的 TD 使用不可分配的 AbstractRef 形状；`Nothing` 不存在对象、box 或合法的非空值。底类型关系在函数参数逆变／结果协变检查中必须保留，不能把 `Nothing` 与 `Any` 或普通空 class 合并。Any 的值仍指向实际动态类型对象，移动 GC、原类型 TD 和装箱协议不变。

普通结果为 `Nothing` 的调用没有正常继续路径，但仍可能抛出异常或进入 GC；异常边及其根、finally 和 Context 清理照常保留。其逻辑结果保留实际底类型身份，物理引用结果即使保留在调用签名中也不能被作为一个成功结果使用。挂起函数的 `CoroutineStep<Nothing>` 可以返回 Suspended，不得把源码底类型误用为整个挂起 ABI 的 `noreturn` 属性。

### 2.3 值类型与装箱

struct、enum、tuple、数值、Ptr、FunPtr 依其静态类型按值存储。提升到 Any 或 interface 时，值存入带原 exact TypeDescriptor 的 managed box；值类型 receiver 使用独立的方法局部副本，不能取得调用方 place 或 box payload 的别名。

runtime 动态拆箱后备对源对象的检查在同一 heap lock 区间内只定位一次：exact size 查询同时确认对象起点，随后复用所得尺寸检查实际 TD 和动态 shape。读取对象头前必须完成对象边界检查；定位结果不得跨 safepoint 保留，类型匹配、整数枚举父类型关系及动态 length／实际分配尺寸检查不变。

整型枚举（语言规范 4.2.1）的 inline size、alignment 和位表示等于底层整数，无额外 tag。直接装箱保留 enum exact identity，其 boxed TD 以对应整数的 boxed TD 为父类型；继承的接口使用整数成员的原签名。按父整数解箱允许这一已声明父子关系，使用同一 scalar payload 表示；反向解箱仍要求实际 enum 身份，不能检查数值后伪造 enum。先上转再装箱的值只有整数身份。未使用整数值不增加 Option niche。

`MaybeUninit<T>` 是与 T 等 size／alignment 的独立值类型，复用 T 的 inline scan。全零存储是合法 wrapper，managed 槽始终为 null 或有效引用；这不表示已经构造合法 T。零构造包括 padding，不使用 undef/poison，不清零对象头或 TD；包装、复制和重置使用普通值存储及写屏障。wrapper 不继承 T 的非空或 tag 不变量。

语言规范 9.3.6 的 Add 等普通运算符接口复用实际 conformance 的 TD/itable 和现有接口调用适配；扩展运算符不增加接口表项。具体值的已选 intrinsic 或直接调用不因另有接口实现而强制装箱，不增加运算符专用 runtime ABI。

语言规范 11.11 的显式 `Equals<T>.equals` 实现使用普通 interface TD、itable 与 callable。表项只来自实际声明或继承的 conformance；结构 operator equals 不增加接口。NoGc 值方法实现 Managed interface slot 时，itable 使用符合 slot ABI 的 Managed value/interface adapter，具体值的直接 operator 比较仍调用原实现；runtime 不按类型名、对象地址或内存字节另行判断相等。

`Unit` 和其他 ZST 的 payload size 可以为 0，类型身份与 box 对象身份仍存在，每次实际装箱产生新对象。所有 profile 的 typed BoxValue 消费已选 box 的完整实例布局，通过既有分配路径分配，并复制 payload（包含 padding）；大小不再决定是否进入专用 box C 后备。release 按原 TLAB 尺寸与布局条件完成快路径，debug 和大对象遵守分配器既有的 TLAB 完成／慢路径策略；不在 runtime 重推已知 payload shape。较大 payload 可复用共享 NoGC 字节复制，避免为每个调用点展开完整复制；调用仅消费分配后有效的源和目的地址，不增加 native transition 或 safepoint。

含引用的源 place 由既有 LIR statepoint live set 保活并移动回写，分配后重新取得源数据；不建立仅供旧 box C 入口检查的 RecursiveRegion frame。复制后按完整值扫描布局执行必要写屏障；只有实际 TLAB 成功且复制前没有 safepoint 时可按 3.6 省略，慢路径结果不能假定年轻。对象头、对齐、清零、失败、TD 和 interface 语义保持，ZST 不构造 payload 存储。`scoop_rt_box_zst`／`scoop_rt_box_value` 及其 anchor／实现退役。拆箱生成 exact TD 比较和复制，真实动态父整数关系与错误路径仍使用 `scoop_rt_unbox_zst`／`scoop_rt_unbox_value`，后者要求目的地满足完整 payload 的大小与对齐。

Array clone 在所有 profile 使用实际源／目标布局生成计数读取、分配、搬迁后源重读、完整 stride 复制及必要屏障，保留 ZST 的计数和独立对象身份。无生产消费者的 `scoop_rt_array_clone`、专属动态 shape 检查及 anchor 退役；普通 extern 不因符号同名被替换。

生成代码的 typed unbox 可以直接读取有效、非 null managed 对象的 TD；与预期 boxed TD 相等时，按已确定的 payload offset／大小／对齐将完整字节（包含 padding）复制到独立 typed local，ZST 只完成类型检查而不读取 payload。该路径不分配或停顿，含引用值仍由后续普通根计划保活和恢复，不查询 heap size 或重放已确定的布局校验。TD 不相等时继续调用原 unbox leaf，保留整型 enum 到父整数的兼容解箱及原错误行为。C 入口仍履行自身合同；不能将原本需要的源码 null／cast 检查替换为无条件 payload load。

### 2.4 String 与数组

String 持有合法 UTF-8，允许 U+0000，无隐含结尾 NUL。byte length 位于 offset 16，bytes 从 offset 24 开始；字节数不超过 `INT64_MAX`。分配大小为 `alignUp(24 + byte_length, 8)`。语言规范 11.4 的 charLength、getCharAt 及字符切片边界由普通 core 正文扫描 UTF-8 前导字节确定，仍为 O(n)，不新增字段或缓存；byteLength 仍直接读取既有字节数。定位前缀不解码不需要的字符；getCharAt 仅在命中位置调用既有有界标量解码。负索引、倒置／越界范围、末尾空切片与原异常保持。旧 length／get／slice_bounds C 入口及其专用聚合结果 shim 退役。

String 的长度在构造时写入，发布后不变。普通 core 扫描可在已建立的范围内使用内部 unsafe 字节读写操作；该操作直接消费 String 引用与字节索引，不保留内部地址。读要求有效字节范围，写另外要求对象由当前构造过程独占、未向用户发布；写入不能改变长度和对象头，发布前必须完成合法 UTF-8。跨 safepoint 的实际字节操作使用重定位后的对象。原受检 byte-at 保留越界 fatal 路径，不把普通 extern 改为内建读取。

fromChars 的长度累计、UTF-8 编码以及严格／lossy 校验使用普通 core 正文。List 输入仍先按语言规范取得一次元素快照；严格校验成功后复制一次，失败返回首个非法子序列的起始偏移。lossy 按 maximal subpart 规则两遍确定精确结果长度并写入，fromChars 同样先计算长度、再编码一次，不分配临时托管输出数组。内部 typed String 零字节分配复用现有分配路径，构造期间的 String 字节 payload 不含引用，GC 只依赖已完成的对象头和长度；跨分配／poll 保活输入与结果，写入本身不需要引用屏障。编码和扫描不在 codegen 重复实现。

语言规范 11.4 的三个有界表示 intrinsic 可在活动 Managed 状态直接读取，或调用内部 NoGC runtime leaf：`scoop_rt_string_byte_length_leaf(String*) -> i64`、`scoop_rt_string_byte_at_leaf(String*, i64) -> i8`、`scoop_rt_string_character_at_byte_leaf(String*, i64) -> u32`。leaf 不分配、park、转入 native、回调或保留传入地址，返回前最多读取一个 UTF-8 标量；调用期间对象不会移动。byte-at 和 cursor 解码保留索引检查；有效 String／标量起点是内部解码前置条件。内部 leaf 直接实现必要的有界读取和错误出口；无生产消费者的旧 native 字节读入口退役。普通显式 extern 声明仍遵守自身调用模式，不得按符号拼写把它改成 intrinsic。完整扫描不因这些有界入口而改变线程协调合同。

短常量比较只在长度保护下读取完整有效范围，保持 UTF-8 无符号字节字典序。String hash 全部使用普通 core 正文：既有短输入分支与共享长输入循环使用相同 FNV-1a 初值、逐字节异或和 64-bit 回绕乘法；不得改变既有 hash 结果。旧 C hash 入口退役。上述操作不分配 String 或保留内部地址。

短 hash 的长度分支必须先于字节读取；超出短路径范围进入普通共享 Scoop 循环，只读取有效字节。该选择不在 String 中缓存 hash，不新增字段、根或固定对象；GC 后继续使用已恢复的 String 引用。扫描和长 hash 保持 Managed 合同，循环 poll 由现有管线生成；release 可按实现规范 2.19 将有界表示读取纳入既有 64 次分块，不将任意长度扫描或同名 extern 改为 NoGC／GCLeaf。

codegen 对普通 typed byte-length／byte-at 调用直接发射表示读取：从 AS1 对象的 offset 16 读取长度，byte-at 以 unsigned `index < length` 同时排除负索引及上界，再从 offset 24 的字节区读取 i8。有效路径不调用 C、不生成 safepoint，也不在中间 park；跨外围 GC 后必须使用已恢复的对象值。越界路径调用原 byte-at leaf 报告同一 fatal 错误，不产生可捕获异常。首批 invoke 形态及单字符 UTF-8 解码继续使用现有 leaf。对象布局保持，不给一般 NoGc 调用附加 readonly 等属性。

整数十进制转换由普通 core 正文使用专用、无 locale 的有界写入算法，一次格式化到最多 20 字节的栈缓冲区，返回实际长度；String 分配与复制通过 typed StringCopy 完成，不要求 NUL，不以再次格式化测量长度。负数先在无符号域计算幅度，最小有符号值同样遵守回绕语义。Char 编码及 Boolean／整数 equals、hash、toString 使用已有普通正文或标量 intrinsic；Boolean 文本选择不可变字符串常量。没有生产消费者的旧 `scoop_rt_bool_equals`、`scoop_rt_bool_hash`、`scoop_rt_long_hash`、`scoop_rt_ulong_hash`、Boolean／整数／Char 的 `scoop_rt_*_to_string`、`scoop_rt_string_eq` 及 long／boolean 专用 print／println 入口退役，不保留重复算法或仅供旧测试使用的适配层。String.equals 使用普通 core 正文和实际比较操作，输出经普通 ToString 与 write 实现；浮点 Ryu 核心保留。

稳定原生 UTF-8 区间的复制要求 pointer 非 null、length 非负、区间内容为合法 UTF-8，并在整个调用期间可读、地址及内容稳定；栈 scratch 满足该条件，managed 字节区须由既有借用协议固定。只复制实际长度，不验证已保证的 UTF-8，不保留输入地址，也不将输入当作 GC root；结果大小与失败仍按同一 String 分配合同。普通 extern 按其声明调用，不能凭与内建操作相同的符号拼写取得 intrinsic 语义。

严格和 lossy 字节转换都只能发布满足该不变量的 String。lossy 的每个 U+FFFD 占三个 UTF-8 字节（`EF BF BD`），计入结果 byte length；结果长度计算和最终分配遵守上述边界，不能直接沿用输入长度。

Array 与 MutableArray 的 logical size 位于 offset 16，元素区从 `alignUp(24, element_alignment)` 开始。分配大小为 `alignUp(element_offset + size * stride, object_alignment)`。size 在 `0..=INT64_MAX`，所有运算还须满足 `u64`、`size_t` 与实际分配边界。

ZST 数组不保存元素 payload，但保留 logical size、完整类型与独立对象身份；clone 和互转产生新对象。按长度初始化仍逐索引执行 initializer，保留副作用与异常。初始化期间数组和已写元素必须可被精确扫描，不得向用户暴露未初始化元素。

typed 数组 clone 的源／目标 element exact type 与 layout 已一致时，所有 profile 的生成代码按源对象的有效、不可变 count 计算相同布局的分配大小，复用普通 TLAB／分配慢路径，并以目标 nominal TD 构造独立对象。源数组属于该分配点的 live roots；分配后必须从恢复的根重新读取源对象，才形成 AS1 payload 地址。复制完整 stride 字节（包含 padding），含引用元素在发布前执行必要的范围写屏障；实际 TLAB 成功且复制区间无 safepoint 时按 3.6 省略，慢分配保留。ZST 只复制 logical count。普通源码循环的 poll 不因 clone 展开而删除。

ArrayList、StringBuilder 等普通库类型使用其声明的 class/value 表示，其公开行为由语言规范第 11 章规定。

`MutableArray<MaybeUninit<T>>` 复用 InlineArray shape，logical size 为 capacity，元素 size／stride 为 T 的 exact size，ZST 仍为零。所有容量槽都保存合法 wrapper，完整 GC 按 capacity 扫描；ArrayList 的 elementCount 不进入 GC 协议。删除与 clear 必须清零不再使用的槽，StringBuilder 复用同一 ArrayList 扩容。

### 2.5 Option 的 niche 表示

普通单字引用的零指针表示 None；interface 的 None 为 `{null, null}`，Some 为完整双字值，两者只按 object 分量区分。Ptr/FunPtr 的内部 data/code-pointer carrier 为全零时，也只表示对应 Option 的 None。裸 reference、Ptr 与 FunPtr 保持非零不变量，Some(payload) 不与 None 碰撞。GC 不追踪 null；`MaybeUninit<T>` 不继承 T 的 niche，其余 Option 表示遵守语言规范 7.4。

### 2.6 Closure 与函数类型

closure 是普通 managed 对象，捕获值在创建点按语言规定取得并存入不可变字段；含引用的捕获参与精确扫描。它的 TD 保存实际函数类型关系。

每个 closure 的 vtable 第 0 槽为动态 invoke，保持参数个数和挂起性，显式参数与结果以 Any 传递。该 managed 边界按 closure 的源码签名解包、调用 typed invoke 并装箱结果；suspend 使用 `Continuation<Any>` / `CoroutineStep<Any>`。静态调用使用原 typed 签名。

函数类型的 exact identity 保留源码 receiver 形状；函数关系中的参数按 receiver 在首位展平，函数类型转换按语言规范 8.1 的结构化关系判断，允许 receiver 与普通首参数双向适配。容器的 exact identity 不因这种函数值适配而合并。closure 的 TD、动态 invoke 和签名由自身 identity 决定，不随消费方的目标签名变化；适配结果与来源不保证引用身份相同。

closure 不能直接作为 FunPtr 交给 native。静态 FunPtr 与 4.3 的 managed callback 分别遵守各自边界。

### 2.7 Initialization unit 与 singleton 发布

每个需要运行期求值的顶层属性、singleton 和 delegated extension application 有独立初始化单元。其 GC-free cell 位于 managed heap 外，size 16、alignment 8，保存 `state: u64` 与 `owner_thread: pointer`。状态编码为 Uninitialized=0、Initializing=1、Initialized=2 或 Failed=3。state 在发布后只以原子方式访问；owner_thread、依赖栈和等待图仍由 coordinator 锁保护。C／C++ 共享布局保留 uint64_t 存储，runtime 使用原子内建访问，生成代码使用同宽、同对齐的 LLVM atomic 操作。

同一 singleton 的所有访问使用同一 ensure、published root 与 cell。generic companion 按完整宿主 application 区分状态；同一 application 跨 Cone 只初始化一次，不同 application 不因布局相同而合并。宿主构造、类型查询和 const 读取不隐式初始化 companion。

M55 的 companion bound、投影存在性与成员要求查询同样不触发初始化；正文或默认参数中实际求值的 `T.Companion` 在具体化后使用本节原有的 ensure、root 与 cell，不为不同要求或消费方创建另一份状态。

winner 完成全部初始化后，先发布 property storage 或 singleton root，再以 release 语义发布 Initialized。waiter 以 acquire 观察终态后重新读取 storage。Initializing 期间不能读取临时 singleton；构造中的 receiver 只由正常 managed root 保活。ZST storage 也保留 initializer、ensure、cell 和副作用。

普通 typed InitializationEnter 调用可以先对共享 cell 的 state 做 acquire load；只有 Initialized 可直接产生 Ready，无 runtime 调用、TLS 查询、锁或根发布。其余状态仍进入原 managed coordinator，保留真实慢边 safepoint、正常／异常根和等待协议；invoke 形式可继续使用完整后备。快速读取只发生在已激活的 managed 线程内，不替代 gateway／函数入口和循环 poll；不得在该检查前读取结果 storage，或跨 safepoint 保存其中的对象地址。Initialized 是不可逆终态，GC 仍可更新 published root。失败发布先写 failure root，再 release store Failed；锁内 state 读取和 Initializing 写入可用 relaxed，锁仍串行化所有 winner 选择及状态转换。

失败时不发布 singleton，不把部分 property storage 视为可读；将原 managed Throwable 存入单独登记的 failure root，再发布 Failed 并唤醒 waiter。后续访问重新抛出该失败，不重试、不回滚已发生副作用。

同线程重入和跨线程等待环产生带稳定 unit path 的 cycle 结果。generated ensure 通过已解析的 core thrower 抛出包含完整路径的 IllegalStateException；只有异常逃离 initializer 时，该 unit 才失败。无环等待必须兼容 3.5 的 GC 握手，不得持有未登记 managed pointer 睡眠。

unit 由 `PersistentInitializationUnitId` 标识；diagnostic path 只用于显示。初始化记录完整关联 cell、storage、failure root、initializer、ensure 及 eager startup gateway。lazy delegated extension 的这些共享实体也按同一 ODR application 合并。

### 2.8 多 image 登记与启动 ABI

runtime ABI contract 为 **21**，metadata ABI 为 **9**。contract 20／21 退役无生产消费者的标量、打印、专用装箱、Array clone 与 Context 生成操作 C 入口；所有 profile 复用相同 typed 布局与根计划。contract 19 增加 9.2 的线程 Context 根槽地址视图；contract 18 的初始化状态原子访问及 release／acquire 发布保持，不能把生成的快速读取与旧 runtime 的普通写入混用。contract 17 的脏卡组登记与 region prefix、contract 16 的完整 TLAB 元数据视图及 contract 15 的接口查询入口保持。对象、接口值、TD、初始化 cell、注册记录和扫描布局不变。metadata 9 删除 runtime 不消费的 body、descriptor、layout、scan、stackmap、gateway、source signature 与 image 内容指纹字段；实体 identity、实际地址、类型布局、扫描与 stackmap 合同保持。producer、公共头、runtime 和启动代码一起更新，旧产物与开发 sysroot 必须重建。带 prefix 的 descriptor 以 `{ u64 magic; u32 abi_version; u32 struct_size; }` 开头，`abi_version == 9`，size 与本节布局精确一致，reserved fields 为 0。公共 C 声明见 [scoop_runtime_metadata_v1.h](../../runtime/include/scoop_runtime_metadata_v1.h)。不保留空的兼容槽或以 bitcast 混用旧布局。

| descriptor | magic | size（bytes） |
| --- | --- | --- |
| Image | `0x53434f4f50494d47` | 208 |
| RootEntry | `0x53434f4f50454e54` | 128 |
| StaticStorage | `0x53434f4f5053544f` | 200 |
| ImmortalObject | `0x53434f4f50494d4d` | 152 |
| InitializationUnit | `0x53434f4f50494e49` | 288 |
| TypeRegistration | `0x53434f4f50545950` | 144 |
| SafepointRegistration | `0x53434f4f50535054` | 168 |
| CallableRegistration | `0x53434f4f5043414c` | 144 |

六类 registration 在 prefix 后依次保存 `{ u32 linkage_kind; u32 reserved_zero; semantic_id[32]; odr_group_id[32]; odr_member_id[32]; }`，共 104 bytes。linkage 为 Strong=1 或 Odr=2；Strong 的 ODR 字段为 0，Odr 的 group/member 非零。semantic ID 的种类由 record kind 决定，分别为 static storage、immortal object、initialization unit、exact type、safepoint site 和 callable body。

各 registration 的其余字段按以下顺序排列：

| record | 字段 |
| --- | --- |
| StaticStorage | `scan_kind:u32, initial_state_kind:u32, writable_base, byte_size:u64, allocation_extent:u64, required_alignment:u64, scan_program, initial_template:ByteSpan, initial_relocations, initial_relocation_count:u64` |
| ImmortalObject | `object_start, object_size:u64, required_alignment:u64, type_registration` |
| InitializationUnit | `schedule_kind:u32, reserved_zero:u32, diagnostic_path:ByteSpan, cell, storage, failure_root, initializer_callable_id[32], ensure_callable_id[32], initializer_entry, ensure_entry, startup_gateway_callable_id[32], startup_gateway` |
| TypeRegistration | `runtime_type_id:u64, reserved_zero:u64, descriptor` |
| SafepointRegistration | `safepoint_id:u64, site_role:u32, root_pair_count:u32, owner_callable_id[32]` |
| CallableRegistration | `entry, context_keys, context_key_count:u64` |

ByteSpan 的物理表示是 pointer 后接 `u64 length`；static relocation 是 `{ pointer_offset:u64, target:ImmortalObjectDescriptor pointer }`；Context key use 是 `{ exact_key[32], slot_cell:u64 pointer }`。擦除后的 callable function-pointer typedef 只用于地址身份，不能据此发起调用。

Image 依次包含 prefix、canonical Cone record（group/name/version ByteSpan 与 ConeIdentity）、直接依赖 identity pointer/count，再按 storage、immortal、initialization、type、safepoint、callable 顺序保存六组 record pointer/count。每个 image 表只列出最终链接选中的 producer，普通 external ref 不重复登记。

RootEntry 依次包含 prefix、owner ConeIdentity、main callable ID、gateway callable ID、failure-root descriptor pointer 与 gateway pointer。main 的源码签名为语言规范 12.4.4 的四种 ordinary 形态之一：无参数或单个 `Array<String>` 参数，返回 `Unit` 或 `Int`。完整签名及 gateway 的参数构造、调用和返回适配由编译器检查并生成，不在 runtime record 中保存其摘要。

四种 root gateway 使用统一的 C ABI；具体源码形态由编译器的 typed entry 和生成的 gateway 正文确定，runtime descriptor 不另加形态 tag，RootEntry 大小为 128 bytes。程序入口与 gateway 的原型为：

```c
typedef uint32_t (*ScoopRootEntryGatewayFnV1)(
    int32_t argc,
    const char *const *argv,
    int32_t *out_exit_code);

int scoop_rt_run_program(
    const ScoopImageDescriptorV1 *const *images,
    uint64_t image_count,
    const ScoopRootEntryDescriptorV1 *root_entry,
    int32_t argc,
    const char *const *argv);

int32_t scoop_rt_program_argc(void);
const char *scoop_rt_program_argv(int32_t index);
```

`scoop_rt_run_program` 每进程只调用一次。images 非空且恰为 root 的完整依赖闭包；输入顺序不承担初始化语义。调用方传入原生 argc/argv，`argc >= 1`，argv 含至少 `argc + 1` 项且 `argv[argc] == NULL`；前 `argc` 项各指向 NUL 结尾的非 null 字节串。runtime 在 eager 初始化前保存该 pointer/count，不复制或修改原始字节；调用方保证其到进程结束均有效，其他代码也不得修改已交给 runtime 的参数。`argv[0]` 保留启动者给出的可执行程序启动路径，不要求绝对路径或 canonical path。

原始 argv 访问器从启动输入保存后可用，均为只读 NoGC 操作；argc 包括第 `0` 项，合法 index 返回相应原始字节串，负数或不小于 argc 的 index 返回 null。这些指针不是 managed ref，不因 GC 移动。访问器不解码或跳过 `argv[0]`，无参数 main 也可通过平台库使用它们。

root gateway 的返回值只表示调用状态：`0` 为成功，`1` 为失败，其他状态非法。runtime 提供独占、非 null、4-byte 对齐的 `int32_t` 输出槽，调用前初始化为 `0`，有效期覆盖整个 gateway 调用。成功时 gateway 必须写入 `out_exit_code`：Unit 写 `0`，Int 写完整的有符号 32-bit 返回值，然后返回状态 `0`。因此 main 正常返回 `1`、负数或边界值都仍是 gateway 成功。失败时 runtime 忽略输出槽，只消费已发布的 failure root。

正常调用及 shutdown 完成后，`scoop_rt_run_program` 将该完整退出码返回生成的 C main，由 C main 返回给目标进程退出机制；当前 target 的 C `int` 为 32-bit，不作额外的低 8 位截断或范围拒绝。POSIX 向父进程呈现的正常退出状态仍遵守其低 8 位规则。启动或 main 的未捕获异常使用第 7 章的诊断与退出码 `1` 路径，不将异常展开到 C，也不把 gateway status 直接当作用户返回值。

所有 image、类型、scan、静态根、初始化单元、callable 和 safepoint 必须在首个 managed initializer 前登记。runtime 验证当前加载边界的格式、引用、实际地址范围、唯一性和 GC 契约，复用编译与链接边界已完成且未变化的语义、布局和 ODR 结果；不重新检查继承图、boxed payload 与 parent 的布局等语言规则。

登记建立完整 typed ID 与 record/address 的对应关系。runtime type ID 与 exact type、非零 SafepointId 与完整 site identity 都必须全程序一一对应；不同完整 identity 的固定宽度碰撞必须在使用前报错。不同 body ID 不得共用 callable entry；同一共享实体不得有多个实际地址。

登记时检查 TD 的 parent、函数类型关系和 itable 的接口 TD 引用。登记完成后这些描述符及其关系在 runtime 生命周期内保持不变；生成代码的 `scoop_rt_itable_find`、`scoop_rt_itable_lookup` 与 `scoop_rt_is_instance` 直接消费有效对象／静态类型携带的 TD，不逐次查询登记表。find 的缺失结果为 null，保证存在表的 lookup 缺失仍报告错误，类型判断继续遵守完整子类型关系；分配、native root 等实际输入边界保留各自必要的类型与对象检查。

callable 正文、EH、stackmap、callable/safepoint registration 与其 Context cells 必须来自同一被选实现。独立 helper 可来自不同 provider；所有共享 TD、storage、cell 和初始化状态仍唯一。没有 safepoint 的 Scoop body 也登记 callable；普通 native extern 和 runtime C body 按各自 ABI 消费。

非空 pointer/count span 的范围、乘加和对齐必须有效。既有 image/registration 空 span 使用类型正确的 addressable sentinel；`context_key_count == 0` 时 context_keys 为 null。descriptor、字符串、scan、template 与 relocation metadata 按不可变数据使用；callable entry 与 storage 分别按代码与可写存储发射。runtime 从平台装载段取得地址范围，不另查 VM region、`/proc/self/maps` 或审计读写／执行权限。

storage 的 `allocation_extent == max(byte_size, 1)`。ZST storage 使用独立的 1-byte writable token、None scan 与 canonical 空 scan。不同 storage 的 allocation range 不重叠，只有同一 ODR member 的重复引用可指向同一存储。

静态初值有两个封闭分支：

- `ZeroedForRuntimeUnit=1`：完整 allocation extent 初始为零，template 与 relocation 为空；用于初始化单元的 storage/published root，以及 unit/root-entry failure root。
- `EncodedStaticValue=2`：template 长度等于 allocation extent，padding、ZST token 和 managed-pointer leaf 为零；实际非 pointer 初值与 template 相同。该 storage 不再作为 runtime unit 的 storage 或 failure root。即使 template 全零，也保持此 tag。

Encoded relocation 按 pointer offset 严格递增，无重复；offset 8-byte 对齐、位于 logical byte size 内且恰为 Recursive scan 的一个 leaf。每个非 null leaf 必须且只能对应一项 relocation，指向已登记、只读 immutable immortal object 的精确 object start；其余 leaf 为 null。None scan 不允许 relocation，immortal 不含可移动引用。编译器生成初值与 relocation，链接器完成实际地址绑定；runtime 直接登记静态根，不遍历初值、重建 scan leaf 集合或逐项比较 relocation 副本。GC 在实际访问对象和根时处理必要的内存边界。

failure root 必须是独占的 8-byte、8-aligned Recursive storage，scan 为 `[1, 0]`，初值为 ZeroedForRuntimeUnit；不能兼作普通属性或另一 unit/root 的失败槽。cell 初值为 `{0, null}`，不同 unit 的 cell 互不重叠，也不与 storage 重叠。初态由编译器发射和链接装载保证；runtime 不在启动时扫描初值或重查 cell 零状态。登记完成后，coordinator 或 gateway 按运行契约修改它们。

eager schedule=1，含已登记 startup gateway；lazy schedule=2，gateway ID 与 pointer 分别为全零、null。initializer/ensure 的 ID 与 entry 必须解析到同一实际 callable。eager gateway 的 entry 来自对应 callable；root gateway 遵守同样生成规则。runtime 直接调用已登记的 gateway。

eager startup gateway 继续使用独立的 `uint32_t(void)` C ABI，成功返回 0、失败返回 1、其他状态非法；runtime 分别按精确原型调用 eager 和 root gateway，不将它们视为同一函数指针类型。root gateway 的 managed catch 覆盖 argv 构造与 main 调用，将原 Throwable 写入 root-entry failure root 并 EndCatch 后返回状态 1；eager gateway 使用 ensure 已发布的 Failed 与 failure root，不复制或覆盖异常。静态失败槽保存普通 managed 引用，不得保存 native unwind record。

每次 init/root gateway 调用建立独立 native→managed boundary，开始为没有 managed frame 的 EntryPending；入口必须 poll 后才成为 ActiveManagedSegment。返回前完成全部 managed 异常处理，只把 GC-free status 及 root 成功时的整数输出交给 C；随后按 LIFO 恢复外层 mode/boundary。GC 仍扫描外层 frozen segment 和 native roots，不把 C coordinator 的 PC 当作 managed anchor。

以下 RuntimeImageFingerprint 仅是 `.slib` 的编译期内容摘要，不是 runtime 字段。最终 linker 按已选定义建立实际 image 表，不重新计算或写入这一摘要。产物生产时对 canonical records 编码，不能 hash raw struct 或 ASLR 地址。编码为 little-endian u32/u64、32-byte ID 原字节、`u64 length + bytes` ByteSpan、`u64 count + elements` sequence、无 padding 的声明序 product 和 `u32 tag + payload` sum。record kind 按六张表顺序为 1..6；payload 先保存 linkage、semantic ID、ODR group/member，再保存下表的字段，不编码 descriptor prefix、padding 或 reserved fields。role 使用以下 u32 tag：

| role | tag |
| --- | --- |
| OwnStorageAtom | 1 |
| OwnImmortalAtom | 2 |
| OwnInitializationCell | 3 |
| CallableEntry | 4 |
| UnitGateway | 5 |
| TypeDescriptorAtom | 6 |
| RootGateway | 7 |
| OwnCallableEntry | 8 |

| record | common identity 之后的 canonical 字段顺序 |
| --- | --- |
| StaticStorage | scan kind、OwnStorageAtom、logical size、allocation extent、alignment、scan choice、scan fingerprint、layout fingerprint、initial-state sum |
| ImmortalObject | OwnImmortalAtom、object semantic ID、size、alignment、target exact type ID |
| InitializationUnit | schedule、diagnostic-path ByteSpan、OwnInitializationCell、unit ID、storage ID、failure-root storage ID、CallableEntry、initializer body ID、CallableEntry、ensure body ID、gateway choice |
| TypeRegistration | runtime type ID、TypeDescriptorAtom、exact type ID、descriptor fingerprint、layout fingerprint |
| SafepointRegistration | safepoint ID、site role、root-pair count、owner callable ID、normalized stackmap fingerprint |
| CallableRegistration | body definition fingerprint、OwnCallableEntry、body ID |

scan kind 为 None=0 或 Recursive=1；scan choice 为 `Empty=0 | Program=1 { scan fingerprint }`；gateway choice 为 `None=0 | Unit=1 { UnitGateway role, body ID, fingerprint }`。static state 使用上述 1/2 tag，Encoded 内联 template ByteSpan 与 `{pointer offset, immortal semantic ID}` sequence。Context key、cell 初值及引用由所属 body 的对象内容指纹覆盖，进程 slot 和加载后的 cell 值不进入指纹。各表按 `(semantic ID, linkage, ODR group, ODR member)` byte 序排序；直接依赖 identity 排序去重。未知或 reserved tag 非法。

`RuntimeImageFingerprint = SHA-256(ByteSpan("scoop-runtime-image-v2") || RuntimeAbiFingerprint || TargetProfileFingerprint || canonical Cone record || sorted direct dependencies || six canonical record sequences)`。两个输入 fingerprint 各为 32 bytes；canonical Cone record 依次编码 group、name、version ByteSpan 和 ConeIdentity。物理对象分片和 ASLR 地址不进入此编码；结果只进入产物摘要，不回填机器对象。body、descriptor、layout、scan 与 stackmap fingerprints 的产物规则见编译器与产物规范 2.6。

`PersistentCallableBodyId = SHA-256(ByteSpan("scoop-callable-body-v2") || canonical(CallableBodyKey))`。key 的 u32 tag 与字段为 Strong=1 `{owner}`、Odr=2 `{member}`、RootGateway=3 `{root_cone, main_body_id}`、InitializationStartupGateway=4 `{unit_id}`、ReleaseHook=5 `{owner_exact_type_id}`。这些 body 的身份不能互换。身份与内容摘要由编译、链接过程生成；runtime 使用实际登记记录及引用，不重新生成摘要，也不以摘要非零断言替代实际索引和引用处理。

stackmap 使用 LLVM v3 record。规范化内容依次为 `{format_version=3, site_id, safepoint_id:u64, owner_callable_id, site_role:u32, root_pair_count:u32, instruction_offset:u32, stack_size:u64, locations, live_outs}`。location tag 为 Register=1 `{size:u32, dwarf_register:u32}`、Direct=2 或 Indirect=3 `{size, dwarf_register, signed_offset_bits:u64}`、Constant=4 `{size, value_bits:u64}`；live-out 为 `{dwarf_register:u32, size:u32}`。保留原 sequence 顺序和数量，i32 offset/constant 符号扩展到 i64 bits，ConstantIndex 解析为 pool 中的 u64 Constant；reserved/flags/padding 为零。

`normalized_stackmap_fingerprint = SHA-256(ByteSpan("scoop-stackmap-record-v1") || canonical(record))`。该摘要仅用于编译期内容计算，不写入 runtime registration。runtime 直接使用已解析的记录。ASLR function address 不入 hash，但必须等于 owner callable entry；instruction offset 与 stack size 入 hash。site role 为 ManagedPoll=1、ManagedCall=2、ManagedInvoke=3、NativeSafeTransition=4、NativeBorrowedTransition=5。实际 record 与 registration 的 site、owner、role、root count 一致。

受支持目标要求 location 数为 `3 + 2 * root_pair_count`，前三项是 8-byte Constant，每对 root 为相同的 8-byte SP/FP Indirect 可写 slot。root count 是实际机器表示中的 relocation pair 数，不必等于源级逻辑 leaf 数；invoke 与 native transition 使用显式 root frame，gc-live 为零。

stackmap 范围可以包含多个完整 v3 blob，必须解析到范围末尾。registration 与 raw record 完整对应，return PC 使用实际原始地址，不作减指令宽度、邻近符号或范围猜测。不同完整 site key 不能复用 SafepointId 或 return PC；同一 ODR site 的重复记录只在 owner、最终地址和实际 location/live-out 等 payload 全等时合并；这一比较用于合并实际重复记录，不建立记录指纹的镜像。

启动收集 site 时检查 ID、role、root count 并建立唯一的 site 索引；owner callable 的身份与归属只在该 site 的首条 raw record 合并时解析一次，同时检查 entry、root count 和目标 frame 布局。后续同一 ODR site 的 raw record 与已验证记录比较完整地址及规范化 payload，复用其 owner 与 frame 验证结果。所有 site 必须有 raw record，全部对应关系仍在任何 managed 执行前验证完毕；不增加持久的 owner 缓存或新的登记字段。此规则对全部 profile、预编译 Cone 和启动测量配置一致。

不可变 metadata 与 stackmap 的段放置、relocation 和 RELRO 由目标工具链与 loader 处理，runtime 直接使用装载结果。native DSO 不构成 Scoop logical image。String 使用实际声明的唯一 TD，`scoop_td_String` 与其地址相同。运行期不新增或卸载 Cone，Scoop runtime 也不执行 managed global destructor；native C／C++ 初始化／析构边界见第 7 章。

## 3. GC 契约

GC stress 支持是显式的构建能力。默认 runtime 在 C 编译时排除 stress 专用实现和环境读取；`SCOOP_GC_STRESS_MOVE` 与 `SCOOP_GC_STRESS_MINOR` 对这种程序无效。`scoop --enable-gc-stress` 选择编入该能力的 runtime；运行时变量值为 `1` 时启用对应模式，为 `0` 或未设置时关闭。构建开关本身不自动开启 stress。

该开关只控制 stress 分配调度、强制搬迁、poison/quarantine 及其专用验证，不关闭普通 GC 的移动、根更新、safepoint 或屏障，也不改变公开 runtime ABI。构建配置、预编译资源选择和最终链接缓存见实现规范 2.7～2.8。GC 正确性专测显式选择所需能力，普通功能测试不编入 stress 支持。

GC 合同由实际代码生成与 runtime 操作实现；不要求在生产编译中重复验证 LLVM 工具链的成功输出，也不在 runtime 补做产物来源或生产过程证明。专门回归测试继续覆盖根、relocation、屏障与运行结果。

GC 支持多个 mutator，collection 在 stop-the-world 协调下执行，使用精确根与对象 scan，不保守猜测整数或 C 栈中的地址。minor 与 full collection 均可移动对象，pin 之外的裸地址不跨 safepoint 稳定。

### 3.1 分配与发布

managed 分配返回直接对象指针，普通小对象分配摊还 O(1)，不要求经 handle 间接访问。runtime 分配入口必须在返回前完成清零、对象头、精确大小与对象起点登记；生成代码的内联快路径可按下述确定初始化规则省去不可观察的重复零写入。可能触发 GC 的入口遵守完整 safepoint 和 root 契约。

generated code 所需的 TLAB 分配视图、对象头状态位和 poll／card 数据布局是明确的共享快路径合同。Immix 的普通对象阈值与 TLAB 资格判断归 GC 分配实现及对应 codegen GC 模块，不能向通用 LIR 泄漏算法策略；当前阈值保持一个 32 KiB block 扣除首个 128-byte line。纯模块整理不改变布局、位值、登记顺序或快路径调用成本；共享视图的布局变更按 2.8 更新 ABI。

`ScoopAllocationContext` 是现有 TLS 指针指向的统一视图，按顺序保存七个 native pointer：cursor、limit、block base、精确大小单位数组、starts 位图、occupied-line 位图、当前 mutator 的分配计数。当前 64-bit 目标的大小为 56 bytes、对齐为 8，字段偏移依次为 0、8、16、24、32、40、48。计数视图是四个对齐到 8 的 atomic u64，依次为 objects、nursery objects、bytes、nursery bytes，大小 32 bytes。计数仍由本线程写入，其他线程按原统计合同读取；视图指针不增加 GC roots。refill 在使 cursor 可用前填好对应年轻块的元数据视图，收集／detach 先令分配范围失效；cursor 不可用时不得解引用块元数据。所有 profile 与 stress runtime 使用同一布局，不按优化开关改变结构。

allocation context 的地址与所属 mutator 在单次 managed 函数调用期间稳定：GC、refill、native 转换及同步 callback 不替换 attachment；detach 只能发生在全部相关 managed frames 退出后。ARM64 release 在首次实际需要 TLAB 视图时读取一次 TLS 并在本次调用中复用该 AS0 地址，不在未执行分配的路径主动取址。复用只涵盖 context 地址；cursor、limit、块元数据视图和计数内容继续按每次分配的当前状态读取，不能跨 GC／refill 固定。缓存为本次调用的普通非托管局部值，在正常与异常路径上遵守同一调用生命周期，不增加对象字段、TLS 状态、GC roots 或 ABI 字段。x86 与 debug 保留逐次读取，所有目标继续使用相同布局与发布合同。

该视图以 32 KiB block 的真实起点为基址，保留首个 128-byte line。`size_units` 为 4096 项、2-byte 对齐的 u16 数组，以对象 byte offset 除以 8 索引，保存对齐后大小除以 8。starts 为 64 项、8-byte 对齐的 u64 位图；对象 word index 为 `offset / 8`，对应 bitmap word 为 `index / 64`、低位起 bit 为 `index % 64`。occupied-line 为 4 项、8-byte 对齐的 u64 位图，以 `offset / 128` 到 `(offset + size - 1) / 128` 的闭区间标记同样的 word／bit 位置。此共享几何及发布顺序属于 contract 16，不能只在 runtime 或某个构建 profile 中更改。

release 复用 LIR 中已有的完整本地 TD 或 typed array layout，将确定的对象对齐与 nursery 资格作为常量发射；不因物理对象分区把已知事实退化为 TD 加载，也不重做布局或 release 语义检查。仅对请求大小为编译期常量、位于 16～128 bytes、对齐已知且确定无 release hook 的分配，在原 TLAB、溢出和边界判断成功后直接完成清零、对象头、精确大小、位图发布和计数。对齐后的范围至多跨两个 line 和两个位图字；同字的 line mask 合并，不引入循环或 safepoint。其 atomic load／store 次序与下述单写者登记一致，starts 最后作 release 发布，随后更新四项 relaxed 计数。使用现成的 allocation context，不再次读取 TLS，不访问 `ScoopGcBlockMeta` 或 mutator 的私有偏移。debug、动态大小、更大对象及缺少完整布局事实的外部／动态 TD 沿原完成入口和必要 TD 加载；有 release hook 的对象仍从旧代慢路径分配。失败分配沿原 collecting slow path，保持完整 live set 和 relocation。

小型内联分配可缩短一次连续清零的末尾：ARM64 接受对齐后不超过 128 bytes 的对象，x86 接受不超过 64 bytes 的对象，仅省去已证明在同一直线执行路径中被后续 typed 字段存储完整覆盖的无引用尾部。证明可跨无条件跳转并跟踪未取址的局部副本；遇到调用、poll、异常出口、条件分支、对象读取、取址、外传或其他未分析操作即停止。只接受实际存储覆盖全部字节、无 padding 的标量或普通 struct／tuple，保留对象头、引用槽、padding、enum／MaybeUninit 存储及其他未证明字节的清零；不拆成多段 memset，不移动构造写入或位图发布。被省去零写入的字节必须在该路径的下一次可观察操作前写好，STW 仍经原合作式 safepoint 协调。慢路径、共享完成入口、typed 数组和 debug 保留完整清零，精确 roots、分配计数、release-ready 与 ABI 不变。

生成代码成功 bump 后的完成入口接收已经按 TD 对齐的普通对象大小，复用当前 mutator 的 nursery block；此前已确认有效 TLAB、大小不溢出且在 limit 内，以及 TD 无 release hook，moving stress 不提供可用 TLAB。完成入口不重复规格化大小、判断分配资格、查找 block 或重查已保留范围的对象起点；它仍清零全部对象字节、设置对象头、登记精确大小，并按原 relaxed line bitmap／release starts 次序发布，更新线程原子计数。此入口不经过 safepoint；动态 runtime 分配入口仍执行其输入与布局检查。

普通 nursery 分配块在 mutator 运行期间只有其所属线程写入对象起点与 occupied-line 位图；moving stress 的独立年轻块也由本次分配线程独占初始化。成功保留范围后不经过 safepoint 即可完成登记；年轻块的元数据发布复用已确认的大小、对齐、空闲与块种类，不在登记函数内再次核对。位图更新复用这项单写者关系：用 relaxed atomic load 取得旧字，再按 OR 后的完整字执行 occupied-line 的 relaxed atomic store 和 starts 的 release atomic store。已发布的其他位必须保留；其他线程的对象查询继续使用 acquire starts 读取。晋升、退役、重新分配和清零均在原 STW／分配区失效协议下完成。共享旧代分配块仍使用原子读改写，collector 的并行标记保持原子首次置位，串行复制按 3.7 复用 STW 独占关系；不能把 nursery 的单写者性质扩展到共享分配块或任何新增的并发写入者。该选择不改变 TLAB、对象、metadata 布局或统计 ABI，所有 profile 遵守相同合同。

每个已 attach 的线程持有一个 GC 管理的 mutator 状态，包含 TLAB、block 所有权及独立对齐的分配计数；线程业务只保存该状态的引用，通过具体生命周期操作创建、绑定、失效和 detach。generated code 仍使用原 TLAB TLS 指针，GC 分配沿该指针取得同一 mutator，不增加另一份 TLS 或线程目录。统计在既有 world／registry 保护下汇总现存 mutator 与已退出总账，detach 在从目录移除前完成归并，collection 在稳定目录下使所有分配范围失效。

进入 collection 时，所有线程的分配状态必须停止使用并与 collector 一致。对象不得在仅部分登记时被扫描；分配失败使用明确的失败出口，不能返回无效对象。

nursery 容量和 collection threshold 是收集触发条件，不是堆空间耗尽的证明。收集后被其他 mutator 抢先取得分配区时，分配方重新尝试补充自己的分配区；不能按固定次数的 nursery 竞争报告 OOM。需要 full collection 才能确认的分配失败，不能把加入另一轮 minor collection 等同于已完成自己的 full collection。

普通运行的 nursery 初始容量为 1 MiB，在 1 MiB 与配置上限之间自适应调整，按 32 KiB block 对齐；默认上限 32 MiB。`SCOOP_GC_NURSERY_MAX_MIB` 可设为正的十进制整数以指定 MiB 上限，数值必须能表示为目标 `size_t` 的字节数；1 表示固定 1 MiB 容量，更大上限允许吞吐与峰值内存之间的取舍，非法配置在 runtime 初始化时报错。它是按需分配的触发上限，调整数值不预先映射或触碰全部容量。每轮收尾复用本轮 nursery block 容量与实际年轻存活字节数，不增加对象年龄字段、逐对象统计或额外遍历。full 后以回收后活跃 block／large mapping 容量的一半作为后续容量基准，向上对齐到 block 并限制在配置上下限内；容量不足基准时直接提高。实际 nursery 使用达到当前容量的四分之三且存活至少四分之一时，容量至多扩大一倍。连续四轮低于十六分之一存活时至多缩小一半，不低于最近 full 确定的基准；中间存活率重置该计数。无年轻分配的显式 minor 不构成调整样本；full 仍可用新的存活堆更新基准。所有调整在原 STW／heap lock 内完成，refill 只读取当前容量，成功 TLAB bump 不增加操作。

自适应运行下，full 的增长阈值针对已提交旧代容量，年轻 block 容量单独受 nursery 上限约束；分配旧代 block／large mapping 时将本次请求计入旧代压力。full 后阈值仍为回收后已提交容量的两倍，最低 16 MiB，溢出时饱和；minor 晋升超过旧代阈值会在随后分配慢路径触发 full，不能借扩容不断上调旧代阈值而推迟回收。显式 moving／minor stress 与 full-only 对照保留原固定触发策略，不采集自适应样本。该策略在普通 debug／release 默认一致，不改变对象、allocation context、runtime ABI、`.slib` 或预编译库合同；survivor 阶段按 3.9 独立选择；自适应使用全部年轻存活字节，不能只用实际晋升量代表存活压力。

堆由按需映射的普通 region 与独立大对象 mapping 构成；普通 region 初始策略为 16 MiB，内含 32 KiB block／128 B line，不限制总堆或单个大对象大小。先复用空闲 block／line 和保留 region，再扩容；超过旧 1 GiB 边界仍能继续分配。大对象按实际大小映射，记录原始 mapping base／length 和对齐后的范围，不遗失对齐前后页。所有 block、TLAB、forwarding 和 nursery 状态明确归属 region。

分配计数为 cache-line 分隔的线程累计值，本线程以 relaxed atomic store 发布，查询以 atomic load 汇总；热路径不对多个全局统计量执行 atomic RMW。detach 在 registry 生命周期保护下并入退出线程总账；STW 汇总更新 collection 基线，当前对象数由基线和随后分配推导，minor 同时保留旧代计数。查询、基线更新和 detach 归并遵守同一锁顺序，不漏计或双计。

累计对象数、nursery 对象数及两种分配字节数在 collection 后不清零。查询按 registry/world lock → heap lock 读取目录与基线；STW collector 在目录已稳定、持 heap lock 时直接汇总，不反向取得 world lock。detached 总账在移除线程前更新，主线程退出后的最终报告仍包含其分配。并发查询是逐线程、逐字段的原子快照，不承诺不同统计字段间的事务一致性；STW 与线程全部退出后的汇总精确。线程计数区域按 128-byte 边界独立对齐并分配，覆盖当前 Darwin/AArch64 的 128-byte 和 amd64 的 64-byte cache line。

外部 native 容量可通过 `void scoop_rt_gc_adjust_pressure(int64_t delta)` 提示 GC。适配层直接调用 GC 操作 `scoop_gc_adjust_pressure`，不经过 managed entry、线程 attachment 或收集协议。入口始终 GC-free、无异常、无 managed 分配、无 safepoint、无 TLS／root／handle／pin 访问、不等待 mutator，并发及未 attach 调用合法。首版操作有意为无状态 no-op；不要求维护精确计数、收集阈值、配额或分配失败重试。后续策略只能在原有合法收集决策位置消费提示，不能从通知栈内发起收集。正负 delta 是调用者已提交资源变化的贡献，零无变化；接口不接管资源所有权。

### 3.2 Safepoint 与机器根

平台实现按实际职责分开：managed frame、caller anchor、返回值 shim 与浮点环境控制属于 arch，线程栈／VM 属于 OS，加载映射与 metadata 属于 image。构建只组合已实现的组件，架构专用 flags 只由对应架构选择；此职责划分不改变 GC、异常、对象布局或公共 runtime ABI。

GC 私有 helper 的目标相关内联选择由平台层提供，通用 GC 不直接使用架构宏选择调用方式。各目标共用同一算法与必要检查；内联选择不改变对象表示、函数 ABI、forwarding 规则或公开配置项，其性能依据按目标分别验证。

managed 函数入口（下述有界叶函数除外）和循环回边，包括 continue 回边，提供 safepoint。编译器可将继续迭代区域内无调用、分配或异常的简单计数循环分块：正文包含局部整数／Boolean／严格浮点计算，或索引合法性已在 MIR 保证的固定表示数组值读写、普通字段读取、身份比较和长度读取（实现规范 2.19）。这些值包括单字引用、interface、相应 MaybeUninit 和单引用 payload 的可选值；写入保留 3.6 的完整屏障，初始化保留 2.4 的部分初始化扫描规则。正文可以含条件分支，但去除唯一回边后必须无环，所有继续迭代的路径到达同一 latch，不能绕过归纳变量与内层倒计数的更新。提前退出沿原 return、break 或检查失败出口执行；只在退出路径上的调用、分配、异常与清理保留原 safepoint／roots，不进入复制区域。原循环 header 保留 poll，生成的内层由实际 CFG 中的常数上限倒计数约束，最多执行 64 次后返回外层 poll。数组的 managed 基址在外层 poll 后取得，内层不新增握手或跨 safepoint 的裸地址；已有引用写屏障在原写入处完成，不能因为分块或“刚分配”而省略。该选择不承诺墙钟暂停上界，不改变任意其他循环或 continue 的 poll 责任。managed references 在 LLVM ABI 中使用 address space 1；跨 safepoint 存活的引用必须由 stackmap 或显式 compiler root frame 描述。

普通 Managed body 从已激活的 MANAGED 状态进入。root／initialization startup gateway 是仅有的 MANAGED_PENDING 入口，必须先完成自身入口的有锁激活；callback 在 runtime 进入协议中先激活，native 返回完成原握手后才恢复 managed 执行。普通调用、动态分派与跨 Cone 调用沿用这个状态保证，不按函数名称或调用点是否可内联区分。

短小的普通 Managed 叶函数可省去入口 poll：正常返回的无环直线路径（可含内联产生的跳转块）、最多 64 条标量／固定大小值操作、字段读取及终结指令，每个值复制不超过 64 字节，无调用、分配、循环、异常或 native／全局存储访问。已激活的线程直接执行有限正文并返回 caller；此选择不改变 callable 的 Managed effect，不向 caller 传播免 poll 资格，不承诺墙钟停止上界。root／initialization gateway 无论正文是否短小都保留首次激活。

普通入口与循环中的 Full poll 只以 acquire 原子读取 world phase，RUNNING 时继续执行，不读取 TLS、mode 或 epoch。collector 在 STOPPING 后仍须等待该 MANAGED 线程停稳；若本次读取与停止请求交错，线程到后续真实 safepoint 才停下，不能在其执行期间扫描或移动对象。原子检查不能被提升出循环。正常路径不取 world lock、不广播、不建立 runtime anchor，也不无条件物化该 poll 的显式 roots；慢路径仍调用协调入口、发布 anchor、按原 phase／epoch 协议 park 并执行 relocation，合流连接快路径原值与慢路径新值。NativeSafe 返回、callback 和 NativeBorrowed 保留各自原握手。

编译器读取的私有 ABI 为 TLS 指针 `scoop_rt_poll_state`，指向 size 16／align 8 的 `{ atomic u32 mode @0; atomic u64 observed_gc_epoch @8; }`；padding 不参与读写。它直接引用注册线程状态中的同一组字段，attach 时设置、detach 时清空，不复制另一套线程状态。全局 `scoop_thread_world_phase` 为 atomic u32／align 4，`scoop_thread_gc_epoch` 为 atomic u64／align 8。Gateway 入口 poll 保留完整检查：依次以 acquire 读取全局 epoch、phase，以 relaxed 读取本线程 mode、observed epoch；仅当 phase 为 RUNNING（0）、mode 为 MANAGED（1）且 epoch 一致时走快路径。MANAGED_PENDING 必须调用 `scoop_rt_safepoint`，由现有有锁协议激活。普通 Full poll 不取得该 TLS 视图；runtime 的 epoch 与 observed epoch 仍用于停稳确认。

线程只写自己的 poll state，包括 attach／detach、pending 激活、park、native 转换、初始化等待、收集者模式和进程终止。收集者读取其他线程的状态，不代写其 mode 或 observed epoch；各线程 publication 的 release／seq_cst 写入及跨线程 acquire／seq_cst 读取保持原协议。生成代码对所属线程的读取不承担跨线程发布的 acquire 职责，依赖同线程先行写入与原子一致性；仍使用原子读取，不能改为普通非原子访问。该内存序选择适用于全部 profile；旧产物的较强 acquire 读取与其兼容，不改变状态布局、TLS、生存期或 runtime ABI。

线程状态地址在单次 managed 函数调用期间稳定；确实需要该视图的 Gateway poll 可复用地址，原子状态检查不能随地址一起提升。所有 profile 采用同一选择，较旧产物多余的 TLS／epoch／mode 检查仍兼容；不改变 runtime ABI、状态布局、GC roots 或握手协议。

SSA 接口在 safepoint 后由更新的 AS1 object 与原 AS0 metadata 分量重建；内存接口只回写 object。不能把两个分量改成整数后依赖保守扫描。

普通 call/poll 的 relocation root 按 2.8 的实际 stackmap 更新。invoke 的正常与异常出口使用显式 compiler roots；native transition 使用 caller root frame，二者的 statepoint gc-live 为零。引用在握手后重新读取，再传给实际 native callee；native 返回的引用在重新允许 GC 前进入有效 root。

调用期间保活不要求返回后恢复已死的直接实参。编译器按实现规范 2.4 保留实际 AS1 实参在调用站点的精确 root，仅为后继仍使用的值生成恢复；必要的传参 spill 仍由机器 ABI／stackmap 决定。间接按值 storage、借用、异常出口以及分配后仍用于初始化或复制的输入遵守各自保活与重读规则，不能由源码调用后无使用推导其已死。省略恢复不允许缩短调用期间的对象存活期，也不改变 collector 对实际物理根的更新。

接口聚合及其 object 分量若经相同输入与前驱得到同一个值，可以共享物理根；编译器在 RS4GC 之前完成这种标量合流归并，逻辑恢复和实际后继使用统一取得更新后的引用。不同路径的输入、重赋前的快照与独立对象仍各自保活。注册的 root-pair count 必须继续与最终机器 stackmap 精确一致，不能忽略数量差异或截断根记录。

native 实际调用不得 unwind 穿越边界。静态 FunPtr 的同步 NoGC 调用不隐式 poll；可能进入 runtime 或 managed callback 的路径遵守线程转换规则。语言规范 13.4.1 的 `@GCLeaf` C 调用不构成 safepoint，也不建立 native transition 或专用于该调用的 caller roots；实参求值和调用前后的真实 safepoint 继续遵守正常根规则。

core 的 Long／ULong hash、Boolean hash／equals 使用普通标量正文，执行固定次数的计算，无分配、阻塞或回调。当前 word hash 在 ULong 上依次 xor 右移 30 位、乘 `0xbf58476d1ce4e5b9`、xor 右移 27 位、乘 `0x94d049bb133111eb`、xor 右移 31 位，按原 64 位结果转换为 Long；Boolean hash 为 false→0、true→1，equals 保持逻辑相等。Boolean.toString 选择 provider 的 immortal 字符串，不新增托管 root 或对象。已有 C 后备符号及调用合同保留供实际外部引用使用，新 core 不再为这些操作声明 native wrapper；不将任意同名 extern 特化，也不改变公开方法签名。String hash／equals／字符计数会随输入增长，继续遵守各自 managed／native 边界，不能沿用这项有界标量选择。

Darwin/AArch64 的 SP/FP DWARF register 分别为 31/29，stack size 为 16 的倍数。Linux/amd64 分别为 7/6；若 stackmap stack size 为 N，要求 `N % 16 == 8`。amd64 的入口 return PC 在 `[RSP]`，call-site SP 为 `RSP + 8`，FP 为 RBP；N 不包含 return address，`FP = SP + N - 8`，`[FP]` 保存前一 FP、`[FP+8]` 保存 return PC、前一 SP 为 `FP+16`。root slot 位于 `[SP, FP)`。这些目标保留 frame pointer，不使用 red zone 或破坏栈遍历的 tail call。

### 3.3 精确根集合

Scoop GC 适配层解释已登记的 image／stackmap、线程发布的 managed segment、各类 root frame、Context 与 handle，并向 collector 访问器交付真实可更新的槽位。collector 不解析这些根的语言表示或遍历线程根链；适配层复用当前 collection 的冻结 registry，不另行复制根目录。显式 pin／handle 的保活及 pin 计数仍由 GC 管理，线程 pin frame 的枚举由适配层完成。

对象扫描描述的解释也归适配层。GC 请求整对象或半开地址范围扫描；并行 marker 通过适配层提供的元素起点、数量与步长视图划分既有大数组任务，不读取 scan 程序编码。描述、对象大小／对齐等未变化的标量事实可静态内联访问，保持原有检查边界与每对象调用成本；不得建立第二套类型／扫描元数据。

分配大小归一化、实例对齐、对象类型初始化和 release hook 的读取同样通过对象适配操作完成；GC 继续决定代际、存储登记与 ready／pin 状态。普通 runtime 使用分配、已发布对象分类、数组克隆源检查与拆箱检查等实际操作，不自行组合堆锁、对象起点和精确大小查询。克隆／拆箱的类型与大小检查保持原有 heap 锁保护及失败语义；检查之后到实际读取之间没有 safepoint，不能据此省去既有检查或宣称无锁拆箱。

完整根集合包括：

- 活动 managed 栈段的 stackmap roots 与 compiler root frames；
- native transition 冻结的外层 managed 栈段，以及全部 caller/native root frames；
- 已登记的 Recursive static storage、初始化与 root-entry failure roots；
- 当前 TaskContext，包括 native-safe 线程保存的 context；
- stable external exception payload 中的引用；
- live GC handles 与 pinned 对象。

immortal 对象只读且不含可移动引用。collector 对每个 root/对象只按其实际 scan 访问引用槽，移动后原地更新；异常 record 的 handle 与其他强 handle 一同更新。

### 3.4 Pin 与 handle

pin 为摊还 O(1)、unpin 为 O(1) 操作。同一对象的显式 pin 共用一项登记和非零计数；每次 pin 增加计数，每次 unpin 减少计数，计数归零才移除登记、清除固定状态。登记期间对象保活且地址稳定，多线程操作由 heap lock 串行化；collector 在同一锁保护下访问登记。对象头的 GC 私有状态字保存其登记索引，移除时交换末项并修正该对象的索引，不线性搜索 pinned 列表。索引不进入语言可见的 `PinnedPtr.raw`，也不改变对象头大小或 typed ABI。非法地址、没有对应显式 pin 的 unpin 或计数溢出属于 fatal ABI error。pin 不替代线程、root 或写屏障协议，不提供对 payload 的同步。

GC handle 是 GC-free opaque 64-bit 值，引用一个当前 live slot 与 generation。slot 复用时 generation 改变；非法、stale 或已释放 handle 是 fatal ABI error。handle 保活对象但不固定地址；解析后跨 safepoint 使用时仍须 root/reload，或显式 pin。

接口 pin／handle 只保存 object；返回静态接口值时以 NoGC 表查询重建对应 itab，不将 metadata 作为 root 或固定对象。

语言规范 13.11 的作用域借用使用 caller 栈上的 `ScoopPinFrame { previous, object }`，按 LIFO 登记在线程状态中。push/pop 是 NoGC、nounwind 的本线程操作，不取锁、不分配；pop 必须匹配当前帧。帧不得跨线程迁移，线程 detach 时不得残留帧。进入 NativeSafe 时帧链随该线程的其他根冻结，collector 只在所有线程停稳后读取。

collector 在移动规划之前固定全部活动帧引用的 GC 对象，并把每个对象作为根；immortal 对象本已地址固定，不修改其只读对象头。一次 collection 结束后清除帧带来的临时固定标记，保留显式 pin 的固定标记；下一轮 collection 按仍活动的帧重新固定，因此 pop 不需要访问全局 pin registry。多个帧／线程引用同一对象天然保留到最后一个借用结束，nursery 中固定对象沿既有 `PINNED_PARTIAL` block 处理。帧本身不提供 payload 同步，也不改变显式 pin 的计数。

### 3.5 线程与握手

每个参与 managed 执行的线程都有已登记的 thread state。原子 mode 明确区分 `MANAGED`、`MANAGED_PENDING`、`NATIVE_SAFE`、`NATIVE_SAFE_RETURNING`、`NATIVE_BORROWED`、`PARKED` 与 `COLLECTOR`；这些是 runtime 内部状态，不新增语言可见状态或改变已有公开状态码。根的发布与恢复具有 release/acquire 同步，NativeSafe 无锁返回与 collector 停顿请求之间还须满足下述 seq_cst 握手，不能丢失 collection 请求。

这些协议负责 runtime 与 GC 的内部同步；safepoint、native 状态切换、pin、handle 和 GC 写屏障不作为用户共享数据的同步原语。数据竞争及其引起的撕裂按语言规范 1.2 属于未定义行为，runtime 不为有数据竞争的程序提供内存安全保证。

NativeSafe 不访问未固定、未受保护的 managed 指针；其冻结栈段和 roots 仍可被扫描。NativeBorrowed 可在本次同步调用中使用 direct ref，跨 safepoint 必须按 4.2 登记并重读。native-safe 返回、native-borrowed 返回及 callback 进入 managed 前都须协调当前 epoch。

转换可嵌套，按 LIFO 保存与恢复 previous mode、boundary 和精确 anchor。活动与冻结 managed 栈段必须分别可识别；walker 在真实 native boundary 停止，不能跨任意 C frame 反向猜测 managed 栈。

foreign thread 在执行 callback 前 attach；只有 attachment 的拥有者可以在退出全部 managed/native-root frames 后 detach。线程栈范围必须与当前实际映射一致；可增长的栈由该线程在同步边界发布有效范围，collector 使用已发布快照。

**NativeSafe 发布与返回**

managed 线程先完整写入 transition、冻结栈段边界与根链，再以 seq_cst 发布 `NATIVE_SAFE`。常见进入路径不获取 world lock，也不广播条件变量。自发布起，直到通过返回握手或有锁 managed 入口的 `RUNNING` 检查前，线程不得修改或撤销这些 GC 可扫描记录，也不得读取 collector 可能正在回写的引用槽；合法 native 代码仍可按 FFI/pin 契约继续执行。

无锁返回先以 seq_cst 将 mode 写为 `NATIVE_SAFE_RETURNING`，再以 seq_cst 读取 world phase。读到 `RUNNING` 后才允许恢复 managed 状态、撤销 transition，并按 3.2、4.2 重读引用和撤销根。否则先以 seq_cst 将 mode 写回 `NATIVE_SAFE`，取得 world lock，在条件变量循环中等待 `RUNNING`；释放锁后必须重新执行 RETURNING/phase 握手，不能仅凭曾被唤醒就恢复 managed。常见返回路径不获取 world lock，也不广播条件变量。

collector 必须以 seq_cst 发布 `STOPPING`，随后以 seq_cst 读取目标线程的 mode；与返回线程的两个 seq_cst 操作一起，排除双方同时依据旧状态继续执行。collector 在停稳判定中观察到 `RETURNING` 时继续等待；若已观察到 `NATIVE_SAFE` 的线程随后进入 `RETURNING`，则在本轮发布 `RUNNING` 前，其返回检查不能通过，根仍被冻结。全部目标停稳后的根扫描因此允许将 `RETURNING` 按 NativeSafe 处理。

**停稳条件与 collector 等待**

停稳判定只读取原子 mode 与 epoch，不能读取仍可能变化的非原子 `managed_segment`、anchor 或根链来辅助判定。`MANAGED_PENDING` 表示新 managed 段尚未激活且已有根保持冻结；其向 `MANAGED` 的激活必须在 world lock 内通过 `RUNNING` 检查。pending 信息由 mode 或停稳后可读的来源状态表达，不再由独立的非原子 `managed_segment` 表达。

| 目标线程状态 | 本轮停稳条件 |
| --- | --- |
| `NATIVE_SAFE` | 已发布的栈段与根被冻结，可视为停稳 |
| `MANAGED_PENDING` | 新 managed 段尚为空，已有根被冻结，可视为停稳 |
| `PARKED` | 根已发布，且 `observed_gc_epoch` 等于本轮 epoch |
| `MANAGED`、`NATIVE_BORROWED`、`NATIVE_SAFE_RETURNING` | 尚未停稳，继续等待 |

同一时刻仅有一个 collector。它在 world lock 内确认 phase 为 `RUNNING` 后取得 collector 独占权、递增 epoch、发布 `STOPPING` 和自身的 collector 状态，并广播 GC 开始通知；已有 collection 时，请求者先按 park 协议协调。独占性由同一锁保护的 phase 转换保证，不再另设与 world lock 反向嵌套的 collector mutex。attach、detach 及注册表成员变更只在持有 world lock 且 phase 为 `RUNNING` 时进行；从 `STOPPING` 到恢复 `RUNNING`，注册表成员、链结构及已注册 thread state 的生命周期保持稳定。

collector 在 world lock 内检查停稳谓词；条件不满足时，使用带有限超时的条件变量等待，等待操作原子地释放 world lock，返回前重新获取该锁。通知、超时和虚假唤醒后均重新检查谓词；NativeSafe 快路径未发通知时也必须能通过超时复查观察到状态变化。等待过程中保留 collector 独占权，但不得持有 heap lock、roots lock 或其他阻止目标线程完成停稳的锁。超时参数属于 runtime 实现细节，按停稳延迟和 CPU 开销选择。

全部目标停稳后，在 world lock 内发布 `COLLECTING`，释放 world lock，再执行既有 STW 扫描与移动流程。此后目标线程的可扫描数据必须持续冻结到恢复 `RUNNING`。完成所有根、引用和分配状态更新并释放 heap/roots 锁后，在 world lock 内恢复 collector 自身状态、释放 collector 独占权，再以 release 或更强的内存序发布 `RUNNING` 并广播；线程的 acquire 观察负责接收 GC 的回写。

测试／基准可通过现有线程测试观测通道，在同一轮发布 `STOPPING`、`COLLECTING`、`RUNNING` 后分别记录 T0、T1、T2。三处观测均紧随 phase 发布、仍持 world lock；已有发布前的 RESUMING 观测点保留原义，不能作为 T2。T1−T0 表示停稳等待，T2−T1 表示 world 停止阶段，T2−T0 表示请求到允许恢复区间；末者不包含随后广播、world 解锁及 OS 调度，也不等于某个 mutator 的实际停顿。线程停止推进到恢复推进的间隔由专用多线程基准另测。观测含回调和时钟读取成本，仅在测试构建启用，不在普通分配、card 标记或无请求 poll 上计时，不增加公共 runtime ABI。

保留 GC 开始、park 确认、GC 结束以及初始化完成/失败的必要通知。初始化等待者可能在 world 为 `RUNNING` 时已处于 `PARKED`，必须在新 GC 开始时被唤醒并确认新 epoch。上一轮的 parker 尚未恢复而下一轮 GC 已开始时，也必须确认新 epoch。`parked_from` 及冻结栈段、根链在一次 park 区间开始时发布，来源状态保留 `MANAGED_PENDING` 的区别；确认新 epoch 或虚假唤醒不得重写这些可扫描记录。

NativeBorrowed 仍可能访问 direct ref，必须在有效 safepoint park 并确认本轮 epoch 后才算停稳。gateway、callback 等低频 managed 入口可继续在 world lock 内完成 `RUNNING` 检查与状态转换；使用完整有锁入口时，不要求再叠加 RETURNING 握手。

native transition 的边界、必要根和 LIFO 检查在所有构建中保留；遍历整个活动 transition 链的查重只用于 debug runtime。release runtime 不为每次调用重放已经成立的整链完整性检查。

runtime 私有的 managed 入口检查同时取得当前线程状态，并在确认 attachment、`MANAGED` mode 与非零 managed depth 后返回该状态。需要操作线程状态的入口复用这个结果，不先后为取址和相同入口检查重复读取 TLS。复用仅覆盖该次同步入口中已经取得的 thread state 地址；mode、epoch、根链和 managed boundary 仍按原协议读写，不跨实际操作省略状态检查。生成代码的 runtime 入口签名、线程状态布局、native 握手及全部精确根契约保持。

`@GCLeaf` 是编译期 C 调用模式，不是新增线程 mode。其实际调用保持 caller 的原状态；从活动 `MANAGED` 调用时，collector 必须继续等待该线程在后续真实 safepoint park，不能提前扫描其栈或移动对象。调用链不得执行 safepoint、park、转换线程状态、回调 Scoop 或进入需要这些动作的 runtime API，且不得阻塞等待其他线程推进。此模式不改变既有 pin、root、指针有效期及用户数据同步契约，也不自动授予其他 runtime 入口的调用资格。

### 3.6 写屏障

向 managed heap 写入 managed reference 后、下一次可能 GC 或发布前，必须执行等价于以下入口的写屏障：

```c
void scoop_rt_gc_write_barrier(const void *destination, size_t bytes);
```

屏障覆盖本次写入的完整范围及其相交的 512-byte cards；零字节为 no-op。屏障有限时间、NoGC、无分配、无 park，并发标记不能丢失已记录的脏状态。

release 可省去已证明年轻对象的初始化屏障：生成分配的实际 TLAB 成功分支保证结果来自当前 nursery，且完成登记不经过 safepoint；在此之后、下一次可能收集或外传前，对该对象的引用初始化不产生 old→young 边。生成代码保留本次 fast／slow 选择，只在成功分支略过卡表查询与标记；collecting slow path、带 release hook、large allocation 及其他未知来源保留完整屏障。字段初始化复用清零分析的未取址局部副本与重赋规则，可沿无条件跳转进入仅有当前前驱的后继；调用、poll、取址、读取对象、外传、条件分支、合流、回边和未知操作结束证明。typed box 分配后紧接的完整 payload 复制，以及数组字面量、clone 和 assembly 在分配后、不调用用户代码且无 safepoint 的内部初始化区间，可使用同一成功事实；数组仍按完整 scan/layout 处理引用与 padding，不能把该事实推广到可调用 initializer 的普通构造循环。未经过 safepoint 的 young→young／young→old 引用随年轻对象扫描处理；晋升及可选 survivor 仍由 collector 按实际边建立 remembered set。此省略不删除字段／元素写入、对象／源引用的根、慢路径恢复或清零，不跨 GC 保存年轻事实，不改变 ABI、原子字段或一般 runtime 后备。

普通非原子引用写入未增加引用边时可以省略本次屏障：新引用为 null，或与写入前同一槽中的引用相同。旧槽读取、完整值写入和条件屏障必须位于同一无 safepoint、发布或用户调用的区间；旧引用仅用于此次比较，不增加根或延长其有效期。原有 old→young 边及 survivor 保留卡仍由既有 remembered set 合同覆盖，不因省略重复标记而清卡。完整值中的非引用分量、itab、tag 与 padding 仍按原语义写入，不能把“引用未变”当作整个值未变。release 当前将动态比较用于 stride 大于 16 bytes、恰有一个 managed leaf 的 typed ArraySet，避免破坏已有单／双字引用数组的批量卡标记；多引用值、原子操作、native 写入、批量复制和初始化保持各自原路径。debug 可保留无条件屏障，全部 profile 的 GC 与 ABI 合同一致。

每个 region／large mapping 有自己的 card storage。按地址 chunk 索引的稀疏 radix page map 覆盖目标 uintptr_t 范围，把地址映射到所属 metadata；不得在 mark／store 热路径线性遍历 region，也不保留单 arena 的预偏置 card-table 合同。新映射先完成 metadata／cards，再 release 发布索引；generated code 用 acquire 读取，内联完成普通单槽寻址和原子置脏，范围屏障逐段覆盖相交索引单元。不得跨 safepoint 缓存可回收 metadata 地址。

当前 64-bit targets 以 64 KiB 为索引 chunk，使用四级、每级 12-bit 的 radix（地址位移依次为 52／40／28／16）。根 `scoop_gc_page_map` 为 4096 个原子指针，其余节点按实际地址分配；末级指向所属 region／large mapping。metadata 的 generated-code prefix 为 `{ uintptr_t base; size_t size; unsigned char *cards; ScoopGcCardGroup *card_groups; }`，偏移分别为 0／8／16／24。索引 load 为 acquire，metadata prefix 发布后不变，card index 为 `(address - base) >> 9`。mutator 运行期间每个 card 字节只表示 clean 0 或 dirty 1；mutator 以 relaxed atomic store 写入 1，多个线程对同一卡重复写入 1 不丢失脏状态。初始化在映射发布前清零，收集后的清零仅在所有 mutator 停稳且 collector 已完成相关扫描／更新的 STW 阶段执行。映射起点及长度按 chunk 对齐，额外映射的首尾页立即解除映射；large mapping 只向 chunk 大小取整，不向普通 region 大小取整。撤销后的空 radix 节点在同一 STW 阶段回收。

每个卡组覆盖固定的 64 张卡，即 32 KiB 地址范围；普通 region 与 large mapping 使用同一表示。组节点为两个 native pointer：偏移 0 的 atomic `next` 和偏移 8 的不可变 owner region，大小 16、对齐 8。`next == null` 表示尚未登记；非空表示登记中、已经登记或从上一轮保留的 remembered 组。置脏后通过 NoGc 入口 `scoop_runtime_record_dirty_card_group(region, card_index)` 完成本组登记；生成代码可先 relaxed 读取 `next`，仅为空时调用该入口。首次线程 CAS 取得节点，再 release 发布到 heap 私有链；自指针结束链，其他写入者只需保留卡字节。登记没有分配、锁、TLS、park 或用户回调，必须在可能 GC／发布前完成。collector 在 STW 后 acquire 取得链，仅枚举这些组；每组内仍按 card 地址扫描。未保留 survivor 的收集在引用更新结束后清零本组 64 字节并重置链接，清空私有链先于 region 撤销。存在 survivor 时，引用更新把仍需记忆的 old→young 槽对应卡暂记为 2；这是只在 STW 内使用的 collector 状态。收尾只保留这些卡并恢复为 1，其他卡清零，按实际保留卡重建组链；后续 mutator 仍仅写 1。full 清空全部组后才撤销 region。节点只保存 metadata 地址，不成为 managed root；所有 profile、native 范围写入与生成代码必须采用相同合同，不能混用仅置脏卡字节的旧 ABI 产物。

同一对象的全部存储属于同一个 region 或 large mapping。在单 basic block、对象引用不变且没有 safepoint、用户调用、原子访问或同步发布的连续普通字段写入之间，codegen 可复用已取得的 immutable base／cards；不同字段仍按实际地址分别计算 card，不假定相邻字段一定属于同一 card。已标记并登记的同一引用槽再次写入可以省去重复 mark。跨上述边界必须丢弃复用状态；保留首次 acquire 查询、必要的 relaxed card store 和卡组登记。卡组登记入口只操作该组和私有链，不使 mapping 或 managed 引用失效。

数组元素写入可以从数组对象基址查询所属 mapping，再按实际元素引用槽计算 card。在实现规范 2.19 的常数有界、无 safepoint 内层中，若目标引用不变，且该查询在每轮所有路径上必经，可以把 acquire 页表与 immutable base／cards 加载移至进入内层的边上。进入边位于外层 poll／引用恢复之后；零次循环、未选择的分支和异常回退不执行新增查询。除下述卡标记合并外，保留每次实际写入后的 card store；不得将 metadata 带过下一个 poll，或把有调用、同步、条件执行的查询或变化目标纳入该复用。

同一有界内层没有用户调用、发布、同步或提前退出，且某个引用槽写入每轮必经、地址在同一 mapping 中保持不变或按已知 8／16 字节步长递增时，可把对应 card store 与卡组登记一起合并到完成内层的唯一退出边。保留全部元素写入和仍必要的 volatile 引用重读，退出边在原 poll／发布之前覆盖本块实际写入触及的全部 cards 和卡组。当前 1～64 次范围每个写入流最多需要三个 relaxed atomic card store；重复端点和卡组登记是幂等的，但不能标记未被本块写入触及的 card。不将 card metadata 或裸引用地址带过 safepoint，不依赖对象是否年轻，不改变 roots 或 heap／接口布局。

用于接口重建的栈槽 volatile 引用重读不触发 GC 或线程同步；按实现规范 2.19 确认其地址仅来自当前函数入口的栈存储后，AArch64 release 可以在保留这些重读的情况下提升独立的 mapping 查询；x86_64 保留原区域选择规则。重读的次数、位置、相互顺序及移动 GC 后的 object／itab 重建保持，查询不能依赖被重读的值；该规则不允许提升 volatile 操作或跨过用户原子、同步、调用与不明地址的 volatile 访存。

重新取得搬迁后的对象引用时，页表各层索引和 card 偏移也必须从当前地址计算；只重新执行 acquire 页表加载、却复用搬迁前的地址整数、位移或掩码，不满足此要求。编译器按实现规范 2.5 保留地址转换的 AS1 operand，直到重定位完成后才降低为整数运算；此过程不增加 safepoint、runtime 入口或根种类。

该要求适用于字段、数组、含引用 aggregate copy、构造、clone、Context 和 Scoop ABI native 写入。发生过可能 GC 的操作后，不能仅凭“刚分配”省略屏障；native root 或 pin 不替代 old→young 引用记录。

runtime 内的装箱与数组克隆使用包含实际 memcpy 和必要范围屏障的完整复制操作。该操作保持静态内联，不为 GC-free 复制增加外部调用，也不在复制和屏障之间引入 safepoint；native 调用者继续遵守上述公开写屏障合同。

`AtomicRef` 的初始化、store、exchange 及成功 CAS 同样写入 managed reference，必须覆盖相应引用槽的卡表；失败 CAS 没有写入，不需要写屏障。计算对象字段地址到原子访问完成，以及引用写入到写屏障完成之间不能插入 safepoint。AtomicRef 对象、expected/new 引用和读取结果跨 safepoint 时遵守普通 root/relocation 契约；collector 在 mutator 停稳后按普通引用槽扫描和回写，移动不改变 CAS 所比较的对象身份。语言内存序由原子指令实现，GC 屏障不能代替 Acquire/Release。

AtomicRef<I> 的原子槽仍只保存 object；返回引用的原子操作在无 safepoint 区间内按静态 I 查询并重建 itab，CAS 不比较表地址。

### 3.7 移动与存储有效期

full collection 按回收收益选择低存活、无 pin 的 region 为 source，把对象集中到未选为 source 的目标 region；高存活／含 pin 区域继续原地复用 Immix line。普通 full 不强制搬迁，显式 moving stress 在目标空间足够时要求 eligible movement。pinned 对象不移动；移动不改变语言对象身份。

首版普通 full 保留存活字节最多的普通 region 作为目标，其他存活量不超过 region 容量四分之一且无 pin 的非空 region 可选为 source。先使用非 source 的旧代空闲 line、空 block 和未使用 block；只有多个 source 可被腾空时才允许增加目标 region。预留完成后，新占用的空 region 数量（含已有空 region 和新映射）必须少于本批 source 数量，使含存活对象的 region 数量实际下降，避免在空闲缓存间往返复制。未取得实际集中收益或目标预留失败时保持非移动回收。该阈值是可调的收集策略，不属于产物 ABI。minor 按年轻 block 选择 source，存活对象按 3.9 进入 survivor 或旧代；显式 moving stress 以 eligible block 为单位验证对象搬迁，不采用普通 full 的收益筛选。

普通 full 只有一个 source region 时，不能通过新占用空 region 获得上述集中收益。目标准备前可用本轮已累计的 region 存活字节计算必要容量上界：只计入非 source、非 large 且已有存活对象的 region，以其全部普通 block 的可用容量减去存活字节。若总上界仍小于 source 的存活量，取消本轮搬迁选择，直接进入正常非移动收尾，不提前扫描目标对象或执行 release。该上界忽略碎片、对齐和不可复用区间，只能否决不可能成功的尝试；通过检查仍须完成实际目标预留及必要回滚。

原存储失效前，所有 roots、存活对象引用和相关运行时引用必须更新到新地址；不能遗留指向已回收副本的 managed reference。对象精确大小与 scan 不得从可能失效的存储猜测。移动或空间预留失败必须保留完整可达图，不能在部分更新后按未移动状态继续执行。

搬迁前预留包括对齐损耗的全部目标空间，失败时缩小 source 集合或非移动回收；minor 晋升不足从完整原图转 full。source 不能又作目标。复制、forwarding 和全部精确引用更新完成后，才允许退休旧副本。

收集器的临时搬迁计划按固定 256 项的 native 分块顺序追加，每项保留原 source／destination、精确大小和所属 block；不反复倍增、复制整份连续数组。预留成功后按相同顺序复制，失败和成功出口都释放全部分块。分块仅存活于原 STW 搬迁阶段，不形成持久缓存、managed root 或新的对象布局，也不改变预留完成前不发布 forwarding／更新 roots 的合同。该表示适用于所有 profile 与 minor／full／stress，使扩大 nursery 不再放大连续计划数组的 native 分配峰值。

全部目标预留成功后，小对象复制直接复用计划中的目标 block、精确大小和已保留范围，不再调用通用分配登记入口重复检查块种类、边界或起点。此时 mutator 已停稳、全部 mark worker 已完成、update worker 尚未发布，只有 coordinator 修改目标元数据；新 block、survivor block 和既有旧代空洞均满足这一阶段的独占条件。复制完成后写入精确 size，合并 occupied／live-line 与对象 mark，更新目标 live bytes，再 release 发布 starts，随后才发布 forwarding 和更新存活清单。occupied／starts 复用 3.1 的单写者 atomic load／OR／store，保留同一字的既有位；line_live／marks 在该独占阶段直接 OR。通用小对象登记入口仅发布未标记的新分配，不保留复制专用的参数或分支；共享 mutator 分配和并行标记的原子读改写保持，large 发布沿原入口，目标预留失败不执行任何复制或目标对象登记。不增加搬迁计划字段、对象元数据、roots 或 ABI，所有 profile 遵守同一顺序。

在规划、预留回退和实际复制全部结束后，以本轮实际完成的搬迁数决定是否更新 roots；零搬迁且没有 survivor 时还跳过 remembered set 与存活对象回写。原地保留 survivor 的 minor 即使没有复制，也须借 remembered set 与年轻存活对象扫描保留下一轮所需的卡；不为此重扫旧堆或新增 roots。非零搬迁仍完整更新相关引用。minor 转 full 使用最终完成轮次的事实，不能复用上轮计数或按计时值判断。各路径都完成必要统计、晋升、release、回收、清卡、nursery／TLAB 重置、临时 pin 解除及线程恢复。

引用更新在本轮稳定的堆视图内，每次访问先解析目标对象的 block／word 位置，并复用该位置判断代际、mark、pin、source 和 forwarding；minor 的旧代目标保持原引用。有 survivor 时，回写后按实际目标副本判断下一轮代际，仅对仍指向 survivor 的 heap 槽定位 owner 并保留卡；roots 不记卡，不增加用户引用存活范围。最终地址与原槽值相同时不写回。需要扫描搬迁后的对象时，只有地址变化才独立解析目标副本的位置，保留对象起点、存活性、forwarding 和当前副本检查；位置不跨 metadata 退休边界保存，不建立持久地址缓存。

普通 runtime 的 mark 与引用更新遇到堆外引用时直接保留：合法的堆外 managed 对象已在映像登记边界验证为无可移动引用的 immortal，不必逐槽重复查询 immortal 地址表。该查询只承担非法堆外引用的诊断，在支持 GC stress 的 runtime（`SCOOP_ENABLE_GC_STRESS=1`）或显式堆验证构建（`SCOOP_GC_VERIFY_HEAP=1`）中保留，与应用 debug／release profile 无关。堆内地址仍须是当前有效的对象起点；映像登记及 pin／handle 等边界所需的检查保持不变。

普通 full 复用已有旧代 block 的空洞时，依据本轮 mark／live-line 结果先释放目标 block 中的死亡对象并移除其起点，保留原有存活对象的 mark。预留回滚只退还新激活的目标 block，不退休既有存活对象；已完成的死亡对象 release 不重复执行。复制到既有 block 的对象与原有存活对象都参与完整引用回写。

small block 的 sweep 与压缩目标准备按 starts bitmap 的置位枚举真实对象起点，跳过零 bitmap word；按地址递增访问，保留各对象的 mark／pin／forwarding 判断、精确 size、release-ready 与 line 重建规则。会清除 starts 的循环先取得当前 bitmap word 的快照，避免退休对象影响同 word 中尚未处理的起点；不维护额外对象列表。

无存活对象覆盖的完整 OS page 可 discard，保留 side metadata；完全空闲 region 从分配结构及地址索引撤销后 unmap。撤销仅在 STW、所有 GC worker 结束且 TLAB／缓存失效后进行，不把 free-list 回收计作 OS 归还。discard 后复用必须重新清零；跨页活对象覆盖的页不可 discard。普通策略保留一个空 region，其他 full 后释放。记录 discard 成功量、unmapped bytes 和当前／峰值 RSS，不能把建议性 discard 等同于立即 RSS 下降。

首版在普通 full 结束时合并已使用 block 中不覆盖任何存活 line 的整页，通过 ThreadVmOps 调用平台 `madvise(MADV_DONTNEED)`；失败保留映射并在后续 full 重试，不影响可达图。block 激活、空洞再次分配或对象退休时才重新请求 discard，不在普通对象分配热路径增加共享计数。统计分别给出成功／失败调用数、成功建议的累计字节和解除映射的累计字节，重复建议不代表新增物理内存归还。RSS 是诊断时采样的进程当前／历史峰值，采样不可用时对应字段为零；既不作为收集正确性条件，也不与 managed mapping 字节等同。

### 3.8 GC-free release hook

release hook 仅属于符合语言规范 9.1.6 的 FixedObject 类型。编译器为该 exact type 生成 ReleaseHook callable 并绑定 descriptor 入口；GC 直接使用已登记的入口，不在启动时重新编码 body ID、计算 hash 或比较另一份 callable 地址。

正常 collection 决定回收 ready 对象时，在 storage 失效前清除 ready 并尝试调用一次 hook。未完成构造的对象不调用；移动副本和旧地址回收不额外调用。ready 的 release/acquire 配对保证构造写入可见，显式 close 的 inert-state 写入也必须在 park 前完成；hook 不为有数据竞争的程序补充同步。

hook 在 world 停止期间执行，不得分配 managed 对象、发起 GC、访问 root/handle/pin/thread API、等待停顿的 mutator、回调 Scoop 或跨出 foreign unwind。允许的 Scoop helper 必须满足 NoGc/release-safe 效果，直接 C extern 遵守 raw leaf 契约；不能保留临时地址或重新进入 runtime。唯一例外是 3.1 的 `scoop_rt_gc_adjust_pressure` 提示：它保持同一 GC-free leaf 合同，不读取 owner 或线程状态，也不触发收集。

符合语言规范 9.1.6、13.4.2 的直接 C extern 可捕获 errno。生成的 bridge 在原调用前清零 libc errno、返回后立即保存为本次调用的局部整数，与 native 结果一起交回普通 GC-free tuple；不访问 Scoop TLS 或 thread runtime，也不改变 collector 状态。hook 中的新捕获不会覆盖 mutator 已取得的错误值，collector 不为此保存或恢复线程级 last-error。其他 release-safe、`ReleaseValue` 与一般 TLS 访问限制继续适用。

release 是 best effort，不保证触发时间、对象间顺序、执行线程、native 成功或退出时调用；abort、fault 或不返回的调用没有重试保证。shutdown 不补调 live、未收集或 unready 对象的 hook。确定性释放由显式 close/release 与 try/finally 保证。

### 3.9 分代收集

minor collection 处理年轻代，依据完整精确根与旧代脏区追踪年轻可达对象；其正常工作不遍历整个旧代图来弥补缺失屏障。full collection 处理两代；显式 `gc.collect()` 请求 full collection。

年轻 block 在 refill／stress 分配取得块时加入 heap 私有清单；普通对象的 bump／登记不维护额外清单。minor 的标记准备、worker 计数汇总和搬迁 source 选择只枚举这份清单，收尾另处理本轮新建的 evacuation target。晋升或退役其余年轻块后，把本轮原地保留或复制得到的 survivor 留在年轻清单中，并结束 target 成员关系；full 仍按全堆枚举，收尾晋升全部年轻存活对象并清空两份清单。目标预留失败时只撤销本轮新建目标和选定 source，minor 转 full 前保持完整原图。活动年轻块／目标块与空闲块按互斥状态复用 side metadata 的链接；退役前取得后继，不能从已经交还空闲链的字段继续清单遍历。block 元数据大小及原热字段偏移保持，不增加对象头、roots、TLAB 或 generated-code prefix，也不改变产物 ABI。

普通运行提供一个可选 survivor 阶段：`SCOOP_GC_SURVIVOR=1` 开启，默认 0；其他值在初始化时报错。首次 minor 存活的普通无 pin 小对象可保留为一个阶段的 survivor，下一次 minor 晋升。每轮选择时，survivor 总 block 容量以本轮 nursery 容量的四分之一为上限；其容量计入下一轮 nursery 占用，收尾后的 nursery 容量仍按 3.1 调整。优先在原 source 选择遍历中保留存活字节不少于 block 可用容量一半的首次存活块，其他首次存活对象可复制到剩余配额内的同龄 survivor block，配额用完后直接晋升。代龄及本轮保留状态只占 block 私有 padding，survivor block 不混入新分配或旧代 free run，不增加对象头、逐对象年龄或分配快路径操作。已有 survivor block 达到同一密度时原地晋升，否则沿既有搬迁路径集中到旧代；含 pin 的 block 原地晋升，large／release-hook 对象仍预晋升。full、moving／minor stress 和 full-only 不保留 survivor。

原年轻块的 generation 在 mark 与 relocation 期间保持，以私有保留标志区分下一轮年轻／旧代；source 的全部活对象完成预留后才发布任何复制或 forwarding，survivor 空间不足可改用旧代目标，整体预留失败撤销本轮保留标志并从原图转 full。借原 remembered 和存活对象回写补记旧对象及本轮晋升对象到 survivor 的卡，不重扫完整旧堆；原地保留的对象数复用本来就要访问的本轮年轻存活清单累计，不增加对象遍历。对象计数须从上轮已计年轻存活中扣除旧 survivor 数，再加入本轮实际年轻存活，避免重复计数；晋升量只包含进入旧代的字节。

具有未使用 block 的普通 region 单独记录，分配／搬迁预留不为寻找一个可用 block 枚举所有已满 region；full 可以按原 source 排除规则选择并重建该清单。重建保持 region 目录的选择顺序，保留的空 region 如仍有未使用 block 则优先复用。大对象和带 release hook 的对象仍预晋升，minor 不产生需要归还的大对象 mapping，因而只在 full 收尾执行 region 回收／VM 归还枚举。卡表的扫描与清零按 3.6 仅枚举实际登记的卡组，可用 `memchr` 跳过组内连续清卡；minor 不为卡表或 full 专用的 region 标志枚举全部 region。每组仍只扫描已发布旧对象覆盖的真实脏卡。full 沿原全堆规则标记和回收，收尾同样先结束脏组成员关系再撤销 mapping。

remembered 枚举在读取大对象 mapping 的卡表前，使用已发布旧对象的既有 TD 判断是否含引用；`object_scan == null` 时整份卡表无需遍历。mark 与 relocation 复用这一枚举规则；真正含引用的大对象仍按脏卡扫描和回写。无引用对象自身仍由正常 roots／mark 保活，并参加统计、搬迁与回收；此优化不改变卡表分配、写屏障或收尾清零，也不重新验证静态 TD。

minor 和 full 共享线程、root、pin、handle、Context、callback、冻结栈段及精确扫描契约。晋升或移动在原存储失效前更新全部相关引用；pin 暴露的地址不变。年轻区复用不能遗漏存活引用或 release hook，旧代死对象可以留到 full collection。

普通 minor 在引用回写与必要验证完成后，可以整块退役全死或已完成搬迁的年轻 small block。搬迁 source 的选择已经排除含 pin 的 block；release-hook 对象预晋升，不进入年轻快速路径。累计原年轻存活字节并扣除本轮 survivor 字节得到实际晋升统计，再按既有 block 退役流程释放 forwarding、撤销活动状态并交还空闲链；不逐对象清理已经失效的源存储。block 再次激活时清空对象起点、大小、mark／pin／scan 和 line 元数据，不能让旧起点随复用重新可见。evacuation target 和仍含存活 pinned 对象的 block 正常收尾，moving stress 保留逐对象 poison 与 quarantine；full 的 release 处理不因此改变。

分配压力可触发 minor 或 full；晋升空间不足时必须从完整对象图进入 full 或明确的分配失败出口。分代策略和容量不改变语言合法性、对象 ABI 或 FFI 保活义务。

STW 后 coordinator 可使用内部常驻 pthread worker 并行 mark，worker 不登记为 mutator，也不执行用户代码。小 minor 可只使用 coordinator；单 worker 与多 worker 共用同一参数化 collector。对象／大对象 mark 使用原子首次置位，只有首次成功者登记存活对象和扫描任务；line_live 原子 OR，计数和存活清单按 worker 汇总。

collector 持有 heap／roots 的独占阶段后，发布不可变的 region、对象起点、精确大小、pin 和 immortal 对象视图。marker 使用明确的 STW reader／原子 marker 接口，不要求各 worker 重新取得 coordinator 持有的锁。每个参与 block 在本轮取得临时统计索引，worker 私有计数按该索引归并；它不是持久对象身份，也不进入产物或 generated-code ABI。存活清单以稳定的分块存储保留对象指针，任务发布后不得由扩容使其失效。

大引用数组按元素区间分任务，普通对象分批；GC-free 数组没有引用扫描任务。全局未完成任务数同时覆盖队列、正在扫描及尚在发布的子任务，子任务先计数再发布，父任务发布完子任务后才完成；队列暂时为空或窃取失败不表示终止。worker 创建失败在本轮开始前退回串行，队列分配失败走明确 runtime 失败出口，不能丢失任务。

roots／remembered 扫描先由 coordinator 发布初始工作，随后参与同一任务循环。普通对象每个任务最多扫描 64 个，可以在该批次内继续处理本 worker 刚发现的后继，避免长链每个对象都经过队列；大数组每个任务最多 1024 个元素，嵌套值仍执行已有精确 scan。未扫描的剩余子工作在当前任务完成前发布。仅一个在途父任务和一个后继时无需唤醒其他 worker，最后一项完成必须唤醒终止等待者；这不改变全局未完成任务数的含义。首次成功标记但不含引用的对象仍进入存活清单与统计，无需扫描任务。本轮候选 block 总容量不足 4 MiB 时默认仅使用 coordinator；minor 使用实际年轻 block 容量，full 使用活跃堆容量，其余按在线 CPU 数选择最多 4 个 worker；在线 CPU 数通过私有 ThreadVmOps 查询。`SCOOP_GC_WORKERS=1..8` 可显式选择本轮 worker 数以验证同义性和测量，worker 数包含 coordinator，属于 runtime 策略参数，不影响程序或产物合法性。

mark 结束后复用首次标记的存活对象集合规划搬迁；实际有搬迁时，使用该集合更新存活副本的出站引用，不在 full 更新阶段再次用 worklist 发现可达图。存活清单的分块和条目地址在本轮保持稳定；搬迁计划保存对应清单条目的地址，替代单独保存 source 指针，不增加每项计划或清单的大小。全体目标预留成功后，coordinator 依次复制对象、登记目标元数据、发布原对象的 forwarding，再把对应清单条目改为当前副本。预留失败不得修改任何清单条目；未搬迁和 pinned 对象继续保留原地址。无引用对象的条目也随实际搬迁更新。该修改发生在全部 mark worker 结束后、任何 update worker 发布之前，不新增 managed roots 或延长对象存活。

此时 full 的 roots 全部更新；minor 更新精确 roots、年轻集合及 remembered set。对象字段和 roots 中的旧引用仍经正常 forwarding 回写，不能因存活清单已经更新而跳过它们。没有搬迁时按 3.7 跳过 relocation。复制、release hook、reclaim 和 VM 归还仍由 coordinator 执行；shutdown 停止并 join worker。

引用回写复用本轮 mark 的 worker pool。coordinator 先串行更新 roots 与 minor remembered slots，再直接使用存活清单中的当前地址；首次标记已保证条目唯一，预留／复制阶段已保证条目指向当前副本，因此普通构建不为清单条目重新定位对象元数据、检查存活身份或登记 relocation 覆盖状态。普通对象按至多 64 个分批、大引用数组按至多 1024 个完整元素拆成互不重叠的槽区间。任务复用稳定的存活清单和既有精确 scan 解释器，forwarding、对象和扫描元数据在整个 update 阶段保持稳定；worker 只读这些元数据并更新各自的引用槽，不重新发现存活对象、修改 roots、执行用户代码或取得 coordinator 持有的 heap／roots 锁。根与对象槽即使别名，也已由前述串行边界隔开。全部 update worker 完成后才允许验证、退休存活清单、释放 source／metadata、执行 release、reclaim、VM 归还及恢复 mutator。单 worker 直接使用相同的当前地址和引用槽转发规则串行更新，不先建立任务队列；创建 worker 失败沿用既有串行回退。

搬迁覆盖登记仅编入启用 GC stress 支持或显式 `SCOOP_GC_VERIFY_HEAP` 的 runtime。此时 coordinator 在发布 update 任务前定位元数据并 claim：检查 collection、对象起点、已标记、当前副本和重复扫描；普通 `scanned` bitmap 不由多个 worker 并发置位。未编入这两类验证能力时，覆盖检查函数、调用、专用 bitmap／large 标志及其分配、清零和释放均由预处理排除，不以运行期开关保留。该状态属于 collector 私有侧元数据，不改变对象、根、scan、forwarding、generated-code ABI 或真实引用槽的合法性检查。

存活性由 mark 与搬迁关系决定，不由 relocation 的 scanned 状态定义。显式 GC heap 验证和 moving stress 在零搬迁时仍检查根、当前存活对象及引用合法性，但不要求或伪造 relocation 覆盖状态；有搬迁时对含引用对象的当前副本保留相应覆盖检查。object_scan 为 null 的无引用对象仍参与存活清单、统计、规划、搬迁和回收，但不为出站引用扫描再解析位置、claim scanned 状态或调用扫描解释器；指向它们的引用仍正常更新。未执行 update 的轮次不累计 update 扫描时间。存活清单保留至规划、必要更新和验证结束。

诊断分别记录停稳等待、roots、remembered、mark、plan、copy、reference update、reclaim 与 VM 归还的墙钟时间，以及各 marker 的线程 CPU／标记对象数、数组任务和窃取次数。mark 阶段加速与总停顿分开报告；root producer 的工作计入 roots／remembered 阶段，不把重叠时间相加成停顿。所有阶段计数在 worker 完成后由 coordinator 归并，release hook 不在 marker 上执行。

既有 `stop_wait_ns` 从调用 `scoop_thread_begin_collection` 前计到成功返回，包含取得 collector 权限的开销；`pause_ns` 从返回后计到 `record_pause`，不包含此前等待和随后解锁／RUNNING 发布。保留这两个字段的含义，与 3.5 的三个发布边界分别报告。阶段累计值的前后差分保持 plan 扣除 copy、reclaim 扣除 VM 归还的口径；阶段之和不能替代整体停顿计时。实际搬迁数使用本轮完成的对象搬迁计数，不能从 `copy_ns == 0` 推断。

region 诊断补充 managed region 映射成功的累计次数、当前可复用的空 ordinary region 数，以及上次普通 full 成功搬迁的 source／target region 数和因 pin 被排除的稀疏候选数。无搬迁或预留回滚的 source／target 数为零；minor／显式 moving stress 不覆盖上次普通 full 的策略记录。统计复用选择和预留阶段已有事实，不能为了诊断重放标记或搬迁。

## 4. Scoop ABI FFI runtime functions

本章规定语言规范第 14 章对应的 runtime 能力。managed generated-code、native-borrowed、runtime internal 与 foreign-callback 入口具有明确的调用角色，入口不从裸 C 栈猜测 caller kind。

Scoop ABI 的 Boolean 参数／结果遵守实现规范 2.4 的 `i1 zeroext` 约定，与本章 C `_Bool` 原型的规范 0／1 值一致。编译器在调用边界生成所需扩展；runtime 不为来自错误调用约定的非规范值增加容错分支。Boolean 对象存储及现有 C 函数原型保持不变。

### 4.1 对象分配

- `scoop_runtime_alloc_pinned(type_desc, size) -> ScoopObjectHeader*` 分配即固定，返回前 pin 生效；具体 API 明确 unpin 所有权。
- `scoop_runtime_alloc(type_desc, size) -> handle` 返回可移动的 GC handle，用于跨 native root frame 保活。
- 只在本次 native 调用内存活的引用可以保存在 native root slot；FFI 没有 managed stackmap，不能绕过 native-borrowed 入口协议。

可能 GC 的 native 分配入口在首个此类动作前取得有效 transition anchor 与完整 root-frame 链。collection 扫描 caller roots、native DirectSlots/RecursiveRegion 及被冻结的 managed 栈段；返回后调用者从更新后的 slot 或 region 重读，不捕获 C return address 代替 managed anchor。

### 4.2 Native root、pin 与 handle 操作

native root frame 为封闭的 DirectSlots 或 RecursiveRegion。前者通过 `push_native_roots(slots, count)` / `pop_native_roots()` 等价能力登记一组 direct-ref slots；后者通过 `push_native_region_root(base, byte_extent, scan)` / `pop_native_region_root()` 等价能力登记 non-null 稳定 base、非零 extent 与 canonical nonempty scan。两者共享严格 LIFO 链，不能以 count=0 或 null scan 伪装另一种 frame。

push/pop 本身不得分配、GC、park 或回调。frame 在可能握手前完整发布，collector 在可扫描状态后观察；pop 只在入口返回且更新后的值已重读之后发生。region 的 scan、alignment 与 extent 必须匹配实际 native 存储。typed boxing 的源 place 由既有 statepoint live set 保活，不重复登记 native region。

generated managed caller 使用 NoGC、nounwind、有限时间的私有 leaf root 操作，不建立额外 native transition；所有非 fatal 出口必须平衡撤销。native 代码跨 safepoint 不得缓存 managed leaf 的旧副本。

`scoop_runtime_pin(ptr)` / `scoop_runtime_unpin(ptr)` 操作 pin 状态；`scoop_runtime_pin_handle(handle) -> ptr` 解析并固定当前对象。handle 操作校验 live slot/generation，非法值按 4.4 处理。

语言侧 PinnedPtr/GcHandle 仍为实际单字段 struct，普通 Scoop 调用使用 aggregate ABI；只有 C 边界采用 UInt64 透明表示。

### 4.3 Managed callback

callback 注册直接接收 ordinary、非 suspend closure，返回 GC-free opaque cookie。cookie 非零，可按目标契约在 uintptr_t/void* 间往返，但绝不解引用；slot 复用必须能检测 stale cookie。

注册持有 closure、binding snapshot 与首个失败的 managed 保活引用，以及完整 adapter/signature、mode、owner/active 数量和状态。整数 ABI 的 mode 为 Reusable=0、OneShot=1；state 为 Registered=0、Active=1、Completed=2、Failed=3。未知值为 fatal ABI error，这些 wire codes 不直接等于语言 enum tag。

静态 C trampoline 按 `(canonical C signature, context index)` 标识。context index 是 zero-based u32，必须指向真实签名中的 Ptr<Unit> 参数；其余参数与结果满足 C-FFI-safe。trampoline 去掉 cookie 参数，把剩余值传入有类型的 args/result storage，再进入 callback runtime。不能用未类型化 varargs 猜测 managed ABI。

invocation 执行 attach-if-needed、enter managed、保活 closure/snapshot、调用 typed adapter、leave managed，完成 token 状态和保活引用的释放后再 detach-if-owned。临时 attachment 覆盖整个 runtime 收尾，不能在仍需释放 GC handles 时让 shutdown 观察到线程与 callback 均已清空。adapter 在入口 poll 后建立独立 TaskContext，调用期间可分配和 GC；所有正常/异常出口先恢复 previous Context，再返回 C。

普通 cookie 值复制不增加 owner。retain/release 显式管理 ownership；owner 与 active lease 均为零后释放保活引用并回收 slot。Reusable 每次只增减 active lease；OneShot 原子 claim 并消费一份 worker ownership。需要读取完成/失败状态的 observer 必须预先 retain，创建失败时释放未转移 ownership。

adapter 在返回 C 前捕获全部 Scoop 异常、保存原 managed Throwable 并结束 native catch；token 以 first-wins 保存失败，trampoline 按真实 C 返回类型返回全零值。observer 可在 API 同步点之后重新抛出。invalid/stale cookie、签名不匹配、OneShot 重复调用和 shutdown 后调用均 fatal。

任意 closure 导出要求 native API 有显式 context/user-data 槽。没有该槽时只允许语言规范规定的静态 FunPtr。

### 4.4 错误处理

无法分配 runtime/GC 元数据、非法或 stale handle、root LIFO 破坏、非法线程转换、callback cookie/signature 错误、无效 ABI 输入和 shutdown 后重新进入，均打印稳定诊断并终止。普通 callback 抛出的 Scoop 异常按 4.3 转换，不属于内部 ABI 错误。

### 4.5 线程状态

普通 C ABI outbound 默认使用 NativeSafe，Scoop ABI outbound 使用 NativeBorrowed；NoGC 声明不取消 caller 的协调与保活义务。NativeSafe 的常见进入/返回使用 3.5 的无锁协议，实际 C 调用期间不持有 world lock。foreign callback attach 后通过独立 boundary 进入 managed；返回 managed 与 callback 进入均不能绕过当前 GC 的协调。

显式 `@GCLeaf` C ABI outbound 按 3.5 保持原线程状态，不执行 NativeSafe/RETURNING 握手，不额外发布或撤销 caller roots。只有语言规范 13.4.1 的完整调用契约允许这种模式；GC-free 参数或 `@NoGC` 本身不能推导它。编译器按实现规范 2.4～2.5 选择直接 C ABI 调用或 storage bridge，两者保持相同的 native 签名和 GC 契约；NativeSafe 复用直接调用时仍须完成原有协调与保活。ABI 路径选择不增加 runtime 状态或运行时判断。release hook 的 raw leaf 调用仍按 3.8 单独检查，`@GCLeaf` 不放宽 release-safe 限制。

errno 捕获遵守语言规范 13.4.2 和实现规范 2.4～2.5：bridge 在实际 C 调用前清零目标 libc errno，返回后立即复制到 native 局部整数，再完成结果传递。NativeSafe 的捕获发生在返回握手之前；GcLeaf 和 release 的捕获保持各自原状态。结果使用本次调用独立且地址稳定的 GC-free storage 和整数返回，不修改冻结根或增加 GC 扫描记录。后续状态切换、GC、其他捕获调用及协程迁移不覆盖已经取得的值；runtime 不提供 `lastErrno` 槽位、捕获 setter/getter 或为此增加全局锁、attachment 和 collector 保存/恢复协议。libc 的 errno 本身在之后仍可变化。

边界可嵌套且严格 LIFO；每次恢复 managed 前按 3.5 协调当前 phase/epoch，并按 3.2、4.2 使用回写后的引用。detach 只能发生在 attachment owner 已退出全部相关 frames 后；RETURNING 归入 NativeSafe 的诊断/统计分类不放宽此条件。

### 4.6 Initialization coordinator

coordinator 只接收已登记的 InitializationUnitDescriptor。`scoop_rt_init_enter(descriptor)` 在锁的 acquire 同步下观察 cell，返回 RunInitializer、Ready、Failed(rooted Throwable) 或 Cycle(stable unit path)。编译器按 2.7 生成的就绪快读直接使用 state 的 acquire load；普通 extern 不按符号名替换，其 runtime 后备仍执行原入口检查。无环等待兼容 GC 握手。

初始化等待者以 `PARKED` 发布已冻结的根，并按 3.5 在 GC 开始通知后确认当前 epoch。只有初始化已有结果且 world 为 `RUNNING` 时才能退出等待并恢复 managed；GC 通知或虚假唤醒都不能单独作为恢复条件。

RunInitializer 执行普通 managed initializer，完成 storage/published root 后调用 `scoop_rt_init_succeed`，以 release 发布 Initialized 并唤醒 waiter。Ready 重新读取已登记 storage。

在 active catch 内取得原 managed Throwable，再经 `scoop_rt_init_fail` 写入专用 failure root、发布 Failed、移除等待关系并唤醒 waiter。重新抛出从该 managed 对象建立新的 native record；cell 不保存 native unwind record。

Cycle 的 path 在复制为 managed String 之前保持稳定，随后调用已解析的 core thrower。runtime 不按名称查询 core 类型或构造器。invalid transition、未登记 descriptor 或非 winner 的完成操作为 fatal ABI error。

## 5. 异常

### 5.1 Exception record 与抛出

异常 ABI 使用 Itanium Level I `_Unwind_*` 接口。每条私有 exception record 含恰好一个满足目标对齐的 `_Unwind_Exception`，并通过普通 GcHandle 保活被抛出的原 managed Throwable；record 内部布局不属于 generated-code ABI，只能经 runtime 入口转换。Windows 使用 LLVM libunwind 的 SEH 实现及其配套 `unwind.h`（`__SEH__` 配置），私有字段数量与 Unix 不同；不得复制 Unix 的 record 定义或混用头文件和库。

`exception_class = 0x53434f4f50000000`。`scoop_rt_throw(object)` 要求 FixedObject shape，在可能展开或 GC 前创建原对象的强 handle，再调用 `_Unwind_RaiseException`。异常对象遵守普通引用语义，不复制、不 pin，也不能直接作为 unwind record；GC 搬迁时更新 handle 中的引用。不再为异常建立堆外对象 root，collector 只处理既有引用槽、区域、handle 和 immortal 对象。

`exception_cleanup` 是释放 handle 和 record 的最终出口，只执行一次。unwinder 私有字段只由 unwind library 使用。`_Unwind_RaiseException` 返回表示无 handler 或 unwind 失败，必须在 handle 仍有效时取得原对象并打印诊断，再清理并终止。

### 5.2 Personality 与 landing pad

`scoop_eh_personality` 消费 LLVM 22.1 Itanium LSDA，只接受 Scoop catch-all 与 cleanup；源码 catch 类型在 landing pad 后按普通类型关系匹配，不使用 C++ RTTI。

LPStart 为 `DW_EH_PE_omit (0xff)`，call-site 编码为 ULEB128。有 catch-all 时，Darwin／Linux 的 TType 为 `0x9b`（4-byte entry），Windows x64 为 `DW_EH_PE_absptr (0x00)`（8-byte entry）；两者都只接受一个 null type entry。TType 为 `0xff` 时无 type-table offset，所有 call-site action 为零，有效表在 call-site 表结束。

Windows 的机器入口 `scoop_eh_personality_seh` 使用系统 SEH 的四参数签名，通过 `_GCC_specific_handler` 调用同一个 Level-I `scoop_eh_personality` 与 LSDA decoder。LLVM `invoke`／`landingpad` 保留该入口，`.pdata`／`.xdata` 提供系统 frame 信息。exception／selector 的机器寄存器来自该目标的 `__builtin_eh_return_data_regno`，不复用 SysV 的寄存器假设。W0 的独立实验先验证实际发射与运行，再开放正式 target，见实现规范 1。

search phase 只由 Scoop exception 的 catch-all 选定 handler；cleanup-only 不终止搜索。cleanup phase 按目标 ABI 设置 exception/selector 寄存器与 landing-pad IP。`resume` 使用 `_Unwind_Resume`，不能代替源码 rethrow。

version、flags、encoding、call-site range 与 action chain 必须有效。未知 encoding、typed filter、损坏表或非 Scoop exception 进入 Scoop EH 区域，均终止；不能交给 BeginCatch 当作 managed 对象。

### 5.3 Catch 与重抛

`scoop_rt_begin_catch(raw)` 验证 exception class、压入当前线程 catch 栈并从 handle 取得原 managed Throwable 的当前引用。同一 active record 不得重复 BeginCatch。

源码 catch 变量直接绑定该对象，可返回、存储或捕获；与 throw 实参保持引用身份、动态类型和可变字段的别名关系。该引用跨 safepoint 时使用普通 managed roots 和 relocation；不得把首次取得的地址当作稳定 native payload。

每条正常离开 handler 的路径调用一次 `scoop_rt_end_catch()`。普通 record 被删除；rethrow record 只弹出 catch 状态并恢复为 in-flight。begin/end/rethrow 不分配 managed 对象、不 GC，也不向源码暴露 record 地址。

`scoop_rt_rethrow()` 要求 active catch，标记并重新 raise 同一个 unwind record，原 handler cleanup 不删除它。handler 抛出另一异常时，先结束旧 catch，再传播新 record。

native exception record 不能跨线程共享或跨挂起保存；可以保存的是 managed Throwable。

### 5.4 保存与发布异常

保存异常不分配或复制 Throwable。普通 catch、初始化 failure root、callback observer、root gateway 与协程 frame 都保存原对象的 managed 引用；结束 native catch 只释放 record 自己的 handle，不使已被普通 roots 保活的对象失效。

跨挂起或线程发布前必须保存 managed 引用并结束 native catch，不能保存 native record。后续传播重新调用 throw，为同一个对象建立新 record；同步 rethrow 保持原 record。不存在额外的异常物化 runtime API。

### 5.5 依赖与 FFI 边界

生产异常 ABI 只依赖 Itanium Level I，不依赖 C++ EH、RTTI 或 terminate。target profile 明确选择兼容 unwind provider；Linux CRT 的普通退出清理符号不视为 C++ EH。

启用 native C++ 的程序可以在 C++ 一侧使用配套的 C++ EH／RTTI／ABI 运行库，不改变 Scoop 异常对象和 personality 的合同。最终链接须协调 Level-I unwind provider：Linux GNU C++ 配置使用配套 libgcc_s，Darwin 使用所选系统 provider；Windows Scoop 的 `_Unwind_*` 仅由相应环境的 LLVM libunwind 提供。MSVC C++ EH 仍由 VCRuntime 提供，两种语言 handler 共用系统 SEH，不删除 native C++ 的运行库，也不再引入第二套 `_Unwind_*`。具体链接选择见实现规范 2.8。C++ 异常必须在 native 一侧捕获处理后再返回 Scoop；Scoop 不接管 C++ 对象的异常或析构语义。

除零、强制类型转换、Option unwrap、数组越界等语言异常，由 managed 代码构造实际 core 异常并抛出。native helper 不得把源码异常藏在穿越 FFI frame 的 unwind 中。

异常不得穿越 C ABI frame，违反属于未定义行为；Scoop ABI native callee 不得向 managed caller 展开，违反时终止。managed callback 和 startup gateway 在返回 C 前完成捕获、保存原 managed 引用与 EndCatch。

## 6. 核心类型的运行时后备

后备遵守语言规范的普通 core 声明与实际 exact type，涉及分配时登记 native roots，GC 后重新取得地址。

标准输出后备为普通 C ABI `scoop_rt_stdout_write(const uint8_t *bytes, int64_t length)`、`scoop_rt_stderr_write(const uint8_t *bytes, int64_t length)` 和 `scoop_rt_flush_stdout(void)`，均返回 void。调用方提供非负长度和本次调用期间有效、稳定的字节区间；core 的 String 包装使用作用域借用。实际操作使用 stdout/stderr 的 stdio 缓冲；caller 按普通 C FFI 进入 NativeSafe，后备不访问可移动的 managed 引用，也不回调 Scoop。语言接口及缓冲、并发和错误结果规则见语言规范 14.4。

String 的公开行为、hash、UTF-8 扫描和编码由普通 core helper 实现；已知表示的读取、分配及复制沿 2.4 的 typed 操作生成。C 仅保留实际消费者需要的共享比较、拼接与有界读取 leaf。普通字符定位 helper 返回 Option<Char> 或 Option<(Long, Long)>，core 对 None 抛出 IndexOutOfBoundsException；内容与已验证边界未变化时直接复用，不调用另一套 C 校验。旧字符定位、解码及复制入口和专用聚合 ABI shim 退役。

JSON／CLI 浮点解析后备返回的 `Option<Float>`／`Option<Double>` 同样为 16-byte tagged 值，固定 tag 与 payload bits 使用两个整数返回分量。平台适配必须按 bits 传递 payload，不能套用 C 的混合整数／浮点 struct 返回分类。这些适配由实际的 scoop.json／scoop.cli extern 声明消费。

普通 core 字节解码 helper 为语言规范 11.4 的 `fromUtf8`、`fromUtf8OrNone` 和 `fromUtf8Lossy` 提供同一套 UTF-8 规则。严格路径的结果明确区分成功 String 与首个非法子序列的零基字节偏移，并通过普通 Scoop ABI 返回；core 分别将失败转换为 CharacterCodingException 或 None，不对相同内容再做一遍独立校验。lossy 路径按 maximal subpart 消费非法输入，每段写入一个 U+FFFD，继续处理失配处的后续字节，不能遗漏合法后缀；空输入、U+0000 与合法 U+FFFD 按原内容保留。两种路径均不依赖 locale。

解码和复制只读取给定长度，输入属于 managed 对象时遵守 root/pin/relocation 契约；原生指针的可读性、稳定性和生命周期由 unsafe 调用者保证。结果复制到 String 自有存储，发布前完成全部初始化。实际输出长度、临时存储与分配的溢出按既有分配失败处理，不伪装为 UTF-8 错误或 None。实现复用同一单步校验正文及已验证的不变内容。

严格解码 helper 接受非零字节指针和非负 Long 长度，返回普通 `(String?, Long)`：成功为 `(Some(string), -1L)`，失败为 `(None, byteOffset)`。该 tuple 的 storage 为 16 bytes、alignment 8，Option<String> 使用 nullable managed pointer niche，字段 offsets 为 0/8；按完整 Scoop ABI 传递。core 的安全 Array 重载通过 3.4 的作用域借用调用同一 helper，pointer 重载在进入 helper 前检查负长度；不为这些函数增加编译器异常角色。lossy 复用有边界的单步校验器，先计算实际输出长度，再分配和转换，整个过程保持输入稳定。

Option<Char> 的 storage 为 16 bytes、alignment 8，tag offset 0、Char offset 8；Option<(Long, Long)> 为 24 bytes、alignment 8，tag offset 0、Long offsets 8/16。实际 core 的 Some/None tag 为 0/1。前者按 GC-free 小值分类使用 DirectParts，后者仍间接返回：Darwin/AArch64 经 x8；Linux/amd64 经 RDI，并在 RAX 返回同一地址。这些 helper 使用普通 Scoop 调用发射，无专用 native shim。

数组后备使用显式目标 TD 分配和浅复制，保持新对象头、logical size 和 canonical padding。ZST 不复制 payload；不能从来源名称或布局猜测目标 exact type。源码 bounds check 与异常构造由 managed 代码承担，受检 helper 收到越界 index 属于内部 invariant error。

`scoop_rt_string_join_parts(const ScoopArray *storage, int64_t part_count)` 消费 MutableArray<MaybeUninit<String>> 的有效前缀并返回 String，使用 Managed native contract。prefix 范围有效，每项由 ArrayList 保证为已初始化 String，元素身份为实际 wrapper application；字节总数 checked 求和。分配后从更新后的 backing 重读 String 引用，结果发布后不再修改 bytes；不管理容量、不复制中间 String 数组、不调用用户 getter 或重新验证不变的 UTF-8 内容。

`scoop_rt_allocation_overflow()` 为无参数、NoGC 后备，在 Long 容量运算溢出时进入 fatal allocation failure，不分配、不回调、不引入新的源码异常。

编译器可以将普通 String 拼接链降低为一次 String 分配及按序复制，条件见实现规范 2.15。长度求和、24-byte 前缀及最终对齐必须保持 checked allocation 规则，不能 wrapping。结果复用既有 TLAB／collecting slow path；源 String 引用在 safepoint 中保活并接受移动回写，复制地址在分配后重取。结果为无引用 UTF-8 payload，不增加写屏障或对象字段；分配消除不承诺保留中间临时 String 的身份、GC 次数或内存耗尽时点。原二元 `scoop_rt_string_concat` 与 builder 的 join_parts 后备及 ABI 保留。

实现规范 2.15 的 typed StringCopy 对稳定原生 UTF-8 区间、已定位的 String 字节范围和 exact Byte 数组生成分配、长度写入及 memcpy，也用于 String 到独立 Byte 数组的快照。复用实际 String TD 和数组布局，保留非负长度、范围及分配溢出规则；只复制给定字节数，不要求或添加 NUL。托管源在分配期间保活，源内部地址只从分配后的重定位引用取得；原生源的稳定性由 Unsafe 调用契约保证。结果初始化完成前不向用户发布，中间没有新的 safepoint。保留的原生适配按实际消费者提供；不能按符号拼写把普通 extern 调用改成该操作，也不因旧公开头文件或测试声明而保留无消费者的重复实现。

integer Hash 与 compareTo 返回 Long。`==` / `!=` 执行编译器已选定的 operator equals；库相等经普通 `Equals<T>.equals` 调用同一 operator 契约，字符串化／哈希按实际 ToString／Hash 声明执行，不提供按 Any 或地址兜底。Equals 不增加 runtime 比较入口；浮点相等遵守语言规范 11.2.2，结构 operator 使用 compiler 生成的逐字段／variant 正文。Iterator、Range、List、ArrayList 和 StringBuilder 的公开行为由普通 core 接口规定，不增加容器专用 runtime ABI。

### 6.1 Float / Double

Float 为 4-byte、4-aligned IEEE binary32，Double 为 8-byte、8-aligned binary64，均 GC-free。存储、复制、FFI 与 toBits/fromBits 保持全部位型，包括 NaN；不使用浮点 niche 替代 Option tag。

toBits/fromBits 的整数载体分别为同宽 Int／Long；这些方法通过既有局部 place 取址与 Ptr load 实现，不新增 ABI 或 native 入口。方法局部副本不逃逸，正文不分配、不执行浮点算术。toBits 为 NoGC；fromBits 仍遵守普通 companion managed call 的初始化与 GC 规则。

默认比较将所有 NaN 视为相等且排在正无穷之后，正负零相等；显式 IEEE 比较沿用 NaN 无序规则。比较不修改位型，不增加 runtime 比较入口；编译器 intrinsic 与普通 core body 实现语言规范 11.2.2 的契约。

C FFI 使用目标真实 float/double ABI。数值操作遵守语言规范 11.2.2 的 rounding、NaN、无穷、signed zero 和转换规则，不能启用改变这些语义的 fast-math。线程的浮点环境采用 round-to-nearest ties-to-even、无 trap、保留 subnormal。

文本转换不依赖进程 locale。解析按声明精度正确舍入；非法输入和溢出返回 None，合法下溢仍返回相应浮点值。格式化遵守语言规定的可往返文本、特殊值与 signed zero 形式。

Float／Double 的最短数字继续由 vendored Ryu 的 `f2s_buffered_n`／`d2s_buffered_n` 产生。普通 core 通过明确的 C ABI GCLeaf 声明调用，只传入 GC-free 标量与足够的栈缓冲；这些同步有界入口不分配、不握手、不回调、不保留指针。core 的普通有界格式 helper 处理小数／科学计数法阈值、至少一位小数、指数符号与零、NaN／Infinity 和负零，再用 typed StringCopy 一次分配并复制实际长度。String 不要求 NUL；旧 `scoop_rt_float_to_string`／`scoop_rt_double_to_string`、C 文本适配和分配包装退役。

JSON／CLI 的现有解析适配继续复用所选 libc 的 `strtof_l`／`strtod_l` 与固定 C numeric locale，按目标精度完整消费输入并检查有限值。长度不足 256 B 时在固定栈缓冲中复制并补 NUL；更长输入使用原有动态缓冲慢路径，两者使用同一解析与错误规则。该阈值只选择存储，不限制可接受输入，不改变舍入、subnormal、signed zero 或下溢语义；动态分配失败仍沿原 fatal 路径。caller 保持原 NativeBorrowed 保活协议，不把整个动态扫描标为 GCLeaf，不改变 locale 的一次性协调规则。

## 7. 程序启动与退出

工具链按目标交付预编译 runtime，普通程序链接默认消费所选 sysroot 的 runtime 索引与对象；显式源码构建和对象覆盖遵守实现规范 2.7、2.8。索引消费只检查实际目标、runtime ABI 与部署兼容性，不要求生产与消费工具路径相同，也不改变本节的启动顺序。

启动先保存 2.8 的原始 argc/argv，建立运行时基础状态并验证全部 image/registration，再建立 GC、主线程与可执行 managed boundary。任何 managed initializer、Context 分配、argv 的 managed 构造或 main 之前必须完成相应登记；C coordinator 不提前构造未登记的 String 或 Array。

实现规范 2.19 的最终链接选择可以删除不可达的完整 callable 和类型／shape 物理成员；启动只登记实际保留的类型、callable／safepoint，删除成员的 stackmap 与记录同时缺席。保留代码仍须满足完整的 owner、site、frame 与精确根合同，保留 TD 必须有同一 exact type 的登记，不能仅为减少启动工作而丢弃仍可执行代码或存活类型的记录。静态根、immortal 和初始化记录沿现有合同保留；此选择不增加对象字段、GC roots 或 runtime 测量代码。

`--enable-startup-stats` 显式请求编入启动诊断，默认不编入；该构建开关独立于 profile 和 GC stress。runtime 的 C 生产配置以 `SCOOP_ENABLE_STARTUP_STATS=0/1` 控制预处理，关闭时不得生成计时、阶段状态、报告、诊断字符串和专用计数访问，也不能通过环境变量重新启用。编入后，每次启动在 `scoop_rt_run_program` 取得 platform／加载镜像范围前开始单调墙钟计时，在 eager 初始化完成、调用 root gateway 之前向 stderr 写一行 JSON：`scoop_startup:1`、`total_ns`、`phases_ns` 和 `counts`。七个互不重叠的阶段为 `loaded_images`、`collect_records`、`code_and_types`、`storage_and_units`、`stackmaps`、`runtime_init`、`eager_init`；阶段纳秒数之和等于 total。runtime_init 包含 String 绑定检查、registry 发布、线程／callback／GC 初始化与主线程 attach。时间不包含 runtime 入口之前的 dyld／CRT、root gateway 内的参数构造、main、shutdown 或诊断输出本身；前置阶段失败时不伪造完整启动记录。

counts 直接读取本次已建立索引的数量：Cone `images`、有元数据的物理 `loaded_images`、`types`、`callables`、`safepoints`、合并后的 `stackmap_records`、不同地址的 `scan_programs`、`static_storages`、`static_roots`、`immortals`、`initialization_units`、`eager_units` 与 `context_keys`。不为诊断重新遍历或验证图；未编入时不存在这些诊断访问，正常登记所需索引保持原职责。该记录只供测量，既不是编译输入也不是 PGO profile，不参与程序合法性、对象／GC 布局或 ABI；进程总启动时间仍由外部测量。计时开启的运行须单独评估观测开销，不能与关闭诊断的吞吐样本混用。

native C／C++ 的标准初始化／析构由目标 CRT／适用的 C++ 运行库负责；发生在 Scoop runtime 建立之前或 shutdown 完成之后的 native 初始化／析构不得调用 managed 代码或进入 Scoop runtime。它们不替代 Scoop 的 initialization unit、release hook 或 shutdown，也不由 Scoop 补调；立即终止进程的路径不保证执行 native 析构。C 源码的初始化／析构及 libc 退出清理接口不要求启用 C++ 模式，输入和链接合同见实现规范 2.8。

eager 初始化遵守语言规范 12.3：依赖 Cone 先于 dependent，互不依赖的 ready Cone 按 canonical coordinate byte order，Cone 内按 PersistentInitializationUnitId bytes。普通 ensure 可以因显式依赖提前初始化目标；lazy unit 不进入 eager loop。

root TaskContext 在首个 gateway 的入口 poll 后创建，在后续 eager gateway 与 main 间保持。每次 gateway 按 2.8 独立 enter/leave。所有 eager 初始化成功后才调用 root gateway；它在入口 poll 后，按编译时已确定的 entry 形态构造参数并调用 main。

有参数的 root gateway 将包括 argv[0] 的全部 `argc` 项按严格 UTF-8 解码，复制为普通 managed String 并构造 `Array<String>`，再按 main 的完整 Scoop ABI 调用。构造期间遵守正常分配、root、relocation 与写屏障规则：部分构造的数组、当前 String 和其他跨 safepoint 的引用均须保活并重读，未初始化槽不得作为非 null String 暴露给 main。无参数形态省略整个数组构造和解码过程。原始 argv 只读且不含 managed ref，不要求 pin。

非法 UTF-8 使用普通 managed 启动失败路径：参数构造 helper 在抛出前经 NativeSafe stderr 输出指出参数下标（包括 0），不依赖顶层诊断调用异常的 toString。参数构造抛出的 Throwable 由 root gateway 捕获并发布到其 failure root。eager 或 argv 构造失败均不执行 main。root gateway 成功返回后，runtime 才使用其 `out_exit_code`；正常 Unit 返回对应 0，Int 对应实际返回值。

`main` 正常返回后进入 ShuttingDown，拒绝新的 attach 与 callback registration。只有主线程之外没有 attachment、没有活动 callback 且全部 token ownership 已释放时，才能销毁 GC 状态并使用 main 的正常退出码。仍有任一项时属于 shutdown 失败：在既有同步协议下取得剩余非主线程数、活动 callback 数与有未释放 ownership 的 token 数，释放相关锁后输出诊断，刷新 stdout/stderr，再调用 `_exit(1)`。该退出码覆盖 main 原返回值；此路径不等待 join、不销毁仍可能被其他线程访问的 GC 状态，输出等待使用 NativeSafe。M33 不提供 daemon 线程，也不将遗留资源诊断改成内部 ABI 错误的 abort。shutdown 不追加 managed destructor 或 release hook；之后非法重新进入仍为 fatal ABI error。

退出诊断固定为 `scoop: shutdown failed: non-main threads=N, active callbacks=A, owned tokens=T` 并以换行结束。N 不包括主线程，A 是仍在执行的 callback lease 数，T 是 ownership 非零的 token 个数；同一 token 的多份 ownership 只计一个 T，已消费 ownership 但尚未返回的 OneShot 计入 A。登记关闭和计数分别在现有 thread／callback registry 的锁下完成，输出时不持有这些锁；计数非零时不先进入异常状态或 GC 元数据的销毁流程。

core 的 `exit(code: Int): Nothing` 显式请求结束整个进程：先刷新 stdout/stderr，再调用 `_exit(code)`，传递完整 Int 值，不执行 finally、release hook、线程/token shutdown 检查或 join。刷新允许等待且按 NativeSafe 协议完成，期间保留其他线程仍可能访问的 GC 状态；不调用 C `exit()` 或补调 atexit/native C++ 析构。跳过刷新的立即终止接口留给后续平台库。

`scoop_rt_exit(int32_t code)` 使用 Scoop ABI，Nothing 保持既有 managed-reference 返回 carrier，但入口永不返回。终止路径在任何可能阻塞的诊断或刷新前单向发布 NativeSafe；此后不得再访问未固定 managed 引用或恢复 managed 执行。当前没有后续使用的 managed 局部不要求新建栈图，既有 caller/native/compiler roots、pin 帧、外层冻结段和 TaskContext 槽保持有效且不被拆除，供并发 collector 扫描。启动 coordinator 已处于 NativeSafe；内部 collector/park 协议执行中请求这一公开终止入口属于 ABI 错误，不能以终止替代未完成的 collector 操作。

语言级 panic、启动阶段未捕获异常与异常逃离 main 的进程退出码固定为 `1`，适用于全部四种 main。异常路径打印已发布异常的类型名与必要的初始化路径，不调用用户 toString；随后刷新 stdout/stderr，并调用 `_exit(1)` 终止进程。输出等待使用 NativeSafe 协议，不因等待 stdio 锁或 I/O 阻止其他线程推进 GC；允许刷新等待，不承诺退出耗时上限。这条失败退出路径不等待其他 Scoop 线程 join、不销毁仍可能被它们访问的 GC 状态。语言级 panic 可直接进入同一诊断／终止路径，不伪造 Throwable 或带空 failure root 的 gateway 失败状态。runtime 内部 ABI／不变量破坏仍使用 fatal 诊断与 `abort()`；外部信号终止保持目标系统的 signal 状态。

## 8. 协程

suspend callable 使用完整的 Continuation 与 CoroutineStep 合同；跨挂起状态保存在实际 managed frame 中。每个 live value 有精确类型、存储与 scan，保存的引用参与普通 GC 和写屏障。

completion 的成功、失败与恢复具有唯一获胜方和 release/acquire 发布；未获胜恢复不能执行正文、改变 TaskContext 或消费状态。异常在挂起前保存为普通 managed 引用并结束 native catch，不能把 active native record 存入 frame。

每次真正进入或恢复协程正文时使用 frame 保存的 TaskContext，退出或传播异常时恢复调用者 Context。其语言行为见语言规范 8.3、11.9，Context 契约见第 9 章。

语言规范 11.9.1～11.9.2 的结构化并发及取消由普通 core 对象实现。Job 树、等待者与结果沿普通 managed 扫描和原子发布；取消以普通异常恢复，不新增 runtime 任务、取消 ABI 或线程调度职责。

## 9. Task-local Context

### 9.1 动态绑定操作

TaskContext 属于逻辑任务，绑定以 exact static type 为 key。每个程序为已登记 key 分配 u32 slot；slot 只在本进程有效，不是持久化类型身份。

try-get 无分配、NoGC、nounwind，返回擦除后的单字 object 或缺失。接口 binding 的 itab 由编译器按该 exact key 的静态接口在取出时重建；runtime 不把内部 binding 当作双字源码接口。push 可分配，成功前保留旧绑定，成功时完整提交并返回可恢复 mark。restore 为 NoGC、严格 LIFO；snapshot 无分配，保存当前绑定；fork 得到独立任务上下文，保留绑定值身份。enter/leave 为 NoGC 并恢复前一 context。

这些操作的读取与写入具有真实内存效果，不能按 pure 常量跨绑定变更移动或合并。

### 9.2 保活与作用域

Context、binding、mark 与 snapshot 使用实际 managed 类型和 scan。当前 context、previous context、未消费 mark 及 callback/suspend snapshot 都必须保活，含引用写入遵守 3.6；消费后的 mark 不继续保活已退出 scope。

生成代码可通过私有 TLS 符号 `scoop_rt_context_slot` 取得当前线程原有 `current_task_context` 根槽的 native 地址。该符号在全部 profile 中都是 8-byte、8-byte 对齐的可变 TLS 指针；主线程与 foreign thread attach 时绑定，detach 时清空。它只指向原根槽，不保存第二份 managed 引用，也不增加 GC root。collector 继续扫描并更新原槽，C 后备与生成代码读取和写入同一槽。合法 managed invocation 期间线程不能 detach，因此可复用槽地址；不得把槽内 TaskContext 或其 root 字段值缓存跨可能改变绑定或发生 GC 的边界。协程在另一线程恢复时由新的 invocation 取得该线程的槽地址。

current、enter、leave 在所有 profile 按上述槽生成普通 managed-pointer 读取／写入，仍为 NoGC；enter 返回原引用，leave 恢复传入引用，previous 的后续使用遵守通常的精确根和重定位规则。槽本身是 native 根位置，不是堆字段，不需要堆写屏障。snapshot 先取得 current，为空时返回空 root，否则读取实际 TaskContext 布局中的 root 字段；读取不改变绑定。上述操作不新增 safepoint，不缓存对象内部地址，不把内存效果改为 pure。绑定树遍历和 push 的动态路径继续使用共享实现；restore 的已知 TaskContext 根字段写入及屏障由生成代码完成。 fork 的源码操作已使用 typed 分配和字段写入，旧 managed fork anchor 退役；首次任务建立由 ensure-root 保留私有分配实现。snapshot 的 C helper 是 callback 注册与动态绑定查找的真实消费者，current 成为该 C 文件的私有 helper；只供生成代码调用的 enter／leave／restore C 包装退役。

同一任务串行改变绑定。child task 继承创建时的 snapshot，后续重新绑定独立；binding 中原对象的身份仍可共享，不隐含深复制或并发同步。

TaskContext 的实例由对象头和一个可空 managed root 字段组成，root 指向已有的不可变 binding tree；fork 只分配新的实例并保存该 root。编译器按同一 TD／scan 布局生成分配及字段写入，复用普通分配失败、TLAB 慢路径和写屏障协议（实现规范 2.16）。snapshot 在可能 GC 的分配期间保活，分配后的写入消费重定位结果；不能跨分配保存内部裸地址。生成路径保持任务身份、绑定快照及恢复语义，不改变当前线程 Context 的根发布规则。

### 9.3 Context key 登记

ContextKey 直接使用 PersistentExactTypeId，不另算类型身份。每个实际使用 key 的 body 有关联的 writable u64 slot cell，初值为 0；live 值为 `slot + 1`，可覆盖完整 u32 slot 域。

cell 与 callable 一起选择，通过 callable record 的 Context key uses 登记。先验证全部 key/cell 的格式、地址、归属与唯一性，再写入 cells；首个 gateway 前完成。不同 body 的 cells 可以映射到同一 exact key slot，不能依赖名称或装载顺序区分类型。

### 9.4 任务与 callback 边界

root Context 在首个 managed poll 后创建；普通同步 native 往返保留外层 Context。callback 注册捕获当时的不可变 binding snapshot，retain 共享快照；每次 invocation 从该快照建立独立 TaskContext。

callback managed adapter 的私有参数顺序为 closure、nullable binding snapshot、result storage、argument storage、exception output，返回 callback status。C gateway 保活 closure、snapshot 和 exception；adapter 在入口 poll 后、用户 closure 前进入 Context，所有正常/异常出口恢复 previous Context 并结束 catch，再返回 C。

协程只有获胜的恢复路径进入 frame Context；完成或异常离开时恰好恢复一次。任务执行不隐式增加取消或跨任务绑定修改语义。

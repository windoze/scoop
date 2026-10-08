# Scoop 运行时规范

本文规定 Scoop 对象表示、GC、异常、线程、初始化与 native 调用的运行时契约。语言行为见 [语言规范](SCOOP-SPEC.md)，编译阶段、产物与链接规则见 [编译器与产物规范](SCOOP-IMPL-SPEC.md)。

## 1. 职责与目标

运行时提供 managed 对象分配、精确移动 GC、类型检查与动态分派所需的数据、异常展开、一次性初始化、Task-local Context 和 FFI 支持。运行时不得重新进行源码名称解析、重载选择或泛型实例化。

受支持目标使用 little-endian、64-bit data/code pointer 和 natural 8-byte metadata alignment。所有大小、数量、对齐和地址范围计算必须检查溢出及实际存储边界；不满足目标表示或 ABI 的输入必须拒绝。

## 2. 对象模型

### 2.1 Managed 对象

managed reference 是对象起点的直接指针；对象可能移动。对象头固定为 16 bytes，依次包含 TypeDescriptor 指针与 GC 状态字，各占 8 bytes。null 仅用于允许空状态的内部存储，不能作为普通非空 Scoop 引用。

GC 状态字的 pin bit 为 `UINT64_C(2)`（bit 1），release-ready bit 为 `UINT64_C(4)`（bit 2）；操作这些位时必须保留其他位。

class 的基类对象布局是派生对象的完整前缀，包括基类尾部 padding；派生字段不得复用该 padding。构造最派生对象时只分配一次，整个构造期间对象头都指向最派生 exact type 的 TypeDescriptor，不随基类构造改变。

对象在可被 GC 观察前必须具有有效对象头、精确分配大小与全零 payload。含引用的字段在初始化完成前保持 null 或合法 managed reference。构造期间的 receiver 是普通精确 root；未完成的实例不能发布为可用对象。

带 release hook 的对象初始 `RELEASE_READY` 为 0。完整的最外层构造成功后、对象发布前，以 release 语义置为 1；构造失败时保持 0。其回收规则见 3.8。

### 2.2 TypeDescriptor 与扫描描述

每个 materialized exact type 具有唯一 TypeDescriptor；同一 ODR 类型在最终程序中具有同一地址。不同声明或不同泛型 application 不因名称、表示或布局相同而共享类型身份。

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

消费边界必须检查 scan 的格式、引用、循环、对齐和范围。动态 Array 的 length 与乘加在扫描时也必须落入本次 region 的 extent；已验证且不变的静态结构可复用。

`parent`、itable 和相关类型引用必须解析到实际登记的 TypeDescriptor。空 vtable 为 null；空接口仍保留以其真实 TD 为键的 itable entry，slot table 可以为 null。slot 与 callable ABI 由产物合同确定。

`relation_kind` 为 0 时 related count 为 0、function result 为 null；1 和 2 分别表示普通与 suspend 函数类型，related types 按参数顺序保存、result 保存结果；该关系中 null TD 表示 Any。3 表示 interface，related types 按 exact identity 排序保存直接父接口，result 为 null。4 表示源码 intrinsic Nothing 的底类型关系，related count 为 0、result 为 null，实例形状必须为 AbstractRef；它是所有类型的子类型，其他类型不能成为它的子类型。其他 tag 非法。

`Any` 与 `Nothing` 的类型身份来自 core 的实际源码声明（语言规范 11.1）。二者的 TD 使用不可分配的 AbstractRef 形状；`Nothing` 不存在对象、box 或合法的非空值。底类型关系在函数参数逆变／结果协变检查中必须保留，不能把 `Nothing` 与 `Any` 或普通空 class 合并。Any 的值仍指向实际动态类型对象，移动 GC、原类型 TD 和装箱协议不变。

普通结果为 `Nothing` 的调用没有正常继续路径，但仍可能抛出异常或进入 GC；异常边及其根、finally 和 Context 清理照常保留。其逻辑结果保留实际底类型身份，物理引用结果即使保留在调用签名中也不能被作为一个成功结果使用。挂起函数的 `CoroutineStep<Nothing>` 可以返回 Suspended，不得把源码底类型误用为整个挂起 ABI 的 `noreturn` 属性。

### 2.3 值类型与装箱

struct、enum、tuple、数值、Ptr、FunPtr 依其静态类型按值存储。提升到 Any 或 interface 时，值存入带原 exact TypeDescriptor 的 managed box；值类型 receiver 使用独立的方法局部副本，不能取得调用方 place 或 box payload 的别名。

语言规范 11.11 的显式 `Equality<T>.equalTo` 实现使用普通 interface TD、itable 与 callable。表项只来自实际声明或继承的 conformance；结构 operator equals 不增加接口。NoGc 值方法实现 Managed interface slot 时，itable 使用符合 slot ABI 的 Managed value/interface adapter，具体值的直接 operator 比较仍调用原实现；runtime 不按类型名、对象地址或内存字节另行判断相等。

`Unit` 和其他 ZST 的 payload size 可以为 0，类型身份与 box 对象身份仍存在。`scoop_rt_box_zst(td)` 每次产生新的 box；`scoop_rt_unbox_zst(object, expected_td)` 检查 exact type。

`scoop_rt_box_value(td, source_place)` 要求 source place 可读、地址稳定、满足该 TD 的 inline size/alignment。含引用的 source place 必须在入口握手前以同一 TD 的 nonempty inline scan 登记为链顶 RecursiveRegion root。分配或 GC 后从更新后的 place 复制，返回后按 LIFO 撤销 root。`scoop_rt_unbox_value(object, expected_td, destination)` 在 exact type 检查成功后按值写入满足大小与对齐的 destination。

### 2.4 String 与数组

String 持有合法 UTF-8，允许 U+0000，无隐含结尾 NUL。byte length 位于 offset 16，bytes 从 offset 24 开始；length 不超过 `INT64_MAX`。分配大小为 `alignUp(24 + byte_length, 8)`。

严格和 lossy 字节转换都只能发布满足该不变量的 String。lossy 的每个 U+FFFD 占三个 UTF-8 字节（`EF BF BD`），计入结果 byte length；结果长度计算和最终分配遵守上述边界，不能直接沿用输入长度。

Array 与 MutableArray 的 logical size 位于 offset 16，元素区从 `alignUp(24, element_alignment)` 开始。分配大小为 `alignUp(element_offset + size * stride, object_alignment)`。size 在 `0..=INT64_MAX`，所有运算还须满足 `u64`、`size_t` 与实际分配边界。

ZST 数组不保存元素 payload，但保留 logical size、完整类型与独立对象身份；clone 和互转产生新对象。按长度初始化仍逐索引执行 initializer，保留副作用与异常。初始化期间数组和已写元素必须可被精确扫描，不得向用户暴露未初始化元素。

ArrayList、StringBuilder 等普通库类型使用其声明的 class/value 表示，其公开行为由语言规范第 11 章规定。

### 2.5 Option 的 niche 表示

引用类型的全零机器字表示 None；Ptr/FunPtr 的内部 data/code-pointer carrier 为全零时，也只表示对应 Option 的 None。裸 reference、Ptr 与 FunPtr 保持非零不变量，Some(payload) 不与 None 碰撞。GC 扫描不能把全零追踪为有效引用；其余 Option 表示遵守语言规范 7.4。

### 2.6 Closure 与函数类型

closure 是普通 managed 对象，捕获值在创建点按语言规定取得并存入不可变字段；含引用的捕获参与精确扫描。它的 TD 保存实际函数类型关系。

每个 closure 的 vtable 第 0 槽为动态 invoke，保持参数个数和挂起性，显式参数与结果以 Any 传递。该 managed 边界按 closure 的源码签名解包、调用 typed invoke 并装箱结果；suspend 使用 `Continuation<Any>` / `CoroutineStep<Any>`。静态调用使用原 typed 签名。

函数类型转换按语言规范 8.1 的结构化关系判断。closure 的 TD、动态 invoke 和签名由自身 identity 决定，不随消费方的目标签名变化；适配结果与来源不保证引用身份相同。

closure 不能直接作为 FunPtr 交给 native。静态 FunPtr 与 4.3 的 managed callback 分别遵守各自边界。

### 2.7 Initialization unit 与 singleton 发布

每个需要运行期求值的顶层属性、singleton 和 delegated extension application 有独立初始化单元。其 GC-free cell 位于 managed heap 外，size 16、alignment 8，保存 `state: u64` 与 `owner_thread: pointer`。状态为 Uninitialized、Initializing、Initialized 或 Failed。

同一 singleton 的所有访问使用同一 ensure、published root 与 cell。generic companion 按完整宿主 application 区分状态；同一 application 跨 Cone 只初始化一次，不同 application 不因布局相同而合并。宿主构造、类型查询和 const 读取不隐式初始化 companion。

winner 完成全部初始化后，先发布 property storage 或 singleton root，再以 release 语义发布 Initialized。waiter 以 acquire 观察终态后重新读取 storage。Initializing 期间不能读取临时 singleton；构造中的 receiver 只由正常 managed root 保活。ZST storage 也保留 initializer、ensure、cell 和副作用。

失败时不发布 singleton，不把部分 property storage 视为可读；异常物化为普通 managed Throwable，存入单独登记的 failure root，再发布 Failed 并唤醒 waiter。后续访问重新抛出该失败，不重试、不回滚已发生副作用。

同线程重入和跨线程等待环产生带稳定 unit path 的 cycle 结果。generated ensure 通过已解析的 core thrower 抛出包含完整路径的 IllegalStateException；只有异常逃离 initializer 时，该 unit 才失败。无环等待必须兼容 3.5 的 GC 握手，不得持有未登记 managed pointer 睡眠。

unit 由 `PersistentInitializationUnitId` 标识；diagnostic path 只用于显示。初始化记录完整关联 cell、storage、failure root、initializer、ensure 及 eager startup gateway。lazy delegated extension 的这些共享实体也按同一 ODR application 合并。

### 2.8 多 image 登记与启动 ABI

runtime ABI contract 为 **11**，metadata ABI 为 **7**。带 prefix 的 descriptor 以 `{ u64 magic; u32 abi_version; u32 struct_size; }` 开头，`abi_version == 7`，size 与本节布局精确一致，reserved fields 为 0。公共 C 声明见 [scoop_runtime_metadata_v1.h](../../runtime/include/scoop_runtime_metadata_v1.h)。M33 实施时同步更新该头文件、producer、reader 与启动代码；旧版无参数 root gateway 与本节不兼容，即使 descriptor 大小相同也必须重建或拒绝，不能强制转换后调用。

| descriptor | magic | size（bytes） |
| --- | --- | --- |
| Image | `0x53434f4f50494d47` | 240 |
| RootEntry | `0x53434f4f50454e54` | 192 |
| StaticStorage | `0x53434f4f5053544f` | 264 |
| ImmortalObject | `0x53434f4f50494d4d` | 152 |
| InitializationUnit | `0x53434f4f50494e49` | 320 |
| TypeRegistration | `0x53434f4f50545950` | 208 |
| SafepointRegistration | `0x53434f4f50535054` | 200 |
| CallableRegistration | `0x53434f4f5043414c` | 176 |

六类 registration 在 prefix 后依次保存 `{ u32 linkage_kind; u32 reserved_zero; semantic_id[32]; odr_group_id[32]; odr_member_id[32]; }`，共 104 bytes。linkage 为 Strong=1 或 Odr=2；Strong 的 ODR 字段为 0，Odr 的 group/member 非零。semantic ID 的种类由 record kind 决定，分别为 static storage、immortal object、initialization unit、exact type、safepoint site 和 callable body。

各 registration 的其余字段按以下顺序排列：

| record | 字段 |
| --- | --- |
| StaticStorage | `scan_kind:u32, initial_state_kind:u32, writable_base, byte_size:u64, allocation_extent:u64, required_alignment:u64, scan_program, scan_fingerprint[32], layout_fingerprint[32], initial_template:ByteSpan, initial_relocations, initial_relocation_count:u64` |
| ImmortalObject | `object_start, object_size:u64, required_alignment:u64, type_registration` |
| InitializationUnit | `schedule_kind:u32, reserved_zero:u32, diagnostic_path:ByteSpan, cell, storage, failure_root, initializer_callable_id[32], ensure_callable_id[32], initializer_entry, ensure_entry, startup_gateway_callable_id[32], startup_gateway_definition_fingerprint[32], startup_gateway` |
| TypeRegistration | `runtime_type_id:u64, reserved_zero:u64, descriptor, descriptor_fingerprint[32], layout_fingerprint[32]` |
| SafepointRegistration | `safepoint_id:u64, site_role:u32, root_pair_count:u32, owner_callable_id[32], normalized_stackmap_fingerprint[32]` |
| CallableRegistration | `body_definition_fingerprint[32], entry, context_keys, context_key_count:u64` |

ByteSpan 的物理表示是 pointer 后接 `u64 length`；static relocation 是 `{ pointer_offset:u64, target:ImmortalObjectDescriptor pointer }`；Context key use 是 `{ exact_key[32], slot_cell:u64 pointer }`。擦除后的 callable function-pointer typedef 只用于地址身份，不能据此发起调用。

Image 依次包含 prefix、canonical Cone record（group/name/version ByteSpan 与 ConeIdentity）、runtime image fingerprint、直接依赖 identity pointer/count，再按 storage、immortal、initialization、type、safepoint、callable 顺序保存六组 record pointer/count。每个 image 表只列出最终链接选中的 producer，普通 external ref 不重复登记。

RootEntry 依次包含 prefix、owner ConeIdentity、main callable ID、source signature fingerprint、gateway callable ID、gateway definition fingerprint、failure-root descriptor pointer 与 gateway pointer。main 的源码签名为语言规范 12.4.4 的四种 ordinary 形态之一：无参数或单个 `Array<String>` 参数，返回 `Unit` 或 `Int`。source signature fingerprint 覆盖实际完整签名，gateway definition fingerprint 覆盖对应的参数构造、调用和返回适配。

四种 root gateway 使用统一的 C ABI；具体源码形态由编译器的 typed entry 和生成的 gateway 正文确定，runtime descriptor 不另加形态 tag，RootEntry 大小仍为 192 bytes。程序入口与 gateway 的原型为：

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

所有 image、类型、scan、静态根、初始化单元、callable 和 safepoint 必须在首个 managed initializer 前登记。runtime 验证当前加载边界的格式、引用、实际地址、权限、唯一性和 GC 契约，复用编译与链接边界已完成且未变化的语义、布局和 ODR 结果。

登记建立完整 typed ID 与 record/address 的对应关系。runtime type ID 与 exact type、非零 SafepointId 与完整 site identity 都必须全程序一一对应；不同完整 identity 的固定宽度碰撞必须在使用前报错。不同 body ID 不得共用 callable entry；同一共享实体不得有多个实际地址。

callable 正文、EH、stackmap、callable/safepoint registration 与其 Context cells 必须来自同一被选实现。独立 helper 可来自不同 provider；所有共享 TD、storage、cell 和初始化状态仍唯一。没有 safepoint 的 Scoop body 也登记 callable；普通 native extern 和 runtime C body 按各自 ABI 消费。

非空 pointer/count span 的范围、乘加、对齐和权限必须有效。既有 image/registration 空 span 使用类型正确的 addressable sentinel；`context_key_count == 0` 时 context_keys 为 null。descriptor、字符串、scan、template 与 relocation metadata 为只读；callable entry 位于可读、可执行且不可写的映射；storage 的完整 allocation range 可写。权限按实际加载映射判断。

storage 的 `allocation_extent == max(byte_size, 1)`。ZST storage 使用独立的 1-byte writable token、None scan 与 canonical 空 scan。不同 storage 的 allocation range 不重叠，只有同一 ODR member 的重复引用可指向同一存储。

静态初值有两个封闭分支：

- `ZeroedForRuntimeUnit=1`：完整 allocation extent 初始为零，template 与 relocation 为空；用于初始化单元的 storage/published root，以及 unit/root-entry failure root。
- `EncodedStaticValue=2`：template 长度等于 allocation extent，padding、ZST token 和 managed-pointer leaf 为零；实际非 pointer 初值与 template 相同。该 storage 不再作为 runtime unit 的 storage 或 failure root。即使 template 全零，也保持此 tag。

Encoded relocation 按 pointer offset 严格递增，无重复；offset 8-byte 对齐、位于 logical byte size 内且恰为 Recursive scan 的一个 leaf。每个非 null leaf 必须且只能对应一项 relocation，指向已登记、只读 immutable immortal object 的精确 object start；其余 leaf 为 null。None scan 不允许 relocation，immortal 不含可移动引用。产物边界验证初值和 relocation 内容，runtime 核对加载后的实际引用及 root 关系。

failure root 必须是独占的 8-byte、8-aligned Recursive storage，scan 为 `[1, 0]`，初值为 ZeroedForRuntimeUnit；不能兼作普通属性或另一 unit/root 的失败槽。cell 初值为 `{0, null}`，不同 unit 的 cell 互不重叠，也不与 storage 重叠。所有引用与初态验证完成后，才允许 coordinator 或 gateway 修改它们。

eager schedule=1，含已登记 startup gateway；lazy schedule=2，gateway ID、fingerprint 与 pointer 分别为全零、全零、null。initializer/ensure 的 ID 与 entry 必须解析到同一实际 callable。eager gateway 的 fingerprint 等于对应 callable body fingerprint，entry 地址逐 bit 相同；root gateway 遵守同样规则。

eager startup gateway 继续使用独立的 `uint32_t(void)` C ABI，成功返回 0、失败返回 1、其他状态非法；runtime 分别按精确原型调用 eager 和 root gateway，不将它们视为同一函数指针类型。root gateway 的 managed catch 覆盖 argv 构造与 main 调用，物化 Throwable、写入 root-entry failure root 并 EndCatch 后返回状态 1；eager gateway 使用 ensure 已发布的 Failed 与 failure root，不再次物化或覆盖异常。不得将 native unwind record 或 BeginCatch payload 地址存入静态失败槽。

每次 init/root gateway 调用建立独立 native→managed boundary，开始为没有 managed frame 的 EntryPending；入口必须 poll 后才成为 ActiveManagedSegment。返回前完成全部 managed 异常处理，只把 GC-free status 及 root 成功时的整数输出交给 C；随后按 LIFO 恢复外层 mode/boundary。GC 仍扫描外层 frozen segment 和 native roots，不把 C coordinator 的 PC 当作 managed anchor。

runtime image fingerprint 对 canonical records 编码，不能 hash raw struct 或 ASLR 地址。编码为 little-endian u32/u64、32-byte ID 原字节、`u64 length + bytes` ByteSpan、`u64 count + elements` sequence、无 padding 的声明序 product 和 `u32 tag + payload` sum。record kind 按六张表顺序为 1..6；payload 先保存 linkage、semantic ID、ODR group/member，再保存下表的字段，不编码 descriptor prefix、padding 或 reserved fields。role 使用以下 u32 tag：

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

`RuntimeImageFingerprint = SHA-256(ByteSpan("scoop-runtime-image-v2") || RuntimeAbiFingerprint || TargetProfileFingerprint || canonical Cone record || sorted direct dependencies || six canonical record sequences)`。两个输入 fingerprint 各为 32 bytes；canonical Cone record 依次编码 group、name、version ByteSpan 和 ConeIdentity。自身 fingerprint slot、物理对象分片和 ASLR 地址不进入此编码。body、descriptor、layout、scan 与 stackmap fingerprints 的产物规则见编译器与产物规范 2.6。

`PersistentCallableBodyId = SHA-256(ByteSpan("scoop-callable-body-v2") || canonical(CallableBodyKey))`。key 的 u32 tag 与字段为 Strong=1 `{owner}`、Odr=2 `{member}`、RootGateway=3 `{root_cone, main_body_id}`、InitializationStartupGateway=4 `{unit_id}`、ReleaseHook=5 `{owner_exact_type_id}`。这些 body 的身份不能互换。

stackmap 使用 LLVM v3 record。规范化内容依次为 `{format_version=3, site_id, safepoint_id:u64, owner_callable_id, site_role:u32, root_pair_count:u32, instruction_offset:u32, stack_size:u64, locations, live_outs}`。location tag 为 Register=1 `{size:u32, dwarf_register:u32}`、Direct=2 或 Indirect=3 `{size, dwarf_register, signed_offset_bits:u64}`、Constant=4 `{size, value_bits:u64}`；live-out 为 `{dwarf_register:u32, size:u32}`。保留原 sequence 顺序和数量，i32 offset/constant 符号扩展到 i64 bits，ConstantIndex 解析为 pool 中的 u64 Constant；reserved/flags/padding 为零。

`normalized_stackmap_fingerprint = SHA-256(ByteSpan("scoop-stackmap-record-v1") || canonical(record))`。ASLR function address 不入 hash，但必须等于 owner callable entry；instruction offset 与 stack size 入 hash。site role 为 ManagedPoll=1、ManagedCall=2、ManagedInvoke=3、NativeSafeTransition=4、NativeBorrowedTransition=5。实际 record 与 registration 的 site、owner、role、root count 一致。

受支持目标要求 location 数为 `3 + 2 * root_pair_count`，前三项是 8-byte Constant，每对 root 为相同的 8-byte SP/FP Indirect 可写 slot。root count 是实际机器表示中的 relocation pair 数，不必等于源级逻辑 leaf 数；invoke 与 native transition 使用显式 root frame，gc-live 为零。

stackmap 范围可以包含多个完整 v3 blob，必须解析到范围末尾。registration 与 raw record 完整对应，return PC 使用实际原始地址，不作减指令宽度、邻近符号或范围猜测。不同完整 site key 不能复用 SafepointId 或 return PC；同一 ODR site 的重复记录只在 owner、最终地址、location/live-out 与 fingerprint 全等时合并。

Darwin 的不可变 metadata 与 stackmap 在 relocation 后必须只读；Linux 按实际加载映射验证，包括已生效的 RELRO。native DSO 不构成 Scoop logical image。String 使用实际声明的唯一 TD，`scoop_td_String` 与其地址相同。运行期不新增或卸载 Cone，Scoop runtime 也不执行 managed global destructor；native C++ 初始化／析构边界见第 7 章。

## 3. GC 契约

GC 支持多个 mutator，collection 在 stop-the-world 协调下执行，使用精确根与对象 scan，不保守猜测整数或 C 栈中的地址。minor 与 full collection 均可移动对象，pin 之外的裸地址不跨 safepoint 稳定。

### 3.1 分配与发布

managed 分配返回直接对象指针，普通小对象分配摊还 O(1)，不要求经 handle 间接访问。快速与慢速分配都必须在返回前完成清零、对象头、精确大小与对象起点登记；可能触发 GC 的入口遵守完整 safepoint 和 root 契约。

进入 collection 时，所有线程的分配状态必须停止使用并与 collector 一致。对象不得在仅部分登记时被扫描；分配失败使用明确的失败出口，不能返回无效对象。

nursery 容量和 collection threshold 是收集触发条件，不是堆空间耗尽的证明。收集后被其他 mutator 抢先取得分配区时，分配方重新尝试补充自己的分配区；不能按固定次数的 nursery 竞争报告 OOM。需要 full collection 才能确认的分配失败，不能把加入另一轮 minor collection 等同于已完成自己的 full collection。

### 3.2 Safepoint 与机器根

managed 函数入口和循环回边，包括 continue 回边，提供 safepoint。managed references 在 LLVM ABI 中使用 address space 1；跨 safepoint 存活的引用必须由 stackmap 或显式 compiler root frame 描述。

普通 call/poll 的 relocation root 按 2.8 的实际 stackmap 更新。invoke 的正常与异常出口使用显式 compiler roots；native transition 使用 caller root frame，二者的 statepoint gc-live 为零。引用在握手后重新读取，再传给实际 native callee；native 返回的引用在重新允许 GC 前进入有效 root。

native 实际调用不得 unwind 穿越边界。静态 FunPtr 的同步 NoGC 调用不隐式 poll；可能进入 runtime 或 managed callback 的路径遵守线程转换规则。语言规范 13.4.1 的 `@GCLeaf` C 调用不构成 safepoint，也不建立 native transition 或专用于该调用的 caller roots；实参求值和调用前后的真实 safepoint 继续遵守正常根规则。

Darwin/AArch64 的 SP/FP DWARF register 分别为 31/29，stack size 为 16 的倍数。Linux/amd64 分别为 7/6；若 stackmap stack size 为 N，要求 `N % 16 == 8`。amd64 的入口 return PC 在 `[RSP]`，call-site SP 为 `RSP + 8`，FP 为 RBP；N 不包含 return address，`FP = SP + N - 8`，`[FP]` 保存前一 FP、`[FP+8]` 保存 return PC、前一 SP 为 `FP+16`。root slot 位于 `[SP, FP)`。这些目标保留 frame pointer，不使用 red zone 或破坏栈遍历的 tail call。

### 3.3 精确根集合

完整根集合包括：

- 活动 managed 栈段的 stackmap roots 与 compiler root frames；
- native transition 冻结的外层 managed 栈段，以及全部 caller/native root frames；
- 已登记的 Recursive static storage、初始化与 root-entry failure roots；
- 当前 TaskContext，包括 native-safe 线程保存的 context；
- stable external exception payload 中的引用；
- live GC handles 与 pinned 对象。

immortal 对象只读且不含可移动引用。collector 对每个 root/对象只按其实际 scan 访问引用槽，移动后原地更新；外部异常 payload 自身地址稳定，但其出站引用仍更新。

### 3.4 Pin 与 handle

pin 为摊还 O(1)、unpin 为 O(1) 操作。同一对象的显式 pin 共用一项登记和非零计数；每次 pin 增加计数，每次 unpin 减少计数，计数归零才移除登记、清除固定状态。登记期间对象保活且地址稳定，多线程操作由 heap lock 串行化；collector 在同一锁保护下访问登记。对象头的 GC 私有状态字保存其登记索引，移除时交换末项并修正该对象的索引，不线性搜索 pinned 列表。索引不进入语言可见的 `PinnedPtr.raw`，也不改变对象头大小或 typed ABI。非法地址、没有对应显式 pin 的 unpin 或计数溢出属于 fatal ABI error。pin 不替代线程、root 或写屏障协议，不提供对 payload 的同步。

GC handle 是 GC-free opaque 64-bit 值，引用一个当前 live slot 与 generation。slot 复用时 generation 改变；非法、stale 或已释放 handle 是 fatal ABI error。handle 保活对象但不固定地址；解析后跨 safepoint 使用时仍须 root/reload，或显式 pin。

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

保留 GC 开始、park 确认、GC 结束以及初始化完成/失败的必要通知。初始化等待者可能在 world 为 `RUNNING` 时已处于 `PARKED`，必须在新 GC 开始时被唤醒并确认新 epoch。上一轮的 parker 尚未恢复而下一轮 GC 已开始时，也必须确认新 epoch。`parked_from` 及冻结栈段、根链在一次 park 区间开始时发布，来源状态保留 `MANAGED_PENDING` 的区别；确认新 epoch 或虚假唤醒不得重写这些可扫描记录。

NativeBorrowed 仍可能访问 direct ref，必须在有效 safepoint park 并确认本轮 epoch 后才算停稳。gateway、callback 等低频 managed 入口可继续在 world lock 内完成 `RUNNING` 检查与状态转换；使用完整有锁入口时，不要求再叠加 RETURNING 握手。

native transition 的边界、必要根和 LIFO 检查在所有构建中保留；遍历整个活动 transition 链的查重只用于 debug runtime。release runtime 不为每次调用重放已经成立的整链完整性检查。

`@GCLeaf` 是编译期 C 调用模式，不是新增线程 mode。其实际调用保持 caller 的原状态；从活动 `MANAGED` 调用时，collector 必须继续等待该线程在后续真实 safepoint park，不能提前扫描其栈或移动对象。调用链不得执行 safepoint、park、转换线程状态、回调 Scoop 或进入需要这些动作的 runtime API，且不得阻塞等待其他线程推进。此模式不改变既有 pin、root、指针有效期及用户数据同步契约，也不自动授予其他 runtime 入口的调用资格。

### 3.6 写屏障

向 managed heap 写入 managed reference 后、下一次可能 GC 或发布前，必须执行等价于以下入口的写屏障：

```c
void scoop_rt_gc_write_barrier(const void *destination, size_t bytes);
```

屏障覆盖本次写入的完整范围及其相交的 512-byte cards；零字节为 no-op。屏障有限时间、NoGC、无分配、无 park，并发标记不能丢失已记录的脏状态。

该要求适用于字段、数组、含引用 aggregate copy、构造、clone、Context 和 Scoop ABI native 写入。发生过可能 GC 的操作后，不能仅凭“刚分配”省略屏障；native root 或 pin 不替代 old→young 引用记录。

`AtomicRef` 的初始化、store、exchange 及成功 CAS 同样写入 managed reference，必须覆盖相应引用槽的卡表；失败 CAS 没有写入，不需要写屏障。计算对象字段地址到原子访问完成，以及引用写入到写屏障完成之间不能插入 safepoint。AtomicRef 对象、expected/new 引用和读取结果跨 safepoint 时遵守普通 root/relocation 契约；collector 在 mutator 停稳后按普通引用槽扫描和回写，移动不改变 CAS 所比较的对象身份。语言内存序由原子指令实现，GC 屏障不能代替 Acquire/Release。

### 3.7 移动与存储有效期

full collection 在有可移动存活对象且目标空间足够时至少移动一个 eligible 对象。pinned 对象不移动；移动不改变语言对象身份。

原存储失效前，所有 roots、存活对象引用和相关运行时引用必须更新到新地址；不能遗留指向已回收副本的 managed reference。对象精确大小与 scan 不得从可能失效的存储猜测。移动或空间预留失败必须保留完整可达图，不能在部分更新后按未移动状态继续执行。

### 3.8 GC-free release hook

release hook 仅属于符合语言规范 9.1.6 的 FixedObject 类型。启动时其函数地址必须解析到该 exact type 对应的 ReleaseHook callable；随后 GC 使用已登记的合同。

正常 collection 决定回收 ready 对象时，在 storage 失效前清除 ready 并尝试调用一次 hook。未完成构造的对象不调用；移动副本和旧地址回收不额外调用。ready 的 release/acquire 配对保证构造写入可见，显式 close 的 inert-state 写入也必须在 park 前完成；hook 不为有数据竞争的程序补充同步。

hook 在 world 停止期间执行，不得分配 managed 对象、发起 GC、访问 root/handle/pin/thread API、等待停顿的 mutator、回调 Scoop 或跨出 foreign unwind。允许的 Scoop helper 必须满足 NoGc/release-safe 效果，直接 C extern 遵守 raw leaf 契约；不能保留临时地址或重新进入 runtime。

符合语言规范 9.1.6、13.4.2 的直接 C extern 可捕获 errno。生成的 bridge 在原调用前清零 libc errno、返回后立即保存为本次调用的局部整数，与 native 结果一起交回普通 GC-free tuple；不访问 Scoop TLS 或 thread runtime，也不改变 collector 状态。hook 中的新捕获不会覆盖 mutator 已取得的错误值，collector 不为此保存或恢复线程级 last-error。其他 release-safe、`ReleaseValue` 与一般 TLS 访问限制继续适用。

release 是 best effort，不保证触发时间、对象间顺序、执行线程、native 成功或退出时调用；abort、fault 或不返回的调用没有重试保证。shutdown 不补调 live、未收集或 unready 对象的 hook。确定性释放由显式 close/release 与 try/finally 保证。

### 3.9 分代收集

minor collection 处理年轻代，依据完整精确根与旧代脏区追踪年轻可达对象；其正常工作不遍历整个旧代图来弥补缺失屏障。full collection 处理两代；显式 `gc.collect()` 请求 full collection。

minor 和 full 共享线程、root、pin、handle、Context、callback、冻结栈段及精确扫描契约。晋升或移动在原存储失效前更新全部相关引用；pin 暴露的地址不变。年轻区复用不能遗漏存活引用或 release hook，旧代死对象可以留到 full collection。

分配压力可触发 minor 或 full；晋升空间不足时必须从完整对象图进入 full 或明确的分配失败出口。分代策略和容量不改变语言合法性、对象 ABI 或 FFI 保活义务。

## 4. Scoop ABI FFI runtime functions

本章规定语言规范第 14 章对应的 runtime 能力。managed generated-code、native-borrowed、runtime internal 与 foreign-callback 入口具有明确的调用角色，入口不从裸 C 栈猜测 caller kind。

### 4.1 对象分配

- `scoop_runtime_alloc_pinned(type_desc, size) -> ScoopObjectHeader*` 分配即固定，返回前 pin 生效；具体 API 明确 unpin 所有权。
- `scoop_runtime_alloc(type_desc, size) -> handle` 返回可移动的 GC handle，用于跨 native root frame 保活。
- 只在本次 native 调用内存活的引用可以保存在 native root slot；FFI 没有 managed stackmap，不能绕过 native-borrowed 入口协议。

可能 GC 的 native 分配入口在首个此类动作前取得有效 transition anchor 与完整 root-frame 链。collection 扫描 caller roots、native DirectSlots/RecursiveRegion 及被冻结的 managed 栈段；返回后调用者从更新后的 slot 或 region 重读，不捕获 C return address 代替 managed anchor。

### 4.2 Native root、pin 与 handle 操作

native root frame 为封闭的 DirectSlots 或 RecursiveRegion。前者通过 `push_native_roots(slots, count)` / `pop_native_roots()` 等价能力登记一组 direct-ref slots；后者通过 `push_native_region_root(base, byte_extent, scan)` / `pop_native_region_root()` 等价能力登记 non-null 稳定 base、非零 extent 与 canonical nonempty scan。两者共享严格 LIFO 链，不能以 count=0 或 null scan 伪装另一种 frame。

push/pop 本身不得分配、GC、park 或回调。frame 在可能握手前完整发布，collector 在可扫描状态后观察；pop 只在入口返回且更新后的值已重读之后发生。region 的 scan、alignment 与 extent 必须匹配实际存储；用于 boxing 时，链顶 region 与 source place、TD inline layout 逐项相符。

generated managed caller 使用 NoGC、nounwind、有限时间的私有 leaf root 操作，不建立额外 native transition；所有非 fatal 出口必须平衡撤销。native 代码跨 safepoint 不得缓存 managed leaf 的旧副本。

`scoop_runtime_pin(ptr)` / `scoop_runtime_unpin(ptr)` 操作 pin 状态；`scoop_runtime_pin_handle(handle) -> ptr` 解析并固定当前对象。handle 操作校验 live slot/generation，非法值按 4.4 处理。

语言侧 PinnedPtr/GcHandle 仍为实际单字段 struct，普通 Scoop 调用使用 aggregate ABI；只有 C 边界采用 UInt64 透明表示。

### 4.3 Managed callback

callback 注册直接接收 ordinary、非 suspend closure，返回 GC-free opaque cookie。cookie 非零，可按目标契约在 uintptr_t/void* 间往返，但绝不解引用；slot 复用必须能检测 stale cookie。

注册持有 closure、binding snapshot 与首个失败的 managed 保活引用，以及完整 adapter/signature、mode、owner/active 数量和状态。整数 ABI 的 mode 为 Reusable=0、OneShot=1；state 为 Registered=0、Active=1、Completed=2、Failed=3。未知值为 fatal ABI error，这些 wire codes 不直接等于语言 enum tag。

静态 C trampoline 按 `(canonical C signature, context index)` 标识。context index 是 zero-based u32，必须指向真实签名中的 Ptr<Unit> 参数；其余参数与结果满足 C-FFI-safe。trampoline 去掉 cookie 参数，把剩余值传入有类型的 args/result storage，再进入 callback runtime。不能用未类型化 varargs 猜测 managed ABI。

invocation 执行 attach-if-needed、enter managed、保活 closure/snapshot、调用 typed adapter、leave managed，完成 token 状态和保活引用的释放后再 detach-if-owned。临时 attachment 覆盖整个 runtime 收尾，不能在仍需释放 GC handles 时让 shutdown 观察到线程与 callback 均已清空。adapter 在入口 poll 后建立独立 TaskContext，调用期间可分配和 GC；所有正常/异常出口先恢复 previous Context，再返回 C。

普通 cookie 值复制不增加 owner。retain/release 显式管理 ownership；owner 与 active lease 均为零后释放保活引用并回收 slot。Reusable 每次只增减 active lease；OneShot 原子 claim 并消费一份 worker ownership。需要读取完成/失败状态的 observer 必须预先 retain，创建失败时释放未转移 ownership。

adapter 在返回 C 前捕获全部 Scoop 异常、物化 managed Throwable 并结束 native catch；token 以 first-wins 保存失败，trampoline 按真实 C 返回类型返回全零值。observer 可在 API 同步点之后重新抛出。invalid/stale cookie、签名不匹配、OneShot 重复调用和 shutdown 后调用均 fatal。

任意 closure 导出要求 native API 有显式 context/user-data 槽。没有该槽时只允许语言规范规定的静态 FunPtr。

### 4.4 错误处理

无法分配 runtime/GC 元数据、非法或 stale handle、root LIFO 破坏、非法线程转换、callback cookie/signature 错误、无效 ABI 输入和 shutdown 后重新进入，均打印稳定诊断并终止。普通 callback 抛出的 Scoop 异常按 4.3 转换，不属于内部 ABI 错误。

### 4.5 线程状态

普通 C ABI outbound 默认使用 NativeSafe，Scoop ABI outbound 使用 NativeBorrowed；NoGC 声明不取消 caller 的协调与保活义务。NativeSafe 的常见进入/返回使用 3.5 的无锁协议，实际 C 调用期间不持有 world lock。foreign callback attach 后通过独立 boundary 进入 managed；返回 managed 与 callback 进入均不能绕过当前 GC 的协调。

显式 `@GCLeaf` C ABI outbound 按 3.5 保持原线程状态，不执行 NativeSafe/RETURNING 握手，不额外发布或撤销 caller roots。只有语言规范 13.4.1 的完整调用契约允许这种模式；GC-free 参数或 `@NoGC` 本身不能推导它。编译器按实现规范 2.4～2.5 选择直接 C ABI 调用或 storage bridge，两者保持相同的 native 签名和 GC 契约；NativeSafe 复用直接调用时仍须完成原有协调与保活。ABI 路径选择不增加 runtime 状态或运行时判断。release hook 的 raw leaf 调用仍按 3.8 单独检查，`@GCLeaf` 不放宽 release-safe 限制。

errno 捕获遵守语言规范 13.4.2 和实现规范 2.4～2.5：bridge 在实际 C 调用前清零目标 libc errno，返回后立即复制到 native 局部整数，再完成结果传递。NativeSafe 的捕获发生在返回握手之前；GcLeaf 和 release 的捕获保持各自原状态。结果使用本次调用独立且地址稳定的 GC-free storage 和整数返回，不修改冻结根或增加 GC 扫描记录。后续状态切换、GC、其他捕获调用及协程迁移不覆盖已经取得的值；runtime 不提供 `lastErrno` 槽位、捕获 setter/getter 或为此增加全局锁、attachment 和 collector 保存/恢复协议。libc 的 errno 本身在之后仍可变化。

边界可嵌套且严格 LIFO；每次恢复 managed 前按 3.5 协调当前 phase/epoch，并按 3.2、4.2 使用回写后的引用。detach 只能发生在 attachment owner 已退出全部相关 frames 后；RETURNING 归入 NativeSafe 的诊断/统计分类不放宽此条件。

### 4.6 Initialization coordinator

coordinator 只接收已登记的 InitializationUnitDescriptor。`scoop_rt_init_enter(descriptor)` 以 acquire 观察 cell，返回 RunInitializer、Ready、Failed(rooted Throwable) 或 Cycle(stable unit path)。无环等待兼容 GC 握手。

初始化等待者以 `PARKED` 发布已冻结的根，并按 3.5 在 GC 开始通知后确认当前 epoch。只有初始化已有结果且 world 为 `RUNNING` 时才能退出等待并恢复 managed；GC 通知或虚假唤醒都不能单独作为恢复条件。

RunInitializer 执行普通 managed initializer，完成 storage/published root 后调用 `scoop_rt_init_succeed`，以 release 发布 Initialized 并唤醒 waiter。Ready 重新读取已登记 storage。

异常在 active catch 内物化为 managed Throwable，再经 `scoop_rt_init_fail` 写入专用 failure root、发布 Failed、移除等待关系并唤醒 waiter。重新抛出从该 managed 对象建立新的 native record；cell 不保存 native unwind record。

Cycle 的 path 在复制为 managed String 之前保持稳定，随后调用已解析的 core thrower。runtime 不按名称查询 core 类型或构造器。invalid transition、未登记 descriptor 或非 winner 的完成操作为 fatal ABI error。

## 5. 异常

### 5.1 Exception record 与抛出

异常 ABI 使用 Itanium Level I `_Unwind_*` 接口。每条私有 exception record 含恰好一个满足目标对齐的 `_Unwind_Exception` 和按 exact TD 大小、对齐保存的对象 payload；record 内部 offset 不属于 generated-code ABI，只能经 runtime 入口转换。

`exception_class = 0x53434f4f50000000`。`scoop_rt_throw(object)` 要求 FixedObject shape，按值复制完整对象，在可能展开或 GC 前将稳定 payload 登记为 external root，再调用 `_Unwind_RaiseException`。原 managed 对象不需要 pin，也不能直接作为 unwind record。

`exception_cleanup` 是撤销 external root 和释放 record 的最终出口，只执行一次。unwinder 私有字段只由 unwind library 使用。`_Unwind_RaiseException` 返回表示无 handler 或 unwind 失败，必须在 payload 有效时打印诊断并终止。

### 5.2 Personality 与 landing pad

`scoop_eh_personality` 消费 LLVM 22.1 Itanium LSDA，只接受 Scoop catch-all 与 cleanup；源码 catch 类型在 landing pad 后按普通类型关系匹配，不使用 C++ RTTI。

LPStart 为 `DW_EH_PE_omit (0xff)`，call-site 编码为 ULEB128。有 catch-all 时 TType 为 `0x9b` 且只有一个 null type entry；TType 为 `0xff` 时无 type-table offset，所有 call-site action 为零，有效表在 call-site 表结束。

search phase 只由 Scoop exception 的 catch-all 选定 handler；cleanup-only 不终止搜索。cleanup phase 按目标 ABI 设置 exception/selector 寄存器与 landing-pad IP。`resume` 使用 `_Unwind_Resume`，不能代替源码 rethrow。

version、flags、encoding、call-site range 与 action chain 必须有效。未知 encoding、typed filter、损坏表或非 Scoop exception 进入 Scoop EH 区域，均终止；不能交给 BeginCatch 当作 managed 对象。

### 5.3 Catch 与重抛

`scoop_rt_begin_catch(raw)` 验证 exception class、压入当前线程 catch 栈并返回稳定 payload 引用。同一 active record 不得重复 BeginCatch。

源码 catch 选定后，在绑定变量前物化一次普通 managed Throwable。该变量可返回、存储或捕获；native payload 地址不能直接绑定为可逃逸源码变量。

每条正常离开 handler 的路径调用一次 `scoop_rt_end_catch()`。普通 record 被删除；rethrow record 只弹出 catch 状态并恢复为 in-flight。begin/end/rethrow 不分配 managed 对象、不 GC，也不向源码暴露 record 地址。

`scoop_rt_rethrow()` 要求 active catch，标记并重新 raise 同一个 unwind record，原 handler cleanup 不删除它。handler 抛出另一异常时，先结束旧 catch，再传播新 record。

native exception record 不能跨线程共享或跨挂起保存；可以保存的是 managed Throwable。

### 5.4 异常物化

`scoop_rt_materialize_exception(caught)` 按 payload TD 分配 managed 对象，保留新对象头，只复制头之后的 payload。复制期间 native payload 仍为 external root。

普通 catch、初始化失败与 suspend handler 使用同一物化契约。跨挂起或线程发布前必须结束 native catch，保存 managed 副本；后续传播重新调用 throw 并建立新 record。一次未物化的 rethrow 则保持原 record。

### 5.5 依赖与 FFI 边界

生产异常 ABI 只依赖 Itanium Level I，不依赖 C++ EH、RTTI 或 terminate。target profile 明确选择兼容 unwind provider；Linux CRT 的普通退出清理符号不视为 C++ EH。

启用 native C++ 的程序可以在 C++ 一侧使用配套的 C++ EH／RTTI／ABI 运行库，不改变 Scoop 异常对象和 personality 的合同。最终链接须协调两者使用同一兼容 unwind provider；GNU C++ 配置使用配套 libgcc_s 提供 Level I，Darwin 使用所选系统 provider，具体选择见实现规范 2.8。C++ 异常必须在 native 一侧捕获处理后再返回 Scoop；Scoop 不接管 C++ 对象的异常或析构语义。

除零、强制类型转换、Option unwrap、数组越界等语言异常，由 managed 代码构造实际 core 异常并抛出。native helper 不得把源码异常藏在穿越 FFI frame 的 unwind 中。

异常不得穿越 C ABI frame，违反属于未定义行为；Scoop ABI native callee 不得向 managed caller 展开，违反时终止。managed callback 和 startup gateway 在返回 C 前完成捕获、物化与 EndCatch。

## 6. 核心类型的运行时后备

后备遵守语言规范的普通 core 声明与实际 exact type，涉及分配时登记 native roots，GC 后重新取得地址。

标准输出后备为普通 C ABI `scoop_rt_stdout_write(const uint8_t *bytes, int64_t length)`、`scoop_rt_stderr_write(const uint8_t *bytes, int64_t length)` 和 `scoop_rt_flush_stdout(void)`，均返回 void。调用方提供非负长度和本次调用期间有效、稳定的字节区间；core 的 String 包装使用作用域借用。实际操作使用 stdout/stderr 的 stdio 缓冲；caller 按普通 C FFI 进入 NativeSafe，后备不访问可移动的 managed 引用，也不回调 Scoop。语言接口及缓冲、并发和错误结果规则见语言规范 14.4。

String 后备提供创建、拼接、内容比较/hash、UTF-8 长度、标量定位与切片等表示操作。定位 leaf 不抛源码异常：get 返回 Option<Char>，slice 定位返回 Option<(Long, Long)>，core 对 None 抛出 IndexOutOfBoundsException。内容与已验证边界未变化时可复用。

字节解码后备为语言规范 11.4 的 `fromUtf8`、`fromUtf8OrNone` 和 `fromUtf8Lossy` 提供同一套 UTF-8 规则。严格路径的结果明确区分成功 String 与首个非法子序列的零基字节偏移，并通过普通 Scoop ABI 返回；core 分别将失败转换为 CharacterCodingException 或 None，不对相同内容再做一遍独立校验。lossy 路径按 maximal subpart 消费非法输入，每段写入一个 U+FFFD，继续处理失配处的后续字节，不能遗漏合法后缀；空输入、U+0000 与合法 U+FFFD 按原内容保留。两种路径均不依赖 locale。

解码和复制只读取给定长度，输入属于 managed 对象时遵守 root/pin/relocation 契约；原生指针的可读性、稳定性和生命周期由 unsafe 调用者保证。结果复制到 String 自有存储，发布前完成全部初始化。实际输出长度、临时存储与分配的溢出按既有分配失败处理，不伪装为 UTF-8 错误或 None。实现复用现有 UTF-8 后备及已验证的不变内容。

严格解码后备接受非零字节指针和非负 Long 长度，返回普通 `(String?, Long)`：成功为 `(Some(string), -1L)`，失败为 `(None, byteOffset)`。该 tuple 的 storage 为 16 bytes、alignment 8，Option<String> 使用 nullable managed pointer niche，字段 offsets 为 0/8；按目标的 Scoop 间接返回 ABI 传递。core 的安全 Array 重载通过 3.4 的作用域借用调用同一后备，pointer 重载在进入后备前检查负长度；不为这些函数增加编译器异常角色。lossy 后备复用有边界的单步解码器，先计算实际输出长度，再分配和转换，整个过程保持输入稳定。

Option<Char> 的 storage 为 16 bytes、alignment 8，tag offset 0、Char offset 8；Option<(Long, Long)> 为 24 bytes、alignment 8，tag offset 0、Long offsets 8/16。实际 core 的 Some/None tag 为 0/1。结果使用 Scoop 间接返回：Darwin/AArch64 经 x8；Linux/amd64 经 RDI，并在 RAX 返回同一地址。

数组后备使用显式目标 TD 分配和浅复制，保持新对象头、logical size 和 canonical padding。ZST 不复制 payload；不能从来源名称或布局猜测目标 exact type。源码 bounds check 与异常构造由 managed 代码承担，受检 helper 收到越界 index 属于内部 invariant error。

`scoop_rt_string_join_parts(const ScoopArray *storage, int64_t part_count)` 消费 MutableArray<Option<String>> 的有效前缀并返回 String，使用 Managed native contract。prefix 范围有效，每项为 Some；字节总数 checked 求和。分配后从更新后的 backing 重读 String 引用，结果发布后不再修改 bytes；不调用用户 getter 或重新验证不变的 UTF-8 内容。

`scoop_rt_allocation_overflow()` 为无参数、NoGC 后备，在 Long 容量运算溢出时进入 fatal allocation failure，不分配、不回调、不引入新的源码异常。

integer Hash 与 compareTo 返回 Long。`==` / `!=` 执行编译器已选定的 operator equals；库相等执行普通 `Equality<T>.equalTo` 调用，字符串化／哈希按实际 ToString／Hash 声明执行，不提供按 Any 或地址兜底。Equality 不增加 runtime 比较入口，也不与 operator 分派绑定；浮点的两种相等均保持 IEEE 语义，结构 operator 使用 compiler 生成的逐字段／variant 正文。Iterator、Range、List、ArrayList 和 StringBuilder 的公开行为由普通 core 接口规定，不增加容器专用 runtime ABI。

### 6.1 Float / Double

Float 为 4-byte、4-aligned IEEE binary32，Double 为 8-byte、8-aligned binary64，均 GC-free。存储、复制、FFI 与 toBits/fromBits 保持全部位型，包括 NaN；不使用浮点 niche 替代 Option tag。

C FFI 使用目标真实 float/double ABI。数值操作遵守语言规范 11.2.2 的 rounding、NaN、无穷、signed zero 和转换规则，不能启用改变这些语义的 fast-math。线程的浮点环境采用 round-to-nearest ties-to-even、无 trap、保留 subnormal。

文本转换不依赖进程 locale。解析按声明精度正确舍入；非法输入和溢出返回 None，合法下溢仍返回相应浮点值。格式化遵守语言规定的可往返文本、特殊值与 signed zero 形式。

## 7. 程序启动与退出

启动先保存 2.8 的原始 argc/argv，建立运行时基础状态并验证全部 image/registration，再建立 GC、主线程与可执行 managed boundary。任何 managed initializer、Context 分配、argv 的 managed 构造或 main 之前必须完成相应登记；C coordinator 不提前构造未登记的 String 或 Array。

native C++ 的标准初始化／析构由目标 CRT／C++ 运行库负责；发生在 Scoop runtime 建立之前或 shutdown 完成之后的 native 初始化／析构不得调用 managed 代码或进入 Scoop runtime。它们不替代 Scoop 的 initialization unit、release hook 或 shutdown，也不由 Scoop 补调；立即终止进程的路径不保证执行 native 析构。

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

completion 的成功、失败与恢复具有唯一获胜方和 release/acquire 发布；未获胜恢复不能执行正文、改变 TaskContext 或消费状态。异常在挂起前物化为 managed Throwable 并结束 native catch，不能把 active native record 存入 frame。

每次真正进入或恢复协程正文时使用 frame 保存的 TaskContext，退出或传播异常时恢复调用者 Context。其语言行为见语言规范 8.3、11.9，Context 契约见第 9 章。

## 9. Task-local Context

### 9.1 动态绑定操作

TaskContext 属于逻辑任务，绑定以 exact static type 为 key。每个程序为已登记 key 分配 u32 slot；slot 只在本进程有效，不是持久化类型身份。

try-get 无分配、NoGC、nounwind，返回当前绑定或缺失。push 可分配，成功前保留旧绑定，成功时完整提交并返回可恢复 mark。restore 为 NoGC、严格 LIFO；snapshot 无分配，保存当前绑定；fork 得到独立任务上下文，保留绑定值身份。enter/leave 为 NoGC 并恢复前一 context。

这些操作的读取与写入具有真实内存效果，不能按 pure 常量跨绑定变更移动或合并。

### 9.2 保活与作用域

Context、binding、mark 与 snapshot 使用实际 managed 类型和 scan。当前 context、previous context、未消费 mark 及 callback/suspend snapshot 都必须保活，含引用写入遵守 3.6；消费后的 mark 不继续保活已退出 scope。

同一任务串行改变绑定。child task 继承创建时的 snapshot，后续重新绑定独立；binding 中原对象的身份仍可共享，不隐含深复制或并发同步。

### 9.3 Context key 登记

ContextKey 直接使用 PersistentExactTypeId，不另算类型身份。每个实际使用 key 的 body 有关联的 writable u64 slot cell，初值为 0；live 值为 `slot + 1`，可覆盖完整 u32 slot 域。

cell 与 callable 一起选择，通过 callable record 的 Context key uses 登记。先验证全部 key/cell 的格式、地址、归属与唯一性，再写入 cells；首个 gateway 前完成。不同 body 的 cells 可以映射到同一 exact key slot，不能依赖名称或装载顺序区分类型。

### 9.4 任务与 callback 边界

root Context 在首个 managed poll 后创建；普通同步 native 往返保留外层 Context。callback 注册捕获当时的不可变 binding snapshot，retain 共享快照；每次 invocation 从该快照建立独立 TaskContext。

callback managed adapter 的私有参数顺序为 closure、nullable binding snapshot、result storage、argument storage、exception output，返回 callback status。C gateway 保活 closure、snapshot 和 exception；adapter 在入口 poll 后、用户 closure 前进入 Context，所有正常/异常出口恢复 previous Context 并结束 catch，再返回 C。

协程只有获胜的恢复路径进入 frame Context；完成或异常离开时恰好恢复一次。任务执行不隐式增加取消或跨任务绑定修改语义。

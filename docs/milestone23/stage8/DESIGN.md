# M23-8 设计：多 image runtime 登记与启动

状态：设计完成，待实施（2026-10-01）。前置条件为已验收的 [M23-6a](../stage6a/ACCEPTANCE.md) 与 [M23-7](../stage7/ACCEPTANCE.md)。本文规定实施目标，不表示新的 runtime 路径或验收已经完成。

本阶段使多个独立 `.slib` 的实际机器产物共同启动：先登记完整静态元数据，再按确定顺序执行 eager initialization，最后经 no-throw gateway 调用 `main`。所有 Cone 共用实际 TypeDescriptor、ODR 实例、静态根与初始化状态，移动 GC 和异常可以穿过跨 Cone 调用链。

权威合同为 [语言规范](../../specs/SCOOP-SPEC.md) 9.1.3、11.7、12.3、12.5，[运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md) 2.2、2.7、2.8、3.3、3.5、7，以及 [实现规范](../../specs/SCOOP-IMPL-SPEC.md) 2.14。本文细化 [M23 总设计](../DESIGN.md) 第 6 章；同步撤销其中尚未实现的 program/core descriptor、Graph hash 重放和第二份初始化描述符要求。

## 1. 交付范围与实际基线

### 1.1 本阶段交付

| 能力 | 完成后的行为 |
| --- | --- |
| 多 Cone 启动 | 任意非空、闭合的实际 image 集合由同一 runtime 入口启动；输入枚举顺序不改变 eager 顺序 |
| 静态登记 | 六类 registration 全部解析、检查并去重后，才允许 managed allocation、initializer 和 callback |
| 类型与 ODR | 同一 exact type/body/storage/unit 使用真正 coalesced 的地址；独立 helper member 合法取并集 |
| GC | 所有 Cone 的静态引用、失败根、immortal object 和完整 stackmap 一次接入现有 collector |
| 初始化 | dependency-first、ready set coordinate tie-break、Cone 内 unit ID 排序；lazy/失败/cycle 保持既有语义 |
| native/managed 边界 | 每次 eager/root gateway 独立 enter/leave，首次 poll 前的空段、嵌套 callback 和 epoch 竞争均有明确处理 |
| 异常 | gateway 返回 GC-free 状态，异常只在 managed 段中捕获与物化；未捕获异常不跨 C startup frame |

M23 的 image 是最终主可执行文件内的 **logical Cone image**。三个 Cone 可以产生许多对象，但仍链接成一个 Mach-O；本阶段不实现 dylib、`dlopen`、卸载、运行期增加静态 image、全局析构、调度器或新 GC 算法。

M23-9 负责正式 `compiler/linker`、runtime-build 与启动对象生产；M23-10 负责一般 native provider；M23-11 负责正式 CLI 和历史单文件入口总迁移。这里必须用实际编译产物链接运行，但不提前建立这些组件的另一套实现。

### 1.2 已有能力及需要接通的位置

| 实际代码 | 已有能力 | Stage 8 的改动 |
| --- | --- | --- |
| [`scoop_runtime_metadata_v1.h`](../../../runtime/include/scoop_runtime_metadata_v1.h) | image、root entry、六类 prefixed record；metadata ABI 2；144-byte TD 固定部分 | 保留字段布局，接入 runtime 消费；因 init 参数迁移将 metadata ABI 升为 3 |
| [`runtime_metadata_v1/`](../../../compiler/codegen/src/runtime_metadata_v1.rs) | 实际 image/record 发射、typed relocation 与摘要 patch | 只保留统一 initialization registration，删除 coordinator 副本 |
| [`lir-lower/production.rs`](../../../compiler/lir-lower/src/production.rs) 与 [`safepoints/`](../../../compiler/lir-lower/src/safepoints/mod.rs) | root/eager gateway、入口 poll、invoke/catch、身份与 root plan | 验证入口合同，修正 root catch 的异常生命周期 |
| [`initialization.c`](../../../runtime/src/initialization.c) | cell 状态机、同线程/跨线程 cycle、可参与 GC 的等待 | 直接消费登记的 unit record；移除单表 eager loop 和每次 ensure 的静态重验 |
| [`gc.c`](../../../runtime/src/gc.c)、[`gc/roots.c`](../../../runtime/src/gc/roots.c) | 单表 roots/immortals、动态 roots、真实 moving collector | 使用完整 registry 的去重结果；heap 初始化不再读取固定单表 |
| [`gc/stackmap.c`](../../../runtime/src/gc/stackmap.c) | 连续 LLVM v3 blob、精确 PC、当前无条件拒绝重复 site | 保留规范化字段，结合 registration 判定合法 ODR 重复 |
| [`thread/`](../../../runtime/src/thread/transitions.c) | mutex/epoch 握手、native-safe/borrowed 与 callback 栈段 | 主线程 native-safe attach；明确 EntryPending；每次 gateway 独立边界 |
| [`platform/image/macho.c`](../../../runtime/src/platform/image/macho.c) | 主 Mach-O 的 stackmap/text section | 提供加载后 metadata 范围与权限，供通用登记代码使用 |
| [`rt.c`](../../../runtime/src/rt.c) | 固定 `scoop_main` 与 `scoop_image_*` 的历史启动 | 通用 runtime 不再拥有这个 `main`；新增生产 startup 函数 |
| [实际产物运行辅助](../../../compiler/driver/src/request/preflight/end_to_end_tests/imported_classes/runtime/execution.rs) | 从真实 Link 数据提取对象，再生成旧 root/immortal 表 | 新测试只生成 image/root 引用和 C 启动调用，直接消费原 registration |

M23-7 已验证模板、布局、ODR 定义和状态共享。Stage 8 不重新具体化模板、重建 HIR、补发 helper 或替 provider 创建 Strong 定义；缺少这些内容仍在 compiler/reader 边界报错。

## 2. 启动输入与职责边界

### 2.1 一个实际 C 入口

私有 startup header 声明：

```c
int scoop_rt_run_program(
    const ScoopImageDescriptorV1 *const *images,
    uint64_t image_count,
    const ScoopRootEntryDescriptorV1 *root_entry);
```

该入口负责整个 runtime 生命周期，每个进程只调用一次。正常 `main` 返回后完成既有 shutdown，返回 0；metadata 错误走 native fatal，未捕获 Scoop 异常按语言规范打印类型名后 abort。重复启动、初始化期间重入或 shutdown 后再调用均为 runtime 使用错误。

调用方提供位于最终主 image 中的静态只读 pointer array、非零 count，以及实际 executable root descriptor。root owner 必须在集合中；集合恰为沿 root dependencies 到达的完整闭包，缺失、重复或不可达 image 都拒绝。每个 image 的六类表仍是原有 record pointer span，空表保留既有 addressable sentinel；不能复制成 by-value 表。

M23-8 的测试胶合代码从共有 Link 结果取得 image/root 的实际 symbol 和 owner，生成普通 `extern` 引用、pointer array 与 `main` 中的一次调用。它不生成 image、registration、初始化状态、String TD 或缺失的机器实现。M23-9 使用同一 C 函数和数据形状生成正式启动对象；无需 program descriptor factory、Graph digest 或新的产物来源类型。

### 2.2 各边界实际检查什么

| 边界 | 负责的事实 |
| --- | --- |
| compiler / `.slib` reader | typed identity/key、语言与 ABI、完整 canonical 定义、静态初值、ODR member 内容和对象 relocation |
| 本阶段的实际对象链接 | 使用 reader 返回的完整对象及 ODR 合并结果，保留 image/root/stackmap；不补缺失定义 |
| runtime 加载边界 | prefix/span、实际加载地址与权限、跨记录引用、地址唯一性、静态 GC/初始化关系、最终 stackmap |
| 运行中的 GC/初始化 | 当前对象范围、动态长度、root 生命周期、cell 状态、线程握手与实际操作要求 |

runtime 没有完整 exact-type key、源码可见性、函数签名或 vtable slot count，不能声称从 digest、类型名、地址区间反推出这些语义。它使用带 kind 的 ID 和实际登记关系；reader 验证过的函数 ABI、slot 组成、canonical 诊断名称及 ODR 机器内容不在 C 中实现第二遍。

`RuntimeImageFingerprint`、record definition、layout、scan 和 callable body fingerprint 保留原有字段及用途；runtime 比较需要一致的镜像字段，但不重算 RuntimeImage、Graph、ObjectDefinition 或 ODR 全图。链接后 stackmap 的规范化摘要是本阶段实际消费的机器格式检查，见第 6 节。

### 2.3 String 与其他 runtime 依赖

`scoop_td_String` 继续链接到前端已经选择、产物中实际发布的 String TD。其地址必须出现在普通 type registry，表示保持现有 InlineBytes 合同。runtime 不从固定 CORE identity 重建 String，不按诊断名寻找它，也不另存 String layout/scan/capability 副本。

修改并重建 core 的真实 fixture 必须走同一入口。测试 runner 可以按既有 typed Link 选择设置 String symbol alias；这是实际符号绑定，不是新增的 metadata producer。

## 3. 在加载边界建立完整登记

### 3.1 先有实际内存范围，再读记录

Mach-O 处理只位于现有 platform image component。它提供主 executable 的静态映射范围、只读/可写/可执行属性和 stackmap section。slide 使用 dyld 已加载地址；header/load-command/span 算术须受实际映射边界约束。

含 relocation 的常量可能位于 `__DATA_CONST`，不能只按 section 名或原 `initprot` 推断当前可写性；Darwin adapter 必须结合加载后的 VM protection。codegen/target section placement 应使完成 relocation 的 descriptor、pointer table、scan、diagnostic 和 template 实际只读。callable 位于 executable range；storage/cell 的完整 extent 位于 writable range。

检查顺序固定为：

1. 验证入口 pointer array 与每个 descriptor prefix 的范围和对齐。
2. 读取 magic、metadata ABI 3、exact struct size 和 reserved fields，再验证完整 struct 范围。
3. 对每个 count 做 `count * sizeof(element)`、地址相加与 `size_t/uintptr_t` 转换检查，再访问数组；字符串一律按 byte span 读取。
4. 对 nested pointer 同样先检查范围，再按其实际类型消费。不能先解引用再判断目标是否已登记。

只读共享 diagnostic bytes、scan DAG child 和空表 sentinel 可以共址；不能把“任意 readonly atom 不能共享”作为额外限制。OOM、真实整数溢出、越界和非法环有明确错误出口，不引入节点/字节/访问次数预算。

### 3.2 收集、关联、发布

登记按三个内部步骤完成，不新增对外验证状态机：

1. 从全部 image 收集 coordinate/dependencies 与六类 record，形成尚未对 GC 可见的 native 索引。
2. 在完整索引上解析引用、验证地址关系、初始化初态、scan/shape 及所有 stackmap；构造 canonical eager 调用序列。
3. 全部成功后一次性把不可变索引交给 GC/初始化，建立 heap/handle 状态并 attach 主线程。

第一步不能执行某个 image 的 initializer。失败时丢弃临时索引并 fatal，cell/storage 尚未被 coordinator 修改，也没有 managed 对象或用户副作用。索引使用普通 C 数据结构，按实际查询需要采用排序数组和二分查找；不引入通用 registry 插件或发布凭证。

冻结的是静态 image/record 集合。native root frame、exception stable external root、handle、pin 与 thread attachment 继续按现有运行期协议变化。

### 3.3 身份、地址与 ODR

六类 semantic ID 分别建立 typed record 查询；不能把同样的 32 bytes 跨 kind 比较。registration member 另核对 `(group, member)` 对应唯一的 kind/semantic ID/record。需要反向索引的地址如下：

| 记录 | 关键地址和关联 |
| --- | --- |
| StaticStorage | record、writable base、完整 allocation extent、scan、初值 relocation targets |
| ImmortalObject | record、精确 object start/size、type registration |
| InitializationUnit | record、cell、storage、专用 failure root、initializer/ensure/startup entries |
| TypeRegistration | record、TD、非零 runtime type ID、相关 TD |
| SafepointRegistration | record、非零 SafepointId、完整 site ID、owner body、最终 return PC |
| CallableRegistration | record、body ID、entry、body definition fingerprint |

规则为：

- Strong record 只能由一个 image 列为 producer。同一个 pointer 在两个 image 的 producer 表中重复也失败；普通 external use 不重复登记。
- ODR 同 member 重复必须指向同一个 coalesced record；semantic ID、group/member、definition fingerprint 和关键目标地址一致。不同 record address 即使内容一样也失败，runtime 不选择 winner。
- 对已检查的同一个只读 ODR record，再次出现时复用结果，不反复比较整个模板、scan 或 descriptor。
- 同组的不同 helper/adapter member 合法取并集，不要求 sibling 的全部 member 集合相等。
- `PersistentExactTypeId ↔ TD address` 与 `PersistentCallableBodyId ↔ entry address` 一一对应；相同布局或相同代码不允许不同身份共址。
- runtime type ID 与 exact ID、SafepointId 与完整 site ID 分别一一对应。64-bit collision 先报错，不能用不同 PC、owner 或类型名消歧。
- 不同 storage 的 allocation range 不重叠；不同 immortal range 不重叠；cell 不与任何 storage 或另一 unit 的 cell 重叠。ODR 去重在 range 检查之前完成。

这些索引的目的就是让 GC 扫描一次真实对象、让初始化共享一次真实状态；不增加来源资格或新的 ODR 身份。

## 4. 类型、扫描、静态根与 immortal

### 4.1 TypeDescriptor 与 scan

使用 M23-7 的 TD：固定部分 144 bytes，尾部为 `related_type_count * 8`。`relation_kind` 0/1/2/3 继续表示无关系、ordinary function、suspend function、interface；计数、结果与空引用遵守现有合同，function 参数/结果中的 null 明确表示 Any。M24 的 release hook 不在本阶段。

登记一次核对 TD 的实际范围、runtime type ID、shape 合法矩阵、layout/descriptor fingerprint 镜像和已登记的 parent/interface/function type 引用。parent 与 interface 继承边检查非法环；不能把所有函数签名、字段类型关系当作必须无环的继承图。vtable/itable 的已知范围与 interface key 需要有效，但没有 slot count 的数组不能靠扫描到 null 来重建；slot 数和逐槽 ABI 已由编译器/对象 reader 负责。

scan 复用 `value_shape.c` / `value_scan.c` 的实际格式与 shape 算法。所有节点先验证 readable readonly 范围，再读取 count/child；active path 检查环，共享 child 用访问记录避免重复展开。结构检查按 node address 复用，涉及 extent/translation 的检查按相应上下文复用，不能把一次小对象检查误当成另一布局的结果。

References 检查 alignment、offset 与实际 extent；Sequence 按已有完整 scan 组合；Array 检查 length header、first offset、非零 stride 和 element scan。variable object 的实际 count/size 仍在分配和扫描该对象时核对。ZST/GC-free array 无 element scan，不因 logical length 很大而遍历空元素。

登记之后，普通分配、装箱、数组与 GC 消费同一 TD/scan，仅保留注册地址与本次对象、length、alignment/root 等动态检查。`SCOOP_VERIFY_METADATA=1` 继续提供显式重型核对，不成为每次分配的默认开销。

### 4.2 StaticStorage

`allocation_extent == max(byte_size, 1)`；logical ZST 保留独立 1-byte token、None scan 和零初值，不加入 GC root 列表。其他 None storage 参与身份/初始化检查；Recursive storage 才以可更新 region 加入静态根。

`ZeroedForRuntimeUnit` 只用于已有 unit storage 和 unit/root failure storage。登记时在任何写入前检查完整 extent 为零、cell 为 `{Uninitialized, null}`，以及封闭角色和 owner 关系；冻结后绝不重复检查这些会改变的初态。

`EncodedStaticValue` 没有初始化 unit。compiler/reader 验证 template 的值、padding、canonical bytes 和对象 relocation；runtime 验证加载后 template/relocation span 与实际 GC leaf：每个非零 leaf 恰好对应一条 relocation，目标为已登记只读 immortal 的精确 object start，其余 leaf 为 null。offset 对齐、严格递增、不重复、落在 byte_size 内且命中实际 scan leaf。不得把 interior pointer、任意静态地址或 heap pointer 当初值。runtime 不重建值类型或重新逐字节证明非引用 payload/padding。

### 4.3 Immortal 与失败根

先收集全部 immortal，再解析静态初值的目标，允许 provider 和 consumer 的枚举顺序任意。immortal 的完整 range 必须只读、size/alignment 符合 TD、对象头指向同一个已登记 TD，且 object scan 为空。可变长 String 的实际 length 与完整 size 仍需 checked 核对。

unit/root failure root 是专用 storage：8-byte size/extent/alignment、Recursive `[1, 0]`、Zeroed 初态；不能兼作 property/published slot、ZST token 或另一 unit 的失败槽。同一已 coalesced ODR unit 可以被多个 image 引用；不同 unit 不共享 failure slot。

GC 使用登记结果中的唯一静态 root/immortal 集合，不再合成 `ScoopManagedGlobalDescriptor` 或旧 immortal by-value 表。动态 exception root、native region frame 等保留原入口和生命周期。

## 5. 初始化记录统一与调度

### 5.1 去掉 coordinator 副本

当前 `InitializationDescriptor`（`id` symbol）保存 schedule、unit ID、C 字符串、cell/storage/failure 和两个 entry；`InitializationRegistration`（`nr` symbol）已经保存上述实际关系及登记所需内容。继续让 runtime 同时消费两份描述符，会使加载边界无法沿一份 record 检查 generated ensure 实际使用的状态。

本阶段让 `scoop_rt_init_enter/succeed/fail/failure/cycle_message` 全部接受 `const ScoopInitializationUnitDescriptorV1 *`。LIR/codegen 的 unit 引用直接定位 `nr`；coordinator 从其 storage/failure registration 取得地址，cycle diagnostic 按 byte span 复制。`cell`、状态机、返回状态码、普通 initializer/ensure 的 managed ABI 保持。

删除旧 coordinator struct/atom、`id` symbol、definition plan、digest node 和外来 coordinator 物理引用，不留下空 descriptor 或 startup 生成的适配副本。普通 unit 仍由定义 Cone Strong 拥有；generic delegate 仍沿同一 group 的 `RegistrationRecord` member 引用。完整记录和必要引用由数据结构保证，不能用 nullable registration 等待 runtime 修补。

### 5.2 eager 的唯一顺序

image 输入无序，runtime 根据实际 dependency IDs 构建一次图。root owner 必须存在，每个 dependency 存在且唯一，自环、环、同 `group:name` 多版本及 root 不可达的额外节点失败。library/executable kind 和 source entry 合法性属于 Link 输入检查，runtime 不从没有 kind 字段的 image 猜测。

采用与语言规范相同的 Kahn 算法：只把全部 dependency 已处理的节点放入 ready set，每次按 `(group UTF-8 bytes, name UTF-8 bytes, canonical version)` 取最小者。不是一次分层后按层排序。每个 Cone 的 eager unit 按 persistent unit ID bytes 排序，diagnostic path、表下标和函数地址都不参与排序。

lazy object/companion/generic delegate 只登记。eager 执行中可以通过普通 ensure 提前初始化另一个 unit；该 unit 排到自身位置时，ensure 观察 Initialized 并返回，不重复副作用。EncodedStaticValue 没有 unit，启动不为它补 cell。

### 5.3 状态、失败和循环

复用已有 `Uninitialized → Initializing → Initialized | Failed` 协议和 thread registry mutex。一个 winner 执行完整 initializer；成功后发布 storage，失败后发布 rooted managed Throwable，唤醒 waiter。状态只管理完整初始化，不在 property 中增加 late-init bit。

同线程 dependency stack 与跨线程 wait-for edge 检测运行期环；检测到 cycle 的当前 ensure 抛出由前端已选 thrower 构造的 `IllegalStateException`。initializer 内可以普通捕获该异常；只有异常逃离 initializer 才使该 unit Failed。间接普通调用、dynamic dispatch、default 或 callback 引起的环由同一协议处理，不反向分析或补造静态 unit 图。

无环等待参与现有 safepoint/epoch park，不持有未登记 managed pointer 睡眠。failure root 与成功引用始终从实际 slot 重新加载，moving GC 后不得从 cell、native 局部或副本读取旧地址。generic delegate 的两个 sibling 必须共享同一 cell、storage、failure 和 entry；已失败 unit 不重试。

每次 eager gateway 成功后继续下一个；第一次失败立即停止全部后续 eager 和 `main`，不回滚已发生的副作用。运行期 unit error 与编译期直接依赖环的既有诊断分开。

## 6. 全部 stackmap 与合法 ODR 重复

### 6.1 在现有 parser 中保留所需信息

继续解析主 executable 中全部连续 LLVM v3 blob，按每个 header 的 function/constant/record counts 和 alignment 消费到 section 末尾。保留原始 function address、instruction offset、stack size、完整 locations、live-outs 和诊断 offset；不能只保留 GC root pair 后再推测丢失信息。

先有 callable/site registry，再关联 raw record。原始 function address 必须等于 owner callable 的 entry；`return_pc = function_address + instruction_offset` checked 计算并保留原值。Darwin/AArch64 roots 继续是 `3 + 2 * root_pair_count` locations，前三项为规定 constant，root pair 为规定的 8-byte SP/FP Indirect。

ConstantIndex 从实际 pool 解析成 Constant，signed offset 按现有规则编码；locations 和 live-outs 的顺序、数量均保留。按运行时规范 2.8 与已有 [产物规范化实现](../../../compiler/slib/src/link_object/stackmap_normalization.rs) 的 runtime scalar/count 编码计算 `SHA-256(u64_le(domain.length) || domain || RuntimeEncode(record))`，domain 为 `scoop-stackmap-record-v1`，与对应 registration 的 normalized fingerprint 核对；此记录不使用 Wire CBOR。SHA-256 使用成熟系统实现；当前 Darwin 可在其平台支持代码中封装 CommonCrypto，不手写算法、不引入加密授权或通用 hash-provider 框架。

### 6.2 覆盖与去重

| 输入关系 | 处理 |
| --- | --- |
| 一个 Strong site 对应一个 raw record | 核对 owner、payload、fingerprint，接受 |
| 同一 ODR site 的多个 blob record | 仅当完整 site ID、group/member、body、最终原始 PC、canonical locations/live-outs 和 fingerprint 全同才合并 |
| 同一 site 的 PC 或 payload 不同 | fatal，不能选择其中一份 |
| 不同完整 site 共用 64-bit ID 或 PC | fatal，即使 body/代码一样 |
| raw record 找不到登记，或登记的 site 没有 raw record | fatal，指出 site 或 blob offset |
| 已登记的 NoGC callable 没有 site | 合法，不为它生成假的 stackmap |

产物 reader 负责不同 source key 是否被错误编码为同一完整 ID；runtime 只能检查实际持有的完整 ID 和加载地址。相同 member 的定义内容已由 Stage 7 比较，runtime 不再反汇编或重放 canonical LIR。

全部 blob 通过后生成一个按原始 return PC 查询的最终索引，直接交给 GC。heap 初始化不再次读 section；GC 不使用 `PC-4`、最近符号或 code range fallback。body→entry 的一一关系继续禁止 function ICF；target 继续拒绝 `-dead_strip` 和 managed tail call。对 section retention 的要求适用于 root/gateway 没有普通用户分配的程序。

## 7. 每次 gateway 的线程与异常边界

### 7.1 进入、首次 poll 与返回

attach 只建立主线程 native-safe 状态、TLS、栈界限及空 roots，不把 runtime `main` 当永久 managed boundary。每个 eager/root 调用都有实际 C wrapper 栈帧，保留 frame pointer、禁止内联/尾调用破坏该边界。

1. wrapper 保存外层 mode/boundary，建立本次 boundary，并在既有 mutex/epoch 协议中发布 `EntryPending` 与 managed mode。
2. EntryPending 表示新段没有可扫描的 managed frame。遇到 Stopping/epoch 变化时可以按空新段 park，外层 frozen segment、native roots 和 callback handle 仍照常扫描；不得把 C PC 当 anchor。
3. gateway 使用既有 root/init body identity。入口 poll 是第一条可握手操作，平台薄入口捕获该 poll 的真实 PC/SP/FP，在同一同步协议下激活新段；随后才能分配、调用、抛异常或访问 managed 值。
4. gateway 的正常和失败出口都只返回 `uint32_t`。返回后 wrapper 在同一同步协议下发布 native-safe、撤下本次 boundary 并恢复外层记录，不能以已经没有 managed frame 的状态尝试按活动段 park。

这是实际线程状态扩展，不是新的语言实体、持久化 ID 或验证 token。活动段在 GC 扫描时没有 anchor 仍是错误；不能把所有 null anchor 当成空段。运行中暂未停下的 active managed 线程继续等待正常 safepoint。

现有 mutex 能提供与 collector 状态发布互斥的 handshake 时直接复用，不为满足“再次检查”的字面顺序另造一套锁外原子协议。必须以测试证明 collector 不会把刚离开 native-safe 的线程错当 quiescent，也不会读取已经离栈的 boundary。

两次 gateway 之间 C coordinator 没有活动 managed 新段。nested callback 沿已有线程/transition 链独立进入和 LIFO 恢复，保持外层 native-safe/borrowed 的 roots；不新增 callback runtime 或 scheduler。

### 7.2 root 与 eager 的失败数据

LIR 已有两种 gateway body key 足以表达身份与用途，不另加 `NativeGatewayEntry` ID、证书或单独 table。完整 LIR 验证这两种 body 的 C `uint32_t(void)` 签名、入口 poll、完整 catch 和 `0 | 1` 返回；所有实际新增 managed call 使用现有 root plan。

root gateway 捕获异常时按顺序执行：`BeginCatch` → 已有 `MaterializeException` managed runtime call → 写专用 failure root → `EndCatch` → 返回 1。`BeginCatch` 的 payload 是 native exception record 中的副本，不能保存该地址后 `EndCatch` 释放它。物化期间保留 native exception 的既有 stable external root，写入 failure slot 后成为普通 managed root。

eager gateway 调用同一个 unit ensure。ensure 在自身异常处理内已把 managed failure 发布到该 unit 的 failure root 并转为 Failed；gateway 只平衡自己的 catch 并返回 1，不再次物化、覆盖失败对象或创建第二个 root。返回 1 但 unit 不是 Failed 或 failure slot 为空，属于 invariant error。

gateway 状态 0/1 之外的值 fatal。root status 1 同样必须有完整 failure root；成功路径不会消费它。非法 foreign unwind 继续沿现有异常 ABI fatal，不穿过 C coordinator。

### 7.3 未捕获异常的 native 报告

wrapper 已返回 native-safe 时，C reporter 只能持有稳定的 failure **slot 地址**。在既有 world/mutex 协议保证 GC 不在移动的短临界区中，从 slot 读取异常并取得只读 TD/diagnostic span，随即结束对 managed 对象的访问；不能缓存异常地址后等待、分配或跨 handshake 再读取。

报告只输出已有 canonical 类型名与必要的 startup unit path，再按语言规范 abort；不调用用户 `toString`、property 或 reflection，不为诊断增加 managed gateway、String 资格表或新异常类型。完整 unit cycle 路径仍由第 5 节的普通 managed 异常 message 保存。

## 8. 编译器、wire 与旧入口迁移

### 8.1 保持单一的生产数据

HIR 的初始化语义与 MIR 的 initializer/ensure body 不变。LIR 保存实际 unit registration 引用，codegen 将同一个 pointer 用于 managed init runtime call 与 image producer table；外来引用沿已有 provider/typed subject/definition/relocation 路径。codegen 不依赖 HIR/MIR lower 实现，runtime 也不接收编译器进程内状态。

root gateway 的物化 call、unit descriptor relocation、startup poll 与新增 site 进入已有 canonical LIR、ObjectDefinition、registration 和 Code/RuntimeImage 摘要流程。relocation 只编码目标 typed identity，不内联目标 definition digest；ensure 指向自己的 unit record 不形成 digest 递归。

### 8.2 版本和退役项

实施时按 [实现规范 2.14](../../specs/SCOOP-IMPL-SPEC.md) 同批迁移：

| 项目 | Stage 8 目标 |
| --- | --- |
| metadata prefix ABI | 3；八种 record 的字段/size、144-byte TD 固定部分保持 |
| `cross-cone-generic` profile | /2 |
| `cone-production` | /4 |
| `cross-cone-layout-link-closure` | /3 |
| `link-identity-closure` | /8 |
| `scoop-lir` object verifier | /4 |

旧 `InitializationDescriptor` 的 mangler tag 10（`id`）、Strong role 10、ODR role 11 和 external shape subject tag 10 退役。保留原 `InitializationRegistration` / `RegistrationRecord`；external shape subject 新 tag 11 为 unit 的 registration。registration plan 退役 fields 15、16、17、26，保留其余编号，并从 dependency plan 删除同一 coordinator 分量。退役字段/tag 不复用，不用保留的名称暗换旧 layout。

HIR `/43`、MIR type bridge `/6`、LIR layout ABI `/5`、manifest production `/2` 与 outer schema 1 不因 runtime 接通而升级。source/exact/application/unit/body/ODR group 的身份公式保持；删除的 member 不再进入目录，受影响的实际定义、ODR、profile 和 runtime ABI fingerprint 更新。

新 reader/publisher/runtime 原子切换，core、provider、consumer、runtime 与构建缓存重建；旧 prefix ABI 及旧 profile 有明确拒绝测试。版本是设计目标，实际发布前不得把当前 Stage 7 的 ABI 2 标记为已迁移。M24 的 generic profile 顺延为 /3，prefixed record 继承 ABI 3；release-hook/TD/outer schema 的后续变更不混入本阶段。

### 8.3 历史单文件 harness

`rt.c` 的固定 `main` 与通用 runtime functions 分离，正式 runtime 源集不再要求 `scoop_main` 或 `scoop_image_*`。新的多 image 路径只消费本设计的实际 descriptor。

所有需要链接 runtime 运行的历史 fixture，本阶段将启动胶合接入同一完整 image 输出与 startup 函数。既有 test runner 可以继续显式编排共有的完整 request 和 fixture-native 输入，不要求尚未落地的 `scoop build <file>`；纯前端/IR golden harness 保留。M23-11 再把运行编排迁到正式 CLI 并删除历史内部入口。

不得复制旧 runtime main/coordinator、同时解释两种 unit struct，或在新登记失败时 fallback。已有 runtime 单元测试可直接构造所测结构，但不能为它们保留旧生产格式。

## 9. 真实 fixture 与验收矩阵

### 9.1 独立与组合场景

在 `tests/fixtures/m23-runtime-images/` 组织 manifest-backed Cone。至少使用 core、普通 provider、consumer/root 三个实际 image；diamond/ODR 场景增加 left/right sibling。provider 源码产生 `.slib` 后移走，再仅凭产物编译 consumer；已有 Stage 7 generic/delegate/function/coroutine fixture 应复用。

| fixture 组 | 必须观察到的行为 |
| --- | --- |
| empty / ordinary / NoGC main | 空 registration 分表合法；有入口 poll；程序实际进入唯一 root gateway 并正常 shutdown |
| chain / diamond / ready-order | dependency-first；每轮 ready set tie-break；Cone 内 unit ID 顺序；打乱 artifact、image、source 枚举后结果一致 |
| early ensure / lazy | eager 间接提前 ensure 后不重复执行；object/companion/generic delegate 在首次访问前无初始化副作用 |
| roots / encoded values | 跨 Cone property、singleton、immortal String/Option 初值、ZST token、大值和含引用聚合；首次 initializer 即可移动 GC |
| ODR identity | sibling 共用 TD/body/storage/cell/failure 的实际地址；不同 application/ZST 类型保持不同地址；独立 adapter member 取并集 |
| failure / cycle | eager 失败使后续 eager/main 不执行；lazy 失败不重试；同线程间接环、跨线程等待环、initializer 内捕获 cycle |
| gateway exception | root 异常在 EndCatch 后仍是合法 managed failure；eager 保留原 unit failure；类型报告与 abort，C frame 不参与异常展开 |
| function / coroutine | 跨 Cone closure、static/dynamic function adapter、泛型 coroutine suspend/resume、异常/finally 和引用存活 |
| rebuilt core | 实际重建 core 后，String 与新增普通/泛型类型使用真实 TD、接口表和引用路径 |
| multi-object stackmaps | 同一及不同 Cone 的多个对象贡献完整 blob，零 root site、ODR 重复、真实 nested managed 调用均命中精确 PC |

每组运行适用的普通模式和 `SCOOP_GC_STRESS_MOVE=1`。typed HIR/MIR golden 锁定既有语义；LIR/object golden 明确锁定唯一 unit registration、真实外来 relocation、gateway poll、root 异常物化和新增 site，不能只断言文件能解码。

### 9.2 损坏与错误边界

| 输入 | 应拒绝的边界 |
| --- | --- |
| 旧 ABI/profile、缺失 prefix、错 size/tag、非零 reserved、null/越界/错对齐 span、count 溢出、metadata 可写 | reader 或 runtime prefix/range 边界，访问损坏 nested pointer 前结束 |
| 缺失 dependency、环、重复/额外 image、root owner 缺失、同名多版本 | 完整登记，任何 managed 副作用之前 |
| Strong 重复；同 ODR member 不同 record/TD/entry/storage/cell 地址；不同 ID 共址 | registry 身份/地址关联 |
| runtime type/site 的 64-bit collision | 关联 raw stackmap 或运行类型查询之前 |
| 非零初始 cell/failure、复用 failure slot、重叠 storage/token/cell、坏 scan child/环/offset | 静态 GC 与 init 关联，任何 initializer 之前 |
| 初值 ref 指向 immortal 内部、未登记目标，TD/header/size 不符 | 加载后的静态 root/immortal 关联 |
| non-reference template/padding 或 canonical body 损坏 | 现有 object/reader 检查；不增加 runtime 全量内容重放 |
| 丢失第二 blob、额外/缺失 site、错误 owner/PC/location/live-out/fingerprint、非法 ODR 重复 | stackmap 登记，heap 和 managed startup 之前 |
| 活动段缺 anchor、status 非 0/1、失败状态无 failure root | 本次运行期 invariant 边界，不能声称都能在 startup 静态判定 |

runtime C 单测可直接构造所需错误输入或使用已有 fake platform；真实产物故障注入只改已知对象字段/relocation 并运行同一 consumer，不新增通用 program descriptor 生产工厂。断言稳定的错误类别、record kind/ID、字段或 blob offset；没有源码位置的错误不能伪造 source span。

### 9.3 线程竞争与验证方式

线程回归用现有 barrier/condition variable 控制顺序，不依靠 sleep 碰运气。至少覆盖：第一次 gateway enter 前已有 GC 请求、EntryPending 停顿、gateway 首次 poll、两次 eager 之间、gateway 返回到 native-safe 发布之间，以及 initializer wait 与 nested callback 中的 GC。断言边界 LIFO、完整 outer roots 和无 C-frame walk。

并发 exactly-once 测试覆盖一名 winner、多名 waiter、成功/失败发布和跨线程 cycle；移动 GC 发生在等待和失败后的再访问之间。C 探针只持有正确登记的 managed roots 或静态 TD/entry 地址，不能让测试自身违反 GC 契约。

每批实现先 `cargo fmt --all` 与 `cargo clippy --workspace --all-targets`，再执行受影响的 compiler/runtime 测试。最终完成实际配套 `scoopc` 构建、全 workspace 回归及本节多 image 运行矩阵，关闭自动更新 golden 的环境变量。新增 C 文件沿现有 target flags 和测试入口编译；实际 metadata C/LLVM `sizeof/offsetof`、新旧版本拒绝与 target section 权限检查一并覆盖。

## 10. 实现顺序与完成门

1. 迁移统一 unit record、实际 symbol/role/plan/relocation 和格式版本；同步 reader/publisher，保证旧格式明确拒绝。
2. 修正并锁定 root gateway 的异常物化，复用既有 poll/root plan；保证每个 gateway 输出完整。
3. 扩展现有 Darwin metadata range 读取，接入六类 record 的收集、地址去重、类型/scan/static root/init 关联。
4. 扩展现有 stackmap parser 的规范化记录和 registration join，形成唯一 PC 索引并供 GC 直接使用。
5. 使 GC roots、immortals 和 init coordinator 消费同一登记结果，删除新路径对旧单表的依赖。
6. 完成 native-safe attach、EntryPending、逐次 gateway wrapper、异常报告与 shutdown；复用已有 thread/callback 同步。
7. 把真实 3+ Cone artifact fixture 接入 startup 函数，完成独立、组合、损坏与受控线程竞争矩阵，再完成全量回归。

代码按实际职责拆分，例如 `runtime/src/metadata/` 保存 range/record/index/stackmap join，`startup.c` 保存编排；既有 initialization、GC、platform 与 thread 文件继续拥有各自算法。不要把所有检查塞进一个大文件，也不为六类记录引入可扩展注册插件。

只有以下条件全部成立才完成 M23-8：实际 image/registration 是唯一生产输入；首个 managed initializer 前完成整个集合登记；所有静态 roots、TD、callable、ODR unit 与 raw stackmap 对应真实链接地址；canonical eager 顺序、lazy、失败、cycle、移动 GC、异常和逐次握手均由运行验证；被替代的 coordinator 副本和新路径单表依赖已删除；新格式、文档、golden 和回归一致。

向 M23-9 交付的是已运行的 startup C 入口、私有 metadata ABI、完整 Link 数据及实际 fixture。正式 linker 只需生成实际 image/root 引用、构建 runtime 并完成对象链接和输出检查，不重新设计 runtime registry，也不借助 compiler 内存或 provider 源码补全产物。

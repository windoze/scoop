# Scoop 编译器与产物规范

本文规定编译阶段的输入输出、实体身份、目标 ABI、`.slib` 格式、构建命令、缓存与链接契约。源码语义见 [语言规范](SCOOP-SPEC.md)，对象表示、GC 与 native 运行边界见 [运行时规范](SCOOP-RUNTIME-SPEC.md)。

## 1. 范围与目标兼容性

一个 Cone 是一次独立编译的单位。library 和 executable Cone 都产生完整 `.slib`；executable 的最终程序由独立链接阶段从产物闭包生成。

编译请求明确选择相容的 LIR target、backend、C bridge toolchain、runtime build 与 final-link 配置。阶段只消费其所需投影；library 编译不要求未使用的最终 linker、CRT 或 unwind archive，产物链接不要求源码编译器或 LLVM 可执行工具存在。

| canonical target triple | LIR target profile | 对象格式 | 平台契约 |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | `org.scoop-lang.target-profile/darwin-aarch64/1` | Mach-O relocatable | Darwin/AArch64 |
| `x86_64-unknown-linux-gnu` | `org.scoop-lang.target-profile/linux-x86-64-gnu/1` | ELF64 relocatable | Linux/amd64、glibc |
| `x86_64-unknown-linux-musl` | `org.scoop-lang.target-profile/linux-x86-64-musl/1` | ELF64 relocatable | Linux/amd64、musl |

这些目标采用 little-endian、64-bit data/code pointer 与 16-byte 调用栈对齐。合法非 null pointer 可经内部 64-bit carrier 逐 bit 往返，null carrier 全零。data、code、managed 与 metadata pointer 的语义仍不同，不因此增加源码整数转换。

backend contract 使用 LLVM 22.1、LLVM stackmap v3 和本规范的 GC/EH 约束。Linux 两个 libc target 共用 `org.scoop-lang.backend-profile/llvm-22-1-linux-x86-64/1`，但 native、runtime 与最终链接输入不能混用 glibc/musl。target、对象格式、调用约定、pointer/storage ABI 及实际 producer 配置必须相容；不支持的组合在生成目标产物前诊断。

## 2. 编译阶段与产物契约

| 阶段 | 输入 | 输出与责任边界 |
| --- | --- | --- |
| parser | 当前 Cone 的完整源码集合 | 带准确源码位置的 AST 与语法诊断。 |
| HIR | 当前 AST 与依赖 Export HIR | 完成源码语义检查，分别输出 ExportHir 和 LocalConcreteHir。 |
| MIR | LocalConcreteHir 与实际选用的依赖 MIR metadata | 完整 concrete 类型、callable、显式控制流及执行 ABI。 |
| LIR | MIR 与实际选用的依赖 LIR metadata、LIR target | 完整布局、ABI、GC roots、符号与物理定义要求。 |
| codegen | 当前 LIR 与相应 producer 配置 | 目标对象、实际定义／引用与 GC/EH 数据。 |
| `.slib` | 三层 metadata 与完整对象集合 | 可独立消费的确定性归档。 |
| program-link | `.slib` 依赖闭包、runtime/native 对象、final-link 配置 | 定义与引用闭合的 executable。 |

stage 只通过对应 IR/metadata 交换数据，不调用上游 stage 的实现。所有源码语义错误在 parser/HIR 结束；后续阶段报告自身的格式、引用、ABI、目标或链接错误。已验证且不变的数据在后续边界复用。

### 2.1 Parser / AST

源码集合非空，文件具有唯一 `SourceIdentity { cone, logical_path }`。manifest Cone 的 logical path 为标准化的 Cone-relative `src/...`；single-file 为 `main.scoop`。host 路径只用于本次 I/O 与诊断。

AST 保留原始源码顺序、byte span、显式类型实参、位置／命名／spread／尾随 lambda 实参，以及独立的声明、表达式、模式和控制转移结构。文件至多有一个开头 package，其后为 import，再后为声明。qualified path 为非空 identifier sequence；package、静态 owner 与实体的分界由名称解析决定。

AST 不包含依赖 locator 解析、构建顺序、已推断类型或实体身份猜测。语法恢复产生 Error；任何解析失败阻止成功 HIR 输出。

### 2.2 HIR

HIR 完成名称解析、类型检查、重载与泛型实参选择、可见性、继承与覆写、模式穷尽性、构造／初始化规则、常量、效果及 FFI 声明检查。源码与依赖声明遵守同一语言规则；文件发现顺序、声明存储位置和 provider 读取顺序不能改变结果。

`SemanticHir` 保存已检查的声明与正文，对外有两个不同的输出合同：

- **ExportHir**：跨 Cone 的声明与必要正文。区分 public/re-export lookup、protected inheritance/slot、generic hidden support 和 interface dependency；支持声明的存在不扩大可见性。保存签名、默认值、const、alias、注解、类型关系与模板，普通非泛型函数的实现正文不复制给下游。
- **LocalConcreteHir**：本 Cone 实际需要发射的完整实体与正文。全部类型参数已替换，所有调用、构造、字段、variant、slot 与 external target 已确定；不含未完成推断、待实例化请求或上游普通函数正文。

两者使用不同的实体 ID 家族和显式映射。导出声明、完整泛型 application 与本地 concrete 实体不能用同一 arena index 或别名混用。

每个 expression 具有非可选类型和 definition/evaluation origin。已解析调用保存完整逻辑签名、适配前 receiver 类型、实参映射、唯一 typed target 与效果；receiver 和实参的执行顺序遵守语言规范 8.5，不由类型求解顺序决定。

默认值、泛型正文及词法 callable 保存定义处绑定、完整 binder、局部值和 capture 身份。再次消费只替换实际类型与求值上下文，不重新选择名称、重载、字段或接口默认实现。局部函数和 closure 的捕获引用原绑定，不能改为消费方的同名局部变量。

默认模板分别保存提供方参数的预期类型和正文表达式的实际类型。前端按子类型规则检查默认值；`Nothing` 正文可以满足任意参数类型，产物投影、读取及实例化必须保留这两个类型，不能要求它们逐字相同或把底类型调用改写为参数类型的成功返回。模板契约仍校验预期类型与原提供方参数一致；通用格式构造器不重复进行依赖源码类型表示的子类型判断。

源码调用记录的实参保留实际表达式类型。通常实参已由前端适配为参数类型；Nothing 实参保留底类型，在 HIR 源码签名和 MIR 实现签名连接时通过实际 nominal 的 intrinsic 表示识别。两处均保留参数个数和位置检查；调用结果仍须与被调用声明的完整结果类型相同，不能因上下文需要而改写。

nominal application 保存原声明与完整实参；struct、enum、class、interface 各有独立 typed identity。alias 透明展开；字段、variant、构造与 property accessor 保存各自原身份。按值循环、继承环、泛型约束、GC-free 和 ZST 属性在所属语义边界确定。

property 具有完整类型、读写能力、getter/setter、访问域与互斥的 stored/accessor/delegated/const/extern 表示。required accessor 有 Body 或 AbstractSlot 的明确类别；普通缺失初始化不能靠空正文、late-init flag 或可空类型补齐。

class 构造输出明确区分 allocation、同 receiver 的 initializer、this/base delegation 与 common initialization；完整成功路径、异常出口与初始化顺序已确定。abstract class 仅有供派生构造使用的 initializer。release policy 为 None 或携带完整 hook target 的 SynchronousGcFree。

singleton、runtime property 与 generic delegated application 保存完整初始化单元及 cycle throw target。constant 已完成依赖环检查与求值，没有 runtime unit。interface/default/virtual 选择在 HIR 唯一确定，qualified super 保存 direct target。

extern 声明产生完整 `SourceNativeExternalContract`：symbol bytes、逻辑 library/default namespace、function/data/TLS、mutability、C/Scoop ABI、calling convention、完整源码签名、GC effect 与定义位置。普通／suspend 效果与 GC effect 独立，后续不得由 symbol 或参数类型反推。

C-FFI-safe、Scoop ABI、GC-free 与 release-safe 是不同合同。C 参数不得含 managed reference，Scoop ABI direct ref 不改为 handle 或 pin。generic 条件按原 binder 保存并在实际 application 检查。

实际依赖分为 lookup observations 与 committed uses。前者包括空查找、不适用候选、star surface 和 re-export；后者才形成 MIR/LIR 的 callable、类型、布局与 native 需求。未展开默认值和未物化模板不产生机器调用。

### 2.3 MIR

MIR 只接收 LocalConcreteHir 和实际选用的依赖 MIR metadata，不接受 ExportHir 模板或执行泛型推断。每个 expression 具有本层 exact type；GC-free、ZST、字段和 variant 身份完整保留，synthetic 类型在产生时也完整定义。

MIR 控制流显式表示普通边、异常边、循环目标和 cleanup。Return、Break、Continue 只执行真正退出的 scope 清理；Throw/Rethrow 保持异常路径；finally 的覆盖规则和挂起不退出作用域的规则遵守语言规范。variant payload 只在同一值、同一 variant 的有效分支内读取。

所有调用已确定 direct、virtual 或 interface target。super 不重新进入动态分派；方法 receiver 为完整值参数，value receiver 的可观察存储属于本次方法调用。

class allocation 只分配最派生对象一次，使用实际 exact TD；base/this initializer 不分配、不修改 TD。receiver 在可能 GC 的操作间保活并重读，只有完整构造成功后发布 release-ready 与结果。构造失败没有可用结果。

closure 具有完整捕获字段、类型、invoke 与扫描属性；绑定方法引用在调用时保留原动态分派。普通 local direct call 的捕获参数不改变原绑定身份。callback adapter 有精确 closure/native storage/status 签名，在返回 C 前处理全部异常。

suspend callable 的执行 ABI 为 `(source args..., Continuation<R>) -> CoroutineStep<R>`，后者仅含 Completed(R) 与 Suspended。输出不保留源码级 suspend call；frame、continuation、恢复入口和 saved slots 均有独立身份和完整类型。跨挂起只保存已初始化的 exact payload，不保存 native block address 或 active EH record。

completion 与 resume 具有唯一获胜方，并明确 payload 的 release/acquire 关系。恢复失败沿原调用点的异常／cleanup 路径传播。挂起状态使用 frame 的 TaskContext，恢复前后的切换见 2.16。

MIR metadata 保存源码声明到实际 callable、constructor、accessor、generated body 的映射，exact signature、GC effect、继承、dispatch、object/init 关系及 Strong/ODR 归属。它不定义目标布局；布局由 LIR metadata 提供。

输出为 Library 或携带本模块非可选 entry 的 Executable，不能由 `Option<Entry>` 与独立 kind 拼成矛盾状态。

### 2.4 LIR

LIR 保存目标上完整的类型布局、值表示、字段 offset、alignment、scan、调用 ABI、控制流与实际定义／引用。codegen 不按名称、实参、result storage 或上下文补类型、签名、poll 或逻辑 live set。

每个 pointer（包括 null）具有 Managed、Raw、Code 或 Metadata provenance；LLVM opaque pointer 不抹去这种区别。managed 使用 address space 1，其余使用 0，转换必须为明确合法的 typed operation。

Scoop ABI 保留完整逻辑参数序列及 exact identity，物理分类为：

| 值 | 参数 | 结果 |
| --- | --- | --- |
| Unit / ZST | ElidedZst，仍求值 | Unit 为 UnitVoid；其他为 ElidedZst。 |
| scalar、managed/raw/code pointer、niche enum | Direct | Direct |
| 非 ZST tuple、普通 struct、tagged enum | Indirect | Indirect |

每个 indirect 实参使用调用方新建的 exact storage，callee 按值接收。物理参数为 indirect result storage（若有），随后按逻辑顺序省略 ZST 并传递其余参数。LLVM 使用对应 exact type/alignment 的 byval/sret；实际寄存器分配由目标 ABI 决定。Darwin sret 使用 x8，amd64 使用 RDI 并在 RAX 返回同一地址。

ZST 的求值、调用、构造、类型及对象身份不能因零 payload 消失。被观察地址的独立 place 有满足 alignment 的存储 token；同时存活的不同 place 不共享地址。Ptr<ZST> 的元素位移为零，仍遵守 operand 求值和 unsafe 有效性条件。

call target 同时确定 destination、calling convention、全部参数与 return convention；void/direct/indirect-result 以及 Managed/NoGc/NativeSafe/NativeBorrowed 使用完整互斥分支。Managed call/poll 带 site ID 与逻辑 live set，invoke 带正常／异常 root set；native 带 site ID、caller roots 与按返回形式确定的 result publication。NoGc 不携带虚构 root plan。

live set 包含 post-site live references 与 managed 实参，以及 aggregate 的完整 managed leaf path。普通 call/poll 的机器 relocation 与显式 invoke/native root frames 遵守运行时规范 3.2；跨 transition 的实参和结果必须使用更新后的存储。

值类型布局使用原 exact 类型与字段身份。tagged enum 的 tag、payload slot 与 scan 一致；niche 只用于语言允许的引用／pointer Option。整数宽度与 signedness、Float/Double 精度、内部计数和 runtime ID 均保留各自类型，物理位宽相同不等于同一实体或源码类型。

wrapping 整数运算不携带 nsw/nuw；除零和 signed MIN/-1 已有安全控制流，shift count 已按位宽正规化，compareTo 的结果为 Long。浮点运算遵守 2.18。

C ABI storage 为封闭类型结构：integer、Boolean、Float/Double、带完整 pointee 的 data pointer、带完整 signature 的 function pointer、完整 C-layout struct。只有结果可为 void，只有 Ptr<Unit> 的 pointee 可为 opaque void。by-value struct 关系无环，pointer 可引用 forward-declared struct。

nullable data/code pointer 保留对应 exact enum 与 pointer 类型，不得把同一 enum 绑定到不同 pointee/signature。CLayout 的 aligned/packed 各为 Natural、A1、A2、A4、A8 或 A16。C ABI storage 与 Scoop 值表示分别准确保存；系统 C compiler 负责目标寄存器分类，不能用 Scoop aggregate ABI 代替。

compiler-owned static storage 带完整 scan 和封闭初值 ZeroedForRuntimeUnit 或 EncodedStaticValue；后者具有 canonical bytes 与排序的 immortal relocations。raw global/TLS 另存完整常量，不进入 managed static-root registration。ZST token、全零初值与 relocation 规则见运行时规范 2.8。

LIR metadata 保存完整 value/instance/element layout、Scoop/C ABI、scan、TD、dispatch、initialization、runtime registration 与 required definitions。ordinary nominal 定义使用原 provider 的布局；实际泛型或 structural application 使用其 ODR 布局。结构字段可按 canonical exact key 组成，不要求为没有实际物理定义的 constituent 另造 layout 实体。

每条 native contract 按 target 正规化 symbol，并保存完整 kind/library/signature/storage/GC effect；它进入 LIR 语义内容，host 搜索路径不进入。String 关联实际声明的 TD 与 provider，不由名字或相同 InlineBytes 布局代替。

### 2.5 Codegen 与对象合同

codegen 的语义输入只有当前 Cone 的完整 LIR；Scoop producer 消费 LIR target/backend，generated-C producer 消费 LIR target/C toolchain。输出可以有多个对象，物理分片数量与文件名不是 Cone 或实体身份。

每个实际物理定义都有 typed owner、role、linkage、symbol 和精确 section range。引用使用 typed target 与真实 relocation；跨对象 external 声明不能变成可缺失的 weak reference。Strong 属于原定义 Cone，ODR 使用其稳定 member identity。

对象边界验证目标格式、section/symbol extent、alignment、权限、relocation 写入宽度/addend、定义与引用闭合。Mach-O 使用其真实 relocation 与 weak 属性；ELF 使用 ET_REL、实际 section/symbol/RELA/COMDAT 数据，TLS 必须保持 STT_TLS/SHF_TLS，不能伪装成普通 data。

relocation 的 section、atom、offset 和 target 均属于本次实际对象。raw source-native symbol 按 Darwin 添加一个前导下划线、ELF 保持 bytes 的规则正规化。Scoop-owned symbol 从 persistent identity 产生，不用 FQN 或 source 短名回退。

所有 Scoop body、NoGc body、managed adapter、root/init gateway 与 release hook 都有 callable registration。生成 C bridge/trampoline、runtime C 和 ordinary native producer 使用各自实际对象合同。实际保留的 safepoint 与其 owner、role、machine root count、stackmap 完整对应；无 safepoint 的 body 也可合法登记。

body 的 EH、LSDA、unwind、stackmap、关联常量与私有 Context cells 属于该 body 的同一次物理选择。TD、scan、dispatch、static storage、初始化和六类 registration 满足运行时规范；release hook 为 raw C、nounwind、NoGC，无 managed transition 或 EH/safepoint。

generated-C bridge 保留实际声明的 C 类型、签名、目标工具链与编译选项。调用不得按 extern 名称被 builtin 替换；bridge 具有精确 primary/signature/context 定义范围，不能由邻近符号猜测。普通 C ABI 与 Scoop NativeBorrowed 调用不混用 storage bridge。

对象内容 fingerprints 使用实际定义 bytes 和 canonical relocation target；物理地址、archive member 分组或 section ordinal 不得冒充语义身份。每个需要回填的 digest slot 为无 relocation 的 32 bytes，初始为零且有唯一写入者；同一 digest 的镜像必须逐 byte 相等。

摘要依赖必须无环，节点仅依赖实际内容所需的上游摘要。规范化对象时，自身与不属于传递依赖的 digest slots 归零，上游 slots 使用已确定值；不能用执行顺序决定哪些 bytes 入 hash。Code、RuntimeImage 与 Artifact 分别覆盖自己的内容，不能建立自引用。

对象中的实际 stackmap 与 LSDA 遵守运行时规范 2.8、5.2。完整 immutable 对象的检查结果由后续打包和链接复用；最终链接只增加本次物理选择、地址与装载事实的验证。

### 2.6 `.slib` 格式与指纹

`.slib` 是 target-specific、bitwise reproducible 的 normal、self-contained ar archive。容器 schema、manifest schema、persistent identity schema 与 mangling schema 的格式版本为 1；HIR/MIR/LIR metadata outer schema 为 2。它不允许 thin archive、外部成员引用或相邻文件 payload。

归档 magic 为 `!<arch>\n`，首项固定为 `manifest.cbor`。其余 payload 按 manifest 中的 SlibMemberId bytes 严格排序，物理短名为从 0 开始的 ordinal：`m` 加 8 位零填充十进制。header 使用 SysV/GNU short-name encoding：名称后为 `/` 与空格填充，mtime/uid/gid 为十进制 0，mode 为八进制 `100644`，size 为无多余前导零的十进制，trailer 为 `` `\n``，奇数 payload 后补 `0x0A`。

reader 拒绝其他 header spelling、重复或未声明 payload、path-like 成员及 `/`、`//`、`__.SYMDEF*` 等特殊成员。payload 不因自身 magic 而递归作为归档解释。

每个非 manifest 成员为五字段 record：`1=id, 2=stable_key, 3=role, 4=byte_length, 5=sha256`。SlibMemberId 来自 ConeIdentity 与稳定非路径 member key，物理名、扩展名和生成顺序不参与身份。

| member role / stable-key tag | 值 | 用途 |
| --- | --- | --- |
| HirMetadata | 1 | Compile |
| MirMetadata | 2 | Compile |
| LirMetadata | 3 | Compile、Link |
| LinkObject | 4 | Link |
| DiagnosticAttachment | 5 | Diagnostics |
| ExtensionBlob | 6 | 显式声明的 purpose |

三种 metadata 各恰有一个成员；完整生产产物具有非空 LinkObject 集合。role 与 stable-key tag 必须匹配，成员由目录身份识别，不由物理名、ordinal 或数量推断 producer。

所有 wire metadata 使用 RFC 8949 deterministic CBOR 的明确 schema：最短整数与长度编码、定长容器、严格有序且无重复的整数 field tags、声明的封闭 variant、完整 typed 引用。不能序列化 Rust discriminant、arena index 或 hash-map 遍历顺序；未知必需字段、variant、重复 key、indefinite container 与非 canonical 输入均拒绝。

```text
ByteSpan(bytes) = little_endian_u64(length(bytes)) || bytes
H(domain, value) = SHA-256(ByteSpan(ASCII(domain)) || WireCborV1(value))
```

domain 无结尾 NUL；可变长度 raw 片段使用 ByteSpan，只有公式明确声明的定长 ID/digest 可以 raw 拼接。运行时 metadata 的 RuntimeEncode 是独立编码，不能用 WireCborV1 替代；callable body identity 使用运行时规范 2.8 的 v2 domain。

`ConeIdentity = H("scoop-cone-id-v1", ConeCoordinate)`。ConeCoordinate 使用语言规范 12.2 的 canonical group、name 与 version；manifest 同时保存 coordinate record 和 identity，二者必须一致。长度转换 checked 到 u64；WireCborV1 item 不隐式再套 ByteSpan，只有明确写出的外层 framing 才参与指纹。

CapabilityId 为 map `{1=namespace, 2=name, 3=nonzero u32 major}`；purpose bits 为 Graph=1、Compile=2、Link=4、Diagnostics=8。capability 表达格式和消费能力，不授予来源资格。LinkObject 明确指定 target、object format 和 verifier capability。内建对象格式为 `org.scoop-lang.object-format/mach-o-relocatable/1` 与 `org.scoop-lang.object-format/elf-relocatable/1`；Scoop verifier 为 `org.scoop-lang.link-object/scoop-lir/5`，generated C 为 `org.scoop-lang.link-object/generated-c-bridge/1`。

Scoop LinkObject logical key 为 `{1=nonzero unit_count, 2=unit_set_digest}`；digest 为 `H("scoop-lir-object-unit-set-v1", sorted unique nonempty ObjectDefinitionPlanId array)`。generated-C 使用同一 map，domain 为 `scoop-generated-bridge-object-unit-set-v1`，元素为 GeneratedBridgeUnitId。物理分组只改变 Code/Artifact 层。

GeneratedBridgeUnitId 的 domain 为 `scoop-generated-bridge-unit-v1`。key 为 OutboundFunction=1、GlobalRead=2、GlobalWrite=3、GlobalAddress=4（各含 native contract fingerprint）、CallbackTrampoline=5（C signature fingerprint 与 context parameter index）、StaticCallbackTrampoline=6（storage-bridge generated callable ID 与 C signature fingerprint）。

bridge unit 是与 producer 无关的 recipe identity；实际 atom 使用 producer ConeIdentity 与 PrimaryEntry、SignatureDescriptor、ContextDescriptor 或 StaticAssertSupport role，domain 为 `scoop-generated-bridge-atom-v1`。前三者可具有实际 bytes/range；StaticAssertSupport 不产生物理定义。unit、实际 atom 和 member 的关系必须完整，不能把 producer-specific symbol 写入 unit identity。

未知 optional capability 在长度与 hash 验证后可跳过，不进入语义 view。ExtensionBlob.required_for 只允许 0 或 Link bit；未知 Link-required capability 或 LinkObject verifier 阻止 Link 消费。blob 不成为第二种对象入口，已知 Link contribution 只能表示规范化 native library requirements，不能注入 raw linker argv、脚本或未检查对象。

完整生产 profile 为 `org.scoop-lang.slib-profile/cross-cone-generic/5`。descriptor 是五字段 map：`1=id, 2=required_manifest, 3=required_hir, 4=required_mir, 5=required_lir`；各列表按 capability 排序。其必需 section 为：

| 位置 / namespace | section 与 major |
| --- | --- |
| Manifest / `org.scoop-lang.manifest` | `single-cone-production/5` |
| HIR / `org.scoop-lang.hir` | `identity-foundation/8`、`core-bootstrap-interface/12`、`cross-cone-interface/61`、`cross-cone-type-semantics/23` |
| MIR / `org.scoop-lang.mir` | `identity-foundation/5`、`core-bootstrap-bridge/1`、`cross-cone-param-free-bridge/2`、`cross-cone-type-bridge/16` |
| LIR / `org.scoop-lang.lir` | `identity-foundation/6`、`cross-cone-param-free-bridge/1`、`cross-cone-link-closure/1`、`cross-cone-layout-abi/11`、`cross-cone-layout-link-closure/5`、`cone-production/11`、`link-identity-closure/15`、`link-support/1` |

各 section 按消费用途检查 required inventory。Compile 需要完整语言与相邻 IR 合同；Link 只消费 identity、ABI、对象、native、production 和链接支持数据，不为链接展开 HIR 模板。profile fingerprint 覆盖 descriptor 的实际内容。

manifest 保存 canonical coordinate、ConeIdentity、kind/source form、完整 exact direct dependencies 与三层语义指纹、language/runtime/identity/mangling ABI、target/backend 配置及其兼容摘要、全部成员目录、三层/Code/RuntimeImage/Artifact fingerprints 和 required sections。

production 与 Link metadata 完整保存实际 body/type/site ID、Strong/ODR 定义、shared ABI、object/bridge unit 到 member/range 的映射、digest patch 位置、runtime registrations、image/root entry 归属、defined symbol owners、undefined requirements、native contracts 与逻辑 library requirements。所有表具有 canonical key/payload、明确排序与唯一性；不保存 producer 的绝对搜索路径。

`single-cone-production/5` 的 field 4 为完整 typed registration projection，field 6 为 RuntimeImage fingerprint，field 11 为物理 ODR member 目录，field 12 为 OptimizationMode；field 5 保留不用。ODR directory 的 member 保存实际 role 与 OdrAbiFingerprint，field 4 保留不用；纯语义 member 不产生空物理条目。

`link-support/1` 为单字段 map `{1=runtime_data_aliases}`，把必要的 runtime 数据符号关联到实际定义；String alias 不创建额外 TD 或存储。

persistent identity 只由首次引入的层声明一次，并同时保存 typed ID 与 canonical key；其他层引用它。相同 kind/ID 的冲突 payload 或跨层重复声明拒绝。LIR foundation 的 exact-type 集合引用已有 HIR/MIR exact records，不复制声明。源码文件、binder、字段、variant、body application 和调用位置的引用必须指向真实定义。

共有 HIR expression 为 `{1=kind, 2=result_type, 3=definition_origin, 4=evaluation_origin}`。Call 的 tag 为 57，payload 保存 callee、arguments、SourceCallReceiver；receiver 为 NoReceiver=1 或 Receiver=2（完整静态类型）。实际 call-site 保留完整逻辑实参与结果、定义／求值位置及原 receiver，Unit/ZST 参数不能按物理省略规则消失。

nominal declaration 保留原 binder 条件、GC-free pointee requirements 与 NoGC 标记。property/accessor、constructor safety/effect、InteriorMutable、CLayout、泛型 companion、release、annotation 和编码合成所需的声明事实均不可由缺字段的默认值替代。读取只检查格式、kind、引用与合同连接，不重新进行定义处的重载或可见性选择。

native-boundary record 保存 owner、type-parameter count、source shape 与 C ABI projection。projection 为 SourceRepresentation=1、UInt64Field=2（唯一 field ID）或 NullablePointer=3（empty variant 与 pointer payload field ID）；必须匹配真实声明的字段、variant 与类型，不改变普通 Scoop aggregate/niche 表示。

三层语义指纹是 Merkle 指纹：各自 canonical own-layer projection 加真实 direct/support/re-export 依赖的同层指纹。其 domains 为 `scoop-hir-semantic-v1`、`scoop-mir-semantic-v1`、`scoop-lir-semantic-v2`。LIR context 包含 identity ABI、target profile 与 target fingerprint，不含 backend optimization、member assignment、range 或 patch offset；backend 兼容性仍单独检查。

HIR 语义包括名称候选、签名、默认值、const、模板和完整依赖；无法完整表达细粒度 lookup observations 时，保守计入全部相关直接依赖。普通私有非泛型正文、机器码、优化模式与对象分组进入 Code/构建缓存，不能反向改变声明或 shared ABI 身份。

`MemberFingerprint = H("scoop-slib-member-content-v1", member_record)`；`LinkMemberFingerprint = H("scoop-slib-link-member-v1", member_record)`，后者只用于 LinkObject 或 Link-required blob。

`CodeFingerprint` 使用 `scoop-code-v1`，覆盖按 member ID 排序的 Link fingerprints、已知 Link contributions、generated-C production、native library requirements、defined/undefined symbol 集合、去除诊断位置的 native contracts，以及 section registry 声明的 Code projections。只改变实际合同或符号需求也必须使 Code 改变；diagnostic/unknown optional payload 不进入 Code。

`ArtifactFingerprint = SHA-256(ByteSpan("scoop-artifact-v1") || ByteSpan(canonical_manifest_without_artifact_fingerprint) || ByteSpan(MemberFingerprint_0) || ... )`，成员顺序为目录顺序。它覆盖全部附件与 blob；manifest 不包含自身 member hash。RuntimeImage 编码及 body/stackmap domains 见运行时规范 2.8。

产物消费失败不返回部分闭包。Graph、Compile、Link 的结果按实际已完成的检查区分，不能用只读过目录的对象替代完整 Link 输入。同一次成功编译的归档发布直接使用已完成的 IR/对象与摘要，不重新解析自身语言语义；外部 bytes 在相应消费边界验证后供后续复用。

### 2.7 `scoop` 与 single-Cone `scoopc`

`scoop` 负责 root input、locator、完整依赖图、构建顺序、全局缓存、runtime 构建和程序链接；`scoopc` 一次只编译指定的一个 Cone，不跟随上游 manifest locator 或读取上游源码。

```text
scoop build [root-input]
scoop run [root-input] [-- program-arg...]
scoop link --root-slib root.slib --runtime-objects index [--dependency-slib dep.slib]... [-o output]
scoopc build <Cone-root-or-Cone.toml-or-file.scoop> [--direct-slib direct.slib]... [--support-slib support.slib]... --out-slib current.slib
```

低层 `scoop-link` 使用相同的 artifact-only 链接合同。build/run 省略 root 时使用调用者 cwd 的 Cone.toml，不向父目录搜索；`run -- args` 也使用该默认 root。低层 scoopc 的输入必需。

输入为两种封闭形式：

- ManifestCone：目录下必须有 Cone.toml，或显式 regular file 恰名为 Cone.toml。源码为 src 下扩展名精确为 `.scoop` 的 regular files，按标准化相对 UTF-8 路径排序；空集合、重复标准化路径、非 UTF-8、symlink 逃逸或循环均在 parse 前失败。
- SingleFile：operand 扩展名精确为 `.scoop`，解析 symlink 后为存在的 regular file。只编译该文件，logical path 固定 main.scoop；不读取相邻 manifest、Scoop/C/C++ source 或 blob。

SingleFile 的 coordinate 为保留值 `scoop:single-file:0.0.0`，kind 为 Executable，只依赖 core；用户 manifest 不能声明该 coordinate。build/run 此时拒绝 cone-path，scoopc 拒绝 direct/support slib 参数。其 `.slib` 可作为本次构建、缓存或显式 link 的 executable root，不能作为依赖或 manifest locator 指向的可分发 Cone。

依赖使用 exact version，全部为 library，同一 group:name 只允许一个版本。启动 compiler 前必须验证完整图的 identity/content 唯一性、target/ABI/schema、语义 fingerprint、无环和 locator 无歧义。canonical 顺序逐次从“全部依赖已完成”的 ready set 中取 coordinate bytes 最小者；不按输入顺序或并发完成顺序决定。

非 core Cone 未显式声明 core 时，注入 `scoop:scoop.core:0.1.0` direct edge；core 不依赖自身。core 是普通库，显式 locator 遵守同一发现规则。缺省 sysroot 来源先取 `lib/scoop.core`；只有目录不存在时才取 `artifacts/<canonical-target-id>/scoop.core.slib`。已选来源损坏或不兼容时直接报错，不回退到另一来源。

配套 scoopc 默认来自 scoop executable 同目录，允许显式指定；必须匹配 machine capability 和实际编译输入。每个 source cache miss 启动一次 single-Cone 编译，传入完整 direct/support artifact。direct 与 manifest edge 一一对应，support 恰为其余传递闭包，两组互斥。缺失、不可达额外输入、identity/content 冲突、stale edge、错分或 executable dependency 均在 parse 当前源码前失败。

library 不要求 main；executable 必须有本 Cone 顶层 ordinary、非泛型、非 suspend `main(): Unit`，依赖的同名函数不参与入口选择。scoopc 的正式输出始终是当前 `.slib`，不构建 runtime、不解析 native library 搜索目录、不调用最终 linker。

build/run 接受 `--profile debug|release`，默认 debug；`--release` 为 release 简写，与显式 profile 同时出现为参数错误。实际优化合同见 2.19。

默认 target root 为 manifest Cone root 下的 target，SingleFile 为调用者 cwd 下的 target。`--target-dir` 相对调用者 cwd 解析并替换该根；输出目录追加 `<canonical-target-triple>/<profile>`。library 文件名为 `<cone-name>.slib`，manifest executable 为 `<cone-name>`，single-file 为 `<sanitized-input-stem>`。build 的 `-o` 覆盖最终 root 文件的完整路径，不追加 triple/profile。

发布为原子替换，失败保留旧输出，不能留下部分 artifact 或 binary。用户输出与缓存内容相互独立；输出不能覆盖本次输入或破坏缓存。并发发布到同一位置时，最后一次成功发布生效。

run 执行本次成功构建的 bytes，不能执行旧 binary 或被另一构建替换的程序。argv[0] 使用稳定输出路径，其余参数原样传递；继承调用者 cwd、environment 和 stdio，保留程序的真实 exit/signal。工具输出在程序启动前完成，程序 stderr 不包装为诊断 JSON。

`scoop link` 接受显式 root/dependency artifacts、cone-path、必需 runtime object index、target、library search roots 与输出路径。它按 artifact exact dependencies 查找，core 也只从已有 artifact 取得；不启动 scoopc、不读取 runtime/Scoop 源码或补建缺失依赖。

`--sysroot` 定位 Scoop core；Linux 的 `--cc`、`--native-sysroot` 选择 C 工具链。`--unwind-prefix` 和 `--link-mode static|dynamic` 属于 runtime/executable 链接输入，library `.slib` 构建不要求 unwind archive。Linux 缺省 unwind prefix 为 `<Scoop sysroot>/native/<canonical triple>/unwind`。Darwin 使用所选 Xcode 开发环境。

全局构建缓存由 scoop 管理。compile key 覆盖 manifest/single-file semantic projection、全部源码内容、compiler/toolchain/protocol、language/schema/runtime ABI、target/backend、实际使用的 C bridge 配置、OptimizationMode 与依赖三层语义指纹。locator、输出路径、dump 请求和程序 argv 不改变语义 identity。源码变化可重建；无源码的 stale prebuilt 必须报错。

machine transport 为一次 request/response 的 length-prefixed canonical CBOR，protocol version 为 4，stdout 只含协议 frame。build request field 9 为 OptimizationMode（Debug=1、Release=2）；target request 保存 canonical triple、可选 C driver 与 native sysroot，host locator 不进入 artifact identity。旧或不匹配协议拒绝。

显式观察使用 `--emit ast|hir|mir|lir|all --dump-dir dir [--dump-scope root|sources]`，默认 root；sources 覆盖源码节点，不从 prebuilt 反造 AST 或 LocalConcreteHir。每个被观察节点在同次完整编译中生成 dump；HIR 同时包含 Export、LocalConcrete 与共有跨 Cone 接口。观察不改变 artifact，失败不返回成功产物；命中缓存也须为请求的源码观察产生本次 dump。

诊断具有明确 Error/Warning severity，primary 与 notes 保存实际 Cone、logical source 和 byte span。产物错误保存 container/manifest/member/section 与 wire field path，I/O 错误不伪装成源码位置。human 与逐行 JSON 使用同一结构化记录，写 stderr；stdout 留给被执行程序。缓存中的 warning 在本次显示时附加 locator，不丢失原位置。

工具中断停止后续构建，向当前 child 传递信号并回收进程，保留真实 signal。某节点失败后不启动 dependent 或 program-link。

### 2.8 Artifact-only program-link

program-link 消费完整 `.slib` Link 闭包、普通 runtime 对象、已有 native 文件、LIR target 与 final-link 配置。它不展开模板、重新编译 Scoop 或编译用户 C/C++；只可用明确选择的 C compiler 生成本次固定启动代码与最终 image tables。

root 必须是唯一 executable，依赖全为 library。闭包核对完整 identity、exact edges、三层 semantic fingerprints、单版本与兼容合同。多个 locator 指向同一完整 artifact 可合并，同 identity 不同内容或不可达额外 artifact 是错误。

每个 LinkObject 按 canonical Cone/member 顺序消费一次，保留原 bytes 与检查结果。diagnostic、opaque、unknown optional 成员不是链接对象；无法处理的 Link-required capability 失败。后续不重新打开 locator 取得另一份输入。

Strong 由原 provider 提供。ODR 在普通依赖语义一致后按完整 group/member key 与 shared ABI 合并，物理选择见 2.13、2.19。全部 typed imports 和实际 relocation 必须闭合；metadata-only 查询可以没有机器引用。

SourceExtern 按实际 target/native symbol 合并完整 library、kind、TLS、mutability、ABI、calling convention、GC effect 与 signature/storage。任一字段冲突均报错并指出来源；随后从实际 native 对象或 provider export 解析定义。core 与普通 Cone 使用同一规则。

普通 native 对象不含完整函数类型时，只核对实际符号、kind、范围、可写性和 relocation 等可观察事实，FFI 实现履行声明的责任不被伪造的类型证明替代。显式 library 的 symbol 必须由该库按平台规则提供，默认 namespace 的多 provider 歧义不能按搜索目录先后静默解决。

参数自由 source extern 的普通 managed 调用经原 provider 的 Scoop 薄入口，包含 native transition 与 caller-root publication，入口的 Scoop GC effect 为 Managed；native callee 自身的 effect 仍是其声明合同。泛型正文在定义方与消费方使用同一入口；NoGc 和 release 的直接 native 调用遵守各自规则。

runtime object index 是普通对象及定义／引用摘要，必须匹配 target、runtime ABI 和实际内容。runtime-build 的源码、头文件、compiler、SDK、flags 与规则变化使其缓存失效；该索引不要求 program-link 读取这些源码。

逻辑 native library requirement 沿完整依赖闭包传递。只从明确 library roots 与平台 provider 解析，不自动发现相邻 native source/object/archive。TargetDefault 与显式 kind/grouping 保持原合同；OrderedGroup 只限制候选顺序，不引入 whole-archive、脚本或任意 linker 参数。

普通 archive 保留真实 member index、offset、length 与 digest，允许 BSD/GNU 命名；拒绝 thin、外部或嵌套 archive 和损坏边界。必要格式与定义索引覆盖候选，完整 relocation/EH/TLS/初始化检查只作用于实际选入的 member。实际 undefined references 驱动成员选择，新增引用继续闭合，每个物理 member 最多加入一次。

选入的 native 对象不能包含未声明的 constructor/destructor、动态 TLS initializer、bitcode/LTO、embedded linker actions、可执行栈或禁止的 C++ EH 依赖。允许普通 C unwind 与静态／零初始化 TLS。普通 native 重复定义不使用 Scoop ODR 判等。

非空 `@Extern(lib=L)` 在 Darwin 的每个显式 library root R 下，对 TargetDefault 检查 `R/L.o`、`R/libL.a`、`R/libL.dylib`、`R/libL.tbd` 与 `R/L.framework/L`；不去掉已有 lib 前缀或扩展名，不递归扫描。显式 kind 只收窄对应格式；相同内容及加载合同的重复候选可合并，不同候选为歧义。空 lib 只查询已引入的 runtime/native 对象和动态 exports，不扫描目录搜索任意库。

Darwin 支持 `.o`、`.a`、dylib/framework 与所选 SDK 的系统 providers。普通 load/re-export、真实 install name、target/deployment、symbol kind 和绑定关系参与解析；显式库不能被另一库的同名 export 替代。`@rpath` 从明确目录解析，实际 LC_RPATH 进入输出和 link plan；依赖中的 `@loader_path` 相对 provider。需要部署布局才能解释的直接 `@loader_path`、`@executable_path` 或未知形式拒绝。

Darwin 使用所选系统 linker、C compiler、SDK/deployment 与 libSystem，Level I unwind 经系统正常导出解析。final-link 不使用 dead_strip、不同 body 的 function ICF、LTO、未声明 autolink 或 raw 用户 linker options。stackmap relocation 后位于只读 `__DATA_CONST`，采用传统 rebase/bind（no_fixup_chains）格式；实际需要的 ad-hoc signing 属于平台装载格式。

Linux 接受 `.o`、`.a`，动态模式另接受 ELF `.so`；framework 与静态程序的动态库需求为错误。用户 `.so` 必须是 DSO，不是 PIE executable 或 linker script。未版本化的源码 extern 只匹配默认导出版本，DT_NEEDED 不等于显式库 re-export；TLS、IFUNC、版本化导入按实际 kind/version 检查。

Linux gnu 默认 dynamic PIE，musl 默认 static ET_EXEC；musl 可显式 dynamic PIE，不支持 glibc static。最终输入明确选择 CRT、LLVM unwind、libc、libm/线程及算术 builtins，不引入第二套 EH provider。动态 metadata/stackmap 满足实际 RELRO，静态 musl 的不可变记录位于只读 PT_LOAD；最终文件不得有 text relocation。

DSO 依赖查找使用显式 roots、provider 目录、实际 RPATH/RUNPATH 与所选系统目录。真实 SONAME、DT_NEEDED、加载顺序和 RUNPATH 进入 link plan，临时快照路径不写入 executable 的依赖名；同名导出使实际 ELF 顺序无法满足显式库绑定时报告冲突。

链接生成 C main、最终 image descriptors、六张 producer pointer tables、image pointer array 与唯一 root entry 引用，调用 `scoop_rt_run_program`。它们只引用所选记录，不复制 TD、storage、cell 或 gateway body。`scoop_td_String` 是实际 String TD 的同地址 alias。

ResolvedLinkPlan 使用 `scoop-resolved-link-plan-v3`，覆盖 root/依赖、Code 与对象内容、runtime/native/startup 输入、ODR 选择、最终 image 内容、系统 providers、库绑定、archive members、加载路径、工具配置和有序操作。物化与输出 locator 不进入内容身份，实际 install name/runpath 必须进入。

最终检查核对本次链接产生的 target/entry、实际输入、symbol/binding、Strong/ODR 地址、String alias、startup/image 引用、stackmap/EH 保留与装载权限。native linker 不能重新选择另一份 ODR 实现；落选 bytes 不能用于解释最终文件。已在对象边界确认的语言、ABI 和不变内容不重复完整重放，也不运行用户程序证明链接成功。

全部检查通过后原子发布 executable，失败保留旧文件。

### 2.9 虚方法与接口分派

每个 slot 有原根声明、完整签名、访问域和 Abstract/Concrete/InterfaceDefault 的明确选择。class vtable 保留 base prefix，override 不改变 family 位置；Any 不预留 equals/hash/toString 槽。

interface 槽序按直接父接口声明序继承，再追加当前声明；相同 exact application 的菱形路径去重，移除被原 typed override 覆盖的成员，保留其余相对顺序。getter/setter 独立，private helper 不占槽。实际类／值类型表项匹配原 slot 的参数、结果、execution 与 GC effect。

itable 以实际 interface TD 为键，不要求跨 Cone 的全局槽编号。generic receiver 始终是完整 exact application，只沿声明中的 exact base/interface 关系分派。

引用 receiver 适配为 Identity 或 ReferenceDispatch，并满足实际继承／接口关系；物理 pointer 形状相同不足以接受不兼容签名。装箱值的方法 receiver 按值取得 payload 副本，可观察的 this 存储不能指向 box；interface default 可以使用同一 box 的合法 interface 视图。

qualified super 保存实际 direct target。dispatch 数据不重新进行 overload、default 或 override 选择，也不因 provider 为 core 使用另一套规则。

### 2.10 Intrinsic 合同

intrinsic 的识别来自实际声明及其完整 name/target/shape/signature，普通候选和效果规则先确定其合法使用。成功 HIR 保存封闭 typed kind，后续不得按 symbol、FQN 或 annotation 文本重新识别。

| 合同 | 必须确定的内容 |
| --- | --- |
| intrinsic type | 原 nominal identity、完整 application 与表示 kind；LIR 给出目标布局。 |
| current_source_location | 当前表达式的 evaluation origin，结果为普通 SourceLocation 值。 |
| Ptr/FunPtr/addressOf | 实际 pointee/signature/place、unsafe 与非零条件，不取普通按值实参的临时副本。 |
| sizeOf/alignOf | 完整被查询类型，结果由目标布局产生。 |
| integer/float operation | 完整 operand/result kind 与语言操作，不保留待决议源码调用。 |
| managed callback | native/managed 签名、context index、mode、原 registration 与实际 application。 |

每个 intrinsic 只承担其指定语义，展开后是普通 typed 运算、内存访问或有明确 runtime 合同的操作。Const 能力只来自声明允许的操作集合，不能由同名用户函数取得。

共有 CallableSourceEffects 的 implementation 为 Scoop=1、Intrinsic=2、SourceExternScoop=3 或 SourceExternC=4；仅 Intrinsic 带完整 kind payload。ordinary/suspend、GC effect、release callability 与 implementation 的组合必须一致，不能用缺失字段或另一个表补出。

### 2.11 输出完备性与 typed identity

所有下游转换必需的数据都必须在上游成功输出中完整且类型化。合法的缺省、空集合或无实例分支使用明确 sum；缺失的类型、签名、root、结果存储、构造目标、字段或 layout 不能由 Option、默认数值或名称回退掩盖。

type、function type、generic declaration、完整 application、concrete function、runtime/extern target、dispatch slot、layout、TD、property/accessor/storage、initialization、callback 与 safepoint 使用不同 ID 类型。stage-local ID 不持久化，跨 stage 关系显式映射；同形结构与相同整数值不建立身份。

persistent keys 保留原 ConeIdentity、typed owner chain、kind 和该声明种类所需的规范化签名；不含正文、arena ordinal、host 路径或 link symbol。透明 alias 不建立新的 exact type，完整 generic arguments 和 structural type constituents 参与 exact identity。

CallableApplicationKey 区分原声明、NoOwner/ExactNominalOwner/EnclosingCallableApplication/EnclosingInitializationApplication，以及无 callable arguments 或完整非空 arguments。lexical body、local value、capture、initializer 与默认值的原定义路径在替换后保持；物理 producer 不替代定义 owner。

persistent mangling schema 为 `persistent-v1`，语义 symbol 为 `scoop$1$<kind>$<lowercase-hex-owner-id>`，再按目标加平台前缀。kind tags 为：

| 实体 | tag | 实体 | tag |
| --- | --- | --- | --- |
| CallableBody | cb | StaticStorage | ss |
| ImmortalObject | io | TypeDescriptor | td |
| Layout | ly | ScanProgram | sp |
| DispatchTable | dt | DispatchSlot | ds |
| InitializationCell | ic | RootRegistration | rr |
| ImmortalRegistration | ir | InitializationRegistration | nr |
| TypeRegistration | tr | SafepointRegistration | sr |
| CallableRegistration | cr | ImageDescriptor | im |
| GeneratedBridge | br | OdrMember | od |
| DefinitionBoundaryStart | bs | DefinitionBoundaryEnd | be |
| RootEntryDescriptor | re | | |

symbol 只表示已存在的 typed entity，不能反向补声明或决定 ABI。完整 identity 的短 runtime type/site ID 必须验证全程序无碰撞。

诊断来源使用实际 Cone、logical source、definition/evaluation context 与 byte span。synthetic entity 使用其真实 generated role，不伪造源码位置；显示名称和 mangled symbol 不代替实体身份。

边界责任分别为源码语义、当前转换生成的 IR、外部字节格式/引用、实际链接绑定及加载后的 GC/地址契约。同一不可变事实在明确边界验证后复用，不以重复完整重放作为发布条件。

### 2.12 Core 与普通依赖

core 是普通 library Cone，可修改和重建，使用相同声明、可见性、泛型、产物、ABI 与链接规则。sysroot 只提供默认 locator，不改变实体归属或来源资格。

只有编译器主动构造或调用的语言角色需要独立的 typed protocol 引用，例如 Unit/Any/Nothing、整数/Char/Float/Double 表示、Option/Iterator、编译器异常、初始化循环、协程和 callback。引用必须解析到实际声明及完整签名，缺失、重复或形状错误在相应边界报告。Any/Nothing 必须具有语言规范 11.1 的源码声明和普通 public binding；内部顶／底类型表示只引用这些声明，不能用固定名字补建缺失的名义声明。

Option protocol 保存同一个实际 owner、Some payload 与 None variant；Integer protocol 完整覆盖八种整数 kind。compiler exception protocol 包含实际 Throwable、UnwrapException、ClassCastException、ArithmeticException、IndexOutOfBoundsException、IllegalStateException 与 IllegalArgumentException 及所需调用目标。

ToString、Hash、普通 operator、print/println、容器和 codec 通过普通声明与接口分派使用。新增普通 core 成员不要求新增编译器角色表；intrinsic 表示不自动赋予未声明的 conformance。

core prelude 从普通 public bindings 提供最低优先级名称，direct/support 依赖与 re-export 保留原身份。普通调用、默认值、模板与 native 引用使用同一 Export/MIR/LIR 合同；String 的 layout、TD、registration 与 alias 也来自实际声明。

Any/Nothing 的 intrinsic kind、原声明、完整 application 与静态描述经过同一 HIR/MIR/LIR metadata 通道。别名和再次发布保留原身份；实际 core 源码声明必须可通过普通依赖查询取得。Any 可以保留内部的顶类型分类，Nothing 可以使用带底类型表示的普通 class application，但分类不替代源码声明。

HIR 将 Nothing 参与的子类型、结果合并和调用完成性质在源码边界确定。向其他类型适配只保留原求值，不生成目标值。MIR 将无正常返回的求值结束为明确的不可达正常边，保留原有异常出口；后续调用实参、字段写入和正常 return 不能越过该终止点。LIR/codegen 复用已确定的控制流和调用签名，不根据函数名称猜测 noreturn，也不把它当成 Unit/ZST。

### 2.13 跨 Cone 模板与 ODR

Export HIR 保存模板的已解析正文、原 binder、默认值、局部 callable/capture、字段/variant 和完整定义位置。实例化只按 actual application 替换类型，不重新执行 provider 的名称、重载、接口或 codec 选择。

普通参数自由定义由原 Cone 提供，即使签名含已闭合 generic application 也仍是该 Strong 定义。泛型和 structural application 的机器实体由实际需要它的 Cone 物化，并保持程序内共享身份。

SpecializationKey 是四分支 CBOR sum：Nominal=1 `{origin generic type, nonempty exact arguments}`、Callable=2 `{application}`、DelegatedProperty=3 `{origin extension property, nonempty receiver arguments}`、StructuralType=4 `{exact type}`。`OdrGroupId = H("scoop-odr-v1", key)`。

OdrMemberKey 为 `{1=group, 2=role, 3=typed discriminator}`，`OdrMemberId = H("scoop-odr-member-v1", key)`。role tags 为 CallableBody=1、GeneratedNominal=2、Layout=3、ScanProgram=4、TypeDescriptor=5、DispatchTable=6、DispatchAdapter=7、StaticStorage=8、ImmortalObject=9、InitializationCell=10、RegistrationRecord=12、DiagnosticBytes=13、AddressTakenConstant=14、ObjectSupport=15、ReleaseHook=16；11 保留不用。

每个物理 member 保存与角色匹配的 shared ABI，OdrAbiFingerprint 使用 `scoop-odr-member-abi-v1`。callable ABI payload 为 `{1=GC effect, 2=ScoopAbiSignature}`；包含实际调用约定、参数/结果、物理类型、布局与 scan，不包含正文或 CFG。registration ABI 包含 kind/version/size，site/root 数量属于所选实现。

普通依赖定义一致后，同一 group/member 的完整 key 与 ABI 必须一致；不比较不同 producer 的 LIR、机器码、EH 或 stackmap 内容。不同优化实例可以具有不同内部实现，但同一 body 与其关联数据必须一起选择。

同组独立 helper 可以取合法并集，不要求整个 group 来自同一 producer。TD、static storage、cell、published/failure root 与初始化状态具有唯一实际地址，不能因多个 consumer 各自物化而重复。

泛型 companion 按原 companion 声明和完整宿主 application 区分，包括 phantom arguments；generic delegated extension 按原 property 与完整 receiver arguments 区分。每次实际物化都完整提供所需 storage、unit、initializer/ensure 和 registration，同一 application 最终只初始化一次。

源码模板可以合法引用其支持声明；support closure 不使这些声明成为普通 public lookup。普通外来 constructor/accessor/helper 仍引用原 Strong 定义，不复制成消费者的新声明。

类型描述符、装箱、closure、协程与其他实际 helper 的依赖必须闭合。ordinary nominal 的有限跨 Cone shape support 由定义方提供，generic/structural helper 按原 application ODR 归属；未用模板、纯语义 identity 或查询本身不产生空机器定义。

### 2.14 Runtime image 与初始化

runtime metadata ABI 6、runtime ABI contract 10 的字段与启动协议由运行时规范 2.7、2.8 定义。每个 `.slib` 保存完整候选 registration 与物理归属；最终 image 表只包含选中的 producer records。

image、root entry、main body、root gateway、initializer、ensure、startup gateway 和 release hook 使用各自身份。根、cell、storage 和失败状态完整关联，不以可空函数指针或命名约定推断 unit kind。

全部 image 和 roots 在首个 managed initializer 前登记。每次 eager/root gateway 有独立 native→managed boundary 和入口 poll，所有异常在回到 C 前处理完成。构造、static initial state、failure root 与 Context cells 遵守运行时规范，不由最终 linker 补造缺失语义。

初始化顺序遵守语言规范 12.3；普通 lazy ensure 可以因实际访问执行。最终链接保证同一 ODR unit 共享唯一状态，runtime 按实际加载地址和当前状态协调。

### 2.15 集合、Char 与字符串

List、MutableList、ArrayList、StringBuilder 及迭代器是普通 core 类型；数组和 String 的属性/成员也从真实声明选择，不能按 `.size`、`.length` 等拼写跳过访问与类型检查。

ArrayGenerate 保存完整目标 application、元素类型、Long count 与 ordinary `(Long) -> T` initializer。负长度、求值顺序、逐索引初始化和异常遵守语言规范 10.6；ZST 只省略 payload，不省略 initializer 调用。生成控制流在任意 GC/异常点保持数组可扫描。

Char 为独立 nominal identity，常量和各层表示保存合法 Unicode scalar，物理布局为 GC-free u32。它不与 UInt 合并，Option、数组、装箱与 ABI 使用实际 Char 类型；不因表示相同增加整数算术或 CharRange。

String 的源码索引和 length 按 Unicode scalar，byteLength 使用物理 UTF-8 byte count。定位后备的 Option 结果、间接 ABI 与异常边界见运行时规范第 6 章；List getter 仍为可抛出的普通 managed 调用。

f-string 按源码顺序保存 text/expression part 与原位置，绑定实际 core StringBuilder 的构造、add 与 build。表达式与对应 toString 交错执行，保持异常和挂起行为；成功 HIR 正文不留待后端解释的字符串插值或 StringBuilder 指令。

普通容器布局和内部存储策略不属于编译器协议。其模板、接口调用、快照和字符串构建使用普通类型/ABI/GC 合同。

### 2.16 Task-local Context

Context 的语言行为见语言规范 8.3.5，运行时 key、scope、task 与 callback ABI 见运行时规范第 9 章。HIR 保存 exact key、完整绑定值类型及已选 Context 操作；泛型条件在实际 application 中替换，不按运行期对象类型猜 key。

Context scope 在完整 value 求值与成功 push 后才生效。真实离开 scope 的正常结果、return、break、continue 和异常都恢复 mark；挂起不退出 scope，已消费的 mark 不继续保活旧绑定。

协程 frame 保存完整 TaskContext 与跨挂起 mark。只有取得实际恢复权的入口切换到 frame context，所有 driver 出口恢复 previous context。普通 closure 只捕获实际词法值，callback 注册另按语言规定捕获 binding snapshot。

每个 body 的 Context key uses 与 slot cells 属于该 body 的物理选择，callable registration 保存完整关联。最终登记在首个 gateway 前填充 cells，同一 exact key 在程序中映射到同一 slot。

### 2.17 静态类型描述与编码方法

静态描述使用原声明、字段、variant、logical property、constructor/default 和 annotation 事实，不构造运行期 TypeInfo 或任意 CTFE。描述查询本身不触发物化、初始化或可见性扩张。

annotation 声明使用独立 PersistentAnnotationId，应用保存实际 typed target 和按参数顺序补齐的常量。名义类型字段保持原身份，generic 字段保留 binder；tuple 保留位置，class 保存真实 field/property/constructor 关联。

HIR foundation field 35 保存 annotation declaration keys，共有源码接口 field 14 保存 annotation declarations 与有序 applications；实际 annotation 依赖使用 AnnotationDependency tag 11。NominalDeclarationDetails field 11 保存可选 primary constructor 及声明序参数到 property 的映射，普通参数为空，val/var 参数引用原 logical property。

property accessor 的 Storage、Constant、Body、AbstractSlot、StorageBody tags 分别为 1..5。StorageBody 表示普通存储语义产生的实际 accessor body，不能通过识别机器正文推断。NominalDeclarationDetails field 12 与 generated callable tag 17 保留不用。

companion 继承直接宿主的 binder 与 bound，不捕获宿主实例或 primary 参数值；ordinary static nested declaration 隔断该 binder。companion declaration、完整 application 和 concrete singleton 分别有身份，泛型状态共享规则见 2.13。

Encodable<R>.encode 与 Decodable<R>.decode 是实际 companion/codec 的普通方法，receiver 与数据参数/结果独立。手写、继承与 default 先按语言规则选定，仅为缺失的 requirement 合成方法；字段 codec、variant、constructor 和默认值在定义处完成选择。

合成结果为普通 typed 字段访问、调用、控制流、闭包与构造；encode/decode 分别保留语言规范 11.13 的求值顺序、缺失字段处理和可见性。Export HIR 保存完整正文及支持声明，下游只实例化，不重新解释 annotation 或搜索 codec。

codec 不因数据继承关系自动传递，不能按 value 的动态类型改选 companion。不同 codec 声明即使处理同一 R 也保持独立身份；数据类型本身不因此获得编码接口。tuple、Unit 和容器组合使用普通 helper，JSON 是普通库，不增加专用 MIR、runtime type table 或 C 入口。

### 2.18 Float / Double

语言语义见语言规范 11.2.2，运行边界见运行时规范 6.1。Float/Double 分别为 F32/F64，alias 不建立新 nominal identity。literal 与 const 按目标精度直接求值，成功输出保存原始同宽 bits，不经宿主浮点近似。

算术、关系、分类、totalOrder 与转换使用明确 typed operation。浮点除零不使用整数异常规则；相等不能替换为 identity 或位型相等。所有 storage、aggregate、数组、closure、frame、boxing 和 C ABI 保留完整精度及 exact type。

浮点操作不得使用 fast、nnan、ninf、nsz、reassoc 或 contract 等改变语言语义的假设。关系分别满足 IEEE ordered/unordered 规则，不能以 `!(a < b)` 替代 `a >= b`。float→integer 按目标位宽饱和，不生成越界 poison 或先转 64-bit 再截断；同格式转换保持 bits。

C storage 使用真实 float/double；fmodf/fmod 的 NoGC 签名分别为 `(F32,F32)->F32`、`(F64,F64)->F64`，作为 target support tags 4/5 进入普通 native 链接闭包。内部常量池与机器局部 label 不获得源码实体身份。

metadata 的相等、去重与 hash 比较 raw bits，包括 signed zero 与 NaN；这不赋予语言 Float/Double Hash conformance。各 wire 域为：

| wire 域 | tag / payload |
| --- | --- |
| FloatKind | unsigned 32 或 64 |
| 浮点常量 | `{1=kind, 2=同宽 raw bits}`，F32 不接受超出 32 bits 的值。 |
| HIR literal / nominal representation / canonical const | 67 / 10 / 5 |
| MIR intrinsic representation | 6 |
| LIR scalar / canonical type / canonical value / shape representation | 4 / 13 或 14 / 14 / 8 |
| canonical C storage 与 LIR C type | 6 |
| HIR intrinsic | 22 |
| HIR expression unary / binary / conversion | 68 / 69 / 70 |
| LIR instruction unary / binary / conversion | 71 / 72 / 73 |
| literal equality pattern | 4，payload 为 FloatKind。 |
| object relocation constant-pool target | 15，payload 为实际常量 ByteSpan。 |

浮点 literal 不匹配 NaN，有限 literal 集合不能覆盖完整浮点域，剩余域需要 wildcard。JSON 使用显式 codec 和目标精度直接解析，有限／非有限值遵守语言格式规则。

### 2.19 优化与物理定义选择

debug 使用 Scoop machine O0 与 generated-C O0，release 使用 O2；两者均完成正确代码生成所需的 SSA/GC 转换。runtime build 配置独立，默认 O2。优化模式不改变源码语义、persistent entity identity、shared state 或对外 ABI，兼容的不同优化产物可以共同链接。

普通优化可以消除已证明不可达的代码及 site，但保留实际求值、异常、Context、initialization、release-ready 与严格浮点语义。类型错误不能因优化删除代码而消失。

最终 GC 计划从实际保留的 callable/site 和完整逻辑 managed leaves 得到。多个逻辑 leaf 可以对应同一 SSA 值，已知 null/不可移动常量不要求 relocation pair；runtime root count 使用实际 pair 数。每个保留 site 具有唯一 owner/role，site identity 不因删除其他 site 而重编号。

GC 转换后每个 post-site managed use 必须来自正确更新的引用；invoke 的 normal/unwind 与 native transition 继续使用各自显式 root frames。不能在 root plan 定稿后增加未经描述的 safepoint、重复 site、外来 owner 或异常边。backend contract field 28 为 1，表示此最终根与发射合同。

ODR 只比较 key 与 shared ABI，按实际需要选择物理 member。body 的机器码、EH、stackmap、callable/safepoint registration、关联常量与 Context cells 必须同选；独立 helper 可分别选择。shared TD、storage、cell 与初始化保持唯一，native linker 不得改变已确定选择。

最终 image descriptors 和六张表由 program-link 从所选 records 生成，不改写输入 `.slib`。选择与实际内容进入 RuntimeImage、Code、Artifact 和 ResolvedLinkPlan 的对应摘要。

OptimizationMode 在 machine request field 9、production manifest field 12 中编码 Debug=1、Release=2；compile cache domain 为 `scoop-cone-compile-cache-v2`，mode 位于 field 13。generated-C flag 的 SelectedOptimization tag 为 15，tag 6 保留不用。

digest kinds 4、8、9，digest owner tags 4、11、9，以及 RegistrationDefinition patch role 1 保留不用。registration 公共 identity 只含 linkage、semantic ID、ODR group/member；body、layout、descriptor、scan 与 normalized-stackmap fingerprints 仍用于所选实现自身的内容、引用及 GC 合同，不作为不同优化正文必须相等的条件。

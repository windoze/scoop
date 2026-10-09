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

Native C++ 支持由语言规范 12.2.1 的 `native.cxx` 显式选择，仅支持 Darwin/AArch64 与 Linux/amd64 GNU。当前 musl 工具链未配套 C++ 标准库和 ABI 运行库，C++ 与 musl 的静态／动态链接组合均不支持；源码构建和 artifact-only 消费都必须诊断该组合。

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

源码集合非空，文件具有唯一 `SourceIdentity { cone, logical_path }`。manifest Cone 的 logical path 为完整的标准化 Cone-relative source path；默认目录中的文件保持 `src/...`，显式源码根不截短路径或改变相同文件的身份。single-file 为 `main.scoop`。host 路径只用于本次 I/O 与诊断。parser 只接收按语言规范 12.2.2 及本规范 2.7 选定的完整源码集合，不自行补扫目录。

AST 保留原始源码顺序、byte span、显式类型实参、位置／命名／spread／尾随 lambda 实参，以及独立的声明、表达式、模式和控制转移结构。文件至多有一个开头 package，其后为 import，再后为声明。qualified path 为非空 identifier sequence；package、静态 owner 与实体的分界由名称解析决定。

AST 不包含依赖 locator 解析、构建顺序、已推断类型或实体身份猜测。语法恢复产生 Error；任何解析失败阻止成功 HIR 输出。

### 2.2 HIR

HIR 完成名称解析、类型检查、重载与泛型实参选择、可见性、继承与覆写、模式穷尽性、构造／初始化规则、常量、效果及 FFI 声明检查。源码与依赖声明遵守同一语言规则；文件发现顺序、声明存储位置和 provider 读取顺序不能改变结果。

`SemanticHir` 保存已检查的声明与正文，对外有两个不同的输出合同：

- **ExportHir**：跨 Cone 的声明与必要正文。区分 public/re-export lookup、protected inheritance/slot、generic hidden support 和 interface dependency；支持声明的存在不扩大可见性。保存签名、默认值、const、alias、注解、类型关系与模板，普通非泛型函数的实现正文不复制给下游。
- **LocalConcreteHir**：本 Cone 实际需要发射的完整实体与正文。全部类型参数已替换，所有调用、构造、字段、variant、slot 与 external target 已确定；不含未完成推断、待实例化请求或上游普通函数正文。

两者使用不同的实体 ID 家族和显式映射。导出声明、完整泛型 application 与本地 concrete 实体不能用同一 arena index 或别名混用。

executable 的入口发现只检查 root Cone，并按语言规范 12.4.4 一次确定 `NoArgsUnit`、`ArgvUnit`、`NoArgsInt`、`ArgvInt` 四种形态之一。LocalConcreteHir 保存实际 main 的 typed identity、完整源码签名和入口形态；有参数时同时解析 argv 构造所需的实际 core `Array<String>`、String 及 helper targets。argv 构造使用普通 internal core 函数 `__scoopProgramArguments(): Array<String>`，通过现有 compiler protocol 记录实际函数 identity；其正文读取 runtime 已保存的 raw argv，严格解码、复制并报告非法参数下标。raw argv 访问器的 Scoop 返回类型为 `Ptr<Int8>?`，保留越界返回 null 的契约；helper 在 argc 范围内访问并按普通 Option 规则取出指针。后续 stage 使用该 typed 引用，不按名称重新查找。缺少入口、多个入口或非法签名在 HIR 报错；不能留给 linker 通过函数名挑选或修补。

每个 expression 具有非可选类型和 definition/evaluation origin。已解析调用保存完整逻辑签名、适配前 receiver 类型、实参映射、唯一 typed target 与效果；receiver 和实参的执行顺序遵守语言规范 8.5，不由类型求解顺序决定。

默认值、泛型正文及词法 callable 保存定义处绑定、完整 binder、局部值和 capture 身份。再次消费只替换实际类型与求值上下文，不重新选择名称、重载、字段或接口默认实现。局部函数和 closure 的捕获引用原绑定，不能改为消费方的同名局部变量。

默认模板分别保存提供方参数的预期类型和正文表达式的实际类型。前端按子类型规则检查默认值；`Nothing` 正文可以满足任意参数类型，产物投影、读取及实例化必须保留这两个类型，不能要求它们逐字相同或把底类型调用改写为参数类型的成功返回。模板契约仍校验预期类型与原提供方参数一致；通用格式构造器不重复进行依赖源码类型表示的子类型判断。

源码调用记录的实参保留实际表达式类型。通常实参已由前端适配为参数类型；Nothing 实参保留底类型，在 HIR 源码签名和 MIR 实现签名连接时通过实际 nominal 的 intrinsic 表示识别。两处均保留参数个数和位置检查；调用结果仍须与被调用声明的完整结果类型相同，不能因上下文需要而改写。

nominal application 保存原声明与完整实参；struct、enum、class、interface 各有独立 typed identity。alias 透明展开；字段、variant、构造与 property accessor 保存各自原身份。按值循环、继承环、泛型约束、GC-free 和 ZST 属性在所属语义边界确定。

property 具有完整类型、读写能力、getter/setter、访问域与互斥的 stored/accessor/delegated/const/extern 表示。required accessor 有 Body 或 AbstractSlot 的明确类别；普通缺失初始化不能靠空正文、late-init flag 或可空类型补齐。

class 构造输出明确区分 allocation、同 receiver 的 initializer、this/base delegation 与 common initialization；完整成功路径、异常出口与初始化顺序已确定。abstract class 仅有供派生构造使用的 initializer。release policy 为 None 或携带完整 hook target 的 SynchronousGcFree。

singleton、runtime property 与 generic delegated application 保存完整初始化单元及 cycle throw target。constant 已完成依赖环检查与求值，没有 runtime unit。interface/default/virtual 选择在 HIR 唯一确定，qualified super 保存 direct target。

`==` / `!=` 按语言规范 9.3、11.11 从普通成员中选择 operator equals，保存实际 callable 或普通接口 bound target；不要求 core Equality conformance。`Equality<T>.equalTo` 按普通 interface 声明、override 和 bound call 处理，不进入 core operator／protocol 身份表，也不为同形方法或结构比较补齐接口。

判定派生签名是否被手写成员占用时，检查名为 `equals`、参数为完整宿主类型的原成员签名，不以 operator 标记或当前调用处的可见性过滤。普通同签名成员仍可按普通函数调用，但不会成为 operator；导入的 nominal 使用已保存的成员声明执行相同判断，不因成员正文或源码不可见而重新派生。

结构比较沿用引入 Equality 统一方案之前的派生、具体化与产物路径。M33 只撤销 operator 对 Equality 的依赖，不新增按泛型上下文区分的比较模板、生成身份或 ODR 规则。`Equality.equalTo` 的新增实现由普通接口、泛型正文、装箱和跨库调用机制承担。原有泛型结构比较问题单独记录，不作为本轮库接口拆分的前置条件。

派生比较保留值传递、可见性和 InteriorMutable 的实际类型使用检查。NoGc 实现满足 Managed interface slot 时分别保留实现体 effect 和接口调用 effect，适用于显式 `equalTo` 及其他普通接口方法；独立的 intrinsic operator 比较保留原 effect。其他签名、访问、ordinary/suspend 与安全性检查遵守普通规则。

extern 声明产生完整 `SourceNativeExternalContract`：symbol bytes、逻辑 library/default namespace、function/data/TLS、mutability、C/Scoop ABI、calling convention、完整的 native 签名、GC effect 与定义位置。Scoop callable 另保留完整源码签名；启用 errno 捕获时，两者按语言规范 13.4.2 建立明确的结果适配关系。普通／suspend 效果与 GC effect 独立，后续不得由 symbol 或参数类型反推。

C ABI extern 函数的声明按语言规范 13.4.1 另存非可选的调用模式 `NativeSafe` 或 `GcLeaf`：未标注时前端填入 NativeSafe，合法 `@GCLeaf` 填入 GcLeaf。此模式随声明、模板实例化和实际调用经 HIR/MIR 传递，不由参数 GC-free、`@NoGC` 或 native symbol 名称推导。它属于当前 Scoop 声明的 caller 协议，不进入目标 C symbol 的物理 ABI 合并键；同一 C symbol 的不同声明可以选择不同模式，但不得因此放宽签名等既有 native 合同检查。

C extern 的结果适配明确区分普通返回与 errno 捕获。前端检查 `captureErrno` 的常量、适用位置，以及透明 alias 展开后的精确二元 tuple `(R, Int)`，将第一项 `R` 投影为 native 返回类型；`Unit` 投影为 `void`，其他类型按既有 C-FFI-safe 规则检查。合法捕获分支完整保存 native `R`、Scoop tuple 结果与捕获行为，不以可缺失字段或后续重新猜测 tuple 形状代替。调用、函数引用及泛型消费保持固定的 Scoop 返回类型，不因结果被忽略而关闭捕获。release 调用对完整 Scoop 参数和结果应用既有 `ReleaseValue` 检查。

C-FFI-safe、Scoop ABI、GC-free 与 release-safe 是不同合同。C 参数不得含 managed reference，Scoop ABI direct ref 不改为 handle 或 pin。generic 条件按原 binder 保存并在实际 application 检查。

实际依赖分为 lookup observations 与 committed uses。前者包括空查找、不适用候选、star surface 和 re-export；后者才形成 MIR/LIR 的 callable、类型、布局与 native 需求。未展开默认值和未物化模板不产生机器调用。

具体化沿用当前 Cone 声明的 nominal 物化闭包作为初始根，包含本地非泛型声明；正文调用、存储及其他实际类型用途继续扩展具体化闭包。本地物化集合与发布声明闭包承担不同职责，不能用较小的发布闭包替代本地根，也不因本地物化而发布私有类型。比较实现按实际调用与发布需求生成；类型的接口集合始终来自普通声明和继承关系。

定义 core 时，输出协议所引用的基本类型主体也通过同一物化请求入口补齐；它们不依赖某个普通接口或无关源码调用间接触发物化。已物化的主体复用原结果。

公开非泛型值类型的可用派生 operator 正文由定义 Cone 发布，消费者引用该生成 callable；字段不可比较时不发布该正文，也不因此拒绝类型声明或构造。普通 `.equals` 调用与 `==` 使用同一派生成员签名；需要实现显式用户接口中的 operator slot 时，定义处即检查并生成对应正文。

### 2.3 MIR

MIR 只接收 LocalConcreteHir 和实际选用的依赖 MIR metadata，不接受 ExportHir 模板或执行泛型推断。每个 expression 具有本层 exact type；GC-free、ZST、字段和 variant 身份完整保留，synthetic 类型在产生时也完整定义。

errno 捕获调用保留 HIR 已确定的结果适配和 native 合同，表达式的结果仍为完整的 Scoop tuple。后续解构、存储或跨 suspend 保存使用普通值规则；不引入 last-error 读取节点、线程槽位或额外的 managed error 对象。

MIR 控制流显式表示普通边、异常边、循环目标和 cleanup。Return、Break、Continue 只执行真正退出的 scope 清理；Throw/Rethrow 保持异常路径；finally 的覆盖规则和挂起不退出作用域的规则遵守语言规范。variant payload 只在同一值、同一 variant 的有效分支内读取。

语言规范 13.11 的数据借用 intrinsic 在 MIR lowering 中展开为一次 receiver/block 求值、typed pin 帧 push、按精确对象种类取得数据指针与长度、普通 closure 调用以及所有正常／异常出口的 pop cleanup。block 为普通非 suspend 函数，帧不跨挂起保存。MIR/LIR 明确区分帧操作与数据地址操作，不把栈帧创建伪装成返回 native callee 栈地址的 runtime 函数。LIR 按实际 Array/String layout 计算元素区和长度字段；codegen 在 caller 入口分配帧存储，调用运行时规范 3.4 的 NoGC push/pop。数据地址发射为标注 scoped-data-borrow 语义的 AS1→AS0 转换，LLVM 验证器按该已确定的 typed 边界接受转换，不允许反向转换或任意 managed 地址空间转换。原始数据指针是借用域中的 GC-free 值，帧中的对象承担保活与禁止移动职责；零长度数据地址不要求实际存在可解引用的元素。

所有调用已确定 direct、virtual 或 interface target。super 不重新进入动态分派；方法 receiver 为完整值参数，value receiver 的可观察存储属于本次方法调用。

`==` / `!=` 保存已选定的 operator callable；若它来自普通用户接口，则保留该接口 application、slot 与实际 implementation。派生结构比较降为普通 typed 成员调用和短路控制流；`Equality<T>.equalTo` 是独立的普通接口调用。bound 调用在单态化时映射到已确定的实现，不能重新按名字搜索 equals 或 equalTo。基本类型的精确 intrinsic 比较保持原运算与 effect；接口分派需要的适配正文同样是完整 typed callable，不能把 NoGc 直接实现误当成具有相同 effect 的 Managed slot 入口。

使用结构表示的值，其装箱接口闭包来自 LocalConcreteHir 的实际声明／继承关系，并由 MIR 的实际表项写入产物。tuple 的结构比较不增加 Equality 表项；Ptr 的显式接口仍按普通规则保留。产物读写检查接口引用、slot、callable 与签名的一致性，不因 exact type 使用结构表示就强制接口集合为空，也不在后续 stage 重放字段相等或源码 implements 的决议。

class allocation 只分配最派生对象一次，使用实际 exact TD；base/this initializer 不分配、不修改 TD。receiver 在可能 GC 的操作间保活并重读，只有完整构造成功后发布 release-ready 与结果。构造失败没有可用结果。

closure 具有完整捕获字段、类型、invoke 与扫描属性；绑定方法引用在调用时保留原动态分派。普通 local direct call 的捕获参数不改变原绑定身份。callback adapter 有精确 closure/native storage/status 签名，在返回 C 前处理全部异常。

suspend callable 的执行 ABI 为 `(source args..., Continuation<R>) -> CoroutineStep<R>`，后者仅含 Completed(R) 与 Suspended。输出不保留源码级 suspend call；frame、continuation、恢复入口和 saved slots 均有独立身份和完整类型。跨挂起只保存已初始化的 exact payload，不保存 native block address 或 active EH record。

接口 adapter 的物理 receiver 是具体对象引用；装箱 adapter 挂起时保存的 receiver 使用生成 box 的 exact type，而不伪装成源码 interface。其 `CoroutineSlot<box<T>>` 以 box 的 exact identity 区分，并与 box 一样归属 T 的定义或 application ODR group；扫描仍为一个 managed object 引用。普通 nominal 的 box slot 随已发布的 box 支持输出，纯内部 saved slot 不扩张私有 payload 的发布闭包。

object 的源码 exact type 与 backing class exact type 是同一对象的两种视图，共用分发表及 receiver adapter。adapter 的 implementor 保留源码 object 身份；backing 表通过类型记录中已有的 `Object { backing }` 关系引用它，不另建 callable 或放宽为任意继承 receiver。

completion 与 resume 具有唯一获胜方，并明确 payload 的 release/acquire 关系。恢复失败沿原调用点的异常／cleanup 路径传播。挂起状态使用 frame 的 TaskContext，恢复前后的切换见 2.16。

MIR metadata 保存源码声明到实际 callable、constructor、accessor、generated body 的映射，exact signature、GC effect、继承、dispatch、object/init 关系及 Strong/ODR 归属。它不定义目标布局；布局由 LIR metadata 提供。

mir-lower 完成 concrete CFG、dispatch、closure／adapter 与协程执行 ABI 后，由独立优化模块依次执行 receiver 实际类型传播、唯一目标去虚拟化、按调用点选择的小函数内联、局部常量／CFG 清理和再次去虚拟化；driver 显式传入优化配置。scoop-mir 保持纯数据职责，program-link 不隐藏重编译或导入普通外部正文。

release 默认启用 MIR 优化，debug 保留未优化的完整 MIR；两种模式的阶段 dump 分别记录实际输出，使用相同的实体身份与 ABI。数据流在异常边合并可能抛出时的局部状态，不能只把 block 的最终状态交给 handler。实际表项或闭包 invoke／bridge 完整可用时才改写目标；外部目标使用已经选中的 callable 签名和引用。

函数内类型事实为不可达、未知或唯一实际类型。最派生 allocation、已知 box／closure 和 final class 提供事实，复制和合法转换传播，合流只保留一致事实，循环求不动点；未知返回、可变字段和取址后的可变 local 保守处理。base／this initializer 不收窄实际类型，initializing receiver 的使用仍受语言规范中的构造约束；静态 open class 或本 Cone 实现数量不构成全程序闭合。去虚拟化按原 typed slot 查询实际表项，同时改变 callee、CallKind 和 receiver 适配，保留原 effect、EH、Context 与 continuation。

自动内联只处理本 Cone 已有完整 concrete 正文的非递归 callable，包括本地物化的外部泛型和普通 adapter；递归 SCC、无正文的外部 Strong、native／callback／gateway 和无法完整重写的特定 EH／Context／协程控制流保留调用。内联重分配 local、value、block 和临时存储身份，连接 return／异常出口，保留实参各求值一次、独立按值 place、位置、初始化发布和清理。Managed、分配或可能抛异常本身不构成拒绝理由；不复制后端 root/frame plan，站点由后续 LIR 统一形成。

首版克隆不含自身 landing pad／catch／resume 协议的普通 CFG；callee 的可能抛出操作与显式 throw 继承调用点的异常出口，caller 已有 catch／finally 可以继续承接异常。依赖函数局部异常槽的 callee 及协程执行入口保留调用。克隆产生的局部值属于 caller 的新 SyntheticValue 身份，所有形参先取得独立局部副本；常量折叠只处理可证明无副作用的布尔与整数运算，整数保留对应 MIR 运算的精确位宽与符号规则。地址可见的 local 不参与常量替换。Managed callee 的循环保留 poll 标记，NoGC callee 的循环内联到 Managed caller 后也不得新增 poll。

多个内联 return 先写入独立的汇合临时值，再在 continuation 初始化原调用结果；不得把原本稳定的结果 local 改为可重复赋值的身份，破坏其后 variant test／payload projection 的关系。优化删除引用后，已选外部 callable 声明目录可以保留未使用项及其稳定索引；声明的完整签名仍保留，实际链接成员由对象中的真实引用选择，不因目录项无调用而拒绝合法 MIR。

带 release 的对象构造保留同块相邻的 allocation、最外层 initializer 调用与 release-ready 发布。内联可以整体复制包含该序列的普通正文，但对应的最外层 initializer 调用本身保留，不将发布拆入缺少构造前序的 continuation；异常路径仍不发布。

输出为 Library 或携带本模块非可选 entry 的 Executable，不能由 `Option<Entry>` 与独立 kind 拼成矛盾状态。entry 保留 HIR 已确定的四种形态、本层 main target 和完整签名；生成的 root gateway 明确区分成功退出码与失败状态，有参数形态包含受检 argv 构造与 main 调用的完整正常／异常路径。gateway 与相关 helper 是实际需要发射的 typed callable，不能留到 program-link 阶段生成 managed 语义。

### 2.4 LIR

`ManagedPoll` 表示运行时规范 3.2 的条件 poll，携带唯一慢路径 site 与完整 live set。codegen 发射 acquire 检查和快／慢分支，只在慢分支物化该站点的 managed leaves、调用协调入口并回写 relocation；后续使用通过普通 canonical storage／SSA 合流取得快路径原值或慢路径新值。入口 pending 的 mode 检查属于同一操作，不能仅按 world phase 删除首次激活。TLS 与全局数据引用使用闭合 runtime ABI 目录中的普通数据合同。

LIR 保存目标上完整的类型布局、值表示、字段 offset、alignment、scan、调用 ABI、控制流与实际定义／引用。codegen 不按名称、实参、result storage 或上下文补类型、签名、poll 或逻辑 live set。

每个 pointer（包括 null）具有 Managed、Raw、Code 或 Metadata provenance；LLVM opaque pointer 不抹去这种区别。managed 使用 address space 1，其余使用 0，转换必须为明确合法的 typed operation。

Scoop ABI 保留完整逻辑参数序列及 exact identity，物理分类为：

| 值 | 参数 | 结果 |
| --- | --- | --- |
| Unit / ZST | ElidedZst，仍求值 | Unit 为 UnitVoid；其他为 ElidedZst。 |
| scalar、单字 managed/raw/code pointer 及其 niche enum | Direct | Direct |
| interface 及其 niche enum | DirectParts：object、itab | DirectParts：object、itab |
| size ≤ 16、alignment ≤ 8 的 GC-free struct／tuple／tagged enum | 按目标分类的 DirectParts 或所需间接形式 | 同一分类的直接或间接结果 |
| 其余非 ZST aggregate | Indirect | Indirect |

DirectParts 明确保存每个 carrier 的类型、来源 offset／extent、alignment 与 provenance，以及完整返回约定。接口为 AS1 object 和 AS0 metadata，不能落入单 scalar 分支；slot 的隐藏 receiver 独立采用单字 object projection。所有 caller／callee、typed invoke、adapter、Scoop ABI extern 和跨 Cone 签名消费同一计划。

LLVM 22.1 的接口结果跨 safepoint 时，沿精确 leaf 回写协议重新取得 object，再与原 metadata 重建双字值；重读必须阻止普通优化将结果重新合并为 GC 前的 aggregate SSA。首版在实际 relocation 路径使用 volatile object reload，条件 poll 仅在慢路径执行该动作，不为快路径引入无条件 root 临时存储。不能假定 RS4GC 会重写 aggregate 内部的 managed leaf。

GC-free 小值按 exact layout 和 scalar leaves 复用下述目标 aggregate classifier，保留整数／浮点／混合寄存器类别；tagged enum 的固定 tag 和共享 payload 字节区域不随 variant 改变分类。每个 aggregate 作为整体分配寄存器或回退；coercion 不越过 exact storage，padding 确定，浮点按 bits 搬运。callee 按需要建立独立 place，不能把 caller storage 当成按值别名。MaybeUninit 依自身值表示分类，不继承 T 的 nonnull、tag、niche 或专门接口 ABI。

小值的参数与结果分别保存 coercion：SysV 参数展开为最多两个 INTEGER／SSE carrier，结果为单 carrier 或双 carrier 结构；参数分类在完整签名中扣除隐藏 sret 和先前参数占用的 GPR／SSE，任一寄存器类别不足时整个 aggregate 使用 byval，未使用的另一类别寄存器留给后续参数。Darwin 普通 aggregate 参数按 64-bit 字块（需要 16-byte 对齐时为 i128）传递，短结果可使用精确位宽整数；HFA 使用保留连续寄存器约束的浮点数组 carrier。HFA 必须由 1 至 4 个同精度浮点叶组成，且叶连续覆盖完整 storage；额外 padding 不算浮点成员。Darwin 的连续 carrier 由 LLVM 的 aggregate 参数规则整体放置，不能展开成彼此无关的标量参数。

carrier 记录实际读取的 extent，允许其机器类型宽于 extent；读入前将 carrier 临时存储清零，只复制 extent 字节，回写亦仅复制该范围。跨 Cone canonical ABI 保存完整 carrier 序列而非仅保存 DirectParts 标签，target classifier 的版本同时进入现有 target fingerprint。产物消费者复用保存的物理计划并检查 exact storage／引用一致性，不另行通过源码重建一套参数分类。

每个 indirect 实参使用调用方新建的 exact storage，callee 按值接收。物理参数为 indirect result storage（若有），随后按逻辑顺序省略 ZST 并传递其余参数。LLVM 使用对应 exact type/alignment 的 byval/sret；实际寄存器分配由目标 ABI 决定。Darwin sret 使用 x8，amd64 使用 RDI 并在 RAX 返回同一地址。

ZST 的求值、调用、构造、类型及对象身份不能因零 payload 消失。被观察地址的独立 place 有满足 alignment 的存储 token；同时存活的不同 place 不共享地址。Ptr<ZST> 的元素位移为零，仍遵守 operand 求值和 unsafe 有效性条件。

call target 同时确定 destination、calling convention、全部参数与 return convention；void/direct/indirect-result 以及 Managed/NoGc/NativeSafe/NativeGcLeaf/NativeBorrowed 使用完整互斥分支。Managed call/poll 带 site ID 与逻辑 live set，invoke 带正常／异常 root set；NativeSafe/NativeBorrowed 带 site ID、caller roots 与按返回形式确定的 result publication。NoGc 与 NativeGcLeaf 不携带虚构 safepoint 或 root plan；NativeGcLeaf 保留完整 C ABI 调用计划，不能伪装成普通 Scoop NoGc ABI 调用。C extern 的物理调用计划为 DirectC 或 StorageBridge，与 NativeSafe/GcLeaf 协议独立。

live set 包含 post-site live references 与 managed 实参，以及 aggregate 的完整 managed leaf path。普通 call/poll 的机器 relocation 与显式 invoke/native root frames 遵守运行时规范 3.2；跨 transition 的实参和结果必须使用更新后的存储。

root gateway 使用运行时规范 2.8 的精确 C 原型：argc、原始 argv、指向独立 Int32 退出码槽的 raw pointer，返回 UInt32 status。其内部对 main 的调用仍使用实际 Scoop ABI；不能因外层返回 UInt32 就覆盖 main 的 Int 结果。入口 poll 先于 argv 的 managed 构造，数组、当前 String 与其他跨 safepoint 的引用使用普通 roots、relocation 和写屏障；数组构造及 main 的异常在 gateway 内完成捕获、failure-root 发布与 EndCatch。eager gateway 仍为无参数 status 返回，不能与 root gateway 共用错误的函数指针原型。codegen 在完整 LIR 边界验证实际 ABI 与 gateway 结构后，对象发布入口复用该结果，不再重放源码 main 形态检查。

值类型布局使用原 exact 类型与字段身份。tagged enum 的 tag、payload slot 与 scan 一致；niche 只用于语言允许的引用／pointer Option。整数宽度与 signedness、Float/Double 精度、内部计数和 runtime ID 均保留各自类型，物理位宽相同不等于同一实体或源码类型。

wrapping 整数运算不携带 nsw/nuw；除零和 signed MIN/-1 已有安全控制流，shift count 已按位宽正规化，compareTo 的结果为 Long。浮点运算遵守 2.18。

C ABI storage 为封闭类型结构：integer、Boolean、Float/Double、带完整 pointee 的 data pointer、带完整 signature 的 function pointer、完整 C-layout struct。只有结果可为 void，只有 Ptr<Unit> 的 pointee 可为 opaque void。by-value struct 关系无环，pointer 可引用 forward-declared struct。

nullable data/code pointer 保留对应 exact enum 与 pointer 类型，不得把同一 enum 绑定到不同 pointee/signature。CLayout 的 aligned/packed 各为 Natural、A1、A2、A4、A8 或 A16。C ABI storage 与 Scoop 值表示分别准确保存；C ABI 分类不能用 Scoop aggregate ABI 代替。

在当前 Darwin/AArch64、Linux/amd64 GNU 和 musl profile 上，固定参数的 cdecl C extern 未选择 `captureErrno = true` 时，合法 canonical C signature 必须使用 DirectC，包括 integer、Boolean、Float/Double、data/code pointer 和按值 C-layout struct。条件按已有 C-FFI-safe projection 判断，包括 Char、PinnedPtr／GcHandle 与合法 nullable pointer；不把普通 C-layout struct 凭大小或字段数当作标量。Unit 仅映射为 void 结果。errno 捕获继续使用 StorageBridge；本条不扩展 callback、FunPtr 或 native global/TLS 协议。

GNU／musl 共享 SysV AMD64 eightbyte 分类，保留 INTEGER／SSE／MEMORY、整 aggregate 的寄存器耗尽回退、byval 和 sret。Darwin/AArch64 使用对应 AAPCS64／Darwin 分类，保留 HFA、普通 aggregate、栈参数和间接结果；四个 Double 的 HFA 不受 Scoop 16-byte 小值阈值限制。完整物理签名覆盖 nested layout、padding、alignment、扩展、coercion 与隐藏参数顺序。classifier 只消费已有 C projection 和完整签名，不建立运行期 libffi 或 FFI 插件；以所选目标 C compiler 产生的独立函数作互调验证。

DirectC 由 LIR lowering 根据 canonical C signature 与目标 C ABI 确定完整的物理参数/结果、calling convention、小整数扩展和 Boolean 表示转换，codegen 按该计划发射 LLVM 类型与 ABI 属性，目标后端分配寄存器及栈位置。不构造 storage bridge 专用的参数局部变量、返回缓冲区或 memcpy 往返；目标 ABI 要求的栈传参、byval、sret 与表示转换保留。StorageBridge 保留完整 storage signature，由系统 C compiler 生成桥接；不能只设置 LLVM C calling convention 就把未经分类的 struct 直接传递。

DirectC 的聚合值计划复用带 offset／extent／alignment 的 carrier 描述，并分别保存参数与结果 coercion。SysV MEMORY 参数记录 `byval` 及至少 8-byte 的参数副本对齐，隐藏 sret 使用结果的实际对齐且占用首个整数参数寄存器；寄存器不足的聚合参数整组回退。Darwin 超出寄存器 aggregate／HFA 分类的参数传递 caller 的独立副本指针，不标为 SysV `byval`，间接结果使用 sret 的 x8 约定。副本对齐与原类型的存储对齐分开保存；原始 packed storage 不因 ABI 要求而改变布局。

当前三个 profile 的 DirectC 参数与结果中，有符号／无符号 8-bit、16-bit 整数分别使用 signext／zeroext；Boolean 使用 LLVM i1 与 zeroext，32-bit、64-bit 整数及浮点、pointer 无整数扩展属性。TargetProfileContract 的 C ABI lowering 记录 aggregate 分类合同，进入现有 target fingerprint 和构建缓存键；迁移时登记新 wire tag，退役的 ScalarDirectOrSystemCBridge tag 2 不复用。

`captureErrno = true` 的 LIR 计划分别保存真实 C 函数结果 `R`、bridge 的 `Int32` 返回值，以及 Scoop `(R, Int)` 的 exact layout 和结果重建操作。带捕获的 bridge 继续通过原有 caller-owned C result storage 写回非 void 的 `R`，以 C `int32_t` 返回捕获的 errno；`R = Unit` 时省略原结果缓冲区，在 Scoop 侧构造零 payload 的 Unit 元素。普通不捕获的 storage bridge 保持原有返回方式。tuple 不出现在目标 C 函数原型中，也不作为 C struct 返回；必要的 C/Scoop 表示转换在已保存 errno 后完成。

每次调用使用独立、在 native 调用期间地址稳定的 GC-free 临时存储；嵌套、递归与 callback 内的调用不能复用仍在使用的缓冲区。NativeSafe 调用在返回握手后使用结果，GCLeaf 与 release 按各自无切换路径使用结果。捕获值及包装 tuple 不增加 GC root、Scoop TLS/thread 操作或堆分配，也不能代替原有实参保活与指针生命周期规则。

compiler-owned static storage 带完整 scan 和封闭初值 ZeroedForRuntimeUnit 或 EncodedStaticValue；后者具有 canonical bytes 与排序的 immortal relocations。raw global/TLS 另存完整常量，不进入 managed static-root registration。ZST token、全零初值与 relocation 规则见运行时规范 2.8。

LIR metadata 保存完整 value/instance/element layout、Scoop/C ABI、scan、TD、dispatch、initialization、runtime registration 与 required definitions。ordinary nominal 定义使用原 provider 的布局；实际泛型或 structural application 使用其 ODR 布局。结构字段可按 canonical exact key 组成，不要求为没有实际物理定义的 constituent 另造 layout 实体。

每条 native contract 按 target 正规化 symbol，并保存完整 kind/library/signature/storage/GC effect；它进入 LIR 语义内容，host 搜索路径不进入。String 关联实际声明的 TD 与 provider，不由名字或相同 InlineBytes 布局代替。

### 2.5 Codegen 与对象合同

codegen 的语义输入只有当前 Cone 的完整 LIR；Scoop producer 消费 LIR target/backend，generated-C producer 消费 LIR target/C toolchain。输出可以有多个对象，物理分片数量与文件名不是 Cone 或实体身份。

managed 单槽写屏障按运行时规范 3.6 内联读取四级 page map 与 region prefix，再原子置脏；这段计算没有 safepoint，不保留跨 safepoint 的 region metadata。runtime ABI 数据符号 PageMap 使用 tag 32，旧单 arena CardTable 的 tag 6 退役；PageMap 的 32768-byte／8-byte 对齐数据合同和新符号进入现有 registry fingerprint。范围写屏障保持既有 NoGC 入口，由 runtime 逐映射标记完整范围。

每个实际物理定义都有 typed owner、role、linkage、symbol 和精确 section range。引用使用 typed target 与真实 relocation；跨对象 external 声明不能变成可缺失的 weak reference。Strong 属于原定义 Cone，ODR 使用其稳定 member identity。

对象边界验证目标格式、section/symbol extent、alignment、权限、relocation 写入宽度/addend、定义与引用闭合。Mach-O 使用其真实 relocation 与 weak 属性；ELF 使用 ET_REL、实际 section/symbol/RELA/COMDAT 数据，TLS 必须保持 STT_TLS/SHF_TLS，不能伪装成普通 data。

relocation 的 section、atom、offset 和 target 均属于本次实际对象。raw source-native symbol 按 Darwin 添加一个前导下划线、ELF 保持 bytes 的规则正规化。Scoop-owned symbol 从 persistent identity 产生，不用 FQN 或 source 短名回退。

所有 Scoop body、NoGc body、managed adapter、root/init gateway 与 release hook 都有 callable registration。生成 C bridge/trampoline、runtime C 和 ordinary native producer 使用各自实际对象合同。实际保留的 safepoint 与其 owner、role、machine root count、stackmap 完整对应；无 safepoint 的 body 也可合法登记。

body 的 EH、LSDA、unwind、stackmap、关联常量与私有 Context cells 属于该 body 的同一次物理选择。TD、scan、dispatch、static storage、初始化和六类 registration 满足运行时规范；release hook 为 raw C、nounwind、NoGC，无 managed transition 或 EH/safepoint。

C extern 按 2.4 的 DirectC/StorageBridge 计划发射。DirectC 使用真实 native symbol 和完整目标 C ABI，不生成或经过 outbound storage bridge；对象中的调用 relocation 直接表达该 native 引用。只有实际采用 StorageBridge 的调用才要求相应 generated-C unit；callback、native global/TLS 等其他桥接用途照常保留。两种路径均保留相同的 native contract 与 library requirement，不因省去 bridge 丢失链接需求。

generated-C bridge 保留实际声明的 C 类型、签名、目标工具链与编译选项；bridge 具有精确 primary/signature/context 定义范围，不能由邻近符号猜测。DirectC 与 StorageBridge 均不得按 extern 名称被 builtin 替换。普通 C ABI 与 Scoop NativeBorrowed 调用不混用 storage bridge。标量直接调用不依赖 bridge 内联、跨对象 LTO 或相容的 Clang bitcode；现有 generated-C 独立编译流程继续适用于保留的 bridge。

errno 捕获 bridge 使用所选 target 的 `<errno.h>`。全部参数解包及必要准备完成后，先执行 `errno = 0`，再调用真实 C 函数；紧接着以独立语句将 errno 复制到 native 局部整数，然后才复制 `R`、执行其他 helper 或返回捕获值。当前 profile 的 C `int` 为 32 位，可精确映射为 Scoop `Int`。非 void `R` 通过既有 C storage 写回，bridge 返回 `int32_t`；void 不产生虚假的 C 返回对象。目标 libc 的 errno 访问所产生的 native 引用进入既有 generated-C 对象与链接需求，不能使用宿主 accessor 或新增 Scoop runtime 槽位 API。

NativeGcLeaf 在两种物理路径上都不发射 enter/leave native、RETURNING、statepoint、caller-root push/pop 或专用于该调用的 relocation reload；调用前后表达式的真实 safepoint 不受影响。NativeSafe 可复用 DirectC，但仍保留自身的线程协调与 caller roots。DirectC 的 GCLeaf 调用不增加桥接函数调用或由桥接协议导致的参数/结果内存往返；不承诺消除 ABI 必需的搬运、正常寄存器 spill、动态链接跳板或显式的 errno、pin/unpin 操作成本。`captureErrno = true` 在 M33 使用带立即捕获的 StorageBridge，不属于无桥接开销的范围。

GCLeaf 契约不意味着 pure、readnone、readonly 或 nosync，不能据此删除外部副作用或推导用户数据同步属性。代码中的调用模式必须与所属 Scoop 声明一致，native symbol 去重不改变已选择的模式。直接调用的 ABI 通过与目标 C compiler 的调用交互验证；验收同时检查优化后的机器码，确认 DirectC 没有新增 bridge 调用或桥接缓冲区，GCLeaf 没有 GC 边界操作，不能只凭 LIR 标记或微基准耗时判定。

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

GeneratedBridgeUnitId 的 domain 为 `scoop-generated-bridge-unit-v1`。key 为 OutboundFunction=1（native contract fingerprint 与普通/errno 捕获的结果适配）、GlobalRead=2、GlobalWrite=3、GlobalAddress=4（各含 native contract fingerprint）、CallbackTrampoline=5（C signature fingerprint 与 context parameter index）、StaticCallbackTrampoline=6（storage-bridge generated callable ID 与 C signature fingerprint）。OutboundFunction 的结果适配确定 bridge 的私有返回方式；同一 native symbol 的捕获与不捕获 recipe 不能共用 unit identity。NativeSafe/GcLeaf 不改变 bridge 本身的结果适配，仍可复用相同 recipe。

bridge unit 是与 producer 无关的 recipe identity；实际 atom 使用 producer ConeIdentity 与 PrimaryEntry、SignatureDescriptor、ContextDescriptor 或 StaticAssertSupport role，domain 为 `scoop-generated-bridge-atom-v1`。前三者可具有实际 bytes/range；StaticAssertSupport 不产生物理定义。unit、实际 atom 和 member 的关系必须完整，不能把 producer-specific symbol 写入 unit identity。

未知 optional capability 在长度与 hash 验证后可跳过，不进入语义 view。ExtensionBlob.required_for 只允许 0 或 Link bit；未知 Link-required capability 或 LinkObject verifier 阻止 Link 消费。blob 不成为第二种对象入口，已知 Link contribution 只能表示规范化 native library requirements，不能注入 raw linker argv、脚本或未检查对象。

完整生产 profile 为 `org.scoop-lang.slib-profile/cross-cone-generic/5`。descriptor 是五字段 map：`1=id, 2=required_manifest, 3=required_hir, 4=required_mir, 5=required_lir`；各列表按 capability 排序。其必需 section 为：

| 位置 / namespace | section 与 major |
| --- | --- |
| Manifest / `org.scoop-lang.manifest` | `single-cone-production/6` |
| HIR / `org.scoop-lang.hir` | `identity-foundation/8`、`core-bootstrap-interface/14`、`cross-cone-interface/69`、`cross-cone-type-semantics/27` |
| MIR / `org.scoop-lang.mir` | `identity-foundation/6`、`core-bootstrap-bridge/1`、`cross-cone-param-free-bridge/2`、`cross-cone-type-bridge/19` |
| LIR / `org.scoop-lang.lir` | `identity-foundation/8`、`cross-cone-param-free-bridge/3`、`cross-cone-link-closure/1`、`cross-cone-layout-abi/15`、`cross-cone-layout-link-closure/8`、`cone-production/11`、`link-identity-closure/15`、`link-support/1` |

各 section 按消费用途检查 required inventory。Compile 需要完整语言与相邻 IR 合同；Link 只消费 identity、ABI、对象、native、production 和链接支持数据，不为链接展开 HIR 模板。profile fingerprint 覆盖 descriptor 的实际内容。

manifest 保存 canonical coordinate、ConeIdentity、kind/source form、完整 exact direct dependencies 与三层语义指纹、language/runtime/identity/mangling ABI、target/backend 配置及其兼容摘要、全部成员目录、三层/Code/RuntimeImage/Artifact fingerprints 和 required sections。

production 与 Link metadata 完整保存实际 body/type/site ID、Strong/ODR 定义、shared ABI、object/bridge unit 到 member/range 的映射、digest patch 位置、runtime registrations、image/root entry 归属、defined symbol owners、undefined requirements、native contracts 与逻辑 library requirements。所有表具有 canonical key/payload、明确排序与唯一性；不保存 producer 的绝对搜索路径。

executable entry 的四种形态、main 的完整源码签名、root gateway 的实际 C ABI 与定义指纹随各层 typed entry 保留，并进入对应语义／ABI fingerprints 和承载它们的 section profile。Link 投影保留精确 main、gateway 与 failure-root 引用，使 artifact-only 链接无需源码或名称推断。M34 的 runtime ABI contract 12、metadata ABI 8 同时进入兼容检查、runtime 与启动对象的构建输入；旧 ABI 的薄接口、物理签名、poll／card 合同及产物必须重建或拒绝，不能用缺省字段或 bitcast 混用。新 ABI 的 debug／release 仍互通；实施中的批次和实际 section 版本见 [M34 记录](../milestone34/PROGRESS.md)。

operator equals 的成员签名、operator 标记、已绑定调用及必要的派生正文使用既有 callable、模板与 exact-type metadata。`Equality<T>.equalTo` 的显式 conformance、接口 slot、实现和分派适配使用普通 interface metadata；两者没有隐式关联，也不保存 Equality 专用的 core protocol 或条件接口规则。

`core-bootstrap-interface/14` 删除旧 Equality protocol 字段；`cross-cone-interface/68` 与 `cross-cone-type-semantics/26` 移除统一方案添加的派生比较接口槽位声明，恢复普通源码接口实现。旧版本产物必须重建，不能把旧 Equality slot 当作新接口成员或独立 operator 使用。既有结构 operator 的生成身份和单态化格式保持不变。

派生 operator 使用既有 typed 生成身份和普通单态化／ODR 规则，所需的宿主、完整签名及字段调用在各自边界确定；没有源码声明的生成 callable 不伪装成 SourceFunctionId。导入的 operator 候选来自已保存的成员或派生签名，不借用 Equality slot。artifact-only 链接只消费已闭合的实现，不重新扫描字段或选择重载；普通接口 adapter 引用实际声明或继承的实现，不因值支持结构比较而新增 itable 项。

接口方法与实际 metadata 内容的变化按既有 semantic／ABI fingerprints、section profiles 和缓存兼容规则演进。实现迁移时同步更新受影响的格式与 core 产物；旧的 Equality equals slot 不能作为 equalTo 消费。不因这次库接口拆分增加 runtime 比较 ABI。

`single-cone-production/6` 的 field 4 为完整 typed registration projection，field 6 为 RuntimeImage fingerprint，field 11 为物理 ODR member 目录，field 12 为 OptimizationMode；field 5 保留不用。ODR directory 的 member 保存实际 role 与 OdrAbiFingerprint，field 4 保留不用；纯语义 member 不产生空物理条目。

`link-support/1` 为单字段 map `{1=runtime_data_aliases}`，把必要的 runtime 数据符号关联到实际定义；String alias 不创建额外 TD 或存储。

persistent identity 只由首次引入的层声明一次，并同时保存 typed ID 与 canonical key；其他层引用它。相同 kind/ID 的冲突 payload 或跨层重复声明拒绝。LIR foundation 的 exact-type 集合引用已有 HIR/MIR exact records，不复制声明。源码文件、binder、字段、variant、body application 和调用位置的引用必须指向真实定义。

共有 HIR expression 为 `{1=kind, 2=result_type, 3=definition_origin, 4=evaluation_origin}`。Call 的 tag 为 57，payload 保存 callee、arguments、SourceCallReceiver；receiver 为 NoReceiver=1 或 Receiver=2（完整静态类型）。实际 call-site 保留完整逻辑实参与结果、定义／求值位置及原 receiver，Unit/ZST 参数不能按物理省略规则消失。

nominal declaration 保留原 binder 条件、GC-free pointee requirements 与 NoGC 标记。property/accessor、constructor safety/effect、InteriorMutable、CLayout、泛型 companion、release、annotation 和编码合成所需的声明事实均不可由缺字段的默认值替代。读取只检查格式、kind、引用与合同连接，不重新进行定义处的重载或可见性选择。

C extern 的 NativeSafe/GcLeaf 模式作为声明及调用的语义字段进入相应 HIR/MIR/LIR metadata、语义指纹和构建缓存。跨 Cone 编译与泛型展开保留原声明的模式；artifact-only program-link 消费已生成的调用，不能按合并后的 C symbol 统一提升或降级调用模式。该字段不改变 C bridge 的物理签名，也不要求为同一 native ABI 构造第二套桥接格式。

`captureErrno`、完整 Scoop 结果类型、native 返回投影及 bridge 结果适配进入对应声明、调用与 bridge 的 HIR/MIR/LIR metadata、语义/Code 指纹和缓存。编译消费方按已保存的结果适配生成 `(R, Int)`，链接消费方保留实际 bridge 及 native requirements；不按合并后的 native symbol 重新决定捕获，也不将旧的单结果 bridge 当作捕获 bridge。必需字段与 recipe key 的变化按既有 metadata/schema 兼容规则演进，旧产物不能缺字段后静默当作不捕获。

HIR `cross-cone-interface/68` 的 C extern implementation 必须保存调用模式与结果适配；LIR `identity-foundation/8` 的 OutboundFunction key 必须保存结果适配。generated-C 的 OutboundWrappers 模板版本为 2。目标工具链展开 `<errno.h>` 后产生的 libc errno accessor 引用作为普通 target-support native requirement 保留：Darwin 为 `__error`，GNU/musl 为 `__errno_location`，均为无参数、返回 native pointer 的 C 函数。源码生成仍只使用 `errno` 宏，不自行生成目标 accessor 调用。

DirectC/StorageBridge 及其完整物理调用计划进入相应 LIR metadata 与既有语义/Code 指纹和缓存投影，不加入 source native symbol 的 ABI 冲突键。DirectC 保留真实 native undefined reference、contract 与 library requirement；没有实际桥接用途时，不生成 outbound bridge recipe、物理定义或 member 要求。跨 Cone 与泛型消费按当前 target 得到同一完整计划；artifact-only 链接只消费产物记录，不重做 ABI lowering，也不为直接调用重新插入 bridge。

native-boundary record 保存 owner、type-parameter count、source shape 与 C ABI projection。projection 为 SourceRepresentation=1、UInt64Field=2（唯一 field ID）或 NullablePointer=3（empty variant 与 pointer payload field ID）；必须匹配真实声明的字段、variant 与类型，不改变普通 Scoop aggregate/niche 表示。

三层语义指纹是 Merkle 指纹：各自 canonical own-layer projection 加真实 direct/support/re-export 依赖的同层指纹。其 domains 为 `scoop-hir-semantic-v1`、`scoop-mir-semantic-v1`、`scoop-lir-semantic-v2`。LIR context 包含 identity ABI、target profile 与 target fingerprint，不含 backend optimization、member assignment、range 或 patch offset；backend 兼容性仍单独检查。

HIR 语义包括名称候选、签名、默认值、const、模板和完整依赖；无法完整表达细粒度 lookup observations 时，保守计入全部相关直接依赖。普通私有非泛型正文、机器码、优化模式与对象分组进入 Code/构建缓存，不能反向改变声明或 shared ABI 身份。

MIR `identity-foundation/6` 与 LIR foundation 保存实际实现的实体目录，包含优化新建的局部值；它们进入 Code 与 Link validation，不作为依赖语义贡献。MIR 的可消费语义由 bootstrap／param-free／type bridge 保存，LIR 的 shared ABI 由对应 layout／param-free section 保存。内联新增局部值不能使同一源码的 debug／release 依赖互相过期；完整 identity、格式、引用与 ABI 校验仍在各自边界执行。

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

- ManifestCone：目录下必须有 Cone.toml，或显式 regular file 恰名为 Cone.toml。未出现顶层 `sources` 字段时递归扫描 `src/`；出现后按语言规范 12.2.2 的完整清单选择当前 target 的源码，不追加默认目录。目录只收集扩展名精确为 `.scoop` 的 regular files，也可显式选择单个 `.scoop` 文件；最终按标准化 Cone-relative UTF-8 路径排序。
- SingleFile：operand 扩展名精确为 `.scoop`，解析 symlink 后为存在的 regular file。只编译该文件，logical path 固定 main.scoop；不读取相邻 manifest、Scoop/C/C++ source 或 blob。

manifest 字段、路径形式及 `when` 的合法性先于 target 筛选检查；文件系统检查和递归枚举只作用于选中路径。选中目录相同或互相包含、单文件与目录重叠、重复标准化路径或 symlink 重复指向同一源文件时，报告来源条目，不静默去重。选中路径缺失或类型不符、最终空集合、非 UTF-8、symlink 逃逸或循环均在 parse 前失败；显式清单为空或全部被筛掉也不回退到 `src/`。

源码选择和文件枚举在 Cone 编译缓存命中判断前完成。compile key 覆盖 manifest 语义投影、所选 target、选中文件的完整规范化相对路径及内容；未被选中的文件内容不作为源码输入。`sources` 的语义投影区分默认与显式模式，规范化路径、条目顺序和条件的等价写法；原始 span 不进入 key。输入快照保留选中的目录，包括没有 `.scoop` 文件的目录，使子编译器使用同一份清单时仍得到同一集合。`scoop` 与直接调用 `scoopc` 使用同一发现规则，向 parser 交付相同的完整集合；后续阶段不能再追加默认源码。`SourceIdentity`、file-private 实体及 SourceLocation 使用从 Cone 根开始的完整路径，选择条目的顺序和分组不进入身份；改变源码集合只通过既有源码内容、依赖和指纹规则影响产物。

SingleFile 的 coordinate 为保留值 `scoop:single-file:0.0.0`，kind 为 Executable，只依赖 core；用户 manifest 不能声明该 coordinate。build/run 此时拒绝 cone-path，scoopc 拒绝 direct/support slib 参数。其 `.slib` 可作为本次构建、缓存或显式 link 的 executable root，不能作为依赖或 manifest locator 指向的可分发 Cone。

依赖使用 exact version，全部为 library，同一 group:name 只允许一个版本。启动 compiler 前必须验证完整图的 identity/content 唯一性、target/ABI/schema、语义 fingerprint、无环和 locator 无歧义。canonical 顺序逐次从“全部依赖已完成”的 ready set 中取 coordinate bytes 最小者；不按输入顺序或并发完成顺序决定。

非 core Cone 未显式声明 core 时，注入 `scoop:scoop.core:0.1.0` direct edge；core 不依赖自身。core 是普通库，显式 locator 遵守同一发现规则。缺省 sysroot 来源先取 `lib/scoop.core`；只有目录不存在时才取 `artifacts/<canonical-target-id>/scoop.core.slib`。已选来源损坏或不兼容时直接报错，不回退到另一来源。

其他已声明的 `scoop` group 依赖在没有显式 locator、且全部显式 artifact search root 均无候选时，最后检查 `<sysroot>/lib/<name>/Cone.toml`。匹配完整 coordinate 和 library kind 后，按普通 source Cone 递归构建并使用普通缓存；不要求额外的 cone-path。新发现的默认源码 Cone 的显式依赖继续优先展开，直至普通依赖工作队列为空；缺失或损坏的默认源码按普通 source locator 报错。其他 group 不查该默认位置；显式来源失败、候选歧义或已选 artifact 不兼容时不回退。它只补充源码依赖定位，不注入依赖或扩大可见性；artifact-only link 仍不读取 sysroot 源码或补建缺失 artifact。

配套 scoopc 默认来自 scoop executable 同目录，允许显式指定；必须匹配 machine capability 和实际编译输入。每个 source cache miss 启动一次 single-Cone 编译，传入完整 direct/support artifact。direct 与 manifest edge 一一对应，support 恰为其余传递闭包，两组互斥。缺失、不可达额外输入、identity/content 冲突、stale edge、错分或 executable dependency 均在 parse 当前源码前失败。

library 不要求 main；executable 必须恰有一个本 Cone 顶层 ordinary、非泛型、非 suspend 的 main，接受无参数或单个 `Array<String>` 参数，返回 Unit 或 Int，具体规则见语言规范 12.4.4。依赖的同名函数不参与入口选择，manifest 与 single-file mode 使用相同规则。scoopc 的正式输出始终是当前 `.slib`，不构建 runtime、不解析 native library 搜索目录、不调用最终 linker。

build/run 接受 `--profile debug|release`，默认 debug；`--release` 为 release 简写，与显式 profile 同时出现为参数错误。实际优化合同见 2.19。

默认 target root 为 manifest Cone root 下的 target，SingleFile 为调用者 cwd 下的 target。`--target-dir` 相对调用者 cwd 解析并替换该根；输出目录追加 `<canonical-target-triple>/<profile>`。library 文件名为 `<cone-name>.slib`，manifest executable 为 `<cone-name>`，single-file 为 `<sanitized-input-stem>`。build 的 `-o` 覆盖最终 root 文件的完整路径，不追加 triple/profile。

发布为原子替换，失败保留旧输出，不能留下部分 artifact 或 binary。用户输出与缓存内容相互独立；输出不能覆盖本次输入或破坏缓存。并发发布到同一位置时，最后一次成功发布生效。

run 执行本次成功构建的 bytes，不能执行旧 binary 或被另一构建替换的程序。argv[0] 使用稳定输出路径，其余参数原样传递；带参数的 Scoop main 得到包括该第 0 项的完整数组，与 Java/Kotlin 从首个用户参数开始的数组不同。继承调用者 cwd、environment 和 stdio，保留操作系统实际报告的 exit/signal，不因源级 Int 超出 `0..255` 而拒绝运行，也不把 main 的非零返回值改写成 gateway 失败。工具输出在程序启动前完成，程序 stderr 不包装为诊断 JSON。

`scoop link` 接受显式 root/dependency artifacts、cone-path、必需 runtime object index、target、library search roots 与输出路径。它按 artifact exact dependencies 查找，core 也只从已有 artifact 取得；不启动 scoopc、不读取 runtime/Scoop 源码或补建缺失依赖。

`--sysroot` 定位 Scoop core 及上述 `scoop` group 默认源码库；Linux 的 `--cc`、`--native-sysroot` 选择 C 工具链。`--unwind-prefix` 和 `--link-mode static|dynamic` 属于 runtime/executable 链接输入，library `.slib` 构建不要求 unwind archive。Linux 缺省 unwind prefix 为 `<Scoop sysroot>/native/<canonical triple>/unwind`。Darwin 使用所选 Xcode 开发环境。

当前 Cone 的 `native.cxx = true` 要求解析同一 target 的配套 C++ driver：GNU 使用对应工具链的 `g++`，Darwin 使用同一 Xcode 工具链的 `clang++`。选中的 C++ 源码交给该 driver，`.c`、generated-C bridge、runtime 和生成的启动 C 代码仍按各自 C 编译配置生成对象。未启用 `cxx` 却选中 C++ 源码、启用后缺少配套 driver／运行库、启用后 target 为 musl，均在调用 native 编译器前诊断；不回退到宿主 C++ 工具链。

`c_flags`／`cxx_flags` 只作用于当前 Cone 的相应 native 源码，按 argv 元素与声明顺序传递。native 默认语言标准分别为 C11 和 C++20，可由对应 flags 中的 `-std` 覆盖。`cxx` 和实际生效的编译参数、C++ 源码及头文件、配套编译器与标准库 ABI 配置进入构建输入和 compile key。library 构建把 C++ 链接及运行库需求写入 `.slib` 的逻辑 native requirements，不执行最终 C++ 链接。

LIR foundation 以必需的 Boolean `native_cxx` 保存当前 Cone 的显式 C++ 运行库需求；该字段与普通 native library 表一起进入 Code 和 production native requirement 投影。foundation wire 升为 8，manifest single-cone-production 升为 6，严格拒绝缺少该字段的旧格式。需求不依赖是否选中 C++ 文件，也不从机器符号推断。C++ 编译器沿已选择 C driver 的配套位置解析，保留相同 target、sysroot 和环境；GNU 的 target 与 compiler version 必须匹配，缺失配套 driver、标准库或 ABI 配置直接诊断。

native 头文件依赖必须在外层 Cone 缓存命中判断前按实际 target、宏与 include 配置发现，包含实际使用的公开 `scoop_rt.h`；仅在子编译器运行后产生 depfile 不足以决定本次命中。driver 使用所选 C/C++ 编译器的预处理输出作为不可变编译输入：在原 Cone 的相对 include 布局中完成预处理，同时取得 depfile，记录源码、实际头文件和生效配置的内容摘要。完整预处理字节与依赖摘要共同进入 compile key；子编译器编译同一份 `.i`／`.ii`，不重新打开工作区头文件。行标记和内建文件名中的 host 路径映射为稳定的 Cone／公开头／系统目录相对名，诊断保留原始行号。发现准备期间的输入变化时丢弃该次结果并重新准备，不能以旧 key 发布新内容。系统头来自所选工具链/SDK，其实际内容由同一预处理输入和依赖摘要覆盖。未变化且已确认的快照和依赖结果直接复用，不增加跨 stage 的重复校验。

全局构建缓存由 scoop 管理。compile key 覆盖 manifest/single-file semantic projection、全部选中源码的规范化路径及内容、compiler/toolchain/protocol、language/schema/runtime ABI、target/backend、实际使用的 C bridge 配置、OptimizationMode 与依赖三层语义指纹。locator、输出路径、dump 请求和程序 argv 不改变语义 identity。源码变化可重建；无源码的 stale prebuilt 必须报错。

machine transport 为一次 request/response 的 length-prefixed canonical CBOR，protocol version 为 5，stdout 只含协议 frame。build request field 9 为 OptimizationMode（Debug=1、Release=2），field 10 为按归一化路径排序的选中 native 源码所对应的预处理输入 locator 数组，数量必须与 manifest 的实际 target 选择一致；没有 native 源码时为空。target request 保存 canonical triple、可选 C driver 与 native sysroot，host locator 不进入 artifact identity。旧或不匹配协议拒绝。

显式观察使用 `--emit ast|hir|mir|lir|all --dump-dir dir [--dump-scope root|sources]`，默认 root；sources 覆盖源码节点，不从 prebuilt 反造 AST 或 LocalConcreteHir。每个被观察节点在同次完整编译中生成 dump；HIR 同时包含 Export、LocalConcrete 与共有跨 Cone 接口。观察不改变 artifact，失败不返回成功产物；命中缓存也须为请求的源码观察产生本次 dump。

诊断具有明确 Error/Warning severity，primary 与 notes 保存实际 Cone、logical source 和 byte span。产物错误保存 container/manifest/member/section 与 wire field path，I/O 错误不伪装成源码位置。human 与逐行 JSON 使用同一结构化记录，写 stderr；stdout 留给被执行程序。缓存中的 warning 在本次显示时附加 locator，不丢失原位置。

工具中断停止后续构建，向当前 child 传递信号并回收进程，保留真实 signal。某节点失败后不启动 dependent 或 program-link。

### 2.8 Artifact-only program-link

program-link 消费完整 `.slib` Link 闭包、普通 runtime 对象、已有 native 文件、LIR target 与 final-link 配置。它不展开模板、重新编译 Scoop 或编译用户 C/C++；只可用明确选择的 C compiler 生成本次固定启动代码与最终 image tables。

root 必须是唯一 executable，依赖全为 library。闭包核对完整 identity、exact edges、三层 semantic fingerprints、单版本与兼容合同。多个 locator 指向同一完整 artifact 可合并，同 identity 不同内容或不可达额外 artifact 是错误。

program-link 从完整产物闭包合并 C++ 链接和运行库需求；任一 Cone 要求 C++，就采用所选 target 的 C++ final-link 配置。GNU 由配套 `g++` 驱动并使用 `libstdc++`，Darwin 由配套 `clang++` 驱动并使用 `libc++`；同时纳入各自的 C++ ABI 运行库与系统依赖。C++ driver 调用相应平台 linker，既有对象格式、GC、EH 与 image 契约继续适用。不能仅把 C 编译器替换为 C++ 编译器后继续原样使用抑制其默认运行库的 C 链接参数，也不能靠 root 重复声明 `cxx` 才满足依赖。

musl 闭包出现 C++ requirement 时，在解析 C++ 工具链或调用最终 linker 前报错，指出要求 C++ 的 Cone；不因其源码已预编译而绕过限制。实际使用的 C++ driver、标准库／ABI 运行库、unwind provider 及链接参数进入 final-link 配置和 ResolvedLinkPlan。

每个 LinkObject 按 canonical Cone/member 顺序消费一次，保留原 bytes 与检查结果。diagnostic、opaque、unknown optional 成员不是链接对象；无法处理的 Link-required capability 失败。后续不重新打开 locator 取得另一份输入。

Strong 由原 provider 提供。ODR 在普通依赖语义一致后按完整 group/member key 与 shared ABI 合并，物理选择见 2.13、2.19。全部 typed imports 和实际 relocation 必须闭合；metadata-only 查询可以没有机器引用。

SourceExtern 按实际 target/native symbol 合并完整 library、kind、TLS、mutability、ABI、calling convention、GC effect 与投影后的 native signature/storage。任一字段冲突均报错并指出来源；随后从实际 native 对象或 provider export 解析定义。`captureErrno`、包装后的 Scoop tuple 结果与 NativeSafe/GcLeaf 是声明的调用行为，不进入 native ABI 合并键；返回 `R` 与捕获后返回 `(R, Int)` 的声明可共用同一 native 定义，Scoop callable 和所需 bridge 仍按各自合同保留。core 与普通 Cone 使用同一规则。

普通 native 对象不含完整函数类型时，只核对实际符号、kind、范围、可写性和 relocation 等可观察事实，FFI 实现履行声明的责任不被伪造的类型证明替代。显式 library 的 symbol 必须由该库按平台规则提供，默认 namespace 的多 provider 歧义不能按搜索目录先后静默解决。

参数自由 source extern 的普通 managed 调用经原 provider 的 Scoop 薄入口，包含 native transition 与 caller-root publication，入口的 Scoop GC effect 为 Managed；native callee 自身的 effect 仍是其声明合同。泛型正文在定义方与消费方使用同一入口；NoGc 和 release 的直接 native 调用遵守各自规则。

runtime object index 是普通对象及定义／引用摘要，必须匹配 target、runtime ABI 和实际内容。runtime-build 的源码、头文件、compiler、SDK、flags 与规则变化使其缓存失效；该索引不要求 program-link 读取这些源码。

逻辑 native library requirement 沿完整依赖闭包传递，manifest 的 native.libraries 与非空 SourceExtern lib 使用同一解析流程。driver 将选中 manifest 要求合并到既有 LIR foundation native library 表；相同 key 去重，产物不要求每个库都有 Scoop extern 使用者。Code 与 production manifest 继续覆盖完整逻辑要求，Link reader 复用该表，不从机器符号反推库名。先在全部显式 library roots 中按既有规则收集候选；该层无候选时，再交给所选 target 的平台 provider 在其默认系统库目录中解析。显式候选损坏、target/ABI 不匹配或歧义直接报错，不能用系统库掩盖。TargetDefault 与显式 kind/grouping 保持原合同；OrderedGroup 只限制候选顺序，不引入 whole-archive、用户脚本或任意 linker 参数。

Cone native 源码对象使用 `org.scoop-lang.link-object/native/1` LinkObject 类别，logical key 为归一化 Cone 相对源码路径的 UTF-8 字节，member identity 仍包含所属 Cone。实际 Link fingerprint 按 member ID 排序加入既有 Code 对象投影；这些对象没有 Scoop Strong/ODR 或 generated-C recipe。program-link 从依赖闭包的对应成员读取普通 native 对象，每个成员作为独立输入参与符号解析；不同 Cone 的相同对象内容不能据此合并。对象格式、target、实际定义与引用沿普通 native 对象规则检查。

Linux 默认目录来自已选 native toolchain/sysroot 及其 target/link-mode 配置；Darwin 来自已选 SDK 的系统库与 framework 目录（包括 `.tbd` provider）。系统 provider 按对应平台的库选择规则解析动态/静态格式；静态模式不得以动态库满足需求。交叉编译不补入宿主的 `/usr/lib`、framework 或其他宿主工具链目录。无合法候选时报告所请求的库和 target；选定 provider 的实际内容、加载合同与工具链/SDK 配置进入既有 ResolvedLinkPlan 和链接缓存。只查已声明的逻辑库，不自动发现相邻 native source/object/archive，也不扩大空 lib 的默认 namespace。

Linux 系统库选择由所选 C driver/linker 按目标配置完成。发行版提供的标准 linker script（例如 GNU 的 `libm.so`）可解析为多个实际 archive/ELF 输入；其脚本内容摘要、实际输入及顺序进入同一解析结果和 link plan。最终链接消费已选文件快照，不重新解释系统脚本或按宿主目录重新选库。该规则不开放显式用户脚本输入。

重复声明已经包含在 target 固定系统输入中的同一 provider 时复用既有定义，不产生第二个符号候选；相同加载名但不同内容仍是冲突。Darwin SDK 的 `$ld$previous$` 指令按 platform 和 deployment 的半开版本区间选择符号的实际加载库及 compatibility version，未命中的指令不改变当前接口。此类信息属于目标 SDK 的正常动态库加载合同。

musl 将数学库、pthread 等接口合入 libc；所选工具链为这些固定系统库提供的空兼容 archive 按该平台规则绑定到现有 libc provider。此别名仅适用于实际系统解析选中的固定兼容文件，显式 roots 中的空 archive 不获得额外符号。

普通 archive 保留真实 member index、offset、length 与 digest，允许 BSD/GNU 命名；拒绝 thin、外部或嵌套 archive 和损坏边界。必要格式与定义索引覆盖候选，完整 relocation/EH/TLS/初始化检查只作用于实际选入的 member。实际 undefined references 驱动成员选择，新增引用继续闭合，每个物理 member 最多加入一次。

选入的 native 对象不能包含未声明的 constructor/destructor、动态 TLS initializer、bitcode/LTO、embedded linker actions、可执行栈或未声明运行库需求的 C++ EH 依赖。允许普通 C unwind 与静态／零初始化 TLS；已声明 C++ 模式的对象可使用相应运行库的 EH／RTTI 和标准 native 初始化／析构记录，由目标 CRT／C++ 运行库执行，不注册为 Scoop initialization unit 或 managed destructor。C++ 异常与初始化边界遵守运行时规范 5.5、第 7 章。普通 native 重复定义不使用 Scoop ODR 判等；同一符号空间内的强／弱绑定沿用目标对象格式规则，强定义覆盖同名弱定义，两个同名强定义报冲突。

非空 `@Extern(lib=L)` 在 Darwin 的每个显式 library root R 下，对 TargetDefault 检查 `R/L.o`、`R/libL.a`、`R/libL.dylib`、`R/libL.tbd` 与 `R/L.framework/L`；不去掉已有 lib 前缀或扩展名，不递归扫描。显式 kind 只收窄对应格式；相同内容及加载合同的重复候选可合并，不同候选为歧义。空 lib 只查询已引入的 runtime/native 对象和动态 exports，不扫描目录搜索任意库。

Darwin 支持 `.o`、`.a`、dylib/framework 与所选 SDK 的系统 providers。普通 load/re-export、真实 install name、target/deployment、symbol kind 和绑定关系参与解析；显式库不能被另一库的同名 export 替代。`@rpath` 从明确目录解析，实际 LC_RPATH 进入输出和 link plan；依赖中的 `@loader_path` 相对 provider。需要部署布局才能解释的直接 `@loader_path`、`@executable_path` 或未知形式拒绝。

Darwin 使用所选系统 linker、按 native 需求选择的 C／C++ 链接 driver、SDK/deployment 与 libSystem，Level I unwind 经系统正常导出解析；C++ 模式同时使用配套 libc++ 与 C++ ABI 运行库。final-link 不使用 dead_strip、不同 body 的 function ICF、LTO、未声明 autolink 或 raw 用户 linker options。stackmap relocation 后位于只读 `__DATA_CONST`，采用传统 rebase/bind（no_fixup_chains）格式；普通绑定与随后执行的 weak coalescing 分别解释，允许同一指针对相同符号和 addend 具有这两阶段记录。实际需要的 ad-hoc signing 属于平台装载格式。

Linux 接受 `.o`、`.a`，动态模式另接受 ELF `.so`；framework 与静态程序的动态库需求为错误。用户 `.so` 必须是 DSO，不是 PIE executable 或 linker script。未版本化的源码 extern 只匹配默认导出版本，DT_NEEDED 不等于显式库 re-export；TLS、IFUNC、版本化导入按实际 kind/version 检查。

Linux gnu 默认 dynamic PIE，musl 默认 static ET_EXEC；musl 可显式 dynamic PIE，不支持 glibc static。非 C++ 链接明确选择 CRT、LLVM unwind、libc、libm/线程及算术 builtins；GNU C++ 链接使用配套 libstdc++／C++ ABI 运行库，并统一由配套 libgcc_s 提供 Scoop Level I 与 C++ 所需的 unwind，不再额外链接另一套 LLVM unwind provider。musl 不支持 C++ 模式。动态 metadata/stackmap 满足实际 RELRO，静态 musl 的不可变记录位于只读 PT_LOAD；最终文件不得有 text relocation。

DSO 依赖查找使用显式 roots、provider 目录、实际 RPATH/RUNPATH 与所选系统目录。真实 SONAME、DT_NEEDED、加载顺序和 RUNPATH 进入 link plan，临时快照路径不写入 executable 的依赖名；同名导出使实际 ELF 顺序无法满足显式库绑定时报告冲突。

链接生成 `int main(int argc, char **argv)`、最终 image descriptors、六张 producer pointer tables、image pointer array 与唯一 root entry 引用。C main 按运行时规范 2.8 调用 `scoop_rt_run_program(images, image_count, root_entry, (int32_t)argc, (const char *const *)argv)`，原样返回其 C int 结果，不丢弃 argv[0] 或主动截断退出码。这些生成物只引用所选记录，不复制 TD、storage、cell 或 gateway body，也不在 C main 中构造 managed Array/String。`scoop_td_String` 是实际 String TD 的同地址 alias。

ResolvedLinkPlan 使用 `scoop-resolved-link-plan-v3`，覆盖 root/依赖、Code 与对象内容、runtime/native/startup 输入、ODR 选择、最终 image 内容、系统 providers、库绑定、archive members、加载路径、工具配置和有序操作。物化与输出 locator 不进入内容身份，实际 install name/runpath 必须进入。

最终检查核对本次链接产生的 target/entry、实际输入、symbol/binding、Strong/ODR 地址、String alias、startup/image 引用、stackmap/EH 保留与装载权限。native linker 不能重新选择另一份 ODR 实现；落选 bytes 不能用于解释最终文件。已在对象边界确认的语言、ABI 和不变内容不重复完整重放，也不运行用户程序证明链接成功。

全部检查通过后原子发布 executable，失败保留旧文件。

### 2.9 虚方法与接口分派

每个 slot 有原根声明、完整签名、访问域和 Abstract/Concrete/InterfaceDefault 的明确选择。class vtable 保留 base prefix，override 不改变 family 位置；Any 不预留 equals/hash/toString 槽。

interface 槽序按直接父接口声明序继承，再追加当前声明；相同 exact application 的菱形路径去重，移除被原 typed override 覆盖的成员，保留其余相对顺序。getter/setter 独立，private helper 不占槽。实际类／值类型表项匹配原 slot 的参数、结果、execution 与 GC effect。

itable 以实际 interface TD 为键，不要求跨 Cone 的全局槽编号。generic receiver 始终是完整 exact application，只沿声明中的 exact base/interface 关系分派。

普通接口值保存 object 和对应 exact interface 的 itab。构造／转换时查询目标表一次，已知实际类型直接引用既有静态表，相同 exact 视图直接复制；`is`／`as` 的检查与表查询可合并，不重查同一事实。空接口以 entry 存在判断成功，不用合法 null slots 判失败。接口方法从 itab 直接取 slot，不再次查询 object TD。

LIR 的 dispatch slot 显式保存单字 object receiver projection 和完整入口签名，普通接口参数／结果仍为双字。兼容 class 实现可直接作为 slot；需要完整接口 this 的 default／变型／装箱入口由已有 typed adapter 重建视图或复制 payload。去虚拟化与直接调用按实际 callee 的签名适配，不能只因都含 pointer 就混同 ABI。静态表和 adapter 继续使用正常 typed definition／reference、provider 和 ODR relocation。

值类型只有显式声明或继承的普通 interface conformance；有界泛型调用、interface 调用、装箱与 `is` / `as` 消费同一实际接口闭包。结构 operator equals 不产生 Equality conformance。显式 `Equality<T>.equalTo` 与其他接口方法使用相同的调用规则；已知 concrete value 的直接调用不因另有接口实现而强制装箱或运行期查表。

按语言规范 13.2，NoGc value method 可以实现 Managed interface slot。源级实现兼容性允许这种 effect 收紧，但实际 itable entry 仍须符合 slot 的完整 Managed ABI：由普通 value/interface adapter 完成 receiver 适配，并以 NoGc 合同调用实际实现；adapter 保留自身的 Managed effect、入口 poll 与必要 roots。具体类型直接调用原 NoGc 方法不经过该 adapter。签名与产物分别记录 slot、adapter 和实现，不能用强制转换或抹去 GC effect 代替适配；不引入新的 runtime 分派机制。

ExportHir 的 interface slot 合同允许目标实现的 NoGc effect 收紧 Managed 声明，并分别保存二者的原始 effect；class vtable override 不采用这项适配规则。值类型与 GC-free 限制由 HIR 的普通声明和 effect 检查完成，产物只检查对应的签名兼容关系，不重新执行源码语义检查。

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
| atomic operation | 实际 AtomicInt/Long/Boolean/Ref application、值类型、操作种类及完整成功/失败内存序；遵守语言规范 11.14。 |
| MaybeUninit operation | 独立 wrapper 与 payload 的完整类型，以及零构造、合法值包装或 unsafe 取值种类；遵守语言规范 11.15，不保存初始化证明或隐藏标记。 |
| managed callback | native/managed 签名、context index、mode、原 registration 与实际 application。 |

每个 intrinsic 只承担其指定语义，展开后是普通 typed 运算、内存访问或有明确 runtime 合同的操作。Const 能力只来自声明允许的操作集合，不能由同名用户函数取得。

原子类型使用真实 core 声明、封闭 intrinsic kind 与目标布局，值字段不作为源码可访问的普通字段。构造沿普通 nominal constructor 参数推导形成 `AtomicNew`，保存实际 application 和初值，不伪造源码函数身份。AtomicRef 的 HIR／MIR 表示及跨 Cone 元数据保留实际引用参数 T；四种对象的目标布局由 object header 和自然对齐的单个隐藏值槽组成，初始化在分配后、发布前以 Relaxed store 写入。HIR 检查内存序是编译期常量且组合合法；MIR 保存已确定的操作和内存序，LIR 使用 AtomicLoad/AtomicStore/AtomicRmw/AtomicCmpXchg，携带对象 base、值字段偏移与实际值类型。codegen 降低为对应 LLVM 原子指令，Boolean 使用 i8，整数 fetchAdd/fetchSub 不添加溢出假设；CAS 使用 strong，AtomicRef 按对象身份比较。五种内存序、CAS 的成功/失败区别及其语义进入普通 IR/metadata 和指纹，跨 Cone 与优化不能退化为普通内存访问。

load、store 与 CAS 分别保存对应操作的合法内存序；CAS 是完整的成功／失败序组合，不能只保存成功序后由后端推断失败序。源码原子值种类封闭为 Int、Long、Boolean、managed reference；既有协程等编译器内部状态字继续使用 MachineAtomicLoad／MachineAtomicStore／MachineAtomicCompareExchange 及原来的固定同步序，不以源码数值类型替代其 machine scalar domain。

每个原子方法的 intrinsic 身份同时包含实际原子族与操作，不能把不同 owner 的源码声明合并为同一函数。HIR 在普通参数推导及默认值物化后，将实际 MemoryOrder variant 转为封闭的操作内存序；只追踪本次调用生成的实参临时值，不把源码局部变量折叠成内存序常量。HIR、默认参数／泛型模板与 MIR 使用完整的原子表达式，分别保存 receiver、所需的值实参及已确定的内存序；后续 stage 不重新解析枚举名或补全缺失的 CAS 失败序。

AtomicRef 的 base、expected/new 引用与读取结果沿用 managed pointer 和 statepoint/relocation 契约。写入后的卡表屏障及无 safepoint 区间遵守运行时规范 3.6；NoGc 或普通优化不能消除必要的原子语义、同步顺序或写屏障。LLVM 22.1 与 RewriteStatepointsForGC 的具体组合按 M33-8 验证，最小 IR 与实际 GC 运行分别验收；遇到后端限制须保持语言原子性与 GC 契约调整实现，不能用普通 load/store 代替。

AtomicRef<I> 的隐藏槽保持一个 AS1 object；store／CAS 投影 object，load／exchange／compareAndExchange 在 NoGC 区间内查询静态 I 的表并重建双字返回值。槽布局不扩大为接口宽度，不用两个原子访问模拟一个逻辑值。

MaybeUninit 保留独立的 intrinsic nominal/application 表示，MIR／LIR 操作不提前擦除其有效值规则。layout／scan 递归复用 T，GC-free 条件随实际 payload 传播；Option 分类不继承 wrapper payload 的 niche。codegen 的 zero、wrap、assumeInit 为有类型的零化或完整复制，含引用值沿普通 roots／写屏障处理。unsafe 调用在 HIR 按既有 context 检查，不在后端增加初始化验证框架。

HIR 在普通调用解析和 unsafe 检查后把三个操作归一化为有类型的 wrapper 操作；类型限定的 companion 仅用于解析，不触发 singleton 初始化。显式 receiver 表达式仍按普通规则求值。默认实参模板保存同一操作（expression tag 73，操作 tag 1／2／3），不把 wrapper 零值重写为合法 T 的常量。

安全操作的方法引用在普通 closure adapter 中展开为同一 wrapper 操作，bound receiver 按引用创建规则只求值一次；不要求 intrinsic 具有独立的普通函数 body。assumeInit 的方法引用仍按普通函数类型的既有 safety 规则拒绝。

LIR 的 MaybeUninit 操作写入完整类型的独立 local place；零大小结果仍保留逻辑 place。包装与取值在该 place 中完成全部字节的复制，普通 aggregate local 间的复制也保留 padding，不能经只含源码字段的 SSA 聚合丢失存储字节。临时 place 由既有局部存储及 roots 规则管理，LLVM 可在不改变这些可观察结果时消除它。

该类型的源码 intrinsic 名为 `core_maybe_uninit`；三个操作分别为 `maybe_uninit_zero`、`maybe_uninit_initialized`、`maybe_uninit_assume_init`。前两者属于该泛型类型的 companion，后者是实例方法，均没有独立的 callable 类型参数。它们的源码 GC effect 为 NoGc，assumeInit 另为 Unsafe；普通 NoGC 使用点仍检查实际 wrapper／payload 是否 GC-free。源码名只在 HIR 声明边界解析，后续使用实际 nominal/application 和操作种类。nominal intrinsic wire sum 增加 tag 14，callable intrinsic sum 增加 tag 25（操作 tag 1／2／3 依次对应上述三个操作）；MIR application 与 LIR exact value layout 都保存 payload 的完整 exact identity，wrapper 的 layout constituent 没有 null niche 或 C storage 资格。

上述扩展对应 HIR interface 69／type semantics 27、MIR type bridge 19、LIR layout ABI 15／layout link closure 8。MIR representation 的 tag 13 保存 payload exact type；LIR value representation 的 tag 9 保存 payload value layout 引用，由该 layout 得到 exact type、size、alignment 和 scan，不伪造源码字段。未改变编码的 section 保留原版本，旧 section 产物重建。

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

write/writeError 通过普通 String 作用域借用调用 C ABI 字节输出后备，flushOutput 通过普通 C ABI 调用刷新 stdout；编译器复用 NativeSafe 调用及 pin 帧，不为输出名称增加 intrinsic 或专用状态切换。print/println/eprint/eprintln 的 ToString bound、求值次序和跨产物泛型展开均遵守普通函数规则（语言规范 14.4）。

exit 是返回实际 Nothing 的普通 core Scoop ABI extern；调用及产物保留底类型、既有返回 carrier 和无正常后继的语义，不伪装成 Unit 或添加不可达占位正文。runtime 的单向终止和刷新合同见运行时规范第 7 章。

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

runtime metadata ABI 8、runtime ABI contract 12 的字段与启动协议由运行时规范 2.7、2.8 定义。RootEntry 的物理大小仍为 192 bytes，旧版 ABI 不可混用。每个 `.slib` 保存完整候选 registration 与物理归属；最终 image 表只包含选中的 producer records。

image、root entry、main body、root gateway、initializer、ensure、startup gateway 和 release hook 使用各自身份。根、cell、storage 和失败状态完整关联，不以可空函数指针或命名约定推断 unit kind。

全部 image 和 roots 在首个 managed initializer 前登记，原始 argc/argv 在 eager 初始化前保存。每次 eager/root gateway 有独立 native→managed boundary 和入口 poll；全部 eager 成功后 root gateway 才构造其需要的 argv 数组并调用 main，所有异常在回到 C 前处理完成。Unit 成功写退出码 0，Int 成功写完整实际值，两者均返回 status 0；failure root 发布后返回 status 1，runtime 忽略退出码槽并按统一失败路径以 1 终止。构造、static initial state、failure root 与 Context cells 遵守运行时规范，不由最终 linker 补造缺失语义。

初始化顺序遵守语言规范 12.3；普通 lazy ensure 可以因实际访问执行。最终链接保证同一 ODR unit 共享唯一状态，runtime 按实际加载地址和当前状态协调。

### 2.15 集合、Char 与字符串

List、MutableList、ArrayList、StringBuilder 及迭代器是普通 core 类型；数组和 String 的属性/成员也从真实声明选择，不能按 `.size`、`.length` 等拼写跳过访问与类型检查。

ArrayGenerate 保存完整目标 application、元素类型、Long count 与 ordinary `(Long) -> T` initializer。负长度、求值顺序、逐索引初始化和异常遵守语言规范 10.6；ZST 只省略 payload，不省略 initializer 调用。生成控制流在任意 GC/异常点保持数组可扫描。

Char 为独立 nominal identity，常量和各层表示保存合法 Unicode scalar，物理布局为 GC-free u32。它不与 UInt 合并，Option、数组、装箱与 ABI 使用实际 Char 类型；不因表示相同增加整数算术或 CharRange。

String 的源码索引和 length 按 Unicode scalar，byteLength 使用物理 UTF-8 byte count。定位后备的 Option 结果、实际 typed ABI 与异常边界见运行时规范第 6 章；List getter 仍为可抛出的普通 managed 调用。

严格、可空和 lossy UTF-8 转换按语言规范 11.4 的实际 core 声明调用 runtime 后备。CharacterCodingException 是普通 core 异常，成功/失败结果通过完整 typed Scoop ABI 传递；不按方法短名增加编译器特判或新的异常角色。lossy 的 maximal subpart 与严格路径的 byteOffset 使用同一解码规则，生成代码不得用截断、locale 转码或忽略非法字节替代。字节借用、结果 String 与跨分配保存的 managed 输入遵守普通 roots、pin 与 relocation 合同。

C 字符串接口由普通 core 代码组合：withCString 检查 NUL，复制到带终止符的 MutableArray<UInt8>，再通过语言规范 13.11 借用；fromCString 通过普通 C ABI 的 strlen 取得长度，复用严格 pointer 解码。strlen 按 GCLeaf 合同调用；没有独立的编译器 C 字符串类型、隐式编码转换或栈缓冲优化。

编译器为异常边生成的零参数构造适配器不参加源码构造重载选择；core 自身的函数体也遵守同一规则。源码调用仍选择实际 source constructor，并按普通规则求值缺省实参，泛型／默认模板保存该源码调用。适配器继续由编译器异常边的完整 typed 引用使用。
f-string 按源码顺序保存 text/expression part 与原位置，绑定实际 core StringBuilder 的构造、add 与 build。表达式与对应 toString 交错执行，保持异常和挂起行为；成功 HIR 正文不留待后端解释的字符串插值或 StringBuilder 指令。

普通容器布局和内部存储策略不属于编译器协议。其模板、接口调用、快照和字符串构建使用普通类型/ABI/GC 合同。

ArrayList 以普通 MutableArray<MaybeUninit<T>> 实现容量存储，elementCount 与扩容、插删、清零均为 core 正文；编译器不按 ArrayList 名称提供特殊 shape、分配或 initialized-length 字段。StringBuilder 继续持有 ArrayList<String>，只把拼接后备的参数及元素描述适配为实际 wrapper 类型；沿用完整容量 scan。

### 2.16 Task-local Context

Context 的语言行为见语言规范 8.3.5，运行时 key、scope、task 与 callback ABI 见运行时规范第 9 章。HIR 保存 exact key、完整绑定值类型及已选 Context 操作；泛型条件在实际 application 中替换，不按运行期对象类型猜 key。

Context runtime 的擦除 binding 槽保存单字 object；compiler-owned `context-binding-ref` 使用与 Any 相同的 managed-pointer 表示及独立 generated identity，不冒充源码空接口。绑定接口时提取 object，按 exact key 取出后为静态接口重建 itab；该重建为 NoGC。用户可见的参数、结果和字段仍保存完整双字接口。

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

Linux/amd64 的 managed 调用保留固定的 outgoing argument area，使运行时规范 3.2 的 `FP = SP + N - 8` 对每个 safepoint 都成立。LLVM 22.1 后端关闭 X86 call-frame size optimization，避免按值参数被改成调用点临时 push/pop；源码参数语义、calling convention 和其余 O2 优化保持。此设置在创建任何 target machine 前由 codegen 统一初始化。

普通优化可以消除已证明不可达的代码及 site，但保留实际求值、异常、Context、initialization、release-ready 与严格浮点语义。类型错误不能因优化删除代码而消失。

MIR 内联按调用点代入已知实参、局部常量和可证明不可达分支后的加权成本选择：极小正文积极内联，普通小正文结合循环位置、常量／receiver 机会和累计增长选择，较大正文保留调用，除非化简后进入前两档。标量运算、字段访问、aggregate copy、分配、调用和复杂控制流具有不同成本；实参副作用始终求值一次。稳定顺序、caller 累计增长及嵌套展开限制只决定优化选择，不决定程序合法性或 ABI。实际参数由性能记录公开，不增加通用跨阶段预算或源码属性。

条件 poll 在 LIR 保存慢路径 site 与完整 live set，仅实际慢路径产生 statepoint／stackmap；合流连接更新引用，NoGC 不新增 poll。内联、去虚拟化及 CFG 改写均先于最终 roots 定稿，后端不靠完整 runtime poll 调用维持普通循环的检查。

依据语言规范 1.2，编译器可以假定普通非原子访问不存在数据竞争，不为竞争场景提供单字或逐字段不撕裂的保证。普通字段、数组元素、全局存储和聚合值复制无需为此改用 LLVM `unordered` 原子 load/store、增加发布屏障或限制访问的机器指令粒度；显式同步原语以及 runtime/GC 内部同步的契约仍须保持。

最终 GC 计划从实际保留的 callable/site 和完整逻辑 managed leaves 得到。多个逻辑 leaf 可以对应同一 SSA 值，已知 null/不可移动常量不要求 relocation pair；runtime root count 使用实际 pair 数。每个保留 site 具有唯一 owner/role，site identity 不因删除其他 site 而重编号。

GC 转换后每个 post-site managed use 必须来自正确更新的引用；invoke 的 normal/unwind 与 native transition 继续使用各自显式 root frames。不能在 root plan 定稿后增加未经描述的 safepoint、重复 site、外来 owner 或异常边。backend contract field 28 为 1，表示此最终根与发射合同。

ODR 只比较 key 与 shared ABI，按实际需要选择物理 member。body 的机器码、EH、stackmap、callable/safepoint registration、关联常量与 Context cells 必须同选；独立 helper 可分别选择。shared TD、storage、cell 与初始化保持唯一，native linker 不得改变已确定选择。

最终 image descriptors 和六张表由 program-link 从所选 records 生成，不改写输入 `.slib`。选择与实际内容进入 RuntimeImage、Code、Artifact 和 ResolvedLinkPlan 的对应摘要。

OptimizationMode 在 machine request field 9、production manifest field 12 中编码 Debug=1、Release=2；compile cache domain 为 `scoop-cone-compile-cache-v2`，mode 位于 field 13。generated-C flag 的 SelectedOptimization tag 为 15，tag 6 保留不用。

digest kinds 4、8、9，digest owner tags 4、11、9，以及 RegistrationDefinition patch role 1 保留不用。registration 公共 identity 只含 linkage、semantic ID、ODR group/member；body、layout、descriptor、scan 与 normalized-stackmap fingerprints 仍用于所选实现自身的内容、引用及 GC 合同，不作为不同优化正文必须相等的条件。

# Scoop 编译器与产物规范

本文规定编译阶段的输入输出、实体身份、目标 ABI、`.slib` 格式、构建命令、缓存与链接契约。源码语义见 [语言规范](SCOOP-SPEC.md)，对象表示、GC 与 native 运行边界见 [运行时规范](SCOOP-RUNTIME-SPEC.md)。

## 1. 范围与目标兼容性

一个 Cone 是一次独立编译的单位。library 和 executable Cone 都产生完整 `.slib`；executable 的最终程序由独立链接阶段从产物闭包生成。产物是编译结果，不承担独立分发认证或源码／工具移除后的可用性保证。缓存只判断正常输入是否变化，不检查输出是否遭到外部破坏。

编译器使用自身及所选 C/C++ 工具链成功产生的输出，后续步骤只读取实际需要的数据，不重新证明生产过程。工作文件按普通生命周期管理，不设为只读、不审计权限或文件来源。实际 I/O、编译和链接错误正常报告；失败不发布部分结果。实体身份、ODR、ABI、GC 与读取边界所需的数据继续保留，不以验证资格或生产凭证替代这些普通合同。

编译请求明确选择相容的 LIR target、backend、C bridge toolchain、runtime build 与 final-link 配置。目标目录由 driver 相关代码维护，typed target/backend/toolchain identity 只规定规范化名称与版本的格式；格式合法不代表相应消费者支持。native 符号的对象前缀由选中目标的数据显式传入；identity 解码只检查身份和符号字节格式，目标消费边界按源码符号重建并核对归一化结果。LLVM/backend 和具体 ABI 实现由 codegen 选择，对象读取与最终链接由 slib/linker 的有限支持集合决定，未知或不相容组合必须明确诊断，不回退到宿主目标。阶段只消费其所需投影；library 编译不要求未使用的最终 linker、CRT 或 unwind archive，链接只执行其实际需要的工具，不通过移除源码编译器或 LLVM 工具证明隔离能力。

| canonical target triple | LIR target profile | 对象格式 | 平台契约 |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | `org.scoop-lang.target-profile/darwin-aarch64/1` | Mach-O relocatable | Darwin/AArch64 |
| `x86_64-unknown-linux-gnu` | `org.scoop-lang.target-profile/linux-x86-64-gnu/1` | ELF64 relocatable | Linux/amd64、glibc |
| `x86_64-unknown-linux-musl` | `org.scoop-lang.target-profile/linux-x86-64-musl/1` | ELF64 relocatable | Linux/amd64、musl |

这些目标采用 little-endian、64-bit data/code pointer 与 16-byte 调用栈对齐。合法非 null pointer 可经内部 64-bit carrier 逐 bit 往返，null carrier 全零。data、code、managed 与 metadata pointer 的语义仍不同，不因此增加源码整数转换。

backend contract 使用 LLVM 22.1、LLVM stackmap v3 和本规范的 GC/EH 约束。Linux 两个 libc target 共用 `org.scoop-lang.backend-profile/llvm-22-1-linux-x86-64/1`，但 native、runtime 与最终链接输入不能混用 glibc/musl。target、对象格式、调用约定、pointer/storage ABI 及实际 producer 配置必须相容；不支持的组合在生成目标产物前诊断。

Linux GNU 直接执行所选 GCC driver，不注入 musl wrapper 专用的 `REALGCC`；musl 将显式 `REALGCC` 或 PATH 中的 gcc 作为 wrapper 的实际 compiler。两者均通过真实 header/object probe 检查 libc 与机器，误选另一 libc 的 driver 必须得到不匹配诊断。

Native C++ 支持由语言规范 12.2.1 的 `native.cxx` 显式选择，仅支持 Darwin/AArch64 与 Linux/amd64 GNU。当前 musl 工具链未配套 C++ 标准库和 ABI 运行库，C++ 与 musl 的静态／动态链接组合均不支持；源码构建和 artifact-only 消费都必须诊断该组合。

## 2. 编译阶段与产物契约

| 阶段 | 输入 | 输出与责任边界 |
| --- | --- | --- |
| parser | 当前 Cone 的完整源码集合 | 带准确源码位置的 AST 与语法诊断。 |
| HIR | 当前 AST 与依赖 Export HIR | 完成源码语义检查，分别输出 ExportHir 和 LocalConcreteHir。 |
| MIR | LocalConcreteHir 与实际选用的依赖 MIR metadata | 完整 concrete 类型、callable、显式控制流及执行 ABI。 |
| LIR 准备 | MIR、实际选用的依赖 LIR metadata 与 codegen 提供的目标布局描述 | 完整布局与逻辑签名输入；复用依赖已经保存的物理 ABI。 |
| ABI 规划 | LIR 定义的布局／签名输入与所选 codegen 目标 | 按完整签名分类的 Scoop/C ABI 计划集合。 |
| LIR 完成 | 准备结果与完整 ABI 计划 | 完整正文、存储、GC roots、符号与物理定义要求。 |
| codegen 发射 | 当前完整 LIR 与相应 producer 配置 | 目标对象、generated-C 源码、实际定义／引用与 GC/EH 数据。 |
| `.slib` | 三层 metadata 与完整对象集合 | 按需消费的确定性编译归档。 |
| program-link | `.slib` 依赖闭包、runtime/native 对象、final-link 配置 | 定义与引用闭合的 executable。 |

stage 只通过对应 IR/metadata 交换数据，不调用上游 stage 的实现。所有源码语义错误在 parser/HIR 结束；后续阶段报告自身的格式、引用、ABI、目标或链接错误。已验证且不变的数据在后续边界复用。

### 2.1 Parser / AST

源码集合非空，文件具有唯一 `SourceIdentity { cone, logical_path }`。manifest Cone 的 logical path 为完整的标准化 Cone-relative source path；默认目录中的文件保持 `src/...`，显式源码根不截短路径或改变相同文件的身份。single-file 为 `main.scoop`。host 路径只用于本次 I/O 与诊断。parser 只接收按语言规范 12.2.2 及本规范 2.7 选定的完整源码集合，不自行补扫目录。

AST 保留原始源码顺序、byte span、显式类型实参、位置／命名／spread／尾随 lambda 实参，以及独立的声明、表达式、模式和控制转移结构。文件至多有一个开头 package，其后为 import，再后为声明。qualified path 为非空 identifier sequence；package、静态 owner 与实体的分界由名称解析决定。

AST 不包含依赖 locator 解析、构建顺序、已推断类型或实体身份猜测。语法恢复产生 Error；任何解析失败阻止成功 HIR 输出。

lexer 使用 Unicode XID 属性库识别普通标识符，并保留反引号 token 的语法类别；AST 名称保存去掉引号后的实际文本。持久化 canonical identifier 接受任意合法的非空标识符文本，包括必须以反引号书写的名称，不重跑普通 token 的 XID 规则。package、注解和成员路径保存完整分段及实际 owner，不能按显示名称中的点号重新拆分。名称的 wire 编码继续为 UTF-8 text，已有合法记录的编码与实体身份不变。

循环标签只保留在 AST 和 HIR lowering 的当前 callable 词法环境中；HIR 的 break/continue 总是引用全局唯一的 LoopId。表达式跳转沿用现有语句 setup 与 Nothing 结果，不产生伪造的正常值。默认参数和泛型 body 的持久化形式以类型化的包围循环深度选择目标，0 表示最内层；标签文本不进入后续 stage。break/continue 的既有单字段 tag 10/11 编码保持为深度 0，非零深度使用同 tag 的第二字段 1（u32）；显式写出的零深度不是 canonical 编码。reader 检查深度确实指向该 body 当前 callable 的包围循环，实例化为新的 LoopId 后直接复用该结果。MIR cleanup 按目标 LoopId 选择已建立的循环入口和出口，不再要求跳转总是指向最内层循环。

do/while 与 while 共用有明确 LoopKind 的结构化循环记录；condition setup、条件、body 均完整保存。AST/HIR lowering 在同一 body 词法作用域内依次检查 do body 和 condition，并按有效 continue 出口检查 condition 的读取。持久化普通 while 继续使用 tag 8，do/while 使用 tag 9，字段与 while 相同；读取时按 LoopKind 的实际执行顺序检查局部定义。MIR 为 do body、条件和出口建立独立 CFG block，初次进入 body，continue 指向条件入口，条件为真回到 body；条件入口保留已有 loop-header GC poll 与挂起恢复契约。

局部 return 标签保留于 AST；parser 为直接具名调用的 lambda 实参补充隐式标签，显式标签优先。HIR lowering 在当前 callable 的词法边界解析标签，并复用已选返回类型与既有 Return / Nothing setup。lambda 无期望结果类型时合并自身显式返回与正常末尾结果，进入嵌套 callable 时保存并恢复独立推导状态；成功 HIR 与持久化 body 不保留标签文本，也不增加跨 callable 转移。

具名表达式体和非局部 property 的省略类型仅存在于 AST 与 hir-lower 内部声明查询。先建立 typed 声明身份和显式参数，再按表达式检查实际访问的声明完成结果类型；查询中的活动依赖用于诊断推导环，不反复猜测类型。重载先按参数选择目标，只有需要结果约束或提交获选调用时才查询声明结果，不能把推导环当作改选其他重载的理由。继承目标、分派槽和默认参数来源按已知签名部分确定，仅将依赖推导结果的类型相等约束延后；结果完成后检查这些约束，不重放继承与布局验证。表达式类型检查、重载选择及适配复用普通 lowering，推导的完整 body 可供后续编排复用。成功 HIR、共享签名及其持久化编码只保存完整结果类型，沿用原有签名/接口依赖失效及访问域检查；MIR/codegen 不重新推导声明。

控制结构的块体和单语句体使用同一 AST block 载体；单语句保留原始 span，并同样在 HIR 建立独立作用域。else-if 保存为 else block 中的嵌套 if，后续阶段沿用已有值合流、终止路径和循环 lowering，不增加平行表示。

parser 中仅有存在性与位置的修饰符使用可选 span；带取值的修饰符同时保存取值和 span，不独立维护 bool 与可缺失位置。AST literal pattern 使用封闭的字面量数据，保留原数字拼写、允许的数值符号和准确 span，不接受任意表达式作为字面量模式。

语言规范 8.4 的五个兼容修饰符在合法语法位置消费后丢弃。parser 可以保留诊断所需位置；HIR、MIR、LIR、共享签名与 ABI 不保存对应语义标志，codegen 不生成内联或尾调用属性。源码位置与哈希仍反映真实文本，不要求去掉标记前后的产物逐字节相同。

when 的 AST 明确区分无 subject、表达式 subject 与 val subject 声明；声明保存完整 binding pattern、可选完整类型及初始化表达式。arm 区分普通条件序列、case 模式与后备条件，保留 guard 和各条件位置。return/throw 可处于表达式位置；调用保留尾随实参来源及分组边界；annotation 数组保留每个元素的位置。

### 2.2 HIR

HIR 完成名称解析、类型检查、重载与泛型实参选择、可见性、继承与覆写、模式穷尽性、构造／初始化规则、常量、效果及 FFI 声明检查。源码与依赖声明遵守同一语言规则；文件发现顺序、声明存储位置和 provider 读取顺序不能改变结果。

smart cast 分析使用已有 BindingId、类型约束及带实际 LoopId 的控制结果，在当前 callable 内传播正常出口事实。多个正常前驱取共同事实，局部写入撤销旧值事实，finally 按实际进入和离开结果合并；循环按条件、body 与回边求稳定结果，并确认被使用的类型视图在对应路径成立。when 和短路 lowering 保存的临时值按实际求值顺序参与分析；不可变捕获值和构造器参数按原 BindingId 关联快照，快照不会因源变量后续写入而改变。完成的 HIR 仅保留普通 typed 读取、必要拆箱与既有控制流，不向 MIR、共享元数据或 runtime 添加流分析状态，也不在后续 stage 重放该分析。

函数 smart cast 的读取视图与 binding 的声明存储类型分别处理。显式类型检查、转换及引用相等读取原存储值；不需要函数适配的期望类型继续使用声明视图。产生适配闭包的读取不建立来源与新值的快照等价关系。MIR 为动态函数适配器保存来源时，先使用既有引用转换将来源适配到捕获字段的 Any 类型；typed function 来源也必须满足同一物理字段契约。

重载探测共享已经检查的声明、签名和正文；候选新增或修改的函数、局部值与词法绑定各自隔离，只有选中候选的结果进入当前状态。一次探测不复制此前所有函数正文或当前函数的全部局部值。内部共享不改变实体分配顺序、诊断与重载规则，输出仍为原有完整 HIR 数据。

候选局部约束求解可以先把已确定的参数、可选 receiver 与 suspend effect 传给 lambda，再用正文推断尚未确定的返回类型。这个部分期望仅存在于 lowering 的推断上下文，不是缺少返回类型的 HIR 函数类型；提交 lambda 时其参数、返回值、捕获和正文仍全部确定。当前 Cone 与依赖模板调用使用同一推断路径，不按扩展名或库名特判。

`SemanticHir` 保存已检查的声明与正文，对外有两个不同的输出合同：

- **ExportHir**：跨 Cone 的声明与必要正文。区分 public/re-export lookup、protected inheritance/slot、generic hidden support 和 interface dependency；支持声明的存在不扩大可见性。保存签名、默认值、const、alias、注解、类型关系与模板，普通非泛型函数的实现正文不复制给下游。
- **LocalConcreteHir**：本 Cone 实际需要发射的完整实体与正文。全部类型参数已替换，所有调用、构造、字段、variant、slot 与 external target 已确定；不含未完成推断、待实例化请求或上游普通函数正文。

两者使用不同的实体 ID 家族和显式映射。导出声明、完整泛型 application 与本地 concrete 实体不能用同一 arena index 或别名混用。

executable 的入口发现只检查 root Cone，并按语言规范 12.4.4 一次确定 `NoArgsUnit`、`ArgvUnit`、`NoArgsInt`、`ArgvInt` 四种形态之一。LocalConcreteHir 保存实际 main 的 typed identity、完整源码签名和入口形态；有参数时同时解析 argv 构造所需的实际 core `Array<String>`、String 及 helper targets。argv 构造使用普通 internal core 函数 `__scoopProgramArguments(): Array<String>`，通过现有 compiler protocol 记录实际函数 identity；其正文读取 runtime 已保存的 raw argv，严格解码、复制并报告非法参数下标。raw argv 访问器的 Scoop 返回类型为 `Ptr<Int8>?`，保留越界返回 null 的契约；helper 在 argc 范围内访问并按普通 Option 规则取出指针。后续 stage 使用该 typed 引用，不按名称重新查找。缺少入口、多个入口或非法签名在 HIR 报错；不能留给 linker 通过函数名挑选或修补。

每个 expression 具有非可选类型和 definition/evaluation origin。已解析调用保存完整逻辑签名、适配前 receiver 类型、实参映射、唯一 typed target 与效果；receiver 和实参的执行顺序遵守语言规范 8.5，不由类型求解顺序决定。

固定元数 intrinsic 的操作种类与必填操作数属于同一分支：一元操作携带 operand，二元操作携带 lhs/rhs；数据借用携带 object、block 及已确定的对象与函数签名信息。函数类型的构造／替换返回 FunctionTypeId，enum variant 保存 enum application 身份；需要通用类型时显式取得对应 canonical type。declared nominal 的字段等数据归属独立 payload，表示专用操作只接收其适用的 payload 或窄引用。

整数表达式的一元／二元分支使用对应的窄操作类别；div/rem 仅属于二元 managed 操作。该完整结构贯穿 Export HIR、LocalConcrete HIR 与绑定默认模板；旧 wire 的操作标签与参数字段在解码绑定处一次组合并检查元数，生产端由完整表达式投影原编码。方法引用生成固定 intrinsic 正文时，按已决议操作直接建立所需形参和值，不将它们重新装成任意参数列表。

数据借用的源码和导出模板仍通过普通泛型调用保存声明与实参映射；HIR 具体化在解析实际 intrinsic 和完整实例签名的边界，将其形成具名 object／block、对象类别和 FunctionTypeId。对象类别及回调签名取自已决议形参，不能从可能为 Nothing 的实参结果猜测。无正文的源码 intrinsic 复用 source parameter interface 中的逻辑参数；导入模板复用其完整实例签名，不为不存在的正文生成参数 local。MIR 接收完整借用操作，先后求值 object 与 block，保留原 pin／finally cleanup 与异常传播，不重新按参数个数或值类型恢复关系。

LIR struct store 在创建 Scoop／C 表示时返回对应的窄引用，后续字段填充沿用该引用；C payload 的消费接口接收已选中的字段与布局。不可变类别的存储投影仍属于普通 arena 查询，不增加另一套身份或重复保存 nominal 数据。宽表示入口只完成一次实际分支选择，intrinsic 不以空 declared 字段参与源字段或 C payload 操作。

MIR 中 intrinsic struct 与 intrinsic class 分别保存其允许的表示类别；例如 Array／Atomic 不可填入 struct 表示，Ptr／MaybeUninit 不可填入 class 表示。该区分沿既有 nominal 生产路径完成，后续物理类型与布局消费不再排除另一类 nominal 的变体。

构造器和未绑定成员引用在 HIR lowering 完成候选选择与全部类型实参推导，再生成具有完整函数类型的普通闭包适配 body。适配 body 只调用已选定的构造器或方法，不复制构造初始化；virtual / interface 调用保留原动态分派目标。适配 body、泛型和默认参数产物复用既有 lambda、调用及构造形态，不另建引用 ABI。

receiver 函数类型在源码 HIR、共享签名及 exact type identity 中保留独立的 receiver 类型槽。类型替换、lambda 参数推导和产物消费使用该形状，不从名字或普通首参数猜测 receiver。lowering 将 receiver 展平为第一个实际源码参数，复用 closure / invoke、函数值适配和既有 ABI 分类；closure environment 等隐藏参数独立处理。`this@label` 在 HIR lowering 绑定到实际不可变 receiver binding，外层 receiver 沿普通捕获路径保存，不把标签带入运行期名称查询。

默认值、泛型正文及词法 callable 保存定义处绑定、完整 binder、局部值和 capture 身份。再次消费只替换实际类型与求值上下文，不重新选择名称、重载、字段或接口默认实现。局部函数和 closure 的捕获引用原绑定，不能改为消费方的同名局部变量。

补齐元数据等新增依赖后再次推进导入正文队列时，已经完成的函数和构造器正文直接复用；不能重新展开构造初始化并替换其中的局部绑定，使既有闭包捕获失去对应定义。

默认模板分别保存提供方参数的预期类型和正文表达式的实际类型。前端按子类型规则检查默认值；`Nothing` 正文可以满足任意参数类型，产物投影、读取及实例化必须保留这两个类型，不能要求它们逐字相同或把底类型调用改写为参数类型的成功返回。模板契约仍校验预期类型与原提供方参数一致；通用格式构造器不重复进行依赖源码类型表示的子类型判断。

源码调用记录的实参保留实际表达式类型。通常实参已由前端适配为参数类型；Nothing 实参保留底类型，在 HIR 源码签名和 MIR 实现签名连接时通过实际 nominal 的 intrinsic 表示识别。两处均保留参数个数和位置检查；调用结果仍须与被调用声明的完整结果类型相同，不能因上下文需要而改写。

nominal application 保存原声明与完整实参；struct、enum、class、interface 各有独立 typed identity。alias 透明展开；字段、variant、构造与 property accessor 保存各自原身份。按值循环、继承环、泛型约束、GC-free 和 ZST 属性在所属语义边界确定。

依赖 struct 的存储属性读取使用已发布字段身份；显式 receiver 与成员、扩展正文中的隐式 receiver 遵守同一规则。Storage accessor 不要求另行导出不存在的函数体。

解析名义声明头时，先建立该批声明的完整继承与接口关系并检查继承环，再验证依赖这些关系的类型实参上界；合法的 `S : I<S>` / `I<T : I<T>>` 不因声明顺序或尚未填入 S 的接口表而失败。延后检查只保留本次实际出现的 typed 上界要求及诊断上下文；局部声明与依赖接口使用同一时机，不重放完整声明解析。

sealed 的继承限制在 HIR 按真实 origin、package 与声明作用域检查；成功声明保存完整直接子类型集合，class 的有效 modality 仍为 abstract。when 按这些关系与已有类型替换检查闭合家族，不在 MIR 或链接阶段重新求穷尽性。共享 nominal details 对 sealed 增加第 12 字段的规范化直接子类型身份列表，未 sealed 的原 11 字段编码保持不变；相关子类型的普通声明与继承签名进入现有共享声明闭包，私有数据不增加 public lookup binding。reader 在普通格式与引用边界检查集合及关系，消费方复用已读信息，不增加继承授权或运行期注册协议。

interface implementation delegation 在 HIR lowering 中完成 slot 选择、隐藏字段分配和普通转发成员的生成。字段身份由真实宿主与继承项位置确定，和源码 property backing/delegate 字段区分；它不产生可查找的源码属性。构造记录在 base/primary store 后、body 初始化前保存 delegate，转发正文只包含普通字段读取与已绑定的接口调用。泛型构造及转发正文保留原 binder、字段和 slot 身份，跨 Cone 通过现有声明、字段和正文闭包消费，不在下游重新决定委托成员；MIR/runtime 沿用普通对象布局、接口分派和 GC 扫描。

透明 alias 的声明查询先在定义处绑定独立的 typed 类型参数、bound 与完整目标，再在使用处替换实参；活动 alias 声明栈检查全部展开依赖中的循环。嵌套 alias 使用实际 owner chain 和正常类型名称空间，局部 alias 只存在于词法作用域并在 HIR 类型中展开。alias 不产生 nominal、构造器、layout、TD 或另一种单态化实体。公开泛型 alias 的接口保存规范化 binder/bound 与 typed signature target，跨 Cone 消费和再次发布复用该目标；不在使用处重绑定源码名称。无参数记录保留已有四字段编码，有参数记录增加第 5 字段的规范化 binder 列表；需要宿主实例化参数时，第 6 字段保存该列表的宿主前缀长度；带 binder 的目标使用完整展开 signature，不把参数应用伪装成无参 alias 边。

property 具有完整类型、读写能力、getter/setter、访问域与互斥的 stored/accessor/delegated/const/extern 表示。required accessor 有 Body 或 AbstractSlot 的明确类别；普通缺失初始化不能靠空正文、late-init flag 或可空类型补齐。

泛型 object/companion 在消费方物化时，字段及其 backing property 保留声明方身份。字段类型位置的校验沿字段的声明来源取得对应属性记录，不要求消费方重新声明该属性。

HIR 将属性赋值右值展开为前置语句时，先固定已经求值的 receiver，再接入这些语句；最终的 storage write 或 setter call 使用保存的目标。不可重新绑定的类引用局部值已经固定了目标，可直接复用。本地与导入的属性遵守相同的求值顺序。

class 构造输出明确区分 allocation、同 receiver 的 initializer、this/base delegation 与 common initialization；完整成功路径、异常出口与初始化顺序已确定。abstract class 仅有供派生构造使用的 initializer。release policy 为 None 或携带完整 hook target 的 SynchronousGcFree。

inner class（语言规范 9.1.3.1）的 binder 先保存外层 class 的完整 binder 前缀，再保存自身参数。普通 class source shape 中的 EnclosingInstance 字段角色及其 signature type 明确保存所需 outer application；该字段使用独立的 typed source field key，不产生源码 property。构造声明接口把 outer 作为独立 receiver 保存，参数名、默认值及 overload signature 仍只对应源码参数。实际 allocation callable 的值输入为 outer、源码参数；initializer 的输入为正在初始化的 inner、outer、源码参数。构造模板使用独立的 EnclosingInstance local selector，不能把 outer 与初始化中的 This 或源码 Parameter 索引混用。隐藏 outer 字段身份使用 source field tag `5`（仅 owner），隐藏参数 selector 使用 tag `7`（无 payload）；普通字段、This 与源码参数的既有编码不变。构造委派通过隐藏参数读取 outer，最终构造器在 base 后先写 outer 字段；普通成员与闭包通过该字段获得外层 receiver。所有引用均按普通字段、参数、捕获及 GC root 处理，不增加 runtime 入口或另一套对象模型。

局部类型与匿名 object（语言规范 9.1.3.2）在定义处完成普通 nominal 声明、类型、继承、成员及构造检查；它们使用原 Cone、词法 owner 和定义路径形成 typed nominal identity，外层 binder 作为完整参数前缀保存。匿名 object 没有 singleton/initialization unit，每次求值使用普通 class allocation。不可变捕获通过已绑定值、隐藏构造输入及普通字段保存，构造引用在创建时固定捕获值；value type 和 interface 不增加捕获环境。成功 HIR 保存完整声明与捕获类型，generic/default template 直接携带所需声明和 typed 引用，消费方不重新解析局部名称。局部/匿名 class 的捕获值按稳定槽位组成普通 tuple（空环境用 Unit），保存为一个隐藏实例字段；构造输入将该环境作为隐式 context receiver，与源码值参数分开。inner 的 outer receiver 与局部类型的捕获 receiver 使用不同的 typed context kind；二者复用普通构造参数传递、字段存储与 GC 描述。捕获集合在该声明全部 body 分析完成后确定，随后一次完成相关构造输入与环境读取的类型，再交付 HIR。捕获字段的 source field tag 为 `6`（仅 owner），隐藏参数 selector tag 为 `8`（无 payload）；已有角色编码不变。

singleton、runtime property 与 generic delegated application 保存完整初始化单元及 cycle throw target。constant 已完成依赖环检查与求值，没有 runtime unit。interface/default/virtual 选择在 HIR 唯一确定，qualified super 保存 direct target。

core 的 Add 等普通运算符接口不进入编译器内建 protocol 表。HIR 先将 operator 表达式形成相应角色的普通调用请求，在定义处完成成员、bound 或扩展决议；不以某个 core 接口的实现关系作为前置过滤。generic/default body 保存所选 typed callable 或 bound slot，具体化按实际 conformance 映射实现，消费方不得重新按 import 或方法名选择。已有数值 intrinsic 可以显式覆写普通 interface slot：其声明保持 Final，产物保存实际 slot 关系与原 intrinsic implementation，不能要求它只能是 DirectOnly。top-level、extension、constructor 与 source extern 仍遵守原直接调用合同。NoGC 实现与 Managed slot 的适配复用现有 callable/ABI；具体值直接调用仍保留 intrinsic。LocalConcrete intrinsic 必须从已解析的 source parameter interface（或已导入模板签名）保留完整物理参数类型，包括 receiver；它没有源码 local arena，不伪造参数 local id。当实际 itable 引用 intrinsic 实现时，MIR 以该实现原有 callable materialization 生成一个具备完整参数、返回值及 effect 的可调用正文，复用直接运算与 callable reference 的 typed 运算降级；不为未参与分派的 intrinsic 额外生成入口。装箱与 receiver 调整仍使用既有 interface adapter。由声明 slot 或 nominal dispatch selection 实际引用的 intrinsic 实现入口随普通 callable binding 导出，供 dependency 的 itable、装箱适配与 bound 具体化引用；只有直接展开用途的 intrinsic 不导出机器入口。

`==` / `!=` 按语言规范 9.3、11.11 从普通成员中选择 operator equals，保存实际 callable 或普通接口 bound target；不要求 core Equals conformance。`Equals<T>.equals` 按普通 interface 声明、override 和 bound call 处理，不进入 core operator／protocol 身份表，也不为同形方法或结构比较补齐接口。

不可重载的 `===` / `!==` 同样从左到右各求值一次；右侧展开出的 HIR 前置语句必须位于左侧取值之后，不能改变左侧已取得的引用。

判定派生签名是否被手写成员占用时，检查名为 `equals`、参数为完整宿主类型的原成员签名，不以 operator 标记或当前调用处的可见性过滤。普通同签名成员仍可按普通函数调用，但不会成为 operator；导入的 nominal 使用已保存的成员声明执行相同判断，不因成员正文或源码不可见而重新派生。

结构比较在当前静态类型环境中绑定字段的 operator equals。宿主包含类型参数时，先按顺序保存两个操作数，再将已绑定的比较正文展开为普通字段调用和短路表达式；enum 的 payload 读取受相应 variant 检查保护。泛型正文、默认参数和跨 Cone 模板保留这些普通 HIR 操作，具体化时不得按具体宿主重新选择字段重载，也不得与直接比较该具体宿主的派生正文合并。封闭宿主仍使用按 exact owner 标识的既有派生 callable。`Equals.equals` 继续由普通接口、泛型正文、装箱和跨库调用机制承担。

开放结构比较的展开模板可以复用已绑定的字段比较，但每次内联生成的表达式必须采用本次比较操作的完整 definition/evaluation origin；嵌套展开沿用该来源。不能把第一次生成模板的方法上下文或位置带入其他方法。比较位于默认参数中时，保留该默认表达式已有的定义／求值来源，由普通缺省实例化继续传播实际调用位置。

按 exact owner 生成的派生 callable 没有独立的词法源码正文，其表达式保留触发派生的来源；该来源可以是依赖中的默认值或模板。检查其中的依赖调用时仍验证 definition/evaluation 的文件、位置和上下文引用，但不能要求求值来源属于当前 Cone，或要求这个按类型生成的 helper 出现在导入函数模板表中。生成身份继续只由 exact owner 决定，不因首次触发派生的文件或调用点改变。

依赖携带的 source record 与 source context 用于诊断及实际语义引用。读取其格式、范围和引用即可使用，不再遍历 provider 的副本逐字段比较来证明来源；源码显示内容不构成产物认证信息。

派生比较保留值传递、可见性和 InteriorMutable 的实际类型使用检查。NoGc 实现满足 Managed interface slot 时分别保留实现体 effect 和接口调用 effect，适用于显式 `equals` 及其他普通接口方法；具体值的 intrinsic operator 比较保留原 effect。其他签名、访问、ordinary/suspend 与安全性检查遵守普通规则。

extern 声明产生完整 `SourceNativeExternalContract`：symbol bytes、逻辑 library/default namespace、function/data/TLS、mutability、C/Scoop ABI、calling convention、完整的 native 签名、GC effect 与定义位置。Scoop callable 另保留完整源码签名；启用 errno 捕获时，两者按语言规范 13.4.2 建立明确的结果适配关系。普通／suspend 效果与 GC effect 独立，后续不得由 symbol 或参数类型反推。

C ABI extern 函数的声明按语言规范 13.4.1 另存非可选的调用模式 `NativeSafe` 或 `GcLeaf`：未标注时前端填入 NativeSafe，合法 `@GCLeaf` 填入 GcLeaf。此模式随声明、模板实例化和实际调用经 HIR/MIR 传递，不由参数 GC-free、`@NoGC` 或 native symbol 名称推导。它属于当前 Scoop 声明的 caller 协议，不进入目标 C symbol 的物理 ABI 合并键；同一 C symbol 的不同声明可以选择不同模式，但不得因此放宽签名等既有 native 合同检查。

C extern 的结果适配明确区分普通返回与 errno 捕获。前端检查 `captureErrno` 的常量、适用位置，以及透明 alias 展开后的精确二元 tuple `(R, Int)`，将第一项 `R` 投影为 native 返回类型；`Unit` 投影为 `void`，其他类型按既有 C-FFI-safe 规则检查。合法捕获分支完整保存 native `R`、Scoop tuple 结果与捕获行为，不以可缺失字段或后续重新猜测 tuple 形状代替。调用、函数引用及泛型消费保持固定的 Scoop 返回类型，不因结果被忽略而关闭捕获。release 调用对完整 Scoop 参数和结果应用既有 `ReleaseValue` 检查。

C-FFI-safe、Scoop ABI、GC-free 与 release-safe 是不同合同。C 参数不得含 managed reference，Scoop ABI direct ref 不改为 handle 或 pin。generic 条件按原 binder 保存并在实际 application 检查。

实际依赖分为 lookup observations 与 committed uses。前者包括空查找、不适用候选、star surface 和 re-export；后者才形成 MIR/LIR 的 callable、类型、布局与 native 需求。未展开默认值和未物化模板不产生机器调用。

具体化沿用当前 Cone 声明的 nominal 物化闭包作为初始根，包含本地非泛型声明；正文调用、存储及其他实际类型用途继续扩展具体化闭包。本地物化集合与发布声明闭包承担不同职责，不能用较小的发布闭包替代本地根，也不因本地物化而发布私有类型。比较实现按实际调用与发布需求生成；类型的接口集合始终来自普通声明和继承关系。

定义 core 时，输出协议所引用的基本类型主体也通过同一物化请求入口补齐；它们不依赖某个普通接口或无关源码调用间接触发物化。已物化的主体复用原结果。

公开非泛型值类型的可用派生 operator 正文由定义 Cone 发布，消费者引用该生成 callable；字段不可比较时不发布该正文，也不因此拒绝类型声明或构造。普通 `.equals` 调用与 `==` 使用同一派生成员签名；需要实现显式用户接口中的 operator slot 时，定义处即检查并生成对应正文。

when 的前端决议遵守语言规范第 5 章：普通条件保存已解析 Boolean 求值及其局部 setup，case 保存 typed pattern；多条件组合为现有有序短路 Boolean 求值，其局部 setup 保留在同一条件内。无 subject 不伪造 Unit subject。subject 声明复用不可失败 binding planner，初始化、快照及解构各按规则执行一次，并继续匹配完整快照。fallback 明确区分源码 else、语句正常落空及已检查的不可达。路径类型事实在当前作用域合并，普通表达式不进入模式名字解析。

有 subject 与无 subject 的 when 使用不同的完整分支；只有前者携带 case 条件与依赖 subject 的覆盖信息，subject 及其实际类型不由独立可空字段拼接。 arm 的 guard、正文及位置可共享表示；纯条件分支的主条件只包含 Boolean 谓词或后备条件，其末端仅允许 else 或正常落空。覆盖结果记录实际选用的穷尽性类别，类型从当前 subject 取得，不再另存需要后续比对的 subject 类型或 enum 身份。导出模板的既有 wire 标签和可选 subject 编码保持不变；解码绑定在这一格式边界一次选定完整分支，后续具体化和 MIR 转换直接消费已选形态。语义 Return 必填结果表达式，正常 Unit 返回携带真实 Unit 值；源码 bare return 在 HIR 转换为这一完整形式。结果表达式的副作用先于清理执行，Nothing 路径没有正常返回值。

已解析的导出正文／默认模板 Return 同样必含结果表达式。现有 wire 的可空返回字段仅用于编码兼容：旧的空字段在解码绑定时形成该语句定义位置的 Unit 常量，新产物写入完整表达式；不将这个 wire 选择传播回语义 IR，也不改变字段编号或 section 版本。

return/throw 与 Elvis 复用正常值和终止路径的封闭表示；只有正常到达合流点的分支提供结果，不为跳转生成 Unit、零值、null 或未初始化 local。表达式跳转的实际 transfer 保存在当前位置的有序 setup 中；其结果及全终止控制表达式使用 Nothing 类型的 Unreachable 节点，明确表示没有正常值，不读取存储也不执行 runtime 调用。MIR 将该节点结束为不可达正常边。尾随 lambda 在每个候选内先映射最后形参，再复用普通调用检查；成功 HIR 只保存完整已选调用。捕获、具体化、完成性质、默认模板、导出正文与 dump 均覆盖这些表示。

整型枚举在源码 HIR 中具有明确的表示类别、底层整数 kind 和已检查的有序 variant 常量；父子关系、继承数值成员、普通上转及接口关系在 HIR lowering 完成。共享源码定义与表示数据保留 enum/variant identity、底层整数和数值，消费方不重新求值 initializer，也不按纯 unit variant 猜测枚举类别。依赖成员查找沿实际整数父类型继续遍历其接口，仅对接口节点读取接口方法与分派选择，不能把整数父类型当成 interface application。数值上转与引用上转均为已检查的单向类型适配；上转结果按目标类型继续参与装箱。

### 2.3 MIR

MIR 只接收 LocalConcreteHir 和实际选用的依赖 MIR metadata，不接受 ExportHir 模板或执行泛型推断。每个 expression 具有本层 exact type；GC-free、ZST、字段和 variant 身份完整保留，synthetic 类型在产生时也完整定义。

调用与短路控制流从表达式提取为 CFG 时，必须先完成并保存此前操作数的取值，再执行后续操作数的调用或分支。没有显式调用的表达式仍可能读取可变字段、全局、指针或原子存储，或执行分配；不能因此延后其求值。聚合元素、调用实参和其他多操作数节点共享这一顺序规则，提前终止的操作数保留此前副作用并阻止后续求值。

`Nothing` 操作数没有正常结果。MIR 的私有构造表示须在建立整数算术、比较、移位或转换节点前传播这一控制流事实；此前操作数仍按顺序求值，终止后的操作数及整数运算本身不执行。不能将 `Nothing` 当作整数值送入要求精确位宽的构造器，也不能用虚构的零值代替；CFG 保留原调用的异常边及 catch/finally 清理。

errno 捕获调用保留 HIR 已确定的结果适配和 native 合同，表达式的结果仍为完整的 Scoop tuple。后续解构、存储或跨 suspend 保存使用普通值规则；不引入 last-error 读取节点、线程槽位或额外的 managed error 对象。

when 的普通谓词、多条件、模式、guard 与 fallback 降为既有 CFG；模式投影仅在匹配成功后执行，guard 失败继续下一 arm，合法落空保留正常边，终止分支不提供结果。return/throw 表达式使用相同 transfer 与 cleanup 通路。

MIR 控制流显式表示普通边、异常边、循环目标和 cleanup。Return、Break、Continue 只执行真正退出的 scope 清理；Throw/Rethrow 保持异常路径；finally 的覆盖规则和挂起不退出作用域的规则遵守语言规范。variant payload 只在同一值、同一 variant 的有效分支内读取。开放派生比较中的嵌套 enum 字段在其 tag 检查到达时保存为一个 immutable local，同一短路分支内对应的 test／payload projection 复用该 local；每层 payload 的取值仍处于外层 tag 的成功分支，不能提前到分支外。MIR 与后端继续校验同一稳定值的支配关系，不通过放宽校验来接受重复字段求值形成的不同值。

语言规范 13.11 的数据借用 intrinsic 在 MIR lowering 中展开为一次 receiver/block 求值、typed pin 帧 push、按精确对象种类取得数据指针与长度、普通 closure 调用以及所有正常／异常出口的 pop cleanup。block 为普通非 suspend 函数，帧不跨挂起保存。MIR/LIR 明确区分帧操作与数据地址操作，不把栈帧创建伪装成返回 native callee 栈地址的 runtime 函数。LIR 按实际 Array/String layout 计算元素区和长度字段；codegen 在 caller 入口分配帧存储，调用运行时规范 3.4 的 NoGC push/pop。数据地址按已确定的借用操作发射为 AS1→AS0 转换，不另附供自产 LLVM 复核的证明标记。原始数据指针是借用域中的 GC-free 值，帧中的对象承担保活与禁止移动职责；零长度数据地址不要求实际存在可解引用的元素。

所有调用以互斥分支确定 direct、virtual、interface、closure 或 function bridge target；各分支仅保存允许的 callee，closure／bridge 签名只保存一次。动态 receiver 为必填值，不从可为空的实参列表恢复。super 不重新进入动态分派；方法 receiver 为完整值参数，value receiver 的可观察存储属于本次方法调用。

structured MIR 与最终 MIR 的动态调用将 receiver 保存在对应目标分支，普通实参列表不再重复包含它；遍历、效果分析与 CFG 求值均先处理 receiver，再按顺序处理实参。virtual／interface 的静态声明目标只允许本地、单态化或外部 Scoop callable。已知 receiver 实际类型的去虚拟化先适配该值，再将其放入新 direct 调用的首个逻辑实参；closure／function bridge 直接消费各自唯一的函数签名身份。

最终 MIR 仅保存已经完成的调用与表达式。语言 Cast 在 mir-lower 的私有正文构造过程中展开，最终表达式类别不保留 Cast。协程注册操作和 pending cleanup chain 留在该 stage 的私有工作数据；成功输出不带临时 callee，也不保存恒为 Root 的 pending 字段。Return 携带完整语义结果，物理 void／ZST 分类属于 LIR。

CFG 的私有协程工作记录保留挂起操作的语义顺序、块内位置、receiver 与结果 local；尚未生成的注册调用不使用普通 callee 或空语句占位。协程变换把这些记录与已有 CFG 语句一起用于 liveness 和恢复分析，生成 continuation adapter 后直接形成完整接口调用。cleanup 中的待定 return／throw／break／continue 继续保留原值和目标，最终只输出引用实际 saved slots 与恢复入口的协程 metadata。

`==` / `!=` 保存已选定的 operator callable；若它来自普通用户接口，则保留该接口 application、slot 与实际 implementation。派生结构比较降为普通 typed 成员调用和短路控制流；`Equals<T>.equals` 是同一 operator 契约的普通接口调用。bound 调用在单态化时映射到已确定的实现，不能重新按名字搜索 equals。基本类型的精确 intrinsic 比较保持原运算与 effect；接口分派需要的适配正文同样是完整 typed callable，不能把 NoGc 直接实现误当成具有相同 effect 的 Managed slot 入口。

使用结构表示的值，其装箱接口闭包来自 LocalConcreteHir 的实际声明／继承关系，并由 MIR 的实际表项写入产物。tuple 的结构比较不增加 Equals 表项；Ptr 的显式接口仍按普通规则保留。产物读写检查接口引用、slot、callable 与签名的一致性，不因 exact type 使用结构表示就强制接口集合为空，也不在后续 stage 重放字段相等或源码 implements 的决议。

class allocation 只分配最派生对象一次，使用实际 exact TD；base/this initializer 不分配、不修改 TD。receiver 在可能 GC 的操作间保活并重读，只有完整构造成功后发布 release-ready 与结果。构造失败没有可用结果。

closure 具有完整捕获字段、类型、invoke 与扫描属性；绑定方法引用在调用时保留原动态分派。普通 local direct call 的捕获参数不改变原绑定身份。callback adapter 有精确 closure/native storage/status 签名，在返回 C 前处理全部异常。

suspend callable 的执行 ABI 为 `(source args..., Continuation<R>) -> CoroutineStep<R>`，后者仅含 Completed(R) 与 Suspended。输出不保留源码级 suspend call；frame、continuation、恢复入口和 saved slots 均有独立身份和完整类型。跨挂起只保存已初始化的 exact payload，不保存 native block address 或 active EH record。

接口 adapter 的物理 receiver 是具体对象引用；装箱 adapter 挂起时保存的 receiver 使用生成 box 的 exact type，而不伪装成源码 interface。其 `CoroutineSlot<box<T>>` 以 box 的 exact identity 区分，并与 box 一样归属 T 的定义或 application ODR group；扫描仍为一个 managed object 引用。普通 nominal 的 box slot 随已发布的 box 支持输出，纯内部 saved slot 不扩张私有 payload 的发布闭包。

object 的源码 exact type 与 backing class exact type 是同一对象的两种视图，共用分发表及 receiver adapter。adapter 的 implementor 保留源码 object 身份；backing 表通过类型记录中已有的 `Object { backing }` 关系引用它，不另建 callable 或放宽为任意继承 receiver。

completion 与 resume 具有唯一获胜方，并明确 payload 的 release/acquire 关系。恢复失败沿原调用点的异常／cleanup 路径传播。挂起状态使用 frame 的 TaskContext，恢复前后的切换见 2.16。

MIR metadata 保存源码声明到实际 callable、constructor、accessor、generated body 的映射，exact signature、GC effect、继承、dispatch、object/init 关系及 Strong/ODR 归属。它不定义目标布局；布局由 LIR metadata 提供。

mir-lower 完成 concrete CFG、dispatch、closure／adapter 与协程执行 ABI 后，由独立优化模块依次执行 receiver 实际类型传播、唯一目标去虚拟化、按调用点选择的小函数内联、局部常量／CFG 清理和再次去虚拟化；driver 显式传入优化配置。scoop-mir 保持纯数据职责，program-link 不隐藏重编译或导入普通外部正文。

release 默认启用 MIR 优化，debug 保留未优化的完整 MIR；两种模式的阶段 dump 分别记录实际输出，使用相同的实体身份与 ABI。数据流在异常边合并可能抛出时的局部状态，不能只把 block 的最终状态交给 handler。实际表项或闭包 invoke／bridge 完整可用时才改写目标；外部目标使用已经选中的 callable 签名和引用。

函数内类型事实为不可达、未知或唯一实际类型。最派生 allocation、已知 box／closure 和 final class 提供事实，复制和合法转换传播，合流只保留一致事实，循环求不动点；未知返回、可变字段和取址后的可变 local 保守处理。base／this initializer 不收窄实际类型，initializing receiver 的使用仍受语言规范中的构造约束；静态 open class 或本 Cone 实现数量不构成全程序闭合。去虚拟化按原 typed slot 查询实际表项，一次生成含完整 callee 与 receiver 适配的新调用分支，保留原 effect、EH、Context 与 continuation。

自动内联只处理本 Cone 已有完整 concrete 正文的非递归 callable，包括本地物化的外部泛型和普通 adapter；递归 SCC、无正文的外部 Strong、native／callback／gateway 和无法完整重写的特定 EH／Context／协程控制流保留调用。内联重分配 local、value、block 和临时存储身份，连接 return／异常出口，保留实参各求值一次、独立按值 place、位置、初始化发布和清理。Managed、分配或可能抛异常本身不构成拒绝理由；不复制后端 root/frame plan，站点由后续 LIR 统一形成。

首版克隆不含自身 landing pad／catch／resume 协议的普通 CFG；callee 的可能抛出操作与显式 throw 继承调用点的异常出口，caller 已有 catch／finally 可以继续承接异常。依赖函数局部异常槽的 callee 及协程执行入口保留调用。克隆产生的局部值属于 caller 的新 SyntheticValue 身份，所有形参先取得独立局部副本；常量折叠只处理可证明无副作用的布尔与整数运算，整数保留对应 MIR 运算的精确位宽与符号规则。地址可见的 local 不参与常量替换。Managed callee 的循环保留 poll 标记，NoGC callee 的循环内联到 Managed caller 后也不得新增 poll。

多个内联 return 先写入独立的汇合临时值，再在 continuation 初始化原调用结果；不得把原本稳定的结果 local 改为可重复赋值的身份，破坏其后 variant test／payload projection 的关系。优化删除引用后，已选外部 callable 声明目录可以保留未使用项及其稳定索引；声明的完整签名仍保留，实际链接成员由对象中的真实引用选择，不因目录项无调用而拒绝合法 MIR。

带 release 的对象构造保留同块相邻的 allocation、最外层 initializer 调用与 release-ready 发布。内联可以整体复制包含该序列的普通正文，但对应的最外层 initializer 调用本身保留，不将发布拆入缺少构造前序的 continuation；异常路径仍不发布。

输出为 Library 或携带本模块非可选 entry 的 Executable，不能由 `Option<Entry>` 与独立 kind 拼成矛盾状态。entry 保留 HIR 已确定的四种形态、本层 main target 和完整签名；生成的 root gateway 明确区分成功退出码与失败状态，有参数形态包含受检 argv 构造与 main 调用的完整正常／异常路径。gateway 与相关 helper 是实际需要发射的 typed callable，不能留到 program-link 阶段生成 managed 语义。

### 2.4 LIR

LIR 以完整操作表达托管分配、字段／数组写入、值复制、原子引用访问、`ManagedPoll` 与 `PublishReleaseReady`，携带实际类型、布局、GC effect 和 live set。Immix 普通对象上限、TLAB 资格及 card/page-map 算法属于 codegen 的 GC 实现，不作为通用 LIR 策略常量；实际 runtime 符号、共享布局和兼容性摘要仍由现有 ABI 数据目录保存。

`ManagedPoll` 表示运行时规范 3.2 的条件 poll，携带唯一慢路径 site 与完整 live set。codegen 发射 acquire 检查和快／慢分支，只在慢分支物化该站点的 managed leaves、调用协调入口并回写 relocation；后续使用通过普通 canonical storage／SSA 合流取得快路径原值或慢路径新值。入口 pending 的 mode 检查属于同一操作，不能仅按 world phase 删除首次激活。TLS 与全局数据引用使用闭合 runtime ABI 目录中的普通数据合同。

driver 按“lir-lower 准备布局与逻辑签名 → codegen 规划目标 ABI → lir-lower 完成正文”的顺序编排，stage 之间只传递 LIR crate 定义的完整数据。准备结果与最终 Module 是不同类型；不得用缺失签名、默认 ABI 或完整 lowering 后丢弃正文重跑来实现准备。布局、scalar leaves、扫描信息与已选依赖 ABI 在准备时建立／映射一次并复用。聚合输入以 typed 字段引用共享已准备的存储节点；不按逻辑字节数展开大型 tagged payload，也不复制嵌套值布局。后端只为可能使用寄存器的聚合读取其标量叶子。

具体 Scoop/C ABI 分类器属于 codegen，按完整参数序列、结果和逻辑类别生成计划；覆盖本地／外部函数、虚／接口槽、closure／bridge、suspend 执行入口、callback、gateway、release 与 runtime helper。计划通过原有 typed callable/type/slot/runtime entry 身份或专用批次局部 ID 关联；定义、调用、adapter 与导出复用相同计划，不按 symbol 或参数数量猜测。通用 lir-lower 只消费布局事实和物理计划，不登记架构、不执行目标 ABI 算法；LIR/identity 仅保留完整数据、编码和必要的局部结构检查。

LIR 保存目标上完整的类型布局、值表示、字段 offset、alignment、scan、调用 ABI、控制流与实际定义／引用。codegen 不按名称、实参、result storage 或上下文补类型、签名、poll 或逻辑 live set。

固定的 LIR foundation 为实际定义建立一次 plan 与 plan/role 查询索引，后续解析和 registration 检查复用这些索引，不为每次查询扫描全部 atom。索引保留同一 plan/role 下的所有定义，唯一目标查询仍区分缺失和歧义；它不进入产物格式或实体身份，返回的定义集合保持原有 canonical 顺序。

对象 symbol surface 同样为 atom boundary 建立一次查询索引，按 atom 的 typed identity 返回既有边界符号；逐 callable／safepoint 发射只查询实际需要的 atom，不重复建立整个 Cone 的 boundary 表。codegen 删除未发射的定义或 atom 后同步更新索引，不能返回已删除记录；索引不写入 wire，也不改变既有 symbol 或定义顺序。

callable 对象发射直接借用 production 中已有的 runtime scan 计划，按 body identity 取得本函数的扫描树与 Context key；不在每个 LLVM module 中重新扫描全部函数来构造相同计划。Context cell 与扫描 atom 的边界也通过上述索引取得；无 callable 输出的 metadata module 不另建扫描计划。实际扫描内容、发射顺序和 GC 契约保持。

ELF 可见性与 COMDAT 准备将固定 symbol surface 的实际符号名和所属定义组索引一次，在各 callable LLVM module 中只查询该 module 实际包含的 global、function 和 alias；不逐 module 重建全部符号名或探查未发射的 atom。保持原有计划顺序、可见性、weak linkage、alias owner 与定义组冲突检查。定义计划完成最终裁剪后，metadata module 使用裁剪后的计划；该 codegen 局部查询索引不进入 IR、wire 或 runtime ABI。

每个 pointer（包括 null）具有 Managed、Raw、Code 或 Metadata provenance；LLVM opaque pointer 不抹去这种区别。managed 使用 address space 1，其余使用 0，转换必须为明确合法的 typed operation。

LLVM module 的 data layout 将 managed 地址空间声明为 non-integral pointer（生产配置为 `ni:1`）。普通复制仍按精确 storage 执行，但优化器不得从整数搬运值自行重建 managed pointer；GC leaf 从 typed pointer load 取得。分配结果、card 地址和 scoped data borrow 的显式 typed 转换继续按各自合同检查，此声明不改变目标的指针大小、对齐或原生 ABI。

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

LLVM 为值复制或 coercion 清零生成的 `memcpy`／`memset` 调用进入既有 target-support native requirements，tag 分别为 1／8。其机器签名分别为 `(pointer, pointer, i64) -> pointer` 与 `(pointer, i32, i64) -> pointer`，均为不抛异常的 NoGC C helper；真实目标对象的直接调用 relocation 与普通 libc 链接继续验证。它们不新增 Managed site，也不能满足或替代周围的 Managed invoke 记录。

小值的参数与结果分别保存 coercion：SysV 参数展开为最多两个 INTEGER／SSE carrier，结果为单 carrier 或双 carrier 结构；参数分类在完整签名中扣除隐藏 sret 和先前参数占用的 GPR／SSE，任一寄存器类别不足时整个 aggregate 使用 byval，未使用的另一类别寄存器留给后续参数。Darwin 普通 aggregate 参数按 64-bit 字块（需要 16-byte 对齐时为 i128）传递，短结果可使用精确位宽整数；HFA 使用保留连续寄存器约束的浮点数组 carrier。HFA 必须由 1 至 4 个同精度浮点叶组成，且叶连续覆盖完整 storage；额外 padding 不算浮点成员。Darwin 的连续 carrier 由 LLVM 的 aggregate 参数规则整体放置，不能展开成彼此无关的标量参数。

carrier 记录实际读取的 extent，允许其机器类型宽于 extent；读入前将 carrier 临时存储清零，只复制 extent 字节，回写亦仅复制该范围。跨 Cone canonical ABI 保存完整 carrier 序列而非仅保存 DirectParts 标签，target classifier 的版本同时进入现有 target fingerprint。产物消费者复用保存的物理计划并检查 exact storage／引用一致性，不另行通过源码重建一套参数分类。

每个 indirect 实参使用调用方新建的 exact storage，callee 按值接收。物理参数为 indirect result storage（若有），随后按逻辑顺序省略 ZST 并传递其余参数。LLVM 使用对应 exact type/alignment 的 byval/sret；实际寄存器分配由目标 ABI 决定。Darwin sret 使用 x8，amd64 使用 RDI 并在 RAX 返回同一地址。

ZST 的求值、调用、构造、类型及对象身份不能因零 payload 消失。被观察地址的独立 place 有满足 alignment 的存储 token；同时存活的不同 place 不共享地址。Ptr<ZST> 的元素位移为零，仍遵守 operand 求值和 unsafe 有效性条件。

call target 同时确定 destination、calling convention、全部参数与 return convention；void/direct/indirect-result 以及 Managed/NoGc/NativeSafe/NativeGcLeaf/NativeBorrowed 使用完整互斥分支。Managed call/poll 带 site ID 与逻辑 live set，invoke 带正常／异常 root set；NativeSafe/NativeBorrowed 带 site ID、caller roots 与按返回形式确定的 result publication。NoGc 与 NativeGcLeaf 不携带虚构 safepoint 或 root plan；NativeGcLeaf 保留完整 C ABI 调用计划，不能伪装成普通 Scoop NoGc ABI 调用。C extern 的物理调用计划为 DirectC 或 StorageBridge，与 NativeSafe/GcLeaf 协议独立。

NativeBorrowed 的调用与结果发布按返回约定一次构造：间接结果复用调用的 result storage，含 managed scan 的直接结果携带对应 root storage，void／ZST／GC-free 结果使用不需要结果 root 的分支。C 与 Scoop struct 的专用布局接口保留具体表示类别，不接受不适用的通用 struct 后再断言。存储分配和表示转换由 lowerer 完成，IR crate 只保存完整数据。

live set 包含 post-site live references 与 managed 实参，以及 aggregate 的完整 managed leaf path。普通 call/poll 的机器 relocation 与显式 invoke/native root frames 遵守运行时规范 3.2；跨 transition 的实参和结果必须使用更新后的存储。

root gateway 使用运行时规范 2.8 的精确 C 原型：argc、原始 argv、指向独立 Int32 退出码槽的 raw pointer，返回 UInt32 status。其内部对 main 的调用仍使用实际 Scoop ABI；不能因外层返回 UInt32 就覆盖 main 的 Int 结果。入口 poll 先于 argv 的 managed 构造，数组、当前 String 与其他跨 safepoint 的引用使用普通 roots、relocation 和写屏障；数组构造及 main 的异常在 gateway 内完成捕获、failure-root 发布与 EndCatch。BeginCatch 返回原 managed Throwable，普通 catch、callback 和协程 lowering 直接保存该引用，不生成复制对象的调用；后续根与 relocation 沿普通引用处理。eager gateway 仍为无参数 status 返回，不能与 root gateway 共用错误的函数指针原型。LIR lowering 生成完整 ABI 与 gateway 结构，codegen 直接发射；对象发布不重放源码 main 或已完成的 gateway 检查。

值类型布局使用原 exact 类型与字段身份。tagged enum 的 tag、payload slot 与 scan 一致；niche 只用于语言允许的引用／pointer Option。整数宽度与 signedness、Float/Double 精度、内部计数和 runtime ID 均保留各自类型，物理位宽相同不等于同一实体或源码类型。

wrapping 整数运算不携带 nsw/nuw；除零和 signed MIN/-1 已有安全控制流，shift count 已按位宽正规化，compareTo 的结果为 Long。浮点运算遵守 2.18。

C ABI storage 为封闭类型结构：integer、Boolean、Float/Double、带完整 pointee 的 data pointer、带完整 signature 的 function pointer、完整 C-layout struct。只有结果可为 void，只有 Ptr<Unit> 的 pointee 可为 opaque void。by-value struct 关系无环，pointer 可引用 forward-declared struct。

nullable data/code pointer 保留对应 exact enum 与 pointer 类型，不得把同一 enum 绑定到不同 pointee/signature。CLayout 的 aligned/packed 各为 Natural、A1、A2、A4、A8 或 A16。C ABI storage 与 Scoop 值表示分别准确保存；C ABI 分类不能用 Scoop aggregate ABI 代替。

在当前 Darwin/AArch64、Linux/amd64 GNU 和 musl profile 上，固定参数的 cdecl C extern 未选择 `captureErrno = true` 时，合法 canonical C signature 必须使用 DirectC，包括 integer、Boolean、Float/Double、data/code pointer 和按值 C-layout struct。条件按已有 C-FFI-safe projection 判断，包括 Char、PinnedPtr／GcHandle 与合法 nullable pointer；不把普通 C-layout struct 凭大小或字段数当作标量。Unit 仅映射为 void 结果。errno 捕获继续使用 StorageBridge；本条不扩展 callback、FunPtr 或 native global/TLS 协议。

GNU／musl 共享 SysV AMD64 eightbyte 分类，保留 INTEGER／SSE／MEMORY、整 aggregate 的寄存器耗尽回退、byval 和 sret。Darwin/AArch64 使用对应 AAPCS64／Darwin 分类，保留 HFA、普通 aggregate、栈参数和间接结果；四个 Double 的 HFA 不受 Scoop 16-byte 小值阈值限制。完整物理签名覆盖 nested layout、padding、alignment、扩展、coercion 与隐藏参数顺序。classifier 只消费已有 C projection 和完整签名，不建立运行期 libffi 或 FFI 插件；以所选目标 C compiler 产生的独立函数作互调验证。

DirectC 由 codegen 的 ABI 规划入口根据 LIR 准备提供的 canonical C signature 与目标 C ABI 确定完整的物理参数/结果、calling convention、小整数扩展和 Boolean 表示转换，codegen 按该计划发射 LLVM 类型与 ABI 属性，目标后端分配寄存器及栈位置。不构造 storage bridge 专用的参数局部变量、返回缓冲区或 memcpy 往返；目标 ABI 要求的栈传参、byval、sret 与表示转换保留。StorageBridge 保留完整 storage signature，由系统 C compiler 生成桥接；不能只设置 LLVM C calling convention 就把未经分类的 struct 直接传递。

DirectC 的聚合值计划复用带 offset／extent／alignment 的 carrier 描述，并分别保存参数与结果 coercion。SysV MEMORY 参数记录 `byval` 及至少 8-byte 的参数副本对齐，隐藏 sret 使用结果的实际对齐且占用首个整数参数寄存器；寄存器不足的聚合参数整组回退。Darwin 超出寄存器 aggregate／HFA 分类的参数传递 caller 的独立副本指针，不标为 SysV `byval`，间接结果使用 sret 的 x8 约定。副本对齐与原类型的存储对齐分开保存；原始 packed storage 不因 ABI 要求而改变布局。

当前三个 profile 的 DirectC 参数与结果中，有符号／无符号 8-bit、16-bit 整数分别使用 signext／zeroext；Boolean 使用 LLVM i1 与 zeroext，32-bit、64-bit 整数及浮点、pointer 无整数扩展属性。TargetProfileContract 的 C ABI lowering 记录 aggregate 分类合同，进入现有 target fingerprint 和构建缓存键；迁移时登记新 wire tag，退役的 ScalarDirectOrSystemCBridge tag 2 不复用。

`captureErrno = true` 的 LIR 计划分别保存真实 C 函数结果 `R`、bridge 的 `Int32` 返回值，以及 Scoop `(R, Int)` 的 exact layout 和结果重建操作。带捕获的 bridge 继续通过原有 caller-owned C result storage 写回非 void 的 `R`，以 C `int32_t` 返回捕获的 errno；`R = Unit` 时省略原结果缓冲区，在 Scoop 侧构造零 payload 的 Unit 元素。普通不捕获的 storage bridge 保持原有返回方式。tuple 不出现在目标 C 函数原型中，也不作为 C struct 返回；必要的 C/Scoop 表示转换在已保存 errno 后完成。

每次调用使用独立、在 native 调用期间地址稳定的 GC-free 临时存储；嵌套、递归与 callback 内的调用不能复用仍在使用的缓冲区。NativeSafe 调用在返回握手后使用结果，GCLeaf 与 release 按各自无切换路径使用结果。捕获值及包装 tuple 不增加 GC root、Scoop TLS/thread 操作或堆分配，也不能代替原有实参保活与指针生命周期规则。

compiler-owned static storage 带完整 scan 和封闭初值 ZeroedForRuntimeUnit 或 EncodedStaticValue；后者具有 canonical bytes 与排序的 immortal relocations。raw global/TLS 另存完整常量，不进入 managed static-root registration。ZST token、全零初值与 relocation 规则见运行时规范 2.8。

LIR metadata 保存完整 value/instance/element layout、Scoop/C ABI、scan、TD、dispatch、initialization、runtime registration 与 required definitions。ordinary nominal 定义使用原 provider 的布局；实际泛型或 structural application 使用其 ODR 布局。结构字段可按 canonical exact key 组成，不要求为没有实际物理定义的 constituent 另造 layout 实体。

每条 native contract 按 target 正规化 symbol，并保存完整 kind/library/signature/storage/GC effect；它进入 LIR 语义内容，host 搜索路径不进入。String 关联实际声明的 TD 与 provider，不由名字或相同 InlineBytes 布局代替。

整型枚举的 MIR/LIR 保留 exact enum 身份并直接使用底层整数标量布局；variant 构造产生其已检查常量，匹配按该表示区分 variant。参数、结果、字段、数组及 C ABI 使用同一整数的 size、alignment、signedness 与目标分类，不添加 tag、payload 区域或值域检查。boxed enum 的父 TD 指向 boxed integer，父整数解箱与函数值适配遵守运行时规范 2.3。Option 的现有 niche 范围不变，源码 enum 身份不能因机器签名相同而合并。

### 2.5 Codegen 与对象合同

GC 代码生成模块集中实现分配、完整托管写操作、发布与 poll 的快／慢路径。字段与数组 store、含引用聚合复制、数组构造／拼接、clone 及原子引用写入必须把实际写入和对应屏障放在同一操作中；通用发射器交付实际地址、值、类型／scan、内存序和必要的 safepoint 数据，不单独拼接写后屏障。批量构造／复制保留整段范围屏障，失败 CAS 不标脏，GC-free 写入不增加屏障。runtime 执行的 clone／boxing 等完整操作沿既有入口调用，内部写入遵守运行时规范 3.6。

分配、单槽写入与 poll 的既有快路径仍静态内联，不为模块隔离增加外部热路调用或重复 TLS 查询；保持原内存序，地址计算、写入与屏障之间无 safepoint。根活跃度、statepoint relocation、异常控制流与调用 ABI 继续由原有模块负责。纯内部职责重组不改变共享布局、runtime ABI 或产物格式。

codegen 的 LLVM 发射入口只接收当前 Cone 的完整 LIR；不在后端重新遍历全部类型、CFG、签名、variant 支配、callback、常量初值、safepoint 身份和根计划来复核上游语义。发射按现有 typed 数据执行实际操作，保留当前查询／计算的范围、引用与 LLVM builder 错误；相关语言和根计划规则由原有前端／lowering 完成。generated-C 同样直接渲染既有 bridge 与 C 布局，不重建 canonical layout 或 atom 计划副本作比较；真实目标 C 编译的 sizeof／alignof／offsetof 断言继续保留。

发射前的 ABI 规划入口只消费 LIR 定义的布局／签名数据，不读取 HIR 或重新解释 MIR 正文。具体 LLVM 初始化、CPU/features、data layout、机器栈与过程 ABI 规则归 codegen；目标布局描述与 backend contract 作为完整数据传递。Scoop producer 消费 LIR target/backend，generated-C 源码生成消费 LIR target/C toolchain 合同。宿主 compiler/SDK/sysroot 路径、环境与命令构造归 toolchain，driver 执行 generated-C 编译后将对象交回现有对象边界处理。输出可以有多个对象，物理分片数量与文件名不是 Cone 或实体身份。

元数据对象按已经确定的物理定义集合投影。实现先按对象序列逐层拆分候选模块，再从较小的模块生成最终对象，避免为每个对象重复复制全部 LLVM 元数据。每层只保留所选定义及其实际引用声明，沿用同一别名、TLS 边界与可见性处理；拆分过程不改变最终对象划分、符号、布局、注册内容或 ABI。

codegen 保留 LIR 已分类的数据关系：direct callee 携带实际 LLVM function，dispatch 携带对应分派数据；native transition 的准备分支一并取得 safepoint 与 roots，发射入口握手后保留退出所需的 roots、frame 与结果存储，不拆成需要彼此同步的可选参数，也不额外保存已消费且后续不再使用的状态。LLVM builder、对象及目标发射的实际失败继续走后端错误路径。

优化后的 AS1 根若通过 phi／select 合并 immortal 常量与动态对象，仍表示对象起点。codegen 在最终根分组处检查该合并链没有派生地址，并以 LLVM 22.1 的 `is_base_value` 标记向 RS4GC 保留这一事实；不能让常量分支被替换为 null base 而产生不符合当前 runtime 的 base／derived pair。纯常量选择仍不参与 relocation；typed 根恢复、base=derived 与实际 stackmap 数据生成保持。

LLVM 优化、RS4GC 与对象发射使用前一步的成功结果，生产路径不在变换前后完整验证自产 module，也不另写 RS4GC 后的策略、指针活跃性和 root restoration 复核器。优化后从实际 SSA 形成根分组、保留必要的根身份并发射 GC 元数据，继续执行真实转换。GC 与 ABI 回归通过实际 IR、机器输出和运行测试验证，不为内部转换添加证明专用的指针边界标记。

managed 单槽写屏障按运行时规范 3.6 内联读取四级 page map 与 region prefix，再原子置脏；这段计算没有 safepoint，不保留跨 safepoint 的 region metadata。runtime ABI 数据符号 PageMap 使用 tag 32，旧单 arena CardTable 的 tag 6 退役；PageMap 的 32768-byte／8-byte 对齐数据合同和新符号进入现有 registry fingerprint。范围写屏障保持既有 NoGC 入口，由 runtime 逐映射标记完整范围。

每个实际物理定义都有 typed owner、role、linkage、symbol 和精确 section range。引用使用 typed target 与真实 relocation；跨对象 external 声明不能变成可缺失的 weak reference。Strong 属于原定义 Cone，ODR 使用其稳定 member identity。

对象格式与架构分别决定容器结构和机器规则：ELF 不隐含 x86-64，AArch64 不隐含 Mach-O。所选对象规则解释 machine、e_flags、relocation 写入范围／addend、stackmap/EH 指针及最终动态 relocation；仅开放当前已实现的完整 target，不增加空 handler。对象边界验证目标格式、section/symbol extent、alignment、权限、relocation 写入宽度/addend、定义与引用闭合。Mach-O 使用其真实 relocation 与 weak 属性；ELF 使用 ET_REL、实际 section/symbol/RELA/COMDAT 数据，TLS 必须保持 STT_TLS/SHF_TLS，不能伪装成普通 data。
已验证的 relocation form 明确保留机器语义；现有 ELF form 的 tag 11 表示 x86-64 RELA（kind、width、signed addend），其编码不变，不把同一数字 relocation kind 自动解释为其他架构。ELF header flags 在所选机器的对象读取边界检查一次，后续 envelope 复用该结果。

relocation 的 section、atom、offset 和 target 均属于本次实际对象。raw source-native symbol 按 Darwin 添加一个前导下划线、ELF 保持 bytes 的规则正规化。Scoop-owned symbol 从 persistent identity 产生，不用 FQN 或 source 短名回退。

所有 Scoop body、NoGc body、managed adapter、root/init gateway 与 release hook 都有 callable registration。生成 C bridge/trampoline、runtime C 和 ordinary native producer 使用各自实际对象合同。实际保留的 safepoint 与其 owner、role、machine root count、stackmap 完整对应；无 safepoint 的 body 也可合法登记。

body 的 EH、LSDA、unwind、stackmap、关联常量与私有 Context cells 属于该 body 的同一次物理选择。TD、scan、dispatch、static storage、初始化和六类 registration 满足运行时规范；release hook 为 raw C、nounwind、NoGC，无 managed transition 或 EH/safepoint。

LLVM 在机器发射中由 switch 生成的引用查找表同样属于该 callable 的关联常量。每个 callable 使用已有 AddressTakenConstant／Singleton 子键描述这组后端常量，机器发射后按实际对象保留或移除；表内对 immortal object 等实体的重定位沿用普通引用闭合及 fingerprint。Mach-O 的后端表放入只读 __DATA_CONST 段，ELF 放入 .data.rel.ro.scoop.constants 并与 callable 一同进行 ODR 选择。它们不获得源码实体身份，不通过忽略 orphan relocation 或关闭 switch 优化绕过对象合同；无引用的数值 literal pool 保持原有按内容表示。

Mach-O 的数值跳转表可以位于 __TEXT,__const 的前缀，与后续已有边界的字符串等常量共享同一段。该局部表的范围止于首个已归属 atom（无 atom 时止于段尾），由代码到段首局部符号的实际引用识别；表内不含 relocation，其完整 bytes 参与 callable 内容指纹。不能因同段存在其他常量而拒绝正常 LLVM 输出，也不能把后续 atom 或其 relocation 纳入数值表。

C extern 按 2.4 的 DirectC/StorageBridge 计划发射。DirectC 使用真实 native symbol 和完整目标 C ABI，不生成或经过 outbound storage bridge；对象中的调用 relocation 直接表达该 native 引用。只有实际采用 StorageBridge 的调用才要求相应 generated-C unit；callback、native global/TLS 等其他桥接用途照常保留。两种路径均保留相同的 native contract 与 library requirement，不因省去 bridge 丢失链接需求。

generated-C bridge 保留实际声明的 C 类型、签名、目标工具链与编译选项；bridge 具有精确 primary/signature/context 定义范围，不能由邻近符号猜测。DirectC 与 StorageBridge 均不得按 extern 名称被 builtin 替换。普通 C ABI 与 Scoop NativeBorrowed 调用不混用 storage bridge。标量直接调用不依赖 bridge 内联、跨对象 LTO 或相容的 Clang bitcode；现有 generated-C 独立编译流程继续适用于保留的 bridge。

errno 捕获 bridge 使用所选 target 的 `<errno.h>`。全部参数解包及必要准备完成后，先执行 `errno = 0`，再调用真实 C 函数；紧接着以独立语句将 errno 复制到 native 局部整数，然后才复制 `R`、执行其他 helper 或返回捕获值。当前 profile 的 C `int` 为 32 位，可精确映射为 Scoop `Int`。非 void `R` 通过既有 C storage 写回，bridge 返回 `int32_t`；void 不产生虚假的 C 返回对象。目标 libc 的 errno 访问所产生的 native 引用进入既有 generated-C 对象与链接需求，不能使用宿主 accessor 或新增 Scoop runtime 槽位 API。

NativeGcLeaf 在两种物理路径上都不发射 enter/leave native、RETURNING、statepoint、caller-root push/pop 或专用于该调用的 relocation reload；调用前后表达式的真实 safepoint 不受影响。NativeSafe 可复用 DirectC，但仍保留自身的线程协调与 caller roots。DirectC 的 GCLeaf 调用不增加桥接函数调用或由桥接协议导致的参数/结果内存往返；不承诺消除 ABI 必需的搬运、正常寄存器 spill、动态链接跳板或显式的 errno、pin/unpin 操作成本。`captureErrno = true` 在 M33 使用带立即捕获的 StorageBridge，不属于无桥接开销的范围。

GCLeaf 契约不意味着 pure、readnone、readonly 或 nosync，不能据此删除外部副作用或推导用户数据同步属性。代码中的调用模式必须与所属 Scoop 声明一致，native symbol 去重不改变已选择的模式。直接调用的 ABI 通过与目标 C compiler 的调用交互验证；验收同时检查优化后的机器码，确认 DirectC 没有新增 bridge 调用或桥接缓冲区，GCLeaf 没有 GC 边界操作，不能只凭 LIR 标记或微基准耗时判定。

对象内容 fingerprints 使用实际定义 bytes 和 canonical relocation target；物理地址、archive member 分组或 section ordinal 不得冒充语义身份。每个需要回填的 digest slot 为无 relocation 的 32 bytes，初始为零且有唯一写入者；同一 digest 的镜像必须逐 byte 相等。

摘要依赖必须无环，节点仅依赖实际内容所需的上游摘要。规范化对象时，自身与不属于传递依赖的 digest slots 归零，上游 slots 使用已确定值；不能用执行顺序决定哪些 bytes 入 hash。Code、RuntimeImage 与 Artifact 分别覆盖自己的内容，不能建立自引用。

对象中的实际 stackmap 与 LSDA 遵守运行时规范 2.8、5.2。LLVM 机器发射成功后直接使用产出，不执行自建的机器指令、frame chain、stackmap 或 LSDA 全面验证；只读取后续地址、GC 数据和补丁计算实际需要的字段，并复用结果。stackmap 按实际 function relocation 关联 callable，以符号地址加 LLVM instruction offset 取得返回地址，读取并归一化实际根记录；不再次反汇编检查 call/frame chain，也不重建预期 member/function/site 全集做覆盖证明。优化后的 invoke 是否仍存在决定 EH atom 是否保留，不为此重建或比对独立 EH manifest；删除 final/provisional 对象复制、指纹清零后重填及最终字节比较等消费 replay。仅为验证服务的边界和索引删除；仍用于 codegen、重定位、GC 或实际链接选择的索引在同一份 surface 上建立一次。

发布直接接收本次 codegen 与 generated-C 的实际对象、成员分配及生成记录。C bridge 的编译器 profile、源码模板、flags 和 unit 集合不再与自行构造的生产记录重复比对；对象中的 SDK／工具版本也不作为来源证明。所需 section 与符号正常读取，Code 指纹继续消费实际生成记录。删除无生产调用的旧 single-Cone 分级资格流水线；不以另一套 `Verified` 包装保留这些入口。当前完整产物的投影直接采用已有生产计划与最终回填结果，不深比较它们的副本来证明同源。

同一次对象生产中的定义、digest patch、stackmap 与各类 registration 处理共享最初解析的 section、符号及 relocation 信息，直接借用原对象字节；不在每个步骤重新计算整包 SHA-256 来证明字节未变化。实际 digest 回填使用已物化的槽位，保留当前写入的范围检查；不再次比对六类 registration 的计划／对象副本，不重数 coverage，不重建最终记录与刚写入的字节逐项比较，也不重复验证先前零槽状态。callable／safepoint registration 直接关联计划与实际 digest 槽位，复用规范化 stackmap 和 canonical ABI，不重新编码记录、核对 context keys 或复查 entry relocation。body 指纹按已解析的 primary／associated atom、实际 relocation 和该 body 的 stackmap 输入计算；runtime scan 同属 associated atom，不从计划重新生成 scan 字节或遍历闭合证明。归一化地址字段不重复比较已经读取的 encoded value。runtime 解析实际 stackmap 建立 GC 索引，不重新编码和计算 normalized stackmap fingerprint；重复 ODR 记录按实际 PC 与 payload 合并。image 与 root entry 直接关联生成计划和已物化 patch；不重新读取 coordinate、诊断文字、registration 指针表或 root 记录来证明与计划相同。entry 的 Library／Executable 分支与可执行计划、实际槽位由同一枚举表示。RuntimeImage 与 root entry 沿用同一规则，完成后更新供产物指纹消费的内容摘要，不复制并清零全部对象、不重新解析未变化的对象结构。首次对象格式检查、实际 definition/body 指纹及运行时记录编码继续保留。

immortal object 的登记身份与 ODR ABI 直接消费现有登记计划及 canonical shape ABI；RuntimeImage 编码同一计划中的对象大小、对齐与 type registration 引用。不为了取得发布资格再次读取 String payload、重建登记记录、核对指针目标或比较 digest 依赖图。实际对象生成、普通符号引用与 ODR 选择继续使用既有数据。

静态存储登记直接关联计划和 scan／layout digest 槽位；按同一计划计算 layout 与 scan 指纹，并复用 canonical ODR ABI。发布不再次核对初值、scan 字节、空哨兵或所有物理指针，也不为 shape 计算保留物理验证资格链。

初始化登记直接关联计划和 Eager gateway 的 digest 槽位；Lazy 分支没有 gateway。gateway body、node 与指纹作为同一完整值保存，实际指纹按 body id 查询已计算的 callable 结果。发布不重读 cell、诊断文本和登记指针，不深比较 callable／initialization 的生产副本或重放 digest 图。

Type registration 直接关联 descriptor／layout 槽位。descriptor 指纹按已解析 atom 范围读取实际 descriptor、诊断数据及 itable directory，并使用现有 typed 语义归一化指针；layout 指纹消费现有 instance shape。保留读取范围和缺失引用诊断，不再重建预期字节、重验 relocation 全集、对照计划副本或比较 undefined-requirement 闭包来取得计算资格。

digest 槽位直接保存 codegen 返回的 intent、member 和对象偏移，宽度固定为 32 bytes。集合按 intent 建立查询索引；不重新验证 digest plan、重数完整覆盖、读回零槽或扫描重定位来证明物化正确。已不参与生产的 source／definition／atom／role 副本随资格包装删除；实际写入仍检查当前对象范围。

Code 指纹直接编码已有的定义、undefined requirements、native contracts、imports 与最终成员目录。计算和归档组装不重建这些集合来比较副本，也不再次比对所有组件的 producer／target 或复制的 relocation 闭包。已有结果直接进入 metadata 和内容摘要；当前编码、范围读取及真实链接引用错误正常处理。

generated-C 的 external requirements 直接复用实际符号分类；target helper 从当前工具链 registry 取得合同。bridge 计划只用于区分自身 source extern 与编译器辅助调用的同名符号，不重建 definition／atom 集合、扫描必需调用覆盖或证明机器 relocation 与模板相同。Scoop LIR 自身产生的 memcpy、TLS 等正常辅助引用使用同一 registry。未知引用按普通链接需求报错。

undefined requirements 完成普通分类后直接合并、排序，保留 callable／shape 的实际依赖分区。finalizer 不重建全部 relocation uses 做 coverage／重叠证明，也不比较 current／external／shape 闭包副本；查找真实 import 时保留索引边界错误。完成分类的外部需求使用同一数据类型，不再套封口资格包装。

物理 definition 读取只查询实际需要的符号和 atom 边界，建立供地址、relocation 和内容指纹使用的索引。保留缺失符号、歧义定义和本次读取范围的诊断；不比较所有生成符号与计划的全集、重验既定 linkage／primary 位置，也不扫描未归属 section 或 padding 来证明零值及生产覆盖。padding 不属于 definition 的语义内容，不再保存无消费者的 padding-end 资格字段。

Runtime metadata 发射直接使用完整 LIR 计划及当前 LLVM module 中的声明，按实际引用查询缺失符号并生成记录、scan、初始化数据及 digest 槽位；不先完整比较 global／function 的类型、地址空间、可变性、linkage、对齐、初值或 producer 副本，不以再次发射同一记录作为资格验收。digest 位置使用 canonical definition 的 owner／primary 与固定 32-byte 宽度，不重放计划全集以证明 coverage。真实 LLVM 发射、物理边界定位和 ABI／GC 输出回归继续保留。 对象物化仅定位所需 symbol／section 并写入实际边界与 linkage；摘要槽保留本次写入的范围计算，不读回零值或重验槽位全集。已由 LLVM 发射的 global alias 不再重放边界计划，callable 不扫描其他 backend 段证明覆盖；generated-C 使用真实主符号及其 extent，不复查 marker 模板和 plan 副本。 shape 和 callable scan 发射复用已有的 typed identity 与 LLVM global，不重新构造其 initializer 文本、比较同 id 的 payload 副本，或统计 layout／scan／dispatch 与计划的全集。实际所需的定义、scan、Context cell 和边界仍按 typed id 查询，生成 ABI 数据及处理本次缺失引用。


Mach-O／ELF 对象按实际目标格式读取 section、符号及 relocation，按标志与本次使用的特殊段分类；Scoop LIR 与 generated-C 使用同一规则，不设来源专属段名／flags 白名单或必须含 text 的资格。版本、优化提示和其他未消费的 load command、section group 不另行审计或保存证明副本；不比较整张物理表的覆盖、分区、padding 与重叠。格式库负责读取所需表，实际地址／索引／内容访问保留局部边界错误。 符号属性按实际位读取，不以描述标志组合或未消费的符号类型拒绝整个对象；relocation 仅解析实际使用的 symbol index，不要求 undefined 符号表每项均有使用，也不以重复名称预审替代本次引用解析。


### 2.6 `.slib` 格式与指纹

`.slib` 是 target-specific、bitwise reproducible 的 normal、self-contained ar archive。容器 schema、manifest schema、persistent identity schema 与 mangling schema 的格式版本为 1；HIR/MIR/LIR metadata outer schema 为 2。它不允许 thin archive、外部成员引用或相邻文件 payload。

归档 magic 为 `!<arch>\n`，首项固定为 `manifest.cbor`。其余 payload 按 manifest 中的 SlibMemberId bytes 严格排序，物理短名为从 0 开始的 ordinal：`m` 加 8 位零填充十进制。header 使用 SysV/GNU short-name encoding：名称后为 `/` 与空格填充，mtime/uid/gid 为十进制 0，mode 为八进制 `100644`，size 为无多余前导零的十进制，trailer 为 `` `\n``，奇数 payload 后补 `0x0A`。

writer 按上述目录与 header 规则生成归档。普通 reader 根据目录按需定位 section，不重读全部成员来验证归档生成过程或对照内容 hash；payload 不因自身 magic 而递归作为归档解释。实际解码与 I/O 错误照常传播，长度转换和索引保持内存安全。

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

所有 wire metadata 使用 RFC 8949 deterministic CBOR 的明确 schema：最短整数与长度编码、定长容器、严格有序且无重复的整数 field tags、声明的封闭 variant、完整 typed 引用。不能序列化 Rust discriminant、arena index 或 hash-map 遍历顺序。消费者按所需 schema 解码，传播解码库自然返回的错误，不增加 canonical 重编码比较、生产语义 replay 或损坏检测合同。

解码入口只要求解码能力，成功消费当前记录后直接返回。最短编码、字段顺序、截断与尾随输入由当前字段或记录的读取处理；不分配第二份完整编码来比较，也不为这种比较设置输出长度额度。

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

未知 optional capability 按目录跳过，不读取或计算其内容 hash，不进入语义 view。ExtensionBlob.required_for 只允许 0 或 Link bit；未知 Link-required capability 或 LinkObject verifier 阻止 Link 消费。blob 不成为第二种对象入口，已知 Link contribution 只能表示规范化 native library requirements，不能注入 raw linker argv、脚本或未检查对象。

完整生产 profile 为 `org.scoop-lang.slib-profile/cross-cone-generic/5`。descriptor 是五字段 map：`1=id, 2=required_manifest, 3=required_hir, 4=required_mir, 5=required_lir`；各列表按 capability 排序。其必需 section 为：

早期 `single-cone-strong` 与 `cross-cone-semantics-strong` profile 不再是生产或消费入口，不随当前 section 版本维护正向 descriptor／fingerprint 矩阵。旧 profile 按不支持的产物 profile 报错，当前 profile 的 required inventory 和版本兼容检查保留。

| 位置 / namespace | section 与 major |
| --- | --- |
| Manifest / `org.scoop-lang.manifest` | `single-cone-production/8` |
| HIR / `org.scoop-lang.hir` | `identity-foundation/10`、`core-bootstrap-interface/16`、`cross-cone-interface/78`、`cross-cone-type-semantics/30` |
| MIR / `org.scoop-lang.mir` | `identity-foundation/8`、`core-bootstrap-bridge/2`、`cross-cone-param-free-bridge/2`、`cross-cone-type-bridge/19` |
| LIR / `org.scoop-lang.lir` | `identity-foundation/9`、`cross-cone-param-free-bridge/3`、`cross-cone-layout-abi/16`、`cone-production/12`、`link-identity-closure/16`、`link-support/1` |

各 section 按消费用途检查 required inventory。Compile 需要完整语言与相邻 IR 合同；Link 只消费 identity、ABI、对象、native、production 和链接支持数据，不为链接展开 HIR 模板。profile fingerprint 覆盖 descriptor 的实际内容。

Compile 的依赖读取直接采用保存的 defined symbol owners、ODR member ABI 目录和产物摘要；typed 引用解析到同一份已读 identity 数据。跨产物同名符号归属、ODR key 和 ABI 冲突仍在本次实际合并中处理。为下游编译取得这些数据不读取机器对象、不生成 final／provisional 副本、不重建注册记录或重算 Code／RuntimeImage 摘要，也不要求消费方提供 C bridge 工具配置。已编译产物的对象生成与注册内容由生产阶段负责，摘要统一使用 manifest 中后续实际消费的字段，不另建 Compile／Link 重复摘要，也不为无消费方的对象数量扫描目录。测试不再以改写这些字节后被 Compile 入口拒绝作为验收。

依赖文件通过同一普通 artifact 入口保持文件句柄、一次解析的 manifest 和成员目录。目录给出成员偏移；仅在请求某个 metadata／对象成员时读取该成员，已读成员复用同一份缓冲区。内存产物与文件产物使用同一目录偏移与成员头解析规则，不为未使用成员扫描内容或逐项复核归档。Compile 加载实际所需的 HIR／MIR／LIR metadata，不读机器对象；文件打开、seek、read 的实际错误正常传播，不比较读取前后的文件身份、长度或时间，也不要求文件被替换或删除后继续成功。

普通构建后的 ProgramLink、显式 `scoop link` 和独立链接器都使用该文件入口。parent 的构建结果只保留路径与摘要，在真正链接时首次打开文件；显式链接的发现与成员消费共享文件句柄和已解析 manifest，不捕获整包输入快照或重算内容摘要。同一 Cone 的重复候选按已有 Artifact 指纹区分结果，不比较整包字节；本次依赖、符号和 ODR 选择继续使用实际数据。

manifest 保存 canonical coordinate、ConeIdentity、kind/source form、完整 exact direct dependencies 与三层语义指纹、language/runtime/identity/mangling ABI、target/backend 配置及其兼容摘要、全部成员目录、三层/Code/RuntimeImage/Artifact fingerprints 和 required sections。

成员内容、三层语义与 Artifact 摘要在生产这些结果时计算并保存。普通 reader 直接读取已保存的值；整包字节容器不计算另一份摘要，目录读取不逐成员哈希，Compile／Link section 解码也不重新计算三层语义摘要或重新编码 manifest 以复核 Artifact 值。格式解析、长度／范围、typed 引用与所选 target／ABI 配置检查仍在实际使用边界完成。该调整不改变 wire 字段、生产端摘要公式或正常输入变化的失效规则，也不新增另一个带复核的 reader。

依赖发现只读取归档头和首个 manifest 成员，取得 Cone、依赖、构建摘要与测试目录，不扫描后续成员头或读取其内容。字节输入与文件输入共用该摘要读取逻辑。准备 prebuilt 输入直接传递发现时取得的摘要和实际路径，不再次捕获完整归档、重算摘要或比较发现前后状态；实际编译／链接需要成员时再读取，普通 I/O 错误照常传播。

显式 artifact、artifact 搜索目录及默认 core 产物的发现共用 `artifact-summaries-v1` 普通摘要记录。按选中实际路径、mtime、大小和所选 target／backend 配置定位；未变化时直接读取已保存的摘要，不打开原归档或重新读取 manifest。路径／输入 metadata 或配置变化后按上述最小读取取得新摘要；普通锁协调同键首次读取，成功后原子发布记录。缺失实际输入仍报告输入错误，记录读取失败正常传播，不扫描或恢复损坏记录。该记录不改变语言身份、语义摘要或源码依赖的失效规则。

production 与 Link metadata 完整保存实际 body/type/site ID、Strong/ODR 定义、shared ABI、object/bridge unit 到 member/range 的映射、digest patch 位置、runtime registrations、image/root entry 归属、defined symbol owners、undefined requirements、native contracts 与逻辑 library requirements。所有表具有 canonical key/payload、明确排序与唯一性；不保存 producer 的绝对搜索路径。

executable entry 保存完整签名以及封闭的声明分支：生产入口引用源码 PersistentFunctionId，测试入口引用 PersistentGeneratedCallableId。测试入口的 generated key 由当前 ConeIdentity 与 TestEntry role 确定，不生成伪造的源码 main identity；两者均通过正常 Strong callable body、root gateway 和 failure-root 链接。HIR 完成入口选择并检查签名，后续 stage 传递实际声明、body 与签名，不重新按名称选择入口。

前端以独立的 TestEntry 函数身份合成测试入口，正文为普通非泛型、非 suspend 的 Scoop HIR。该函数可以作为测试初始化与方法调用适配的词法根；各适配保留所对应测试声明的源码文件与位置。具体化将入口及其闭包按既有 generated callable materialization 处理，不把生成函数加入源码函数、成员分派或生产 main 的候选集合。

M43 的 TestEntry generated key 使用 tag 17，HIR／MIR `identity-foundation` 分别升至 9／7；入口声明改为 Source／Generated 封闭分支后，HIR `core-bootstrap-interface` 升至 15、MIR `core-bootstrap-bridge` 升至 2、LIR `cone-production` 升至 12，仍使用 Strong profile 的 `strong-production` 升至 22。旧 section 需重建；其余未改变编码的 section 与 runtime ABI 保持原版本。

测试标记按语言规范 16.3 在前端使用已绑定的 annotation identity 和源码角色检查。Target、重复应用与参数复用普通注解规则；容器、方法角色、签名及同类 hook 数量由当前 Cone 的声明关系决定，不依赖名称或运行期扫描。后续 stage 消费已检查的调用与入口，不重做测试声明语义检查。

Export HIR 的当前目标测试目录按语言规范 16.4 保留 typed class／constructor／function 引用、声明位置和忽略原因，不作为依赖的测试发现接口。构造重载选择与普通调用共享同一候选决议；默认实参的代码生成在入口合成的实际调用处完成，目录发现本身不创建可执行的初始化副作用。

测试用途产物在可选 manifest section `org.scoop-lang.manifest/test-directory/1` 中保存该目录的展示投影：容器限定名、用例方法名与完整显示名、各自的 Cone-relative 源码路径及一基行列、忽略原因。目录按前端顺序嵌套保存容器与用例，不通过解析显示名恢复关系；可执行调用仍使用 HIR 中的 typed 引用。该 section 不参与 Compile／Link 的语义要求，受普通 ArtifactFingerprint 覆盖；生产产物不写入，测试产物即使无容器也写入空目录。manifest summary 在读取边界解码并复用该投影，缓存命中不重新发现测试或加载完整 HIR。

入口的四种 ABI 形态、完整签名、root gateway 的实际 C ABI 与定义指纹随各层 typed entry 保留，并进入对应语义／ABI fingerprints 和承载它们的 section profile。Link 投影保留精确 entry、gateway 与 failure-root 引用，使 artifact-only 链接无需源码或名称推断。M34 的 runtime ABI contract 12、metadata ABI 8 同时进入兼容检查、runtime 与启动对象的构建输入；旧 ABI 的薄接口、物理签名、poll／card 合同及产物必须重建或拒绝，不能用缺省字段或 bitcast 混用。新 ABI 的 debug／release 仍互通；各批次的实际实现与 section 版本见 [M34 记录](../milestone34/PROGRESS.md)。

operator equals 的成员签名、operator 标记、已绑定调用及必要的派生正文使用既有 callable、模板与 exact-type metadata。`Equals<T>.equals` 的显式 conformance、接口 slot、实现和分派适配使用普通 interface metadata；该 slot 使用源码显式声明的 operator 实现，不按结构比较补齐接口，也不保存 Equals 专用的 core protocol 或条件接口规则。

自 `core-bootstrap-interface/14` 起删除旧 Equality protocol 字段；自 `cross-cone-interface/68` 与 `cross-cone-type-semantics/26` 起移除统一方案添加的派生比较接口槽位声明，恢复普通源码接口实现。旧版本产物必须重建，不能把旧 Equality slot 当作新接口成员或独立 operator 使用。既有结构 operator 的生成身份和单态化格式保持不变。M34 后续编码扩展版本为 `cross-cone-interface/69`、`cross-cone-type-semantics/27`。M35 普通 when 增加可缺省 subject、Case/Predicate/Always 条件和显式 Fallthrough 后，版本升级为 `cross-cone-interface/70`、`cross-cone-type-semantics/28`；随后显式 Nothing/Unreachable 表达式增加为 `cross-cone-interface/71`、`cross-cone-type-semantics/29`。旧 body 编码必须重建。注解参数完整 signature key 与静态数组值随后将 `cross-cone-interface` 升至 72；type semantics、MIR/LIR 与 runtime ABI 在这一批保持原版本。

派生 operator 使用既有 typed 生成身份和普通单态化／ODR 规则，所需的宿主、完整签名及字段调用在各自边界确定；没有源码声明的生成 callable 不伪装成 SourceFunctionId。导入的 operator 候选来自已保存的成员或派生签名；派生候选不伪造 Equals slot。artifact-only 链接只消费已闭合的实现，不重新扫描字段或选择重载；普通接口 adapter 引用实际声明或继承的实现，不因值支持结构比较而新增 itable 项。

接口方法与实际 metadata 内容的变化按既有 semantic／ABI fingerprints、section profiles 和缓存兼容规则演进。实现迁移时同步更新受影响的格式与 core 产物；M41 删除 core Equality/equalTo，改为普通 Equals/operator equals，旧依赖须按既有失效规则重建。不为库接口迁移增加 runtime 比较 ABI。

`single-cone-production/8` 只保存两个实际消费字段：field 11 为物理 ODR member 目录，field 14 为 runtime image records。field 1～10、12 和 13 不再写入；distribution、output、image owner、typed registration projection、OptimizationMode 副本、重复的 RuntimeImage／Code fingerprint、native contract／library／C++ 和 C bridge production 投影不再作为后续复核材料持久化。RuntimeImage 与 Code fingerprint 使用顶层 manifest 已保存的值，native requirements 使用 LIR foundation，入口使用实际语义与 production 数据。ODR directory 的 member 保存实际 role 与 OdrAbiFingerprint，field 4 保留不用；纯语义 member 不产生空物理条目。删除 production section 的 C bridge 与全量投影重建比较 API；对象生产阶段原有的 Code 输入与指纹公式不变。

`link-identity-closure/16` 只保存三个字段：field 1 为物理成员目录（Scoop member 保留 typed definition 分配，generated-C member 只保存 member id，不复制已无消费者的 unit 列表），field 4 为 defined symbol owners，field 5 为 undefined symbol uses。不再写入只服务消费者重放的 definition atom ranges、patch sites、对象目录副本及 image／entry owner 投影（原 field 2、3、6、7、8）。实际机器对象的生成、重定位和摘要填充仍使用生产端数据，不为生成此 section 再构造上述索引或对象投影。两个 section 的旧格式按版本规则重建；runtime ABI 不变。

`single-cone-production/8` 的 field 14 保存最终链接实际使用的 runtime image records，编码为 `{1=prefix bytes, 2=six record arrays}`。prefix 是运行时规范 2.8 所定义的 RuntimeAbiFingerprint、TargetProfileFingerprint、canonical Cone record 与 direct dependencies 的连续 runtime 编码；六组记录依次为 static storage、immortal object、initialization、type、safepoint、callable，每项为 `[ObjectDefinitionPlanId, canonical runtime record bytes]`。记录在生产时按运行时规范的表顺序编码一次。最终链接按实际选择的 definition 筛选记录，写入所选条数与保存的记录字节，计算最终 image fingerprint；不为此反向解析对象、重建注册证明或重算各记录的 definition／layout／stackmap 指纹。该字段直接参与最终输出，不是缓存复核材料；读取时解析 typed definition 引用，不与对象或另行重建的记录逐项比较。runtime ABI 与 fingerprint 编码保持不变；缺少该字段的旧 production section 按版本规则重建。

`cross-cone-link-closure` 与 `cross-cone-layout-link-closure` 不再作为 metadata section 生成、保存或消费。已保存的 ordinary／layout ABI imports、member、symbol 与 ODR 数据用于实际编译和链接，不另存语义 imports 副本、relocation use 副本、完整对象目录及 coverage digest，也不在读取或发布时重建并比较它们。生产端 Code contribution 直接使用既有的 ordinary 与 physical import 数据，沿用原 capability id 作为该指纹输入的字段名称，保持 Code 指纹公式；这些名称不再注册为 section contract。移除 section 同步改变 cross-Cone profile descriptor／fingerprint，旧 profile 按格式规则重建，不保留旧 reader。runtime ABI 不变。前批取消 native contract Code 副本后失去消费方的 projection reader 与全量比对 API 也一并退役；实际 native contract 编码与指纹输入保留。

`link-support/1` 为单字段 map `{1=runtime_data_aliases}`，把必要的 runtime 数据符号关联到实际定义；String alias 不创建额外 TD 或存储。

ProgramLink 直接读取保存的 member 到 typed definition 映射、defined symbol owners、undefined symbol uses、ODR ABI 与 native requirements，执行本次实际的对象选择和符号连接。它不再为消费已生成的 Scoop／generated-C 对象重建 producer partition、重定位、digest patch、stackmap、registration、final/provisional 对象副本、Code／RuntimeImage 指纹或 coverage 证明，也不再次读取 production section 来重算 Code。每个文件输入的 manifest、metadata 与按需成员共用同一个文件入口。显式依赖 locator 首次读取 manifest 摘要后保留该文件入口，并随候选与已完成构建结果共享；不得丢弃入口，再由 Compile／Link 打开同一文件重读 manifest。命中已保存的摘要记录时仍不打开产物，首次真正需要成员时才建立入口。选中的稳定对象文件已存在时直接传给 native linker，不读取对应归档成员；缺少该输出时才从同一入口读取该成员并保存一次。用户 native 对象仍按真实 native 符号选择的需要读取，链接失败照常报告。

ProgramLink 从已保存的 LIR 数据取得 definition 的 owner／role、image 与入口符号、六类注册的 definition／symbol，以及 native ABI 实际使用的标量／指针表示。它不重建整份 strong production、digest finalization、注册生产计划、shape support、descriptor／dispatch exports 或 physical imports，也不重新编码比较它们。runtime 数据别名直接读取其 typed owner；String 的布局合同在生产该别名时检查。链接所需的 native contract、ODR ABI、符号选择与 runtime image records 继续使用，当前符号连接错误正常报告。此读取简化不增加持久字段，不改变 wire/profile 或 runtime ABI。

Compile 为本次实际使用选择外部物理定义时，直接从 provider 的导出记录取得 callable ABI、layout／scan、descriptor、dispatch 和初始化／静态存储语义，并从已有 definition symbol plan 取得符号与 primary atom。不再为同一导出重建物理符号、扫描 atom 集合，或反查整份 type／callable 注册表重做布局、scan、继承与 dispatch slot 比对。provider 视图只借用实际需要的导出、definition symbol plan 和初始化／静态存储记录，不要求整份 strong production。缺失的实际 subject、错误的本地／外部引用及当前选择所需的类型与 ABI 规则仍在相应边界报告；导出生产时的完整性检查保持。

Compile reader 从 production 字段直接读取 definition symbol plan、静态存储语义、初始化语义及其 cell／registration 定义引用，解析本次消费所需的 typed 引用及当前依赖的初始化定义。只供对象生产使用的 digest node／patch、完整注册生产字段不进入 Compile 数据结构。它不重建整份 ConeProductionSection、digest graph、type／callable／safepoint／immortal 注册表、入口或 runtime image，也不再从 HIR 公开类型、Function 声明和 MIR 初始化使用重新证明这些已有记录。消费方不把读取结果重新编码后与原字节比较。已保存的 LIR dependency records 只解析 typed 引用，不从 HIR materialized roots 和注册表再次推导闭包并比较；实际外部导入选择继续使用这些记录。失去消费者的完整 V2 production／registration 重放、digest 重建及其独占测试一起退役。完整生产计划继续供对象发射使用；此次读取职责调整不改变持久字段、profile 或 runtime ABI。

Compile 读取已完成的 HIR 接口时，直接解析保存的 typed 声明、默认值、调用点与类型信息；不再为同一依赖重新验证声明形状、可见性、默认参数来源、const 求值、intrinsic 或调用点签名。别名展开与实际引用解析仍用于后续编译；当前源码的语言检查继续由 HIR lowering 完成。仅服务消费端复核的 authority 实现、改写候选后要求拒绝的测试和发布时语义重放一起删除。

Compile 读取 ordinary callable bridge 与 callable ABI 时，直接采用已保存的 ABI、调用协议和 typed 引用，不从 HIR／MIR 声明重新生成同一桥接表，不重做参数／返回值布局分类或逐项比较。读取 MIR dependency records 时只解析实际 typed 引用，不再从 HIR materialized type／shape／property／singleton uses 推导整份候选闭包并比较，也不重做已有 HIR call-site 与 MIR 调用清单证明。生产端源码检查、依赖选择及 ABI／GC 生成保持；只服务上述消费者复核的测试、目录与重放 API 一并退役。

Compile 读取 MIR 类型、callable、constructor、object、dispatch、coroutine 与 equality 记录时保留本层格式、typed 引用和必要签名关系，不重新遍历 HIR 声明／继承图推导同一清单和语义。初始化单元从已保存的 strong callable 记录按 unit／role 取得 initializer 与 ensure 及其签名，不重算 generated identity，也不与 HIR source-unit 清单再次证明；缺失调用对仍报告实际引用错误。对应 HIR→MIR 对照重放及改写候选后要求来源复核的专用测试退役，真实源码生产、产物读写和语言规则 fixture 保留。

callable ABI 的底层 reader 直接解析保存的 signature／protocol、body ID、definition plan 与 symbol。它用已有 typed body key 和 definition 记录解析实际引用关系，读取 primary atom，不调用生产构造器重新派生 body／definition ID，不重编码 signature 或整张 ABI 表再比较。保留调用协议与 GC effect、目标配置、provider、表顺序及实际引用错误；生产端 ABI 分类和 definition 生成照常。只供候选与重建期望对照的单条／表级 ABI API 和测试一起退役。

Compile 的 dispatch 导出同样直接读取保存的 table、owner、role、slot signature、implementation 和 ABI body 引用，不从 MIR schema 重新生成表和 ABI 绑定，不把解码后的表重编码成另一份期望。reference-dispatch 的 receiver layout 从已读取的本地／依赖 layout 导出中按 typed exact 引用取得，不重算布局。表顺序、entry position、typed 引用和 definition 关系保留；生产端 slot／receiver／ABI 检查继续执行。

Compile 的 param-free shape-support 从保存的八种 role 读取 source、exact、layout／scan／descriptor／registration 与 box／coroutine helper 引用。它不从 MIR root 清单或源码 nominal 重新生成相同闭包，不为同一 role 重编码比较。source 与 descriptor 的必要角色、typed 引用及 canonical source 顺序在读取边界确定；实际布局、helper 生产和物理导入仍使用各自的数据与规则。专供重建期望的 shape-support reader API、候选修改工厂及重复表构造器一并删除。

Compile 的 layout／descriptor 导出直接读取已保存的值存储、字段偏移、enum 区域、class 完整字段和实例形状，以及 descriptor 的布局、继承、分派、注册和诊断名称引用。字段、MaybeUninit payload、box payload 和数组元素在表示中保存自己的 typed layout key 与完整存储事实，不要求这些内嵌值另外拥有物理布局导出或 symbol。reader 解析 key 中的类型引用并读取存储，不从 MIR 重新放置字段、重建继承布局、推导 GC／niche 或重生成 descriptor。保留实际格式、引用、目标配置及内存范围检查；不再次证明生产端语义。只为重建期望或修改候选后复核来源而存在的重放模块、API 和测试一并退役。生产端布局算法与 runtime ABI 保持；上述内嵌存储编码随本批 `cross-cone-layout-abi/16` 发布。

物理 shape import 只保存 provider、typed subject、symbol 与 definition 引用，不再复制整份 ABI／layout／scan／descriptor／dispatch／storage／initialization 语义合同。消费时从已读取的选中 provider 取得实际合同并连接保存的 symbol／definition；不重编码比较合同副本。删除对应合同副本 reader、逐字段比较 API 与独占改写测试。此字段删除将 `cross-cone-layout-abi` 升至 16，旧版本产物按普通 profile 兼容规则重建；Code contribution 继续编码实际内存合同，runtime ABI 不变。

归档 writer 接收同一次对象生产得到的 member 与 Code 数据后，不再逐 member 重新计算 fingerprint 与生产目录比较。member 的普通格式检查和实际归档编码保留。defined symbol owners、undefined symbol uses、ODR directory 的 consumer 已使用保存数据后，失去用途的“与重建结果比较”API 及其专用测试也一并移除。

HIR／MIR／LIR foundation 读取复用 identity 边界已解析的 typed records，保留当前实际使用的引用和结构检查；不得为证明解码前后相同而完整编码原始 foundation、再编码重建对象并逐字节比较。仅服务这一深比较的错误分支和候选顺序改写测试退役。

旧 single-cone Link 的逐级验证资格 API、从 final 对象清零摘要再重建 provisional 的路径及其独占测试一并删除，不保留为备用 reader。仍被正常 Compile／ProgramLink 使用的 section 读取和真正生产对象的编码、重定位、摘要填充继续存在；归档格式测试使用正常产物，不能通过改坏最终对象来延续退役的消费复核合同。

旧 single-cone strong 与 cross-cone semantics strong Compile 的独立解码／验证／提交流水线及其专属测试退役，随之删除仅供这些入口使用的 native ABI 全量重放。正常源码依赖与当前产物统一由共有 layout reader 读取，不为历史阶段 profile 保留备用 Compile reader；归档／section 格式辅助数据和当前路径使用的基础引用检查继续保留。

persistent identity 只由首次引入的层声明一次，并同时保存 typed ID 与 canonical key；其他层引用它。相同 kind/ID 的冲突 payload 或跨层重复声明拒绝。LIR foundation 的 exact-type 集合引用已有 HIR/MIR exact records，不复制声明。源码文件、binder、字段、variant、body application 和调用位置的引用必须指向真实定义。

共有 HIR expression 为 `{1=kind, 2=result_type, 3=definition_origin, 4=evaluation_origin}`。Call 的 tag 为 57，payload 保存 callee、arguments、SourceCallReceiver；receiver 为 NoReceiver=1 或 Receiver=2（完整静态类型）。实际 call-site 保留完整逻辑实参与结果、定义／求值位置及原 receiver，Unit/ZST 参数不能按物理省略规则消失。

nominal declaration 保留原 binder 条件、GC-free pointee requirements 与 NoGC 标记。property/accessor、constructor safety/effect、InteriorMutable、CLayout、泛型 companion、release、annotation 和编码合成所需的声明事实均不可由缺字段的默认值替代。读取只检查格式、kind、引用与合同连接，不重新进行定义处的重载或可见性选择。

C extern 的 NativeSafe/GcLeaf 模式作为声明及调用的语义字段进入相应 HIR/MIR/LIR metadata、语义指纹和构建缓存。跨 Cone 编译与泛型展开保留原声明的模式；artifact-only program-link 消费已生成的调用，不能按合并后的 C symbol 统一提升或降级调用模式。该字段不改变 C bridge 的物理签名，也不要求为同一 native ABI 构造第二套桥接格式。

`captureErrno`、完整 Scoop 结果类型、native 返回投影及 bridge 结果适配进入对应声明、调用与 bridge 的 HIR/MIR/LIR metadata、语义/Code 指纹和缓存。编译消费方按已保存的结果适配生成 `(R, Int)`，链接消费方保留实际 bridge 及 native requirements；不按合并后的 native symbol 重新决定捕获，也不将旧的单结果 bridge 当作捕获 bridge。必需字段与 recipe key 的变化按既有 metadata/schema 兼容规则演进，旧产物不能缺字段后静默当作不捕获。

HIR `cross-cone-interface/69` 的 C extern implementation 必须保存调用模式与结果适配；LIR `identity-foundation/8` 的 OutboundFunction key 必须保存结果适配。generated-C 的 OutboundWrappers 模板版本为 2。目标工具链展开 `<errno.h>` 后产生的 libc errno accessor 引用作为普通 target-support native requirement 保留：Darwin 为 `__error`，GNU/musl 为 `__errno_location`，均为无参数、返回 native pointer 的 C 函数。源码生成仍只使用 `errno` 宏，不自行生成目标 accessor 调用。

DirectC/StorageBridge 及其完整物理调用计划进入相应 LIR metadata 与既有语义/Code 指纹和缓存投影，不加入 source native symbol 的 ABI 冲突键。DirectC 保留真实 native undefined reference、contract 与 library requirement；没有实际桥接用途时，不生成 outbound bridge recipe、物理定义或 member 要求。跨 Cone 与泛型消费按当前 target 得到同一完整计划；artifact-only 链接只消费产物记录，不重做 ABI lowering，也不为直接调用重新插入 bridge。

native-boundary record 保存 owner、type-parameter count、source shape 与 C ABI projection。projection 为 SourceRepresentation=1、UInt64Field=2（唯一 field ID）或 NullablePointer=3（empty variant 与 pointer payload field ID）；必须匹配真实声明的字段、variant 与类型，不改变普通 Scoop aggregate/niche 表示。

三层语义指纹是 Merkle 指纹：各自 canonical own-layer projection 加真实 direct/support/re-export 依赖的同层指纹。其 domains 为 `scoop-hir-semantic-v1`、`scoop-mir-semantic-v1`、`scoop-lir-semantic-v2`。LIR context 包含 identity ABI、target profile 与 target fingerprint，不含 backend optimization、member assignment、range 或 patch offset；backend 兼容性仍单独检查。

HIR 语义包括名称候选、签名、默认值、const、模板和完整依赖；无法完整表达细粒度 lookup observations 时，保守计入全部相关直接依赖。普通私有非泛型正文、机器码、优化模式与对象分组进入 Code/构建缓存，不能反向改变声明或 shared ABI 身份。

MIR `identity-foundation/6` 与 LIR foundation 保存实际实现的实体目录，包含优化新建的局部值；它们进入 Code 与 Link validation，不作为依赖语义贡献。MIR 的可消费语义由 bootstrap／param-free／type bridge 保存，LIR 的 shared ABI 由对应 layout／param-free section 保存。内联新增局部值不能使同一源码的 debug／release 依赖互相过期；完整 identity、格式、引用与 ABI 校验仍在各自边界执行。

`MemberFingerprint = H("scoop-slib-member-content-v1", member_record)`；`LinkMemberFingerprint = H("scoop-slib-link-member-v1", member_record)`，后者只用于 LinkObject 或 Link-required blob。

`CodeFingerprint` 使用 `scoop-code-v1`，覆盖按 member ID 排序的 Link fingerprints、已知 Link contributions、generated-C production、native library requirements、defined/undefined symbol 集合、去除诊断位置的 native contracts，以及 section registry 声明的 Code projections。只改变实际合同或符号需求也必须使 Code 改变；diagnostic/unknown optional payload 不进入 Code。

`ArtifactFingerprint = SHA-256(ByteSpan("scoop-artifact-v1") || ByteSpan(canonical_manifest_without_artifact_fingerprint) || ByteSpan(MemberFingerprint_0) || ... )`，成员顺序为目录顺序。它覆盖全部附件与 blob；manifest 不包含自身 member hash。RuntimeImage 编码及 body/stackmap domains 见运行时规范 2.8。

产物数据按本次 Graph、Compile、Link 的实际需要读取，不以验证资格转换表达阶段关系。一个消费进程复用同一按需入口；同一产物的同一 section 首次需要时解码一次，后续共享或借用结果。发布直接使用已生成的 IR、对象与摘要，不重新打开自身归档，不由 parent 重做 child 的正确性检查。显式 locator 与缓存共用普通读取入口，不维护另一套不可信产物 reader；更新判定只需路径和记录时不打开归档。

### 2.7 `scoop` 与 single-Cone `scoopc`

`scoop` 负责 root input、locator、完整依赖图、构建顺序、全局缓存、runtime 构建和程序链接；`scoopc` 一次只编译指定的一个 Cone，不跟随上游 manifest locator 或读取上游源码。

```text
scoop build [root-input]
scoop run [root-input] [-- program-arg...]
scoop link --root-slib root.slib --runtime-objects index [--dependency-slib dep.slib]... [-o output]
scoopc build <Cone-root-or-Cone.toml-or-file.scoop> [--direct-slib direct.slib]... [--support-slib support.slib]... --out-slib current.slib
```

低层 `scoop-link` 使用相同的普通产物链接合同。build/run 省略 root 时使用调用者 cwd 的 Cone.toml，不向父目录搜索；`run -- args` 也使用该默认 root。低层 scoopc 的输入必需。

输入为两种封闭形式：

- ManifestCone：目录下必须有 Cone.toml，或显式 regular file 恰名为 Cone.toml。未出现顶层 `sources` 字段时递归扫描 `src/`；出现后按语言规范 12.2.2 的完整清单选择当前 target 的源码，不追加默认目录。目录只收集扩展名精确为 `.scoop` 的 regular files，也可显式选择单个 `.scoop` 文件；最终按标准化 Cone-relative UTF-8 路径排序。
- SingleFile：operand 扩展名精确为 `.scoop`，解析 symlink 后为存在的 regular file。只编译该文件，logical path 固定 main.scoop；不读取相邻 manifest、Scoop/C/C++ source 或 blob。

manifest 字段、路径形式及 `when` 的合法性先于 target 筛选检查；文件系统检查和递归枚举只作用于选中路径。选中目录相同或互相包含、单文件与目录重叠、重复标准化路径或 symlink 重复指向同一源文件时，报告来源条目，不静默去重。选中路径缺失或类型不符、最终空集合、非 UTF-8 或实际 I/O／目录遍历循环错误均在 parse 前失败；显式清单为空或全部被筛掉也不回退到 `src/`。

更新判定只发现选中文件集合及其修改时间、大小和逻辑名，不先读源码内容。`sources` 的语义投影区分默认与显式模式，规范化路径、条目顺序和条件的等价写法；未选中文件的内容与原始 span 不使编译失效。需要编译时，parent 传递实际文件位置与选中清单，scoopc 读取本次所需源码；不创建、封存完整输入镜像，不检查读取前后 inode、owner、权限或文件替换。`scoop` 与直接调用 `scoopc` 使用同一发现规则，向 parser 交付相同的完整集合；后续阶段不能再追加默认源码。`SourceIdentity`、file-private 实体及 SourceLocation 使用从 Cone 根开始的完整路径，选择条目的顺序和分组不进入身份；改变源码集合只通过既有源码内容、依赖和指纹规则影响产物。

SingleFile 的 coordinate 为保留值 `scoop:single-file:0.0.0`，kind 为 Executable，只依赖 core；用户 manifest 不能声明该 coordinate。build/run 此时拒绝 cone-path，scoopc 拒绝 direct/support slib 参数。其 `.slib` 可作为本次构建、缓存或显式 link 的 executable root，不能作为依赖或 manifest locator 指向的可分发 Cone。

测试配置遵守语言规范 12.2.4。manifest 解码保留独立的测试源码清单、测试依赖及其 locator/span；生产语义投影仅含生产字段。普通 build/run 与生产用途的 scoopc 不发现测试文件或展开测试依赖，缓存键不包含测试配置或测试文件摘要。manifest 未变化时使用记录的解析结果；变化时解析一次并按有效配置判断相应用途是否更新。原始文本供实际需要的诊断与编译使用，文本变化本身不等于生产语义变化。父进程的根、显式源码依赖与默认标准库使用同一 `manifests-v2` 解析记录：按实际 manifest 路径、mtime、大小和解析器 target catalog 判断更新，保存已解析的生产／测试／native 配置、locator、诊断区间及对应文本。命中不打开原 manifest 或重跑 TOML parser；缺失输入仍先报告普通定位错误。记录格式或解析规则改变时升级 namespace，不迁移旧记录或重放语言规则。实际编译的 scoopc 仍读取本次自己的 manifest 输入。

single-Cone 编译请求显式携带 Production 或 Test 用途，默认 Production；直接调用 `scoopc build --test` 选择 Test，且输入必须是 manifest Cone。测试用途在解析源码前将 manifest 的生产依赖与测试依赖并为所需 direct artifact 集合，support 仍恰为其余传递闭包；scoopc 不定位或补建缺失的测试依赖。解析时使用同一 manifest 的生产／测试源码选择规则，将仅用于测试的依赖身份交给前端，并以原 ConeIdentity 产生具有 TestEntry 的 executable `.slib`，不要求改写源码 manifest kind。该字段自 machine protocol 6 加入；当前协议版本见下文，旧版本拒绝协商；普通生产请求的用途必须显式编码，不能靠源码目录或输出名推断。

`scoop test [root-input]` 使用同一构建图，只对 manifest 根 Cone 启用 Test 用途：发现根的生产与测试依赖，依赖节点仍按 Production 构建，不递归启用依赖自身的测试。根按 executable 产物规划和链接，保留源码 manifest 及 ConeIdentity。测试编译键包含用途、每个源码的角色、测试源码选择与测试依赖语义；本机源码实际路径、修改时间和大小参与更新判定，诊断显示 locator 与 span 不进入键；生产键不包含这些测试配置。测试源码或配置改变不使未改变的生产依赖缓存失效。

测试程序发布到 `target/<canonical-target-triple>/<profile>/tests/<cone-name>`，`--target-dir` 替换 target 根目录，`--no-run` 完成构建和链接后返回。运行直接使用本次成功构建确定的已有或新生成 executable，不制作 execution copy；失败构建不得运行旧文件。过滤和忽略选择不改变测试编译产物；最终报告需要独立的用例结果通道，不能仅凭进程退出码判定整轮执行完成。

`--list` 完成更新判定后从已保存的测试目录输出容器与用例，不启动测试程序，与 `--no-run` 互斥。human 列表写 stdout，JSON 模式沿用 stderr 的结构化记录流，构建记录之后输出 `test-list` 记录；记录包含嵌套容器、用例位置与忽略原因，以及容器、用例和忽略计数。

`--filter text` 对完整显示名进行大小写敏感的子串匹配；默认选择未忽略用例，`--ignored` 改为只选择被忽略用例。列表与执行采用同一选择函数，移除没有选中用例的容器；空选择区分无测试、无匹配和忽略策略下无用例。`--no-run` 仍构建完整程序。前端为全部用例（含 Ignore）生成适配，并在实例初始化之前用普通库选择调用包围完整生命周期；入口参数沿既有 Array<String> main ABI 传递，无需新增 runtime ABI 或改变测试编译键。

每次实际测试运行创建独立的临时运行目录，以 `SCOOP_TEST_RUN_DIR` 交给普通测试库。库向其中的 events.jsonl 写入并刷新 begin（目录内用例序号）、end（同一序号、纳秒耗时和失败数组）及 complete 记录。父命令只按该独立通道生成测试状态，不从用户 stdout/stderr 识别事件；读取时检查实际选择顺序、开始／结束配对和唯一完成记录。缺少完整记录时，即使进程退出 0 也失败；shutdown 的非零退出或信号不能被已有 complete 掩盖。已开始但未结束的用例失败，剩余未开始用例为未运行；启动或收尾失败记录到目标。空选择直接报告 0 用例与原因，不启动程序。

默认将目标 stdout/stderr 写入独立文件；测试库在 begin 后刷新 stdio、保存并重定向 fd 1／2 到本次用例的文件，在生命周期结束后刷新并恢复，再写 end。文件按开始顺序命名，不从显示名构造路径；后台工作应在用例结束前清理，异步输出仍遵守既有线程／stdio 规则。`--nocapture` 保持继承的 stdout/stderr，不重定向，并与 --list／--no-run 互斥。捕获包含实例初始化到清理的全程；失败与中断用例的输出、目标输出分别报告，普通通过用例的输出默认不展示。用例中提前退出时，尚未恢复的 stdio 输出随该未结束用例保留，包括此时运行的 native atexit 输出；不能根据用户输出内容猜测阶段。JSON 报告中的输出以 UTF-8 text 或非 UTF-8 hex 保存，不有损转换。运行目录在报告后清理，不在缓存中持久化运行结果。

测试源码发现复用相同的路径选择、平台筛选和文件读取规则，在合并前保留生产集合的非空要求，在合并时检查两种角色之间的路径重叠及物理文件重复。源码角色是编译输入属性，不参与 SourceIdentity；parser 将角色随每个已解析文件交给前端，不能由目录名、文件顺序或显示路径重新推断。成功结果保留按完整 source identity 排序的全部文件及其角色，解析错误仍按原文件定位。

前端输入显式标明仅用于测试的 direct dependency 身份，在同一已加载的依赖语义数据上按源码角色收集名称候选。package binding 在按实体合并前筛选直接来源，qualified-type 索引保留来源贡献以复用相同数据；选中真实 nominal 后的静态成员遍历不按原始声明 Cone 再次排除实体。不为生产与测试分别加载或完整校验相同的依赖闭包，不在下游 IR 重做定义处名称查找。

依赖使用 exact version，全部为 library，同一 group:name 只允许一个版本。构建图解析实际依赖、版本、target/ABI/schema 与 locator，诊断循环和歧义；不为复用已完成节点重读其产物或重算语义／内容摘要。canonical 顺序逐次从“全部依赖已完成”的 ready set 中取 coordinate bytes 最小者；不按输入顺序或并发完成顺序决定。

非 core Cone 未显式声明 core 时，注入 `scoop:scoop.core:0.1.0` direct edge；core 不依赖自身。core 是普通库，显式 locator 遵守同一发现规则。缺省 sysroot 来源先取 `lib/scoop.core`；只有目录不存在时才取 `artifacts/<canonical-target-id>/scoop.core.slib`。已选来源损坏或不兼容时直接报错，不回退到另一来源。

显式声明 core 版本但未指定 locator 时，复用图中已选 core；尚未选定时先查显式 artifact search root。搜索无候选不立即报缺失，待其他声明的源码／产物依赖展开完毕后，若仍无 core 才使用上述 sysroot 缺省来源。这使普通库的显式 core 依赖与隐式 core 边绑定同一实际库，也允许后续发现的依赖提供显式 core locator；无效候选或显式 locator 错误仍直接报告。

其他已声明的 `scoop` group 依赖在没有显式 locator、且全部显式 artifact search root 均无候选时，最后检查 `<sysroot>/lib/<name>/Cone.toml`。匹配完整 coordinate 和 library kind 后，按普通 source Cone 递归构建并使用普通缓存；不要求额外的 cone-path。新发现的默认源码 Cone 的显式依赖继续优先展开，直至普通依赖工作队列为空；缺失或损坏的默认源码按普通 source locator 报错。其他 group 不查该默认位置；显式来源失败、候选歧义或已选 artifact 不兼容时不回退。它只补充源码依赖定位，不注入依赖或扩大可见性；artifact-only link 仍不读取 sysroot 源码或补建缺失 artifact。

配套 scoopc 默认来自 scoop executable 同目录，允许显式指定。首次使用或工具路径、修改时间、大小及配置改变时取得其能力／协议；未变化时复用记录，不启动探测、不完整读取程序计算指纹，不在执行前后重捕获工具。本机工具记录保存在 `tools-v1`，保存路径、mtime、长度、生效的配套协议配置和实际能力响应；工具输入摘要由这组普通输入生成。compile key domain 接续为 `scoop-cone-compile-cache-v7`，使用独立的 C 工具输入与 native 对象构建输入摘要，不改变语言实体或 runtime ABI。每个 source cache miss 启动一次 single-Cone 编译，传入实际 direct/support artifact 路径；direct 对应 manifest edge，support 为其余传递依赖，两组互斥。工具启动、协议读写和实际编译失败正常报告，缓存复用不重放 child 成功产物及依赖闭包的完整验证。

C 工具链的版本、SDK／libc 配置和实际调用参数保存在 `native-tools-v1` 普通记录。更新依据为请求、影响工具选择的配置以及实际程序、探测所用头文件、specs 和相关搜索目录的路径／mtime／大小；命中不启动 xcrun、版本查询、预处理或探测编译。Darwin 记录选中的 developer／SDK 路径、SDK 版本和 deployment；Linux 记录 GCC 版本及实际预处理宏配置，不读取整个 GCC、cc1 或 assembler 内容来构造身份。Linux CBridge 合同中的 compiler inputs 摘要表示探测得到的编译配置；本机路径和文件 metadata 只进入构建更新键，不进入产物语义指纹。首次探测仍检查实际目标／libc 能力；失败不发布记录，普通输入缺失和工具执行失败正常报告。

C++ 配套 driver、标准库配置、宏及搜索输入保存在 `cxx-tools-v1`；只在实际 C++ 请求和相关输入变化时发现，不完整读取工具或标准库二进制来计算更新键。C++ 与 native 的本机输入 metadata 仅用于本机构建更新，不进入语言身份或 runtime ABI。工具版本与目标名称的文本不作为配对资格，不通过比较 `-dumpfullversion`／`-dumpmachine` 的字符串拒绝可用的编译器。

最终链接工具使用 `final-tools-v1` 普通发现记录，按已选 C 调用配置、最终链接选项（含实际生效的 unwind prefix）及 C++ 需求定位；保存实际 linker、版本、运行库选择、系统输入摘要、探测头文件与搜索候选，Darwin 同时保存已解析的 SDK 导出目录。输入未变化时直接复用发现结果，即使 Scoop 源码变化需要重新链接，也不重复 xcrun／版本／搜索目录查询、探测编译、探测链接或读取系统库内容。实际工具、SDK／系统输入、搜索候选或配置变化时重新发现；普通锁协调同键请求，失败不发布记录。SDK stub 的摘要和长度在首次读取时保存，不保留供身份重算或输入复制使用的完整原始字节；首次工具探测直接使用选中的 SDK 文件，实际程序链接仍按本次符号选择生成必要 stub。

`scoop link` 与独立 `scoop-link` 同样复用 C／C++／最终链接工具发现记录；两者支持 `--cache-dir`，并沿用普通构建的 `SCOOP_CACHE_DIR` 与平台默认缓存位置。显式 artifact 输入的读取和必要链接工作继续按实际请求执行。

native 预处理同时取得实际 include 名称及其所在文件，按该次 include 的名称、引号查找目录和配置搜索根记录候选；不把所有头文件的父目录互相交叉展开。预处理器输出的 include 观察指令不进入后续预处理单元编译，原有行号与诊断映射保持。

搜索目录不要求预先存在，不在预处理前 canonicalize 或核验每个 include 参数。缺失候选仍可作为普通更新观察，新增实际头文件后按原搜索顺序重新选择；实际文件读取失败由执行读取的工具报告。

用户 native 源码的普通对象结果保存在 `native-objects-v1`，记录实际选中的源码、上次预处理得到的头文件、include 搜索配置与相关候选路径、编译参数和 C／C++ 工具输入。记录命中且对象存在时直接返回对象路径，不预处理、不读头文件、不复制预处理结果或对象。更新时由构建编排调用 native 工具预处理并编译一次，全部成功后发布对象和记录；不同输入的对象使用不同位置。仅 Scoop 源码变化时复用 native 对象。更新判断观察实际头文件及可改变其选择的候选路径；不因同目录新增无关文件而重新预处理。parent 将这些对象交给 scoopc 打包，child 不再次发现 C++ 工具或编译同一 native 源码；独立 scoopc 仍从实际源码完成正常 native 编译。native 路径只用于输入选择与诊断，不比较 inode、审计符号链接来源或限制头文件的物理归属。头文件搜索和 forced include 接受含 `..` 的相对路径及绝对路径，使用普通 PathBuf 按 Cone 根解析，不套用 Scoop source identity 的 ConeRelativePath。

完成节点保存实际生成物位置和后续需要的摘要，不强制持有整份归档或多份验证结构。后续需要编译或链接时按需读取 section，命中库输出或最终 executable 时不读取产物。不同子进程各自首次读取所需依赖数据是正常工作，不增加驻留服务或共享内存协议来消除进程边界。

生产用途的 library 不要求 main；executable 必须恰有一个本 Cone 顶层 ordinary、非泛型、非 suspend 的 main，接受无参数或单个 `Array<String>` 参数，返回 Unit 或 Int，具体规则见语言规范 12.4.4。依赖的同名函数不参与入口选择，manifest 与 single-file mode 使用相同规则。测试用途仅适用于 manifest Cone，由前端选择本 Cone 的 generated TestEntry；其测试 `.slib` 是 executable root。scoopc 的正式输出始终是当前 `.slib`，不构建 runtime、不解析 native library 搜索目录、不调用最终 linker。

build/run 接受 `--profile debug|release`，默认 debug；`--release` 为 release 简写，与显式 profile 同时出现为参数错误。实际优化合同见 2.19。

默认 target root 为 manifest Cone root 下的 target，SingleFile 为调用者 cwd 下的 target。`--target-dir` 相对调用者 cwd 解析并替换该根；输出目录追加 `<canonical-target-triple>/<profile>`。library 文件名为 `<cone-name>.slib`，manifest executable 为 `<cone-name>`，single-file 为 `<sanitized-input-stem>`。build 的 `-o` 覆盖最终 root 文件的完整路径，不追加 triple/profile。

新结果在临时输出位置生成，工具成功后 rename 发布，再更新构建记录；失败不发布部分结果。普通构建锁协调相同结果的生产，取得锁后重查记录即可复用，不比较两份完整产物。不同输入的结果不得覆盖正在使用的同一缓存输出。用户首次指定另一输出位置时可发布一次，此后命中不重读、哈希或覆盖已有文件。不进行写后读回、sync_all、目录同步、只读封存、私有目录或路径来源审计。

最终程序使用 `executables-v1` 普通构建记录：键采用当前图的 Code 结果、runtime 构建结果、target、优化模式、最终链接选项、library roots 和实际 Scoop／C 工具输入及生效调用配置。记录保存稳定 executable 路径、link plan 摘要，以及上次真实链接使用的工具、系统输入、库文件、脚本和会影响库选择的候选路径 metadata。缺失候选也参与正常更新判断；新增候选、symlink 选择变化或实际库更新时重新解析并链接。命中判断先于 Link 闭包读取、final 工具发现、startup／response file 生成和对象物化；不读取命中 executable 内容。所需 executable 缺失时更新，用户输出缺失或首次选择另一输出位置时只发布已有结果，不重新链接。编译或链接失败不发布新记录；同键构建使用普通锁，锁内重查。

用户 library／executable 输出共用按结果标识保存的发布记录，命中且文件存在时直接返回。发布通过临时文件与 rename 完成，只在需要新位置或新结果时复制一次；不检查输入／输出 inode、owner、符号链接来源或缓存目录归属，不制作 execution copy，也不同步落盘。run／test 使用本次结果的稳定缓存路径，argv[0] 保留用户输出位置；另一构建并发发布不同结果不会改变已选执行目标。普通文件创建、复制、rename 和工具执行错误照常报告。

run 直接执行本次成功构建确定的文件；同一请求构建失败时不得执行旧结果。不通过私有二进制副本防御外部文件替换。argv[0] 使用稳定输出路径，其余参数原样传递；带参数的 Scoop main 得到包括该第 0 项的完整数组，与 Java/Kotlin 从首个用户参数开始的数组不同。继承调用者 cwd、environment 和 stdio，保留操作系统实际报告的 exit/signal，不因源级 Int 超出 `0..255` 而拒绝运行，也不把 main 的非零返回值改写成 gateway 失败。工具输出在程序启动前完成，程序 stderr 不包装为诊断 JSON。

`scoop link` 接受显式 root/dependency artifacts、cone-path、必需 runtime object index、target、library search roots 与输出路径。它按 artifact exact dependencies 查找，core 也只从已有 artifact 取得；不启动 scoopc、不读取 runtime/Scoop 源码或补建缺失依赖。

`--sysroot` 定位 Scoop core 及上述 `scoop` group 默认源码库；Linux 的 `--cc`、`--native-sysroot` 选择 C 工具链。`--unwind-prefix` 和 `--link-mode static|dynamic` 属于 runtime/executable 链接输入，library `.slib` 构建不要求 unwind archive。Linux 缺省 unwind prefix 为 `<Scoop sysroot>/native/<canonical triple>/unwind`。Darwin 使用所选 Xcode 开发环境。

当前 Cone 的 `native.cxx = true` 要求解析同一 target 的配套 C++ driver：GNU 使用对应工具链的 `g++`，Darwin 使用同一 Xcode 工具链的 `clang++`。选中的 C++ 源码交给该 driver，`.c`、generated-C bridge、runtime 和生成的启动 C 代码仍按各自 C 编译配置生成对象。未启用 `cxx` 却选中 C++ 源码、启用后缺少配套 driver／运行库、启用后 target 为 musl，均在调用 native 编译器前诊断；不回退到宿主 C++ 工具链。

`c_flags`／`cxx_flags` 只作用于当前 Cone 的相应 native 源码，按 argv 元素与声明顺序传递。native 默认语言标准分别为 C11 和 C++20，可由对应 flags 中的 `-std` 覆盖。`cxx` 和实际生效的编译参数、C++ 源码及头文件、配套编译器与标准库 ABI 配置进入构建输入和 compile key。library 构建把 C++ 链接及运行库需求写入 `.slib` 的逻辑 native requirements，不执行最终 C++ 链接。

LIR foundation 以必需的 Boolean `native_cxx` 保存当前 Cone 的显式 C++ 运行库需求；该字段与普通 native library 表一起进入 Code。foundation wire 为 8；production/8 不再复制这一数据，读取方采用同一份 foundation。需求不依赖是否选中 C++ 文件，也不从机器符号推断。C++ 编译器沿已选择 C driver 的配套位置解析，保留相同 target、sysroot 和环境；GNU 的 target 与 compiler version 必须匹配，缺失配套 driver、标准库或 ABI 配置直接诊断。

native 与 runtime 的真实编译保存源码、depfile 中实际使用的头文件（含 scoop_rt.h）、include 配置、工具输入及影响 include 选择的目录状态。后续先检查这些输入的 metadata；全部未变化时不调用预处理器、不读取全部头文件。增加、删除或改变 include 搜索输入后重新发现并编译必要部分；所选 target、宏、SDK、C/C++ 配置与诊断位置沿正常工具调用传递。不为构建中外部替换输入建立稳定性检测、重试或不可变预处理镜像。

全局构建缓存由 scoop 管理。同一工作区、Cone、Production/Test、target、profile 可直接定位普通构建记录；本机路径可参与记录定位，不进入语言身份。记录保存已解析的有效配置、选中输入的逻辑名／修改时间／大小、工具与协议版本、实际依赖输入键／必要语义摘要、生成物及用户输出位置。更新判定先于源码读取、输入复制、预处理、能力查询和链接准备。输入内容在确需编译时读一次，供 parser 与必要语义计算共用。

记录不存在、缓存格式改变、有效输入变化、所需输出不存在或请求的观察结果尚未产生时更新相应结果。输入未变化且结果存在时直接采用记录；不核对 artifact/member/object hash、warning 来源、收据或指纹自洽性，不检测、修复、恢复或隔离外部破坏的缓存。记录按 schema 普通解码，实际读取错误直接传播；不通过扫描或重建来取得另一份更可信记录。旧缓存用新版本或 namespace 自然失效，不解析或迁移旧证明链。

输入缺失是普通源码构建错误，输出缺失只生成该输出。manifest 仅测试配置变化不使生产节点失效；未选中源码、程序 argv、测试过滤及显示方式不使编译失效。工具变化可以直接失效，不为恢复内容可能相同的命中强制读取整个程序。尚未使用的 generator 配置不影响构建；实际使用的工具按同一规则记录。

machine transport 为一次 request/response 的 length-prefixed canonical CBOR，protocol version 为 11，stdout 只含协议 frame。build request field 9 为 OptimizationMode（Debug=1、Release=2），field 10 为按归一化路径排序的选中 native 源码所对应的已编译 native 对象 locator 数组，数量必须与 manifest 的实际 target 选择一致；没有 native 源码时为空。field 11 为当前 Cone 的 CompilationPurpose（Production=1、Test=2）。target request 保存 canonical triple，以及待解析的 C driver／native sysroot 选择或已解析的 C 工具调用记录。parent 将已得到的记录直接交给 child，child 解码后用于实际编译，不重复执行探测；独立 scoopc 请求仍执行正常工具发现。调用记录保存普通版本／deployment 配置、host locator 和本机输入摘要，不是产物验证资格，host locator 不进入 artifact identity。旧或不匹配协议拒绝。request envelope field 5 为 observation-only 标志（unsigned 0／1）；为 true 时必须请求非空 dump 集合，沿同一编译实现执行到最后一个所需阶段，不发布机器对象或 `.slib`。观察成功使用 response tag 3，携带 warning 和 dump 描述，不伪造产物摘要。

成功响应直接携带生产时得到的产物摘要：Cone、直接依赖、target、产物 profile、各语义指纹、artifact fingerprint 及测试目录，并携带 warning 和 dump 描述。parent 将该摘要与产物实际位置保存为普通构建记录，不重新打开 child 产物、重读摘要或逐字段核对响应与归档。摘要是后续编译、链接及测试列举需要的数据，不是验证资格；不得包含无人消费的归档长度或成员数来支持复核。protocol 8 用此摘要替换原独立的七个指纹字段。

本机编译结果保存在 `compile-v2`：按输入键保存 `artifact.slib` 和普通 `record.cbor`（生产摘要与 warning），不保留 v1 收据、收据指纹或产物交叉验证。命中只读记录并检查所需输出是否存在；输出数据在真正的编译／链接消费者需要时才读取。编译成功后的输出经 rename 发布，记录普通原子替换，不读回、同步落盘或比较并发产物。取得构建锁后已有结果即采用，不建立等价性证明。库的用户输出在目标位置旁保存所对应结果的普通记录；未变化且文件存在时不读回或重新复制。相同位置改为可执行文件时废止此前库输出记录，不使下一次库构建误用其他构建结果。

显式观察使用 `--emit ast|hir|mir|lir|all --dump-dir dir [--dump-scope root|sources]`，默认 root；sources 覆盖源码节点，不从 prebuilt 反造 AST 或 LocalConcreteHir。HIR dump 同时包含 Export、LocalConcrete 与共有跨 Cone 接口。当前输入已有相应 dump 时直接复用；缺失时只执行产生该观察所需的前置阶段，不重做仍有效的机器对象、归档或最终链接。源码变化使观察记录失效，诊断使用当前定位；观察失败不能报告成功。dump 文件保存在对应编译输入键的缓存目录，命中后按请求复制到输出目录；只为缺失阶段调用 observation-only child。dump 描述只保存阶段及文件位置，不保存或复核内容摘要，不要求私有空目录、来源隔离或持久化同步屏障。

诊断具有明确 Error/Warning severity，primary 与 notes 保存实际 Cone、logical source 和 byte span。源码读取等文件级错误仅有实际 host path、没有字节区间时，使用独立的 HostPath origin（wire tag 5），不得丢失路径或伪造区间；已有 HostPathSpan 编码保持。产物错误保存 container/manifest/member/section 与 wire field path，I/O 错误不伪装成源码位置。human 与逐行 JSON 使用同一结构化记录，写 stderr；stdout 留给被执行程序。缓存中的 warning 在本次显示时附加 locator，不丢失原位置。显示源码诊断时仅按需读取涉及的文件以计算行列，同一轮显示共享读取结果；此读取不参与缓存更新判定。

工具中断停止后续构建，向当前 child 传递信号并回收进程，保留真实 signal。某节点失败后不启动 dependent 或 program-link。

### 2.8 程序链接与最终结果复用

program-link 消费完整 `.slib` Link 闭包、普通 runtime 对象、已有 native 文件、LIR target 与 final-link 配置。它不展开模板、重新编译 Scoop 或编译用户 C/C++；只可用明确选择的 C compiler 生成本次固定启动代码与最终 image tables。

root 是唯一 executable，依赖为 library。正常构建使用已确定的依赖结果、目标及链接配置，显式 link 读取所需 Link 输入。不以重算三层语义指纹、整包内容或重复生产校验来授予链接资格。

最终 executable 是缓存结果。先根据 Cone 代码结果、runtime/native 输入、库选择、工具与链接配置判断更新；输入未变化且输出存在时直接返回，不能调用 read_program_link_closure、生成 startup/image tables/response file、调用 C compiler/linker 或复制／读取 executable 内容。只改变 native、runtime 或链接配置时不重编译无关 Scoop Cone。库搜索记录影响实际选择的目录状态，新增优先候选或配置改变后重新选择。

program-link 从完整产物闭包合并 C++ 链接和运行库需求；任一 Cone 要求 C++，就采用所选 target 的 C++ final-link 配置。GNU 由配套 `g++` 驱动并使用 `libstdc++`，Darwin 由配套 `clang++` 驱动并使用 `libc++`；同时纳入各自的 C++ ABI 运行库与系统依赖。C++ driver 调用相应平台 linker，既有对象格式、GC、EH 与 image 契约继续适用。不能仅把 C 编译器替换为 C++ 编译器后继续原样使用抑制其默认运行库的 C 链接参数，也不能靠 root 重复声明 `cxx` 才满足依赖。

musl 闭包出现 C++ requirement 时，在解析 C++ 工具链或调用最终 linker 前报错，指出要求 C++ 的 Cone；不因其源码已预编译而绕过限制。实际使用的 C++ driver、标准库／ABI 运行库、unwind provider 及链接参数进入 final-link 配置和 ResolvedLinkPlan。

首次生产将对象保存在 `objects-v1/<member-content-digest>.o` 稳定缓存位置，后续链接直接传递对象路径；不每次从 slib 解包、改写、哈希或重新物化。文件名使用生成成员时已有的内容摘要，不能为查缓存重新哈希或读取已有对象。归档 writer 保存所写对象的字节范围与摘要，发布时借用同一份归档缓冲区，不另存对象字节副本。scoopc 使用 `SCOOP_CACHE_DIR` 或平台默认缓存位置，parent 把当前构建缓存位置传入 child；独立链接使用自身选中的缓存。缺失所需对象时从本次已有对象字节补产，临时文件写入成功后原子发布；并发已有结果直接采用，不比较内容或同步落盘。显式输入首次缺少独立对象文件时按需生成并记录，后续复用；已有的普通 native 独立对象直接使用原文件路径，归档内选中成员才需要独立缓存文件。供 native 工具使用的对象路径按当前工作目录解析为绝对路径，避免工具切换目录改变相对缓存路径的含义。同一进程按需入口共享已读 section，diagnostic、opaque、unknown optional 成员不是链接对象；无法处理的实际 Link-required 格式仍报告错误。

Strong 由原 provider 提供。ODR 在普通依赖语义一致后按完整 group/member key 与 shared ABI 合并，物理选择见 2.13、2.19。全部 typed imports 和实际 relocation 必须闭合；metadata-only 查询可以没有机器引用。

SourceExtern 按实际 target/native symbol 合并完整 library、kind、TLS、mutability、ABI、calling convention、GC effect 与投影后的 native signature/storage。任一字段冲突均报错并指出来源；随后从实际 native 对象或 provider export 解析定义。`captureErrno`、包装后的 Scoop tuple 结果与 NativeSafe/GcLeaf 是声明的调用行为，不进入 native ABI 合并键；返回 `R` 与捕获后返回 `(R, Int)` 的声明可共用同一 native 定义，Scoop callable 和所需 bridge 仍按各自合同保留。core 与普通 Cone 使用同一规则。

普通 native 对象只读取本次符号选择和 FFI 绑定所需的格式、目标、定义／引用、kind 和可写性；读取字段时保留必要的边界检查，不重复完整验证 relocation、unwind、TLS、段权限或装载结构。对象不含完整函数类型时，FFI 实现履行声明的责任不被伪造的类型证明替代。显式 library 的 symbol 必须由该库按平台规则提供，默认 namespace 的多 provider 歧义不能按搜索目录先后静默解决。

参数自由 source extern 的普通 managed 调用经原 provider 的 Scoop 薄入口，包含 native transition 与 caller-root publication，入口的 Scoop GC effect 为 Managed；native callee 自身的 effect 仍是其声明合同。泛型正文在定义方与消费方使用同一入口；NoGc 和 release 的直接 native 调用遵守各自规则。

runtime object index schema 3 保存普通对象路径、生产时已有的对象摘要、合并后的定义／引用及 target/runtime ABI。读取 index 直接采用这些结果，不重读对象、核对 digest、重建符号摘要或重放 runtime ABI 导出检查；首次生成对象时读取所需符号并检查 runtime ABI 导出。定义的绑定区分 Strong、Common 与 Weak；wire tag 分别为 0、2、1。链接器把 index 中的对象路径直接交给 native linker，不重新物化对象。

runtime 构建使用 `runtime-v3` 普通记录，按实际源码根、选中源码、include／unwind 配置、flags、C 工具输入和规则版本定位。命中前只查询上次实际源码、depfile 头文件及 include 候选的路径／mtime／大小；不读取源码、扫描并复制整棵头文件树或重读 compiler 内容。更新时直接从原始源码预处理并编译，观察实际 include 名称；全部成功后保存对象、index 与记录。对象采用稳定且不同批次不覆盖的位置；所需 index／对象缺失时补产，真实输入缺失照常报错，失败不发布新记录或替换已有结果。本机 metadata 不改变 runtime ABI。

native linker 成功后直接发布其输出，不重新读取整份 Mach-O／ELF，重放输入对象、重定位、stackmap、EH、动态绑定或代码签名的正确性检查；不通过链接 map／trace 审计实际读取文件。必要的符号选择、对象格式、ABI／GC 数据及工具参数在产生输入与实际链接时完成，工具退出失败正常传播。生成程序的正常运行和专门 ABI／GC 回归承担输出行为验收，不保留修改刚生成的可执行文件后要求发布复核拒绝的测试。

逻辑 native library requirement 沿完整依赖闭包传递，manifest 的 native.libraries 与非空 SourceExtern lib 使用同一解析流程。driver 将选中 manifest 要求合并到既有 LIR foundation native library 表；相同 key 去重，产物不要求每个库都有 Scoop extern 使用者。Code 覆盖完整逻辑要求，Link reader 复用同一份 LIR foundation 表，不从机器符号反推库名。先在全部显式 library roots 中按既有规则收集候选；该层无候选时，再交给所选 target 的平台 provider 在其默认系统库目录中解析。显式候选损坏、target/ABI 不匹配或歧义直接报错，不能用系统库掩盖。TargetDefault 与显式 kind/grouping 保持原合同；OrderedGroup 只限制候选顺序，不引入 whole-archive、用户脚本或任意 linker 参数。

Cone native 源码对象使用 `org.scoop-lang.link-object/native/1` LinkObject 类别，logical key 为归一化 Cone 相对源码路径的 UTF-8 字节，member identity 仍包含所属 Cone。实际 Link fingerprint 按 member ID 排序加入既有 Code 对象投影；这些对象没有 Scoop Strong/ODR 或 generated-C recipe。program-link 从依赖闭包的对应成员读取普通 native 对象，每个成员作为独立输入参与符号解析；不同 Cone 的相同对象内容不能据此合并。对象格式、target、实际定义与引用沿普通 native 对象规则检查。

Linux 默认目录来自已选 native toolchain/sysroot 及其 target/link-mode 配置；Darwin 来自已选 SDK 的系统库与 framework 目录（包括 `.tbd` provider）。系统 provider 按对应平台的库选择规则解析动态/静态格式；静态模式不得以动态库满足需求。交叉编译不补入宿主的 `/usr/lib`、framework 或其他宿主工具链目录。无合法候选时报告所请求的库和 target；选定 provider 的实际内容、加载合同与工具链/SDK 配置进入既有 ResolvedLinkPlan 和链接缓存。只查已声明的逻辑库，不自动发现相邻 native source/object/archive，也不扩大空 lib 的默认 namespace。

Linux 系统库选择由所选 C driver/linker 按目标配置完成。发行版提供的标准 linker script（例如 GNU 的 `libm.so`）可解析为多个实际 archive/ELF 输入；其脚本内容摘要、实际输入及顺序进入同一解析结果和 link plan。最终链接消费已选文件快照，不重新解释系统脚本或按宿主目录重新选库。该规则不开放显式用户脚本输入。

重复声明已经包含在 target 固定系统输入中的同一 provider 时复用既有定义，不产生第二个符号候选；相同加载名但不同内容仍是冲突。Darwin SDK 的 `$ld$previous$` 指令按 platform 和 deployment 的半开版本区间选择符号的实际加载库及 compatibility version，未命中的指令不改变当前接口。此类信息属于目标 SDK 的正常动态库加载合同。

musl 将数学库、pthread 等接口合入 libc；所选工具链为这些固定系统库提供的空兼容 archive 按该平台规则绑定到现有 libc provider。此别名仅适用于实际系统解析选中的固定兼容文件，显式 roots 中的空 archive 不获得额外符号。

普通 archive 保留真实 member index、offset、length 与 digest，允许 BSD/GNU 命名；拒绝 thin、外部或嵌套 archive 和损坏边界。通过普通对象 reader 建立候选的定义／引用索引，不为候选或选中 member 另建完整 relocation/EH/TLS/初始化验证器。实际 undefined references 驱动成员选择，新增引用继续闭合，每个物理 member 最多加入一次。

native 对象按普通目标工具链输入处理，不按段名、load command 白名单、C++ 符号前缀、COMMON 或可执行栈标志拒绝。Scoop 索引所需的格式与目标必须可读取，其余对象内容及嵌入的正常链接要求交给目标 linker 处理；实际工具失败照常报告。C/C++ 运行库按所选工具链配置参与链接。普通 C unwind、TLS 和目标 CRT 支持的初始化／析构记录沿平台规则执行，不要求额外 manifest 开关，不注册为 Scoop initialization unit 或 managed destructor。异常与初始化边界遵守运行时规范 5.5、第 7 章。普通 native 重复定义不使用 Scoop ODR 判等；Strong 覆盖 Common 和 Weak；ELF 的 Common 覆盖 Weak，Mach-O 的已有 Weak 定义覆盖 Common。同名 Common 由 native linker 合并大小与对齐，两个同名 Strong 报冲突。

非空 `@Extern(lib=L)` 在 Darwin 的每个显式 library root R 下，对 TargetDefault 检查 `R/L.o`、`R/libL.a`、`R/libL.dylib`、`R/libL.tbd` 与 `R/L.framework/L`；不去掉已有 lib 前缀或扩展名，不递归扫描。显式 kind 只收窄对应格式；相同内容及加载合同的重复候选可合并，不同候选为歧义。空 lib 只查询已引入的 runtime/native 对象和动态 exports，不扫描目录搜索任意库。

Darwin 支持 `.o`、`.a`、dylib/framework 与所选 SDK 的系统 providers。普通 load/re-export、真实 install name、target/deployment、symbol kind 和绑定关系参与解析；显式库不能被另一库的同名 export 替代。`@rpath` 从明确目录解析，实际 LC_RPATH 进入输出和 link plan；依赖中的 `@loader_path` 相对 provider。需要部署布局才能解释的直接 `@loader_path`、`@executable_path` 或未知形式拒绝。

Darwin 使用所选系统 linker、按 native 需求选择的 C／C++ 链接 driver、SDK/deployment 与 libSystem，Level I unwind 经系统正常导出解析；C++ 模式同时使用配套 libc++ 与 C++ ABI 运行库。final-link 不使用 dead_strip、不同 body 的 function ICF、LTO 或 raw 用户 linker options。stackmap relocation 后位于只读 `__DATA_CONST`，采用传统 rebase/bind（no_fixup_chains）格式；普通绑定与随后执行的 weak coalescing 分别解释，允许同一指针对相同符号和 addend 具有这两阶段记录。实际需要的 ad-hoc signing 属于平台装载格式。

Linux 接受 `.o`、`.a`，动态模式另接受 ELF `.so`；framework 与静态程序的动态库需求为错误。用户 `.so` 必须是 DSO，不是 PIE executable 或 linker script。未版本化的源码 extern 只匹配默认导出版本，DT_NEEDED 不等于显式库 re-export；TLS、IFUNC、版本化导入按实际 kind/version 检查。

Linux gnu 默认 dynamic PIE，musl 默认 static ET_EXEC；musl 可显式 dynamic PIE，不支持 glibc static。非 C++ 链接明确选择 CRT、LLVM unwind、libc、libm/线程及算术 builtins；GNU C++ 链接使用配套 libstdc++／C++ ABI 运行库，并统一由配套 libgcc_s 提供 Scoop Level I 与 C++ 所需的 unwind，不再额外链接另一套 LLVM unwind provider。musl 不支持 C++ 模式。动态 metadata/stackmap 满足实际 RELRO，静态 musl 的不可变记录位于只读 PT_LOAD；最终文件不得有 text relocation。

最终链接按既有选择顺序将对象路径写入临时 response file，正确引用路径中的空白、引号和反斜杠。Linux 由实际 GNU linker 展开对象列表，不能由 C／C++ driver 先展开成再次受系统 argv 长度限制的命令。response file 与链接工作目录只承担工具参数传递，不改变对象选择、输入检查、顺序或 ResolvedLinkPlan 身份，也不开放用户 linker 参数。

DSO 依赖查找使用显式 roots、provider 目录、实际 RPATH/RUNPATH 与所选系统目录。真实 SONAME、DT_NEEDED、加载顺序和 RUNPATH 进入 link plan，临时快照路径不写入 executable 的依赖名；同名导出使实际 ELF 顺序无法满足显式库绑定时报告冲突。

链接生成 `int main(int argc, char **argv)`、最终 image descriptors、六张 producer pointer tables、image pointer array 与唯一 root entry 引用。C main 按运行时规范 2.8 调用 `scoop_rt_run_program(images, image_count, root_entry, (int32_t)argc, (const char *const *)argv)`，原样返回其 C int 结果，不丢弃 argv[0] 或主动截断退出码。这些生成物只引用所选记录，不复制 TD、storage、cell 或 gateway body，也不在 C main 中构造 managed Array/String。`scoop_td_String` 是实际 String TD 的同地址 alias。

ResolvedLinkPlan 使用 `scoop-resolved-link-plan-v3`，覆盖 root/依赖、Code 与对象内容、runtime/native/startup 输入、ODR 选择、最终 image 内容、系统 providers、库绑定、archive members、加载路径、工具配置和有序操作。物化与输出 locator 不进入内容身份，实际 install name/runpath 必须进入。

本次链接按实际 target、symbol、Strong/ODR 选择、startup/image 与 GC/EH 要求生成程序，native linker 不能改选另一份 ODR 实现。工具成功后普通 rename 发布并保存链接结果，不重新读取刚生成的 executable 完整复核生产过程。目标装载格式与执行位属于输出工作；后端、链接与 GC 合同由针对性程序／结构测试验证，不成为每次发布和命中的重放阶段。失败不发布部分文件。

### 2.9 虚方法与接口分派

每个 slot 有原根声明、完整签名、访问域和 Abstract/Concrete/InterfaceDefault 的明确选择。class vtable 保留 base prefix，override 不改变 family 位置；Any 不预留 equals/hash/toString 槽。

interface 槽序按直接父接口声明序继承，再追加当前声明；相同 exact application 的菱形路径去重，移除被原 typed override 覆盖的成员，保留其余相对顺序。getter/setter 独立，private helper 不占槽。实际类／值类型表项匹配原 slot 的参数、结果、execution 与 GC effect。

itable 以实际 interface TD 为键，不要求跨 Cone 的全局槽编号。generic receiver 始终是完整 exact application，只沿声明中的 exact base/interface 关系分派。

普通接口值保存 object 和对应 exact interface 的 itab。构造／转换时查询目标表一次，已知实际类型直接引用既有静态表，相同 exact 视图直接复制；`is`／`as` 的检查与表查询可合并，不重查同一事实。空接口以 entry 存在判断成功，不用合法 null slots 判失败。接口方法从 itab 直接取 slot，不再次查询 object TD。

LIR 的 dispatch slot 显式保存单字 object receiver projection 和完整入口签名，普通接口参数／结果仍为双字。兼容 class 实现可直接作为 slot；需要完整接口 this 的 default／变型／装箱入口由已有 typed adapter 重建视图或复制 payload。去虚拟化与直接调用按实际 callee 的签名适配，不能只因都含 pointer 就混同 ABI。静态表和 adapter 继续使用正常 typed definition／reference、provider 和 ODR relocation。

intrinsic class 的 adapter 同样使用该声明已经确定的物理类型与 exact identity。`String`／`Any` 的 receiver 保持本层对应的 intrinsic reference type，不为 adapter 另造普通 `Class` 别名；源级接口继承及 default body 仍按普通声明处理。

值类型只有显式声明或继承的普通 interface conformance；有界泛型调用、interface 调用、装箱与 `is` / `as` 消费同一实际接口闭包。结构 operator equals 不产生 Equals conformance。显式 `Equals<T>.equals` 与其他接口方法使用相同的调用规则；已知 concrete value 的直接调用不因另有接口实现而强制装箱或运行期查表。

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

target/backend/C toolchain 的 typed identity 保留既有 canonical 编码及实体派生关系，支持集合在实际消费者处判断；仅移动实现和改变进程内表示不提升 wire/schema/runtime ABI，只有实际编码或可消费合同变化才升级相关格式。artifact-only 消费使用已保存的布局与 ABI，不加载 codegen、创建 TargetMachine 或重新分类。具体 target/backend 布局与合同常量可由 codegen 所属的静态数据模块供 artifact-only 消费者共享；该模块不含 LLVM、ABI 分类器或工具执行，公共 identity/LIR 只定义数据类型和规范化编码。

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

正确性由产生结果的相应 stage 完成：源码语义、当前转换的完整 IR、ABI/GC 数据和实际链接绑定分别承担其职责。消费已有结果按需解码并安全索引，不完整重放名称、类型、引用、布局或生产语义，不用 Verified 包装和深比较建立证明链。加载后的 GC、地址与内存边界契约保持。

对象定义的符号表序号查询复用一次建立的 member 内索引。image 注册表校验按当前 atom 收集并按 offset 查询实际 relocation，Strong binding 复用已排序的 `(member, atom, offset)` 查询；不得为每个 safepoint／type／callable 表项重新扫描整个对象或 Cone 的 relocation／symbol 集合。数量、唯一 offset、实际 target、role、宽度与编码值检查保持不变；这些查询数据不进入产物格式或实体身份。

对象定义的符号表序号查询复用一次建立的 member 内索引。image 注册表校验按当前 atom 收集并按 offset 查询实际 relocation，Strong binding 复用已排序的 `(member, atom, offset)` 查询；不得为每个 safepoint／type／callable 表项重新扫描整个对象或 Cone 的 relocation／symbol 集合。数量、唯一 offset、实际 target、role、宽度与编码值检查保持不变；这些查询数据不进入产物格式或实体身份。

### 2.12 Core 与普通依赖

core 是普通 library Cone，可修改和重建，使用相同声明、可见性、泛型、产物、ABI 与链接规则。sysroot 只提供默认 locator，不改变实体归属或来源资格。

M42 标准基础库：`scoop:scoop.std:0.1.0` 是只直接依赖 core 的普通 library Cone，公开契约见 [标准基础库规范](SCOOP-STDLIB-SPEC.md)。其代码全部由 Scoop 源码组成，使用公开 core API 和普通跨 Cone 编译、单态化、产物及链接路径。不得为库新增 native source、`@Extern`／`@Intrinsic`、按库名识别的 lowering、特殊容器布局或产物字段。Collection 是普通 core 源码接口扩充，新增接口关系按已有 API／布局／代码指纹失效规则重建依赖，不另增版本或发布机制。若合法库源码暴露编译器、core 或 runtime 缺陷，先建立独立最小回归并修复底层，再继续库功能；不得在库中建立绕过缺陷的平行实现。修复确实改变既有语言、ABI 或格式契约时，先修订相应规范并按原规则迁移，不预先为 M42 扩大协议。

只有编译器主动构造或调用的语言角色需要独立的 typed protocol 引用，例如 Unit/Any/Nothing、整数/Char/Float/Double 表示、Option/Iterator、编译器异常、初始化循环、协程和 callback。引用必须解析到实际声明及完整签名，缺失、重复或形状错误在相应边界报告。Any/Nothing 必须具有语言规范 11.1 的源码声明和普通 public binding；内部顶／底类型表示只引用这些声明，不能用固定名字补建缺失的名义声明。

Option protocol 保存同一个实际 owner、Some payload 与 None variant；Integer protocol 完整覆盖八种整数 kind。compiler exception protocol 包含实际 Throwable、UnwrapException、ClassCastException、ArithmeticException、IndexOutOfBoundsException、IllegalStateException 与 IllegalArgumentException 及所需调用目标。

内存中的 core protocol 使用具名、类型明确的角色字段，不以位置数组保存可任意混合的 entry 类别。wire 解码在既有边界检查 tag、引用与 owner 后形成具体字段；只改变 Rust 内存表示而不改变编码时不提升 section 版本。普通 provider、签名及实体关系检查保持，不增加来源资格或证明对象。

ToString、Hash、普通 operator、print/println、容器和 codec 通过普通声明与接口分派使用。新增普通 core 成员不要求新增编译器角色表；intrinsic 表示不自动赋予未声明的 conformance。

Disposable、普通／挂起 use 和 ResourceUseException 同样是普通 core 声明。use 的成功与失败出口由 Scoop try/catch 和函数调用表达，跨 Cone 复用已绑定的泛型正文与协程转换，不按名称生成专用 cleanup，也不把该异常加入 compiler exception protocol。

gcAdjustPressure 是普通 core C ABI extern，绑定运行时规范 3.1 的 GC-free pressure 操作。沿普通 GCLeaf 调用、native 引用及链接路径处理；不添加 intrinsic、compiler-known role、buffer 特判或产物字段。NativeByteBuffer 的 owner、游标、同步借用和 release hook 使用已有普通 class、try/finally、FFI 及 release lowering。

write/writeError 通过普通 String 作用域借用调用 C ABI 字节输出后备，flushOutput 通过普通 C ABI 调用刷新 stdout；编译器复用 NativeSafe 调用及 pin 帧，不为输出名称增加 intrinsic 或专用状态切换。print/println/eprint/eprintln 的 ToString bound、求值次序和跨产物泛型展开均遵守普通函数规则（语言规范 14.4）。

exit 是返回实际 Nothing 的普通 core Scoop ABI extern；调用及产物保留底类型、既有返回 carrier 和无正常后继的语义，不伪装成 Unit 或添加不可达占位正文。runtime 的单向终止和刷新合同见运行时规范第 7 章。

core prelude 从普通 public bindings 提供最低优先级名称，direct/support 依赖与 re-export 保留原身份。普通调用、默认值、模板与 native 引用使用同一 Export/MIR/LIR 合同；String 的 layout、TD、registration 与 alias 也来自实际声明。

Any/Nothing 的 intrinsic kind、原声明、完整 application 与静态描述经过同一 HIR/MIR/LIR metadata 通道。别名和再次发布保留原身份；实际 core 源码声明必须可通过普通依赖查询取得。Any 可以保留内部的顶类型分类，Nothing 可以使用带底类型表示的普通 class application，但分类不替代源码声明。

HIR 将 Nothing 参与的子类型、结果合并和调用完成性质在源码边界确定。向其他类型适配只保留原求值，不生成目标值。MIR 将无正常返回的求值结束为明确的不可达正常边，保留原有异常出口；后续调用实参、字段写入和正常 return 不能越过该终止点。LIR/codegen 复用已确定的控制流和调用签名，不根据函数名称猜测 noreturn，也不把它当成 Unit/ZST。

### 2.13 跨 Cone 模板与 ODR

普通泛型模板所引用的非泛型 suspend 函数／成员，按实际支持声明生成 provider 的完整 callable 导出，包括非 public 声明。选择过程使用同一共有声明目录；公开 binding 与用于模板消费的支持声明不构成不同的执行资格。消费者复用原声明的协程执行 ABI，不因其访问域遗漏 MIR 导出，也不将原正文重新单态化为另一函数。

非泛型接口的私有普通方法与其他 nominal 的直接成员使用相同的自动物化规则：所属类型及签名进入物化闭包时，provider 生成完整正文和 callable binding，不要求先出现本地调用。方法仍保持私有访问域，不占接口分派槽；带类型参数的方法仍按实际 application 物化。

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

语言规范 11.9.1～11.9.2 的 Job、任务树和取消适配是普通 core 源码，使用精确 Job Context binding、已有原子、异常与 continuation。编译器不按这些库名称生成专用身份、调度或取消路径；跨 Cone 按普通泛型模板、可见性与调用规则消费。

每个 body 的 Context key uses 与 slot cells 属于该 body 的物理选择，callable registration 保存完整关联。最终登记在首个 gateway 前填充 cells，同一 exact key 在程序中映射到同一 slot。

### 2.17 静态类型描述与编码方法

静态描述使用原声明、字段、variant、logical property、constructor/default 和 annotation 事实。语言规范 9.6 的 TypeInfo/TypeRef/TypeShape 及成员、注解记录由普通 core 类型承载；编译器内部查询仍为原 HIR 数据的视图，公开描述在具体类型已知时生成普通值和当前层 describe 闭包，不引入 CTFE。引用的相等编码既有 typed persistent/exact identity，不能用名称或地址回退；成员的声明处 Option 事实复用默认 codec 的原类型查询。查询不执行被描述声明的 constructor/default/getter/companion，也不扩张可见性。

公开描述的递归关系由普通 TypeRef 和闭包断开，引用构造不调用其 describe。描述函数及子引用闭包沿既有泛型具体化机制生成：同型递归复用已有请求，实参沿递归环增长时使用现有 polymorphic-recursion 诊断，不加深度预算或内容缺失的占位描述。公开模型的普通数据声明本身不改变 `.slib` wire 格式或 runtime ABI；查询的 HIR 登记及产物依赖必须使用实际声明身份。

`typeInfo<T>` 是普通 core 泛型正文；内部 `__scoopTypeReference<T>()` 先调用 `type_reference` 取得 TypeReferenceData，再用普通 TypeRef constructor 和调用 `type_description<T>` 的普通闭包构造引用。引用数据由 identity、displayName、nominal、arguments 四项组成；成员引用为各自独立的普通 struct。两个 intrinsic 在 concrete HIR 中生成普通正文，不进入 MIR。引用构造函数作为 core protocol 的 field 10 保存实际 generic callable identity，下游按此身份请求模板，不按名称猜函数。intrinsic wire tags 26／27 分别表示引用数据和当前层描述；因此 HIR core-bootstrap-interface 升至 16，cross-cone-interface 升至 77。注解声明的惰性描述正文使用 `GeneratedCallableKey::AnnotationDescription`（tag 18），键保存实际 PersistentAnnotationId 与查询所在 callable template；无参数辅助函数继承该 callable 的实例化上下文，复用其普通 ODR 归属。递归注解描述保持同一查询宿主，不逐层产生新的宿主。它不是 runtime 注册表。HIR／MIR／LIR identity-foundation 分别升至 10／8／9；runtime ABI 保持。cross-cone-interface/77 的 property declaration field 9 保存已经绑定的原 property override 身份序列，供描述直接读取。

`annotation_key`（intrinsic tag 28）只用于标识公开 core 查询声明。HIR lowering 先进行普通 callee 绑定，再将声明实参解析为 PersistentAnnotationId，输出 AnnotationKey 表达式；不制造普通类型实参或运行时 intrinsic 调用。共享正文使用表达式 tag 75 保存这个 typed id，默认参数的直接引用表 field 7 保存注解声明引用及其正常定义来源。引用闭包按已有声明依赖规则保留注解，具体化复用上述普通 AnnotationKey 和惰性描述辅助函数。cross-cone-interface 升至 78，cross-cone-type-semantics 升至 30；core protocol 结构、identity foundation 与 runtime ABI 不变。

描述生成使用已经绑定的声明事实。正常类型加载补齐注解参数、计算属性结果和未调用构造器参数的类型，避免只有布局使用过的类型才能被描述；不调用对应正文或重新执行注解求值。

语言规范 11.17 的 inspect 合成在 HIR lowering 复用普通 requirement 选择、方法签名登记、成员访问和显式依赖选择；生成普通 typed 读取、活动 variant 分支、闭包及 core 视图构造。合成作用域保持实现声明的实际 owner 与 source file，不增加成员访问许可。共享正文保存已选成员／inspector 与词法捕获，后续泛型具体化只替换参数。ValueView、集合 inspector 与 DebugPrinter 是普通 core 源码；MIR、LIR 与 runtime 不解释 DebugIgnore／DebugRedact 或读取反射字段。没有专用布局、全局 inspector 表或新的 wire 格式。

语言规范 12.7 的 scoop.cli 完全由普通库源码实现：ClassInfo.isFinal 直接读取已有 class modifier；schema 使用描述中的成员／primary constructor 关联与原声明 Option 事实，在构造边界检查一次配置。argv 归一化、稳定 Decoder、数值文本及 help 不进入编译器 stage，复用已有浮点文本 extern ABI。

concrete HIR 的 TypeIdentityLiteral 指向已完全绑定的 TypeId，值类型固定为 String。MIR lower 只从现成 exact-type 表读取该 id 的十六进制 bytes 并生成普通 StringConst；不重新计算身份、遍历字段或解释描述。这让描述函数沿同一实例化队列生成，身份仍由原最终关系统一建立。

annotation 声明使用独立 PersistentAnnotationId。普通值参数保存完整 signature type key；annotation 原子及其数组参数使用独立的 annotation identity 和实际 core Array 模板身份，不把 annotation class 编成 runtime TypeId。应用保存实际 typed target 和按参数顺序补齐的静态值。CanonicalConstValueV1 保持原标量用途；CanonicalAnnotationValueV1 以封闭的 Scalar、IntegralEnum、Annotation、Array 保存静态值。整型枚举值保留完整 application signature、variant identity 和底层整数位型，嵌套应用保存实际 annotation identity 和完整参数；数组保存明确元素类型、有序值以及逐元素的 SourceIdentity 和 span，默认值保留定义处来源。共享 reader 在既有边界检查引用、枚举 variant/数值、嵌套参数及 tag/payload 一致性，不重放名称查找或默认值求值。参数/default 的循环依赖在源码定义处诊断。名义类型字段保持原身份，generic 字段保留 binder；tuple 保留位置，class 保存真实 field/property/constructor 关联。

HIR foundation field 35 保存 annotation declaration keys，共有源码接口 field 14 保存 annotation declarations 与有序 applications；实际 annotation 依赖使用 AnnotationDependency tag 11。NominalDeclarationDetails field 11 保存可选 primary constructor 及声明序参数到 property 的映射，普通参数为空，val/var 参数引用原 logical property。

注解 target 保留原声明的类型化身份：named callable、constructor、accessor、annotation 和 typealias 使用各自声明身份；普通值参数以 callable 身份和源码参数序号选择，receiver 单独选择。局部 callable 复用既有 lexical generated callable identity。字段使用现有 field/variant-field identity；顶层或泛型 property 的存储以原 property 身份与 backing/delegate 类别表达。use-site 只在 HIR 名称绑定时选择真实已存在的实体，不新增运行期布局或隐藏参数。共享接口保存这些已绑定位置；reader 检查实际声明引用和共享参数表中的索引；局部 callable 复用已绑定的源码参数位置，不另存一份只用于重放检查的签名表，也不重放 accessor body 或默认目标选择。

annotation 声明保存已绑定的允许目标集合与 repeatable 属性。集合使用语言规范 9.4 的十二种类别，元注解应用本身仍是普通有序静态数据；不在 reader 中重跑 core 名称解析或元注解求值。所有 annotation 参数/default 和元注解先完成绑定，再按最终声明属性检查前向应用；普通使用处按实际目标和 annotation identity 检查。共享 reader 在原注解数据边界检查目标类别、重复应用与该声明属性一致。annotation declaration record 的 field 5 保存这两个属性。

类型使用注解独立于 TypeId 保存。每个使用记录以实际 SourceIdentity 和类型语法范围组成类型化位置身份，包含已解析的完整类型、binder 与使用树内的注解路径；没有注解的节点由该完整类型给出，不另存一套语义类型图。路径区分类型实参、tuple 元素、函数 receiver/参数/结果与 pointer pointee。字段/property/完整参数/结果、alias 目标等描述位置通过实际 typed 声明和位置选择关联使用记录，不能由名称或源码范围包含关系猜测宿主。

HIR lower 在正常类型解析过程中记录这些位置和 alias 的实际替换关系，注解值待相应 schema/常量完成后仅绑定一次。alias 与泛型替换复用原 binder 映射，合并相应路径的静态应用，不污染共享类型或重跑类型检查。annotation 参数的 annotation class 及其 Array 使用独立 annotation identity，不制造 runtime TypeId；用于确认 Array alias 转发关系的临时参数不进入静态描述。

共享注解表增加类型使用记录与声明位置关联，完整类型用既有 signature key 与独立 annotation 参数类型表示，应用沿用 canonical 静态值并保留各自来源。reader 在原静态数据边界检查实际引用、binder、路径及 TYPE/Repeatable 规则，不重放 alias 展开、默认值或布局。静态查询从同一记录取得类型、各节点注解和参数替换结果。cross-cone-interface 当前版本为 78，旧产物须重建；MIR/LIR 和 runtime ABI 保持不变。

property accessor 的 Storage、Constant、Body、AbstractSlot、StorageBody tags 分别为 1..5。StorageBody 表示普通存储语义产生的实际 accessor body，不能通过识别机器正文推断。NominalDeclarationDetails field 12 保存 sealed 直接子类型（2.2）；generated callable tag 17 为 TestEntry，tag 18 为 AnnotationDescription。

companion 继承直接宿主的 binder 与 bound，不捕获宿主实例或 primary 参数值；ordinary static nested declaration 隔断该 binder。companion declaration、完整 application 和 concrete singleton 分别有身份，泛型状态共享规则见 2.13。

Encodable<R>.encode 与 Decodable<R>.decode 是实际 companion/codec 的普通方法，receiver 与数据参数/结果独立。手写、继承与 default 先按语言规则选定，仅为缺失的 requirement 合成方法；字段 codec、variant、constructor 和默认值在定义处完成选择。

Serialize 的 rename／required／emitNull／ignore 来自同一已绑定 annotation 配置，当前源码与依赖使用已有静态值，不扩展 wire 参数值域。字段的声明处 Option 事实从原成员已绑定、展开 alias 的类型取得；具体化后不重新推断。None 省略只检查 default 的直接 typed None 表达式，不执行 default 或通用常量等价性分析。合成结果为普通 typed 字段访问、Option presence 分支、None 值、调用、控制流、闭包与构造；encode/decode 分别保留语言规范 11.13 的求值顺序、缺失字段处理和可见性。Export HIR 保存完整正文及支持声明，下游只实例化，不重新解释 annotation 或搜索 codec。

codec 不因数据继承关系自动传递，不能按 value 的动态类型改选 companion。不同 codec 声明即使处理同一 R 也保持独立身份；数据类型本身不因此获得编码接口。tuple、Unit 和容器组合使用普通 helper，JSON 是普通库，不增加专用 MIR、runtime type table 或 C 入口。

### 2.18 Float / Double

语言语义见语言规范 11.2.2，运行边界见运行时规范 6.1。Float/Double 分别为 F32/F64，alias 不建立新 nominal identity。literal 与 const 按目标精度直接求值，成功输出保存原始同宽 bits，不经宿主浮点近似。

算术、默认比较、分类与转换使用明确 typed operation。`compareTo` 返回 Long；六种默认 predicate 与其符号保持一致。显式 `ieeeCompare` 通过普通 core body 组合分类与比较，返回普通 IeeeComparison enum，不增加 enum 专用 intrinsic。浮点除零不使用整数异常规则；相等不能替换为 identity 或位型相等。所有 storage、aggregate、数组、closure、frame、boxing 和 C ABI 保留完整精度及 exact type。

浮点操作不得使用 fast、nnan、ninf、nsz、reassoc 或 contract 等改变语言语义的假设。默认比较将所有 NaN 合并并排在正无穷之后，将正负零合并；const 与后端使用相同规则。显式 IEEE 比较的 Unordered 不满足有序关系。float→integer 按目标位宽饱和，不生成越界 poison 或先转 64-bit 再截断；同格式转换保持 bits。

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

FloatBinaryOperator 的算术 wire tags 1～5 不变；旧 IEEE predicates 的 6～11 与 totalOrder 的 12 退役，不再解码。CompareTo 使用 13，默认 Equal/NotEqual/Less/LessEqual/Greater/GreaterEqual 使用 14～19，避免消费旧 IR 时悄然改变其比较语义。受影响 core 与依赖须重建；沿用现有未知 tag 与 core 协议兼容性检查，不增加 runtime ABI。

浮点 literal 语法不能表示 NaN，有限 literal 集合不能覆盖完整浮点域，剩余域需要 wildcard。JSON 使用显式 codec 和目标精度直接解析，有限／非有限值遵守语言格式规则。

### 2.19 优化与物理定义选择

debug 使用 Scoop machine O0 与 generated-C O0，release 使用 O2；两者均完成正确代码生成所需的 SSA/GC 转换。runtime build 配置独立，默认 O2。优化模式不改变源码语义、persistent entity identity、shared state 或对外 ABI，兼容的不同优化产物可以共同链接。

Linux/amd64 的 managed 调用保留固定的 outgoing argument area，使运行时规范 3.2 的 `FP = SP + N - 8` 对每个 safepoint 都成立。LLVM 22.1 后端关闭 X86 call-frame size optimization，避免按值参数被改成调用点临时 push/pop；源码参数语义、calling convention 和其余 O2 优化保持。此设置在创建任何 target machine 前由 codegen 统一初始化。

LLVM 22.1 关闭寄存器分配后的独立 tail duplication pass 及机器基本块布局中的 tail duplication，避免在 roots 与 site identity 定稿后复制 STATEPOINT。只关闭布局阶段不够：独立 pass 也可能复制共同的终结调用或异常合流后的 Managed 调用。条件 poll 后接不返回的 Managed 调用也必须只保留一个实际调用点及一条对应 stackmap 记录；给 statepoint 添加 LLVM `noduplicate`／`convergent` 调用属性不能阻止该机器优化。此限制沿用统一的 target machine 初始化，保留其余 release O2 优化，不改变 SafepointId、返回地址或对象验证规则。

普通优化可以消除已证明不可达的代码及 site，但保留实际求值、异常、Context、initialization、release-ready 与严格浮点语义。类型错误不能因优化删除代码而消失。

MIR 内联按调用点代入已知实参、局部常量和可证明不可达分支后的加权成本选择：极小正文积极内联，普通小正文结合循环位置、常量／receiver 机会和累计增长选择，较大正文保留调用，除非化简后进入前两档。标量运算、字段访问、aggregate copy、分配、调用和复杂控制流具有不同成本；实参副作用始终求值一次。稳定顺序、caller 累计增长及嵌套展开限制只决定优化选择，不决定程序合法性或 ABI。实际参数由性能记录公开，不增加通用跨阶段预算或源码属性。

条件 poll 在 LIR 保存慢路径 site 与完整 live set，仅实际慢路径产生 statepoint／stackmap；合流连接更新引用，NoGC 不新增 poll。内联、去虚拟化及 CFG 改写均先于最终 roots 定稿，后端不靠完整 runtime poll 调用维持普通循环的检查。

依据语言规范 1.2，编译器可以假定普通非原子访问不存在数据竞争，不为竞争场景提供单字或逐字段不撕裂的保证。普通字段、数组元素、全局存储和聚合值复制无需为此改用 LLVM `unordered` 原子 load/store、增加发布屏障或限制访问的机器指令粒度；显式同步原语以及 runtime/GC 内部同步的契约仍须保持。

最终 GC 计划从实际保留的 callable/site 和完整逻辑 managed leaves 得到。多个逻辑 leaf 可以对应同一 SSA 值，已知 null/不可移动常量不要求 relocation pair；仅在这些常量之间选择的 phi/select 同样不要求重定位，包含任何动态 managed 输入的选择仍按移动根处理。runtime root count 使用实际 pair 数。每个保留 site 具有唯一 owner/role，site identity 不因删除其他 site 而重编号。

GC 转换后每个 post-site managed use 必须来自正确更新的引用；invoke 的 normal/unwind 与 native transition 继续使用各自显式 root frames。不能在 root plan 定稿后增加未经描述的 safepoint、重复 site、外来 owner 或异常边。backend contract field 28 为 1，表示此最终根与发射合同。

ODR 只比较 key 与 shared ABI，按实际需要选择物理 member。body 的机器码、EH、stackmap、callable/safepoint registration、关联常量与 Context cells 必须同选；独立 helper 可分别选择。shared TD、storage、cell 与初始化保持唯一，native linker 不得改变已确定选择。

最终 image descriptors 和六张表由 program-link 从所选 records 生成，不改写输入 `.slib`。选择与实际内容进入 RuntimeImage、Code、Artifact 和 ResolvedLinkPlan 的对应摘要。

OptimizationMode 在 machine request field 9 中编码 Debug=1、Release=2；生产端 Code 输入继续保存模式，production section 不再复制该值；compile cache domain 为 `scoop-cone-compile-cache-v2`，mode 位于 field 13。generated-C flag 的 SelectedOptimization tag 为 15，tag 6 保留不用。

digest kinds 4、8、9，digest owner tags 4、11、9，以及 RegistrationDefinition patch role 1 保留不用。registration 公共 identity 只含 linkage、semantic ID、ODR group/member；body、layout、descriptor、scan 与 normalized-stackmap fingerprints 仍用于所选实现自身的内容、引用及 GC 合同，不作为不同优化正文必须相等的条件。

# M39 设计：目标平台边界重构

状态：实施中。实际基线为 `e88f7d73`；功能与验证进度见 [PROGRESS.md](PROGRESS.md)。本文不代表新增架构已经通过编译、链接或运行验证。

日期：2026-10-12。

背景：后续需要增加 Linux ARM64、RISC-V 64、LoongArch64 等 target。重构前的 Linux 路径多处直接采用 amd64 假设，且目标 ABI 算法、LLVM 配置和工具链行为分散在公共 IR 与 lowering crate 中。M39 先整理这些职责，使后续移植有明确、有限的修改范围。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)，以及 [M28 Linux 支持](../milestone28/DESIGN.md)、[M33 平台库前置能力](../milestone33/DESIGN.md)、[M36 类型完备性](../milestone36/DESIGN.md)。实施前记录实际代码基线；本文不假定其他进行中的里程碑已经完成。

设计稿原先仅新增本文件；2026-10-12 开始实施，先同步权威规范与路线图，再按功能提交实现。

## 0. 目标与范围

M39 的交付是目标平台边界重构，保持现有三个 target 的可用能力：

| target | 必须保持的行为 |
| --- | --- |
| `aarch64-apple-darwin` | 现有 Mach-O、Darwin/AArch64 ABI、系统工具链和运行能力 |
| `x86_64-unknown-linux-gnu` | 现有 glibc 动态 PIE、C/C++ 互操作和运行能力 |
| `x86_64-unknown-linux-musl` | 现有默认静态 executable、显式动态 PIE 和运行能力 |

完成后，在现有 64-bit little-endian 语言和 runtime 模型内增加一种目标架构，编译器的机器相关实现应只需要修改 `codegen`，以及目标选择、工具链、产物读写和链接相关代码。parser、HIR、MIR 及通用 LIR lowering 不应增加该架构的分支。

本文把 `driver`、`scoop`、`toolchain`、`manifest`、`protocol` 中的目标选择和构建编排，以及 `linker` 中的最终链接，归为 driver 相关功能；把 `.slib` 的对象读取、格式兼容和重定位处理归为 slib 相关功能。按功能划分例外，不要求这些代码物理上全部位于两个同名目录。

本次重构允许一次性修改 `identity`、`lir` 和 `lir-lower`，以移出已有的具体目标知识。后续新增目标不应反复修改这些公共数据结构来登记架构名称或加入 ABI 算法。如果实际新 ABI 需要现有模型无法表达的物理事实，应先修订相关数据合同；不能为了承诺绝对零修改而使用无类型字符串、通用属性包或隐藏状态。

Linux ARM64、RISC-V 64、LoongArch64 的实际支持属于后续移植，不作为 M39 的交付或验收条件。M39 不新增未实现的 target 条目、空 backend、占位 runtime 或仅为测试存在的平台。Windows、32 位、big-endian、新语言功能、JIT、动态加载和通用工具链发行系统均不在范围内。

## 1. 审查结论与修改起点

静态搜索并沿生产调用路径检查后，未在 parser、HIR、MIR 的生产代码中发现需要为新增这些架构修改的 Linux/amd64 分支。主要例外从 LIR 的目标描述与物理 ABI 分类开始。这个结论不等同于新架构已验证可运行。

下表记录实施前的问题；已删除路径以代码文本保留，当前所有者通过链接标明。

| 编号 | 基线位置 | 基线问题 | M39 处理 |
| --- | --- | --- | --- |
| R01 | [lir-lower/abi.rs](../../compiler/lir-lower/src/abi.rs) | 非 `DarwinAarch64` 一律使用 SysV AMD64 寄存器分配和 aggregate 回退 | 具体 Scoop ABI 分类归 codegen |
| R02 | `lir-lower/externs/direct_c.rs` | C 调用同样以非 Darwin 代表 SysV；间接参数分类绑定平台，整数扩展函数没有 target 输入 | 具体 C ABI 分类及其验证共用 [codegen ABI 规则](../../compiler/codegen/abi-plan/src/c.rs) |
| R03 | `lir/abi/aggregate.rs` | IR crate 实现 HFA、eightbyte 分类、6 GPR/8 SSE 分配算法 | IR 保留布局与 carrier 数据，算法移入 [codegen](../../compiler/codegen/abi-plan/src/aggregate.rs) |
| R04 | [lir/target.rs](../../compiler/lir/src/target.rs)、[backend.rs](../../compiler/lir/src/backend.rs)、[target_selection.rs](../../compiler/lir/src/target_selection.rs) | LLVM data layout、CPU、backend 合同及部分栈寄存器事实写在 LIR 内，具体目标组合由 LIR 决定 | 区分纯数据描述、目标登记和后端实现 |
| R05 | [identity/capability/target.rs](../../compiler/identity/src/capability/target.rs)、[capability.rs](../../compiler/identity/src/capability.rs) | target、backend、C toolchain 的支持集合在公共身份层封闭列举 | 保留 typed identity 和格式规则，支持列表移到实际消费者 |
| R06 | [lir/c_bridge_toolchain.rs](../../compiler/lir/src/c_bridge_toolchain.rs)、`lir/c_bridge_invocation.rs` | LIR 既保存 profile，又选择具体平台并构造 C 编译进程命令 | 纯产物数据留在 IR，宿主工具链与执行行为移到 [toolchain](../../compiler/toolchain/src/c_bridge_invocation.rs) |
| R07 | [codegen/artifact/elf.rs](../../compiler/codegen/src/artifact/elf.rs)、[EH relocations](../../compiler/codegen/src/artifact/eh/elf_relocations.rs) | ELF 固定为 X86_64，AArch64 ELF EH 被当作不可能组合 | ELF 容器逻辑与架构指令、重定位逻辑分开 |
| R08 | [slib ELF reader](../../compiler/slib/src/link_object/elf.rs)、[envelope](../../compiler/slib/src/link_object/elf/envelope.rs)、[relocations](../../compiler/slib/src/link_object/elf/relocations.rs) | ELF machine、重定位及 `e_flags == 0` 使用 amd64 合同 | 由选中目标的对象规则解释，不作为通用 ELF 规则 |
| R09 | [Linux C toolchain](../../compiler/toolchain/src/linux_c.rs)、[final image](../../compiler/toolchain/src/final_link/linux/image.rs)、[linker ELF](../../compiler/linker/src/final_image/elf.rs) | `REALGCC`、probe、loader 名称及 `R_X86_64_RELATIVE` 写死 | 分离架构、libc、工具链和最终链接事实 |

基线的实现规范 2.4 要求 LIR lowering 计算物理 C ABI，2.5 又要求 codegen 只消费完整 LIR。实施前已同步修订[现行 2.4](../specs/SCOOP-IMPL-SPEC.md#24-lir)和 [2.5](../specs/SCOOP-IMPL-SPEC.md#25-codegen-与对象合同)：通过独立的 ABI 规划入口和 driver 编排移动目标算法，LLVM 发射仍只消费完整 LIR。

## 2. 重构后的职责边界

### 2.1 按实际差异分离

| 范围 | 所有者 | 传递给其他层的内容 |
| --- | --- | --- |
| 源码语法、类型规则、泛型具体化、控制流和协程执行语义 | parser / HIR / MIR 及其 lowering | 已有完整语义 IR；不加入目标架构 |
| 具体目标的标量/指针布局、过程 ABI、LLVM machine 配置 | codegen | 完整目标布局描述、Scoop/C ABI 计划 |
| 通用结构布局、值表示、逻辑 roots、typed 调用和存储构造 | lir-lower | 消费目标事实及已分类 ABI，输出完整 LIR |
| 类型、布局、ABI carrier、target/backend 身份和产物合同记录 | identity / lir | 纯数据、规范化编码及必要的局部结构检查 |
| 支持目标、CLI alias、host 默认值、源码条件、C/C++ 工具链、runtime 构建与缓存 | driver 相关代码 | 当前请求的明确目标和实际使用的工具输入 |
| 对象容器、架构重定位、产物兼容、artifact-only 链接 | slib / linker | 已读取的对象事实和实际链接要求；不依赖 LLVM |
| anchor、机器栈根、ABI shim、OS/VM、已加载 image | runtime | 现有 runtime API 和相应平台实现 |

架构、过程 ABI、OS、libc、对象格式和链接模式是不同事实。不得继续使用 `target != Darwin` 推导 SysV AMD64，也不得从 ELF 推导 x86-64、从 AArch64 推导 Mach-O、从 GNU 推导某个固定动态加载器。

这些区别服务现有两个架构和两种对象格式，不建立任意组合平台的插件系统。driver 仍只开放实际实现并验收的完整 target；拆开内部职责不表示用户可以任意拼接 ABI、OS 和 libc。

### 2.2 IR 保留数据，不提供目标实现

`lir` 可以保留 `AbiValue`、`AbiPart`、`AbiCoercion`、完整调用签名、scalar/pointer layout 和 backend 合同的描述与编码。offset、extent、alignment、引用类型和计划完整性的局部检查仍属于数据边界。

以下内容移出公共 IR：

- 按具体 target 选择 HFA、eightbyte、寄存器耗尽、间接参数或整数扩展的算法。
- LLVM 初始化、CPU/features 默认值、具体 data layout 的登记与目标组合选择。
- C/C++ 可执行文件发现、环境变量处理、SDK/sysroot 路径和 `Command` 构造。

不以函数指针、trait object、回调或通用属性表把相同实现藏回 IR。codegen 也不通过依赖 lir-lower 实现 crate 来调用上游 stage；stage 间继续以 IR 数据通信，由 driver 编排。

## 3. 完整 LIR 的构造与 ABI 分类

### 3.1 采用显式的数据交接

保留通用 lir-lower，拆开当前交织的布局准备、目标 ABI 分类和正文完成三个步骤。下列名称描述职责，不要求机械新增同名 crate：

```text
已选择的 target
    ├─ toolchain：构建与链接配置
    └─ codegen：完整目标布局描述

完整 MIR + 已选择的依赖 LIR + 目标布局描述
    → lir-lower：布局与逻辑签名准备
    → 完整的布局/签名输入数据
    → codegen：目标 ABI 分类
    → 完整的 ABI 计划集合
    → lir-lower：正文、存储、roots 与调用构造
    → 完整 ConeLirOutput
    → codegen：LLVM 发射与对象生成
```

driver 负责这些调用的顺序。codegen 的 ABI 入口只消费 `lir` 定义的布局和签名数据，不读取 HIR、执行泛型推断或重新解释 MIR 正文。LLVM 发射入口仍只接受完整 LIR；新增的是发射之前的目标规划入口。

准备结果与最终 LIR 是不同的数据类型。准备结果完整表达自己的布局与签名输入职责，不伪装成一个缺失 ABI 的 `lir::Module`，也不使用 `Option<Signature>`、默认 ABI 或待填列表满足输出类型。

已有布局、exact type、scalar leaves 和扫描信息在准备时完成一次，后续直接复用。聚合输入以 typed 字段引用共享已准备的存储节点；不按逻辑字节数展开大型 tagged payload，也不复制嵌套值布局。后端只为可能使用寄存器的聚合读取其标量叶子。不得为了得到 ABI 先跑一遍完整 lowering，再丢弃正文重跑；也不复制一套语言规则来预判 ABI。

### 3.2 覆盖所有真实签名来源

当前 ABI 分类不只出现在顶层函数定义。实施时必须沿现有调用点迁移以下来源：

- 本地函数、已具体化泛型、普通 adapter、Scoop ABI extern。
- virtual/interface slot，包括 interface 的 object-only receiver projection。
- closure invoke、function bridge，以及 suspend 变换后含 continuation 的执行签名。
- C extern 的 DirectC、StorageBridge 入口与 callback 的实际调用边界。
- root/init gateway、release hook 和 runtime helper 的已知机器签名。
- 已选择的外部 callable、native contract 和跨 Cone ABI 导出。

重点入口包括 [lowering.rs](../../compiler/lir-lower/src/lowering.rs)、[native_abi.rs](../../compiler/lir-lower/src/native_abi.rs)、[external_callables.rs](../../compiler/lir-lower/src/external_callables.rs)、[interface receiver](../../compiler/lir-lower/src/function/interfaces.rs)、[indirect dispatch](../../compiler/lir-lower/src/function/call/dispatch.rs) 和 [runtime call protocol](../../compiler/lir-lower/src/function/call/protocol.rs)。不能只搬走 `abi.rs` 后仍在这些位置按目标重新分类。

签名准备复用这些真实调用路径的逻辑签名构造，按现有 callable、function type、slot、runtime entry 等 typed 身份关联。需要批次内索引时使用专用局部 ID，不用 FQN、symbol 拼写或参数个数寻找计划，也不把内部索引变成新的持久化实体身份。

同一计划供定义、调用、adapter 和导出复用。导入的布局和物理 ABI 从已选择的依赖 LIR 读取，完成消费者本地类型引用的映射；不按消费者源码重新分类后覆盖 provider 的计划。必要的目标、exact type、签名与引用一致性检查保留。

### 3.3 分类必须以完整签名为单位

SysV AMD64 的隐藏 sret 和前序参数影响后续 aggregate 能否使用寄存器，因此不能把目标接口降成互不相关的 `classify(type)`。Scoop ABI 和 DirectC 都以完整参数序列、结果及已有逻辑类别作为输入。

计划继续明确表示：

- Unit/ZST、scalar、interface parts、普通 aggregate 的直接或间接形式。
- 每个 carrier 的类型、offset、extent、alignment 和 pointer provenance。
- 参数与结果各自的 coercion、隐藏参数顺序和物理参数位置。
- C 参数的 byval 或 caller-copy、实际副本对齐，以及标量的 extension。
- Scoop 的 managed reference、接口 receiver 与普通 C 值之间的区别。

保留现有小值阈值和语言语义。整数扩展规则、HFA 连续性、aggregate 整组回退等算法及其目标相关校验集中在 codegen；[当前 C plan 校验](../../compiler/codegen/src/validation/c_call_plan.rs) 不再维护另一份脱离分类入口的目标规则。

实际布局与物理 ABI 已完成后，lir-lower 只按计划建立独立参数副本、返回存储和 typed 调用。复制范围、padding、GC roots、NativeSafe/NativeBorrowed/GCLeaf 协议继续由各自现有规则处理，不能因移动分类算法改变求值顺序或 native transition。

## 4. 目标身份、登记与兼容数据

### 4.1 公共身份不再列举支持目标

基线的 `TargetProfileId::ALL` 和 `TargetProfileWireId::refine` 将身份格式与编译器支持列表绑定。backend 和 C toolchain 的 refinement 也列举具体实现；这些支持列表现已移出公共身份层。

M39 保留独立的 typed target/backend/toolchain identity，以现有 canonical 编码表达名称与版本；将“这个版本能否编译、读取或链接”交给相应消费者的有限支持表。具体 target 名称、triple alias 和 host 选择位于 driver 相关代码，具体 ABI/LLVM 实现位于 codegen，对象读取能力位于 slib。

格式合法的 ID 不等于支持该 target。未知或不相容的目标必须在对应请求或产物消费边界清楚诊断，不能回退到 host、Darwin 或 amd64。保留不同身份类型，不能改成在各处自由传递的普通字符串。

已有三个 target 的 canonical triple、持久化身份和实体派生关系应保持。不能因 Rust 表示从枚举改为数据记录，就重命名已有 target、合并 GNU/musl 或使现有 exact/layout identity 无故改变。

### 4.2 描述与消费者各取所需

目标布局描述包含通用 lowering 实际需要的 scalar/pointer size、alignment、byte order 和现有对象边界要求。具体 LLVM data layout 和 backend 配置由 codegen 提供并核对，不再由 `lir` 按 target 名称硬编码。

产物继续保存消费所需的目标、ABI、backend 合同和指纹。artifact-only 路径使用已保存的布局/ABI、所选兼容合同及对象读取规则，不加载 codegen，不创建 TargetMachine，不重新运行 ABI classifier。

仅移动代码、改变内存结构或职责归属，不自动提升 wire/schema/runtime ABI。编码内容或可消费合同确实变化时，只升级受影响的格式并给出明确兼容诊断。不得统一更换所有指纹来掩盖分类结果变化；也不能在实际合同变化时伪称兼容。

### 4.3 manifest 条件不依赖后端枚举

[manifest 条件解析](../../compiler/manifest/src/selection/parse.rs) 在基线中从 `TargetProfileId::ALL` 推导允许的 os/arch/env，再把谓词保存为一组具体 target。目标支持集合移出 identity 后，这一输入由 driver 的目标目录明确提供，不让 manifest 的生产依赖包含 codegen 或 LLVM。

保留未知键/值诊断、同键数组取“或”、不同键取“与”和按构建 target 选择源码的语义。不把原先显式的源码条件默默变成宿主条件。若调整谓词的 canonical 表示，必须同步现有 manifest 语义投影和缓存键；未选中源文件仍不成为编译输入。

## 5. codegen、对象格式与机器规则

### 5.1 codegen

在现有 `target`、`abi`、`artifact` 等模块内整理职责，不新建通用 target 插件层。具体目标选择明确得到完整后端配置；公共 LLVM 发射消费它，不散布按 OS 推断架构的分支。

架构相关规则包括 LLVM backend 初始化、CPU/features、过程 ABI、指令边界和调用指令、return PC、SP/FP DWARF 编号、帧记录及 root slot 范围。对象格式相关规则包括 section/symbol、容器读取和 relocation 表组织；具体 relocation 的意义仍由架构决定。

清除“所有 ELF 都是 X86_64”“AArch64 EH 只能是 Mach-O”一类跨维度判断。M39 实现已有 Mach-O/AArch64 与 ELF/x86-64 的明确分派；未实现的组合仍在支持边界拒绝，不添加空的 AArch64 ELF handler。

后续 Linux ARM64 不能仅复用 Darwin 的 triple 外壳；新增 RISC-V 64 或 LoongArch64 也不能仅初始化 LLVM target。实际移植还要实现并验证所需的 ABI、statepoint、stackmap、机器帧及 EH 规则。

### 5.2 slib 与 linker

ELF 的 header、section、symbol、RELA、COMDAT 等容器处理保留共有代码。以下内容由所选对象架构规则决定：

- machine 和合法 `e_flags`，不是统一要求 `e_flags == 0`。
- relocation 种类、写入范围、addend、调用/地址/TLS 引用含义。
- stackmap 函数指针及 EH/personality 指针的 relocation。
- 最终 PIE 中 metadata 指针的动态 relocation。

迁移 [stackmap architecture](../../compiler/slib/src/link_object/stackmap_normalization/architecture.rs)、[pointer relocation](../../compiler/slib/src/link_object/relocation_verification/pointer.rs) 和 [reference semantics](../../compiler/slib/src/link_object/relocation_verification/semantics.rs) 中散落的假设时，共用已有对象事实与读取结果。未来目标的 relocation 若需要多个记录共同表达，应按真实格式扩展局部模型，不能直接套用 amd64 的单字段规则。

codegen 的机器输出检查与 slib 的外部对象输入检查有不同边界；保留各自必要职责。相同且未改变的已解码事实直接复用，不为了证明上一阶段完成而增加完整 ABI、布局或机器代码重放，也不建立第二套来源证明或资格状态。

## 6. driver、工具链与 runtime

### 6.1 工具链执行归属

把 `ValidatedCBridgeToolchainInvocation` 中的宿主路径、环境变量和命令构造移到 toolchain；LIR 只携带目标代码生成和产物兼容实际需要的合同数据。generated-C source 仍由 codegen 产生，其编译由 driver 以所选工具链完成，结果按既有对象边界读回。

target 描述分别决定 native compiler 配置、GNU/musl 环境、默认链接模式、实际 loader、unwind 输入和 runtime 源文件组合。现有约束继续保持：不交叉混用 GNU/musl，不回退到宿主头文件或库，不把 glibc 静态、musl static PIE 或 musl C++ 顺带开放。

`REALGCC`、C/C++ 配套 driver 和对象 probe 均从当前选择的工具链解释，不能要求所有 Linux 请求都能发现 `x86_64-linux-gnu-gcc`。library 编译仍不要求无关的 CRT、最终 linker 或 unwind archive；artifact-only link 仍不启动源码编译器。

目标、优化模式、实际工具链输入与 runtime 配置继续进入对应缓存键。新结构不增加通用构建框架，也不把不参与本次构建的其他目标工具链纳入缓存输入。

### 6.2 runtime 边界

runtime 不受“编译器改动只在 codegen”这一目录约束。现有 [platform 接口](../../runtime/src/platform/platform.h) 已分出 metadata image、OS/VM 和 managed frame，可继续沿用。

M39 只整理当前真实的平台职责：机器帧、anchor、返回值 shim 和浮点环境控制归 arch；线程栈/VM 归 OS；加载映射和 metadata 归 image。特别是 [floating.c](../../runtime/src/floating.c) 的架构条件应收拢到实际架构实现，不让未来扩展继续向通用浮点转换代码增加分支。构建源文件和 flags 按这些真实组件组合；`-mno-red-zone` 只属于需要它的架构配置。

不改变 collector、线程握手、对象布局、GC root 协议或公共 runtime ABI。OS、image 和 arch 的可复用部分以实际实现为准，不为尚未实现的目标创建空 bundle。

后续移植需要另外完成以下工作，本次不宣称已经具备：

- Linux ARM64 的 ELF 汇编符号、CFI 和函数边界；现有 [aarch64_anchor.S](../../runtime/src/platform/arch/aarch64_anchor.S) 使用 Darwin 前导下划线及 `.subsections_via_symbols`，不能直接复用为 ELF。
- 每个新架构的 managed caller anchor、可写精确 root 定位、逐帧遍历和 unwind 配合。
- String/UTF-8/浮点 Option 等后备的 Scoop ABI 返回适配及浮点环境。
- 对应 libc 的 LLVM libunwind 构建和实际异常运行验证。

## 7. 规范调整清单

开始实现前按 spec 先行约定处理下表；实施中的权威合同以三份规范为准。

| 文档 | 本次同步的内容 |
| --- | --- |
| 实现规范 1、2、2.4～2.5 | 区分目标描述、布局准备、ABI 规划和完整 LIR 发射；明确 codegen 目标算法与 driver 编排职责 |
| 实现规范 2.6～2.8、2.11 | typed target 身份、产物兼容、工具链执行归属、artifact-only 路径及按实际变化处理格式版本 |
| 语言规范 12.2、13.8 | 必要时同步 manifest 条件允许值的来源和 ABI 阶段引用；保持当前合法源码集合与 FFI 语义 |
| 运行时规范 3.2、5、6.1 | 核对现有机器栈根、异常和浮点合同保持；仅职责移动不新增架构合同或提升 runtime ABI |
| ROADMAP | 登记 M39 边界重构及实际完成状态 |

现有 64 位 pointer、ZST、interface、对象头、最大 managed alignment、AS1 与 GC/EH 合同在 M39 保持。不得借此次整理扩展成支持任意 pointer width、任意 OS 或任意 ABI 的框架。

## 8. 实施顺序

每一步围绕真实编译和产物路径完成，不保留两套可选的生产 ABI 分类器。

1. **记录基线与调整规范。** 记录当前三个目标的 ABI、产物和实际验证范围；先确定第 3 节的阶段输入输出及第 4 节的身份兼容合同。
2. **拆开目标数据与工具行为。** 移动具体目标登记、LLVM/backend 定义及 C toolchain 执行逻辑；公共 identity/LIR 保留 typed 数据。同步 manifest/protocol/缓存消费者。
3. **迁移 ABI 规划与完整 LIR 编排。** 从现有布局和签名准备中提取数据交接，迁移 Scoop/C 分类器，覆盖全部签名来源、导入及导出路径，删除 lowerer 和 IR 中的机器算法。
4. **分离对象格式与架构规则。** 整理 codegen/slib/linker 的 machine、relocation、stackmap、EH 和动态指针规则，保持现有目标读写结果。
5. **整理工具链和 runtime 组件选择。** 去掉 Linux 与 amd64 的隐式等价关系，保持实际 compiler、libc、loader、unwind 和 runtime 输入相容。
6. **完成回归与边界审查。** 用现有三个 target 的真实程序和产物验证；记录后续每个新目标所需修改位置，不用虚构 target 或只供测试的实现验收架构。

不把整套 lir-lower 合并进 codegen，也不让 slib/linker 链接 LLVM 来减少表面修改位置。通用布局和正文构造继续保留其职责，目标差异通过明确数据和 codegen 算法处理。

## 9. 验收

### 9.1 边界验收

- parser、HIR、MIR 及其 lowering 没有新增架构分支或目标 ABI 知识。
- lir-lower 不再按 target 名称执行分类算法；`lir`/`identity` 不再登记具体 LLVM CPU/data layout 或提供机器分类、工具执行实现。
- 所有 ABI 来源经过同一目标规划路径，最终 LIR 的必要 layout、signature、root 和物理计划非可选且完整。
- 已选择的依赖 ABI 可正常绑定、再次发布和链接，不由消费者重新分类或按名称回退。
- artifact-only 链接不需要 codegen、LLVM 库或 LLVM 可执行工具；现有 library 编译的最小工具要求保持。
- 未实现目标在正式入口明确报错。目标登记、后端和对象 reader 的差异不能靠默认值掩盖。

搜索具体 target 名称只是审查手段，不是单独的验收结果。IR 数据记录、已有测试和错误诊断中的目标名称，不等于生产算法泄漏。

### 9.2 功能回归矩阵

| 范围 | 必须覆盖的真实组合 |
| --- | --- |
| Scoop ABI | scalar、ZST、interface、niche、GC-free 小 aggregate、含引用 aggregate、间接结果、寄存器耗尽及后续参数 |
| C ABI | 窄整数/Boolean 扩展、Float/Double、nested/packed/aligned CLayout、HFA、混合 aggregate、byval、sret、caller-copy |
| 调用形式 | direct、virtual、interface receiver projection、closure、function bridge、generic、suspend 执行入口、release、root/init gateway |
| native 边界 | DirectC、errno StorageBridge、callback、Scoop ABI extern、GCLeaf、NativeSafe/NativeBorrowed、global/TLS |
| GC/EH | 普通 statepoint、invoke 正常/异常出口、接口结果 relocation、moving GC、深栈/参数压力、catch/finally、Context/协程 |
| 产物 | provider/consumer 分开编译、重建 core、ODR、native 对象/归档/DSO、artifact-only link、debug/release 组合 |
| 目标与工具 | Darwin、GNU PIE、musl 静态/动态；显式 target、host 默认、manifest 源码选择、缓存目标隔离及错误工具输入 |

优先复用现有独立和组合 fixture。缺少覆盖的实际分支补独立 fixture；新增目标/工具/产物错误规则补对应 negative fixture，检查诊断。HIR/MIR/LIR golden 保持结构合同；机器 ABI 必须同时核对真实对象及 native 互调，不能仅以 dump 相同作为证明。

格式化、lint 和验证遵守仓库顺序：Rust 先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再运行相应测试；Python 先按仓库指定 Ruff 版本格式化和检查。正式验收包括 `cargo test --workspace`、三个 CLI binary 构建、fixture runner unittest 和 `python3 tests/run_fixtures.py --all` 的适用目标覆盖。结果按实际执行的 host、target、链接模式和 profile 记录，不把未执行平台计为通过。

仅设计文档阶段只检查本文内容、引用和修改范围，不执行会改动代码或批量快照的操作。M39 完成时，应能明确说明新增 Linux ARM64、RISC-V 64、LoongArch64 的后续工作分别落在 codegen、driver/slib 相关代码及 runtime；实际移植仍需各自的 LLVM 输出、ABI、GC/EH 和运行验证。

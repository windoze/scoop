# Scoop 原生调试信息与独立调试文件设计

日期：2026-10-29。

状态：本轮基础调试、程序栈诊断、LLDB／lldb-dap 打包和 VS Code 集成已完成，实际验证及平台限制见 [实施记录](IMPLEMENTATION.md)。基础调试合同见实现规范 2.22；用户确认的 Throwable 历史栈快照与 trap 栈打印见 [栈诊断设计](STACK_TRACES.md) 和实现规范 2.21。本文保留初始调研依据及后续语言专用调试设计；完整求值、managed 写入和协程逻辑栈不属于已交付能力。

## 1. 结论与范围

将现有 map 功能的请求传递、伴随输出、最终结果缓存和发布通路改造成“独立调试文件”通路，复用现有 linker 的目标分支。源码位置、变量和类型信息则从前端经 MIR / LIR 传到 LLVM，由 LLVM 生成标准调试信息。

| 目标 | 对象中的调试信息 | 最终调试产物 | 生成方式 |
| --- | --- | --- | --- |
| Darwin/AArch64 | Mach-O 中的 DWARF | `<output>.dSYM/` | 保留 debug map 的最终链接，再执行 dsymutil |
| Linux/amd64 GNU、musl | ELF 中的 DWARF | `<output>.debug` | 完整链接，再用 llvm-objcopy 分离，程序保留 .gnu_debuglink |
| Windows 目标 | COFF 中的 CodeView | PDB，命名为 `<output>.pdb` | 支持 Windows 的 linker 在链接时生成 |

现有文本 `.map` 只记录链接地址、范围和符号，不能恢复已经丢失的源码行、词法作用域或变量类型。改造可以复用输出管理，但调试内容必须在对象生成前建立。Darwin 的 Mach-O **debug map** 是 dsymutil 使用的对象定位信息，与当前 `-map` 生成的文本文件是两个不同概念。[R1][R2]

首轮交付覆盖当前三个目标的源码断点、单步、物理调用栈、静态类型与可用局部值，以及 lldb-dap 的直接接入。String、数组、enum 等值的友好展示已纳入本轮。完整 Scoop 表达式求值、Scoop 专用异常过滤和协程逻辑调用栈单独分阶段实施。

2026-10-29 本轮实施范围为第 12 节 A～C、Throwable 历史栈快照和 trap 栈打印，并完成发行包中的 LLDB / lldb-dap 及运行依赖、Python formatter 和 VS Code 集成。协程逻辑栈在基础链路完成后评估实际映射与工具复杂度；完整求值、managed 赋值和 GC 感知数据断点留待后续阶段。本地全部功能完成后，再在独立 Linux checkout 验证 GNU／musl，使用定向编译器与工具协议用例，不反复执行无关的完整测试套件。

程序自身的异常／trap stack trace 是相关的独立能力：Throwable 在首次抛出时保存快照，printStackTrace 和未捕获报告读取原栈，trap 即时采集并打印。采集不依赖 LLDB 或 DWARF；源码符号化复用本设计的调试文件。其布局、GC、符号器与实施顺序见 [栈诊断设计](STACK_TRACES.md)。

Windows target、COFF、调用 ABI、runtime、GC / EH 与链接支持由 [Windows 移植](../windows-targets/DESIGN.md)实现；调试信息复用本设计的共享源码记录，选择 CodeView／PDB 后端。实际 Windows 验收范围和剩余工作见[实施进度](../windows-targets/PROGRESS.md)。

## 2. 实施前的仓库基线

以下记录设计开始时的代码状态，用于解释实施动机，不是当前缺口清单。最终实现与验证见实施记录。

| 部位 | 当前能力与缺口 | 主要代码 |
| --- | --- | --- |
| HIR 源码信息 | Expr 有 span 和定义／求值来源；Local 有名称、类型、Source / Synthetic 定义分类。已有 SourceRecord 保存行起点及位置点 | [body.rs](../../compiler/hir/src/body.rs)、[source_interfaces.rs](../../compiler/hir/src/source_interfaces.rs)、[source_record.rs](../../compiler/hir/src/source_record.rs) |
| MIR | Statement 保留 SourceSpan；Expr 只有类型和操作。局部值身份已有专门关系，但尚无供调试消费的完整位置、作用域和变量绑定数据 | [control_flow.rs](../../compiler/mir/src/control_flow.rs)、[local_values.rs](../../compiler/mir/src/local_values.rs) |
| LIR | Function、BasicBlock 和指令没有调试位置／作用域；Local 保留显示名和存储表示。statement lowering 只消费 statement.kind | [function.rs](../../compiler/lir/src/function.rs)、[instruction.rs](../../compiler/lir/src/instruction.rs)、[statements.rs](../../compiler/lir-lower/src/function/statements.rs) |
| LLVM codegen | 未找到 DIBuilder、DICompileUnit、DILocation 或局部变量调试记录的发射代码 | [codegen](../../compiler/codegen/src/lib.rs) |
| debug profile | machine O0、generated-C O0；debug 仍执行 SROA、mem2reg 等必要转换。它目前不代表源码调试信息 | [optimization.rs](../../compiler/lir/src/optimization.rs)、[policy.rs](../../compiler/codegen/src/statepoint/policy.rs) |
| 栈展开基础 | 已设置 frame-pointer；Managed 函数禁用机器尾调用；已有 GC / EH 展开合同。需要复用并实测调用栈连续性 | [policy.rs](../../compiler/codegen/src/statepoint/policy.rs)、[runtime.rs](../../compiler/toolchain/src/runtime.rs) |
| generated-C | canonical flags 显式选择 NoDebugInformation，实际传入 -g0；另有禁用 unwind tables 的现有设置 | [flags.rs](../../compiler/lir/src/c_bridge_toolchain/flags.rs)、[command.rs](../../compiler/toolchain/src/c_bridge_invocation/command.rs) |
| map 与 strip | debug 默认保留符号并生成 map；Darwin release 使用 -x -S，ELF 使用 --strip-all | [executable.rs](../../compiler/scoop/src/build/executable.rs)、[darwin.rs](../../compiler/linker/src/link/darwin.rs)、[elf.rs](../../compiler/linker/src/link/elf.rs) |
| 结果管理 | map 已有缓存键、结果路径与发布通路，但 OutputLocation.publish 通过文件复制发布，不能直接发布 dSYM 目录包 | [key.rs](../../compiler/scoop/src/build/executable/key.rs)、[record.rs](../../compiler/scoop/src/build/executable/record.rs)、[materialize.rs](../../compiler/scoop/src/materialize.rs) |
| 编辑器 | VS Code 扩展已有 LSP、语法和格式化贡献，没有 breakpoints / debuggers 贡献或调试启动配置 | [package.json](../../tooling/vscode-scoop/package.json) |
| 性能消费方 | 原先依赖 map 地址映射；本轮已迁移到 executable、独立 DWARF 与精确 linkage name | [symbolize.py](../../tests/benchmarks/symbolize.py)、[dwarf_symbols.py](../../tests/benchmarks/dwarf_symbols.py)、[PROFILING.md](../../tests/benchmarks/PROFILING.md) |

实施前的[实现规范 2.8](../specs/SCOOP-IMPL-SPEC.md) 规定 map 不生成 DWARF，且 Darwin 在 linker 内 strip、签名后不再改程序。本轮已先行修订对应合同，独立调试文件的现行规则见 2.22。

## 3. 工具调研与最小实验

LLVM 22.1 的标准调试元数据和 debug records 足以承担底层表示；inkwell 0.10 已提供编译单元、文件、函数、词法块、基础／聚合类型和变量记录接口。复杂类型所需的具体绑定以实际实现为准，优先使用已有 inkwell / llvm-sys，不新建调试格式 writer。[R1]

设计阶段在 macOS/AArch64 做了以下独立工具实验；其结果不能替代后续真实 Scoop 验收：

- LLVM、llc、dsymutil、llvm-dwarfdump：22.1.8。
- LLDB：Apple lldb-2103.0.34.103；本机 lldb-dap 报告 LLVM 21.0.0。
- 输入为手写 LLVM IR，关联一个包含 main 和 Int 局部值的 probe.scoop；没有调用 scoopc，没有 Scoop runtime 或 GC。
- 所有试验文件和原始日志保存在仓库外；这里只记录结果与限制。

| 实验 | 结果 | 可以支持的结论 |
| --- | --- | --- |
| 发射 DWARF 5、链接 Mach-O、生成 dSYM | 成功；llvm-dwarfdump --verify 无错误 | LLVM 22.1.8 与本机工具的基础格式链路可行 |
| DW_LANG_C、DW_LANG_C_plus_plus、数值 0x8000 分别发射相同小程序 | 三者均能按 probe.scoop:3 命中断点并读到 answer = 42 | 扩展名和 Kotlin 风格源码文本不妨碍行表断点；此结果不证明 LLDB 理解 Scoop |
| 直接命名为 Int 的 DIBasicType | LLDB 显示为 int | 调试器可能规范化基础类型名 |
| 用 typedef 将基础 signed i32 显示为 Int | LLDB 与 DAP 都显示 Int | 源码基础类型名应通过调试类型别名保留 |
| 查询 LLDB 实际使用的 symbol file | 路径为生成的 dSYM 内 DWARF 文件 | 断点实验确实加载了外置 dSYM |
| lldb-dap 的 initialize、launch、setBreakpoints、configurationDone、stackTrace、scopes、variables | 成功；断点 verified，局部值为 Int / 42 | 初始 DAP 接入可以直接复用现成 adapter |
| DAP evaluate(answer)、evaluate(answer + 1) | 分别为 42、43；后者类型显示 int | 现成求值器能处理这个 C 兼容例子，不能据此承诺 Scoop 表达式语义 |
| DAP 初始化的异常过滤列表 | C++、Objective-C、Swift 的 catch / throw；没有 Scoop | Scoop 异常过滤需要额外支持 |
| llvm-symbolizer 使用 executable、dSYM 和 linkage / JSON 输出 | 返回 main、probe.scoop 第 3 行第 5 列及函数起始地址 | 可用标准工具接替 map 的地址归因基础 |

上述最小实验未覆盖 Linux 分离后的实际程序调试、复合类型、优化后的变量位置和 moving GC；后续 Scoop 验收的实际范围见实施记录。Windows PDB 的移植进度见 [Windows 实施记录](../windows-targets/PROGRESS.md)。编译器后端与 debugger 的 LLVM 主版本不必相同，兼容性应通过目标格式和实际调试行为确认。

## 4. 用户接口与默认行为

### 4.1 源码构建

`scoop build/run/test` 和 `scoopc` 提供明确的生产选项：

```text
--debug-info=none|line-tables|full
```

| 值 | 编译对象中的内容 | umbrella 产生 executable 时 |
| --- | --- | --- |
| none | 不生成调试元数据 | 不生成独立调试文件 |
| line-tables | 文件、行列、函数位置及必要的内联位置关系 | 生成目标平台的独立调试文件 |
| full | line-tables，加静态类型、作用域、参数、局部值和全局量描述 | 生成目标平台的独立调试文件 |

默认值：debug 为 full，release 为 none。显式选择覆盖默认值，因此 release + full 是正式支持的组合；调试级别不改变 O0 / O2 的选择或程序 ABI。

源码构建示例：

```sh
scoop build app
scoop build app --release --debug-info=full
scoop build app --release --debug-info=line-tables
scoop build app --debug-info=none
```

library 的调试信息保存在其 .slib 内对象中。选定级别沿本次需要源码编译的构建图传递，已选用的预编译输入不因此自动重建。独立 scoopc 只负责当前 Cone 的编译对象，不负责最终 dSYM / .debug。

### 4.2 仅消费产物的链接

`scoop link` 和独立 `scoop-link`（package 为 scoop-linker）使用 `--emit-debug-file`，只请求从已有对象生成独立调试产物，不接受暗示能补齐变量信息的 full 生产选项。

已有输入可以混合不同调试级别；输出只包含实际选中对象能够提供的信息。链接不能为没有调试信息的预编译库补造源码位置，也不自动重编译它。没有源码调试内容的输入不因此获得源码级调试能力；工具无法生成请求的文件时正常报告错误，不为这项请求增加全闭包 DWARF 验证或逐次缓存审计。

`--keep-symbols` 保留现有“是否在 executable 内保留普通符号表”的含义。它与外置调试信息独立：release 可以使用裁剪过普通符号表的程序，同时具有完整外置调试文件。

### 4.3 替换 map

在独立调试文件闭环可用并迁移消费者后，移除 `--emit-symbol-map`、默认 map 输出和对应内部字段，更新文档及 fixture。新功能使用准确的新名称，不把旧选项静默解释成另一种产物。

保留 `--emit symbols` 的逻辑符号观察职责。它仍可给性能报告补充实体名称、角色和泛型实例信息，调试文件则提供地址到源码及 linkage name 的映射。

## 5. 编译管线的数据设计

### 5.1 从源语义保留到最终发射

调试数据只描述已有程序，不另建一套语言语义系统。各 stage 经输入／输出 IR crate 传递数据；codegen 不反查 HIR lowering 或 parser。

需要保留五类信息：

1. **文件与位置**：复用 SourceIdentity、SourceRecord、span 和现有位置换算。byte span 必须绑定所属文件，不能仅凭函数名猜测来源。
2. **源码函数与作用域**：函数声明／定义位置，词法父子关系，以及生成函数对应的实际源码 callable。
3. **源码变量**：复用已有 binding／局部值身份，保存名称、声明位置、源码类型和参数序号，区分用户变量与 Synthetic。
4. **操作位置**：MIR 表达式、语句、分支和返回，以及 LIR 指令和 terminator 的位置。清理代码、poll、桥接等由显式生成分类表达，不能套用上一条用户指令的位置。
5. **变换关系**：泛型实例的定义位置，内联调用链，局部重映射，closure capture 及协程 saved slot 与源码变量的关系。

调试关闭时可整体不建立后端调试数据；启用后，记录应在结构上完整。真正没有用户位置的生成操作使用明确分支，避免到 codegen 再对缺失字段猜测或补零。调试元数据局部索引只负责本层引用，不新增一套与现有实体 ID 平行的全局身份。

这些信息不要求在 HIR、MIR、LIR 中各保存一份完整记录。文件身份、位置、源码作用域和变量声明集中在共享的源码／调试旁表中，节点只携带有类型的局部引用及 Source／Generated 分类。声明和变量身份复用现有 typed identity；旁表索引不是新的全局实体身份。共享记录随输入／输出 IR 传递，由一次编译拥有，后端无需引用上游 stage 实现或保留完整上游 IR。

各阶段单独维护实际发生变化的关系：内联调用点、单态化后的类型、源码变量到 MIR local／LIR 存储、closure capture 与协程 saved slot。共享声明不能代替这些映射，也不能仅凭名称或对最终指令编号的猜测恢复。变换新增或克隆位置时复用未变化的文件／声明记录，生成操作仍显式分类。已有语义来源不因调试关闭而删除；定义来源与求值来源允许引用相同记录，但不能合并其不同用途。

HIR lowering 建立词法作用域和变量声明关系；MIR CFG 化时随真实求值顺序转移这些信息；内联、常量传播和协程变换更新现有绑定；LIR 在存储／ABI 规划完成后将变量关联到实际位置。指令的插入、删除、克隆必须同步处理对应调试记录。

### 5.2 定义来源、求值来源与内联

已有 `DefinitionOrigin` / `EvaluationOrigin` 继续服务原合同。默认参数的定义位置和调用处求值位置都需保留；调试位置及内联链不能覆盖或重写 `currentSourceLocation()` 的编译期结果。

MIR 已经执行的源码内联必须显式保留 callee 定义与调用点链，供 LLVM 形成 `DILocation.inlinedAt` 和 abstract / concrete subprogram 关系。不能指望 LLVM 从扁平化后的指令自行还原 Scoop 内联。

泛型函数各实例保留实际 linkage name 和 concrete 类型实参，同时关联同一源码定义。闭包及其他生成 helper 标记为 artificial，默认单步不反复落到无用户意义的位置。

### 5.3 源码路径

调试路径与语言的 canonical semantic source path 分开：

- 本次有实际源码路径的文件，可直接用该路径生成 DIFile，支持本地直接调试。
- 从 .slib 实例化的正文复用其 SourceIdentity 和位置记录。driver 若已知道所属 Cone 的源码根，提供对应位置；否则保留可映射的逻辑来源。
- 预编译库中的生成路径通过 LLDB sourceMap 映射到本地源码根；不在链接阶段重读源码重建调试信息。
- 本轮不引入源代码服务器、自动下载或专用源码包。源文件不存在时仍可使用符号、类型和反汇编；源码浏览需要用户可用的对应文件。

跨机器的路径重映射可以进入普通调试生产配置。它不改变实体身份、`SourceLocation` 的值或语言可见性。UTF-8 byte offset、源码行列与 DAP 坐标在各自边界显式换算，测试包含中文标识符和 CRLF。

## 6. LLVM 调试信息发射

### 6.1 编译单元、函数与语言标记

每个实际发射的 LLVM module 建立对应的 DICompileUnit、DIFile 和 DISubprogram，并设置目标所需的调试版本 module flags。当前三目标初版采用 DWARF 5，通过阶段 A 的工具验收确认兼容性；没有实际兼容问题时不增加多版本选择开关。函数分别提供可读的源码 name 和精确 linkageName；后者使用既有符号规则，保持性能归因和泛型实例区分。

DIFile 将文件名与目录分开描述，验收实际完整路径，而不只检查行号。运行时符号器的补充实验发现过 DWARF 5 绝对文件名与 comp_dir 被重复拼接的问题；具体结果及源码名／linkage name 的接入要求见栈诊断设计第 7 节。不能删除 linkageName 来迎合某个符号器，或把 opaque 符号标识显示成已恢复的源码函数名。

当前后端按 callable 分开发射对象。沿用该划分，让每个对象包含自身需要的调试单元／类型引用，避免为调试重新合并所有函数或改变 ODR 选择边界。

初期采用 stock LLDB 可读取的 C 兼容 DWARF 类型视图，编译单元使用 `DW_LANG_C`，producer 明确记录 scoopc。基础类型加 Scoop 命名的 typedef；聚合类型描述实际布局。这个选择服务静态内存观察，不承诺 C 表达式具有 Scoop 语义。0x8000 的最小实验不作为正式 Scoop 语言编号。

正式原生 Scoop 语言支持需要独立登记语言编号、增加 LLDB LanguageType / TypeSystem 等支持。届时替换语言标记和对应 debugger 插件，不改变运行 ABI。[R6]

LLVM 22 的变量描述优先按新 debug records 接口使用；inkwell 部分接口名称仍包含 dbg.declare / dbg.value。应验证其 LLVM 22 分支，不把旧版 intrinsic 的使用方式直接套入新接口。完整调试元数据在执行 LLVM 优化前完成并 finalize。[R1]

### 6.2 类型描述

| Scoop 类型 | 调试描述需要表达的内容 |
| --- | --- |
| Boolean、定宽整数、Float / Double、Char | 位宽、符号与编码；用 typedef 保留 Scoop 名称 |
| struct、tuple、继承的 class | 已计算的大小、对齐、字段名与真实偏移；class 包括对象头及基类前缀 |
| 引用、Ptr、FunPtr | 逻辑 pointee／函数类型及实际机器表示；不能因 LLVM opaque ptr 丢掉源码类型 |
| enum | tag、各 variant 和 payload 的实际布局；payload 只在当前 variant 下展开 |
| Option | 复用已选 tagged / niche 表示；普通指针、双字 interface 等按各自真实 niche 解码 |
| ZST | 保留类型、变量与语义值；不为调试强行分配不存在的 payload |
| 泛型实例 | 使用 actual exact type 身份构造并缓存对应 DI type，保留具体实参 |
| interface、Any、closure | 先描述真实静态表示；运行期动态展示属于第 10 节 |

所有大小、偏移和 ABI 分类都来自已有 LIR layout / ScoopAbiSignature。调试发射不能重新按 C 规则计算 Scoop 布局，不能根据物理寄存器参数反推源码参数。被拆成多个参数／寄存器的聚合仍对应同一个源码参数；hidden sret、receiver、continuation 等按实际源码或人工参数分类处理。

当前目标的 managed AS1 引用仍是同一原生内存中的直接指针。调试位置按真实机器地址描述，不把 LLVM 用于 GC 追踪的 address-space 编号直接解释成调试器中的另一种硬件地址空间。

### 6.3 变量位置、优化与 moving GC

有地址的变量通过 declare 记录关联真实存储；SSA 值通过 value 记录关联；拆分的聚合使用 fragment / piece 关系。全局量和 TLS 使用各自真实存储描述。

debug 仍运行 SROA / mem2reg，不能假定所有变量始终位于入口 alloca。release 中被消除或无法定位的值应显示 unavailable / optimized out，不能用历史存储猜值。

Scoop 的额外重点是 moving GC：

- 调试变量在 safepoint 后必须指向当前被重定位的值或回写后的存储。
- 审查 RS4GC 前后的自有聚合拆分、指针合并、restore / reload 和 address-space 处理，保证指令位置和变量绑定随变换更新；不能只依赖 LLVM 通用 pass 处理。
- 不为了调试增加一份保活对象副本，不把 debugger 需要观察等同于程序仍然需要 root。
- 源码词法作用域内、但运行期已没有有效值／root 的引用可以 unavailable；显示 GC 前的失效指针是错误。
- 不跨继续执行保存对象裸地址。formatter 每次 stop 从当前变量位置读取；在 GC 内部停下时，不能假定堆处于完整的 mutator 可见状态。

这些要求通过定向 moving-GC 调试测试验证，不给正常 runtime 热路径增加调试查询或扫描。

## 7. .slib 与预编译库

已有 .slib 存放完整对象，DWARF 可以自然随 LinkObject 保存。Mach-O 的 S_ATTR_DEBUG 和 ELF 的非 SHF_ALLOC section 已归为 ObjectMetadata；沿用普通 section／重定位处理，核对分片与 ODR 处理不会丢失调试关系，不添加调试 section 来源白名单。

供泛型实例化、默认参数和跨 Cone 内联使用的持久模板，还需要保留第 5 节要求的源码位置与变量关系。复用已有 HIR 来源记录，只保存下游实例化真正需要的声明／作用域信息；不把完整 MIR／LIR 调试旁表再写入各层 metadata。外部模板被实例化时，由消费方 codegen 生成该实例的调试信息；不能仅依赖提供方对象里的 DWARF。

编译期旁表集中存放与原生 DWARF 是两个不同的消费边界：旁表减少来源重复，LLDB 仍读取对象及最终 dSYM／`.debug` 中的标准信息。本轮保持 `.slib` self-contained，不引入相邻的外部模板文件；现有来源 wire 的全面索引化、LIR 身份去重及对象粒度调整的收益和成本见[体积调查记录](../slib-storage/FOLLOW_UP.md)。

调试覆盖率不改变 ODR 等价判定或物理定义选择顺序。调试信息必须对应实际保留的 body；不能把被舍弃候选的源码范围套在另一个已选 body 上。

即使提供方机器对象选择 none，其可供后续实例化的模板仍应保留必要的已有来源及新增模板调试结构。修改实际 wire 字段时更新对应 section 版本和生产配置，沿用普通重建，不建立旧／新双轨 reader。

Code 指纹已纳入 LinkObject member 指纹，实际对象中的调试字节变化应沿这条通路使结果更新。不要照抄旧 map“不进入 Code”的条款，也不要另外建立一份调试收据。验证“只改变源码行号、机器行为不变”时仍产生正确的新调试结果。

初期允许没有调试信息的依赖，入口程序和其他有信息的帧仍然可调试。对 core / runtime 的源码单步，需要维护脚本显式准备带调试信息的预编译 sysroot；普通应用继续复用它，不逐 consumer 重建。O2 与调试信息可以共存，runtime 不必因此改成 O0。

generated-C 桥没有用户源码映射，保留 -g0，并通过真实符号和 frame pointer／unwind 数据参与物理回溯，不为人工桥生成伪造的 Scoop 源码行。用户 native 源码按其正常编译配置提供信息。调试桥接不改变原来的调用、异常与 GC 边界。

## 8. 平台调试文件生成

### 8.1 Darwin：链接、dsymutil、strip、签名

在已有 Darwin final-link 分支内按以下顺序完成：

1. 将实际 .slib 成员／native／startup 对象准备为真实链接输入。
2. 生成未裁剪 debug-map 符号的候选 executable。
3. 在这些对象及临时输入仍有效时运行 `dsymutil candidate -o candidate.dSYM`。
4. 若最终请求不保留普通符号，对候选 executable 执行所需 strip。
5. 在最后一次修改 executable 字节后完成平台要求的 ad-hoc signing。
6. 所有生成步骤成功后，发布程序与完整 dSYM bundle。

dsymutil 使用 executable 中的 debug map 回到对象文件合并 DWARF，不读取现有文本 map。最终 dSYM 与 executable 使用标准 Mach-O UUID 关联。[R2]

当前 `-x -S` 必须在请求 dSYM 时延后；特别是 `-S` 不能先移除 dsymutil 所需的信息。当前固定的 `-adhoc_codesign` 也要按最终字节形成的时机安排，不能在已签名程序上 strip 后直接发布。无需修改 executable 的路径可保留现有签名；发生后处理时必须完成最终签名。

这是当前实现规范 2.8 “只在 linker 内清理、不后改签名文件”的必要修订，不以再链接一次或另建发布验证器解决。没有调试文件请求的路径保持现有正常链接配置。

dSYM 是目录包；最终 bundle 及其 Contents/Resources/DWARF 内文件命名都应与发布策略一致。不能把 fs::copy 换一个后缀就当作完整支持。

### 8.2 Linux：先分离，再裁剪

GNU、musl 共用 ELF 调试文件流程，工具按实际目标配置选择。示意：

```sh
llvm-objcopy --only-keep-debug app.unstripped app.debug
llvm-objcopy --strip-debug --add-gnu-debuglink=app.debug app.unstripped app
```

不保留普通符号时，第二步选择相应的 strip-all 策略；保留普通符号时只移除调试 section。两种结果都保留动态链接、GC / stackmap、EH 和加载所需内容。

当前 linker 的 `--strip-all` 在此路径必须延后。若链接时已经删除调试 section，后续 objcopy 无法恢复。[R3]

最终程序带 .gnu_debuglink，记录匹配调试文件的文件名和标准 CRC；GNU／musl 最终链接生成 SHA-1 build-id，供动态及静态程序的模块匹配使用。本轮采用邻接 debug 文件及 debuglink，不增加符号服务器或强制的 build-id 目录布局。

.debug 仍是标准 ELF 调试文件，不能只将若干 section dump 成自定义字节包。runtime 展开信息继续留在 executable 中；外置源码调试信息不替代 EH / 栈展开。

### 8.3 Windows：在链接阶段输出 PDB

Windows 应共享前端到 LLVM 的逻辑调试数据，在 Windows codegen 中选择 CodeView module flags，发射带 CodeView 的 COFF 对象；由 link.exe 或 lld-link 的 `/DEBUG`、`/PDB:` 等选项输出 PDB。LLVM 官方提供从同一类 IR 调试元数据发射 CodeView 的路线，LLD 支持 PDB。[R1][R8]

PE 与 PDB 的路径／GUID／age 匹配按平台规则处理。PDB 通常由最终链接器构造，不能将 ELF 的 objcopy 方案或现成 DWARF 文件直接换一个后缀。

## 9. 缓存、命名与结果发布

复用当前 Executable / ProgramLinkOutput 和 executable record。将 symbol_map 替换为有明确目标种类的可选调试产物；当前只需要 dSYM bundle 和 ELF debug file 两个实际分支，不引入插件式产物工厂。

更新内容：

- 调试级别、实际源码路径重映射配置进入对象生产选项和正常更新键。
- 是否生成调试文件、strip 策略、调试工具及其实际输入进入最终结果更新判断。
- dSYM 和 .debug 的结果位置随 executable 一起记录；按所需结果是否存在判断复用。
- 命中后不读取、哈希或验证调试文件内容，不启动 dsymutil / objcopy 或重新准备链接输入。重型 DWARF 验证只在测试或显式验证中执行。
- 目录包增加必要的暂存、复制／rename 发布支持；工具失败时不发布新结果，沿用普通构建锁和已有失败处理。

**必须处理缓存改名问题。** 当前缓存把程序统一叫 program，最终通过复制发布到任意 -o；map 文本可以随意改名，.gnu_debuglink 中的文件名却嵌在 executable 内。若内部写 program.debug，外部只发布 myapp.debug，调试器不能按这条链接自动找到它。

首版将最终 executable basename 传给调试结果生成，统一决定 debuglink 文件名、dSYM 内文件名及伴随文件名。basename 在请求调试文件时进入最终结果键：不同目录、同 basename 可复用；换 basename 可重新生成最终链接结果，但不重编译上游 Cone。不为仅有这个需求引入另一套中间 executable 缓存。

标准 UUID、debuglink 和 PDB 标识仅用于平台调试文件关联，不扩展为产物来源认证或每次运行前的检查。

## 10. LLDB、lldb-dap 与编辑器

### 10.1 初期直接复用 lldb-dap

debug adapter 处理协议、启动／附加、断点和 UI 请求；它消费 LLDB 的能力。基础调试链路不需要实现 Scoop DAP server，也不需要把 DAP 放进 scoop-lsp。[R4][R5]

VS Code 扩展实现以下接入：

1. 为 scoop 语言注册 breakpoints 贡献。
2. 注册 Scoop debug type，提供配置片段／启动配置生成，直接启动匹配平台的官方 lldb-dap。
3. 将构建任务与调试启动串联，取得实际 executable、参数、cwd、环境及对应调试文件；调试目标是编译出的程序。
4. 为逻辑来源／预编译库配置 sourceMap，按需通过 initCommands 导入 formatter。
5. 验证启动、attach、程序退出、stdin / TTY 和测试入口。交互终端使用所选 adapter 版本实际支持的配置。

Scoop 扩展需要可定位的 lldb-dap 可执行文件，由 Scoop 发行包提供或通过扩展设置指定；其协议能力来自官方 adapter。[R4]

调试已有程序的基础 launch 配置：

```json
{
  "type": "scoop",
  "request": "launch",
  "name": "Scoop",
  "build": false,
  "program": "/absolute/path/to/program",
  "cwd": "/absolute/path/to/project",
  "args": [],
  "stopOnEntry": false
}
```

默认邻接的 .dSYM / .debug 应可自动定位；显式符号加载主要用于非默认布局和验收。sourceMap 的语法按选定版本处理；LLVM 22.1 文档使用路径对数组，不能混用其他 adapter 的配置格式。

### 10.2 值展示

首版通过 Python summary / synthetic children formatter 读取暂停进程的内存和 SBValue。它们是 LLDB 的显示扩展，不要求修改 Scoop runtime ABI。[R7]

| 对象 | 展示策略 |
| --- | --- |
| struct / tuple / class | 直接消费 DWARF 字段、基类和类型；人工对象头可折叠 |
| String | 按 byte length 读取 UTF-8，支持内嵌 U+0000；不能作为 NUL 结尾 C 字符串扫描 |
| Array / MutableArray | 使用实际元素类型、长度、stride 和布局；支持按范围展开 |
| enum / Option | 按实际 tag / niche 展示当前变体；不显示其他 payload 为有效值 |
| Any / interface | 初期显示静态视图、对象地址与已有 TD 的动态类型名称；完整动态字段重建另行实现 |
| closure / coroutine frame | 初期按生成类型展示可解释字段；源变量和逻辑栈体验按后续映射补齐 |
| Throwable | 栈诊断能力实施后可以只读展示其原有快照；变量窗口不调用 printStackTrace 来执行输出或替代格式化 |

String 当前 length 位于对象 offset 16，bytes 从 offset 24 开始，且没有隐含 NUL；interface 为 object / itab 双字值。formatter 必须遵守这些真实布局，复用已发射的字段信息及现有 runtime 合同。

不在变量窗口自动调用 `toString()`、DebugPrinter、属性 getter 或 managed 反射闭包。此类调用可能分配、GC、抛异常或需要其他线程继续执行。显示时的条目截断只是 UI 行为，不影响语言或 runtime 合法性。

### 10.3 表达式求值

必须区分三个层次：

- 读取局部变量及 debugger 支持的简单字段路径：可在基础版本使用。
- Clang 求值器接受的标量算式：属于 debugger 当前的 C 兼容能力；不能将其推广为 Scoop 的整数、Option、相等或运算符语义。
- 正式 Scoop 表达式：需支持名称／作用域绑定、泛型和重载、扩展成员、运算符、源码类型及求值结果展示；若允许调用用户代码，还需解决 Scoop ABI、GC roots、线程状态、异常和副作用。

Python formatter 不能使 Clang 自动理解 Scoop。完整求值需要复用编译器语义，但不等于必须先修改 LLDB 本体；接入方式应按需要的使用体验选择：[R5][R6]

| 接入方式 | 工作与边界 |
| --- | --- |
| Python 自定义命令调用 Scoop 求值入口 | 例如后续提供 `scoop-expr` 命令。脚本取得暂停位置、局部值和模块上下文，编译器负责解析、绑定、类型检查与求值代码生成；可复用现成 LLDB。脚本承担编排，不能替代这些语言语义工作 |
| LLDB 原生语言支持 | 实现 LanguageType、TypeSystem、DWARF 类型解析与表达式支持，必要时增加 Language / LanguageRuntime plugin；让 LLDB 的标准表达式接口理解 Scoop |
| adapter 层转接 | 将相应 DAP evaluate 等请求转给同一个 Scoop 求值入口；需要处理 frame / scope、结果变量引用、错误、条件断点与 logpoint 的实际执行路径，不能只替换一个输入框 |

编译器需要提供可复用的调试表达式入口，并能恢复当前作用域中的名称、源码类型、泛型实参、模块导入与可调用实体。普通 DWARF 的地址和字段布局不能完整代替这些语义上下文。不要在 Python 中复制一套 Scoop parser / type checker；具体选原生接入还是 adapter 转接，在完整求值阶段单独确定，不作为基础调试信息的前提。

第一版不宣称支持完整 Scoop Watch、条件断点或 logpoint 表达式。完整求值在静态观察闭环之后单独设计和验收。

修改变量同样需要独立处理。向 managed 字段写值必须遵守实际表示和 GC 写屏障；首版静态观察不能因为 adapter 暴露 setVariable，就宣称支持任意 Scoop 赋值。

### 10.4 异常、协程和数据断点

**异常过滤主要可以由脚本完成。** Scoop 通过 `scoop_rt_throw`、自有 personality 和 Itanium Level I 展开，现成 C++ throw / catch 过滤不对应它。Python 可以给实际 throw runtime 符号设置断点，读取异常对象及 TD 的类型信息，按类型或继承关系匹配；断点回调返回 false 即可继续。读取参数优先使用调试信息，必要时按已经确定的目标 ABI 读取。此路径不需要改变每个 throw 的编译规则，也不需要修改 LLDB 本体。[R11]

“仅未捕获”需要识别实际失败边界。[eh.c](../../runtime/src/eh.c) 已有 `_URC_END_OF_STACK` 分支，[startup/failure.c](../../runtime/src/startup/failure.c) 有 gateway 的失败报告入口；它们不同于普通 throw。内部函数被优化或内联后若无法稳定断住，可在 runtime 的必要边界提供小的调试 hook。协程通过 `resumeWithException` 传递失败、Deferred 保存失败再由 await 重抛，也不能一律视为程序未捕获异常。若要求在首次抛出点就按最终处理结果筛选，普通 throw 回调无法仅凭对象类型作出该判断，需要额外的展开／运行时语义配合。设置 throw 断点也不会自动为异常对象补出历史调用栈。

脚本设置断点，与 DAP 的异常筛选复选框、exceptionInfo 是不同接入点。本机 adapter 的 initialize 实测只声明 C++、Objective-C、Swift 的异常筛选项；普通 Python 断点脚本不会自动增加 Scoop 项。控制台命令／launch commands 可以先直接使用，标准 UI 集成需要 adapter 或编辑器扩展层的专门接入，不因此把自研 DAP server 作为基础版本要求。

历史抛出栈由 [Throwable 栈快照](STACK_TRACES.md) 的 runtime 能力提供；调试脚本可以在该能力完成后读取快照。设置 throw 断点、当前物理栈和异常已保存的首次抛出栈仍是不同信息，不能互相冒充。

**协程逻辑栈需要编译器输出映射，栈重建可以交给脚本。** 物理栈可以依靠现有展开信息与 DWARF。当前 [CoroutineFrame](../../compiler/mir/src/module/coroutine_frame.rs) 已有 owner、state、completion、task、saved_values，[CoroutineResumePoint](../../compiler/mir/src/module.rs) 已有关联 frame、挂起点与成功／异常恢复入口。应将这些现有信息关联到源码函数、挂起行、源变量及生成字段，并输出给调试器；现在这些 MIR 信息尚未形成可消费的完整原生调试映射。沿 completion 链还需识别生成的 continuation adapter 与终端 completion，不能把 Job 的父子树直接当成函数调用链。

LLDB 22.1.8 已有 Python `ScriptedFrameProvider`，可以补充或替换线程的栈帧；`ScriptedFrame` 提供函数名、符号上下文、变量等接口。因此应优先评估“编译器输出映射 + Python frame provider”，不预设必须修改 LLDB 本体，也不把脚本能力限制为控制台打印 `scoop-bt`。[R12][R13] lldb-dap 的 stackTrace 从 SBThread / SBFrame 取得栈帧，具备复用这些帧的入口。[R14]

本机补充实验：在原有手写 IR 产物上加载 Python frame provider，未修改 LLDB 或 lldb-dap；实际 DAP stackTrace 返回了新增的 `scoop_async_probe` 栈帧及 `probe.scoop:3` 源位置。但该版本的合成帧 Locals 请求返回空列表，不能据此声称 saved locals 已可用。需要锁定调试器版本，分别验收合成帧选择、变量读取、源码跳转和表达式上下文；这个实验也没有实现真实 Scoop 协程链。

logical step 还要处理恢复点、挂起后转移线程和目标帧的持续识别，可评估 Python ScriptedThreadPlan；显示一条合成栈不自动获得这些执行语义。仅把所有恢复 helper 标成同一源码函数不能还原协程栈。脚本也无法恢复已经被优化掉、且没有任何记录的父帧或源变量；只复原现有存活链，不为调试默认保留全部任务历史或强制所有调用分配 frame。调度器普通线程和逻辑任务也不能直接当作同一种 DAP thread。

普通硬件 watchpoint 对稳定地址可用；移动对象字段的地址可能在继续执行后变化。GC 感知的数据断点属于额外 debugger 能力，初期不对 managed heap 字段承诺持续有效的 data breakpoint。

## 11. 性能符号化与 map 消费者迁移

原有 Darwin／GNU map parser 和 `symbolize.py --link-map` 已移除，性能工具迁移遵守以下合同：

- 由 llvm-symbolizer 消费 executable 及其独立调试文件，获取文件／行、内联帧和 linkage name；使用其现有 JSON 输出等结构化接口，不新写 DWARF 地址解析器。
- 继续通过既有 symbols 观察补充 Scoop 实体角色；地址关联使用精确 linkage name，不能仅按可读函数名合并重载或泛型实例。
- 保留 Darwin ASLR slide 与 Linux PIE load bias 的现有处理。采样中的运行地址必须先按实际映射归一化。
- `PROFILING.md`、CPU-PROFILING、EXECUTABLE-SYMBOLS 及脚本测试同步更新；原始历史测量与当前行为分开记录。

llvm-symbolizer 是显式性能分析工具的依赖，不成为普通程序运行依赖。line-tables 模式也要验证离线函数归因，不能以丢失当前 map 用途换取源码调试。

## 12. 实施顺序与 spec 变更

### 12.1 分阶段交付

| 阶段 | 主要工作 | 完成标准 |
| --- | --- | --- |
| A：线级闭环 | 源位置贯通，LLVM 行表／函数信息，map 输出通路改造，Darwin / Linux 调试文件、缓存和性能脚本迁移 | 三目标真实 Scoop 程序可按 .scoop 行断点、单步与回溯；release 可离线符号化 |
| B：静态变量与类型 | 词法作用域、参数／局部／全局类型、实际存储绑定、聚合与 ZST、优化及 GC 位置维护 | 局部值正确或明确 unavailable；跨 Cone、泛型、内联及 moving GC 定向验证通过 |
| C：日常编辑器调试 | VS Code 配置和构建衔接，LLDB formatters，常用测试调试入口 | String / Array / enum 展示，launch / attach / TTY 与源码映射可用 |
| D：语言专用调试 | 正式 Scoop 求值、动态类型、异常过滤、协程逻辑栈与 managed data breakpoint | 各能力独立设计、实现及验证，不作为 A～C 的前置条件 |
| 并行：程序栈诊断 | Throwable 原栈保存／打印、未捕获异常与 trap 输出、runtime 符号化 | 原始采集可独立实施；源码行显示依赖 A 的调试产物，不依赖 D 的求值器和逻辑栈 |

调试文件输出本身的工作集中在 linker / toolchain / scoop；主要编译器工作量在完整位置与变量数据贯通，以及现有变换后的正确维护。LLDB 的完整 Scoop 求值是独立的大项，不能计作一份 launch.json 的工作。

### 12.2 先修改的规范

| 文档／章节 | 需要明确的合同 |
| --- | --- |
| SCOOP-IMPL-SPEC 2.1～2.5 | 各层调试来源／作用域／变量数据的责任；LLVM 发射与生成操作位置 |
| SCOOP-IMPL-SPEC 2.6 | 模板调试结构所需的实际 wire 变更；对象调试内容和原有 Code 更新关系 |
| SCOOP-IMPL-SPEC 2.7 | debug-info 级别、默认值、请求传递、预编译输入和缓存 |
| SCOOP-IMPL-SPEC 2.8 | 退役 map；平台调试文件、strip／签名时序、命名与发布 |
| SCOOP-IMPL-SPEC 2.19 | O0 / O2 与调试级别正交；变换必须维护位置；优化后变量的可用性 |
| SCOOP-SPEC 11.12 | 核对 currentSourceLocation 的既有独立语义；初期无需改变语言行为 |
| SCOOP-RUNTIME-SPEC 2、3、5、8 | debugger 的基础静态观察复用现有合同；程序栈诊断另按 5.6 修改 Throwable 前缀、Managed 准备与输出 |
| SCOOP-SPEC 11.7、SCOOP-RUNTIME-SPEC 5.6／7、SCOOP-IMPL-SPEC 2.10／2.21 | 首次抛出栈、printStackTrace、intrinsic Throwable、trap 与编译职责；规范、实现及实际 ABI／模板版本已配套更新 |
| ROADMAP 与性能分析文档 | 登记实际实施阶段，迁移 map 消费说明，不提前标记功能完成 |

### 12.3 主要实现落点

请求与缓存涉及 compiler/protocol、compiler/driver、compiler/scoop；前端来源涉及 compiler/hir 与 hir-lower；变换数据涉及 mir / mir-lower、lir / lir-lower；标准元数据发射集中在 compiler/codegen 的独立调试模块；平台工具配置归 compiler/toolchain，生成与发布归 compiler/linker 和现有输出通路。

编辑器代码位于 tooling/vscode-scoop，formatter 放入其明确的调试资源目录。scoop-lsp 保持语言服务职责，不成为 debugger 代理。

## 13. 验收与成本观察

调试是编译产物、外部进程和工具协议能力，适合放在 compiler-correctness 的专门 fixture 中。普通库行为仍使用原来的 Cone 测试；不将全量普通 fixture 改为 debug 或 GC stress。

必须覆盖：

1. **位置**：多文件、中文标识符、CRLF、同名局部遮蔽、分支／循环、return、异常清理及生成指令；断点命中正确的文件和行。
2. **类型与值**：参数、局部、全局、TLS、struct / tuple、class 继承、tagged enum、niche Option、ZST、泛型实例与闭包捕获。
3. **变换**：debug 的 SROA / mem2reg，release 内联和变量消除，跨 Cone 实例化／内联，正确的 inlinedAt；不可用值不能显示旧数据。
4. **GC / EH / FFI**：停在真实移动前后检查同一变量，异常路径根回写，Scoop → C → Scoop 物理回溯。GC stress 只用于此定向组。
5. **独立调试文件**：llvm-dwarfdump --verify、真实 debugger 加载、Darwin UUID 与最终签名、GNU / musl 的 debuglink、程序中仍有必需 GC / EH section。
6. **缓存与输出**：debug / release / 显式级别切换、改行号但不改机器行为、同 basename 换目录、-o 换 basename、缺失所需输出、工具生成失败不发布，以及普通并发生产。
7. **真实复用**：源码和工具一直存在，记录真实 child / linker / dsymutil / objcopy 调用；命中应无重复生成。不得通过隐藏、删除或移动原输入来要求缓存仍成功。
8. **DAP**：initialize 到 disconnect 的实际会话、verified 断点、stackTrace、scopes / variables、继续／单步／退出、launch / attach；明确不支持的 Scoop 求值和异常过滤不冒充可用。
9. **离线归因**：裁剪过普通符号的 release 程序，在外置调试文件下仍能解析地址、源码行和准确 linkage name。

记录新增编译时间、最终链接与调试文件生成时间、热缓存耗时、.slib／程序／外置调试文件大小。debug 数据本身不应增加 managed 分配或 GC roots；运行行为应与相同优化模式下的 none 一致，但不要求独立工具处理后的 executable 字节与旧产物相同。

Rust 或 Python 实施变更按 AGENTS.md 先格式化和 lint，再运行对应测试。原始日志与逐轮报告放仓库外，文档只保留范围、结论与尚未验证的边界。

## 14. 外部依据

- [R1：LLVM 22.1.8 Source Level Debugging，包括 debug records、变量 fragment 和 CodeView](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/docs/SourceLevelDebugging.rst)。
- [R2：LLVM 22.1.8 dsymutil 文档](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/docs/CommandGuide/dsymutil.rst)。
- [R3：llvm-objcopy 文档](https://llvm.org/docs/CommandGuide/llvm-objcopy.html)，相关选项另以本机 22.1.8 --help 核对。
- [R4：LLVM 22.1.8 LLDB DAP 说明与配置](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/lldb/tools/lldb-dap/README.md)。
- [R5：LLDB 架构中的 Expression 与 Data Formatters](https://lldb.llvm.org/resources/architecture.html)。
- [R6：LLDB Adding Programming Language Support](https://lldb.llvm.org/resources/addinglanguagesupport.html)。
- [R7：LLDB Variable Formatting](https://lldb.llvm.org/use/variable.html)。
- [R8：LLD Windows Support](https://lld.llvm.org/windows_support.html)。
- [R9：Debug Adapter Protocol 概览](https://microsoft.github.io/debug-adapter-protocol/overview.html)。
- [R10：VS Code debugger extension 贡献点与配置](https://code.visualstudio.com/api/extension-guides/debugger-extension)。
- [R11：LLDB Breakpoint-Triggered Scripts](https://lldb.llvm.org/use/tutorials/breakpoint-triggered-scripts.html)。
- [R12：LLVM 22.1.8 ScriptedFrameProvider 接口](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/lldb/examples/python/templates/scripted_frame_provider.py)。
- [R13：LLDB ScriptedFrame 接口](https://lldb.llvm.org/python_api/lldb.plugins.scripted_process.ScriptedFrame.html)，具体方法与执行效果需按选定 LLDB 版本验收。
- [R14：LLVM 22.1.8 lldb-dap StackTraceRequestHandler](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/lldb/tools/lldb-dap/Handler/StackTraceRequestHandler.cpp)。

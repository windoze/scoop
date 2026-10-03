# M23-9 设计：基础 artifact-only program-link

状态：已完成并验收（2026-10-02）。前置条件为已验收的 [M23-6a](../stage6a/ACCEPTANCE.md)、[M23-7](../stage7/ACCEPTANCE.md) 和 [M23-8](../stage8/ACCEPTANCE.md)。实际源码、产物、运行及完整回归见 [验收记录](ACCEPTANCE.md)。

本阶段交付：只凭 executable `.slib`、完整依赖 `.slib` 和 runtime 对象，在全新进程中产生真正可运行的多 Cone 可执行文件。链接复用既有定义、ODR、image/root 与 runtime startup；provider 源码和编译器前端状态在链接时均不需要。

权威合同为 [语言规范](../../specs/SCOOP-SPEC.md) 12.3～12.6、13.4、14.2，[运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md) 2.8、3.3、5.5、7，以及 [实现规范](../../specs/SCOOP-IMPL-SPEC.md) 2.8、2.14。本文细化 [M23 总设计](../DESIGN.md) 3.7、5.5、10；同步修订其中与实际启动、固定 runtime extern 和验证职责不符的旧要求。

## 1. 交付范围与实际基线

### 1.1 可用能力

| 能力 | 实际交付行为 |
| --- | --- |
| 独立链接 | `compiler/linker` 提供库与低层 `scoop-link` 入口；输入全部来自产物和明确 target/toolchain |
| Link 读取 | 只读取本次链接所需的身份、ABI、定义、引用和对象；不构造 HIR 模板世界或执行具体化 |
| runtime-build | `scoop` 按明确源集、头文件、toolchain 和规则构建普通对象，并可复用内容缓存 |
| 启动对象 | 一个普通 C main 引用每个实际 image 和唯一 root entry，调用已有 `scoop_rt_run_program` |
| 固定系统输入 | Darwin/AArch64 显式链接 runtime 所需的 libSystem 及既有 Level I unwind；不依赖 `cc` 隐式输入 |
| 合并与运行 | Strong 原 provider、逐 member ODR、共享 TD/storage/init、全部 stackmap、异常和 moving GC 正常工作 |
| 发布 | 完成必要的最终 Mach-O 检查后原子发布 executable；失败不破坏已有输出 |

本阶段仍只有 macOS/AArch64、单个主 Mach-O 和静态完整 Cone 图。一般用户 native library 解析、archive 成员选择与 dynamic provider 由 [M23-10](../stage10/DESIGN.md) 完成；该阶段已明确不为尚无实际调用方的 Cone 内 C/C++ producer 或 Link-required blob 增加 handler，unknown required capability 继续诊断。umbrella `build/run`、单文件 CLI 和历史 fixture 全面迁移由 M23-11 完成。final-link cache 是后续可选优化，本阶段不实现。

本阶段限制的是物理输入的供应方式。所有 `SourceExtern` 使用同一套声明合同合并与符号解析，从实际 runtime 对象和已选系统 provider 的定义／export 中查找目标；不建立 core 函数清单、普通 extern 白名单或专用解析分支。新增 native library 的定位、archive 抽取等输入供应能力留给 M23-10；当前输入中找不到的符号按普通未解析引用报错，core 也不例外。

### 1.2 实施前基线与真实缺口

| 位置 | 实施前事实 | Stage 9 的工作 |
| --- | --- | --- |
| [`layout_compile_closure/read.rs`](../../../compiler/slib/src/layout_compile_closure/read.rs) | Link decode 后进入 `into_shared_sections` 和 `replay_semantics` | 独立读取机器输入；解除完整 HIR/MIR 重放依赖 |
| [`link_decode/layout.rs`](../../../compiler/slib/src/link_decode/layout.rs) | Link 数据仍保存 HIR interface、type semantics 与 MIR bridge | 保留 Compile 通道；Link 通道仅保留实际需要的数据 |
| [`layout_link_objects.rs`](../../../compiler/slib/src/layout_link_objects.rs)、[`layout_link_symbols.rs`](../../../compiler/slib/src/layout_link_symbols.rs) | 已有对象、registration、ABI、relocation、Code 检查 | 抽取所需的完整输入，使 Compile/Link 共用同一实现 |
| [`lir_physical/odr/merge.rs`](../../../compiler/slib/src/layout_compile_closure/lir_physical/odr/merge.rs) | 已按 group/member 比较 key、ABI、definition，并保留所有候选 | 去掉对完整语义 section 的持有要求，继续用同一合并器 |
| [`toolchain/registry.rs`](../../../compiler/toolchain/src/registry.rs) | runtime 源集已列明；final-link 主要还是 `cc` 和少量 flags | 解析实际 linker、startup compiler、SDK、固定系统输入及完整选项 |
| [`scoop/schedule.rs`](../../../compiler/scoop/src/schedule.rs) | executable 构建返回 `ExecutableArtifact` | 提供 runtime-build 与产物链接编排函数，不改变 single-Cone compiler 职责 |
| [`runtime/execution.rs`](../../../compiler/driver/src/request/preflight/end_to_end_tests/imported_classes/runtime/execution.rs) | 测试生成 C harness、打包 `program.a`，从 MIR 查 String 后传 `-alias` | 正式入口直接消费对象、typed alias 和 root/image；去掉 Stage 9 用例的专用胶合 |
| [`startup.h`](../../../runtime/src/startup.h)、[`startup.c`](../../../runtime/src/startup.c) | metadata ABI 3 的生产多 image startup 已完成 | 直接调用，保持 registry、gateway、初始化和 GC 合同 |

Stage 8 的部分运行 fixture 通过额外 native trace 函数观察行为。这些外部函数不能偷偷进入 Stage 9 的固定输入集；本阶段使用普通 Scoop 状态、core 输出和异常结果改写观察点。需要 C 注入的并发／损坏边界测试继续留在现有 focused runtime 测试中。

## 2. 工具与输入边界

### 2.1 库、独立进程和编排

新增 `compiler/linker`（`scoop-linker`），提供实际 `link_program` 库入口及 `scoop-link` binary。库只依赖 identity/wire、LIR/ABI 数据、slib reader、toolchain 与现有通用对象/I/O 库；不依赖 parser、HIR/MIR lowering 或 Scoop codegen，不加载 LLVM。

低层调用形式为：

```sh
scoop-link --root-slib app.slib \
  --dep-slib core.slib --dep-slib library.slib \
  --runtime-objects runtime/index.cbor \
  --target aarch64-apple-darwin -o program
```

`--dep-slib` 可重复指定并要求完整可达闭包；不会查找 `Cone.toml`、source locator 或默认 core，也不隐式搜索缺失依赖。`--runtime-objects` 是第 4 节的普通对象索引，不接受裸 `.a`、任意额外 `.o` 或 linker 参数注入。没有 `--library-path` 分支；新增 native 输入的定位与供应由 M23-10 接入。

低层链接只解析已有产物的 LIR target 与本次完整 final-link projection；backend 合同取自产物兼容头，不探测 LLVM。源码构建的完整 `ResolvedTargetProfile` 仍由 scoop 编排持有，不能为了独立链接构造假的 backend/runtime-build 字段或要求未使用的 compiler 存在。

`compiler/scoop` 继续拥有构建 DAG、配套 `scoopc`、runtime-build 与缓存；它可以把同次已读取的 Link 结果及 runtime 对象直接交给相同库入口。独立 `scoop-link` 用于已有 artifact 的正式消费与独立进程验收，不是只为测试创建的工厂。M23-11 的公开 `scoop link` 是同一库入口的编排包装，不另写 linker。

内部边界使用完整值：`ProgramLinkClosure` 保存唯一 root、canonical Cone 顺序、每个对象的 bytes/member、完整定义／引用及 ODR 合并结果；`RuntimeObjectSet` 保存实际对象与合同；`ProgramLinkOutput` 保存输出路径、计划 fingerprint 和诊断。root、image owner、必要 ABI 和对象不能以可缺失的字段流入成功 Link 输入。library 的无 entry 状态用既有 `Library | Executable(entry)` 表示。

### 2.2 读取与归一化顺序

1. 读取每个显式 artifact 的 bytes、typed member directory、manifest 和兼容头；按实际 `ConeIdentity` 汇合。相同完整 `ArtifactFingerprint` 的重复 locator 合并；同 identity 不同内容失败。
2. 从 root artifact 自身得到 coordinate、executable kind、entry、source form 和依赖；校验完整闭包、单版本、stale semantic edges、不可达额外输入、executable dependency 和 target/runtime 兼容性。
3. 形成语言规范 12.3 的 dependency-first Kahn 顺序；每轮取当前 ready set 中 coordinate 最小者。目录与 CLI 枚举不影响顺序。
4. 按第 3 节读取机器 Link 数据并检查对象、引用、ABI 和 ODR；完成后直接保留结果，不再次按路径读取。
5. 合并所有 native 声明及实际 compiler-support 合同，统一解析当前输入中的定义；合同冲突、未解析符号和需要新增 native 输入的 requirement 在生成启动对象和调用 linker 前分别报告。

synthetic single-file `.slib` 可以作为显式唯一 root，继续遵守 `LocalExecutableRoot` 限制；它不能成为 dependency。library 中普通名为 `main` 的函数不会参与 root 选择。不存在“从符号名找到一个 main 就继续”的路径。

### 2.3 职责只检查一次

| 边界 | 本次负责检查的事实 |
| --- | --- |
| 源码编译／Compile reader | 名称、可见性、模板、类型及完整语言语义；产生实际机器表示 |
| Link reader | 外部格式、typed identity/key、机器 ABI、provider 引用、对象范围／relocation、ODR 内容与 Code 一致性 |
| runtime 对象读入 | 实际对象内容、target、符号、编译器可见 ABI 和 native 依赖 |
| 系统链接前 | Cone/runtime/program/system 定义与需求合并、固定 provider 选择和完整输入顺序 |
| 最终 Mach-O | 入口、绑定、实际地址合并、section/fixup/load command 与禁止依赖 |
| Stage 8 runtime | 加载后的地址／权限、registration、初始化初态及精确 PC/root payload |

同次编译和发布复用已有 IR/对象结果；外部 bytes 新读入时检查其负责边界。不能为获得 Link 类型而重新跑整个 HIR/MIR，也不把整套 runtime registry 移植一遍到 final verifier。

## 3. 独立 Link 数据与共有 reader

### 3.1 复用已有机器数据，补齐真实缺项

manifest 的输出分支、image/root owner、native contracts、registration 和 ODR 目录，三层已有 identity foundation，LIR `cone-production/4`、`link-identity-closure/8`、callable/layout 的既有 Link import section，以及全部 typed `LinkObject` 继续使用。新 reader 不根据 symbol 文本反推 provider，也不要求源码或前端 session。

HIR/MIR/LIR foundation 已保存 canonical identity keys、MIR callable signature、LIR native contract 和 C ABI signature/layout。独立 Link 直接读取这些数据表，复用各 IR crate 的 wire/key 解析；不读取 HIR interface/template/type-semantics 或 MIR type bridge 来重放语言语义，也不把原有身份和合同复制到新 section。拆开 foundation 的数据／引用检查与 Compile 语义检查，保持原编码和 typed ID。

现有 `CanonicalScoopAbiFunctionSignature` 已携带 exact type、receiver、参数顺序、size/alignment、ZST elision、direct/indirect passing、结果与 GC effect；layout ABI 原表保存完整 value/instance 表示、字段／variant、scan 及 provider。Link 原位读取并检查这些记录的机器一致性，generated-C 继续使用 foundation 中完整的 C signature/storage/layout。constructor 的 initializer ABI 返回 Unit，root/eager gateway 使用已有 C `uint32_t(void)` 合同。无需从源码形状重新求布局，也不做 subtype、override、默认值或 visibility 检查。

实现核对没有发现需要新增 ABI 补表的实际缺项，因此删除原设计的预留 `abi_support` 字段。新增必需 LIR section `org.scoop-lang.lir/link-support/1` 只补实际 runtime 数据 alias：固定单字段 map `{1=runtime_data_aliases}`，无 alias 的 Cone 显式编码空数组。不得为了保留一个预留字段而复制原 ABI／布局表；以后出现实际缺项时再以具体输入修订对应格式。

producer 从同次已完成的 LIR descriptor 投影 alias。身份查询使用实际 definition、import、native contract、registration、ODR directory 沿 canonical key 闭合；模板声明可作为 application key 的身份组成，不展开模板正文。ABI 和机器 definition 必须 fully concrete，body 与 specialization 保持不同 typed ID。已有 production 足以描述的 TD、registration、image、entry 原位引用，不再列同内容的 Link 证明表。

`runtime_data_aliases` 的 record 为 `{1=RuntimeSymbolContractId, 2=StrongDefinitionOwnerV1}`，按 contract ID 排序，target 必须由当前 Cone 实际定义。当前唯一用途是现有 runtime String TD 数据符号；定义方从 LIR 的真实 String descriptor 引用投影此条目，其他 Cone 集合为空。reader 通过该 owner 取得已有 TD、layout 和 symbol，检查原 String ABI 表示；全闭包中该符号必须有唯一 target。条目不提供类型来源资格、不追加 String layout/scan 副本，不向语言开放任意 alias 功能。

三层 identity foundation、LIR ordinary bridge 与 layout ABI section 的 `required_for` 同步为 Compile|Link，既有 wire payload 与 semantic sink 不变。其余语言 section 保持 Compile purpose。

### 3.2 Link 读取过程

- 通过目录和 section purpose 验证 actual required inventory；读取已有三层 foundation 和机器 Link sections，其余 HIR/MIR payload 仍受容器长度／摘要约束，Link 不解码模板、声明正文或语言语义表。
- 从原 foundation 和 Link sections 取得 identity/ABI，用现有 typed key 检查建立本次引用查询；跨 Cone key 通过实际依赖闭包取得。重复 ID 的完整 key 不一致、kind 错误、缺少引用或非法依赖环均拒绝；完整 foundation 的正常数据读取不等于导入前端语义世界。
- 字段和 callable ABI 复用 nominal layout 与结构化 exact key 的物理存储计算。内嵌 tuple、函数值和指针存储不要求独立导出的 layout identity；字段记录中的 layout ID 仍须等于由完整 exact key、target 和 representation role 得到的 ID，大小、对齐与 scan 继续接受原布局检查。
- 将完整 production、ABI 与对象输入交给已有 Mach-O、generated-C、registration、stackmap、symbol 和 relocation 检查函数。调整函数参数以接受所需数据，不能复制一套 verifier 或把 Link 数据重新包成 HIR/MIR。
- native contract 从实际 manifest/LIR 数据取得；不因 Link 没有 HIR 就跳过 extern 完整签名比较。ordinary 与 layout Link import 保留原 provider、typed target、required definition 和真实 use；只有真实 relocation 才要求物理覆盖。
- 复用原 canonical digest/ODR 实现核对对象与记录，计算 Code 所需的实际贡献，核对当前 artifact 的 Code/RuntimeImage 内容；保留完整结果供后续链接使用。

原 Code 读取中仅为 source count／分发规则再次访问 HIR foundation 的分支移回 Compile 边界；Link 从 manifest 的实际 source form、输出 kind 和 entry 验证其链接职责，不能伪造 source count 调用旧接口。独立 Link 输入错误和 Compile 语言错误使用不同诊断位置。

### 3.3 Compile/Link 迁移与删除

Compile 继续检查完整共同 HIR/MIR；在得到同一机器输入后与独立 Link 路径汇合。同次 producer 不把刚写出的 `.slib` 回读一遍来获得发布资格。对本次未变的已检查记录直接共享，新增 support 投影由普通 fixture/golden 锁定。

删除正式 Link 路径上的 `into_shared_sections → replay_semantics`、对 `SemanticIdentitySession` 的需求、只为复用旧接口而保留的整份 HIR/MIR section。旧 Compile 功能不因此删除；其实际语言检查保留原入口。`merge_cross_cone_odr_definitions` 只接收已解析 keys、目录和对象结果，不再要求持有全部语义 section。

## 4. Runtime 构建、对象与缓存

### 4.1 构建职责

`compiler/scoop` 的 runtime 构建模块使用 `ResolvedTargetProfile` 中的 `lir_target + c_bridge_toolchain + runtime_build`，按现有 `runtime_sources` 列表构建 C 和 AArch64 assembly。不扫描用户 Cone 下的 C/C++ 文件。runtime root 是构建输入的 locator，不进入实体 identity，也不凭路径授予额外资格。

沿用现有 runtime flags，并明确 target、SDK/deployment、优化配置、无 LTO、frame pointer 与禁 sibling-call 优化；所有实际 options 进入 build key。runtime 内部 C 调用继续通过普通 C 声明／共享头文件检查，不给每个内部 helper 发明 Scoop 类型或通用 ABI 证明。编译器产生的 runtime/target 调用使用既有 ABI 合同；普通 `SourceExtern` 使用产物中的实际声明，不另登记一套 core API 名单。

每次 cache miss 编译列明源集，保存普通 compiler dependency files。构建 key 覆盖源文件、runtime 头文件、实际选中 SDK/compiler resource 头文件、compiler identity/content、target、flags、runtime ABI 与规则版本。runtime 头文件目录的内容快照同时覆盖新增／删除的 include 候选；SDK/resource 使用同一明确 toolchain 输入及依赖内容。不能只用 `.c` digest、SDK 路径或 mtime 决定命中。若无法取得当前工具链的完整 include 输入，禁用该 runtime cache 条目的复用，照常构建。

源文件数、object 数和映射不是 ABI 基数。默认实现可以逐 source 编译；接口接收任意非空对象集合，按实际 typed `RuntimeObjectId` 规范化。一个对象可定义多个入口，跨 runtime 对象引用正常解析。

### 4.2 普通对象索引

复用总设计 3.7 的 `RuntimeObjectRecord`、`RuntimeObjectId` 和 `RuntimeArtifactFingerprint` 内容模型：length/digest、definitions、requirements、target、runtime ABI 和实际构建配置。build key 记录“哪些输入需要重编译”，artifact fingerprint 记录“本次实际提供哪些对象及合同”，两者不混用。

`index.cbor` 是 schema 1 的本地缓存／进程交接索引，保存 target/runtime ABI、C toolchain 和 build 配置、已解析定义／需求、按 object ID 排序的 records，以及每项对应的相对文件 locator。canonical 内容摘要排除 locator 和 cache 根路径；文件 locator 只负责找到该项 bytes。它不是 `.slib`，不声称具有 Cone identity，不是新的可分发 runtime 格式。

读入索引时验证 schema、各 ID/key、对象长度和 digest，并用同一普通对象 reader 检查 Mach-O target、符号 kind、定义／undefined 集合与既有 ABI 合同。重复对象、同符号冲突、缺入口、额外隐式 native 输入、与 runtime ABI 不一致的记录均失败。对象全部读为同一份 bytes 后交给 linker；不能在记录检查后再让系统 linker 打开未经对应的旧 locator。

编译器产生的调用和 native 声明各自保留实际 canonical ABI；runtime 对象中的定义以 symbol、kind、对象和 relocation 关联，不伪造从机器码恢复出的 C 函数类型，也不要求普通 extern 先出现在 runtime 白名单中。`scoop_rt_run_program` 是固定 program→runtime C 入口，`scoop_td_String` 是反向绑定到 Cone TD 的数据需求，均有明确角色。runtime 不定义 C main、root gateway、Cone image 或 String TD。

cache 按 key 使用现有 build lock 与原子写入机制；同次新构建结果直接传递，持久 cache 命中作为外部对象读入一次。损坏 cache 按 miss 重建；显式低层对象索引损坏则报错，不搜索源码修补。源码或工具链变化使 runtime 重建并重新链接，不因该变化本身重编译 ABI 未变的 Cone。

## 5. 统一 native 解析与 Strong/ODR 合并

### 5.1 声明合并与定义解析共用一条路径

先按 target/symbol 合并所有真实 native 合同，再解析其 provider。完整比较 library、C/Scoop ABI、calling convention、GC effect、function/data/TLS/mutability 及参数／结果；没有调用的声明仍参与冲突检查。不同合同报告首个差异和全部相关声明。core、普通库和再次发布的泛型实例不影响该过程；`SourceExtern` 始终保留源码 extern 身份，不改标为 `RuntimeAbi` 来绕过普通规则。

当前可解析的物理输入是本次提供的 runtime 对象及明确 SDK 系统 provider。对象的定义来自实际 symbol table，dynamic export 来自实际 SDK stub 及其 re-export；不按 core 当前用到了哪些函数裁剪候选。`DefaultNativeNamespace` 在这一输入集合中按普通符号规则解析：存在唯一兼容定义则绑定，多定义／合同冲突或找不到目标则报错。一个系统 export 即使未被 core 或 runtime 使用，也可以被普通合法 extern 引用，无需登记新白名单。

| 实际需求／解析结果 | Stage 9 处理 |
| --- | --- |
| 普通 `SourceExtern` | 合并已有完整声明合同，按其 library binding 查找实际定义，保留原声明位置 |
| 同一 symbol 也被 `RuntimeAbi`、`TargetEhSupport` 或 `CBridgeTargetSupport` 引用 | 合并这些实际存在的合同，核对一致性；这些记录描述编译器产生的调用，不授予源码 extern 资格 |
| 当前输入没有目标定义 | 普通 unresolved-symbol 错误，列出 symbol、library 与全部引用来源 |
| 需要定位新的逻辑 library 或加入 archive／dynamic input | 报输入供应能力未实现，由 M23-10 增加 provider 后继续使用同一解析器 |
| 与 Scoop-owned、generated bridge 或既有 compiler-private 保留定义冲突 | 按已有符号归属和保留符号规则拒绝，不给任何 Cone 豁免 |
| unknown Link-required capability | 在对应 artifact/member/section 报不可处理输入，不按扩展名试用 |

普通 Mach-O/C 对象不提供可恢复的完整函数类型。Link 检查已有声明之间的完整 ABI、实际可见的 symbol kind/storage 与对象引用；同一符号已有 runtime/target ABI 合同时也必须比较。对只有普通 FFI 声明的函数，不伪造一份“从 export 读出的完整 ABI”，也不为证明它正确而新建 API 准入表。外部实现符合声明仍遵循现有 FFI 作者责任；core 中的 extern 同样承担这项责任。C ABI 调用继续使用 artifact 已携带的 generated-C bridge，链接时不补生成。

源码 extern 的跨 Cone 调用使用定义方产物中的普通 Scoop ABI 入口。MIR 为每个参数自由的 source extern 生成薄函数体，参数／结果保持原声明；入口包含 native transition 与 caller-root publication，Scoop callable effect 为 Managed，原 native callee 的 GC effect 保留在 SourceExtern 合同中。函数体沿既有 extern lowering 调用 native 目标（C ABI 继续经过原 generated-C bridge）。入口使用原 source function 的 Strong callable body 身份，HIR implementation 与 native requirement 仍为 SourceExtern；不会把声明改成 runtime intrinsic 或在 consumer 复制实现。普通导入、泛型正文和默认参数均选择同一入口，因此 core 的 `println -> write` 和普通库中的等价调用都可消费。此前统一拒绝 dependency native call 的阶段占位限制退役；额外物理 provider 是否存在由正式 Link 输入决定。

### 5.2 定义和 ODR

所有实际对象都进入链接，不能先合成 archive 再依赖抽取保留 metadata。每个 Cone 的对象合计恰好有一个实际 image；root descriptor、registration、body 和 dependency reference 均使用已解析 typed owner。

Strong definition 只由其原 provider 贡献；consumer 不补同名实现。逐 `(group, member)` 比较 Stage 7 的 canonical key、ABI 与 definition，合法相同候选继续让系统 linker coalesce；不同 helper member 的并集保留。逐 member 必需引用闭合，不因为相邻 member 存在就忽略缺项，不把整个 group 的候选集合强制改为相同。

同一个 non-local symbol 对应不同 owner、Strong 重复、ODR key/ABI/body/scan/EH/stackmap/relocation 漂移均在系统 linker 前失败。合并结果保存所有兼容候选及稳定诊断 origin；最终地址由 linker 决定，不能在链接前虚构 winner。

### 5.3 String 的普通符号 alias

从第 3 节的 typed alias 得到实际 descriptor primary symbol，形成 Darwin 的等地址 `-alias` 操作。alias 是目标 TD 的另一个符号拼写，不另分配 TD、复制 constant 或建立指向 TD 的 pointer variable。最终检查 `_scoop_td_String` 与该 TD 地址完全相同，Stage 8 继续检查它是已登记的实际 InlineBytes 类型。

alias target 使用定义方的现有 TD symbol/owner；provider 的版本、声明位置或重新构建造成 symbol 变化时自然跟随新产物。不得按固定 CORE digest、`String` 文本或“找到唯一 InlineBytes”反推目标。改建 core 和包含同名普通类型的 fixture 必须经过该路径。

参数自由 source extern 的 provider generated-C canonical flags 包含 `-fno-builtin`（flag tag 13），保留普通 extern 的真实符号调用及其 relocation，旧 flag fingerprint 的产物与缓存需重建。入口包含 native transition 与 caller-root publication，其 Scoop callable effect 为 Managed；原 native callee 的 NoGc/Managed 合同保留在 source extern record 中，不能用于取消入口的边界切换。

## 6. 启动对象与固定 target 输入

### 6.1 启动对象只连接已有入口

program-link 生成的 C 代码形状如下；symbol label 由已经核对的 typed Link 引用产生，不能把任意 artifact 文本直接插入 C 字符串：

```c
#include <stdint.h>

typedef struct ScoopImageDescriptorV1 ScoopImageDescriptorV1;
typedef struct ScoopRootEntryDescriptorV1 ScoopRootEntryDescriptorV1;

extern const ScoopImageDescriptorV1 image_0 __asm__("<object symbol>");
extern const ScoopRootEntryDescriptorV1 root_entry __asm__("<object symbol>");
extern int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *,
                                uint64_t,
                                const ScoopRootEntryDescriptorV1 *);

__attribute__((section("__DATA_CONST,__const")))
static const ScoopImageDescriptorV1 *const images[] = { &image_0 };

int main(void) {
    return scoop_rt_run_program(images, sizeof(images) / sizeof(images[0]),
                                &root_entry);
}
```

实际声明和 array 恰好覆盖 canonical Cone 顺序中的每个 image，一份 root 引用。header 类型仅 forward declaration；不包含 runtime 源树路径，不生成字段布局或重建 descriptor。标准头文件来自明确 SDK。main 不直接调用 managed main/initializer、不建立第二条 GC boundary；整个生命周期由 Stage 8 入口负责。

final-link profile 携带实际 startup C compiler invocation；复用 toolchain 的 C 编译命令构造能力，使用明确 target、SDK/deployment、无 LTO 和固定 C options，只编译这份生成代码。它与 runtime-build 是不同职责，但可以使用同一个已解析 compiler。program-link 不能为了生成 main 调用 LLVM Scoop codegen，也不重新编译 artifact 中已有 generated-C bridge。

生成对象必须为当前 target 的普通 Mach-O relocatable：恰有 C main 及实际必要 local data，undefined 只有输入 image/root、`scoop_rt_run_program` 和 profile 明确允许的 C compiler support。没有 managed safepoint、user constructor/destructor、TLS initializer、异常 personality 或动态注册。producer 直接保存本次 source、options、对象和引用记录；对象检查按实际符号与 relocation 关联，不重编译第二次作字节证明。

### 6.2 直接调用系统 linker

现有测试通过 `cc` 间接链接；本次核对的 driver 会隐式加入 `-L` 搜索路径、libSystem、compiler-rt archive 等输入。正式路径改为执行 registry 明确解析的系统 `ld`。解析发生在 toolchain 边界，保存实际路径用于执行、版本／内容 identity 用于普通输入 fingerprint；不要求该 linker 或 Apple Clang 的 LLVM 版本是 Scoop codegen 的 22.1。

固定参数包含：arm64、macOS platform/deployment/SDK、普通 executable/PIE、`_main`、必要 symbol table/map、显式 libSystem SDK 输入、`-no_deduplicate`、stackmap `-rename_section` 和 target 必要的 ad-hoc signing。选择该 profile 时一次验证其实际输出格式可由 final verifier 读取。不能从别的 projection 的名称猜上述参数；`CC`、`CFLAGS`、`LDFLAGS`、`LIBRARY_PATH` 等不会额外注入 input 或 options。

固定对象顺序为 startup object、canonical Cone/member 对象、按 RuntimeObjectId 排序的 runtime 对象、实际存在的 target support objects，最后是固定 dynamic providers；alias 等有位置语义的操作按 profile 确定的位置保留。所有直接对象各一次，不把它们打包为 archive，不引入 group/whole-archive/force-load 的本阶段空壳分支。

libSystem 使用已解析 SDK stub、install name、版本/target 和实际 export 目录；其实际 re-export 输入同样列入工具链解析结果。profile 选择物理 provider，export 目录由输入读取，不维护供 core 使用的函数子集。只读入／物化实际选中的必要 stub 文件，不复制整个 SDK 或系统 dylib。SDK stub 的内容摘要不声称是运行时 dyld shared cache 的字节摘要；目标平台提供系统 ABI 的正常承诺，本阶段不建立 OS 来源认证或 platform pinning 协议。

当前固定 runtime 使用的 Level I unwind、C memory/stdio、pthread、Darwin VM/image 和 CommonCrypto 等入口均由该系统输入闭合。所需符号不存在、SDK 不兼容或出现需额外物理输入的 compiler helper 时，报告明确 toolchain/input 错误；不能静默加入 compiler-rt、另一个库或任意搜索目录。若实际支持的 toolchain 确实需要额外 helper，先把其对象及真实调用合同补入同一 profile，并完成对应 fixture；普通 SourceExtern 的解析规则不变。

当前 Darwin profile 明确传入 `-no_fixup_chains`，采用 `LC_DYLD_INFO_ONLY` 的 rebase/bind 操作流；final verifier 读取该实际格式。没有使用 chained fixup 的输入或调用方，因此本阶段不增加其备用解析器。SDK v4 stub 的目标选择遵循 Darwin 的 arm64／arm64e stub 兼容规则，并以实际 `ld` 探针验证；读取完整 re-export 闭包，不按使用到的函数过滤 exports。

此格式的 lazy symbol stub 由 `ld` 引用系统 `dyld_stub_binder`。profile 将该真实 linker-generated 需求及所选 SDK export 纳入解析和 fingerprint；它不要求额外对象，也不放宽普通未解析符号检查。

### 6.3 链接后生成的结构

Mach-O header、load commands、dyld stub/GOT、rebase/bind 或 chained fixups、compact unwind 等由 linker 形成，使用实际 final-link profile 的正常格式规则解释。需要被外部对象引用的 linker-generated symbol 具有明确 target role，例如实际出现的 executable header；其引用不能落入“未分类 undefined 都接受”的分支。

没有独立 startup/support `.o` 时相应输入集合为空；header/stub 等不是伪造的 `RuntimeObjectId` 或 `SlibMemberId`。system dynamic provider 不成为受控定义的 owner，不能替代原 Scoop/runtime/program 定义。禁止输入 bitcode/LTO、未声明的 autolink、C/C++ 全局 constructor/destructor、额外 dynamic provider 和未经处理的 native TLS 初始化。

## 7. 链接计划、最终检查与原子发布

### 7.1 普通链接计划

`ResolvedLinkPlan` 只保存本次实际使用的数据：root、canonical Cone 列表、各 Cone Code 和 member 内容、runtime artifact/object records、startup source/options/object、固定系统输入、typed String alias、有序操作、target/deployment 和实际 linker/toolchain。canonical 编码沿用 WireCborV1 和 domain `scoop-resolved-link-plan-v1`，不把 host/cache/output 路径或进程号放入摘要。

对象、合同或 aliases 改变必须改变 plan；不能只比较 `.slib` 文件名或只记录 object digest 而丢失 ABI/owner。source locator 的变化不改变已有产物的 Link 意义。有顺序语义的操作保留顺序，不先去重为集合；只在请求归一化时合并重复的同一 artifact。

本阶段不要求通用 native input/action/evidence 枚举的未来分支已存在。第 6 节的真实固定输入用完整记录表达即可，M23-10 再增加实际 native variant。plan 与 link map/trace 是诊断和正确性检查数据，不产生发布令牌、收据或完整程序来源证明。

读取的对象 bytes 写入本次私有临时目录，文件名用 actual Cone/member 或 runtime object ID 隔离，create-new 写入后供 command 使用。系统 stub 也使用本次解析的明确输入。被验证的 bytes 和链接消费保持对应；不在同一次链接中重新按原始路径查找对象。只保留必要的 I/O 范围、溢出和原子写入检查。

### 7.2 最终 Mach-O 检查

| 链接新产生的事实 | 必须检查的结果 |
| --- | --- |
| 文件与 entry | little-endian arm64 `MH_EXECUTE`，正确 platform/deployment；唯一 C main 和正确 entry |
| image/root 引用 | startup array 恰好指向输入闭包每个 image 一次；root 指向输入 executable 的原 descriptor |
| 符号解析 | 受控 definition/use 解析到原 typed owner；没有残余必需 undefined 或意外 dynamic import |
| String alias | 固定 runtime 数据符号与所选真实 TD 同址，没有第二个 TD 或中间 pointer slot |
| ODR 地址 | 相同 member 的实际引用指向唯一 winner；各 sibling 的 registration、body、TD、storage/unit 引用一致 |
| 不同实体 | 不同 exact type/body/storage 没有因 ICF 或不合法合并而共址；ZST storage 的 addressable token 保持 |
| GC/EH sections | 全部 stackmap blob 被保留到 `__DATA_CONST`，必要 EH/unwind、metadata 与可写 storage 段仍存在 |
| 系统输入 | load command 与绑定只来自固定 profile；无 libc++abi、C++ personality/terminate 或额外库 |

map/trace 只引用本次实际输入；无法对应的对象、load 或 non-local definition 报错。最终 symbol table、exports、binding 和输入 relocation 共同提供地址关联；不能仅凭 `ld` 退出 0、`nm` 没有某个字符串或程序偶尔能运行就通过。

文件里的 pointer 必须通过 Mach-O VM/file mapping 和实际 fixup 格式解释：rebase 得到同一 preferred-address 空间，bind 根据 ordinal/symbol 关联明确 provider，chained fixup 按其真实 pointer format 解码。不能把 raw pointer bits 当运行地址、用 nearest symbol 替代实体，或要求 ASLR 后地址与磁盘数值相同。使用已有 `object` crate 的 Mach-O 读取能力；只补它未覆盖而当前 profile 确实输出的 fixup 读取，不另造通用 executable reader。

stackmap reader 在对象边界已经验证 canonical v3 payload。最终检查逐 blob 确认完整保留、范围和相关 callable 的链接位置；Stage 8 startup 在实际加载地址核对 site/body/原始 PC、locations/live-outs 与规范化 fingerprint，建立唯一 GC 索引。final verifier 不重新计算全部语言布局、ODR 摘要或每张 runtime 表，不要求重新执行 startup 才能发布。显式深度核对及损坏矩阵放在测试中。

### 7.3 失败与发布

系统 linker 在输出目录所在文件系统的私有临时位置写 binary，固定临时 basename 和签名标识，避免用户 `-o` 拼写进入内容。成功后检查目标平台要求的最终文件／签名结构，完成临时文件写入、必要同步和验证，再以一次原子 rename 替换输出。替换前的错误或 rename 失败保持旧输出；成功替换是发布完成点，不承诺文件系统故障后的跨崩溃事务回滚。

目标需要的 ad-hoc code signature 由现有 Darwin toolchain 产生，不验证发布者身份，也不形成 Scoop 信任链。保存必要符号直到 final verifier 完成；本阶段不增加 strip/优化产物的第二条通路。

`link_program` 不自动运行用户程序；运行是 `scoop run` 或 fixture 的下一步。错误输出包含失败阶段、canonical Cone/member、typed owner 或 runtime object、symbol/首个 ABI 差异与必要的本地 locator；host 绝对路径只作 presentation，不写入 canonical snapshot 或 fingerprint。

## 8. 诊断与不支持的输入

| 输入或错误 | 诊断位置／行为 |
| --- | --- |
| root 为 library、缺 root entry、多个 executable | root manifest／实际冲突 Cone，在调用 linker 前失败 |
| 缺依赖、stale edge、版本冲突、不可达额外 artifact | manifest dependency edge，列出期望和实际 identity/fingerprint |
| Link section 缺失、旧格式、wrong purpose 或未知 required | artifact、member、capability；提示重建不兼容产物 |
| ABI、Strong、ODR 或真实 relocation 冲突 | 两端 provider/member/typed target，以及首个不同合同或内容叶子 |
| 普通 extern 合同冲突或目标未解析 | 保留全部 source contract origin，报告首个差异或缺失定义；core 与其他 Cone 使用同一错误 |
| 需要额外物理输入的 native requirement | symbol/library/origins 和 M23-10 输入供应边界；不提示添加 raw `.o` 绕过 |
| runtime index/cache 损坏 | 显式 index 报错；构建缓存按 miss 重建，均不把错对象传给 linker |
| toolchain、SDK、隐式 input 或禁止 linker option | 实际 profile/input；失败前无输出发布 |
| ld 或 final verifier 失败 | 区分工具启动、链接错误、最终格式／绑定错误与输出 I/O |

所有错误均有确定出口，不使用 `TODO`、panic、空分支或“暂时发一个占位 body”。未知 native capability 的显式诊断是本阶段输入范围，不允许成功产物缺少必需定义。

## 9. 格式、fingerprint 与兼容性

| 项目 | Stage 9 决定 |
| --- | --- |
| 新 LIR section | `org.scoop-lang.lir/link-support/1`，固定第 3.1 节单字段 map，Compile/Link 必需；原 foundation 不复制 |
| 新 section 的 sinks | `Code` 与 `LinkValidationOnly`；通过正常 section/成员目录进入 Artifact fingerprint |
| generic profile | 保持 `cross-cone-generic/2` 的 ID，required inventory 增加新 section，descriptor fingerprint 同步变化 |
| 既有机器 payload | `cone-production/4`、`link-identity-closure/8`、layout link closure `/4`、object verifier `/4` 保持，复用现有对象格式 |
| HIR/MIR/LIR 语言与 ABI | 现有 HIR `/43`、MIR type bridge `/6`、LIR layout ABI `/5`、foundation 与三层 outer schema 保持 |
| runtime | metadata ABI 3、144-byte TD、初始化 unit、startup C 函数、String 表示保持 |
| native/toolchain 记录 | runtime build 与 final-link profile 的实际 descriptor 使用各自 `/1`，内容 fingerprint 包含本阶段实际规则／工具 |
| runtime 对象索引 | 本地 schema 1，仅用于缓存／进程交接；不成为新的 Cone artifact 类型 |

新 section 的普通 Code contribution 使用已有 known-extension 编码，payload 为完整 canonical alias 数据；不更改 Code hash framing 或新造 proof digest。已有 production 和对象提供的事实不复制进入 payload；语义未变时 HIR/MIR/LIR fingerprint 不因物理链接支持数据而改变。alias、完整合同、对象／定义变化仍会改变 Code、Artifact 与 Link plan。

没有新 section 或 descriptor fingerprint 不匹配的旧 `.slib` 及 cache 必须重建；不能在旧 profile 下把缺项默认为空，不能借 HIR 重放兼容旧 Link 输入。保留 profile ID 不表示允许旧 required inventory。container、persistent identity、mangler、ODR key/hash、RuntimeImage 和 runtime C ABI 不因本阶段改版。

若实施中发现必须改变既有 payload，而非补充独立 Link support，则在修改实现前明确修订该 section major、required inventory 和对应文档；不得仅修改 verifier 行为接受两种同名 wire，也不能通过同名不同类型字段规避版本。单纯读取函数拆分和移除重复重放不升级无变化格式。

## 10. 真实 fixture 与验收

### 10.1 源码、产物和运行闭环

建议目录 `tests/fixtures/m23-program-link/`。每组先用配套 `scoopc` 构建完整 provider/consumer/root 产物，再移走所有 Scoop 源目录；runtime 由正式 builder 产生对象索引。用全新 `scoop-link` 进程读取磁盘输入，链接时不提供 `scoopc`、LLVM、runtime 源树或 runtime headers。可访问的 SDK/C compiler 只用于普通 startup C 对象。

| 独立／组合 fixture | 核对结果 |
| --- | --- |
| core + library + executable 的普通／NoGC／空 main | 唯一真实 root gateway、正常返回，library 普通 main 不参与竞争 |
| 4～6 Cone 的链与 diamond | 任意输入枚举得到相同 canonical plan；eager 动态 ready-set 顺序符合语言规范 |
| 每 Cone 多个 Scoop/generated-C `LinkObject` 与同名物理成员 | 全部对象恰一次；跨 object 引用成功；opaque/diagnostic/optional blob 不传给 ld |
| core `println`、普通库中相同 extern、core/runtime 未使用的系统 export | 共用声明合并与实际定义解析，不登记 core API 或函数白名单 |
| generic sibling 与独立 helper member 并集 | 真正唯一 body/TD/storage/registration 地址；不要求整组成员集合相等 |
| generic delegate + eager/lazy + failed/cycle | 全程序 storage/cell/failure 共享、exactly-once、不重试；未访问 lazy 没有副作用 |
| ZST、大值、含引用 struct/enum/tuple、Option/Array | ABI、扫描和跨 Cone 值流正确，ZST token 不丢失 |
| callable reference、closure、协程及 try/finally | 所需机器 helper 已在 artifact；链接不补生成，异常和恢复正常 |
| root/eager 未捕获异常 | 原 failure root、真实类型报告、阻止后续 eager/main，异常不穿过 C startup |
| 修改并重建 core、同名普通 String | alias 指向实际协议选中的 TD，未使用 FQN 或固定 identity 回退 |
| 独立 runtime cache 与二次链接 | 仅 runtime/header/toolchain 改动导致正确重建／重链；相同产物换路径不改变 plan |

普通与 `SCOOP_GC_STRESS_MOVE=1` 两种模式运行适用矩阵；Stage 7 ODR 和 Stage 8 多 image/gateway 回归继续保留。观察运行结果使用正式语言能力和既有 runtime 输出；不额外注入 fixture-native object。

### 10.2 Negative、golden 与损坏边界

- 每类新错误有独立负例，断言 canonical Cone/member、位置及错误内容；组合负例覆盖 native ABI 冲突、stale dependency、Strong/ODR 和 root/image 错误同时出现时的稳定诊断顺序。
- 将相同 extern 声明分别放入修改后的 core、普通库和 root，解析结果一致；错误 library、声明间 ABI/effect 冲突、缺失符号和保留定义冲突同样失败。普通 Cone 引用未被 core/runtime 使用的实际系统 export 必须成功，不能靠扩充允许函数清单修复该 fixture。
- 新 Link support 的单字段与 empty/nonempty alias 集合、既有 foundation 引用、完整 ABI、alias、required purpose/version、Code contribution 有固定向量和 corruption tests。既有 HIR/MIR/LIR golden 继续验证语义未变；新增 Link dump、startup C/object 引用和 canonical plan golden。
- 无模板／HIR session 的低层进程可以读取 Link 数据；Compile-only 可选内容变化不会触发模板 decode。截断／digest 不一致仍在 envelope 边界拒绝，不能把“无需解码 HIR”解释为跳过容器完整性。
- object/member 变化、extra definition/undefined、缺失 image/entry、错误 ABI、错误 String target、未知 capability、物理成员重排和同名成员覆盖各有负例；opaque 中放置合法 Mach-O bytes 也不得被提取。
- runtime 源文件和头文件修改、新增 include 候选、compiler/SDK/flags/ABI 变化、截断缓存、错 object digest、同 symbol 多定义与缺固定 export 覆盖缓存和普通输入边界。
- final verifier 以真实链接产物为基础修改 entry、load/binding、alias、ODR 地址、stackmap section 和 fixup，断言拒绝；不构造新的来源证明测试工厂。
- 调换相同 artifact 的 locator、source/cache/output 目录和重复输入，canonical plan 不变；改变有效对象、合同、alias、工具或有序操作则变化。最终 binary 不要求跨不同 linker/SDK 版本逐 byte 相同。
- linker/verifier/I/O 失败与并发同输出发布验证旧 binary 保留；read-only 操作和构建缓存不重复执行整套语言语义。

### 10.3 验证方法

每批实现完成后先 `cargo fmt --all` 和 `cargo clippy --workspace --all-targets`，再执行相关 crate/fixture。最终用真实配套 compiler 运行全 workspace，显式清除全部 `SCOOP_UPDATE_*`、名称含 `SNAPSHOT` 的 `SCOOP_*`、`INSTA_UPDATE` 与其他快照更新覆盖。先审阅语义／对象差异，再受控更新必要快照；开启更新的运行不计最终验收。

不为文档变更重跑编译器全套测试。设计核对不能替代实现／运行验收；关闭更新的完整回归与各功能提交见 [验收记录](ACCEPTANCE.md)。

## 11. 实现顺序与完成门

1. **独立 Link 数据。** 复用原 foundation 实现 identity/ABI 数据读取，补齐实际 runtime alias 的 support producer/wire；复用原对象／ODR 代码，删除 Link 对完整 HIR/MIR 重放的依赖。完成真实产物和格式向量。
2. **runtime 对象与统一 extern 解析。** 接通现有源集与 C toolchain、头文件依赖、普通对象索引、既有 runtime ABI 及缓存；从实际对象／SDK export 建立符号输入，统一合并声明合同并解析定义。
3. **正式启动与固定 linker。** 生成引用原 image/root 的 C main；接通 typed String alias、明确系统 ld/libSystem、固定 flags 和多个直接对象，完成三 Cone 首个可运行闭环。
4. **最终文件与发布。** 关联 map/symbol/bind/fixup，验证最终入口、ODR 地址、section 和 dynamic imports，完成原子输出与失败保留。
5. **独立进程和组合验收。** 提供生产 `scoop-link`，让 `scoop` 的编排函数调用同一库；完成无源树的磁盘读取、runtime cache、ODR/初始化/异常/moving GC 矩阵及完整回归。

每步按实际功能提交并同步 spec、golden、进度与必要格式，不建立额外发布门禁。设计缺口影响当前实现时先修订对应合同，不能通过泛型空表、删除真实引用检查或特殊 CORE 分支过关。

Stage 9 只有同时满足以下条件才完成：

- 全新进程只用完整 `.slib` 闭包、普通 runtime 对象索引和明确 target/toolchain 产生并运行可执行文件；所有 Scoop 与 runtime 源树、前端内存状态不可用。
- Link 不执行 HIR/MIR 语言语义或模板具体化；Compile 与 Link 共用必要机器检查，没有新的平行 verifier 或发布凭证。
- runtime/system 的实际定义／exports、所有 SourceExtern、String alias、target-generated 引用和对象均闭合；没有 core API 白名单或专用 extern 路径，不依赖 `cc` 隐式搜索或 fixture-native 注入。
- 真正逐 member ODR coalesce、多 object stackmap、eager/lazy/failed/cycle、GC/异常/gateway、ZST/大值/引用及重建 core 的组合通过。
- 正常格式、ABI、对象、最终绑定检查和原子发布完成；独立／组合／negative／golden 与关闭更新的全仓回归通过。
- 需要额外物理输入的 native requirement 有稳定诊断，文档记录实际交付与剩余 Stage 10/11 工作；没有将本阶段设计或旧 harness 通过记为实现验收。

## 12. 后续交接

M23-10 复用本阶段的独立 Link reader、统一 extern 合并／解析器、对象/definition/reference 模型、runtime 对象、启动代码、系统 linker 和最终检查；增加用户 native direct/archive/dynamic 输入的定位、候选／抽取／绑定。它扩展物理输入供应能力，不另写 core 或普通库的 extern 解析器，也不重新建立 HIR native 前端。必要快照和可选 final cache 围绕真实输入内容实现，不能恢复来源认证、通用预算或完整程序重放。

M23-11 让正式 `scoop build/run/link` 使用同一编排与 program-link，把历史单文件与跨 Cone fixture 迁入生产工具链，删除剩余测试胶合。`scoopc` 始终只产生一个 Cone 的完整 `.slib`；它不构建 runtime，也不顺带产生最终 executable。

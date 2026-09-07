# M23 执行计划

版本：0.1

最后更新：2026-09-07

状态：进行中

本文是 `docs/milestone23/DESIGN.md` 的执行账本，不替代语言、runtime 或实现规范。语义冲突时以 `docs/specs/` 下的规范为准；发现设计缺口时先修订规范，再继续实现。

## 1. 执行纪律

- 每个可独立验收的功能作为一个提交；实现、单元测试、negative fixture 与相关 golden 同提交，不把已知失败留给后续提交。
- 同一切片的代码、validator、artifact、fixture 与审查修复先集中完成，再统一执行 `cargo fmt --all`、受影响 crate 的定向 clippy / 测试；专项完整 suite 只在该切片稳定后的完成门运行一次。
- HIR / MIR / LIR 的信息必须结构完备；stage 之间只通过各自 IR crate 通信；`scoop` 与 `scoopc`、program-link 之间只经 manifest/protocol/`.slib` 边界通信，Cargo dependency 方向即工具边界（DESIGN §5.5）。
- reader 对损坏输入（`.slib`、CBOR、ar、manifest）必须无 panic、有界、返回结构化错误。
- 每个功能提交执行 `git diff --check` 并确认没有 `.snap.new`；实际命令与结果记录在第 5 节。
- 本文件持续更新：开始任务时标为“进行中”，完成验证并提交后记录 commit 与验证结果。

## 2. 阶段与任务

执行顺序总体遵循 DESIGN 第 10 章，但按“每个提交可构建、可验证”重排为下列任务。任务编号 Tn，完成后标注提交哈希。

### 阶段 A：共享身份与 manifest 基础（DESIGN §1.1、§1.2、§3.1）

- **T1 spec/ROADMAP 同步核验**（已完成：`4ea2442e` 已把三份 spec 与 ROADMAP 同步到 M23 设计；语言规范 §12、IMPL spec 2.x、runtime spec 相应章节齐备）。本计划落地时若发现 spec 缺口随时先改 spec。
- **T2 `compiler/identity` crate**（已完成）：canonical CBOR codec（`CborReader`/`CborWriter`：最短整数、定长、integer-key map 严格递增、拒绝 float/tag/indefinite/reserved、声明长度不超输入、nesting 预算、`SeqGuard`/`MapGuard` RAII）、`Digest256` + `DomainHasher`（domain tag + 每字段 u64 LE 长度前缀）、`ConeCoordinate`（group/name grammar、canonical SemVer 2.0.0 文本保留、SemVer 优先序 + build tiebreak 的全序）、`ConeIdentity`（`scoop-cone-id-v1` domain hash over canonical CBOR）、reserved core coordinate、`CapabilityId`（namespace 255B/segment 63B grammar + major ≥ 1）、`TargetProfileWireId`/`ObjectFormatWireId` typed wrapper 与四个内建 capability（darwin-aarch64 target profile、mach-o-relocatable、scoop-lir、generated-c-bridge link-object verifier）。验证：`cargo test -p scoop-identity` 17/17；`cargo clippy -p scoop-identity --all-targets -- -D warnings`；workspace build。提交 `37dc0c0b`。
- **T3 `compiler/manifest` crate**（已完成）：`Cone.toml` v1 严格解析（`toml_edit`，span 诊断）：schema=1、`[cone]` 四字段（group/name/version/kind）、`[dependencies]` exact key + string 短式/inline table/sub-table 三种 value 形态、locator 互斥、unknown field 拒绝（含拼错 semantic 字段）、reserved core coordinate 依赖声明拒绝、sysroot core manifest 自身合法。semantic/locator projection 分离（`semantic_dependency_coordinates()` 不含 locator）。executable-as-dependency 与坐标一致性检查属 graph 解析（T8/T6），经本 crate 的模型表达。验证：`cargo test -p scoop-manifest` 15/15；clippy `-D warnings`。提交 `d4081687`。
- **T4 `compiler/protocol` crate**（已完成）：`Message::{Hello, BuildRequest, BuildOutcome}` canonical CBOR 编码 + `u32` LE 长度前缀 frame（`write_frame`/`read_frame`，16 MiB 上限）；`CompilerIdentity`（protocol/slib container/HIR/MIR/LIR wire schema/identity schema 七字段 exact match，compiler version 仅诊断）；`ProtocolDiagnostic`（severity/phase/Cone-relative source location，无 host 绝对路径）。验证：round-trip、truncation、unknown tag、篡改 payload、frame 循环 9/9；clippy `-D warnings`。提交 `37dc0c0b`。

### 阶段 B：`.slib` 容器与 envelope（DESIGN §4.1、§4.2）

- **T5 slib envelope（Graph view）**（已完成）：canonical SysV/GNU short-name ar writer/reader（60-byte header 逐字段 canonical 校验、`` `'\n' `` fmag、`m%08d` 目录名、thin/special/reserved member 拒绝、odd payload `0x0A` pad）；`MemberStableKey`（tag 1–6）+`SlibMemberRole`+`MemberPurposeSet`+`SlibMemberRecord`（field key 1–5）CBOR 编解码与 key/role/capability 配对、`required_for` v1 组合规则；`SlibMemberId = SHA-256("scoop-slib-member-v1"‖ConeIdentity‖canonical key)`；`manifest.cbor`（magic SCOOPSLIB、container/wire schema 版本、语言/runtime/identity ABI、coordinate+identity 重算、kind、dependency 表、target/backend fingerprint、member 目录 field 19、artifact fingerprint field 20，指纹排除自引用重算）；`SlibBuilder`（BTreeMap 按 id 排序 → bitwise 确定性）；`DecodedSlibEnvelope`（长度/hash/序数名/资源预算校验）→ `ValidatedGraphArtifact`；集中式 `SlibDecodeLimits`（§4.5 常量）。测试 12/12：ar golden bytes、reserved/non-canonical/truncation 拒绝、member id 派生、pairing 矩阵、round-trip + identity 重算、bitwise 确定性 + 插入顺序无关、物理名序数、损坏矩阵无 panic、预算、optional blob 只改 ArtifactFingerprint。后续 Compile/Link view 与 wire schema 随 T19/T27 扩展。提交 `3fd836d7`。
- **T6 dependency table 与 artifact 一致性**（已完成，含于 T5 之上独立提交）：slib `closure.rs` 的 `validate_explicit_closure<P>`：reserved core 身份/core 零依赖、跨集合重复 identity、direct/support 错分、`group:name` 多 version、executable dependency、悬空边（= 缺失 artifact）、不可达 support、cycle/自环（迭代 DFS 带 coordinate path）、target profile 与 schema/ABI 逐字段一致性；输入顺序无关（按 identity 排序后处理）。`ValidatedArtifactClosure<P>` purpose marker。dependency fingerprint 记录已在 manifest 结构中（record 型），stale-fingerprint 比对在 T21 语义指纹落地后接入。验证：closure 测试 15 项 + envelope 12 项全绿；clippy `-D warnings`。提交 `d3abbc6c`。

### 阶段 C：工具边界（DESIGN §1.3、§1.4、§5.5）

- **T7 source discovery 与 ConeOutputKind**（已完成）：driver `cone.rs`：`discover_cone`（`Cone.toml` 语义读取 + `src/**/*.scoop` 精确扩展名 regular file 递归收集、规范化 Cone-relative 路径、UTF-8 byte 排序、symlink 全拒、非 regular/非 UTF-8/重复归一化/空集合/缺 manifest/缺 src 七类错误）；`ResolvedConeInput::from_parts` 供 fixture runner 构造 synthetic 输入（不进 CLI/env/.slib）；`ConeSourceFile` 路径校验（无 `.`/`..`/空段/前导斜杠）。scoop-hir 增加 `ConeOutputKind::{Library, Executable{local_entry}}` 封闭类型（entry 规则接线随 T13 parser/T22 core 分离落地——当前 `Module.entry` 仍为必有字段，届时替换）。验证：cone_discovery 7/7、scoop-hir 14/14、clippy。提交 `6f9d98ed`。
- **T8 `compiler/scoop` graph**（已完成）：`compiler/scoop` crate（bin `scoop` + lib，Cargo 依赖仅 identity/manifest/slib——依赖方向即工具边界）。`resolve_graph` 经 `BuildInputs` trait（磁盘实现 + 内存测试 harness）：root manifest 出发的 locator 解析（path→source manifest、artifact→Graph view、search→`<group>/<name>/<version>/cone.slib` 多 root 全候选 fingerprint 一致性）、implicit core edge（trusted sysroot slot，非 core Cone 必挂）、coordinate 逐字匹配、identity 重复、`group:name` 单 version、executable dependency/root 唯一 executable、prebuilt 传递依赖经 dependency record 的 coordinate 走 search 解析、cycle（迭代 DFS + coordinate path 诊断）、canonical topological order（ready set 中 coordinate byte order 最小）。前置：slib `DependencyRecord` 增加字段 5-7 存 dependency coordinate（identity 不可逆，`scoop` 需要它做搜索定位；reader 校验 coordinate 哈希 == identity）。测试 13/13。提交 `6f9d98ed`。
- **T9 `scoop` 调度与缓存（fake runner）**（已完成）：`schedule.rs`：`schedule_build` 按 canonical order 逐节点处理——Core/Prebuilt 经 `ReuseGate` 门禁（生产 gate = Compile+Link 双 view；测试可编程失败；当前默认实现为 Graph view 门禁，双 view 接线随 T19/T27）；Source 节点计算 cache key（`scoop-cone-cache-key-v1`：manifest semantic projection + 排序 source SHA-256 + CompilerKeyIdentity 八字段 + 全部 direct dependency 三层 fingerprint —— interim 由 metadata member payload digest 派生 `DependencyLayers::from_manifest_core`，T21 换真正 Merkle 指纹）→ cache hit（gate 后复用、零调用）/ miss（恰好一次 `ScoopcRunner::build`，父进程重读 artifact、Graph view 重验、coordinate 匹配、gate、cache 原子发布）；失败停止 dependents（`Aborted`）；stale prebuilt 报告而非重建。测试 21/21（graph 13 + schedule 8：恰一次/顺序、全 cache hit、源变化失效链、失败截断、stale prebuilt、拒绝输出不进 cache、layers 变化、cache key 四因子单测）。提交 `4eac40e9`。
- **T10 `scoopc`/`scoop` `build` 双形态 CLI、单文件日常功能、`run` 与各阶段 dump 开关**（推迟，原因记录；范围按用户要求扩展）：CLI 形态与参数契约已由 `scoop-protocol::BuildRequest`、`scoop-build::BuildTask` 与 T6 闭包验证在库层面锁定（fake runner 路径已可测全部分支）。真正接线需 scoopc 能产出合法 `.slib`（Export HIR wire T18-T19、LIR member 化 T27、core 分离 T22）；在此之前挂出 CLI 只能得到"后续任务未完成"的占位错误，违反不留占位符纪律。T10 最终范围（DESIGN 5.5.1 / IMPL spec 2.7 已修订锁定）：
  1. `scoopc build <path>`：`<path>` 按精确 `.scoop` 扩展名区分 Cone root 与单文件；Cone root 形态 = `--direct-slib/--support-slib/--out-slib` 契约 + manifest 语义读取、source discovery 接线、closure 预验证、结构化诊断分层、原子发布 + 双 view round-trip；单文件形态 = executable synthetic Cone（coordinate 取祖先 `Cone.toml`，driver 已有该解析）→ 与其他 Cone 相同的 `.slib`（trusted core slot 唯一依赖）。
  2. `scoop build <path>`：同一双形态接受；单文件经 synthetic manifest 走完整图/cache/`.slib`/program-link 产出二进制（与 Cone root 完全同路径，配套子进程委托边界不变）。
  3. `scoop run <file.scoop> [--] [args...]`：单文件 build（复用 cache）+ 执行产物 + 转发参数与退出状态，不引入第二套编译路径。
  4. 各阶段 dump 独立开关 `--dump-ast/--dump-hir/--dump-mir/--dump-lir`（任意组合，启用者写 `<out>/<stem>.<stage>` 文本）；单阶段 stdout `--emit <stage>` 保留；fixture runner 继续走 driver 内部 API。
  5. 过渡期约定：core 分离（T22）前，两处 `build` 的单文件形态沿用既有 core+user 同单元管线实现（今日 `scoopc build <file.scoop>` 行为即其原型，保持可用使现有 fixture/脚本不受影响）；T22 后同一 CLI 切换为 trusted core artifact 的独立单Cone编译，dump 格式不变。dump 开关可先行落地（driver 内部已产出全部 dump，无需等待 `.slib` 管线）。

### 阶段 D：persistent identity 与 mangling（DESIGN §3.1、§3.3、§3.5）

- **T11 persistent id 框架**（已完成，`scoop-identity::persistent`）：13 个 kind 隔离 newtype（`PersistentTypeId`/`GenericTypeId`/`FunctionId`/`GenericFunctionId`/`GenericCallableId`/`PropertyId`/`ExtensionPropertyId`/`TypeAliasId`/`DispatchSlotId`/`InitializationUnitId`/`LayoutId`/`StaticStorageId`/`ImmortalObjectId`/`SafepointSiteId`/`CallableBodyId`，`from_definition_key(cone, canonical key)`，无裸 digest API）；`DefinitionKey::{Source, GeneratedByOwner, GeneratedByRole}` canonical CBOR（typed owner chain、封闭 OwnerKind/GeneratedRole tag）；`ExactTypeKey` 七 variant + `ExactTypeTable`（intern 前置子注册检查、id 重算、DAG 无环验证）+ `CanonicalDiagnosticName` printer（esc `%HH` 大写、`n()/g()/a()/t()/f()/r()/x()` 语法、`-` 空 owner、C/I/S/E/O/A tag、role 01-05 两位小写 hex、memoized per-path subtree cost 16 MiB 上限 + cycle 拒绝）；`CallableBodyKey` 四 variant（u32 LE tag + 声明序 product，Strong/Odr/RootGateway/InitStartupGateway，typed refinement `StrongCallableDefinitionOwner`/`CallableOdrMemberId`/`MainCallableBodyId`）+ decode 拒绝 unknown tag/截断；`SpecializationKey` 四 variant（NoOwnerApplication/NoCallableArguments typed marker 与空 Vec 结构可区分）+ `OdrGroupId`/`OdrMemberId`（role 13 类不入 group key）；mangler `scoop$1$<kind>$<hex>`（32 个 SymbolKind tag）+ `truncated_runtime_id` 非零 64 位。测试 26/26（26 项含 golden name vectors、ODR/markler/callable round-trip、cost 上限拒绝）。提交 `e981c547`。
- **T12 HIR 定义键接入与 mangler 切换**（T12a/第 1-3 批已完成，T12c 待做）：
  - **T12a**（`b4d4c748`）：`DeclarationOrigin { provider, file }` 落到 Function/Global/四个 nominal 声明；`hir::persistent`（PersistentWorld、PersistentIds、签名引用编码、function/generic ids）。
  - **第 1 批**（`303ac406`）：`CompilationUnit.cone` + concretize 桥接 `ExactTypeTable`，concrete `Module.exact_of/exact_types` 全类型物化。
  - **第 2 批**（`dc7188da`）：`Concretizer::function_symbol` 按 FunctionKey×genericity 矩阵计算 `concrete::Function.symbol`（Plain=PersistentFunctionId；specialization=OdrMemberId(Callable group, Body)；derived equality=GeneratedByRole；Intrinsic/Extern 空）。
  - **第 3 批**（`463fc367`）：mir-lower `declare_symbol` 消费 concrete symbol（entry 特例 `scoop_main` 保留至 T32）；删除 compact-v2 overload/instance mangling 死路径；`ManglingSchemaIdentity::PersistentV1` 默认（lir-lower 拒绝 CompactV2）；单测语义化改写；187 快照重刷 + 机械审计（符号归一化后一致、run/trap/warning 逐字相同）+ no-update 复跑稳定。
  - **T12c**（进行中，1/3 已落地）：
    1. **T12c-1 mir 侧 exact 传递**（已完成）：`mir::Type`/`mir::FunctionType` 补 `Eq+Hash`；mir-lower `run()` 末尾新增一次性翻译——遍历 `concrete.exact_of`，用现成 id 映射（`struct_map`/`class_map`/`EnumRegistry.by_hir`/`InterfaceRegistry.mir_id`/FunctionTypeId 按索引对齐的 `remap_idx`）把 concrete `TypeId` 翻成 `mir::Type`，Tuple/Ptr 递归、primitives 直接映射，得到 `mir::Module.exact_of: HashMap<mir::Type, PersistentExactTypeId>`。
    2. **T12c-2 生成型 nominal 的 exact 登记（在 mir-lower 生成点）**：box class（= core `Box` 模板对 value exact 的 NominalApplication，需 concretize 顺带导出 core Box generic template id 或在 mir-lower 经 export 侧查）、closure environment（GeneratedNominalRole::ClosureEnvironment + 捕获类型 exact 序列作 structural discriminator）、adapter/coroutine frame 同理用各自 role + 操作数 exact；全部写入 `mir::Module.exact_of`，禁止 arena ordinal 进键。
    3. **T12c-3 lir/codegen 消费端**（TD 部分已完成：interface/function-type/class 三类位点经 `persistent_td_symbol` 切换——exact 命中用 `mangle(TypeDescriptor, exact)`，未命中回退旧 `scoop_td_<name>`；String TD 保留 runtime 强符号 `scoop_td_String` 至 T32 core binding；closure/生成型 nominal 的 TD 留 T12c-2。剩余）：string constant（`scoop.str.N`→`mangle(StringConstant, …)` 以内容+序作 generated 键或按 DESIGN content-role）、scan program、vtable/itable symbol、layout helper。runtime 引用 `scoop_td_String` 的位点（image_roots 的 string_td）随之切换——runtime C 侧 `extern const ScoopTypeDescriptor scoop_td_String` 需改为由 program descriptor core binding 传入（与 T32 的 ScoopRuntimeCoreBindings 衔接；若 runtime 侧尚不能改，则 String TD 暂留强符号名并在 T12c 提交信息与本计划注明）。
    4. **T12c-4 mir-lower 合成函数符号**：thunk（dispatch/boxing.rs `format!("scoop.{name}")`）、init/ctor（`scoop.init.X.$cN`/`scoop.ctor.*`）、coroutine driver/adapter、variance bridge、callback adapter——全部换 `GeneratedByRole`/`GeneratedByOwner` persistent 键（mir-lower 持 cone？concrete Module 已含 exact_of 但无 cone——需把 cone 一并存入 concrete::Module 或经 mir meta 传递，二选一在实现时定）。
    5. **T12c-5 global/singleton**：`mangle_global`/`mangle_singleton_root`→`PersistentStaticStorageId`（GeneratedByRole InitStorage + property owner）与 `mangle(SingletonRoot, …)`。
    6. **T12c-6**：snapshot 全量重刷 + 符号归一化机械审计（模式沿用本轮：`scoop\$1\$[A-Za-z0-9_$]+` 与旧式各 namespace 归一化后多重集一致、run/trap/warning 逐字相同）+ no-update 复跑 + M25 EH 门禁回归。

### 阶段 E：parser package/import（DESIGN §5.1、§2.1）

- **T13 AST/parser 文件头**：`SourceFile { package, imports, declarations, span }`、`PackageSyntax::{RootPackage, QualifiedPackage}`、`ImportSyntax::{Exact{public,path,alias}, Star{public,path}}`；语法约束（package ≤1 且最先、import 仅文件头、star 无 alias、qualified path 非空 identifier 序列）；per-header 恢复边界；多错误恢复。parser 正/负测试 + AST golden。

### 阶段 F：HIR semantic world 与 import（DESIGN §2.2–2.5、§5.2）

- **T14 SemanticWorld 与 binding group**：provider = `WorldConeId`（persistent `ConeIdentity`）；`ImportedBinding`/`ImportedTargetBinding`/`ImportBindingSource`（CurrentCone | DirectDependency）落地为 resolver 输入；diamond 按 origin id 合并、witness 排序去重；split-package 歧义诊断。
- **T15 候选层接入**：无 receiver 层级 lexical → this member → exact import → current package → star → prelude → contextual variant fallback（M22）；extension scope exact → package → star → prelude；每层 shape filter + applicability + MSC 语义保持（M16）；同层 origin 去重；声明/依赖顺序不 tie-break。
- **T16 public import / re-export**：re-export binding 写入当前文件 package、source 全部 DirectDependency、destination 冲突诊断、star 展开 snapshot、链式 re-export witness。
- **T17 跨 Cone visibility**：public lookup surface 限定；internal 限 origin Cone；file/member private 不因 metadata 存在可见；inheritance surface 只在 subclass/implementation 上下文经 witness 访问。删除 `user_file_index` 驱动的特判（core 分离时完成）。

### 阶段 G：Export HIR 闭包与 wire（DESIGN §4.3、§4.4）

- **T18 Export surface 拆分**：HIR 输出结构上区分 PublicLookupSurface / InheritanceSurface / TemplateSupportClosure / InterfaceDependencyClosure / SourceInterfaceTemplates / BindingIndex；hidden/inheritance-only 实体无普通 binding entry。
- **T19 HIR wire schema + Compile view**：Export HIR 显式 wire 编码（typed id、canonical CBOR、无 arena 序数）、reader 两阶段（identity 验证 → typed remap）、`ValidatedCompileArtifact`、`ImportedHirSet`；round-trip + corruption 测试。
- **T20 MIR/LIR wire schema**：MIR meta（external bridge、dispatch/ancestry、ODR relation）与 LIR meta（layout/scan/TD/callable/definition surface）wire 化，`ImportedMirSet`/`ImportedLirSet`、`CrossConeUseSet` 投影。
- **T21 三层 semantic fingerprint**：HIR/MIR/LIR Merkle fingerprint（own-layer + support edge），packager 写入 manifest；保守基线 = 全部 direct dependency 同层 fingerprint。

### 阶段 H：core 分离（DESIGN §5.4）

- **T22 core 独立编译**：sysroot trusted slot 中预编译 `scoop:scoop.core:0.1.0` 为 `.slib`；`scoopc` 验证 core authority（仅 trusted slot 来源）；core prelude 从 typed export metadata（package star + Option variant scope）读取；删除 core/user 同单元编译与 `user_file_index`/Option 短名特判；`scoop` bootstrap 重建逻辑。

### 阶段 I：跨 Cone MIR/LIR 与 generic（DESIGN §4.4、§8）

- **T23 imported meta 消费**：MIR/LIR lower 接入 `SelectedImportedMir/Lir`；external callable/TD/layout 消费（复用既有 `ExternalCallable`/`ExternalTypeDescriptor` 钩子并补全 producer）；跨 Cone 继承/接口/默认实现/vtable/itable。
- **T24 consumer generic concretization**：上游 generic template 在 consumer Cone 的 HIR concretization → LocalConcrete 实例 → MIR/LIR/发射；hidden support closure 的 param-free helper 作为 external target；同 specialization ODR coalesce 的 HIR/MIR 侧准备。

### 阶段 J：ZST 与 typed ABI（DESIGN §3.6、§6.1 部分）

- **T25 layout/ABI ZST 收口**：`ValueStorageLayout`/`ArrayElementStorage`/`TypeInstanceShape` 封闭 sum 全阶段贯通；ZST elision ABI（`ElidedZst` 已有 M22 基础，补跨 Cone 一致性）；ZST box/unbox、static identity token、`Array<ZST>` 数据 offset/scan/迭代规则；runtime scan v1 canonical form 与资源预算（writer + reader/runtime 双侧）。
- **T26 descriptor ABI header**：`runtime/include/scoop_runtime_metadata_v1.h`（§6.1 全部 record + magic/size 常量 + `_Static_assert`）、Rust 侧 mirror + canonical encoder golden（与 C runtime 共享 golden vectors）。

### 阶段 K：codegen collection 化与 ODR（DESIGN §3.4、§7）

- **T27 multi-member codegen 与 object verification**：`ProvisionalLinkObjectMembers` + `MemberMaterializationIndex`；`ObjectDefinitionPlan`/`DigestFinalizationPlan`；per-member object verifier（Mach-O symbol/section/relocation 解析、boundary atom、definition range）；fingerprint finalizer（typed DAG、patch site 回填）。
- **T28 ODR group 与 coalesce**：`OdrRecord` manifest 记录、`linkonce_odr` 发射、link 前 member set/definition fingerprint 比较、重复 specialization 真正 coalesce 验证。
- **T29 generic delegated extension property**（§7）：specialization key、lazy ODR group（storage/cell/failure/initializer/ensure/descriptor）、exactly-once 语义、moving GC root。

### 阶段 L：runtime-build、program-link 与 runtime registry（DESIGN §3.7、§5.5、§6）

- **T30 `compiler/runtime-build`**：从 profile 固定受信任 source set 构建 `ValidatedRuntimeArtifact`（typed object records、definitions/requirements、fingerprint）。
- **T31 program-link**：`compiler/linker` artifact-only 组件；`ValidatedArtifactClosure<Link>` 消费、member 物化（create-new、Cone 分区）、native library requirement 合并、`VerifiedProgramDescriptorObject` 生成与反验、`ResolvedLinkPlan` + `FinalLinkEvidence`（v1 范围：Cone object、program object、runtime object、必要 native input；archive/dynamic provider 按 §3.7 封闭建模但按实际输入验证）、`scoop link` CLI。
- **T32 runtime registry 启动**：runtime main 改为 program-descriptor 驱动；§6.2 六阶段 envelope/provisional/semantic/image-hash/graph-hash/commit；多 image root/TD/init/callable/stackmap 登记；canonical 初始化顺序（依赖优先 + coordinate + unit id）；no-throw root/init gateway + transition wrapper；删除 `scoop_main`/`scoop_image_*` 直接引用。

### 阶段 M：组合测试与收口（DESIGN §8、§9）

- **T33 组合 fixture 矩阵**：`tests/fixtures/m23-*`（imports/reexports/generics/zst/initialization/errors）+ `tests/slib/` format/corruption/reproducibility；多 Cone fixture runner（每 Cone 目录 + manifest，graph root 快照）；M1–M22 fixture 迁移 synthetic cone input 回归。
- **T34 缓存失效矩阵与可复现门**：HIR-only/layout/object-only 变化分别触发正确 fingerprint；两路径 bitwise `.slib` equality；`scoop` fake runner 顺序锁定；真实多 Cone 端到端（imports → generics → dispatch → init → moving GC/exception）。
- **T35 M23 收口**：完成门清单（DESIGN §10 末段）逐项核验、ROADMAP 标记完成、清理旧入口（single-file 生产路径、`scoop_main` 固定符号、compact mangling 生产使用等）。

## 3. 环境注意事项

- 本机 brew 默认 `llvm` 已升级至 23.1，与工具链要求的 LLVM 22.1 不符；`llvm@22` 仍安装于 `/opt/homebrew/opt/llvm@22`。构建/测试 codegen 前须 `export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`，否则 llvm-sys 重链 23.1 后 codegen 全部单测报 "unsupported LLVM backend"。

## 4. 关键不可变决定（本计划锁定）

- v1 只支持静态无环 exact Cone 图；无 registry/lockfile/dynamic loading。
- `scoop`（umbrella）、`scoopc`（single-Cone）、program-link（artifact-only）三层工具经 Cargo dependency 方向固定；`scoopc` 不递归构建、不最终链接。
- `.slib` = canonical ar + typed member directory + canonical CBOR；无第二真源（物理名/扩展名/顺序不承担语义）。
- persistent typed id 按 kind 隔离、domain-separated SHA-256、canonical key 仅用于验证/诊断；无裸 `[u8;32]` 公共 API。
- 保守缓存基线：HIR compile key 纳入全部 direct dependency HIR Merkle fingerprint；宁可多编译，不可错误复用。
- 串行调度；诊断排序不依赖 HashMap/枚举顺序。

## 5. 验证矩阵

| 变更类型 | 最低验证 |
| --- | --- |
| identity/manifest/protocol | 单元测试 + canonical/grammar 拒绝矩阵 |
| slib envelope/wire | golden bytes、corruption 无 panic、resource limits、reproducibility |
| graph/scheduling | in-memory/fake runner 行为锁定 |
| parser/AST | parser 单测 + recovery negative + AST golden |
| HIR resolver/import | HIR 单测、transaction、Export HIR golden、negative fixture |
| MIR/LIR/LIR wire | stage golden、round-trip、cross-layer bridge 验证 |
| codegen/object/ODR | LLVM artifact 检查、object verifier、ODR 一致性失败用例 |
| program-link/runtime | 端到端运行、moving-GC stress、descriptor 损坏 fatal |
| 文档 | `git diff --check`、跨 spec 引用复核 |

## 6. 更新记录

- 2026-09-07：建立本计划。T1 经核验已完成（spec 同步在 `4ea2442e` 及此前文档提交中闭合）。开始 T2 `compiler/identity`。
- 2026-09-07：完成 T2 `compiler/identity`（canonical CBOR、domain hash、coordinate/identity、capability registry）。注：`SlibMemberId` 与 `ArtifactFingerprint` 的定义随 T5 slib envelope 落在 slib 侧，identity crate 只承载跨 crate 共享的底层类型。开始 T3 `compiler/manifest`。
- 2026-09-07：完成 T3 `compiler/manifest`（Cone.toml v1 严格解析 + semantic/locator projection 分离，15 测试）。开始 T4 `compiler/protocol`。
- 2026-09-07：完成 T4 `compiler/protocol`（CBOR frame + CompilerIdentity exact match + typed 诊断，9 测试）。开始 T5 slib envelope。
- 2026-09-07：完成 T5 `.slib` envelope 基础（canonical ar、typed member directory、manifest.cbor、Graph view、SlibBuilder、集中 decode 预算，12 测试）。开始 T6 显式闭包一致性验证。
- 2026-09-07：完成 T6 显式闭包一致性验证（closure.rs，15 测试：chain/diamond/core-only 正向 + 10 类错误）。开始 T7 source discovery 与 ConeOutputKind。
- 2026-09-07：完成 T7 source discovery 与 ResolvedConeInput/ConeOutputKind 类型（7 测试）。开始 T8 compiler/scoop graph。
- 2026-09-07：完成 T8 `compiler/scoop` graph 解析（13 内存测试：chain/diamond/tie-break/全部错误类 + DependencyRecord coordinate 扩展）。开始 T9 调度与缓存（fake runner）。
- 2026-09-07：完成 T9 调度与缓存（fake runner，8 新测试）。T10 评估后推迟到管线可产出 artifact 时与端到端一起落地（契约已在库层锁定）；开始 T11 persistent id 框架。
- 2026-09-07（用户要求，范围扩展）：单文件编译定为 `build` 的一等输入形态（非独立子命令、非仅供开发），配套 `scoop run` 与各阶段独立 dump 开关；DESIGN 5.5.1 与 IMPL spec 2.7 已按此修订（`<path>` 双形态、synthetic Cone、core 分离前后过渡约定、`<stem>.<stage>` dump 文件、`--emit` stdout 保留）。T10 据此扩展，暂不实现。
- 2026-09-07：完成 T11 persistent id 框架（typed newtype ×15、DefinitionKey、ExactTypeKey/Table + canonical name printer、CallableBodyKey、SpecializationKey/ODR、mangler，26 测试）。开始 T12 HIR 定义键接入与 mangler 切换。
- 2026-09-07：完成 T12a DeclarationOrigin + hir::persistent（PersistentWorld/PersistentIds/签名引用编码/function ids，4 新测试，hir-lower 740 全绿）。T12b/c（MIR/codegen 符号切换与 snapshot 刷新）随后。
- 2026-09-07：T12b 部分落地：`CompilationUnit.cone`（driver 从 Cone.toml 祖先解析用户 Cone 身份，无 manifest 时用稳定测试 Cone `scoop.test:user:0.0.0`）；`concretize::lower(module, cone)`；concrete `Module` 新增 `exact_of: HashMap<TypeId, PersistentExactTypeId>` 与 `exact_types: ExactTypeTable`——concretize 末尾对全部 interned 类型物化 exact 身份（primitive 经 export 整数/Boolean/String core 声明、Unit/Any 为 core 虚拟声明、nominal app 递归、tuple/fn/fnptr/ptr 结构键），按依赖序 intern 并 debug_assert 与 memo 一致；`PersistentWorld` 语义修正（provider 0=core，其余=当前 Cone，覆盖 allowlisted 测试 provider）；`PersistentIds` 新增 `builtin_type_id`/`generic_callable_id`/`generic_{enum,class,interface}_id`。测试：m23_persistent 第 5 项（表完备/验证/distinct/core 跨 cone 稳定）。提交 `303ac406`（全树验证：workspace 1709/0、scoopc lib/cone/mir_determinism、fixtures 4/5 全绿）。
- 2026-09-07：T12b 进行中发现并修复 M22 遗留非确定性：`mir-lower/pipeline.rs` 的 `interface_methods` HashMap 迭代顺序驱动接口槽签名填充→MIR 枚举懒创建顺序随进程随机（跨进程 dump 不稳定、snapshot 无法复现；已在 M22 收口提交 a64ebd73 验证存在）。修复为按 concrete arena 序排序后迭代；进程内/跨进程各 4 次验证 MIR dump 稳定。178 个受影响 fixture snapshot 以确定性顺序重刷并机械化审计（枚举 id 归一化后新旧逐行多重集全部一致，仅编号重排）。回归测试 `mir_determinism.rs` 锁定同输入双 lowering 的 MIR 实体序一致。修复独立提交 `8b3f41aa`（该提交点独立验证：build + mir-lower 102/102）。
- 2026-09-07：T12b 调研完成并写入续作步骤（concrete::Function 携带 symbol、concretize 桥接 ExactTypeTable、OdrMemberId 实例符号、overload 判别器退役、Unit/Any 虚拟声明开放问题）。实现进行中。
- 2026-09-07：T12b 第 2 批完成（`dc7188da`）：`concrete::Function.symbol` 全量计算（FunctionKey×genericity 矩阵 + structural derived equality GeneratedByRole 键 + 结构性 owner 空链修正），m23_persistent 第 6 项符号断言测试。注：本提交因计划锚文本失配漏带计划更新，随下一计划提交补记。
- 2026-09-07：T12b 第 3 批完成（`463fc367`）：mir-lower 符号消费切换 + `ManglingSchemaIdentity::PersistentV1`（lir-lower 拒绝 CompactV2）+ compact-v2 死路径删除（overloaded_names/instance symbol 字段/symbol()/all_arguments()/lower_params）+ 单测语义化（前缀/区分性/symbol_of，golden dump format! 插值）+ 187 快照重刷与机械审计（符号 token 归一化后逐行多重集一致；run/trap/warning 段逐字相同）+ no-update 复跑 4/4 稳定；workspace 1710/0。发现并顺带修复：历史重整时任务列表 T12 展开文本丢失（本条随任务列表恢复一并补全）。
- 2026-09-07：阶段门禁：`LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22` 下 workspace（除 scoopc slow fixtures）全绿 1704/0；记录 brew llvm 漂移的环境注意事项。
- 2026-09-08：T12c 第 2 批完成（mir-lower 合成实体符号迁移）：concrete `Module.cone` 字段；mir-lower `generated_function_symbol(cone, role, discriminator, operands)` helper（GeneratedByRole persistent 键，discriminator 一律结构性无 arena 序数）。迁移位点：class/struct 初始化器与构造器（InitStorage + `class-initializer|init.<name>`/`struct-constructor|ctor.<name>`）、init failure root（FailureStorage + unit stable_key）、box adjust thunk（AdjustThunk + 原 thunk 名）、closure/dynamic/函数桥 adapter（CallableAdapter + 签名编码）、continuation resume/resumeWithException、coroutine start、callable reference invoke（CallableAdapter + `<index>/<签名编码>`，index 为 reference 声明序——修复了仅按签名编码导致的同签名重复符号 LIR 冲突，fixture callable-references 捕获）、interface 抽象槽签名壳（CallableAdapter + interface/method 名）、managed global（InitStorage + `global/<name>`，Extern global 不经 mangler 保持原样）、singleton root（InitStorage + `singleton-root/<link_name>`）。显示名同步（$reference.N→$reference.<编码>）。保留旧式：`scoop.str.N`（字符串常量，per-occurrence 编号，T27 内容寻址时迁移）、`scoop_td_<name>`（closure/boxed 生成型 TD，T12c-2）。187 快照重刷 + 六模式符号归一化审计（含 `:`/`-` 字符、显示名、$Closure/$reference 名）全部一致；run/trap/warning 逐字相同；no-update 复跑 4/4；workspace 1710/0、mir-lower 102/102。
- 2026-09-08：T12 阶段整体 review（独立审查代理）发现并修复 2 BLOCKER + 1 MAJOR：(B1) 嵌套 nominal 定义键未携带 typed owner chain——`First.Nested`/`Second.Nested` 碰撞；修复为 `nominal_owner_step` 递归解析 owner 的 persistent id 进入四个 nominal id 与 generic template id 键（Object owner 经 backing class 的 class id + OwnerKind::Object tag；companion 对象方法恢复可解析）＋嵌套同名回归测试。(B2) generic template 缓存跨 nominal kind 共享、按裸 arena index 键控——struct/enum/class/interface 独立 arena 原始索引互相碰撞时首个 kind 污染后续查找；修复为 per-kind 缓存＋混合 kind 回归测试。(M) 合成符号 discriminator 含 arena 序数（callable-reference 的 concrete reference index、ctor 的 source_discriminator）——分别改为 normalized source span（DESIGN 允许的 per-expression 实体源身份）与构造器参数类型序列的结构编码（`constructor_signature_key` 经 mir compact 编码声明序参数类型）。审查确认项：function_symbol 矩阵全覆盖无 reachable panic、exact 翻译在全部 registry 定型后执行、签名引用处理 Param/generic owner、无新增 TODO/unimplemented、文件长度达标。已知遗留（记录在案）：`from_validated` 是 wire-decode 所需的受信入口（文档契约）；`InstanceSymbol::Overloaded` 判别器与旧 mir mangler 死代码待 T12c-3 收尾清理；`global/<name>`、`interface-signature/<name>` 的名字型 discriminator 依赖当前单 package 无嵌套同名前提，package 落地（T13/T14）时需复查。187 快照重刷审计全过（符号归一化后一致、run/trap/warning 逐字相同）；workspace 1712/0、fixtures 4/4。
- 2026-09-08：T12c 第 1 批完成（mir exact 传递 + TD 符号切换）：`mir::Type/FunctionType` 补 `Eq+Hash`；`MirMeta.exact_of: HashMap<mir::Type, PersistentExactTypeId>`，mir-lower `run()` 在 lowering 完成后一次性翻译 concrete exact 表（nominal 经 struct_map/class_map/enums.mir_id/interfaces.mir_id、FunctionTypeId 索引对齐、Enum 键保留实参列表使结构查找命中）；lir-lower `persistent_td_symbol`（exact 命中→`scoop$1$td$<hex>`，否则回退 `scoop_td_<name>`）应用于 interface/function-type/class 三类 TD 位点；String TD 保留 `scoop_td_String`（runtime extern，T32 换 core binding）。187 快照重刷 + TD 符号归一化审计（全部仅 TD 符号变化，run/trap/warning 逐字相同）；workspace 1710/0、codegen 194/194、fixtures 4/4。

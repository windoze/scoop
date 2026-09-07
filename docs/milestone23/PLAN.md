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
- **T10 `scoopc build` CLI 契约**（推迟，原因记录）：CLI 形态与参数契约已由 `scoop-protocol::BuildRequest`、`scoop-build::BuildTask` 与 T6 闭包验证在库层面锁定（fake runner 路径已可测全部分支）。真正接线需 scoopc 能产出合法 `.slib`（Export HIR wire T18-T19、LIR member 化 T27、core 分离 T22）；在此之前挂出 CLI 只能得到"后续任务未完成"的占位错误，违反不留占位符纪律。CLI 在 T22 后与首个端到端 `.slib` 产物同批落地，届时补齐：manifest 语义读取、source discovery 接线、closure 预验证、结构化诊断分层、原子发布 + 双 view round-trip。

### 阶段 D：persistent identity 与 mangling（DESIGN §3.1、§3.3、§3.5）

- **T11 persistent id 框架**：typed newtype 家族（`PersistentTypeId`/`PersistentFunctionId`/… 按 §3.1 清单）、definition key canonical 编码与 domain hash、`ExactTypeKey`/`PersistentExactTypeId`（含结构 DAG 无环验证、dependency-first 编码）、`CanonicalExactTypeDiagnosticName` printer（source/generated atom、esc 规则、memoized cost + 16 MiB 上限）、`CallableBodyKey`/`PersistentCallableBodyId`、`SpecializationKey`/`OdrGroupId`/`OdrMemberId`、mangler `scoop$1$<kind>$<hex>`。全部为纯函数 + golden vectors。
- **T12 HIR 定义键接入**：HIR 为每个跨 artifact 实体计算 persistent id（origin ConeIdentity、package、owner chain、kind、名称、normalized signature key；编译器生成实体按 typed owner path），`IntrinsicProviderId` 的 Cone 身份职责迁移到 `WorldConeId`/`ConeIdentity`（§3.2）；MIR/LIR symbol 生成切换到 persistent mangler，消除 `scoop.<name>` 跨 Cone 碰撞；既有单 Cone fixture snapshot 全量刷新。

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

## 3. 关键不可变决定（本计划锁定）

- v1 只支持静态无环 exact Cone 图；无 registry/lockfile/dynamic loading。
- `scoop`（umbrella）、`scoopc`（single-Cone）、program-link（artifact-only）三层工具经 Cargo dependency 方向固定；`scoopc` 不递归构建、不最终链接。
- `.slib` = canonical ar + typed member directory + canonical CBOR；无第二真源（物理名/扩展名/顺序不承担语义）。
- persistent typed id 按 kind 隔离、domain-separated SHA-256、canonical key 仅用于验证/诊断；无裸 `[u8;32]` 公共 API。
- 保守缓存基线：HIR compile key 纳入全部 direct dependency HIR Merkle fingerprint；宁可多编译，不可错误复用。
- 串行调度；诊断排序不依赖 HashMap/枚举顺序。

## 4. 验证矩阵

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

## 5. 更新记录

- 2026-09-07：建立本计划。T1 经核验已完成（spec 同步在 `4ea2442e` 及此前文档提交中闭合）。开始 T2 `compiler/identity`。
- 2026-09-07：完成 T2 `compiler/identity`（canonical CBOR、domain hash、coordinate/identity、capability registry）。注：`SlibMemberId` 与 `ArtifactFingerprint` 的定义随 T5 slib envelope 落在 slib 侧，identity crate 只承载跨 crate 共享的底层类型。开始 T3 `compiler/manifest`。
- 2026-09-07：完成 T3 `compiler/manifest`（Cone.toml v1 严格解析 + semantic/locator projection 分离，15 测试）。开始 T4 `compiler/protocol`。
- 2026-09-07：完成 T4 `compiler/protocol`（CBOR frame + CompilerIdentity exact match + typed 诊断，9 测试）。开始 T5 slib envelope。
- 2026-09-07：完成 T5 `.slib` envelope 基础（canonical ar、typed member directory、manifest.cbor、Graph view、SlibBuilder、集中 decode 预算，12 测试）。开始 T6 显式闭包一致性验证。
- 2026-09-07：完成 T6 显式闭包一致性验证（closure.rs，15 测试：chain/diamond/core-only 正向 + 10 类错误）。开始 T7 source discovery 与 ConeOutputKind。
- 2026-09-07：完成 T7 source discovery 与 ResolvedConeInput/ConeOutputKind 类型（7 测试）。开始 T8 compiler/scoop graph。
- 2026-09-07：完成 T8 `compiler/scoop` graph 解析（13 内存测试：chain/diamond/tie-break/全部错误类 + DependencyRecord coordinate 扩展）。开始 T9 调度与缓存（fake runner）。
- 2026-09-07：完成 T9 调度与缓存（fake runner，8 新测试）。T10 评估后推迟到管线可产出 artifact 时与端到端一起落地（契约已在库层锁定）；开始 T11 persistent id 框架。

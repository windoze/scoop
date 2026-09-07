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
- **T2 `compiler/identity` crate**（已完成）：canonical CBOR codec（`CborReader`/`CborWriter`：最短整数、定长、integer-key map 严格递增、拒绝 float/tag/indefinite/reserved、声明长度不超输入、nesting 预算、`SeqGuard`/`MapGuard` RAII）、`Digest256` + `DomainHasher`（domain tag + 每字段 u64 LE 长度前缀）、`ConeCoordinate`（group/name grammar、canonical SemVer 2.0.0 文本保留、SemVer 优先序 + build tiebreak 的全序）、`ConeIdentity`（`scoop-cone-id-v1` domain hash over canonical CBOR）、reserved core coordinate、`CapabilityId`（namespace 255B/segment 63B grammar + major ≥ 1）、`TargetProfileWireId`/`ObjectFormatWireId` typed wrapper 与四个内建 capability（darwin-aarch64 target profile、mach-o-relocatable、scoop-lir、generated-c-bridge link-object verifier）。验证：`cargo test -p scoop-identity` 17/17；`cargo clippy -p scoop-identity --all-targets -- -D warnings`；workspace build。提交 `<hash>`。
- **T3 `compiler/manifest` crate**（已完成）：`Cone.toml` v1 严格解析（`toml_edit`，span 诊断）：schema=1、`[cone]` 四字段（group/name/version/kind）、`[dependencies]` exact key + string 短式/inline table/sub-table 三种 value 形态、locator 互斥、unknown field 拒绝（含拼错 semantic 字段）、reserved core coordinate 依赖声明拒绝、sysroot core manifest 自身合法。semantic/locator projection 分离（`semantic_dependency_coordinates()` 不含 locator）。executable-as-dependency 与坐标一致性检查属 graph 解析（T8/T6），经本 crate 的模型表达。验证：`cargo test -p scoop-manifest` 15/15；clippy `-D warnings`。提交 `23c14ec4`。
- **T4 `compiler/protocol` crate**：`scoop` ↔ `scoopc` 版本化结构化消息（编译请求、typed 诊断、产物完成/失败），schema version 常量；首批只定义类型 + round-trip 测试。

### 阶段 B：`.slib` 容器与 envelope（DESIGN §4.1、§4.2）

- **T5 slib envelope（Graph view）**：canonical SysV ar writer/reader（精确 header 字段、`m%08d` 名、thin/special member 拒绝、odd pad）、`SlibMemberRecord`/`MemberStableKey`/`SlibMemberRole`/`MemberPurposeSet` wire 编码、`manifest.cbor` 结构（magic、版本、coordinate、kind、member 目录、fingerprint 排除规则）、`DecodedSlibEnvelope` → `ValidatedGraphArtifact`（`ArtifactPurpose<P>` marker、purpose closure、unknown optional/required capability 规则）、`MemberFingerprint`/`LinkMemberFingerprint`。测试：canonical ar golden、CBOR golden（field key/tag/purpose bit）、corruption 矩阵、resource limits（§4.5 常量集中一处）、bitwise reproducibility。
- **T6 dependency table 与 artifact 一致性**：manifest direct dependency records（`ConeIdentity` + 三层 semantic fingerprint 占位结构）、reader 对 closed input set 的图一致性验证（缺失/重复/错分/额外不可达/cycle/自环/多 version/executable dependency/伪造 core）→ 供 `scoopc` 输入检查复用。

### 阶段 C：工具边界（DESIGN §1.3、§1.4、§5.5）

- **T7 source discovery 与 ConeOutputKind**：`src/**/*.scoop` 递归收集（.scoop 精确扩展名、UTF-8、路径归一化、symlink 逃逸、空 Cone 诊断、byte 排序）、`ConeOutputKind::{Library, Executable { local_entry }}`（entry 规则：root 唯一、ordinary/non-generic/non-suspend/`() -> Unit`）、fixture runner 内部 synthetic `ResolvedConeInput` 能力。
- **T8 `compiler/scoop` graph**：只读解析全部 manifest（source）与 `.slib` bounded summary（prebuilt），建立 `ResolvedBuildGraph`：implicit core edge、coordinate/版本唯一、cycle（DFS + coordinate path）、canonical topological order（ready set + coordinate byte order）。fake/in-memory 测试锁定全部图错误与顺序。
- **T9 `scoop` 调度与缓存（fake runner）**：cache key 计算（manifest semantic fields、source digests、language/schema/runtime ABI、target、三层 Merkle fingerprint —— 先保守纳入全部 direct dependency）、prebuilt/cache 候选必须双 view（Compile+Link）门禁后命中、每个 source cache miss 恰好一次 `scoopc` 调用、失败停止 dependents。先以 trait `ScoopcRunner` 的 recording fake 锁定行为；真实子进程调用随后续任务接入。
- **T10 `scoopc build` CLI 契约**：`scoopc build <cone-root> --direct-slib ... --support-slib ... --out-slib ...`；解析当前 manifest semantic projection、发现当前 src、验证显式 `.slib` 闭包（T6 验证器）在 parse 源码前完成；不跟随 locator、不读上游 source。错误诊断分层（manifest/输入集合 vs 源码）。

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

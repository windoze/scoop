# M23-3 设计：single-Cone artifact 与 core 分离

版本：0.1（设计完成，待实现；2026-09-13）

依赖：M23-2

上位设计：`docs/milestone23/DESIGN.md`

规范依据：

- `docs/specs/SCOOP-SPEC.md` 第 9.1.3、12.1～12.6 节；
- `docs/specs/SCOOP-IMPL-SPEC.md` 第 2.1、2.6～2.8、2.11 节；
- `docs/specs/SCOOP-RUNTIME-SPEC.md` 第 2.2、2.8 节；
- `docs/milestone23/DESIGN.md` 第 0、1.1～1.3、3.3～3.5、4、5、6.1、8、9.3、10 章；
- `docs/milestone23/stage1/DESIGN.md` 与 `docs/milestone23/stage2/DESIGN.md` 已冻结的 source、identity、mangler、wire、profile 与 reader 契约。

本文只定义 M23-3。语言与最终 artifact 语义仍以上述规范和 M23 总设计为准；本文负责把“当前一个 Cone 如何成为可发布 artifact”闭合到可实现、可验证的 typed pipeline。M23-4 以后只能在这个 artifact 边界上增加构建图、名称语义、layout、ODR、runtime 与最终链接能力，不能恢复 core/source 拼接、裸 object 传递或 `scoopc` 直链旁路。

## 0. 结论

M23-3 第一次产生真正可发布、同时通过 Compile/Link 两种 purpose 验证的 `.slib`。它不是 M23-2 foundation archive 外加一个未经检查的 object。完成本阶段后：

1. `Cone.toml` v1、manifest source discovery、single-file synthetic input、library/executable 输出和 entry 选择都由封闭 typed input 表示；
2. `scoopc` 每次只编译当前一个 Cone，唯一成功产物是该 Cone 的 `.slib`；它不查找或构建普通 dependency，不构建 runtime，不调用最终 linker；
3. `scoop.core` 由 trusted sysroot source slot 独立编译为 reserved library artifact，普通 Cone 只消费经过双视图验证的 core `.slib`，不再把 core source 拼入当前 AST/HIR；
4. M23-3 只允许 core-only 的成功编译图。显式非 core dependency 会先完成输入闭包和 artifact 验证，再以稳定的阶段能力诊断结束；它不会进入 resolver；
5. 当前 Scoop LIR object 与 generated C bridge object 都通过自己的 versioned verifier capability，成员数量、文件名、扩展名和 archive ordinal 不承担语义；
6. 每个成功 artifact 的全部 `LinkObject` 联合定义且只定义一个当前 Cone 的 hidden strong `ScoopImageDescriptorV1`，并产生六类 strong registration、member-aware definition/undefined requirement、typed digest DAG、`StrongRegistrationFingerprint`、`RuntimeImageFingerprint` 与 `CodeFingerprint`；
7. M23-3 production profile 对任意 ODR group/member/body/linkage/symbol fail closed。即使 specialization 只在一个 Cone 中出现，也不能降级成临时 strong；真正 ODR 到 M23-7 才开放；
8. core 中允许跨 Cone 引用的 param-free source nominal，由定义 Cone 预物化 M23-2 冻结的有限 `ExactOwnerRoot` shape-support closure；consumer 只能引用这些 external strong definition，不能替 core 重新发射；
9. library 不需要 `main`；executable 在 HIR 输出中非可选地携带唯一 local entry，并由 LIR/codegen产生 root failure storage、root gateway 与 `ScoopRootEntryDescriptorV1`；它仍先成为 `.slib`，本阶段不生成 `ScoopProgramDescriptorV1` 或 binary；
10. writer 只有在同一份重新读取的 artifact 分别构造 `ValidatedCompileArtifact<SingleConeStrongProfile>` 和 `ValidatedLinkArtifact<SingleConeStrongProfile>` 后，才原子发布输出。Graph-only、Compile-only、foundation 或未验证 object 都不是成功产物。

M23-3 的“core-only”描述依赖关系，不等于恢复 core 专用的名称/ABI 后门。core prelude、well-known relation、MIR/LIR external bridge都来自 core artifact 中的 typed section；本阶段的 consumer API只开放该受限能力。ordinary exact/star import、re-export、generic concretization和通用跨 Cone layout分别仍由M23-5、M23-7和M23-6负责。

## 1. 范围与阶段边界

### 1.1 本阶段交付

- `Cone.toml` schema 1 的 strict parser、semantic/locator projection 分离与带span诊断；
- manifest Cone 的递归 `src/**/*.scoop` discovery、canonical logical path、symlink containment与确定性排序；
- `CurrentConeInput::{Manifest, SingleFile, TrustedCoreBootstrap}` 与 `ConeOutputKind::{Library, Executable}`；
- `compiler/manifest`、`compiler/protocol` 两个共享crate及其依赖边界；
- `scoopc build` 的 single-Cone request、`--direct-slib`、`--support-slib`、`--out-slib`与typed diagnostic协议；
- explicit dependency input的完整闭包验证，以及本阶段对非core dependency的稳定能力门禁；
- trusted core source/artifact slot、bootstrap authority、core独立artifact、typed prelude和well-known relation；
- current Cone parser/HIR/MIR/LIR/codegen pipeline，不再存在正式的core+user combined input；
- executable entry选择、root gateway/failure root/entry descriptor；
- `SingleConeStrongProfile`及本阶段mandatory manifest/HIR/MIR/LIR capability；
- strong-only `ObjectDefinitionPlan`、`DigestFinalizationPlan`、六类registration plan与image plan；
- Scoop LIR/generated C bridge 两种 `LinkObject` verifier；
- member-aware definition、undefined requirement、materialization、range、relocation和patch验证；
- `StrongRegistrationFingerprint`、`RuntimeImageFingerprint`、`CodeFingerprint`及其patch/finalization；
- `ScoopImageDescriptorV1`和六类descriptor的compiler-side producer及共享C ABI header；
- 基础 `ValidatedLinkArtifact<SingleConeStrongProfile>`；
- core bootstrap、core-only library/executable、single-file local root、reproducibility和损坏矩阵测试；
- legacy executable fixture runner与新artifact pipeline的单向隔离。

### 1.2 本阶段明确不做

- `scoop` umbrella binary、dependency locator、resolved multi-Cone DAG、cache或child scheduling；这些属于M23-4/M23-11；
- 普通 dependency 的exact/star/alias import、re-export、split-package lookup、跨Cone visibility/access/default语义；这些属于M23-5；
- 通用跨Cone `ValueStorageLayout`、typed Scoop ABI、scan/dispatch/protected bridge与完整ZST矩阵；这些属于M23-6；
- consumer-side generic concretization、generic hidden support、generic delegated extension、ODR member closure和coalescing；这些属于M23-7；
- C runtime对多image的登记、启动、初始化、stackmap串接消费或program graph验证；这些属于M23-8；
- runtime build、program descriptor object、native linker调用、binary、provider resolution或final link evidence；这些属于M23-9/M23-10；
- 正式 `scoop build/run/link`、默认输出目录和历史fixture整体迁移；这些属于M23-11；
- C/C++ source Cone member、任意raw object注入、artifact registry下载、签名、动态库ABI或动态image。

本阶段虽然冻结 `ScoopImageDescriptorV1` 与六类registration的producer形状，但不在 C runtime 中“试运行一半registry”。compiler/link reader验证的是relocatable object、typed relocation和canonical record；M23-8才在ASLR后的最终程序地址空间中执行完整consumer验证与commit。

### 1.3 与相邻阶段的交接

| 阶段 | M23-3 接收 | M23-3 交付 |
| --- | --- | --- |
| M23-2 | persistent identity、`PersistentV1`、foundation metadata、container/directory、Graph/Compile reader | publishable strong profile、Link reader、object/image/digest proof |
| M23-4 | 单次显式dependency input验证与versioned request DTO | 可由orchestrator调用的single-Cone child协议和双视图有效artifact；M23-4增加locator/DAG/cache，不改编译器请求语义 |
| M23-5 | core-only semantic world与core prelude受限入口 | 所有dependency均已有可信artifact/typed origin；M23-5增加ordinary public surface/import/re-export capability，不把transitive artifact自动变成候选 |
| M23-6 | core param-free source nominal的窄shape-support obligation和external strong引用 | 以新required capability推广到通用跨Cone layout/scan/ABI/dispatch，不改本阶段已发射identity/definition/image关系 |
| M23-7 | `RejectAll` production profile、已经冻结但不可发布的ODR identity | 新profile以完整member/definition proof开放ODR；不得把本阶段强定义事后解释为ODR |
| M23-8 | compiler侧完整image/registration ABI、canonical hash与object proof | runtime consumer、registry commit和启动；不得修改M23-3共享header布局 |
| M23-9/10 | 非空verified link-object集合、image owner、defined/undefined/native contract | artifact-only program-link和native provider闭包；不得补造本阶段缺失的requirement |
| M23-11 | 新single-file request与独立artifact路径、隔离的legacy runner | 启用正式CLI并删除legacy runner |

### 1.4 本阶段的封闭成功子集

一个请求只有同时满足下列条件才能成功发布：

- 当前输入是trusted core bootstrap、只隐式依赖trusted core的manifest Cone，或固定的single-file Cone；
- current Cone和其实际lowering/materialization没有产生任何ODR-owned实体；
- 所有跨Cone semantic target都来自本次已验证的trusted core capability，并属于M23-3明确允许的param-free/prelude strong子集；
- 当前LIR的每个linker-visible definition、undefined use、digest patch、registration和image relation都能落入本阶段封闭sum；
- 至少有一个已知capability的`LinkObject`，且全部object联合产生唯一image；
- Compile view和Link view均从最终输出bytes独立重建成功。

这里没有“先产残缺artifact，后续stage补齐”的成功形态。任何条件不满足都在当前请求内失败，且不留下可被cache/dependency消费的输出。

## 2. crate与依赖方向

### 2.1 新增共享crate

新增：

```text
compiler/manifest  (scoop-manifest)
  syntax/          strict TOML decode与span
  semantic/        ConeManifestSemantic、DependencyDeclaration
  locator/         只保存path/artifact spelling，不授予artifact proof
  discovery/       manifest source discovery
  input/           ManifestRootInput、SingleFileRootInput

compiler/protocol  (scoop-protocol)
  request/         versioned ScoopcBuildRequest
  response/        success/diagnostic envelope
  framing/         bounded same-toolchain child framing
```

`scoop-manifest`依赖`scoop-identity`、`scoop-wire`、`toml`与`semver`，不依赖parser/HIR/MIR/LIR、slib或driver。它只把host locator与semantic projection分开，不打开`.slib`。

`scoop-protocol`依赖`scoop-manifest`、`scoop-identity`、`scoop-wire`以及只含target selection request DTO的低层crate；它不依赖任何lowerer、codegen或driver implementation。协议中的host path只是同机child locator carrier，不进入canonical Cone/source/entity/artifact identity。

### 2.2 现有crate职责调整

- `compiler/driver`继续产出`scoopc` bin/lib；lib的正式入口改为`build_single_cone`，只编排当前Cone；
- `compiler/slib`新增strong production profile、Link-purpose capability registry、object verifier、typed Link proof与publish gate；
- `compiler/hir`、`mir`、`lir`分别拥有本阶段新增的canonical/decoded/validated/imported metadata类型；
- `compiler/codegen`只把一个`CurrentConeLir`变成provisional Scoop object、generated bridge input和member materialization，不打包`.slib`；
- generated C compiler调用由`scoopc`的native-member producer适配层执行，输入只能是codegen生成的canonical recipe与validated C-bridge projection；
- runtime source、runtime object和final linker从`scoopc`正式依赖图中移除。

M23-3不必为object verifier新建一个依赖`scoop-slib`的stage crate。`SlibMemberId`与purpose proof由slib crate所有，Link reader又必须调度capability handler，因此两种内建object verifier放在`compiler/slib`的`link_object`子模块；其纯Mach-O解析/normalize代码不得依赖writer或driver。现有`codegen/artifact`中可复用的Mach-O/EH检查应抽成不依赖inkwell的内部模块，避免writer和reader各有一套判定。

### 2.3 workspace依赖图

生产依赖保持：

```text
parser       -> ast
hir-lower    -> ast + hir
mir-lower    -> hir + mir
lir-lower    -> mir + lir
codegen      -> lir

manifest     -> identity + wire
protocol     -> manifest + identity + wire + target-request DTO
slib         -> hir + mir + lir + identity + wire + object parser
driver       -> manifest + protocol + parser/lower/codegen + slib
```

禁止：

- `hir-lower`读取`.slib`或`Cone.toml`；driver必须先投影成`ImportedHirSet`与current input；
- `slib`回调codegen来判断object“应该是什么”；reader必须从canonical LIR verification surface独立验证；
- `scoop-manifest`根据dependency path打开上游source/artifact；
- 新component调用legacy `compile_file`取得binary或core AST；
- 以共享全局arena/registry把两个Cone的未持久IR接在一起。

### 2.4 legacy runner隔离

M1～M22历史运行fixture在M23-11前仍可使用旧直编直链路径，但必须移动到显式`legacy_fixture`模块并满足：

- 名称、类型和返回值都带`Legacy`，不能实现新protocol trait；
- `scoopc`正式CLI、新`build_single_cone`、slib writer/reader和未来`scoop`不能依赖它；
- 新增M23-3测试不得借它取得artifact、object proof、runtime或link结果；
- legacy路径继续回归只是迁移保障，不构成第二套production语义；
- M23-11删除模块、feature和调用点为同一批变更。

## 3. `Cone.toml` v1

### 3.1 strict syntax projection

最小schema沿用总设计：

```toml
schema = 1

[cone]
group = "dev.example"
name = "sample"
version = "0.1.0"
kind = "library" # 或 executable

[dependencies]
"org.foo:bar" = { version = "1.2.3", path = "../bar" }
"org.acme:util" = { version = "2.0.0", artifact = "../util.slib" }
"org.other:log" = "3.1.0"
```

parser输出两份不可混用的projection：

```text
ParsedConeManifest {
    semantic: ConeManifestSemantic,
    locators: DependencyLocatorTable,
    diagnostic_spans: ManifestDiagnosticSpans,
}

ConeManifestSemantic {
    coordinate: ConeCoordinate,
    requested_kind: RequestedConeKind,
    dependencies: CanonicalMap<DependencyCoordinateKey,
                               ExactDependencyCoordinate>,
}

DependencyLocator = SearchRoots
                  | SourcePath(HostPathLocator)
                  | ArtifactPath(HostPathLocator)
```

`scoopc`只接收`semantic`和已经由调用者给出的artifact路径；它不获得能follow的locator API。M23-4的`scoop`可使用`locators`建立graph，但locator始终不进入semantic identity、`.slib`、fingerprint或source location。

### 3.2 字段与错误规则

- top-level只允许`schema`、`cone`、`dependencies`；三者中前两项必需，`dependencies`可省略并等价为空table；
- `[cone]`只允许`group`、`name`、`version`、`kind`且全部必需；
- dependency inline table只允许`version`、`path`、`artifact`；`version`必需，后两者至多一个；
- `schema`必须是integer 1，不接受string、float、1.0或未知版本；
- `kind`只接受精确lowercase `library`/`executable`；
- coordinate grammar、canonical SemVer与hash完全复用M23-2，不在manifest crate另写一套trim/normalize；
- dependency key精确为`group:name`，只含一个冒号且两侧分别满足group/name grammar；value中的version与key共同形成exact coordinate；
- 同一`group:name`重复、同表多version、空locator、同时给path/artifact、未知字段或错类型都在读取source前失败；
- 用户manifest不得声明reserved core或single-file coordinate，也不得显式声明`scoop:scoop.core:0.1.0` dependency；core edge由typed请求注入；
- executable dependency是否合法不由文本parser猜测，待artifact闭包验证获得上游kind后诊断；
- TOML parser的宽松行为不得吞掉duplicate key或unknown semantic field。每项错误保留原manifest host path和精确TOML span，但host path不进入semantic diagnostic key。

### 3.3 manifest根

M23-3低层入口只接受：

```text
ManifestRootLocator = ConeDirectory | ExactConeManifestFile
```

目录分支只读取其直接子项`Cone.toml`；文件分支要求basename精确为`Cone.toml`。其他regular file不启发式上溯或搜索相邻manifest。root path可为symlink，但在做source containment时使用解析后的真实root；调用时spelling只作诊断。

manifest Cone的source form固定为`Manifest`。即使其coordinate恰好与reserved值hash碰撞也不能获得reserved authority；合法构造器先拒绝reserved canonical coordinate，再计算identity。

## 4. source discovery与single-file

### 4.1 manifest source discovery

`scoop-manifest`从已验证manifest root的`src/`递归发现扩展名精确为`.scoop`的source。唯一算法为：

1. 解析并固定real Cone root与real `src/` root；`src/`缺失或最终目标不是directory时失败；
2. 对每个目录读取完整entry集合；任一entry名不是UTF-8即失败，再按relative component的UTF-8 bytes排序后遍历；
3. regular file仅在basename扩展名精确为`.scoop`时成为source；大小写不同、目录名以`.scoop`结尾、socket/device等均不成为source；
4. symlink逐跳解析。最终target必须位于real `src/` root内；逃逸、dangling link、cycle或超过统一symlink-depth budget均失败；
5. symlink到directory时按该logical path继续遍历，并以active real-directory stack检测cycle；同一real directory经不同非循环logical path到达不偷偷合并；
6. source identity使用从Cone root起算、含`src/`前缀的logical path，而不是symlink target real path；每个component交给M23-2的`NormalizedSourcePath::from_relative_path`；
7. 全部发现完成后按`NormalizedSourcePath` UTF-8 bytes严格排序并拒绝重复，才读取UTF-8 source text、构造`SourceIdentity { current_cone, logical_path }`；
8. source集合为空、文件在读取期间改变类型/逃逸、读取失败或内容不是UTF-8均使整个discovery原子失败。

允许同一real regular file由两个不同canonical logical path引用；它们是两个不同source identity，并按普通重复声明规则处理。不能用inode去重，因为inode/real path不是跨artifact语义。内容与symlink target变化由source content digest和后续cache key捕获。

discovery输出：

```text
DiscoveredManifestSources = NonEmptyCanonicalVec<DiscoveredSource>

DiscoveredSource {
    identity: SourceIdentity,
    display_locator: SourceDisplayLocator,
    source_text: SourceText,
    content_digest: SourceContentDigest,
}
```

`display_locator`只存在于请求sidecar；排序、parser source index、HIR identity、`.slib`与golden semantic name均使用`SourceIdentity`。

### 4.2 single-file input

显式single-file请求遵守总设计固定projection：

```text
SingleFileSemanticProjection {
    coordinate: scoop:single-file:0.0.0,
    kind: Executable,
    source_identity: SourceIdentity {
        cone: ConeIdentity::SINGLE_FILE,
        logical_path: "main.scoop",
    },
    ordinary_dependencies: Empty,
    implicit_core_dependency: Required,
    distribution: LocalExecutableRoot,
}
```

- operand basename的扩展名必须精确为`.scoop`；跟随symlink后的目标必须是已存在regular file；
- 不读取相邻`Cone.toml`、`src/`、其他`.scoop`、C/C++ source、archive或blob；
- resolved host path、basename和symlink spelling不进入identity。不同输入文件共享reserved Cone/source identity是设计内事实，artifact/cache区分依赖source content、core fingerprint及toolchain；
- `--direct-slib`和`--support-slib`都必须为空；
- 输出`.slib`可作为本次build/run或未来显式`--root-slib`的唯一root，但不能成为dependency、manifest locator候选或可分发library；
- M23-3只生成该`.slib`，不规定M23-11的默认binary输出路径。

### 4.3 parsed source原子门

parser接收按identity排序的`NonEmptyCanonicalVec<IdentifiedSourceInput>`，只有全部source无parser错误时才构造：

```text
CurrentConeParsedSources {
    cone: ConeIdentity,
    sources: NonEmptyCanonicalVec<IdentifiedParsedSource>,
    source_texts: CurrentSourceTextTable,
    diagnostic_context: CurrentDiagnosticContext,
}
```

private checked constructor证明所有source identity的Cone相同、logical path唯一、顺序canonical、source table与text/diagnostic sidecar全覆盖。不存在error node、missing source或跨Cone source混入的成功值。

## 5. single-Cone请求与dependency输入

### 5.1 typed request

正式library API接收：

```text
SingleConeBuildRequest {
    current: CurrentConeInput,
    dependencies: ExplicitDependencyInputs,
    trusted_core: TrustedCoreInput,
    target: ResolvedTargetProfile,
    output: SlibOutputDestination,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
}

CurrentConeInput =
    Manifest { root: ManifestRootLocator }
  | SingleFile { source: SingleFileLocator }
  | TrustedCoreBootstrap {
        source_slot: TrustedCoreSourceSlot,
        authority: CoreBootstrapAuthority,
    }

TrustedCoreInput =
    Artifact(TrustedCoreArtifactInput)
  | BootstrapSelf { artifact_slot: TrustedCoreArtifactSlot }
```

`CoreBootstrapAuthority`、`TrustedCoreSourceSlot`和`TrustedCoreArtifactSlot`没有public字符串/path构造器，只能由当前toolchain的trusted sysroot resolver成组产生；bootstrap constructor还逐项验证三者的source root、artifact slot、target与toolchain compatibility相等。artifact自报reserved coordinate、目录名叫`scoop.core`或用户传入普通path都不能构造它。普通分支只能携带已经解析到该slot同一regular file的`Artifact`；bootstrap分支只能携带`BootstrapSelf`且输出必须就是成组产生的artifact slot。

完整`ResolvedTargetProfile`由请求入口原子解析，driver只把精确projection下发：HIR不接target；LIR接`lir_target`；Scoop codegen接`lir_target + backend`；generated bridge producer接`lir_target + c_bridge_toolchain`。`runtime_build`与`final_link`在本阶段仅参与请求整体兼容性证明，不传给current-Cone pipeline，也不进入`.slib`，除非某个实际generated bridge section按第11.5节记录C-bridge production contract。

### 5.2 dependency参数的角色

```text
ExplicitDependencyInputs {
    direct: Vec<HostArtifactLocator>,
    support: Vec<HostArtifactLocator>,
}
```

参数顺序、basename和绝对路径不决定角色以外的任何语义。validation读取artifact自报identity后构造`ConeIdentity -> ArtifactInput`表，并按以下顺序检查：

1. 每个path是可读取regular self-contained `.slib`，完整通过container/hash/budget；
2. 对同一最终bytes分别构造声明profile的Compile与Link view；Graph-only不能进入下一步；
3. 按Cone identity去重；同identity不同artifact/semantic fingerprint失败，同一artifact重复path也作为重复输入报告而不是静默依赖argument顺序；
4. manifest分支的non-core dependency declaration与`direct`逐coordinate一一对应；single-file/core bootstrap要求`direct`为空；
5. `support`恰好是从direct dependency record递归可达、排除current/core/direct后的其余闭包；direct/support交叉、额外、缺失和不可达均失败；
6. 每条dependency record的coordinate/id/三层fingerprint与实际artifact逐项相等；
7. 拒绝self edge、cycle、同一`group:name`多version、executable dependency、single-file dependency、target/schema/ABI/profile不兼容；
8. 普通参数中出现reserved core identity一律失败；core只由独立trusted slot提供。

这些检查发生在读取当前source前。M23-3尚未提供一般`ValidatedArtifactClosure<P>`给orchestrator；内部结果命名为`ValidatedExplicitDependencyInputSet`，只证明本次调用参数闭合，不能冒充M23-4的resolved build graph或cache proof。

### 5.3 core-only阶段门禁

在上述闭包完全验证后：

- trusted core bootstrap要求闭包为空且不注入self dependency；
- manifest/single-file请求注入且只允许一个trusted core direct edge；
- 只要manifest声明任意non-core dependency，或direct/support集合非空，就返回`SCOOPC_CAPABILITY_NON_CORE_DEPENDENCY_UNAVAILABLE`；
- 诊断列出canonical coordinate和最初manifest dependency span，不解析dependency export、不执行current source parser；
- 已验证的dependency不被写入partial HIR、输出manifest或临时artifact。

这一门禁不能简化成“忽略dependency继续编译”，也不能让artifact已在内存中就成为名称候选。M23-5删除能力拒绝并接入ordinary surface时，前面的闭包检查保持不变。

### 5.4 child protocol

`scoop-protocol`冻结逻辑消息：

```text
ScoopcRequestEnvelopeV1 {
    protocol_version: 1,
    request_id: RequestCorrelationId,
    build: ScoopcBuildRequestV1,
}

ScoopcResponseEnvelopeV1 =
    Success {
        request_id,
        output_artifact_fingerprint,
        cone_identity,
        hir_fingerprint,
        mir_fingerprint,
        lir_fingerprint,
        code_fingerprint,
        runtime_image_fingerprint,
        warnings,
        emitted_dump_descriptors,
    }
  | Failure { request_id, diagnostics: NonEmpty<StructuredDiagnostic> }
```

协议不传AST/HIR/MIR/LIR arena、validated view token、open file handle或native linker参数。host path使用同机opaque path carrier并受长度预算，不要求成为UTF-8，也不进入canonical semantic encoder。M23-3提供bounded encode/decode与round-trip测试；M23-4才由`scoop`实际调度child。直接CLI与child protocol必须归一到同一个`SingleConeBuildRequest`，不能形成两套默认值。

wire固定为Wire CBOR v1。request envelope是closed product `1=magic bytes "SCOOPREQ"`, `2=protocol_version 1`, `3=request_id`（16 bytes）, `4=build`；response envelope对应为`1=magic bytes "SCOOPRES"`, `2=protocol_version 1`, `3=response`。`build`字段固定为`1=current`, `2=direct_slibs`, `3=support_slibs`, `4=trusted_core`, `5=target`, `6=out_slib`, `7=diagnostics`, `8=emit`：

- `current`为`ManifestRoot=1 {1=path}`、`SingleFile=2 {1=path}`、`TrustedCoreBootstrap=3`；
- `trusted_core`为`ArtifactSlot=1 {1=path}`或`Bootstrap=2`，且只有前两种current与前者、bootstrap current与后者的组合合法；single-file与bootstrap的direct/support必须都为空；
- `target`是closed product `1=canonical_triple`，值为1…255 bytes的printable ASCII且不含`/`或`\\`；direct CLI先解析host selection再传显式triple，child不得重新读取host默认值；
- `diagnostics`为pure tag `Human=1 | Structured=2`；`emit`为`None=1 | Stage=2 {1=kind}`，stage kind按`Ast=1, Hir=2, Mir=3, Lir=4`；
- host path carrier是`1=encoding, 2=raw bytes`，encoding为`UnixBytes=1 | WindowsWtf16Le=2`；长度为1…16,384 bytes，不允许编码对应平台的NUL，consumer拒绝非本机encoding。它只服务同机child transport，不进入semantic hash。

response sum为`Success=1`或`Failure=2`。Success字段固定为`1=request_id`, `2=artifact_fingerprint`, `3=cone_identity`, `4=hir_fingerprint`, `5=mir_fingerprint`, `6=lir_fingerprint`, `7=code_fingerprint`, `8=runtime_image_fingerprint`, `9=warnings`, `10=emitted_dump_descriptors`；所有identity/fingerprint槽精确32 bytes。Failure为`1=request_id`, `2=diagnostics`，后者非空且至少包含一条Error。structured diagnostic是`1=severity`, `2=stable_code`, `3=message`, `4=origin`, `5=notes`；severity为`Error=1 | Warning=2`，stable code匹配`[A-Z][A-Z0-9_]{0,127}`。origin封闭为`None=1`、`HostPathSpan=2 {1=path,2=start,3=end}`、`SemanticSourceSpan=3 {1=cone,2=logical_path,3=start,4=end}`或`ArtifactPath=4 {1=path,2=semantic_path}`；byte span满足`start <= end`。note固定为`1=message,2=origin`。dump descriptor固定为`1=stage,2=destination,3=content_digest`，destination为`Stdout=1 | File=2 {1=path}`。

framing不是CBOR streaming：每帧为`little_endian_u64(payload_length) || canonical_payload`，payload上限16 MiB，必须恰好包含一个完整request或response，不允许trailing/拼接帧。每个dependency list最多4096项、diagnostic/warning最多4096项、每条diagnostic最多64条note、每个success最多一个emitted dump descriptor，message/semantic path另受1 MiB leaf上限。构造器与reader执行同一组限制；reader还使用收窄的`DecodeLimits`累计限制nesting、node、owned bytes与work。magic、version、字段、tag、长度、组合或本机path encoding不符都在构造typed request/response前失败。

## 6. output kind与entry

### 6.1 输入kind与输出kind分离

manifest/single-file只提供请求kind：

```text
RequestedConeKind = Library | Executable
```

HIR完整收集声明、检查signature并选择entry后才构造：

```text
ConeOutputKind =
    Library
  | Executable { local_entry: LocalExecutableEntry }

LocalExecutableEntry {
    declaration: PersistentFunctionId,
    local_function: CurrentFunctionId,
    source_signature: ExactOrdinaryNoArgUnitSignature,
    source_signature_fingerprint: SourceSignatureFingerprint,
}
```

`Executable`没有`Option<Entry>`；`Library`也没有可访问的entry field。

### 6.2 entry规则

- 只检查当前Cone顶层ordinary source function；core/dependency、member、local、extension、property-like invoke均不参与；
- 必须non-generic、non-suspend、无receiver、无参数、返回`Unit`；
- visibility不参与entry资格；`main`不要求public，internal合法，top-level private仍保留普通名称访问域但不阻止当前Cone的entry发现；
- 0个合法entry报告missing，2个及以上按persistent source location排序报告multiple；
- 同名但signature不合法的函数只作为普通声明存在，并在missing诊断notes中列出拒绝原因；
- library完全不运行entry选择。它可以包含任意合法的`main`重载且不获得root linkage。

### 6.3 root artifact定义

对`Executable { local_entry }`，MIR/LIR非可选地产生：

- main的普通strong machine body；
- `RootEntryFailureRoot { root_cone, main }`专用strong storage；
- `CallableBodyKeyV1::RootGateway { root_cone, main_body_id }`；
- gateway自己的callable registration和全部safepoint registration；
- `ScoopRootEntryDescriptorV1`及其typed definition plan；
- manifest中的`ExecutableRootProjection`与owner member。

gateway精确为`uint32_t(void)`、成功返回0、捕获并发布未捕获Scoop异常后返回1；异常不能跨C frame。M23-3只验证object中record/relocation/definition关系，不调用gateway。library分支结构上不存在这些root-only定义；如果object或manifest私自出现root entry即Link proof失败。

## 7. `scoop.core`独立artifact

### 7.1 trusted sysroot slot

sysroot resolver返回：

```text
TrustedCoreSlot {
    source: TrustedCoreSourceSlot,
    artifact: TrustedCoreArtifactSlot,
    expected_coordinate: ConeCoordinate::CORE,
    toolchain_compatibility: CompositeIdentityAbiFingerprint,
}
```

slot的host布局是安装策略，不进入identity。M23-3 workspace adapter把source固定映射到`<sysroot>/lib/scoop.core`，artifact固定映射到`<sysroot>/artifacts/<target-profile-id>/scoop.core.slib`；这些片段只存在于driver的sysroot resolver，semantic代码不能拼接字符串。`SCOOP_SYSROOT`若保留，只选择整套受信安装根，不授权一个普通dependency path成为core。resolver先canonicalize整套sysroot、以trusted-core manifest入口验证source slot，再产生字段私有的`TrustedCoreSlot`；bootstrap可允许artifact尚不存在，普通消费则要求请求path与该slot解析为同一regular file。

trusted source manifest必须声明exact core coordinate、`kind=library`、无dependency；它本身不能写“intrinsic=true”。`IntrinsicAuthority::Core`只来自`CoreBootstrapAuthority` sidecar。bootstrap artifact也不持久化一个可被复制来伪造authority的bool。

### 7.2 bootstrap

core bootstrap执行普通manifest discovery和同一parser→HIR→MIR→LIR→object→slib pipeline，唯一差异是：

- current Cone是reserved core；
- 不注入core self edge；
- HIR可由typed authority接受core intrinsic declarations；
- 输出必须是library、Manifest source form、direct dependency table为空；
- 输出仍使用同一个`SingleConeStrongProfile`和同一双视图publish gate。

bootstrap失败时不保留或覆盖旧slot artifact。普通`scoopc`请求不会因slot缺失/stale而自行bootstrap；M23-4的trusted orchestration负责先显式发起bootstrap，再把成功artifact交给dependent。

### 7.3 core Compile capability

本阶段新增的HIR/MIR section都使用封闭`NotCore | Core`分支。writer只有持有`CoreBootstrapAuthority`，且current Cone为reserved core、source form为Manifest、output为Library、dependency table为空时，才可构造`Core`；普通writer只能构造`NotCore`，不能携带空的伪core表。raw reader只验证`Core`分支的结构与内容关系，不因看见reserved coordinate或该tag就授予authority；consumer仍须从trusted slot单独构造7.4节的`ValidatedTrustedCoreArtifact`。

`CoreHirInterfaceV1::Core`包含：

- 按persistent binding id排序的普通direct-public declaration surface；
- typed prelude binding snapshot，至少覆盖现有普通prelude function/type及`Option` variant scope；
- well-known compiler relation与`RuntimeCoreCapability::String`的source/exact identity；
- 每个binding的最终typed target、signature/parameter shape、visibility/export witness和definition origin；
- 每个target在M23-3是否可作为`ParamFreeStrong`消费的checked capability；
- exported param-free source nominal的shape-support obligation集合。

`CoreMirBridgeV1::Core`包含HIR允许target到strong exact signature subject、callable/global/constructor/accessor implementation和generated helper的typed bridge。`CoreLirBridgeV1`位于第9章strong production section，进一步给出external calling convention、effect/root-plan、persistent symbol request和param-free shape-support definition引用。

这些record由同一`ExportHir`/MIR/LIR正式投影产生，不通过扫描名字、文件顺序或旧core arena补造。`NotCore`分支编码为显式tag，不用缺section表示。

### 7.4 consumer core proof

普通request从trusted artifact slot读取core bytes，并从同一个`DecodedSlibEnvelope`独立构造：

```text
ValidatedTrustedCoreArtifact {
    compile: ValidatedCompileArtifact<SingleConeStrongProfile>,
    link: ValidatedLinkArtifact<SingleConeStrongProfile>,
    authority: TrustedCoreArtifactAuthority,
    core_interface: ValidatedCoreInterface,
}
```

构造器验证reserved coordinate、Manifest、Library、empty dependency、profile、target/ABI、`Core` section、prelude/well-known完整性、unique image及String capability relation。`authority`证明实际locator来自trusted slot；artifact内容相同但经`--direct-slib`传入仍不能构造该类型。

### 7.5 M23-3 resolver边界

HIR只从`ValidatedCoreInterface`构造：

```text
ImportedHirSet<CorePreludeOnly>
```

它只暴露prelude候选层和well-known relation，没有ordinary package/exact/star/re-export枚举API：

- 省略import时的prelude lookup可选中core typed target；
- 源码显式`import scoop.core.X`、`import scoop.core.*`与任何`public import`仍以阶段能力诊断结束；
- 选中target若不是`ParamFreeStrong`，或替换/materialization需要generic/structural/ODR能力，报告`SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE`；
- selected target只能通过同一core proof投影成`SelectedImportedMir`/`SelectedImportedLir`；
- consumer codegen只发external symbol requirement，不复制core body、TD、storage或helper；
- package/name只参与lookup与诊断，不作为external symbol或identity fallback。

M23-5把普通direct dependency surface接入同一resolver层级，M23-7开放generic core template；两者都不会改变本阶段prelude target的origin identity。

### 7.6 param-free shape-support closure

对core中每个可跨Cone引用的、type parameter count为0的source nominal exact subject，bootstrap在定义Cone预物化：

```text
ParamFreeShapeSupportClosure {
    owner: PersistentExactTypeId,
    root: ExactOwnerRoot::SourceCone(ConeIdentity::CORE),
    roles: CompleteSet<ParamFreeShapeSupportRole>,
}

ParamFreeShapeSupportRole =
    SourceNominal
  | ValueLayout
  | RefScan
  | TypeDescriptor
  | TypeRegistration
  | BoxedValue
  | CoroutineStep
  | CoroutineSlot
  | ContinuationShell
  | CoroutineStart
```

某个role在该type category语义上不产生definition时，用checked `NotApplicable { reason: ClosedReasonTag }`记录；不能用缺项表示。所有实际definition沿M23-2的`ExactOwnerRoot`回到core Cone并使用`ConeStrong`，其body/layout/scan/TD/registration/associated constant作为一个完整subject closure验证。任何role错误落到Nominal/Structural ODR root、缺definition/registration、或consumer准备重发Strong都失败。

M23-3只把该闭包作为core authority下的窄external bridge；普通dependency没有通用layout查询API。M23-6新增required layout/ABI/scan capability后，将同一obligation推广到所有可跨Cone引用的param-free exported source nominal，并提供通用consumer proof。

## 8. strong-only production profile

### 8.1 capability id

M23-3在M23-2 registry中新增：

| location | capability | `required_for` | sink |
| --- | --- | ---: | --- |
| Manifest | `org.scoop-lang.manifest/single-cone-production/1` | Link | Code, RuntimeImage, LinkValidationOnly |
| HIR | `org.scoop-lang.hir/core-bootstrap-interface/1` | Compile | Hir |
| MIR | `org.scoop-lang.mir/core-bootstrap-bridge/1` | Compile | Mir |
| LIR | `org.scoop-lang.lir/strong-production/1` | Compile\|Link | Lir, Code, RuntimeImage |
| LIR | `org.scoop-lang.lir/link-identity-closure/1` | Link | LinkValidationOnly |

`LinkValidationOnly`字段必须能从foundation、strong-production、directory/object和已经进入Code/RuntimeImage的canonical projection完整重算；任何新增、不能重算的语义必须进入Code或RuntimeImage sink，不能藏在closure section中。

artifact profile固定为：

```text
ArtifactCapabilityProfileId =
    org.scoop-lang.slib-profile/single-cone-strong/1

SingleConeStrongProfileDescriptor {
    required_manifest: [single-cone-production/1],
    required_hir: [core-bootstrap-interface/1,
                   identity-foundation/1],
    required_mir: [core-bootstrap-bridge/1,
                   identity-foundation/1],
    required_lir: [identity-foundation/1,
                   link-identity-closure/1,
                   strong-production/1],
    code_requirement: MustBeAvailable,
    runtime_requirement: MustBeAvailable,
    publication_class: Publishable,
    validation_policy: {
        odr: RejectAll,
        extra_sections: AllowPurposeDisjointOpaqueAndEnvelopeOptional,
        decode_cost_model: DeterministicLogicalCostV1,
        link_proof: Required,
    },
}
```

capability array仍按`CapabilitySortKey`的实际ASCII顺序编码；上面为可读性列出，不授权writer沿文档顺序写wire。profile fingerprint按M23-2既有domain从该descriptor计算，并在实现第一批变更中加入固定CBOR/hex golden。改变mandatory set、sink、ODR门禁或publication语义必须提升profile major；不能只改reader代码。

### 8.2 foundation不可提升

`IdentityFoundationProfile`与`SingleConeStrongProfile`没有cast、upgrade或“补一个object”的API。production writer必须重新构造全部required section、Available fingerprint和最终manifest。reader看到foundation profile时：

- 即使archive碰巧含合法Mach-O object，也不能构造Link view；
- 即使code/runtime slot被篡改为Available，也因profile不符失败；
- 即使所有persistent identity都可重算，也不能作为core或dependency；
- 只有完整single-cone profile验证才返回publishable proof。

### 8.3 distribution class

profile的`Publishable`只说明artifact通过production验证，不等于任何位置都可作为dependency。manifest production section另有封闭：

```text
ArtifactDistributionClass =
    DistributableCone       // manifest library/executable
  | LocalExecutableRoot    // single-file，仅root
```

`LocalExecutableRoot`必须与reserved single-file coordinate、SingleFile source form、Executable output、恰一source和core-only dependency同时出现。dependency input validator拒绝它；root-link validator到M23-9可接受它作为唯一root。core是`DistributableCone`，但取得intrinsic authority仍需要trusted slot proof。

## 9. 本阶段metadata payload

### 9.1 HIR section

`CoreBootstrapInterfaceSectionV1`是Wire CBOR closed product：

```text
CoreBootstrapInterfaceSectionV1 {
    core_interface: NotCore | Core(CoreHirInterfaceV1),
    output_contract: Library | ExecutableSourceEntry(PersistentFunctionId),
    direct_public_surface: CanonicalDirectPublicSurfaceV1,
}
```

它不包含import文本、failed candidate、current display locator或完整source。`ExecutableSourceEntry`必须与`ConeOutputKind`及foundation function/source identity反指一致；library不能用zero id模拟None。core public/prelude target只引用同section中完整typed declaration record或foundation persistent id。

本section进入HIR semantic fingerprint。即使普通Cone使用`NotCore`，其output contract/direct-public surface变化仍必须改变HIR fingerprint；这为M23-5升级一般public surface提供明确的失效边界，而不是靠object变化偶然触发。

### 9.2 MIR section

`CoreBootstrapBridgeSectionV1`：

```text
CoreBootstrapBridgeSectionV1 {
    core_bridge: NotCore | Core(CoreMirBridgeV1),
    entry_bridge: Library | Executable(EntryMirBridgeV1),
    strong_callable_bridges: CanonicalVec<StrongCallableBridgeV1>,
}
```

每个callable bridge连接HIR persistent declaration/generated identity、MIR `CallableSignatureSubjectV1::Strong`、exact signature与实现origin；subject必须递归回到当前Cone。Core分支完整覆盖HIR中标为`ParamFreeStrong`的prelude callable，不能多出未授权target或漏项。entry bridge只能指向当前Cone main。

本section不保存machine body、link symbol或object member；这些由LIR决定。

### 9.3 LIR strong production section

`StrongProductionSectionV1`至少包含：

```text
StrongProductionSectionV1 {
    external_bridges: CanonicalVec<StrongExternalLirBridgeV1>,
    canonical_definitions: CanonicalVec<CanonicalLirDefinition>,
    object_definition_plans: CanonicalVec<ObjectDefinitionPlan>,
    digest_finalization_plan: DigestFinalizationPlan,
    registration_plans: StrongRegistrationPlanSet,
    image_plan: ConeImagePlan,
    entry_plan: Library | Executable(ExecutableEntryPlan),
    core_shape_support: NotCore | Core(ParamFreeShapeSupportPlanSet),
    generated_bridge_plan: GeneratedBridgePlanSet,
}
```

`external_bridges`在普通Cone中只允许origin为validated core；core bootstrap中为空。每项包含typed target、expected persistent symbol、calling convention、effect/root-plan和required upstream definition identity。没有“symbol string only”分支。

`object_definition_plans`覆盖每个参与definition/digest的strong primary和associated atom，但不含member assignment。`DigestFinalizationPlan`可使用M23总设计已经冻结的全部kind enum；本profile只允许SourceSignature/Layout/Scan/LirDefinition/ObjectSupport/ObjectDefinition/StackmapRecord/StrongRegistration/RuntimeImage，出现OdrDefinition node即拒绝。

### 9.4 Link identity closure

`LinkIdentityClosureSectionV1`由packager在object verification之后构造：

```text
LinkIdentityClosureSectionV1 {
    materializations: CanonicalVec<MemberMaterialization>,
    definition_indexes: CanonicalVec<VerifiedObjectDefinitionIndex>,
    patch_sites: CanonicalVec<MaterializedPatchSite>,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSet,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSet,
    verified_link_objects: CanonicalVec<VerifiedLinkObjectSummary>,
    image_owner: VerifiedImageOwnerProjection,
    entry_owner: Library | Executable(VerifiedEntryOwnerProjection),
}
```

它不贡献新的LIR semantic bytes。reader从`StrongProductionSectionV1`、manifest directory和实际object重新计算全部字段后逐byte比较；range/offset/member id只出现在本section与Code/Artifact proof中，不进入LIR own-layer fingerprint。

### 9.5 manifest production section

`SingleConeProductionManifestV1`保存：

```text
SingleConeProductionManifestV1 {
    distribution: ArtifactDistributionClass,
    output: Library | Executable(ExecutableRootProjection),
    image_owner_member: SlibMemberId,
    runtime_registration_projection: CanonicalRuntimeRegistrationProjection,
    strong_registration_set: CanonicalStrongRegistrationFingerprintSet,
    runtime_image_fingerprint: RuntimeImageFingerprint,
    code_fingerprint: CodeFingerprint,
    native_contracts: CanonicalNativeExternalContractSet,
    native_library_requirements: CanonicalNativeLibraryRequirementSet,
    c_bridge_production: CBridgeProductionSet,
}
```

bootstrap manifest field 9中Code/RuntimeImage两槽必须为`Available`且逐byte等于本section及object中相应值。`image_owner_member`必须等于Link closure重算值。重复摘要不是第二真源；任一不一致使Link proof失败。

`ExecutableRootProjection`保存main body、source signature fingerprint、gateway body/fingerprint、failure-root identity和entry owner member。raw pointer、object-local symbol index或未来program graph不进入manifest。

### 9.6 wire字段与tag冻结

本阶段新增payload继续使用M23-2第6章冻结的Wire CBOR规则：product是以正整数field id为key的definite-length map，closed product遇到未知、重复、缺失或多余field均失败；sum是map，field `0`为正整数tag，其余field由该variant定义；array按相应typed key的canonical byte顺序排列。不得编码Rust enum discriminant、源码声明顺序、hash-map遍历顺序或host path。嵌套的既有persistent identity、foundation record与fingerprint复用其所属schema的canonical wire，不另造简写。

顶层field id固定如下：

| payload | field id与含义 |
| --- | --- |
| `CoreBootstrapInterfaceSectionV1` | `1=core_interface`, `2=output_contract`, `3=direct_public_surface` |
| `CoreBootstrapBridgeSectionV1` | `1=core_bridge`, `2=entry_bridge`, `3=strong_callable_bridges` |
| `StrongProductionSectionV1` | `1=external_bridges`, `2=canonical_definitions`, `3=object_definition_plans`, `4=digest_finalization_plan`, `5=registration_plans`, `6=image_plan`, `7=entry_plan`, `8=core_shape_support`, `9=generated_bridge_plan` |
| `LinkIdentityClosureSectionV1` | `1=materializations`, `2=definition_indexes`, `3=patch_sites`, `4=defined_symbols`, `5=undefined_symbols`, `6=verified_link_objects`, `7=image_owner`, `8=entry_owner` |
| `SingleConeProductionManifestV1` | `1=distribution`, `2=output`, `3=image_owner_member`, `4=runtime_registration_projection`, `5=strong_registration_set`, `6=runtime_image_fingerprint`, `7=code_fingerprint`, `8=native_contracts`, `9=native_library_requirements`, `10=c_bridge_production` |

本阶段共用的封闭sum tag固定为：

| sum | tag |
| --- | --- |
| core branch | `NotCore=1`, `Core=2` |
| output/entry branch | `Library=1`, `Executable=2`；带source entry的HIR variant仍使用tag 2 |
| distribution | `DistributableCone=1`, `LocalExecutableRoot=2` |
| C bridge production | `NotUsed=1`, `Used=2` |

每个`Core`、`Executable`或`Used`variant的payload字段从`1`连续编号，按该类型在本章伪代码中的字段顺序冻结；空variant只含field `0`。canonical vector不能含重复typed key；需要表达“适用但集合为空”时编码空array，需要表达语义分支时必须使用对应sum tag，两者不可互换。首批实现必须为五个顶层payload、所有空/非空sum variant、canonical排序及unknown/missing/duplicate field rejection加入固定CBOR与hex golden。

## 10. strong-only门禁

### 10.1 拒绝位置

ODR不能等到native linker前才发现。检查至少在四层重复：

1. HIR/MIR concrete output：出现`PersistentCallableApplicationId`、Nominal/DelegatedProperty/Structural specialization materialization或ODR subject时停止；
2. LIR：出现`OdrGroupId`/`OdrMemberId`实际生产记录、`CallableBodyKey::Odr`、`ConeEmissionSubject::OdrOwned`、`OdrWeak` symbol request或OdrDefinition digest node时停止；
3. object verifier：出现weak/coalesced/linkonce definition、ODR section attribute或未由strong plan解释的同名definition时停止；
4. Link reader：foundation/section/manifest/object任一层出现ODR record/body/symbol即profile失败。

identity foundation允许描述ODR key不等于production profile允许携带它。对本profile，相关table必须为空，而不是“record存在但未被object使用”。

### 10.2 不得降级的场景

下列需求稳定报告`SCOOPC_CAPABILITY_ODR_UNAVAILABLE`：

- current Cone实际调用generic function或实例化generic nominal；
- concrete generic body中的closure/coroutine/callback/adapter；
- non-nominal exact type需要materialized TD/layout；
- `ExactOwnerRoot`得到Nominal或Structural group；
- future generic delegated extension application；
- 任意本应coalesce的shape helper，即使当前请求中只有一个producer。

不得为了让core-only smoke通过而把它们临时标成ConeStrong。诊断锚定触发materialization的source use，并显示template/origin与缺失能力；纯artifact输入中的非法ODR则使用artifact/section/typed key路径。

### 10.3 允许的strong subject

成功definition的owner必须唯一回溯为：

- 当前Cone的param-free source declaration；
- 当前Cone的param-free source-anchored generated descendant；
- M23-2 `ExactOwnerRoot`明确回到当前Cone的param-free shape helper；
- current Cone root/init gateway；
- current Cone实际使用的generated bridge atom；
- per-Cone image及其associated tables/sentinels/boundaries。

全部使用`ConeStrong`。`TemplateSupportHidden`在M23-7前同样拒绝；private/internal不是取得hidden linkage的证明。

## 11. object production与验证

### 11.1 producer输出

Scoop codegen和generated C producer都返回：

```text
ProvisionalLinkObjectMember {
    capability: ScoopLirV1 | GeneratedCBridgeV1,
    logical_key: CanonicalObjectLogicalKey,
    units: NonEmptyCanonicalSet<TypedUnitId>,
    backing: ImmutableTemporaryObject,
    materializations: NonEmptyCanonicalVec<ProvisionalMaterialization>,
}
```

`logical_key`与unit-set digest严格使用M23-2第8.3节冻结的公式。一个producer可以输出一个或多个object，一个object可以含一个或多个unit；空unit object、固定`code.o`/`bridge.o`角色和“第一个object拥有image”都非法。

generated bridge absent时没有generated object member，但manifest的`CBridgeProductionSet::NotUsed`仍是显式variant。存在bridge时每个实际unit在当前Cone恰有一个`PrimaryEntry(unit)` atom；同unit不得跨多个member重复生产。

### 11.2 member id在finalization前确定

`SlibMemberId`只依赖Cone identity与stable key，不依赖payload hash。packager因此先：

1. 验证每个producer unit set canonical、完整且互不重叠；
2. 从capability/logical key计算stable key与member id；
3. 建立`plan/unit -> SlibMemberId`的唯一assignment；
4. 才运行object verifier与digest patch；
5. 最后以final bytes计算member hash/record。

这样typed patch site可带member id，同时不形成member hash自引用。改变object分片会改变member id和Code/Artifact fingerprint，但只要canonical LIR语义不变就不改变LIR semantic fingerprint。

### 11.3 共同Mach-O门禁

当前target的两个verifier都至少验证：

- 64-bit little-endian AArch64 Mach-O，file type精确为`MH_OBJECT`；
- CPU subtype、deployment、relocation模型和backend/C-bridge profile匹配；
- header、load command、section、symbol/string/relocation table全部有界且无overlap/overflow；
- capability允许的section/flags/relocation/symbol kind closed allowlist；
- 明确拒绝`LC_LINKER_OPTION`、autolink directive、embedded linker option/script、dylib load command、unexpected constructor/destructor与target不允许的debug/linkedit输入；
- `MH_SUBSECTIONS_VIA_SYMBOLS`与每个受检atom的stable start/end boundary；
- boundary同section、有序、不重叠，padding为canonical zero并归前一atom；
- 每个受检relocation完全落入一个typed atom，width/kind/addend/target满足plan；
- 所有linker-visibledefinition和undefined symbol use都有唯一typed解释；
- provisional digest slot初始全零，非patch byte与canonical plan相符。

精确allowlist按`ObjectFormatId + verifier capability + target/backend或C-bridge production contract`登记并由golden冻结。实现不能写一个“object crate能parse就接受”的generic fallback。

### 11.4 Scoop LIR verifier

`org.scoop-lang.link-object/scoop-lir/1` verifier从LIR plan独立重建：

- 每个persistent symbol request的normalized Mach-O symbol；
- strong callable/storage/immortal/layout/scan/TD/dispatch/init/registration/image/entry atom；
- object support atom、definition range、associated diagnostic/template/relocation table；
- statepoint function/stackmap contribution及site owner；
- 所有typed relocation与undefined requirement；
- 每个digest intent的32-byte zero slot。

object中多一个未计划的managed entry、registration、image descriptor、Scoop-mangled external/weak definition或受检relocation都失败。普通local machine support也必须由某个plan的associated closure认领；不能因symbol local就跳过。

### 11.5 generated C bridge verifier

`org.scoop-lang.link-object/generated-c-bridge/1` verifier接收canonical `GeneratedBridgePlanSet`与：

```text
CBridgeProductionSet =
    NotUsed
  | Used {
        profile_id: CBridgeToolchainProfileId,
        profile_fingerprint: CBridgeToolchainFingerprint,
        source_template_fingerprint: GeneratedCSourceTemplateFingerprint,
        canonical_flag_fingerprint: CanonicalCBridgeFlagFingerprint,
        units: NonEmptyCanonicalSet<GeneratedBridgeUnitId>,
    }
```

profile由请求级`ValidatedCBridgeToolchainProfile`生成，至少承诺target triple、SDK/deployment、compiler identity、canonical flags、generated source template和被允许的environment投影；host compiler path与临时source path不进入contract。改变任一契约字段必须改变profile fingerprint并进入Code fingerprint。

verifier检查每个unit的producer-specific `GeneratedBridgeAtomId`、primary entry、signature/context descriptor与actual native symbol/relocation；LIR/ODR canonical target仍只保存producer-independent unit。`StaticAssertSupport`只由canonical source/template proof承诺，不得在object中伪造atom、symbol或definition range。

generated object中的source extern、runtime callback/EH或其他native use仍产生typed requirement。编译器输出的额外全局、constructor、destructor、autolink或未计划helper失败；不能把“来自受信clang”当作跳过object检查的理由。

### 11.6 finalization顺序

固定流程：

```text
Frozen LIR semantic projection
  -> provisional object(s) + materialization
  -> stable member ids
  -> provisional object verification
  -> semantic/object/stackmap leaf normalization
  -> strong registration fingerprints
  -> runtime image fingerprint
  -> typed patch writes
  -> full final object re-verification
  -> immutable VerifiedLinkObjectMember set
  -> Link closure section + manifest production section
  -> Code/Artifact fingerprint
  -> deterministic ar
```

patch writer只能改`MaterializedPatchSite`声明的固定32 bytes，每个intent恰一个writer；一个node写多个等价slot时全部slot必须相同。final verification重新检查range、relocation、owner、requirement、digest和image，不信任provisional结果。pack完成后任何member不可再修改。

## 12. definition与undefined requirement

### 12.1 member-aware owner

```text
DefinedLinkSymbolOwner {
    member: SlibMemberId,
    symbol: NormalizedNativeLinkSymbol,
    owner: LinkDefinitionOwner,
}

LinkDefinitionOwner =
    StrongDefinition(StrongDefinitionOwner)
  | GeneratedBridge(GeneratedBridgeAtomId)
  | ConeImage(ConeIdentity)
  | VerifierBoundary(ObjectDefinitionAtomId, BoundaryRole)
```

`StrongDefinitionOwner`是M23-2已冻结kind-specific owner的封闭sum，覆盖callable、storage、immortal、layout、scan、TD、dispatch、init cell、registration、entry及associated atom；不能退化为`PersistentEntityId`裸bytes或symbol string。每个owner只允许一个primary symbol；boundary不能被final relocation当作普通target。

全部definition set按`(normalized symbol bytes, member id, owner canonical key)`排序。strong primary symbol全artifact唯一；同名不同owner、同owner多primary或manifest owner与object不一致均失败。

### 12.2 undefined use

```text
UndefinedSymbolRequirement {
    member: SlibMemberId,
    use_site: VerifiedRelocationUse,
    symbol: NormalizedNativeLinkSymbol,
    requirement: FinalUndefinedSymbolRequirement,
}

FinalUndefinedSymbolRequirement =
    IntraConeStrong { owner: StrongDefinitionOwner }
  | CoreStrong { core: ConeIdentity, owner: StrongDefinitionOwner }
  | GeneratedBridge { unit: GeneratedBridgeUnitId }
  | SourceExtern { contract: NativeExternalContractFingerprint,
                   library_requirement: NativeLinkRequirementId }
  | RuntimeAbi { contract: RuntimeSymbolContractId }
  | TargetEhSupport { contract: TargetEhRequirementId }
```

`VerifiedRelocationUse`保存member、typed containing atom、section role、checked offset、width、relocation kind和canonical addend；它不是裸object-local ordinal。真实symbol bytes必须由requirement的typed key和target normalization重算一致。

### 12.3 本阶段解析范围

- `IntraConeStrong`必须在当前artifact内解析到唯一owner，可跨member；
- `GeneratedBridge`必须解析到当前producer的唯一`PrimaryEntry(unit)`；
- `CoreStrong`在普通Cone中必须能由validated core Link/Compile bridge找到相同owner/symbol，当前artifact仍保留external requirement；
- `SourceExtern`只验证完整contract、library requirement及object use一致，不搜索host provider；
- runtime/EH requirement只验证为当前target/runtime ABI的known contract，不构建runtime；
- M23-9/M23-10在完整closure/final input上解析后四类实际provider。

任何undefined symbol没有requirement、一个use命中多个requirement、requirement symbol与object不符或外部contract只剩library名字都使Link proof失败。root Cone没有直接使用某个上游extern也不能在以后丢弃该requirement。

## 13. 六类registration与image producer

### 13.1 共享ABI

M23总设计第6.1节列出的`ScoopDescriptorPrefixV1`、六类registration、`ScoopRootEntryDescriptorV1`与`ScoopImageDescriptorV1`字段顺序、magic、tag、size/alignment和empty-span sentinel规则在本阶段成为compiler-side冻结ABI。实现新增唯一共享header`runtime/include/scoop_runtime_metadata_v1.h`；C runtime和Rust/LLVM mirror都从该文件的常量/golden验证，不能各自维护不同数字。

M23-3不发射`ScoopProgramDescriptorV1`或`ScoopRuntimeCoreBindingsV1`实例；header可以声明最终v1形状，实际builder/consumer分别到M23-8/M23-9实现。M24 release hook字段绝不能提前混入M23 schema 1。

### 13.2 strong registration identity

每张表的semantic id类型固定为：

| table | semantic id |
| --- | --- |
| static storage | `PersistentStaticStorageId` |
| immortal object | `PersistentImmortalObjectId` |
| initialization unit | `PersistentInitializationUnitId` |
| type registration | `PersistentExactTypeId` |
| safepoint registration | `PersistentSafepointSiteId` |
| callable registration | `PersistentCallableBodyId` |

本profile全部`linkage_kind=Strong`，ODR group/member 64 bytes全零。`definition_fingerprint`由相应StrongRegistration node写入；callable的`body_definition_fingerprint`独立取body atom的ObjectDefinition fingerprint。六张表分别按总设计canonical key严格排序，即使为空也在image/hash中保留count 0，并在object中使用typed addressable sentinel。

### 13.3 registration完备性

- 每个compiler-owned static storage恰一条storage registration；GC-free storage也不能为省事丢descriptor；
- 每个只读immortal object恰一条immortal registration并引用当前/允许external type registration；
- 每个M21 initialization unit恰一条init registration，cell/storage/failure root/initializer/ensure及Eager gateway完整；
- 每个runtime-materialized exact type恰一条type registration，TD/runtime type/layout/descriptor fingerprint一致；
- 每个LIR safepoint site恰一条safepoint registration并由normalized LLVM v3 record反证；
- 每个`RegisteredCallableBody`恰一条callable registration，即使没有safepoint；native extern/runtime函数不因有地址而混入；
- root gateway、initialization startup gateway和compiler adapter只要是Scoop machine body，同样登记callable/safepoint；
- 所有record由实际producer member拥有；普通external reference不在consumer image重复登记。

### 13.4 image plan

```text
ConeImagePlan {
    cone: ConeRecordV1,
    dependencies: CanonicalVec<ConeIdentity>,
    tables: {
        static_storages,
        immortal_objects,
        initialization_units,
        type_registrations,
        safepoints,
        callables,
    },
    symbol: PersistentV1::Image(ConeIdentity),
    definition_plan: ObjectDefinitionPlanId,
    fingerprint_patch: DigestPatchIntentId,
}
```

core bootstrap的dependencies为空；普通manifest/single-file当前恰为`[core]`。它们按identity bytes编码，不从manifest声明顺序取得。image symbol hidden strong且只能由一个member定义。image中的pointer table只列当前Cone实际生产的strong record；外部core record不复制进来。

全部LinkObject联合验证后必须：

- image definition恰好一个；
- manifest `image_owner_member`指向该member；
- 其余member没有任何当前或其他Cone image descriptor；
- coordinate bytes/identity/dependency table/六张pointer table与plan逐项一致；
- 每个pointer relocation命中正确registration record symbol；
- runtime image slot是唯一32-byte patch并与manifest Available值相等。

## 14. digest与fingerprint

### 14.1 leaf与DAG

沿用总设计的`DigestFinalizationPlan`和单writer规则。M23-3至少实际计算：

- source signature、layout、scan、canonical LIR definition；
- object support与object definition；
- normalized stackmap record；
- strong registration；
- runtime image；
- composite Code与Artifact fingerprint。

每个node只观察声明的typed direct input；自身、peer、descendant及无关patch slot按规范归零。合法topological traversal选择不能改变结果。环、缺边、unknown kind、双writer、无site、错member/offset、非零provisional slot或写入宽度不为32都失败。

### 14.2 strong registration fingerprint

算法逐byte采用总设计：

```text
StrongRegistrationFingerprint =
    SHA-256(
        ByteSpan("scoop-strong-registration-v1") ||
        RuntimeEncode(StrongRegistrationFingerprintInputV1)
    )

StrongRegistrationFingerprintInputV1 {
    record: CanonicalStrongRecordSansOwnDefinition,
    direct_inputs: Sequence<StrongRegistrationDirectInputV1>,
}
```

`record`是完整`RuntimeImageRecordKey` variant，linkage固定Strong、ODR字段全零，只把当前registration自己的`definition_fingerprint`槽写32个零；其他已经声明为direct dependency的layout/scan/descriptor/gateway/callable/stackmap digest保持最终值。不能在variant前再编码第二份record-kind tag，也不能把全部digest一律归零。

direct input按`(kind, DigestNodeId)`严格排序，保存kind、node id和digest。finalizer、reader都从canonical record/LIR plan/verified object独立重建；manifest只保存结果与定位关系。

### 14.3 runtime image fingerprint

使用总设计`RuntimeEncode`和固定tag：

```text
RuntimeImageFingerprint = SHA-256(
    ByteSpan("scoop-runtime-image-v1") ||
    RuntimeAbiFingerprint ||
    TargetProfileFingerprint ||
    RuntimeEncode(ConeRecordV1) ||
    RuntimeEncode(sorted direct dependency identities) ||
    RuntimeEncode(each of six separately counted/sorted tables)
)
```

实际encoder按总设计6.1声明序写字段，不能以Wire CBOR或上面排版伪代码替代。raw pointer、ASLR地址、member bytes、archive ordinal、root/core program binding和runtime image自身slot不进入hash；typed role/identity、record definition、initial state/template/relocation和normalized stackmap digest进入。

compiler finalizer写image slot；`BootstrapManifestV1` field 9中的runtime-image槽和production section保存同值；Link reader从object/metadata重算三方相等。M23-8 runtime在最终地址空间再重算同一值，不改变算法。

### 14.4 CodeFingerprint

packager只有在全部Link-purpose输入就绪后计算：

- 按`SlibMemberId`排序的每个`LinkObject`的`LinkMemberFingerprint`；
- known Link-required extension及handler的canonical native library requirement；本阶段没有内建extension handler，集合通常为空；
- `CBridgeProductionSet` canonical projection；
- `CanonicalDefinedLinkSymbolOwnerSet`；
- `CanonicalUndefinedSymbolRequirementSet`；
- `CanonicalNativeExternalContractSet`与target-tagged native library requirements；
- strong/image verification surface中registry指定进入Code sink的canonical projection。

domain固定为`scoop-code-v1`。optional blob、diagnostic attachment、physical ar name、host path和producer临时目录不进入。只改object分片会改变member/Code fingerprint但不改变LIR semantic fingerprint；只改optional附件只改变Artifact fingerprint。

### 14.5 fingerprint可用性

`SemanticFingerprintRecordV1`在本profile中：

- HIR/MIR/LIR均为既有non-optional typed digest；
- Code必须`Available(CodeFingerprint)`；
- RuntimeImage必须`Available(RuntimeImageFingerprint)`。

禁止使用全零digest代表“稍后计算”，也禁止在profile为Available时留`Unavailable`。`GraphFingerprint`仍只属于最终program，不进入`.slib`。

## 15. Link reader与双视图发布

### 15.1 Link proof顺序

`validate_link::<SingleConeStrongProfile>`固定按以下依赖顺序：

1. 接收已通过的`DecodedSlibEnvelope`和Graph proof；
2. 验证profile descriptor、Available fingerprint、mandatory Link/Compile|Link section inventory；
3. 解码LIR strong-production/link-closure与manifest production payload；
4. 重放`RejectAll`并拒绝全部ODR identity/linkage/symbol；
5. 为每个`LinkObject`按capability调用known verifier；unknown capability fail closed；
6. 重建unit/member assignment、definition range、relocation、patch和stackmap leaf；
7. 验证defined owner、undefined requirement及intra-Cone/generated bridge闭合；
8. 验证六类registration、strong fingerprint、table排序与entry分支；
9. 验证唯一image owner并重算RuntimeImage fingerprint；
10. 重算Code fingerprint、member/artifact fingerprint交叉关系；
11. 原子构造immutable `ValidatedLinkArtifact<SingleConeStrongProfile>`。

Link验证不要求先构造Compile view，也不借用Compile已commit的session-local arena。两条proof可以共享同一raw envelope/hash bytes，但必须分别解码/验证自己需要的semantic surface。这样link-only consumer无需把HIR导入semantic world。

### 15.2 Link API

```text
ValidatedLinkArtifact<SingleConeStrongProfile> {
    graph: ValidatedGraphArtifact,
    link_objects: NonEmptyCanonicalVec<VerifiedLinkObject>,
    image_owner: VerifiedImageOwner,
    output: ValidatedLibraryArtifact
          | ValidatedExecutableArtifact,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSet,
    undefined_requirements: CanonicalUndefinedSymbolRequirementSet,
    native_contracts: CanonicalNativeExternalContractSet,
    code_fingerprint: CodeFingerprint,
    runtime_image_fingerprint: RuntimeImageFingerprint,
}
```

object bytes只能从Link view以`VerifiedLinkObject`访问；Graph/Compile view只保留hash-valid opaque envelope。API不暴露“第一个object”“code object”“bridge object”字段。

### 15.3 publish gate

writer在同一输出目录创建private temporary file，写完整deterministic archive并关闭写端；随后重新以只读方式打开最终bytes，分别构造：

```text
ValidatedCompileArtifact<SingleConeStrongProfile>
ValidatedLinkArtifact<SingleConeStrongProfile>
```

再构造：

```text
PublishableSingleConeArtifact {
    artifact_fingerprint,
    cone,
    compile_summary,
    link_summary,
}
```

构造器要求两份proof来自同一ArtifactFingerprint/Cone/profile，output kind、三层/Code/RuntimeImage fingerprint一致。只有成功后才以同目录atomic rename发布`--out-slib`。任一步失败删除temporary，不覆盖现有成功artifact。

output destination不能与current manifest/source、trusted core input或任一dependency artifact解析为同一文件；检查失败早于写入。host I/O error不伪造成source diagnostic。

### 15.4 core input门

普通Cone不仅需要core Compile view用于semantic bridge，也保留同一core Link view以证明其确为可发布dependency。若core只通过Compile、Link object损坏、image不唯一、profile foundation或target不匹配，必须在parse current source前失败。不能因为当前compile“暂时只用名字”而接受不可链接core。

## 16. `scoopc` CLI边界

### 16.1 正式命令

M23-3正式CLI为：

```text
scoopc build <Cone-root-or-Cone.toml-or-file.scoop>
    --direct-slib <direct.slib>...
    --support-slib <support.slib>...
    --out-slib <current.slib>
    [--emit ast|hir|mir|lir]
```

target/sysroot选择沿现有toolchain配置，但解析成typed request后不再允许stage读取环境补默认。`-L/--library-path`从正式`scoopc build`删除；native provider search只属于未来`scoop/program-link`。`-o`若保留兼容提示，只能明确报新语法，不得继续生成binary。

### 16.2 操作数分流

- basename精确为`Cone.toml`的regular file选择manifest；
- directory选择其直接`Cone.toml`；
- basename扩展名精确为`.scoop`且最终为regular file选择single-file；
- 其他文件、missing path、wrong type、dangling/cyclic symlink稳定失败；
- 显式`.scoop`文件优先，永不因邻近manifest切换模式。

这只是single-Cone输入解析，不做M23-4 locator或M23-11用户级build lifecycle。

### 16.3 成功与失败

成功只意味着`.slib`已经双视图验证并原子发布。`--emit`输出本次current Cone stage dump，不改变artifact bytes/fingerprint；warning写stderr或structured response，不能污染dump/stdout协议。命令不产生runtime archive、program descriptor、executable或执行程序。

## 17. 诊断与失败原子性

### 17.1 错误阶段

稳定前置顺序：

```text
request/protocol
  -> current manifest/single-file shape
  -> explicit dependency/core artifact closure
  -> M23-3 dependency capability gate
  -> source discovery/read
  -> parser
  -> HIR/source semantics + entry
  -> ODR/materialization gate
  -> MIR/LIR structural validation
  -> producer/toolchain
  -> object/fingerprint/image verification
  -> pack round-trip Compile/Link
  -> atomic publish
```

后一步不得在前一步失败后启动。独立检查可并行，但公开primary error按canonical Cone/source/member/section/path/code排序；worker完成顺序不参与。

### 17.2 error code family

新增至少：

- manifest：`MANIFEST_IO`、`MANIFEST_TOML`、`MANIFEST_SCHEMA`、`MANIFEST_UNKNOWN_FIELD`、`MANIFEST_COORDINATE`、`MANIFEST_DEPENDENCY`、`MANIFEST_RESERVED_COORDINATE`；
- discovery：`SOURCE_DISCOVERY_MISSING_ROOT`、`SOURCE_DISCOVERY_EMPTY`、`SOURCE_DISCOVERY_NON_UTF8_PATH`、`SOURCE_DISCOVERY_SYMLINK_ESCAPE`、`SOURCE_DISCOVERY_SYMLINK_CYCLE`、`SOURCE_DISCOVERY_DUPLICATE_IDENTITY`、`SOURCE_DISCOVERY_IO`；
- request/dependency：`SCOOPC_INPUT_KIND`、`SCOOPC_INPUT_DUPLICATE_ARTIFACT`、`SCOOPC_INPUT_DIRECT_MISMATCH`、`SCOOPC_INPUT_SUPPORT_MISMATCH`、`SCOOPC_INPUT_STALE_DEPENDENCY`、`SCOOPC_INPUT_CORE_AUTHORITY`、`SCOOPC_INPUT_EXECUTABLE_DEPENDENCY`；
- capability：`SCOOPC_CAPABILITY_NON_CORE_DEPENDENCY_UNAVAILABLE`、`SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE`、`SCOOPC_CAPABILITY_ODR_UNAVAILABLE`；
- entry：`CONE_ENTRY_MISSING`、`CONE_ENTRY_MULTIPLE`、`CONE_ENTRY_INVALID_SIGNATURE`；
- Link：`SLIB_LINK_OBJECT_CAPABILITY`、`SLIB_LINK_OBJECT_FORMAT`、`SLIB_LINK_OBJECT_AUTOLINK`、`SLIB_LINK_MATERIALIZATION`、`SLIB_LINK_DEFINITION`、`SLIB_LINK_UNDEFINED_REQUIREMENT`、`SLIB_LINK_PATCH`、`SLIB_LINK_DIGEST`、`SLIB_LINK_REGISTRATION`、`SLIB_LINK_IMAGE`、`SLIB_LINK_ENTRY`、`SLIB_LINK_ODR_FORBIDDEN`；
- publish：`SLIB_PUBLISH_VIEW_MISMATCH`、`SLIB_PUBLISH_OUTPUT_ALIAS`、`SLIB_PUBLISH_IO`。

slib reader错误继续使用M23-2的typed `WirePath`、member/capability/typed key origin。source诊断使用canonical semantic source name排序，CLI最后才装饰display locator。untrusted manifest/artifact text只作为escaped argument。

### 17.3 原子性

- dependency/reader失败不commit imported world；
- 任一source parser失败不产生`CurrentConeParsedSources`；
- HIR/entry/ODR失败不进入MIR；
- object/finalizer失败不产生`VerifiedLinkObjectMember`；
- Link round-trip失败不产生publishable proof；
- publish前失败不改变已有output；
- error path不留下可被后续request误识别为core/cache/dependency的partial `.slib`。

## 18. 测试设计

### 18.1 manifest与discovery

- valid library/executable、empty/多dependency、string/inline-table locator；
- schema/type/unknown/duplicate/missing field、noncanonical SemVer、invalid group/name、reserved coordinate/core dependency；
- dependency枚举与TOML table顺序不改变semantic projection；locator spelling只改变sidecar；
- nested `src`、UTF-8 path与byte排序、empty set、wrong extension/case/nonregular entry；
- symlink file/directory留在root、escape、dangling、cycle、depth边界；
- 同real file不同logical path保持不同identity，normalized duplicate稳定失败；
- 两个隔离absolute root得到相同source/entity/artifact bytes。

### 18.2 request与闭包

用small recording artifact覆盖：

- direct/support顺序置换等价；
- missing/extra/cross-role/duplicate identity、same identity different artifact、stale edge、cycle/self/multiversion；
- executable/single-file artifact作为dependency拒绝；
- 普通path伪造core、trusted slot内错coordinate/profile/target/image拒绝；
- 非coredependency先通过完整双视图验证，再稳定命中阶段能力诊断且current parser未启动；
- single-file带任一direct/support参数拒绝；
- core bootstrap不产生self edge。

### 18.3 core分离

- trusted core source独立产生Library `.slib`，无entry、dependency为空、image唯一；
- 普通core-only library/executable只从core artifact取得prelude/well-known/strong bridge；
- 删除旧core+user AST拼接后，param-free prelude smoke的AST/HIR/MIR/LIR identity与预期一致；
- 显式core exact/star/public import仍稳定拒绝；
- generic core target、generic local application和structural TD materialization命中对应capability门，不生成partial LIR；
- consumer对core param-free target只产生external requirement，object内没有第二份body/TD/registration；
- 每个exported param-free core nominal的有限shape-support role完整，缺项/错root/错linkage被Compile或Link proof拒绝；
- core source/artifact不兼容时普通`scoopc`只失败，不读取core source或自行重建。

### 18.4 kind与entry

- library无main、一个/多个合法main都成功且无root metadata；
- executable missing、multiple及每种invalid signature有negative fixture和精确span/note；
- 不同package各一合法main仍报告multiple；
- dependency/core main不参与；
- executable成功时main、root failure storage、gateway、callable/safepoint registration和entry descriptor全部互证；
- library伪造entry或executable缺任一root relation使Link view失败。

### 18.5 profile与wire

- `SingleConeStrongProfileDescriptor` canonical CBOR与fingerprint fixed vector；
- 五条新增capability的location/required_for/sink矩阵；
- mandatory section缺失、重复、错purpose、错location、unknown required稳定失败；
- foundation artifact无法提升，Available/Unavailable、publication、RejectAll/Required任一篡改失败；
- `NotCore/Core`、Library/Executable、Distributable/LocalRoot各variant round-trip；
- single-file distribution与ConeRecord/source table组合逐项篡改失败；
- Compile/Link view类型编译期不可互换，Graph/Compile无object API。

### 18.6 object verifier

- 一个/多个Scoop object、零/一个/多个generated bridge object及混合member排序；
- logical unit集合1、128与预算边界，重复/漏unit/跨member重用；
- 任意物理basename/无`.o`后缀的LinkObject仍验证；同bytes登记为opaque blob绝不进入Link；
- non-Mach-O、wrong arch/filetype/profile、truncated table、overlap/overflow；
- `LC_LINKER_OPTION`、autolink、constructor/destructor、unexpected section/symbol/relocation；
- boundary错section/顺序/重叠、nonzero padding、orphan relocation；
- weak/linkonce/ODR symbol与未认领strong definition；
- generated bridge缺/multiple primary、跨producer atom、signature/context/contract错配、伪StaticAssert atom；
- provisional patch非零、错width/offset/member、双writer；final patch后再次验证成功。

### 18.7 definition与requirement

- definition/undefined use跨多个member仍携带正确member id；
- intra-Cone requirement解析到另一member；
- core external、generated bridge、SourceExtern、runtime/EH requirement各variant；
- 无requirement、多个requirement、symbol normalization错、contract/library relation错；
- root没有直接extern但上游object requirement仍保留；
- 同名不同owner、同owner双primary、boundary被普通relocation引用均拒绝。

### 18.8 registration、digest与image

- 六张table分别为空/非空并保留独立count/sentinel；
- 每种record的semantic id/linkage/zero ODR field/definition/body digest；
- initialization Eager/Lazy、专用cell/storage/failure root/gateway矩阵；
- callable无safepoint仍登记、native extern不误登记；
- stackmap Constant/ConstantIndex正规化、owner/root count/record缺失；
- strong fingerprint逐字段变化、只归零own definition、direct input排序；
- digest DAG不同合法topological traversal同值，cycle/缺边/双writer拒绝；
- runtime image的empty/nonempty dependency与六表golden，raw pointer/member分片变化不改变canonical image输入；
- zero/multiple image、wrong owner member、dependency/table pointer relocation错、image slot/manifest值错；
- Code fingerprint对object/owner/requirement/contract/C-bridge profile变化敏感，对diagnostic/optional blob不敏感；
- Rust encoder与共享C header的`sizeof/alignof/offsetof`静态断言逐字段一致。M23-8加入C runtime encoder时复用同一golden，不重新定tag。

### 18.9 end-to-end与可复现性

至少提供：

- trusted core bootstrap；
- core-only manifest library；
- core-only manifest executable；
- core-only single-file executable；
- 含String literal、param-free prelude call、exception/root gateway、global/init、closure/callback中不触发ODR的组合fixture；
- SourceExtern只记录requirement而不尝试link的fixture；
- 同输入在不同absolute checkout/temp目录、不同source/member输入顺序下逐byte相同；
- writer输出重新读取后Compile/Link均成功，任一metadata/object/digest/owner/requirement bit flip至少使一条view稳定失败；
- 正式`scoopc build`只产生`.slib`，没有runtime build、native linker或binary副作用；
- 全部现有unit/golden继续回归，legacy executable fixture只经隔离入口运行。

## 19. 实现顺序

1. 新增`compiler/manifest`，完成strict manifest DTO、locator/semantic分离、source discovery与negative测试；
2. 新增`compiler/protocol`和`SingleConeBuildRequest`，固定request/response/framing并接入`scoopc`参数归一化；
3. 实现trusted core slot/bootstrap authority、`ValidatedTrustedCoreArtifact`与core-only dependency input门；
4. 用`CurrentConeParsedSources`替换正式`LegacyCombinedSources`，完成output kind/entry选择及core prelude-only Imported HIR；
5. 在HIR/MIR/LIR crate增加三层本阶段canonical/decoded/validated/imported section和strong/entry/core bridge；
6. 在LIR形成strong definition、registration、image、entry与digest plan，并加入四层ODR拒绝；
7. 生成共享runtime metadata v1 header，改造codegen输出per-Cone image、六表、root descriptor、boundary与zero patch；
8. 实现Scoop LIR/generated C两种LinkObject verifier、member-awareowner/requirement及C-bridge production proof；
9. 实现digest finalizer、final re-verification、Code/RuntimeImage fingerprint和Link closure/manifest section；
10. 注册`SingleConeStrongProfile`并实现`ValidatedLinkArtifact`、双视图publish gate及atomic output；
11. 切换正式`scoopc build`只产`.slib`，移除其runtime/link依赖，把旧运行fixture入口隔离命名；
12. 补齐core bootstrap、library/executable/single-file、corruption、reproducibility与组合fixture。

每完成一批代码变更，先执行`cargo fmt --all`与`cargo clippy --workspace`，再运行该批unit/golden/fixture；不能把format/lint集中拖到全部实现之后。

## 20. 完成门

M23-3只有同时满足以下条件才完成：

- `Cone.toml` semantic projection、source discovery和single-file固定projection全部有strict、确定性实现；
- `scoopc`正式入口每次只编译一个Cone，只消费显式artifact/trusted core，只输出`.slib`，不搜索、递归、构建runtime或最终链接；
- core由trusted authority独立生成library artifact，普通Cone的AST/HIR中不再混入core source，prelude/well-known/strong bridge只来自已验证core metadata；
- 非coredependency、generic/structural/ODR能力在本阶段稳定拒绝且没有残缺IR/artifact；
- core bootstrap、core-only library、core-only executable和single-file都能产生逐byte可重复的`SingleConeStrongProfile` artifact；
- 每个成功artifact都至少含一个verified LinkObject，全部object联合恰有一个当前Cone image，并完整证明六类strong registration、entry分支、definition、undefined requirement、patch、Strong/RuntimeImage/Code fingerprint；
- 任意ODR group/member/body/linkage/symbol都无法取得production Compile/Link proof；
- core param-free exported source nominal的有限shape-support closure由core定义并完整物化，consumer只产生external typed requirement；
- writer只有在最终bytes分别通过Compile和Link验证后才原子发布；foundation、Graph-only、Compile-only、损坏object或错image均不可发布/消费；
- 成员语义不依赖文件名、扩展名、ordinal或固定object数量，opaque blob绝不被提升为object；
- library不需要main且没有root metadata，executable entry/root gateway/failure root非可选且互证；
- 新增unit/golden/negative/组合fixture、`cargo fmt --all`、`cargo clippy --workspace`和完整`cargo test`全部通过；
- legacy executable runner只保留为明确隔离的历史回归入口，任何新component都不能调用它。

到达该完成门后，M23-4可以只把`.slib`视为不透明、已双视图验证的节点来做DAG与调度；它不需要也不允许进入任何Cone的parser、IR或object producer内部。

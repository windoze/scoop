# M23-4 设计：resolved build graph 与调度

版本：1.0（实现完成；2026-09-16）

依赖：M23-3

上位设计：`docs/milestone23/DESIGN.md`

规范依据：

- `docs/specs/SCOOP-SPEC.md` 第 12.1～12.3、12.5～12.6 节；
- `docs/specs/SCOOP-IMPL-SPEC.md` 第 2.6～2.7、2.11 节；
- `docs/specs/SCOOP-RUNTIME-SPEC.md` 第 2.8 节中关于 canonical Cone order 的契约；
- `docs/milestone23/DESIGN.md` 第 0、1、4.5～4.6、5.3～5.5、8～10 章；
- `docs/milestone23/stage2/DESIGN.md` 与 `docs/milestone23/stage3/DESIGN.md` 已冻结的 identity、container、reader、single-Cone request、child protocol、strong artifact 与双视图发布契约。

本文只定义 M23-4。语言名称语义、runtime 与最终链接仍以上述规范和 M23 总设计为准；本文负责把“如何从一个 root 精确解析整张 Cone 图，并只通过 `.slib` 边界确定性地调度 `scoopc`”闭合成可实现、可验证的构建系统。M23-4 不修改源码可见性：普通 dependency 即使已经被定位、验证和传给 child，也要继续在 M23-3 的 HIR 能力门稳定失败，直到 M23-5 原子开放跨 Cone 名称语义。

## 0. 结论

M23-4 第一次建立多 Cone 的**构建图**，但不建立多 Cone 的**语言世界**。完成本阶段后：

1. `compiler/scoop` 提供独立于 `scoopc` implementation crate 的 orchestration library；它解析 root、locator、trusted core、静态 exact DAG、cache 与 child 生命周期，只通过版本化协议启动配套 `scoopc`；
2. `BuildRootInput::{ManifestCone, SingleFile}` 先变成只供定位和调度的 `ResolvedBuildGraph`。该图只含 source projection、trusted-core projection和 bounded prebuilt summary，不能冒充 Graph/Compile/Link artifact proof；
3. manifest `path`、`artifact` 与 search-root locator 都按 exact coordinate 工作。同一 `ConeIdentity` 的全部 locator claim 必须收敛到同一种、同一份内容；同 coordinate 的不同 source root、不同 artifact fingerprint或 source/artifact 混用均在启动第一个 compiler child 前失败；
4. 整张图先验证 reserved identity、coordinate、kind、single version、无环、唯一 root、core 注入和 target/schema/ABI summary，再按 canonical dependency-first Kahn order 调度。manifest 枚举顺序、locator 参数顺序、hash seed与未来并发完成顺序不影响图、child 顺序或诊断顺序；
5. prebuilt、cache hit和新 child 输出都必须从不可变 byte snapshot 分别重建完整 Compile 与 Link view。只有两种 view 都成功且依赖记录与当前图逐项相等，节点才进入 completed set；Graph-only、summary-only或 response 中声称的 fingerprint都不能提升节点状态；
6. source node使用由semantic manifest、排序后的source content digest、实际direct dependency三层Merkle fingerprint、compiler/schema/ABI和当前single-Cone会消费的target/toolchain projection计算的内容寻址cache key。locator、绝对路径、mtime、inode、诊断展示路径和child request id不进入key；
7. source输入在任何child启动前完成全图discovery与immutable snapshot。所有Cone（包括core）的child读取私有snapshot，不重新遍历原始源码目录；
8. 每个source cache miss恰好启动一次独立child。child只收到已经completed的direct/support artifact路径、独立trusted core slot和私有output path；父进程不调用`scoopc` lib，不传AST/IR，不让child解析locator或递归构建；
9. M23-4串行执行canonical order。child失败立即停止，不启动任何dependent，也不进入program-link；已成功、已双视图验证并原子发布的content-addressed cache entry可以保留；
10. library root成功结果是已验证root `.slib`及Compile/Link两份closure authority；executable root也只返回同类root artifact和closure。本阶段不构建runtime、program descriptor、native provider或binary，不执行程序，也不宣称正式`scoop build/run/link` CLI已经完成；
11. `compiler/scoop`以recording runner、fake artifact gate和真实M23-3 core-only节点分别验证graph算法与进程边界。含普通dependency的真实source图可以完整规划和调度上游，但当前node最终仍由`SCOOPC_CAPABILITY_NON_CORE_DEPENDENCY_UNAVAILABLE`结束，不生成残缺artifact；
12. cache、prebuilt与child output都不是信任边界。每次使用都受同一个`SlibClosureDecodeLimitsV1`、双视图reader、依赖闭包核对及target/toolchain核对约束；本地cache receipt只用于把key与已验证artifact/warning绑定，不替代artifact proof或发行者签名。

M23-4 的关键不变量是：

```text
ResolvedBuildGraph != ValidatedArtifactClosure<Graph>
Graph summary       != reusable artifact
child success       != completed node
cache path hit      != cache hit

completed node = immutable artifact snapshot
               + Compile-purpose validation
               + Link-purpose validation
               + exact dependency/target/plan match
```

## 1. 范围与阶段边界

### 1.1 本阶段交付

- `compiler/scoop` orchestration library及其production/test runner边界；
- shared target/toolchain registry从stage implementation crate中抽离，使`scoop`无需依赖codegen实现；
- `BuildGraphRequest`、`BuildRootInput`、`ResolvedBuildGraph`、`PreparedBuildGraph`与`BuildGraphOutcome`的typed状态链；
- manifest source、prebuilt artifact、trusted core与single-file四类graph input projection；
- `path`、`artifact`、search-root及artifact dependency record的exact locator算法；
- bounded `.slib` manifest summary probe；
- identity/content claim合并、reserved identity、kind、single-version、cycle与canonical topological order验证；
- graph-wide source snapshot、artifact snapshot及TOCTOU检查；
- `SlibClosureDecodeLimitsV1`及build-wide monotonic budget；
- per-artifact Compile/Link双视图handle，以及purpose保持不擦除的`ValidatedArtifactClosure<Compile>` / `<Link>`；
- direct/support closure投影与canonical child argument顺序；
- `ConeCompileCacheKeyV1`、cache receipt、per-key lock、atomic entry publication与warning replay；
- trusted core slot freshness、bootstrap调度和slot/cache隔离规则；
- 配套`scoopc`解析、版本化stdin/stdout child transport、request/response correlation、退出状态和bounded stderr失败报告；
- child output的coordinate、kind、source form、dependency、target、fingerprint和planned cache key交叉验证；
- chain、diamond、cycle、multiversion、ambiguous locator、stale dependency、cache失效、child失败传播和确定性测试；
- core bootstrap、core-only manifest library/executable及single-file的真实process集成测试；
- `docs/milestone23/DESIGN.md`和`docs/ROADMAP.md`中的M23-4详细设计链接。

### 1.2 本阶段明确不做

- 普通dependency的package surface、exact/star/alias import、re-export、visibility/access/default、alias或split-package lookup；这些属于M23-5；
- 跨Cone layout、scan、typed ABI、dispatch、protected bridge与完整ZST矩阵；这些属于M23-6；
- consumer-side generic concretization、ODR member closure/coalescing和generic delegated extension storage；这些属于M23-7；
- C runtime multi-image registry、program descriptor消费或managed启动；这些属于M23-8；
- runtime-build、program-link、native provider解析、最终link evidence、binary或执行；这些属于M23-9～M23-10；
- 正式用户级`scoop build/run/link`参数、默认输出目录、program argv与fixture runner总迁移；这些属于M23-11；
- registry/network下载、version range/SAT求解、lockfile、workspace、feature、optional/dev/build dependency或remote cache；
- 自动发现相邻source、native object/archive、C/C++ source或blob；
- 把同identity的source与prebuilt artifact按“看起来等价”合并，或用编译器版本字符串代替schema/ABI/profile核对；
- 并行child执行。M23-4固定串行；未来并行只能复用本文的canonical ready set、commit order与diagnostic order；
- cache eviction、LRU、容量回收或跨用户cache共享。M23-4只定义正确性所需的lookup、lock、验证和原子发布；
- 修改M23-2已冻结的persistent identity、mangler、container/member schema，或修改M23-3已冻结的strong profile、runtime image ABI与single-Cone request语义。

M23-4不以“已有dependency artifact”为理由删除M23-3能力门。真实普通dependency请求必须走完graph、artifact closure和child preflight，然后由原有稳定诊断结束；orchestrator不能忽略dependency后编译，也不能把artifact中的public name提前导入HIR。

### 1.3 与相邻阶段的交接

| 阶段 | M23-4 接收 | M23-4 交付 |
| --- | --- | --- |
| M23-3 | strict manifest/locator projection、trusted core slot、single-Cone request、child DTO、双视图有效strong artifact | exact DAG、cache、真实child orchestration与purpose-preserving closure；不改`scoopc`编译语义 |
| M23-5 | graph中已完成的direct/support artifact和Compile closure | `SemanticWorld`可直接消费的完整artifact集合与稳定origin；删除的只有非core能力拒绝，不重做locator/cache |
| M23-6/7 | Link closure、三层fingerprint与stable origin | 新layout/ODR profile仍经同一cache/child/double-view流程；cache key按新增实际消费capability升版而非旁路验证 |
| M23-8 | canonical dependency-first Cone order | runtime registration/startup沿用该order，不从link input或地址顺序重算另一套 |
| M23-9/10 | root及完整`ValidatedArtifactClosure<Link>` | program-link直接消费closure；不读取manifest locator、source snapshot或cache receipt |
| M23-11 | 可测试的orchestration library与root artifact outcome | 薄`scoop` binary、正式build/run/link lifecycle、默认materialization与fixture迁移 |

### 1.4 本阶段的封闭成功子集

production runner在M23-4可以真正成功的source node仍须满足M23-3的core-only语言子集：

- trusted core bootstrap；
- 只依赖trusted core的manifest library；
- 只依赖trusted core的manifest executable；
- 固定只依赖trusted core的single-file executable。

含普通dependency的图仍可完成以下工作：

- 解析全部locator和manifest/prebuilt summary；
- 验证整张DAG、single-version、kind和target/schema/ABI；
- 对prebuilt及已完成上游建立双视图proof；
- 计算canonical order、direct/support closure和child request；
- 通过recording runner验证完整调度；
- 通过production runner确认真实`scoopc`在当前node以M23-3稳定能力诊断失败。

它不能在production runner下伪造“多Cone成功”。任何fake/recording authority只能由`cfg(test)`或crate-private test harness构造，不进入stable library API，不生成可被production reader接受的绕过proof。

## 2. crate与依赖方向

### 2.1 新增`compiler/scoop`

新增package：

```text
compiler/scoop  (scoop)
  request/      BuildGraphRequest、root/cache/search/toolchain配置
  locator/      exact locator与bounded summary discovery
  graph/        claim merge、DAG验证、canonical order、closure projection
  snapshot/     source/artifact immutable snapshot与TOCTOU门禁
  artifact/     双视图gate、purpose closure与completed artifact handle
  cache/        key、receipt、lock、lookup与atomic publication
  child/        配套scoopc解析、protocol transport与response验证
  schedule/     dependency-first serial state machine
  diagnostic/   graph/build diagnostic与稳定排序
```

M23-4只要求library target。最终薄`scoop` binary与稳定用户命令在M23-11接入同一library；本阶段不建立一个临时公开CLI，也不新增会在M23-11保留的第二套命令语义。允许crate内集成测试通过test-only harness调用library，但不能把`cargo run -p scoop -- ...`当作生产合同。

`scoop`允许依赖：

```text
scoop-identity
scoop-manifest
scoop-protocol
scoop-slib
scoop-toolchain
scoop-wire
```

它不得依赖：

```text
scoopc lib
scoop-parser
scoop-hir-lower
scoop-mir-lower
scoop-lir-lower
scoop-codegen implementation
```

因此每个source Cone都必须跨process、跨`.slib`边界。测试runner也只能替换process/clock/filesystem/cache backend等orchestration port，不能调用single-Cone pipeline函数作为“更快的测试路径”。

### 2.2 shared toolchain profile

当前`ResolvedTargetProfile`及registry若仍由codegen implementation crate拥有，`scoop`将被迫依赖stage实现，违反总设计。M23-4新增leaf package：

```text
compiler/toolchain  (scoop-toolchain)
  registry/         canonical target request -> ResolvedTargetProfile
  compiler/         配套scoopc tool identity与protocol capability
  fingerprint/      single-Cone实际消费projection的canonical fingerprint
```

迁移后：

```text
ResolvedTargetProfile {
    lir_target,
    backend,
    c_bridge_toolchain,
    runtime_build,
    final_link,
}
```

仍由唯一registry原子构造。M23-4的single-Cone orchestration只消费前三项；`runtime_build`和`final_link`保持在完整product中但不进入本阶段compile cache key，也不下发给不需要它们的artifact reader。`scoopc`接收canonical target request后从同一registry得到相同product，再把对应projection交给各producer。这个迁移不改变M23-2 artifact中只持久化`ValidatedLirTargetSelection { lir_target, backend }`的wire规则；generated-C object继续由其required capability和object verifier承诺C bridge profile。

`scoop-toolchain`不依赖parser/lower/codegen实现。各projection的data type仍放在对应IR/meta或专用共享crate；`scoop`与`scoopc` driver依赖registry取得完整product，再把typed projection传给stage。codegen等stage implementation不反向依赖registry。不能保留一份codegen-local registry再让`scoop`复制常量。

### 2.3 现有crate调整

`scoop-manifest`新增或公开：

- 不泄漏可变内部map的canonical dependency iterator；
- `DependencyLocator`的typed accessor；
- manifest原始bytes与span-backed diagnostic decorator所需的只读projection；
- 从已加载manifest建立source snapshot时需要的root/identity接口。

它不新增graph、cache或artifact reader逻辑。

`scoop-slib`新增：

- `PrebuiltManifestSummaryV1`的bounded probe；
- immutable artifact snapshot上的双视图验证入口；
- `SlibClosureDecodeLimitsV1`与共享closure meter；
- purpose保持的artifact certificate/closure组合API。

`.slib` container、member、IR capability与strong profile schema不变。summary probe不是新的ArtifactPurpose，也不能调用Compile/Link API。

`scoop-protocol`沿用M23-3的`ScoopcRequestEnvelopeV1`/`ScoopcResponseEnvelopeV1` wire bytes；M23-4只增加transport helper和capability handshake，不回改V1字段。若实际实现发现V1缺少完成本文不变量所必需的字段，必须先以新protocol version演进，不能在V1尾部私加字段或通过stderr传语义。

`scoopc` binary新增隐藏、版本化的child transport入口，并把decoded request归一到与直接CLI相同的`SingleConeBuildRequest`。它不依赖`scoop`，也不取得locator/cache API。

### 2.4 依赖图

```text
scoop-manifest ─┐
scoop-protocol ─┼─> scoop orchestration lib ──process──> scoopc bin
scoop-slib ─────┤
scoop-toolchain ┘

scoopc lib -> parser -> HIR -> MIR -> LIR -> codegen -> slib writer
     ^                                                   |
     └────────────── no dependency from scoop ───────────┘
```

`scoop`可以读manifest和`.slib`，但不能读取IR implementation arena；`scoopc`可以编译当前Cone，但不能follow dependency locator；未来program-link只读Link closure，不读source/cache。这三条边界用Cargo dependency test锁定。

## 3. orchestration request与typed状态链

### 3.1 request

M23-4 library入口接收：

```text
BuildGraphRequest {
    root: BuildRootInput,
    artifact_search_roots: Vec<ArtifactSearchRoot>,
    cache_root: ArtifactCacheRoot,
    sysroot: TrustedSysrootRoot,
    target: TargetSelectionRequest,
    compiler: PairedScoopcLocator,
    diagnostics: DiagnosticsPolicy,
    limits: BuildLimitsProfileV1,
}

BuildRootInput = ManifestCone(ManifestRootLocator)
               | SingleFile(SingleFileLocator)
```

规则如下：

- root分流完全复用M23-3已冻结的basename/type/symlink规则；
- `artifact_search_roots`只为无显式artifact locator的artifact resolution服务，不搜索source；
- `cache_root`、`sysroot`与`compiler`都是typed locator，不从cwd、`PATH`或环境变量在阶段中补默认；未来CLI可以在构造request前解析配置；
- `diagnostics`只决定装饰和输出；M23-4的所有child request固定`emit = None`。stage dump仍可由低层直接`scoopc`取得，正式orchestration dump/fixture策略留给M23-11，不让本阶段cache hit暗中重跑compiler；
- request中的host path允许非UTF-8，只作定位和诊断，绝不进入semantic hash；
- limits profile在开始任何I/O前完成验证，不能由untrusted manifest降低或放大。

M23-4没有output binary path或native library search root。library/executable root的成功产物都仍是root `.slib` handle；M23-11再增加用户materialization，M23-9/10再增加link输入。

### 3.2 状态链

公开状态转换固定为：

```text
BuildGraphRequest
  -> LoadedBuildRoot
  -> DiscoveredBuildGraph
  -> ResolvedBuildGraph
  -> PreparedBuildGraph
  -> ExecutedBuildGraph
  -> BuildGraphOutcome
```

- `LoadedBuildRoot`只证明root operand和trusted sysroot/toolchain locator shape；
- `DiscoveredBuildGraph`可以含尚未全局核对的summary/claim，不能启动child；
- `ResolvedBuildGraph`已通过coordinate/content唯一、kind、version、cycle与canonical order验证，但source/prebuilt bytes尚未全部形成execution snapshot；
- `PreparedBuildGraph`已完成全图source/artifact snapshot、summary复核、core计划、closure budget reservation与cache namespace解析；这是启动第一个compiler child的唯一authority。prebuilt完整双视图和stale-edge验证仍在其全部dependency completed后执行；
- `ExecutedBuildGraph`中的每个节点都是completed dual-view artifact，且root closure已构造；
- `BuildGraphOutcome`只暴露root artifact、Compile closure、Link closure、warnings和本次cache/child观测摘要，不暴露可变scheduler state。

任一步失败都消费并丢弃前一状态，不存在从`DiscoveredBuildGraph`直接调用scheduler、从summary构造completed node或从child response跳过artifact gate的公开方法。

### 3.3 outcome

```text
BuildGraphOutcome =
    Library {
        root: CompletedRootArtifact,
        compile: ValidatedArtifactClosure<Compile>,
        link: ValidatedArtifactClosure<Link>,
        warnings: CanonicalDiagnosticSet,
        observations: BuildObservations,
    }
  | ExecutableArtifact {
        root: CompletedRootArtifact,
        compile: ValidatedArtifactClosure<Compile>,
        link: ValidatedArtifactClosure<Link>,
        warnings: CanonicalDiagnosticSet,
        observations: BuildObservations,
    }
```

`ExecutableArtifact`不是executable binary。两个variant以root artifact已验证的`ConeKind`构造，不能用`Option<Entry>`或调用方传入bool选择。`BuildObservations`只报告哪些节点来自prebuilt/cache/child、child invocation顺序及cache key；它不进入artifact、cache key或语言语义。

## 4. exact locator与graph discovery

### 4.1 graph node与edge

discovery阶段使用：

```text
DiscoveredNode =
    ManifestSource(ManifestSourceProjection)
  | PrebuiltSummary(PrebuiltArtifactProjection)
  | TrustedCore(TrustedCoreProjection)
  | SingleFile(SingleFileProjection)

DiscoveredEdge {
    dependent: ConeIdentity,
    dependency: ConeIdentity,
    coordinate: ConeCoordinate,
    origin: EdgeOrigin,
    expected_semantic: None | DependencySemanticExpectation,
}

EdgeOrigin = ManifestDeclaration { manifest, span, locator_kind }
           | ArtifactDependencyRecord { artifact, record_index }
           | InjectedTrustedCore { dependent }
           | SyntheticSingleFileCore
```

source manifest edge没有编译时fingerprint expectation；其dependency实际完成后，当前source将以这份新fingerprint编译。artifact edge携带HIR/MIR/LIR expectation；实际dependency完成后必须逐项相等，否则该prebuilt是stale。edge origin只作诊断和claim规则，不进入Cone identity或artifact fingerprint。

### 4.2 source `path` locator

对`path = "..."`：

1. 以声明它的`Cone.toml`所在real root为基准解析relative path；absolute path也只作host locator；
2. 复用`ManifestRootLocator`规则，最终必须解析为directory或basename精确为`Cone.toml`的regular file；
3. canonicalize后读取strict manifest，不在此时读取`src`正文；
4. 被定位manifest的canonical coordinate必须与dependency key/version逐字相等；
5. claim内容为`SourceClaim { coordinate, real_manifest_root }`。

同一coordinate出现多个source claim时，只有canonical real root完全相等才去重；不同real root即使manifest semantic和source bytes当前相同也报告`ConflictingSourceLocator`。构建图不能在编译前证明两个可变source tree永久等价，也不能按发现顺序任选一个。

source symlink alias若canonicalize到同一real root可以合并；用户原始spelling全部保留为diagnostic origins，但不影响选择和排序。

### 4.3 explicit `artifact` locator

对`artifact = "..."`：

1. relative path以声明manifest为基准；
2. 目标跟随symlink后必须是regular file，directory、device、dangling/cycle均失败；
3. `scoop-slib`执行bounded summary probe，取得coordinate/id/kind/source form、direct dependency records、compatibility、target selection、profile id、member总量及claimed `ArtifactFingerprint`；
4. summary coordinate必须与dependency declaration逐字相等；
5. manifest dependency位置禁止executable、single-file与reserved core artifact；
6. claim内容为`ArtifactClaim { coordinate, artifact_fingerprint, resolved_locator }`。

同一coordinate的多个artifact claim只有claimed完整`ArtifactFingerprint`相等才可继续；不同fingerprint立即报告ambiguous/conflicting artifact。相等仍不是可复用proof：第6章preflight会对**每个**实际候选形成immutable snapshot并完整双视图验证，防止两个损坏文件只伪造了相同summary。

### 4.4 search-root locator

省略`path`/`artifact`的manifest dependency，对每个显式`ArtifactSearchRoot`检查：

```text
<root>/<group>/<name>/<version>/cone.slib
```

三段均使用canonical完整文本，`.`不拆目录。检查规则：

- `lstat`/等价操作确认完全不存在时记为absent；dangling symlink、permission error、wrong type或存在但无法canonicalize不是absent，而是locator error；
- canonicalize到同一regular file的多个search-root候选先按resolved file去重；
- 零个候选报告`ArtifactNotFound`并列出受检search root；
- 所有存在候选都执行bounded summary probe并要求coordinate匹配；
- 所有candidate的完整`ArtifactFingerprint`必须相同，否则报告`AmbiguousArtifact`，诊断按resolved host path稳定排序展示全部候选；
- 相同fingerprint时semantic选择只有一个，不因search-root参数顺序改变。实现可保留全部locator供preflight交叉验证；若需要一个display representative，使用平台定义的canonical host-path byte order，不把该选择写入任何semantic值。

search root本身不会被递归遍历，也不接受“最接近版本”或任意文件名fallback。

### 4.5 artifact dependency record的后继定位

`.slib` dependency record只有exact coordinate/id/三层fingerprint，不携带host locator。处理prebuilt summary的edge时：

- 若同coordinate已由某个manifest的显式`path`/`artifact`/search-root claim唯一解析，则复用该graph node；
- 否则按当前request的artifact search roots解析；
- reserved core始终绑定trusted sysroot slot，绝不进入普通search roots；
- 不能从当前artifact所在目录、文件名、相邻`Cone.toml`或任意环境变量猜locator。

artifact edge的expected semantic fingerprint在node完成时验证，不参与选择另一个版本或另一个coordinate。

### 4.6 claim收敛

每个`ConeIdentity`的claim集合按以下规则原子收敛：

| claims | 结果 |
| --- | --- |
| 同一canonical source root的一项或多项source claim | 一个source node |
| 相同完整artifact fingerprint的一项或多项artifact claim | 一个prebuilt node，保留全部候选待验证 |
| source claim + artifact claim | `ConflictingNodeRepresentation` |
| 不同source root | `ConflictingSourceLocator` |
| 不同artifact fingerprint | `AmbiguousArtifact` |
| 同identity、不同canonical coordinate record | artifact/identity corruption |

不能以“source稍后也许编出相同artifact”放宽source/artifact冲突；也不能把artifact fingerprint相同当作发行者认证。相同fingerprint只说明按M23容器公式声称同一完整envelope，仍须读取全部bytes验证。

claim处理使用以`ConeIdentity`为key的typed map，同时维护`(group,name) -> coordinate`索引；不以FQN、path或artifact fingerprint代替Cone key。

### 4.7 bounded summary不是artifact proof

`PrebuiltManifestSummaryV1`最多暴露graph所需字段：

```text
PrebuiltManifestSummaryV1 {
    cone,
    direct_dependencies,
    compatibility,
    target_selection,
    profile,
    semantic_fingerprints,
    code_fingerprint_availability,
    runtime_image_fingerprint_availability,
    artifact_fingerprint,
    member_count,
    archive_length,
}
```

summary reader验证normal-archive envelope、manifest canonical wire、directory shape、长度/range与manifest自身可重算字段；它可以stream/hash为选择提供必要信息，但不运行Compile capability handler、LinkObject verifier、registration/digest graph或typed remap。返回类型不实现`ValidatedGraphArtifact`，也没有`artifact_bytes()`、metadata或object accessor。

summary只允许：

- 发现outgoing exact dependency coordinate；
- 做coordinate/kind/compatibility的早期拒绝；
- 比较search候选的claimed完整artifact fingerprint；
- 估算并预扣graph/closure资源。

任何后续bytes变化、summary与full view不一致、member hash损坏或unknown required capability都在preflight失败；不能换一个同coordinate候选继续，除非所有原始candidate从一开始就是同一fingerprint且全部验证成功。

### 4.8 discovery worklist

discovery不用递归调用栈追locator。它维护按expected coordinate排序的bounded worklist：

1. 插入root source projection和trusted core projection；
2. 取最小pending claim，解析其manifest或summary并立即按ConeIdentity intern；
3. claim与已有node冲突时记录4.6错误，不覆盖已有值；
4. 对source manifest的每条dependency生成带原span和locator kind的claim；
5. 对prebuilt summary的每条dependency生成unlocated exact requirement；若当前没有manifest locator claim，使用search roots；
6. 已intern identity仍合并incoming origin与fingerprint expectation，但不重复展开同一canonical projection；
7. 直到worklist为空，再统一执行single-version、kind、reachability、SCC与Kahn验证。

为了避免worklist先后决定“unlocated artifact requirement是否看到另一分支的manifest claim”，实现分两轮达到fixpoint：每轮先耗尽全部显式source/artifact/search-root manifest claim并展开新source manifest，再为仍无node的artifact-record requirement运行search roots；search得到的新artifact只会产生新的artifact-record requirement，不会携带manifest locator。若之后仍出现表示冲突，按4.6失败而不是改变既有选择。

worklist entry、incoming origin和summary probe都向graph meter预扣。cycle不会导致无限discovery：identity在展开outgoing edge前已经intern，真正cycle在完整edge集合上由5.6报告。

## 5. resolved graph与canonical order

### 5.1 `ResolvedBuildGraph`

```text
ResolvedBuildGraph {
    root: ConeIdentity,
    core: ConeIdentity,
    nodes: CanonicalMap<ConeIdentity, ResolvedGraphNode>,
    edges: CanonicalSet<ResolvedDependencyEdge>,
    dependency_first: NonEmptyCanonicalVec<ConeIdentity>,
    root_kind: RequestedConeKind,
    target_selection: ValidatedLirTargetSelection,
}

ResolvedGraphNode =
    Source(ResolvedManifestSource)
  | Prebuilt(ResolvedPrebuiltSummarySet)
  | TrustedCore(ResolvedTrustedCoreNode)
  | SingleFile(ResolvedSingleFileNode)
```

字段私有，只有完成本章全部验证的构造器能返回该类型。`nodes`排序用于canonical encoding/诊断，不代表构建顺序；唯一构建顺序是`dependency_first`。

### 5.2 core注入与single-file

- trusted sysroot先解析出reserved `scoop:scoop.core:0.1.0` source slot和artifact slot；用户manifest/ordinary locator不能声明或占用该identity；
- core node没有direct dependency，不注入self edge；
- 每个非core node恰有一条direct core edge。source manifest不写该edge，resolver注入；prebuilt summary必须已经包含与其编译时core fingerprint对应的dependency record，resolver把该record绑定到trusted core node；
- prebuilt缺core、重复core或把core放成transitive-only都拒绝；
- single-file graph不运行manifest dependency discovery，结构上精确包含core和synthetic root两个node、一条root→core edge；
- reserved single-file node只能是本次root、kind必须是executable、不能被任何其他node依赖，也不能来自prebuilt locator。

core artifact是否fresh属于第6/8章的prepared/cache状态，不改变graph identity或edge。

### 5.3 kind与root不变量

- root为manifest source或single-file source，不接受prebuilt root；显式`scoop link --root-slib`属于M23-9/11；
- root manifest可为library或executable；
- 除root外所有node必须为library；
- executable dependency在summary阶段即可拒绝，不能等待child或native linker；
- library root的图中不存在executable node；executable root的图中恰有root一个executable；
- source manifest声明kind与后续child artifact kind必须一致；prebuilt summary kind与full双视图kind必须一致。

### 5.4 single-version与identity/content唯一

构图完成后，以`(group UTF-8 bytes, name UTF-8 bytes)`聚合全部canonical coordinate：

- 一个key只有一个canonical version时通过；
- 出现多个version时报告`MultipleVersions`，列出每个version、ConeIdentity及全部incoming edge origin；
- 不做版本选择、偏好root、semver比较或“兼容版本”合并；
- 同identity若summary/manifest携带不同coordinate是hash/record corruption，不归为普通multiversion；
- 同identity的内容冲突先按4.6报告，不能等map覆盖后消失。

诊断排序使用`group`、`name`、canonical version UTF-8 bytes，不按digest或发现顺序。

### 5.5 edge与闭包不变量

每个edge必须满足：

- dependent与dependency都存在；
- 两端不同，除core外也不允许任何self edge；
- manifest edge的coordinate/id与被定位node相同；
- artifact edge的record coordinate/id与被定位node相同；
- dependency不是executable/single-file；
- ordinary edge不冒充core；
- 相同dependent/dependency只能有一条semantic edge；若来自多个origin，origins合并用于诊断，但预期fingerprint必须全等。

artifact diamond中同一个origin node只intern一次。不同父节点对同一prebuilt dependency record给出不同三层fingerprint时，在node完成时报告所有冲突origin；不能按先到者选一份。

### 5.6 cycle诊断

resolver在完整node/edge集合上先运行SCC，再运行canonical Kahn：

1. adjacency按dependency coordinate排序；
2. SCC内部含多node或含self edge即为cycle；
3. primary cycle选择含最小coordinate的cyclic SCC；
4. 从该SCC最小coordinate开始，按edge目标coordinate顺序寻找第一条回到起点的simple path；
5. 诊断打印完整`A -> B -> ... -> A` coordinate链，并为每条edge附原始manifest span或artifact record origin；
6. 其余cycle作为按SCC最小coordinate排序的secondary diagnostic。

这样cycle文本不取决于DFS hash iteration、manifest声明顺序或future worker完成顺序。cycle存在时不产生topological order、不查cache、不启动child。

### 5.7 canonical dependency-first order

对无环图使用stable Kahn：

- “ready”精确定义为所有direct dependency都已从剩余图移除的node；
- 每轮从ready set按`(group UTF-8 bytes, name UTF-8 bytes, canonical version)`取最小者；
- 输出该node后，更新依赖它的dependent；
- core通常较早ready，但不通过特殊case强行排第一；其无依赖和coordinate自然决定位置；
- 最终sequence必须恰含每个node一次，并且每条`dependent -> dependency`中dependency index更小；
- Kahn cost按M23-2的确定性公式向graph meter预扣，不能按实际heap比较次数计费。

该order同时成为：

- M23-4串行child顺序；
- warning/child failure的Cone主排序；
- 后续program descriptor image order；
- runtime eager initializer的Cone一级顺序。

任何后续stage不得从artifact参数顺序、link order或地址重新导出另一套Cone order。

### 5.8 direct与support closure

对每个source node `N`预计算：

```text
direct(N)  = manifest direct dependency set，排除注入core
reachable(N) = 从direct(N)沿dependency edge可达的全部node
support(N) = reachable(N) - direct(N) - {core, N}
```

规则：

- direct与support都按coordinate排序；
- diamond中同identity只出现一次；
- core只经`TrustedCoreRequestV1::ArtifactSlot`传递，绝不出现在两组普通参数；
- single-file与core bootstrap的direct/support结构上为空；
- support不是“全图中除direct外所有node”，与N不可达的节点不能传给child；
- M23-4 graph本身从root可达，因此通常没有全局孤儿；测试构造器仍必须拒绝unreachable node；
- child output dependency table只应记录direct（含core），不能把support误写成direct。

closure投影在图验证后计算，不从命令行argument反推；其结果随后由`scoopc` M23-3已有preflight再次独立验证。

## 6. graph preflight、snapshot与资源预算

### 6.1 `PreparedBuildGraph`的原子门

`ResolvedBuildGraph::prepare`在任何compiler child启动前完成：

1. 为全部source node发现、读取并固定source set；
2. 为全部prebuilt candidate建立immutable artifact snapshot；
3. 从snapshot复核全部summary、candidate fingerprint一致性及可在Graph phase确定的compatibility；
4. 解析trusted core slot，决定reuse或bootstrap计划；
5. 保存所有prebuilt artifact edge的typed fingerprint expectation，供dependency completed后逐项验证；
6. 预创建本次build的private staging root、cache namespace与per-node output plan；
7. 验证配套`scoopc`、target registry和protocol capability；
8. 完成build-wide resource reservation并冻结diagnostic decoration表。

任一失败都不启动child、不写cache entry、不修改trusted core slot，也不产生`PreparedBuildGraph`。preflight可以并行做只读检查，但error集合必须在commit前按第12章排序；本文首版实现可以串行。

“全图错误在第一个child前失败”至少覆盖locator、manifest、summary、claim、reserved identity、kind、multiversion、cycle、source discovery、summary级不兼容、search ambiguity、toolchain/protocol不匹配和resource budget。完整prebuilt payload/view错误、依赖上游source实际输出后才能判断的stale expectation、动态cache candidate损坏和child自身编译错误属于execution error，不能伪称已经在graph phase可知；但它们都必须早于任何dependent child。

### 6.2 source snapshot

manifest source snapshot为：

```text
ManifestSourceSnapshot {
    cone,
    requested_kind,
    manifest_semantic,
    manifest_bytes,
    sources: NonEmptyCanonicalVec<SourceSnapshot>,
    original_diagnostics: SourceDisplayMap,
}

SourceSnapshot {
    identity: SourceIdentity,
    logical_path: NormalizedSourcePath,
    content_digest: SourceContentDigest,
    bytes: ImmutableBytes,
}
```

`sources`完全复用M23-3 discovery的symlink containment、UTF-8、extension、non-empty和canonical path规则。snapshot时：

- 先对每项读取bytes并记录resolved target，再重新canonicalize/inspect，目标或类型改变即失败；
- source text必须是UTF-8；content digest来自实际固定bytes；
- manifest bytes同样在读取后重验目标；
- snapshot不记录mtime/inode为语义，只可把它们用作变化检测的附加证据；
- source总数/总bytes向build-wide meter单调计费。

ordinary child不直接读取用户source tree。orchestrator在private、权限受限、create-new的snapshot root中物化：

```text
snapshot/<ConeIdentity>/Cone.toml
snapshot/<ConeIdentity>/<normalized Cone-relative logical path>
```

写入原始manifest bytes和每个已固定source bytes，不复制symlink、无关文件、cache、native source或blob。所有目录/文件用create-new构造并检查没有path escape；logical path是唯一布局来源。child因此仍执行自己的M23-3 manifest/source discovery与parser，但观察到的只能是父进程已经纳入cache key的immutable内容。

manifest中的relative dependency locator在snapshot位置可能不再指向原host目标；这是允许且必须的，因为`scoopc`按契约只解析其syntax/semantic projection，不follow locator。若child开始依赖这些path能被访问，即构成边界回归测试失败。

single-file snapshot只物化一个private `main.scoop` regular file，并以`CurrentConeRequestV1::SingleFile`调用；basename只需满足`.scoop` operand规则，semantic logical path仍由single-file loader固定为`main.scoop`。

### 6.3 core源码快照

core与其他manifest Cone使用同一source discovery、immutable snapshot和私有输入目录。编译子进程通过普通ManifestRoot请求读取快照，输出写入本次build的staging；core不依赖自身。用户可随时编辑原始core目录，本次构建使用已捕获内容，下次构建按新内容计算普通cache key。不再持有sysroot专用锁或在child结束后重复发现core源码。

### 6.4 artifact snapshot

artifact snapshot不把可替换host path直接交给validator：

```text
ArtifactSnapshot {
    source_locator: DiagnosticHostPath,
    byte_length,
    sha256,
    backing: PrivateImmutableFile | ImmutableOwnedBytes,
}
```

构造步骤：

- open regular file，拒绝directory/device；
- 在size limit内把bytes复制到本次build private staging或受检immutable backing；
- hash只消费snapshot backing；
- copy完成后重新inspect原文件，若resolved target/size或可用的稳定file identity改变则报告`ArtifactChangedDuringSnapshot`；
- 后续summary复核、Compile view、Link view、cache publication与child input materialization都消费snapshot，不再重开原locator；
- child需要path时，在private artifact input目录以create-new方式物化或hard-link只读snapshot，并在调用前后核对digest。不能把用户可替换path直接透传。

artifact `sha256`只是snapshot完整性值，不是`ArtifactFingerprint`。后者必须由`.slib` reader从canonical manifest/member record重算。

### 6.5 fixed prebuilt验证

所有explicit/search/prebuilt candidate在graph preflight中先完成第1步；scheduler到达该node且其全部dependency Completed后完成第2～6步：

1. 从snapshot重跑summary，必须与discovery summary逐字段相同；
2. 使用当前target、profile、已完成trusted core owner proof和C bridge profile分别运行M23-3 self-describing Compile与Link验证；后续profile若要求完整dependency closure，则从已完成dependency的purpose handle构造对应reader输入；
3. 构造第7章dual-view handle；
4. 对同coordinate的全部候选比较实际`ArtifactFingerprint`和semantic/code/runtime-image summary；
5. 即使fingerprint相同，每个candidate自身也必须通过；一个损坏副本不会因另一个副本有效而被忽略；
6. 成功后按host locator的canonical诊断顺序选一个snapshot作为实际child input，semantic结果与选择无关。

prebuilt artifact是不可重建node。其任一dependency expectation与最终selected dependency不符时报告`StalePrebuiltDependency`，不能退回同coordinate的另一个版本、从相邻source重编译或把错误降级为cache miss。

### 6.6 `SlibClosureDecodeLimitsV1`

M23-2单artifact limit之上增加build/closure总量：

| resource | 默认上限 |
| --- | ---: |
| Cone node数 | 4,096 |
| dependency edge数 | 65,536 |
| graph深度 | 1,024 |
| artifact search root数 | 256 |
| locator candidate总数 | 65,536 |
| source file总数 | 1,048,576 |
| source bytes总量 | 4,294,967,296 |
| unique artifact snapshot bytes总量 | 17,179,869,184 |
| archive nonmanifest member总数 | 1,048,576 |
| manifest/member directory carrier bytes总量 | 536,870,912 |
| decoded logical heap累计 | 8,589,934,592 |
| decoded node累计 | 67,108,864 |
| decoded edge累计 | 268,435,456 |
| owned/copied decode bytes累计 | 4,294,967,296 |
| validation work units累计 | 1,073,741,824 |
| child request数 | 4,096 |

规则：

- 单artifact仍同时受M23-2较小的各项上限；closure limit不能放宽单项；
- graph summary、source snapshot、Compile view、Link view、cache receipt和protocol framing共享build meter中的对应维度；
- 相同`ArtifactFingerprint`的相同snapshot bytes在同一purpose中只计一次；Compile和Link是两次独立验证，各计一次decode/work，但physical snapshot bytes只计一次；
- diamond/repeated path不得重复获得预算；不同fingerprint冲突candidate在报告冲突前仍受candidate/summary上限，不能用大量冲突绕过资源门；
- 所有加法/乘法使用checked `u64`，分配前预扣，失败不返还；
- production默认值由`scoop-toolchain`集中提供并以`SlibClosureLimitProfileIdV1`标识。profile id属于consumer compatibility/诊断，不进入Cone identity；cache hit仍用当前limits重新验证，因此无需仅因limit放宽使compile cache miss；
- 同一profile和meter算法同时接入`compiler/scoop`、直接`scoopc build`的显式dependency closure以及未来program-link；三者可以因实际purpose不同消费不同work，但不能各自定义同名、不同默认值的closure limit；
- test可以整体注入更小limits，不能为单node/handler重置meter。

超过上限返回`SCOOP_GRAPH_RESOURCE_LIMIT`或更精确的artifact reader错误，包含resource kind、configured limit、checked observed value和canonical graph/artifact origin；不得panic、OOM或产生partial graph。

## 7. 双视图artifact authority与closure

### 7.1 immutable dual-view handle

M23-3的validated view借用artifact bytes；M23-4不能用unsafe self-reference或泄漏内存把它伪装成长期owner。新增逻辑类型：

```text
DualValidatedArtifactHandle {
    snapshot: Arc<ArtifactSnapshot>,
    publication: PublishableSingleConeArtifact,
    compile_certificate: CompileViewCertificateV1,
    link_certificate: LinkViewCertificateV1,
}
```

唯一构造器对同一immutable snapshot分别打开两次reader，运行正式Compile/Link入口，再比较M23-3 publish gate的全部共同字段。certificate只保存由成功proof投影出的immutable canonical summary、decode usage、profile和snapshot digest；它没有从raw summary构造的公开入口。

需要实际metadata/object accessor时使用scoped API：

```text
handle.with_compile_view(|ValidatedCompileArtifact| -> R)
handle.with_link_view(|ValidatedLinkArtifact| -> R)
```

API在同一snapshot上重建相应view并核对certificate后调用closure；不能把borrowed view移出scope。M23-4 orchestration主要使用certificate；M23-5与program-link可通过purpose closure的scoped批量入口消费完整view。任何重开失败都使handle失效并终止build，不降级为summary。

### 7.2 purpose不能擦除

```text
ValidatedArtifactClosure<P> {
    root: ConeIdentity,
    order: NonEmptyCanonicalVec<ConeIdentity>,
    artifacts: CanonicalMap<ConeIdentity, PurposeArtifactHandle<P>>,
    edges: CanonicalSet<ValidatedDependencyEdge>,
}

P = Compile | Link
```

- `<Compile>`只能从每个dual handle的Compile certificate投影；
- `<Link>`只能从每个dual handle的Link certificate投影；
- 无`AnyPurpose`、无无参`ValidatedArtifactClosure`、无Compile→Link cast；
- Graph summary/`ValidatedGraphArtifact`都不能调用构造器；
- 两份closure共享Arc snapshot可以避免复制，但proof/certificate类型独立；
- root/order/edge相同不允许把一个purpose token重解释成另一个。

### 7.3 closure构造

对completed root `R`：

1. 从resolved graph计算R可达子图；
2. 要求每个node都有dual handle；
3. 对每个artifact重新核对coordinate/id/kind/source form/target/profile；
4. artifact direct dependency record集合必须等于resolved direct edge集合（core包含在artifact记录中）；
5. 每条record的HIR/MIR/LIR fingerprint必须等于dependency handle；
6. 同identity只能有一个artifact fingerprint；同`group:name`只能一个version；
7. closure order是全图canonical order在可达集合上的稳定投影，并再次验证dependency-first；
8. Compile/Link两种purpose分别构造，任一失败则dual closure整体失败。

closure构造不再次解析locator，也不从artifact dependency table扩张图；若artifact宣称graph外额外edge，直接报告`UnexpectedArtifactDependency`。

### 7.4 completed node

```text
CompletedNode {
    cone: ConeIdentity,
    origin: Prebuilt | CacheHit | Compiled | TrustedCore,
    artifact: DualValidatedArtifactHandle,
    compile_closure: ValidatedArtifactClosure<Compile>,
    link_closure: ValidatedArtifactClosure<Link>,
    materialized_child_path: PrivateArtifactPath,
}
```

node只有在两份closure都成功后才commit进scheduler completed map。这样即使当前child只直接消费Compile metadata，损坏Link object的dependency也不能成为上游或cache hit。

`materialized_child_path`指向本次build private、digest-checked的`.slib` snapshot，不是用户prebuilt path或可替换cache path。路径只作transport。

### 7.5 stale dependency分类

- source node的cache key因dependency semantic fingerprint改变而自然变化，旧entry不被查询，属于cache miss；
- exact新key下的receipt/artifact却记录不同dependency，属于cache corruption；
- prebuilt artifact记录不同dependency，属于`StalePrebuiltDependency`且不可重建；
- child新输出记录不同dependency，属于`ChildOutputPlanMismatch`，说明child/protocol/toolchain违反合同；
- trusted core slot artifact与当前core source key不符，属于core slot miss并bootstrap；artifact本身损坏先分类为`CoreRebuildReason::Corrupt`，再由trusted authority重建，不能把损坏描述成“源码更新”或在重建前把它交给ordinary child。只有重建也失败时才把slot corruption作为最终error context。

任何分类都不能把旧typed id接到新metadata。

## 8. compile cache

### 8.1 cache作用域

M23-4 cache只保存single-Cone `.slib`和重放该次source编译warning所需的receipt。它不缓存：

- `ResolvedBuildGraph`或locator结果；
- source snapshot；
- runtime object、program descriptor、native provider、link plan或binary；
- Graph/Compile/Link内存proof；
- trusted core intrinsic authority本身。

每次lookup仍重建artifact双视图和closure proof。cache不使reader、target检查或stale edge检查可选。

### 8.2 `ConeCompileCacheKeyV1`

canonical input为：

```text
ConeCompileCacheInputV1 {
    schema: 1,
    cone: ConeRecord,
    manifest_semantic: CurrentConeSemanticProjectionV1,
    sources: CanonicalVec<{
        source: SourceIdentity,
        content: SourceContentDigest,
    }>,
    dependencies: CanonicalVec<CompileDependencyInputV1>,
    compiler: PairedCompilerFingerprintV1,
    compatibility: IdentityAbiDescriptor,
    artifact_profile: ArtifactCapabilityProfileId,
    protocol: ScoopcProtocolCapabilityV1,
    lir_target: LirTargetProfileFingerprint,
    backend: BackendProfileFingerprint,
    c_bridge_toolchain: CBridgeToolchainProfileFingerprint,
}

CompileDependencyInputV1 {
    cone: ConeRecord,
    hir: HirFingerprint,
    mir: MirFingerprint,
    lir: LirFingerprint,
    single_file_core_code: None | CodeFingerprint,
}

ConeCompileCacheKeyV1 =
    DomainSeparatedCborHash("scoop-cone-compile-cache-v1",
                            ConeCompileCacheInputV1)
```

`CurrentConeSemanticProjectionV1`：

- manifest分支编码coordinate、requested kind及按dependency coordinate排序的exact semantic dependency集合；不编码locator、TOML whitespace/comment/span；
- single-file分支编码固定reserved coordinate、Executable、唯一`main.scoop`和core-only标记；
- core使用普通manifest分支，编码其实际coordinate、Library及manifest dependency语义；不再编码bootstrap profile。

上述逻辑字段的Wire CBOR精确冻结为：

```text
ConeCompileCacheInputV1 = map(12) {
    1: schema (= 1),
    2: cone,
    3: current_semantic,
    4: sources,
    5: dependencies,
    6: compiler,
    7: compatibility,
    8: artifact_profile,
    9: protocol,
    10: lir_target,
    11: backend,
    12: c_bridge_toolchain,
}

SourceCacheInputV1 = map(2) {
    1: source,
    2: content,
}

CompileDependencyInputV1 = map(5) {
    1: cone,
    2: hir,
    3: mir,
    4: lir,
    5: single_file_core_code,
}

OptionalCoreCodeFingerprintV1 =
    None map(1) { 0: 1 }
  | Some map(2) { 0: 2, 1: code_fingerprint }

CurrentConeSemanticProjectionV1 =
    Manifest map(4) {
        0: 1,
        1: coordinate,
        2: requested_kind,
        3: sorted_exact_dependency_coordinates,
    }
  | SingleFile map(3) {
        0: 2,
        1: reserved_single_file_coordinate,
        2: normalized_path("main.scoop"),
    }

RequestedConeKindV1 = Library unsigned(1) | Executable unsigned(2)

PairedCompilerFingerprintV1 = map(3) {
    1: scoopc_executable_sha256,
    2: toolchain_distribution_id,
    3: compiler_build_identity,
}
```

map key按数值严格递增；sequence count显式编码。`sources`按`SourceIdentity` canonical bytes严格排序，`dependencies`按coordinate byte order严格排序且唯一。`sorted_exact_dependency_coordinates`不含注入core；core在外层`dependencies`中作为实际direct artifact出现。`protocol`字段编码protocol version、request/response schema fingerprint与machine transport capability的封闭record，不能只写自由文本版本。

`single_file_core_code = Some`只允许当前projection为`SingleFile`且本dependency恰为reserved core；该分支结构上必须恰出现一次。其他manifest dependency及core bootstrap一律为`None`，core bootstrap的dependency sequence为空。非法组合在hash前拒绝，不能让两个不同逻辑输入编码成同一宽松record。

dependency规则：

- M23 v1安全基线对每个direct dependency保守纳入HIR/MIR/LIR三层Merkle fingerprint；
- support节点不单独进入key，因为direct三层fingerprint必须由其artifact profile证明已经包含完整Merkle support edge；不能证明该性质的artifact不满足本阶段cache/dependency profile，直接拒绝，不能在V1临时改成另一套“把reachable列表塞进options”的key；
- single-file按语言规范额外把trusted core `CodeFingerprint`放入`single_file_core_code`；其他node为`None`，最终link的transitive code fingerprint不属于一般compile key；
- 不使用whole `ArtifactFingerprint`，因此optional diagnostic/opaque attachment变化不会误触发重编译；
- M23-5以后若以`CrossConeUseSet`安全裁剪MIR/LIR edge，必须提升cache input schema/capability；不能在V1悄悄改变字段含义。

`PairedCompilerFingerprintV1`按上述三字段覆盖配套`scoopc` executable content digest、toolchain distribution和compiler build identity；protocol capability由cache input field 9独立编码，不在compiler record重复。producer版本字符串只作诊断。M23-4没有额外可改变artifact的自由compile option，因而V1不预留空`semantic_options`字段；未来增加optimization/language mode时必须升级cache input schema。diagnostic format、display path、cache root、output path、request id和并发度明确排除。

runtime-build/final-link projection不进入key，因为single-Cone child不消费它们；C bridge toolchain进入key，因为generated C object及CodeFingerprint会受其影响。

### 8.3 cache layout

cache root内部固定versioned namespace：

```text
compile-v1/<full-cache-key-hex>/
    artifact.slib
    receipt.cbor
```

`compile-v1/.locks/<full-cache-key-hex>.lock`保存per-key advisory lock inode，`compile-v1/.staging/entry-*`保存尚未发布的private entry directory；两个点前缀目录都是namespace保留实现目录，不能被当作entry。目录名必须使用完整32-byte key的64个hex字符，不允许缩写成为实际路径key。cache root path不进入key。已发布entry目录只允许上述两个regular file，缺失、额外entry或任何symlink都算corruption。

`CacheReceiptV1`至少包含：

```text
CacheReceiptV1 {
    body: CacheReceiptBodyV1 {
        schema,
        cache_key,
        artifact_fingerprint,
        cone record,
        target selection,
        direct dependency records,
        compiler fingerprint,
        artifact profile,
        structured warnings,
    },
    fingerprint: CacheReceiptFingerprintV1,
}

CacheReceiptFingerprintV1 =
    DomainSeparatedCborHash("scoop-cache-receipt-v1",
                            CacheReceiptBodyV1)
```

Wire CBOR字段固定为：

```text
CacheReceiptV1 = map(2) {
    1: body,
    2: fingerprint,
}

CacheReceiptBodyV1 = map(9) {
    1: schema (= 1),
    2: cache_key,
    3: artifact_fingerprint,
    4: cone,
    5: target_selection,
    6: direct_dependency_records,
    7: compiler_fingerprint,
    8: artifact_profile,
    9: structured_warnings,
}

CacheTargetSelectionV1 = map(2) {
    1: lir_target_tag (= 1 for Darwin/AArch64),
    2: backend_tag (= 1 for LLVM 22.1),
}
```

dependency records和warnings分别按其canonical key严格排序且唯一。warning key是`map(2) { 1: code, 2: origin }`的canonical Wire CBOR bytes；同key而payload不同是receipt corruption，不按读取顺序任选。receipt只接受`Warning` severity，warning及note origin只允许`None`或`SemanticSourceSpan`；`HostPathSpan`和带host locator的`ArtifactPath`必须由父进程先转换为semantic origin，否则该warning不可写入cache。

receipt使用canonical Wire CBOR、bounded decode，并从不含fingerprint字段的独立body重算上述digest；不是把自身slot归零。它不是`.slib` member，不改变artifact bytes。receipt只能由父进程在child output已经双视图验证、plan match成功后生成。warning origin只保存semantic source path/wire path，cache hit时用当前snapshot display map装饰；不保存临时snapshot path。

### 8.4 lookup

对source node，在全部dependency completed后才能计算key。lookup顺序：

1. 取得per-key shared/exclusive advisory lock协议所需的lock file；
2. 若entry目录不存在，返回miss；
3. bounded读取receipt并要求key逐byte相等；
4. snapshot `artifact.slib`；
5. 完整双视图验证；
6. 比较receipt、artifact、resolved node、target、profile和当前dependency records；receipt的dependency列表按coordinate排序，artifact的列表遵守slib自己的顺序，两者先按同一coordinate顺序比较完整record，不能把表示顺序差异误报为内容不一致；
7. 构造两份purpose closure；
8. 成功才返回cache hit并重放warnings。

exact key位置存在但receipt/artifact损坏、缺文件、wrong type、symlink、fingerprint不符或plan不符时返回`CacheEntryCorrupt`，不静默当miss并覆盖证据。M23-4不自动删除；用户或未来cache maintenance可显式清理。旧schema位于不同namespace，按miss处理。

成功lookup直接完成该source node且不调用child。M23-4不请求stage dump，因此不存在为了ephemeral output绕过cache或重跑compiler的旁路。

### 8.5 lock与atomic publish

锁使用由toolchain platform adapter提供的advisory file lock，生命周期绑定打开的file descriptor；不使用只写PID、需要猜stale状态的裸lock file。锁对象只覆盖一个完整cache key，core同样使用per-key锁；加锁路径本身用create-new/目录权限约束，不能跟随cache内symlink。

miss后：

1. 取得exclusive per-key lock；
2. 在lock内重新lookup，另一个process可能已经发布；
3. 仍miss才启动本node child；
4. 在cache root同filesystem创建private entry directory；
5. create-new写`artifact.slib` snapshot与`receipt.cbor`，flush/sync并关闭；
6. 从private目录再次lookup式验证；
7. sync目录后以平台的atomic no-replace rename把整个目录发布为key目录；不允许先检查不存在再调用可覆盖destination的普通rename；
8. 若destination已由竞争者产生，验证winner。winner与本次artifact/receipt一致则使用winner；不一致报告`NondeterministicCacheProduction`，绝不覆盖；
9. 失败依靠private-dir guard清理，不改变已有entry。

artifact与receipt不能用两个独立rename“近似原子”发布。M23-4不使用`rm -rf`清理未知cache root，也不在失败时截断现有entry。

### 8.6 core复用普通compile cache

core使用与所有Cone相同的Manifest语义投影、`ConeCompileCacheKeyV1`、`CacheReceiptV1`、per-key lock、原子目录发布和artifact解码路径。删除额外CoreSourceSnapshotKey、bootstrap profile、slot receipt和core-slot锁。cache命中时completed origin为CacheHit，重新编译时为Compiled；后续Cone从这个completed node取得artifact路径，不检查core专属origin或固定sysroot slot。

源码、普通manifest语义、依赖fingerprint、compiler或target变化按已有cache key失效；core重建的artifact变化按同一依赖规则传播。旧core slot receipt不迁移、不作为缓存输入，首次新构建产生普通cache entry。旧slot可保留为显式prebuilt输入；没有额外授权或attestation协议。

## 9. scheduler

### 9.1 private状态机

```text
NodeExecutionState =
    Waiting { remaining_dependencies }
  | Ready
  | ResolvingCache
  | RunningChild { request_id }
  | VerifyingOutput
  | Completed(CompletedNode)
  | Failed(BuildFailure)
```

状态只存在于`PreparedBuildGraph::execute`内部。外部不能插入Completed、修改remaining count或取得尚未验证的output path。

### 9.2 serial canonical schedule

M23-4严格遍历`dependency_first`：

- 进入node前断言全部direct dependency已Completed；
- prebuilt node按6.5验证全部snapshot、核对stale edge并构造closure，不查compile cache、不启动child；
- trusted core按6.3/8.6 reuse或bootstrap；
- source/single-file先计算cache key并lookup；
- cache miss启动一次child；
- child output通过第11章验证后发布cache并commit Completed；
- 任一node失败立即终止本次execution；序列中后续node均未启动。

fail-fast位置因此只由canonical order决定。未来允许并行ready set时，可以同时计算，但observable commit、diagnostic选择和child invocation recording必须按同一order缓冲发布；本文首版不实现并行。

### 9.3 child input artifact set

为source node N：

- `--direct-slib`来自`direct(N)`各Completed node的private materialized path；
- `--support-slib`来自`support(N)`；
- 两组均按coordinate排序；
- trusted core通过独立slot path；
- 所有path在spawn前重验regular、private backing和snapshot digest；
- path basename/argument index不承担semantic角色，child仍按artifact自报identity重建闭包；
- root/dependency artifacts来自同一target/profile和本次completed map，不能混入另一个build session的临时文件。

对ordinary manifest snapshot，`current`指向private snapshot root；single-file指向private source copy；core bootstrap不携带普通current path。

### 9.4 output plan

每个child得到独立private output destination：

```text
staging/outputs/<topological-index>-<ConeIdentity>/candidate.slib
```

目录由parent预创建并只授予child必要写权限；output必须不存在，不能与任何input alias。child仍使用M23-3自己的同目录temporary + double-view + atomic rename。parent不允许child直接写最终cache key目录或用户路径。

child成功后parent只读取`candidate.slib`；额外文件、非空dump descriptor、symlink替换、缺失output或wrong type是transport/output error。output path不进入cache key或artifact identity。

### 9.5 M23-4普通dependency门禁

当N具有普通direct dependency时，production child预期在M23-3能力门返回结构化failure：

```text
SCOOPC_CAPABILITY_NON_CORE_DEPENDENCY_UNAVAILABLE
```

orchestrator：

- 将其作为当前阶段合法、可预期但仍使build失败的compiler diagnostic呈现；
- 不把它改写成graph error；
- 不产生N的cache receipt或partial completed node；
- 不启动N的dependent或program-link；
- 可以保留N之前已独立完成并发布的upstream cache entry。

recording runner可以为调度算法返回测试artifact，但test authority不能流入production cache。M23-5删除child能力拒绝后，scheduler、cache、direct/support和artifact gate不变。

### 9.6 warning

- child success warning由response提供，parent验证origin属于当前Cone/source或合法artifact origin；
- newly compiled warning写入receipt；cache hit重放同一structured warning；
- prebuilt dependency不重放其历史编译warning，因为`.slib`不是warning日志；
- root/dependency warning最后按topological index、semantic source、span、code、message排序；
- 后续node失败不丢弃已产生warning，但最终输出先呈现error，再按统一policy附warning；
- human rendering只在orchestration边缘进行，cache和protocol始终保存typed diagnostic。

## 10. 配套child protocol与process边界

### 10.1 配套`scoopc`选择

`PairedScoopcLocator`由当前toolchain installation解析，不查询任意`PATH`。构造`ResolvedPairedScoopc`时验证：

- resolved executable是regular file且位于当前toolchain允许的installation root；
- content digest、build identity、protocol capability、identity/language/runtime ABI与`scoop-toolchain` registry expectation相等；
- canonical target request受双方支持；
- 启动前后executable digest不变；变化报告`PairedCompilerChanged`，本次output不缓存。

build identity不由`--version`自由文本决定。可以使用随toolchain安装的受信任manifest加executable content digest，但最终`PairedCompilerFingerprintV1`必须是typed canonical record。

### 10.2 transport mode

`scoopc`新增隐藏的machine入口，例如逻辑形态：

```text
scoopc --child-protocol 1
```

具体flag spelling不是用户stable CLI，但必须唯一、无其他build参数。入口：

- stdin读取恰好一个M23-3 length-prefixed canonical request frame，EOF后不得有trailing bytes；
- stdout写恰好一个canonical response frame，不写human text、progress或dump正文；
- stderr只用于在无法构造protocol response的process/bootstrap fatal情形，parent按bounded opaque bytes捕获；
- normal compiler diagnostic，包括manifest/source/HIR/codegen/pack失败，都必须成为Failure response；
- orchestrated request固定`emit = None`，Success response必须有空dump descriptor sequence；
- frame length、diagnostic count/text和path继续受`scoop-protocol`及build-wide meter限制。

direct CLI仍归一到同一个`SingleConeBuildRequest`，但human renderer不参与machine mode。

### 10.3 process runner

```text
trait SingleConeCompilerRunner {
    fn invoke(
        &mut self,
        tool: &ResolvedPairedScoopc,
        request: ScoopcRequestEnvelopeV1,
        io: ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError>;
}
```

production实现只由`scoop`内部构造；public orchestration入口不接受任意trait object，以免调用方用fake mint production artifact。recording/fake runner放在`cfg(test)`或sealed test support中。

runner必须：

- 使用显式argv/env/cwd，不继承会改变compiler/toolchain选择的未建模环境；允许继承的locale/color等也不能改变structured bytes；
- 写完request后关闭child stdin；
- 并行drain bounded stdout/stderr，避免pipe deadlock；
- 拒绝oversized/truncated/multiple response frame；
- wait取得真实exit status；
- response request id必须匹配；
- Success要求exit 0，Failure要求约定的compiler-failure status；signal、其他status或response/status矛盾均为transport error；
- 本阶段不定义wall-clock timeout作为语义；外部取消可终止child并返回Cancelled，但不能把partial output/cache当成功。

`request_id`只用于correlation。production可从build nonce与node identity派生不冲突值，test可注入固定generator；它不进入cache/artifact/diagnostic排序。

### 10.4 child failure

有合法Failure response时，parent只增加graph Cone context并稳定排序typed diagnostics，不解析或改写message来判断错误种类。是否为M23-4预期能力门按diagnostic code typed比较。

没有合法response时，`ChildTransportError`至少区分spawn、stdin write、stdout frame、protocol decode、request-id mismatch、exit mismatch、signal和output I/O。bounded stderr作为escaped note，不能当作source diagnostic或执行建议。transport error不查找另一个`scoopc`重试。

### 10.5 request构造

每个child request字段来自单一计划：

| request字段 | 唯一来源 |
| --- | --- |
| `current` | private source snapshot或trusted bootstrap variant |
| `direct_slibs` | resolved direct closure的Completed paths |
| `support_slibs` | resolved support closure的Completed paths |
| `trusted_core` | configured trusted core slot/bootstrap |
| `target` | request起点解析的canonical target |
| `out_slib` | private per-node output plan |
| `diagnostics` | 固定Structured |
| `emit` | M23-4全部固定None |

parent不从child stderr、artifact symbol、package name或path补任何字段。request在spawn前做一次canonical encode/decode round-trip测试并计入protocol budget。

## 11. child output验证与cache commit

### 11.1 response不是authority

Success response中的identity/fingerprint只作快速cross-check。parent必须：

1. 确认private output存在且为regular file；
2. 建立immutable artifact snapshot；
3. 完整运行Compile/Link双视图；
4. 用actual artifact结果核对response全部fingerprint；
5. 再核对resolved graph、source plan、dependency plan和cache key；
6. 构造两份purpose closure；
7. 最后才生成receipt并发布cache。

response匹配但artifact无效仍失败；artifact有效但response字段不符也失败。不能“相信更完整的一方”继续。

### 11.2 plan match矩阵

actual artifact必须满足：

- coordinate/id等于planned node；
- manifest root的kind等于source manifest，single-file为Executable，core为Library；
- source form等于Manifest/SingleFile计划；
- target selection等于requested `lir_target + backend`；
- compatibility的language/runtime/identity/mangler/schema exact match；
- artifact profile是当前阶段允许的strong profile；
- direct dependency集合精确等于graph direct edge加core；
- 每条dependency三层fingerprint等于Completed dependency；
- code/runtime-image fingerprint为profile要求的Available并通过Link proof；
- single-file distribution class仍是local executable root，不可作为dependency；
- root/dependency entry分支与kind一致；
- artifact output没有引用graph外Cone或未完成artifact。

`planned cache key`不直接写进`.slib`。parent通过重新从当前source snapshot、dependency handles和toolchain重算同一key，并把它与receipt/cache destination绑定；actual artifact semantic fields若不匹配上述矩阵，key绑定失败。

### 11.3 source/compiler变化

ordinary child读取private snapshot，原用户tree变化不影响本次artifact；diagnostic display仍可指出原路径并附“build used snapshot”语义，不重新读取内容计算line。构建结束不要求用户tree仍相同；下一次build会得到新source digest与cache key。

paired compiler在spawn后重hash；若变化，即使artifact双视图有效也不发布cache，因为key中的compiler fingerprint可能没有描述实际执行bytes。trusted core按6.3额外重验source key。

### 11.4 commit顺序

成功node固定顺序：

```text
child response validate
  -> output snapshot
  -> Compile view
  -> Link view
  -> response cross-check
  -> graph/dependency/target match
  -> Compile closure
  -> Link closure
  -> private cache entry validate
  -> atomic cache publish
  -> Completed map commit
```

任何一步失败，node保持未完成；dependent不启动。cache publish成功而进程随后在Completed map commit前被外部取消时，entry仍是独立、完整、下次会重新验证的cache value，可以保留。

## 12. 诊断、确定性与失败原子性

### 12.1 phase与排序

orchestrator diagnostic phase固定为：

```text
Request
  -> Toolchain
  -> Root
  -> Locator
  -> Summary
  -> GraphIdentity
  -> GraphVersionKind
  -> GraphCycleOrder
  -> SourceSnapshot
  -> PrebuiltArtifact
  -> Cache
  -> ChildTransport
  -> ChildDiagnostic
  -> ChildOutput
  -> CachePublish
```

同phase内排序key依次为：canonical Cone coordinate、dependent/dependency coordinate、semantic source path或artifact member/wire path、span start、diagnostic code、message bytes。host path只在semantic key相同后作为展示tie-breaker；并发/worker完成顺序永不进入。

graph phase尽量收集互不依赖的多个错误后一次排序返回；execution phase串行fail-fast。untrusted manifest/artifact/child text必须escaped，不能作为format string、terminal control或建议命令执行。

### 12.2 error code family

至少新增：

- locator：`SCOOP_LOCATOR_NOT_FOUND`、`SCOOP_LOCATOR_WRONG_TYPE`、`SCOOP_LOCATOR_COORDINATE_MISMATCH`、`SCOOP_LOCATOR_CONFLICTING_SOURCE`、`SCOOP_LOCATOR_CONFLICTING_REPRESENTATION`、`SCOOP_LOCATOR_AMBIGUOUS_ARTIFACT`；
- graph：`SCOOP_GRAPH_RESERVED_IDENTITY`、`SCOOP_GRAPH_MULTIPLE_VERSIONS`、`SCOOP_GRAPH_EXECUTABLE_DEPENDENCY`、`SCOOP_GRAPH_SINGLE_FILE_DEPENDENCY`、`SCOOP_GRAPH_MISSING_CORE`、`SCOOP_GRAPH_CYCLE`、`SCOOP_GRAPH_UNREACHABLE_NODE`、`SCOOP_GRAPH_RESOURCE_LIMIT`；
- prebuilt：`SCOOP_PREBUILT_SUMMARY_MISMATCH`、`SCOOP_PREBUILT_VIEW_INVALID`、`SCOOP_PREBUILT_STALE_DEPENDENCY`、`SCOOP_PREBUILT_CHANGED`；
- cache：`SCOOP_CACHE_IO`、`SCOOP_CACHE_ENTRY_CORRUPT`、`SCOOP_CACHE_LOCK`、`SCOOP_CACHE_NONDETERMINISTIC_PRODUCTION`、`SCOOP_CACHE_PUBLISH`；
- 默认core位置加载失败沿用`SCOOP_CORE_SLOT_CORRUPT`；core源码编译、缓存及产物失败使用普通Cone对应phase与code，不另设bootstrap/source-change错误族；
- child：`SCOOP_CHILD_TOOL_MISMATCH`、`SCOOP_CHILD_TRANSPORT`、`SCOOP_CHILD_PROTOCOL`、`SCOOP_CHILD_EXIT`、`SCOOP_CHILD_OUTPUT_MISSING`、`SCOOP_CHILD_OUTPUT_PLAN_MISMATCH`、`SCOOP_CHILD_RESPONSE_MISMATCH`。

底层`scoopc`/slib structured code保留原值；orchestrator通过typed nesting增加Cone/phase context，不把所有错误压成一个字符串code。

### 12.3 failure原子性

- graph失败：无child、无cache write、无core slot write；
- source/artifact snapshot或summary复核失败：同上；private snapshot由guard清理；
- core child遵守与其他源码Cone相同的失败原子性；原始core目录变化不影响已经捕获的本次源码快照，下次构建按新快照重新计算cache key；
- cache lookup失败：不启动child覆盖corrupt exact-key entry；
- child failure/transport failure：无node receipt、无Completed commit；child private output不是cache；
- output验证失败：无cache publish、无Completed commit；
- cache publish失败：artifact虽有效但node不commit，避免dependent收到一个无法稳定引用的transport path；
- root失败：不产生`BuildGraphOutcome`；
- 任意失败都不运行runtime-build/program-link或用户程序。

private staging cleanup只能删除本次以create-new建立且由guard持有的精确目录；不能递归删除cache root、sysroot、用户root或未验证path。

### 12.4 可复现性

下列变化不得改变graph、cache key、child语义request或artifact bytes：

- `Cone.toml` dependency table枚举顺序；
- source directory枚举顺序；
- `--cone-path`顺序在所有candidate内容相同时的排列；
- relative/absolute/symlink locator spelling最终指向同一source root/artifact；
- cache root、staging root、cwd和host用户名；
- child request id；
- warning human renderer/color/locale；
- future worker完成顺序。

manifest whitespace/comment改变不会改变normalized semantic cache字段；source bytes改变必然改变source digest/key。locator改变但最终resolved content claim相同不使被编译Cone失效；若改变node representation/content则在claim或key处显式体现。

## 13. 测试设计

### 13.1 locator与summary

- `path`相对manifest、absolute、symlink alias与coordinate mismatch；
- `artifact` missing/wrong type/dangling/cycle/coordinate mismatch；
- search-root零、一、多个相同fingerprint、多个不同fingerprint；
- group/name中的`.`不拆目录；
- candidate顺序变化不改变结果；
- source/artifact representation冲突、不同source root冲突；
- artifact dependency record由已有claim满足及无claim时走search root；
- summary合法但member损坏，证明preflight不会把summary提升为artifact；
- summary与snapshot重读不一致的TOCTOU negative。

### 13.2 graph

- root-only/core、chain、diamond、宽ready set；
- self cycle、two-node cycle、多个SCC，断言canonical cycle path；
- 同`group:name`多version；
- executable dependency、single-file dependency、reserved identity伪造；
- 缺/重复core edge；
- unreachable injected node；
- manifest枚举、map/hash seed和edge insertion全排列下order一致；
- direct/support closure在chain/diamond/sibling图中的精确集合；
- node/edge/depth/candidate各limit的边界值与超限值。

### 13.3 snapshot与TOCTOU

- source symlink在read中改变、artifact在copy中改变、manifest target替换；
- 用户source在snapshot后改变，child仍编译snapshot bytes且下一次build key改变；
- private snapshot没有复制locator target、旁文件、native source或symlink；
- normalized path escape、duplicate与create-new collision；
- child input只引用private artifact snapshot，不引用用户/cache可替换path；
- 修改core原始源码不改变正在编译的快照；下一次build按普通key重建。

### 13.4 dual view与closure

- Compile成功/Link失败不能完成；Link成功/Compile失败不能完成；
- Graph-only/summary-only类型无法调用completed构造器的compile-fail测试；
- same bytes两种purpose独立decode；
- missing/extra/stale direct edge、transitive mismatch、wrong target/profile；
- diamond只保存一份snapshot且closure只含一份identity；
- Compile closure不能传入Link API，反向同理；
- scoped view不能逃逸snapshot lifetime的compile-fail测试。

### 13.5 cache key

fixed vector锁定`ConeCompileCacheKeyV1`的domain、field tag与排序；分别改变：

- source bytes；
- manifest coordinate/kind/dependency semantic；
- direct HIR/MIR/LIR fingerprint；
- single-file core Code fingerprint；
- compiler executable digest；
- language/runtime/identity/mangler/schema/profile；
- lir target、backend、C bridge toolchain；

M23-4没有自由`semantic option`字段，因此不构造一个虚假的option mutation；未来首次增加optimization或language mode时必须升级cache input schema，并同时新增该字段的独立miss测试。

上述每项必须miss。只改变locator spelling、artifact optional attachment、artifact whole fingerprint而三层/所需code不变、cache/output/staging path、mtime、request id、diagnostic policy或runtime-build/final-link projection时，key必须不变。

### 13.6 cache存储

- miss→child→private validate→atomic publish→hit；
- exact key下缺receipt、缺artifact、symlink、truncated、wrong key/fingerprint/dependency均报corrupt，不覆盖；
- 两个writer竞争产生相同artifact时复用winner；不同artifact时报nondeterministic；
- child失败、output invalid、receipt encode失败、rename失败均无partial visible entry；
- warning在fresh build与cache hit一致重放；
- full 32-byte key目录、防缩写collision；
- core与用户Cone同样验证cache hit并重放warnings，不使用专用slot。

### 13.7 child protocol

- request/response canonical frame fixed vector；
- truncated/oversized/multiple frame、trailing bytes、wrong request id；
- success+nonzero、failure+zero、signal/no response；
- stdout被human text污染；
- bounded stderr capture；
- arbitrary`PATH`中假`scoopc`不被选择；
- paired executable在spawn前后变化；
- direct/support参数顺序稳定且child再验证角色；
- response fingerprint与actual artifact各字段不一致矩阵。

### 13.8 recording scheduler

sealed recording runner记录`ScoopcInvocation`并返回test-only completed artifacts，覆盖：

- chain严格dependency-first；
- diamond公共dependency只调用一次；
- cache hit不调用child；
- prebuilt不调用child；
- 上游failure后dependent和后续node不调用；
- manifest枚举和模拟future完成顺序不改变observable invocation/diagnostic order；
- 所有orchestration child的emit固定None且response不含dump；
- request id变化不改变其余request/cache结果。

### 13.9 真实process集成

- trusted core slot已有fresh artifact时复用；缺失/stale时bootstrap一次；
- core-only manifest library：手工`scoopc`与orchestrator root artifact逐byte相同；
- core-only manifest executable同上；
- single-file同上且graph精确两个node；
- source root依赖另一个core-only library：先成功编译/cache上游，再在root得到M23-3非core能力诊断，root无cache entry；
- prebuilt/cache Link object损坏在该node完成前失败，且不启动任何dependent child；
- child成功后parent再次双视图验证；
- 第二次build零child且artifact/warning一致；
- source/core/toolchain/dependency fingerprint分别变化时只使正确节点及dependent miss；
- 全部临时/缓存路径变化不改变`.slib` bytes。

### 13.10 regression与依赖边界

- M23-3全部manifest/protocol/core/artifact/reader测试回归；
- Cargo metadata测试断言`scoop`不依赖`scoopc` lib和任何parser/lower/codegen implementation；
- `scoopc`不依赖`scoop`；
- locator/cache模块不能导入HIR/MIR/LIR implementation arena；
- production API无法构造fake runner/fake artifact authority；
- 全workspace format、clippy、test通过。

## 14. 实现顺序

1. 抽出`compiler/toolchain`，迁移唯一target registry与paired compiler identity，不改变现有`scoopc`行为；
2. 在`scoop-slib`实现bounded summary probe、closure limits/meter和immutable snapshot上的dual-view certificate；
3. 新增`compiler/scoop` request/root/toolchain normalization与crate dependency boundary test；
4. 实现manifest/artifact/search-root locator、claim收敛及negative矩阵；
5. 实现resolved graph、reserved/kind/version/cycle验证、canonical Kahn和direct/support projection；
6. 实现source/artifact snapshot、private materialization、全图preflight与TOCTOU测试；
7. 实现purpose-preserving artifact closure和prebuilt stale-edge验证；
8. 实现`ConeCompileCacheKeyV1` fixed vectors、receipt、lookup、per-key lock和atomic directory publication；
9. 在`scoopc`接入隐藏protocol transport入口，并实现production process runner；
10. 实现serial scheduler、child request构造、response/output/plan交叉验证和warning汇总；
11. 实现trusted core slot freshness/bootstrap调度；
12. 补齐recording chain/diamond/failure测试、真实core-only process集成及可复现性测试；
13. 更新M23总设计与ROADMAP链接，审计M23-5交接面。

每完成一批代码变更，先运行`cargo fmt --all`与`cargo clippy --workspace`，再运行该批unit/integration/golden；不能把format/lint拖到最后。涉及cache/filesystem/process的测试使用私有temp root和显式fake clock/nonce，不读取开发机全局cache或`PATH`。

## 15. 完成门

M23-4只有同时满足以下条件才完成：

- `compiler/scoop` orchestration library已落地，且Cargo依赖上无法调用`scoopc`内部pipeline或任一lower/codegen implementation；
- target/toolchain registry只有一份共享实现，parent与child对canonical target得到相同projection；
- manifest `path`/`artifact`/search-root及artifact dependency record locator全部exact、bounded、确定性工作；
- source/artifact claim冲突、ambiguous candidate、coordinate mismatch、reserved identity、multiversion、kind错误与cycle全部在第一个compiler child前失败；
- `ResolvedBuildGraph`与artifact Graph/Compile/Link proof类型上分离，summary/Graph-only无法成为completed node；
- canonical dependency-first order、direct/support closure和diagnostic order不受manifest枚举、hash seed、path spelling或future concurrency影响；
- 全部source在child前形成immutable snapshot，普通child只读取private snapshot；artifact child input也来自immutable private backing；
- `SlibClosureDecodeLimitsV1`覆盖全图physical bytes、decode/work和protocol资源，diamond/重复path不能重复获得预算；
- prebuilt、cache和child output都分别通过Compile/Link完整验证，dependency/target/profile/response/plan逐项相等后才completed；
- purpose-specific closure无擦除/cast，library或executable root成功时同时保留Compile与Link closure；
- cache key由normalized semantic/source/dependency/compiler/ABI/profile/实际消费toolchain字段唯一计算，locator/path/mtime/diagnostic不污染；single-file包含core code fingerprint；
- cache exact-key corruption稳定失败，entry以per-key lock和atomic directory rename发布，竞争不同结果报告nondeterminism且不覆盖；
- trusted core只由sysroot authority和slot bootstrap产生，ordinary cache不能提升core authority；core source变化、slot损坏和bootstrap失败均原子处理；
- 每个source miss恰好启动一次配套`scoopc` child；不用`PATH`挑版本，不解析human stderr，不传AST/IR，不让childfollow locator；
- child失败不启动dependent，成功response不替代parent artifact验证，partial output永不进入cache/completed set；
- M23-4真实成功子集只包括core bootstrap/core-only manifest/single-file；普通dependency仍由M23-3能力门结束，没有名称语义偷跑；
- core-only library、core-only executable和single-file经手工`scoopc`与orchestrator产生逐byte相同artifact；第二次build可由验证后的cache零child复用；
- chain/diamond/cycle/multiversion/ambiguous/stale/cache invalidation/child failure/TOCTOU/resource limit/确定性矩阵完整；
- `cargo fmt --all`、`cargo clippy --workspace`和完整`cargo test`通过。

到达该完成门后，M23-5只需要在`scoopc`中把已经验证的direct/support Compile closure接入`SemanticWorld`并删除非core能力拒绝；它不需要修改locator、DAG、cache、child transport、snapshot或双视图完成条件。M23-9以后也可以直接消费保留的Link closure，而不重新读取source manifest或cache receipt。

# M23-2 设计：persistent identity 与 `.slib` wire 基础

版本：0.1（设计完成，待实现；2026-09-09）

依赖：M23-1

上位设计：`docs/milestone23/DESIGN.md`

规范依据：

- `docs/specs/SCOOP-SPEC.md` 第 12.2、12.5 节；
- `docs/specs/SCOOP-IMPL-SPEC.md` 第 2.1、2.6、2.11 节；
- `docs/specs/SCOOP-RUNTIME-SPEC.md` 第 2.2、2.8 节中关于 identity、canonical encoder 与登记 key 的契约；
- `docs/milestone23/DESIGN.md` 第 0、1.1、1.3、3.1～3.3、3.5、4、5.1、5.3、9.3、10 章。

本文只定义 M23-2。语言与最终 artifact 语义仍以上述规范和 M23 总设计为准；本文负责把 M23-2 必须冻结、而总设计只给出原则的字节级 schema、typed API 与阶段门禁补全。

## 0. 结论

M23-2 建立的是所有后续多 Cone 能力共同依赖的可信 wire 地基，不是一个提前缩水的 M23 最终 artifact。完成本阶段后：

1. Cone、source/context、declaration/local binding/value、exact type、callable application、initialization unit、machine callable body、callback registration/application、native contract与safepoint site都有不依赖arena、枚举顺序、FQN、symbol或host path的canonical identity；不同语义kind在Rust API和wire table中都不可混用；
2. 当前 pipeline 中全部 Scoop-owned linker-visible definition 原子切换到 `PersistentV1` mangler；`@Extern` native symbol 与固定 runtime C ABI symbol仍是明确例外；
3. `.slib` v1 的 canonical CBOR、bootstrap manifest、typed member directory、normal self-contained deterministic `ar`、member/artifact fingerprint 与资源预算完成并由固定向量锁定；
4. HIR/MIR/LIR metadata 使用共同的 versioned outer envelope，但各层 payload 和 imported type 仍由各自 IR/meta crate 所有；后续阶段只能增加 versioned section/capability，不能回改本阶段 tag；
5. reader 形成以 `DecodedSlibEnvelope → ValidatedGraphArtifact` 为共享主干、Compile与未来Link从Graph分叉的 typed proof DAG；Compile分支按“wire decode → structural validation → typed remap → atomic commit”工作；
6. member envelope 从第一天起允许零个、一个或任意多个 `LinkObject`，以及任意 diagnostic attachment、optional blob 与 Link-required blob；Graph/Compile 只验证未识别 Link payload 的 envelope/hash并保持 opaque；
7. 本阶段没有 object/image verifier，也没有能够构造 `ValidatedLinkArtifact` 的公开入口。M23-2 的 foundation `.slib` 只用于 wire/identity/reader 验证，不能发布、缓存为 Cone dependency、交给 native linker或被描述成 M23 可发布 artifact；
8. M23-1 的 `SourceFile` header AST 不变；request-local `Stage1SourceHandle` 在 parser 输出边界被 `SourceIdentity { cone, logical_path }`替换，display locator继续只作本次诊断装饰。

M23-2 不通过把当前 Rust `Module` 直接 serde 化来换取“能 round-trip”的表象。wire只保存本阶段已定义且结构完备的 identity/source foundation projection；public lookup、inheritance、generic hidden closure、跨 Cone layout、ODR definition、image 与 native link verification由后续子里程碑以独立required section加入。当前generic body已经使用的ODR group/member **identity** 则必须在本阶段冻结并实际产生。

## 1. 范围与阶段边界

### 1.1 本阶段交付

- 共享的 canonical CBOR、hash framing、bounded decode 与 typed 32-byte identity基础；
- `ConeCoordinate` grammar、`ConeIdentity`、reserved core/single-file coordinate；
- `SourceIdentity`、canonical semantic source name、source table基础记录及 display locator 分离；
- source declaration/context/local binding/value、derived entity、exact type、initialization unit、callable application/body、callback registration/application、source/native extern contract与safepoint site的persistent key/id schema；
- `RuntimeTypeId`与`SafepointId`的确定性 nonzero `u64`派生和碰撞检查；
- `CanonicalExactTypeDiagnosticName` printer；
- `PersistentV1` mangler、symbol kind registry与linkage分类；
- HIR/MIR/LIR identity foundation projection及其 outer metadata envelope；
- `.slib` bootstrap manifest v1、manifest section envelope、member stable-key/role/capability/purpose；
- deterministic CBOR与canonical normal `ar` writer/reader；
- `MemberFingerprint`、`LinkMemberFingerprint`、三层foundation semantic fingerprint与`ArtifactFingerprint`；
- raw envelope、单 artifact Graph proof、单 artifact Compile proof与session-local typed remap；
- 集中的单 artifact decode budget、结构化错误与失败原子性；
- fixed hash/wire/ar golden、round-trip、corruption、budget、capability、顺序/路径可复现测试；
- 当前 M1～M23-1 pipeline 对 persistent source/runtime/symbol identity 的迁移与全量回归。

### 1.2 本阶段明确不做

- `Cone.toml`正式 semantic projection、manifest source discovery、symlink containment 与 library/executable entry检查；
- `scoopc build`的single-Cone child protocol、`--direct-slib`/`--support-slib`/`--out-slib`；
- core独立编译、trusted core artifact authority、core prelude metadata消费；
- locator、resolved DAG、cache、dependency-first调度或 `ValidatedArtifactClosure<P>`；M23-2只证明单个 artifact，不把一项 proof 命名成 closure proof；
- 跨 Cone import/re-export、access provenance、default/template、alias或split-package lookup；
- 最终 Export HIR public/inheritance/hidden support payload、MIR dispatch bridge或**通用、可跨Cone复用**的LIR layout/Scoop ABI/scan payload；
- 一般ZST materialization/跨Cone ABI、generic concretization、ODR完整member闭包、definition fingerprint、object materialization与native coalescing；本阶段只冻结现有generic body已经需要的specialization/group/member identity key；
- `ScoopImageDescriptorV1` producer/consumer、六类registration完整记录或runtime multi-image启动；
- Scoop/generated bridge `LinkObject`内容验证、Mach-O section/symbol/relocation验证、image-owner唯一性；
- `ValidatedLinkArtifact`、runtime-build、program descriptor、program-link、native requirement/provider/evidence；
- 正式 `scoop build/run/link`或历史fixture runner切换；
- 发布任何由本阶段foundation writer生成的`.slib`。

本阶段可以把任意bytes登记为带typed role的`LinkObject`或blob，以验证container和purpose行为；它绝不能根据magic、扩展名或内容探测把bytes提升成verified object。

唯一窄例外是第5.4节`NativeBoundaryTypeDefinitionRecordV1`：它只为当前extern/callback contract的传递类型闭包提供足以重算C storage/layout与Scoop physical signature的source witness，不携带scan、dispatch或一般consumer lookup，也不从Compile API暴露为layout服务。M23-6仍首次提供可跨Cone复用的完整layout/scan/typed-ABI proof。

### 1.3 与相邻阶段的交接

| 阶段 | M23-2 接收/保留 | 交付给该阶段 |
| --- | --- | --- |
| M23-1 | 已冻结的`SourceFile` package/import header与当前单元语义 | 用persistent source identity替换request-local handle，不修改AST grammar |
| M23-3 | 当前legacy pipeline与空壳`compiler/slib` | stable identity、container、manifest/member envelope、Graph/Compile reader；由M23-3补strong-only production writer、object/image verifier与Link proof，并拒绝任何ODR production input |
| M23-5 | HIR identity foundation section | 以新required HIR section加入public/re-export/access/default payload，不改identity key或基础section |
| M23-6 | exact/layout id、native-boundary witness与LIR foundation section | 以新required MIR/LIR section加入通用、可跨Cone复用的layout/ABI/scan/dispatch proof；不改exact type identity或第5.4节已冻结的extern/callback contract bytes |
| M23-7 | 已冻结的specialization/group/member identity、callable-body ODR引用与mangle role | 补齐ODR完整member闭包、ABI/definition fingerprint、object materialization与跨Cone member-set/definition一致性验证；不能改identity key或PersistentV1语法 |
| M23-8～10 | typed member/purpose/capability、未来Link门禁位置 | 分别加入runtime/image/link/native verifier capability，不增加第二条raw object入口 |
| M23-11 | 明确隔离的legacy executable shim | 删除legacy shim并让正式single-file orchestration成为唯一fixture路径 |
| M24 | container v1、identity schema v1、PersistentV1，以及预留的`ReleaseHook=16` ODR member role | 三层wire schema与发生变化的foundation capability/profile major整体升为2、callable-body domain升为v2；旧v1 artifact整体重建 |

### 1.4 版本时序

M23-2 实现历史顺序上的 M23 baseline，不能把已经写入最终规范的未来 M24 变更倒灌进 v1：

| 维度 | M23-2 / M23 baseline | M24完成后 |
| --- | --- | --- |
| `.slib` container schema | 1 | 仍为1 |
| bootstrap manifest schema | 1 | 仍为1 |
| persistent identity schema | 1 | 仍为1 |
| mangler schema | `persistent-v1` | 不变 |
| HIR/MIR/LIR outer wire schema | 1/1/1 | 2/2/2，三层必须一致升级 |
| HIR/MIR/LIR identity-foundation capability major | 1/1/1 | 2/2/2；引用它们的artifact profile也使用新major |
| callable body identity | `scoop-callable-body-v1`，4个variant | `scoop-callable-body-v2`，增加`ReleaseHook` |
| runtime metadata ABI prefix | 1 | 仍为1；TypeDescriptor/runtime fingerprint另行变化 |

M24 的升级是规范已经声明的整体不兼容重建，不构成修改 M23-2 frozen tag 的先例。M23 artifact中不能混入body-v2，M24 artifact中也不能保留任何body-v1。outer schema升代不能代替capability major升代：payload shape发生变化的`org.scoop-lang.{hir,mir,lir}/identity-foundation/1`必须分别换成`.../2`，引用这些capability的profile也必须换新id/major，旧handler绝不能用“outer schema已经是2”为理由解释新payload。

`PersistentCallableApplicationId`与`OdrGroupId`不含body schema，M24保持不变；primary callable member若以CallableApplication/GeneratedCallable作discriminator也保持其identity。`PersistentCallableBodyId`、其下safepoint site，以及以CallableBody/SafepointSite为discriminator的派生`OdrMemberId`、registration symbol/member set与ODR fingerprint则随body-v2整体重生，不能笼统声称所有ODR member id跨M24不变。

## 2. crate与依赖方向

### 2.1 两个共享基础crate

新增两个职责单一的基础crate，避免让容器、编码机制与语义identity变成一个相互回调的大模块：

```text
scoop-wire
  cbor/           deterministic CBOR与strict decoder
  digest/         Digest256、hash framing
  budget/         可注入的累计budget meter与structured path

scoop-identity -> scoop-wire
  cone/           ConeCoordinate、ConeIdentity
  source/         SourceIdentity、NormalizedSourcePath
  entity/         kind-specific persistent ids/keys
  exact_type/     exact type graph与diagnostic printer
  callable/       callable body、safepoint、runtime id
  mangling/       ManglingSchemaIdentity、PersistentV1 request

AST -> scoop-identity
HIR / MIR / LIR -> scoop-identity + scoop-wire
scoop-slib -> scoop-wire + scoop-identity + HIR + MIR + LIR
```

`scoop-wire`不认识Cone、entity或IR；它只实现本文第3章的codec、digest、checked scalar和累计预算。`scoop-identity`只依赖`scoop-wire`，不依赖AST/HIR/MIR/LIR、任一lowerer、codegen、driver或filesystem。它也不提供“所有实体共用”的public `PersistentEntityId`。允许内部用macro生成布局相同的newtype，public API仍是互不转换的具体类型。

workspace新增`compiler/wire`（`scoop-wire`）与`compiler/identity`（`scoop-identity`）两个member。HIR/MIR/LIR各自在IR crate中拥有`persistent.rs`、`wire_v1/`与`imported.rs`；它们为实现自己拥有的wire DTO而直接依赖`scoop-wire`，并为typed identity直接依赖`scoop-identity`。不另建一个依赖lowerer的serializer crate，也不让`scoop-slib`扫描arena来重建语义。`compiler/slib`是唯一打开、写入`.slib`容器的crate；它直接依赖这两个基础crate和HIR/MIR/LIR，但不得依赖parser、任一lowerer、codegen或driver，只编排各IR crate的typed encoder/decoder/validator入口。

### 2.2 stage通信边界

M23-2 后的相关边界为：

```text
parser:
    IdentifiedSourceInput -> IdentifiedParsedSource

hir-lower:
    AllIdentifiedParsedSources -> HIR + HirIdentityFoundation

mir-lower:
    HIR(with persistent origins) -> MIR + MirIdentityFoundation

lir-lower:
    MIR(with persistent origins) -> LIR + LirIdentityFoundation

slib-foundation:
    GraphManifestInput
      + HirIdentityFoundation
      + MirIdentityFoundation
      + LirIdentityFoundation
      + TypedOpaqueMembers
    -> foundation `.slib`

slib-read:
    bytes + SlibDecodeLimits
    -> DecodedSlibEnvelope
    -> ValidatedGraphArtifact
    -> ValidatedCompileArtifact<IdentityFoundationProfile>
```

foundation writer是M23-2测试与迁移工具，不接入当前driver的production输出，也不返回`PublishableArtifact`。M23-3将以同一低层writer接收完整single-Cone产物，并在Compile、Link两种proof均成功后才创建可发布类型。

### 2.3 所有权与不变量

- canonical codec只在`scoop-wire`定义一份，persistent id/key与source identity只在`scoop-identity`定义一份；capability的wire原子由`scoop-wire`所有，语义refinement由定义它的IR/容器模块所有；HIR/MIR/LIR不得各复制一套tag或hash函数；
- HIR/MIR/LIR foundation payload分别定义在对应IR crate；它们不得引用lowerer内部arena或implementation-only type；
- `SlibMemberId`、member record、manifest与validated view定义在`scoop-slib`；IR semantic payload不得引用物理archive ordinal/name；
- current source entity、imported entity、LocalConcrete entity、MIR/LIR local arena id继续是不同family。persistent id是跨边界origin，不取代stage-local id；
- symbol字符串是persistent owner的机械投影，不得反向解析成entity id，也不得被lowerer当作缺失typed relation的替代物。

## 3. canonical bytes与hash基础

### 3.1 三种编码不可混用

M23同时存在三种目的不同的canonical encoding：

1. **Wire CBOR v1**：identity key、manifest、member record及HIR/MIR/LIR metadata使用的RFC 8949 deterministic CBOR子集；
2. **Hash framing v1**：在SHA-256输入中分隔domain和可变长片段；
3. **Runtime metadata canonical encoder v1**：总设计6.1定义的`u32/u64 little-endian + count + product/sum`编码，仅用于callable body、runtime registration和后续runtime可重算key。

同一个逻辑record只能由其规范指定的一种encoder编码。不得因为三者都“canonical”而把CBOR bytes送入runtime hash，或把Rust/C结构内存当作任一种canonical bytes。

### 3.2 Wire CBOR v1

Wire CBOR v1固定如下：

- 只使用schema明确允许的unsigned integer、byte string、UTF-8 text string、definite-length array与definite-length map；
- unsigned integer和长度使用RFC 8949最短编码；禁止indefinite item、float、tag、simple value及schema未声明的negative integer/null/bool；
- product编码为只含已声明positive integer key的map，field key从1开始；
- sum编码为map，key `0`保存非零variant tag，其余payload字段从1开始；
- 所有variant都无payload的C-like closed enum直接编码为一个CBOR unsigned nonzero tag；只要该类型任一variant有payload，整个algebraic sum（包括其中无payload的variant）都使用上一项的`{0: tag, ...}` map，不能由调用点在raw integer与map之间任选；
- digest与所有32-byte typed id编码为恰好32-byte byte string；不同typed field虽有同样wire宽度，decoder入口仍不同；
- map key按canonical encoded key顺序排列。本文product/sum的field key都是正整数；对RFC 8949最短编码的非负整数，这一顺序等价于数值严格递增，包括24及以上需要额外参数byte的key；
- array只在语言语义要求保序时保留原顺序；语义set/table必须先按本文指定typed key严格排序并拒绝重复；
- text按输入UTF-8 bytes原样编码，不trim、不做Unicode normalization或大小写折叠；
- required closed record遇到缺失、重复、额外field或unknown tag一律失败；可扩展性只通过本文定义的capability section/blob envelope提供；
- known payload decoder必须证明输入已经canonical；`decode → encode`的bytes不等于原bytes即为`NonCanonicalCbor`，不能静默正规化后继续。

实现应基于成熟CBOR与SHA-256库，并在其上建立上述受限schema/validator；不手写通用CBOR parser，也不使用会暴露Rust enum ordinal、field name或map迭代顺序的derive序列化作为协议。

### 3.3 Hash framing v1

```text
ByteSpan(bytes) = little_endian_u64(bytes.len) || bytes

DomainSeparatedCborHash(domain, value) =
    SHA-256(ByteSpan(ASCII(domain)) || WireCborV1(value))
```

长度转换先做checked `u64`。domain不带NUL，大小写与连字符逐byte固定；公式中的字符串字面量均指其ASCII bytes。typed 32-byte id在某个公式中标为`raw(id)`时直接写32 bytes。`DomainSeparatedCborHash`中紧随唯一domain的`WireCborV1(value)`已经是自定界canonical item，故不再套`ByteSpan`；其他复合hash stream是否包裹CBOR或固定宽度digest，以其显式公式为准。凡未由公式证明边界的可变长raw片段都必须写`ByteSpan`，不能靠delimiter拼接。

本阶段固定的主要公式为：

```text
ConeIdentity =
    DomainSeparatedCborHash("scoop-cone-id-v1", ConeCoordinate)

PersistentKId =
    DomainSeparatedCborHash(domain(K), CanonicalKeyK)

SlibMemberId =
    SHA-256(ByteSpan("scoop-slib-member-v1")
           || raw(ConeIdentity)
           || WireCborV1(MemberStableKey))

MemberFingerprint =
    DomainSeparatedCborHash("scoop-slib-member-content-v1",
                            SlibMemberRecord)

LinkMemberFingerprint =
    DomainSeparatedCborHash("scoop-slib-link-member-v1",
                            SlibMemberRecord)
```

`LinkMemberFingerprint`构造器只接受`LinkObject`或`ExtensionBlob { required_for = Link }`的refined record；其他role在类型检查前即没有调用入口。

### 3.4 Digest与typed id API

`Digest256`只表示content/hash结果，不是semantic entity id；`ConeIdentity`以及第5.2、6章列出的每一种persistent identity各自都是独立newtype，不存在公开的可擦除总ID。

每个newtype内部可为`[u8; 32]`，但：

- semantic id不实现从`Digest256`或另一persistent id的safe转换；
- public producer只能从对应canonical key构造；raw-byte constructor限于typed decoder模块并立即伴随key重算；
- public只读API可返回`&[u8; 32]`用于排序、显示hex和hash输入；
- `Display`只输出固定lowercase 64-hex诊断文本，不产生source name或FQN；
- 排序一律按原始32 bytes lexicographic，不按hex string或host整数解释；
- hash碰撞检查比较完整typed key。相同id/相同key去重，相同id/不同key是确定性artifact错误；不同kind即使raw bytes相同也不是同一id。

## 4. Cone与source identity

### 4.1 `ConeCoordinate`

`ConeCoordinate`的Wire CBOR v1是map：

| field | 含义 |
| ---: | --- |
| 1 | `group: text` |
| 2 | `name: text` |
| 3 | `version: text` |

`group`/`name`各由一个或多个`.`分隔segment构成，每段匹配`[a-z][a-z0-9-]*`；总文本必须是ASCII。`version`必须逐byte等于其SemVer 2.0.0 parser的canonical输出；禁止`v`前缀、前导零、trim或大小写正规化，合法pre-release/build metadata原样保留。

canonical display唯一为`group:name:version`，只用于诊断和稳定显示；identity始终使用第3.3节CBOR hash，不能hash display string。

本阶段内建并测试两个reserved coordinate：

```text
scoop:scoop.core:0.1.0
scoop:single-file:0.0.0
```

reserved性质来自compiler/tool authority，不来自artifact自报布尔值。M23-2只冻结值和identity；trusted core slot验证留M23-3。

### 4.2 `SourceIdentity`

`SourceIdentity`本身就是持久source identity的canonical product，不再额外制造一个与它竞争的`PersistentSourceId`：

```text
SourceIdentity {
    cone: ConeIdentity,
    logical_path: NormalizedSourcePath,
}
```

Wire CBOR map固定为`1=cone`、`2=logical_path`。`NormalizedSourcePath`是UTF-8相对路径，使用`/`分隔；禁止absolute/root/prefix、空segment、`.`、`..`、NUL与U+005C backslash。producer从filesystem component构造而不是对任意host path做字符串替换；reader只接受已经是canonical form的text。

manifest Cone path包含`src/`前缀，例如`src/model/User.scoop`。single-file path严格为`main.scoop`。文件扩展名/source discovery约束属于M23-3，M23-2 validator只负责canonical path形状及single-file reserved pair的一致性。

canonical semantic source name为：

```text
<canonical ConeCoordinate>/<logical_path>
```

它用于structured diagnostic、`SourceLocation.file`和source table显示，但不被重新hash成另一身份。得到coordinate必须经同一artifact/world中已验证的`ConeIdentity → ConeCoordinate`记录，不能从路径或package反推。

### 4.3 source输入与M23-1迁移

```text
IdentifiedSourceInput {
    identity: SourceIdentity,
    text: SourceText,
}

IdentifiedParsedSource {
    identity: SourceIdentity,
    ast: SourceFile,
}

ParserDiagnosticContext {
    display_locator_by_source: Map<SourceIdentity, SourceDisplayLocator>,
}
```

`ParserDiagnosticContext`是正交request sidecar，不是stable parser input的field。display locator不进入AST、HIR、persistent key、wire、fingerprint或semantic diagnostic排序。parser可以在一次调用内部保留request token来关联buffer，但该token不得越过`IdentifiedParsedSource`边界。

由于manifest discovery尚未实现，本阶段只允许调用方显式提供已经验证的identity：

- legacy core输入使用reserved core identity和显式`src/<file>`logical path；
- 当前单用户文件使用reserved single-file identity和固定`main.scoop`；
- parser/unit测试使用显式test coordinate，不能从temp path、cwd或file index派生。

legacy core和user仍暂时进入同一次旧pipeline，但其source origin已经不同。这个混合入口封装为`LegacyCombinedSources`，不能传给foundation `.slib` writer；M23-3以`CurrentConeParsedSources`取代它并从类型上要求所有source具有同一Cone identity。

`AllParsedSources`继续保证非空和source identity唯一。两个不同display path若映射到同一`SourceIdentity`必须在HIR前失败；同一display path指向不同identity不构成semantic相等。

### 4.4 `SourceRecordV1`

HIR foundation source table中的record固定为：

| field | 含义 |
| ---: | --- |
| 1 | `SourceIdentity` |
| 2 | UTF-8 source byte length `u64` |
| 3 | `SourceContentDigest = SHA-256(raw UTF-8 source bytes)` |
| 4 | strictly increasing line-start byte offsets `array<u64>` |
| 5 | strictly increasing `SourcePointRecordV1` table |

`SourcePointRecordV1`的field为`1=byte_offset:u64, 2=line:u64, 3=column:u64`，必须覆盖所有进入required metadata的location以及每个serialized span的start/end offset，按offset严格递增；同一offset只保存一条point。line/column都是1-based，column按UTF-8解码后的Unicode scalar value计数；CR在CRLF中是前一行的一个scalar，LF开始新行。producer持有source text时必须证明line-start第一项为0、其余项恰为每个换行后的byte offset，point不落在UTF-8 continuation byte且line/column均可由source重算。

完整source缺省不进入required metadata；若diagnostic attachment包含source，Diagnostics handler必须重算digest、length、line starts、UTF-8边界及每个point的scalar column并逐项相等。无source attachment时，reader只能验证line-start/point严格递增且落在`0..=byte_length`、point的line落入已声明line-start区间、column大于0，以及每个required location/span endpoint都有point；它不能仅凭digest证明UTF-8边界、CRLF或scalar column。后面三项由已验证producer的semantic payload承诺，并在source attachment可用时加强验证。span必须满足`start <= end <= byte_length`，`current_source_location`只读已固化point，不在consumer扫描不存在的source text。

当前Cone编译期另有不序列化的`CurrentSourceTextTable { SourceIdentity -> SourceText }`，HIR lowering/concretization用它一次性计算point。trusted core authority同样是compile-session的`ConeIdentity -> IntrinsicAuthority`关系，不是`SourceRecordV1`字段。这两个side table都由driver显式传入，不靠HIR中的full source或provider整数补猜。

source table按`(ConeIdentity raw bytes, logical_path UTF-8 bytes)`严格排序并拒绝重复。host absolute path、inode、mtime、创建顺序和display locator不存在于record中。

### 4.5 typed source origin

request-local `provider + file:u32 + SourceContextId`不能进入stable HIR。source context本身使用可重算identity：

```text
SourceContextKey =
    File { source: SourceIdentity }                                  // tag 1
  | Nominal { source: SourceIdentity,
              owner: NominalDeclarationOwner }                    // tag 2
  | Callable { source: SourceIdentity, owner: CallableOwner }      // tag 3
  | Property { source: SourceIdentity, owner: PropertyOwner }      // tag 4
  | Initialization { source: SourceIdentity,
                     unit: PersistentInitializationUnitId }          // tag 5

PersistentSourceContextId =
    DomainSeparatedCborHash("scoop-source-context-id-v1", key)

SourceSpan { start_byte: u64, end_byte: u64 } // fields 1/2

DefinitionOrigin {
    source: SourceIdentity,                 // field 1
    span: SourceSpan,                     // field 2
    context: PersistentSourceContextId,     // field 3
}

EvaluationOrigin {
    source: SourceIdentity,                 // field 1
    span: SourceSpan,                     // field 2
    context: PersistentSourceContextId,     // field 3
}

ConcreteExpressionOrigin {
    definition: DefinitionOrigin,         // field 1
    evaluation: EvaluationOrigin,         // field 2
}

ExpressionOrigin =
    Definition { origin: DefinitionOrigin }                  // tag 1, field 1
  | Concrete { origin: ConcreteExpressionOrigin }            // tag 2, field 1

DefinitionOriginSubject =
    Type { id: PersistentTypeId }                           // tag 1
  | GenericType { id: PersistentGenericTypeId }             // tag 2
  | Function { id: PersistentFunctionId }                   // tag 3
  | GenericFunction { id: PersistentGenericFunctionId }     // tag 4
  | Constructor { id: PersistentConstructorId }             // tag 5
  | Property { id: PersistentPropertyId }                   // tag 6
  | ExtensionProperty { id: PersistentExtensionPropertyId } // tag 7
  | PropertyAccessor { id: PersistentPropertyAccessorId }   // tag 8
  | TypeAlias { id: PersistentTypeAliasId }                 // tag 9
  | Field { id: PersistentFieldId }                         // tag 10
  | EnumVariant { id: PersistentEnumVariantId }             // tag 11
  | EnumVariantField { id: PersistentEnumVariantFieldId }   // tag 12
  | GeneratedCallable { id: PersistentGeneratedCallableId } // tag 13
  | InitializationUnit { id: PersistentInitializationUnitId }// tag 14
  | LocalBinding { id: PersistentLocalBindingId }           // tag 15
  | LocalValue { id: PersistentLocalValueId }               // tag 16
  | CallbackRegistration { id: PersistentCallbackRegistrationId } // tag 17
  | SourceNativeContract { id: PersistentSourceNativeExternalContractId } // tag 18

DefinitionOriginRecord {
    subject: DefinitionOriginSubject,     // field 1
    origin: DefinitionOrigin,             // field 2
}
```

两种origin即使wire shape相同也没有public转换；只有“普通表达式以自身定义点求值”的checked constructor可复制字段。`ExpressionOrigin`使用本章统一的closed-sum map：field 0是tag，field 1非可选地携带对应origin；`Definition`不能省略field 1，也不能把`DefinitionOrigin`三字段直接摊平到sum外层。template expression使用`ExpressionOrigin::Definition`，进入LocalConcrete HIR的表达式必须使用`Concrete`，definition/evaluation都不可缺失。context的source必须等于origin source，span端点必须出现在第4.4节point table。source-backed subject key中的origin Cone必须等于该source的Cone；若key含`SourceScoped`/`LexicalScoped` source，也必须逐字段等于definition source。context display name从已验证owner record生成，不保存`function_name/type_name`第二真相；core authority仍从compile-session side table取得。

foundation wire只承载identity/source projection，不承载expression body，因此本阶段在HIR table中序列化`DefinitionOriginRecord`，而不制造没有expression subject的游离Evaluation/Concrete record。source declaration、accessor、source field/variant及其field、initialization unit、HIR首次创建且有source definition site的local value、callback registration与source native contract各恰有一条definition record。一个`PersistentLocalBindingId`可能合并同source内多条语义相同的import/binding来源；其foundation definition origin固定取按`(source identity, span start, span end, context id)`最小的canonical representative，M23-5完整origin set必须包含该项且保存全部来源，不能让插入/worker顺序选择representative。MIR首次创建的SuspensionResult/Synthetic local value没有HIR definition record；承载它的后续MIR operation/transform capability以内联`ExpressionOrigin`给出其语义来源。只有key能沿typed owner/path唯一回溯到source site的generated nominal或generated callable才可有record；这类record出现时必须逐边重算到该唯一site，但它不属于所有generated entity都必须存在的覆盖集合。全局按exact shape复用且没有唯一source site的helper不得伪造一个first-use origin。record按`(subject tag, subject raw id)`严格递增。M23-5的Export/template body section与后续LocalConcrete payload把`ExpressionOrigin`直接放在其expression record中；MIR/LIR后续required payload同样以内联typed source location承载自己的operation origin。它们都复用此处schema与source/context表，但不把body origin倒灌成foundation空表。

## 5. persistent entity identity v1

### 5.1 source declaration key

源声明不共用一个可擦除kind的key。为避免每个key重复语法，wire先定义以下只能被kind-specific constructor消费的product：

```text
SourceDeclarationKey {
    origin: ConeIdentity,                    // field 1
    package: PackagePath,                   // field 2
    owners: DefinitionOwnerChain,           // field 3
    name: DeclarationName,                // field 4
    declaration_kind: SourceDeclarationKind, // field 5
    duplicate_signature: DuplicateSignatureKey, // field 6
    scope: DeclarationScope,               // field 7
}
```

`PackagePath`编码为segment array，root package是空array；每个segment必须是已通过语言identifier检查的原始UTF-8 text。`CanonicalIdentifier`同样保留源码UTF-8 bytes，不做Unicode normalization。`DeclarationName`是`Named=1 {1=CanonicalIdentifier}`或无payload的`Constructor=2`；只有constructor declaration使用后者，不能伪造`<init>`等字符串。`DefinitionOwnerChain`从外到内编码typed owner atom，atom的v1 tag为`Type=1, GenericType=2, Function=3, GenericFunction=4, Constructor=5, Property=6, ExtensionProperty=7, GeneratedCallable=8, PropertyAccessor=9`，payload只能是对应typed persistent id。空chain表示top-level；owner必须已在dependency-first declaration table中出现。getter/setter body中的named local declaration必须含tag 9的具体accessor owner，不能退回property owner；因此同一property的getter与setter即使出现相同name/path也不会碰撞。

`SourceDeclarationKind`的tag为`Class=1, Interface=2, Struct=3, Enum=4, Object=5, AnnotationClass=6, Function=7, Constructor=8, Property=9, ExtensionProperty=10, TypeAlias=11`。kind-specific constructor必须检查kind、`DuplicateSignatureKey` variant与hash domain三者匹配；例如extension property不能以`Property=9`生成相同key后仅靠Rust调用点解释。

`DeclarationScope`为封闭sum：`ConeWide=1`无payload，`SourceScoped=2 {1=SourceIdentity}`，`LexicalScoped=3 {1=SourceIdentity, 2=StructuralDefinitionPath}`。只有语言duplicate-declaration规则允许两个不同source拥有同key的file-private/top-level hidden声明才能使用`SourceScoped`；named local function（包括local generic template）使用LexicalScoped，path来自parent callable的stable lexical traversal。其他visibility或member私有性不影响identity。由此local generic同样取得`PersistentGenericFunctionId`并可作为callable application origin，不能因其local而退化为generated display identity。

`DuplicateSignatureKey`只编码语言判定重复/重载所需部分，tag和payload为：

| tag | variant | payload field |
| ---: | --- | --- |
| 1 | `Nominal` | `1=type_parameter_count: u32` |
| 2 | `Function` | `1=type_parameter_count, 2=receiver: OptionalSignatureType, 3=parameters: array<SignatureTypeKey>` |
| 3 | `Constructor` | `1=parameters: array<SignatureTypeKey>` |
| 4 | `Property` | `1=type_parameter_count, 2=receiver: OptionalSignatureType` |
| 5 | `TypeAlias` | 无payload |

`OptionalSignatureType`是`Absent=1`/`Present=2 { 1=type }`的显式sum，不使用CBOR null。function return type、parameter name/default、property `val/var`、body、annotation和visibility不参与duplicate signature；其中任一项改变会通过semantic fingerprint失效，不会偷换声明identity。

`SignatureTypeKey`的tag固定为：

| tag | variant | payload field |
| ---: | --- | --- |
| 1 | `Nominal` | `1=PersistentTypeId` |
| 2 | `NominalApplication` | `1=PersistentGenericTypeId, 2=non-empty array<SignatureTypeKey>` |
| 3 | `Tuple` | `1=non-empty array<SignatureTypeKey>` |
| 4 | `Function` | `1=effect, 2=array<SignatureTypeKey>, 3=result` |
| 5 | `RawPointer` | `1=pointee` |
| 6 | `NativeFunctionPointer` | `1=calling_convention, 2=array<SignatureTypeKey>, 3=result` |
| 7 | `Binder` | `1=de-Bruijn depth: u32, 2=index: u32` |

`effect` tag为`Ordinary=1, Suspend=2`，`calling_convention`的v1唯一值为`C=1`。typealias在生成signature key前透明展开；nullable语法先脱糖为core `Option`。binder是签名中唯一允许的非concrete type ref，其depth/index必须落在对应declaration binder stack中。

non-generic type/function constructor要求`type_parameter_count=0`，generic type/function constructor要求大于0。普通property要求count 0且receiver absent，extension property可有binder且receiver必须present；M23 typealias严格non-generic，因而其duplicate variant无count，未来generic alias必须提升identity schema。反之均是identity-construction error，不在reader中猜测修复。

### 5.2 kind domain与构造器

下表的domain ASCII是协议常量，不得从Rust type name生成。“key”一列也是public constructor允许的唯一输入：

| typed id | hash domain | canonical key |
| --- | --- | --- |
| `PersistentTypeId` | `scoop-type-id-v1` | `TypeIdentityKeyV1` |
| `PersistentSourceContextId` | `scoop-source-context-id-v1` | `SourceContextKey` |
| `PersistentGenericTypeId` | `scoop-generic-type-id-v1` | `SourceDeclarationKey(Nominal)` |
| `PersistentFunctionId` | `scoop-function-id-v1` | `SourceDeclarationKey(Function)` |
| `PersistentGenericFunctionId` | `scoop-generic-function-id-v1` | `SourceDeclarationKey(Function)` |
| `PersistentCallableApplicationId` | `scoop-callable-application-id-v1` | `CallableApplicationKey` |
| `PersistentConstructorId` | `scoop-constructor-id-v1` | `SourceDeclarationKey(Constructor)` |
| `PersistentPropertyId` | `scoop-property-id-v1` | `SourceDeclarationKey(Property)` |
| `PersistentExtensionPropertyId` | `scoop-extension-property-id-v1` | `SourceDeclarationKey(Property)` |
| `PersistentObjectValueId` | `scoop-object-value-id-v1` | `{1=PersistentTypeId source object declaration}` |
| `PersistentTypeAliasId` | `scoop-type-alias-id-v1` | `SourceDeclarationKey(TypeAlias)` |
| `PersistentPropertyAccessorId` | `scoop-property-accessor-id-v1` | `{1=property owner sum, 2=Getter(1)/Setter(2)}` |
| `PersistentFieldId` | `scoop-field-id-v1` | `FieldIdentityKey` |
| `PersistentEnumVariantId` | `scoop-enum-variant-id-v1` | `EnumVariantIdentityKey` |
| `PersistentEnumVariantFieldId` | `scoop-enum-variant-field-id-v1` | `{1=variant, 2=EnumVariantFieldSelector}` |
| `PersistentGeneratedCallableId` | `scoop-generated-callable-id-v1` | `GeneratedCallableKey` |
| `PersistentDispatchSlotId` | `scoop-dispatch-slot-id-v1` | `{1=dispatch declaration owner, 2=dispatch role}` |
| `PersistentLocalBindingId` | `scoop-local-binding-id-v1` | `LocalBindingKey` |
| `PersistentLocalValueId` | `scoop-local-value-id-v1` | `LocalValueKey` |
| `PersistentCallbackRegistrationId` | `scoop-callback-registration-id-v1` | `CallbackRegistrationKey` |
| `PersistentCallbackApplicationId` | `scoop-callback-application-id-v1` | `CallbackApplicationKey` |
| `PersistentSourceNativeExternalContractId` | `scoop-source-native-contract-id-v1` | `SourceNativeExternalContractKey` |
| `PersistentNativeExternalSymbolId` | `scoop-native-link-symbol-v1` | `NativeExternalSymbolKey` |
| `NativeLinkRequirementId` | `scoop-native-link-requirement-v1` | `NativeLinkRequirementKey` |
| `PersistentExportBindingId` | `scoop-export-binding-id-v1` | `ExportBindingKey` |
| `PersistentInitializationUnitId` | `scoop-initialization-unit-id-v1` | `InitializationUnitKey` |
| `PersistentLayoutId` | `scoop-layout-id-v1` | `{1=exact type, 2=target profile id, 3=representation role}` |
| `PersistentScanId` | `scoop-scan-id-v1` | `{1=layout id, 2=scan role}` |
| `PersistentDispatchTableId` | `scoop-dispatch-table-id-v1` | `{1=exact type, 2=table role, 3=optional interface exact type}` |
| `PersistentStaticStorageId` | `scoop-static-storage-id-v1` | `{1=typed owner, 2=storage role}` |
| `PersistentImmortalObjectId` | `scoop-immortal-object-id-v1` | `{1=ImmortalObjectOwner, 2=object role, 3=structural path}` |
| `OdrGroupId` | `scoop-odr-v1` | `SpecializationKey` |
| `OdrMemberId` | `scoop-odr-member-v1` | `OdrMemberKey` |
| `GeneratedBridgeUnitId` | `scoop-generated-bridge-unit-v1` | `GeneratedBridgeUnitKey` |
| `GeneratedBridgeAtomId` | `scoop-generated-bridge-atom-v1` | `GeneratedBridgeAtomKey` |
| `ObjectDefinitionPlanId` | `scoop-object-definition-plan-v1` | `ObjectDefinitionPlanKey` |
| `ObjectDefinitionAtomId` | `scoop-object-definition-atom-v1` | `ObjectDefinitionAtomKey` |

`PersistentCallableApplicationId`不是“generic function id”的别名；它统一表示因generic nominal owner、callable自身binder或两者而需要具体化的callable：

```text
CallableTemplateOrigin =
    Function { id: PersistentFunctionId }                 // tag 1
  | GenericFunction { id: PersistentGenericFunctionId }   // tag 2
  | Constructor { id: PersistentConstructorId }           // tag 3
  | Accessor { id: PersistentPropertyAccessorId }          // tag 4

CallableArguments =
    NoCallableArguments                                    // tag 1
  | Arguments { values: NonEmpty<PersistentExactTypeId> }  // tag 2, field 1

CallableApplicationKey {
    origin: CallableTemplateOrigin,      // field 1
    instantiation_owner: CallableInstantiationOwner, // field 2
    callable_arguments: CallableArguments, // field 3
}
```

`CallableInstantiationOwner`是`NoOwner=1`、`ExactNominalOwner=2 {1=PersistentExactTypeId}`、`EnclosingCallableApplication=3 {1=PersistentCallableApplicationId}`或`EnclosingInitializationApplication=4 {1=PersistentInitializationUnitId}`。普通method、constructor或普通property accessor若声明于generic nominal中，使用ExactNominalOwner和`NoCallableArguments`；这里的exact owner必须是声明宿主的exact application，不是调用点receiver或其动态派生类型。generic method同时保存nominal owner与callable arguments；top-level generic function只有callable arguments；generic extension-property accessor使用NoOwner与该property receiver binder对应的arguments。local callable若外层generic环境来自普通callable application，则使用EnclosingCallableApplication；该parent application已经封装其nominal owner和外层callable arguments，不能再压平复制。若local callable的lexical parent链穿过generic delegated initializer/ensure（包括其中lambda或anonymous callable），则使用EnclosingInitializationApplication；unit必须是同一source extension property与receiver arguments的`GenericDelegatedExtensionApplication`，并完整提供receiver substitution。它不得用于普通initializer、别的property或从调用点动态receiver合成。两项都不存在时不构造application id而直接使用declaration id。owner/argument数量必须分别精确等于nominal/callable、extension-property或lexical binder数且全部concrete；`Function`和`Constructor`禁止携带自身callable arguments，`GenericFunction`必须携带非空且arity匹配的自身arguments，`Accessor`是否允许arguments由其property owner kind决定。local generic的自身arguments即使不出现在exact signature（phantom parameter）也必须进入key；其enclosing initialization owner只提供外层receiver substitution，不得吞并或复制这些arguments。validator必须沿source lexical parent与generated-parent relation逐边证明tag 3或tag 4恰好对应nearest enclosing materialization root；没有这条证明的application id非法。它与declaration id、session-local substitution/ref是不同type，不可通过u32、FQN或空array互换。

为了区分模板级generated declaration与某次concrete materialization，另有以下closed products：

```text
CallableTemplateOwner =
    Function { id: PersistentFunctionId }                  // tag 1
  | GenericFunction { id: PersistentGenericFunctionId }    // tag 2
  | Constructor { id: PersistentConstructorId }            // tag 3
  | Accessor { id: PersistentPropertyAccessorId }           // tag 4
  | Generated { id: PersistentGeneratedCallableId }         // tag 5

CallableMaterializationContext =
    NoSubstitution                                           // tag 1
  | Application { id: PersistentCallableApplicationId }      // tag 2, field 1
  | InitializationApplication {
        unit: PersistentInitializationUnitId }                // tag 3, field 1

CallableMaterialization {
    template: CallableTemplateOwner,                       // field 1
    context: CallableMaterializationContext,               // field 2
}
```

`NoSubstitution`只允许template及其nominal/lexical/initialization owner binder总数为0；否则普通callable上下文必须引用恰好替换该owner链的`Application`，generic delegated initializer/ensure上下文必须引用`InitializationApplication`且unit为同一source extension property的`GenericDelegatedExtensionApplication`。Application record的origin/parent链必须能回溯到template；若application使用`EnclosingInitializationApplication`，该链必须终止于相同unit。InitializationApplication的property/receiver arguments必须逐项替换Initialization template声明级unit所属property的binder，并回溯到该template；不能使用调用点receiver的动态类型或另一个generic实例。Initialization template禁止使用`Application` context，非Initialization template也禁止凭空使用`InitializationApplication`，除非其lexical parent链最终到达该Initialization template。`LexicalCallableParent`是`CallableTemplateOwner`的refinement，其中Generated只接受下述Lexical、CallableReferenceInvoke或Initialization角色；它永不直接接受application id。generated owner递归必须无环，Initialization以声明级unit作为template owner终点，以materialization context中的application unit作为concrete emission root。

`TypeIdentityKeyV1`是`Source=1 {1=SourceDeclarationKey(Nominal)}`或`Generated=2 {1=GeneratedNominalKey}`。source generic nominal必须生成`PersistentGenericTypeId`，不能伪装成`PersistentTypeId`；generated nominal一定是concrete，使用`PersistentTypeId`的`Generated`分支。生成type使用角色特定的语义key，而不是把所有角色强行绑定到“第一次使用”的source ordinal：

```text
GeneratedNominalKey =
    ClosureEnvironment { callable: CallableMaterialization,
                         role: ClosureEnvironmentRole }                   // tag 1
  | CallableAdapterEnvironment { key: CallableAdapterEnvironmentKey }     // tag 2
  | CoroutineFrame { source_callable: CallableMaterialization }           // tag 3
  | ContinuationAdapterEnvironment { source_callable: CallableMaterialization,
                                     suspension_site: StructuralDefinitionPath } // tag 4
  | CoroutineStep { result: PersistentExactTypeId }                         // tag 5
  | BoxedValue { payload: PersistentExactTypeId }                           // tag 6
  | CoroutineSlot { value: PersistentExactTypeId }                          // tag 7
  | ObjectBackingClass { object: PersistentTypeId }                         // tag 8
```

`ClosureEnvironmentRole`为`Lambda=1, AnonymousFunction=2, CallableReference=3`，必须与materialization.template的source/generated kind匹配；lexical path已经包含在template callable id中，不重复写入environment key。`CallableAdapterEnvironmentKey`是`Static=1 {1=source signature, 2=target signature}`或`Dynamic=2 {1=target signature}`。closure/frame/continuation environment在generic callable上下文中按Application分开，在generic delegated initializer上下文中按InitializationApplication分开，在param-free上下文中使用NoSubstitution；adapter、box、coroutine step/slot分别按exact source/target、payload、result/value形状全局复用；source object的backing class由object declaration唯一确定。任何role都不读取first-use source、arena ordinal、临时type name或worker完成顺序。

`GeneratedCallableKey`同样是角色特定的封闭sum：

```text
GeneratedCallableKey =
    Lexical { parent: LexicalCallableParent,
              role: LexicalCallableRole,
              path: StructuralDefinitionPath }                            // tag 1
  | Initialization { unit: PersistentInitializationUnitId,
                     role: InitializationCallableRole }                    // tag 2
  | DerivedEquality { exact_owner: PersistentExactTypeId }                  // tag 3
  | FunctionAdapter { source: ExactCallableSignature,
                      target: ExactCallableSignature }                     // tag 4
  | DynamicFunctionAdapter { target: ExactCallableSignature }             // tag 5
  | CallableReferenceInvoke { parent: LexicalCallableParent,
                              path: StructuralDefinitionPath }            // tag 6
  | StaticNoGcCallbackStorageBridge { source: CallableMaterialization,
                                      signature: ExactCallableSignature } // tag 7
  | ForeignCallbackManagedAdapter { application: PersistentCallbackApplicationId } // tag 8
  | CoroutineDriver { source_callable: CallableMaterialization }          // tag 9
  | ContinuationShell { result: PersistentExactTypeId,
                        role: ContinuationShellRole }                       // tag 10
  | CoroutineStart { result: PersistentExactTypeId }                         // tag 11
  | CoroutineAdapter { source_callable: CallableMaterialization,
                       suspension_site: StructuralDefinitionPath,
                       role: CoroutineAdapterRole }                        // tag 12
  | FunctionBridge { environment: PersistentTypeId,
                     target: ExactCallableSignature }                      // tag 13
  | DispatchAdjust { slot: PersistentDispatchSlotId,
                     implementor: PersistentExactTypeId,
                     target: CallableMaterialization }                     // tag 14
  | BoxingAdjust { slot: PersistentDispatchSlotId,
                   payload: PersistentExactTypeId,
                   interface: PersistentExactTypeId }                        // tag 15
```

`GeneratedCallableKey::Initialization.unit`只接受声明级`TopLevelProperty`、`ExtensionProperty`、`Object`或`Companion` unit；它是initializer/ensure模板的source owner。`GenericDelegatedExtensionApplication`是concrete materialization unit，禁止直接写进该template key：generic delegated extension property仍以其声明级`ExtensionProperty` unit产生唯一Initialization template，再以`CallableMaterializationContext::InitializationApplication`携带每组exact receiver arguments。否则同一source initializer会被错误复制成多个“模板”，其中的binder callback/local declaration也无法先于concretization取得identity。

`LexicalCallableParent`使用`CallableTemplateOwner`相同的1…5 wire tag，但parent tag 5进一步只接受`GeneratedCallableKey` tag 1/2/6产生的`PersistentGeneratedCallableId`；其中tag 2的Initialization callable以声明级unit终止，而不是继续寻找source function。它不含任何application id，因而同一个generic模板/initializer内的lambda、anonymous function或callable-reference wrapper只取得一个declaration identity；不同concrete callable application或generic delegated initialization unit由materialization relation区分。嵌套lexical callable递归引用模板级parent，cycle拒绝。

`LexicalCallableRole`为`LambdaBody=1, AnonymousFunctionBody=2`；named local function无论是否generic都走带LexicalScoped的source function declaration identity，不进入此sum。tag 1/6都不把exact signature写入identity，因为Export HIR中的合法signature仍可能含binder；每个param-free实现或concrete application的完整signature改由第9.2节以Strong/ODR subject记录。callable-reference的resolved target也不进入identity：它继续保存在当前typed HIR body，未来实际承载body/default的required HIR capability必须以封闭target sum覆盖named/local/bound/derived-equality/dispatch形态并将target/source/target signature纳入HIR semantic projection。identity-foundation本身不宣称序列化body。这样同一lexical site不会因alias/dispatch refinement换identity，也不要求`CallableOwner`假装覆盖全部call target。

`InitializationCallableRole`为`Initializer=1, Ensure=2`；`ContinuationShellRole`与`CoroutineAdapterRole`均为`Success=1, Failure=2`。`CallbackRegistrationKey`是map `1=parent: LexicalCallableParent, 2=StructuralDefinitionPath, 3=SourceCAbiFunctionSignature, 4=context_index: CallbackParameterIndex, 5=managed SignatureCallableShape, 6=CallbackMode`。`SignatureCallableShape`精确为map `1=effect, 2=OptionalSignatureType receiver, 3=array<SignatureTypeKey> parameters, 4=SignatureTypeKey result`，复用第5.1节允许binder的type tree，不能在Export HIR阶段伪造exact type。这是HIR可验证的target-independent callback declaration，不引用到LIR才产生的`GeneratedBridgeUnitId`。

concrete callback application另有`CallbackApplicationKey {1=PersistentCallbackRegistrationId, 2=CallableMaterializationContext}`及`PersistentCallbackApplicationId = DomainSeparatedCborHash("scoop-callback-application-id-v1", key)`。NoSubstitution只允许两份source signature都无binder；否则普通callable site使用恰好覆盖registration parent binder stack的Application，generic delegated initializer/ensure site使用同property、完整receiver arguments的InitializationApplication；替换后每个type都必须exact且C-safe。MIR relation以该application为主键加入exact managed signature与固定storage/status ABI；target-specific canonical C signature到LIR才产生。由此同一generic source site可有多个concrete callback application，同一canonical C signature/index仍可复用一个trampoline unit。

```text
PersistentCallbackRegistrationId =
    DomainSeparatedCborHash("scoop-callback-registration-id-v1",
                            CallbackRegistrationKey)
```

静态`@NoGC` callback storage bridge按`(source, signature)`复用；每个foreign callback application只生成一个GC-aware、machine-status managed adapter，C trampoline由LIR的GeneratedBridgeUnit/Atom表示而不是第二个generated callable。LIR把concrete source C signature正规化成target-specific canonical C signature并记录`{PersistentCallbackApplicationId, GeneratedBridgeUnitId}` bridge relation。source/target完全相同的function adapter只有一个identity；derived equality、continuation shell/start和coroutine step/slot都按exact type全局复用。foreign callback adapter的固定machine-status result由tag 8和target profile机械决定，不伪装成`PersistentExactTypeId`；第9章的callable signature bridge使用显式machine variant验证它。generated identity只依赖M24仍稳定且在本stage可用的declaration/application/source-contract owner，绝不依赖会从body-v1整体换成body-v2的`PersistentCallableBodyId`或未来stage identity。需要改变上述判等规则或增加角色时提升identity schema，不能把显示name塞进现有tag。

`StructuralDefinitionPath`是non-empty array的`{1=site_role, 2=ordinal: u32}`。compiler对每个owner的直接identity-bearing child按语言评估/声明源顺序遍历，对每个`site_role`分别从0计数，然后递归拼接路径。site role固定为`LocalDeclaration=1, Lambda=2, DefaultValue=3, AnonymousObject=4, CoroutineTransform=5, CallbackConversion=6, CallableConversion=7, DispatchAdapter=8, Initializer=9, SynthesizedBridge=10, SyntheticValue=11, StringConstant=12`。source string literal按表达式语言求值顺序取得当前owner下的StringConstant ordinal；desugar/transform新增的string constant在其已冻结的transform emit顺序中取得ordinal。content bytes只进入definition fingerprint，不参与ordinal或identity；多个内容相同的literal仍有不同path。MIR在同一source/transform site产生多个相同role temporary时，必须按该transform规范的emit顺序追加不同`SyntheticValue` ordinal，不能让多个temporary共享path。该算法可因owner body结构改变而改变generated identity，但空白、comment、arena id、并行完成顺序与其他source排序不影响它。

`InitializationUnitKey`是封闭sum：`TopLevelProperty=1 {1=PersistentPropertyId}`、`ExtensionProperty=2 {1=PersistentExtensionPropertyId}`、`Object=3 {1=PersistentTypeId}`、`Companion=4 {1=PersistentTypeId}`、`GenericDelegatedExtensionApplication=5 {1=PersistentExtensionPropertyId, 2=non-empty array<PersistentExactTypeId> receiver_arguments}`。tag 5的argument数量必须精确等于property receiver binder数，其key同时作为M23-7 delegated-property specialization的unit origin，不能让两个exact receiver application共用unit。初始化策略、body bytes和物理存储不进unit identity；它们进semantic/code fingerprint。

其他role tag也在identity schema v1中封闭：`DispatchRole { VirtualMethod=1, InterfaceMethod=2, PropertyGetter=3, PropertySetter=4 }`；`RepresentationRole { ManagedValue=1, ManagedObject=2, CValue=3, NativeFunctionPointer=4 }`；`ScanRole { InlineValue=1, ManagedObject=2, ArrayElement=3 }`；`DispatchTableRole { VTable=1, ITable=2 }`；`StorageRole { PropertyBacking=1, PropertyDelegate=2, SingletonPublishedRoot=3, InitializationFailureRoot=4, RootEntryFailureRoot=5, StaticPlaceToken=6 }`；M23-2的`ImmortalObjectRole`唯一值为`StringConstant=1`。init cell直接以`PersistentInitializationUnitId`的`ic` symbol表示，不再制造第二个storage identity；singleton instance与box是普通movable heap object，static place token是writable storage，三者都不能冒充只读immortal object。optional interface exact type使用`Absent=1`/`Present=2`，不用null；每个key constructor对owner kind/role组合做封闭校验，不对外暴露raw integer constructor。

static storage的owner/role矩阵精确为：

| storage role | 唯一允许的`DefinitionOwner` |
| --- | --- |
| `PropertyBacking` | `Property`；或只对`GenericDelegatedExtensionApplication`使用其`InitializationUnit` |
| `PropertyDelegate` | `Property`且该source property为delegated；或只对`GenericDelegatedExtensionApplication`使用其`InitializationUnit` |
| `SingletonPublishedRoot` | `Nominal::Declaration::Concrete`且目标是source object/companion declaration |
| `InitializationFailureRoot` | `InitializationUnit` |
| `RootEntryFailureRoot` | `RootEntry` |
| `StaticPlaceToken` | 本应有静态address identity但logical payload为ZST的`Property`，或`GenericDelegatedExtensionApplication`的`InitializationUnit` |

同一逻辑property/delegate storage根据layout在普通role与`StaticPlaceToken`间二选一，不能同时构造；strategy由eager改lazy不改变owner。其他nominal、callable、field、enum variant或不匹配的unit/role全部拒绝。generic delegated application必须用application-specific unit，不能退回extension property declaration而让不同receiver arguments碰撞。

上表中所有“typed owner”在wire上都不是裸32-byte bytes，而是以下可审查的sum：

```text
PropertyOwner =
    Property { id: PersistentPropertyId }                 // tag 1, field 1
  | ExtensionProperty { id: PersistentExtensionPropertyId } // tag 2, field 1

NominalDeclarationOwner =
    Concrete { id: PersistentTypeId }                     // tag 1
  | GenericTemplate { id: PersistentGenericTypeId }       // tag 2

NominalOwner =
    Declaration { owner: NominalDeclarationOwner }      // tag 1
  | ExactApplication { id: PersistentExactTypeId }        // tag 2

CallableOwner =
    Function { id: PersistentFunctionId }                 // tag 1
  | GenericTemplate { id: PersistentGenericFunctionId }   // tag 2
  | Application { id: PersistentCallableApplicationId }   // tag 3
  | Constructor { id: PersistentConstructorId }           // tag 4
  | Accessor { id: PersistentPropertyAccessorId }         // tag 5
  | Generated { id: PersistentGeneratedCallableId }       // tag 6

DispatchDeclarationOwner =
    Function { id: PersistentFunctionId }                 // tag 1
  | Accessor { id: PersistentPropertyAccessorId }         // tag 2

DefinitionOwner =
    Nominal { owner: NominalOwner }                     // tag 1, field 1
  | Callable { owner: CallableOwner }                   // tag 2, field 1
  | Property { owner: PropertyOwner }                   // tag 3, field 1
  | Field { id: PersistentFieldId }                       // tag 4, field 1
  | EnumVariant { id: PersistentEnumVariantId }           // tag 5, field 1
  | InitializationUnit { id: PersistentInitializationUnitId } // tag 6, field 1
  | RootEntry { root_cone: ConeIdentity,
                main: MainCallableBodyId }                 // tag 7, fields 1/2

ImmortalObjectOwner =
    Callable { owner: CallableMaterialization }           // tag 1, field 1
  | Property { owner: PropertyOwner }                     // tag 2, field 1
  | InitializationUnit { id: PersistentInitializationUnitId } // tag 3, field 1
```

sum仍使用`0=tag`，每个variant payload从field 1开始。`PersistentPropertyAccessorId`只接受property declaration owner；`FieldIdentityKey::Source`只接受source nominal declaration，Generated分支只接受generated `PersistentTypeId`，exact application不能另造per-application field declaration id；`PersistentEnumVariantId`的Source分支只接受source enum的Concrete/GenericTemplate owner，Generated分支只接受匹配role的generated enum。`PersistentDispatchSlotId`只接受`DispatchDeclarationOwner`，generic callable declaration、constructor、application和generated callable不能另造声明slot。trusted HIR constructor还检查：VirtualMethod owner是virtual family root、InterfaceMethod owner是direct interface member、getter/setter role匹配对应family-root accessor，所有override复用同一slot id。上述virtual/interface/family关系不在identity key中，M23-2 foundation wire只能验证owner typed kind与“generic callable不得成为slot”这类key内事实；`ValidatedCompileArtifact<IdentityFoundationProfile>`不暴露“dispatch合法”API。M23-5必须用新的required HIR dispatch-declaration provenance section重放其余检查，production profile取得该proof后才可导出slot/实现relation。现有LIR的closure/function-bridge call slot只是function-local ABI table index，不是persistent dispatch declaration；adjust/boxing/variance thunk是某个slot的implementation body，也不是新slot。application-specific field/variant/dispatch relation由“declaration id + exact owner/substitution”表达。storage/immortal key才可使用较宽的`DefinitionOwner`。这些sum仅用于保留kind的product field，不提供跨variant cast或“任意entity lookup”API。

第5.2节各个shorthand key的CBOR field也由此精确化：

| key | exact field schema |
| --- | --- |
| callable application | `1=CallableTemplateOrigin, 2=CallableInstantiationOwner, 3=CallableArguments` |
| property accessor | `1=PropertyOwner, 2=AccessorRole(Getter=1/Setter=2)` |
| field | `FieldIdentityKey`，见下文 |
| enum variant | `EnumVariantIdentityKey`，见下文 |
| enum variant field | `1=PersistentEnumVariantId, 2=EnumVariantFieldSelector` |
| dispatch slot | `1=DispatchDeclarationOwner, 2=DispatchRole` |
| layout | `1=PersistentExactTypeId, 2=TargetProfileWireId, 3=RepresentationRole` |
| scan | `1=PersistentLayoutId, 2=ScanRole` |
| dispatch table | `1=PersistentExactTypeId, 2=DispatchTableRole, 3=OptionalExactInterface` |
| static storage | `1=DefinitionOwner, 2=StorageRole` |
| immortal object | `1=ImmortalObjectOwner, 2=ImmortalObjectRole, 3=StructuralDefinitionPath` |

M23的StringConstant owner矩阵同样封闭：ordinary function/method/accessor、lambda/anonymous/generated callable body中的literal必须使用完整`CallableMaterialization`；const/property declaration级literal使用`Property`；initializer/ensure及generic delegated initializer使用`InitializationUnit`。nested generic lambda因此同时保留generated template与外层application，不得退回裸`GeneratedCallableId`或只用outer application再猜一段跳层path。每个path都从该精确owner的直接StringConstant child开始；其他`DefinitionOwner` variant不能构造immortal object。

source field与compiler-generated physical field不能共用display name判等：

```text
FieldIdentityKey =
    Source { key: SourceFieldKey }                       // tag 1
  | Generated { owner: PersistentTypeId,
                key: GeneratedFieldKey }                // tag 2

SourceFieldKey =
    Declared { owner: NominalDeclarationOwner,
               name: CanonicalIdentifier }                // tag 1
  | PropertyBacking { owner: NominalDeclarationOwner,
                      property: PersistentPropertyId }     // tag 2
  | PropertyDelegate { owner: NominalDeclarationOwner,
                       property: PersistentPropertyId }    // tag 3

GeneratedFieldKey =
    BoxPayload                                             // tag 1
  | ClosureCapture { value: PersistentLocalValueId }      // tag 2
  | CallableReferenceReceiver { value: PersistentLocalValueId } // tag 3
  | CoroutineFrameState                                   // tag 4
  | CoroutineFrameCompletion                              // tag 5
  | CoroutineFrameSaved { value: PersistentLocalValueId } // tag 6
  | CoroutineFrameFailure                                 // tag 7
  | CoroutineAdapterFrame                                 // tag 8
  | CoroutineAdapterState                                 // tag 9
  | CoroutineAdapterResult                                // tag 10
  | CoroutineAdapterFailure                               // tag 11
  | FunctionAdapterSource                                 // tag 12
  | ObjectBackingProperty { property: PersistentPropertyId } // tag 13

EnumVariantIdentityKey =
    Source { owner: NominalDeclarationOwner,
             name: CanonicalIdentifier }                  // tag 1
  | Generated { owner: PersistentTypeId,
                role: GeneratedEnumVariantRole }        // tag 2
```

field constructor的closed矩阵为：Source Declared只允许source struct的直接声明字段；PropertyBacking/PropertyDelegate只允许source class中的property physical field，且二者都携带该class的`PersistentPropertyId`。top-level property、extension property及其delegate是static/global storage，只走`PersistentStaticStorageId`，不能伪装成nominal field。BoxPayload→BoxedValue；ClosureCapture/CallableReferenceReceiver→ClosureEnvironment；FrameState/Completion/Saved/Failure→CoroutineFrame；AdapterFrame/State/Result/Failure→ContinuationAdapterEnvironment；FunctionAdapterSource→CallableAdapterEnvironment；ObjectBackingProperty→ObjectBackingClass。coroutine slot是generated enum，其payload只用variant的`PersistentEnumVariantFieldId(..., Positional(0))`，不生成class field。`GeneratedEnumVariantRole`为`CoroutineStepCompleted=1, CoroutineStepSuspended=2, CoroutineSlotEmpty=3, CoroutineSlotValue=4`。capture/saved physical field按下述local-value raw id排序，因此parameter、bound receiver与compiler temporary不退回arena index或display name；closure创建时各capture source expression的求值顺序仍由独立semantic capture plan按语言顺序保存，不能因field排序而重排副作用。`EnumVariantFieldSelector`是`Named=1 {1=CanonicalIdentifier}`或`Positional=2 {1=declaration_index:u32}`。named source payload使用声明名；`Circle(Int)`、`Rect(Int, Int)`及generated enum payload使用从0开始的声明index，不能制造空name或按type判等。applied field/variant ref分别是`{declaration id, exact owner}`typed relation，不产生新的persistent declaration id。

`OptionalExactOwner`、`OptionalExactInterface`均是`Absent=1`或`Present=2 {1=PersistentExactTypeId}`；后者present时exact type必须是interface nominal/application。`ExactCallableSignature`是map `1=effect, 2=OptionalExactOwner, 3=array<PersistentExactTypeId> parameters, 4=PersistentExactTypeId result`，是generated key与MIR bridge中Scoop signature的唯一schema。

跨artifact name binding另有独立identity，而不是复用目标entity id：

```text
ExportBindingKey {
    exporter: ConeIdentity,               // field 1
    package: PackagePath,                 // field 2
    namespace: BindingNamespace,        // field 3: Type=1, Value=2
    name: CanonicalIdentifier,            // field 4
    target: BindableEntity,             // field 5
    role: BindingRole,                  // field 6
}
```

`BindableEntity`的tag固定为`Type=1, GenericType=2, ObjectValue=3, Function=4, GenericFunction=5, Property=6, ExtensionProperty=7, TypeAlias=8, EnumVariant=9`，payload为对应typed id；constructor不是独立package binding。`BindingRole`固定为`TypeName=1, ObjectValue=2, Function=3, ExtensionFunction=4, Property=5, ExtensionProperty=6, TypeAlias=7, EnumVariant=8`。closed matrix精确为：Type namespace只接受`TypeName×(Type|GenericType)`与`TypeAlias×TypeAlias`；Value namespace只接受`ObjectValue×ObjectValue`、`Function×(Function|GenericFunction)`、`ExtensionFunction×(Function|GenericFunction且source declaration receiver present)`、`Property×Property`、`ExtensionProperty×ExtensionProperty`及`EnumVariant×EnumVariant`。其他namespace/role/target组合全部拒绝。generic nominal/function只使用GenericType/GenericFunction target，不同时发普通Type/Function target；object恰有TypeName与ObjectValue两条binding。`PersistentObjectValueId`由`DomainSeparatedCborHash("scoop-object-value-id-v1", {1=PersistentTypeId})`生成且constructor要求目标是source object declaration，不能与它的type id互转。

field 1始终是发布该destination binding的当前Cone，不是target declaration的Cone。Direct/ReExport、visibility、access provenance与逐跳route witness属于M23-5 surface payload和semantic fingerprint，不进入binding key；因此同一exporter/package/name/role/target由direct改为合法re-export或删掉一条diamond route不会改变binding id，但payload fingerprint会改变。M23-2已为当前Cone每个直接public package binding生成record；M23-5在同一table增加resolved re-export binding和完整surface payload，不回改identity或用FQN替代。

当前Cone name-resolution witness与callable内部value也各有独立identity：

```text
LocalBindingKey {
    origin: ConeIdentity,              // field 1
    source: SourceIdentity,            // field 2
    package: PackagePath,              // field 3
    namespace: BindingNamespace,     // field 4
    local_name: CanonicalIdentifier,   // field 5
    target: BindableEntity,          // field 6
    binding_role: BindingRole,       // field 7
    source_role: LocalBindingRole,   // field 8
}

LocalValueKey {
    owner: CallableMaterialization,  // field 1
    selector: LocalValueSelector,    // field 2
}

LocalValueSelector =
    This                                                         // tag 1
  | Parameter { declaration_index: u32 }                         // tag 2
  | LocalDeclaration { path: StructuralDefinitionPath }        // tag 3
  | BoundReceiver { path: StructuralDefinitionPath }           // tag 4
  | SuspensionResult { site: StructuralDefinitionPath }        // tag 5
  | Synthetic { path: StructuralDefinitionPath,
                role: SyntheticLocalRole }                      // tag 6
```

`LocalBindingRole`为`Declaration=1, ExactImport=2, StarImport=3, AliasImport=4`；`origin`必须逐byte等于`source.cone`及artifact Cone，同一source中重复导入同target/local name/role合成同一binding，合法来源集合进入M23-5 payload而不进identity。其`DefinitionOriginRecord`的source必须就是key中的source，不能借另一个文件的同名import充当representative。`SyntheticLocalRole`为`Temporary=1, DefaultValue=2, DesugaredIterator=3, CoroutineProtocol=4, CallbackContext=5`。parameter index按source signature声明序，`This`、callable-reference bound receiver与suspension result互不混用；synthetic path来自已冻结的表达式/变换遍历。`PersistentLocalBindingId`用于`CurrentCone` witness，`PersistentLocalValueId`用于capture/frame field，两者不能与HIR `BindingId`或local arena ordinal互换。

local value的owner必须同时保留模板与materialization context。param-free callable/initializer使用`NoSubstitution`；generic source callable使用自身Application；generic delegated initializer使用自身InitializationApplication；其中的lambda/anonymous/callable-reference wrapper以该generated template为`template`、以可回溯到其lexical parent或unit的同一context为`context`。因此两个callable application或两个receiver-specific initialization unit中的同一lambda local不会碰撞，也不会丢失它属于哪个lambda。未具体化的Export generic template只保存后续body capability定义的template-local typed selector，不提前构造`PersistentLocalValueId`；进入某个LocalConcrete application/unit后，HIR才为每个可能进入capture/frame的this、parameter、source local与bound receiver非可选地产生context-specific local-value id。MIR新建的eligible temporary使用Synthetic/SuspensionResult构造，并把coroutine saved set按raw id排序。capture/frame field的generated owner与local value必须具有同一个materialization context；只比较模板owner或结构path不足以通过validator。

`GeneratedBridgeUnitKey`与总设计4.1的object logical-key基础使用同一sum：`OutboundFunction=1 {1=NativeExternalContractFingerprint}`、`GlobalRead=2 {1=NativeExternalContractFingerprint}`、`GlobalWrite=3 {1=NativeExternalContractFingerprint}`、`GlobalAddress=4 {1=NativeExternalContractFingerprint}`、`CallbackTrampoline=5 {1=CanonicalCAbiSignatureFingerprint, 2=CallbackParameterIndex}`。`CallbackParameterIndex`是zero-based u32，所指参数必须恰为该registration指定的`Ptr<Unit>` context槽；同一签名可以另有普通`Ptr<Unit>`参数。这个unit key不含closure、managed adapter、arena id或object分片。

bridge/object atom不使用“排序后第几个”的logical index；它们由稳定语义subkey区分：

```text
GeneratedBridgeAtomKey {
    producer: ConeIdentity,                         // field 1
    atom: GeneratedBridgeAtomRoleKey,             // field 2
}

GeneratedBridgeAtomRoleKey =
    PrimaryEntry { unit: GeneratedBridgeUnitId }                         // tag 1
  | SignatureDescriptor { unit: GeneratedBridgeUnitId,
                          signature: CanonicalCAbiSignatureFingerprint } // tag 2
  | ContextDescriptor { unit: GeneratedBridgeUnitId,
                        context_index: CallbackParameterIndex }          // tag 3
  | StaticAssertSupport { unit: GeneratedBridgeUnitId,
                          layout: CanonicalCAbiLayoutFingerprint }       // tag 4

GeneratedBridgeSemanticTarget {
    unit: GeneratedBridgeUnitId,                    // field 1
}

StrongDefinitionEntity =
    CallableBody { id: PersistentCallableBodyId }                // tag 1
  | StaticStorage { id: PersistentStaticStorageId }              // tag 2
  | ImmortalObject { id: PersistentImmortalObjectId }            // tag 3
  | ExactType { id: PersistentExactTypeId }                      // tag 4
  | Layout { id: PersistentLayoutId }                            // tag 5
  | Scan { id: PersistentScanId }                                // tag 6
  | DispatchTable { id: PersistentDispatchTableId }              // tag 7
  | DispatchSlot { id: PersistentDispatchSlotId }                // tag 8
  | InitializationUnit { id: PersistentInitializationUnitId }    // tag 9
  | SafepointSite { id: PersistentSafepointSiteId }              // tag 10
  | ConeImage { id: ConeIdentity }                               // tag 11
  | GeneratedBridgeAtom { id: GeneratedBridgeAtomId }            // tag 12

StrongDefinitionRole =
    CallableBody               // tag 1
  | StaticStorage              // tag 2
  | ImmortalObject             // tag 3
  | TypeDescriptor             // tag 4
  | Layout                     // tag 5
  | ScanProgram                // tag 6
  | DispatchTable              // tag 7
  | DispatchSlot               // tag 8
  | InitializationCell         // tag 9
  | InitializationDescriptor   // tag 10
  | RootRegistration           // tag 11
  | ImmortalRegistration       // tag 12
  | InitializationRegistration // tag 13
  | TypeRegistration           // tag 14
  | SafepointRegistration      // tag 15
  | CallableRegistration       // tag 16
  | ImageDescriptor            // tag 17
  | GeneratedBridge            // tag 18

ObjectDefinitionPlanOwner =
    Strong { producer: ConeIdentity,
             entity: StrongDefinitionEntity }                  // tag 1, fields 1..2
  | Odr { member: OdrMemberId }                                  // tag 2, field 1

ObjectDefinitionPlanRole =
    Strong { role: StrongDefinitionRole }                       // tag 1, field 1
  | OdrMemberPrimary                                             // tag 2

ObjectDefinitionPlanKey {
    owner: ObjectDefinitionPlanOwner,       // field 1
    definition_role: ObjectDefinitionPlanRole, // field 2
}

ObjectDefinitionPlanId =
    DomainSeparatedCborHash("scoop-object-definition-plan-v1",
                            ObjectDefinitionPlanKey)

ObjectDefinitionAtomKey {
    plan: ObjectDefinitionPlanId,      // field 1
    role: DefinitionAtomRole,        // field 2
    subkey: DefinitionAtomSubkey,    // field 3
}

DefinitionAtomSubkey =
    Singleton                                                        // tag 1
  | CallableBody { id: PersistentCallableBodyId }                    // tag 2
  | StaticStorage { id: PersistentStaticStorageId }                  // tag 3
  | ImmortalObject { id: PersistentImmortalObjectId }                // tag 4
  | InitializationUnit { id: PersistentInitializationUnitId }        // tag 5
  | ExactType { id: PersistentExactTypeId }                          // tag 6
  | SafepointSite { id: PersistentSafepointSiteId }                  // tag 7
  | StructuralPath { path: StructuralDefinitionPath }              // tag 8
```

`GeneratedBridgeSemanticTarget`是LIR canonical definition、ODR fingerprint与undefined requirement中唯一可引用的bridge target；它只有unit，不含producer/atom。每个实际producer Cone必须为所用unit恰好生成一个`PrimaryEntry { unit }` atom，codegen把semantic target映射成`GeneratedBridgeAtomKey { producer = current artifact Cone, atom = PrimaryEntry(unit) }`。object verifier核对真实`br` symbol后，把relocation规范化回`GeneratedBridgeUnit { id }` canonical target；defined owner仍是producer-specific atom。这样两个Cone生成同一ODR body时，其机器relocation可各指向本Cone强bridge，但canonical LIR/object definition都观察相同unit recipe，不把consumer-local atom id写进ODR fingerprint。缺失/多个primary、跨producer atom或LIR直接保存atom id全部拒绝。

`StrongDefinitionEntity × StrongDefinitionRole`矩阵逐项对应第7.1节key tag 1…18：CallableBody只接受role 1/16，StaticStorage接受2/11，ImmortalObject接受3/12，ExactType接受4/14，Layout、Scan、DispatchTable、DispatchSlot分别接受5…8，InitializationUnit接受9/10/13，SafepointSite接受15，ConeImage接受17，GeneratedBridgeAtom接受18。GeneratedBridgeAtom entity只接受`PrimaryEntry`、`SignatureDescriptor`或`ContextDescriptor`三种可物理materialize的atom id；`StaticAssertSupport`虽有identity record，却永远不能进入plan。Strong owner的entity必须按第7.2节求得同一个`producer` Cone；ODR owner只能与`OdrMemberPrimary`配对，Strong owner只能与`Strong` role配对。由此同一typed entity的body与registration可得到两个不同plan，而不同kind的裸32-byte值不能混用。plan key不含member、section、range、offset或digest；M23-3/M23-7只增加expected boundary/associated-role payload和物理materialization relation，不得重定义plan id。

`DefinitionAtomRole`为`Primary=1, Lsda=2, EhFrame=3, CompactUnwind=4, Stackmap=5, RuntimeRecord=6, AddressTakenConstant=7`。同一Cone中的相同语义subkey必然得到相同bridge atom id；不同Cone可复用同一`GeneratedBridgeUnitId`与canonical C source recipe，但必须得到各自的atom id和`ConeStrong` symbol。v1刻意不把generated bridge atom纳入Specialization ODR；M23-7也不能移除producer或把`br`改成weak，未来若要跨Cone共享atom必须提升identity/schema。generated-bridge object verifier从artifact Cone与unit-to-member relation重算field 1；producer不是first use、worker或临时translation-unit identity。`StaticAssertSupport`只证明canonical generated-C source中的compile-time assertion recipe，成功object中没有对应section bytes，因此它可以有identity record但绝不能产生`br` request、ObjectDefinitionPlan或defined-symbol owner；只有PrimaryEntry、SignatureDescriptor与ContextDescriptor可物理materialize。缺少预期assert proof或为它凭空造sentinel atom都拒绝。同plan/role下的多个object atom必须有不同typed subkey，而不能因插入一个更早元素重编号。M23-2 foundation可以把plan/atom table编码为空，但任何出现的record都必须按上述完整key重算；M23-3/M23-7接入实际producer/materialization，不接受raw bytes或伪plan。

现有generic materialization已经要求可持久引用的ODR identity，因此M23-2冻结最小、但不声称完成definition等价验证的key：

```text
SpecializationKey =
    Nominal { origin: PersistentGenericTypeId,
              arguments: NonEmpty<PersistentExactTypeId> }                // tag 1
  | Callable { application: CallableApplicationKey }                    // tag 2
  | DelegatedProperty { origin: PersistentExtensionPropertyId,
                        receiver_arguments: NonEmpty<PersistentExactTypeId> } // tag 3
  | StructuralType { exact_type: PersistentExactTypeId }                  // tag 4

OdrMemberKey {
    group: OdrGroupId,                         // field 1
    role: OdrMemberRole,                     // field 2
    discriminator: OdrMemberDiscriminator,   // field 3
}
```

`OdrMemberRole`固定为`CallableBody=1, GeneratedNominal=2, Layout=3, ScanProgram=4, TypeDescriptor=5, DispatchTable=6, DispatchAdapter=7, StaticStorage=8, ImmortalObject=9, InitializationCell=10, InitializationDescriptor=11, RegistrationRecord=12, DiagnosticBytes=13, AddressTakenConstant=14, ObjectSupport=15, ReleaseHook=16`。tag 16在identity schema v1中为M24预留唯一语义，M23 HIR/MIR/LIR schema v1不得产生它；这不是unknown/reserved tag。`OdrMemberDiscriminator`固定为`Singleton=1, CallableApplication=2, GeneratedCallable=3, GeneratedNominal=4, ExactType=5, Layout=6, Scan=7, DispatchTable=8, DispatchSlot=9, StaticStorage=10, ImmortalObject=11, InitializationUnit=12, StructuralPath=13, CallableBody=14, SafepointSite=15`，除Singleton外field 1携带名字所示typed id/path。

role与discriminator允许矩阵也属于v1 schema：`CallableBody→CallableApplication|GeneratedCallable|InitializationUnit`；`GeneratedNominal→GeneratedNominal`；`Layout→Layout`；`ScanProgram→Scan`；`TypeDescriptor→ExactType`；`DispatchTable→DispatchTable`；`DispatchAdapter→DispatchSlot|GeneratedCallable`；`StaticStorage→StaticStorage`；`ImmortalObject→ImmortalObject`；`InitializationCell|InitializationDescriptor→InitializationUnit`；`RegistrationRecord→CallableBody|SafepointSite|ExactType|StaticStorage|ImmortalObject|InitializationUnit`；`DiagnosticBytes→ExactType|CallableApplication|GeneratedNominal|StructuralPath`；`AddressTakenConstant→ImmortalObject|StructuralPath`；`ObjectSupport→Singleton|StructuralPath`；`ReleaseHook→ExactType`。其他组合拒绝。`CallableOdrMemberId`只接受`CallableBody`/`DispatchAdapter`及`CallableApplication|GeneratedCallable|InitializationUnit` discriminator；其中`CallableBody/InitializationUnit`只允许generic delegated-property组的startup gateway，discriminator unit必须逐字段等于group root的property与receiver arguments。`ReleaseHookOdrMemberId`只接受`ReleaseHook/ExactType`，且只有M24 outer schema v2可把它作为semantic member使用。Callable group还必须逐字段比较内嵌`CallableApplicationKey`与对应`PersistentCallableApplicationId` record，不能只相信一份32-byte摘要。

合法role/discriminator还不等于合法group成员；v1同时冻结`SpecializationKey variant × member provenance`门禁：

- `Callable`组的`CallableApplication`必须逐字段等于group key；generated callable/nominal、body、site、storage与support必须沿模板级lexical owner加该application的materialization relation回溯到它，不能把另一个application或全局shape helper挂入；
- `Nominal`组的root必须是与`origin + arguments`相同的`ExactTypeKey::NominalApplication`；layout/scan/TD/table/field/adapter及其generated descendant都须沿exact owner链回溯到该root；
- `DelegatedProperty`组的initialization unit必须逐字段等于同一property与receiver arguments，storage/cell/descriptor、initializer/ensure与`InitializationStartupGateway` callable body及support只能沿该unit回溯；
- `StructuralType`组的layout/scan/TD必须直接以group exact type为root。

为使generated key中的多个typed ref不会各自竞争owner，v1进一步冻结下列纯root函数。`MaterializationRoot(m)`按context取NoSubstitution source/unit Cone、Callable Application组或InitializationApplication unit root；`ExactOwnerRoot(t)`对source nominal取定义Cone、nominal application取Nominal组、结构exact取`StructuralType(t)`，对generated nominal递归使用第一张表；`StructuralRoot(t)`只接受经exact-type validator证明为非nominal的tuple/function/raw pointer/native function pointer，并取`StructuralType(t)`；`CallbackRoot(a)`按registration parent加NoSubstitution/Application/InitializationApplication求根。`FunctionShape(s)`要求adapter target signature无exact receiver，并以其effect/parameters/result构造已经存在的managed `ExactTypeKey::Function`；有receiver的adapter必须先形成独立的bound-receiver environment，不能把receiver悄悄丢掉。

| `GeneratedNominalKey` variant | 唯一root | 只作dependency、不得竞争root的字段 |
| --- | --- | --- |
| ClosureEnvironment | `MaterializationRoot(callable)` | role |
| CallableAdapterEnvironment::Static | `StructuralRoot(FunctionShape(target))` | source signature |
| CallableAdapterEnvironment::Dynamic | `StructuralRoot(FunctionShape(target))` | 无 |
| CoroutineFrame | `MaterializationRoot(source_callable)` | 无 |
| ContinuationAdapterEnvironment | `MaterializationRoot(source_callable)` | suspension site |
| CoroutineStep | `ExactOwnerRoot(result)` | 无 |
| BoxedValue | `ExactOwnerRoot(payload)` | 无 |
| CoroutineSlot | `ExactOwnerRoot(value)` | 无 |
| ObjectBackingClass | source object declaration的Cone | 无 |

| `GeneratedCallableKey` variant | 唯一root | 只作dependency、不得竞争root的字段 |
| --- | --- | --- |
| Lexical / CallableReferenceInvoke | emission relation给出的nearest parent `MaterializationRoot` | role/path |
| Initialization | 实现relation中的`MaterializationRoot({template=该generated id, context})` | template key中的声明级unit与role |
| DerivedEquality | `ExactOwnerRoot(exact_owner)` | 无 |
| FunctionAdapter / DynamicFunctionAdapter | `StructuralRoot(FunctionShape(target))` | source signature（如有） |
| StaticNoGcCallbackStorageBridge | `MaterializationRoot(source)` | signature |
| ForeignCallbackManagedAdapter | `CallbackRoot(application)` | 无 |
| CoroutineDriver | `MaterializationRoot(source_callable)` | 无 |
| ContinuationShell / CoroutineStart | `ExactOwnerRoot(result)` | role（如有） |
| CoroutineAdapter | `MaterializationRoot(source_callable)` | suspension site/role |
| FunctionBridge | referenced environment的`GeneratedNominalRoot` | target signature |
| DispatchAdjust | `ExactOwnerRoot(implementor)` | slot与target callable |
| BoxingAdjust | `ExactOwnerRoot(payload)` | slot与interface |

Lexical、Initialization与CallableReferenceInvoke的template id本身没有concrete materialization root；只有带第9.2节Strong/ODR signature subject及context relation的实现才可求根。Initialization template的声明级unit只约束source owner，不能覆盖该relation中的application unit。DispatchAdjust的target可以是继承来的别root callable、FunctionBridge target可以是别的function shape、itable interface也可以是别root exact type；这些都是canonical typed dependency，不因“另一个ref也能求根”改变表中owner。若必需dependency与当前root形成禁止的反向边、或一个variant无法满足表中的kind/context约束，则拒绝，而不是按first use、producer Cone或字段顺序选择。

validator从完整canonical key/typed relation重算上述可达性；producer Cone、object member、symbol、同layout或“当前只有一个候选”都不是provenance。无法回溯到恰好一个root的member拒绝，而不是任意归组。

`ExactOwnerRoot`落到source nominal定义Cone时，同时形成该定义artifact的materialization obligation，而不是授权consumer替别的Cone发Strong定义。M23-3对当前Cone已实际使用的param-free source exact subject闭合其box/coroutine/continuation等有限shape support；M23-5/6在开放跨Cone surface与LIR bridge时，要求每个可被下游合法请求的param-free exported exact subject把同一有限support closure非可选地包含在定义artifact的production proof中。consumer只能引用该owner，缺失时按required capability失败；不得改由当前Cone发同名Strong、临时Hidden或为source nominal伪造StructuralType组。nominal application和非nominal exact type仍分别由Nominal/Structural ODR组按需物化，不要求template定义Cone穷举应用。

M24 generic release hook使用`{group = 该owner的Nominal specialization group, role = ReleaseHook, discriminator = ExactType(该owner application)}`。validator必须逐字段证明group的`origin + arguments`与该`PersistentExactTypeId`的`NominalApplication` key一致；每个有release policy的generic exact owner恰有一个该member，hook body的ObjectDefinitionPlan owner就是它，`cb(CallableBodyKeyV2::ReleaseHook(owner))`取`OdrWeak`。另有且只有一个同组`RegistrationRecord/CallableBody(body id)` member供`cr`使用；TD relocation必须命中该`cb` entry，不得再为同一hook制造`CallableBody` member或`od`第二primary，release body也不得有`safepoint/sr`。若body调用verified pure C leaf bridge，canonical definition只引用unit并按前述规则把producer-local atom relocation正规化。

param-free hook不构造ReleaseHook ODR member；其body、callable registration与TD都继承同一个exact source subject，因而可按M23-7完整hidden proof统一为ConeStrong或TemplateSupportHidden。这样M24只启用M23-2已经赋义的role/matrix，不改变`OdrGroupId`、`OdrMemberKey`或`persistent-v1`；但M23 schema v1的artifact profile仍一律拒绝ReleaseHook semantic member。

M23-2对当前callable application生成`SpecializationKey::Callable`、对应`CallableBody` member与`OdrWeak` symbol，使同一generic owner/callable application从一开始就没有temporary Strong identity。但foundation profile不可发布、没有Link view；M23-3的production profile仍明确拒绝任何ODR member。直到M23-7补齐完整member闭包、ABI/definition fingerprint、object materialization和跨Cone member-set/definition一致性证明后，含ODR member的artifact才可取得Publishable/Link proof。M23-7不重定义上述group/member key。不使用FQN、symbol、producer Cone、object分片或全零bytes作占位。

### 5.3 identity record与顺序

每个kind有独立table：

```text
CborIdentityRecordV1<K> {
    id: PersistentKId,       // field 1, 32-byte byte string
    key: CanonicalKeyK,      // field 2, nested canonical CBOR item
}

RuntimeIdentityRecordV1<K> {
    id: PersistentKId,       // field 1, 32-byte byte string
    key_bytes: bytes,        // field 2, canonical runtime-encoder bytes
}
```

除callable body外，本阶段所有identity table都使用`CborIdentityRecordV1`，field 2是内嵌CBOR item，不是再包一层byte string。callable body table独占`RuntimeIdentityRecordV1<PersistentCallableBodyId>`，field 2是第6.2节可由typed runtime decoder完整消费并重编码相同的bytes；它不包含padding、host struct或CBOR mirror。两种record均严格只允许field 1/2。

每张table以该table内key的typed dependency边做stable Kahn topological order，ready set按raw id bytes排序；没有table-internal edge时等价于raw id严格递增。跨kind/table引用由整个foundation graph验证，不用field顺序假装依赖顺序。reader对每条record重算id，检查key中所有typed owner、source、binder和kind，再检查全表唯一性。不同kind使用不同table/decoder；相同32 bytes在不同kind中出现不会自动合并。

声明、exact type与其他derived graph都使用上述规则；无法给出全量topological order表示cycle或缺owner，不回退到插入顺序。

### 5.4 source/native ABI contract leaf

identity foundation不能让FFI继续靠symbol string串联三层。HIR先为每个合法`@Extern` function和extern global建立target-independent contract：

```text
SourceNativeExternalOwner =
    Function { id: PersistentFunctionId }                    // tag 1
  | Property { id: PersistentPropertyId }                    // tag 2

SourceNativeExternalContractKey { owner }                  // field 1

PersistentSourceNativeExternalContractId =
    DomainSeparatedCborHash("scoop-source-native-contract-id-v1", key)

SourceNativeExternalContractRecord {
    id: PersistentSourceNativeExternalContractId,            // field 1
    key: SourceNativeExternalContractKey,                   // field 2
    contract: SourceNativeExternalContract,                 // field 3
}
```

owner必须是同一条extern source declaration；其唯一source location由第4.5节`DefinitionOriginRecord::SourceNativeContract`承载，不在contract record复制第二份。contract内容变化进入HIR semantic fingerprint而不偷换declaration/contract identity。`SourceNativeLibraryBinding`为`DefaultNativeNamespace=1`或`LogicalLibrary=2 {1=CanonicalNativeLibraryName}`。`CanonicalNativeLibraryName`保存1…255 byte的有效UTF-8源码逻辑名：禁止NUL、`/`、`\`、ASCII control、首尾ASCII whitespace以及完整值`.`/`..`，不做Unicode normalization或大小写折叠；因此canonical bytes就是通过检查的原UTF-8 bytes，不含host搜索路径、扩展后的framework路径或locator。`SourceCAbiFunctionSignature`是map `1=array<SignatureTypeKey> parameters, 2=SourceCAbiReturn`，return为`Void=1`或`Value=2 {1=SignatureTypeKey}`；只有source `Unit` result编码Void，parameter不能是Unit。该schema也供generic callback registration复用，所以类型树本身允许合法binder；但`SourceNativeExternalContractRecord`的owner只能是top-level non-generic extern function/property，其constructor必须额外证明整份signature/storage binder-free。每个type都先透明展开alias并通过source C-FFI-safe predicate，不能包含参数名/default。`SourceScoopAbiFunctionSignature`使用相同map shape，但parameter/result保留完整Scoop signature type，result没有Void特例，extern constructor同样拒绝binder。

`SourceNativeExternalContract`的closed sum为：

| tag | variant | fields |
| ---: | --- | --- |
| 1 | `Function` | `1=symbol bytes, 2=SourceNativeLibraryBinding, 3=SourceExternFunctionAbi, 4=SourceCallingConvention` |
| 2 | `ReadOnlyData` | `1=symbol bytes, 2=library, 3=SignatureTypeKey` |
| 3 | `MutableData` | 同tag 2 |
| 4 | `ReadOnlyTls` | 同tag 2 |
| 5 | `MutableTls` | 同tag 2 |

source symbol是1…4095 byte、无NUL的annotation UTF-8值；target规范化前不宣称它就是object symbol bytes。`SourceExternFunctionAbi`是`C=1 {1=SourceCAbiFunctionSignature}`或`Scoop=2 {1=SourceScoopAbiFunctionSignature, 2=GcEffect}`；`GcEffect`为`Managed=1, NoGc=2`，`SourceCallingConvention`的唯一值为`Cdecl=1`。data/TLS不携带calling convention且只接受C-safe storage type。`CallbackMode`固定为`Reusable=1, OneShot=2`；callback的`SourceCAbiFunctionSignature`复用本段唯一schema。

仅有identity与field id不足以从不受信任artifact重算`@CLayout`或Scoop value ABI；因此foundation还携带一个**只服务native boundary闭包**、不公开为通用layout API的最小source witness。`CLayoutOverride`是`Natural=1`或`Bytes=2 {1=1|2|4|8|16}`：

```text
NativeBoundaryNominalOwnerV1 =
    Concrete { id: PersistentTypeId }                    // tag 1, field 1
  | GenericTemplate { id: PersistentGenericTypeId }      // tag 2, field 1

NativeBoundaryFieldDefinitionV1 {
    field: PersistentFieldId,            // field 1
    type: SignatureTypeKey,            // field 2
}

NativeBoundaryVariantDefinitionV1 {
    variant: PersistentEnumVariantId,                    // field 1
    fields: array<NativeBoundaryVariantFieldDefinitionV1>, // field 2
}

NativeBoundaryVariantFieldDefinitionV1 {
    field: PersistentEnumVariantFieldId, // field 1
    type: SignatureTypeKey,            // field 2
}

NativeBoundaryCLayoutPolicyV1 =
    NotCLayout                                             // tag 1
  | CLayout { aligned: CLayoutOverride,
              packed: CLayoutOverride }                 // tag 2, fields 1..2

NativeBoundaryNominalShapeV1 =
    Reference                                              // tag 1
  | Struct { c_layout: NativeBoundaryCLayoutPolicyV1,
             fields: array<NativeBoundaryFieldDefinitionV1> } // tag 2, fields 1..2
  | Enum { variants: array<NativeBoundaryVariantDefinitionV1> } // tag 3, field 1

NativeBoundaryTypeDefinitionRecordV1 {
    owner: NativeBoundaryNominalOwnerV1, // field 1
    type_parameter_count: u32,           // field 2
    shape: NativeBoundaryNominalShapeV1, // field 3
}
```

数组顺序就是source declaration order；不能按field/variant id排序后伪造顺序。Concrete/GenericTemplate owner、parameter count、source kind、field/variant owner与完整集合必须逐项等于同artifact的identity records；field type允许合法binder，但depth/index必须落入owner binder stack。Reference只接受class/interface/object/annotation class且不携带field列表；Struct/Enum分别只接受对应source kind。`CLayout`只接受带该annotation的struct并保存已完成constant evaluation的规范override，其他struct必须用`NotCLayout`。trusted core registry以相同逻辑提供`Unit`、Boolean、定宽integer、`Option`、`Ptr`/`FunPtr`、`PinnedPtr`/`GcHandle`及其他可命名core nominal的sealed witness，普通artifact不能覆写；tuple、managed function、raw/native pointer的结构边直接来自`ExactTypeKey`。

每个artifact的record集合精确覆盖其source extern、callback registration/application与target contract所引用exact/signature type的传递source-nominal闭包；无关额外record拒绝，避免改变fingerprint却没有用途。对generic application，validator从exact key的arguments按binder位置替换上述source shape，再按语言规范与`ValidatedLirTargetSelectionV1`重算：C-FFI-safe predicate、C storage/layout、Scoop value size/alignment/shape及Elided/Direct/Indirect选择。若闭包边指向direct dependency且当前profile没有相应closure proof，Graph仍可成功，但M23-2单artifactCompile以`SLIB_CAPABILITY_NATIVE_BOUNDARY_CLOSURE_REQUIRED`稳定失败，不返回“部分验证”的ABI对象；M23-6的`ValidatedArtifactClosure<Compile>`取得依赖witness与通用layout/scan section后完成它。M23-3的core-only profile只允许当前Cone加trusted core可闭合的boundary witness，不能跳过缺项。

LIR在当前LIR target profile下完成native symbol/calling-convention与canonical C storage/bridge正规化后产生唯一target-specific leaf。以下所有product按列出的field编号编码，所有sum仍使用`0=非零tag`：

```text
NativeExternalSymbolKey {
    target_profile: TargetProfileWireId,     // field 1
    native_link_symbol: bytes,               // field 2, 1..4096 bytes, no NUL
}

PersistentNativeExternalSymbolId =
    DomainSeparatedCborHash("scoop-native-link-symbol-v1", key)

NativeLibraryBinding =
    DefaultNativeNamespace                                  // tag 1
  | Requirement { id: NativeLinkRequirementId }             // tag 2, field 1
```

`NativeLinkRequirementKey`是map `1=TargetProfileWireId, 2=CanonicalNativeLibraryName, 3=NativeLibraryKind, 4=NativeLibraryGrouping`；kind为`TargetDefault=1, Dynamic=2, StaticArchive=3, Framework=4`，grouping为`Independent=1`或`OrderedGroup=2 {1=CanonicalNativeGroupName, 2=position:u32}`。`CanonicalNativeGroupName`使用与`CanonicalNativeLibraryName`完全相同的byte长度、UTF-8、禁止字符与“原bytes、不normalize”规则，但两者是不可转换的newtype；`position`从0开始、同group内必须严格连续且同一position唯一。`NativeLinkRequirementId = DomainSeparatedCborHash("scoop-native-link-requirement-v1", key)`。当前`lib`短式唯一映射到`TargetDefault/Independent`；M23-10 resolver只能为该typed requirement补provider evidence，不能把host路径写回key或在那时重新赋义group name。

`CanonicalCStorageType`是closed sum：`Integer=1 {1=exact type, 2=Signedness, 3=bit_width}`、`Boolean=2 {1=exact type}`、`DataPointer=3 {1=exact type, 2=CDataPointee, 3=CPointerStorage}`、`CodePointer=4 {1=exact type, 2=CanonicalCAbiSignatureFingerprint, 3=CPointerStorage}`、`Struct=5 {1=exact type, 2=CanonicalCAbiLayoutFingerprint}`。signedness为`Signed=1, Unsigned=2`，bit width只接受当前语言/target共同登记的8/16/32/64；pointee为`OpaqueUnit=1`或`ExactObject=2 {1=PersistentExactTypeId}`；pointer storage为`Direct=1`或`NullableWrapper=2 {1=PersistentExactTypeId}`。exact type与kind/provenance/layout必须逐项匹配，不能只凭相同size接受。

`Integer`的合法source映射也封闭：八种定宽integer按自身signedness/width映射；core `PinnedPtr<T>`与`GcHandle<T>`则是仅有的transparent carrier例外，必须保留完整wrapper application的`source_exact_type`，同时固定映射为`Unsigned + 64`，并验证nominal origin分别是trusted core登记的`PinnedPtr`/`GcHandle`、恰有一个concrete type argument且其ABI与`ULong`一致。它们不能改写为裸`ULong` exact type，也不能仅凭任意8-byte value冒充carrier。`Boolean`、data/code pointer和`@CLayout` struct仍走各自variant；Float/Double按第8.1节在M23 profile中拒绝。

```text
CanonicalCAbiParameter {
    source_exact_type: PersistentExactTypeId,               // field 1
    storage: CanonicalCStorageType,                        // field 2
}

CanonicalCAbiReturn =
    Void                                                     // tag 1
  | Value { source_exact_type: PersistentExactTypeId,
            storage: CanonicalCStorageType }               // tag 2, fields 1..2

CanonicalCAbiFunctionSignature {
    calling_convention: TargetCallingConvention,           // field 1
    parameters: array<CanonicalCAbiParameter>,              // field 2
    result: CanonicalCAbiReturn,                            // field 3
}
```

当前Darwin/AArch64 profile只登记`TargetCallingConvention::Cdecl=1`。这份signature是canonical C**源码storage contract**，刻意不手写或持久化平台register class、integer extension、`byval`/`sret`等真实C ABI分类；所有C function/global/callback都由M12的canonical generated-C bridge交给`ValidatedCBridgeToolchainProfileV1`指定的system C compiler完成该侧lowering，Scoop侧桥接入口使用本设计已经类型化的storage ABI。decoder从exact type与C layout closure重算每项storage，并由后续generated-bridge capability验证bridge source/object及其compiler/toolchain evidence；不能从host ABI默认值补一个pass mode。`CanonicalCAbiSignatureFingerprint = DomainSeparatedCborHash("scoop-c-abi-signature-v1", CanonicalCAbiFunctionSignature)`。

```text
CanonicalCAbiLayoutField {
    field: PersistentFieldId,                    // field 1
    offset: u64,                                 // field 2
    storage: CanonicalCStorageType,            // field 3
}

CanonicalCAbiLayout {
    exact_type: PersistentExactTypeId,            // field 1
    byte_size: u64,                               // field 2
    alignment: NonZeroU64,                        // field 3
    aligned: CLayoutOverride,                   // field 4
    packed: CLayoutOverride,                    // field 5
    fields: array<CanonicalCAbiLayoutField>,    // field 6, declaration order
}
```

field offset/alignment/size均checked，field identity/owner/order必须与source `@CLayout` struct一致；nested Struct storage引用其已验证layout fingerprint，layout graph必须无by-value cycle。`CanonicalCAbiLayoutFingerprint = DomainSeparatedCborHash("scoop-c-abi-layout-v1", CanonicalCAbiLayout)`。

Scoop ABI contract不复用C pass mode。`CanonicalScoopStorage`是map `1=PersistentExactTypeId, 2=byte_size:u64, 3=alignment:NonZeroU64, 4=ScoopAbiValueShape`，shape为`Scalar=1, Aggregate=2`；ZST必须`byte_size=0`，non-ZST必须大于0，shape、size和alignment都由当前target profile重算。`CanonicalScoopAbiFunctionSignature`是map `1=ExactCallableSignature, 2=array<ScoopAbiArgument>, 3=ScoopAbiReturn, 4=GcEffect`；argument为`ElidedZst=1 {1=CanonicalScoopStorage}`、`Direct=2 {1=storage}`或`Indirect=3 {1=storage}`，return为`UnitVoid=1`、`ElidedZst=2 {1=storage}`、`Direct=3 {1=storage}`或`Indirect=4 {1=storage}`。Elided只接受size 0，Direct/Indirect只接受nonzero size并符合profile的scalar/aggregate passing规则；signature中的exact type必须逐位置等于storage exact type。field 4必须逐项等于source `SourceExternFunctionAbi::Scoop`的Managed/NoGc effect；它与`ExactCallableSignature.effect`的Ordinary/Suspend轴正交，不能因两者都是“effect”而互相推导。这样同symbol与同物理value shape的Managed/NoGc extern仍产生不同contract fingerprint与typed callee-effect proof；两者都保持M15的`NativeBorrowed` transition与caller-root publication，`NoGc`不能偷降为普通`NoGc` callsite。M23-6会以新required ABI/layout section提供可跨Cone复用的完整layout/scan证明，但不改变这里已经冻结的extern physical signature bytes。

`NativeExternalContract`的tag和field固定为：`Function=1 {1=NativeLibraryBinding, 2=NativeExternAbi, 3=TargetCallingConvention}`、`ReadOnlyData=2`、`MutableData=3`、`ReadOnlyTls=4`、`MutableTls=5`，后四者均为`{1=library, 2=CanonicalCStorageType}`。`NativeExternAbi`为`C=1 {1=CanonicalCAbiFunctionSignature}`或`Scoop=2 {1=CanonicalScoopAbiFunctionSignature}`。data/TLS只用C storage；C function的field 3必须逐tag等于其C signature field 1，Scoop function则只在外层field 3保存由source contract正规化出的calling convention，内层Scoop signature不复制第二份。当前二者唯一合法值均为`Cdecl=1`，但validator仍按上述分支检查，不能读取不存在的Scoop内层字段或默认补值。

```text
NativeExternalContractFingerprintInput {
    symbol_id: PersistentNativeExternalSymbolId, // field 1
    contract: NativeExternalContract,            // field 2
}

NativeExternalContractFingerprint =
    DomainSeparatedCborHash("scoop-native-external-contract-v1", input)

NativeExternalContractRecord {
    source: PersistentSourceNativeExternalContractId, // field 1
    symbol_id: PersistentNativeExternalSymbolId,      // field 2
    symbol_key: NativeExternalSymbolKey,             // field 3
    fingerprint: NativeExternalContractFingerprint,    // field 4
    contract: NativeExternalContract,                  // field 5
}
```

reader分别重算symbol id与contract fingerprint，再验证source contract经当前target/library registry规范化后逐字段得到该record；不得从symbol bytes反推ABI、把bare digest当leaf或让alias/re-export另造contract。HIR field 26保存source record，LIR field 14保存本段target record，完整record进入对应semantic fingerprint。

## 6. exact type、callable body与runtime id

### 6.1 `ExactTypeKey`

`ExactTypeKey`沿用第5.1节结构type的1…6 tag，但不允许`Binder`，所有child都是`PersistentExactTypeId`：

| tag | variant | payload field |
| ---: | --- | --- |
| 1 | `Nominal` | `1=PersistentTypeId` |
| 2 | `NominalApplication` | `1=PersistentGenericTypeId, 2=non-empty array<PersistentExactTypeId>` |
| 3 | `Tuple` | `1=non-empty array<PersistentExactTypeId>` |
| 4 | `Function` | `1=effect, 2=array<PersistentExactTypeId>, 3=result` |
| 5 | `RawPointer` | `1=pointee` |
| 6 | `NativeFunctionPointer` | `1=calling_convention, 2=array<PersistentExactTypeId>, 3=result` |

`PersistentExactTypeId = DomainSeparatedCborHash("scoop-exact-type-v1", ExactTypeKey)`。primitive/`Unit`都是core nominal，tuple和function是结构type，`RawPointer`与managed nominal、`NativeFunctionPointer`与managed function互不转换。nominal ref作为leaf，exact table对其他exact edge做cycle检查并以dependency-first顺序编码。

`CanonicalExactTypeDiagnosticName`完全沿用M23总设计3.1的ASCII grammar。M23-2实现只能从已验证identity graph生成它，并用memoized subtree cost在分配前实施16 MiB单字段上限；wire中若冗余保存diagnostic name，reader必须重算后逐byte相等。

### 6.2 callable body v1

`PersistentCallableBodyId`不是source function id的alias。其key使用runtime metadata canonical encoder v1，而非CBOR：

```text
CallableBodyKey =
    Strong { owner: StrongCallableDefinitionOwner }                 // tag 1
  | Odr { member: CallableOdrMemberId }                              // tag 2
  | RootGateway { root_cone: ConeIdentity, main: MainCallableBodyId }// tag 3
  | InitializationStartupGateway { unit: PersistentInitializationUnitId } // tag 4

PersistentCallableBodyId =
    SHA-256(ByteSpan("scoop-callable-body-v1") || RuntimeEncode(key))
```

variant tag是小端u32，product按声明顺序编码。`StrongCallableDefinitionOwner`是typed sum，tag固定为`Function=1, Constructor=2, PropertyAccessor=3, GeneratedCallable=4`，只带对应persistent id。`CallableOdrMemberId`与`MainCallableBodyId`是refinement，前者只接受callable ODR member，后者只能由root Cone符合entry规则的top-level ordinary non-generic `() -> Unit` Strong body得到。

M23-2当前pipeline迁移中，param-free source body以及按第7.2节递归后**唯一回到某个Cone**的source-anchored generated body生产`Strong`；param-free本身不等于Cone-owned或OdrOwned。origin-free shape helper严格执行第5.2节root表：例如FunctionAdapter的function shape进入Structural组，box/coroutine helper若以source nominal exact type为root则回到该声明Cone，以nominal application为root则进入Nominal组，只有非nominal exact root才进入StructuralType组；不得因“全局复用”四字统一强制成某一类。任何`PersistentCallableApplicationId`同样先构造Callable specialization/group/member，再生产`Odr` body与`OdrWeak` symbol，即使它暂时只在一个Cone内materialize也不允许降级成temporary Strong。generic body内部的closure/coroutine/adapter以回溯到同一application的generated callable作为member discriminator。legacy executable shim只调用main的Strong body。`RootGateway`与`InitializationStartupGateway`的typed producer在M23-3接入；M23-7补齐ODR完整member集合与definition proof，但不改本阶段已经使用的`Odr` key。unknown tag必须拒绝，不能降级为Strong或一般function。M24改用body-v2时三层schema整体升级，不在v1尾部插入variant。

### 6.3 safepoint site

LIR创建call/invoke/poll时只记录closed `SafepointSiteRole`，不立即分配全局数字。identity role与runtime registration使用同一套已冻结tag：`ManagedPoll=1, ManagedCall=2, ManagedInvoke=3, NativeSafeTransition=4, NativeBorrowedTransition=5`。不另建single-use的entry/loop/array/global role namespace。

CFG fold、prune与loop analysis完成后才插poll。每个非NoGC function的entry block与每个canonical loop header取set union，每block最多插一个`ManagedPoll`；entry同时是loop header也只有一个site。如果block以`LandingPad`或`CleanupPad`开头，poll紧随该pad，否则是第一条instruction。

然后对每个`PersistentCallableBodyId`独立执行canonical reverse-postorder。先从entry执行标记DFS，按下述semantic successor顺序递归，离开block时追加postorder，最后反转整个postorder list。普通block从terminator取successor：`Br`一个，`CondBr`先then后else，`Return/Resume/Unreachable`为空。如果block最后一条instruction是`Invoke`，要求其terminator恰为`Br(invoke.normal)`，semantic successor只取`[normal, unwind]`，不再把冗余`Br(normal)`算第二次。其他位置的Invoke、不匹配的redundant branch或prune后仍不可达的block都是LIR结构错误。block内按instruction顺序收集site。

每条instruction到identity/runtime role的映射唯一为：

| LIR site/instruction | role |
| --- | --- |
| `ManagedPoll` | `ManagedPoll` |
| `Call::Managed`, `ArrayAlloc`, `ArrayAssembly`, `ArrayClone` | `ManagedCall` |
| `Invoke::Managed` | `ManagedInvoke` |
| `Call::NativeSafe`, `NativeGlobalLoad`, `NativeGlobalStore`, `NativeGlobalAddress` | `NativeSafeTransition` |
| `Call::NativeBorrowed` | `NativeBorrowedTransition` |
| `Call::NoGc`, `Invoke::NoGc`及其余instruction | 无safepoint site |

ordinal是上述canonical traversal中**同role**之前site数量，u32 checked。runtime registration的`site_role`必须逐tag等于该identity role，不再从instruction spelling二次推导。

```text
SafepointSiteKey { owner: PersistentCallableBodyId, role, ordinal }
PersistentSafepointSiteId =
    DomainSeparatedCborHash("scoop-safepoint-site-v1", key)
```

它不读`BlockId`、instruction arena id、function vector ordinal或lowering线程顺序。重排不影响CFG语义的arena不改id；改变canonical CFG/site序列则必须改id和LIR fingerprint。

### 6.4 nonzero 64-bit派生

```text
RuntimeTypeIdDigest =
    SHA-256(ByteSpan("scoop-runtime-type-id-v1") || raw(PersistentExactTypeId))

SafepointIdDigest =
    SHA-256(ByteSpan("scoop-safepoint-id-v1") || raw(PersistentSafepointSiteId))

u64 value = little_endian_u64(digest[0..8])
```

两种public id都包装`NonZeroU64`。派生值为0时返回`DerivedIdIsZero`，不重hash、加salt或换取其他digest slice。每个artifact在LIR foundation保存`{full persistent id, derived u64}`全量表，reader重算并检查u64唯一；两个不同full id截断到同u64是`DerivedIdCollision`。同一规则在以后的全program closure重做，不将“单artifact未碰撞”当成program proof。

## 7. `PersistentV1` mangler

### 7.1 唯一字符串语法

```text
scoop$1$<kind-tag>$<64 lowercase hexadecimal digits>
```

prefix、`$`、schema十进制字面量和kind tag均为ASCII，hex不允许大写或缩写。mangler入口是封闭`PersistentSymbolKeyV1`，不是`mangle(&str, &[u8])`：

| key tag | kind tag | symbol key owner | 用途 |
| ---: | --- | --- | --- |
| 1 | `cb` | `PersistentCallableBodyId` | Scoop callable body/gateway |
| 2 | `ss` | `PersistentStaticStorageId` | static/global/delegate/failure storage |
| 3 | `io` | `PersistentImmortalObjectId` | immortal object/string/constant atom |
| 4 | `td` | `PersistentExactTypeId` | TypeDescriptor |
| 5 | `ly` | `PersistentLayoutId` | layout helper/record |
| 6 | `sp` | `PersistentScanId` | scan program |
| 7 | `dt` | `PersistentDispatchTableId` | vtable/itable |
| 8 | `ds` | `PersistentDispatchSlotId` | dispatch slot anchor（不是implementation thunk） |
| 9 | `ic` | `PersistentInitializationUnitId` | initialization cell |
| 10 | `id` | `PersistentInitializationUnitId` | initialization descriptor |
| 11 | `rr` | `PersistentStaticStorageId` | root registration record |
| 12 | `ir` | `PersistentImmortalObjectId` | immortal registration record |
| 13 | `nr` | `PersistentInitializationUnitId` | initialization registration record |
| 14 | `tr` | `PersistentExactTypeId` | type registration record |
| 15 | `sr` | `PersistentSafepointSiteId` | safepoint registration/stackmap owner |
| 16 | `cr` | `PersistentCallableBodyId` | callable registration record |
| 17 | `im` | `ConeIdentity` | image descriptor |
| 18 | `br` | `GeneratedBridgeAtomId` | generated native bridge entry/support atom |
| 19 | `od` | `OdrMemberId` | ODR member primary atom |
| 20 | `bs` | `ObjectDefinitionAtomId` | verifier start boundary |
| 21 | `be` | `ObjectDefinitionAtomId` | verifier end boundary |

`PersistentSymbolKeyV1`在wire上是`0=key tag, 1=typed owner`的封闭sum，key tag与上表一一对应。`PersistentSymbolRequestV1`是map `1=PersistentSymbolKeyV1, 2=LinkageClass`，不冗余存储symbol text；decoder验证后机械重生成字符串。request table按`(key tag, owner raw id)`排序，同key的两个linkage request是冲突而不是两条合法record。

`OdrMemberId`与`ObjectDefinitionPlanId`的完整identity key都已在第5.2节冻结；M23-3/M23-7只增加expected atom/associated-role闭包、definition payload与物理materialization relation。M23-2 producer不接受raw bytes冒充typed plan owner。generated bridge与boundary都使用本阶段按语义subkey定义的atom id；同一owner若有多个link-visible atom，必须先产生不同typed derived owner，不在hex后追加临时suffix或排序index。

### 7.2 linkage与例外

`MangledSymbolV1`与`LinkageClass`分开。linkage封闭为`ConeStrong=1, TemplateSupportHidden=2, OdrWeak=3, RuntimeAbi=4`；语言visibility不直接cast为linkage。`RuntimeAbi` tag供相邻的runtime/native definition contract共用同一closed enum，但在全部21种`PersistentSymbolRequestV1`中都非法；runtime ABI永远不取得PersistentV1 symbol。

validator不从linkage反猜owner，而是先从完整identity graph计算一个唯一的、只存在于validated view中的`EmissionRootV1 = ConeOwned { producer: ConeIdentity, subject: ConeEmissionSubjectV1 } | OdrOwned { group: OdrGroupId, member: OdrMemberId }`。`ConeEmissionSubjectV1`是validated-only closed sum，精确区分source declaration/generated callable root、source nominal exact root、initialization unit、static storage、immortal object、dispatch slot、root gateway、Cone image与generated bridge atom；它不编码到wire，也不接受裸digest。layout/scan/table/type registration共享exact subject，cell/descriptor/init registration共享unit subject，root registration共享storage subject，immortal registration共享object subject，site/callable registration共享body subject，bs/be共享plan所指subject：

- source declaration、property、source object与其param-free、source-anchored generated descendant递归回溯到声明的origin Cone；body的`Strong` constructor只在owner恰好解析到一个Cone时合法；
- callable application及其application-specific generated descendant回溯到`SpecializationKey::Callable`；generic delegated unit回溯到`DelegatedProperty`；nominal application回溯到`Nominal`；tuple/function/raw pointer/native function pointer回溯到以自身exact type为root的`StructuralType`；
- generated nominal/callable的闭包、frame、continuation、adapter、box、coroutine step/slot、derived equality、dispatch/boxing adjust等严格使用第5.2节已经冻结的owner/materialization/root函数：`NoSubstitution`的source-anchored实体回到唯一Cone，`Application`回到对应Callable组，`InitializationApplication`回到对应unit的Cone或DelegatedProperty组；shape helper按表选择`ExactOwnerRoot`或仅接受非nominal exact type的`StructuralRoot`，前者仍可能得到source Cone、Nominal组或StructuralType组；找不到root、找到两个root或跨root混合都拒绝；
- `InitializationUnitKey`的前四个variant回到声明Cone，`GenericDelegatedExtensionApplication`回到对应DelegatedProperty组；`RootGateway`回到`root_cone`，`InitializationStartupGateway`继承unit root；
- 每个`OdrOwned`结果还必须定位第5.2节中role/discriminator正确、root provenance完整的唯一member record。只有group id而没有member，或只有同role的别组member，都不能获得`OdrWeak`。

21种symbol key到root/member的映射是穷尽表；`XRoot`表示按上段对X的canonical key递归求根：

| key tag | root函数 | allowed linkage | ODR时要求的member role/discriminator或额外约束 |
| ---: | --- | --- | --- |
| 1 `cb` | `BodyRoot(body)` | C/H/O | `Odr`直接使用其member；generic startup gateway使用`CallableBody/InitializationUnit`；M24 release body使用`ReleaseHook/ExactType` |
| 2 `ss` | `StorageRoot(storage)` | C/H/O | `StaticStorage/StaticStorage` |
| 3 `io` | `ImmortalRoot(object)` | C/H/O | `ImmortalObject/ImmortalObject` |
| 4 `td` | `ExactRoot(exact type)` | C/H/O | `TypeDescriptor/ExactType` |
| 5 `ly` | `ExactRoot(layout.exact_type)` | C/H/O | `Layout/Layout` |
| 6 `sp` | `LayoutRoot(scan.layout)` | C/H/O | `ScanProgram/Scan` |
| 7 `dt` | `ExactRoot(table.exact_type)` | C/H/O | `DispatchTable/DispatchTable`；optional interface是可跨另一root的typed dependency，不改变implementor table root |
| 8 `ds` | dispatch declaration的origin Cone | C/H | 只表示slot anchor；application-specific adjust thunk改用`cb`，不能只按slot id命名 |
| 9 `ic` | `InitializationUnitRoot(unit)` | C/H/O | `InitializationCell/InitializationUnit` |
| 10 `id` | `InitializationUnitRoot(unit)` | C/H/O | `InitializationDescriptor/InitializationUnit` |
| 11 `rr` | `StorageRoot(storage)` | C/H/O | `RegistrationRecord/StaticStorage` |
| 12 `ir` | `ImmortalRoot(object)` | C/H/O | `RegistrationRecord/ImmortalObject` |
| 13 `nr` | `InitializationUnitRoot(unit)` | C/H/O | `RegistrationRecord/InitializationUnit` |
| 14 `tr` | `ExactRoot(exact type)` | C/H/O | `RegistrationRecord/ExactType` |
| 15 `sr` | `BodyRoot(site.owner)` | C/H/O | `RegistrationRecord/SafepointSite`，site必须属于同一body member |
| 16 `cr` | `BodyRoot(body)` | C/H/O | `RegistrationRecord/CallableBody` |
| 17 `im` | key中的Cone | C | producer必须等于artifact Cone |
| 18 `br` | atom key中的producer Cone | C | atom只能是可materialize的tag 1…3，producer必须等于artifact Cone |
| 19 `od` | key中member的group | O | 只接受下文没有kind-specific primary key的member |
| 20 `bs` | `ObjectDefinitionPlanRoot(atom.plan)` | C/H/O | 精确继承plan primary |
| 21 `be` | 与同atom的`bs`相同 | C/H/O | 与`bs`逐项相同，不得另选linkage/root |

表中C/H/O分别是ConeStrong、TemplateSupportHidden、OdrWeak；RuntimeAbi对全部21行都非法。每个ObjectDefinitionPlan恰有一个primary symbol key，ODR member也不能同时发一个kind-specific primary和`od` alias。primary选择固定为：CallableBody/DispatchAdapter→`cb`；Layout→`ly`；ScanProgram→`sp`；TypeDescriptor→`td`；DispatchTable→`dt`；StaticStorage→`ss`；ImmortalObject→`io`；InitializationCell/Descriptor→`ic`/`id`；RegistrationRecord按discriminator唯一映射`rr/ir/nr/tr/sr/cr`；ReleaseHook→对应body的`cb`。`od`只接受确有物理primary的GeneratedNominal，以及没有上述kind-specific key的DiagnosticBytes、AddressTakenConstant（两种合法discriminator）和ObjectSupport；AddressTakenConstant即使以ImmortalObject作provenance也按member id命名，不能争用该object自身`ImmortalObject` member的`io(id)`。同member双primary、两个member争同primary、两个primary指同range或用alias让Mach-O不同symbol各选不同producer都拒绝。

`ConeOwned`默认且在M23-2唯一可证明的linkage是`ConeStrong`。`TemplateSupportHidden`不是“private等于hidden”的cast：M23-5最多为source root建立visibility/access eligibility；M23-7的required HIR template-support section才从实际Export template body证明`{producer, template owner, source support subject}`的reachability，随后M23-7 LIR/object section把该subject投影为上述`ConeEmissionSubjectV1`并闭合全部派生definition。两个proof都存在且一致后，Link/production profile才可把**整个subject**收窄成`TemplateSupportHidden`；同一subject下的body/site/registration/boundary或exact/layout/scan/TD/registration不得各自选择不同linkage，同key也不能重复请求。可被最终claim的key tag封闭为`cb/ss/io/td/ly/sp/dt/ds/ic/id/rr/ir/nr/tr/sr/cr/bs/be`；Cone-owned initialization startup gateway随其unit subject继承，M24 param-free ReleaseHook及其registration随exact source subject继承，所以“strong”可具体为ConeStrong或TemplateSupportHidden。只有root gateway、image和generated bridge永远不得claim。foundation profile一律拒绝linkage 2；M23-3 production profile同样只接受`ConeStrong`并拒绝linkage 2/3，M23-5 eligibility单独存在也不开放linkage，M23-7 profile才随完整required proof开放。

`OdrOwned`只能选择`OdrWeak`，`ConeOwned`不能选择`OdrWeak`。因此constructor的最终判定精确为：先求root，再检查上表和可选hidden-support proof，最后比较request linkage；不允许用“当前只有一个producer”、语言visibility、symbol spelling或native linker容忍重复来降级/升级。

`@Extern`指定的native symbol与runtime固定C ABI symbol走`NativeExternalSymbol`/`RuntimeAbiSymbol`独立type，不进mangler。M23-1旧runner需要的`scoop_main`仅由driver/codegen的`LegacyExecutableShim`定义，其body只调用已有`cb`持久symbol；同一Scoop main body不同时发射CompactV2与PersistentV1两个symbol。shim不进`.slib`、identity和semantic fingerprint，M23-11删除。

`ManglingSchemaIdentity`可保留`CompactV2`用于诊断输入不兼容，active producer唯一值为`PersistentV1`且不实现`Default`。任何`format!`手工Scoop symbol、arena ordinal discriminator、source ordinal stem或通过临时伪`Module`调用mangler的production path都在本阶段删除。

## 8. `.slib` bootstrap manifest与typed directory

### 8.1 `BootstrapManifestV1`

`manifest.cbor`的top-level map字段精确为：

| field | value |
| ---: | --- |
| 1 | magic byte string `SCOOPSLIB` |
| 2 | `manifest_schema = 1` |
| 3 | `container_schema = 1` |
| 4 | `producer: ProducerRecordV1` |
| 5 | `compatibility: CompatibilityRecordV1` |
| 6 | `cone: ConeRecordV1` |
| 7 | `direct_dependencies: array<DependencyRecordV1>` |
| 8 | `members: array<SlibMemberRecordV1>` |
| 9 | `semantic_fingerprints: SemanticFingerprintRecordV1` |
| 10 | `sections: array<ManifestSectionV1>` |
| 11 | `artifact_fingerprint: ArtifactFingerprint` |

`ProducerRecordV1` field 1是最多255 UTF-8 byte的compiler version，只作诊断与whole-envelope区分：它不进入HIR/MIR/LIR/Code/RuntimeImage semantic fingerprint，但作为bootstrap manifest的一部分会改变`ArtifactFingerprint`。`CompatibilityRecordV1`精确字段为：

| field | value |
| ---: | --- |
| 1 | `LanguageAbiFingerprint` |
| 2 | `RuntimeAbiFingerprint` |
| 3 | `identity_schema: NonZeroU32 = 1` |
| 4 | `ManglingSchemaIdentity`，CBOR text恰为`persistent-v1` |
| 5/6 | `TargetProfileWireId` / `TargetProfileFingerprint` |
| 7/8 | `BackendProfileWireId` / `BackendProfileFingerprint` |
| 9/10/11 | `hir_schema/mir_schema/lir_schema: NonZeroU32 = 1/1/1` |
| 12 | `CompositeIdentityAbiFingerprint` |
| 13/14 | `ArtifactCapabilityProfileId` / `ArtifactCapabilityProfileFingerprint` |

上述六类fingerprint均是不可互换的typed 32-byte digest。`ManglingSchemaIdentity`不是数字version：v1 decoder是封闭text enum，只接受`persistent-v1`；`compact-v2`可作为旧输入诊断值但M23 producer/consumer不把它视为同family的较小版本。Graph/Compile要求当前toolchain认识所有profile且exact match，不以producer version text判断兼容。

其中language/runtime ABI leaf不是未定义的外部常量。M23 registry冻结：

```text
LanguageAbiContractV1 {
    revision: M23LanguageAbi = 1,              // field 1
}

RuntimeAbiContractV1 {
    object_and_gc_contract: M23MovingImmixV1 = 1, // field 1
    runtime_metadata_record_abi: u32 = 1,          // field 2
    type_descriptor_contract: M23NoReleaseHookV1 = 1, // field 3
}

LanguageAbiFingerprint =
    DomainSeparatedCborHash("scoop-language-abi-contract-v1",
                            LanguageAbiContractV1)
RuntimeAbiFingerprint =
    DomainSeparatedCborHash("scoop-runtime-abi-contract-v1",
                            RuntimeAbiContractV1)
```

两份contract bytes分别是`a10101`与`a3010102010301`，M23固定digest分别为`2d2188ce81a619a325eeb6b4475bd4ac1dc0a220e463f3858e58f26ef21c9c96`和`72be5e11c123875bccf1dcad1d91fd5ae870e076ae91c1c2a264a3dcc0e56a8e`。它们是人工升代、由规范各章赋义的兼容revision，不是对Markdown文件bytes做hash；任何影响跨Cone语言解释的变更必须换language revision，任何影响object/GC/runtime调用与metadata/TypeDescriptor解释的变更必须换runtime contract值/domain并更新golden。M24至少把language revision换为新variant、把TypeDescriptor contract换为带release hook的variant，因此两项都与M23不同；不能只靠HIR/LIR schema字段掩盖ABI变化。

`CompositeIdentityAbiFingerprint`必须由reader重算，不能相信manifest中的冗余digest：

```text
IdentityAbiDescriptorV1 {
    language_abi: LanguageAbiFingerprint,          // field 1
    runtime_abi: RuntimeAbiFingerprint,             // field 2
    identity_schema: NonZeroU32,                    // field 3
    wire_cbor_schema: NonZeroU32,                   // field 4 = 1
    hash_framing_schema: NonZeroU32,                // field 5 = 1
    callable_body_schema: NonZeroU32,               // field 6 = 1
    mangling_schema: ManglingSchemaIdentity,        // field 7
    runtime_type_id_derivation_schema: NonZeroU32,  // field 8 = 1
    safepoint_id_derivation_schema: NonZeroU32,     // field 9 = 1
    runtime_metadata_encoder_abi: NonZeroU32,       // field 10 = 1
    hir_schema: NonZeroU32,                         // field 11
    mir_schema: NonZeroU32,                         // field 12
    lir_schema: NonZeroU32,                         // field 13
}

CompositeIdentityAbiFingerprint =
    DomainSeparatedCborHash("scoop-composite-identity-abi-v1",
                            IdentityAbiDescriptorV1)
```

M23上述descriptor的Wire CBOR v1与固定fingerprint为：

```text
ad0158202d2188ce81a619a325eeb6b4475bd4ac1dc0a220e463f3858e58f26ef21c9c9602582072be5e11c123875bccf1dcad1d91fd5ae870e076ae91c1c2a264a3dcc0e56a8e0301040105010601076d70657273697374656e742d7631080109010a010b010c010d01
CompositeIdentityAbiFingerprint = f4e2a5c02cedc7b713a65e4165d3b8182fe831237ee2e00d540ca07879f042e2
```

该向量非可选地锁定field 3…6、8…13均为1以及`mangling_schema = persistent-v1`；不能只比较前两个ABI leaf后由consumer补默认值。

M24把callable body与三层schema改为2并改变runtime ABI，因而必然得到不同值；prefixed runtime record自身的ABI version仍是独立维度。target/backend profile各自以`DomainSeparatedCborHash("scoop-target-profile-contract-v1", {1=id,2=canonical contract})`与`DomainSeparatedCborHash("scoop-backend-profile-contract-v1", {1=id,2=canonical contract})`计算。registry要求一个profile id永久绑定唯一canonical contract/fingerprint；改变contract必须提升capability major，不能复用同id后只换digest。

M23唯一target contract是以下closed product；所有整数都是CBOR unsigned，所有enum tag都是独立的nonzero u32 type：

```text
TargetProfileContractV1 {
    canonical_triple: text,                         // field 1
    llvm_data_layout: text,                         // field 2
    object_format: ObjectFormatId,                  // field 3
    byte_order: ByteOrderV1,                        // field 4
    scalar_layouts: array<ScalarLayoutRecordV1>,    // field 5
    managed_pointer_layout: PlainPointerLayoutV1,   // field 6
    data_pointer: QualifiedPointerLayoutV1,         // field 7
    code_pointer: QualifiedPointerLayoutV1,         // field 8
    metadata_pointer_layout: PlainPointerLayoutV1,  // field 9
    stack_alignment_bytes: u64,                     // field 10
    maximum_managed_alignment: u64,                 // field 11
    maximum_managed_object_size: u64,               // field 12
    scoop_abi_classifier: ScoopAbiClassifierV1,     // field 13
    c_abi_lowering: CAbiLoweringProfileV1,           // field 14
    native_symbol_normalization: NativeSymbolNormalizationV1, // field 15
}

ScalarLayoutRecordV1 { kind, size_bytes, alignment_bytes } // fields 1..3
PlainPointerLayoutV1 { size_bytes, alignment_bytes }        // fields 1..2
QualifiedPointerLayoutV1 {
    size_bytes, alignment_bytes, null_encoding, carrier     // fields 1..4
}
```

`DarwinAarch64V1`的field 1…15精确取：`"aarch64-apple-darwin"`；`"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"`；`org.scoop-lang.object-format/mach-o-relocatable/1`；`Little=1`；按kind严格递增的`I1=1:(1,1), I8=2:(1,1), I16=3:(2,2), I32=4:(4,4), I64=5:(8,8)`；`(8,8)`；`(8,8, AllZeroBits=1, BitPreservingU64=1)`；同前；`(8,8)`；`16`；`16`；`9,223,372,036,854,775,807`；`ElideZeroSizedDirectScalarIndirectAggregate=1`；`GeneratedBridgeSystemCCompiler=1`；`MachOExternalUnderscore=1`。array不得遗漏、重复或改变scalar顺序。Float/Double尚未进入当前编译器实现子集，也不是M23 C-FFI-safe storage，因此不得伪装成I32/I64；未来启用F32/F64必须扩展scalar/storage contract并提升target profile major。

`NativeSymbolNormalizationV1::MachOExternalUnderscore`把1…4095 byte、无NUL且不以LLVM escape byte `0x01`开头的logical symbol逐byte映射为`0x5f || logical`，输出恰为2…4096 byte；不做Unicode、大小写或已有underscore折叠。固定向量是`foo → _foo`、`_foo → __foo`、`scoop$1$cb$<64hex> → _scoop$1$cb$<64hex>`。Scoop LLVM object与generated-C bridge object verifier都必须应用同一policy；`NativeExternalSymbolKey.native_link_symbol`保存输出bytes，不能一个保存logical C name、另一个保存Mach-O `nlist` name。第7章的`MangledSymbolV1`语法描述logical Scoop symbol；codegen/object verifier以本field唯一映射到真实object symbol。

该contract的Wire CBOR v1为：

```text
af0174616172636836342d6170706c652d64617277696e027847652d6d3a6f2d703237303a33323a33322d703237313a33323a33322d703237323a36343a36342d6936343a36342d693132383a3132382d6e33323a36342d533132382d466e333203a301781c6f72672e73636f6f702d6c616e672e6f626a6563742d666f726d617402726d6163682d6f2d72656c6f63617461626c65030104010585a3010102010301a3010202010301a3010302020302a3010402040304a301050208030806a20108020807a4010802080301040108a4010802080301040109a2010802080a100b100c1b7fffffffffffffff0d010e010f01
```

连同profile id按上述公式计算的`TargetProfileFingerprint`固定为`42697b4e4e2102ef19d81bd624f7e428065d2ddff90279f1fc458bfcfdb13671`。

M23唯一backend contract同样是closed product：

```text
BackendProfileContractV1 {
    llvm_api_version: { major, minor },              // field 1, inner 1..2
    inkwell_binding: { major, minor, feature },      // field 2, inner 1..3
    target_cpu: text,                                // field 3
    target_features: text,                           // field 4
    optimization: OptimizationProfileV1,             // field 5
    relocation: RelocationProfileV1,                 // field 6
    code_model: CodeModelProfileV1,                  // field 7
    object_emission: ObjectEmissionProfileV1,        // field 8
    machine_pipeline: MachinePipelineProfileV1,      // field 9
    managed_address_space: u32,                      // field 10
    stack_map_version: u32,                          // field 11
    statepoint_roots: StatepointRootProfileV1,       // field 12
    post_ra_statepoint_fixup: RequirementV1,         // field 13
    deopt_live_in: PermissionV1,                     // field 14
    raw_backend_options: PermissionV1,               // field 15
    frame_pointers: FramePointerProfileV1,           // field 16
    tail_calls: TailCallProfileV1,                   // field 17
    merge_functions: MergeFunctionsProfileV1,       // field 18
    llvm_target_backend: LlvmTargetBackendV1,        // field 19
    unwind_model: UnwindModelV1,                     // field 20
    personality_abi: PersonalityAbiV1,               // field 21
    exception_data_registers: u32,                   // field 22
    lsda_encodings: { lp_start, type_table, call_site }, // field 23, inner 1..3
    unwind_provider: UnwindProviderV1,               // field 24
    artifact_inspection: ArtifactInspectionV1,       // field 25
}
```

`Llvm22_1V1`精确取`{22,1}`、`{0,10,Llvm22_1=1}`、`"generic"`、空text、`None=1`、`Pic=1`、`Default=1`、`RelocatableObject=1`、`Llvm22SelectionDagStandard=1`、`1`、`3`、`WritableIndirectSpOrFpEightBytes=1`、`Required=1`、`Forbidden=1`、`Forbidden=1`、`All=1`、`Disabled=1`、`Disabled=1`、`Aarch64=1`、`ItaniumDwarf=1`、`ScoopLsdaSubsetV1=1`、`2`、`{0xff,0x9b,0x01}`、`DarwinLibSystem=1`、`MachO=1`。LLVM patch version不进contract，reader只接受major/minor恰为22.1；未列出的pass、透传LLVM option、target feature或替代emission/EH mode都是禁止状态，不是隐含默认。contract CBOR与fingerprint golden固定为：

```text
b81901a20116020102a30100020a0301036767656e657269630460050106010701080109010a010b030c010d010e010f01100111011201130114011501160217a30118ff02189b0301181801181901
BackendProfileFingerprint = 03ab3ae611e31f2ac7dde6486deea185c981f5313b939f5cf93641b7e5e9aff7
```

上述Target profile只标识LIR可观察的layout/Scoop ABI与“C边界必须经generated bridge”策略，Backend profile只标识Scoop LIR→LLVM/Mach-O的受检codegen契约。SDK/deployment target、实际system C compiler identity、canonical generated-C flags/source template、runtime source-set/build flags、default native libraries和最终linker actions属于后续独立的C-bridge/runtime/link toolchain capability，并分别进入Code、RuntimeImage或ResolvedLinkPlan fingerprint；它们不得暗中写进或改义这两个`/1` profile。M23-3若尚未注册并验证所需toolchain capability，只能拒绝对应generated bridge/runtime/link production路径，不能把Target/Backend fingerprint冒充完整可执行toolchain proof。

请求级完整选择使用另一种不可部分构造的product，而不是把上述任一leaf继续膨胀：

```text
ResolvedTargetProfileV1 {
    lir_target: ValidatedLirTargetProfileV1,          // field 1
    backend: ValidatedBackendProfileV1,               // field 2
    c_bridge_toolchain: ValidatedCBridgeToolchainProfileV1, // field 3
    runtime_build: ValidatedRuntimeBuildProfileV1,    // field 4
    final_link: ValidatedTargetLinkProfileV1,          // field 5
}
```

registry原子解析五个projection并证明canonical triple、object format、deployment、calling-convention、pointer/storage ABI、compiler output与link input互相相容；不存在“先取host默认值、后来再补”的构造器。M23-2只构造并持久化前两项组成的`ValidatedLirTargetSelectionV1`，不伪造完整`ResolvedTargetProfileV1`；后三项由后续required capability分别冻结，并进入Code、RuntimeArtifact/RuntimeImage和ResolvedLinkPlan fingerprint。`lir-lower`只接收`lir_target`，Scoop codegen接收`lir_target + backend`，generated-C producer接收`lir_target + c_bridge_toolchain`，runtime-build接收`lir_target + c_bridge_toolchain + runtime_build`，program-link接收`lir_target + final_link`以及已经验证的其他stage产物。任一stage都不能从host、另一projection或已生成object反推自己缺少的项。

`ConeRecordV1`字段为`1=coordinate, 2=identity, 3=kind, 4=source_form`。kind tag为`Library=1, Executable=2`，source form为`Manifest=1, SingleFile=2`。reader重算Cone identity；`SingleFile`必须使用reserved coordinate，其他artifact不得使用它。M23-2 foundation不因kind强制main或可发布性，这两个证明由M23-3增加。

`DependencyRecordV1`字段为`1=coordinate, 2=identity, 3=hir_fingerprint, 4=mir_fingerprint, 5=lir_fingerprint`，按dependency Cone raw id严格递增，坐标与id必须重算一致。M23-2只验证record自身与无重复，不查找artifact或宣称已验证DAG closure。

`SemanticFingerprintRecordV1`字段为`1=hir, 2=mir, 3=lir, 4=code, 5=runtime_image`。前三项是非可选typed 32-byte layer digest，后两项是`Unavailable=1`/`Available=2 {1=typed digest}`的显式sum。foundation artifact的`code`与`runtime_image`使用`Unavailable`，不填全零digest。M23-3 production writer使用`Available`并提供对应required section。Graph/Compile可携带但不把`Unavailable`提升为Link proof。

### 8.2 manifest extension section

```text
ManifestSectionV1 {
    capability: CapabilityId, // field 1
    required_for: MemberPurposeSet, // field 2
    payload: byte string,     // field 3
}
```

section按第8.3节`CapabilitySortKey`严格递增，同capability最多一条。v1 envelope只容纳`required_for=0`、`Compile(0x2)`、`Link(0x4)`或`Compile|Link(0x6)`，known capability还必须精确等于registry contract；Graph必需内容必须直接在bootstrap中，Diagnostics是正交decorator且不能要求manifest semantic section。known handler要求payload是它的canonical inner schema并生成registry指定的fingerprint contribution；unknown section的payload一律是opaque bytes，reader只检查外层canonical/size并保持，不声称其内层canonical。是否阻塞proof只由`required_for ∩ EffectivePurpose(request)`决定：`EffectivePurpose(Graph)={Graph}`、`EffectivePurpose(Compile)={Graph,Compile}`、`EffectivePurpose(Link)={Graph,Link}`；交集非空且handler unknown时该proof失败，交集为空则保持opaque。因此unknown Compile-required阻塞Compile，unknown Link-required不妨碍Graph/Compile但阻塞Link，unknown Compile|Link同时阻塞Compile与Link。

M23-2不在bootstrap中为未实现的public index、ODR、image等字段留nullable占位；后续阶段以已冻结capability加required section。这些section不改manifest schema 1，但一旦声明required，对应purpose的旧reader必须fail closed。

### 8.3 member directory

`SlibMemberRecordV1`、`MemberStableKey`、`SlibMemberRole`、`CapabilityId`与`MemberPurposeSet`的wire完全采用M23总设计4.1表格：record field是`1=id, 2=stable_key, 3=role, 4=byte_length, 5=sha256`；stable-key/role tag都为`HirMetadata=1, MirMetadata=2, LirMetadata=3, LinkObject=4, DiagnosticAttachment=5, ExtensionBlob=6`；purpose bit为`Graph=0x1, Compile=0x2, Link=0x4, Diagnostics=0x8`。

metadata stable key无payload，必须与同tag role配对，且三种metadata各恰好一个。其他stable key的capability必须与role中capability逐byte相等；`logical_key`是1…4,096 byte的capability-owned canonical bytes。`byte_length`是payload原始byte数的checked u64，`sha256 = SHA-256(raw payload bytes)`。manifest的`members` array与所有后续处理统一按raw `SlibMemberId` bytes严格递增并拒绝重复；reader不得先sort再接受。manifest不在directory中，也不包含自身member record/hash。`SlibMemberId`、`MemberFingerprint`和`LinkMemberFingerprint`按第3.3节重算。

`CapabilityId={1=namespace,2=name,3=major_version}`的grammar与总设计相同：单个小写label精确匹配`[a-z][a-z0-9-]{0,62}`；namespace是1…255 ASCII byte、由一个或多个该label以`.`分隔，name是1…63 byte的单个label，major是nonzero u32。全协议唯一排序键为`CapabilitySortKey = (namespace raw ASCII bytes, name raw ASCII bytes, major_version numeric)`；manifest section、metadata section、profile inventory、fingerprint contribution与诊断都使用它，不能改用CBOR编码bytes或display `namespace/name/version`排序。

`TargetProfileWireId`、`BackendProfileWireId`、`ObjectFormatId`与`ArtifactCapabilityProfileId`是私有constructor产生、互不可转换的`CapabilityId` refinement；它们在wire复用三字段形状，但API不能把任意section capability cast成target/backend/profile。相应fingerprint也各有typed newtype。

M23-2 registry识别：

- `org.scoop-lang.target-profile/darwin-aarch64/1`；
- `org.scoop-lang.backend-profile/llvm-22-1/1`；
- `org.scoop-lang.object-format/mach-o-relocatable/1`；
- artifact profile `org.scoop-lang.slib-profile/identity-foundation/1`；
- metadata foundation capabilities `org.scoop-lang.hir/identity-foundation/1`、`org.scoop-lang.mir/identity-foundation/1`、`org.scoop-lang.lir/identity-foundation/1`。

上述两个contract DTO、singleton值、canonical bytes与digest是registry定义本身，不由host `llvm-config`、inkwell runtime枚举或TargetMachine默认值反推。`org.scoop-lang.link-object/scoop-lir/1`与`.../generated-c-bridge/1`的id/logical-key schema也按总设计冻结，但M23-2不声称识别其payload、不运行object verifier；Graph/Compile只保存hash-valid opaque member。

consumer内建的`CapabilityContractRegistryV1`为每个known section capability冻结：唯一允许的`SectionLocationV1 { Manifest=1, Hir=2, Mir=3, Lir=4 }`、唯一合法`required_for`、payload decoder/validator、canonical semantic projection及`FingerprintSinkSetV1`。sink tag为`Hir=1, Mir=2, Lir=3, Code=4, RuntimeImage=5, EnvelopeOnly=6, LinkValidationOnly=7`；同一section可向多个sink产生不同typed projection。producer声明的location或`required_for`必须与registry逐项相等，不能自行选择较弱purpose。unknown section没有semantic handler，sink视为EnvelopeOnly；它可以存在于envelope，但不能取得与其`required_for`相交的proof。

purpose到sink的兼容矩阵精确冻结为：`required_for=0`只允许`EnvelopeOnly`；`Compile`只允许`Hir|Mir|Lir`且至少一个；`Link`只允许`Code|RuntimeImage|LinkValidationOnly`且至少一个；`Compile|Link`必须至少有一个`Hir|Mir|Lir`和至少一个`Code|RuntimeImage|LinkValidationOnly`。`EnvelopeOnly`不能与其他sink并列。location再收窄：Hir只能产生Hir，Mir只能产生Mir；Lir的Compile面只能产生Lir、Link面只能产生Code/RuntimeImage/LinkValidationOnly；Manifest可按registry选择任一相容sink。由此Link-only section永不进入唯一的HIR/MIR/LIR semantic fingerprint，旧Compile reader无需理解它也能重算相同layer digest。

`LinkValidationOnly`只允许payload是其他已经进入LIR/Code/RuntimeImage fingerprint的canonical records之**完全可重算派生闭包**。handler必须从这些上游record独立重算后逐byte比较，且不产生新的semantic contribution；section bytes仍受member/artifact hash保护。若payload含任何不能从已指纹化输入重算的新语义，就必须改用Code或RuntimeImage sink，并在对应fingerprint的规范输入中显式加入projection，不能借tag 7绕过hash。

当前三条foundation contract分别是`Hir/Compile/{Hir}`、`Mir/Compile/{Mir}`、`Lir/Compile/{Lir}`。每个handler恰产生一条contribution，projection逐byte等于通过其closed decoder验证后的原始canonical inner payload；不得重新serde成另一种等价bytes。M23-3的object/image/runtime section必须以新known capability给出Link对应的Code/RuntimeImage sink；`org.scoop-lang.lir/link-identity-closure/1`只列出从LIR verification surface与directory重算的派生闭包，使用LinkValidationOnly，不二次改变Compile-facing LIR fingerprint，也不声称有一条本文未定义的Code contribution聚合公式。不能把M23-2 LIR foundation偷偷升级成Link输入。`ManifestSectionV1`与`MetadataSectionV1`共用这张registry，故未来Compile/Link-required manifest section也有明确fingerprint落点。

artifact profile不是producer自报的第二份section清单。其registry descriptor固定为：

```text
ArtifactCapabilityProfileDescriptorV1 {
    id: ArtifactCapabilityProfileId,             // field 1
    required_manifest: array<CapabilityId>,       // field 2
    required_hir: array<CapabilityId>,            // field 3
    required_mir: array<CapabilityId>,            // field 4
    required_lir: array<CapabilityId>,             // field 5
    code_requirement: FingerprintAvailabilityRequirementV1, // field 6
    runtime_requirement: FingerprintAvailabilityRequirementV1, // field 7
    publication_class: PublicationClassV1,         // field 8
    validation_policy: ArtifactValidationPolicyV1, // field 9
}

ArtifactCapabilityProfileFingerprint =
    DomainSeparatedCborHash("scoop-artifact-capability-profile-v1", descriptor)
```

所有capability array按`CapabilitySortKey`排序去重；它们精确列出与该profile所请求purpose相交的mandatory required set，而不是实际envelope inventory的全集。与requested purpose相交的每条required section必须出现在对应descriptor array，array中的每项也必须实际存在；与requested purpose不相交的known/unknown section只验证envelope/hash并保持opaque。实际envelope还可以包含registry允许的EnvelopeOnly optional section。availability为`MustBeUnavailable=1`或`MustBeAvailable=2`，publication为`FoundationOnly=1`或`Publishable=2`。

`ArtifactValidationPolicyV1`是closed product `1=OdrValidationPolicyV1, 2=ExtraSectionPolicyV1, 3=SlibDecodeCostModelV1, 4=LinkProofPolicyV1`。ODR policy为`IdentityOnlyNonPublishable=1, RejectAll=2, RequireCompleteDefinitionProof=3`；extra section policy的v1唯一值为`AllowPurposeDisjointOpaqueAndEnvelopeOptional=1`；decode cost model的v1唯一值为`DeterministicLogicalCostV1=1`；Link policy为`Forbidden=1, Required=2`。这些predicate直接进入profile fingerprint，不是profile id背后的未哈希代码约定；改变门禁、cost model或optional inventory规则必须换profile major/descriptor并得到新fingerprint。

`identity-foundation/1`的mandatory manifest清单为空，HIR/MIR/LIR数组分别只含对应foundation，code/runtime均Unavailable、publication为FoundationOnly，validation policy精确为`{IdentityOnlyNonPublishable, AllowPurposeDisjointOpaqueAndEnvelopeOptional, DeterministicLogicalCostV1, Forbidden}`。reader先由实际envelope反证mandatory subset及所有extra section合法性，再返回`ValidatedCompileArtifact<IdentityFoundationProfile>`；缺一条mandatory section即使producer未在`required_for`声明也失败。M23-3必须注册新的strong-only production profile，要求其完整Compile/Link mandatory set、Available fingerprints、Publishable以及`RejectAll/Required` policy；该profile发现任意ODR group/member/body/symbol立即失败。M23-7再注册使用`RequireCompleteDefinitionProof/Required`的profile版本。任何阶段都不能把foundation proof cast成publishable artifact。

按第3.2节pure enum规则，该foundation descriptor的Wire CBOR v1与profile fingerprint golden固定为：

```text
a901a301781b6f72672e73636f6f702d6c616e672e736c69622d70726f66696c6502736964656e746974792d666f756e646174696f6e030102800381a301726f72672e73636f6f702d6c616e672e68697202736964656e746974792d666f756e646174696f6e03010481a301726f72672e73636f6f702d6c616e672e6d697202736964656e746974792d666f756e646174696f6e03010581a301726f72672e73636f6f702d6c616e672e6c697202736964656e746974792d666f756e646174696f6e030106010701080109a40101020103010401
ArtifactCapabilityProfileFingerprint = 6461601c81a77ecbd34698c708d9035732f0a75f6e5ec98d2c561250ab0111f9
```

v1 `ExtensionBlob.required_for`只允许0或恰好`Link(0x4)`；Graph/Compile/Diagnostics bit、多bit或unknown bit都是目录错误。`DiagnosticAttachment`对语义purpose总是optional。`LinkObject`隐含Link required，但不得因Graph/Compile reader不认识verifier而失败；它只能在请求Link proof时fail closed。

### 8.4 artifact fingerprint

`artifact_fingerprint`字段不参与自身hash：

```text
ArtifactFingerprint = SHA-256(
    ByteSpan("scoop-artifact-v1")
    || ByteSpan(WireCborV1(ArtifactManifestInputV1))
    || Concat(i in [0, n),
              ByteSpan(raw(MemberFingerprint[i])))
)
```

`Concat(i in [0, n), f(i))`表示按递增index串接恰好`n`项；`n=0`时是空bytes，index/count运算均checked。`ArtifactManifestInputV1`是与`BootstrapManifestV1` field 1…10完全相同、但结构上不存在field 11的独立closed product；不是对缺required field的manifest DTO调用普通encoder。member fingerprint按directory的`SlibMemberId`顺序，每个固定32-byte值仍按统一framing加长度前缀。writer先构造input并计算，再增加field 11编码完整manifest；reader投影为input而不是将field 11归零。archive header、物理member name与padding不进fingerprint，但仍必须通过canonical container检查。

## 9. HIR/MIR/LIR metadata foundation

### 9.1 共同outer envelope

三层metadata member payload共用如下outer envelope，但magic、section capability和inner decoder不可互换：

```text
MetadataEnvelopeV1 {
    magic: bytes,                         // field 1
    outer_schema: NonZeroU32,             // field 2
    sections: array<MetadataSectionV1>,   // field 3
}

MetadataSectionV1 {
    capability: CapabilityId,             // field 1
    required_for: MemberPurposeSet,        // field 2
    payload: bytes,                        // field 3
}
```

magic分别是ASCII `SCOOPHIR`、`SCOOPMIR`、`SCOOPLIR`，`outer_schema=1`。section按`CapabilitySortKey`严格递增并拒绝重复；known section还必须满足第8.3节registry给出的location、purpose和sink。HIR/MIR section只允许optional或Compile required；LIR section允许optional、Compile、Link或Compile|Link。Graph bit与Diagnostics bit不得出现在metadata inner section：Graph不解码IR，diagnostic attachment有独立member role。

outer envelope的member hash保护全部inner bytes，inner section不再带重复hash field。unknown optional section仅在完成outer canonical decode、payload length与member hash检查后跳过；unknown Compile-required section在分配IR arena之前失败。

### 9.2 foundation payload schema

M23-2的三个identity foundation section都严格是Compile required且每层恰好一个；它们不是未来Link proof的隐式输入。M23-3另加self-contained、Link-required的`org.scoop-lang.lir/link-identity-closure/1`，使Link分支无需解码HIR/MIR。payload是Wire CBOR v1 product，每个identity table都按第5.3节的stable dependency-first规则排序；无table-internal edge的简单映射按其主typed id raw bytes严格递增。

同一种identity按“首次产生它的stage”只进入一个delta table。Compile validator先有界解码三层DTO，再按`(kind,id)`建立全artifact union graph并验证全部跨kind owner/edge；只有同kind table内部要求dependency-first，不靠不同table的field编号伪装拓扑。HIR key只能引用HIR本层或更早authority，MIR可引用HIR/MIR，LIR可引用三层，任何上层引用未来stage都拒绝。跨层重复`(kind,id)`即使key相同也拒绝，lowerer必须复用既有id而不是重发record；同id不同key仍是identity collision。这样每层IR自包含自己首次创建的persistent origin，又允许同层不同kind的合法互引由全图DAG/owner validation一次证明。

`HirIdentityFoundationV1`（capability `org.scoop-lang.hir/identity-foundation/1`）：

| field | table |
| ---: | --- |
| 1 | `SourceRecordV1` |
| 2 | HIR首次创建的source non-generic `PersistentTypeId` records |
| 3 | `PersistentGenericTypeId` records |
| 4 | `PersistentFunctionId` records |
| 5 | `PersistentGenericFunctionId` records |
| 6 | `PersistentConstructorId` records |
| 7 | `PersistentPropertyId` records |
| 8 | `PersistentExtensionPropertyId` records |
| 9 | `PersistentObjectValueId` records |
| 10 | `PersistentTypeAliasId` records |
| 11 | `PersistentPropertyAccessorId` records |
| 12 | HIR首次创建的`PersistentFieldId` records |
| 13 | HIR首次创建的`PersistentEnumVariantId` records |
| 14 | HIR首次创建的`PersistentEnumVariantFieldId` records |
| 15 | HIR首次创建的`PersistentExactTypeId` records |
| 16 | 当前Cone direct public `PersistentExportBindingId` records；M23-5在同一identity表增加resolved re-export binding，并由新surface section承载route/provenance |
| 17 | HIR首次创建的`PersistentCallableApplicationId` records |
| 18 | HIR首次创建的`PersistentGeneratedCallableId` records |
| 19 | HIR首次创建的generated `PersistentTypeId` records |
| 20 | `PersistentDispatchSlotId` records |
| 21 | `PersistentInitializationUnitId` records |
| 22 | `PersistentSourceContextId` records |
| 23 | `PersistentLocalBindingId` records |
| 24 | HIR首次创建的`PersistentLocalValueId` records |
| 25 | `PersistentCallbackRegistrationId` records |
| 26 | `SourceNativeExternalContractRecord` records |
| 27 | HIR首次创建的`OdrGroupId` records |
| 28 | HIR首次创建的`OdrMemberId` records |
| 29 | `DefinitionOriginRecord` records |
| 30 | `NativeBoundaryTypeDefinitionRecordV1` records |

`MirIdentityFoundationV1`（capability `org.scoop-lang.mir/identity-foundation/1`）：

| field | table |
| ---: | --- |
| 1 | MIR首次创建的`PersistentExactTypeId` records |
| 2 | MIR首次创建的`PersistentGeneratedCallableId` records |
| 3 | MIR首次创建的generated `PersistentTypeId` records |
| 4 | MIR generated class/struct的`PersistentFieldId` records |
| 5 | MIR generated enum的`PersistentEnumVariantId` records |
| 6 | MIR generated enum的`PersistentEnumVariantFieldId` records |
| 7 | `MirCallableSignatureRecordV1` records |
| 8 | MIR首次创建的`PersistentLocalValueId` records |
| 9 | `PersistentCallbackApplicationId` records |
| 10 | `MirCallbackApplicationRecordV1` records |
| 11 | MIR首次创建的`OdrGroupId` records |
| 12 | MIR首次创建的`OdrMemberId` records |

每个HIR中已形成的`PersistentCallableApplicationId`同时产生对应Callable group及以CallableApplication为discriminator的primary callable member；属于该application且在HIR已知的lexical generated callable也可产生以GeneratedCallable为discriminator的member。MIR transform新增的generated callable/member只写MIR delta并回指既有group。由此MIR signature subject绝不前向引用LIR；LIR只为layout/scan/site/registration等首次在LIR出现的member补delta。

`MirCallableSignatureRecordV1`是map `1=CallableSignatureSubjectV1, 2=ExactCallableSignature`。subject为`Strong=1 {1=CallableOwner}`或`Odr=2 {1=CallableOdrMemberId}`；Strong只接受按第7.2节递归为唯一`ConeOwned`的source/source-anchored generated实现，origin-free shape helper即使param-free也必须使用其唯一ODR member。concrete generic/local-lambda实现同样使用Callable ODR member，所以同一模板级`PersistentGeneratedCallableId`可以在不同application中具有不同exact signature而不冲突。record按`(subject tag, subject raw id)`严格排序且一个subject恰有一条。record只证明persistent implementation subject与完整Scoop signature传播，不包含body implementation、dispatch implementation、generic predicate或external link symbol；这些内容由后续required section承载。

`MirCallbackApplicationRecordV1`是map `1=PersistentCallbackApplicationId, 2=CallableSignatureSubjectV1 managed_adapter, 3=ExactCallableSignature managed_signature, 4=ForeignCallbackStorageAbiV1, 5=CallbackMode`。名字刻意使用Application而不是Registration：HIR registration可含binder并标识source conversion site，这张MIR record只描述一次fully concrete materialization。`ForeignCallbackStorageAbiV1`在v1唯一为`ClosureResultRootsThrowableToU32=1`，精确表示现有adapter的`{closure, result storage pointer, root storage pointer, throwable storage pointer} -> u32 status`机器边界；它不是C callback的target-specific signature。record按callback application id排序，逐项重算其registration/context、验证替换后的source/managed exact signature以及adapter subject；target-specific canonical C storage/bridge正规化尚未参与MIR。

`LirIdentityFoundationV1`（capability `org.scoop-lang.lir/identity-foundation/1`）：

| field | table |
| ---: | --- |
| 1 | LIR首次创建的`PersistentExactTypeId` records |
| 2 | `PersistentLayoutId` records |
| 3 | `PersistentScanId` records |
| 4 | `PersistentDispatchTableId` records |
| 5 | `PersistentStaticStorageId` records |
| 6 | `PersistentImmortalObjectId` records |
| 7 | LIR首次创建的`OdrGroupId` records |
| 8 | LIR首次创建的`OdrMemberId` records |
| 9 | `PersistentCallableBodyId` records |
| 10 | `PersistentSafepointSiteId` records |
| 11 | `RuntimeTypeMappingRecordV1` records |
| 12 | `SafepointMappingRecordV1` records |
| 13 | `PersistentSymbolRequestV1` records |
| 14 | `NativeExternalContractRecord` records |
| 15 | `CanonicalCAbiSignatureFingerprintRecord` records |
| 16 | `CanonicalCAbiLayoutFingerprintRecord` records |
| 17 | `GeneratedBridgeUnitId` records |
| 18 | `GeneratedBridgeAtomId` records |
| 19 | `LirCallbackBridgeRecordV1` records |
| 20 | `NativeLinkRequirementId` records |
| 21 | `ObjectDefinitionPlanId` records |
| 22 | `ObjectDefinitionAtomId` records |

`RuntimeTypeMappingRecordV1`恰为`1=PersistentExactTypeId, 2=RuntimeTypeId(nonzero u64)`；`SafepointMappingRecordV1`恰为`1=PersistentSafepointSiteId, 2=SafepointId(nonzero u64)`，分别按field 1 raw id排序并全双射。symbol request按第7.1节唯一规则`(numeric key tag, owner raw id)`排序；linkage不进sort key，同key的不同linkage在排序前就是冲突。

field 15/16的两个fingerprint record都是`{1=typed fingerprint, 2=canonical preimage}`，preimage分别为第5.4节的`CanonicalCAbiFunctionSignature`与`CanonicalCAbiLayout`；reader以`scoop-c-abi-signature-v1`、`scoop-c-abi-layout-v1` domain重算。field 14保存完整`NativeExternalContractRecord`并按该节同时重算symbol id与contract fingerprint。不能把bare digest当可信leaf；bridge unit/atom key引用的每个contract/signature/layout必须能在这些已验证表中定位。

`LirCallbackBridgeRecordV1`是map `1=PersistentCallbackApplicationId, 2=CanonicalCAbiSignatureFingerprint, 3=GeneratedBridgeUnitId`，按callback application id排序。field 2的preimage必须是由该application替换后的source C signature经当前target profile唯一正规化得到的canonical C signature；field 3必须逐字段等于`CallbackTrampoline { field 2, context_parameter }`的unit key。MIR application、LIR signature与unit三方缺失或mode/context不一致使Compile proof失败；不同application得到相同signature/index时必须引用同一个unit，不得复制trampoline identity。

foundation中的layout/scan/dispatch table只有identity key，没有可被consumer当作ABI证明的payload。M23-6添加完整layout/ABI/scan required section后，Compile view才能暴露对应typed API。同理，foundation不包含public lookup、template body、dispatch implementation、ODR definition fingerprint/member闭包、object range或runtime image record；它只含已经冻结且当前generic body所需的ODR identity records。

各层payload不保存arena id、Rust discriminant或物理member index。如果一条LIR identity依赖HIR/MIR identity，wire直接存typed 32-byte id，不借用同数值table index表示跨kind关系。table index只允许作为同kind内部压缩，且DTO的index newtype不能逃出wire module。

### 9.3 IR crate API

每个IR crate公开三类、且只公开三类边界：

```text
Canonical*Foundation               // lowerer构造的arena-independent projection
Validated*FoundationWire           // decode + structural validation后的临时graph
Imported*Set + Imported*Id family  // commit后的session-local world
```

encoder只接收`Canonical*Foundation`，不接收`Module`；decoder不直接写semantic world。`ImportedHirSet`、`ImportedMirSet`、`ImportedLirSet`各有独立arena/id family，它们不与current entity、LocalConcrete entity或相互的id互转。M23-2的imported set只提供identity/origin查询，API命名不使用`ExportHir`、`LinkableLir`或其他强于它已证明内容的词。

## 10. deterministic normal `ar`

### 10.1 writer的唯一输出

writer接收已按typed stable key构造的member，先计算id/record/fingerprint，然后按`SlibMemberId` raw bytes排序。物理顺序为：

1. `manifest.cbor`；
2. directory ordinal 0对应`m00000000`；
3. 后续ordinal以八位零填充十进制递增，最大`m00065535`。

v1唯一接受的SysV/GNU short-name bytes为：

- global magic恰好ASCII `!<arch>\n`；
- 60-byte header的六个文本field全部左对齐，内容后方只用ASCII space `0x20`填至固定宽度；name field内容为逻辑名加`/`，宽16 bytes；
- mtime/uid/gid的内容分别是十进制单字节`0`，宽12/6/6 bytes；
- mode内容为八进制ASCII `100644`，宽8 bytes；
- size内容为payload长度的canonical十进制，宽10 bytes；零恰写单字节`0`，非零禁止`+`、前导space与前导零；
- trailer恰好`` `\n``；odd payload追加一个不计入size的`0x0A`。

writer不生成symbol table、long-name table或special member，不写timestamp、host path、basename或原始扩展名。实现可以复用成熟archive库的底层record能力，但必须能逐field指定并验证上述bytes；如果库自动产生不同等价header，则不能用于v1 writer。

### 10.2 reader的container proof

reader不把`.slib`交给system linker或通用archive extraction。archive总长从第一个byte起包含8-byte global magic、每个60-byte header、payload及odd pad。它先有界解析第一个header，要求名为`manifest.cbor`，再根据manifest directory精确预测剩余成员数、名称、顺序与长度，并在读取其余payload前以checked u64验证`8 + Σ(60 + payload_len + (payload_len & 1)) == input_len`；求和包含manifest和每个directory member。任一raw header field不是第10.1节的唯一形式都返回`NonCanonicalArchive`，即使通用`ar`工具会视为等价。

拒绝thin archive、`/`、`//`、`__.SYMDEF*`、BSD extended name、path separator、重复/未声明/缺失member、额外trailing bytes和错误odd padding。物理ordinal/name不是semantic member identity，API只在诊断中以`ArchiveMemberOrdinal`暴露它。

## 11. reader proof、typed remap与失败原子性

### 11.1 proof链

```text
SlibInputBytes
  -> DecodedSlibEnvelope
  -> ValidatedGraphArtifact
       |-> ValidatedCompileArtifact<P>   // M23-2实现foundation profile
       `-> ValidatedLinkArtifact         // M23-3首次实现
```

`DecodedSlibEnvelope`证明canonical ar、canonical bootstrap CBOR、directory/id/order、每个member的length/hash、artifact fingerprint和基础资源上限。它保留immutable backing bytes及checked member ranges，不提供IR、object或语义查询。

`ValidatedGraphArtifact`在envelope上追加magic/schema/ABI/profile exact match、Cone coordinate/id、kind/source form、direct dependency record与member role/purpose/capability envelope验证。它只是一个artifact graph node proof，不说dependency artifact存在、唯一、无环或已形成closure。

`ValidatedCompileArtifact<P: CompileCapabilityProfile>`再按profile解码三个metadata outer envelope与所有Compile-required section，完成单层结构、identity重算、第4.5节精确矩阵规定的HIR definition-source覆盖、跨层typed identity bridge、derived u64、symbol request和semantic fingerprint检查，最后原子提交该profile允许的`ImportedHirSet/ImportedMirSet/ImportedLirSet`。M23-2唯一可构造的`P`是`IdentityFoundationProfile`。expression evaluation/concrete origin要等承载body的后续required section才进入相应Compile proof，本foundation proof不声称验证不存在的body。该proof只把LinkObject/Link-required blob作为hash-valid opaque envelope保存，没有`VerifiedLinkObject`、object bytes extraction、linker argv或native requirement API；不同profile之间没有unchecked cast。

M23-2不定义public `validate_link`，也不定义伪`ValidatedLinkArtifact`。M23-3必须从同一`ValidatedGraphArtifact`分叉添加object/image verifier，并共享immutable envelope/backing member ranges；Link不依赖Compile或HIR/MIR decode，也不从raw envelope开第二条捷径。

### 11.2 decode与remap算法

Compile validation严格分为四步：

1. **wire decode**：在budgeted cursor上解码为不持有semantic arena的DTO，保留field/index path；
2. **structural validation**：检查canonical order、id/key重算、owner/edge、arity、non-empty、cycle、span、kind和跨层覆盖，得到不可再变的validated wire graph；
3. **typed remap**：对每个kind按raw persistent id顺序为`PendingSemanticWorld`分酌fresh imported id，然后解析全部edge；
4. **atomic commit**：三层及所有cross-layer bridge全部成功后，一次性将pending world并入session，返回Compile proof。

任一错误只丢弃pending world，不会留下已注册一半的entity、string intern、derived-id slot或dependency edge。不使用`Drop`的“尽力回滚”作为正确性基础。

session有两层interner。artifact-origin registry以`ConeIdentity`为key，要求同Cone的三层semantic fingerprint一致；这里发现同origin不同fingerprint时在entity commit前失败。entity interner以`(kind, persistent id)`为key并始终要求canonical key一致；只对key中带origin Cone的declaration/Strong entity校验其origin registry，不把某个producer的整层fingerprint附加到origin-free结构identity上。

因此两个Cone产生相同tuple/function exact type、layout key或bridge unit时，只要canonical key相同就复用同一structural world id；它们各自的materialization/definition内容保留per-origin record，由后续ODR/object proof比较。diamond路径再次读取同declaration origin/key/fingerprint时复用world id；同id但key不同仍是确定性冲突。不同Cone的同FQN或display name不会intern成同一declaration。

re-encode只读取persistent id/key并按canonical sort重建DTO，不读imported arena number。因此`encode -> decode -> remap -> re-encode`必须逐byte相同，即使consumer事先预占了不同数量的arena entry。

### 11.3 跨层完备性

Compile proof至少检查：

- MIR引用的source/generic/generated callable origin在HIR/MIR foundation中恰有一个；
- foundation中每个`DefinitionOriginRecord`的source、context与point table一致；后续body capability出现时，再对其内联`ExpressionOrigin`逐项验证definition/evaluation。local binding/value selector的owner存在，capture/frame field引用的local value来自HIR/MIR union且kind正确；
- LIR callable body的Strong owner在HIR/MIR中存在且是callable kind；
- 每个Callable ODR group/member满足第5.2节root provenance；M23-2 foundation允许验证这些identity，但profile/publication门禁仍禁止把它提升成Link input；
- 每个safepoint site的owner body存在，同owner/role ordinal从0连续且不重复；
- 每个runtime/safepoint u64表与对应full id全双射，不存在没有full-key的裸u64；
- 每个PersistentV1 symbol request的owner存在、kind tag正确且linkage合法；
- source native contract、MIR callback storage record、LIR target contract/C signature/bridge unit分别具有唯一上游relation；任何层都不能按symbol或signature display text重新猜关系；
- HIR/MIR/LIR各自声明的exact type union对同一id/key一致，child/nominal origin都能定位；
- metadata role声明的wire schema、outer schema与manifest compatibility三者相等。

本阶段不用“后续会补”跳过foundation中声明的relation；另一方面，本阶段没有声明的public lookup/layout/object/image relation也不伪造空record来让validator通过。

## 12. resource budget与错误模型

### 12.1 `SlibDecodeLimitsV1`

M23-2固定的单artifact默认上限全部是**inclusive**（`observed <= limit`）：

| resource | limit |
| --- | ---: |
| archive总长（含global magic、全部header与pad） | 2,147,483,648 bytes |
| `manifest.cbor` | 67,108,864 bytes |
| 非manifest member数 | 65,536 |
| 单个metadata section的inner payload | 268,435,456 bytes |
| 单LinkObject/ExtensionBlob payload | 1,073,741,824 bytes |
| 单DiagnosticAttachment payload | 536,870,912 bytes |
| CBOR nesting | 128 |
| 任一semantic table entry数 | 16,777,216 |
| 单text/bytes语义leaf | 16,777,216 bytes |
| type/body/owner/path递归深度 | 1,024 |
| decoded logical heap累计 | 2,147,483,648 bytes |
| decoded node累计 | 16,777,216 |
| decoded edge累计 | 67,108,864 |
| owned/copied bytes累计 | 1,073,741,824 bytes |
| validation work units累计 | 268,435,456 |

`MetadataSectionV1.payload`、`ManifestSectionV1.payload`、`CanonicalSemanticContributionV1.projection`与member backing range是carrier bytes，不受16 MiB semantic-leaf上限重复限制；它们分别受section/member/manifest/archive上限。前三条foundation projection必须借用同一immutable inner-payload range或直接stream进hash，不能复制整份payload；其他handler也必须在registry声明borrowed/streaming projection策略。known handler进入inner schema后，其中每个实际语义text/bytes leaf仍受16 MiB限制。把大leaf再包一层bstr、分块成相邻字段或先复制到handler私有buffer都不能绕过累计预算。

CBOR nesting的top-level item计depth 1，每进入一个array/map/tagged-sum payload中的子item加1；scalar/text/bytes本身不再增加第二层。bootstrap manifest、每个metadata outer envelope分别以depth 1开始。section `payload`在outer中只是一个byte string leaf；known handler随后把其inner bytes作为一棵新的CBOR document、再次从depth 1开始，但使用同一个累计node/heap/work meter，不重置任何非depth预算。type/body/owner/path递归深度也以被验证的root为1，每沿一条对应semantic edge加1；两种depth limit彼此独立。

计数单位精确定义如下：

- node：每个record、sum occurrence与collection element各1；collection容器本身另计1。decoder读取count后先checked预扣全部slot/node预算，再遍历内容；
- edge：每个persistent/index/owner/graph relation occurrence每次规定的validator pass各1。identity全图建边、cross-layer bridge等各是独立明确pass，不能把同一输入反复扫描而不计费；Kahn pass使用下一项的整式计费，替代而不是叠加这一逐edge费用；
- SHA-256：对一次长度为`L`的完整hash stream预扣`floor((L + 72) / 64)`个work unit，即包含padding的block数；`L + 72`先checked。分段feed不能重置或少计；
- canonical sequence只做相邻严格递增/去重检查，长度`n`预扣`max(n-1, 0)`；不按实际比较器提前退出次数计费。untrusted table禁止“先sort再接受”；
- stable Kahn的ready set对`n`个node固定预扣`n * ceil_log2(max(n, 2)) + edge_count`，无论实际heap比较次数；这已经完整覆盖该pass的ready-set与逐edge消费；
- 每个CBOR item occurrence、archive header/member和session interner probe各扣1；每次capability dispatch固定扣32，不读取当前registry长度或实际lookup比较次数。known capability handler另加固定`base_work`，当前HIR/MIR/LIR foundation handler各为64，再按inner node/edge/hash-block使用本表通用公式；同一payload换handler不能获得新总预算。

验收/拒绝不得取决于标准库sort/hash table的实际comparison/probe次数、hash seed、allocator rounding、registry增长或并行调度。需要canonical排序的producer输入属于trusted in-memory数据；reader面对wire只验证已排序。`SlibDecodeCostModelV1::DeterministicLogicalCostV1`冻结logical heap cost：record/sum/collection node每项64 bytes，collection element slot 32 bytes，graph adjacency edge slot16 bytes，ready-set element40 bytes，pending remap entry96 bytes，owned text/byte/canonical-temporary按请求的logical byte数1:1另计；一个对象同时具有多个角色时逐项相加，不以Rust `size_of`或allocator结果替代。默认limit、work公式、这些cost与handler `base_work`在`scoop-slib::limits`/capability registry各有唯一V1定义并用golden锁定；它们是profile已哈希的consumer cost-model policy，不是producer可选字段。test只能整体注入更小的limits，production不为section、member或handler重置meter。

decoded logical heap按meter批准的**logical requested slots/bytes**记账，而不是按allocator返回的实际capacity记账：byte/string copy按请求byte数同时计入heap与owned/copied；typed Vec/table、graph adjacency、ready set、temporary canonical buffer、pending remap world和handler state按`requested logical slots × V1 slot cost`计入heap。输入backing及其zero-copy slice不计heap，但一旦复制即计费。replace/shrink不返还额度，避免处理顺序改变可用预算；所有输入规模驱动的分配先扣逻辑额度，再调用fallible `try_reserve_exact`或等价精确请求API，分配失败返回结构化`ResourceAllocation`且不提交world。allocator即使内部round up也不改变meter或验收结果；handler不能使用meter外的unbounded allocation。

所有长度/偏移先在u64做checked add/multiply/range，通过limit后才checked转`usize`。decoder先核对剩余bytes与累计budget，再执行fallible `try_reserve_exact`；不根据不受信任count直接`Vec::with_capacity`或调用可能abort的infallible reserve。identity graph、diagnostic printer与remap使用显式stack或在递归前扣减depth，不让输入触发进程stack overflow。

M23-4会在多artifact层叠加`SlibClosureDecodeLimits`。M23-2的type/API预留可共享meter，但不把“读完一个artifact”称为已满足closure budget。

### 12.2 结构化错误

public error是封闭大类加typed detail，不传出第三方库错误文本：

| category | 代表性detail |
| --- | --- |
| `Container` | bad magic/header/name/order/pad, thin/special/trailing member |
| `CanonicalWire` | non-minimal CBOR, wrong major type, duplicate/extra/missing field, unknown tag |
| `ResourceLimit` | `LimitExceeded { resource kind, configured limit, observed checked value }`或`ResourceAllocation { requested logical bytes/slots }`；后者只表示fallible allocation失败，不把它误报为输入超过已配置limit |
| `Fingerprint` | member/artifact/layer fingerprint mismatch |
| `Compatibility` | schema/ABI/mangler/identity/target/backend mismatch |
| `Directory` | duplicate/missing member, id/stable-key/role/purpose mismatch |
| `Capability` | malformed id, unknown capability required for requested purpose |
| `Identity` | recompute mismatch, wrong kind, missing owner, cycle, hash collision |
| `Reference` | out-of-range index, wrong typed table, arity/binder/span/path error |
| `Bridge` | HIR/MIR/LIR origin/coverage/derived-id/symbol mismatch |
| `SessionConflict` | same origin with different key or semantic fingerprint |

每个error携带`artifact Cone coordinate/id`、`manifest/member id`、`section capability`与结构化path（`Field(u32)`/`Index(u64)`/`Key(typed id)`）中可获取的部分。诊断renderer可加display locator，但排序先按Cone id、member id、section、path和error code；不受信任text只作escaped argument，不作format string。validator可以并行收集错误，但公开返回的primary error必须在所有已启动且预算允许完成的独立检查结果中按同一排序键取最小；若某个更早的顺序化前置门（container → bootstrap canonical wire → directory/range/hash → compatibility/profile → section structural → cross-reference → session conflict）失败，则不启动依赖其输出的后续门。预算扣费顺序也按此门序与canonical record顺序，不能用“哪个worker先返回”仲裁。

reader对任意bytes只能返回typed error，不允许panic、OOM-before-budget、无界递归、整数wrap或部分world commit。

## 13. semantic fingerprint与schema evolution

### 13.1 M23-2 foundation layer fingerprint

每个known section handler先产生零个或多个canonical contribution：

```text
CanonicalSemanticContributionV1 {
    location: SectionLocationV1,      // field 1
    capability: CapabilityId,         // field 2
    sink: FingerprintSinkV1,          // field 3
    projection: bytes,                // field 4，handler的canonical inner CBOR
}

SupportEdgeV1 {
    origin: ConeIdentity,             // field 1
    role: SupportEdgeRoleId,          // field 2
    fingerprint: CorrespondingLayerFingerprint, // field 3
}

LayerFingerprintInputV1 {
    context: LayerFingerprintContextV1,                 // field 1
    contributions: array<CanonicalSemanticContributionV1>, // field 2
    support_edges: array<SupportEdgeV1>,                 // field 3
}

LayerFingerprint = DomainSeparatedCborHash(layer_domain, input)
```

`LayerFingerprintContextV1`是封闭sum：`Hir=1 {1=CompositeIdentityAbiFingerprint}`；`Mir=2 {1=CompositeIdentityAbiFingerprint, 2=ManglingSchemaIdentity}`；`Lir=3 {1=CompositeIdentityAbiFingerprint, 2=TargetProfileWireId, 3=TargetProfileFingerprint, 4=BackendProfileWireId, 5=BackendProfileFingerprint}`。因此target/backend contract即使section payload碰巧未变也一定改变LIR fingerprint；同一profile id改变contract本身仍被第8.1节禁止。

layer domain分别是`scoop-hir-semantic-v1`、`scoop-mir-semantic-v1`、`scoop-lir-semantic-v1`。每层只取sink等于自己的contribution，按`(location tag, CapabilitySortKey, sink tag)`严格排序并拒绝重复。`SupportEdgeRoleId`是`CapabilityId` refinement；M23-2内建`org.scoop-lang.support-edge/all-direct/1`，并预注册M23后续使用的`hir-lookup-observation/1`、`hir-reexport/1`、`hir-default-template/1`、`hir-hidden-support/1`、`mir-callable-origin/1`、`mir-dispatch/1`、`lir-layout/1`、`lir-callable/1`、`lir-initialization/1`与`lir-native-contract/1`。role必须为当前layer允许的known contract；unknown role无法构造Compile proof。

support edge按`(origin raw bytes, role CapabilitySortKey)`严格排序并按完整pair去重；同一origin的不同role合法。M23-2对manifest中每个direct dependency、每个layer恰产生一条`all-direct/1` edge，其fingerprint精确取该`DependencyRecordV1`的对应层字段。M23-5可由`CrossConeUseSet`替换成更精确的role/edge集合，但不改record shape、公式或排序。

本层已识别且registry指向本sink的required section必须贡献projection；unknown required无法得到fingerprint proof因而对应view已失败。known/unknown optional section都只能是EnvelopeOnly并不进三层semantic fingerprint；DiagnosticAttachment、optional ExtensionBlob、LinkObject、member id/分片/archive ordinal同样不进。但它们仍进whole `ArtifactFingerprint`，因此不会被未检查地替换。

M23-2 foundation的code/runtime-image fingerprint是第8.1节的`Unavailable`；不从空object list哈希出看似有效的code proof。M23-3开始按总设计4.6计算`CodeFingerprint`，不修改本阶段的member/artifact/layer公式。

### 13.2 schema/capability规则

- closed record增删field、重用tag、改变hash domain/field顺序/编码或改变已知capability payload，必须提升其schema/major；
- 增加新独立semantic能力优先新建section capability，不给bootstrap或foundation closed product加nullable field；
- unknown optional只可保持/跳过，不影响任何semantic proof；unknown required按effective purpose fail closed；
- HIR/MIR/LIR outer schema在同一artifact必须同代，不允许`1/2/1`这类混合；
- M24保留container/manifest/identity/PersistentV1，三层outer schema整体升2且callable body整体换v2。v1与v2不在一个artifact/world中混用，旧artifact重建而不原地升级。

## 14. 当前pipeline迁移设计

### 14.1 source与HIR

- 用`SourceIdentity`替换正式IR中的`Stage1RequestId/Stage1SourceHandle`；request token最远只到parser buffer mapping。
- HIR source table从`display_name + full source`改为`SourceRecordV1`所需的identity/digest/length/line-starts/point table；当前编译可在driver保留source bytes供诊断，但不把它带入semantic key。
- file-private visibility/origin/import binding使用`SourceIdentity`，不再读request/local index。`stable_user_source_name`的cwd/canonical path结果只是display locator，不升级为persistent name。
- HIR lowering一次性产生完整kind-specific declaration key/id和foundation；每个name-resolution witness生成`PersistentLocalBindingId`，每个可能进入capture/frame的this、parameter、source local与bound receiver生成`PersistentLocalValueId`，后续不从FQN、local name、link stem或diagnostic text反推。
- AST仍保留enum variant payload field的named/positional style；AST→HIR时必须把`EnumVariantFieldSelector::{Named, Positional}`及其persistent field id显式写入HIR。现有将positional field打印成`_1`的display name只能保留为诊断装饰，禁止由`_N`字符串反向恢复selector。
- 每个source extern与foreign callback conversion分别生成第5.4节source contract及callback registration record；generic Export HIR只保存允许binder的source signature，不能提前构造exact/target ABI。进入LocalConcrete/MIR时为每个实际substitution生成typed callback application，再建立adapter/signature relation。

### 14.2 MIR、LIR与codegen

- 删除production `CallableLinkStem`/`NominalLinkStem`和`CompactV2`生成路径；MIR只保存typed persistent origin、Strong/ODR signature subject、MIR-first local value与callback storage ABI，不为调mangler构造伪`Module`。
- closure/callback/coroutine/adapter/constructor/string/global等分散`format!`全部改用第7章入口；arena id/state number不得作overload discriminator。
- 每个LIR function非可选地携带`PersistentCallableBodyId`；external runtime/native body使用其他typed target，不填伪body id。
- LIR在body/layout/scan/table/storage/site/bridge owner全部形成并完成Strong/ODR分类后，集中构造`PersistentSymbolRequestV1`与mangled symbol；MIR没有这张表，也不能预猜LIR才存在的owner。
- safepoint lowering先记site role，在final CFG后集中生成site/full/u64映射。runtime type id从exact type派生，删除String=1和遍历顺序自增规则。
- codegen只消费已验证的mangled symbol、runtime type id和safepoint id，不持有hash/mangler的fallback实现。legacy executable shim与Scoop body分层清晰。

### 14.3 crate dependency收紧

production stage依赖仍为相邻IR：

```text
parser    -> ast
hir-lower -> ast + hir
mir-lower -> hir + mir
lir-lower -> mir + lir
codegen   -> lir
```

lowerer通过输入/输出IR re-export的typed constructor使用identity，不直接依赖`scoop-wire`、`scoop-slib`或上游implementation crate。现有MIR只为`Span`依赖AST的跨层关系随source identity迁移改为稳定source-span数据类型，不让MIR lowerer依赖AST来补查source。

## 15. 诊断与可观测性

### 15.1 诊断排序与定位

诊断semantic location使用`SourceLocation { file: canonical semantic source name, span }`。CLI在最后一层可将`file`装饰为本次用户输入路径，但JSON/golden的stable key与排序仍用canonical name。同一artifact的错误顺序不因archive member输入顺序、hash map seed、线程调度或display path改变。

identity诊断显示`kind + 64-hex id`，并在key可信后显示Cone coordinate/package/owner/name。在key未验证前不把artifact自报name当真实来源。exact type使用canonical diagnostic printer，不使用consumer import alias或link symbol。

### 15.2 稳定错误码

M23-2至少固定以下error code family：`SLIB_CONTAINER_*`、`SLIB_WIRE_*`、`SLIB_LIMIT_*`、`SLIB_DIRECTORY_*`、`SLIB_CAPABILITY_*`、`SLIB_COMPAT_*`、`SLIB_IDENTITY_*`、`SLIB_REFERENCE_*`、`SLIB_BRIDGE_*`、`SLIB_FINGERPRINT_*`、`SLIB_SESSION_CONFLICT`。具体message可改善，code、structured path和primary origin是golden契约。资源错误同时报resource kind/limit/observed，不只报“文件损坏”。

## 16. 测试设计

### 16.1 fixed vectors与golden

每个以下类别都保存人工可审查的input、canonical bytes、32-byte hex和（如适用）mangled/u64 output：

- ConeCoordinate、SourceIdentity、每个source declaration kind、derived key、generated structural path与exact type 1…6 variant；`CallableInstantiationOwner`四个tag都必须有向量，其中tag 4覆盖generic delegated initializer内lambda再声明named local function的application；
- callable body 1…4 tag的runtime encoder bytes，Strong owner每个variant，以及M24 body-v2在v1 reader中的拒绝用例；
- safepoint 5个role、canonical CFG traversal、同role ordinal、runtime/safepoint little-endian截断；用可注入hash test double覆盖zero和u64 collision分支，production hash仍固定SHA-256；
- PersistentV1每个kind tag、linkage非法组合、`@Extern`/runtime ABI绕过与legacy shim单向调用；
- generated root表逐variant覆盖；特别以source nominal、nominal application、tuple/function分别作为coroutine result/value，证明前两者走`ExactOwnerRoot`而不产生非法StructuralType group；
- manifest field 1…11、三层outer envelope、HIR foundation field 1…30及MIR/LIR foundation每张空/单/多entry table；language/runtime/composite identity-ABI与target/backend contract的canonical bytes、fingerprint，以及native symbol normalization边界也必须是fixed vector；
- `SlibMemberRecord` field 1…5、stable-key/role tag 1…6、purpose bit、Capability grammar边界、logical-key上限；
- canonical ar global magic/header/pad、ordinal 0/9/10/65,535，一个、多个与零个LinkObject的directory；
- member/artifact/HIR/MIR/LIR fingerprint及optional attachment不影响semantic fingerprint的对照。

golden文件包含schema/domain/tag版本说明。更新golden必须在review中展示canonical bytes diff，不提供“全部重生成并默认接受”的快捷步骤。

### 16.2 可复现性与round-trip

- 对相同semantic input随机化source输入顺序、arena预占、hash map插入顺序、member提交顺序和lowering任务完成顺序，identity、symbol、metadata与整个`.slib`逐byte一致；
- 在两个隔离临时目录、相对/绝对/等价symlink display spelling下编译single-file fixture，semantic bytes一致；
- 三层分别做`encode -> decode -> validate -> typed remap -> re-encode`，在consumer arena初始状态不同时仍逐byte一致；
- 重复从diamond路径导入同artifact得到同world id，但冲突fingerprint在commit前失败。

### 16.3 negative/corruption/budget

对每个closed record做缺field、多field、重复field、错major type、unknown/reserved tag、非最短integer与乱序table用例。对archive做bit flip、truncation、错size/pad/name/order、thin/special member、重复/缺失/未声明member和hash/fingerprint错配。对identity graph做错kind、错owner、cycle、binder/index/arity/path/span越界与cross-layer bridge漏项；另覆盖tag 4 initialization owner指向别的property/unit、nominal exact type误造StructuralType group、`StaticAssertSupport`产生plan、以及同bridge unit跨两个Cone复用unit/recipe但atom/symbol必须不同的正反对照。

native-boundary witness另有三组Compile测试：当前Cone加trusted core能闭合时成功；缺少本地传递record或夹带无关record时失败；边跨direct dependency时Graph仍成功而M23-2 Compile稳定返回`SLIB_CAPABILITY_NATIVE_BOUNDARY_CLOSURE_REQUIRED`，待M23-6 closure profile提供完整proof后成功。C/Scoop extern还需覆盖Managed与NoGc产生不同fingerprint、却都保持NativeBorrowed caller-root publication的断言。

每项budget都测试`limit-1`、`limit`、`limit+1`，并覆盖多个各自未超限但累计超限的输入。fuzz/property test向raw archive、CBOR、identity graph与remap输入任意bytes，验收条件是有界返回、不panic、不提交部分world，同bytes/同limit产生同error code/path。

unknown optional capability在完整envelope/hash验证后可跳过；unknown Compile-required capability只允许Graph，Compile失败；unknown Link-required object/blob允许Graph/Compile保持opaque，不存在M23-2 Link API可将它误提升。

### 16.4 fixture分层

- `scoop-wire`：codec/hash/budget unit golden和fuzz corpus；
- `scoop-identity`：每个kind/key/mangler/runtime id unit golden；
- HIR/MIR/LIR：foundation projection/wire/remap/cross-reference golden；
- `scoop-slib`：container/directory/manifest/view/corruption/budget测试；
- `tests/fixtures/`：source order/path independence、组合language feature与现有运行行为回归；每条新编译错误规则有独立negative fixture并断言位置/error code/message。

## 17. 实现顺序

1. 建立`scoop-wire`，用固定CBOR/hash/budget/error-path vector锁定最底层；
2. 建立`scoop-identity`，先落Cone/source/kind-specific id/exact type，再落callable/safepoint/runtime/mangler；
3. 同步迁移parser输入、HIR source/origin/visibility与driver reserved source adapter，消除request-local handle渗透；
4. 让HIR lowering产生完整identity foundation，并将current/imported/LocalConcrete id family分开；
5. 迁移MIR全部Scoop symbol及generated entity identity，关闭CompactV2 producer；
6. 迁移LIR callable body、final-CFG safepoint、runtime type id和typed symbol request，codegen只机械消费；
7. 在三个IR crate中实现foundation wire projection、validator、imported set与re-encoder；
8. 在`scoop-slib`实现directory/manifest/fingerprint、canonical normal ar writer/reader、Graph/Compile proof和atomic remap；
9. 运行全量format/lint/test与可复现性/fuzz回归，最后删除所有可到达legacy identity/mangling fallback。

每批实现变更先`cargo fmt --all`与`cargo clippy --workspace`，再跑相关crate、golden和workspace test。不在基础codec未锁定时并行写出多套暂定tag。

## 18. 完成门

M23-2只在以下条件同时成立时完成：

1. 全部目标identity与persistent symbol不因arena、输入、path spelling、hash map seed或线程完成顺序改变，fixed hash/wire/mangle/u64 golden稳定；
2. 所有Scoop-owned linker-visible production symbol都由PersistentV1 typed request产生，`@Extern`、runtime ABI与legacy shim边界有正反测试，CompactV2/ad-hoc path不可达；
3. HIR/MIR/LIR `encode -> decode -> typed remap -> re-encode`逐byte不变，第4.5节HIR definition-source覆盖矩阵与跨层typed identity bridge不能部分缺失；后续body section加入后再把evaluation/concrete origin纳入同一门；
4. canonical `.slib`在隔离目录与乱序输入下bitwise reproducible，raw reader会拒绝所有等价但非canonical的CBOR/ar spelling；
5. typed directory覆盖0/1/N个LinkObject、diagnostic、optional blob与Link-required blob，任何角色都不由文件名、扩展名或member数量推断；
6. `DecodedSlibEnvelope`、Graph、Compile API不能在类型上当作Link proof，foundation artifact不能发布或作dependency；
7. 任意corruption、unknown required capability、超budget、identity/reference/bridge冲突都在原子commit前产生确定性typed error，不panic、不越界、不留部分状态；
8. 现有workspace的format、lint、test、stage dump golden和运行fixture全部通过，新增的identity/wire/slib negative matrix全部通过；
9. M23-3能只通过新section/capability、object/image verifier与production orchestration向前扩展，无需修改本文的identity domain、tag、mangler、bootstrap、directory、container或proof顺序。

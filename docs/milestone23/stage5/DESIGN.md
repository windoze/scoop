# M23-5 设计：多 Cone 名称语义

版本：1.0（设计完成，待实现；2026-09-16）

依赖：M23-4

上位设计：`docs/milestone23/DESIGN.md`

规范依据：

- `docs/specs/SCOOP-SPEC.md` 第 9.1.5、12.3～12.5 节；
- `docs/specs/SCOOP-IMPL-SPEC.md` 第 2.1～2.3、2.6～2.8、2.11 节；
- `docs/milestone17/DESIGN.md` 的 source interface/default template 契约；
- `docs/milestone21/DESIGN.md` 的 access domain、property/accessor 与 export 契约；
- `docs/milestone22/DESIGN.md` 的 non-generic transparent typealias 契约；
- `docs/milestone23/DESIGN.md` 第 0、1.4、2、4.3～4.6、5.2～5.3、8～10 章；
- `docs/milestone23/stage1/DESIGN.md` 至 `docs/milestone23/stage4/DESIGN.md` 已冻结的 source/import、persistent identity、artifact、reader、graph、cache 与 child orchestration 契约。

本文只定义 M23-5。它把 M23-4 已验证的 artifact graph 转成 HIR 可消费的多 Cone semantic world，并闭合 ordinary dependency 的 package/import/re-export、visibility provenance、default、non-generic alias，以及本阶段可安全发射的窄 param-free external callable 子集。layout/scan/typed ABI、继承/dispatch/protected receiver、generic concretization/ODR、runtime multi-image 与最终链接仍分别属于 M23-6～M23-10。

## 0. 结论

M23-5 第一次让普通 dependency 成为**语言名称来源**，但 artifact 在内存中存在不等于其全部内容都可枚举。完成本阶段后：

1. `scoopc` 在解析当前源码前，把 M23-4 已经验证的 direct/support `ValidatedArtifactClosure<Compile>` 重新收窄成一个原子、只读的 `ValidatedCrossConeSemanticClosure`。只有 manifest direct dependency（含 implicit trusted core）的 public binding 可进入普通 lookup；transitive support artifact 只供 typed reference、re-export route、default、alias与后续stage精确取回，永不因已加载而成为候选；
2. `SemanticWorld` 同时保存 session-local `WorldConeId` 与 persistent `ConeIdentity`，所有外部实体按 kind-specific persistent id intern。FQN、package、link symbol、artifact枚举顺序或arena index都不能补猜identity；
3. exact/star/`as` import沿 M23-1 冻结的候选层接入 direct dependency surface。split package只合并namespace视图，不合并实体；同一typed origin经重复import或diamond route到达时合并target并union全部canonical source witness，不同origin保持不同候选；
4. `public import` 同时建立当前文件import与当前Cone re-export binding。每个target必须至少有一个source且全部source都是validated direct-dependency route；current-Cone、internal、private、protected或仅support可达target都不能被提升。re-export保留最终origin identity，不生成wrapper、storage、TypeDescriptor、body或第二个alias target；
5. public star在当前Cone编译时展开为逐binding snapshot。下游只读取已解析snapshot，不重新执行上游glob。任一target非法时整条`public import ...*`失败，不发布部分snapshot；
6. foreign public declaration和current declaration投影成同一种typed view，M16/M18仍逐候选执行shape filter、applicability与MSC。`.slib`来源、route长度、dependency顺序与“本地优先”都不是新的tie-break；
7. 跨Cone普通lookup只有`PublicDependencyLookupWitness`这一条成功证明。foreign internal/private实体没有可构造的lookup witness；它们即使因上游实现、object或future generic support出现在closure中，也不能进入import/member候选。protected inheritance surface在M23-6前不开放；
8. exported default继续是定义方已完成名称解析和overload选择的hygienic typed template。consumer只做type/value substitution与evaluation-origin构造，不按本地import或re-export重新解析。re-export引用同一template，不复制body；
9. non-generic typealias保留自己的persistent alias identity、visibility与target。跨Cone import/re-export保留alias binding；使用时由一个bounded、memoized的closure-wide expander透明展开。alias identity不因target变化而改变，但HIR fingerprint必须改变；
10. M23-5 的 executable external-use成功子集严格限定为：public `const val`的core-closed常量值，以及非generic、non-suspend、non-extern的top-level function、top-level property accessor或top-level extension function/property accessor，其完整exact签名只含trusted core已经由M23-3证明的param-free ABI leaf。consumer只发typed undefined requirement，不重发provider body或任何Strong definition；
11. 名称解析本身可以成功指向class/struct/enum/interface/object、constructor/member、generic declaration或任意公开property；但一旦具体使用需要foreign nominal layout/scan/TypeDescriptor、constructor/materialization、member/virtual dispatch、receiver-dependent protected access、function-value representation、generic application或native provider，就在HIR winner commit前以对应阶段的唯一能力诊断失败，不产生`LocalConcreteHir`残片；
12. 本阶段新增`cross-cone-semantics-strong/1` artifact profile，以及HIR general interface、MIR/LIR param-free bridge和Link-only cross-Cone use closure四条capability。M23-3的`single-cone-strong/1`仍可被旧reader识别，但不能进入M23-5 build；trusted core、prebuilt与cache artifact必须按新profile重建；
13. M23-3既有`core-bootstrap-interface/1`、`core-bootstrap-bridge/1`、`strong-production/1`与`link-identity-closure/1`字节和语义不变。ordinary dependency callable不伪装成`CoreStrong`；它在新的LIR semantic arena与新的Link-only physical-use closure中形成互斥分区，Code fingerprint通过既有known Link-required extension contribution机制覆盖该分区；
14. HIR layer在M23 v1继续保守纳入全部direct dependency HIR fingerprint；MIR/LIR也继续沿用全部direct dependency对应层fingerprint作为cache安全基线。`LookupObservationSet`和`SelectedExternalSet`本阶段完整产生并测试，但不用于减少cache edge，避免negative lookup、star snapshot或re-export变化被错误复用；
15. 每个成功artifact仍必须从最终bytes分别通过Compile与Link view，并在closure级证明所有ordinary external callable requirement精确命中route终点provider的strong definition。任一wire、route、visibility、bridge、object use或fingerprint关系不一致均原子失败；
16. M23-4的locator、DAG、source snapshot、cache store、child protocol和canonical调度不重写。production child不再因“存在non-core dependency”无条件失败，而是在完整semantic closure通过后按本文能力矩阵编译；parent仍在发布cache前重新执行双view与closure验证。

核心不变量是：

```text
artifact in support closure != source-visible provider
public symbol           != public language binding
re-export binding       != copied declaration
successful lookup       != executable external-use capability

ordinary lookup source = current Cone
                       | validated direct dependency public route

external code use = committed HIR winner
                  + terminal provider export bridge
                  + exact MIR/LIR selected relation
                  + typed undefined-use proof
```

## 1. 范围与阶段边界

### 1.1 本阶段交付

- `ValidatedCrossConeSemanticClosure`、`SemanticWorld`、direct/support provider role和closure-wide typed identity interning；
- current/direct/support三类authority在类型上的隔离；
- general public binding index、declaration/source-interface/default/const/alias semantic surface；
- `ImportedBinding`、`ImportedTargetBinding`、`ImportBindingSource`与canonical route witness；
- exact/star/alias import、longest visible package prefix、static owner traversal与split-package lookup；
- `public import` exact/star snapshot、chain/diamond re-export、当前Cone public-surface conflict检查；
- public/internal/private跨Cone边界及kind-specific access provenance；
- M16/M18 candidate layer接入、duplicate-origin folding、overload group与negative lookup observation；
- M17 exported default template的wire、closure validation、winner-only实例化与definition/evaluation origin；
- M22 non-generic typealias的wire、跨Cone展开、re-export与cycle/budget检查；
- `CrossConeUseSet { lookup_observations, selected_external }`；
- `LocalConcreteHir`、MIR与LIR中的external param-free callable target；
- ordinary dependency strong callable的producer export bridge、consumer selected bridge、typed object relocation与Link-only requirement closure；
- `cross-cone-semantics-strong/1` profile、四条新增capability与profile migration；
- HIR/MIR/LIR Merkle contribution/support-edge及M23-4 cache invalidation衔接；
- direct/transitive、split package、re-export、visibility、default、alias、stage gate、corruption、dual-view与determinism测试矩阵；
- M23总设计、language/implementation spec与ROADMAP的阶段链接和profile说明。

### 1.2 本阶段明确不做

- 通用foreign nominal `ValueStorageLayout`、scan、TypeDescriptor、boxing、Scoop typed ABI、ZST token或array representation；这些属于M23-6；
- 跨Cone构造器调用、struct/enum payload构造/解构、class allocation、foreign field direct place、object/singleton value读取；它们需要M23-6的layout/scan/materialization proof；
- 跨Cone真实member/virtual/interface dispatch、inheritance/slot fulfillment、interface default选择或receiver-dependent protected access；这些属于M23-6；
- generic nominal/callable/property application、generic hidden support closure、consumer concretization、ODR member/definition proof或generic delegated extension；这些属于M23-7；
- runtime multi-image registration、eager initialization执行、stackmap合并或managed program启动；这些属于M23-8；
- program descriptor、runtime-build、native linker调用、binary或运行；这些属于M23-9～M23-10；
- 直接调用foreign `@Extern` declaration、native provider解析或把ordinary dependency callable改写成native symbol；general native closure属于M23-10；
- registry下载、version range、dependency alias、friend/internal export、sealed package、dynamic image或ABI-stable plugin；
- generic/nested typealias；
- 用精确lookup observation裁剪cache edge。本文先产生完备observation，但v1 cache仍保守；
- 修改M23-2冻结的persistent identity/mangler/container、M23-3冻结的runtime descriptor ABI或M23-4 locator/cache/transport语义。

### 1.3 与相邻阶段的交接

| 阶段 | M23-5 接收 | M23-5 交付 |
| --- | --- | --- |
| M23-4 | 已完成节点的direct/support Compile与Link closure、canonical dependency order、cache/child gate | 可直接传给HIR的semantic closure；scheduler只更新accepted profile和删除无条件non-core拒绝 |
| M23-6 | persistent exact/source/owner identity、name/access witness、selected external relation与明确的layout/dispatch拒绝 | 新layout/ABI/scan/inheritance capability可在同一world上精化target，不重跑import或按名称找实体 |
| M23-7 | generic declaration name/source interface、type parameter/bound identity与明确的generic拒绝 | hidden support/ODR capability沿已选typed template继续，不把support entity加入普通lookup |
| M23-8 | per-Cone image与canonical graph order、外部accessor内部自有init unit | runtime登记全部image后，provider accessor按自身unit语义执行；consumer不复制init storage |
| M23-9/10 | Link closure中的typed cross-Cone requirement、terminal provider与physical use proof | program-link只做全程序definition/native resolution，不补做源码visibility/import |
| M23-11 | multi-Cone library/executable已可经orchestrator产出完整`.slib` closure | 正式CLI、fixture迁移与真实link/run总验收 |

### 1.4 封闭成功子集

M23-5把“名称可见”和“可生成machine use”分开。下表是本阶段唯一成功矩阵：

| 形态 | import/re-export | 仅作type/namespace surface | 进入LocalConcrete/MIR/LIR |
| --- | --- | --- | --- |
| public non-generic type/constructor/member | 是 | 是 | 否，报M23-6 layout/dispatch能力诊断 |
| public generic declaration | 是 | 是 | 否，application报M23-7能力诊断 |
| public non-generic typealias | 是 | 是，透明展开 | 仅最终target完全落入本表其他成功格时 |
| public `const val` | 是 | 是 | core-closed constant允许内联；不引用provider storage |
| ordinary top-level function | 是 | 是 | 满足1.5 param-free bridge条件时允许 |
| top-level/extension property | 是 | 是 | 非const只允许通过满足1.5的getter/setter bridge |
| member function/property/constructor/object value | 是 | 是 | 否，报M23-6能力诊断 |
| source `@Extern` | 是 | 是 | 否，报M23-10 native-closure能力诊断 |
| internal/private/protected declaration | 否 | 只可作为规定的非lookup support | 否；protected到M23-6继承入口 |

unused import、纯re-export Cone、跨Cone alias facade和只使用允许bridge的param-free library/executable都能成功产生双view有效artifact。M23-5不以“当前还不能最终运行”为理由拒绝这类artifact；M23-8/9负责runtime/link消费，而不是回来补语言语义。

### 1.5 `ParamFreeCoreClosedCallableV1`

ordinary external callable只有同时满足以下全部条件，才能从成功lookup精化为可执行target：

1. target是terminal provider自己定义的top-level ordinary/extension function，或top-level/extension property的getter/setter；不是constructor、member、local、lambda、callable reference、intrinsic或source extern；
2. declaration和owner都非generic，callable non-suspend；source parameter interface不存在需要generic array application的vararg；
3. MIR implementation是provider现有`StrongCallableDefinitionOwner`，且provider的M23-3 strong production已经定义body、entry、registration和required object definition；
4. receiver、全部parameter与result的`ExactCallableSignature`均通过同一个`CoreClosedExactLeafClassifierV1`。classifier只接受trusted core artifact已经由M23-3 `ParamFreeStrong`/well-known builtin proof闭合的exact nominal leaf；拒绝foreign nominal、nominal application、tuple、function、raw/native pointer和type parameter；
5. LIR canonical Scoop ABI由上述core leaf proof机械重放，不能读取provider host layout或新建M23-6 layout record；
6. `GcEffect`与calling convention完整一致，Managed调用使用statepoint，NoGc调用使用NoGc root plan；ordinary/suspend与GC effect仍是两个不同维度；
7. selected target有至少一条从当前Cone direct dependency开始并终止于该provider binding的合法source route；provider是transitive support时只能由re-export route到达；
8. consumer只建立`DependencyExternalCallable`和undefined requirement；body、registration、TypeDescriptor、storage、initializer与definition fingerprint继续由provider拥有。

classifier复用M23-3 core param-free proof，不冻结一般Scoop ABI；只要某个参数需要M23-6的新layout/ABI section，整个candidate仍可参与名称诊断，但不能成为本阶段的可执行winner。

## 2. crate与依赖方向

### 2.1 不新增stage实现依赖

M23-5主要扩展已有crate：

```text
scoop-identity  persistent binding/route所需typed DTO（不新增entity id family）
scoop-hir       cross-Cone interface、imported world view、HIR selected set
scoop-hir-lower SemanticWorld构造与resolver接入
scoop-mir       param-free export/selected bridge DTO
scoop-mir-lower external callable lowering
scoop-lir       dependency external callable与semantic bridge DTO
scoop-lir-lower exact selected-set消费
scoop-slib      新profile/capability、closure reader、Link physical-use proof
scoop-driver    direct/support preflight、stage投影与artifact publication
scoop           accepted profile/cache completion更新；不依赖lowerer实现
```

依赖方向继续是：

```text
hir-lower -> ast + hir
mir-lower -> hir + mir
lir-lower -> mir + lir
driver    -> 各IR crate + 各stage实现 + slib
scoop     -> manifest/protocol/toolchain/slib，不依赖parser/lower/codegen实现
```

`SemanticWorld`的只读数据类型和builder proof位于`scoop-hir`；`hir-lower`只执行当前AST到HIR的变换。`scoop-slib`返回typed imported sections，不依赖`hir-lower`。MIR/LIR的selected input分别定义在`scoop-mir`/`scoop-lir`，driver只按`CrossConeUseSet`投影，不能让lowerer打开`.slib`。

### 2.2 typed输入链

```text
Explicit dependency paths
  -> ValidatedArtifactClosure<Compile>
  -> ValidatedCrossConeSemanticClosure
  -> ImportedSemanticWorld
  -> CurrentConeSemanticWorld
  -> CompleteHirOutput {
       export,
       local_concrete,
       cross_cone_uses,
     }
  -> SelectedImportedMir
  -> MirOutput + CrossConeMirBridgeSectionV1
  -> SelectedImportedLir
  -> LirOutput + CrossConeLirBridgeSectionV1
  -> objects + CrossConeLinkClosureSectionV1
  -> dual-view valid .slib
```

每个箭头消费前一状态并产生不可伪造的新状态。不存在从`ValidatedArtifactClosure<Compile>`直接取“metadata map”、从persistent digest直接构造imported arena id、或从Link view反向恢复HIR声明的入口。

### 2.3 current/direct/support authority

```text
WorldProviderRole = Current
                  | Direct { edge: ValidatedDirectDependencyEdge }
                  | Support { via: NonEmpty<ValidatedDependencyEdge> }
```

- `Current`可按M21 access domain枚举本Cone声明；
- `Direct`可枚举其public binding surface并产生`PublicDependencyLookupWitness`；
- `Support`只能按已经持有的kind-specific persistent ref取回声明/definition，不提供package、name、member或prelude枚举API；
- implicit core是`Direct`，同时额外持有trusted-core capability；ordinary direct artifact不能构造该marker；
- 一个Cone对当前root恰有一个role。既是多条路径的support时合并路径；只要存在direct edge即规范化为`Direct`，但仍保留全部graph path用于诊断和fingerprint验证。

公开API不提供`role -> bool`后再调用统一枚举器；只有`DirectProviderView`实现`public_bindings()`，`SupportProviderView`只有typed lookup方法，从类型上阻止误枚举。

## 3. artifact profile、capability与迁移

### 3.1 新profile

新增：

```text
org.scoop-lang.slib-profile/cross-cone-semantics-strong/1
```

descriptor固定为：

```text
required_manifest = [
  org.scoop-lang.manifest/single-cone-production/1,
]

required_hir = [
  org.scoop-lang.hir/core-bootstrap-interface/1,
  org.scoop-lang.hir/cross-cone-interface/1,
  org.scoop-lang.hir/identity-foundation/1,
]

required_mir = [
  org.scoop-lang.mir/core-bootstrap-bridge/1,
  org.scoop-lang.mir/cross-cone-param-free-bridge/1,
  org.scoop-lang.mir/identity-foundation/1,
]

required_lir = [
  org.scoop-lang.lir/cross-cone-link-closure/1,
  org.scoop-lang.lir/cross-cone-param-free-bridge/1,
  org.scoop-lang.lir/identity-foundation/1,
  org.scoop-lang.lir/link-identity-closure/1,
  org.scoop-lang.lir/strong-production/1,
]

code_requirement    = MustBeAvailable
runtime_requirement = MustBeAvailable
publication_class   = Publishable
validation_policy   = {
  odr: RejectAll,
  extra_sections: AllowPurposeDisjointOpaqueAndEnvelopeOptional,
  decode_cost_model: DeterministicLogicalCostV1,
  link_proof: Required,
}
```

数组实际按`CapabilitySortKey`编码；上面为便于阅读按层分组。profile仍拒绝任何ODR group/member/body/linkage/symbol，M23-7才更换为`RequireCompleteDefinitionProof`。profile fingerprint由M23-2既有公式计算并加入fixed vector；不得手写常量后跳过descriptor重算。

### 3.2 新capability contract

| capability | location | required_for | sinks |
| --- | --- | --- | --- |
| `org.scoop-lang.hir/cross-cone-interface/1` | HIR | Compile | HIR |
| `org.scoop-lang.mir/cross-cone-param-free-bridge/1` | MIR | Compile | MIR |
| `org.scoop-lang.lir/cross-cone-param-free-bridge/1` | LIR | Compile | LIR |
| `org.scoop-lang.lir/cross-cone-link-closure/1` | LIR | Link | Code + LinkValidationOnly |

前三条payload的完整canonical inner bytes分别成为对应layer contribution。Link closure的semantic projection（provider/target/symbol/definition，不含member/range/offset）进入Code contribution；物理use site、member range和verification index只作`LinkValidationOnly`，必须从object与semantic projection完全重算。

`cross-cone-link-closure/1`不产生LIR contribution，不能被Compile reader用来补一个缺失的semantic bridge。反之Compile-only bridge不能被Link reader当作物理proof。只有同一artifact完成两种view并通过3.4 dual-view equality后，才成为可发布handle。

### 3.3 旧capability不改义

- `core-bootstrap-interface/1`继续保存output contract、direct public binding inventory、prelude与well-known core协议；M23-5 general HIR section的current-owned direct binding投影必须逐byte等于该inventory，re-export不写回旧字段；
- `core-bootstrap-bridge/1`和`strong-production/1.external_bridges`继续只描述M23-3 trusted-core consumer路径；ordinary dependency使用新arena，不能把provider identity塞入`CoreStrong { core }`字段；
- `link-identity-closure/1`继续精确覆盖其既有intra-Cone/core/generated/native/runtime/target分区；ordinary dependency object relocation从构造时即属于新的`CrossConeStrongRelocation`分区，不进入旧closure的coverage输入；
- object union coverage额外证明：每个raw undefined relocation恰属于旧closure或新cross-Cone closure之一，二者use site集合不相交，联合后无遗漏；
- `single-cone-production/1`的Code计算继续使用既有通用“known Link-required contribution”入口纳入新section，不改变旧字段、tag或hash domain。

### 3.4 profile迁移与双视图

M23-5 production graph只接受新profile：

- source Cone、single-file、trusted core都写新profile，即使新section为空；
- M23-3 `single-cone-strong/1` prebuilt/cache entry报告明确的profile mismatch并要求重建，不做in-memory upgrade；
- trusted core slot receipt和compile cache key都绑定新profile id/fingerprint；
- graph summary仍可读取旧artifact用于报告coordinate，但旧artifact不能成为completed node或dependency authority；
- writer从最终bytes独立构造`ValidatedCompileArtifact<CrossConeSemanticsStrongProfile>`与对应Link view；publish gate再比较Compile LIR bridge的import projection与Link closure的semantic projection，逐byte不等即失败。

迁移不提升outer schema、identity foundation或container schema。新增能力通过section inventory fail closed；旧reader看到新required capability必须拒绝，而不是忽略后继续编译或链接。

## 4. semantic closure与原子import

### 4.1 closure输入

`scoopc`收到的普通dependency仍分成`direct_slibs`与`support_slibs`。M23-4已验证path/graph；child必须独立重建：

```text
CrossConeClosureInputV1 {
    current: ConeIdentity,
    direct: CanonicalMap<ConeIdentity, CompileArtifactHandle>,
    support: CanonicalMap<ConeIdentity, CompileArtifactHandle>,
    edges: CanonicalSet<DependencyRecordEdge>,
    trusted_core: TrustedCoreArtifactHandle,
}
```

`direct`精确等于manifest direct edge加implicit core；`support`精确等于它们的递归依赖减去direct/current。命令行位置、basename或传入顺序不决定role。缺失、额外、重复identity、same-id different fingerprint、stale edge、cycle、multiple version、wrong kind/profile/target在parser前失败。

### 4.2 验证状态机

closure构造固定为：

```text
DecodedCrossConeClosure
  -> EnvelopeValidatedClosure
  -> ProfileValidatedClosure
  -> IdentityRegisteredClosure
  -> SurfaceValidatedClosure
  -> RouteValidatedClosure
  -> BridgeValidatedClosure
  -> AtomicallyCommittedSemanticClosure
```

1. 按M23-4 canonical dependency-first order重开每个Compile view；
2. 验证profile inventory、section canonical bytes、resource budget与三层fingerprint；
3. 在一个closure transaction中先登记全部artifact自己声明的identity delta，再登记已经验证的external reference leaf；同kind/id的canonical key冲突、重复owner或origin fingerprint冲突使整个transaction失败；
4. 验证每个general HIR surface只声明本Cone拥有的declaration/binding，foreign target只作typed ref；
5. 验证re-export route、signature/default/alias external reference closure和direct/support reachability；
6. 验证provider MIR/LIR export bridge、consumer selected bridge与本阶段eligibility；
7. 最后一次性向`SemanticIdentitySession`和world arenas commit。

任一步失败都丢弃全部pending ids、provider slots、package index、route cache、alias expansion cache和bridge selection；不能保留“已经成功导入的前几个dependency”。

### 4.3 identity authority

M23-2 HIR foundation field 16继续保存所有current-exporter的`PersistentExportBindingId` record：

- direct declaration binding的target由当前artifact声明；
- re-export binding的`ExportBindingKey.exporter`仍是当前Cone，但target可以是dependency声明的persistent id；
- 解析re-export key前，closure validator必须已经从terminal provider导入该target的canonical identity key；只登记raw digest而没有validated provider authority不能满足；
- re-export identity按既有`ExportBindingKey { exporter, package, namespace, name, target, role }`重算，不新增`PersistentReexportId`；
- 同一destination/name/target但route集合变化时binding id不变、surface payload与HIR fingerprint变化。这保证route是可失效的授权证据，不错误成为实体identity。

### 4.4 world输出

```text
ImportedSemanticWorld {
    providers: CanonicalArena<ImportedProvider>,
    direct_packages: DirectPackageIndex,
    typed_entities: ImportedEntityArenas,
    public_bindings: DirectPublicBindingIndex,
    reexport_routes: ReexportRouteIndex,
    source_interfaces: ImportedSourceInterfaceSet,
    aliases: ImportedAliasSet,
    constants: ImportedConstSet,
    callable_bridges: ImportedParamFreeCallableSet,
}
```

arena按`(kind tag, persistent id bytes)`稳定intern；分配出的local index不进入wire、fingerprint或诊断排序。`DirectPackageIndex`只接收direct provider；typed arenas可含support实体但没有反向name enumeration。dump显示`external(origin@coordinate,id)`，display字符串不参与判等。

## 5. HIR wire：`cross-cone-interface/1`

### 5.1 顶层payload

```text
CrossConeHirInterfaceSectionV1 {
    public_bindings: CanonicalVec<PublicExportBindingRecordV1>,       // field 1
    nominal_interfaces: CanonicalVec<NominalInterfaceRecordV1>,      // field 2
    callable_interfaces: CanonicalVec<CallableInterfaceRecordV1>,    // field 3
    property_interfaces: CanonicalVec<PropertyInterfaceRecordV1>,    // field 4
    type_aliases: CanonicalVec<TypeAliasInterfaceRecordV1>,           // field 5
    source_interfaces: CanonicalVec<CallableSourceInterfaceV1>,       // field 6
    default_templates: CanonicalVec<ExportDefaultTemplateV1>,         // field 7
    constants: CanonicalVec<ExportConstValueV1>,                      // field 8
    definition_sources: CanonicalVec<ExportDefinitionSourceV1>,       // field 9
    external_references: CanonicalVec<ExternalHirReferenceV1>,        // field 10
}
```

十张顶层table均按各record声明的typed primary key严格递增并拒绝重复；producer输入乱序由writer排序，reader绝不排序修复。record内部标为`CanonicalVec`/`CanonicalSet`的集合也按其元素typed key或canonical wire bytes严格递增；标为declaration/source order的序列保持源码语义顺序，以隐含的zero-based `u32` position作identity，reader不得排序。payload只保存canonical semantic interface，不直接serde `ExportHir` arena，不保存arena id、import文本、host path、failed candidate或body-local display name。

### 5.2 public binding与route

```text
PublicExportBindingRecordV1 {
    binding: PersistentExportBindingId, // field 1
    source: ExportBindingSourceV1,       // field 2
}

ExportBindingSourceV1 =
    DeclaredCurrent { declaration: BindableEntity }      // tag 1
  | Reexport { routes: NonEmpty<ReexportRouteV1> }        // tag 2

ReexportRouteV1 {
    immediate_provider: ConeIdentity,              // field 1
    hops: NonEmpty<ReexportRouteHopV1>,             // field 2
}

ReexportRouteHopV1 {
    exporter: ConeIdentity,                         // field 1
    binding: PersistentExportBindingId,             // field 2
}
```

验证规则：

- binding必须恰好存在于当前artifact HIR foundation field 16，key的exporter等于当前Cone；
- `DeclaredCurrent` target必须与binding key target相等、origin为当前Cone、对应声明显式public且owner effective domain为public；该集合逐byte等于`core-bootstrap-interface/1.direct_public_surface`，无论当前Cone是否core；
- `Reexport` binding不进入旧direct surface。每条route的`immediate_provider`必须是当前Cone direct dependency，`hops[0].exporter`与之相等；
- 每个hop的binding key exporter等于hop exporter，target/namespace/role在整条route上相同，package/name允许因alias/re-export变化；
- terminal hop必须是其exporter的`DeclaredCurrent` binding；中间hop必须在对应provider的`Reexport.routes`中存在完全相同的suffix。reader不能只检查“id存在”而忽略route连续性；
- route不能重复Cone或binding，长度不超过closure node count，因此cycle与unbounded chain均拒绝；
- routes按完整`(immediate provider coordinate/id, hop sequence)`canonical bytes排序去重。diamond的每条合法route都保留；
- re-export target若是typealias，terminal target仍是`PersistentTypeAliasId`，不能提前替换为展开type；
- 同一binding不能同时`DeclaredCurrent`和`Reexport`，也不能以空routes表示无来源。

### 5.3 declaration interface

HIR section使用封闭typed record，不复制实现body：

```text
NominalInterfaceRecordV1 {
    declaration: SourceNominalId,         // field 1
    kind: Struct | Enum | Class | Interface | Object, // field 2
    type_parameters: CanonicalBinderList, // field 3
    exact_supertypes: CanonicalVec<SignatureTypeKey>,  // field 4
    constructors: CanonicalVec<PersistentConstructorId>, // field 5
    members: CanonicalVec<PublicMemberRefV1>,            // field 6
    nested_bindings: CanonicalVec<PersistentExportBindingId>, // field 7
    value_shape: NominalSourceShapeV1,     // field 8
}

CallableInterfaceRecordV1 {
    declaration: CallableDeclarationId,   // field 1
    owner: TopLevel | Nominal(SourceNominalId) | Extension, // field 2
    type_parameters: CanonicalBinderList, // field 3
    receiver: None | SignatureTypeKey,    // field 4
    parameters: CanonicalVec<SourceParameterShapeV1>, // field 5
    result: SignatureTypeKey,             // field 6
    effects: CallableSourceEffectsV1,      // field 7
    modality: Final | Open | Abstract | InterfaceDefault, // field 8
    access: PublicLookupAccessV1,          // field 9
}

PropertyInterfaceRecordV1 {
    declaration: PropertyDeclarationId,   // field 1
    owner: TopLevel | Nominal(SourceNominalId) | Extension, // field 2
    receiver: None | SignatureTypeKey,    // field 3
    value_type: SignatureTypeKey,          // field 4
    mutability: Val | Var,                 // field 5
    getter: PersistentPropertyAccessorId,  // field 6
    setter: None | PersistentPropertyAccessorId, // field 7
    representation: Const | RuntimeAccessor | AbstractSlot, // field 8
    access: PropertyPublicAccessV1,        // field 9
}
```

声明引用复用M23-2 identity wire的既有typed id，不再包一层无语义的digest：

```text
SourceNominalId =
    Concrete(PersistentTypeId)                         // tag 1
  | GenericTemplate(PersistentGenericTypeId)           // tag 2

CallableDeclarationId =
    Function(PersistentFunctionId)                     // tag 1
  | GenericFunction(PersistentGenericFunctionId)       // tag 2
  | Constructor(PersistentConstructorId)               // tag 3
  | PropertyAccessor(PersistentPropertyAccessorId)     // tag 4
  | EnumVariantConstructor(PersistentEnumVariantId)    // tag 5

PropertyDeclarationId =
    Property(PersistentPropertyId)                     // tag 1
  | ExtensionProperty(PersistentExtensionPropertyId)   // tag 2

PublicMemberRefV1 =
    Callable(CallableDeclarationId)                    // tag 1
  | Property(PropertyDeclarationId)                    // tag 2
```

前三个sum分别逐byte等同既有`NominalDeclarationOwner`、`CallableTemplateOrigin`和`PropertyOwner`编码，reader复用其decoded type与canonical identity resolver；不得重新分配tag，也不得把它们退化成`{ kind: integer, digest: bytes }`。`PublicMemberRefV1`是新增的外层kind sum。source constructor在nominal的`constructors`列出并另有callable/source interface，enum variant在`value_shape`列出且其payload constructor另有callable/source interface，nested nominal/value通过`nested_bindings`授权；这些实体都不得再重复出现在`members`。

四类table主键与内部集合顺序固定为：

- nominal table按`SourceNominalId`的`(tag, raw id bytes)`，callable table按`CallableDeclarationId`，property table按`PropertyDeclarationId`严格递增；
- `exact_supertypes`按完整`SignatureTypeKey` canonical bytes排序去重；`constructors`与`nested_bindings`按raw id bytes排序去重；`members`按`PublicMemberRefV1`的`(tag, nested tag, raw id bytes)`排序去重；
- binder、callable parameter、struct field、enum variant及variant field是declaration-order序列，不按名称或类型排序；其数量和position必须可表示为`u32`，同一owner内的source name遵守各自语言重复声明规则；
- 每个member/constructor/nested binding必须由当前nominal直接拥有；每个callable/property的`owner`必须与其canonical declaration key及nominal record一致。consumer不能扫描FQN、其他table或arena ordinal猜测owner。

`SourceNominalId`、`CallableDeclarationId`、`PropertyDeclarationId`和`PublicMemberRefV1`因此都是kind-specific closed sum，不能用裸digest union。struct/enum的declaration-order fields/variants、class constructor source shape、object type/value relation、operator/infix标志、property accessor effect及annotation的语义部分通过各自closed constituent进入上述record；它们只支撑HIR lookup/type checking，不授权layout或dispatch。

只有public lookup surface中的声明写入这些表。public owner所需的protected inheritance/slot内容不塞入nullable字段；M23-6以独立required capability加入。generic declaration可在这里提供name、binder、bound和source signature，但没有template body/hidden support；任何application由M23-7 gate拒绝。

### 5.4 source interface与default

`CallableSourceInterfaceV1`完整编码M17参数协议：owner、声明顺序、source name、value type，以及`Required | Default(template) | VarargEmpty | VarargDefault(template)`封闭sum。owner和parameter position必须与`CallableInterfaceRecordV1`逐项对应。

`ExportDefaultTemplateV1`保存：

```text
ExportDefaultTemplateV1 {
    key: ExportDefaultTemplateKeyV1,      // field 1，owner + parameter position
    definition_root: PersistentLexicalRootV1, // field 2
    definition_path: StructuralDefinitionPath, // field 3
    locals: CanonicalTemplateLocalTableV1,      // field 4
    body: ExportDefaultBodyV1,                  // field 5
    result: SignatureTypeKey,                   // field 6
    allows_suspend: bool,                       // field 7
    type_parameters: CanonicalBinderUseList,    // field 8
    receiver: None | TemplateReceiverV1,        // field 9
    value_parameters: CanonicalVec<TemplateValueParameterV1>, // field 10
    references: ExportDefaultReferenceSetV1,    // field 11
    definition_origin: ExportDefinitionSourceV1, // field 12
}
```

`ExportDefaultTemplateKeyV1`只是本section内按`(kind-specific owner, parameter position)`排序和引用的canonical key，不产生新的persistent id，也不进入M23-2 identity foundation。wire内对template的引用使用经范围验证的table index；跨artifact稳定语义由owner persistent identity、parameter position与完整template payload共同确定。

`ExportDefaultBodyV1`是M17已typed的statement/expression tree之canonical wire：节点按结构递归编码，statement和expression使用不同closed sum；local/type/callable/constructor/global/singleton/field引用分别使用不同typed ref；每个expression非可选地保存result type与definition origin。它覆盖当前语言已允许的完整default表达式，不把未解析name、import path、candidate set、arena index或调用方span写入wire。

每个`ExportDefaultReferenceSetV1` constituent携带目标kind-specific persistent id、definition origin与`ExportDefaultAccessWitnessV1 { owner, call_domain, target_domain }`。reader从目标public interface重算domain包含关系；foreign internal/private target、缺失route、普通`Export*Id`冒充refined ref或body引用未列入reference set均拒绝。reference set必须与body实际typed引用的规范去重集合完全相等，无多余项。

consumer只在winner与完整type arguments确定后实例化。template的definition origin保持provider source；新concrete expression的evaluation origin取当前call expression。若展开后的任一operation需要M23-6/7/10能力，则该candidate在applicability阶段失败并记录结构化原因；不能先commit winner再让MIR失败。

### 5.5 const与typealias

```text
ExportConstValueV1 {
    property: PersistentPropertyId, // field 1
    value_type: SignatureTypeKey,   // field 2
    value: CanonicalConstValueV1,   // field 3
    definition_origin: ExportDefinitionSourceV1, // field 4
}

TypeAliasInterfaceRecordV1 {
    alias: PersistentTypeAliasId,   // field 1
    target: SignatureTypeKey,       // field 2
    access: PublicLookupAccessV1,   // field 3
    definition_origin: ExportDefinitionSourceV1, // field 4
}
```

const table精确覆盖public const property，不含ordinary property storage/initializer。consumer内联value并使用当前usage evaluation origin；String等需要本地materialization的常量由当前Cone按既有literal规则拥有，不引用provider storage。

alias target允许指向direct dependency public type/alias。closure-wide expander以`PersistentTypeAliasId`作memo key，沿typed target展开，执行checked node/depth/text budget并报告完整alias chain。使用点`as` alias和import local name不是typealias identity，不进入该图。展开完成后所有type equality、overload signature和persistent application都使用最终target；diagnostic可保留alias spelling作decorator。

### 5.6 external reference closure

`external_references`是本HIR payload全部foreign typed ref的精确闭包：

```text
ExternalHirReferenceV1 {
    origin: ConeIdentity,                // field 1
    target: ExternalHirTargetV1,         // field 2，kind-specific sum
    roles: NonEmptyCanonicalSet<ExternalHirReferenceRoleV1>, // field 3
    witnesses: CanonicalVec<DependencyBindingWitnessV1>,     // field 4
}
```

role封闭为`ReexportTarget`、`SignatureDependency`、`AliasTarget`、`DefaultDependency`、`ConstType`和`ConcreteSelectedUse`。reader从fields 1～9的实际引用重建去重后的expected closure并逐byte比较；extra/missing role或错误origin均失败。只有需要source-name授权的role携带binding witness；signature dependency仍须有定义方已验证的signature exposure proof，但不会因此创建下游短名。

## 6. re-export构造与公开表面

### 6.1 import先于re-export commit

每个source file按以下顺序建立transaction：

1. 解析package并取得current/direct可见package view；
2. 逐条解析ordinary/public exact或star import，得到尚未commit的typed binding group；
3. 为public import验证全部source是DirectDependency、target public/exportable、route闭合；
4. public star先生成完整canonical expansion，再整体验证；
5. 所有文件完成后，把拟发布binding与current public declaration合并，执行全Cone duplicate/conflict检查；
6. 只有全部通过才生成persistent re-export binding identity并一次性commit到Export HIR。

失败file不留下local import；失败public import不留下任一re-export；不同file产生的合法相同binding按target origin合并route，不按file顺序覆盖。

### 6.2 destination与conflict

- destination package恒为声明该`public import`的source package；
- exact destination name是`as` alias或target短名；exact function/extension selector可得到一个overload group，alias重命名整个已唯一确定的group，不从歧义group任选；
- star为每个target使用其当前导出短名，且不递归子namespace；
- 同destination namespace/name下，同origin target合并route；不同origin function/extension允许组成overload set，但normalized signature相同是定义冲突；不同origin非overloadable target直接冲突；
- type/value namespace继续分离，role不匹配不能靠同名合并；
- local public declaration与re-export应用同一规则。current declaration不因“更本地”自动胜出，也不能由`public import`复制为另一binding；
- 公开surface conflict在当前Cone HIR结束前报告，不能发布等待下游歧义的artifact。

### 6.3 链式与diamond

若`B`声明target，`A public import B.target`，`C`只direct依赖A：

```text
C source witness:
  immediate provider = A
  route = [A.export(target), B.declared(target)]
  terminal origin = B.target
```

C的candidate identity、default、alias和最终external body都取B；A只提供访问route。diamond中A1/A2都转发B时，C若direct依赖A1/A2，target只intern一次并保留两组routes。删除一路会改变C观察到的source witness snapshot与HIR fingerprint；另一路仍在则lookup继续成功。

## 7. namespace、import与candidate层

### 7.1 package index

`DirectPackageIndex`的key是canonical `PackagePath`，value为来自current和每个direct provider的non-empty contribution。package不是Cone identity；split package合法。support provider不建立contribution。

selector解析：

1. 在当前world查找能形成可见package binding的最长前缀；
2. 一旦选定不回退到较短package前缀；
3. 剩余segment沿static nested nominal/object/companion的typed namespace edge前进；
4. exact终点必须是binding group，star终点必须是namespace；
5. package与owner同名冲突按最长package规则固定，不扫描FQN或artifact列表重猜。

### 7.2 import binding

```text
ImportedBinding {
    local_name: CanonicalIdentifier,
    targets: NonEmpty<ImportedTargetBinding>,
}

ImportedTargetBinding {
    target: ImportedTarget,
    sources: NonEmptyCanonicalSet<ImportBindingSource>,
}

ImportBindingSource =
    CurrentCone { binding: PersistentLocalBindingId,
                  witness: LocalImportWitness }
  | DirectDependency { immediate_provider: WorldConeId,
                       provider_identity: ConeIdentity,
                       route: ReexportRouteV1,
                       witness: PublicDependencyLookupWitness }
```

`ImportedTarget`继续是type/function/property/object/typealias/variant等kind-specific closed sum。sources按persistent内容排序；`WorldConeId`只作session handle，不进入判等/wire。target先按kind-specific origin id合并，再union sources；不同kind不能cast。

### 7.3 exact/star与冲突

- exact import保存完整target group；对function/extension允许多个不同signature target；对非overloadable角色多origin为ambiguous selector；
- `as`只改变当前文件local name，local binding identity使用M23-2既有`LocalBindingRole::AliasImport`；它不改变export target、type identity或diagnostic canonical name；
- star snapshot按endpoint namespace的binding key排序。多个star contribution在star层合并；同origin去重，不同origin遵守普通overload/歧义规则；
- 两条exact import给同local name带来不兼容non-overloadable target时在import phase诊断；exact与star同名仍按候选层而非header顺序处理；
- inaccessible declaration只进入diagnostic shadow set，不进入applicable candidate group，也不让高层遮蔽低层。

### 7.4 candidate顺序

无显式receiver：

```text
lexical/local
  -> implicit this member
  -> exact import
  -> current package
  -> star import
  -> core prelude
  -> contextual enum variant
```

显式receiver：真实member先于extension；extension scope为exact import → current package → star import → core prelude。每层按M16/M18执行shape filter、candidate-local applicability、首个含适用候选层与MSC。foreign target先以普通source interface参与；只有candidate准备成为concrete winner时才执行第10章stage capability检查。因此“不支持的foreign candidate”会形成准确的applicability failure，却不会无条件遮蔽较低层合法candidate。

### 7.5 lookup observation

每次query产生：

```text
LookupObservationV1 {
    site: PersistentSourceContextId,
    namespace: BindingNamespace,
    name: CanonicalIdentifier,
    receiver_shape: None | CanonicalReceiverQuery,
    layers: NonEmpty<LookupLayerObservationV1>,
}

LookupLayerObservationV1 {
    layer: LookupLayer,
    snapshot: BindingGroupSnapshotFingerprint,
    outcome: Empty | Inaccessible | ShapeRejected
           | ApplicabilityRejected | MscRejected | Winner,
}
```

snapshot覆盖该层观察到的binding id、target origin、完整source route集合和source-interface fingerprint，不只记录winner。observation只存在于`CrossConeUseSet`和golden/debug dump；M23-5 cache仍使用全direct conservative edge，不能据此漏掉未被本次实现完整证明的查询维度。

## 8. visibility与access provenance

### 8.1 current Cone

current declaration继续使用M21的`DeclaredVisibility`、`EffectiveLookupDomain`、`SlotContractDomain`和kind-specific witness：

- `internal`精确为当前Cone；同Cone不同package仍可在正确import/qualified路径下访问；
- top-level private精确为`SourceIdentity`；member private为lexical owner；
- omission已经在HIR入口正规化为internal；
- same Cone不等于忽略package/import。

### 8.2 dependency Cone

foreign ordinary lookup没有“重新计算access domain”的自由。reader只把满足以下条件的record放入`DirectPublicBindingIndex`：

- terminal declaration显式public；
- 全部owner effective lookup domain为public；
- binding是terminal direct declaration或每跳都已验证的public re-export；
- target kind/role与binding key一致；
- signature exposure/reference closure完整。

成功返回`PublicDependencyLookupWitness { terminal declaration, route, public-domain proof }`。HIR candidate、property read/write、default ref和re-export各自保存适合用途的refinement，不能把“有lookup witness”转成override/default/constructor proof。

foreign internal/private声明没有普通lookup入口。若恶意section为它构造binding，surface validation在world commit前失败；若它只存在于provider object或未来hidden support，不影响普通lookup。`protected` record不进入本阶段general interface；合法subclass context与receiver restriction由M23-6新section一次性开放。

### 8.3 member与setter

public nominal的public member source interface可以被枚举并参与诊断，但具体member use在M23-6前统一拒绝。property setter narrowing仍保持M21规则：先选中logical property，再验证setter witness；不可写不会改选较低层extension property。

## 9. default与typealias消费

### 9.1 default winner流程

对foreign callable：

1. 从provider source interface读取parameter names/calling shape；
2. 运行M17 source-argument mapping、constraint与MSC；
3. candidate若需要default，先验证模板在当前concrete arguments下可替换且其operations满足本阶段能力；
4. 只有winner commit后实例化模板；
5. definition origin保留provider节点，evaluation origin取当前call；
6. 输出完整ordered actual argument list，default/template/import信息不进入LocalConcrete以后。

re-export route只证明当前源码可选择callable；default body内部ref继续使用定义方已绑定的terminal identity，不沿调用方route重查。provider改变default target/body会改变其HIR fingerprint并经dependency edge使consumer cache miss。

### 9.2 alias展开

alias lookup返回`ImportedTypeAliasRef`而不是直接返回target type。所有type入口调用统一expander：type annotation、qualified constructor、static nested qualifier、expected type、cast/pattern、overload signature比较。expander完成后：

- type identity、layout request、RTTI与mangling只见最终target；
- source diagnostic可显示alias chain；
- re-export alias仍由原alias declaration拥有identity；
- alias target若是foreign nominal，名称/签名位置可保留；一旦进入需要layout/ABI的concrete use，走M23-6 gate；
- alias target变化进入HIR section与fingerprint，import local alias变化只影响当前source binding和实际解析结果。

## 10. stage capability gate

### 10.1 gate位置

每个candidate经历：

```text
name/access collection
  -> source-shape applicability
  -> type inference / MSC
  -> CrossConeCapabilityCheck
  -> winner commit / default instantiation
  -> LocalConcreteHir
```

gate必须在winner commit和任何local persistent materialization前完成。失败candidate保留结构化reason供resolver继续较低层；若最终无winner，主诊断选择普通type/applicability错误或最接近的capability error，排序规则固定，不把后端错误冒充源码诊断。

### 10.2 能力分类

```text
CrossConeCapability =
    SurfaceOnly
  | InlineCoreClosedConst
  | ParamFreeCoreClosedCallable
  | RequiresLayoutAbi
  | RequiresDispatchOrProtected
  | RequiresGenericOdr
  | RequiresNativeClosure
```

- `SurfaceOnly`只允许import、qualified namespace、re-export、alias surface和diagnostic inspection；
- `InlineCoreClosedConst`按5.5内联；
- `ParamFreeCoreClosedCallable`按1.5进入lowering；
- 后四种分别产生M23-6、M23-6、M23-7、M23-10稳定诊断，且没有fallback到local Strong、opaque pointer、erased generic或native string symbol。

### 10.3 禁止的降级

- consumer不得为foreign source nominal补发layout/TD/scan或把它标成current-owned Strong；
- 不得因class reference“看起来就是一个pointer”绕过typed ABI；
- 不得把direct call改写成native extern或按mangled symbol直接调用；
- 不得把generic target按first-use concrete type强制实例化为Strong；
- 不得用re-exporting Cone生成forwarder规避terminal provider proof；
- 不得因default template已经typed就跳过其concrete operation capability检查；
- 不得让unsupported高层candidate硬遮蔽低层适用candidate。

## 11. `SemanticWorld`与HIR输出

### 11.1 read-only world

`CurrentConeSemanticWorld`由imported world加current declaration overlay组成。world构造完成后只读；candidate probing使用cloneable selection plan，只有winner commit产生branded ref：

```text
ImportedSelectionPlan
  -> SelectedImportedHirSet<'world>
  -> LocalConcrete external arenas + CrossConeUseSet
```

不同world/selection的ref在类型上不可混用；不能保存borrowed artifact view越过reopen closure。selected sidecar拥有重开所需的artifact identity/fingerprint certificate。

### 11.2 LocalConcrete形状

新增封闭target：

```text
ConcreteCallableTarget =
    Local(LocalConcreteCallableId)
  | CoreExternal(ImportedCoreCallableUseId)
  | DependencyExternal(ImportedDependencyCallableUseId)
```

`DependencyExternal`只由`ParamFreeCoreClosedCallable`builder构造，非可选地保存terminal provider、strong owner、exact signature、GC root plan与route proof sidecar。表达式仍有完整concrete type、ordered arguments和definition/evaluation origin；没有name、default id、route text或candidate set。

const读取在LocalConcrete中已经是ordinary literal/constant materialization，不保留external property/storage id。typealias完全消失为target type，但`CrossConeUseSet`保留alias semantic edge供dump/fingerprint验证。

### 11.3 `CrossConeUseSet`

```text
CrossConeUseSet {
    lookup_observations: LookupObservationSet,
    selected_external: SelectedExternalSet,
}

SelectedExternalSet {
    hir: CanonicalSet<SelectedExternalHirRef>,
    mir: CanonicalSet<SelectedExternalMirRequest>,
    lir: CanonicalSet<SelectedExternalLirRequest>,
}
```

HIR set覆盖re-export/signature/default/alias/const/concrete winner；MIR/LIR set只包含实际进入相应stage的callable。driver不能扫描`LocalConcreteHir`名字或symbol重建set，也不能把全部可见surface传给后续stage。

## 12. MIR param-free bridge

### 12.1 payload

`org.scoop-lang.mir/cross-cone-param-free-bridge/1`：

```text
CrossConeMirBridgeSectionV1 {
    exports: CanonicalVec<ParamFreeMirCallableExportV1>, // field 1
    selected: CanonicalVec<SelectedDependencyMirCallableV1>, // field 2
}

ParamFreeMirCallableExportV1 {
    declaration: DependencyCallableDeclarationId, // field 1
    implementation: StrongCallableDefinitionOwner, // field 2
    signature: ExactCallableSignature,              // field 3
}

SelectedDependencyMirCallableV1 {
    provider: ConeIdentity,                         // field 1
    declaration: DependencyCallableDeclarationId,  // field 2
    implementation: StrongCallableDefinitionOwner, // field 3
    signature: ExactCallableSignature,              // field 4
}
```

ordinary Cone的`exports`精确等于本Cone public interface中满足1.5前3项和exact signature classifier的最大集合；re-export target不复制export bridge。core的此表为空并继续使用既有core bridge，避免同一authority两份真源。

`selected`精确等于LocalConcrete `DependencyExternal` use去重集合。closure validator在terminal provider的`exports`中逐项匹配；provider必须是support closure成员且HIR selected route可达。signature/implementation任一不一致即artifact invalid，不从body symbol反推。

### 12.2 MIR lowering

MIR call target新增：

```text
MirCallee::DependencyStrong(ImportedDependencyMirCallableId)
```

它携带effect-refined call semantics，不是native call。Managed调用保持普通managed safepoint/exception edge，NoGc保持NoGc；调用方参数和返回已经是exact core-closed值。MIR不读取export binding、default template或alias。

## 13. LIR semantic bridge与Link closure

### 13.1 Compile-facing LIR bridge

`org.scoop-lang.lir/cross-cone-param-free-bridge/1`：

```text
CrossConeLirBridgeSectionV1 {
    exports: CanonicalVec<ParamFreeLirCallableExportV1>, // field 1
    selected: CanonicalVec<SelectedDependencyLirCallableV1>, // field 2
}

ParamFreeLirCallableExportV1 {
    declaration: DependencyCallableDeclarationId, // field 1
    target: StrongCallableDefinitionOwner,         // field 2
    abi_signature: CanonicalScoopAbiFunctionSignature, // field 3
    expected_symbol: PersistentSymbolRequest,      // field 4
    calling_convention: CallingConvention,         // field 5
    root_plan: ManagedStatepoint | NoGc,            // field 6
    required_definition: ObjectDefinitionPlanId,   // field 7
}

SelectedDependencyLirCallableV1 {
    provider: ConeIdentity,                         // field 1
    bridge: ParamFreeLirCallableExportV1,           // field 2
}
```

provider exports从同artifact MIR export、LIR foundation/strong production机械重建；expected symbol和required definition由M23-2 persistent rules派生，不接受producer自由字符串。selected表逐项匹配provider export并与MIR selected exact signature重放出的canonical ABI相等。

LIR module使用独立`DependencyExternalCallableId` arena。codegen只为其生成external declaration和typed relocation intent；它不能出现在local definition、registration、image plan或Strong owner集合中。

### 13.2 Link-only physical closure

`org.scoop-lang.lir/cross-cone-link-closure/1`：

```text
CrossConeLinkClosureSectionV1 {
    semantic_imports: CanonicalVec<CrossConeLinkSemanticImportV1>, // field 1
    requirements: CanonicalVec<CrossConeUndefinedRequirementV1>,  // field 2
    object_coverage: CrossConeObjectCoverageProofV1,               // field 3
}

CrossConeLinkSemanticImportV1 {
    provider: ConeIdentity,                    // field 1
    target: StrongCallableDefinitionOwner,     // field 2
    abi_signature: CanonicalScoopAbiFunctionSignature, // field 3
    expected_symbol: PersistentSymbolRequest,  // field 4
    required_definition: ObjectDefinitionPlanId, // field 5
}

CrossConeUndefinedRequirementV1 {
    use_site: CanonicalUndefinedRelocationUseV1, // field 1
    import_index: u32,                            // field 2
}
```

`semantic_imports`逐byte等于Compile-facing selected bridge去掉declaration/calling decorator后的projection，并进入Code contribution。`requirements`按物理use key `(member, containing atom, offset, target slot)`严格递增；index必须命中semantic import。`object_coverage`从全部verified object重算cross-Cone relocation use集合及digest，只作LinkValidationOnly。

Link reader验证：

- 每个use site实际是对应expected symbol的undefined relocation；
- containing atom属于当前Cone已验证definition；
- use不出现在旧`link-identity-closure/1`或另一cross-Cone requirement；
- 全部`DependencyExternalCallable` relocation恰覆盖一次，其他relocation不能混入；
- consumer defined-symbol set不定义该expected symbol；
- closure级终端provider的defined-symbol owner与required definition、callable body和symbol逐项相等且为Strong；
- provider不等于consumer，且由HIR route可达；
- provider artifact不是executable/single-file dependency。

### 13.3 fingerprint

- HIR/MIR/LIR semantic bridge分别进入原layer Merkle contribution；
- Link closure的`semantic_imports`进入Code sink，physical fields只作LinkValidationOnly；
- provider body内容变化由provider自身LIR/code fingerprint覆盖；consumer最终link key总会包含provider code fingerprint；
- current consumer不复制provider `ObjectDefinitionFingerprint`作为自己的semantic definition；
- object重新分片只改变physical closure/member/artifact/code按既有规则应变部分，不反向改变LIR semantic bridge；
- re-export route变化、default、const、alias target、public binding snapshot与source-interface变化都改变HIR fingerprint。

## 14. fingerprint、cache与graph衔接

### 14.1 conservative support edge

M23-5固定：

```text
HIR support edge = every direct dependency HIR fingerprint
MIR support edge = every direct dependency MIR fingerprint
LIR support edge = every direct dependency LIR fingerprint
```

role继续使用M23-2现有`all-direct/1`。由于每个provider自己的layer fingerprint递归包含其direct edge，`C -> A(re-export B)`中B的相关变化会经A传播给C。实现可以额外验证精确selected route，但不能从fingerprint输入删除all-direct edge；未来裁剪必须换cache input schema/profile并证明observation完备。

### 14.2 M23-4 cache

`ConeCompileCacheKeyV1`字段形状不变，但profile id/fingerprint、compiler build identity和dependency三层fingerprint自然使旧entry miss。cache receipt必须绑定新profile。以下变化至少使当前Cone miss：

- direct provider新增/删除同名candidate，即使旧winner不变；
- star namespace snapshot改变；
- re-export route集合改变；
- default body/target、const value、alias target改变；
- public/internal/private改变导致binding surface改变；
- provider MIR/LIR callable bridge、GC effect或ABI relation改变。

仅diagnostic attachment或host path变化不使semantic cache miss；provider只有object bytes变化而semantic metadata不变时至少使最终link失效。M23-5可以继续保守使dependent compile miss，但不能错误命中。

### 14.3 scheduler变化

M23-4 scheduler只改三点：

1. planned/actual profile期望改为`cross-cone-semantics-strong/1`；
2. 删除`SCOOPC_CAPABILITY_NON_CORE_DEPENDENCY_UNAVAILABLE`作为普通dependency的预期终点；
3. child success后除原双view/graph match外，增加cross-Cone semantic closure与physical requirement closure验证。

locator、canonical order、snapshot、cache lock、child argv/protocol、failure propagation和atomic publish顺序不变。

## 15. 诊断与失败原子性

### 15.1 source diagnostic family

至少冻结以下code family：

| code | 语义 |
| --- | --- |
| `SCOOP_HIR_IMPORT_TARGET_UNREACHABLE` | selector不在current/direct公开表面 |
| `SCOOP_HIR_IMPORT_TARGET_AMBIGUOUS` | exact selector命中不同origin且不能形成合法overload group |
| `SCOOP_HIR_IMPORT_ENDPOINT_KIND` | exact/star终点不是所需binding/namespace |
| `SCOOP_HIR_PUBLIC_IMPORT_SOURCE` | public import含CurrentCone、support-only或非public source |
| `SCOOP_HIR_REEXPORT_CONFLICT` | destination public surface冲突 |
| `SCOOP_HIR_DEPENDENCY_ACCESS` | foreign declaration/member不可访问 |
| `SCOOP_HIR_ALIAS_CYCLE` | alias展开cycle或非法target |
| `SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED` | 需要M23-6 layout/ABI/materialization |
| `SCOOP_HIR_CROSS_CONE_DISPATCH_REQUIRED` | 需要M23-6 member/dispatch/protected capability |
| `SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED` | 需要M23-7 generic/ODR能力 |
| `SCOOP_HIR_CROSS_CONE_NATIVE_REQUIRED` | 需要M23-10 native closure |

Stage1已有的syntax/current-unit code保持含义；同一源码在M23-5因新增合法direct target成功时不再触发旧external-unavailable诊断。artifact损坏、route伪造、profile不匹配或bridge/object矛盾属于reader/build错误，不伪装成source diagnostic。

### 15.2 锚点与排序

- import错误锚定完整selector；可附最长成功package prefix、candidate coordinate和route；
- public star失败主诊断锚定star selector，notes按target binding key排序；
- visibility/member错误锚定use site并指向terminal declaration definition source；
- capability错误锚定准备commit的具体call/type/member use，说明需要的后续capability，不建议按symbol或native方式绕过；
- 多candidate诊断按layer、namespace/role、persistent origin id、provider coordinate、route bytes排序；不按artifact path或加载顺序；
- untrusted artifact text只作escaped decorator，不作为format string或修复命令。

### 15.3 原子性

```text
dependency closure failure -> parser未启动
semantic world failure     -> current HIR未启动
import/re-export failure   -> 无re-export/public surface commit
HIR capability failure     -> 无LocalConcrete/MIR/LIR
MIR/LIR bridge failure     -> 无object
Link closure failure       -> 无package/cache publish/completed node
```

candidate-local失败可在resolver内部回滚selection plan并继续下一candidate；这不是partial stage output。只有整个HIR成功后才返回`CompleteHirOutput`。

## 16. 测试设计

### 16.1 profile与wire

- 新profile descriptor canonical bytes/fingerprint fixed vector；required section缺失、重复、错location/purpose/sink；
- 旧profile在M23-5 build稳定拒绝，新profile空section的core-only artifact可重复；
- 四个payload的field/tag/order golden、unknown/duplicate/missing field与resource limit；
- re-export identity仍由既有field16/key重算，route变化不改变binding id但改变HIR fingerprint；
- malicious external raw id无provider authority不能满足identity resolution；
- Compile/Link bridge projection mismatch、旧/new undefined-use partition overlap或gap全部失败。

### 16.2 closure与authority

- direct A可见，support B不可见；A re-export B后B可见；移除route后同一support artifact仍不可见；
- chain、diamond、多条route去重与删除单一路径；
- support set缺失/额外/stale、same identity different fingerprint、profile/target mismatch；
- atomic identity import：最后一个artifact损坏时session/world为空；
- implicit core既是direct又有trusted marker，ordinary artifact不能伪造prelude/intrinsic authority。

### 16.3 exact/star/alias与split package

- current/direct split package、两个direct provider贡献同package；
- longest package prefix与static nested owner同名；
- exact function overload group、non-overloadable ambiguity、same signature different origin；
- repeated exact、multiple star、exact+star、`as`重命名与type/value namespace；
- transitive未re-export、qualified FQN文本存在但不可达；
- inaccessible高层candidate不遮蔽低层，全部不适用exact candidate继续star/package层；
- manifest/dependency/artifact枚举顺序变化不改变结果或diagnostic。

### 16.4 public import

- exact、alias、star、current package destination与root package；
- CurrentCone target、internal/private/protected/support-only target稳定失败；
- public star某一非法target使整条statement失败，无partial binding；
- local declaration/re-export conflict、overload merge、diamond同origin route union；
- re-export type/function/property/object/typealias/enum variant identity保持terminal origin；
- downstream不重新执行上游star，新target只在重编译上游后进入snapshot。

### 16.5 visibility、default与alias

- same-Cone internal跨package、top-level private跨file、member private；
- dependency public成功，internal/private不可枚举，malicious re-export拒绝；
- public member可见但具体use触发M23-6；protected receiver触发独立dispatch/protected诊断；
- imported callable named/default mapping、winner-only实例化、definition/evaluation origin、default引用另一个public dependency callable；
- default直接引用internal/private、缺refined witness或body/reference closure不等的corruption negative；
- alias import/re-export/chain、target change invalidation、transparent type equality、跨Conecycle/budget；
- `as` alias与typealias identity严格区分。

### 16.6 capability矩阵

positive：

- imported/re-exported declarations完全unused；
- public alias facade指向dependency type；
- core-closed public const read；
- `(Int) -> Int`、`() -> Unit` ordinary top-level call；
- core-closed extension function；
- core-closed top-level/extension property getter/setter；
- managed与NoGc两种root plan；
- 通过一跳/多跳re-export调用terminal provider body；
- imported callable使用provider default。

negative：

- foreign nominal参数/返回、tuple/function/pointer/nominal application；
- constructor、member、virtual/interface call、field/place、object value、callable reference；
- generic function/type/property application；
- suspend callable；
- source extern；
- consumer为foreign exact type生成Strong layout/TD/registration或为foreign callable生成body。

每个negative断言HIR失败、无MIR/LIR/object/artifact，并匹配唯一code/span。

### 16.7 MIR/LIR/object

- provider export最大集合与HIR/MIR strong graph一致；re-export不复制export bridge；
- consumer selected集合只含实际winner，candidate probe/unused import不进入；
- terminal provider、owner、exact signature、ABI、GC effect、symbol、definition逐字段篡改；
- codegen external declaration/relocation，不出现local definition/registration/image member；
- old core requirement与new ordinary requirement物理use集合互斥且联合完整；
- dependency symbol由provider恰好一个Strong owner满足，consumer/其他Cone重复定义失败；
- Compile semantic projection与Link semantic import逐byte相等；
- object分片变化不改变LIR semantic fingerprint，错误member/range/offset使Link proof失败。

### 16.8 cache与orchestrator

- direct新增适用overload、仅新增不适用overload、star target、route、visibility、default、const、alias、MIR/LIR bridge分别变化的失效矩阵；
- diamond公共dependency只编译一次，dependent按canonical order；
- source library A先成功，root B真正通过non-core编译，不再出现M23-4阶段门；
- child output仍由parent双view+closure验证，伪造success response无效；
- 第二次build零child，profile升级后旧cache miss；
- 全部path/mtime/request id变化不改变artifact bytes。

### 16.9 regression与crate边界

- M1～M22与M23-1～M23-4全部回归；
- core-only/single-file artifact bytes在新空section/profile下更新golden；
- Cargo dependency test保证IR/stage方向不倒置、`scoop`不依赖lowerer/codegen；
- `rg`/API测试确认普通resolver无FQN/symbol/artifact-scan fallback；
- full `cargo fmt --all`、`cargo clippy --workspace`、`cargo test`。

## 17. 实现顺序

1. 先更新language/implementation spec、M23总设计与ROADMAP，冻结新profile、capability和Stage5成功矩阵；
2. 在identity/HIR meta实现re-export binding的external authority registration与route DTO，不增加新persistent entity id；
3. 实现`cross-cone-interface/1` wire、canonical validation、default/alias/const与fixed vectors；
4. 在slib实现新profile、closure-wide atomic identity/surface/route validation和purpose-preserving handles；
5. 在HIR建立`ImportedSemanticWorld`、direct/support API隔离和package/static-owner namespace；
6. 接入exact/star/alias/public import、candidate layers、visibility witness、split package与re-export conflict；
7. 实现default实例化、alias expander、lookup observation、selected HIR set与stage capability gate；
8. 实现MIR export/selected bridge及`DependencyStrong` lowering；
9. 实现LIR semantic bridge、dependency external arena、codegen relocation intent；
10. 实现Link-only closure、old/new relocation分区、object union coverage与Code contribution；
11. 更新driver preflight/publication，删除无条件non-core gate并接入selected MIR/LIR投影；
12. 更新`scoop` expected profile、cache receipt和parent closure validation，不修改locator/scheduler算法；
13. 完成wire/corruption、resolver/re-export/default/alias/capability、object/cache/process集成矩阵；
14. 格式化、lint、分crate测试、全量测试与文档交叉引用审计。

每批实现先运行`cargo fmt --all`与`cargo clippy --workspace`，再运行对应unit/golden/integration test。不能先放开resolver后留下MIR/LIR `unimplemented`，也不能先接受generic/layout target再等后续stage兜底；HIR capability gate与每一种新成功target的完整artifact proof必须同批提交。

## 18. 完成门

M23-5只有同时满足以下条件才完成：

- 所有production Cone（含core/single-file）使用`cross-cone-semantics-strong/1`，旧profile不会被静默upgrade或混入closure；
- 四条新capability的wire、registry contract、fingerprint contribution、resource budget与fixed vector完成；
- direct/support semantic closure在parser前原子验证，support artifact不能枚举名称，direct artifact不能伪造core authority；
- persistent identity按kind/origin统一intern，re-export binding复用M23-2 identity key并在terminal provider authority下验证；
- exact/star/alias、longest package prefix、static owner、split package、candidate layer和MSC全部按规范工作；
- public import exact/star snapshot、chain/diamond route、destination conflict与visibility不扩大规则完整；
- public/internal/private access provenance完备，foreign internal/private无法通过metadata/object/symbol旁路进入lookup；protected有唯一M23-6诊断；
- default template保持定义方绑定和definition origin，只在winner后实例化并使用调用点评估origin；
- non-generic alias跨Cone透明展开、保留alias identity并正确失效；
- `LookupObservationSet`覆盖空/不可访问/shape/applicability/MSC/winner，cache仍保守纳入全部direct fingerprint；
- 本阶段positive external-use只落入const或`ParamFreeCoreClosedCallableV1`，所有layout/dispatch/generic/native形态在HIR原子拒绝；
- LocalConcrete/MIR/LIR对ordinary dependency使用独立typed target，consumer不发provider Strong definition；
- provider export、consumer selected、LIR semantic、object relocation与Link physical closure逐层一一对应；旧core closure与新ordinary closure互斥且联合覆盖全部undefined use；
- chain/diamond中最终call target始终是terminal origin，re-exporting Cone不生成forwarder或第二份body；
- 每个成功artifact从最终bytes通过Compile/Link双view及closure级definition resolution，失败不发布cache/completed node；
- M23-4 locator/DAG/snapshot/cache/child协议行为不变，真实ordinary dependency source图可以完成编译并复用cache；
- 全部negative诊断有稳定code/span，artifact corruption归reader/build错误而非source错误；
- M1～M22、M23-1～M23-4回归，format、clippy和完整test通过。

到达该门后，M23-6可以直接在同一`SemanticWorld`和selected persistent target上增加layout/ABI/scan/inheritance capability；它不需要重做package/import/re-export、default或alias，也不能通过放宽本文的support枚举规则来实现dispatch。

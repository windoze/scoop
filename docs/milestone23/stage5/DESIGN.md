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

十张顶层table均按各record声明的typed primary key严格递增并拒绝重复；`definition_sources`是唯一没有声明identity主键的闭包表，按`(source ConeIdentity bytes, normalized source path UTF-8 bytes, span start, span end, context id bytes)`严格递增并拒绝重复。producer输入乱序由writer排序，reader绝不排序修复。record内部标为`CanonicalVec`/`CanonicalSet`的集合也按其元素typed key或canonical wire bytes严格递增；标为declaration/source order的序列保持源码语义顺序，以隐含的zero-based `u32` position作identity，reader不得排序。payload只保存canonical semantic interface，不直接serde `ExportHir` arena，不保存arena id、import文本、host path、failed candidate或body-local display name。

`ExportDefinitionSourceV1`是与M23-2 `DefinitionOrigin`不同的Rust语义类型，但wire逐byte复用后者已经冻结的三字段map：`1=SourceIdentity, 2=SourceSpan, 3=PersistentSourceContextId`，不增加wrapper tag或外层field。它只表示定义方位置，不能转换为`EvaluationOrigin`或consumer位置。reader必须在同artifact已验证的HIR foundation中解析source/context，要求source Cone等于当前artifact、context的source完全相同，并要求span两个端点均存在于该`SourceRecord`的canonical point table。顶层`definition_sources`精确等于fields 1～8中所有内联`ExportDefinitionSourceV1`（包括default body节点）的规范去重集合；缺项和多余项都拒绝。foundation field 29已经按typed subject保存的声明origin不因本表而删除或放宽，内联到alias/const/default根的origin必须与对应foundation subject的origin逐字段相等。

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
    kind: PublicNominalKindV1,             // field 2
    type_parameters: CanonicalBinderList, // field 3
    exact_supertypes: CanonicalVec<SignatureTypeKey>,  // field 4
    constructors: CanonicalVec<PersistentConstructorId>, // field 5
    members: CanonicalVec<PublicMemberRefV1>,            // field 6
    nested_bindings: CanonicalVec<PersistentExportBindingId>, // field 7
    source_shape: NominalSourceShapeV1,    // field 8
}

CallableInterfaceRecordV1 {
    declaration: CallableDeclarationId,   // field 1
    owner: PublicDeclarationOwnerV1,       // field 2
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
    owner: PublicDeclarationOwnerV1,       // field 2
    type_parameters: CanonicalBinderList, // field 3
    receiver: None | SignatureTypeKey,    // field 4
    value_type: SignatureTypeKey,          // field 5
    capability: PropertyCapabilityV1,     // field 6
    representation: PropertyRepresentationV1, // field 7
    access: PropertyPublicAccessV1,        // field 8
}
```

callable record的参数列表只保存声明签名本身；default/vararg omission仍由5.4的独立source interface保存：

```text
SourceParameterShapeV1 {
    name: CanonicalIdentifier,       // field 1
    value_type: SignatureTypeKey,    // field 2
}

CallableSourceEffectsV1 {
    execution: Effect,                         // field 1
    safety: CallableSafetyV1,                  // field 2
    gc_effect: GcEffect,                       // field 3
    implementation: CallableImplementationV1, // field 4
    operator_role: CallableOperatorRoleV1,     // field 5
    infix: CallableInfixV1,                    // field 6
}

CallableSafetyV1 =
    Safe    // unsigned 1
  | Unsafe  // unsigned 2

CallableImplementationV1 =
    Scoop              // unsigned 1
  | Intrinsic          // unsigned 2
  | SourceExternScoop  // unsigned 3
  | SourceExternC      // unsigned 4

CallableOperatorRoleV1 =
    None                                      // { 0: 1 }
  | Language(CallableOperatorV1)              // { 0: 2, 1: operator }
  | PropertyDelegate(PropertyDelegateOperatorV1) // { 0: 3, 1: operator }

CallableOperatorV1 =
    UnaryPlus   // { 0: 1 }
  | UnaryMinus  // { 0: 2 }
  | Not         // { 0: 3 }
  | Inc         // { 0: 4 }
  | Dec         // { 0: 5 }
  | Plus        // { 0: 6 }
  | Minus       // { 0: 7 }
  | Times       // { 0: 8 }
  | Div         // { 0: 9 }
  | Rem         // { 0: 10 }
  | RangeTo     // { 0: 11 }
  | RangeUntil  // { 0: 12 }
  | Contains    // { 0: 13 }
  | Get         // { 0: 14 }
  | Set         // { 0: 15 }
  | Invoke      // { 0: 16 }
  | PlusAssign  // { 0: 17 }
  | MinusAssign // { 0: 18 }
  | TimesAssign // { 0: 19 }
  | DivAssign   // { 0: 20 }
  | RemAssign   // { 0: 21 }
  | CompareTo   // { 0: 22 }
  | Equals      // { 0: 23 }
  | Component { index: NonZeroU32 } // { 0: 24, 1: index }
  | Iterator    // { 0: 25 }

PropertyDelegateOperatorV1 =
    ProvideDelegate // unsigned 1
  | GetValue        // unsigned 2
  | SetValue        // unsigned 3

CallableInfixV1 =
    Ordinary // unsigned 1
  | Infix    // unsigned 2

CallableModalityV1 =
    Final            // unsigned 1
  | Open             // unsigned 2
  | Abstract         // unsigned 3
  | InterfaceDefault // unsigned 4

PublicLookupAccessV1 =
    DirectOnly // unsigned 1
  | PublicSlot // unsigned 2

PropertyCapabilityV1 =
    ReadOnly { getter: PersistentPropertyAccessorId }
        // { 0: 1, 1: getter }
  | ReadWrite {
        getter: PersistentPropertyAccessorId,
        setter: PersistentPropertyAccessorId,
        setter_access: PropertySetterPublicAccessV1,
    }   // { 0: 2, 1: getter, 2: setter, 3: setter_access }

PropertySetterPublicAccessV1 =
    Restricted // unsigned 1
  | Public     // unsigned 2

PropertyRepresentationV1 =
    Const           // unsigned 1
  | RuntimeAccessor // unsigned 2
  | AbstractSlot    // unsigned 3

PropertyPublicAccessV1 =
    DirectOnly // unsigned 1
  | PublicSlot // unsigned 2
```

`Effect`与`GcEffect`逐byte复用identity wire已经冻结的`Ordinary = 1 | Suspend = 2`和`Managed = 1 | NoGc = 2`，不得另分配近义tag。`SourceParameterShapeV1.value_type`是callee实际接收的完整参数type；对vararg它是`Array<element>`，element type及`Empty | Default` omission由5.4记录并与该array application交叉验证。参数列表保持声明顺序、长度必须可表示为`u32`且name在同一callable内唯一；reader以该顺序逐项核对canonical declaration key的parameter signature，不能排序或只比较数量。

`CallableImplementationV1::Scoop`同时覆盖普通源码body、compiler生成但具有普通Scoop调用语义的derived body以及无body的abstract declaration；是否必须/禁止body由modality和定义方Export HIR交叉验证。`Intrinsic`必须命中typed intrinsic registry；两个`SourceExtern*`只保存名称解析与能力诊断所需的ABI类别，完整native contract仍来自M23-2独立typed contract，v1唯一calling convention `cdecl`不在这里重复编码。ordinary dependency的`Intrinsic`与`SourceExtern*`都不能取得本阶段的param-free executable capability。

`CallableOperatorRoleV1`把ordinary language operator与M21 property-delegate protocol保持为不相交的typed role；`None`不是缺字段。`infix`独立保存，因为它与operator role正交；它使用显式两值枚举而不是Wire CBOR v1禁止的native boolean。reader逐variant检查map长度，`Component.index`不能为0，也不能按source name恢复operator role。

`PublicLookupAccessV1`只是一种已经收窄到foreign public surface的证明，不是通用visibility枚举：两种variant都要求declaration显式public且owner effective lookup domain为universal；`PublicSlot`额外声明该callable承担public slot contract，因而可区分普通final callable与final override。`Open`、`Abstract`和`InterfaceDefault`必须使用`PublicSlot`；top-level、extension、constructor及variant constructor必须是`Final + DirectOnly`。protected/internal/private没有variant，也不能用`PublicSlot`冒充M23-6 inheritance authority。

property能力沿用M21的封闭sum；wire中不存在`is_var + Option<setter>`、独立`mutability`或用缺字段表示`val`。`ReadOnly`恰有getter；`ReadWrite`恰有getter、setter及setter的foreign public access分类。两种accessor id必须分别解析为同一property的`Getter`/`Setter` `PropertyAccessorKey`，不能按arena位置、名称或另一accessor推导。`Restricted`表示setter确实存在，但其declared/effective access没有形成foreign universal lookup witness；它不暴露private/internal/protected的具体类别，且对应setter不能进入public callable interface table。`Public`要求setter显式public、effective lookup domain为universal，并要求同id的callable interface record存在。getter visibility恒等于property visibility，因此每个property的getter都必须有对应public callable interface record。

`PropertyPublicAccessV1`是逻辑property及其getter的typed public proof；`DirectOnly`与`PublicSlot`的含义逐项对应`PublicLookupAccessV1`，但两者类型不可互换。top-level与extension property只能是`DirectOnly`；nominal property承担public slot contract时必须是`PublicSlot`，包括final public override。`ReadWrite + Public`的setter继承同一slot分类；`ReadWrite + Restricted`没有foreign setter slot/lookup witness。reader必须把property access与getter callable access逐项交叉验证，并在public setter存在时同样核对setter callable access。

`PropertyRepresentationV1`是cross-Cone访问类别，不复制provider storage布局。`Const`要求`ReadOnly + DirectOnly`、无extension receiver及own binder，并由同property的`ExportConstValueV1`给出typed值；其合法owner仍精确为M21允许的top-level或object。`AbstractSlot`要求nominal owner、`PublicSlot`，且全部required accessor implementation均为abstract；其中每个public accessor的callable record必须为`Abstract`，`Restricted` setter则只由定义方typed authority核对，不为它伪造public callable record。其余非const形态统一为`RuntimeAccessor`：stored、accessor-only、delegated与native storage都不向consumer泄漏field/global/delegate identity；interface中default/abstract混合也属于`RuntimeAccessor`，每个public accessor究竟是`Final`、`Open`、`Abstract`还是`InterfaceDefault`由对应callable record表达。`RuntimeAccessor`不得伪装一个全部accessor implementation均为abstract的property。native storage仍由typed native contract/definition source识别，并按1.4拒绝取得M23-5 executable bridge，不能因折叠为`RuntimeAccessor`绕过M23-10。

callable closure validator必须从kind-specific canonical identity及已验证的相邻record重放一个非wire的`CallableDeclarationIdentityShapeV1 { owner, own_type_parameter_arity, outer_type_parameter_arity, receiver, parameter_types }`，再与callable record逐项比较。普通/generic function与constructor的source declaration key直接给出receiver、own arity和parameter type sequence；property accessor由`PropertyAccessorKey`、对应property record及getter/setter role给出owner、receiver和参数；variant constructor由source `EnumVariantIdentityKey`及对应nominal source-shape variant给出owner和declaration-order payload参数。任一来源缺失、kind不匹配或同一事实不一致都失败，不能按FQN、源码名称或table位置补猜。function result及binder name/bound不属于duplicate-declaration identity，仍以本record为canonical interface并进入HIR fingerprint；reader必须验证其闭合性，但不得把它们错误加入persistent declaration id。

signature scope固定由`own_type_parameter_arity`和`outer_type_parameter_arity`两层压缩规则构造：own arity非零时own binder位于depth 0，outer arity非零时位于其后的depth；任一arity为零都不创建空frame。普通nominal member的outer frame来自nominal owner；constructor和variant constructor只有该outer frame；property accessor不声明own binder，其outer frame来自所属property的可见binder namespace——generic extension property使用property binder，generic nominal member property使用nominal owner binder。top-level/extension function只有自己的binder frame。validator以该封闭scope递归验证binder bound、receiver、全部parameter与result中的每个`SignatureTypeKey`，同时重放所有nominal reference kind与application arity。constructor、variant constructor与property accessor的own arity必须为零；extension必须恰有receiver且非extension不得伪造receiver。modality、implementation、slot access、operator/infix legality及source interface长度必须与定义方typed Export HIR逐项一致。

property closure validator同样必须从canonical ordinary/extension property identity重放非wire的`PropertyDeclarationIdentityShapeV1 { owner, own_type_parameter_arity, outer_type_parameter_arity, receiver }`。ordinary top-level/member property的own arity恒为0且没有receiver；generic extension property的own arity与receiver来自其source declaration key；nominal member的outer arity只来自已验证nominal interface，不能复制进property binder list。validator用相同的压缩scope验证binder bound、receiver与`value_type`，再验证capability中的每个accessor key。

closure随后以property record为唯一签名来源交叉验证public accessor callable：getter没有value parameter且result exact等于`value_type`；public setter恰有一个`value_type`参数且result是canonical core `Unit`；两者own binder arity恒为0，outer frame取property own binder或nominal owner binder。所有property accessor都必须ordinary/non-infix且没有language/delegate operator role。representation、property/accessor access、modality、safety、GC effect、implementation与定义方typed Export HIR必须逐项一致；`Restricted` setter仍必须由authority证明owner、role、access与implementation，只是不要求或允许其出现在public callable interface。缺少required public accessor record、为`Restricted` setter伪造public callable record、role/owner/signature不一致或const/abstract分类不一致都在world commit前拒绝。

其中两个公共闭合类型的wire固定为：

```text
PublicNominalKindV1 =
    Class      // unsigned 1
  | Interface  // unsigned 2
  | Struct     // unsigned 3
  | Enum       // unsigned 4
  | Object     // unsigned 5

PublicDeclarationOwnerV1 =
    TopLevel                         // { 0: 1 }
  | Nominal(SourceNominalId)         // { 0: 2, 1: owner }
  | Extension                        // { 0: 3 }
```

`PublicNominalKindV1`逐值复用`SourceDeclarationKind`中五种当前可声明nominal的tag；M23-1已排除尚未开放的自定义`annotation class`，因此它没有可伪造的第六种public surface variant。`PublicDeclarationOwnerV1::Nominal`必须解析为同一closure中存在的nominal interface；reader按sum tag精确检查map长度，不能用缺失owner id猜测`TopLevel`或`Extension`。closure validator必须进一步证明：nominal record的kind与其canonical source declaration key完全一致，callable/property的owner variant及nominal id与canonical declaration owner chain一致；constructor只能是`Nominal`，extension declaration只能是`Extension`，其余无owner声明只能是`TopLevel`。

nominal的源码形状是与kind一一对应的封闭sum：

```text
NominalSourceShapeV1 =
    Class                                      // { 0: 1 }
  | Interface                                  // { 0: 2 }
  | Struct { fields: [StructSourceFieldV1] }   // { 0: 3, 1: fields }
  | Enum { variants: [EnumSourceVariantV1] }   // { 0: 4, 1: variants }
  | Object { value: PersistentObjectValueId }  // { 0: 5, 1: value }

StructSourceFieldV1 {
    field: PersistentFieldId,       // field 1
    value_type: SignatureTypeKey,   // field 2
}

EnumSourceVariantV1 {
    variant: PersistentEnumVariantId, // field 1
    style: EnumSourceVariantStyleV1,  // field 2
    fields: [EnumSourceFieldV1],      // field 3
}

EnumSourceVariantStyleV1 =
    Unit         // unsigned 1
  | Positional   // unsigned 2
  | Named        // unsigned 3
  | Constructor  // unsigned 4

EnumSourceFieldV1 {
    field: PersistentEnumVariantFieldId, // field 1
    value_type: SignatureTypeKey,        // field 2
}
```

`NominalSourceShapeV1`的tag逐值等于`PublicNominalKindV1`；nominal record constructor和reader都必须拒绝`kind/source_shape`不一致。field、variant与variant-field数组保留源码声明顺序，长度必须可表示为`u32`，同一直接owner内的typed id不得重复；`Unit` variant的fields必须为空。identity foundation中相应canonical key必须证明struct field owner、enum variant owner，以及variant field owner/selector；positional selector的`declaration_index`必须等于数组位置，named/constructor selector必须保持其canonical source name。每个field的`value_type`使用该nominal自己的binder list作为唯一depth 0 scope。`Object.value`必须是由同一object source declaration key产生的`PersistentObjectValueId`；class/interface的源码形状没有nullable payload，constructor与public member分别由record的field 5、6关联。

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

前三个sum分别逐byte等同既有`NominalDeclarationOwner`、`CallableTemplateOrigin`和`PropertyOwner`编码，reader复用其decoded type与canonical identity resolver；不得重新分配tag，也不得把它们退化成`{ kind: integer, digest: bytes }`。`PublicMemberRefV1`是新增的外层kind sum。source constructor在nominal的`constructors`列出并另有callable/source interface，enum variant在`source_shape`列出且其payload constructor另有callable/source interface，nested nominal/value通过`nested_bindings`授权；这些实体都不得再重复出现在`members`。

四类table主键与内部集合顺序固定为：

- nominal table按`SourceNominalId`的`(tag, raw id bytes)`，callable table按`CallableDeclarationId`，property table按`PropertyDeclarationId`严格递增；
- `exact_supertypes`与binder interface bounds都按完整`SignatureTypeKey`结构序排序去重：先比较variant wire tag，再按该variant的wire field顺序递归比较；id比较raw bytes，数字/leaf enum比较wire数值，序列作逐元素字典序比较且相同前缀下较短者在前。`constructors`与`nested_bindings`按raw id bytes排序去重；`members`按`PublicMemberRefV1`的`(tag, nested tag, raw id bytes)`排序去重；
- binder、callable parameter、struct field、enum variant及variant field是declaration-order序列，不按名称或类型排序；其数量和position必须可表示为`u32`，同一owner内的source name遵守各自语言重复声明规则；
- 每个member/constructor/nested binding必须由当前nominal直接拥有；每个callable/property的`owner`必须与其canonical declaration key及nominal record一致。consumer不能扫描FQN、其他table或arena ordinal猜测owner。

`exact_supertypes`中的每一项必须以`Nominal`或`NominalApplication`为根，不能用tuple、function、pointer或裸binder冒充父类型；generic application必须覆盖目标声明的全部参数。其内部类型实参只使用当前nominal binder list作为唯一depth 0 scope，并递归通过同一closure中的nominal interface校验。目标kind只能是class或interface：class/object至多列出一个class，其余为interface；interface/struct/enum只能列出interface。reader必须重放这些规则，不能把“typed id存在”当成合法继承证明；继承环和dispatch/slot完备性仍由M23-6的required inheritance capability闭合。

`CanonicalBinderList`不是集合，而是以下declaration-order product：

```text
TypeParameterBinderV1 {
    name: CanonicalIdentifier,             // field 1
    bounds: TypeParameterBoundsV1,         // field 2
}

TypeParameterBoundsV1 =
    Unconstrained                          // tag 1
  | Value                                  // tag 2
  | Ref                                    // tag 3
  | Nominal {                              // tag 4
        class: None | SignatureTypeKey,    // field 1
        interfaces: CanonicalVec<SignatureTypeKey>, // field 2
    }
```

`Nominal`必须至少包含class或一个interface；interface bounds按完整`SignatureTypeKey`的结构序严格递增并拒绝重复，class最多一个。closure validator根据被引用nominal interface重放class/interface kind、reference kind与完整application约束；reader不能仅因它能解析为一个type id就接受。`Value`/`Ref`与`Nominal`结构互斥，因此wire不能表达两类bound混合。

binder list长度必须可表示为`u32`，name必须canonical且在同一list唯一，position就是binder index。nominal自身的签名和bound以该list为depth 0；top-level/extension callable或property自己的binder也为depth 0。nominal member若没有自己的binder，owner binder为depth 0；若有自己的binder，则own binder为depth 0、owner binder为depth 1。callable record只保存callable自己声明的binder，property record只保存extension property自己声明的binder，不复制owner nominal binder；constructor、variant constructor和property accessor不声明独立binder。后三类callable仍按上一段从其nominal或property声明取得唯一outer frame，绝不复制binder定义或制造空depth。所有`SignatureTypeKey::Binder { depth, index }`必须在上述封闭scope stack内，reader对越界depth/index硬失败。

`SourceNominalId`、`CallableDeclarationId`、`PropertyDeclarationId`和`PublicMemberRefV1`因此都是kind-specific closed sum，不能用裸digest union。struct/enum的declaration-order fields/variants、class constructor source shape、object type/value relation、operator/infix标志、property accessor effect及annotation的语义部分通过各自closed constituent进入上述record；它们只支撑HIR lookup/type checking，不授权layout或dispatch。

只有public lookup surface中的声明写入这些表。public owner所需的protected inheritance/slot内容不塞入nullable字段；M23-6以独立required capability加入。generic declaration可在这里提供name、binder、bound和source signature，但没有template body/hidden support；任何application由M23-7 gate拒绝。

### 5.4 source interface与default

source-call protocol使用以下canonical语义结构；parameter position是`parameters`中隐含的zero-based `u32`下标，不再写一份可能不一致的position field：

```text
CallableSourceInterfaceV1 {
    owner: CallableDeclarationId,                          // field 1
    parameters: SourceOrderVec<CallableSourceParameterV1>, // field 2
}

CallableSourceParameterV1 {
    name: CanonicalIdentifier,                    // field 1
    value_type: SignatureTypeKey,                 // field 2
    calling: CallableParameterCallingV1,          // field 3
    definition_origin: ExportDefinitionSourceV1,  // field 4
}

CallableParameterCallingV1 =
    Required                                         // tag 1
  | Default { template: ExportDefaultTemplateKeyV1 } // tag 2
  | VarargEmpty { element_type: SignatureTypeKey }   // tag 3
  | VarargDefault {                                  // tag 4
        element_type: SignatureTypeKey,
        template: ExportDefaultTemplateKeyV1,
    }

ExportDefaultTemplateKeyV1 {
    owner: CallableDeclarationId, // field 1
    parameter_position: u32,      // field 2
}
```

wire中`CallableParameterCallingV1`使用精确map：`Required={0:1}`、`Default={0:2,1:template_index}`、`VarargEmpty={0:3,1:element_type}`、`VarargDefault={0:4,1:element_type,2:template_index}`。`template_index`是canonical `default_templates`表的zero-based unsigned `u32`下标，不是任何compiler arena id。decoded wire type在顶层section尚未闭合前暂存该index；只有当index可表示为`u32`、在表内，且目标record的key精确等于`(source owner, parameter position)`时，才解析为canonical语义类型中的`ExportDefaultTemplateKeyV1`。writer反向以key查canonical table index；不允许将raw index混入已解析HIR。

`source_interfaces`按owner的`(tag, raw id bytes)`严格递增。当前artifact的public `Function`、`GenericFunction`、`Constructor`和`EnumVariantConstructor`每个都必须恰有一条record，即使无参也保留空`parameters`；`PropertyAccessor`不使用源码call-argument protocol，因此不得出现。每个parameter的name与value type必须按位置逐项等于同owner的`CallableInterfaceRecordV1.parameters`，列表长度必须可表示为`u32`；这一对照同时锁定参数名唯一性和binder scope，consumer不得用名字重新建立position。

一个parameter list最多只能有一个`VarargEmpty`/`VarargDefault`。vararg的`value_type`必须由trusted core type authority证明为精确`Array<element_type>`application，不能按显示名或用户同形nominal猜测；普通`Required`/`Default`的`value_type`就是callee实际参数类型。只有`Default`和`VarargDefault`可以引用template，且`default_templates`必须与这些引用形成双向精确闭包：每个引用恰命中一条同key record，每条template record也恰被对应owner/position引用，不接受orphan或共享不同key的index。override继承的default以当前override owner/position建立新key并保存已实例化的type-argument mapping；template内的definition root、path和origin仍指向真实provider，不复制源码也不冒充当前override定义。

parameter的`definition_origin`是当前artifact的定义方位置，必须通过与其他inline origin相同的source/context/point校验，并进入顶层`definition_sources`精确闭包。它不是调用方evaluation origin。

`ExportDefaultTemplateV1`保存：

```text
ExportDefaultTemplateV1 {
    key: ExportDefaultTemplateKeyV1,      // field 1，owner + parameter position
    definition_root: PersistentLexicalRootV1, // field 2
    definition_path: StructuralDefinitionPath, // field 3
    locals: CanonicalTemplateLocalTableV1,      // field 4
    body: ExportDefaultBodyV1,                  // field 5
    result: SignatureTypeKey,                   // field 6
    allows_suspend: CanonicalBooleanV1,         // field 7
    type_parameters: CanonicalBinderUseListV1,  // field 8
    receiver: OptionalTemplateReceiverV1,       // field 9
    value_parameters: CanonicalTemplateValueParametersV1, // field 10
    references: ExportDefaultReferenceSetV1,    // field 11
    definition_origin: ExportDefinitionSourceV1, // field 12
}
```

`default_templates`按`ExportDefaultTemplateKeyV1`的`(owner tag, owner raw id bytes, parameter position)`严格递增并拒绝重复。`ExportDefaultTemplateKeyV1`只是本section内排序和引用的canonical key，不产生新的persistent id，也不进入M23-2 identity foundation。wire内对template的引用使用上述经范围与key验证的table index；跨artifact稳定语义由owner persistent identity、parameter position与完整template payload共同确定。

template envelope使用以下封闭结构：

```text
PersistentLexicalRootV1 =
    Function { declaration: PersistentFunctionId }                    // tag 1
  | GenericFunction { declaration: PersistentGenericFunctionId }      // tag 2
  | Constructor { declaration: PersistentConstructorId }              // tag 3
  | EnumVariantConstructor { declaration: PersistentEnumVariantId }    // tag 4

CanonicalBinderUseListV1 = SourceOrderVec<SignatureTypeKey>

TemplateLocalRecordV1 {
    selector: LocalValueSelector,              // field 1，record primary key
    value_type: SignatureTypeKey,               // field 2
    mutable: CanonicalBooleanV1,                // field 3
    definition: TemplateLocalDefinitionV1,      // field 4
}

TemplateLocalDefinitionV1 =
    Source { origin: ExportDefinitionSourceV1 } // tag 1
  | Synthetic                                   // tag 2

TemplateReceiverV1 {
    local: LocalValueSelector,                  // field 1，wire为local_index
    value_type: SignatureTypeKey,               // field 2
}

OptionalTemplateReceiverV1 =
    Absent                                      // tag 1
  | Present { receiver: TemplateReceiverV1 }    // tag 2

TemplateValueParameterV1 {
    position: u32,                              // field 1
    local: LocalValueSelector,                  // field 2，wire为local_index
}
```

四种`PersistentLexicalRootV1`的wire分别是`{0:1,1:id}`、`{0:2,1:id}`、`{0:3,1:id}`与`{0:4,1:id}`；它刻意排除property accessor和generated callable。root必须解析为真实default provider的source declaration；`definition_path`必须非空、以该root为根且最后一段是`DefaultValue`。直接default的root就是key owner对应的source root；继承default允许二者不同，但必须由override/default-source relation证明其唯一provider，不得按方法名或相同payload猜测来源。

`CanonicalBinderUseListV1`不是集合，也不排序去重；它是长度可表示为`u32`的declaration-order映射。provider root的binder按“nominal owner frame在前、callable own frame在后”，每个frame内部按声明顺序展平；第`i`项是在**key owner的封闭signature binder scope**中表示的、用于替换provider第`i`个binder的`SignatureTypeKey`。直接default保存identity mapping，继承default保存已经沿override chain组合完毕的mapping。template的locals、body、result与receiver中出现的`Binder { depth, index }`仍以definition root的provider scope解释；consumer先按本表替换到key owner scope，winner确定后再代入concrete arguments。列表必须精确覆盖provider全部binder，不能缺项、多项或仅保存body碰巧使用的子集。

reader从provider identity重放binder shape时必须分别保留`nominal_owner_binder_arity`与
`callable_own_binder_arity`，不能只保留二者之和；否则在两个frame同时存在时无法把
`Binder { depth: 1, index }`与`Binder { depth: 0, index }`唯一映射到上述展平表位置。两项arity的
checked sum必须可表示为`u32`，且展平位置固定为先nominal owner frame、后callable own frame；没有own
frame而只有owner frame时，signature中的唯一frame仍使用`depth = 0`，但其展平位置从0开始。

`CanonicalTemplateLocalTableV1`是`TemplateLocalRecordV1`按`LocalValueSelector`完整结构序严格递增的array，拒绝重复，长度必须可表示为`u32`。它不保存`LocalId`、`BindingId`或body-local display name。`LocalValueSelector`沿用M23-2固定的tag与wire；本表只接受`This`、`Parameter`、`LocalDeclaration`、`BoundReceiver`和`Synthetic`，拒绝只会在后续coroutine transform产生的`SuspensionResult`。`This`、`Parameter`、`LocalDeclaration`与`BoundReceiver`必须使用`Source` definition，`Synthetic`必须使用`Synthetic` definition；所有带path的selector必须位于本template的`definition_path`之下。每个`Source` origin都按本section统一规则验证并进入顶层`definition_sources`精确闭包。所有`value_type`以definition root binder scope解释。

body、receiver、value-parameter和后续嵌套template-owned entity在wire中以canonical local table的zero-based unsigned `u32`下标引用local。decoded形态只暂存`local_index`；解析后语义形态必须保存对应的`LocalValueSelector`，writer再按selector反查canonical index。越界、selector/index不一致或同一selector映射到多条record均拒绝，任何raw `LocalId`/arena ordinal不得进入已解析HIR。

`OptionalTemplateReceiverV1`的wire为`Absent={0:1}`、`Present={0:2,1:receiver}`；`TemplateReceiverV1`为`{1:local_index,2:value_type}`。若key owner没有receiver则必须Absent；若有receiver则必须Present，local必须命中唯一`This`、不可变且provider-scope原始类型逐结构等于receiver的`value_type`。validator先在definition root的provider scope中验证该原始类型，再用`CanonicalBinderUseListV1`替换全部provider binder；替换后的类型必须逐结构等于key owner callable interface的receiver type。直接default的identity mapping自然得到相同结果，继承generic default不得跳过替换而直接比较两个不同binder scope中的表示。

`CanonicalTemplateValueParametersV1`是按`position`严格递增的array，record wire为`{1:position,2:local_index}`。对key中`parameter_position = p`的template，它必须精确包含`0..p`的全部前置参数且不含当前/后置参数；每条local必须命中不可变的`Parameter { declaration_index: position }`，其原始类型先在definition root的provider scope中验证，再经`CanonicalBinderUseListV1`替换后逐结构等于同owner source interface该位置的`value_type`。这张表按position建立hygienic替换，consumer绝不重新解析参数名，也不得把provider-scope binder与key-owner-scope binder按相同`depth/index`误判为同一类型。`mutable`、`allows_suspend`及本default wire中的其他布尔语义一律复用`CanonicalBooleanV1`的unsigned `False=1`、`True=2`编码，不接受CBOR native boolean。

template的`result`同样先在definition root的provider scope中验证；经上述完整binder mapping替换后，它必须逐结构等于key所指当前source parameter的`value_type`。`body.value.result_type`必须逐结构等于尚未替换的provider-scope `result`，不能只让body与自己一致却与发布方参数类型脱节。`allows_suspend`必须精确等于key owner callable的execution effect是否为`Suspend`：普通callable与全部constructor为`False`，suspend function为`True`；继承default不得保留一个与当前owner调用契约不一致的permission。

`ExportDefaultBodyV1`是M17已typed的statement/expression closure之canonical wire。它不是对`ExportHir`
arena的serde投影：每个跨Cone实体都使用persistent identity，template local只使用上述
canonical local table下标，循环目标由语法嵌套表示。根product固定为：

```text
ExportDefaultBodyV1 {
    statements: SourceOrderVec<DefaultStatementV1>, // field 1
    value: DefaultExpressionV1,                     // field 2
}

DefaultStatementV1 {
    kind: DefaultStatementKindV1,                   // field 1
    definition_origin: ExportDefinitionSourceV1,    // field 2
}

DefaultExpressionV1 {
    kind: DefaultExpressionKindV1,                  // field 1
    result_type: SignatureTypeKey,                  // field 2
    definition_origin: ExportDefinitionSourceV1,    // field 3
}
```

所有sequence长度必须可表示为`u32`。标为`SourceOrderVec`的sequence严格保留语义顺序，
writer和reader都不排序。所有optional都用`Absent={0:1}` / `Present={0:2,1:value}`
显式sum，不用CBOR null；所有boolean都用`CanonicalBooleanV1`。`definition_origin`
只能是definition-side source；consumer实例化时以调用点构造evaluation origin，不改写wire中的
definition origin。Export HIR中另有独立span的statement、when arm、catch、binding action、
iterator conformance与next step都各自保存`ExportDefinitionSourceV1`；不再并行保存裸`Span`。

body使用以下portable typed reference：

```text
DefaultCallableDeclarationV1 =
    Function(PersistentFunctionId)                   // tag 1
  | GenericFunction(PersistentGenericFunctionId)     // tag 2
  | PropertyAccessor(PersistentPropertyAccessorId)   // tag 3
  | Generated(PersistentGeneratedCallableId)         // tag 4

DefaultCallableRefV1 {
    declaration: DefaultCallableDeclarationV1,       // field 1
    owner: OptionalSignatureType,                    // field 2
    type_arguments: SourceOrderVec<SignatureTypeKey>,// field 3
}

DefaultBinderRefV1 { depth: u32, index: u32 }         // fields 1, 2

DefaultBoundCallableSourceV1 =
    Class { bound: SignatureTypeKey,
            callable: DefaultCallableRefV1 }          // tag 1, fields 1, 2
  | Interface { bound: SignatureTypeKey,
                member: CallableDeclarationId }       // tag 2, fields 1, 2

DefaultBoundCallableRefV1 {
    receiver_parameter: DefaultBinderRefV1,           // field 1
    source: DefaultBoundCallableSourceV1,              // field 2
    instantiated_signature: SignatureTypeKey,          // field 3; Function
}

DefaultMethodCalleeV1 =
    Callable(DefaultCallableRefV1)                     // tag 1
  | Bound(DefaultBoundCallableRefV1)                   // tag 2
  | DerivedEquality { owner_type: SignatureTypeKey }   // tag 3, field 1

DefaultConstructorRefV1 =
    Struct { declaration: PersistentConstructorId,
             owner_type: SignatureTypeKey }            // tag 1, fields 1, 2
  | Class { declaration: DefaultClassConstructorIdV1,
            owner_type: SignatureTypeKey }              // tag 2, fields 1, 2
  | Variant { declaration: PersistentEnumVariantId,
              owner_type: SignatureTypeKey }            // tag 3, fields 1, 2

DefaultClassConstructorIdV1 =
    Source(PersistentConstructorId)                     // tag 1
  | Generated(PersistentGeneratedCallableId)            // tag 2

DefaultEnumVariantRefV1 {
    declaration: PersistentEnumVariantId,               // field 1
    owner_type: SignatureTypeKey,                        // field 2; Enum application
}

DefaultEnumVariantFieldRefV1 {
    declaration: PersistentEnumVariantFieldId,          // field 1
    owner_type: SignatureTypeKey,                        // field 2; Enum application
}

DefaultFieldRefV1 =
    Struct { declaration: PersistentFieldId,
             owner_type: SignatureTypeKey }             // tag 1, fields 1, 2
  | Tuple { declaration_index: u32 }                    // tag 2, field 1
  | Class { declaration: PersistentFieldId,
            owner_type: SignatureTypeKey }              // tag 3, fields 1, 2

DefaultPlaceV1 =
    Local { local: LocalValueSelector }                 // tag 1; wire field 1=local_index
  | Global { property: PersistentPropertyId }           // tag 2, field 1
```

`DefaultCallableRefV1.owner`是声明方法所属的完整nominal application；top-level、extension、
local和generated callable必须`Absent`。`type_arguments`只保存callable own binder的声明顺序实参；
owner binder已在`owner`中。reader根据foundation与callable interface重放declaration kind、owner kind、
binder arity、function effect和精确signature，不允许通过缺省owner或空type arguments猜测。
`DefaultBoundCallableRefV1.receiver_parameter`必须在provider binder scope内，`bound`与
`instantiated_signature`也使用同一scope。`DerivedEquality`刻意以开放`SignatureTypeKey`表示；只有winner
的完整类型代换完成后才产生exact generated callable identity。

`DefaultConstructorRefV1`的`owner_type`必须分别是对应struct/class/enum声明的完整类型；
`DefaultFieldRefV1`同理。field和variant field使用persistent declaration id，不使用应用arena的
`local_index`。global读写使用`PersistentPropertyId`，singleton value使用
`PersistentObjectValueId`，initialization ensure使用`PersistentInitializationUnitId`，callback conversion
使用`PersistentCallbackRegistrationId`。这些identity都必须在同artifact foundation或已验证dependency
foundation中解析，不从显示名、symbol或arena ordinal回退。

嵌套callable的普通body仍由其terminal provider拥有，default template不复制该body。template只保存
创建closure/直接调用所需的下列descriptor：

```text
DefaultCaptureV1 {
    source: LocalValueSelector,                  // field 1; wire为local_index
    value_type: SignatureTypeKey,                // field 2
    first_use_origin: ExportDefinitionSourceV1,  // field 3
}

DefaultCallableBodyTypeArgumentsV1 =
    Lexical                                      // tag 1
  | Explicit(SourceOrderVec<SignatureTypeKey>)   // tag 2, field 1

DefaultLocalFunctionV1 {
    declaration: CallableDeclarationId,          // field 1; only Function/GenericFunction
    definition_path: StructuralDefinitionPath,   // field 2
    function_type: SignatureTypeKey,              // field 3; Function
    captures: SourceOrderVec<DefaultCaptureV1>,   // field 4; hidden ABI order
    owner_type_parameter_count: u32,              // field 5
}

DefaultLambdaV1 {
    body: PersistentGeneratedCallableId,          // field 1; Lexical/LambdaBody
    definition_path: StructuralDefinitionPath,    // field 2
    function_type: SignatureTypeKey,              // field 3; Function
    body_type_arguments: DefaultCallableBodyTypeArgumentsV1, // field 4
    captures: SourceOrderVec<DefaultCaptureV1>,   // field 5
    owner_type_parameter_count: u32,              // field 6
}

DefaultAnonymousFunctionV1 {                      // fields identical to DefaultLambdaV1
    body: PersistentGeneratedCallableId,          // field 1; Lexical/AnonymousFunctionBody
    definition_path: StructuralDefinitionPath,    // field 2
    function_type: SignatureTypeKey,              // field 3
    body_type_arguments: DefaultCallableBodyTypeArgumentsV1, // field 4
    captures: SourceOrderVec<DefaultCaptureV1>,   // field 5
    owner_type_parameter_count: u32,              // field 6
}

DefaultCallableReferenceTargetV1 =
    Named(DefaultCallableRefV1)                    // tag 1
  | Local { declaration: CallableDeclarationId,
            callee: DefaultCallableRefV1 }         // tag 2, fields 1, 2
  | BoundMember { receiver: DefaultExpressionV1,
                  callee: DefaultMethodCalleeV1 }  // tag 3, fields 1, 2
  | BoundExtension { receiver: DefaultExpressionV1,
                     callee: DefaultCallableRefV1 }// tag 4, fields 1, 2

DefaultCallableReferenceV1 {
    invoke: PersistentGeneratedCallableId,         // field 1; CallableReferenceInvoke
    definition_path: StructuralDefinitionPath,     // field 2
    target: DefaultCallableReferenceTargetV1,      // field 3
    function_type: SignatureTypeKey,               // field 4; Function
    captures: SourceOrderVec<DefaultCaptureV1>,    // field 5
    owner_type_parameter_count: u32,               // field 6
}
```

descriptor内的path必须是template `definition_path`的严格后代，并与相应persistent key中的
parent/role/path逐字节一致。capture sequence是provider hidden ABI的唯一顺序；每个source必须命中
canonical local table中的不可变local，`value_type`逐结构等于local类型。`BindingId`、capture name和
provider `FunctionId`不进入wire。同一persistent nested body可在表达式树中出现多次；每次都是
独立closure创建，因此descriptor内联而不建立另一张以body id去重的arena表。

statement kind的tag和payload固定为：

| tag | variant | payload fields |
|---:|---|---|
| 1 | `Expr` | `1=value` |
| 2 | `InitializationEnsure` | `1=PersistentInitializationUnitId` |
| 3 | `LocalFunction` | `1=DefaultLocalFunctionV1` |
| 4 | `Return` | `1=OptionalDefaultExpressionV1` |
| 5 | `ValDecl` | `1=pattern, 2=init` |
| 6 | `Assign` | `1=target, 2=value` |
| 7 | `If` | `1=condition, 2=then statements, 3=OptionalDefaultStatementListV1` |
| 8 | `While` | `1=condition setup, 2=condition, 3=body` |
| 9 | `For` | `1=DefaultForIterationPlanV1` |
| 10 | `Break` | no payload |
| 11 | `Continue` | no payload |
| 12 | `When` | `1=DefaultWhenV1` |
| 13 | `Try` | `1=DefaultTryV1` |
| 14 | `Throw` | `1=value` |

`while`/`for`自身即引入一个结构化loop scope，`Break`和`Continue`只能出现在非空loop
stack中且总是指向最内层；所以wire不保存`LoopId`。未来若增加labeled break，必须升级schema，
不能把当前无payload tag重解释为ordinal。

pattern和assign target为：

```text
DefaultPatternV1 =
    Binding { local }                              // tag 1; field 1=local_index
  | Wildcard                                       // tag 2
  | Literal { value, equality, subject_type }       // tag 3; fields 1..3
  | Variant { variant, fields }                    // tag 4; fields 1,2
  | Tuple { elements }                             // tag 5; field 1
  | Struct { owner_type, fields }                  // tag 6; fields 1,2

DefaultPatternFieldV1 { declaration_index: u32, pattern: DefaultPatternV1 } // fields 1,2

DefaultLiteralEqualityV1 =
    Integer { kind: DefaultIntegerKindV1,
              target: DefaultCallableRefV1 }       // tag 1, fields 1,2
  | Ordinary { target: DefaultCallableRefV1 }       // tag 2, field 1

DefaultAssignTargetV1 =
    Local { local }                                 // tag 1; field 1=local_index
  | Global { property: PersistentPropertyId }       // tag 2, field 1
  | Index { array, index }                          // tag 3, fields 1,2
  | Field { receiver, field: DefaultFieldRefV1 }    // tag 4, fields 1,2
```

pattern field sequence按declaration index严格递增且拒绝重复；tuple element与pattern tree保留源顺序。
`SingletonPublishedRoot`与`InitializingClassField`是initializer-only capability，不属于default的
`DefaultAssignTargetV1`。producer若从Export HIR default观察到它们必须拒绝构造artifact，reader的closed sum
也没有可以表示它们的tag。

control-flow product固定为：

```text
DefaultWhenV1 {
    subject: DefaultExpressionV1,                  // field 1
    arms: SourceOrderVec<DefaultWhenArmV1>,        // field 2
    fallback: DefaultWhenFallbackV1,               // field 3
}

DefaultWhenArmV1 {
    pattern: DefaultPatternV1,                     // field 1
    guard: OptionalDefaultWhenGuardV1,             // field 2
    body: SourceOrderVec<DefaultStatementV1>,      // field 3
    definition_origin: ExportDefinitionSourceV1,  // field 4
}

DefaultWhenGuardV1 {
    setup: SourceOrderVec<DefaultStatementV1>,     // field 1
    condition: DefaultExpressionV1,                // field 2
}

DefaultWhenFallbackV1 =
    Else(SourceOrderVec<DefaultStatementV1>)       // tag 1, field 1
  | IrrefutableArm { subject_type }                // tag 2, field 1
  | PatternMatrix { subject_type }                 // tag 3, field 1
  | EnumPatternMatrix { subject_type, owner_type } // tag 4, fields 1,2

DefaultTryV1 {
    body: SourceOrderVec<DefaultStatementV1>,      // field 1
    catches: SourceOrderVec<DefaultCatchV1>,       // field 2
    finally_body: OptionalDefaultStatementListV1,  // field 3
}

DefaultCatchV1 {
    local: LocalValueSelector,                     // field 1; wire为local_index
    value_type: SignatureTypeKey,                  // field 2
    body: SourceOrderVec<DefaultStatementV1>,      // field 3
    definition_origin: ExportDefinitionSourceV1,  // field 4
}
```

`DefaultExpressionKindV1`的tag与完整payload如下。表中的`expressions`/`arguments`/`fields`均为
`SourceOrderVec<DefaultExpressionV1>`；`optional offset`使用显式optional sum：

| tag | variant | payload fields |
|---:|---|---|
| 1 | `StringLiteral` | `1=utf8 text, 2=DefaultStringOwnerV1` |
| 2 | `IntegerLiteral` | `1=CanonicalIntegerConstantV1` |
| 3 | `BooleanLiteral` | `1=CanonicalBooleanV1` |
| 4 | `UnitLiteral` | no payload |
| 5 | `TupleLiteral` | `1=elements` |
| 6 | `StructInit` | `1=DefaultConstructorRefV1::Struct, 2=arguments` |
| 7 | `StructConstruct` | `1=owner type, 2=fields` |
| 8 | `ClassInit` | `1=DefaultConstructorRefV1::Class, 2=arguments` |
| 9 | `VariantConstruct` | `1=DefaultEnumVariantRefV1, 2=arguments` |
| 10 | `VariantTest` | `1=operand, 2=DefaultEnumVariantRefV1` |
| 11 | `VariantPayloadProject` | `1=operand, 2=DefaultEnumVariantFieldRefV1` |
| 12 | `Local` | `1=local_index` |
| 13 | `GlobalRead` | `1=PersistentPropertyId` |
| 14 | `SingletonValue` | `1=PersistentObjectValueId` |
| 15 | `Lambda` | `1=DefaultLambdaV1` |
| 16 | `AnonymousFunction` | `1=DefaultAnonymousFunctionV1` |
| 17 | `CallableReference` | `1=DefaultCallableReferenceV1` |
| 18 | `FunctionCoercion` | `1=source, 2=source function type, 3=target function type` |
| 19 | `PtrFromNonZeroULong` | `1=operand` |
| 20 | `PtrToULong` | `1=operand` |
| 21 | `PtrCast` | `1=operand` |
| 22 | `PtrLoad` | `1=pointer, 2=optional offset` |
| 23 | `PtrStore` | `1=pointer, 2=optional offset, 3=value` |
| 24 | `PtrOffset` | `1=pointer, 2=offset, 3=subtract` |
| 25 | `AddressOf` | `1=DefaultPlaceV1` |
| 26 | `SizeOf` | `1=queried type` |
| 27 | `AlignOf` | `1=queried type` |
| 28 | `FunctionAddress` | `1=DefaultCallableDeclarationV1` |
| 29 | `ForeignCallbackRegister` | `1=PersistentCallbackRegistrationId, 2=closure` |
| 30 | `ForeignCallbackOperation` | `1=DefaultForeignCallbackOperationV1, 2=callback` |
| 31 | `FieldAccess` | `1=receiver, 2=DefaultFieldRefV1` |
| 32 | `MethodCall` | `1=receiver, 2=DefaultMethodCalleeV1, 3=arguments` |
| 33 | `DirectSuperMethodCall` | `1=receiver, 2=DefaultMethodCalleeV1, 3=arguments` |
| 34 | `Box` | `1=operand` |
| 35 | `Unbox` | `1=operand` |
| 36 | `IsInstance` | `1=operand, 2=checked type` |
| 37 | `Cast` | `1=operand, 2=optional` |
| 38 | `ArrayLiteral` | `1=elements` |
| 39 | `ArrayAssembly` | `1=DefaultArrayAssemblyV1` |
| 40 | `Index` | `1=DefaultArrayAccessKindV1, 2=receiver, 3=index` |
| 41 | `ArraySet` | `1=DefaultArrayAccessKindV1, 2=receiver, 3=index, 4=value` |
| 42 | `ArrayLen` | `1=operand` |
| 43 | `ArrayClone` | `1=operand` |
| 44 | `Call` | `1=DefaultCallableRefV1, 2=arguments` |
| 45 | `LocalFunctionCall` | `1=local declaration, 2=callee, 3=captures, 4=arguments` |
| 46 | `CallableCall` | `1=callee expression, 2=function type, 3=arguments` |
| 47 | `PrimitiveBinary` | `1=DefaultPrimitiveBinaryKindV1, 2=lhs, 3=rhs` |
| 48 | `PrimitiveUnary` | `1=DefaultPrimitiveUnaryKindV1, 2=operand` |
| 49 | `IntegerOperation` | `1=DefaultIntegerOperationV1, 2=DefaultIntegerArgumentsV1` |
| 50 | `IntegerConversion` | `1=source kind, 2=target kind, 3=target callable, 4=operand` |
| 51 | `Binary` | `1=DefaultBinaryOperatorV1, 2=lhs, 3=rhs` |
| 52 | `Unary` | `1=DefaultUnaryOperatorV1, 2=operand` |
| 53 | `SomeWrap` | `1=operand` |
| 54 | `NoneLiteral` | no payload |
| 55 | `IsSome` | `1=operand` |
| 56 | `Unwrap` | `1=operand, 2=trap_on_none` |

Export HIR的`ImportedCoreCall`在此正规化为tag 44 `Call`，因为portable callable identity已表达其
terminal origin，process-local imported arena不是语义。`ConstructorParam`、`Capture`、
`InitializingClassFieldAccess`和`InitializingStructFieldAccess`只属于constructor/nested-callable body；
exported default的独立scope不能产生它们，v1 expression sum不为它们分配tag。nested callable
body中的`Capture`仍留在provider body，不进入default wire。

其余expression辅助结构固定为：

```text
DefaultStringOwnerV1 =
    CurrentInstantiation                            // tag 1
  | Property(PersistentPropertyId)                  // tag 2, field 1

DefaultArrayAssemblyV1 {
    element_type: SignatureTypeKey,                 // field 1
    parts: SourceOrderVec<DefaultArrayAssemblyPartV1>, // field 2
    result_type: SignatureTypeKey,                  // field 3; Array application
}

DefaultArrayAssemblyPartV1 =
    Element(DefaultExpressionV1)                    // tag 1, field 1
  | CopyArray(DefaultExpressionV1)                  // tag 2, field 1

DefaultIntegerOperationV1 =
    NoGc { kind: DefaultIntegerKindV1,
           operation: DefaultNoGcIntegerOperationV1,
           target: DefaultCallableRefV1 }           // tag 1, fields 1..3
  | Managed { kind: DefaultIntegerKindV1,
              operation: DefaultIntegerDivRemV1,
              target: DefaultCallableRefV1 }        // tag 2, fields 1..3

DefaultIntegerArgumentsV1 =
    Unary(DefaultExpressionV1)                      // tag 1, field 1
  | Binary { lhs: DefaultExpressionV1,
             rhs: DefaultExpressionV1 }             // tag 2, fields 1,2
```

leaf enum的unsigned tag固定为：

- `DefaultIntegerKindV1`: `Signed8=1, Signed16=2, Signed32=3, Signed64=4, Unsigned8=5, Unsigned16=6, Unsigned32=7, Unsigned64=8`；
- `DefaultNoGcIntegerOperationV1`: `UnaryPlus=1, UnaryMinus=2, Inc=3, Dec=4, Add=5, Sub=6, Mul=7, CompareTo=8, Equals=9, And=10, Or=11, Xor=12, Inv=13, Shl=14, Shr=15, Ushr=16`；
- `DefaultIntegerDivRemV1`: `Div=1, Rem=2`；
- `DefaultPrimitiveBinaryKindV1`: `StringConcat=1, StringCompareTo=2`；`DefaultPrimitiveUnaryKindV1`: `BooleanNot=1`；
- `DefaultArrayAccessKindV1`: `ImmutableGet=1, MutableGet=2, MutableSet=3`；
- `DefaultForeignCallbackOperationV1`: `Retain=1, Release=2, State=3, Failure=4`；
- `DefaultBinaryOperatorV1`: `Lt=1, Le=2, Gt=3, Ge=4, RefEq=5, RefNe=6, And=7, Or=8`；`DefaultUnaryOperatorV1`: `Not=1`。

`DefaultForIterationPlanV1`与binding plan的wire是：

```text
DefaultBindingTemporaryV1 { local, value_type }     // fields 1,2; local wire为local_index
DefaultBindingLeafV1 { local, value_type, mutable } // fields 1..3

DefaultBindingShapeV1 =
    Binding(DefaultBindingLeafV1)                   // tag 1, field 1
  | Wildcard                                        // tag 2
  | Tuple(SourceOrderVec<DefaultBindingShapeV1>)    // tag 3, field 1
  | Struct { owner_type,
             fields: CanonicalVec<DefaultBindingStructFieldV1> }
                                                    // tag 4, fields 1,2
  | Class { owner_type,
            components: SourceOrderVec<DefaultBindingClassComponentV1> }
                                                    // tag 5, fields 1,2

DefaultBindingStructFieldV1 {
    declaration_index: u32,                         // field 1
    shape: DefaultBindingShapeV1,                   // field 2
}

DefaultBindingClassComponentV1 {
    index: NonZeroU32,                              // field 1
    shape: DefaultBindingShapeV1,                   // field 2
}

DefaultBindingProjectionV1 =
    TupleIndex(u32)                                 // tag 1, field 1
  | StructField(DefaultFieldRefV1::Struct)          // tag 2, field 1

DefaultBindingActionV1 =
    Project { source, result, projection,
              definition_origin }                  // tag 1, fields 1..4
  | Component { source, index: NonZeroU32, result,
                setup, call, definition_origin }   // tag 2, fields 1..6
  | Bind { source, target, definition_origin }      // tag 3, fields 1..3

DefaultBindingPlanV1 {
    subject: DefaultBindingTemporaryV1,             // field 1
    shape: DefaultBindingShapeV1,                   // field 2
    actions: SourceOrderVec<DefaultBindingActionV1>,// field 3
}

DefaultIteratorConformanceV1 {
    source: DefaultBindingTemporaryV1,              // field 1
    iterator: DefaultBindingTemporaryV1,            // field 2
    interface_type: SignatureTypeKey,               // field 3
    definition_origin: ExportDefinitionSourceV1,    // field 4
}

DefaultAppliedOptionV1 {
    some_payload: DefaultEnumVariantFieldRefV1,     // field 1
    none: DefaultEnumVariantRefV1,                  // field 2
}

DefaultIteratorNextV1 {
    callable: DefaultCallableRefV1,                 // field 1
    result: DefaultBindingTemporaryV1,              // field 2
    option: DefaultAppliedOptionV1,                 // field 3
    element: DefaultBindingTemporaryV1,             // field 4
    definition_origin: ExportDefinitionSourceV1,    // field 5
}

DefaultForIterationPlanV1 {
    source_setup: SourceOrderVec<DefaultStatementV1>, // field 1
    source: DefaultBindingTemporaryV1,                // field 2
    source_init: DefaultExpressionV1,                 // field 3
    iterator_setup: SourceOrderVec<DefaultStatementV1>, // field 4
    iterator_call: DefaultExpressionV1,               // field 5
    conformance: DefaultIteratorConformanceV1,         // field 6
    next: DefaultIteratorNextV1,                       // field 7
    binding: DefaultBindingPlanV1,                     // field 8
    body: SourceOrderVec<DefaultStatementV1>,          // field 9
}
```

struct binding fields按field declaration index严格递增，class component sequence保留source order且不得重复
`NonZeroU32` index。所有temporary/leaf/local read/place/capture的类型和mutability都必须与canonical local
table中同selector record一致。iterator interface、next callable和Option variant relation由trusted core protocol
authority重放，不按名称`Iterator`/`next`/`Some`/`None`认定。

body semantic validation还必须检查：所有expression result type和operation期望类型一致；局部声明的
definition-before-use、可变性、pattern/action完整性与循环嵌套成立；嵌套callable descriptor的
identity/path/role/signature/capture ABI与provider foundation一致；任何callable、constructor、type、global、
singleton和field引用都与`references`的规范去重闭包精确相等。解码和这些递归检查共用顶层
`BudgetMeter`；每个node、edge、owned byte、collection reserve和semantic depth均在分配/下潜前扣费，
不另起无限制递归或私有budget。

default reference closure沿用M17的六个互不兼容的typed domain，不把target压成无类型entity id：

```text
ExportDefaultReferenceSetV1 {
    callables: CanonicalSet<ExportDefaultCallableReferenceV1>,       // field 1
    constructors: CanonicalSet<ExportDefaultConstructorReferenceV1>,// field 2
    types: CanonicalSet<ExportDefaultTypeReferenceV1>,               // field 3
    globals: CanonicalSet<ExportDefaultGlobalReferenceV1>,           // field 4
    singleton_values: CanonicalSet<ExportDefaultSingletonReferenceV1>,// field 5
    fields: CanonicalSet<ExportDefaultFieldReferenceV1>,             // field 6
}

ExportDefaultCallableReferenceV1 = ExportDefaultReferenceV1<ExportDefaultCallableTargetV1>
ExportDefaultConstructorReferenceV1 = ExportDefaultReferenceV1<DefaultConstructorRefV1>
ExportDefaultTypeReferenceV1 = ExportDefaultReferenceV1<SignatureTypeKey>
ExportDefaultGlobalReferenceV1 = ExportDefaultReferenceV1<PersistentPropertyId>
ExportDefaultSingletonReferenceV1 = ExportDefaultReferenceV1<PersistentObjectValueId>
ExportDefaultFieldReferenceV1 = ExportDefaultReferenceV1<DefaultFieldRefV1>

ExportDefaultReferenceV1<T> {
    target: T,                                      // field 1
    definition_origin: ExportDefinitionSourceV1,   // field 2
    witness: ExportDefaultAccessWitnessV1,          // field 3
}

ExportDefaultCallableTargetV1 =
    Callable(DefaultCallableRefV1)                  // tag 1, field 1
  | Bound(DefaultBoundCallableRefV1)                // tag 2, field 1
  | DerivedEquality { owner_type: SignatureTypeKey }// tag 3, field 1
  | LocalFunction { declaration: CallableDeclarationId } // tag 4, field 1
  | Lambda { body: PersistentGeneratedCallableId } // tag 5, field 1
  | AnonymousFunction { body: PersistentGeneratedCallableId } // tag 6, field 1
  | CallableReference { invoke: PersistentGeneratedCallableId } // tag 7, field 1
  | FunctionAddress { declaration: DefaultCallableDeclarationV1 } // tag 8, field 1

ExportDefaultAccessWitnessV1 {
    owner: CallableDeclarationId,                  // field 1
    call_domain: ExportDefaultCallDomainV1,        // field 2
    target_domain: ExportDefaultTargetDomainV1,    // field 3
}

ExportDefaultCallDomainV1 =
    DirectPublic                                   // unsigned 1
  | DirectAndPublicSlot                            // unsigned 2

ExportDefaultTargetDomainV1 =
    Universal                                      // unsigned 1
```

`ExportDefaultCallableTargetV1`逐variant对应定义方HIR中产生access witness的八种callable
target；它不从body形状猜出另一种target。`LocalFunction.declaration`只接受Function或
GenericFunction，并由其source declaration key重放local path；三个lexical generated variant的
persistent key必须分别具有`LambdaBody`、`AnonymousFunctionBody`或`CallableReferenceInvoke`的精确
root/path/role。`DerivedEquality`在winner替换前没有exact generated callable identity，所以保留
provider-scope的`owner_type`并由type authority重放；不能为它伪造一个persistent callable id。
constructor、field与type target保留完整applied/structural ref，因此同一declaration的不同
provider-scope application不会被错误合并。tuple field是合法的structural `DefaultFieldRefV1`，不为它
制造persistent field id。

每个typed set按record的`(target, definition_origin, witness)`结构序严格递增并拒绝重复；target sum
按上述tag排序，product按字段序，persistent id按raw bytes，其余叶使用各自已冻结的canonical顺序。
六个set彼此不合并排序，长度均必须可表示为`u32`。record与witness都使用上述精确三字段map；
callable target的八种wire均为`{0:tag,1:payload}`。reader不得排序修复、按显示名合并target，或把
不同definition origin的两次绑定折成一条记录。

cross-Cone section只发布public callable的source interface，因此成功witness的direct call domain与
target access domain都必为universal；承担public slot的owner还必须同时覆盖universal slot domain。
wire据此使用两个refined leaf type，而不序列化含本地arena id的M21通用`AccessDomain`：
`DirectPublic`对应owner callable record的`DirectOnly`，`DirectAndPublicSlot`对应`PublicSlot`，
`Universal`由target public interface或同template lexical ownership重算。这个收窄不是省略证明：
reader必须逐条核对witness owner等于template key owner、owner callable access精确映射到
`call_domain`，并从target种类重建`target_domain`及调用域包含关系。foreign internal/private target、
缺失`DefaultDependency` external route、generated key与template root/path/role不一致、普通`Export*Id`
冒充refined ref均拒绝。

reference set必须与template locals及body在优化、const folding和desugaring前直接绑定的typed引用按
上述record identity形成双向精确闭包：缺项、多余项或错误definition origin均拒绝。body traversal
保持source order，但最终set按canonical key去重；同一target在不同definition origin出现时是两条记录。
expression result、显式type operand、callable/constructor/field applied owner与local record中的
`SignatureTypeKey`都参与type reference收集；binder leaf本身不是外部实体。template-owned
local/lambda/anonymous/callable-reference target仍携带witness，并由其lexical identity证明universal；
callback registration和initialization unit是相应template operation的identity，不另造第七种access
reference domain。reader从目标public interface重算domain包含关系；reference set与body实际typed
引用的规范去重集合必须完全相等，无多余项。

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
    target: TypeAliasTargetV1,      // field 2
    access: PublicLookupAccessV1,   // field 3
    definition_origin: ExportDefinitionSourceV1, // field 4
}

TypeAliasTargetV1 =
    Signature { target: SignatureTypeKey } // { 0: 1, 1: target }
  | Alias { alias: PersistentTypeAliasId } // { 0: 2, 1: alias }
```

`CanonicalConstValueV1`的v1 wire只覆盖当前HIR已经能够产生的三类编译期值，并固定为以下封闭sum：

```text
CanonicalConstValueV1 =
    Integer { value: CanonicalIntegerConstantV1 } // { 0: 1, 1: value }
  | Boolean { value: CanonicalBooleanV1 }         // { 0: 2, 1: value }
  | String { utf8: text }                         // { 0: 3, 1: utf8 }

CanonicalIntegerConstantV1 =
    Signed8 { raw_bits: u8 }     // { 0: 1, 1: raw_bits }
  | Signed16 { raw_bits: u16 }   // { 0: 2, 1: raw_bits }
  | Signed32 { raw_bits: u32 }   // { 0: 3, 1: raw_bits }
  | Signed64 { raw_bits: u64 }   // { 0: 4, 1: raw_bits }
  | Unsigned8 { raw_bits: u8 }   // { 0: 5, 1: raw_bits }
  | Unsigned16 { raw_bits: u16 } // { 0: 6, 1: raw_bits }
  | Unsigned32 { raw_bits: u32 } // { 0: 7, 1: raw_bits }
  | Unsigned64 { raw_bits: u64 } // { 0: 8, 1: raw_bits }

CanonicalBooleanV1 = False // unsigned 1
                   | True  // unsigned 2
```

signed integer payload继续保存对应宽度的二进制补码raw bits；reader必须在构造variant前检查payload可由对应`u8/u16/u32/u64`表示，不能截断、符号扩展或按数值大小改写variant。String是解码后拥有的有效UTF-8字节序列，保持源码求值结果的byte identity，不做Unicode normalization、NUL过滤或host编码转换，并服从semantic-leaf与owned-byte累计budget。Wire CBOR v1不接受native boolean，因此boolean payload必须使用上述显式unsigned枚举。当前尚未开放的`Char`与floating const没有保留的伪variant；开放对应语言能力前必须显式修订该versioned schema与profile。

const table精确覆盖public const property，不含ordinary property storage/initializer。每条record的property必须是当前artifact foundation中的canonical、`ConeWide`的ordinary property，且同id的property interface必须为`Const + ReadOnly + DirectOnly`；`value_type`逐结构等于property interface的类型。definition origin逐字段等于foundation中`DefinitionOriginSubject::Property(property)`，并属于当前Cone。value/type一致性由trusted core const-type authority证明：八种integer variant分别只匹配其canonical fixed-width有/无符号core nominal，Boolean与String只匹配各自canonical non-generic core nominal；不能按显示名称、bit width或同布局用户类型接受。const table按property raw id严格递增、拒绝重复，并与property interface中全部且仅有的`Const`记录形成双向精确闭包。

consumer内联value并使用当前usage evaluation origin；String等需要本地materialization的常量由当前Cone按既有literal规则拥有，不引用provider storage。

alias在M23-5仍只允许top-level、non-generic声明，因此record的`access`必须是`DirectOnly`；`PublicSlot`、nested/local identity、非`ConeWide` declaration scope或target中的`Binder`一律拒绝。`alias`必须解析为当前artifact foundation中的canonical `TypeAlias` source declaration，`definition_origin`必须逐字段等于同foundation `DefinitionOriginSubject::TypeAlias(alias)`的记录；仅凭raw digest或来自另一Cone的origin不能构造记录。

源码target若最外层直接命中另一个typealias，producer保留为`TypeAliasTargetV1::Alias`；其余target先递归展开内部alias，再写成alias-free的`Signature`。这样`typealias A = B`链保留每条alias edge，而`Container<B>`之类复合target没有第二套递归type wire，内部`B`按透明语义写入其最终target。`Alias`可以指向当前artifact的public alias，或经direct dependency public binding/完整re-export route授权的foreign public alias；不能按FQN、名称或support artifact枚举取得。`Signature`使用空binder scope完成全部nominal kind/application arity验证。

closure-wide expander以`PersistentTypeAliasId`作memo key，沿`Alias` edge展开；authority只可返回已经完成单record语义验证的alias interface，并对每条`(source alias, target alias)`分别证明：同Cone target确实位于该Cone public alias table，或foreign target具有从source Cone出发的direct public binding/完整re-export route。不能用“target record存在”替代可达性证明，也不能从FQN、名称或support artifact枚举取得target。展开器按当前alias table的canonical顺序启动root，成功输出同顺序的`alias -> final SignatureTypeKey` typed map；transitive foreign alias只进入共享memo而不凭遍历副作用加入当前table输出。

展开入口必须接收closure共用的`BudgetMeter`与调用方提供的`WirePath`，不得创建或重置私有meter。资源计量固定如下：每个首次访问且尚未memo的distinct alias扣一个decoded logical node和一个validation work unit；每条实际跟随的`Alias` edge扣一个decoded edge和一个validation work unit；root记semantic depth 1，每跟随一条非cycle edge加1并在lookup前调用`check_semantic_depth`。root输出vector、memo map、active-position map、显式DFS stack与cycle chain都必须在分配前使用meter的collection/map reserve API预扣logical heap；实现必须使用显式stack，不能让untrusted chain消耗Rust调用栈。已memo节点不重复扣node/edge/work，其final target以共享只读值复用，不能按入边数量深拷贝完整type tree。

发现active target时，cycle错误保存从该target首次进入active stack的位置起、直到当前source、再追加一次该target的完整typed id链；例如`A -> B -> C -> A`必须报告`[A, B, C, A]`，不能只报告首节点或截断前缀。构造该链前先checked计算其canonical diagnostic文本上界`64 * id_count + 4 * (id_count - 1)` bytes（每个id为64位小写hex，分隔符为` -> `），并用`check_semantic_leaf`核对；错误本身保存typed ids、按需格式化，不保存来自artifact的任意文本。单边失败次序固定为：先扣edge/work并验证访问授权，再检查active cycle，随后检查下一depth并查找target record；因此未授权edge不会泄漏target是否存在，资源上限也不能通过cycle诊断绕过。任何checked算术、meter或fallible allocation失败均返回原`WireError`和传入path，且不提交alias expansion cache。

由于正常resolved dependency graph无环，跨Cone环也属于artifact损坏而不是允许的递归类型；同artifact自环/多节点环同样拒绝。使用点`as` alias和import local name不是typealias identity，不进入该图。展开完成后所有type equality、overload signature和persistent application都使用最终`SignatureTypeKey`；diagnostic可保留alias spelling作decorator。alias table按`alias` raw id严格递增并拒绝重复；reader不得排序修复。

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

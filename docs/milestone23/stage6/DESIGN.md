# M23-6 设计：跨 Cone layout、typed ABI 与 ZST

版本：1.0（设计完成，待实现；2026-09-19）

依赖：M23-5

上位设计：[M23 系列设计](../DESIGN.md)

规范依据：

- [语言规范](../../specs/SCOOP-SPEC.md) 第 3.3、4.7、7.4、9.1、10、12.5、13.4～13.10、14 章；
- [Runtime 规范](../../specs/SCOOP-RUNTIME-SPEC.md) 第 2.1～2.8、3.1～3.3、3.6、4.2 节；
- [实现大纲](../../specs/SCOOP-IMPL-SPEC.md) 第 2.2～2.6、2.9、2.11 节；
- [M23 总设计](../DESIGN.md) 第 3.1～3.6、4.4～4.6、5.3、6.1、9.4～9.5、10 章；
- [M23-2](../stage2/DESIGN.md) 的 exact identity、`ExactOwnerRoot`、native witness、target projection 与 canonical ABI 编码；
- [M23-3](../stage3/DESIGN.md) 的 strong production、有限 shape-support、definition/image proof 与双 view；
- [M23-5](../stage5/DESIGN.md) 的 semantic world、access provenance、selected metadata 与 cross-Cone object-use 分区。

本文中的 runtime ABI 指 M23 基线：TypeDescriptor 最后一个字段是 `diagnostic_name`，callable body 使用 v1 identity，三层 metadata 的 `outer_schema` 均为 1。规范中已提前写出的 M24 `release_hook`、callable-body-v2 与 schema 2 不属于本阶段。

## 0. 结论

M23-6 把已有的本地类型表示变成可独立验证、可跨 Cone 消费的接口。成功解析到外部 nominal 之后，consumer 必须取得定义方的完整语义、layout、ABI、scan、TypeDescriptor 与 dispatch 证明，才能产生 machine use。

1. 新增 `cross-cone-layout-strong/1` production profile。它在 M23-5 inventory 上新增 HIR type/inheritance、MIR type bridge、LIR layout/ABI 和 Link-only layout-use closure，并将强定义语义升级为 `strong-production/2`，以表达 ordinary dependency TD/dispatch 引用；仍拒绝全部 ODR production。
2. 不改变 M23-2 的任何 persistent identity、native-boundary witness、extern/callback contract bytes，也不扩大 M23-3/M23-5 旧 capability 的含义。一般 layout 服务只能由新 required section 构造。
3. HIR 输出每个 concrete type 完备的 `gc_free`、value `ZstStatus` 与继承/slot 语义；MIR 输出表示无关的类型、构造器、成员、slot 与生成 helper 关系；LIR 独占 target layout、Scoop ABI 与递归 scan 的生产权。
4. 外部实体保持定义 Cone 的 Strong ownership。consumer 可以检查和在本地类型中内联外部 value 的表示，但不能重新定义其 body、layout constant、scan、TD、dispatch table、registration 或初始化 storage。
5. 定义 Cone 为每个可跨 Cone 引用的 param-free source nominal 预物化完整、有限的 `BoxedValue`（仅 value）、`CoroutineStep`、`CoroutineSlot` shape-support。closure 从 source subject 展开一次，不递归把 helper 当作新 source subject。
6. 通用 `ValueStorageLayout` 使用 `ZeroSized | NonZero`，zero-sized 分支不能携带 ref scan；未装箱 value layout 与五类 managed instance shape 分离。可分配操作只能接收排除 `AbstractRef` 的 refined descriptor。
7. Scoop ABI 保留全部 logical exact type，并用 `ElidedZst` 删除物理 payload。`UnitVoid` 与用户 ZST result 分开；全部 direct、invoke、virtual/interface、adapter 与 Scoop extern 复用同一分类和 logical-to-physical 映射。
8. ZST 的求值、异常、构造和方法语义不消失。需要地址的 local/parameter/value `this` 使用独立 token，static token 沿已有 persistent storage contract；每次 boxing 仍有 fresh managed identity。
9. `Array`/`MutableArray<ZST>` 保留 logical size、bounds、求值和 index iteration，allocation 不随 length 增长，不能生成 zero-stride scan、payload copy 或逐元素 token。
10. 开放 closure 完备的 param-free 跨 Cone 构造、value 投影、member/accessor、object value、继承、virtual/interface dispatch、`is/as` 与 protected access。protected bridge 是带用途证明的元数据通道，不是把 protected 声明提升成 public wrapper。
11. 通用表示算法和 compiler/layout/object 测试本阶段覆盖 generic/structural shape；需要独立 Nominal/Structural ODR materialization 的生产请求仍留 M23-7。`Array<T>`、tuple、function、pointer 不因布局简单获得 Strong 特赦。
12. 本阶段产出双 view 有效的多 Cone `.slib`，验证 local runtime 表示操作；真实 multi-image startup、artifact-only program-link 和多 Cone moving-GC 分别留 M23-8、M23-9、M23-11。

核心关系为：

```text
source access proof + complete type/inheritance interface
    -> committed exact external use
    -> selected MIR representation/dispatch relation
    -> selected LIR layout/ABI/scan/descriptor proof
    -> metadata-only dependency | verified physical Strong use

same size/alignment != same exact type
same physical signature != same callable contract
external layout knowledge != permission to emit an external definition
ExactOwnerRoot == SourceCone != exemption from dependency ODR requirements
```

## 1. 范围、基线与阶段边界

### 1.1 当前实现基线

截至 M23-5，仓库已具备若干可复用的本地表示，但尚未形成一般跨 Cone authority：

| 位置 | 已有能力 | 本阶段工作 |
| --- | --- | --- |
| `compiler/lir/src/abi.rs` | `AbiZst`、Direct/Indirect/ElidedZst、logical-to-physical 参数映射 | 绑定 persistent exact type 与 imported layout proof，统一全部调用入口 |
| `compiler/lir/src/type_descriptor.rs` | checked value/array storage、五类 instance shape 与扫描约束 | 形成跨 Cone required wire、closure validator 和 refined external descriptor |
| `compiler/lir/src/metadata.rs`、`compiler/lir-lower/src/metadata/layouts/` | 本地 aggregate/enum/field layout | 收口 raw size/alignment 构造，接入定义方布局与继承 prefix |
| `compiler/lir/src/production/shape_support/` | core 的有限 shape-support proof | 推广到普通定义 Cone，保留 core 旧 proof 的唯一 authority |
| `compiler/hir/src/visibility.rs`、`compiler/hir-lower/src/visibility.rs` | lookup/inheritance/slot domain 与本地 protected 规则 | 导出 persistent inheritance surface，生成跨 Cone receiver witness |
| HIR/MIR/LIR 的 `cross_cone_*` 模块 | M23-5 core-closed callable 子集与 selected bridge | 增加一般类型、构造、slot/dispatch 和 object-value selection |
| `compiler/lir-lower/src/function/expression.rs`、`runtime/src/rt.c` | 现有 box/unbox/array 执行路径 | 消除旧 payload/size/scan 多源调用与固定 header-offset 假设 |
| `compiler/codegen/src/function.rs` | 本地地址存储发射 | 在 LIR 明确 token place，codegen 不再由 LLVM 空类型猜 ZST |

本文的“首次冻结”指可跨 artifact 复用的通用契约，不表示重新实现所有本地算法。现有内部类型只有经过新 proof 构造器验证后才能进入导出表；Rust DTO 已存在不等于它已是稳定 wire。

### 1.2 生产成功矩阵

| 使用 | M23-6 结果 | 必要证明 |
| --- | --- | --- |
| M23-5 已成功的 const、top-level/extension callable | 继续成功 | 旧 route/bridge 原样保留 |
| param-free public struct/enum 参数、返回、构造、模式、copy update | 成功 | public representation + MIR shape + LIR layout/ABI |
| param-free class allocation、base constructor、member/accessor | 成功 | constructor access、完整 object shape、initializer bridge |
| param-free interface/default/virtual dispatch、`is/as` | 成功 | 完整 ancestry、slot contract、唯一 implementation 与 TD |
| public object/companion value及 runtime property | 成功 | provider ensure/value/accessor target 与初始化 ownership |
| subclass 中合法 protected member/constructor/nested type 使用 | 成功 | inheritance route、用途专属 access witness 与完整所需 bridge |
| imported value 嵌入本地 field/enum/capture | 成功 | representation closure；新生成实体的 ownership 也须通过 Strong gate |
| generic nominal/callable application，generic delegated extension | M23-7 能力诊断 | 不生成 partial LocalConcrete/MIR |
| 独立 tuple/function/raw/native-pointer TD 或其他 Structural ODR entity | M23-7 能力诊断 | 不因物理布局简单改成 Strong |
| suspend external call、需要 `ContinuationShell`/`CoroutineStart` 的操作 | M23-7 能力诊断 | hidden ABI 依赖 generic nominal application |
| direct foreign source `@Extern`/native storage 使用 | M23-10 能力诊断 | 完整 native closure 尚未开放 |

所有“成功”格还要求传递物化闭包不包含 M23-7/10 能力。param-free declaration 不保证其字段、签名、默认参数或 generated dependency 也是 param-free；gate 必须检查实际闭包，不能只检查声明的 type-parameter count。

### 1.3 表示覆盖与 ODR gate

本阶段完整定义和验证 `Array<ZST>`、`Option<ZST>`、`Phantom<A/B>`、tuple/function/pointer 的表示，不据此开放这些 application 的 production ownership。

- compiler/layout tests 可直接构造 fully concrete typed input，验证算法、IR、wire constituent 与 object emission；不能伪装成已通过 production profile 的 artifact。
- runtime 单 image harness 可验证 box、array、scan、token 的真实行为；不得将它报告为多 Cone startup/program-link 验收。
- production 继续应用 M23-3 §10.2～10.3：任何 ODR record、body、symbol、definition node 均拒绝，哪怕只有一个 producer。
- source-owned aggregate 的嵌套结构只为计算外层 shape 时，可用无 persistent identity 的 transient layout value；结果吸收进外层 layout/scan。该例外不生成独立 TD、dispatch、box、callable 或 addressable entity，不授予 nested generic application 的源码执行能力。

Stage 7 只补物化和一致性证明，不能再修改本阶段冻结的零尺寸、field offset、scan 或 typed ABI 规则。

### 1.4 不属于本阶段

不新增源码语法、泛型能力、target、C register classifier、runtime release hook、friend visibility、field/array-element `addressOf`、动态 image、最终 linker 或正式 `scoop run`。M23-4 locator/cache/调度和 M23-5 名称候选顺序保持既有契约。

## 2. crate 与输入边界

```text
scoop-hir       type facts / inheritance interface / selected use / access witness
scoop-hir-lower 语义检查、receiver proof、完整外部请求与 winner commit
scoop-mir       representation-neutral type/callable/slot bridge
scoop-mir-lower 外部构造、ensure、dispatch 和 helper 的机械 lowering
scoop-lir       layout/ABI/scan/descriptor DTO、proof、wire 与 verifier
scoop-lir-lower 唯一 target layout producer、外部 proof 消费与 root/place plan
scoop-codegen   完整 LIR -> LLVM/object，不推导语言行为或缺失布局
scoop-slib      section/profile、原子 closure 验证、双 view 与 publication
scoopc          selected metadata 投影、stage 编排
scoop           accepted profile/cache key 更新
runtime         既定 shape ABI 下的 box/array/scan 执行与防御验证
```

不新增 stage 实现间依赖。导入算法需要的通用验证逻辑放在对应 IR crate；不能让 `scoop-slib` 调用 `hir-lower` 或 `lir-lower` 来重编译上游。target-aware layout 的纯数据验证与重放 API 由 `scoop-lir` 提供，lowering 调用相同构造器。

输入链扩展为：

```text
ValidatedArtifactClosure<Compile>
    -> ValidatedCrossConeSemanticClosure
    -> ImportedTypeSemantics + ImportedInheritanceSurface
    -> HIR winner + CrossConeUseSet
    -> SelectedMirTypeBridgeSet
    -> local MIR + SelectedLirLayoutAbiSet
    -> complete local LIR + external typed definitions
```

`SelectedMirTypeBridgeSet`、`SelectedLirLayoutAbiSet` 是经过 dependency closure 检查的 branded handle，不接受裸 table、symbol 或任意 exact-id 列表。MIR 无法读取 default/import/access 语法，LIR 无法读取 Export HIR body，codegen 无法重新查询全部 dependency。

## 3. profile、section 与 wire 演进

### 3.1 新 production profile

新增：

```text
org.scoop-lang.slib-profile/cross-cone-layout-strong/1
```

其 required inventory 从 M23-5 `cross-cone-semantics-strong/1` 出发，移除 `org.scoop-lang.lir/strong-production/1`，替换为 `/2`，再加入下表前四项；`code_requirement`、`runtime_requirement`、publication、decode-cost model、extra-section policy 和 Link proof policy 原样继承，`odr = RejectAll`。

| capability | location | required_for | sinks |
| --- | --- | --- | --- |
| `org.scoop-lang.hir/cross-cone-type-semantics/1` | HIR | Compile | Hir |
| `org.scoop-lang.mir/cross-cone-type-bridge/1` | MIR | Compile | Mir |
| `org.scoop-lang.lir/cross-cone-layout-abi/1` | LIR | Compile | Lir |
| `org.scoop-lang.lir/cross-cone-layout-link-closure/1` | LIR | Link | Code + LinkValidationOnly |
| `org.scoop-lang.lir/strong-production/2` | LIR | Compile、Link | Lir + Code + RuntimeImage |

新 HIR section 同时承载 type facts 与独立 inheritance surface；它不修改 `cross-cone-interface/1` 的 public-only record。前三条的完整 canonical inner bytes 分别进入对应 layer contribution。新增Link-only section仅以 semantic physical-import projection 进入 Code，member/range/patch 信息只作 LinkValidationOnly；strong-production/2沿用强定义section自己的三个sink。

所有 source Cone、trusted core、single-file 与 cache 产物都写新 profile、四个新增section及strong-production/2，空集合也必须显式编码。旧 profile 可以被 Graph view 识别并报告，但不能成为本阶段 completed dependency；core receipt、compiler compatibility 与 cache key 绑定新 profile fingerprint，全部重建，不做内存升级。section自身major与outer schema是两个版本维度；本次strong-production/2不表示进入M24的outer schema2或runtime release ABI。

### 3.2 不改义的既有 section

- identity-foundation 的布局/scan/dispatch key 继续只证明 identity；本阶段新 payload 引用它们，不重复声明同 kind/id。
- `NativeBoundaryTypeDefinitionRecordV1` 只服务原 extern/callback witness，不通过它提供一般 field/scan/TD 查询。
- core 既有 shape-support/bridge 仍由 core 专属字段拥有；新 section 为它补充完整表示证明并逐字段校验，不能形成另一份可独立修改的 core authority。新 `shape_support` 表只存 ordinary producer 的八 role，core 中必须为空；通用查询对 core 委托旧 `core_shape_support`，对 ordinary 委托新表，二者返回同一只读接口。
- M23-5 最大 core-closed callable export 集不变，ordinary dependency 的该子集仍走旧 MIR/LIR bridge 和旧 Link 分区。
- 新 callable bridge 只承载上述旧集合以外、现在可证明的 target；同一 callable 不能同时登记在旧、新 external arena。完整类型证明可以被两类 bridge 共用。分区优先检查冻结的 core bridge，其次检查 M23-5 ordinary bridge，剩余 target 才进入新 bridge；新开放的 core member/constructor/shape use 若不属于旧 bridge 的固定集合，也走新 bridge，不能借此扩大旧集合。
- `strong-production/1` 保留旧格式和验证规则，新profile不再生产/要求它。`strong-production/2`保持既有top-level十字段及identity、definition plan、digest DAG、core shape/bridge、image plan结构，只把TD/dispatch语义中无法表示ordinary provider的引用sum版本化；runtime registration/image的C ABI不改变。

本阶段不提升 container/outer schema，不改 `persistent-v1` mangler，不重分配既有 tag。新增 mandatory section 使旧 reader fail closed。

`strong-production/2`中的三个版本化constituent固定为：

```text
StrongTypeDescriptorRefV2 =
    Local(PersistentExactTypeId)                      // tag 1
  | CoreExternal(PersistentExactTypeId)               // tag 2
  | DependencyExternal { provider, exact }           // tag 3

StrongTypeDispatchCallableRefV2 =
    Local(PersistentCallableBodyId)                  // tag 1
  | CoreExternal(PersistentCallableBodyId)            // tag 2
  | Runtime(RuntimeFunction)                         // tag 3
  | DependencyExternal { provider, body }            // tag 4

OptionalStrongTypeDescriptorRefV2 =
    Absent                                           // tag 1
  | Local(PersistentExactTypeId)                      // tag 2
  | CoreExternal(PersistentExactTypeId)               // tag 3
  | DependencyExternal { provider, exact }           // tag 4
```

前述既有variant的payload逐byte复用V1实际编码，包括optional Absent的`{0:1,1:0}`；新增variant是`{0:tag,1:provider,2:exact_or_body}`。body是`PersistentCallableBodyId`，必须反向join同provider已验证的Strong owner、ABI export与definition，不能只比较symbol。TD parent、itable interface key、vtable/itable entry及引用它们的type-registration semantic plan统一使用V2，不保留另一个可矛盾的V1 plan。DependencyExternal必须由新layout/ABI bridge和Link import逐项证明；不允许把真实parent写成Absent再由sidecar补齐。core原ref仍按旧分区使用CoreExternal，新增core能力按3.2的分区规则使用新ref。descriptor/dispatch dependency fingerprint使用同一完整V2 relation；旧variant的canonical bytes保持，新variant追加tag，不能遗漏provider或把foreign target编码成local。

其他strong-production constituent以及runtime registration的typed key/record不变。Compile/Link handler先按capability解码V1或V2，再取得不可降格的validated production view；旧Link identity section只消费其既有子集与同一member全集，新physical use由11.3覆盖。profile拒绝同时出现两个strong-production版本，避免两套definition authority。

V2另扩展initialization dependency的验证域：wire仍是原顺序/排序契约下的`PersistentInitializationUnitId`序列，不改变id或record字段；内存证明改为`LocalUnit(ref) | DependencyExternalUnit { provider, unit_ref }`。旧V1只能解析本Cone producer unit的规则保留。V2的foreign id必须命中同一显式dependency closure中已选中的provider unit、对应initialization descriptor与required definition，producer unit表仍只含本地定义，不能复制foreign canonical unit record冒充本地。missing/错provider/跨closure dependency在artifact commit前拒绝；不允许省略真实edge来让旧validator通过。

### 3.3 编码约定

本文新 record 使用 M23-2 Wire CBOR v1 closed product：field 从 1 开始按文中声明顺序编号，sum 使用 field 0 的 tag，payload field 从 1 开始；新增 sum 的 tag 按列出顺序从 1 递增。所有引用既有类型的地方复用原编码，不重新给 `Effect`、`GcEffect`、exact key、symbol request、definition plan、scan fingerprint 或 canonical ABI 编号。

table 按 kind-specific typed 主键的 canonical bytes 严格递增；set 排序去重，重复输入拒绝而非 reader 自动修复。参数、字段、variant、base prefix、slot position 是有序语义序列，不按名称重排。各 constituent 使用 foundation 的 text/bytes/count 限额和 deterministic logical budget；checked arithmetic、depth/cycle 与展开成本检查在分配前完成。

文中 `Checked`、`Selected`、`Ref`、`Witness` 表示构造完成后的内存类型，不把 arena index 或“已验证”布尔值写入 wire。wire 只保存重建这些证明所需的 typed id 与 canonical facts；reader 重建后才返回 handle。

## 4. HIR：type facts 与继承接口

### 4.1 type semantics section

```text
CrossConeTypeSemanticsSectionV1 {
    exact_facts: CanonicalVec<ExactTypeFactsV1>,
    representation_support: CanonicalVec<NominalRepresentationSupportV1>,
    inheritance: CanonicalVec<NominalInheritanceInterfaceV1>,
    protected_declarations: CanonicalVec<ProtectedDeclarationInterfaceV1>,
    protected_source_interfaces: CanonicalVec<ProtectedSourceInterfaceV1>,
    protected_defaults: CanonicalVec<ProtectedDefaultTemplateV1>,
    definition_sources: CanonicalVec<ExportDefinitionSourceV1>,
    selected: CanonicalVec<SelectedExternalTypeUseV1>,
}

ExactTypeFactsV1 {
    exact: PersistentExactTypeId,
    kind: Value { zst: ZeroSized | NonZero } | Reference,
    gc: GcFree | ContainsManagedReferences,
}
```

field7 `definition_sources`精确等于fields2～6全部inline definition origin的去重并集，包括representation/access、inheritance constructor与slot root/implementation、递归nested source support（含setter及const）、source parameter、default root/local/body/reference来源。reader穷尽typed constituent，不从候选field7反推使用集合；缺失与额外来源均拒绝。继承合同中的foreign origin保留真实provider，并由type-section独立authority逐项核对其已验证foundation/provider及实际使用关系；不能套用旧public section整表`current_cone`约束，也不能把foreign origin改写为当前Cone。field1的exact facts和field8的typed selected关系没有inline origin；selected的源码provenance另由其独立use authority重放。该闭包本身只证明来源完整性，不授予source lookup或selected资格。

这些事实覆盖本 Cone 导出的 param-free exact subject，以及其必须供下游检查的表示/继承 support；generic template 不能伪装成 concrete facts。`Reference` 的 `gc` 固定为 ContainsManagedReferences，描述的是该引用值；对象内部 `object_scan` 可以为空，二者不能混淆。`Value/ZeroSized` 必须是 GcFree。enum 每个 variant 的 gc flag另随 representation record保存，并重放 `gc_free(enum) = AND(gc_free(variant))`，不是对ContainsManagedReferences取AND。

value `ZstStatus` 在 HIR 完成：只有 Unit 或全 ZST 的普通非 CLayout struct/tuple 为 ZeroSized，intrinsic scalar/pointer 与 enum 不从空字段推断。MIR 机械转写；LIR 验证 status 与 layout 一致，不能用 `size == 0` 为缺失 HIR 信息补值。

`NominalRepresentationSupportV1` 的主键是 param-free source nominal 的 `PersistentTypeId`，保存 kind-specific 源码表示：ordinary struct 的声明序 `{ field, exact/signature type }` 和 CLayout policy，enum 的声明序 variant/payload 与 variant gc，class 的已解析 base 及声明序 backing/delegate field，object 的 backing-class 关系，intrinsic 的 typed representation family。本阶段不导出generic template的representation support，尤其不能为依赖binder的enum variant填入无条件concrete gc flag。字段仍可用既有`SignatureTypeKey`引用fully concrete的generic application；这些引用不能包含binder，选中请求闭包若需要ODR生产仍由M23-7 gate拒绝。generic template的表示实例化与对应物化证明留M23-7，不能增加Unknown/default flag。

其wire固定为 `{ 1: owner, 2: declaration_access, 3: shape }`。field type只保存一个`SignatureTypeKey`，不平行保存可矛盾的exact/signature字段；concrete消费通过已验证binder-free转换取得exact id。shape的tag1～6依次为：`Struct { fields, c_layout_policy }`、`Enum { variants }`、`Class { base, declared_fields }`、`Interface`、`Object { backing_class, declared_fields }`、`Intrinsic { representation }`。field是`{ field_id, value_type }`的declaration-order product，enum variant是`{ variant_id, fields, gc }`；source struct field与class backing/delegate field沿用既有`PersistentFieldId`，但必须经不同的checked wrapper隔离，并逐项验证foundation `FieldIdentityKey`中的`Declared`与`PropertyBacking`/`PropertyDelegate`角色；enum-variant-field使用独立的`PersistentEnumVariantFieldId`，三种字段用途不能混用。这里不新增persistent id家族或改变既有identity wire。CLayout policy与intrinsic representation复用对应IR的封闭语义模型并显式wire映射，不以annotation文本、短名或bool替代。

普通 public struct/enum 的 public representation 必须逐项等于 M23-5 source shape。class 的 private/internal backing field 仅供布局重放，不加入 `members`、import、default binding 或 source field lookup。读取 shape-support 的 authority 与源代码访问 field 的 authority 是两种不同 handle。

CLayout policy使用独立sum：tag1为`Ordinary`，tag2为`CLayout { contract }`，field1的contract复用HIR语义product`{ aligned, packed }`（field1、2）；各`HirCLayoutValue`使用无payload的sum，tag1～6依次为`Natural/A1/A2/A4/A8/A16`。intrinsic source representation family使用tag1～7的`Integer { signedness, width }/Boolean/String/Array/MutableArray/Ptr/FunPtr`；Integer的field1、2分别保存HIR signedness和width，二者均为无payload的sum，tag分别为`Signed=1/Unsigned=2`和`W8=1/W16=2/W32=3/W64=4`。family记录source nominal的封闭表示类别，实际generic application的element/pointee/function type仍由既有exact/signature key提供；不能从family省略应用参数或取得ODR生产能力。这些是新section constituent的显式编码，不修改native-boundary的CLayout/ABI编码。

Class shape的`base`复用既有`OptionalSignatureType`编码，`Absent`表示无class base，`Present`只接受binder-free的nominal或nominal application signature；其exact class关系在inheritance closure中重放。Object shape的`backing_class`是`PersistentTypeId`，必须逐项等于`GeneratedNominalKey::ObjectBackingClass { object: owner }`派生的identity，不能借此把object的source identity当作backing class identity。

### 4.2 inheritance surface

```text
NominalInheritanceInterfaceV1 {
    owner: PersistentExactTypeId,
    modality: Final | Open | Abstract | Interface,
    direct_base: NoClassBase | ClassBase { exact },
    direct_interfaces: CanonicalVec<PersistentExactTypeId>,
    domains: NominalAccessDomainsV1,
    constructors: CanonicalVec<InheritanceConstructorInterfaceV1>,
    slots: CanonicalVec<InheritanceSlotContractV1>,
    protected_members: CanonicalVec<ProtectedDeclarationRefV1>,
    slot_schemas: CanonicalVec<InheritanceSlotSchemaV1>,
}
```

该表是普通 public lookup surface之外的第二条接口。protected declaration record 携带与同类 public record 相同的完整 owner、source signature、source parameter/default interface、effect、modality 和 property/accessor 关系，但使用独立的 inheritance access proof，不能 cast 成 `PublicLookupAccessV1`。

`InheritanceConstructorInterfaceV1`使用三字段product：field1为`PersistentConstructorId`，field2为真实`DeclarationAccessSourceV1`，field3为完整`NominalSourceCallablePayloadV1`。它只保存当前param-free Class/Struct上声明为Public或Protected的构造入口，owner无binder、constructor own binder及slot关系为空；enum construction仍使用typed variant plan。Private/Internal constructor供provider本地初始化或委托使用时，由其body及Strong support闭包保存，不进入该foreign source构造surface。该product复用source signature叶子，不取得普通public lookup；Protected constructor须与同id的protected declaration逐项join。ctor参数/default协议与真实源参数表闭合，重叠的旧public元数据也须同值，不生成第二个声明identity。

继承表按owner exact严格递增，constructor序列按constructor id递增；完整record继续平铺field1～4的edges及field5～9的domains、constructors、slots、protected_members、slot_schemas。独立定义侧inventory决定所需owner、constructor及protected member集合，并提供真实callable signature/access/modality和已解析的slot实现选择；不得用正在读取的表反推这些全集或实现选择。slot合同须恰好覆盖该owner所有schema的slot并集，root和target各自与完整source callable合同join，再核对override实现选择。protected member引用与完整protected声明表的owner逐项相等，constructor由独立constructors字段承载。这些source/inheritance表的checked结果仍需在完整section中同default coverage、表示、selected及provider来源闭合后才能授予生产资格。

`NominalAccessDomainsV1` 分别保存 effective lookup、inheritance 和 slot contract 所需的域。persistent domain 是 `Empty | Conjunction(constraints)`，空 conjunction 表示 universal；constraint 仅为 `Cone(ConeIdentity)`、`File(SourceIdentity)`、`LexicalOwner(kind-specific nominal id)`、`SubclassesOf(exact class)`。约束按 tag/id 严格排序，owner intersection、不可居住性与包含关系由 reader 重放，不保存 session ClassId 或 visibility 数字大小。

每个protected/support声明都保存 `DeclarationAccessSourceV1 { declared_visibility, lexical_owners, definition_origin }`：visibility是独立closed enum `Public=1 | Internal=2 | Private=3 | Protected=4`，owner链按outer→inner顺序保存typed nominal id，origin复用Stage5 `ExportDefinitionSourceV1`。reader从foundation的真实owner chain/source与这份declared visibility重算域，不能只比较producer给出的两个计算结果。`NominalAccessDomainsV1`的三个字段依次为lookup、inheritance、slot；用途不同的域不能互转。

nominal的lookup由声明visibility与全部owner域重放；class的inheritance沿既有本地语义：Final为Empty，Open/Abstract为lookup与`SubclassesOf(self)`的交集；interface的inheritance为其lookup，struct/enum/object等不可继承nominal为Empty。nominal本身不是callable slot，因此该record的slot字段固定Empty，reader拒绝非Empty；它不保存member域的并集，也不覆盖任何member合同。每条`InheritanceSlotContractV1`仍保存自己的完整root-slot域，通过原始声明source独立重放。

`ProtectedDeclarationInterfaceV1`的tag1～4依次为`Callable`、`Constructor`、`Property`、`NestedNominal`，每个variant field1为对应kind-specific声明id、field2为`DeclarationAccessSourceV1`、field3为完整接口payload。Callable/Constructor payload按顺序保存owner、own binder list、receiver、parameter shapes、result、effects、modality、source interface、slot relations；constructor own binder固定为空，signature scope压缩规则复用Stage5。Property保存owner、value type、getter、`ReadOnly | ReadWrite { setter, setter_access }`、representation与slot relations；NestedNominal保存source nominal id及其inheritance/representation表引用。protected owner必须是class；其visibility为Protected，随owner收窄后的effective domain仍须允许该inheritance路径。不可lookup的internal concrete slot只出现在`InheritanceSlotContractV1`的support target关系，不借此表取得protected声明身份。

`ProtectedDeclarationRefV1`使用同一tag1～4及field1的kind-specific id作为声明表的canonical key；Callable仅接受Function、GenericFunction、Accessor，Constructor和NestedNominal分别保持`PersistentConstructorId`和`SourceNominalId`。完整record直接使用field0的tag及field1～3的四字段sum，不在field1外再嵌套已有三字段record。声明表和ref集合均按该key的canonical bytes排序、唯一；reader拒绝乱序，不修补。定义侧独立提供完整protected声明ref集合，source table验证先精确对照全集，再逐条验证source identity/access/signature及nested support，最后闭合property的getter和真实Protected setter；private/internal setter不要求也不得伪造为Protected callable。public property的protected setter可作为独立Callable根存在，不要求伪造对应Protected Property。该checked source table只证明来源、全集和accessor关系，不独立授予继承选择、default覆盖或生产资格。

Callable分支的声明字段复用既有`CallableTemplateOrigin`编码，但只接受Function、GenericFunction与Accessor；Constructor只使用独立constructor分支的typed id。Callable/Constructor payload为field1～9的product：`owner, type_parameters, receiver, parameters, result, effects, modality, source_interface, slot_relations`。owner使用`SourceNominalId`，binder、parameter shape、result、effects及modality分别复用Stage5对应constituent编码。这里是source signature：nominal member的implicit this只由owner提供，`receiver`使用既有`OptionalSignatureType::Absent`，reader拒绝Present；它与slot/MIR exact callable signature的显式receiver字段属于不同层。`source_interface`是独立closed sum：tag1为无payload的`AccessorNoSourceInterface`；tag2、3、4分别为Function、GenericFunction、Constructor，field1为对应typed declaration id并须等于外层声明。它引用本节protected source-interface表，不能被旧public source-interface索引解释。`slot_relations`为按canonical slot id排序且去重的序列，不表示dispatch位置；constructor固定为空，abstract/open callable须关联真实slot。generic source metadata的binder保留，class generic method按语言既有规则固定Final且不能关联slot；本阶段concrete生产仍受param-free gate限制。

Property payload是field1～6的product：`owner, value_type, getter, mutability, representation, slot_relations`；声明字段使用`PersistentPropertyId`，owner使用`SourceNominalId`，getter/setter使用独立`PersistentPropertyAccessorId`。mutability的tag1是无payload的`ReadOnly`，tag2是`ReadWrite`，field1、2分别为setter与完整`DeclarationAccessSourceV1`。representation复用Stage5 `PropertyRepresentationV1`，但class-owned protected property不能为Const；AbstractSlot必须携带slot关系。reader由foundation property/accessor key重放同一logical property及getter/setter role，再与source逻辑type、mutability、representation逐项join。getter的protected访问源与property一致；setter保留实际声明源，并按语言9.1.5及既有本地实现比较effective lookup域的包含关系，不比较visibility整数。protected setter须在独立protected callable表中闭合；private/internal setter不由该表取得protected callable身份。getter及可访问setter的完整source签名/effect和slot关系在相应表中继续闭合。

NestedNominal payload为field1～3的product：`source_nominal, source_interface, support`。source_interface使用独立`ProtectedNestedSourceInterfaceV1`，field1～9依次为`kind, modality, type_parameters, supertypes, constructors, members, children, source_shape, source_support`；kind/binder/supertype/source shape复用Stage5叶子constituent，modality复用`NominalInheritanceModalityV1`的closed wire并与kind及真实source声明逐项join，不能从kind或source key猜测generic class的Final/Open/Abstract。constructors是typed constructor id集合，members是独立的Function/GenericFunction/Property typed ref集合（tag1/2/3、field1为对应id），children是`SourceNominalId`集合。各集合与source_support按typed declaration key的canonical bytes严格排序且唯一；source_support key是与记录同tag的二字段sum（field0为tag，field1为typed declaration id），不含access/payload。完整section从定义侧权威source全集投射source_roots，source-only根只闭合source identity/词法链，不生成exact inheritance node或concrete能力。不使用`CanonicalPublicMemberRefsV1`或`PersistentExportBindingId`，不生成不存在的普通export binding。

source_support在nested constituent内部闭合：独立closed sum的tag1～4分别为Callable、Constructor、Property、NestedNominal，每条field1为相应typed declaration、field2为真实`DeclarationAccessSourceV1`、field3为完整source payload；nested child递归携带自己的完整source_interface。成员、constructor、child以及property accessor refs必须逐项命中同一容器中kind和owner均相等的support记录。support可保留declared-public/internal/private/protected的真实来源，effective域仍与全部nominal owner相交；记录存在性不授予lookup资格，也不能把declared-public但owner受限的member伪称Protected塞入外层protected声明表。callable signature、source parameter/default、effect、modality、property/accessor关系均由其source payload与独立protected source/default表闭合，不复用旧public表解释这些引用。

support的Callable/Constructor payload复用本节九字段source signature constituent，并由独立support wrapper验证实际nominal owner及声明access；interface owner按既有语言规则允许InterfaceDefault/Abstract，private helper只能Final且无slot，interface method不能有own binder。外层Protected Callable仍只接受class声明，不能因此接收InterfaceDefault。support Property payload为tag1的`Runtime { interface }`或tag2的`Const { value }`：field1分别保存六字段source property constituent，或既有`ExportConstValueV1`四字段const constituent（property、type、typed value、definition origin）。Runtime保留真实getter/setter访问源并验证owner种类；Const仅允许object/companion owner，property identity及origin必须与外层support相等，不携带伪getter、setter或slot。这里复用的是const值语义，不经旧public property interface取得资格。

Nested support的Callable分支另接受既有`CallableTemplateOrigin::VariantConstructor` typed variant id；其九字段payload使用独立`NominalSupportSourceInterfaceUseV1`，tag1～4逐项对应上述accessor/function/generic-function/constructor协议，新增tag5的field1为`PersistentEnumVariantId`。variant的own binder为空、receiver为Absent、执行为Ordinary、modality为Final且无slot；host binder来自对应enum，参数及顺序逐项等于同一enum source shape的variant字段，result为该enum source type/application。reader必须用foundation `EnumVariantIdentityKey`与source enum owner证明identity和访问源，并把tag5协议同该variant的source parameter/default接口join；不能伪造nominal/function `SourceDeclarationKey`或把variant当普通constructor。外层Protected Callable/Constructor及`ProtectedSourceInterfaceUseV1`仍拒绝variant，不扩其closed wire。

两种support分支共享上述完整source层：tag1为`ParamFree { inheritance_exact, representation_owner }`（field1、2分别为`PersistentExactTypeId`和`PersistentTypeId`），必须指向同一source nominal在inheritance/representation表中的记录；tag2为无payload的`GenericTemplate`，保留generic source identity、binder与owner语义，但不持有concrete表引用，也不授予M23-6 concrete生产能力。两分支均须独立验证canonical source key、完整词法owner链和source_support闭包；source接口constituent不得携带或转换成普通public lookup资格。GenericTemplate及处在generic词法owner中的source property只保存并校验真实setter声明源、逻辑type、accessor合同和定义侧完整shape，checked结果显式为GenericSourceMetadata；它不伪造`SubclassesOf(exact)`，也不以visibility整数替代effective域比较。只有全部相关owner均有param-free来源的property才产生ParamFreeDomains证明并重放getter/setter effective域包含关系；generic实例化时的权限重放属于M23-7，当前M23-6 gate不得据source-only证明选择或物化该generic目标。

public property带protected setter时，旧public property仍保留`Restricted`，新表只以Callable分支登记该setter的`PersistentPropertyAccessorId`和protected access；不把整个property改成protected，也不向旧public callable表添加setter。consumer先选中public getter/property，再通过两表的同一property/accessor key取得setter witness。

`InheritanceSlotContractV1` 保存：既有 `PersistentDispatchSlotId`、声明 owner、function/getter/setter 的 typed declaration origin、完整 signature/effect、slot contract domain，以及 `Abstract | Concrete(target) | InterfaceDefault(target)`。每条 slot 与 canonical slot key 关联；override 引用原 slot identity，不以相同方法名或新 owner 重造 base slot。

slot合同wire为field1～7的product：`slot, declaration_owner, declaration, signature, domain, implementation, declaration_access`。本阶段source owner为param-free `PersistentTypeId`；declaration使用`Function/Getter/Setter`（tag1/2/3、field1为对应typed id），getter与setter须分别匹配foundation accessor key及slot role。signature为`{ exact_signature, effects }`（field1、2），复用既有`ExactCallableSignature`与`CallableSourceEffectsV1`编码，双方execution必须相等，receiver必须存在且精确对应source owner。function参数与foundation的binder-free source signature逐项join；getter无显式参数，setter恰有一个参数并返回Unit。implementation为tag1的`Abstract`或tag2/3、field1含target的`Concrete/InterfaceDefault`。target为field1～5的`{ declaration, owner, signature, modality, declaration_access }`，保留实现自身的source owner、完整签名/effect、modality与source access，不能用root-slot receiver冒充实际实现receiver；target不能为Abstract，InterfaceDefault分支只接受interface default modality。原始slot与target的参数/result/execution以及安全性、GC、operator、infix合同逐项相等；intrinsic与普通Scoop body的实现类别可以不同，source extern不进入dispatch support。这里的GC相等比较的是两个source声明的合同，沿用本地override的source attributes相等规则；5.2允许的NoGc value目标到Managed入口只发生在生成的boxing/dispatch adjust层，由两份lowered signature表达，不能据此接受GC source合同不同的override。调用的实际receiver/boxing或dispatch adjust由5.3的typed实现关系承接。

`NominalInheritanceInterfaceV1`按上述顺序编码field1～9，新增的field9为独立`slot_schemas`；合同表`slots`仍按slot id的canonical bytes排序。`InheritanceSlotSchemaV1`是`{ role, slots }`（field1、2）：role为`ClassVtable`（tag1）或`Interface { interface_exact }`（tag2、field1），slots是无重复且保留provider语义顺序的`PersistentDispatchSlotId`序列。schema表按role的canonical bytes严格排序。该序列只定义该exact owner及typed table语境中的dispatch顺序，不保存byte offset、LLVM类型或程序级全局ordinal；同一slot可以出现在多个interface schema，不能为slot合同附加一个缺少table语境的position。interface自身schema与class/value/object实现该interface的schema逐项join同一provider的合同及顺序；derived的ClassVtable保留base完整slot序列为prefix，override只替换已有slot的实现关系，不改变slot identity或位置。MIR按每条序列的顺序枚举position，再依5.3逐项join合同signature与implementation。

必要的 internal concrete slot 可以作为不可 lookup/override 的继承 support 保留，使下游 table 保留其已有实现。private member 永不进入继承/dispatch；public owner 不得留下下游不可实现的 hidden abstract obligation。internal/private concrete owner 中的 public override 可以填充更宽 slot，但不因此成为普通 foreign lookup target。

公开继承入口及其传递 base/interface/slot closure由定义方完整导出。protected nested type 同样保留自己的 identity 与 owner chain，通过合法 subclass scope取得访问证明；不能通过 `public import` 再导出成普通 binding。

### 4.3 protected access 与 default

HIR 依次证明：

1. base/interface 来自合法 source route，或者是已授权 inheritance relation 的传递 support；support artifact 不因此成为普通 import 来源。
2. 当前词法访问位置处于声明 class 或其 subclass body；有效 owner domain 同时满足。
3. 显式 receiver 的 exact 静态 class 是当前访问 subclass 或其 subclass。仅“运行时对象可能是 derived”不够；`Base` 静态 receiver 不获准访问 Derived 作用域中的 protected member。
4. constructor delegation、implicit `this`、explicit receiver、qualified `super` 分别产生用途专属 witness，不用一个可任意复用的 `is_protected_allowed` 标志。
5. property 先选 getter/logical property，随后检查 setter domain。setter 不可见立即报 assignment 错误，不回退其他 overload。

词法检查沿foundation验证后的owner链查找实际授权的class/subclass，witness保存这一个exact access class；它可以是static nested或companion的外层class，不要求最内层owner本身为class。显式receiver仍须是该access class或其子类，且nested/companion不因此获得implicit outer `this`；这一回放沿用语言9.1.5及M21设计5.1的既有授权范围。域相交保留所有独立约束：两个`SubclassesOf`不能仅因class单继承且彼此无subtype关系就判为Empty，因为合法词法owner链可能分别提供两个授权class。只有Cone/File/词法owner约束本身矛盾，或某个必需的subclass区域在已验证且封闭的词法区域内不可能满足时，才归约为Empty；包含关系也须按同一词法/继承语义证明，不能将wire次序当作visibility大小。

object依语言9.1.3直接继承class时，其body也是9.1.5的subclass访问scope，能使用自己的`this`或满足同一规则的显式object receiver访问继承protected成员。witness分别保存source object exact与generated backing-class exact；两者须通过该object的representation record、foundation `ObjectBackingClass { object }` canonical key和exact key逐项join，不能把source object id当作backing class id。继承/静态receiver关系沿source object语义边验证，物理class身份由该checked对应取得。protected声明owner仍只允许source Class；没有直接继承相应class的companion只能沿外层class取得授权，并且没有implicit outer `this`。

inherited protected callable 的 default 继续在定义处解析，使用 M17 的 kind-specific export-interface ref 和完整 call-domain coverage。consumer 只在合法 winner 提交后实例化；不把 private/internal hidden dependency装入 default，也不把 protected reference转换成 universal public witness。M23-5 的 public default wire 原样保留，新的 protected source-interface record单独引用相同表达式语义 constituent。

具体使用独立 `ProtectedDefaultTemplateV1`：field1～10和field12逐项复用Stage5 `ExportDefaultTemplateV1`的key、definition root/path、locals、纯body、result、suspend、binder mapping、receiver、前置参数与origin；field11替换为`ProtectedDefaultReferenceSetV1`。不得把整个旧template/ref-set直接复用，因为旧witness只允许public/universal domain。

新的reference set仍是六个按`(target, definition_origin)`排序的kind-specific集合：callable、constructor、type、global、singleton、field；target类型逐项复用Stage5，record为`{ target, definition_origin, witness, uses }`。同一target的不同origin分别保留；相同target与origin只允许一条record，即使witness或uses不同也视为重复，不靠合并修补。witness采用下述封闭sum；其ParamFree分支保留`{ owner, direct_call_domain, slot_call_domains, target_domain }`，domains使用本节persistent域；slot_call_domains是按slot id严格递增的`{ slot: PersistentDispatchSlotId, domain: PersistentSlotContractDomainV1 }`二字段product序列，逐项覆盖该source callable关联的全部root-slot，constructor及不能参与dispatch的generic callable保持为空。uses精确保留每个附着于Expression的实际引用occurrence的`{ expression_index, receiver_use }`，按expression_index及receiver_use的canonical编码排序且唯一。expression_index是body按wire字段序、source sequence序前序遍历的Expression节点u32序号，不是arena index或新persistent identity；receiver_use是`None | ImplicitThis | Explicit { receiver_expression_index } | ConstructorDelegation`。

局部type、statement binding、pattern、iterator protocol及其他非Expression节点的引用不伪造expression_index，也不生成uses元素。reader仍由完整typed body visitor逐个重放其target、definition/evaluation origin、词法访问scope和实际receiver；protocol或binding receiver只可来自已验证typed plan，不能一律按NoReceiver处理。六域reference闭包必须精确等于Expression引用与这些metadata引用的并集；只有metadata引用的record允许uses为空，但仍须通过全部target、来源和整个default call-domain coverage检查。空uses不得作为跳过依赖、访问或coverage的条件。

`ProtectedDefaultAccessWitnessV1`的tag1为ParamFree：field1～4依次为owner、direct_call_domain、slot_call_domains、target_domain；direct与target均使用`PersistentLookupDomainV1`，slot使用独立`PersistentSlotContractDomainV1`，每个domain必须由实际source/slot合同独立重放，不能把wire DTO当作domain来源。tag2为GenericSourceMetadata，仅有field1 owner；owner在两分支都必须等于完整template key的owner。GenericSourceMetadata只在已验证source的完整词法owner链包含GenericTemplate且独立定义侧将该完整default分类为generic metadata时成立，不能由wire tag、outer qualifier或callable自身binder列表自证。定义侧从完整source/default语义闭包决定profile，reader逐项join该预期分类；artifact不能任意选metadata分支降级应可验证的param-free default。依语言9.1.3，static nested没有outer type parameter：直接source owner为Concrete、外层generic只作为qualifier且实际domain与传递依赖均可证明时，仍使用ParamFree；实际需要未具体化generic owner/default域时才保留metadata。param-free nominal下仅函数自身generic时仍用ParamFree，不允许借metadata分支跳过可证明的coverage。

完整default先从独立checked callable source取得owner/profile证明，即使body没有外部引用、六域reference set全部为空，也不得跳过该分类。reference访问重放仅接收实际typed occurrence、真实receiver上下文及该source证明；独立authority先验证target来源和词法/receiver规则，再返回与同一checked inheritance graph关联的target域，不能读取witness所声称的target域作为预期值。每个实际occurrence随后与wire witness逐项join，并验证完整direct及每个root-slot调用域的coverage。

两分支都重放完整body、binder、source origin、typed target、receiver及源码合法性。GenericSourceMetadata只推迟无法定义的concrete-domain包含证明，不保存假exact或Empty/Universal域，且不产生concrete access资格；所有selected、default展开和物化入口必须显式拒绝该分支。其存在只使GenericTemplate的源元数据在本节完整保存，M23-6的param-free执行门限不变。metadata proof与可执行的ParamFree coverage proof使用不同checked类型，不提供隐式转换。

`ProtectedDefaultExpressionUseV1`是二字段product：field1为`expression_index: u32`，field2为`ProtectedDefaultReceiverUseV1`。receiver_use的tag1、2、3、4依次为None、ImplicitThis、Explicit、ConstructorDelegation；只有Explicit分支另有field1 `receiver_expression_index: u32`，其余为仅tag的sum。canonical uses按expression_index及receiver_use的tag/内部index严格递增，producer拒绝重复，reader拒绝重复和逆序，不修补输入；序号范围、receiver种类与完整body的精确对应由独立visitor回放。`ProtectedDefaultSlotCallDomainV1`的field1、2为slot与domain，canonical表按typed slot id严格递增；同slot重复即使domain相同也拒绝。以上集合仅是传输数据，空集合不证明无receiver或无root-slot，完整template必须对照实际body及source callable的独立全集。

receiver_use从实际typed节点作唯一投射：member field/method（含direct-super）引用的receiver若为该模板receiver local，使用ImplicitThis；其他实际receiver expression使用其完整body前序编号的Explicit。type、global、singleton及无receiver的ordinary call引用使用None。Stage5的ClassInit表示普通构造表达式，不证明ConstructorDelegation；在没有真实delegation typed节点的本节default body中声称该tag必须拒绝。metadata引用保留其完整typed statement/iterator/binding上下文供独立source、origin、domain及receiver校验，不通过None抹除真实receiver。精确body/ref-set闭包只证明出现关系，不能单独产生访问、default展开或selected资格。


reader重放完整body/reference闭包、definition-before-use及各receiver的静态type；每个occurrence必须命中相同typed target并满足4.3的词法/receiver规则。target domain必须覆盖direct call domain和每一个slot call domain；domain不是两个可比较visibility整数，也不能只覆盖当前某一个consumer调用点。default provider、参数位置、完整binder映射和definition/evaluation origin仍按Stage5规则验证。继承或扩大override调用域时重新检查coverage，不可把protected dependency藏进public default；constructor默认表达式仍遵守初始化receiver禁用规则。

protected source-interface表额外保存其default template集合及definition_sources精确闭包，使用独立索引空间；不能把protected key插入旧public source-interface/default表。表的source parameter形状和省略类别复用Stage5规则，template引用由当前protected表的checked key/index解释。

protected source-interface的owner全集由已验证的protected declaration surface（递归包含Nested的完整source_support）和inheritance constructor surface投射，不由candidate协议/default表反推；各source-use中AccessorNoSourceInterface不生成协议行。跨surface出现同一constructor时，其完整source signature、access、binder、effects及source-use必须同值，才能共用一条协议；同identity的不同payload不得覆盖或合并。Public inheritance constructor使用其payload明确携带的独立协议，重叠的旧public source接口仍须同值。这个全集仅证明source metadata支持，private/internal成员不因协议存在而获得lookup资格。

protected source-interface表是按`CallableTemplateOrigin`的canonical编码排序且唯一的record序列；每条record的field1、2为owner与声明序parameters。owner只允许Function、GenericFunction、Constructor、VariantConstructor，Accessor使用无source-interface协议。parameter的field1～4为name、value_type、calling、definition_origin；calling的tag1～4依次为Required、Default、VarargEmpty、VarargDefault，字段与Stage5叶子wire相同，但default位置只由独立`ProtectedDefaultTemplateIndexV1`解释。semantic key使用独立`ProtectedDefaultTemplateKeyV1 { owner, parameter_position }`（field1、2），不引入新persistent id，不向旧public key/index提供隐式转换。顶层protected_defaults按该key排序，source-interface的每个省略位置恰好引用同owner/position的template，全部template都须可由该独立source表到达；definition_sources由完整section对全部来源取精确闭包。读取方逐项对照已验证source callable签名、真实省略/vararg类别和parameter origin；不借旧public callable interface作为protected签名authority。

在本阶段，access bridge 指“source target → checked inheritance/receiver witness → MIR external target”的关系。只有已有 `DispatchAdjust`/`BoxingAdjust` 等语义确实要求时才生成 thunk，并使用既有 generated identity；不为绕过 visibility 新增 public wrapper 或新 persistent id 家族。

### 4.4 HIR selected set

`SelectedExternalTypeUseV1` 保存 terminal provider、exact/declaration target 与封闭的 use kind：`Signature`、`Representation`、`Construct`、`MemberCall`、`SlotCall`、`TypeTest`、`SingletonValue`、`Inheritance`、`ShapeSupport`。涉及 declaration 的分支携带相应 kind-specific declaration ref；slot 分支携带 exact receiver 与 slot；inheritance 分支携带当前 derived owner 和 direct base edge。

`CrossConeUseSet` 对 type、constructor、member/accessor、slot、TD、object ensure/value 增加独立 typed request 家族，不把所有请求塞入 callable id。source use 的 lookup/access provenance 保留在 HIR；由 lowerer 产生的表示/dispatch support edge 携带选中语义 parent，不伪造 import route。

capability gate 在 winner commit、default expansion 和 persistent materialization 前完成整个请求闭包。缺 source access 是语言错误，缺必需 section/record 是 artifact 错误，闭包需要 ODR/native 能力则使用相邻阶段诊断。成功 `LocalConcreteHir` 只包含 local body 与 external complete ref，不复制 provider param-free body。

## 5. MIR：表示无关的 type/callable bridge

### 5.1 section 结构

```text
CrossConeMirTypeBridgeSectionV1 {
    types: CanonicalVec<ParamFreeMirTypeExportV1>,
    callables: CanonicalVec<ParamFreeMirCallableBindingV1>,
    dispatch: CanonicalVec<ParamFreeMirDispatchSchemaV1>,
    object_values: CanonicalVec<ParamFreeMirObjectValueV1>,
    shape_support: CanonicalVec<ParamFreeMirShapeSupportV1>,
    initialization_uses: CanonicalVec<SelectedExternalInitializationUseV1>,
    selected: SelectedDependencyMirTypeSetV1,
}
```

所有 export 由 provider 的当前 HIR/LocalConcrete 与 MIR 输出逐项 join 产生；re-export Cone 只保存 selected relation，不复制 terminal provider 的 export。

`ParamFreeMirTypeExportV1` 保存 `{ exact, origin, facts, representation, base_and_interfaces }`。origin是`SourceNominal(PersistentTypeId) | GeneratedNominal { nominal, role }`，generated role与foundation canonical key逐项相等，不伪装成source声明。representation 是 MIR 自有的 closed sum：scalar/intrinsic、struct、enum、class、interface、object backing 与 generated helper；field、variant、base、capture 使用对应 typed id 和 exact type。它包含可重放布局的完整顺序和 CLayout policy，但没有 byte offset、LLVM type、stride、scan 或 ABI pass mode。

该新 constituent 的 product 使用 field1～5，顺序与上述五项一致；type 表按 exact id 的 canonical bytes 严格递增。producer 可以对已验证 record 排序，reader 必须拒绝重复及乱序输入，不能替 artifact 修补。origin 的 tag1 是 `SourceNominal`（field1 nominal），tag2 是 `GeneratedNominal`（field1 nominal、field2 完整既有 `GeneratedNominalKey`）。exact key 必须逐项等于 `Nominal(origin.nominal)`，source 分支只能引用 parameter-free source declaration；generated 分支必须匹配同一 canonical generated key。`ObjectBackingClass` 的 key 由 HIR foundation 提供并保留在已验证 identity graph 中，其余有限 helper 同时要求存在于 MIR foundation，不能把 HIR-owned backing class 强行补进 MIR 的生成类型表。

facts 为 `{ kind, gc }`（field1、2）。kind 的无 payload sum tag1～3 为 `ZeroSizedValue/NonZeroValue/Reference`，gc 的 tag1、2 为 `GcFree/ContainsManagedReferences`。ZST 必须 GC-free，reference 必须含 managed reference；integer/boolean intrinsic 固定为 nonzero GC-free，空 ordinary struct 为 ZST，enum 保留 tag 因而 nonzero，enum 整体 GC-free 等于全部 variant GC-free 的合取。非空 aggregate 的完整事实由 HIR/LocalConcrete join 提供，不能按“字段数为零”分类 intrinsic，或以 type constituent 的局部核验代替该 join。

representation 的 tag1～10 依次为 `Intrinsic/Struct/Enum/Class/Interface/ObjectBacking/BoxedValue/CoroutineStep/CoroutineSlot/Object`。Intrinsic 的 field1 保存固定 param-free family；Struct 的 field1～3 为声明序 fields、CLayout policy、interior-mutable 的 0/1 unsigned 值；Enum/Step/Slot 的 field1 为语义序 variants；Class 的 field1、2 为 class kind 与 declared fields；Interface 无 payload；ObjectBacking 的 field1 为 declared fields；BoxedValue 的 field1 为唯一 payload field；Object 的 field1 为 backing exact。class kind 的 tag1～3 为 `Final/Open/Abstract`。field product 为 `{ field: PersistentFieldId, value: PersistentExactTypeId }`；variant product 为 `{ variant: PersistentEnumVariantId, fields, gc }`，其 fields 使用 `{ field: PersistentEnumVariantFieldId, value: PersistentExactTypeId }`。field/variant 序列不按 id 重排；重复 id、错误 owner 和 variant-field owner 均拒绝。

固定 param-free intrinsic 的 tag1～4 为 `Unit/Integer/Boolean/String`；Integer 的 field1、2 为 signedness 和 width，tag 与 4.1 一致。generic intrinsic family 不进入该 sum。CLayout policy 的 tag1 为 Ordinary，tag2 为 CLayout，后者 field1 保存 `{ aligned, packed }`；值的 tag1～6 为 `Natural/A1/A2/A4/A8/A16`。这只是源 policy，不携带目标布局；空 CLayout struct 被拒绝。base-and-interfaces 的 field1 保存 `None`（tag1）或 `Base`（tag2、field1 exact），field2 保存 provider direct-interface exact 的 canonical 集合，按 exact id 严格递增。该集合不承载 dispatch 顺序；后者独立来自 4.2 的 slot schema。base 只可属于 Class、Object 或 ObjectBacking，目标必须是 source class exact；interface 目标必须是 source interface exact，重复、乱序和自身关系拒绝。完整继承闭包及 object/backing 两侧关系的一致性由 section 的 HIR/MIR join 核验。

BoxedValue 的 payload exact 与 `BoxedValue { payload }` key 逐项相等，field id 必须由既有 box payload field key 派生。Step/Slot 的 variant 与 payload field id 复用既有 generated enum keys，严格保留 7.2 的两 variant 顺序。三个 helper 的 subject 只能是 source-root exact，不能递归触发生成闭包。closure environment、frame 及 callable/continuation adapter 的执行型 shape 在本 constituent 明确按相邻阶段 gate 拒绝；不以任意 Class 表示伪装导出，也不为这些 shape 新造 machine 字段。单条 record 与 table 的 decoded→validated 路径均在 identity 解析、集合分配和语义遍历前消耗相应预算。这里只建立可核验的 type constituent；完整 section、selected closure 与 production artifact 资格仍必须满足本章其余各节和 11.2。

`ParamFreeMirShapeSupportV1` 是上述有限闭包的表示无关索引，其新 product 的 field1～5 固定为 `{ source_nominal, exact, boxed, coroutine_step, coroutine_slot }`。后两项是必需的 helper exact；boxed 的 tag1 为 `Available`（field1 helper exact），tag2 为无 payload 的 `ReferenceNominalRequiresNoBox`。value 必须取 Available，reference 必须取后一分支；不得用可选项表示缺失能力。source nominal 与 exact 必须指向同一完整 source type export；三个 helper exact 分别 join 同一 types 表的 generated key、representation、payload exact 与 GC facts，Step 的 Completed 和 Slot 的 Value 的 GC facts 必须等于 source，另一个 variant 必须 GC-free。helper 不能充当 source，错误 role 或不同 source 的同形 helper 不能替代。

shape-support 表按 source nominal 的 canonical bytes 严格递增。ordinary producer 的每项 source authority 必须属于当前 provider；完整 section 用独立重建的 required source-root 集合核验精确覆盖，不能从本表反推所需全集。遵守 3.2，core 的新 MIR shape-support 表也必须为空，其旧 `CoreMirBridgeV1.shape_support_roots` 仍是唯一 root authority；完整 section 对旧 roots 与新完整 types 表重放相同的有限 helper 关系，不另存一份可修改的 root 清单。该 MIR 五字段索引不复制 7.2 的 LIR 八 role product，后者另行证明布局、scan、TD、registration 与 definition。

record 之间的查询可以借用本地完整表及已验证 dependency 表组成的只读索引，索引不编码进本地 export wire。type、callable、schema 索引分别使用 exact、Strong target、owner exact 的独立 key 空间；相同 key 出现在两个输入表中也必须拒绝，不能以“内容相同”吞并重复 authority。dispatch 的 base prefix、继承 interface schema 与 implementation 查询均使用这个完整视图，而待导出的 canonical schema 表只保存当前 provider 的 records。索引只承载已验证 constituent 的关系查询，不自行授予 dependency selection；完整 section 必须另将每个借用来源 join 到终端 provider proof，不能把任意表拼装为 Selected。

MIR 定义必需的 source-join 协议，由 `mir-lower` 从当前已验证 HIR/LocalConcrete 与实际 MIR 输出独立投射，reader 则从同一已验证 HIR/基础身份闭包重建对应合同。协议提供当前 provider、独立的 type/callable/dispatch/object export 全集、required source roots、逐项完整预期 record 及已提交 initialization-use 全集；这些查询不能从待校验的 transport table 实现。MIR 检查每张 canonical inventory 的精确覆盖、每条完整 record 的逐字段相等、source root 的当前定义 Cone 及有限 helper 闭包。该 source join 不代替终端 provider/selected 闭包，也不赋予单独的表或 source proof 生产 artifact 资格；最终七字段 section 必须同时持有两类证明。

完整 section 的语义引用使用独立 `MirTypeBridgeTargetV1`：tag1～6 依次为 `Type(exact)`、`Callable(Strong owner)`、`Dispatch(owner exact)`、`Object(value id)`、`ShapeSupport(source nominal)`、`InitializationUnit(unit id)`，均在 field1 保存既有 typed id/owner。该 sum 只是待闭合的语义目标，不是 Selected handle。selected record 是 field1、2 的 `{ terminal_provider, target }`；表按 provider 后 target 的 canonical bytes 严格排序且唯一，reader 不从该表反推 required use 全集。当前 checked source 的已提交用途及本地 exports 的全部语义边共同决定闭包，terminal provider 必须提供自己的完整 local export；facade 的 selected relation 不能代替 terminal export。旧 core/ordinary callable 分区按3.2优先核验，重复登记或仅凭 foundation signature 伪造旧 selected proof 均拒绝。

语义边收集逐项覆盖 representation 的字段、variant、base/interface 与 Object backing，callable 的两份完整 signature、role 与 generated relation，dispatch 的 owner/base/interface provider、原 slot declaration、实现与调用 signature，object 的 source/backing/ensure/unit，有限 shape family 的全部 exact，以及显式 initialization-use 的 local/dependency unit 与 object cause。它不读取 foreign body 推断调用图，也不把 property accessor cause 无条件再登记到新 callable 分区。字段/variant 的嵌套 tuple 只在 transient storage 上下文递归到 nominal 叶子；其它 structural/generic 应用以及 signature/独立目标中的 tuple 仍在 M23-7 gate 拒绝。每个 exact 查找、边、节点、排序与嵌套深度共享预算，重复的语义边归为集合，而 wire selected 的重复输入始终是错误。收集器只返回完整待闭合边集；source、终端 provider、旧 bridge 与实际初始化函数证明全部 join 之前不能构造 Selected。

完整 section 的 source 协议还独立提供 committed external-use 全集和当前 local initialization-unit 全集及两种 role 的逻辑签名；不从 selected wire 或待验证 unit 查询结果生成预期集合。unit proof 在内存中区分 `ProducerEmitted` 与 `ReaderSemanticReplay`：前者借用 Strong sealer 的真实 emitted roots，后者只由 checked HIR unit 来源、canonical generated roles 与 MIR foundation/Strong signature surface 重建语义合同。两者都验证当前 provider、ordinary Managed、无 receiver/参数及 Unit exact；reader 不声称已取得 machine body。现有 unit key 已唯一决定两个 role，七字段 wire 不另保存冗余 unit-role 表。

MIR section 保留 terminal dependency section 的借用及本地 source-join 结果，只有完整 export/required-use 递归闭包校验成功才构造本层 selected set。相同 terminal section 经 diamond 重复到达只访问一次；同 provider 的另一份输入 authority 不能覆盖或合并。canonical selected relation 仅包含实际需要的 foreign 目标，缺项、多项、错 provider、把本地目标写成 selected 或跨 request 使用 handle 均拒绝。该证明限于 MIR 语义；外部 machine arena 的最终资格还必须由同一 artifact closure 的 LIR/Strong V2/profile 验证取得，不从 MIR section 单独授予。

本节不遍历 callable body，因而不会把 body 内的旧 ordinary/core 调用重新登记为新边。slot declaration、dispatch 的 source implementation 及 adjust 的 source target 必须由 canonical source key 证明为 nominal member：直接 owner 是 param-free type，source receiver 不是 extension receiver；generated dispatch target 则仅接受已支持的 adjust/derived role。object ensure 仅指向同 unit 的 generated ensure。它们与旧 TopLevel/Extension callable 分区不相交；普通 accessor 的 initialization cause 仍只贡献既有 unit relation。将旧 callable 显式放入本节 export/selected，或把顶层/extension callable 伪装为 slot/adjust target，均拒绝。

### 5.2 callable 与 constructor

`ParamFreeMirCallableBindingV1` 保存 `{ source_or_generated_origin, implementation, semantic_signature, lowered_signature, lowering_role }`。implementation 只接受既有 `StrongCallableDefinitionOwner`；lowering role 为 ordinary、class initializer、struct value constructor、accessor、dispatch adjust、boxing adjust、object ensure/value 等已有 typed role，不根据 name 推断。

新 binding product 按上述顺序编码 field1～5。两份 signature 都使用新 constituent `{ exact, gc_effect }`（field1、2）：exact 原样复用既有 `ExactCallableSignature`，gc effect 是独立无 payload sum，tag1 为 `Managed`，tag2 为 `NoGc`。旧 exact-signature wire 不变。普通 callable、accessor 与 pure-virtual trap 的两份 signature 相等；class initializer 仅按本节规则转换 receiver/result；adjust 保留目标语义 signature 与 slot 调用 signature，并逐项检查参数、result、suspend 及 GC effect 的合法关系。foundation 的 callable signature 锁定 implementation 的 lowered exact signature；GC effect 另与 HIR 合同及实际 MIR function metadata join，不能从同形 LLVM 函数类型补出。

origin 的 tag1～4 是 `Function/Constructor/Accessor/Generated`：前三者的 field1 为相应 typed id，Generated 的 field1、2 为 generated callable id 与完整既有 canonical key。lowering role 的 tag1～10 是 `Ordinary/ClassInitializer/ValueConstructor/Accessor/DispatchAdjust/BoxingAdjust/ObjectEnsure/ObjectInitializer/PureVirtualTrap/DerivedEquality`。Ordinary/Accessor 无 payload；ClassInitializer/ValueConstructor/DerivedEquality 的 field1 为 owner exact；两个 Adjust 的 field1 为所调用的 Strong target；ObjectEnsure/ObjectInitializer 的 field1 为 object/companion initialization unit；PureVirtualTrap 的 field1 为原 dispatch slot。constructor owner必须逐项等于源码 owner chain 的最内层 nominal；trap implementation必须等于该slot的原 declaration owner。object value读取仍用5.4的已授权storage plan，不新造一个返回object的initializer身份。

NoGc signature 的receiver、参数及结果必须全部GC-free。两个Adjust允许GC-free value目标保持NoGc，而接收interface/class ref的wrapper为Managed；Managed目标不能适配为NoGc调用。非adjust角色不改变GC effect。constructor/accessor/object initializer/ensure/derived equality均为ordinary同步角色；object ensure/initializer保持Managed。callable canonical表按Strong implementation的既有kind/id顺序保存，reader拒绝重复或乱序；两份签名与role均不可缺失。该constituent核对canonical identity、signature和type facts；slot选择、真实body和GC effect的完整authority仍须经HIR/MIR生产join取得。

source signature 与 lowered signature不能合成一个字段：class constructor 的源码结果为 class value，MIR initializer 取得同一个 initializing receiver并返回 Unit；enum variant construction 可以只有 representation operation而没有独立 machine body，使用专用 construction plan，不虚构 callable definition。

class construction 固定为 exact allocation一次、同一 receiver direct调用 initializer；base/`this` delegation 不分配、不改 header，最派生 TD 从 allocation 起保持。abstract class可有供 derived调用的 initializer，但不能构造 allocation target。跨 Cone构造保留 base-before-derived、共同初始化一次、异常不发布结果及每次 call 后 receiver relocation。

普通 member、extension、value constructor和adjust thunk都执行按值 receiver/参数语义。`@InteriorMutable` 或 `addressOf(this)`可观察时必须有方法局部 copy；即使 physical ABI使用 pointer，也不能把 caller/box内存变成方法的可修改 `this`。

### 5.3 dispatch schema 与 table 构造

`ParamFreeMirDispatchSchemaV1` 按 exact owner 保存 class vtable schema 与按 exact interface排序的 itable schema。每条 entry包含 `{ slot, position, slot_signature, implementation }`；implementation是 `AbstractObligation { declaration, trap_target, receiver_adaptation } | DirectStrongTarget | InterfaceDefaultTarget | AdjustThunkTarget`。abstract分支沿用当前MIR的typed pure-virtual trap body，使用原abstract declaration对应的Strong callable identity与完整signature；它有明确fatal出口，不留下null/未解析function，也不授予源码direct call权限。

schema wire固定为field1～3的`{ owner, vtable, itables }`；vtable是tag1的无payload `NoClassVtable`或tag2、field1为有序entry序列的`ClassVtable`。itable为`{ interface_exact, entries }`，表按interface exact严格递增；interface provider保留以自身exact命名的schema。entry按上述四项编码，position为table内的typed u32序号，必须逐项等于语义序列的0至长度减1，不是persistent id或全程序ordinal。slot signature复用5.2的完整`{ exact, gc_effect }`。implementation按上述顺序使用tag1～4；Abstract的field1、2分别复用既有`DispatchDeclarationOwner`与`StrongCallableDefinitionOwner`，field3保存receiver adaptation；后三个分支的field1保存Strong target。DirectStrongTarget与InterfaceDefaultTarget另以field2保存无payload的`Identity`（tag1）或`ReferenceDispatch`（tag2）receiver adaptation。

Direct receiver adaptation沿实现规范2.9的同一object身份规则：Identity要求slot与target完整签名相等；ReferenceDispatch仅允许receiver不同，其参数/result/execution/GC完全相同，且从当前schema owner通过显式base/interface/Object→backing边分别到达两个reference receiver。两条规范路径由验证器重建：先取最短，再对同长路径的完整exact-id序列取canonical bytes最小者；不在wire保存任选路径。receiver相等时必须使用Identity，因此同一关系只有一种编码。真实value payload变换仍必须选择AdjustThunkTarget并由既有generated key与两份完整signature证明。值类型采用interface default时也保留BoxingAdjust：semantic receiver是default body的interface，lowered receiver是key指定的目标itable interface，body仅重解释同一个box；payload到default interface的可达关系由schema验证，不能以Direct/Identity分支绕过。

itable的slot signature receiver固定为该表的interface exact，class vtable则保留原slot declaration receiver；因此derived class base prefix保持完整签名不变，而继承interface的provider/consumer共享当前interface的调用签名。原declaration与该调用签名仅receiver可不同，须由当前interface到声明interface的typed路径证明。生产lowering须按这份table调用签名建立interface call ABI；当前本地callee signature不能仅因receiver物理形状相同就替代table合同。AbstractObligation也携带上述闭合receiver adaptation，把当前table receiver适配到原trap receiver，保留原Strong identity和完整lowered signature。

reader核对每个slot的原declaration callable signature、所选target的完整lowered signature以及adjust的目标semantic/lowered关系。schema表按owner exact canonical排序，并统一重放base prefix、interface provider的slot/signature序列及abstract obligation；这种MIR内部闭合检查不替代HIR source slot schema的原始顺序与override选择join。Object和ObjectBacking沿class-like vtable规则处理；有限BoxedValue/CoroutineStep/CoroutineSlot shape support不另造source dispatch schema，box的descriptor dispatch从payload语义schema与既有boxing adjust机械构造。

- derived vtable保留完整 base prefix；既有 slot的 position保持，override只替换 target；新增 virtual family按当前owner的方法声明序首次出现时追加，保持现有MIR语义顺序。
- interface保留既有“继承slot在前、当前声明slot随后”的schema顺序和去重规则，consumer调用携带interface TD + schema内position；不存在程序级global slot ordinal。按id查找的wire record table可以canonical排序，但position必须保留provider的语义序列，不能按id重排物理table。
- 每个 concrete owner必须填满 obligation；abstract class可以保留 obligation，但 LIR allocation仍拒绝 abstract identity。
- HIR完成“最近 class concrete override → 删除较不 specific interface candidate → 唯一 default或诊断”的选择；MIR只验证并机械构造，getter/setter分别选择。
- `super`、`super<I>`保存强制 direct target；不会因为 callee属于 open family重新走 table。
- value实现 interface时 table entry指向按 exact implementor/slot/target产生的 adjust thunk；ZST thunk产生 typed value与独立需要的 this token，无 payload load。

slot调用签名与实现签名不同的任何合法情况必须由已有 typed adaptation relation闭合，不能用 LLVM function-pointer bitcast消除差异。没有语义允许的 adaptation时，在 HIR拒绝 override。

### 5.4 object value 与初始化

`ParamFreeMirObjectValueV1` 保存 object-value identity、backing exact class、provider-owned unit、ensure callable和value读取入口/已授权storage关系。consumer按普通语义先 ensure，再取得值；不复制 singleton allocation、init cell、failure root或 backing storage。top-level/delegated property继续经唯一 accessor；“有布局”不授权直接读取 private backing field。

该object product按上述顺序固定field1～5；value为`PersistentObjectValueId`，backing为exact id，unit为既有初始化unit id，ensure为既有Strong callable owner。read-plan是tag1的`PublishedSingletonRoot`，field1保存source object exact；MIR只保留该已授权读取关系，LIR由同一source nominal机械派生既有`StaticStorageKey::singleton_published_root`，不在MIR引入offset或物理storage id。reader将value的canonical source key、source Object representation、generated backing key、Object/Companion unit、GeneratedCallableKey::Initialization(Ensure)和ObjectEnsure binding逐项join；没有返回object的虚构initializer。object表按value id严格递增且唯一，provider从canonical source key取得。

Strong MIR sealer必须逐项将每个local initialization unit的ensure/initializer函数与该unit的既有 `GeneratedCallableKey::Initialization` 两个role join，要求两者均为真实emitted Strong body、普通Managed、无receiver/参数并返回同一Unit exact。HIR-owned source callable materialization和MIR-generated relation按其真实来源读取，不要求HIR生成的initializer另出现在MIR-generated delta。sealed materialization plan保存这条完整typed关联，不能只保存unit arena id后让LIR猜函数。该局部函数证明不代替foreign unit的同一terminal provider、真实descriptor/cell和definition closure；后者仍由完整section及11.2闭合。

本阶段关闭 artifact中的所有 typed edge与registration关系，不执行全图 eager startup。M23-8会在登记全部image之后按这些既有unit关系启动，不回到名称解析补依赖。

当前initializer中的显式external ensure通过`SelectedExternalInitializationUseV1 { local_unit, provider, dependency_unit, cause }`记录；cause是`ObjectValue(object_value_id) | PropertyAccessor(accessor_id) | InitializationSupport(unit_id)`，必须由已提交的typed ensure语义产生。LIR selected set保留同一edge，strong-production/2按3.2验证foreign unit，不要求它出现在本地unit arena。这里只记录现有语义的真实ensure dependency，不读取foreign body做跨Cone调用图推断；普通external callable内部自己的ensure继续由provider负责。image/runtime使用canonical unit id解析这些edge，相关真实descriptor/cell relocation按11.2验证。

initialization-use product的field1～4按上述顺序保存；cause是tag1/2/3、field1分别为object-value/accessor/unit id的closed sum。canonical表按local-unit、provider、dependency-unit、cause的typed key顺序保存，拒绝重复。local-unit必须属于当前consumer，provider必须是dependency-unit的真实定义Cone且不是consumer。ObjectValue cause必须对应同一Object/Companion unit；PropertyAccessor cause必须对应同一top-level/extension property unit，或同一object/companion owner的成员property；InitializationSupport必须等于该dependency-unit。param-free表拒绝generic delegated application unit。该constituent验证identity及关系，完整section仍必须把每条use与已提交的typed ensure语义/真实MIR调用逐项join，不能由canonical key匹配推断foreign body依赖。

## 6. LIR：完整 layout 与 scan

### 6.1 section 与 authority

```text
CrossConeLayoutAbiSectionV1 {
    layouts: CanonicalVec<ExactLayoutExportV1>,
    descriptors: CanonicalVec<ExactDescriptorExportV1>,
    dispatch: CanonicalVec<ExactDispatchExportV1>,
    callables: CanonicalVec<ExactCallableAbiExportV1>,
    shape_support: CanonicalVec<ParamFreeShapeSupportExportV1>,
    selected: SelectedDependencyLayoutAbiSetV1,
}

SelectedDependencyLayoutAbiSetV1 {
    semantic_uses: CanonicalVec<LayoutAbiDependencyV1>,
    physical_imports: CanonicalVec<ExternalShapeLinkImportV1>,
}

LayoutAbiDependencyV1 {
    provider: ConeIdentity,
    target: LayoutAbiSemanticTargetV1,
}

LayoutAbiSemanticTargetV1 =
    Layout(PersistentLayoutId)                  // tag 1
  | Descriptor(PersistentExactTypeId)           // tag 2
  | Dispatch(PersistentDispatchTableId)         // tag 3
  | Callable(StrongCallableDefinitionOwner)     // tag 4
  | ShapeSupport(PersistentTypeId)               // tag 5

ExactLayoutExportV1 {
    layout: PersistentLayoutId,
    exact: PersistentExactTypeId,
    target: TargetProfileWireId,
    role: RepresentationRole,
    body: Value { storage: ValueStorageLayout, representation: ExactRepresentationLayoutV1 }
        | Instance { shape: TypeInstanceShape, representation: InstanceRepresentationV1 },
    scan: PersistentScanId,
    definition: StrongShapeDefinitionV1,
}
```

`selected`是field1/2分别保存`semantic_uses`与`physical_imports`的closed product；`LayoutAbiDependencyV1`的field1/2分别保存provider与target，target按上列tag编码且payload在field1。semantic表按`(provider, target canonical bytes)`严格递增。每项provider必须不是consumer，并精确命中显式dependency closure中唯一terminal provider的对应五张表；内存中的selected entry保留该terminal section引用和request-local brand，不能由wire relation、裸id或单张constituent table直接构造。

producer与reader都从同一份已提交MIR→LIR typed selection和完整本地LIR记录独立重算semantic闭包：layout递归跟随base/field/variant/array等内嵌layout引用；descriptor跟随value/instance layout、parent/interface TD及vtable/itable；dispatch跟随interface/owner type与每个target callable ABI；callable跟随receiver/parameter/result layout；shape-support跟随八个role对应的layout/descriptor/helper记录。metadata-only读取保留在`semantic_uses`即可，不因此产生relocation。每个跨provider递归edge都进入真实terminal provider，依赖section中的转发selected关系不能代替terminal记录；环、同target多provider、当前Cone回指、缺失/额外关系及旧core/M23-5分区冒充新selection均拒绝。

`physical_imports`就是11.2定义的canonical semantic-import projection，按`(provider, subject canonical bytes)`严格递增且唯一。它由实际machine use、strong-production/2引用及已授权object/init support独立收集，再与对应terminal semantic记录、Strong definition及旧分区逐项join；semantic use可以没有physical import，physical import不能只有symbol或definition而没有完整semantic/support authority。该数组与Link section field1及Code contribution逐byte相等，selected的brand和terminal引用不编码。

layout 表以 `PersistentLayoutId` 为主键，同一 exact 可以有不同 representation role 的多项。`TargetProfileWireId`、`RepresentationRole` 原样复用 foundation 的 `LayoutKey`：ManagedValue/CValue/NativeFunctionPointer 必须匹配 Value body，ManagedObject 必须匹配 Instance body；scan key 的 layout/role 同样重放。target 必须等于同 artifact 已验证 LIR projection，不能仅比较可读名称。`StrongShapeDefinitionV1` 复用 M23-3 的 semantic-id/definition-plan/symbol product。每项 layout/scan/descriptor引用 foundation中的既有 key；definition必须在 provider strong production中有唯一primary atom。

普通引用值的 Value body 是 managed pointer大小/对齐和单个 managed leaf；它指向的对象 field layout属于独立 ManagedObject layout 的 Instance body及object scan。`InstanceRepresentationV1`的tag1～5依次为`ClassObject { base_prefix, declared_fields, complete_fields }`、`BoxedPayload { payload_exact, value_layout }`、`InlineBytes`、`InlineArray { element_exact, element_storage }`、`AbstractReference`。class base prefix与新增字段只在ClassObject中拥有authority；complete_fields是从base prefix和declared_fields机械拼出的全序投影，reader逐项核对，不能自由提供一份不同的flattened list。不能把空 class误判为 size 0引用值。

scan identity的role矩阵保持既有规则：ManagedValue、CValue、NativeFunctionPointer均配InlineValue；普通ManagedObject配ManagedObject；intrinsic array的ManagedObject layout配ArrayElement，其canonical scan描述单element。array descriptor的object scan另从该element scan与length/data offset/stride派生，不能把element scan当作object-relative scan，也不能为方便改写已冻结ScanKey。BoxedPayload的inline scan来自payload value layout，object scan来自checked平移。

consumer只能通过 `ExternalExactLayoutRef`取得完整事实，并在本地 aggregate中重放外层布局。外部 layout/scan constant、TD和table全部保持 external definition；本地内联 field offsets不构成复制外部addressable constant的许可。

### 6.2 storage 与 field layout

```text
ValueStorageLayout =
    ZeroSized { alignment: NonZeroPow2 }
  | NonZero { size: NonZeroU64, alignment: NonZeroPow2, scan: RefScan }

ArrayElementStorage =
    ZeroSized { alignment: NonZeroPow2 }
  | Inline { stride: NonZeroU64, alignment: NonZeroPow2, scan: RefScan }

FieldStorage =
    ElidedZst { exact, offset: ByteOffset, alignment: NonZeroPow2 }
  | Stored { exact, offset: ByteOffset, layout: NonZeroValueLayoutRef }
```

这些 sum具有封闭 checked constructor；raw wire不能直接实例化 `NonZeroPow2`、scan或 field ref。现有 `ValueStorageLayoutV1::Inline` 对应这里的 NonZero，内部命名可保留；新 wire只使用本节规定的 variant语义，不能直接序列化 Rust enum。

普通 struct/tuple按声明序计算：ZST field的canonical offset为0，不推进 cursor；nonzero field按其alignment对齐cursor，再checked加size；outer alignment取全部field最大alignment，最后checked tail padding。空ordinary struct与Unit是0/1；全ZST aggregate为0/max-alignment。class以完整base instance size作为新增field cursor，不复用base tail padding，继承field offset保持不变；object alignment至少8，并包含16-byte header。这是本阶段统一的class ABI冻结：替换当前flatten全部base/derived fields再布局的做法，本地和外部base都使用同一prefix算法。仅含Int8的base size为24时，derived首个nonzero字段最早从offset24开始，不能占用旧算法的offset17；新profile重建与layout golden同步迁移。

`ExactRepresentationLayoutV1` 穷尽 scalar、qualified pointer、struct、tuple、tagged enum、niche enum与intrinsic value family；class/interface/function等reference value使用managed qualified pointer分支。每个field/variant记录持久 typed field/variant identity、exact type与对应storage，source field sequence逐项匹配MIR。Instance的ClassObject保存 `NoBase | BasePrefix { exact, layout, byte_size, alignment }`，证明prefix和provider导出完全相等。

enum继续使用已有tag宽度与分配规则；不新增discriminant elision。tagged enum保存tag、pure-value共享区和每个ref-bearing variant独占连续slot；construction清零全部value/padding/inactive slots后再写active内容。niche只适用于规范7.4的封闭同构形状；managed-ref niche有managed scan，raw/code-pointer niche无managed scan。`Option<ZST>`保持非零tagged layout。

`@CLayout`只接受已通过source predicate的具体字段，按既有aligned/packed契约重放，并与canonical C layout逐字段一致。每个实际materialized的本地CLayout都必须由同一checked C storage算法生成canonical layout contract，即使该类型未出现在native call边界；该contract属于CLayout自身的表示依赖，不新增native callable、pass classifier或边界witness。不得从一般Scoop layout反推出C pass classifier。所有size、offset、stride、alignUp使用checked内部machine scalar并受target capability约束。

#### 6.2.1 完整 layout record 的 canonical 编码

`ExactLayoutExportV1`固定为field1～7依次保存`layout/exact/target/role/body/scan/definition`的product。body的tag1为`Value`，field1、2依次为本节的storage与representation；tag2为`Instance`，field1、2依次为checked shape与instance representation。layout与scan的id必须重算并逐项匹配foundation中的既有`LayoutKey/ScanKey`，不能只检查id存在。definition原样复用`StrongShapeDefinitionV1<PersistentLayoutId>`的既有map3：field1为`semantic_id`，field2为`definition_plan`，field3为`PersistentSymbolRequest`；semantic id必须等于record的layout id。provider、唯一primary atom及layout/scan各自的物理definition关系从同一foundation重建，保存在checked proof内，不扩展该旧product，也不据此授予import权限。

value representation的variant严格互斥，tag与payload固定如下；每项列出的payload从field1连续编码。

| tag | variant | payload |
| --- | --- | --- |
| 1 | Scalar | `kind` |
| 2 | QualifiedPointer | `kind` |
| 3 | Struct | `policy, interior_mutable, fields` |
| 4 | Tuple | `elements` |
| 5 | TaggedEnum | `tag_layout, pure_region, variants` |
| 6 | NicheEnum | `pointer_kind, variants, payload_variant` |
| 7 | IntrinsicValue | `family` |

Scalar kind的tag1为`Integer { signedness, width }`，tag2为无payload的`Boolean`；integer字段沿用4.1的既有tag。QualifiedPointer kind是无payload的sum，tag1～3为`Managed/Raw/Code`。String、class、interface、object、managed function及intrinsic array的value只使用Managed；`Ptr`只使用Raw，`FunPtr`只使用Code；完整pointee或signature沿record的同一exact key核验，不复制第二份可独立修改的identity或signature。IntrinsicValue family只有tag1的`Unit`，其storage固定为0/1；Integer、Boolean及各pointer family不能再通过IntrinsicValue编码。compiler-owned machine scalar没有source exact导出身份，不为它伪造一个Scalar分支；它仍可参与不产生独立persistent layout的内部geometry。

Struct policy的tag1为无payload的Ordinary，tag2为`CLayout { aligned, packed, canonical_c_layout }`，三个field依次保存5.1的closed CLayout value与既有canonical C layout fingerprint；reader逐字段比对该C layout，不把它当作一般Scoop representation的authority。`interior_mutable`只接受unsigned 0/1。nominal field product为`{ field: PersistentFieldId, storage: FieldStorage, access_alignment }`；field identity的canonical owner必须等于source/generated nominal owner，field exact从FieldStorage与被引用value layout逐项相等。字段保持MIR声明序，不能按id排序；owner、唯一性、GC事实与顺序都须在完整section的source join中核对。

Tuple element不是nominal declaration field，不派生或复用`PersistentFieldId`。它的product为`{ index: TupleElementIndexV1, storage: FieldStorage, access_alignment }`；typed index编码为unsigned，严格依次为0至元素数减1，element exact逐项等于该record的`ExactTypeKey::Tuple`。这一位置只在该tuple exact内有意义。tuple、generic nominal及pointer/function等structural表示的完整wire constituent不改变1.3的production gate。

TaggedEnum的`tag_layout`与`pure_region`均为`{ offset, byte_size, alignment }`。tag geometry从已验证target的既有enum tag layout重放，当前Darwin AArch64为0/8/8；这里不新增tag宽度规则。variant product为`{ variant: PersistentEnumVariantId, fields, slot }`，fields中的product为`{ field: PersistentEnumVariantFieldId, storage: FieldStorage, access_alignment }`，variant-field owner必须等于对应variant。slot的tag1为`SharedPure { byte_size, alignment }`，tag2为`Dedicated { offset, byte_size, alignment }`。GC-free variant只能使用SharedPure，其offset来自同一pure region；含managed leaf的variant只能使用独占Dedicated。pure region取全部GC-free variant的最大size及最大alignment，最大size本身不额外tail-pad；随后按语义variant顺序追加独占slot，最后才完成整个enum的tail padding。slot size/alignment、region、tag与field placements全部由同一次checked replay产生，reader拒绝任何独立矛盾的输入。

所有variant FieldStorage的Stored offset统一相对整个enum value起点，不能有的相对slot、有的相对object；ElidedZst无论variant位置均为canonical offset 0。tagged scan只组合独占slot中的managed leaves，并验证它们与整个value extent。NicheEnum的variants沿原语义顺序保存`{ variant, fields }`，fields复用上述typed variant-field product；必须恰有两个variant，一个无field，另一个恰有一个qualified pointer field，`payload_variant`必须精确指向后者。pointer kind逐项等于payload value layout的QualifiedPointer kind，payload field offset为0。niche与tagged由规范7.4的同一封闭判定重放，不能让producer在等价bytes之间任选编码。

InstanceRepresentation沿6.1的tag1～5，ClassObject的field1～3为`base_prefix/declared_fields/complete_fields`；base prefix的tag1为NoBase，tag2依次保存`exact/layout/byte_size/alignment`。BoxedPayload的field1、2为`payload_exact/value_layout`，InlineBytes无payload，InlineArray的field1、2为`element_exact/element_storage`，AbstractReference无payload。class fields使用nominal field product；complete fields只能从已验证base的完整projection与本class的declared fields机械拼接。box、array及class shape均从同一typed依赖重建并与body.shape逐字段相等；shape中的object-relative scan与record的scan role按6.1严格区分。

BoxedPayload只接受两种exact关系：value自身TD的ManagedObject layout与payload同exact，或该payload的既有`BoxedValue { payload }` generated exact；两者均引用同target的ManagedValue layout。前者保持runtime spec 2.2中包括Unit与其他ZST在内的value TD契约，后者保持有限shape-support helper身份。Unit的Value body只能是IntrinsicValue；这不排除其ManagedObject body为BoxedPayload。reference exact不能因为其value表示恰为一个pointer而选择BoxedPayload，完整section须与MIR的value/reference facts逐项join。

object沿既有HIR/MIR表示保留source object exact作为物理ManagedValue、ManagedObject与TD identity；`ObjectBackingClass { object }` exact只提供独立shape来源，不因此生成第二套物理定义。其ClassObject body从对应MIR ObjectBacking representation重放，declared field的owner必须是该key派生的backing nominal；普通class字段仍匹配自己的source nominal。完整section逐项验证Object→ObjectBacking key、base/fields与同一实际backing Class arena的关系，不能把两个exact合并或仅凭相同geometry接受其他object的backing。

单record的checked replay证明canonical identity、representation几何及所有scan范围，不替代完整section的HIR/MIR source join、export surface或selected dependency closure。Unit必须等于compiler固定的`CoreBuiltinNominal::Unit` exact，且该exact的Value body不能选择其他representation；Integer/Boolean/String/Array/MutableArray没有compiler固定的nominal identity，其kind/family必须由完整section与同一trusted-core receipt下的既有intrinsic binding逐项join，不能按名称或FQN重建identity，也不能仅凭任意nominal加family输入声称已经验证core authority。reader在解析typed identity、分配field/variant表与复制完整base projection前消费共享`BudgetMeter`；按by-value依赖与base active path拒绝cycle。完整record只持有closed storage/shape及已核验依赖，不通过裸size、默认offset、missing-key fallback或native-boundary witness补齐authority。

### 6.3 scan normal form 与预算

scan完全复用runtime spec 2.2，普通node的offset相对明确的base：

- References严格递增且无重复；Sequence flatten、删除None、合并同层References，并按 `(child fingerprint, canonical bytes)`排序去重。
- Array只能是 `{ length_offset, first_element_offset, nonzero_stride, nonempty_element }`，两个offset相对object base；GC-free/ZST element直接为None。
- tagged enum扫描所有独占ref-bearing slot，不读取tag；pure-value共享区不进入scan。
- box的inline scan相对payload，object scan通过checked offset平移；Array node平移length/first offset，不平移其element child。

五项限额固定复用：depth 64、distinct nodes 65536、distinct words 1048576、expanded nodes 1048576、canonical bytes 16777216。cycle检测使用active path；共享DAG的expanded cost按每条路径重计，用memoized checked subtree cost在展开前拒绝。producer正规化到fixed point，reader拒绝非canonical输入而非替它排序修补。

`ScanFingerprint`继续使用既有 `scoop-scan-v1` 与runtime canonical typed bytes，不换成Wire CBOR hash。layout/TD reader重算scan及所有offset边界，不能只验证digest长度或“scan id已存在”。

## 7. TypeDescriptor 与有限 shape-support

### 7.1 descriptor record

`ExactDescriptorExportV1` 保存 `{ exact, value_layout, instance_layout, shape, object_scan, ancestry, dispatch, diagnostic_name, definition, registration }`。两个layout引用分别指向该exact的ManagedValue与ManagedObject记录；shape/object_scan是跨record关系证明，必须与instance layout逐字段相等，不是可独立修改的第二authority。shape只接受下列checked sum；ancestry/table edge使用 typed external/local ref，不保存地址。

| shape | allocation/inline规则 | object scan |
| --- | --- | --- |
| FixedObject | 含header、完整fields和tail padding的非零exact allocation | 完整object-relative scan |
| BoxedValue | `inline_offset = alignUp(16, value alignment)`；minimum为`alignUp(offset + value size, max(8, alignment))` | inline scan checked平移 |
| InlineBytes | minimum/offset 24，instance alignment 8，element size/stride/alignment 1 | None |
| InlineArray | minimum/offset `alignUp(24, element alignment)`；ZeroSized与Inline分开 | 只有nonempty element scan才有Array node |
| AbstractRef | 所有instance/inline size、alignment、offset为0 | None，不可分配 |

`AllocatableTypeDescriptorRef`排除AbstractRef；box、array、class allocation再分别要求对应refined variant。运行时拒绝错误TD只是防御，正常LIR不可表达对AbstractRef分配。

abstract class仍有完整FixedObject instance布局供derived prefix和initializer使用，不等于interface/纯reference identity的AbstractRef。`ClassAllocationTarget`必须同时持有FixedObject shape proof与`ConcreteClass` modality proof；abstract class不能构造该target，但可提供`BaseInitializerTarget`。仅检查shape不是合法class construction证明。

`ExactDispatchExportV1`以`PersistentDispatchTableId`为主键，product固定为`{ table, owner_exact, role, entries, definition }`。role是`Vtable | Itable { interface_exact }`，必须匹配foundation DispatchTableKey；entries按物理position保存`{ position, slot, slot_signature, implementation, abi }`。implementation使用5.3的已验证target/adjust关系，abi引用同一local/external callable ABI export；abstract obligation只可出现在abstract owner允许的schema，物理entry必须指向同signature的既有trap target，concrete owner不得残留该分支。table及owner TD中的对应table ref、interface key、slot数和每项target逐字段相等；definition绑定同一Strong table atom。

`CanonicalExactTypeDiagnosticName`由已验证exact key重算，遵守总设计3.1的grammar、generated role和16 MiB checked展开预算。import/re-export/typealias拼写不参与。name bytes按值进入descriptor definition，关联只读atom由同一definition plan覆盖；不能让consumer替外部TD提供本地display string。

### 7.2 普通 Cone shape-support

ordinary producer 的 `ParamFreeShapeSupportExportV1` 使用M23-3八role的closed product，语义和field顺序不变，provider限制从新section的当前producer证明取得。core只从旧record取得同一role集合，新表必须为空；两条authority不复制彼此的root/role记录：

```text
SourceNominal / ValueLayout / RefScan / TypeDescriptor / TypeRegistration
BoxedValue / CoroutineStep / CoroutineSlot
```

source集合从已验证HIR public/inheritance接口的可跨Cone请求subject闭包独立重建，包括合法protected nested subject；不能从wire已有closure反向枚举“应有全集”。纯re-export不产生新的source obligation。

前五项和Step/Slot总是Available。BoxedValue对value为Available，对reference nominal只允许既有 `ReferenceNominalRequiresNoBox`。不能增加泛化的Unavailable/Unsupported reason来掩盖缺项。

Step为 `Completed(T) | Suspended`，Slot为 `Empty | Value(T)`；使用既有generated nominal/variant/field key、完整gc flag和tagged/niche规则。每个generated role包含自身layout、scan、TD、registration和definition proof；这些helper不能再次作为source root触发无限 `Box<Step<Slot<...>>>` 展开。

owner严格由 `ExactOwnerRoot(subject)`决定：source nominal回定义Cone，application/structural回ODR。consumer请求source-root helper时仅导入provider definition；缺失closure直接使artifact无效，不能本地补Strong或伪造Structural组。

`ContinuationShell`与`CoroutineStart`不在本closure中；它们依赖`Continuation<R>`/`SuspendTask<R>`，到M23-7完整ODR proof后才加入。box/interface语义需要的adjust thunk按MIR dispatch relation独立闭合，不把“非callable shape-support”当作免验证生成任意body的入口。

## 8. Scoop typed ABI

### 8.1 canonical contract

`ExactCallableAbiExportV1` 保存 `{ target, canonical_signature, calling_convention, call_protocol, layout_dependencies, definition }`，按此顺序使用field1～6的product。target为既有`StrongCallableDefinitionOwner`，definition复用`StrongShapeDefinitionV1<PersistentCallableBodyId>`的三字段product；body id必须从`CallableBodyKey::strong(target)`重算，并与同provider的physical definition/唯一primary atom相等。calling_convention原样复用LIR的既有`Cdecl`编码。`call_protocol`是无payload的closed sum，tag1、2分别为`OrdinaryManaged/OrdinaryNoGc`，逐项匹配canonical signature的GcEffect。

本Strong callable export的目标拥有Scoop body；既有`NativeSafe/NativeBorrowed`继续只由native contract与相应callsite承载，不能给本export伪造native分支或Strong extern definition。Scoop extern的NoGc不等于本表的OrdinaryNoGc，直接source extern production gate不变。

canonical signature原样复用M23-2的 `CanonicalScoopAbiFunctionSignature`：field 1 exact signature、field 2 logical arguments、field 3 result、field 4 GcEffect；storage仍是exact type/byte size/alignment/scalar-or-aggregate的既有product。参数tag为ElidedZst=1、Direct=2、Indirect=3；result为UnitVoid=1、ElidedZst=2、Direct=3、Indirect=4。

本section增加的是每个storage的完整layout/scan来源和可调用definition证明，不增加另一套extern signature编码。`layout_dependencies`是field1～3的product：`receiver, parameters, result`。receiver为tag1的无payload`NoReceiver`或tag2的`Layout`（field1为`PersistentLayoutId`）；parameters为保持声明顺序的layout id序列，result为单个layout id，即使UnitVoid也必须保存Unit的layout引用。所有位置均引用同target的ManagedValue记录，逐项与exact signature及storage相等；重复type保留重复位置，不能改成去重集合。receiver在source logical signature中独立保存，降低时按既有规则作为第一个logical input；不能静默遗漏。

当前Darwin/AArch64 classifier：scalar、qualified pointer、niche enum为Direct；非ZST tuple/ordinary struct/tagged enum/exception record为Indirect；ZST input为ElidedZst；Unit result为UnitVoid，其他ZST result为ElidedZst。不得按aggregate大小或system C classifier另选pass mode。

### 8.2 physical signature与调用

physical参数顺序只计算一次：若result indirect，首参数为result storage；随后按logical顺序跳过ZST、发出direct value或indirect pointer。每个indirect参数有fresh exact caller storage，callee遵循按值语义。

LLVM definition、call、invoke、dispatch和Scoop extern在同一physical index使用 `byval(exact LLVM type) align N`；indirect result使用 `sret(exact LLVM type) align N`。显式statepoint wrapper把callee参数attribute平移到intrinsic参数 `5 + i`。codegen从同一checked signature计算，禁止维护第二张可独立修改的physical表。

全部ZST实参仍按源码顺序求值。callee只在观察参数地址时分配token；用户ZST result产生typed logical value，不借Unit sentinel丢失exact type。两个source callable物理签名相同也不能共享identity、symbol、override slot或ABI fingerprint。

### 8.3 GC与异常边界

含ref aggregate按已验证scan和typed storage拆为AS1 leaf，不用byte array擦除provenance。ordinary managed call/invoke复用M15 root plan：invoke前root frame同时覆盖normal/unwind存活leaf及可移动实参，两个后继reload并pop，不产生exceptional gc.relocate。

Indirect参数/结果storage、value receiver copy、外部field内联ref与dispatch receiver都参与同一活跃性/root plan。post-statepoint只使用relocated/reloaded值，不能从旧indirect temp缓存ref。

普通NoGc与native Scoop NoGc不是一种callsite。Scoop extern无论GcEffect值均保持NativeBorrowed/caller-root publication；C bridge继续NativeSafe。effect轴不改变ordinary/suspend signature identity，也不使direct source-extern能力提前开放。

## 9. ZST place、boxing 与 static storage

### 9.1 logical value与place

LIR区分 `LogicalZstValue { exact }`、`AddressableZstPlace { place, exact, alignment, lifetime }` 和nonzero storage。token需求在MIR保留、LIR定稿；codegen不由LLVM store size为0反向发明place。

parameter/local/value `this`真正取址时分配non-null、至少1-byte、满足alignment的token。有效期重叠的不同semantic place不得共址；重复取同place地址稳定。token不能标成可合并的 `unnamed_addr` 常量，也不能通过共享零地址/singleton实现；不重叠lifetime允许复用。普通SSA ZST、field和array element不自动取得token。

### 9.2 box/unbox执行路径

```text
BoxPayload = ZeroSized | NonZero { source_place }
UnboxResult = ZeroSized | NonZero { destination_place }

scoop_rt_box_zst(td)
scoop_rt_box_value(td, source_place)
scoop_rt_unbox_zst(object, expected_td)
scoop_rt_unbox_value(object, expected_td, destination_place)
```

接口严格按runtime spec 2.3：size/alignment/inline_offset/scan只从TD读取，删除旧 `box(td, payload, size, scan)` 和固定`+16`路径。ZST入口没有payload/result pointer；box仍分配TD规定的非零managed object并取得fresh ref identity，unbox先检查exact TD再产生logical value。

nonzero source place须地址稳定、对齐且在call前已写入完整值。inline scan非空时caller先经compiler-private NoGc leaf `PushRecursiveRegion`登记该temp，再进入box runtime和可能park的managed-entry handshake；root保持到分配、从collector更新后的同一temp复制及返回完成后才LIFO pop。所有非fatal出口配对；runtime只验证root已活跃，不在入口后补登记。空scan可以省略frame。

box payload不能作为可观察的value `this`存储暴露；adjust thunk初始化独立方法局部值，ZST需要地址时另建token。moving GC依靠完整object scan与精确side-metadata size，不依赖payload非零。

### 9.3 static token与初值

compiler-managed ZST storage保留persistent storage/unit identity，logical `byte_size=0`、allocation extent=1、scan None、canonical token byte=0。两种静态初态仍严格区分：有runtime unit的storage/failure/published root使用ZeroedForRuntimeUnit；无unit的静态值使用EncodedStaticValue，即使bits全零也不改tag。

EncodedStaticValue的template恰覆盖allocation extent，padding/pointer leaf初始归零，immortal relocation按offset排序且只指向已登记immutable object-start。ZST token没有relocation，None scan使用既有static sentinel。ordinary property不因有token开放`addressOf`；本地raw global/TLS遵守原GC-free和lvalue规则，C-boundary ZST storage仍拒绝。

本阶段immortal仍限于既有String表示，`ImmortalObjectTypeRegistrationRefV1`保持Local/CoreExternal范围；imported const String继续按Stage5在consumer生成自己的literal/immortal，不引用provider immortal地址。`StaticImmortalRelocationPlanV1`继续只解析本Cone immortal producer表，不能因通用layout API已存在便扩大为任意foreign immortal relocation。

## 10. Array、pointer 与 C 边界

### 10.1 ZST array

array type仍由core generic nominal提供identity，typed `ArrayTypeId`非可选地携带exact owner、mutable/immutable kind和element storage。offset16的count是内部u64 machine metadata，源码size/index是Long；logical count必须在`0..=INT64_MAX`。

- `data_offset = alignUp(24, element alignment)`，ZST allocation恰为该offset，与count无关；Inline allocation按checked `alignUp(data_offset + count * stride, instance alignment)`。
- get按receiver→index求值，再检查bounds，成功产生exact ZST；set按receiver→index→RHS求值，再检查bounds，成功不写payload。越界不能跳过RHS。
- literal/assembly/spread/vararg保留每个part的求值与checked计数；不会因为element size为0删除producer或把length溢出隐藏成小allocation。
- clone/互转验证source exact array TD、physical count与side metadata一致，再分配fresh target并复制logical size；ZST不发payload memcpy或write barrier。
- iterator固定保存array ref和Long index，比较`index < size`、每次加1；禁止pointer-end/stride-progress实现。
- GC-free/ZST element直接None scan，collector工作量不随logical count增长；含ref Inline使用真实data offset和nonzero stride的Array scan。

String、Inline array的所有乘加/alignUp同时检查u64、target size_t和maximum_managed_object_size。非法内部count/layout或溢出沿既有fatal invariant/allocation路径，不按名称虚构IllegalArgumentException；不新增length-based源码constructor。

### 10.2 pointer

unsafe `Ptr<ZST>` plus/minus/load/store offset的byte displacement恒为0，pointer bits不变；receiver、offset、value仍求值。load/store不访问payload但继续要求non-null/alignment/lifetime和合法逻辑place。`Ptr<Unit>`是opaque void pointer，逐byte arithmetic必须显式使用`Ptr<UInt8>`。

本节是通用表示规则；Ptr exact type和相关generic callable的生产物化仍受1.3 ODR gate约束。不得以“pointer只占8 bytes”为理由本地Strong发TD或绕过specialization。

### 10.3 C ABI source规则

HIR在source边界统一检查，不能推迟到LIR/generated C/native linker：

| 边界 | 规则 |
| --- | --- |
| Unit result | 唯一void例外 |
| ZST by-value parameter/result/callback | 拒绝，包括Unit参数 |
| extern global/TLS ZST | 拒绝 |
| 空CLayout或具体ZST字段 | 拒绝 |
| `Ptr<Unit>`、`Option<Ptr<Unit>>` | opaque data pointer例外 |
| `Ptr<其他ZST>`及其Option | 拒绝C pointee |

generic CLayout的binder-dependent字段保留 `CFieldSafeAndNonZst { signature_type, declaration_field_path }` predicate，条件进入template fingerprint。无字段或与binder无关的非法字段在定义处报错；concretization检查在M23-7接入生产，本阶段在typed constituent测试锁定。不能把待替换条件提前编码为true，或在consumer丢弃字段路径。

C端真实寄存器/aggregate lowering继续由validated generated-C toolchain完成。M23-2的canonical C storage/layout、extern contract与callback bytes保持；general layout proof只能与它们交叉验证，不能反向扩充旧native witness的授权范围。

## 11. closure 验证、Link proof 与 fingerprint

### 11.1 原子验证顺序

```text
envelope/profile/target/resource checks
  -> foundation identities + direct/support role
  -> HIR type facts / access / inheritance closure
  -> MIR source-to-implementation / slot / helper relations
  -> LIR layout replay / scan normal form / ABI / TD
  -> complete source-root support obligation
  -> selected request closure and external arena commit
  -> final object verification + Link-only use closure
  -> Compile/Link equality + publish
```

继承环和by-value representation环拒绝；通过managed reference的递归class合法，layout重放在reference leaf停止，不沿对象图无限展开。所有provider来自同一target-compatible显式artifact closure。记录存在、id匹配或digest相同都不能替代逐字段关系证明。

错误前不发布partial world、arena、artifact或cache entry。MIR/LIR自身不报告新的源码visibility/overload错误；不完整selected集合属于compiler/artifact invariant。

### 11.2 semantic dependency与physical use分开

编译只读取外部field offset、size或scan事实时，可以没有对provider layout constant的object relocation；metadata-only TD authority也不伪造地址使用。反之call、TD address、parent/interface table、dispatch target、registration与object ensure的实际relocation必须逐条验证。

`SelectedDependencyLayoutAbiSetV1`按6.1的field1/2完整保留semantic use与physical import；新Link section只保存后者。其`semantic_imports`必须与Compile selected field2逐byte相等，不得从requirements或symbol表反推、补齐或裁剪：

```text
CrossConeLayoutLinkClosureSectionV1 {
    semantic_imports: CanonicalVec<ExternalShapeLinkImportV1>,
    requirements: CanonicalVec<ExternalShapeUndefinedUseV1>,
    object_coverage: ExternalShapeObjectCoverageV1,
}

ExternalShapeLinkImportV1 {
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
    contract: ShapeLinkContractV1,
}
```

`ExternalStrongShapeSubjectV1` 精确采用下表，tag 是本新增sum的编号，不修改被引用identity的kind/tag：

| tag | variant | payload |
| --- | --- | --- |
| 1 | Callable | `StrongCallableDefinitionOwner` |
| 2 | Layout | `PersistentLayoutId` |
| 3 | Scan | `PersistentScanId` |
| 4 | TypeDescriptor | `PersistentExactTypeId` |
| 5 | DispatchTable | `PersistentDispatchTableId` |
| 6 | TypeRegistration | `PersistentExactTypeId` |
| 7 | StaticStorage | `PersistentStaticStorageId` |
| 8 | StaticStorageRegistration | `PersistentStaticStorageId` |
| 9 | InitializationCell | `PersistentInitializationUnitId` |
| 10 | InitializationDescriptor | `PersistentInitializationUnitId` |

每项wire是 `{ 0: tag, 1: payload }`，定义和symbol role从variant唯一派生。7～10只可由provider已导出的object-value/initialization support relation选择；不得通过它们枚举私有storage，也不能引用任意其他unit的cell/failure root。ordinary property access依然只使用accessor，不以此公开backing storage。来源为已存在旧core bridge的subject按3.2留在旧分区，不能重复登记。foreign immortal不是本sum的variant，其现有String/constant路径遵守9.3。

`ShapeLinkContractV1`精确分为七个variant：`CallableAbi { canonical_signature, calling_convention, protocol }`、`Layout { record }`、`Scan { layout, role, canonical_scan }`、`Type { descriptor_projection }`、`Dispatch { table_projection }`、`StaticStorage { storage_projection }`、`Initialization { unit_projection }`，tag按此顺序为1～7。subject1～5分别只能匹配contract1～5；subject6复用Type、7/8复用StaticStorage、9/10复用Initialization。

Layout/Type/Dispatch分别复用6.1/7.1的canonical semantic record，去掉definition/registration的后置digest槽；Scan保存完整scan tree而非仅digest；storage/unit projection逐字段复用strong-production/2对应semantic plan，不包含member/range或后置object/registration digest。它们不是任意bytes，reader按subject取得provider同一plan并逐字段比较。required definition由subject和owner重算，provider必须真实Strong定义它，consumer defined-symbol set必须不包含它。

新projection保留原字段编号：Layout恰为exact-layout record的fields1～6，Type恰为exact-descriptor的fields1～8，Dispatch恰为exact-dispatch的fields1～4；三者分别排除field7、fields9～10与field5的definition/registration后置槽。StaticStorage恰为strong-production static record的fields1～10（storage、symbol、layout、scan id、完整scan、scan kind、logical size、allocation extent、alignment、initial state）；Initialization恰为unit record的fields1～8（unit、diagnostic path、schedule、storage、failure root、initializer、ensure、ordered dependencies）。这些语义字段均不含后置digest，已有initial-state、schedule及V2 dependency的wire保持不变；typed dependency proof仍在provider同一strong-production/2计划中保存，不能由wire unit id重造。每个projection复用完整record的同一字段编码和逐字段重放，只改变新product的字段数，不改变旧完整record bytes。

requirements沿用M23-5 canonical relocation-use结构和排序，并引用本section import index；每个physical import至少一个use，每个actual relocation恰有一项。object coverage绑定全部最终LinkObject成员集合及canonical use set，digest使用 `DomainSeparatedCborHash("scoop-cross-cone-layout-object-coverage-v1", { verified_link_objects, relocation_uses })`，仅作LinkValidationOnly。

### 11.3 三路 relocation分区

最终object undefined-use集合被构造时分成三个互斥集合：

1. M23-3原core/intra-Cone/generated/native/runtime/target分区；
2. M23-5原ordinary core-closed callable分区；
3. M23-6新增general callable/type/dispatch/shape分区。

三者并集精确覆盖所有nonlocal undefined relocation。旧section继续验证自己的子集，不把新subject塞进`CoreStrong`或旧callable-only target。

ObjectDefinition relocation规范化在既有tag1～11之后新增tag12 `DependencyShapeStrong { provider, subject }`；它只从新Link proof派生，不按symbol解析。该hash路径使用既有object-definition runtime scalar encoder：`u32(12) || provider.raw32 || u32(subject_tag) || subject_payload`；Callable payload复用既有StrongCallableDefinitionOwner runtime编码，其余payload是对应typed id的raw32。这里不是Wire CBOR，不套用3.3的map格式，也不改变只供callable-body identity使用的`RuntimeEncode(key)`契约。tag11继续仅表示M23-5的`DependencyStrong`。这是受新required capability保护的object-definition target扩展，不改persistent identity key或旧capability payload。

object verifier既检查undefined target，也检查provider实际TD/scan/table bytes、alignment、field offset、ABI adapter和typed relocation。单靠object symbol table不能证明Scoop signature；ABI一致性由source→MIR→LIR→emission关系和object evidence共同证明。

### 11.4 hash与cache

新Link section的Code贡献沿M23-3 `KnownLinkExtensionCodeContributionV1`：capability为新section id，payload精确为canonical `semantic_imports` array，空时也存在。Compile selected的physical projection、Link section和Code contribution逐byte三方相等。

layout、scan、LIR definition和registration沿既有 `scoop-layout-v1`、`scoop-scan-v1`、`scoop-lir-definition-v1` 与digest DAG算法；不改hash encoder。新semantic record覆盖exact identity、target layout projection、字段/variant/prefix、ABI、scan、TD name与dispatch relation。layout不依赖provider code digest，metadata/table间指向只用typed identity，避免互相引用TD/dispatch/function形成digest环。

runtime-image fingerprint通过既有strong registration/digest graph覆盖新定义。MIR/LIR semantic bytes不包含SlibMemberId、object range、atom placement或host路径；object重新分片只改变对应physical/code/artifact部分。

M23-4 cache继续保守纳入全部direct dependency各层fingerprint，不在本阶段按selected set裁剪。field、base prefix、slot、default/access域、ZST status、layout/scan/ABI或target变化必须使对应consumer层失效；provider body-only变化仍由provider code和后续program-link闭包跟踪，不复制body到consumer HIR。

## 12. 诊断

source错误仍在parser/HIR结束，并断言主span与必要的provider声明/字段路径note：

- 非法protected receiver/词法位置、不可见setter或constructor；
- final base继承、非法override、未实现abstract slot、冲突interface default；
- C ABI ZST参数/result/global/TLS/pointee、空/含ZST CLayout；
- 非lvalue addressOf、非法private backing access；
- 实际请求依赖M23-7 ODR或M23-10 native能力。

artifact错误以capability/table/typed key/field path报告：缺record或source obligation、错owner/provider、forged gc/zst、layout/scan/ABI不一致、slot重复/错position、非法TD shape、required capability/profile/target不符、physical-use coverage缺项或重叠。报告期不按FQN补查来源。

target表示上限由LIR报告明确target/layout错误；内部array count、side metadata、box root-frame或runtime shape违反是fatal invariant，不转成源码异常。错误排序沿现有source顺序与typed key canonical顺序，provider路径只作diagnostic decorator。

## 13. 测试与验收矩阵

### 13.1 独立fixture与组合fixture

生产fixture采用provider→consumer，另加facade re-export和diamond；provider源码在consumer编译时不可见。建议目录 `tests/fixtures/milestone23_stage6/`，每项同时检查HIR/MIR/LIR golden、selected metadata和双view artifact；运行行为在单image harness验证，并标出待M23-9/11复用的真实多Cone运行断言。

| 主题 | 独立positive | 组合与negative |
| --- | --- | --- |
| 外部value | 空struct、嵌套ZST、混合integer/ref struct、enum | re-export/alias、嵌入本地class、模式/copy update；错field/variant identity |
| ABI | 多个ZST参数、用户ZST/Unit result、nonzero indirect | mixed参数与sret、default求值、throw/invoke、member/dispatch；错exact/pass/effect |
| constructor | public构造、protected base initializer、abstract base | base-before-derived、init异常、moving receiver；不可见/abstract allocation |
| dispatch | class override、interface/default、getter/setter | diamond/default冲突、super direct、value boxing；缺slot、错position、窄override |
| visibility | subclass implicit/explicit receiver、protected nested | Base静态receiver、同级subclass、非subclass、support-only import、setter拒绝 |
| object/property | provider singleton/companion、runtime accessor | 跨facade访问、ensure identity、delegated storage；consumer重复storage/initializer |
| support | provider未在自身body使用的公开value仍导出完整support | 缺任一role、错owner、递归helper展开、consumer重发Strong、shell/start提前加入 |

每条编译错误规则都有独立negative fixture并断言span/message，不以单一“大型失败程序”覆盖全部规则。旧M23-5 narrow callable fixture应保持旧分区，用新增nominal/member fixture命中新分区。

### 13.2 表示、GC与address测试

- Unit、空struct、全ZSTtuple/struct为0/正alignment；不同exact ZST有不同identity/TD。内部typed case覆盖alignment>1及超过target上限。
- Option<ZST>保留tag；enum含ref variant独占slot、inactive清零、scan不读tag；niche ref与raw/code pointer扫描分开。
- 五种TD逐variant验证C/LLVM sizeof/offsetof、shape round-trip和所有非法组合；AbstractRef不可形成allocation LIR。
- nonzero box包含ref及over-alignment，验证caller recursive root先于handshake、GC更新同一temp、异常cleanup配对；ZST box/unbox无payload pointer且每次fresh。
- address-taken parameter/local/value this的non-null、alignment、有效期及同时存活place不共址；重复同place稳定，field/element仍不可取址。
- static token extent1/logical0/scanNone；ZeroedForRuntimeUnit与全零EncodedStaticValue区分；错误template长度、relocation leaf/target/order以及跨identity range重叠拒绝。
- Array<Unit>、MutableArray<Phantom<T>>的0/1/大合法count、literal/spread/assembly、越界set RHS副作用/异常、index iterator、clone fresh identity；ZST无copy/barrier/length相关扫描。
- 含ref、alignment>8的array element使data offset不等于24，检查scan位置与relocation；篡改length offset/first offset/stride分别失败。
- String/InlineArray/ZSTArray在INT64_MAX、u64乘加、alignUp、target size_t及对象上限边界的成功/失败；不引入signed-length源码API。
- unsafe Ptr<ZST>正负offset bits不变、load/store求值保留；C ABI逐项测试10.3矩阵和generic CLayout deferred predicate。

generic/structural cases使用1.3规定的typed test harness；对应production请求另有M23-7拒绝fixture，不能通过削弱profile让positive harness结果冒充生产成功。

### 13.3 wire、object、cache与健壮性

- 四个新增capability、strong-production/2和新profile fixed vectors，empty/nonempty、unknown required、错purpose、旧profile拒绝；M23-2 foundation/extern/callback与M23-3/5旧section vectors不变。V2的ordinary parent/itable key/dispatch target正反例必须经过真正的strong production wire round-trip，不能只在新layout sidecar中通过。
- 每个record去掉/增加/错tag/错kind/错owner/乱序/重复逐项拒绝；reader独立重放字段layout、scan、ABI和source-root obligation。
- scan/type-name共享DAG、cycle、深度/展开/bytes预算边界在分配前失败；合法递归ref class成功，by-value环失败。
- 别名/re-export spelling改变不改变exact TD name/ABI；改变base prefix、ZST exact identity、slot contract或scan offset改变对应fingerprint。
- metadata-only外部layout没有伪relocation也能成功；physical callable/TD/table use缺relocation、错definition或多归属失败。
- 三路undefined-use分区两两不交且并集完整；consumer定义foreign Strong、weak/ODR symbol、错误TD/scan/table bytes或关联diagnostic atom均失败。
- 相同semantic program换source枚举、dependency枚举、arena顺序、object分片不改变相应semantic fingerprint；physical/code/artifact变化遵守既有规则。
- producer field/base/interface/ABI变化触发consumer cache miss；失败child/invalid artifact不得发布cache；source/direct/support graph回归不受影响。
- consumer initializer读取provider object/property时保留external unit dependency；V2 round-trip验证provider与descriptor，缺unit、错provider、伪造local、省略真实edge逐项拒绝。callable内部provider自有ensure不被consumer复制。
- class prefix单独覆盖base尾padding、derived更高alignment、ZST own field和abstract base；同Cone与跨Cone布局逐字段一致。slot位置锁定声明序语义，不受wire table按id排序、import或dependency枚举影响。
- 运行M23-1～5及本地value/GC/constructor/property/FFI/closure/coroutine回归，检查stage crate依赖方向。

## 14. 实现顺序

1. 固定本文profile/section/wire constituent及测试vector；添加required capability gate和拒绝旧profile路径。先有验证入口，再放宽source能力。
2. 收口HIR concrete facts与persistent inheritance surface，完成protected/domain/default witness和source negative；保留旧public section字节。
3. 实现MIR type/constructor/object/slot bridge与selected closure；以golden锁定没有foreign body复制、layout offset或generic template。
4. 收口LIR storage/refined shape、general layout replay、scan normal form和external exact arena；实现provider有限shape-support导出和consumer验证。
5. 接入通用ABI及logical-to-physical映射，验证call/invoke/dispatch/byval/sret/root plan，再开放对应param-free source成功格。
6. 完成box/unbox、ZST place/static、array/Ptr执行路径与C边界矩阵；删除被替代的旧size/scan/header-offset入口。
7. 完成新Link-only closure、三路object coverage、definition规范化、Code贡献和双view发布；更新core、scheduler、cache accepted profile。
8. 完成独立/组合/negative/golden/corruption与回归矩阵，将多Cone运行场景交给M23-9/11复用。

每批代码变更完成后先 `cargo fmt --all`、`cargo clippy --workspace`，再运行相关test；最终运行完整workspace与runtime/fixture验证。不能用最终linker尚未实现为理由跳过本阶段object和双view证明。

## 15. 完成门

- source/access/inheritance、MIR relation、LIR layout/ABI/scan/TD、actual object use形成可从最终artifact独立重建的完整链；没有symbol/FQN/host layout fallback。
- param-free跨Cone构造、value/member/object使用、inheritance/dispatch/protected成功矩阵全部能生成新profile双view有效artifact，未选中的foreign body不复制。
- 每个合法source subject在定义Cone拥有完整有限shape-support；consumer只引用external typed definition，全部ODR生产继续拒绝。
- ZST logical semantics、typed ABI、place/static token、box/array/Ptr/C边界及scan/TD矩阵全部锁定；codegen/runtime不再从size0或空LLVM struct猜语义。
- nonzero box、indirect aggregate和跨Conefield的managed provenance/root/relocation完整；不存在握手后才登记root或从旧ref副本复制的路径。
- 新required section/profile、strong-production/2的完整foreign TD/dispatch引用、fingerprint/cache迁移和三路Link coverage完整；既有identity、extern/callback bytes、旧capability语义保持。
- 独立、组合、negative、各stage golden与corruption/determinism回归通过；文档明确M23-7/8/9/10/11交接，真实多Conemoving-GC不被提前宣称完成。

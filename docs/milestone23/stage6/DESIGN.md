# M23-6 设计：跨 Cone layout、typed ABI 与 ZST

2026-09-23 GC handle Scoop ABI 修正：PinnedPtr/GcHandle 的 C 透明表示不适用于 Scoop 调用。Scoop canonical ABI 从实际 nominal 字段重放普通 aggregate，并按共有规则间接传参/返回，固定 core 身份不能省略 witness 或字段闭包；具体合同见语言规范 14.1、14.2 与实现规范 2.11。

2026-09-23 当前 Link requirement 约定：所有外来 strong 选择在共有步骤中按实际 provider/symbol/typed owner 解析，取消 core proof 与独立 core owner 参数。已有 verified dependency owner 集合同时服务外部 callable、descriptor、registration support 和普通 callable，按实际 capability/subject 保持用途互斥与 relocation 完整覆盖。最终 requirement 的旧 CoreStrong tag 2 退役，tag 8 保存 provider/typed strong owner，object-definition fingerprint 的扁平 target sum 使用独立 tag 13；link-identity-closure 升级为 /2，旧版本/tag 拒绝，profile、Code fingerprint、writer/reader 和固定向量同步。完整合同见实现规范 2.11。

2026-09-22 当前 native intrinsic 表示约定：NativeBoundaryNominalShape 使用新 tag 4 保存完整共有 NominalIntrinsicRepresentationV1，生产与依赖投影不再丢弃 family。HIR identity-foundation 升级为 /2，旧 field 30 退役，field 33 保存完整 native 类型表；三十二字段集合为 1～29、31～33，旧字段/版本及 optional 混入拒绝。标量 canonical ABI 重放按实际 typed 声明的 intrinsic family 计算，不再重建 Integer/Boolean 的 CORE 身份，Unit 保持语言内建 identity；其余 Option 与 GC handle 固定身份分支继续迁移。具体表示、覆盖与版本合同见实现规范 2.11。

2026-09-22 当前外部引用约定：String 和初始化 callable 使用共有 typed provider/definition 记录，禁止从 CORE 常量恢复 provider。strong production 的旧 field 1 退役，field 12 保存共有外部引用；旧 callable tag 1 退役，tag 3 直接保存完整 SelectedDependencyLirCallableV1，TypeDescriptor tag 2 保留已有含 provider 的载体。两种 TD/dispatch schema 统一使用显式 provider 的 DependencyExternal，CoreExternal tag 不复用；immortal registration 使用新的 tag 3 provider/exact 引用。strong-production /3、/4 升级为 /5、/6，cross-cone-layout-abi 与 cross-cone-layout-link-closure 升级为 /2。旧格式和 optional 版本混入均拒绝；local/runtime/absent、String 表示、runtime C ABI 和用途分区保持，详细编码与验证见实现规范 2.11、2.12。

2026-09-22 初始化服务 ABI 迁移记录：LIR 删除 CoreLirBridge 及 Core/NotCore 外层，strong production 的旧 field 10 退役，新增 field 11 的 0/1 array，直接保存完整共有 CallableAbiRecordV1。strong-production 在该批从 `/1`、`/2` 分别升级为 `/3`、`/4`，随后按上面的外部引用约定继续升级为 `/5`、`/6`；后者保留 V2 layout reference schema；旧字段、旧 tag、旧版本和交叉 profile payload 均拒绝。MIR 实际角色决定 ABI 有无，MIR/LIR 导入投影和 callable 选择按所选实际 provider 验证 typed target、body、symbol、definition 和完整签名，不追加 CORE 来源条件；calling convention、GC effect、root plan、预算、registration 与用途分区保持。详见实现规范 2.12，runtime C ABI 与 String 表示不变。

2026-09-22 当前 HIR 协议定义约定：`core-bootstrap-interface/3` 删除 `CoreHirInterfaceBranchV1`。section 的旧 field 1 及 Core/NotCore tag 退役，保留 field 2 output、field 3 direct surface，新增 field 4 长度 0/1 的完整 definitions array；由实际 Defined/Imported 决定发布，不按 CORE 身份或输出种类选择分支。String 按实际非泛型 class 与 exact nominal 关系解析，intrinsic 按其 canonical 声明 origin 查询对应可达 provider 的完整类型角色。旧版本、旧字段、多份定义、缺失/冲突角色和错误关系均拒绝；typed 引用、effect、签名、成员、预算和后续布局/registration 检查保持。完整合同见实现规范 2.12；LIR/Link 外层另行迁移，runtime C ABI 与 String capability kind 不变。 MIR production 同时删除基于 CORE 坐标判定初始化角色缺失、多余或必须 library 的分支；角色有无交给相邻 HIR 实际定义核对，MIR 的唯一性、source function、完整签名覆盖、entry 归属与实现检查保持。

2026-09-22 当前 nominal intrinsic 约定：共有 source shape 以新增 tag 6 保存完整 `NominalIntrinsicRepresentationV1`，public、source contract、nested support 与独立 representation 使用相同 family、声明 kind 和 binder 合同；不再将 intrinsic 退化为空 struct 或普通 class。`hir/cross-cone-interface` 升级为 `/3`，旧 major 拒绝并重建产物。公开常量与 vararg reader 沿自身完整 typed 类型引用查询实际 provider，核对 intrinsic kind 及元素类型关系，删除这些消费者的 trusted-core 查询；不以同名、同布局或全局候选扫描替换类型 id。最小 native source witness 保持既有 wire，完整规则见实现规范 2.11。

2026-09-22 当前 Unit binding 约定：protected callable 与 dispatch slot 使用语言内建 Unit identity，删除仅为取 Unit 而传入的 imported core 协议；setter result、slot signature 和完整 exact key 的一致性检查保持。其他 intrinsic/source nominal 仍必须查询实际声明与 provider，不能套用 Unit 例外，见实现规范 2.11。

2026-09-22 当前整数正规化约定：源码intrinsic调用完成共有成员候选及参数/effect检查后，HIR只保留封闭typed operation、conversion和操作数；导入声明不物化成本地函数，后续IR和default body不保留冗余callee。整数default wire删除该字段，旧格式要求重建；exact identity、witness及extern/callback bytes保持。导入成员复用共有源码调用探测与winner commit，完整规则见实现规范2.10。

2026-09-22 当前intrinsic声明约定：共有CallableSourceEffectsV1直接保存Intrinsic(IntrinsicFunctionKind)，不以标志或core专用operation表补全kind，不携带provider授权或annotation字符串。closed-sum wire的field 0为实现tag，仅Intrinsic具有field 1=完整typed kind；旧unsigned leaf要求重建artifact/cache。整数kind的GC effect与Ordinary execution由builder和reader共同验证。常量及静态initializer从共有callable目录取得整数语义；method/infix同时按typed声明id读取canonical源码名称、source参数名和infix属性，解析结果不依赖本地FunctionId。删除ImportedCoreProtocols中的重复compiler-operation载体；源码名称只用于匹配，不用于反推kind，非const实例成员仍须接入普通跨Cone调用选择。完整规则见实现规范2.10。

2026-09-22 当前外部描述符约定：String的旧外部描述符桥不再复制target/symbol/definition；它直接保存与通用layout选择、foundation投影和LIR arena相同的完整ExternalTypeDescriptor。共有wire为四字段closed product：1=provider、2=target、3=expected_symbol、4=required_definition；旧隐含CORE的三字段格式拒绝，artifact/cache须重建。reader在共有identity graph中解析provider和exact target，由实际provider重新推导symbol/definition并逐字核对完整record，definition的owner/role/primary symbol使用同一关系校验。String协议外层继续从well-known明确引用选择记录并验证所需CORE provider；不能把其他外部描述符按provider归入String角色。不再保留StrongExternalTypeDescriptorBridgeV1、core专用TD identity helper或CoreExternalBuildError。

2026-09-22 当前callable生产约定：LIR初始化服务、普通调用桥和通用layout/ABI发布共用实际callable关联：从typed StrongCallableDefinitionOwner取得同一MIR strong记录、实际物化root与对应LIR body，并核对exact签名。普通调用与初始化调用使用同一canonical ABI投影，统一检查GC effect、calling convention及逻辑参数数量，再构造完整CallableAbiRecordV1；初始化角色额外限定function、ordinary、无receiver。通用layout/ABI继续重放自己的layout/physical证明，但不再重复查找MIR/LIR函数；其resource meter在共有查找前按相同表长度计量，不免除预算。投影失败返回携带typed target的共有错误，不按core名称或symbol字符串补目标。此批合并生产实现，不改变角色外层wire和public可见性。

2026-09-22 当前LIR清理约定：初始化发布、外部调用与普通依赖共用CallableAbiRecordV1及ExternalCallableRootPlan；普通调用桥wire为declaration与完整ABI record组成的两字段product，旧七字段格式拒绝。provider显式输入共有identity/definition推导，初始化外层角色仍明确保存。

2026-09-22 当前清理约定：MIR production 删除独立 CoreMirBridge 与字段1；所有 strong callable 记录使用必需 CallableRole，初始化声明与实现只保存在共有记录中。HIR 协议、MIR foundation 与 LIR 投影按同一 typed 实现和签名核对，旧记录格式拒绝并重建；详见实现规范的当前约定。

M23-6当前约定：MIR初始化服务和普通依赖共用一个完整的`SelectedExternalMirSet`及typed选择引用。初始化角色显式保存，typed bridge与签名检查后进入共有集合，不保留core专用MIR借用凭证或泛型协议sidecar。strong输入使用共有consumer/覆盖/引用校验；旧wire与LIR协议仅在投影边界按角色区分，String与ABI契约保持完整。以`SCOOP-IMPL-SPEC.md`的共有外部调用约定为准。

LIR初始化服务与普通依赖使用同一`SelectedExternalLirSet`，完整保存provider、typed declaration/target、canonical ABI、calling convention、root plan、symbol及definition；选择角色与MIR使用同一语义枚举。MIR strong输出只保留一张完整外部callable根表，LIR共用选择覆盖、角色、签名、GC effect、参数/结果exact type查询和物理ABI分类，不再保留core专用callable集合或借用凭证。初始化服务与普通调用只在旧metadata角色物化时区分。String的foundation投影返回已有的完整`ExternalTypeDescriptor`，lowering显式接收Local/External描述符输入，所有测试也经过同一输入；不保留仅测试可用的runtime String替代分支。上述合并不改变String表示、内部函数源码可见性或旧wire契约。

core 是可由用户修改、扩展和重建的普通 library Cone。源码层面的特殊处理仅限于前端识别 `@Intrinsic`，并把它正规化为既有 typed IR，以及 desugar 通过普通声明引用使用基础库提供的类型和函数。sysroot 是默认查找位置，不是信任边界；源码目录、输出位置、相同 coordinate 或用户修改过的 core 不需要授权 token。metadata 解码、typed identity 一致性、依赖闭包、ABI、缓存失效和 slib fingerprint 使用所有 Cone 共用的规则。不得为 core 另建来源防伪、slot 授权、receipt 信任链或重复 pipeline；既有专用实现须合并或删除，旧文档的冻结条款不阻止此次清理。

当前优先目标与验收改为[core 普通 library 清理](../CORE-LIBRARY.md)。暂停扩展六类default来源授权、防篡改证明及独立source-authority框架。已有typed语义和普通metadata保持可用，专用证明链按实际依赖删除或合并。

版本：1.2（core 普通 library 与重复检查清理；2026-09-21）

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

1. 新增 `cross-cone-layout-strong/1` production profile。它在 M23-5 inventory 上新增 HIR type/inheritance、独立 HIR source authority、MIR type bridge、LIR layout/ABI 和 Link-only layout-use closure，并将强定义语义升级为 `strong-production/6`，以表达 ordinary dependency TD/dispatch 引用；仍拒绝全部 ODR production。
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
| `compiler/lir/src/production/shape_support/` | core 的有限 shape-support proof | 合并到所有定义 Cone 共用的形状表，移除 core 专用查询分支 |
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

ordinary callable 的 canonical ABI 校验对 core 与普通 provider 无差别执行。旧 callable bridge 借用当前 artifact 与其实际可达依赖的 native-boundary witness，重复 owner 必须完整一致；layout profile 在完整本地和依赖 section 通过后，按 exact type 查询唯一的 ManagedValue layout，不回退到 witness。两条路径使用 LIR 共有的 direct/indirect/ZST 分类，逐项核对 logical signature、GC effect、参数次序与结果；缺失、重复或不匹配的 layout 拒绝，查询与签名复制持续使用当前 artifact 的预算。此处共有 ABI 重放不代替 native-boundary 专用外来类型入口与固定 core 身份识别的后续迁移。

native-boundary reader 在同一 validated identity graph 中解析本地与外来声明，统一重建 owner、kind、参数个数、binder 及成员关系；不保留 external-core 的零字段/Reference 特许恢复分支。完整性查询共享当前和依赖图的 canonical field、variant 与 variant-field records，依赖 key 以共享引用读取，不加入本地定义 inventory。缺少 canonical 声明或遗漏实际成员必须失败，闭包查询使用调用方预算。generic 声明的结构解析不授予 generic application 执行或 ODR 物化能力；profile 的既有 gate 继续检查。producer 的外来类型输入按下述共有 world 规则闭合；后端 typed 表示与通用 layout/ABI 查询的连接仍须按清理设计完成。

共有 struct source shape 在 `cross-cone-interface/3` 中精确为 `{ 0: 3, 1: source-order fields, 2: NominalCLayoutPolicyV1 }`；policy 使用 type-semantics 的既有闭合编码，由实际 HIR `@CLayout` 属性投影。公开 source shape、nominal source contract 与 nested source support 共享该结构，representation join 必须同时比较字段与 policy。intrinsic source shape 使用新增 tag 6 明确保存完整 family，并与 representation 比对，不能使用普通 struct/class shape 代替。旧 `/1`、`/2` section 及旧两字段 struct shape 直接拒绝并重建产物，profile fingerprint、inventory 和 HIR fingerprint 随之更新，不回改 native-boundary witness 或 C ABI。本次扩展只补齐声明事实，不以 source shape 授予 layout、scan 或物化能力。

native-boundary producer 直接借用共有 dependency world，不再接收 `CurrentArtifactOnly/TrustedCore` sum 或 `ImportedCoreNativeBoundaryTypes`。外来 owner 必须解析到实际 provider 的 canonical source key；共有 v3 source shape 提供 intrinsic family、完整 struct CLayout/字段或 enum variant/字段，成员 key 来自同一 provider。非公开但已有 native witness 的声明可沿已持有 typed 引用继续闭合，Reference 由真实声明 kind 决定；共有 source shape 与已有 native witness 同时存在时逐项一致。缺失或不一致立即失败，不按 CORE、名称或空字段补默认记录。本地和外来 shape 都进入同一 signature-type 传递遍历，支持跨 direct/support provider 的字段/variant 闭包并按实际 owner 规范化；不把该最小 witness 作为一般 layout、lookup 或物化能力。

compiler protocol 的 constituent 仅验证实际声明与角色关系，不额外要求当前 export、声明 owner 或导入 foundation 的来源为 CORE。已有 identity/definition-origin 检查、完整 signature 与 binder、constructor/generated-adapter source、enum kind/member owner、effect 和固定角色完整性继续执行，普通 provider 的相同声明经同一路径验证；缺失或不一致不能以来源身份豁免。Unit 保留既有语言内建身份。producer/reader 删除重复来源资格，typed 引用编码与前端 intrinsic 使用边界不变；完整 operation 表按下一段退役，HIR 的 Core/NotCore 外层按本节 `/3` 修订迁移；其余协议投影及 String/初始化服务的 LIR/Link 外层继续共有化。

完整 intrinsic operation 表从 protocol product 的 field 9 退役，HIR `core-bootstrap-interface` 的 `/2` 修订使协议 product 只接受 field 1～8，当前 `/3` 同时使用本节开头规定的 definitions section，旧 `/1`、`/2` capability 与旧九字段 payload 拒绝；field 9 保留为退役编号。共有 callable effects 是发布 intrinsic kind 的唯一记录，普通 Compile 与 layout-profile 均在实际共有 callable/public-source 验证路径中检查 source owner、own binder、参数/结果与 execution，所用语言类型角色来自已解析的实际 provider 依赖闭包，不能由 CORE 常量或名称补出。验证按已有 callable 表逐项执行，重复 kind、角色缺失/冲突和错误签名拒绝，查询与临时记录沿用同一预算。前端完整 intrinsic 声明检查和固定语言协议角色继续保留；没有实际导入消费者的完整 operation 表不作为新的授权或执行目录。

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

其 required inventory 从 M23-5 `cross-cone-semantics-strong/1` 出发，移除 `org.scoop-lang.lir/strong-production/5`，替换为 `/6`，再加入下表前五项；`code_requirement`、`runtime_requirement`、publication、decode-cost model、extra-section policy 和 Link proof policy 原样继承，`odr = RejectAll`。

| capability | location | required_for | sinks |
| --- | --- | --- | --- |
| `org.scoop-lang.hir/cross-cone-type-semantics/1` | HIR | Compile | Hir |
| `org.scoop-lang.hir/cross-cone-type-source-authority/1` | HIR | Compile | Hir |
| `org.scoop-lang.mir/cross-cone-type-bridge/1` | MIR | Compile | Mir |
| `org.scoop-lang.lir/cross-cone-layout-abi/2` | LIR | Compile | Lir |
| `org.scoop-lang.lir/cross-cone-layout-link-closure/2` | LIR | Link | Code + LinkValidationOnly |
| `org.scoop-lang.lir/strong-production/6` | LIR | Compile、Link | Lir + Code + RuntimeImage |

`cross-cone-type-semantics/1` 同时承载 type facts 与独立 inheritance surface；它复用 public-only record 的声明职责；为完整保留 struct CLayout policy 与 intrinsic family，共有接口按本节规则使用 `cross-cone-interface/3`。`cross-cone-type-source-authority/1` 保存从同一次 sealed Export/LocalConcrete HIR 与实际 committed-use trace 独立投影的 source authority，只供 reader 重放前一 section，不能作为普通 lookup、layout 或 materialization API。前四条 Compile section 的完整 canonical inner bytes分别进入对应 layer contribution；两个 HIR section都进入 HIR contribution。新增Link-only section仅以 semantic physical-import projection进入Code，member/range/patch信息只作LinkValidationOnly；strong-production/6沿用强定义section自己的三个sink。

所有 source Cone、trusted core、single-file 与 cache 产物都写新 profile、五个新增section及strong-production/6，空集合也必须显式编码。旧 profile 可以被 Graph view 识别并报告，但不能成为本阶段 completed dependency；core receipt、compiler compatibility 与 cache key 绑定新 profile fingerprint，全部重建，不做内存升级。section自身major与outer schema是两个版本维度；本次strong-production/6不表示进入M24的outer schema2或runtime release ABI。

#### 3.1.1 HIR source authority transport

`cross-cone-type-semantics/1` 是待验证的发布候选，不能同时充当它自己的 source authority。只把其八张表放入 artifact 会丢失 private backing field 的真实 source type、独立 required inventory、实际 committed-use occurrence 及其 access/receiver provenance；prebuilt/cache reader 因而无法只凭 artifact bytes 重建 11.1 的第一段证明。为避免 production 时验证一次、读取时改为信任 candidate 的降格，新 profile 必须同时携带：

```text
CrossConeTypeSourceAuthoritySectionV1 {
    foundation: TypeFoundationSourceAuthorityV1,
    declarations: TypeDeclarationSourceAuthorityV1,
    defaults: TypeDefaultSourceAuthorityV1,
    committed_uses: CommittedTypeUseSourceAuthorityV1,
}
```

四个字段依次使用field 1～4。它们是 source-side canonical transcript，而不是第二套可被consumer直接查询的语义表：

- `foundation` 精确保存真实 `source_roots`、本地fact inventory、dependency fact refs、每个exact的fact shape、required representation owner与其source representation/access snapshot、本地inheritance edges、object backing/generated identity refs及accessor role keys；其中identity、definition origin与source context引用仍必须命中同artifact既有HIR foundation，不复制或改写其canonical key。
- `declarations` 精确保存definition-side required protected declaration、inheritance owner/constructor/protected-member/slot-schema inventory，以及每个constructor/callable/property/nested source合同和slot implementation选择。顺序按相应typed key的canonical bytes；同key重复或未被validator查询的额外record均拒绝。
- `defaults` 精确保存protected source parameter calling facts、default profile、typed body/data-flow/operation/reference-access/nested-callable transcript及每个definition-source occurrence。它复用4.3冻结的body、origin、domain和receiver-use constituent，不保存AST、arena index或host path；每个validator query必须唯一命中，全部record必须被消费。
- `committed_uses` 保存每个实际HIR root occurrence和递归semantic edge，而不是field 8的去重target集合。root记录包含稳定definition origin/path内ordinal、`SelectedExternalTypeUseV1`、`TypeSectionCommittedRootOriginV1`及完整lookup/access/receiver witness；edge记录另包含parent request与kind-specific semantic-parent relation。reader按真实root/edge顺序重放，随后要求其去重闭包逐byte等于type-semantics field 8；遗漏、额外、错provider或把support edge伪装成source route均拒绝。

producer必须先从 sealed HIR 投影并封闭source-authority section，再独立构造type-semantics candidate；writer只接受二者已通过production-side交叉校验的typed pair。reader先解析authority，再用它实现`TypeSectionFoundationSemanticAuthority`、`TypeSectionDeclarationSemanticAuthority`、`TypeSectionDefaultSemanticAuthority`和`CommittedTypeUseSemanticAuthorityV1`，完整调用同一个`validate_semantics`入口。禁止从type-semantics candidate、MIR/LIR candidate、symbol或“唯一匹配项”反向合成authority。即使四个子域均为空，该required section也必须存在并按四字段空product编码。

foundation的fact-shape子表是按exact id严格递增的array，每项精确为`{1: exact, 2: source_shape}`；它保存重算事实的输入，不保存candidate的`ExactTypeFactsV1`结论。`source_shape`采用field 0的封闭tag：`Unit=1`、`Scalar=2`、`Pointer=3`、`Reference=4`（均无其他field）；`OrdinaryStruct=5`、`CLayoutStruct=6`、`Tuple=7`的field 1是声明序exact type ref array，tuple至少一个element；`Enum=8`的field 1是声明序variant array，每项为`{1: variant id, 2: 声明序field exact refs, 3: source variant GC fact}`。field类型允许重复，variant id不得重复，声明序不能被canonical table排序改写。所有ref必须通过同一validated identity closure解析；unknown tag、额外field、表乱序/重复、空tuple或共享预算耗尽均拒绝。该子表的wire/identity校验本身不授予事实、layout或selection authority，完整source-authority仍须核对独立inventory、source representation和foundation，并以同一semantic validator重算GC/ZST事实。

foundation的其他inventory同样使用canonical array：source roots按`SourceNominalId`的canonical bytes严格递增；source nominal snapshot每项精确为`{1: owner, 2: DeclarationAccessSourceV1}`并按owner排序，不重复identity key；dependency fact每项精确为`{1: provider, 2: exact}`，按exact id排序，同一exact不得以两个provider重复；本地inheritance edges复用既有四字段`NominalInheritanceEdgesV1`，按exact owner排序，其direct interface序列仍遵守原有canonical规则。producer可排序无序输入，但不得静默去重；reader必须拒绝已有字节中的乱序和重复，不得先排序修复。解析前共享预算必须覆盖table、排序/扫描、输出分配、嵌套source origin与继承边；这些inventory只提供重放输入，不单独授予checked source authority。

`TypeFoundationSourceAuthorityV1`精确使用13字段product，field 1～13依次为：provider、exact key refs、source nominal snapshots、source representation snapshots、generated nominal key refs、accessor key refs、definition sources、source roots、本地fact inventory、dependency facts、本地inheritance edges、fact shapes、required representation owners。key refs只编码既有typed id的canonical集合，不复制key；representation snapshots复用`NominalRepresentationSupportV1`的编码，但必须来自独立source投影。source roots必须精确覆盖source nominal snapshots，representation owner清单必须精确覆盖representation snapshots，fact inventory必须精确覆盖fact shapes；dependency facts与本地facts互斥。每个representation owner必须有同owner的source access snapshot；本地inheritance owner必须精确覆盖这些source nominal的`ExactTypeKey::Nominal` id（先重算、再按exact id排序，不比较不同实体的id）。这些局部一致性检查和wire解析仍不授予完整section authority；来源、access、shape、generated role与全部key内容仍须join既有HIR foundation，并在四域section组合验证中重放。

M23-6专用HIR foundation producer还必须从同一次sealed HIR已登记的generated nominal keys投影对应`ExactTypeKey::Nominal` record，即使该generated nominal尚无实际表达式使用；object backing-class继承证明依赖这条exact relation。该补充属于新profile的producer输入闭包，不修改既有identity key、旧profile投影规则或wire schema。reader不得临时重建缺失record，也不得把旧foundation在内存中升级后用于source-authority验证。

foundation来源记录解析后，reader必须将每个exact、source nominal、generated nominal和accessor key ref绑定到当前artifact实际发布的相应typed identity table，并核对同一validated identity graph中的canonical key；仅在依赖closure中存在同id不够。source nominal的key origin必须等于provider，access snapshot的definition origin必须精确等于该typed声明subject的foundation origin，完整词法owner链必须通过既有access validator。definition-source集合中的每个span必须命中同foundation的source/context/point记录；object representation中的backing class必须join已发布`ObjectBackingClass { object }` key与对应exact key。该绑定凭证只证明source transcript与identity/source foundation一致，不能单独替代公开接口、GC/ZST重算、inheritance/default/committed-use闭包或完整type-section验证。

foundation来源投影统一接收任意Cone的sealed Export/LocalConcrete pair，直接生成source roots、facts依赖、表示snapshot、继承edges和origin集合，不要求构造candidate的constructor/slot/default表，也不需要core source factory、core coordinate检查、来源授权或重算CoreShapeSupport计划。本地与依赖类型按已有typed声明的origin及导入协议区分，不重新证明core归属；普通Cone与core使用相同投影。来源中的generic application只引用HIR已经存在的exact identity与shape，来源投影不执行单态化，candidate的generic ODR执行条件由candidate阶段负责。缺失exact仍报错。非空dispatch及protected constructor不妨碍foundation来源生成，完整candidate的相应合同另行验证。来源transcript的编码、解析与绑定继续使用共享预算和所有Cone共用的metadata一致性规则。

foundation重放适配器按provider组合上述绑定凭证和经旧十表完整validator构造的public-support凭证；每对凭证的provider必须相同，依赖按provider严格排序且排除当前provider。公开nominal必须命中同provider的source root，公开struct/enum的source shape还须与独立representation snapshot逐字段一致。跨artifact重复的exact/generated/accessor key必须内容一致；source nominal与本地fact的所有权不能重复。每条dependency fact必须精确命中其声明provider的本地fact inventory，不能从其他provider补齐。组合后本地inventory仍只来自当前provider，跨Cone的nominal、key和definition-origin查询则经显式typed索引定位；不合并本地与依赖inventory，不凭名称或唯一匹配项推断来源。该适配器只提供完整type-section validator所需的source输入，GC/ZST、inheritance及selected closure仍由原validator验证。

`TypeDeclarationSourceAuthorityV1`精确为九字段product，field 1～9依次为required protected refs、完整nominal合同、完整constructor合同、完整property合同、完整callable合同、inheritance inventory、interface dispatch来源、slot选择来源、dispatch callable合同。各字段复用既有canonical constituent及typed resolver，不复制identity key，不另存递归nested候选：nested source_support由平铺的完整来源合同和精确词法child/member清单重放。required protected refs独立从sealed HIR投影，绑定时与完整来源重算的全集精确比较。参数calling facts仍由defaults域的完整nominal参数表拥有；declarations绑定必须显式接收该独立表并组合既有参数证明，不允许省略或以候选参数表替代。空域也必须编码全部九个字段，reader拒绝缺失/额外字段、未知身份及任一子表的乱序/重复。

该域producer只接收sealed Ordinary HIR pair，先独立发现source roots，再投影完整nominal、constructor/property/callable（含accessor及enum variant）集合和四张dispatch来源表，不读取public/type-semantics候选。artifact重放在同一foundation、trusted-core协议与预算下依次绑定nominal、member、constructor、参数和dispatch/slot来源，完成两个来源域的组合及required protected全集对照后，才在回调作用域内交付声明authority；任何失败都不得执行回调或泄露部分proof。借用链不以自引用容器、unsafe、重复绑定实例或新预算规避。该域自身仍是source transcript，完整四域section及候选事务继续拥有defaults、use和执行资格闭合。

declarations中的slot选择来源子表按`(owner exact id, slot id)`严格递增，每项精确为`{1: owner, 2: slot, 3: selection}`。selection采用field 0的封闭tag：`Abstract=1`无其他field，`Concrete=2`和`InterfaceDefault=3`的field 1复用既有`InheritanceCallableDeclarationV1`，保留Function/Getter/Setter的不同typed role。相同slot在不同exact owner下可有不同选择，同owner/slot重复则拒绝；reader不排序修复已有字节。来源选择必须从sealed HIR实际实现决策独立投影，解析只验证typed ref、canonical顺序和共享预算；角色、owner、abstract/concrete合法性、interface default冲突及完整inventory消费仍由declarations与既有inheritance validator交叉核验。

slot选择producer独立接收sealed Export/LocalConcrete pair，并在构造inheritance candidate之前执行。class/object按virtual family读取最近覆写，包含abstract与FinalOverride；interface自身对完整有效成员保留Body/AbstractSlot决策。class/struct/enum/object的接口选择直接读取sealed `InterfaceImplementationTarget`，`Subclass`仅允许abstract class，实际Method application的owner为interface时记录InterfaceDefault，其他名义owner记录Concrete；不能重新按名称/签名选择default，也不能从MIR thunk、trap或candidate还原source declaration。同owner的多个interface role引用相同slot时必须选中相同typed source目标，冲突拒绝，来源投影仅将一致的重复路径合并成一条记录。所有继承及目标application必须通过param-free exact检查；Function/Getter/Setter引用取自真实sealed source identity，generated callable不得冒充source function。图遍历、合并、排序和输出容量共用预算。

派发callable来源合同子表按`InheritanceCallableDeclarationV1`的typed role与id严格递增，每项`InheritanceSourceCallableV1`精确为`{1: declaration, 2: signature, 3: modality, 4: declaration_access}`。signature复用完整`InheritanceCallableSignatureV1`，分别保留声明自身的exact receiver、源码参数序列、结果与完整effects；modality复用Final/Open/Abstract/InterfaceDefault，不能把interface default按函数体统一降成Open。access复用`DeclarationAccessSourceV1`，getter/setter各自引用其真实accessor definition origin与visibility，词法owner链来自其logical property。所需集合恰为独立source schema的原始slot声明与实际Concrete/InterfaceDefault目标的并集，private helper、无关direct方法和未选中的实现不加入。producer从同一sealed HIR直接读取每项合同，不能从public-only callable候选、slot候选或MIR签名回填；公开表面遗漏的protected槽同样必须完整投影。reader不修复重复/乱序或替换未知typed ref，嵌套签名与access来源共用预算，完整validator仍须把每项来源合同、origin和实际消费集合交叉闭合。

本地派发来源绑定组合上述四张独立子表与已绑定的foundation来源。inheritance owner必须精确覆盖foundation的本地继承边；interface来源必须精确覆盖其中的interface owner；slot选择必须精确覆盖每个owner的schema槽集合；callable必须精确覆盖根槽声明与实际目标的并集。Function、logical Property、Accessor和DispatchSlot key只能借用当前artifact实际持有且已在同一validated identity graph核验的记录，不能用依赖graph中恰好存在的id补足。每项callable的definition origin必须与该Function/Accessor的foundation subject逐项相等，access词法链与signature exact ref也须闭合。绑定全过程共享预算；成功结果只提供独立schema与来源合同查询，仍须通过既有继承图、schema、slot、default、protected和完整type-section组合验证才能获得machine-use资格。

槽合同来源重放从已绑定dispatch来源和已验证导入的trusted-core fundamental protocol组合，不接受调用方自行选择Unit。Unit exact由已有typed Unit声明直接派生，不再次核验core归属或要求重复artifact来源；所有继承图、schema、access及签名检查共用预算。单槽重放与完整inheritance表复用同一入口：先验证typed slot身份、receiver、参数、getter/setter角色、访问域和继承适用性，再把根与目标的完整signature/effects/access/modality及实际implementation选择逐项join独立来源。该单槽凭证只证明指定owner/slot的合同与选择，不替代整表完整性、constructor/protected/default闭包或machine-use资格；不能用graph上可适用的另一base实现替代sealed HIR的实际override选择。

checked schema结果保留已遍历的完整interface conformance集合；interface owner即使只输出自身role，也可选择该集合内祖先interface的default。实现适用性查询此集合并计入预算，不能要求interface source schema额外输出所有祖先role。protected root槽的protected override保留root protected region，按语言9.1.5与实现规范2.2重放slot覆盖；public root仍拒绝protected实现，impl owner本身较窄的direct lookup不改变slot覆盖。

构造器来源合同子表按`PersistentConstructorId`严格递增，记录复用`NominalSupportConstructorInterfaceV1`的三字段wire（declaration、declaration_access、payload），保留真实owner、源码参数名称和类型顺序、结果、effects及source-interface引用。所需集合从sealed HIR的param-free root直接提取public/protected source constructor；internal/private构造器、generated零参数adapter与object初始化入口不属于该集合。producer不能借用public callable或inheritance候选的合同，必须独立读取source parameter interface、typed constructor identity与definition origin。构造器上的default/vararg只在本表保留其参数值类型；省略调用协议及default正文继续由独立source-interface/default来源域和完整validator闭合，不能用签名表替代。reader复用既有三字段解析器，拒绝重复或乱序，不修复已有字节；投影、解析、分配和嵌套签名解析共用预算。

构造器来源绑定必须精确覆盖独立inheritance inventory中每个owner的constructor集合，同一constructor不得归属两个owner。inventory owner自身须精确覆盖同foundation的本地inheritance edges；每项constructor key须由当前artifact持有且通过绑定foundation所用的同一identity graph核验。payload owner、access词法owner和constructor key owner必须共同指向inventory指定的param-free class/struct；source visibility仅接受该清单允许的public/protected，parameter type序列逐项等于typed constructor key，result等于其owner的nominal signature。definition origin必须精确命中`Constructor(id)` subject，不能以同文件的其他有效origin替换。所有比较、索引及递归signature检查共用预算。该凭证只提供绑定后的来源合同；参数名/default协议、source effect/body、继承可调用性与完整候选验证仍须经各自来源域和组合validator闭合。

declarations的inheritance inventory按owner exact id严格递增，每项精确为`{1: owner, 2: constructor refs, 3: protected member refs, 4: slot schemas}`。三个子表复用既有canonical constituent；constructor refs只列可由inheritance surface引用的public/protected构造器，protected member refs不得混入Constructor，slot schemas保留每个role内部的声明槽序。同owner重复、已有字节的乱序和子表预算耗尽均拒绝。producer直接遍历sealed HIR的声明与dispatch关系生成该清单，必须先于inheritance candidate构造；不能从candidate的constructor/member/schema表回填清单。owner集合由来源清单唯一确定，完整validator再要求其精确覆盖foundation规定的inheritance owner，以及candidate的同三类输入，不能把空清单作为省略这些证明的默认值。

source inventory投影入口独立接收sealed Export/LocalConcrete pair，不要求先构造public/candidate section。param-free public source root须逐项join两份HIR的exact identity；generic source root只保留在foundation source metadata中，不伪造exact inventory。class/object沿真实base链投影完整virtual family序列，Direct方法不占槽，FinalOverride复用原family；interface只输出自身role，class/value/object输出传递conformance中的每个interface role。interface schema按实现规范2.9先父后子的声明遍历及typed override抑制规则生成，包含独立getter/setter slot，排除private helper。slot identity直接读取sealed dispatch relation，不能从名字、签名或candidate重建。投影的图遍历、输出容量和深度使用同一共享预算；环或预算耗尽必须在返回清单前拒绝。该来源投影本身不授予完整type-section或machine-use资格。

interface dispatch来源子表按owner exact严格递增，每项`InterfaceSourceDispatchV1`精确为`{1: owner, 2: parents, 3: members}`。parents是无重复的直接父interface exact声明序，不能用canonical排序的inheritance edges替代；members是当前interface直接声明的slot序列，每项精确为`{1: slot, 2: overrides}`，overrides复用按slot id严格递增的canonical ref集合。private helper不在members中；同owner内重复parent/member和自override拒绝。同一typed override经多条继承路径到达时，producer按关系集合合并重复路径，再生成overrides集合；这不允许reader修复wire中的重复ref。source producer从sealed HIR的parent应用、直接member和typed override边独立生成，reader只解析typed ref、顺序、形状与共享预算，不借此授予调用资格。

schema validator必须将parents的集合与已验证继承图逐项join，并核对每个直接member的canonical slot key、interface owner和role。按父接口声明序组合完整未抑制的来源成员序列，菱形路径按slot identity去重，再追加当前members；每条override只允许指向完整继承来源集合中的同role槽。沿全部来源边合并抑制集合，最后一次性移除被抑制槽，逐项比较candidate序列；不能只合并父接口已抑制的schema，否则菱形的另一条路径可能重新引入旧槽。遗漏、额外、重排、错owner/role或非祖先override均拒绝，所有展开及比较共享预算。此证明只覆盖slot结构与顺序，签名/effect、implementation选择、default及access仍由完整declarations和inheritance合同验证。

protected callable来源子表复用`ProtectedCallableInterfaceV1`三字段record，按`CallableTemplateOrigin`的typed role与id严格递增，精确覆盖独立inheritance inventory中的Callable成员。producer直接读取sealed HIR的普通方法、getter与setter；stored accessor即使没有实现函数也须投影，public getter的protected setter只加入setter。参数名称/值类型、binder及bounds、result、effects、modality、source-interface和真实virtual-family关系均保留；FinalOverride保留原槽，Direct无槽，constructor、logical property及nested nominal由各自合同拥有。generic方法仅保存source metadata，不授予M23-7物化资格。definition origin对Function/GenericFunction/PropertyAccessor分别取真实subject；不能从public-only接口或dispatch候选反推。producer与reader的类型树、名称、集合、身份解析共用预算，reader拒绝乱序/重复/未知ref及多余字段；签名来源记录仍须结合property/default、artifact绑定和完整declarations validator验证。

完整nominal callable来源使用独立`CanonicalNominalSourceCallablesV1`，按`CallableTemplateOrigin`的typed role与id严格递增，record复用`NominalSupportCallableInterfaceV1`三字段product，保存Function、GenericFunction、Accessor及VariantConstructor。producer接收定义侧独立required typed declaration集合，逐项从sealed Export HIR投影，拒绝缺失来源、foreign/top-level/extension成员以及普通Constructor等其他role；不读取public或nested候选。全部声明visibility、完整词法owner、真实subject origin、host/own binder区分、参数名与值type、result、effects、modality及真实slot关系均须保留。generic host的访问器使用host binder但没有own binder，static nested不捕获外层binder；const不生成accessor。interface default/abstract成员按真实interface member relation投影其slot，private helper固定Final且无slot。variant使用真实typed variant identity及origin、enum host binder和源码字段顺序，保留Named/Constructor参数名称；default协议引用由独立parameter来源保存并在完整declarations事务中join，不伪造function key。与旧protected表重叠的来源共用投影，再由protected wrapper独立核验其封闭visibility/role/modality约束；不扩展旧表wire。reader拒绝重复/倒序、未知typed ref、多余字段与预算耗尽，复制、签名、参数、来源和dispatch查询共用预算。完整来源仍须与nominal成员/variant清单、属性、参数/default来源及artifact foundation在declarations事务中join，不单独授予lookup、ODR或执行资格。

完整nominal成员来源绑定在同一事务中接收已绑定nominal合同、完整property来源、完整callable来源及已验证导入的core fundamental protocol。property所需集合由nominal成员清单独立决定；callable所需集合为Function/GenericFunction成员、enum source shape的全部variant及runtime property的完整getter/setter并集。两表必须精确覆盖该集合，并逐项匹配owner；const不取得callable记录。function/generic function/property key只能借用同artifact实际持有且在同一identity graph核验的记录，accessor与variant使用各自typed key，不能伪造function key。runtime property的getter/setter全集还须与artifact自有accessor key按logical property逐项相等；const允许内部身份表保留getter identity，但不能有setter或生成source accessor能力。

该绑定重放已有support callable/property语义：完整词法访问源与真实subject origin、host/own binder及bounds、参数/result、variant字段及enum self result、property逻辑type、setter访问域与representation必须闭合。Unit和const integer/Boolean/String的type直接使用已有导入角色，不再为每个消费者重复检查core归属，const origin独立join artifact Property subject；generic owner或generic词法链的property保留GenericSourceMetadata凭证。所有callable通过语义验证后，runtime property逐项join其getter/setter的访问源、签名、modality以及slot refs，property的slot集合恰等于accessor槽并集。来源绑定不授予lookup/default/ODR/machine-use，也不替代独立dispatch来源对slot schema、根与实际实现选择的完整重放。全部集合、身份索引、签名、origin、const key拷贝和语义查询使用共享预算；任一表失败时不能发布部分成员绑定。

完整nominal构造器来源使用独立`CanonicalNominalSourceConstructorsV1`，按`PersistentConstructorId`严格递增并复用`NominalSupportConstructorInterfaceV1`三字段product。producer接收定义侧独立required constructor集合，逐项从sealed Export HIR读取所有visibility的class/struct源码构造器；generic及static nested保留真实直接owner、完整词法链与host binder，constructor own binder、receiver及slot为空。参数名称、源码值type及顺序须与同一typed constructor key一致，结果为真实owner self type/application，Safety与GC effect来自已验证声明；primary struct的内部NoGc组装不冒充源码NoGC合同。generated adapter、object初始化入口、foreign或缺失来源不能补足required集合。继承来源表复用同一投影，但仍由独立inheritance inventory限制为param-free public/protected入口，重叠record字节相同。reader拒绝重复/倒序、未知typed ref、额外字段及预算耗尽；所有扫描、集合、binder/signature、参数和origin投影使用共享预算。该表仅保存独立source合同，完整declarations仍须将其与artifact foundation、nominal constructor清单、参数/default来源逐项join；generic metadata不授予ODR或machine-use资格。

完整构造器来源绑定直接借用已绑定的nominal来源凭证，以其完整constructor清单独立重算所需集合并与来源表精确相等。每个constructor key必须由同一artifact实际持有，并在该foundation使用的validated identity graph核对；owner、完整词法链、真实Constructor subject origin、参数signature与owner self result逐项闭合。generic signature仅使用直接宿主的binder scope，不能捕获static nested外层参数，且重放全部signature nominal kind/arity。构造合同固定Ordinary、Scoop implementation、非operator且非infix；class构造器必须Managed，struct保留已验证来源的真实GC effect。成功绑定只提供独立source查询，不能凭来源中的Safety/GC标记跳过后续候选合同逐项比较、default验证或generic能力门禁。索引、来源、签名递归及语义查询共用预算，缺失或多余记录、异owner、错误origin/signature/effect均拒绝。

完整nominal参数协议使用独立`CanonicalNominalSourceParameterProtocolsV1`，按typed callable owner严格递增；支持Function、GenericFunction、Constructor、VariantConstructor，accessor没有独立源码参数协议。每个`NominalSourceParameterProtocolV1`保存owner及声明顺序的参数，每个参数沿用shape、Required/Default/VarargEmpty/VarargDefault和真实definition origin三字段product；零参数声明也必须有记录。producer只接受定义侧独立required集合，从sealed Export HIR的真实参数接口核对source signature或enum variant字段，保留host/own binder区分、vararg的Array值类型及参数origin；缺失、重复、foreign和非nominal来源拒绝。旧inheritance参数表通过封闭role wrapper复用同一投影与wire record，仍拒绝VariantConstructor；不能为新增角色扩大旧表的接纳范围。解码、排序、签名、参数和origin投影共用资源预算，reader拒绝未知身份、重复/倒序和多余字段。calling kind只声明省略/展开协议，default body、hygienic template和执行许可仍须由完整defaults/declarations事务独立重放，不能由这张表自行授予。

default正文投影核心与public lookup envelope分离：核心只读取sealed default source/expression、真实provider scope和目标binder映射，输出完整typed body、locals、receiver/value-parameter selector、result、suspend能力、binder uses、definition root/path/origin，并保留原始reference来源供各自域继续投影。public模板继续按原public callable/access表生成reference envelope；protected/source-authority入口不能借用该public表替代自身来源。二者复用同一正文投影逻辑，不能复制另一套expression lowering；共享核心的输出本身不提供reference access、资源事务或完整source-authority资格。

共享正文投影的资源上下文必须借用调用方唯一`BudgetMeter`，类型展开沿用sealed HIR签名预检；表达式、语句、pattern及binding shape在递归前检查depth/node/work，集合、selector/path、字符串和definition origin在分配或拷贝前计费。provider binder和source type arguments也属于本次投影预算。资源错误保持typed错误，失败不返回部分正文。public入口可在最外层建立默认预算，来源入口必须继续传递所属section的共享预算；该正文计费不替代后续reference transcript或semantic replay计费。

独立正文producer按sealed source parameter owner与声明序position选择默认值，Required或VarargEmpty位置必须明确拒绝；同owner的source parameter interface缺失或重复也拒绝。目标scope与provider scope分别从真实声明推导，继承default保留provider root/path与类型参数代换。该production intermediate保留完整typed正文和借用的原始reference来源，支持所有声明visibility及普通导入sidecar，但不编码为source-authority、不生成空reference envelope，也不授予访问或调用资格。后续defaults域必须继续投影原始references、default profile和独立semantic transcript。

默认目标访问所需的声明来源使用独立`CanonicalDefaultSourceAccessDeclarationsV1`，不受nominal public根清单限制。每项精确为`{1: subject, 2: declaration access}`，subject复用DefinitionOriginSubject的既有typed tag，但封闭为Type、GenericType、Function、GenericFunction、Constructor、Property、ExtensionProperty、PropertyAccessor八种；不为variant、field、singleton或generated/local引用伪造声明，它们后续按真实typed key映射至权限所属声明或专用正文合同。表按subject的role/id严格递增，access复用声明visibility、完整词法nominal owner与真实subject origin，不能由reference witness的target域反推。

producer接收定义侧独立required subject集合，从sealed Export HIR读取实际声明，并自动补齐完整词法owner链（包括static nested之外的owner）。支持top-level、extension及全部visibility；generic source只保留原声明身份。仅当前artifact实际定义的声明可补足required集合，foreign、缺失、非声明role、const accessor或含callable owner的local声明须拒绝；generated构造入口不能冒充source constructor。getter/setter分别保留自身visibility及PropertyAccessor origin，logical property与extension property保留各自subject。producer不扫描candidate section、默认witness域或public-only接口。所有源扫描、集合、owner链、origin拷贝及reader解析共用预算；reader拒绝重复/逆序、未知typed ref、其他subject role、多余字段和预算耗尽。此表只是访问来源transcript，仍须在完整defaults事务中与实际artifact key/origin、独立需求闭包、重叠nominal/member来源逐项绑定并重放target-domain；单独读取成功不授予访问、profile、default展开或执行资格。

默认目标声明的artifact绑定显式接收定义侧独立required subject集合；以artifact实际持有且在同一validated identity graph核验的typed key重算完整词法nominal owner闭包，不能从来源record自报的owner链反推需求。来源表须精确覆盖该闭包，缺失或多余记录均拒绝。普通声明分别join其Type/GenericType/Function/GenericFunction/Constructor/Property/ExtensionProperty key；PropertyAccessor先核验自有accessor key，再按其真实Property/ExtensionProperty owner取得并核验逻辑声明key，不能构造替代function key。所有key必须属于当前artifact的provider；依赖图知道某id但artifact没有发布它仍须拒绝。

每条访问来源必须精确匹配同artifact真实subject的definition origin，并核验source/context/span点、声明scope source、完整词法owner顺序、owner来源文件和protected最近owner为Class。词法owner来源可位于public nominal根之外，但仍必须由同一事务绑定；与foundation已绑定nominal来源重叠时，visibility、owner链与origin整体必须相等。全部需求展开、身份索引、canonical key核验、来源查询、哈希及access语义重放使用共享预算；任一记录失败不发布部分绑定。artifact可保留const getter的内部identity及origin，这些事实不构成runtime accessor需求或能力；独立来源需求仍排除const accessor，完整属性合同负责核验runtime分类。此凭证仅绑定声明访问来源；完整defaults事务仍须join重叠member/constructor来源、映射六类实际target、验证profile、receiver与访问域覆盖，不得提前授予default展开或machine-use资格。

已绑定目标声明的source lookup域与原provider direct域共用同一可见性重放规则：本声明visibility与完整词法nominal owner链逐项取交集，支持八类声明及public根之外的来源，不从witness或public接口反推域。top-level Private保留Cone/File，member Private保留最近LexicalOwner，Internal保留Cone；每一层Protected均要求已绑定owner的真实source key为Class。concrete class的SubclassesOf必须join同artifact已发布并在同identity graph核验的`ExactTypeKey::Nominal`，仅能推导出exact id而缺少artifact来源仍须拒绝；generic class保存typed generic约束，不能用某个application exact替换。

源码Constructor访问来源的直接owner必须是实际Class或Struct声明。object/companion初始化入口即使保留Constructor identity及definition origin，也不是源码可选择构造器；producer拒绝把它投影为访问声明，artifact绑定按已核验owner key拒绝伪造来源。其内部初始化权限不得按普通private member域重放；singleton引用须按实际object声明及初始化合同单独闭合。

默认引用的间接权限主体先由artifact自有identity记录确定，再交给声明访问来源绑定：StructField指向其实际struct声明，EnumVariant指向实际enum声明，Singleton指向实际object声明；ClassField按backing/delegate key指向逻辑Property，不继承getter或setter权限。object backing field必须沿实际GeneratedNominal的ObjectBackingClass关系回到source object，再核验property的直接owner。每一步都要求本artifact实际发布的key与同一validated identity graph相等，并核验nominal种类、property owner及provider；不接受只有dependency graph知道的key，也不能从reference自报owner type、名称或候选域推导主体。查询与key检查使用共享预算。此映射可用于独立required声明集合发现，但只证明身份关联；字段representation、constructor/member合同、applied owner type、访问覆盖及receiver仍由完整引用事务验证。

构造引用按真实typed目标重放权限主体：Struct/Class source入口必须命中本artifact的Constructor key，其直接owner分别为Struct/Class；Variant沿实际variant key使用Enum声明。Class Generated分支只允许同artifact、同identity graph中的`ZeroArgumentConstructorAdapter`，随后必须沿其typed Constructor id命中真实Class source构造器，不能从adapter的调用owner或签名猜测来源。每一分支再核对引用owner type的nominal身份与声明owner相等，Concrete使用Nominal，GenericTemplate使用自身arity完全一致的NominalApplication；static nested之外的generic词法链不增加application实参数。owner根匹配不替代完整type argument/binder语义、source参数协议、adapter可执行合同或访问覆盖。对象初始化入口、错误generated role、缺少任一artifact key、错误nominal kind、owner身份或arity均拒绝，查询和typed边共用预算。

字段引用沿相同的artifact自有field→source声明关系核对applied owner根与自身arity。object字段的物理identity仍属于实际ObjectBackingClass，正文SignatureTypeKey则按既有HIR投影使用source object；两者必须经同一个真实generated key关联，不能接受backing nominal冒充source object类型。Tuple字段没有source声明权限主体，返回携带原始位置的独立TupleElement分支；该分支只分类structural访问需求，不能直接当作Universal权限或已验证字段，完整事务仍须检查receiver tuple shape、位置范围和结果类型。source字段同样仍须完成representation、receiver与操作类型验证，不能由owner根匹配授予访问资格。

身份路由必须遵守foundation既有canonical依赖顺序：声明以及带同表依赖的generated记录不保证按ID递增。直接借用这些记录的查询不得假定ID有序而二分查找；无独立索引时逐项定位，并将完整表长的比较成本计入共享预算。读取命中后仍须与同一个validated identity graph核对真实key。

callable访问需求按真实artifact key分类：普通Function/GenericFunction与PropertyAccessor保留各自声明subject，accessor必须先join实际Property/ExtensionProperty key；局部Function/GenericFunction须同时具有LexicalScoped和直接callable owner，返回LocalFunction正文合同需求。generated callable只有真实Lexical LambdaBody、AnonymousFunctionBody或CallableReferenceInvoke可映射至对应nested身份；显式LocalFunction/Lambda/AnonymousFunction/CallableReference target的种类必须与该映射精确一致。Bound只提取其实际callee/member权限需求，FunctionAddress同样保留实际callable身份；这不验证bound receiver、函数地址资格、applied owner或signature。DerivedEquality保留原始owner type作为专属结构化需求，不能伪造Function声明或直接当作Universal。global引用必须命中本artifact实际top-level Property key，拒绝member或lexical property；runtime storage/const分类仍由完整property合同验证。所有声明key核验provider、同一identity graph与资源预算，nested结果还必须join已绑定的完整descriptor、parent/path、捕获和ABI来源，分类成功不授予访问或执行资格。

组合type的访问需求按SignatureTypeKey结构前序遍历，保留每次出现及其完整wire path，不按相同identity合并。Nominal保留原typed id，NominalApplication同时借用完整实参数组并逐项递归；Tuple、普通/挂起Function递归全部元素、参数与结果。RawPointer与NativeFunctionPointer先报告独立wrapper需求，再遍历pointee或完整native参数/结果；后续来源绑定必须用已验证core角色取得实际Ptr/FunPtr声明，不能把wrapper当作Universal或只检查其内部类型。Binder保留depth/index需求，由原provider frame验证；Unit/Any仍以原Nominal报告，是否为compiler builtin须由真实canonical来源判断。遍历不复制类型树、不建立权限证明，callback沿用同一预算且失败立即停止；节点、边、表长、递归深度及查询工作均须受限。完整引用事务随后绑定这些需求的本Cone/依赖声明并重放域交集、arity和binder语义。

type来源域查询显式接收当前已绑定声明表及按Cone严格递增的依赖表，拒绝重复、当前Cone冒充依赖及不同identity graph。每个source nominal先以同一graph的canonical key定位provider，再要求该provider的实际已绑定声明表命中对应typed subject，并核验nominal种类与自身arity；static nested外层qualifier不增加应用参数。Binder逐项核验调用方提供的原provider SignatureBinderScope，空frame不占depth。Unit/Any使用已解析的语言内置类型身份，不增加声明访问约束；其他nominal按普通声明可见性计算。Ptr/FunPtr直接使用前端或依赖导入得到的typed角色，不再次接收core artifact、验证core归属或重建角色绑定；其声明访问域与其他nominal共用查询。各组成域按原始约束取交集并canonical去重，generic subclass约束完整保留；不做继承归约，也不因已得Empty域而跳过后续来源查询。所有路由、key检查、scope检查、域复制、排序和交集共用预算，失败不交付结果。该查询只产生原始source域，完整事务仍须证明所提供scope与原声明关联、操作类型、receiver、profile和覆盖关系。

nominal default的type引用绑定只接收BoundNominalDefaultDeclarationsV1，复用其原provider声明、nested identity及精确occurrence闭包；当前access表必须来自同一份bound foundation。逐occurrence选择原provider scope，LocalFunction metadata attachment才增加同一graph内已验证函数key声明的自身非空frame，不能从descriptor的binder出现情况推断。每次独立重放完整target域并与其source witness逐项相等比较，保留模板key和type序号的错误定位；重复target、继承发布与空引用模板均不能跳过事务预算。全部成功后交付借用原声明凭证的独立BoundNominalDefaultTypeDomainsV1，不复制正文或借用witness反推来源，也不授予其他target、receiver、profile或执行资格。

foundation投影对所有Cone共用入口，按实际typed声明归属处理本地和依赖类型；移除core专用来源入口及其CORE、Defined协议、shape-support授权检查。generic物化和ODR是独立的代码生成语义，不用于建立core信任链。

重复约束canonical去重，完整外层visibility保持不变，static nested边界不得丢弃外层generic约束。公共成员和private setter各按自身声明visibility重放，外层protected区域不自动产生成员receiver限制。查询、哈希、域分配、交集去重与编码排序共用预算；结果仍是原始source域，不因已有继承图而提前删减约束，也不授予checked lookup/slot、protected receiver或default执行资格。六类正文target到权限所属声明的映射、组合type域和candidate witness比较仍由完整引用访问事务负责。

defaults来源域的访问snapshot必须保存原始default provider的完整call-domain与target-domain，不因继承而提前改写为发布owner。`DefaultSourceAccessDomainV1`为二字段product：field1复用冻结的`PersistentAccessDomainV1`，保存Cone、File、LexicalOwner及具有真实source exact的SubclassesOf约束；field2是按typed id严格递增的generic source class id集合，表示其尚未具体化的SubclassesOf约束。两部分取交集；Empty不得携带generic约束，Universal且无generic约束才表示全域。泛型class不能伪造concrete exact；generic集合仅保存实际sealed约束，不由owner binder或候选profile反推。读取方拒绝重复/逆序、缺失/额外字段和未知身份；source key为class、exact/key/source所有权以及完整域语义须在后续来源绑定中核验。该snapshot不提供PersistentLookup/Slot的checked凭证，也不决定default profile。

`DefaultSourceAccessWitnessV1`为四字段product：field1原始provider的`CallableTemplateOrigin`，field2直接调用来源域，field3可选槽来源域（tag1 Absent、tag2 Present且field1为来源域），field4目标来源域；Absent与Present(Empty)不可混同。Accessor不能作为默认值provider，constructor/variant不能携带slot域。producer只从同一次sealed HIR的`ExportDefaultAccessWitness`投影，原始单个slot覆盖域仍须与独立完整root-slot集合逐项join，不能把它当作完整slot inventory。source domain的partition、源文件path复制、canonical排序/编码及全部identity解析共用调用方预算；wire与projection成功均不授予reference access、receiver、source profile、default展开或selected资格。

默认来源reference闭包以完整typed body visitor为统一遍历协议：先按canonical local selector顺序访问local类型，再按正文结构前序逐项访问表达式与metadata；每个实体内的类型、capture、callable/constructor/field引用顺序由同一visitor固定。source HIR collector须遵守同一协议，source projection保留六类原始序列，不在reader排序或搜索匹配项。reader为每一类独立维护单调cursor，将正文occurrence与下一条来源record的完整typed target和definition origin逐项相等比较，并保留expression index或metadata attachment、该类来源序号及借用witness。重复记录也须一对一消费；缺失、多余、乱序、错误target或origin均拒绝。只有全部六类cursor到达末尾才交付闭包凭证，遍历、类型比较、origin比较及索引分配共用原预算。此凭证仅证明来源与正文的逐次对应，不能替代独立profile、target来源、访问域、receiver或nested ABI验证。

默认来源引用闭包还须从同一正文独立建立表达式前序索引，并以共享receiver投影逐occurrence保存不可缺失的上下文：Expression分支同时拥有expression-use与实际None/Member receiver，Metadata分支保留原始typed metadata，不能用缺失expression-use冒充无receiver。member field/method与direct-super保留实际receiver节点；仅当该节点直接读取模板receiver local时投影ImplicitThis，其他节点必须用完整正文索引投影Explicit，并单独保留direct-super标记。type、global、singleton、普通无receiver调用和ClassInit使用None；ClassInit不生成ConstructorDelegation。source与protected candidate共用此投影规则，索引只用借用节点地址定位本次遍历位置，不成为persistent identity或wire字段。两遍正文遍历、索引分配与查询共用原预算；空正文引用集合也不得跳过结构预算。该上下文仅证明receiver-use的正文来源，完整target权限、protected receiver合法性与域覆盖仍由访问事务验证。

默认来源声明绑定从原provider已绑定的声明access和完整词法owner链独立重算direct call-domain，variant直接采用其enum的有效lookup域。Public不增加约束，Internal增加Cone，Private增加最近LexicalOwner或top-level的Cone/File，Protected增加实际Class的SubclassesOf；concrete class使用同artifact发布并验证的source exact，generic class保留独立generic约束。重复约束按交集语义合并，但不删除外层visibility约束；static nested的binder边界不截断visibility链。结果按既有source-domain格式canonical化，并与六类reference occurrence的原provider direct域逐项相等比较；空reference集合也重算并保留结果。继承default不能改用发布owner的域。查询、域复制、去重、编码、排序与比较共用原预算；该来源证明不授予target/receiver访问、完整root-slot coverage、profile或展开资格。

默认来源的原provider槽合同按语言8.5.1独立重放：override不得重新声明默认值，因而首次来源只能是无槽的Final声明或自身首次声明的Open/Abstract/InterfaceDefault槽。constructor、variant与只有自身generic binder的direct function不产生槽；private interface helper保持Final/Direct，不能仅因owner是interface而生成slot域；非Final函数必须恰有一个对应自身typed Function身份的VirtualMethod或InterfaceMethod槽，role由真实source nominal种类决定。槽key必须由同一artifact实际发布且通过相同identity graph验证，不能仅凭候选slot关系或已知id接受；指向其他声明的virtual family不能冒充原default provider。首次声明槽的域等于该provider已独立重放的完整effective lookup域，包括全部外层visibility及generic约束。六类source reference的Absent/Present和Present域逐项精确匹配，空reference正文仍保存provider的Direct/Slot判定；继承default始终检查原provider，不能换成override的声明域。该证明仅闭合source snapshot的原始单槽，不替代发布owner全部root-slot的枚举、target coverage或实际override/default唯一性证明。槽身份、域比较及所有查询共用原预算，不改变冻结wire。

`DefaultSourceReferencesV1`为六字段product，fields1～6依次为callable、constructor、type、global、singleton、field来源序列。每项`DefaultSourceReferenceV1<T>`精确为`{1: target, 2: definition origin, 3: DefaultSourceAccessWitnessV1}`；target与origin复用既有typed constituent。序列按sealed reference collector的同类遍历顺序保存，序号只表示该类来源出现位置，不表示expression index；metadata引用同样保留。重复出现具有独立来源意义，producer和reader均不得排序或去重。来源顺序、出现次数及完整消费仍须与独立typed正文重放join，单独恢复这些序列不证明闭包。来源正文producer在同一sealed graph与import sidecar上下文内投影这些记录，拒绝witness provider与正文definition root不一致；不得事后将另一个Export HIR的arena id接入正文。六类集合分配、target展开、origin复制与witness投影/解析沿用同一预算，失败不发布部分production。

`DefaultSourceTemplateV1`持久化独立正文producer的完整结果，使用12字段product：field1为既有`ProtectedDefaultTemplateKeyV1`（发布owner与声明序parameter position的语义key，不是candidate表索引），fields2～12依次为definition root、definition path、locals、body、result、allows suspend、binder uses、receiver、value parameters、`DefaultSourceReferencesV1`、definition origin。除独立references外均复用已冻结的default constituent；正文、receiver与value parameter中的local以同record的canonical local table索引编码，恢复后重新得到稳定selector。继承default的key仍属于发布声明，root/path/origin与reference provider保持定义侧身份，binder uses保留目标到provider的真实映射；不序列化HIR arena id或`HirSignatureBinder`。

来源正文的结构检查必须核对result与body结果type、全部local索引以及六类witness与definition root的provider一致；未知身份、缺失/多余字段、错误local index和不一致record均拒绝。索引生成先使用完整typed body遍历检查深度并预收临时索引树的保守存储上界，各indexed wrapper的实际尺寸与遍历节点总数决定此上界；receiver/value parameter集合及selector查找、错误路径复制也计入同一预算。reader在解析locals、body、binder、receiver及origin前沿用既有共享预算预检，不通过未计费的构造器重建已解析正文。production intermediate通过所有权转移生成该snapshot，不复制正文、借用候选模板或创建空reference envelope。该record仅提供后续default profile、provider合同、data-flow、operation、nested callable、origin和完整reference occurrence重放的来源，不单独授予任何checked default或执行资格。

默认来源的声明合同绑定显式接收同一identity graph中已绑定的完整nominal参数/成员/constructor来源；foreign provider通过按Cone严格递增的依赖来源路由，不能仅凭已存在的identity或origin接受provider。事务先验证完整origin闭包，再将每个发布key和原provider root按同一parameter position连接到真实参数协议，双方该位置均须有默认值。nominal来源的definition path必须精确为单个DefaultValue段，其ordinal等于原provider此前具有默认值的参数数；member/constructor/variant各自已是独立definition root，nested nominal也不增加词法段，不能只核对末段而接受伪造的LocalDeclaration、Lambda或其他路径前缀。host/own binder shape、隐含receiver与suspend能力直接从已绑定声明取得；直接来源要求恒等映射，继承来源的完整映射在发布scope内验证，并逐项重放所有参数的provider到发布type代换。正文receiver、此前参数local和result分别在原provider frame中精确匹配原声明，不允许仅靠代换后偶然相等接受错误的raw type。查询、receiver构造、签名比较与代换共用预算，失败不发布部分绑定。此凭证只证明声明合同与来源位置；实际override边、唯一default provider、未使用binder的继承代换、正文/profile/reference-access重放仍须在完整defaults事务中独立证明，不能实现完整default authority或授予展开资格。

声明来源绑定还须在同一事务内检查完整provider类型envelope：全部local selector属于原definition path，全部local type以及完整body遍历中的表达式、capture、binding、iterator、callable owner/type argument和显式binder引用均在原provider frame验证。类型的nominal种类与arity来自已验证identity graph的source key，不能用候选type或其代换后结果充当shape authority。body的类型遍历复用既有完整provider-envelope visitor；origin已由前置独立artifact闭包验证，该类型pass不引入接受任意origin的伪authority。所有遍历、scope诊断、shape查询和类型树验证沿用同一预算，任一失败均不发布部分bound表。该envelope证明仍不代替operation typing、data-flow、override/default唯一性及reference-access回放。

完成origin、声明与类型envelope绑定后，默认来源在同一原子事务中复用完整local data-flow validator：receiver与前置参数作为初始定义，其他local必须在合法控制流中先定义后使用；可变性、循环控制、解构临时量及binding shape/action的完整对应均按Stage5规则重放。struct解构的field ordinal只由同一identity graph中显式路由的已绑定nominal source shape声明序求得；field canonical key的source owner、实际struct owner与shape中的typed field必须一致，不能信任正文projection自报ordinal或借FQN回退。foreign field owner需要提供其artifact的完整nominal参数来源，即使该identity已知也不能省略来源。参数/依赖查询、field序查找与完整data-flow遍历共用原事务预算；失败不发布bound表。这仍不授予default的operation、nested-callable ABI、访问、继承来源唯一性或执行证明。

nested callable来源重放沿用Stage5的typed occurrence查询：独立来源按完整正文descriptor前序保存每次出现，candidate验证以template、`Body { ordinal }`及query种类精确命中，并比较typed identity。相同invoke ID经过不同generic default展开后允许有不同function/capture ABI；不得用identity唯一表合并，亦不得用candidate的function type反选来源。`Standalone`仅服务单descriptor校验，不提供完整defaults域的出现次数或消费证明。该查询协议不修改既有body wire格式，完整transcript仍须精确消费全部来源出现。

完整正文nested ABI遍历同时按源码顺序重放局部函数的词法可见性：每个statement block拥有独立声明集合，内层可读取仍有效的外层前置声明，不能读取后声明或其他branch/loop/when/catch/finally中的声明。根statement与末尾value共享作用域；condition、guard、iterator及component的setup与对应表达式共享独立求值作用域。相同persistent声明可在不同作用域重复展开，仍按各occurrence查询身份与ABI；同作用域重复拒绝。进入/退出作用域、声明集合与查找共用原遍历预算，不改变descriptor ordinal或冻结wire。这项局部可见性证明仍不替代direct-call ABI、完整default-dependency与reference-access证明。

reader从独立来源正文建立借用的nested occurrence索引：复用完整reference遍历，按descriptor前序保留四种callable及其definition origin，不把普通local call、callee引用或capture算成descriptor。索引按来源template及ordinal定位后再逐项核对typed identity，`Standalone`、越界ordinal和identity不匹配均明确拒绝；不按function type搜索。索引不复制正文或ABI树，不合并重复identity，构造与查询继续使用调用方预算。声明绑定事务将同一索引逐项绑定到artifact实际identity记录后交付；这只提供已绑定身份与原始来源descriptor，完整ABI/provenance重放和逐query消费仍是独立义务。

默认来源中的每个nested callable descriptor还须通过完整正文遍历核对artifact持有的typed identity key：LocalFunction仅接受真实LexicalScoped Function/GenericFunction，Lambda与AnonymousFunction分别要求相应Lexical generated role，CallableReference要求CallableReferenceInvoke。descriptor的完整definition path必须逐项等于canonical key；local function的source identity还须等于该occurrence的真实definition source。来源artifact按已绑定occurrence的Cone显式路由，不能只在identity closure中找到同id就接受；查询、key比较与遍历共用预算。这项身份检查不单独证明parent/default-dependency provenance、owner binder、capture/function ABI或访问合同；这些仍由完整defaults transcript重放。

nested descriptor的词法父实体还须与occurrence的真实definition context逐项相等：LocalFunction取canonical declaration owner链的最后一个callable atom，generated lexical body与reference invoke取key中的typed parent。Function、GenericFunction、Constructor、Accessor与Generated分别匹配相同role/id的source Callable context，不能按路径、同文件或签名推断父实体。EnumVariantConstructor沿本artifact实际variant key取得enum owner，匹配Nominal context并要求occurrence span位于该variant的真实foundation definition span内，防止同enum中具有相同default路径的variant互换。File、Property、Initialization和materialized Application context不能冒充源码callable父实体。此检查复用已绑定origin、显式artifact路由和原预算；它证明声明context关联，完整default-dependency边、binder代换和ABI仍须另外闭合。

默认来源nested descriptor的owner type parameter数量由其真实词法父实体的artifact canonical key独立重算。函数与局部函数累加自身binder和最近owner提供的binder，source nominal只提供自身binder并截断static nested外层qualifier；accessor、variant与合法generated lexical parent沿各自typed key回溯。每一层key必须由同一来源artifact实际发布并匹配validated identity graph，查询、节点、边和递归深度共用原预算。四类descriptor的owner count必须等于重算结果，lambda/anonymous的显式body arguments也必须恰有该数量；未出现在签名或capture中的参数不能省略。卫生展开始终读取原声明父实体，不能借当前provider的arity替代。此项证明不代替完整参数代换、default-dependency provenance或ABI内容验证。

命名局部函数的source HIR descriptor保留声明时的完整definition origin；source正文和六类reference来源在卫生展开后仍读取该origin，不从当前template root重新构造文件/context。原有statement-origin与descriptor wire字段保持不变。

局部函数自身非空binder组在其descriptor签名中占depth 0，当前provider frame整体外移一层；自身arity为零时不建立空frame。capture与call-site type arguments保持provider frame，不能随局部声明签名一起移动。type reference collector以typed局部声明Signature target保存这种上下文，正文与reference序列共用映射，冻结wire不增加字段。类型envelope通过独立local-function binder authority取得canonical声明自身arity，不从descriptor owner count或候选签名反推；source实现查询同一validated identity graph的真实声明key，artifact所属权与词法parent仍由同一原子事务的nested identity绑定闭合。generic nominal的generic method内局部泛型签名可有三层frame，全部深度、arity、类型树与查询使用共享预算。

public canonical type reference可去重，但作用域证明按正文typed attachment逐occurrence重放；按完整target与definition origin匹配，不能按类型形状、span或单一命中放宽所有引用。多个匹配作用域必须全部有效，无匹配记录按provider scope检查并继续要求exact closure。此重放及签名比较、binder frame分配共用artifact预算。

局部函数经其他默认值展开进入来源正文时，descriptor签名和capture求值源必须处于当前provider frame，原声明签名单独保留供callee具体化；每次展开的声明、direct call与reference通过typed descriptor映射关联，persistent declaration不变。局部函数自身的type parameter以显式identity mapping保留，只有继承的provider binder参与外层代换，未知binder仍须拒绝。四类nested callable的capture selector从已映射的Local求值源投影；只有真正的Capture读取才按原binding查找。definition origin仍保留捕获的原first-use位置，不能将求值源local的身份误作被提升函数的capture binding身份。此规则不修改冻结的nested descriptor wire。

默认值卫生展开中的函数引用保留source HIR声明方的完整owner arguments，逐层代换后再建立LocalConcrete invoke与父materialization关系，不能借调用方的binder前缀或仅由ABI中出现的类型推断。命名局部函数descriptor同样保存每次出现的完整owner arguments，包含未出现在签名、capture或调用中的参数；其长度给出原声明的owner arity。lambda/anonymous已有显式body arguments同样须组合中间代换。此项修正source HIR到LocalConcrete的内部表示；冻结的default descriptor wire不变，完整defaults provenance transcript仍须独立证明包含未使用binder的映射。

M23-6 foundation producer须从sealed Export HIR的全部callable-reference声明投影CallableReferenceInvoke key及GeneratedCallable definition origin，包含private、generic及未被调用的默认正文；不能只依赖LocalConcreteHir中已物化的引用。投影与default正文共用真实lexical root/path的key构造，已有同id/key/origin必须完全一致才能合并。新增key、origin、排序、集合和来源点计入共享预算；旧profile的foundation投影保持原契约，reader不补造缺失key或origin。

`CanonicalDefaultSourceTemplatesV1`使用按发布owner/parameter position语义key严格递增的array，元素为完整`DefaultSourceTemplateV1`；空集合显式编码为空array。producer可排序无序输入但不得去重，reader必须拒绝重复和逆序，不能通过排序修复字节。其key集合必须精确覆盖独立完整nominal参数协议中Default与VarargDefault的声明位置；Required、VarargEmpty和零参数声明不产生正文。逐项coverage检查拒绝缺失与多余正文，保留继承default的发布key和真实provider区别，不以共享provider合并多个发布位置。这只证明来源集合之间的一致性，不代替参数协议的foundation绑定或default语义重放。

完整nominal默认值来源producer直接从同一次sealed Export HIR的source roots、完整nominal合同和member/constructor/variant inventory推导所需参数owner，再投影完整参数协议与全部默认值正文；不从candidate协议、public default表或调用点反推清单。private/internal/protected/public成员、nested与generic源码owner、源码constructor和enum variant均按真实声明保存；top-level/extension callable仍属于旧public来源路径，不混入nominal集合。普通导入sidecar必须与用于正文投影的同一输出配对。排序、身份索引、参数扫描、正文投影、临时local索引、解码与coverage检查使用同一预算，全部成功后才返回成对的参数来源和正文来源；该production结果尚不构成完整defaults域authority。

独立default来源的origin遍历必须逐次保留root、source-backed local、typed body及六类reference occurrence，并带其具体site与wire path；重复origin不合并，generated local不伪造source origin。body复用完整的typed遍历，包含nested callable、capture first-use、pattern/binding和iterator协议位置；所有回调与遍历共用预算，回调失败立即中止。M23-6专用HIR foundation producer从同一次完整nominal默认来源收集上述origin端点，包括尚未物化的private和generic正文，再扩展source records；已有参数、声明和public接口端点继续保留。canonical source record缺少必需端点时仍拒绝，reader不补写；此端点闭包只支持后续来源绑定，不等于default profile、访问或正文语义证明。

default来源的artifact位置绑定接收已绑定的完整nominal参数协议、完整来源正文及显式dependency foundation凭证，先核对省略位置的精确coverage，再逐次核验全部origin。dependency按provider严格递增、排除当前provider，且必须使用同一validated identity graph；每次origin按自身provider命中相应artifact的source/context/point记录，不能借当前artifact中的重复来源或其他provider补齐。正文root还须命中实际provider自有的Function/GenericFunction/Constructor/EnumVariant key及对应definition subject，并与正文root origin使用相同source；正文context必须为该Function/GenericFunction/Constructor的Callable context，EnumVariant则为其source enum owner的Nominal context，不能把声明位置的外围context当作正文context。继承正文保留原provider，发布owner仍由当前参数协议约束。全部扫描、查询与正文遍历共享预算，失败不发布部分凭证。该绑定只证明参数coverage和artifact位置/根身份，不证明default path的语义位置、override来源关系、profile、访问域、正文操作或可执行资格；后续defaults事务必须独立完成这些证明。

完整nominal参数协议绑定组合完整构造器与成员来源，两者必须借用同一个已绑定nominal来源凭证；相同provider或内容相等不能替代该来源关系。required owner集合从已绑定构造器及Function/GenericFunction/VariantConstructor成员独立重算，排除Accessor，且精确等于参数表（包含零参数record）。每个参数逐位置join已绑定签名的名称与逻辑type，使用与inheritance参数绑定相同的origin端点、所属source文件及canonical Array检查；Array类型沿用成员持有的已解析typed角色；不再重新检查core归属、声明种类和arity，参数本身仍按普通类型规则验证。成功凭证保留构造器、成员和协议的借用关系，按(owner, position)精确重放候选shape、calling kind与origin。所有集合和逐项校验共用预算；失败不发布部分凭证。此事务仍只提供source protocol查询，不验证default正文，不授予default展开、lookup、ODR或machine-use。

完整nested候选producer只接收sealed Export HIR、指定的typed nested nominal及共享预算，不能读取候选section或已绑定authority反填。它从真实词法owner关系独立遍历该根全部后代，再投影完整nominal、constructor、method/generic method/variant、runtime/const property及accessor来源；top-level、foreign、缺失、重复或非树状owner闭包拒绝。按owner消费这些投影并递归组装source_support，每个投影record必须恰好进入结果，不保留未消费的旁路记录。参数协议从sealed parameter interface重新投影，仅包含该子树的Function/GenericFunction/Constructor/VariantConstructor，Accessor排除、零参数保留；default key精确使用owner与参数位置，vararg element由真实Array值signature取得。producer返回nested record与完整参数协议的product，source合同在组装时转移所有权，必要的vararg element拷贝、集合、canonical排序、closure检查和递归均计费。ParamFree支持引用由真实source nominal的exact key确定，GenericTemplate保留metadata角色；此输出仍须与独立source authority、default正文、inheritance/representation及完整section事务交叉验证，不绕过production能力门禁。

完整protected声明候选producer从sealed Export HIR独立发现source nominal根闭包，并按真实declared visibility收集这些owner直接声明的Function/GenericFunction、runtime property及其protected getter/setter、source constructor和nested nominal，生成精确的`CanonicalProtectedDeclarationRefsV1`全集。generated adapter不充当source constructor；无关private/internal nominal分支不因存在protected成员而成为新根；public property的protected setter单独入表。以该独立全集逐项投影四分支声明表，nested使用完整递归来源生成器；不得从候选表反推required refs，也不得将effective访问受限的declared-public成员重标为protected。参数协议独立投影完整声明需求与各nested子树需求的typed owner并集，Accessor不伪造独立参数记录；多条合法源码路径到达同一声明只合并需求，不允许reader修复重复record。候选记录、required全集、协议都共用预算并可独立持久化；该producer只产生来源候选，完整artifact来源绑定、default验证、继承选择和machine资格仍由组合事务负责。

protected声明全集的artifact绑定以同一`BoundNominalParameterProtocolsV1`为入口，从已绑定的完整nominal/member/constructor来源与foundation名义access重新取得全部declared-protected refs，精确对照候选四分支声明表；不得使用候选refs或producer附带的required表自证全集。普通callable、constructor、runtime property的access与完整source payload逐项相等，nested复用完整递归source_support重放；private/internal/public声明不能因owner受限而成为protected根。每个非Accessor callable/constructor与递归nested来源使用独立参数来源验证对应candidate protocol，合法重叠路径只共享同一已验证协议。结果保留原members、constructors、parameter来源和候选声明/representation的借用链，并仅暴露实际用到的协议证明；它不代替default body、slot implementation选择或machine-use资格验证。

来源shape的语义查询允许返回借用的canonical key：artifact绑定已保留field、enum variant/field与object value的identity key时，validator直接借用同一key，不因旧owned查询接口重复分配或复制。兼容既有authority的owned返回使用标准`Cow`表示；新artifact来源adapter必须使用Borrowed分支，所有key哈希、结构访问及签名重放继续计入同一事务预算。该调整不改变wire bytes、typed identity或来源完整性要求。

完整nominal来源与dispatch来源的组合必须来自同一已绑定foundation，并沿同一完整member/constructor/parameter链关联。每个inheritance owner的modality、public/protected source constructor全集和protected member全集须从完整nominal来源重新推导后精确比较；generic metadata owner不被伪造成concrete inheritance owner。dispatch的每个callable同时命中完整member合同，其真实access、modality、全部effects、receiver source nominal、声明序parameter/result类型必须相等；SignatureTypeKey与exact type通过同一foundation的结构关系逐项匹配。class virtual method/getter/setter的Concrete选择必须命中完整callable来源的同一virtual-family关系；interface实现是receiver特定选择，普通Direct方法也可实现interface，不能要求target把interface槽列为自身virtual槽，其选择仍由完整slot validator按独立implementation记录重放。core Unit角色也必须与完整member来源一致。该组合只闭合两份独立source transcript的重叠语义，不把来源选择表当作已验证candidate slot或machine-use凭证；全部索引、结构匹配、全集比较共享预算。

section声明authority以同一完整nominal/dispatch来源组合实现，所有shape、property、callable、constructor、enum、slot与来源全集查询均沿该借用链返回，required protected refs从完整来源独立重推。主section入口在原有protected声明校验之前调用authority的来源合同重放：artifact adapter必须逐项比较完整payload/access并验证每个候选参数协议，再执行同一section继承图上的既有声明、nested与accessor校验；不能仅实现旧identity查询接口而跳过完整合同。继承接口继续使用独立source inventory、schema与selection重放。测试/内部authority的默认入口仍执行完整既有声明校验，必须显式实现该section trait，不能以blanket impl让新artifact adapter静默选择较弱路径。该入口只交付声明证明，defaults和committed uses仍须在同一section事务中闭合。

完整nested source_support的artifact重放以已组合的完整nominal参数凭证为入口，沿同一nominal/member/constructor来源递归核对candidate。每个nominal的kind、modality、binder、supertypes、constructor/member/child全集、source shape及真实access必须逐项等于独立来源；每条callable、constructor、property记录还须完整比较payload与access，不能仅因source key/signature匹配而放过effects、modality、slot、setter或const事实篡改。非Accessor callable及每个constructor必须命中候选source protocol表，并逐位置与独立参数协议重放；零参数也不得省略。ParamFree分支复用同一foundation继承图与候选representation的source shape、access及supertypes join，GenericTemplate保留独立metadata分支。checked结果保留同一个完整member/constructor/parameter来源的借用关系，并保存所验证的nested根、representation表和实际使用的protocol凭证，供完整section事务继续核对来源一致性及闭合；未使用的其他protocol不由单个nested子树授予资格。所有递归、集合、合同比较及查询使用共享预算，失败不发布部分结果；default正文、slot选择、完整root集合及机器使用资格仍由后续事务验证。

inheritance property来源子表按`PersistentPropertyId`严格递增，复用`NominalSupportPropertyInterfaceV1`的三字段record，并只接受Runtime payload；Const不参与本表的protected/dispatch accessor闭包，nested const继续由其独立source support拥有。所需集合精确为独立inheritance inventory中Property成员、Protected Callable accessor的logical owner，以及source slot原始声明和实际选择目标中的accessor logical owner的并集。producer从sealed property/accessor关系直接读取owner、逻辑值类型、getter、完整mutability/真实setter来源、representation和getter/setter原始dispatch槽关系；不从public property、protected callable或slot候选回填。public getter加protected setter保留Public property与Protected setter，private/internal setter也保留真实来源而不取得protected callable资格；抽象属性和interface的独立getter/setter槽同样完整保存。所有集合、签名树、来源位置及排序共用预算，reader拒绝Const、乱序/重复、未知typed ref与多余字段。来源记录不授予lookup资格，仍需artifact所属权、独立清单、accessor合同、setter域与完整source-authority组合验证。

nominal完整成员属性来源使用独立`CanonicalNominalSourcePropertiesV1`，同样按`PersistentPropertyId`严格递增并复用三字段source record，但保留Runtime与Const两个真实分支。producer接收定义侧独立所需property id集合，逐项从sealed HIR投影全部声明visibility及完整词法owner链；缺失、foreign、top-level或extension owner均拒绝，不从public候选表补齐。runtime属性的值类型使用其直接nominal host的binder frame，static nested不捕获外层generic binder；getter/setter关系、access、representation和slot与inheritance来源共用投影。Const仅允许object/companion的只读无slot属性，保存已求值的typed integer/boolean/string和真实Property subject origin，不生成accessor。reader严格拒绝重复/乱序、未知typed ref、额外字段及预算耗尽；字符串拷贝、binder/type遍历、origin与输出容量均计入共享预算。该独立来源表仍须与nominal成员清单、artifact foundation及完整nested source_support事务join，不单独授予lookup或执行资格。

属性来源绑定以已绑定的dispatch来源为输入，精确重放protected Property/Accessor清单与dispatch accessor的logical owner并集；protected成员必须属于清单标记的声明owner并保留Protected可见性。property key须由同一artifact实际持有，getter/setter须以同一foundation中的typed accessor key证明logical property与role，property和setter origin分别精确命中各自subject。绑定使用独立继承图重放getter/setter lookup域包含，并把dispatch accessor的逻辑值type、receiver及可见性同property来源逐项join。原始slot关系精确来自该owner schema中accessor自身声明的槽，或ClassVtable中实际选择该accessor的继承槽。abstract override没有concrete target，须通过typed root accessor/property key、相同声明名、class祖先关系与逻辑值type重放其继承槽；不得因Abstract选择缺少target而丢槽，也不能按名字猜测或替换实体id。不能把普通interface实现的Direct accessor伪造成虚槽。所有索引、来源链、类型树、集合比较共用预算。该绑定仅产生来源查询凭证，完整source signature/effects、core Unit角色、modality及机器使用权限仍由完整declarations/type-section validator验证。

protected callable来源绑定从同一已绑定property/inheritance来源取得所需Callable清单，拒绝缺失、额外、重复归属或错owner的记录。Function与GenericFunction分别只借用本artifact自有、且经同一identity graph核验的typed key；Accessor经已绑定logical property和foundation accessor role闭合，不能由普通export候选补身份。每项origin精确命中其Function/GenericFunction/PropertyAccessor subject，setter实际access与property来源一致。绑定重放既有`ProtectedCallableInterfaceV1::validate_source`，签名nominal kind/arity从同一validated canonical source key取得，getter值type与setter参数来自已绑定property；setter直接使用已有typed Unit角色检查结果类型，不重新建立core专用来源绑定。泛型方法仅保留source binder/bounds，不产生ODR应用。所有索引、签名树和来源查询共用预算；成功值只提供来源合同和查询adapter，完整source/default、派发选择及机器资格仍由完整section事务闭合。

inheritance参数协议来源子表按`CallableTemplateOrigin`的typed role与id严格递增，每项精确为`{1: owner, 2: 参数声明序array}`，零参数声明也必须有记录。所需owner精确为独立inheritance inventory的constructor与protected Function/GenericFunction并集，Accessor不参与源码实参协议。每个参数精确为`{1: SourceParameterShapeV1, 2: calling_kind, 3: ExportDefinitionSourceV1}`；shape复用参数名称与逻辑值type二字段product，calling_kind使用整数1～4分别表示Required、Default、VarargEmpty、VarargDefault。vararg的逻辑值type保留真实Array application，element与canonical Array的关系由完整source protocol validator重放，不另存一份可矛盾的element事实；default正文以owner与参数位置关联独立default来源，不复制candidate的template索引。producer直接读取sealed source parameter interface、vararg record、default source和参数真实origin，并核对同一source declaration key中的参数类型序列；不得从public/protected候选签名回填。参数名重复、多个vararg、未知owner/type/origin、表乱序/重复、多余字段或预算耗尽均拒绝。解析只恢复调用事实，完整source/default与artifact来源绑定仍须另行闭合，不授予调用或ODR物化资格。

参数协议来源绑定只组合从同一个foundation绑定凭证取得的constructor和protected callable来源，两者的独立inheritance inventory必须逐项相等；参数协议owner集合必须精确覆盖其constructor与非Accessor protected callable并集。每个位置的参数名称与逻辑值type逐项join已绑定来源签名，origin必须属于同一callable的source文件，并命中当前artifact的source/context/point记录；vararg值必须是已验证导入的trusted-core Array单参数application，Array角色直接使用已有导入结果，不重复验证core归属。source参数没有独立的foundation declaration subject；不能把物化后的LocalValue身份用作generic/abstract源码参数的通用替代。M23-6 foundation producer须直接从同一sealed HIR的全部source parameter interface收集参数origin端点，包含尚未物化的generic/abstract参数；后续public接口的source point补全只能扩展该集合，不能覆盖丢失已有端点。已有canonical source record缺失所需端点时仍拒绝，reader不修复。绑定后的不可变协议按`(owner, position)`提供完整`ProtectedSourceProtocolSemanticAuthority`查询，候选的calling kind和origin必须逐位置精确相等；仅在同文件找到另一个合法origin不构成候选参数匹配。该凭证不验证default正文或授予物化资格，default类别仍须与完整独立default来源及候选模板闭包交叉验证。所有集合、签名比较、source/context/point查询与core角色核验共用预算。

nominal声明来源合同子表独立保存sealed HIR的源码结构。表按`SourceNominalId`严格递增，每条`NominalSourceContractV1`精确为八字段product：`owner, modality, type_parameters, supertypes, constructors, members, children, source_shape`。kind由source_shape的封闭tag决定；class保留真实Final/Open/Abstract，interface为Interface，struct/enum/object为Final。Concrete owner只接受空own binder，GenericTemplate必须有own binder，object没有own binder。binder、supertype、constructor、member、child与source shape复用上述既有constituent；member及child按各自canonical typed key排序，field/variant仍保留源码序列和真实variant style。constructors只允许class/struct，包含该owner的真实source constructor；members包含自身声明的Function/GenericFunction/Property，children包含直接词法nested source nominal，均保留public/internal/private/protected来源，不包含继承成员、generated adapter或object backing class。

declarations的名义来源根从sealed HIR独立发现：以实际public nominal surface为入口，沿本Cone的直接base/interface源码owner及词法owner闭合，并加入这些owner自身声明的protected nested类型。每个protected nested入口的全部词法后代递归纳入完整source_support，不以private/internal/public筛掉其child；普通public owner下无关的private/internal child和无关顶层声明不自动加入。所有关系使用typed source identity，object沿其source owner与真实base/interface关系遍历，generated backing class不作为源码根；generic application只引用其source template，不建立ODR application。foreign provider不纳入本地根集合，跨Cone证明仍必须经完整provider closure取得。输出只确定declarations所需source合同，不表示这些根都已具备exact representation/inheritance、lookup或machine-use资格；index、工作队列、重复路径合并、排序与输出分配共用预算。

foundation producer直接使用这份独立发现的完整source根集合，逐项保存真实nominal key、声明visibility、完整词法owner链和definition origin；来源发现与这些快照的查找、复制使用调用方共享预算。即使某个Concrete来源未被表达式使用、LocalConcrete没有其exact record，也可作为source-only根写入foundation并与完整nominal来源表绑定；不得为此伪造exact identity、fact、representation或inheritance record。具体表示需求单独收集，并继续要求真实LocalConcrete exact关系及完整表示/继承证明；reader不能根据source根存在性补出这些证明。

ordinary HIR producer的具体表示需求覆盖上述源码闭包中所有无自身类型参数的本地nominal：public入口、继承support、protected nested入口及其完整子孙使用同一集合投影fact、representation、inheritance edges与各独立继承来源表。每项必须先核对sealed Export的canonical exact与LocalConcrete已有exact身份；缺失时拒绝，不能通过只保存源码根来降级本应具有ParamFree证明的nested接口。static nested按自己的binder判定，generic词法owner不阻止无自身binder的child进入该集合；generic模板本身仍仅保留来源合同，真实generic base/interface application继续受M23-7 ODR门限约束。来源闭包中的无关private sibling不进入表示需求；这些需求不依赖public或nested候选表。

该表producer接收独立的required source nominal集合，逐项从sealed HIR投影，拒绝缺失来源；不得从public lookup候选、nested候选或kind猜测modality，也不能把无关private类型自动添加为root。access/typed key仍由同owner的foundation来源拥有；成员完整合同、nested递归source_support和参数/default来源在完整declarations事务中join。单独的nominal来源合同不持有source_support候选、不授予lookup或concrete物化资格，也不代表已验证required root全集。wire reader不修复乱序、重复或未知typed ref；所有集合、签名树、field/variant及投影分配共用预算。

nominal来源绑定的owner集合必须精确等于同一已绑定foundation的`source_roots`，kind与own binder arity逐项join其source nominal key。constructor、Function/GenericFunction/Property member及直接词法child清单，从本artifact自有identity表独立重算并精确比较，包含非public声明；object的初始化constructor、generated adapter及backing class不计入。child引用仅证明其artifact自有identity与直接词法owner，不据此自动扩大root集合；递归support所需root仍由完整declarations事务决定。struct源码field、enum variant及variant field集合也必须完整，逐项核验同一validated identity graph中的canonical key、owner和selector；object value必须命中同artifact对应source object。binder bounds、supertypes和field signature重放既有nominal kind/arity及binder scope规则。enum variant origin借用其真实foundation subject，核对owner source文件和source/context/point闭包。identity没有编码的源码字段顺序、Named与Constructor语法区别及class modality仍由独立来源合同保存，不能声称由key反推。索引、集合比较、签名递归和必要的origin/key拷贝均使用共享预算。成功绑定只提供独立source查询，不授予source_support、default、lookup或machine-use资格。

继承来源查询组合nominal、constructor、protected callable及slot四种已绑定来源，四者必须借用同一个foundation绑定凭证，constructor/protected/dispatch的独立inheritance inventory必须逐项相等；相同provider或内容相似的另一份foundation不能替代。每个本地继承节点的modality必须与nominal来源合同一致；组合成功前重放全部constructor来源的`NominalSupportConstructorInterfaceV1::validate_source`。完整`NominalInheritanceInterfaceSemanticAuthority`的owner、constructor、protected member、slot schema/selection及callable查询分别借用这些独立表；普通constructor key不能经protected-only表兜底，enum constructor查询必须使用已绑定nominal的真实variant key、field shape和origin。所有来源比较及语义重放使用同一预算。该组合只建立完整继承来源查询，不替代候选inheritance与protected declaration/default/source_support的事务验证，也不授予machine-use资格。

当前Cone的HIR→MIR→LIR形状物化需求统一由本Cone共有公共声明投影：先按export binding identity的实际exporter选取当前Cone的DeclaredCurrent绑定，不能把聚合读取的其他provider公共绑定误作本地需求；所有本地public param-free nominal均生成完整source、按需box、coroutine step与slot。HIR携带必备LocalShapeSupportPlan，空需求使用空roots；不以core协议的Defined/Imported分支替代需求。MIR接收同一typed source集合并核验规范顺序、实际source/exact与helper，LIR对实际producer验证完整layout、scan与descriptor，删除Core/NotCore物化分支及固定CORE归属。reader从共有direct public surface独立重建相同需求。alias、reexport和generic template不自动增加本地物化根；协议选择与形状需求互不决定。

### 3.2 不改义的既有 section

- identity-foundation 的布局/scan/dispatch key 继续只证明 identity；本阶段新 payload 引用它们，不重复声明同 kind/id。
- `NativeBoundaryTypeDefinitionRecordV1` 只服务原 extern/callback witness，不通过它提供一般 field/scan/TD 查询。
- core与其他Cone共用通用MIR/LIR shape-support表。每个source root均由当前provider的typed source声明及完整类型、layout、descriptor记录验证，完整section核对独立source-root集合的精确覆盖。不得因provider为core而要求空表、跳过验证或委托旧core表；MIR查询与依赖选择直接读取同一通用表。LIR production字段8直接保存所有producer共有的有限shape-support计划数组，删除Core/NotCore包装；source、closure root与definition owner必须属于实际producer，reader从独立source集合完整重算，不能为通用section补齐缺失记录或提供第二份root来源。MIR CoreMirBridge的重复shape_support_roots及wire字段2已删除；本地物化直接消费HIR提交的完整source声明并验证MIR实体，reader从共有公共声明推导LIR义务。
- M23-5 最大 core-closed callable export 集不变，ordinary dependency 的该子集仍走旧 MIR/LIR bridge 和旧 Link 分区。
- 新 callable bridge 只承载上述旧集合以外、现在可证明的 target；MIR与LIR的所有外部callable在各自stage共用一个实体、typed id和arena，MIR输出与sealer复用引用及重复implementation校验，LIR同一body不得重复；旧、新metadata分区从实体的明确选择角色投影。完整类型证明可以被两类 bridge 共用。callable分区优先检查保留的core初始化协议bridge，其次检查M23-5 ordinary bridge，剩余target才进入新bridge；shape的layout、scan、TD与registration不因出现在production形状计划中而被整组排除，按共有shape-link校验实际provider与definition；新开放的 core member/constructor/shape use 若不属于旧 bridge 的固定集合，也走新 bridge，不能借此扩大旧集合。
- 所有外部TD在LIR中统一为`ExternalTypeDescriptor`及一个arena，必需保留实际provider、exact、symbol和definition；Local/External引用只表达本地定义与外部引用。String角色由well-known明确引用保存，既有协议wire仅投影该角色；其他外部TD（包括core普通类型）必须通过通用layout selection，V1缺少对应layout selection时明确拒绝，V2保留实际provider并重放完整semantic/physical selection。codegen统一发射和校验外部描述符，foundation与layout投影共用实体，不维护第二套core TD。通用ExactDispatch从callable ABI实际provider推导Local/DependencyExternal引用，core普通dispatch不降为旧协议CoreExternal。
- `strong-production/5`与`strong-production/6`共有的字段8改为直接shape-support计划数组，旧Core/NotCore tagged sum拒绝读取，已有产物必须重建。新profile只生产/要求V2；两版保持top-level十字段及identity、definition plan、digest DAG、协议bridge、image plan结构，V2另将TD/dispatch语义中无法表示ordinary provider的引用sum版本化；runtime registration/image的C ABI不改变。

本阶段不提升 container/outer schema，不改 `persistent-v1` mangler，不重分配既有 tag。新增 mandatory section 使旧 reader fail closed。

`strong-production/6`中的三个版本化constituent固定为：

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

前述既有variant的payload逐byte复用V1实际编码，包括optional Absent的`{0:1,1:0}`；新增variant是`{0:tag,1:provider,2:exact_or_body}`。body是`PersistentCallableBodyId`，必须反向join同provider已验证的Strong owner、ABI export与definition，不能只比较symbol。TD parent、itable interface key、vtable/itable entry及引用它们的type-registration semantic plan统一使用V2，不保留另一个可矛盾的V1 plan。普通形状的 DependencyExternal 必须由共有 layout/ABI selection 和 Link import 逐项证明；String 与初始化角色使用同一 provider/typed target 引用，并按实际已选 descriptor/callable 与 definition 校验，重复来源不得互相补齐缺失证明；不允许把真实parent写成Absent再由sidecar补齐。core原ref仍按旧分区使用CoreExternal，新增core能力按3.2的分区规则使用新ref。descriptor/dispatch dependency fingerprint使用同一完整V2 relation；旧variant的canonical bytes保持，新variant追加tag，不能遗漏provider或把foreign target编码成local。

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

fields1～3的顶层inventory由本provider的独立checked source/support全集决定；foreign nominal的facts、representation和inheritance完整顶层record只从其terminal provider的checked section读取，不能复制到consumer表。本地inheritance record可保存真实foreign slot root/implementation合同，graph借用terminal的foreign edges；struct等递归GC/ZST事实也从terminal checked facts取得。Tuple等没有唯一source owner的structural exact事实可由本Cone必要support inventory保存，ownership不能按ExactTypeKey形状猜Cone；M23-7的独立Strong物化gate不阻断本阶段对本地transient/组合语义所需的结构事实重放。完整section同时对照本地inventories、显式source_roots与跨provider递归selected闭包。

source_roots本身只证明source identity和词法闭包，Concrete与GenericTemplate均可作为source-only根；只有被独立required_representation_owners或local_inheritance_edges要求的Concrete根必须同时闭合facts、representation与inheritance。source-only根不取得concrete target token，selected不能只凭其源码存在性通过。

完整type-section保留同provider旧public十表的checked借用凭证；该凭证只能经旧section完整semantic validator构造，不能由raw DTO或单表制造。公开final成员的declaration/provider/receiver必须join旧表实际合同，protected/inheritance合同由新表闭合，重叠时逐项同值而不任选来源。local inventories、source roots、facts ownership与source inheritance edges由独立foundation/source authority投射，graph不能从候选edges构造后再以同一候选自证。production时该authority来自sealed Export/LocalConcrete HIR；持久化后必须来自3.1.1的独立required source-authority section。producer与reader均通过相同完整组合入口取得checked section，读取prebuilt/cache artifact时不得要求源码或编译期内存token仍然存在。

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

语言9.1.5的receiver限制只属于目标method/property/accessor自身声明的protected权限；其effective lookup中的外层nominal protected约束仍只用于词法域重放。不能把nested类型的普通成员receiver代入外层class的SubclassesOf条件。getter与setter分别按各自access source判断是否需要protected receiver证明。

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

public与protected default的receiver合同均来自独立原provider声明：正文receiver及其This local须在provider binder frame中精确匹配该声明的source receiver。继承默认值仍保存Base（或Base<T>）的receiver，不能要求其等于发布override的Derived receiver，也不能把正文receiver重写成Derived来通过验证。独立继承来源验证同时接收完整provider到发布owner的binder映射，核对实际override边、代换及发布receiver到代换后provider receiver的合法关系，包括正文未使用的binder。直接定义使用恒等binder映射，provider的binder shape和receiver必须分别等于已检查的发布声明合同。public callable接口的receiver字段仅表示extension receiver；成员默认值的隐含This必须由已检查的nominal owner与host/own binder frame核对，不能把该字段的Absent误当成成员没有receiver。constructor与enum variant的provider receiver固定不存在。protected/source-authority的上述查询和raw receiver/local比较共享本次预算；旧public语义入口保持既有预算边界和wire格式，不从候选receiver、候选mapping或witness反推预期来源。

public与protected候选默认值还必须从独立authority取得原provider的完整参数表及该definition path对应的参数位置。借用合同必须结构上携带存在的当前参数，越界位置不能产生合同；其位置和总参数数须与发布声明一致。原始prefix local与result在provider frame中分别精确匹配此前参数和当前参数，随后逐项核对完整参数表（包含当前参数之后的位置）的provider到发布type代换。不能因A、B在继承映射中都变为T而允许把provider的A改成B；不能从候选local/result拼出预期provider参数。声明来源绑定保留这份借用合同以供后续完整authority组合，且不因此跳过override唯一性与body/reference-access验证。protected查询、原始比较和全部代换使用同一个预算，public保持旧wire与预算边界。

protected source-interface表额外保存其default template集合及definition_sources精确闭包，使用独立索引空间；不能把protected key插入旧public source-interface/default表。表的source parameter形状和省略类别复用Stage5规则，template引用由当前protected表的checked key/index解释。

protected source-interface的owner全集由已验证的protected declaration surface（递归包含Nested的完整source_support）和inheritance constructor surface投射，不由candidate协议/default表反推；各source-use中AccessorNoSourceInterface不生成协议行。跨surface出现同一constructor时，其完整source signature、access、binder、effects及source-use必须同值，才能共用一条协议；同identity的不同payload不得覆盖或合并。Public inheritance constructor使用其payload明确携带的独立协议，重叠的旧public source接口仍须同值。这个全集仅证明source metadata支持，private/internal成员不因协议存在而获得lookup资格。

protected source-interface表是按`CallableTemplateOrigin`的canonical编码排序且唯一的record序列；每条record的field1、2为owner与声明序parameters。owner只允许Function、GenericFunction、Constructor、VariantConstructor，Accessor使用无source-interface协议。parameter的field1～4为name、value_type、calling、definition_origin；calling的tag1～4依次为Required、Default、VarargEmpty、VarargDefault，字段与Stage5叶子wire相同，但default位置只由独立`ProtectedDefaultTemplateIndexV1`解释。semantic key使用独立`ProtectedDefaultTemplateKeyV1 { owner, parameter_position }`（field1、2），不引入新persistent id，不向旧public key/index提供隐式转换。顶层protected_defaults按该key排序，source-interface的每个省略位置恰好引用同owner/position的template，全部template都须可由该独立source表到达；definition_sources由完整section对全部来源取精确闭包。读取方逐项对照已验证source callable签名、真实省略/vararg类别和parameter origin；不借旧public callable interface作为protected签名authority。

在本阶段，access bridge 指“source target → checked inheritance/receiver witness → MIR external target”的关系。只有已有 `DispatchAdjust`/`BoxingAdjust` 等语义确实要求时才生成 thunk，并使用既有 generated identity；不为绕过 visibility 新增 public wrapper 或新 persistent id 家族。

### 4.4 HIR selected set

`SelectedExternalTypeUseV1` 保存 terminal provider、exact/declaration target 与封闭的 use kind：`Signature`、`Representation`、`Construct`、`MemberCall`、`SlotCall`、`TypeTest`、`SingletonValue`、`Inheritance`、`ShapeSupport`。涉及 declaration 的分支携带相应 kind-specific declaration ref；slot 分支携带 exact receiver 与 slot；inheritance 分支携带当前 derived owner 和 direct base edge。

selected record固定为二字段product：field1 `terminal_provider: ConeIdentity`，field2 `typed_use`。typed_use的tag1～9依次对应上述九类：Signature/Representation/TypeTest/ShapeSupport各以field1保存exact；Construct的field1、2为构造结果的source exact与typed declaration；MemberCall为实际静态receiver exact与typed callable declaration；SlotCall为实际静态receiver exact与persistent slot；SingletonValue为source object exact与`PersistentObjectValueId`，不得用generated backing identity替代source object；Inheritance为当前derived exact与直接继承edge。构造declaration的tag1、2分别为Constructor和EnumVariant（field1为各自typed id）；member declaration复用`InheritanceCallableDeclarationV1`的Function/Getter/Setter叶子（tag1/2/3），不包含generic template/application或generated实现。直接edge的tag1、2分别为ClassBase和Interface，field1为其target exact，没有NoBase分支。完整record按canonical编码严格排序、唯一，reader拒绝重复和乱序；仅取得这些typed id不构成selected资格。

该persistent集合只按terminal provider与完整typed target去重，不保存或任意挑选某一个source/root/parent provenance。独立checked committed-use authority从全部实际typed HIR roots和递归semantic edges重算精确target集合，完整section逐项对照，再重放每一个真实source使用的lookup/access、receiver、definition/evaluation来源及每一条派生support edge的semantic parent。相同target被多个source或parent使用时，所有证明仍须成立；wire不能以去重为由跳过其中任何一次使用。Representation和ShapeSupport使用同样规则，不伪造import route。候选selected表不能反推实际roots/parents或自证terminal provider，GenericSourceMetadata default不能产生selected、展开或物化入口。

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

shape-support表按source nominal的canonical bytes严格递增。所有producer（包括core）的每项source必须属于当前provider；完整section用独立重建的required source-root集合核验精确覆盖，不能从本表反推所需全集。通用MIR表统一保存source、exact、box、coroutine step与slot关系，producer及reader共用相同helper角色、GC与typed归属校验；删除Core/Ordinary root来源枚举及section内第二份core shape缓存。缺少source、helper或所需表项均报共有错误，不能另建或从旧CoreMirBridge补齐第二份root清单。该 MIR 五字段索引不复制 7.2 的 LIR 八 role product，后者另行证明布局、scan、TD、registration 与 definition。

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

origin 的 tag1～4 是 `Function/Constructor/Accessor/Generated`：前三者的 field1 为相应 typed id，Generated 的 field1、2 为 generated callable id 与完整既有 canonical key。lowering role 的 tag1～10 是 `Ordinary/ClassInitializer/ValueConstructor/Accessor/DispatchAdjust/BoxingAdjust/ObjectEnsure/ObjectInitializer/PureVirtualTrap/DerivedEquality`，tag11 为 `PrimaryValueConstructor`，field1 同样为 owner exact。ValueConstructor 用于 struct 次构造，PrimaryValueConstructor 明确表示只组装已求值字段的主构造 leaf。Ordinary/Accessor 无 payload；ClassInitializer/ValueConstructor/DerivedEquality 的 field1 为 owner exact；两个 Adjust 的 field1 为所调用的 Strong target；ObjectEnsure/ObjectInitializer 的 field1 为 object/companion initialization unit；PureVirtualTrap 的 field1 为原 dispatch slot。constructor owner必须逐项等于源码 owner chain 的最内层 nominal；trap implementation可以是该slot的原 declaration owner，或保留同一slot的abstract class override；后者的源码声明必须属于实际receiver class，该class须沿已导出的直接基类链严格继承root receiver，双方参数/result/execution签名保持一致，实际abstract选择仍由同次HIR/MIR生产核对。object value读取仍用5.4的已授权storage plan，不新造一个返回object的initializer身份。

源码 NoGc signature 的receiver、参数及结果必须全部GC-free。两个Adjust允许GC-free value目标保持NoGc，而接收interface/class ref的wrapper为Managed；Managed目标不能适配为NoGc调用。PrimaryValueConstructor 的源码合同固定Managed，lowered固定NoGc，两份exact signature相同且无receiver、返回owner，参数必须逐项等于struct完整声明字段的exact类型；这一无安全点内部leaf可以组装含引用的值，不授予源码NoGC调用资格。其余角色的lowered NoGc signature同样要求GC-free；除adjust与上述主构造角色外不改变GC effect。constructor/accessor/object initializer/ensure/derived equality均为ordinary同步角色；object ensure/initializer保持Managed。callable canonical表按Strong implementation的既有kind/id顺序保存，reader拒绝重复或乱序；两份签名与role均不可缺失。该constituent核对canonical identity、signature和type facts；slot选择、真实body和GC effect的完整authority仍须经HIR/MIR生产join取得。

source signature 与 lowered signature不能合成一个字段：class constructor 的源码结果为 class value，MIR initializer 取得同一个 initializing receiver并返回 Unit；enum variant construction 可以只有 representation operation而没有独立 machine body，使用专用 construction plan，不虚构 callable definition。

class construction 固定为 exact allocation一次、同一 receiver direct调用 initializer；base/`this` delegation 不分配、不改 header，最派生 TD 从 allocation 起保持。abstract class可有供 derived调用的 initializer，但不能构造 allocation target。跨 Cone构造保留 base-before-derived、共同初始化一次、异常不发布结果及每次 call 后 receiver relocation。

普通 member、extension、value constructor和adjust thunk都执行按值 receiver/参数语义。`@InteriorMutable` 或 `addressOf(this)`可观察时必须有方法局部 copy；即使 physical ABI使用 pointer，也不能把 caller/box内存变成方法的可修改 `this`。

### 5.3 dispatch schema 与 table 构造

`ParamFreeMirDispatchSchemaV1` 按 exact owner 保存 class vtable schema 与按 exact interface排序的 itable schema。每条 entry包含 `{ slot, position, slot_signature, implementation }`；implementation是 `AbstractObligation { declaration, trap_target, receiver_adaptation } | DirectStrongTarget | InterfaceDefaultTarget | AdjustThunkTarget`。abstract分支沿用当前MIR的typed pure-virtual trap body，使用实际abstract声明对应的Strong callable identity与完整signature；interface obligation使用原interface declaration，class再次抽象化可使用保留同一slot的derived abstract override；它有明确fatal出口，不留下null/未解析function，也不授予源码direct call权限。

schema wire固定为field1～3的`{ owner, vtable, itables }`；vtable是tag1的无payload `NoClassVtable`或tag2、field1为有序entry序列的`ClassVtable`。itable为`{ interface_exact, entries }`，表按interface exact严格递增；interface provider保留以自身exact命名的schema。entry按上述四项编码，position为table内的typed u32序号，必须逐项等于语义序列的0至长度减1，不是persistent id或全程序ordinal。slot signature复用5.2的完整`{ exact, gc_effect }`。implementation按上述顺序使用tag1～4；Abstract的field1、2分别复用既有`DispatchDeclarationOwner`与`StrongCallableDefinitionOwner`，field3保存receiver adaptation；后三个分支的field1保存Strong target。DirectStrongTarget与InterfaceDefaultTarget另以field2保存无payload的`Identity`（tag1）或`ReferenceDispatch`（tag2）receiver adaptation。

Direct receiver adaptation沿实现规范2.9的同一object身份规则：Identity要求slot与target完整签名相等；ReferenceDispatch仅允许receiver不同，其参数/result/execution/GC完全相同，且从当前schema owner通过显式base/interface/Object→backing边分别到达两个reference receiver。两条规范路径由验证器重建：先取最短，再对同长路径的完整exact-id序列取canonical bytes最小者；不在wire保存任选路径。receiver相等时必须使用Identity，因此同一关系只有一种编码。真实value payload变换仍必须选择AdjustThunkTarget并由既有generated key与两份完整signature证明。值类型采用interface default时也保留BoxingAdjust：semantic receiver是default body的interface，lowered receiver是key指定的目标itable interface，body仅重解释同一个box；payload到default interface的可达关系由schema验证，不能以Direct/Identity分支绕过。

itable的slot signature receiver固定为该表的interface exact，class vtable则保留原slot declaration receiver；因此derived class base prefix保持完整签名不变，而继承interface的provider/consumer共享当前interface的调用签名。原declaration与该调用签名仅receiver可不同，须由当前interface到声明interface的typed路径证明。生产lowering须按这份table调用签名建立interface call ABI；当前本地callee signature不能仅因receiver物理形状相同就替代table合同。AbstractObligation也携带上述闭合receiver adaptation，把当前table receiver适配到实际trap receiver，保留该声明的Strong identity和完整lowered signature；不把derived class的再次抽象化强制回退为基类body。

reader核对每个slot的原declaration callable signature、所选target的完整lowered signature以及adjust的目标semantic/lowered关系。schema表按owner exact canonical排序，并统一重放base prefix、interface provider的slot/signature序列及abstract obligation；这种MIR内部闭合检查不替代HIR source slot schema的原始顺序与override选择join。Object和ObjectBacking沿class-like vtable规则处理；有限BoxedValue/CoroutineStep/CoroutineSlot shape support不另造source dispatch schema，box的descriptor dispatch从payload语义schema与既有boxing adjust机械构造。

ObjectBacking schema调用同一object自身声明的成员时，receiver验证允许直接的backing→source object适配；必须同时匹配已验证的`ObjectBackingClass { object }` origin、source object nominal及其`Object { backing }` representation。该适配不改变两个exact identity，也不加入继承图，不产生双向继承环；其余receiver仍沿原继承图重放规范路径。

interface provider之间的MIR独立核验只比较父子表仍共同保留的slot：共同slot的非receiver签名与相对顺序必须一致，全部保留的继承slot仍在当前新增slot之前。子接口可按typed override抑制父槽，不能要求每个父槽继续出现在子表。被抑制集合、完整父接口声明序及新增slot是否合法，由同次HIR完整source schema与MIR逐项join证明；组成表通过不替代这一完整发布条件。

- derived vtable保留完整 base prefix；既有 slot的 position保持，override只替换 target；新增 virtual family按当前owner的方法声明序首次出现时追加，保持现有MIR语义顺序。
- interface保留“继承slot在前、当前声明slot随后”的schema顺序和去重规则；按typed override关系抑制被覆盖的旧成员后，其余槽保持相对顺序，getter/setter分别处理。consumer调用携带interface TD + schema内position；不存在程序级global slot ordinal。按id查找的wire record table可以canonical排序，但position必须保留provider的语义序列，不能按id重排物理table。
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

当前initializer中的显式external ensure通过`SelectedExternalInitializationUseV1 { local_unit, provider, dependency_unit, cause }`记录；cause是`ObjectValue(object_value_id) | PropertyAccessor(accessor_id) | InitializationSupport(unit_id)`，必须由已提交的typed ensure语义产生。LIR selected set保留同一edge，strong-production/6按3.2验证foreign unit，不要求它出现在本地unit arena。这里只记录现有语义的真实ensure dependency，不读取foreign body做跨Cone调用图推断；普通external callable内部自己的ensure继续由provider负责。image/runtime使用canonical unit id解析这些edge，相关真实descriptor/cell relocation按11.2验证。

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

两条路径在计算闭包前都调用同一独立source authority：它必须逐项核对本地五张export表确由已提交的MIR→LIR输入产生，并把`physical_imports`与真实machine use、Strong production引用及已授权object/init support精确对照。候选section wire、候选provider view、单独通过构造器的semantic table或已重放import都不能充当该authority；reader随后还须将五类一般import contract重新绑定到dependency closure中的同一terminal section记录。

`physical_imports`就是11.2定义的canonical semantic-import projection，按`(provider, subject canonical bytes)`严格递增且唯一。它由实际machine use、strong-production/6引用及已授权object/init support独立收集，再与对应terminal semantic记录、Strong definition及旧分区逐项join；semantic use可以没有physical import，physical import不能只有symbol或definition而没有完整semantic/support authority。该数组与Link section field1及Code contribution逐byte相等，selected的brand和terminal引用不编码。

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
  -> HIR source-authority transcript / type facts / access / inheritance closure
  -> MIR source-to-implementation / slot / helper relations
  -> LIR layout replay / scan normal form / ABI / TD
  -> complete source-root support obligation
  -> selected request closure and external arena commit
  -> final object verification + Link-only use closure
  -> Compile/Link equality + publish
```

继承环和by-value representation环拒绝；通过managed reference的递归class合法，layout重放在reference leaf停止，不沿对象图无限展开。所有provider来自同一target-compatible显式artifact closure。prebuilt/cache验证只能读取该closure内的required section，不得要求原始source、compiler进程内token或调用方注入authority factory。记录存在、id匹配或digest相同都不能替代逐字段关系证明。

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

每项wire是 `{ 0: tag, 1: payload }`，定义和symbol role从variant唯一派生。7～10只可由provider已导出的object-value/initialization support relation选择；不得通过它们枚举私有storage，也不能引用任意其他unit的cell/failure root。ordinary property access依然只使用accessor，不以此公开backing storage。已经由 strong production 外部引用或 ordinary callable selection 记录的 subject，按实际 capability/subject 从共有依赖索引验证，不能在 layout 接口重复认领。foreign immortal不是本sum的variant，其现有String/constant路径遵守9.3。

`ShapeLinkContractV1`精确分为七个variant：`CallableAbi { canonical_signature, calling_convention, protocol }`、`Layout { record }`、`Scan { layout, role, canonical_scan }`、`Type { descriptor_projection }`、`Dispatch { table_projection }`、`StaticStorage { storage_projection }`、`Initialization { unit_projection }`，tag按此顺序为1～7。subject1～5分别只能匹配contract1～5；subject6复用Type、7/8复用StaticStorage、9/10复用Initialization。

Layout/Type/Dispatch分别复用6.1/7.1的canonical semantic record，去掉definition/registration的后置digest槽；Scan保存完整scan tree而非仅digest；storage/unit projection逐字段复用strong-production/6对应semantic plan，不包含member/range或后置object/registration digest。它们不是任意bytes，reader按subject取得provider同一plan并逐字段比较。required definition由subject和owner重算，provider必须真实Strong定义它，consumer defined-symbol set必须不包含它。

新projection保留原字段编号：Layout恰为exact-layout record的fields1～6，Type恰为exact-descriptor的fields1～8，Dispatch恰为exact-dispatch的fields1～4；三者分别排除field7、fields9～10与field5的definition/registration后置槽。StaticStorage恰为strong-production static record的fields1～10（storage、symbol、layout、scan id、完整scan、scan kind、logical size、allocation extent、alignment、initial state）；Initialization恰为unit record的fields1～8（unit、diagnostic path、schedule、storage、failure root、initializer、ensure、ordered dependencies）。这些语义字段均不含后置digest，已有initial-state、schedule及V2 dependency的wire保持不变；typed dependency proof仍在provider同一strong-production/6计划中保存，不能由wire unit id重造。每个projection复用完整record的同一字段编码和逐字段重放，只改变新product的字段数，不改变旧完整record bytes。

七种contract的新wire均使用field0保存tag：CallableAbi为`{0:1,1:canonical_signature,2:calling_convention,3:protocol}`，Scan为`{0:3,1:layout,2:role,3:canonical_scan}`，其余五种为`{0:tag,1:对应typed projection}`。import的五字段依次为provider、subject、expected symbol、required definition、contract；canonical数组按provider与subject的组合canonical bytes严格递增，不合并重复项。provider查询先复用subject的唯一Strong definition/primary/symbol规则，再与同provider的实际Strong production及完整semantic records核对。storage/unit查询协议只返回候选semantic plan：实现须由完整section封闭持有，先证明导出的object-value/initialization support关系，import重放再将候选与provider实际unit及其storage/failure root逐字段join；候选本身不授予import或selected资格。完整section仍须将已构造import的五类一般contract重新绑定到同一terminal section的实际ABI/layout/scan/TD/dispatch表，不能凭provider与subject相等接受来自另一份同ID语义表的record。

requirements沿用M23-5 canonical relocation-use结构和排序，并引用本section import index；每个physical import至少一个use，每个actual relocation恰有一项。object coverage绑定全部最终LinkObject成员集合及canonical use set，digest使用 `DomainSeparatedCborHash("scoop-cross-cone-layout-object-coverage-v1", { verified_link_objects, relocation_uses })`，仅作LinkValidationOnly。

本section的三字段编号依次为semantic_imports、requirements、object_coverage。每个requirement为`{1:canonical_relocation_use,2:import_index_u32}`，严格按既有`(member, containing_atom, offset_within_atom, target_slot)`排序且唯一；只从共有 strong dependency 分类后的 remainder 中按 target 规范化后的 expected symbol 匹配，已由其他 capability/subject 认领的 symbol 不得再次认领。未匹配的native/runtime候选留给后续既有分类，不得丢弃。coverage为`{1:verified_link_objects,2:relocation_use_set_digest}`，其hash preimage为`{1:verified_link_objects,2:canonical_relocation_uses}`，uses保留完整十字段而不含import index。最终对象证明必须与分类输入的同一verified relocation closure逐成员内容和完整binding join，不能仅以producer或member id相等替代。reader在共享预算内先同Compile完整physical-import表重放，再同独立重建的requirements、全部最终objects和coverage digest精确比较；raw wire本身不授予分类或对象覆盖资格。

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

- 五个新增capability、strong-production/6和新profile fixed vectors，empty/nonempty、unknown required、错purpose、旧profile拒绝；M23-2 foundation/extern/callback及persistent identity vectors不变；HIR协议定义与strong production旧格式退役，更新对应section/profile vectors。HIR source-authority逐子域覆盖缺失、额外、重复、错origin、错access/receiver、错committed root及候选表自证拒绝，并以仅artifact bytes的prebuilt/cache路径重放。V2的ordinary parent/itable key/dispatch target正反例必须经过真正的strong production wire round-trip，不能只在新layout sidecar中通过。
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

1. 固定本文profile/section/wire constituent及测试vector；添加required capability gate和拒绝旧profile路径。先持久化独立HIR source authority并让production与bytes-only reader共用验证入口，再放宽source能力。
2. 收口HIR concrete facts与persistent inheritance surface，完成protected/domain/default witness和source negative；保留旧public section字节。
3. 实现MIR type/constructor/object/slot bridge与selected closure；以golden锁定没有foreign body复制、layout offset或generic template。
4. 收口LIR storage/refined shape、general layout replay、scan normal form和external exact arena；实现provider有限shape-support导出和consumer验证。
5. 接入通用ABI及logical-to-physical映射，验证call/invoke/dispatch/byval/sret/root plan，再开放对应param-free source成功格。
6. 完成box/unbox、ZST place/static、array/Ptr执行路径与C边界矩阵；删除被替代的旧size/scan/header-offset入口。
7. 完成新Link-only closure、三路object coverage、definition规范化、Code贡献和双view发布；更新core、scheduler、cache accepted profile。
8. 完成独立/组合/negative/golden/corruption与回归矩阵，将多Cone运行场景交给M23-9/11复用。

每批代码变更完成后先 `cargo fmt --all`、`cargo clippy --workspace`，再运行相关test；最终运行完整workspace与runtime/fixture验证。不能用最终linker尚未实现为理由跳过本阶段object和双view证明。

## 15. 完成门

- 独立持久化的HIR source authority、候选source/access/inheritance、MIR relation、LIR layout/ABI/scan/TD、actual object use形成可从最终artifact bytes独立重建的完整链；候选表不能自证，reader不依赖source、compiler token、symbol/FQN或host layout fallback。
- param-free跨Cone构造、value/member/object使用、inheritance/dispatch/protected成功矩阵全部能生成新profile双view有效artifact，未选中的foreign body不复制。
- 每个合法source subject在定义Cone拥有完整有限shape-support；consumer只引用external typed definition，全部ODR生产继续拒绝。
- ZST logical semantics、typed ABI、place/static token、box/array/Ptr/C边界及scan/TD矩阵全部锁定；codegen/runtime不再从size0或空LLVM struct猜语义。
- nonzero box、indirect aggregate和跨Conefield的managed provenance/root/relocation完整；不存在握手后才登记root或从旧ref副本复制的路径。
- 新required section/profile、strong-production/6的完整foreign TD/dispatch引用、fingerprint/cache迁移和三路Link coverage完整；既有identity、extern/callback bytes、旧capability语义保持。
- 独立、组合、negative、各stage golden与corruption/determinism回归通过；文档明确M23-7/8/9/10/11交接，真实多Conemoving-GC不被提前宣称完成。

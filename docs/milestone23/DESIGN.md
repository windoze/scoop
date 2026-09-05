# M23 设计：多 Cone、导入系统与 `.slib`

版本：0.1（设计完成，待实现；2026-09-05）

对应`docs/ROADMAP.md`的M23。M23在M16/M17统一调用决议与default template、M20 exact generic application、M21 typed access domain/全局初始化以及M22非generic typealias之上，把“core源码与用户源码同一编译单元”的过渡模型替换为真正的独立Cone编译：每个Cone单独产生可复用`.slib`，下游只通过版本化metadata消费其语义接口，最终程序再把依赖闭包中的native object和image descriptor静态链接起来。

本里程碑不是给现有arena id外面套一层Cone编号。跨artifact identity、import provenance、generic specialization、TypeDescriptor唯一性、初始化登记与缓存失效是同一个闭环：只要其中一处仍以FQN、link symbol、输入顺序或host路径补猜，钻石依赖就可能把同一实体复制成两个运行期类型，或把不同实体错误合并。

## 0. 关键决策与范围

- **Cone是module/distribution/build unit，package只是源码namespace。** 二者不互相推导；一个Cone可包含多个package，同一package也可由多个依赖Cone贡献声明，冲突由typed origin与普通名称规则诊断；
- M23 v1只支持最终链接前已完整解析的、**静态无环Cone图**。不实现registry下载、版本范围求解、lockfile生成、动态库加载、`dlopen`、image卸载或ABI-stable plugin；
- `Cone.toml` v1使用exact coordinate与library/executable kind。dependency locator可以是source path、`.slib`路径或搜索路径中的exact artifact，但locator不进入Cone identity和产物metadata；
- `ConeIdentity`由canonical `group:name:version`产生，artifact content另有fingerprint；同coordinate不同metadata不能伪装成同一个可替换artifact；
- source `package`、exact/star import、`as` alias和`public import`在M23一次落地。re-export只建立新的公开binding，保留最终实体的origin identity，不复制声明或改写type identity；
- import只给M16/M18既有resolver增加typed candidate source。每个候选仍独立完成M17参数映射、constraint、postponed argument、applicability和MSC；不存在“来自`.slib`所以先选中”的远程捷径；
- Export HIR的序列化闭包分成**public lookup surface**、**inheritance/slot surface**与**generic hidden support closure**。后两者可包含不能普通import的实体；语言可见性与native link visibility严格分离；
- `.slib` v1是target-specific、可复现的确定性archive，包含manifest、Export HIR metadata、MIR metadata、LIR metadata、本Cone object及可选C bridge object。wire schema显式版本化，不直接`serde`当前Rust struct或持久化`la_arena::Idx`；
- 跨artifact实体使用按kind隔离的persistent typed id；reader验证后重映射成consumer session-local typed id。canonical key和link symbol只分别用于identity生成/发射，不能在缺失typed relation时作为lookup fallback；
- 上游generic template在每个实际需要它的下游Cone完成concretization、MIR/LIR lowering与发射。相同specialization的函数、layout、TypeDescriptor、dispatch table、adapter及generic static storage使用同一ODR group和fingerprint整体coalesce；
- M21延期的generic delegated extension property在M23开放：每个exact receiver application拥有program-wide唯一、lazy exactly-once的delegate storage/init unit，identity由property template与完整concrete receiver arguments决定；
- `.slib`首次冻结跨Cone layout与typed Scoop ABI，因此M23同时收口ZST：logical size保持0，address-taken/static place使用独立token，Scoop ABI显式elide payload，ZST array以logical index而非pointer推进，C ABI拒绝无portable C object representation的零尺寸值；
- 每个Cone artifact的`code.o`恰好导出一个名称唯一的hidden strong `ScoopImageDescriptorV1`；可选`bridge.o`不重复发射。最终link step另生成唯一`ScoopProgramDescriptorV1`，显式引用依赖闭包中每个image、root executable entry与well-known core binding；runtime不扫描weak symbol或按符号前缀猜image；
- runtime先验证并登记全部image、stackmap、callable、TypeDescriptor、static storage/root、immortal object和init unit，之后才初始化heap、attach主线程并执行任何managed initializer。eager顺序为依赖优先、同层Cone coordinate稳定排序、Cone内`PersistentInitializationUnitId` bytes排序；
- reserved core Cone `scoop:scoop.core:0.1.0`从用户编译单元中移出，作为sysroot中由driver信任的独立library Cone预编译/缓存。普通Cone对它具有隐式direct dependency；core prelude来自其typed export metadata，不再由user/core file index或`Option`短名特判模拟；
- M23包含M22**非generic**typealias的跨Coneimport、re-export与展开；generic/nested typealias、interface method自身type parameter、动态image与跨版本ABI承诺继续留在后续。

## 1. Cone identity、manifest与构建图

### 1.1 三种身份不得混用

M23区分：

```text
ConeCoordinate = { group, name, version }
ConeIdentity   = SHA-256("scoop-cone-id-v1" || canonical(ConeCoordinate))
ArtifactFingerprint = SHA-256("scoop-artifact-v1" || canonical artifact input defined in 4.2)
```

- `ConeCoordinate`是用户、诊断与依赖声明中的稳定文本身份。`group`/`name`各由一个或多个`.`分隔的lowercase ASCII段组成，每段匹配`[a-z][a-z0-9-]*`；`version`是SemVer 2.0.0的唯一合法文本（含pre-release/build metadata时原样参与identity，禁止前导零与多余`v`）。不做trim、大小写折叠、Unicode归一化或路径别名解析；不满足canonical grammar直接拒绝；
- `ConeIdentity`包含version。两个版本即使源码相同也拥有不同nominal/type/callable identity；M23 driver要求一个resolved graph内同一`group:name`只出现一个version，版本调停仍由上层构建工具完成；
- `ArtifactFingerprint`描述这个coordinate的一次具体产物，按4.2排除自引用字段后计算。相同`ConeIdentity`而artifact/semantic fingerprint不同的两个`.slib`不能同时进入一次构建，也不能根据搜索路径顺序任取其一；
- source package、FQN、manifest/path、artifact digest、compiler session中的provider slot都不能代替`ConeIdentity`。源码无法构造或声明一个内部`ConeIdentity`值；
- SHA-256输入使用domain tag、固定字段顺序和长度前缀，不用分隔符拼接。完整coordinate与canonical identity record保存在manifest中，reader重新计算digest；digest不匹配或同digest对应不同canonical record是损坏artifact，禁止以文本名继续工作。

稳定排序显示用`(group UTF-8 bytes, name UTF-8 bytes, canonical version)`，不按digest排序。这样无依赖Cone和诊断路径对人可预测，同时identity仍由typed digest承载。

### 1.2 `Cone.toml` v1

生产Cone根目录必须有一个`Cone.toml`和固定`src/`目录。最小schema为：

```toml
schema = 1

[cone]
group = "dev.example"
name = "app"
version = "0.1.0"
kind = "executable" # 或 "library"

[dependencies]
"org.foo:bar" = { version = "1.2.3", path = "../bar" }
"org.acme:util" = { version = "2.0.0", artifact = "../artifacts/util.slib" }
"org.other:log" = "3.1.0" # 从driver提供的artifact search roots解析
```

规则如下：

- `schema`、`[cone]`四字段必需，unknown required table/field直接诊断；未来可新增明确标为optional且不影响语义的字段，v1实现不能静默忽略拼错的semantic字段；
- dependency key是exact `group:name`，value给出exact version。字符串短式等价于只写`version`；不接受`^`、`~`、区间、`latest`或可选dependency；
- table value至多有一个locator：`path`指向source Cone根，`artifact`指向`.slib`，二者都省略时依次检查每个`--cone-path`下的`<group>/<name>/<version>/cone.slib`（三项均使用canonical文本，不拆`.`）。所有存在的候选必须具有相同完整artifact fingerprint，否则报告ambiguous artifact；root顺序不能决定选取不同内容。relative path相对当前manifest，仅用于定位；它不写入`.slib`、stable key、diagnostic source identity或缓存语义；
- path Cone或artifact manifest中的coordinate必须与dependency key/version逐字canonical相等，否则在读取源码前失败；
- 除reserved coordinate `scoop:scoop.core:0.1.0`外不存在隐式dependency。这里的Cone name `scoop.core`与源码package `scoop.core`只是当前core发布约定，不存在由package推导Cone的规则。用户manifest不能声明、覆盖或用path伪造该coordinate；driver只从当前sysroot的trusted core slot注入direct edge；
- v1没有dev/build dependency、feature、platform条件、dependency alias或native link option。native library仍由已经验证的FFI declaration进入LIR/link metadata；
- executable不能作为另一个Cone的dependency。build root可以是library或executable：root为executable时graph中恰有一个executable且只能是root；root为library时graph中没有executable。library产物不含program descriptor或C `main`；
- manifest自身不是源码语义表达式，错误由driver给出manifest path和TOML span，不伪造成HIR file index诊断。

### 1.3 source discovery、package与entry

- driver递归收集`src/`下扩展名精确为`.scoop`的regular file，以标准化Cone-relative `/`路径按UTF-8 byte顺序排序；空source set、非UTF-8路径、两个路径归一化到同一identity、或symlink逃出Cone root均为driver错误；
- host绝对路径、目录枚举顺序、inode、mtime和canonicalized临时目录不进入任何semantic identity。source path identity是`(ConeIdentity, normalized relative path)`；
- 文件路径不决定`package`。一个文件可声明任意合法package，省略则属于root package；跨package引用必须按第2章规则导入或限定；
- library Cone不要求entry。它可以声明名为`main`的普通函数，但不获得entry linkage；pipeline各层以封闭的`ConeOutputKind::Library | Executable { local_entry }`表示产物种类，不能用`Option<Entry>`让library/executable与entry presence形成可矛盾状态；
- executable root必须恰有一个top-level ordinary、non-generic、non-suspend、无参数、返回`Unit`的`main`。只在root Cone中发现，dependency中的同名声明不参与竞争；`Executable`分支的`local_entry`非可选且entry可保持internal；
- root实际entry也使用普通namespaced stable symbol。最终program descriptor保存typed code pointer，runtime不再调用固定`scoop_main`符号。

现有单文件fixture不需要为每个历史目录手写生产manifest：fixture runner可以通过内部API构造带稳定测试coordinate的`ResolvedConeInput`。该能力不进入CLI、环境变量或`.slib`，也不能用于生产intrinsic authority。

### 1.4 resolved DAG与缓存输入

driver先只读解析全部source manifest与prebuilt `.slib` manifest，再建立以`ConeIdentity`为key的graph：

1. 仅对非core Cone注入trusted `scoop:scoop.core:0.1.0` direct edge；core bootstrap root不依赖自身；
2. 验证coordinate/content唯一、dependency kind、exact version与target/schema兼容；
3. 以typed DFS/SCC检测cycle并打印完整coordinate edge path；
4. 验证同一`group:name`没有多个version；
5. 得到唯一canonical topological order：初始及每轮ready set精确定义为“全部direct dependency都已输出”的剩余Cone，从中按coordinate byte order取最小者；因此dependency总在dependent之前，不依赖edge在实现中的存储方向；
6. 才按该顺序执行parse/HIR/MIR/LIR/codegen/pack。

一个`.slib`的dependency table记录其编译时每个direct dependency的`ConeIdentity`与三层semantic fingerprint。若当前解析到的artifact不匹配，source dependency必须重编译；只有prebuilt artifact时报告stale dependency，而不能把旧typed id接到新metadata。

M23的缓存key至少包含：normalized manifest semantic fields、全部source content digest、compiler language/schema/runtime ABI、target profile fingerprint及当前Cone实际消费的三层**Merkle semantic fingerprint**。每层fingerprint除本Cone canonical section外，还按typed support/re-export edge纳入对应origin artifact的同层Merkle fingerprint；因此`C -> A(re-export B)`即使A自己的binding bytes未变，B的相关HIR/MIR/LIR变化也会沿A传播到C。

HIR依赖不能只记录winner：一次lookup在exact/star层看到空surface或全部不适用候选后落到低层时，上游新增适用overload也会改变结果。`CrossConeUseSet`因此包含`LookupObservationSet`，记录每次查询观察到的binding group/snapshot及空、shape-filtered、applicability失败、MSC rejected等surface fingerprint。只有该observation闭包完备时才能精确裁剪HIR edge；**M23 v1的强制安全基线是纳入全部direct dependency HIR Merkle fingerprint**。MIR/LIR仍可在已重新运行HIR后按selected typed external edge裁剪。实现可以对其他层保守纳入全部direct dependency fingerprint，但不能只hash本地section或winner后宣称re-export/负查询会自动失效。上游仅object body变化而semantic metadata不变时下游无需重新执行HIR/MIR/LIR，但最终link key必须纳入完整transitive code fingerprint；M23可以先实现保守重编译，不能实现会错误复用的欠完备缓存。

## 2. `package`、`import`与re-export

### 2.1 文件头语法

一个source file的顺序固定为：

```text
packageHeader? importHeader* declaration*

packageHeader = package QualifiedName
importHeader  = public? import ImportSelector (as Identifier)?
ImportSelector = QualifiedName | QualifiedName . *
```

示例：

```kotlin
package dev.example.api

import org.foo.model.User
import org.foo.ops.render as renderUser
import org.foo.model.State.*
public import org.foo.model.Result
public import org.foo.errors.*
```

- `package`至多一次且必须先于import/declaration；import只允许在文件头，声明之后出现是语法错误；
- exact import可以写`as`，alias只改变本文件binding的短名。star import不能写alias；
- `public import`同时是本文件的explicit/star import和当前Cone的re-export声明。其导出binding位于**当前文件package**，名称为alias或目标短名；public star把本次解析到的每个目标短名分别写入该package；
- public star是已解析API的snapshot，不是下游重新执行的文本规则。上游新增/删除目标会改变re-export index与semantic fingerprint；
- `public`在此仍是import上下文关键字，不创建普通visibility declaration；除`public import`外没有`internal/private import`修饰符；
- qualified type path先取能形成可见package binding的最长前缀，再沿static nested nominal/object/companion scope走typed owner edge。它不把点连接的字符串直接当FQN查全图；
- v1的value/function表达式仍通过import后的短名或普通receiver语法访问顶层实体，不新增dependency-coordinate-qualified表达式语法。

### 2.2 import可到达性与origin

普通import selector只能解析：

1. 当前Cone中按visibility允许该file访问的声明；
2. manifest的direct dependency公开binding；
3. direct dependency公开binding中已经解析好的re-export target；
4. trusted core dependency的公开/prelude binding。

driver虽然为link和metadata closure加载transitive `.slib`，但未被direct dependency re-export的transitive public surface不会自动成为源码候选。因而“metadata已在内存中”不是可见性证明。

每个import/re-export结果非可选地保存binding group；exact function/extension/property角色可能对应overload set，不能伪装成单一entity：

```text
ImportedBinding {
    local_name: Identifier,
    targets: NonEmpty<ImportedTargetBinding>,
}

ImportedTargetBinding {
    target: ImportedTarget,
    sources: NonEmpty<ImportBindingSource>,
}

ImportBindingSource = CurrentCone { source_binding: LocalBindingId,
                                    witness: LocalImportWitness }
                    | DirectDependency { provider: DirectDependencyId,
                                         provider_identity: ConeIdentity,
                                         exported_binding: DependencyExportBindingId,
                                         witness: DependencyImportWitness }

ImportedTarget = Function { origin: PersistentFunctionId, witness: FunctionImportWitness }
               | Property { origin: PersistentPropertyId, witness: PropertyImportWitness }
               | Type { origin: PersistentTypeId, witness: TypeImportWitness }
               | TypeAlias { origin: PersistentTypeAliasId, witness: AliasImportWitness }
               | ... // 对每个合法namespace/role穷尽列举的封闭sum
```

`sources`说明本Cone为何有权看到该target，target的kind-specific `origin`说明它究竟是哪一个实体。普通exact/star import可使用两个source分支；`public import`的source集合必须非空且全部是`DirectDependency`，不能把`CurrentCone` binding复制成re-export。链式re-export只延长dependency witness，不改变target origin。

钻石图中先按kind-specific origin id合并target，再对其所有合法source witness排序去重；不能在第一次命中时丢弃其余路径。`DirectDependency` source的canonical key为`(provider canonical coordinate, provider_identity, exported_binding persistent id, witness中的逐跳{ConeIdentity, export binding persistent id})`，`CurrentCone` source则使用persistent local binding/source identity；session-local `DirectDependencyId`、artifact加载顺序和路径长度都不参与判等。诊断选择排序后的第一条路径作主说明并可列出其余路径；re-export metadata保存完整排序后的source集合，因此删除任一路径都会确定性改变snapshot/fingerprint，而保留的另一条路径仍能继续授权。不同origin即使package/name/signature文本相同也保持不同候选并按普通重复/歧义规则处理。禁止用可任意cast的`PersistentEntityId<Kind>`或裸32-byte值绕过封闭target sum。

split package允许存在，但不合并实体identity。exact selector若命中不同origin的非overloadable实体，报告含Cone coordinate的歧义；函数/extension的不同origin可形成同一候选层中的overload set，相同展开签名仍是歧义而不是偷偷按dependency顺序覆盖。`as`作用于已经唯一解析的exact selector，不能用来让一个本身歧义的selector任取其一。

### 2.3 名称与候选层

无显式receiver的名称/调用层从高到低为：

1. 词法binding与local function；符合M11/M18规则的callable-value binding保持hard shadow；
2. 隐含`this`的真实member；
3. 当前文件的exact import（含alias与`public import`）；
4. 当前package的可访问声明/binding；
5. 当前文件全部star import；
6. core `.slib`提供的typed prelude；
7. M22由唯一expected exact enum application产生的contextual variant fallback。

显式receiver首先取真实member分区；extension scope再按exact import → current package → star import → core prelude分层。property-like `invoke`在每个scope内继续使用M18的c-level分区，property/extension property读写继续使用M21的typed accessor/place模型。constructor、variant、operator、callable reference和typealias qualifier进入各自既有typed入口，不建立import专用resolver。

对callable层，M16的规则保持不变：在每层执行shape filter与candidate-local applicability，选择第一个**至少有一个适用候选**的层，然后只在该层做MSC。同名但全部不适用的exact import不会阻止下一层；失败trace仍可用于最终诊断。对type、object、property name等非overloadablelookup，同层多个不同origin直接歧义。

同一typed origin在一个层内重复import或由多个star path到达只保留一次；同一origin同时出现在高低两层时保留层级关系。声明顺序和dependency枚举顺序不能作为tie-break。

### 2.4 re-export表面

`public import`只允许最终target至少由一个direct dependency surface授权（implicit core也算direct edge），且合并后的source witness集合必须全部为`DirectDependency`；同一origin可以同时由多个direct dependency surface授权。当前Cone自己的public声明已经由普通export规则进入表面，不需要也不能经public import复制。dependency surface中的target可以本身是re-export，因此链式转发合法。

re-export不生成wrapper、forwarder、第二个TypeDescriptor、第二个typealias target或第二份generic body。`.slib`只写：

- destination package/name与namespace kind；
- target的persistent typed origin id；
- 非空、canonical排序的immediate direct dependency/export binding/public reachability witness集合；同一origin可同时由多个direct dependency表面授权；
- overload group/extension/operator/property/typealias所需的typed角色索引。

两个local declaration/public import在同一destination namespace形成非法不可重载冲突时，在发布当前Cone `.slib`前报告；不能把有歧义的public surface留给下游。public star展开后逐项应用同一规则。

### 2.5 visibility、inheritance、default与alias

- 跨Cone普通lookup只看public lookup surface；internal精确限制在origin Cone，file/member private不因metadata closure存在而可见；
- public open/abstract class、public interface及其可继承contract还导出独立inheritance/slot surface，其中可包含合法protected constructor/member、abstract obligation、interface default source与override relation。它们只在下游subclass/implementation上下文通过M21 inheritance witness访问，不进入top-level import或普通member候选；
- public slot由narrower owner中的实现填充时，MIR metadata可以引用该实现symbol，但这不会把实现声明加入HIR lookup surface；
- exported generic template的private/internal helper进入hidden support closure，仅供typed template concretization；reader不给resolver暴露其名称。link-visible hidden support symbol不等于语言public；
- exported default template仍只能含M17 refined export-interface refs。decoder只重映射这些typed refs与definition origin；调用方的import、alias、同名声明或re-export路径不能重新解析default body；
- public `const val`携带typed value；re-export不复制storage。non-generic typealias携带自身persistent alias id和typed target，import/re-export保留alias binding，使用时仍展开到同一个target type identity；
- public API签名中通过typed dependency closure出现但未re-export的外部type仍可供推断、layout与member检查使用，但不会自动获得可书写短名。要向只依赖当前Cone的用户提供名称，作者应显式`public import`；
- re-export不会扩大target自身member visibility，也不能把protected/internal/private target转成public。

## 3. Persistent typed identity、mangling与ODR

### 3.1 entity identity

每个可跨artifact引用的实体kind拥有不同Rust/wire newtype，例如：

```text
PersistentTypeId
PersistentFunctionId
PersistentGenericFunctionId
PersistentPropertyId
PersistentTypeAliasId
PersistentDispatchSlotId
PersistentInitializationUnitId
PersistentLayoutId
PersistentExactTypeId
PersistentCallableBodyId
PersistentStaticStorageId
PersistentImmortalObjectId
PersistentSafepointSiteId
```

不能定义一个可随意cast的`PersistentEntityId([u8; 32])`公共API。wire上即使payload同为32字节，field tag与decoder目标kind也必须匹配。

id由domain-separated SHA-256(canonical definition key)产生。definition key至少含：origin `ConeIdentity`、package、typed owner chain、实体kind、源码名称以及语言duplicate-declaration规则采用的normalized signature key。signature使用alias展开后的persistent type refs与binder位置，不含body、default表达式、visibility、arena ordinal、source枚举顺序或link symbol。改变body保持声明identity但会改变semantic/code fingerprint；改变合法重载签名产生新identity。

编译器生成实体使用typed owner path：closure/local function/default expansion helper以owner persistent callable + structural definition path；box/adapter/coroutine frame以生成角色 + exact source/target type；global/string/init storage以所属声明或content role。只有file-private/top-level hidden且语义允许同名的实体才把normalized Cone-relative source identity纳入key。

每个metadata section保留`IdentityRecord { typed_id, canonical_key }`。reader重新计算并验证。相同typed id对应不同canonical key、同一canonical key重复成不同kind或key引用不存在owner均为artifact错误；canonical key只用于验证/诊断，不在typed lookup失败时回退搜索。

跨Cone签名、specialization与runtime type登记使用的“exact type”也有一套封闭、可重算的结构身份，不能把session-local `TypeId`、display string或某个布局碰巧相同当作identity：

```text
ExactTypeKey = Nominal { declaration: PersistentTypeId }
             | NominalApplication { origin: PersistentGenericTypeId,
                                    arguments: NonEmpty<PersistentExactTypeId> }
             | Tuple { elements: NonEmpty<PersistentExactTypeId> }
             | Function { effect: Ordinary | Suspend,
                          parameters: Vec<PersistentExactTypeId>,
                          result: PersistentExactTypeId }
             | RawPointer { pointee: PersistentExactTypeId }
             | NativeFunctionPointer {
                   calling_convention: NativeCallingConvention,
                   parameters: Vec<PersistentExactTypeId>,
                   result: PersistentExactTypeId
               }

PersistentExactTypeId = SHA-256("scoop-exact-type-v1" || canonical(ExactTypeKey))
```

primitive、`Unit`及其他core nominal type走`Nominal`；`T?`先按语言规则脱糖成core `Option<T>` application；non-generic typealias先透明展开，alias identity不进入exact type key。`RawPointer`专门表示13.10的`Ptr<T>`，不能伪装成普通core nominal application；`NativeFunctionPointer`专门表示`FunPtr<F>`的native code pointer，不能与managed `Function`混用。后者v1的`NativeCallingConvention`封闭为`C`，参数/结果是完成C-FFI-safe验证后的exact type，ordinary/non-suspend约束在构造该key前检查；未来增加calling convention必须新增稳定tag而不能复用display string。template中的binder ref不是exact type，仍以带binder位置的signature type表达；只有替换完成的concrete type才能产生`PersistentExactTypeId`。tuple与managed function是结构类型；pointer两种variant则保留其不同provenance/ABI。相同有序结构跨Cone得到同一identity，不同形状或不同pointer/function category即使当前target layout相同也不能合并。wire保存按dependency-first顺序编码的`ExactTypeRecord { id, key }`，reader在把nominal ref视为叶节点后验证结构边无环并重算id；不能用递归字符串hash或遍历插入顺序决定结果。

`TypeDescriptor.diagnostic_name`不采用源码、import/re-export alias或consumer本地display spelling，而由已验证的`ExactTypeRecord`通过唯一的`CanonicalExactTypeDiagnosticName` printer产生。printer只输出ASCII，先定义`esc(UTF-8 bytes)`：`[A-Za-z0-9._-]`原样保留，其余每个byte（包括`%`）写成`%HH`大写十六进制。nominal declaration atom固定写为`n(c=<esc(canonical ConeCoordinate)>;p=<esc(package)>;o=<owner>;k=<kind>;x=<esc(source name)>)`；`kind`是封闭ASCII tag `C=class | I=interface | S=struct | E=enum | O=object/companion | A=annotation class`，typealias没有tag因为进入printer前已展开。空owner写`-`，非空typed owner chain按从外到内的`<kind>:<esc(source name)>`以`/`连接并使用同一tag表。它只读取声明的canonical origin coordinate/package/typed owner/name，不读取使用点qualifier、alias路径或可读mangle。递归variant grammar固定为：`Nominal = <declaration atom>`、`NominalApplication = a(<origin atom>;[<D(arg)>,...])`、`Tuple = t([<D(element)>,...])`、ordinary/suspend managed function分别为`f(o;[<D(param)>,...]-><D(result)>)`/`f(s;[...]->...)`、`RawPointer = r(<D(pointee)>)`、C native function pointer为`x(c;[<D(param)>,...]-><D(result)>)`；`,`、`;`、`[]`、`()`和`->`都是字面ASCII delimiter，元素顺序不变，空parameter list写`[]`。这里`D`按dependency-first exact-type graph递归；transparent alias在进入key/printer前已展开。producer/reader在分配或写出字符串前，以memoized checked subtree byte cost按每条展开路径重计共享type DAG，并复用4.5单个text/bytes语义字段16 MiB上限；cycle、算术溢出或超过上限均拒绝，不能先递归展开再检查长度。reader必须从key重算这些UTF-8 bytes，并拒绝同一`PersistentExactTypeId`对应不同bytes。

上段`n(...)`只适用于拥有source declaration atom的nominal。compiler-generated nominal不能伪造source name，固定写为`g(r=<8位小写十六进制role tag>;i=<64位小写十六进制PersistentTypeId>)`。v1 `GeneratedNominalRole`冻结为`ClosureEnvironment=1, CallableAdapterEnvironment=2, CoroutineFrame=3, ContinuationAdapterEnvironment=4, CoroutineStep=5`；stable structural definition path及callable/exact owner已经进入该`PersistentTypeId`的canonical key，printer不得读取arena ordinal、临时name或mangle，新增role必须扩展identity schema。递归grammar中的`Nominal`与`NominalApplication.origin`接受且只接受上述source/generated atom两分支。

`PersistentLayoutId`由exact type、target ABI profile和封闭representation role派生，只标识target-specific layout record；它不是language type identity。`PersistentStaticStorageId`由owner declaration或specialization identity加封闭storage role派生，`PersistentImmortalObjectId`由owner declaration/specialization、stable definition path与object role派生，`PersistentSafepointSiteId`由concrete callable identity加CFG site role/ordinal派生。它们分别用于layout bridge、static address、immortal range与stackmap site，互不转换；constant内容进入definition fingerprint而不替换owner identity。TypeDescriptor本身直接以`PersistentExactTypeId`作为语义登记key，再由strong/ODR member id标识具体record，不另设会与exact type产生双重真相的`PersistentTypeDescriptorId`。

每个由LIR定义、在当前Cone `code.o`中实际发射的Scoop callable body另有统一但不擦除来源的owner identity，供entry pointer、stackmap与safepoint site共同引用：

```text
CallableBodyKey = Strong { owner: StrongCallableDefinitionOwner }
                | Odr { member: CallableOdrMemberId }
                | RootGateway { root_cone: ConeIdentity,
                                main: MainCallableBodyId }
                | InitializationStartupGateway {
                      unit: PersistentInitializationUnitId
                  }

PersistentCallableBodyId =
    SHA-256(ByteSpan("scoop-callable-body-v1") || canonical(CallableBodyKey))
```

`CallableBodyKey`使用与6.1相同的`u32`小端tag与声明序product编码，v1变体严格冻结为：`Strong=1`，payload只有`owner`；`Odr=2`，payload只有`member`；`RootGateway=3`，payload依次为`root_cone, main`；`InitializationStartupGateway=4`，payload只有`unit`。不得按Rust enum ordinal、字段名、display string或host布局编码；unknown/reserved tag一律拒绝。

`StrongCallableDefinitionOwner`与`CallableOdrMemberId`是只接受callable atom的typed refinement，不能塞入storage/TD member；`MainCallableBodyId`则只接受root Cone中top-level ordinary/non-generic/non-suspend `() -> Unit` main的Strong body id。source/generated initializer/ensure等普通body使用前两种，两个C-callable no-throw gateway必须使用后两种专门variant，不能伪装成被调用的main/ensure。所有C record中的`*_callable_id`与safepoint的`owner_callable_id`均精确表示`PersistentCallableBodyId`；对应identity record保存完整tagged key供reader/link verifier重算。gateway primary symbol由该body id的专用mangler kind产生；root gateway始终`ConeStrong`，init startup gateway的strong/ODR linkage及group/member归属与其unit一致。这样runtime可直接用root record暴露的main body id重算gateway id，而无需反推出source function id；gateway内部managed call及异常物化路径上的每个site都以`{ body id, typed site role, stable local ordinal }`产生自己的`PersistentSafepointSiteId`，不会与main、ensure或另一Cone的wrapper共享编号。

这里的注册全集精确等于LIR的`RegisteredCallableBody`集合：包含`code.o`中的普通managed/NoGC Scoop body、compiler-generated adapter/trampoline及root/init gateway，不包含只有声明而没有本Cone body的`@Extern` target、runtime archive函数，也不包含由C compiler写入`bridge.o`的native storage bridge。后三类分别使用extern/native-bridge typed identity与既有object verifier，不获得`PersistentCallableBodyId`，也不进入callable table；因此“每个body登记”不要求在`.slib`封装后回头patch已验证的`bridge.o`。

凡concrete exact type进入当前Cone LIR的layout/type closure，或作为param-free exported LIR bridge，就属于**runtime-materialized type**并必须生成TypeDescriptor registration；只存在于尚未替换的Export HIR template/binder中的type尚不materialize。tuple、managed function、raw/native pointer等非nominal exact type以其`PersistentExactTypeId`建立3.4的`StructuralType` ODR group，多个Cone重复materialize时整体coalesce。因而每个最终程序中materialized exact type恰有一个TypeDescriptor地址，而纯template type不会为了“可能未来使用”提前生成伪descriptor；是否materialize不改变语言type identity。

### 3.2 session-local remap

serialized table index只是wire压缩索引，不是semantic id。reader分两阶段：

1. 验证所有identity record并按kind建立`Persistent*Id → Imported*Id`映射；
2. 分配consumer semantic world的fresh typed arena id，再解析全部edge、body、bound、origin与witness。

同一origin从钻石路径再次出现时复用已有world id，并要求schema payload/semantic fingerprint一致；不同origin永不因同FQN或相同结构intern成一个declaration。current Cone source实体、imported export实体和LocalConcrete实体仍使用互不兼容id家族；从imported template到local specialization必须通过显式`ConcretizationOrigin` relation。

`IntrinsicProviderId`在M23后退为compile-session authority slot，不再承担持久Cone身份。driver把trusted core `ConeIdentity`映射到`IntrinsicAuthority::Core`；测试allowlist仍是内部typed capability。普通artifact不能通过写同coordinate、provider整数或manifest flag取得intrinsic authority。

### 3.3 symbol规则与linkage

所有Scoop-owned linker-visible symbol由唯一mangler消费typed persistent identity，基本形态为：

```text
scoop$1$<kind-tag>$<lowercase 64-hex persistent/ODR id>
```

human name只放debug/display metadata，不进入linker判等。以下全部迁移，不能只修普通function：

- non-generic/generic function与method、constructor、accessor、closure/coroutine/callback adapter；
- global/property/delegate/singleton/failure storage、init cell/entry/descriptor；
- String/constant/scan program、layout helper、TypeDescriptor、vtable/itable/adjust thunk；
- C bridge wrapper与image descriptor；
- executable root entry。

`@Extern`指定的native symbol不改写；C ABI bridge自身是Scoop-owned symbol并必须namespaced。runtime固定ABI symbol保持`extern "C"`命名，不与source entity mangler混用。

linkage使用封闭分类：`ConeStrong`、`TemplateSupportHidden`、`OdrWeak`、`RuntimeAbi`。语言public/internal/private不直接映射为这四项：例如generic public body调用的private helper必须是`TemplateSupportHidden`并可被下游specialization解析，但仍不可import；只有真正同一specialization才可用`OdrWeak`。

### 3.4 specialization、layout与ODR group

program-wide specialization key以封闭sum区分被实例化的语义实体：

```text
SpecializationKey = Nominal { origin: PersistentGenericTypeId, exact arguments }
                  | Callable { origin: PersistentGenericCallableId,
                               exact owner application,
                               exact callable arguments }
                  | DelegatedProperty { origin: PersistentExtensionPropertyId,
                                        exact receiver arguments }
                  | StructuralType { exact_type: PersistentExactTypeId }
OdrGroupId = SHA-256("scoop-odr-v1" || canonical(SpecializationKey))
OdrMemberId = SHA-256("scoop-odr-member-v1" || OdrGroupId || generated role || typed owner path)
```

所有argument/application必须是persistent exact type identity；没有owner或没有callable arguments由对应variant的typed `NoOwnerApplication`/`NoCallableArguments`分支表达，不能靠空Vec猜实体类别。`generated role`标识group内member而不进入group key；否则function、TypeDescriptor和storage会被错误拆成互不关联的多个“组”。

- param-free上游type/callable由定义Cone以`ConeStrong`发射，layout/dispatch/TypeDescriptor由其LIR meta导出；
- 上游generic template与下游local type组成的新application在**实际消费Cone**完成HIR concretization、layout、MIR/LIR与发射。定义Cone不可能预计算`Upstream<DownstreamLocal>`，因此“layout只在定义Cone计算一次”只适用于param-free实体；
- 多个Cone产生相同specialization时，各自发射同一`OdrGroupId`，每个同role member具有相同`OdrMemberId`、symbol和`OdrWeak`属性。nominal group包含它产生的layout、TypeDescriptor、vtable/itable、scan与相关box/adjust member；callable group包含body及它产生的closure/coroutine/adapter；delegated-property group包含storage/root/cell/failure/init/ensure/descriptor；structural-type group包含materialized layout、scan、box与TypeDescriptor。任何specialization-owned的runtime registration record、address-taken string/constant、diagnostic bytes或其他relocation target都必须是同组的显式member，不能指向consumer-local private定义，也不增加未列入`SpecializationKey`封闭sum的“content group”旁路。group可以typed-ref引用另一个由既有封闭variant产生的canonical group，但不能把自身产生的runtime identity漏在组外，尤其不能只coalesce函数而留下两个TypeDescriptor、registration或static storage；
- Darwin/Mach-O按每个`OdrMemberId`发射同名`linkonce_odr`/`weak_odr` coalesced symbol，并不假设Mach-O存在原子COMDAT group；支持COMDAT的未来target可以把同组member放入COMDAT。两条路径都先验证完整member set与definition fingerprint，使native linker逐member任选winner也只能得到等价的一组定义；
- 每个producer在`.slib` manifest记录`OdrRecord { group, members, abi_fingerprint, definition_fingerprint }`。ABI值为`SHA-256("scoop-odr-abi-v1" || canonical sorted member ABI shapes)`；每个受检definition同时由LIR meta保存`CanonicalLirDefinition { owner, canonical payload }`，reader从payload重算`LirDefinitionFingerprint = SHA-256("scoop-lir-definition-v1" || owner || payload)`。最终ODR digest本身不进入该payload或LIR semantic own-layer，避免反向自引用。`OdrDefinitionFingerprint = SHA-256("scoop-odr-definition-v1" || group || 按member id排序的{role, LirDefinitionFingerprint, ordered object-definition fingerprints, ordered stackmap fingerprints})`；没有对应leaf的列表使用typed empty variant，不能靠遗漏字段表示。ABI fingerprint只用于接口诊断；link前必须比较完整member set与definition fingerprint，只有ABI相同而body、initializer、constant、scan/vtable、stackmap或relocation不同仍必须失败；
- actual-object范围不能只覆盖ODR，否则strong root/init gateway、param-free TypeDescriptor和strong registration会退回“相信LIR”。`CurrentConeLir`非可选地携带`ObjectDefinitionPlan { owner, role, expected primary boundary symbols, expected associated-record roles }`；`owner`是封闭sum `Strong { kind-specific persistent entity id, strong role } | Odr { member: OdrMemberId }`。plan覆盖每个参与派生fingerprint的strong definition和每个ODR member。Mach-O `nlist`没有可靠symbol size，因此codegen为每个primary atom及address-taken constant/registration发射plan指定的stable start/end boundary symbol，设置`MH_SUBSECTIONS_VIA_SYMBOLS`，形成不重叠named subsection atom。object verifier从真实symbol/section/relocation table产生`VerifiedObjectDefinitionIndex { DefinitionAtomRange, associated records, ObjectDefinitionFingerprint... }`；manifest保存全部definition range，`OdrRecord`只引用其中owner为该组member的精确子集，artifact reader再从object独立重算。range要求边界同section、严格有序、padding按规范全零且归属于前一atom，并拒绝未被任何range归属的受检relocation；
- LIR另输出`DigestFinalizationPlan { nodes }`，每个`DigestNode { typed node id, DigestKind, canonical direct inputs, patch sites }`。`DigestKind`封闭为`SourceSignature`、`Layout`、`Scan`、`LirDefinition`、`ObjectSupport`、`ObjectDefinition`、`StackmapRecord`、`OdrDefinition`、`StrongRegistration`、`RuntimeImage`；前四类从canonical LIR metadata产生语义leaf，允许没有object落槽，其余leaf来自声明的object range或canonical runtime record。`DigestInputRef`也是封闭sum，只能引用上述typed node id，不能把裸digest、object offset或“当前已算出的值”当依赖。`PatchSite`非可选指明object member、target definition owner/atom role、checked offset、固定width 32与唯一source node，即使digest源atom与落槽descriptor属于不同owner也不靠裸offset关联；
- 允许边按kind封闭：四种semantic leaf与object support无依赖；stackmap可依赖owner的source signature与object support；object definition可依赖其semantic leaf、associated stackmap及object support；ODR definition依赖完整member set的LIR definition、object definition与stackmap leaf；strong registration依赖自己的LIR/layout/scan/source、object definition及适用的stackmap leaf；runtime image必须显式依赖其每个canonical record key中出现的definition/layout/scan/descriptor/gateway/stackmap digest。交叉record仍只引用对方typed registration identity，除非其digest确实作为本record字段出现。peer、descendant、反向边或kind不允许的shortcut一律非法；
- digest node自身也有可重算的persistent identity：`DigestKind`按上段顺序冻结为`1..10`，`DigestNodeId = SHA-256(ByteSpan("scoop-digest-node-id-v1") || u32(kind) || ByteSpan(canonical owner-and-role key))`。owner-and-role key使用LIR wire的persistent typed owner与封闭role，不含arena id、patch offset或任何digest值。一个node的direct input sequence按`(DigestKind tag, DigestNodeId bytes)`严格递增且无重复，每项编码`{ u32 kind, DigestNodeId[32], digest[32] }`并带`u64 count`；plan缺少必需input、增加kind不允许的input或同id对应不同owner key均拒绝；
- strong registration的算法固定为`StrongRegistrationFingerprint = SHA-256(ByteSpan("scoop-strong-registration-v1") || CanonicalStrongRecordSansOwnDefinition || ordered typed direct inputs)`。`CanonicalStrongRecordSansOwnDefinition`恰为6.1完整`RuntimeImageRecordKey` sum的canonical bytes：最外层sum tag作为record kind只写一次，variant payload中的registration不再重复kind；linkage必须为Strong、ODR group/member全零、当前registration的`definition_fingerprint`写32个零字节。其他source/layout/scan/descriptor/gateway/callable/stackmap digest字段保留已完成的直接依赖值，pointer按typed role/identity替换。它不读取PatchSite、RuntimeImage或最终object地址。finalizer与reader从record、LIR plan和verified object index独立重建该输入；不得在完整variant bytes前再写第二份kind，只能把own definition slot归零，不能按遍历时机再归零已经声明的上游digest；
- digest计算是纯函数：每个node只读取其canonical source与声明的直接input digest；对object-backed source，`normalize(node, ranges)`只保留该node**传递依赖**所写且位于受检range中的digest，node自身及所有非依赖（peer、descendant、无关image）patch slot一律归零，再规范化relocation槽。不能用“当前遍历时尚未计算”定义hash视图。`ProvisionalCodeObject`中所有graph-managed digest slot初始为零；plan verifier在codegen前与object产出后都验证node id/order canonical、allowed-edge DAG无环、每个object-resident digest槽恰有一个writer、所有应有槽被覆盖、patch range无relocation且精确32 bytes。finalizer按任意合法topological order计算后一次性核对/回填；manifest保存可重放的typed graph，reader从canonical source重算，而不是信任历史写入顺序；
- per-Cone C字段与writer node固定一一映射：source/layout/scan字段分别由`SourceSignature`/`Layout`/`Scan`写；TypeDescriptor的`descriptor_fingerprint`及每条callable registration的`body_definition_fingerprint`由对应atom的`ObjectDefinition`写；root及**Eager** init的gateway definition字段不是独立承诺，而是同一gateway `ObjectDefinition` node的额外PatchSite，必须逐byte等于其callable registration的body字段。Lazy init的gateway body id/fingerprint是固定全零tagged encoding，不是graph-managed slot、没有node/PatchSite；`normalized_stackmap_fingerprint`由对应`StackmapRecord`写；registration的`definition_fingerprint`由strong record的`StrongRegistration`或ODR record所属`OdrDefinition`写；image字段由`RuntimeImage`写。`LirDefinition`与`ObjectSupport`可以只存在verifier index而无C落槽。禁止为这些字段另造同名hash算法，且一个node写多个等价slot时所有slot最终byte必须相同；
- canonical relocation target是封闭sum：`NamedPersistentSymbol { kind, persistent/ODR/runtime id }`、`OwningAtomOffset { owner, checked offset }`、`AssociatedRecordOffset { owner, record role, checked offset }`、`SectionBasePair { canonical section role, associated owner/support id, checked addend }`。后两项覆盖LLVM正常产生的section-relative compact-unwind与`SUBTRACTOR(section base) + UNSIGNED(function)` EH pair；reader把Mach-O composite relocation先解析成typed form，要求offset/addend落在已声明range内，绝不能hashsection number、temporary symbol或object-local index。AArch64 relocation pair作为一个有序typed relocation归一化，unknown或跨range pair直接失败；
- 任一function owner的associated-record闭包都必须覆盖与其语义绑定的`__gcc_except_tab` LSDA、`__eh_frame` FDE/CIE relation、`__compact_unwind`记录及LLVM stackmap function/callsite payload；记录按上述typed owner和relocation关联，而不是只hash`__text`。共享CIE等以`ObjectSupportFingerprint = SHA-256("scoop-object-support-v1" || support role || normalized bytes/relocations)`成为support node；definition leaf使用`"scoop-object-definition-v1"`，stackmap leaf使用`"scoop-stackmap-record-v1"`并覆盖完整site/owner/v3 payload。safepoint registration的normalized fingerprint必须等于对应stackmap node。object-level测试从真实Mach-O重算definition graph，并人工篡改strong/ODR primary atom、support与每类associated metadata证明finalization会失败；
- native linker完成coalesce后，最终artifact verifier确认每个TypeDescriptor identity只有一个地址、dispatch引用指向winner、无重复strong定义；runtime再做防御性验证；
- same source template但不同exact arguments、same arguments但不同template origin必须是不同group；同group的不同generated role必须是不同member而不是不同group。

该规则正式取代旧的Cone-local generic overload discriminator和只按短name编码type的mangling。重复实例去重是semantic identity驱动的结果，不是碰巧同名。

### 3.5 source、unit、runtime type与safepoint identity

- definition/evaluation origin保存`ConeIdentity + normalized relative source path + span + typed source context`。`.slib`不保存host绝对路径；re-export不改变definition origin；
- required source table保存path、UTF-8 content digest和line-start offsets，使下游default/current-source-location可稳定计算行列。完整源码可放optional diagnostic section，但不参与typed语义，缺失时仍能给路径/行列诊断；
- M21的二进制unit identity唯一使用`PersistentInitializationUnitId`；该id的definition key已经包含origin `ConeIdentity`或ODR specialization identity。canonical Cone coordinate与declaration path只形成独立`diagnostic_path`，不参与unit identity或初始化排序；作为descriptor的诊断内容，它仍按6.1进入registration definition、RuntimeImage fingerprint与ODR duplicate逐字段一致性检查。排序先按typed unit id bytes，诊断与cycle message才显示coordinate/path；
- `runtime_type_id: u64`与`SafepointId: u64`分别由对应256-bit persistent type/site key的domain-separated digest截取非零64位。每个`.slib`列出`{id, full_key, ODR/entity owner}`全集；全程序允许同一full key的重复记录，但必须验证其ODR fingerprint且link后指向同一descriptor/PC，随后去重。不同full key得到同一个64位id才是碰撞并使构建失败，不能加随机salt或退回FQN；
- safepoint site key含上述`PersistentCallableBodyId`、typed CFG site role与stable local site ordinal。root/init no-throw gateway与其调用的main/ensure是不同body owner；不同Cone从1开始编号不再合法。

### 3.6 ZST布局与跨Cone ABI

M23不能把ZST留成“LLVM空struct碰巧能工作”的后端细节：layout、Scoop ABI、array metadata与TypeDescriptor一旦进入`.slib`，上下游必须对size 0使用同一规则。这里采用与Rust相近的**零payload**模型，但不复制`Vec` capacity/dangling-pointer等库实现约定；Scoop的`Array`是有对象头和logical size的固定长度ref object。

```text
ValueStorageLayout = ZeroSized { alignment: NonZeroPow2 }
                   | NonZero { size: NonZeroU64,
                               alignment: NonZeroPow2,
                               scan: RefScan }

ArrayElementStorage = ZeroSized { alignment: NonZeroPow2 }
                    | Inline { stride: NonZeroU64,
                               alignment: NonZeroPow2,
                               scan: RefScan }

AbiArgument = ElidedZst { exact_type: PersistentExactTypeId }
            | Direct { ... }
            | Indirect { ... }

AbiReturn = UnitVoid
          | ElidedZst { exact_type: PersistentExactTypeId }
          | Direct { ... }
          | Indirect { ... }
```

- `Unit`与空ordinary struct固定0/1；tuple和其他ordinary非`@CLayout` struct在全部element/field为ZST时递归为ZST，alignment仍取最大值。ZST field可共享offset 0且不增加outer size；M23不新增enum discriminant消除，`Option<ZST>`仍有tag。`ZeroSized`结构上蕴含`gc_free`及`RefScan::None`，但GC-free类型不一定是ZST；
- generic `@CLayout`不能在provider处把依赖type parameter的字段误判为“当前非ZST”。无字段或不依赖binder而已知不满足C表示/NonZst的声明立即诊断；依赖binder的字段在Export HIR保存`CLayoutFieldRequirement::CFieldSafeAndNonZst { signature_type: SignatureTypeRef, declaration_field_path }`，每个consumer concrete application替换后重放该predicate并在concretization点诊断。该predicate进入template semantic fingerprint，不能仅导出一个未知时默认为true的bool；
- exact type identity与layout分离：`Phantom<A>()`、`Phantom<B>()`、`Unit`即使都是0/1也有不同`PersistentExactTypeId`和TypeDescriptor。constructor/call/getter/RHS/array literal或spread的求值与异常/副作用都保留，只消除payload byte的copy/load/store；
- address-taken参数、local、raw global或value-typed `this`按place/lifetime物化满足alignment的non-null 1-byte token，同时存活的不同place不得共址。compiler-managed static/init storage使用6.1的persistent token；token的allocation extent为1但logical `byte_size`仍为0。普通ZST SSA value不分配token，array element和field也不因内部GEP自动获得独立地址；
- typed Scoop ABI保留source signature中的每个exact ZST，但物理参数/返回不传payload。`UnitVoid`与“函数返回用户ZST”不能合成一个IR variant；callee需要`addressOf`参数时自行物化method-local token。mangler、override、function type、adapter和call fingerprint仍包含被elide的exact type，所以两个物理LLVM signature相同的source callable不会碰撞；
- `Array<Z>`/`MutableArray<Z>`的对象仍有独立ref identity、header和`size: Int`；data offset为`alignUp(24, alignOf<Z>())`，exact allocation size就等于该offset并满足element alignment，与logical length无关。`get`按receiver → index求值后做整数bounds check并产生typed ZST；`set`遵守普通call顺序，先求值receiver → index → RHS，再由intrinsic执行bounds check，成功后不写payload，因此越界也不能跳过RHS副作用/异常。assembly仍对各part计数并保留全部求值，clone/互转仍分配新array但不发`memcpy`；
- array iterator永久使用`{ array ref, Int index }`，以`index < size`终止并按1递增；不能使用`current += stride`或`current == end`，否则stride 0不前进。LIR/runtime对ZST或其他GC-free element把object scan直接规范化为`None`；非空array object scan使用自包含的`RefScan::Array { length_offset: 16, first_element_offset: data_offset, stride: NonZeroU64, element: NonEmptyRefScan }`，两个offset和stride均进入scan fingerprint，不能把over-aligned element的data offset默认为24。collector工作量不随ZST logical length增长；
- boxing ZST仍分配普通managed object：其TypeDescriptor使用`BoxedValue { inline_storage: ZeroSized }`，allocation采用6.1的对齐后`inline_offset/minimum_size`，side metadata登记精确非零object size，payload不占字节。LIR以`BoxPayload::{ZeroSized, NonZero { source_place }}`和对应`UnboxResult::{ZeroSized, NonZero { destination_place }}`封闭区分；ZST分支不制造/传递/解引用payload pointer，non-ZST的size/alignment/scan只从exact TD读取而不由callsite重复提交。NonZero的`source_place`必须是跨本次runtime call地址稳定、满足value alignment的caller temp；若TD的inline scan非空，caller必须先把完整value写入该temp，再用封闭`GeneratedRootFrameOp::PushRecursiveRegion`调用compiler-private `GeneratedNoGcLeafTargetRef`，以TD的inline scan登记覆盖该temp的root。该leaf不分配、不park、不握手、不回调且不产生statepoint；push必须在调用`scoop_rt_box_value`及其managed-entry handshake之前完成。region root贯穿entry handshake、allocation与payload copy，moving collector原地更新temp中的ref slots；runtime从同一已登记temp复制，返回caller后才以匹配的typed pop按LIFO移除。CFG verifier要求push/pop在所有非fatal出口配对，runtime入口只验证root已经活跃，不能在入口内补登记；空scan可省略该frame。不能在push前后缓存并改从旧AS1 leaf/旧副本复制。每次boxing照常产生ref identity，不使用shared singleton或place token；ZST unbox产生typed logical value，只有后续真正取址才另建place token；
- `Ptr<ZST>`在Scoop unsafe代码中合法：offset的byte displacement恒为0、pointer bits不变，load/store保持全部operand求值与pointer validity/alignment/lifetime前置条件但不访问payload。算法不得用pointer变化表达进度；`Ptr<Unit>`是opaque `void *`，需要byte arithmetic时使用`Ptr<UInt8>`。C ABI只把`Unit` result映射为`void`并保留`Ptr<Unit>`例外；ZST by-value参数/result、C-boundary extern global/TLS、空或含ZST字段的`@CLayout`及其他`Ptr<ZST>` C pointee在v1拒绝，本Cone内部compiler-owned/raw ZST storage仍按token规则存在。

这些规则必须进入LIR wire schema、layout/ABI fingerprint与artifact tests；否则同一个generic template可能在provider/consumer分别按empty aggregate、1-byte占位或zero-stride array生成不兼容代码。

### 3.7 native extern contract closure

native linker只按目标符号工作，不会检查两个Cone是否把同一`@Extern`声明成不同ABI。M23因此把extern声明加入构建期typed closure；它不是runtime metadata，不增加第七张image table。

HIR先解析省略的`name`并保存source-level extern contract；LIR在target ABI classifier完成后生成：

```text
NativeExternalSymbolKey = {
    target_profile: TargetProfileId,
    native_link_symbol: ByteSpan
}

PersistentNativeExternalSymbolId =
    SHA-256(ByteSpan("scoop-native-link-symbol-v1") ||
            canonical(NativeExternalSymbolKey))

NativeLibraryBinding = DefaultNativeNamespace                         // tag 0
                     | Requirement(NativeLinkRequirementId)           // tag 1

NativeExternalContract =
    Function {                                                        // tag 1
        library: NativeLibraryBinding,
        abi: C { signature: CanonicalCAbiFunctionSignature }          // tag 1
           | Scoop { signature: CanonicalScoopAbiFunctionSignature }, // tag 2
        calling_convention: TargetCallingConvention
    }
  | ReadOnlyData { library, storage: CanonicalCStorageType }          // tag 2
  | MutableData { library, storage: CanonicalCStorageType }           // tag 3
  | ReadOnlyTls { library, storage: CanonicalCStorageType }           // tag 4
  | MutableTls { library, storage: CanonicalCStorageType }            // tag 5

NativeExternalContractFingerprint =
    SHA-256(ByteSpan("scoop-native-external-contract-v1") ||
            PersistentNativeExternalSymbolId ||
            canonical(NativeExternalContract))
```

`native_link_symbol`是target profile规范化后真正进入object symbol table的bytes；当前Darwin object中的undefined reference不携带声明的`lib`，所以只按该symbol id分组，`library`是必须相等的contract内容。`NativeLinkRequirementId`引用4.2中不含host绝对路径的target-tagged逻辑requirement；空`lib`只能编码为`DefaultNativeNamespace`。M12 v1的function calling convention只有规范化后的`Cdecl`，省略与显式`cdecl`得到同一值；data variant不携带calling convention且ABI固定为C。

两种function signature都在移除参数名/default表达式、把source `vararg`物化为普通array参数并展开transparent alias后编码参数声明顺序与result。C variant使用bridge已经验证的完整C storage type tree、`@CLayout` layout fingerprint、data/code pointer provenance及target参数/返回classifier；Scoop variant使用每个`PersistentExactTypeId`及完备`AbiArgument::{ElidedZst, Direct, Indirect}`/`AbiReturn::{UnitVoid, ElidedZst, Direct, Indirect}`。因此相同size或LLVM function type不构成相同contract；只有canonical native signature逐字段相同才相等。

每个Cone的HIR/MIR/LIR section保留相应typed relation，manifest保存按`(PersistentNativeExternalSymbolId, contract fingerprint)`排序的完整record与local declaration origin集合。public import/re-export保留origin contract；下游extern call/global use及`code.o`或`bridge.o`中由这些extern目标产生的undefined relocation只能引用已有contract id。object verifier把每个`SourceExtern` undefined symbol反向关联到一个record；漏record、一个relocation匹配多个contract或record的symbol/library requirement与真实relocation/link input不一致都拒绝。合法`FunPtr`取址只指向非extern Scoop-owned `@NoGC` body，继续使用callable/object identity；`FunPtr`仅在它作为extern参数/结果的C code-pointer type时进入该contract signature。

每条object undefined-symbol use还必须唯一归属于封闭`UndefinedSymbolRequirement::{ScoopOwned(persistent_entity), SourceExtern(contract_id), RuntimeEntry(runtime_target), GeneratedBridge(bridge_id), TargetSupport(profile_capability)}`。前三种分别由typed IR external ref、extern contract和runtime ABI registry产生；bridge requirement由本次C bridge plan产生；target support只能由target profile的完备compiler-support能力表产生，覆盖后端或C compiler可引入的memory/TLS/EH等helper及其native library requirement。consumer按artifact target/runtime ABI独立验证该表，不能把任意符号、名字前缀或“C compiler生成”当作豁免。manifest保存完整typed requirement index并由LIR/object verifier交叉验证；无分类、同一use多分类或找不到producer/support capability都拒绝。`SourceExtern`才要求源码contract，其他四类继续核对各自typed definition/ABI与link input。同一公开native symbol可以同时被源码extern和target support/公开runtime API使用（例如`memcpy`）；此时support/API能力表必须提供同样完整的canonical native ABI/library contract，linker逐字段合并后才允许共用symbol。Scoop-owned、generated bridge及compiler-private runtime symbol仍禁止被源码extern别名占用，不相容的多种requirement在native linker前失败。

最终linker在调用native linker前合并完整transitive Cone closure中的全部record。同一symbol id必须先证明canonical symbol key相同，再要求library、kind/TLS/mutability、ABI、calling convention及完整signature/storage逐字段相等；相等者去重为一份contract和native requirement，不同者报告全部declaration origin及第一个不同字段。即使其中某个声明在当前root未调用也不交给native linker任选。该闭包只能证明程序内部声明一致；外部binary是否真实实现该contract仍是FFI作者责任。

## 4. `.slib` v1格式与reader

### 4.1 deterministic archive

`.slib` v1使用标准deterministic `ar`容器。manifest中的`object_members`是非空、有序、无重复的成员清单；M23的规范生成器依次使用下列名称：

```text
manifest.cbor
hir.meta.cbor
mir.meta.cbor
lir.meta.cbor
code.o
bridge.o          # 仅存在C bridge时
sources.cbor      # 可选，仅用于更丰富的上游诊断
```

- archive header的timestamp/uid/gid/mode使用规范化固定值；不记录build目录、producer host或临时文件名；
- metadata使用RFC 8949 deterministic/canonical CBOR：整数field tag、definite length、canonical map key order、最短整数编码，不接受重复key、indefinite item或浮点语义字段；
- object bytes原样保存且目标格式由manifest target profile固定。`code.o`始终列入`object_members`，需要C ABI bridge时再列入`bridge.o`；reader与linker按清单而非“一个artifact只有一个`.o`”的假设消费全部对象。driver从已验证archive提取明确标记的object member再交给native linker，不把含metadata member的整个`.slib`盲传给linker；
- 同样输入、compiler/schema/target与dependency semantic fingerprints必须逐byte产生相同`.slib`。两次隔离临时目录构建的bitwise equality属于验收门。

选择标准archive只解决容器边界，不把IR wire schema委托给`ar`。实现应使用成熟archive/CBOR/hash库，不手写通用TOML、archive或CBOR parser。

### 4.2 manifest与兼容头

`manifest.cbor`至少包含：

- magic `SCOOPSLIB`、container version与每个HIR/MIR/LIR wire schema version；
- producer compiler版本（仅诊断）、language ABI、runtime ABI、mangling/identity version；
- canonical Cone coordinate、`ConeIdentity`、Cone kind；
- exact direct dependency records及编译时三层semantic fingerprints；
- target profile id/fingerprint、LLVM/backend profile、object format；
- 每个**非manifest** member的required/optional kind、byte length与SHA-256；manifest不能包含自身member hash；
- public/re-export/prelude index摘要、source table摘要；
- ODR records、全部strong/ODR `DefinitionAtomRange`、每个ODR record引用的精确range子集、typed digest graph、runtime type/Safepoint/callable-body full-id映射、六类runtime registration record摘要、typed `UndefinedSymbolRequirement` index与`object_members`；
- 完整`NativeExternalContractRecord`表及独立的target-tagged native **library** link requirements；contract按`(symbol id, contract fingerprint)`排序，保存canonical symbol key/payload/local declaration origins，library requirement保存逻辑库名、kind、顺序/分组约束和target条件。二者都不记录producer机器的绝对搜索路径，均沿完整transitive link closure传播；
- `hir_semantic_fingerprint`、`mir_semantic_fingerprint`、`lir_semantic_fingerprint`、`code_fingerprint`、`runtime_image_fingerprint`与whole-artifact fingerprint。`graph_fingerprint`只存在最终program descriptor，因为library `.slib`尚无最终闭包。

M23 v1对required schema、language/runtime ABI、identity/mangling version及target fingerprint采用exact match。producer patch版本不同但这些值相同时可以读取；不能简单用“编译器版本字符串相同”替代各层兼容检查，也不能对unknown enum variant猜默认。

`ArtifactFingerprint`精确定义为对domain tag `scoop-artifact-v1`、移除`artifact_fingerprint`字段后的canonical manifest bytes，以及按manifest顺序排列的每个非manifest member `{name, length, sha256}`做长度前缀哈希。writer最后回填该字段并重新canonical encode；reader以同一排除规则重算。这样manifest/member校验没有自引用，archive header的规范化值也不会成为另一套identity。fingerprint只做一致性与cache验证，不是发行者签名。

reader先限制archive/member总大小，再读取manifest并校验所有section hash，之后才反序列化IR。hash是损坏检测和cache key，不是签名；M23不提供恶意发行者认证。

### 4.3 Export HIR wire closure

不能直接序列化当前`ExportHir = Module`别名。packager从typed root建立封闭图，并在wire上标出互斥角色：

1. `PublicLookupSurface`：源码显式public且effective lookup domain允许跨Cone的声明、public alias，以及resolved re-export binding；
2. `InheritanceSurface`：公开可继承owner需要的protected constructor/member、slot/default/override contract和typed access witness；
3. `TemplateSupportClosure`：公开generic nominal/callable/property template body及其递归typed hidden dependency、template-owned lambda/local function、derived body与concretization predicate；只有仍需substitution/concretization的generic或template-owned body进入闭包，origin Cone已经发射的param-free private/internal helper只导出typed signature、persistent target与link requirement，不复制其body；
4. `InterfaceDependencyClosure`：上述表面的签名type、exact ancestry/conformance、annotation、const、default source和well-known core relation；
5. `SourceInterfaceTemplates`：M17 parameter shape与只使用refined export-interface ref的default template；
6. `BindingIndex`：package/name/namespace → typed roots，及prelude/re-export provenance。

同一实体可被多个闭包引用，但wire definition只有一份并携带用途集合。hidden support/inheritance-only实体没有普通binding index entry，reader API也不能枚举其短名。无关private/internal non-generic body、当前Cone调用产生的LocalConcrete instance、solver scratch、import文本和失败候选不写入。

generic hidden closure允许引用narrower实体，但每条edge仍有origin typed id与linkage requirement。param-free hidden dependency在consumer中必须成为external target而不是新的LocalConcrete body；只有依赖consumer type arguments的template节点才在consumer产生ODR实体。default template没有hidden closure能力；两者在wire type上必须不同。

`concretization predicate`不是自由文本或nullable flag，而是版本化封闭sum；M23至少包含既有`RequiresGcFreePointee`与`CLayoutFieldRequirement::CFieldSafeAndNonZst { signature_type, declaration_field_path }`。predicate引用binder位置和persistent type ref，consumer只能在完整替换后得到Success或带provider origin的typed diagnostic；unknown variant使reader拒绝artifact，不能被当成“无需检查”。

### 4.4 MIR与LIR metadata

MIR section提供下游MIR需要的、不从HIR name重建的关系：

- HIR persistent source id到external callable/global/constructor/accessor symbol的typed bridge；
- exact ancestry、virtual/itable slot identity、default implementation/adjust thunk与dispatch table schema；
- param-free concrete entity、已发射specialization及其ODR/linkage record；
- callback/coroutine/closure等可跨Cone引用的hidden ABI relation。

LIR section提供：

- param-free exact layout、field offsets/alignment、C ABI分类与recursive scan；
- TypeDescriptor/parent/interface/callable typed refs和symbol；
- exported exact generic application若已经materialize的layout/ODR record；
- external target完整calling convention、return convention、effect/root-plan类别；
- current image descriptor所需的root/immortal/init/type/safepoint摘要。
- `DefinitionVerificationSurface`：每个受检strong/ODR owner的canonical LIR/constant definition payload、`LirDefinitionFingerprint`、`ObjectDefinitionPlan`与`DigestFinalizationPlan`。payload使用与wire schema同样的typed id和canonical field tag，不能保存session arena顺序；最终ODR/registration/runtime-image digest不进入payload或LIR own-layer。

consumer新建的generic specialization由本次LocalConcrete HIR逐层生成本地MIR/LIR；只有引用既有上游param-free/materialized实体时才消费external meta。LIR lower不能因找到上游layout就跳过specialization key验证，也不能为同一个persistent type建立第二个non-ODR TypeDescriptor。

三层reader分别返回`ImportedHirSet`、`ImportedMirSet`、`ImportedLirSet`；不存在返回无类型map的通用`read_metadata`，也不存在Export template id到Concrete id、HIR callable到link symbol的unchecked cast。

### 4.5 decode验证与资源上限

`.slib`是编译输入，即使来自本地cache也不能使compiler panic或越界。reader至少验证：

- archive/member count、section byte size、CBOR nesting、string/source length、arena/table entry count和递归type/body深度上限；
- index范围、typed kind、identity hash、owner/decl/application关系、non-empty与exact-arity不变量；
- 每条external ref都能由manifest direct/transitive support graph定位到唯一origin artifact；
- public/inheritance/hidden/default用途不被非法互转，access witness引用完整；
- MIR/LIR bridge覆盖HIR宣称的每个external executable/layout需求且不存在额外同id矛盾记录；
- canonical LIR definition leaf、typed digest DAG/patch单writer、actual object leaf、ODR member/fingerprint、runtime type id、SafepointId、symbol/mangling重新计算一致；
- source path为normalized relative path，span/line offsets在声明的source长度范围内；
- target/layout/ABI与当前profile完全一致。

M23 v1的默认`SlibDecodeLimits`固定为：archive总长不超过2 GiB、member数不超过64、单个metadata section不超过256 MiB、单个object member不超过1 GiB、optional sources member不超过512 MiB、CBOR nesting不超过128、任一table不超过16,777,216项、单个text/bytes语义字段不超过16 MiB、type/body递归验证深度不超过1,024。所有长度先以checked `u64`运算，再验证可转换为host `usize`；不预分配声明值而未核对剩余bytes。v1没有压缩member，因此不存在解压后尺寸旁路。未来放宽limit可以是reader capability变化，但当前实现、错误和边界测试必须使用同一集中常量，不能各section散落不同默认值。

解析分“wire decode → structural validation → typed remap → semantic-world commit”四步；任何失败丢弃整个artifact，不留下半注册entity。错误包含artifact coordinate、section和field/index路径，但不把不受信任字符串当格式串或执行建议。

### 4.6 fingerprint与失效

- HIR fingerprint使用domain `scoop-hir-semantic-v1`，覆盖public/inheritance/template/default/const/re-export/prelude、declared source extern contracts及其typed closure；
- MIR fingerprint使用domain `scoop-mir-semantic-v1`，覆盖external symbol、selected extern contract bridge、dispatch、ancestry、ODR executable relation；
- LIR fingerprint使用domain `scoop-lir-semantic-v1`，覆盖target ABI、layout、scan、TypeDescriptor、call signature、完整target-native extern contract与不含最终digest的definition verification payload；
- code fingerprint由packager使用domain `scoop-code-v1`，依次覆盖按`object_members`顺序排列的每个最终object `{kind, canonical name, length, SHA-256(bytes)}`、canonical target-tagged native library link requirements及`CanonicalNativeExternalContractSet`。后者按`(symbol id, contract fingerprint)`排序并编码完整canonical key/payload，不编码诊断origin。object finalizer只能产出finalized `code.o`及其member hash，不能在尚未看到driver生成的`bridge.o`、最终requirements与contracts时计算composite code fingerprint。

前三项都是Merkle fingerprint：`layer_fingerprint = SHA-256(layer domain || canonical own-layer payload || 按{origin ConeIdentity, edge role}排序的support edge {origin identity, corresponding dependency layer_fingerprint})`。长度前缀与deterministic CBOR field tag沿4.1统一，重复edge按完整typed key去重。support edge至少覆盖re-export target、signature/inheritance closure、generic hidden/default target及MIR/LIR external bridge；不能只保存persistent id而漏掉其内容fingerprint。`CrossConeUseSet`给出精确边集合；保守实现可纳入全部direct dependency同层fingerprint。由此A re-export B时，即使A本地binding bytes不变，B的相关层变化仍改变A，再使只依赖A的C失效。

下游compile key依赖所消费dependency的三层Merkle fingerprint；最终link key另依赖完整transitive code/native-library/extern-contract fingerprint与全部`OdrDefinitionFingerprint`。optional full-source/producer timestamp等非语义数据不得使下游重编译；任何缓存命中都不能跳过artifact identity、Merkle edge、extern contract或ODR验证。

## 5. Pipeline、core分离与driver/linker

### 5.1 AST与parser

AST `SourceFile`新增完整header；文件的跨stage来源由外层typed input承载，不把host path塞进语法节点：

```text
SourceInput { source_identity: SourceIdentity, text: SourceText }
ParsedSource { source_identity: SourceIdentity, ast: SourceFile }

SourceFile {
    package: PackageSyntax,
    imports: Vec<ImportSyntax>,
    declarations: Vec<Decl>,
    span: Span,
}

ImportSyntax = Exact { public, path, alias }
             | Star  { public, path }
```

省略package使用显式`RootPackage` variant，不用空字符串。qualified path是非空identifier sequence，package/owner分界留给HIR基于typed namespace解析。parser按package、每条import、顶层declaration分别恢复；任一诊断时该file AST仍按既有规则整体丢弃。

### 5.2 HIR semantic world与resolver

driver把当前Cone所有AST、manifest-normalized input、direct dependency `ImportedHirSet`、transitive support artifact map及trusted core capability交给hir-lower。HIR先建立只读`SemanticWorld`：

- 每个provider有session-local `WorldConeId`和persistent `ConeIdentity`；
- public binding只从direct surfaces导入，hidden support只可沿已绑定template edge访问；
- current package、exact/star/prelude scope预先解析为typed binding groups；
- 同一origin经钻石路径只intern一次；
- 当前Cone声明和imported声明都投影成同一种`CallableView`/nominal/property view，来源不改变applicability。

输出仍严格为当前Cone的`ExportHir`与`LocalConcreteHir`。此外产生结构化`CrossConeUseSet`：`LookupObservationSet`保存HIR实际观察的完整候选/binding surface及负查询，`SelectedExternalSet`保存后续stage实际使用的external semantic id、materialized specialization和link requirement。M23 v1 compile key仍保守消费全部direct dependency HIR Merkle fingerprint；MIR/LIR只按selected set投影，不扫描所有dependency meta猜哪些需要导入。

所有**源码语义错误**必须在parser/HIR结束；manifest/DAG由driver、wire损坏/兼容由slib reader、ODR/final artifact冲突由link orchestration报告。MIR以后不补源码名称、visibility、generic inference或import错误。

### 5.3 stage输入边界

```text
parser:    SourceInput -> ParsedSource
hir-lower: CurrentConeParsedSources + ImportedHirSet -> ExportHir + LocalConcreteHir + CrossConeUseSet
mir-lower: LocalConcreteHir + SelectedImportedMir -> Mir + MirMeta
lir-lower: Mir + SelectedImportedLir + TargetProfile -> Lir + LirMeta
codegen:   CurrentConeLir(with ObjectDefinitionPlan) -> ProvisionalCodeObject + Optional<CBridgeSource>
driver:    CBridgeSource + TargetProfile -> VerifiedBridgeObject
object:    ProvisionalCodeObject + CurrentConeLir.ObjectDefinitionPlan
           -> FinalizedCodeObject + VerifiedObjectDefinitionIndex
slib:      manifest inputs + ExportHirMeta + MirMeta + LirMeta + FinalizedCodeObject
           + Optional<VerifiedBridgeObject> + CanonicalNativeLinkRequirements
           + CanonicalNativeExternalContractSet
           + VerifiedObjectDefinitionIndex -> .slib
```

- `.slib`读取、dependency graph与stage调度只在driver/slib crate；stage implementation crate不打开archive、manifest或上游source；
- imported meta类型定义在对应IR/meta crate。driver按`CrossConeUseSet`从完整reader结果投影出`SelectedImportedMir`/`SelectedImportedLir`；mir-lower只依赖HIR输入/MIR输出类型，lir-lower只依赖MIR输入/LIR输出类型；
- codegen仍只接收本Cone完整LIR，上游target是typed external ref；
- fingerprint finalization消费3.4已经验证的typed DAG。LIR metadata提供canonical ordinary source signature、exact type/target layout、`RefScan`和`CanonicalLirDefinition`；finalizer分别以`scoop-source-signature-v1`、`scoop-layout-v1`、`scoop-scan-v1`与`scoop-lir-definition-v1`计算四类semantic leaf，object verifier同时证明实际descriptor/scan bytes与这些语义值一致。`ProvisionalCodeObject`把**所有**`DigestNode.patch_sites`（包括semantic leaf落槽）置零，其他字段不得被finalizer改写；
- verifier按node的纯`normalize`视图计算object support、definition与stackmap leaf并回填其patch sites，未直接落槽的leaf保存在`VerifiedObjectDefinitionIndex`。随后以完整LIR/object/stackmap input计算ODR节点，以record kind/semantic id、6.1封闭static shape、referenced registration identity及声明的semantic/object/stackmap input计算strong registration节点；交叉record只按typed identity引用，除非对方digest就是当前record的显式字段；
- runtime-image节点最后从6.1的canonical `RuntimeImageRecordKey`序列、Cone/runtime/target/dependencies计算。每个node都只观察声明的传递依赖digest；own、peer、descendant与无关槽固定归零，因此同一个graph以任意合法topological traversal得到相同结果。回填完成后重跑graph、atom、EH、stackmap、layout/scan与patch-slot verifier，才产出不可修改的`FinalizedCodeObject`和该member的SHA-256；
- packager在finalized `code.o`、可选verified `bridge.o`、canonical native library link requirements及完整extern contract set全部就绪后计算4.6的composite `CodeFingerprint`，再计算manifest/member hash与`ArtifactFingerprint`；最终program link时才计算`GraphFingerprint`。artifact reader从LIR verification surface、object与manifest graph逐层重算到runtime image/code/artifact，而不是信任producer历史顺序。不能让codegen回写已经定稿的LIR meta，也不能在pack之后修改object；
- packager只消费各stage正式输出、finalized object与typed verifier index，不反向读取implementation crate内部arena；artifact reader不信任manifest range，仍从object重算；
- dump必须区分`local`/`external(origin@coordinate)`/`odr(group)`，但显示字符串不参与语义。

### 5.4 `scoop.core`独立编译

sysroot固定提供reserved core coordinate `scoop:scoop.core:0.1.0`与source/artifact slot。它的Cone name与源码package当前同为`scoop.core`只是发布约定，不建立通用推导。driver在特殊bootstrap模式编译core：

- core不隐式依赖自身；
- 只有来自trusted sysroot locator的该Cone获得`IntrinsicAuthority::Core`；
- core HIR export包含普通public surface、typed prelude binding表、全部well-known compiler/runtime core relation；
- 每个普通Cone把已验证core `.slib`作为direct dependency加载，不再把core source与用户AST拼进一个scope；
- core source/artifact版本、semantic fingerprints与compiler ABI不匹配时先重建trusted source，不能退回旧的“同单元编译”旁路；
- runtime需要的String TypeDescriptor等well-known地址由最终program descriptor的`ScoopRuntimeCoreBindings`传入，不再依赖硬编码`scoop_td_String`源码符号。

core prelude是其typed metadata的一部分，至少能表示package star与`Option` variant scope。用户Cone不能在manifest自定义全局prelude，也不能伪造core marker；普通library仍可由调用方显式/star import。

### 5.5 build、pack与link

`scoopc build <Cone-root-or-Cone.toml>`按DAG逐Cone执行：

1. 复用或构建direct dependency `.slib`；
2. parse整个当前Cone；
3. 读取三层import meta并完成pipeline；
4. codegen生成`code.o`与可选C bridge source，driver按同一target profile把后者编译为`bridge.o`，组成非空`object_members`，验证全部undefined relocation的typed requirement分类及`SourceExtern` contract关联并收集target-tagged native library link requirements及完整extern contract set；
5. 生成/验证image descriptor与ODR manifest；
6. 原子写临时`.slib`，完整round-trip验证后rename到cache/output；
7. library root到此结束；executable root继续最终link。

最终link按`ConeIdentity`去重完整transitive closure，比较全部ODR record，按3.7以native symbol id合并并逐字段核对全部extern contract，再按每个artifact的`object_members`提取全部object，并按target profile验证、稳定合并其传递native library link requirements；contract冲突必须在调用native linker前报告，上游object的native unresolved symbol不能因根Cone没有直接声明该FFI而漏掉依赖。driver生成一个小型target object，唯一导出`scoop_program_descriptor`，其中按canonical topological order引用每个Cone的image descriptor、root entry和core bindings。该object与runtime archive、全部Cone objects一起链接。

linker dead-strip不能丢失未被普通call graph引用但需要登记的image/global/init/callable metadata：program descriptor到每个image是强引用，image再强引用其storage/immortal/init/type/safepoint/callable六张表。`public`语言声明是否被调用不决定metadata存活。

最终artifact verifier至少检查：一个v1 program descriptor、一个no-throw root entry gateway、每个expected image恰好一次、无unexpected Scoop image、ODR winner唯一、TypeDescriptor双向地址identity唯一、runtime type/safepoint/body id到full key为一对一（同key重复已coalesce）、每个LIR `RegisteredCallableBody`恰有一个entry且不存在不同body id的共址、全部串接stackmap blob覆盖去重后的site全集，以及M25 EH依赖门禁仍成立。

## 6. Runtime多image登记与启动

### 6.1 descriptor ABI

M23把descriptor定为版本化的私有C ABI，而不是“实现自行选择字段”的概念接口。下面是v1的完整逻辑布局；`scoop_runtime_metadata_v1.h`由runtime与codegen共同包含/镜像，Darwin/AArch64 profile固定64-bit pointer、little-endian、natural 8-byte struct alignment。codegen按LLVM `DataLayout`发射同一字段顺序，并以object verifier和C `_Static_assert(sizeof/offsetof)`逐字段比对：

```c
typedef struct {
    uint64_t magic;
    uint32_t abi_version;
    uint32_t struct_size;
} ScoopDescriptorPrefixV1;

typedef struct { uint8_t bytes[32]; } ScoopDigest256V1;
typedef struct { const uint8_t *data; uint64_t length; } ScoopByteSpanV1;

/* SHA-256(ByteSpan("scoop-runtime-core-capability-v1") || ByteSpan("String")) */
static const uint8_t SCOOP_CORE_STRING_CAPABILITY_ID_V1[32] = {
    0xd6, 0x96, 0x47, 0x67, 0x50, 0x41, 0xab, 0x4e,
    0x7a, 0x25, 0x50, 0xb9, 0xa5, 0xa2, 0x7b, 0x49,
    0x74, 0x1c, 0x08, 0xb3, 0x0a, 0xb0, 0x7b, 0x25,
    0x9d, 0x99, 0x8e, 0x60, 0x6f, 0x21, 0x8b, 0x77
};

typedef struct {
    ScoopByteSpanV1 group;
    ScoopByteSpanV1 name;
    ScoopByteSpanV1 version;
    ScoopDigest256V1 identity;
} ScoopConeRecordV1;

typedef struct ScoopTypeDescriptor ScoopTypeDescriptor;
typedef struct ScoopInitializationCell {
    uint64_t state;       /* 初始0 = Uninitialized；只由coordinator同步访问 */
    void *owner_thread;   /* 初始null；raw runtime thread identity */
} ScoopInitializationCell; /* v1在当前profile固定sizeof=16、alignof=8 */
typedef struct {
    uint32_t instance_kind;       /* 1=FixedObject, 2=BoxedValue, 3=InlineBytes,
                                     4=InlineArray, 5=AbstractRef */
    uint32_t inline_storage_kind; /* 0 = None, 1 = Inline, 2 = ZeroSized */
    uint64_t minimum_size;        /* fixed/box的exact allocation；variable的data offset */
    uint64_t instance_alignment;
    uint64_t inline_offset;       /* box payload或variable inline data起点 */
    uint64_t inline_size;         /* value payload/element logical size；ZST为0 */
    uint64_t inline_stride;       /* InlineBytes为1；InlineArray/Inline非零；其余为0 */
    uint64_t inline_alignment;    /* None为0；否则非零 */
    const uint64_t *inline_scan;  /* 相对payload/element起点；空scan为null */
} ScoopTypeInstanceShapeV1;

typedef struct {
    const ScoopTypeDescriptor *interface;
    const void *const *slots;
} ScoopItableEntryV1;

struct ScoopTypeDescriptor {
    uint64_t type_id;
    ScoopTypeInstanceShapeV1 instance_shape;
    const uint64_t *object_scan;    /* 始终相对managed object start；可为null */
    const ScoopTypeDescriptor *parent;
    const void *const *vtable;
    const ScoopItableEntryV1 *itables;
    uint64_t itable_count;
    ScoopByteSpanV1 diagnostic_name;/* canonical exact-type UTF-8，仅诊断 */
};

typedef struct {
    uint32_t linkage_kind;       /* 1 = strong, 2 = ODR */
    uint32_t reserved_zero;
    ScoopDigest256V1 semantic_id;/* kind由外层record type封闭 */
    ScoopDigest256V1 odr_group_id;
    ScoopDigest256V1 odr_member_id;
    ScoopDigest256V1 definition_fingerprint;
} ScoopRegistrationIdentityV1;

typedef struct ScoopImmortalObjectDescriptorV1 ScoopImmortalObjectDescriptorV1;
typedef struct ScoopStaticImmortalRelocationV1 {
    uint64_t pointer_offset;
    const ScoopImmortalObjectDescriptorV1 *target;
} ScoopStaticImmortalRelocationV1;

typedef struct ScoopStaticStorageDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint32_t scan_kind;          /* 0 = None, 1 = Recursive */
    uint32_t initial_state_kind; /* 1 = ZeroedForRuntimeUnit,
                                    2 = EncodedStaticValue */
    void *writable_base;
    uint64_t byte_size;            /* 语言值的logical payload size，可为0 */
    uint64_t allocation_extent;    /* 恰为max(byte_size, 1) */
    uint64_t required_alignment;
    const uint64_t *scan_program;
    ScoopDigest256V1 scan_fingerprint;
    ScoopDigest256V1 layout_fingerprint;
    ScoopByteSpanV1 initial_template;
    const ScoopStaticImmortalRelocationV1 *initial_relocations;
    uint64_t initial_relocation_count;
} ScoopStaticStorageDescriptorV1;

typedef struct ScoopTypeRegistrationDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint64_t runtime_type_id;
    uint64_t reserved_zero;
    const ScoopTypeDescriptor *descriptor;
    ScoopDigest256V1 descriptor_fingerprint;
    ScoopDigest256V1 layout_fingerprint;
} ScoopTypeRegistrationDescriptorV1;

typedef void (*ScoopCallableAddressV1)(void); /* 只作address identity，不经此类型调用 */
typedef struct ScoopCallableRegistrationDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration; /* semantic id = PersistentCallableBodyId */
    ScoopDigest256V1 body_definition_fingerprint;
    ScoopCallableAddressV1 entry;
} ScoopCallableRegistrationDescriptorV1;

struct ScoopImmortalObjectDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    const void *object_start;
    uint64_t object_size;
    uint64_t required_alignment;
    const ScoopTypeRegistrationDescriptorV1 *type_registration;
};

typedef void (*ScoopManagedUnitEntryFnV1)(void);
typedef struct ScoopInitializationUnitDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint32_t schedule_kind;      /* 1 = EagerStartup, 2 = LazyAccess */
    uint32_t reserved_zero;
    ScoopByteSpanV1 diagnostic_path;
    ScoopInitializationCell *cell;
    const ScoopStaticStorageDescriptorV1 *storage;
    const ScoopStaticStorageDescriptorV1 *failure_root;
    ScoopDigest256V1 initializer_callable_id; /* PersistentCallableBodyId */
    ScoopDigest256V1 ensure_callable_id;      /* PersistentCallableBodyId */
    ScoopManagedUnitEntryFnV1 initializer_entry;
    ScoopManagedUnitEntryFnV1 ensure_entry;
    ScoopDigest256V1 startup_gateway_callable_id; /* Eager nonzero；Lazy zero */
    ScoopDigest256V1 startup_gateway_definition_fingerprint;
    uint32_t (*startup_gateway)(void); /* Eager非null；Lazy为null */
} ScoopInitializationUnitDescriptorV1;

typedef struct ScoopSafepointRegistrationDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopRegistrationIdentityV1 registration;
    uint64_t safepoint_id;
    uint32_t site_role;          /* 1=ManagedPoll, 2=ManagedCall, 3=ManagedInvoke,
                                    4=NativeSafeTransition, 5=NativeBorrowedTransition */
    uint32_t root_pair_count;
    ScoopDigest256V1 owner_callable_id; /* PersistentCallableBodyId */
    ScoopDigest256V1 normalized_stackmap_fingerprint;
} ScoopSafepointRegistrationDescriptorV1;

typedef uint32_t (*ScoopRootEntryGatewayFnV1)(void); /* 0 = Succeeded, 1 = Failed */
typedef struct ScoopRootEntryDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopDigest256V1 owner_cone_identity;
    ScoopDigest256V1 callable_id; /* main的PersistentCallableBodyId */
    ScoopDigest256V1 source_signature_fingerprint; /* ordinary () -> Unit */
    ScoopDigest256V1 gateway_callable_id; /* PersistentCallableBodyId */
    ScoopDigest256V1 gateway_definition_fingerprint;
    const ScoopStaticStorageDescriptorV1 *failure_root;
    ScoopRootEntryGatewayFnV1 gateway;
} ScoopRootEntryDescriptorV1;

typedef struct ScoopImageDescriptorV1 ScoopImageDescriptorV1;
typedef struct ScoopRuntimeCoreBindingsV1 {
    ScoopDescriptorPrefixV1 prefix;
    const ScoopImageDescriptorV1 *core_image;
    ScoopDigest256V1 string_type_id;
    const ScoopTypeRegistrationDescriptorV1 *string_type;
    uint64_t object_header_size;
    uint64_t string_length_offset;
    uint64_t string_bytes_offset;
    uint64_t string_minimum_size;
    uint64_t string_alignment;
    ScoopDigest256V1 string_scan_fingerprint;
} ScoopRuntimeCoreBindingsV1;

struct ScoopImageDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopConeRecordV1 cone;
    ScoopDigest256V1 runtime_image_fingerprint;
    const ScoopDigest256V1 *dependencies;
    uint64_t dependency_count;
    const ScoopStaticStorageDescriptorV1 *const *static_storages;
    uint64_t static_storage_count;
    const ScoopImmortalObjectDescriptorV1 *const *immortal_objects;
    uint64_t immortal_object_count;
    const ScoopInitializationUnitDescriptorV1 *const *initialization_units;
    uint64_t initialization_unit_count;
    const ScoopTypeRegistrationDescriptorV1 *const *type_registrations;
    uint64_t type_registration_count;
    const ScoopSafepointRegistrationDescriptorV1 *const *safepoints;
    uint64_t safepoint_count;
    const ScoopCallableRegistrationDescriptorV1 *const *callables;
    uint64_t callable_count;
};

typedef struct ScoopProgramDescriptorV1 {
    ScoopDescriptorPrefixV1 prefix;
    ScoopDigest256V1 runtime_abi;
    ScoopDigest256V1 target_profile;
    ScoopDigest256V1 graph_fingerprint;
    const ScoopImageDescriptorV1 *root_image;
    const ScoopRootEntryDescriptorV1 *entry;
    const ScoopRuntimeCoreBindingsV1 *core;
    const ScoopImageDescriptorV1 *const *images;
    uint64_t image_count;
} ScoopProgramDescriptorV1;

extern const ScoopProgramDescriptorV1 scoop_program_descriptor;
```

每个有prefix的record使用不同magic：program `0x53434f4f50505247`、image `0x53434f4f50494d47`、entry `0x53434f4f50454e54`、core `0x53434f4f50434f52`、storage `0x53434f4f5053544f`、immortal `0x53434f4f50494d4d`、init `0x53434f4f50494e49`、type `0x53434f4f50545950`、safepoint `0x53434f4f50535054`、callable `0x53434f4f5043414c`；`abi_version == 1`且v1的`struct_size`必须**恰好**等于共享header中的`sizeof`，不是“至少这么大”。所有reserved字段为零。runtime先只读固定16-byte prefix，在magic/version/size通过后才解释余下字段。每个pointer/count pair的pointer在count为0时仍指向对应类型的只读addressable sentinel；非零span须做count上限、乘加溢出、alignment和已加载segment权限验证。digest总是inline 32 bytes；文本总是non-null byte span而非NUL字符串。

`ValueStorageLayout`描述未装箱值，`ScoopTypeInstanceShapeV1`只描述带对象头的managed instance；两者不能再共用一个含糊的`size/align/scan`。LIR内部先使用下列封闭和类型，所有variant只能经private checked constructor产生；C product只是codegen/runtime边界上的机械编码，reader必须完整验证后解码回该sum：

```text
TypeInstanceShape =
    FixedObject { allocation_size: NonZeroU64,
                  instance_alignment: NonZeroPow2,
                  object_scan: RefScan }
  | BoxedValue { payload_offset: NonZeroU64,
                 allocation_size: NonZeroU64,
                 instance_alignment: NonZeroPow2,
                 value: ValueStorageLayout }
  | InlineBytes
  | InlineArray { data_offset: NonZeroU64,
                  instance_alignment: NonZeroPow2,
                  element: ArrayElementStorage }
  | AbstractRef

AllocatableTypeDescriptorRef = FixedObjectDescriptorRef
                             | BoxedValueDescriptorRef
                             | InlineBytesDescriptorRef
                             | InlineArrayDescriptorRef
```

每种分配LIR进一步要求精确refined descriptor ref（class/closure、box、String、array分别对应上式四个variant）；通用allocation helper最多接受`AllocatableTypeDescriptorRef`，`AbstractRef`只能用于RTTI/subtyping关系。runtime仍防御性拒绝伪造的AbstractRef分配。`object_scan`始终以managed object start为基址，`inline_scan`始终以box payload或单个array element起点为基址；BoxedValue与InlineArray的object scan由constructor从value/element scan机械派生，不能平行传入另一份。五种shape编码到C字段后的合法矩阵为：

- `FixedObject`用于普通class/closure等固定对象：`minimum_size`是含16-byte header及尾padding的exact normalized allocation size，`instance_alignment >= 8`；全部`inline_*`为0/null，`object_scan`直接描述对象字段；
- `BoxedValue`用于每个exact value type：`inline_offset = alignUp(16, inline_alignment)`、`instance_alignment = max(8, inline_alignment)`、`minimum_size = alignUp(inline_offset + inline_size, instance_alignment)`，`inline_stride == 0`。非ZST使用`Inline`、`inline_size > 0`及相对payload的value scan；ZST使用`ZeroSized`、`inline_size == 0`、两个scan均null。若value scan非空，`object_scan = translate(inline_scan, inline_offset)`：References的每个offset相加、Sequence递归平移child、Array的length/first offset相加而element child仍相对单个element；全部checked并重新canonicalize。为空则`object_scan == null`；
- `InlineBytes`只用于String：`minimum_size == inline_offset == 24`、instance alignment 8、`Inline`的size/stride/alignment均为1，两个scan均null；
- `InlineArray`用于`Array`/`MutableArray`：`minimum_size == inline_offset == alignUp(24, inline_alignment)`，instance alignment为`max(8, inline_alignment)`。`Inline`要求`inline_size == inline_stride > 0`；`ZeroSized`要求size/stride为0。只有element `inline_scan`非空时，`object_scan`才是`Array { length_offset: 16, first_element_offset: inline_offset, stride: inline_stride, element: inline_scan }`，GC-free/ZST element时二者均null；
- `AbstractRef`用于interface、managed function type、`Any`/`Nothing`等不可直接分配的exact reference type：minimum/alignment/全部inline字段与两个scan均为0/null；任何allocation入口看到该shape都fatal。

`minimum_size == 0`因此只合法于`AbstractRef`；ZST的“零”存在于`ValueStorageLayout::ZeroSized`与`BoxedValue.inline_size == 0`，其box仍有非零minimum allocation。registration的descriptor/layout fingerprint覆盖整个shape、两个scan及按typed role规范化的pointer内容；reader、link verifier与runtime拒绝其他组合。target profile还给出非零`maximum_managed_alignment`与`maximum_managed_object_size`，任何fixed/box/array layout超过alignment上限都在LIR layout阶段失败，任意allocation超过object-size上限都失败；allocator不能静默降级、wrap或截断。

每个TypeDescriptor的canonical LIR definition非可选地内嵌该exact type重算后的`CanonicalExactTypeDiagnosticName` byte span；其`ObjectDefinition` node把这份LIR semantic leaf作为直接输入，因而`descriptor_fingerprint`承诺name的length与bytes，而不只承诺一个relocation target。实体中的span必须指向内容完全相同的只读diagnostic atom；该atom对ODR type是同组显式member并以自身object fingerprint进入`OdrDefinitionFingerprint`，对strong type则是descriptor definition plan覆盖的typed associated atom。codegen不得用当前import alias、source pretty-printer或consumer-local string替换这些bytes。

共享header同时冻结scan program v1：null只表示empty scan；普通固定偏移节点编码`[count, offset...]`且`0 < count < UINT64_MAX-1`；sequence编码`[UINT64_MAX-1, child_count, child_ptr...]`且`child_count >= 2`；array编码`[UINT64_MAX, length_offset, first_element_offset, nonzero_stride, nonempty_element_scan_ptr]`。canonical `RefScan` normal form要求References offset严格递增且无重复；Sequence不含None或直接Sequence child，先把同层References合并成一个offset并集，再将**包含该合并References节点在内**的全部已规范化child统一按`(child ScanFingerprint, canonical child bytes)`严格递增、去重，零/单child分别折叠为None/该child；Array element必须是已规范化NonEmptyRefScan。producer在fingerprint和发射前规范化到fixed point，reader/runtime拒绝任何非canonical tree。所有offset/stride/count做checked乘加并验证alignment/object allocation range，且每个top-level scan固定受`SCOOP_SCAN_V1_MAX_DEPTH=64`（root为1）、`SCOOP_SCAN_V1_MAX_NODES=65536`、`SCOOP_SCAN_V1_MAX_WORDS=1048576`、`SCOOP_SCAN_V1_MAX_EXPANDED_NODES=1048576`与`SCOOP_SCAN_V1_MAX_CANONICAL_BYTES=16777216`限制；node/word预算对物理共享child按地址只计一次，depth按active path计数，expanded-node与canonical-byte预算则按展开每条parent→child路径重计。validator以memoized checked subtree node/byte cost在继续展开/比较/编码前拒绝超限，active recursion stack仍拒绝cycle，因此共享DAG不能制造指数或重复大leaf的二次工作；常量全部进入runtime ABI fingerprint。

scan的canonical typed bytes使用本节下述scalar/count规则且没有pointer：`None = u32(0)`；`References = u32(1) || u64(count) || each u64(offset)`；`Sequence = u32(2) || u64(child_count) || each canonical child`；`Array = u32(3) || u64(length_offset) || u64(first_element_offset) || u64(stride) || canonical element`。`ScanFingerprint = SHA-256(ByteSpan("scoop-scan-v1") || canonical typed bytes)`；共享物理child按semantic tree路径重复编码，但expanded预算保证工作有界。static storage的`scan_kind == None`使用指向canonical `[0]`的非null sentinel以维持record字段契约；该sentinel不是合法Recursive root，TypeDescriptor中的empty `object_scan/inline_scan`则按上述矩阵使用null。

variable instance的exact normalized allocation size也属于ABI：`InlineBytes(len) = alignUp(24 + checked len, 8)`；`InlineArray::Inline(count) = alignUp(inline_offset + checked count * inline_stride, instance_alignment)`；`InlineArray::ZeroSized(count) = minimum_size`。M23没有接收任意signed length的public array constructor：literal/assembly/vararg的logical count由非负component count经checked加法得到且位于`0..=INT64_MAX`；clone/互转则先验证source exact array TD、side-metadata allocation与`0 <= source.size <= INT64_MAX`一致，再读取该size作为target count并使用target refined InlineArray TD分配，不能把它伪装成component求和。任一内部入口看到负count都是compiler/runtime invariant failure，不按名称构造源码异常。String length、乘法、加法与`alignUp`全部使用checked `u64`并要求结果可转成target `size_t`且不超过target profile的`maximum_managed_object_size`；算术溢出、对象过大或底层资源耗尽沿用当前fatal allocation failure，不得wrap、截断或把ZST length当0。未来若增加接收signed length的普通core API，其源码可见异常由该API自行声明/实现。collector读取array count时还用side metadata的exact size反证`first_element_offset + count * stride`位于对象内；损坏对象/descriptor是fatal runtime invariant error。

`ScoopRegistrationIdentityV1.semantic_id`由外层record赋予kind-specific类型：分别是`PersistentStaticStorageId`、`PersistentImmortalObjectId`、`PersistentInitializationUnitId`、`PersistentExactTypeId`、`PersistentSafepointSiteId`和`PersistentCallableBodyId`。strong record的两个ODR字段必须全零；ODR record的group/member必须非零且与manifest重算结果一致。`definition_fingerprint`对strong record是domain-separated registration definition fingerprint，对ODR record就是所属group的`OdrDefinitionFingerprint`；二者遵循5.3的分层DAG，不能在计算上游object leaf时把已经完成的leaf再次归零。callable record的独立`body_definition_fingerprint`则始终是该body atom的`ObjectDefinitionFingerprint`，不能与registration definition混用。`scan_kind == None`的storage仍有descriptor并指向canonical空scan sentinel，使init storage总能被typed引用，但GC只把`Recursive`记录加入root集合。

`byte_size`是语言值的logical payload size；`allocation_extent`必须恰为`max(byte_size, 1)`，range/segment验证使用后者，layout/scan和读写语义只使用前者。每个storage都发射独立、address-significant且不可合并的writable atom；`byte_size == 0`时底层仍发射一个1-byte identity token，要求`scan_kind == None`、空scan program，并禁止`unnamed_addr`、common/tentative symbol、constant merge或ICF。strong token按storage persistent id唯一命名，ODR token是所属group的显式member并按同一member symbol coalesce；除已验证的同一ODR identity外，两个storage的`[writable_base, writable_base + allocation_extent)`不得重叠。这样`Unit`、空struct/tuple或其他零尺寸top-level/init storage仍有非null、全程序唯一且可验证的地址，而不会把identity token误当成一字节语言值。

compiler-owned static storage的初值不是任意LLVM constant，而是LIR中的封闭sum：

```text
StaticInitialState =
    ZeroedForRuntimeUnit
  | EncodedStaticValue {
        template: CanonicalAllocationExtentBytes,
        relocations: Sorted<StaticImmortalRelocation>
    }

StaticImmortalRelocation = {
    pointer_offset: u64,
    target: ImmortalObjectRegistrationRef
}
```

`ZeroedForRuntimeUnit`只用于需要runtime unit写入的ordinary property/delegate storage、singleton published slot，以及initialization/root-entry failure storage；即使其最终值GC-free，完整`allocation_extent`在任一managed代码前也必须逐byte为零。该分支的C编码为`initial_state_kind == 1`、长度为0但data指向typed read-only sentinel的`initial_template`，以及count为0但pointer指向typed sentinel的`initial_relocations`。initializer只在成功发布路径写入logical value；failure/published协议按各自状态机写入。不能用undef、未初始化BSS假设或“第一次GC前肯定会写”代替该契约。

`EncodedStaticValue`恰用于spec 9.1.3允许**省略 initialization unit**的directly encodable值；即使其最终bits全零也不能退化成前一variant。其C编码为`initial_state_kind == 2`，`initial_template.length == allocation_extent`，template逐byte给出target representation，所有padding及ZST identity-token byte canonical为零，且Recursive scan展开出的每个managed-pointer leaf在template中也必须是全零8-byte carrier。relocation sequence按`pointer_offset`严格递增且无重复；每项offset必须8-byte对齐、完整落在logical `byte_size`内并恰好命中一个Recursive leaf，target必须解析为本程序已登记的typed immortal-object record。`scan_kind == None`要求relocation count为0；`scan_kind == Recursive`要求实际storage中每个非null leaf与恰好一项relocation一一对应、其bits逐bit等于该target的`object_start`，其余leaf仍为null。runtime在commit registry前先建立全部immortal provisional map，再逐byte比较storage：非leaf bytes必须等于template，leaf template必须为零，relocation以外不得出现任何managed pointer，尤其不能接受任意heap ref或immortal interior pointer。无unit的`Option.None`因此使用无relocation的Encoded variant，而immortal String ref使用指向其object-start的单项typed relocation。

`initial_template`与`initial_relocations`本身位于只读segment；Encoded分支的writable storage atom只允许在上述pointer leaf拥有exact-width、zero-addend的`NamedPersistentSymbol { kind: ImmortalObject, persistent id: target }` relocation，relocation record中的`target` pointer同时必须指向同一target registration。`CanonicalLirDefinition`包含完整`StaticInitialState`，`ObjectDefinitionPlan`覆盖writable atom、必有的`StaticInitialTemplate { storage }` atom、非空时的`StaticInitialRelocationTable { storage }` atom及这两类relocation；空relocation sequence使用共享typed sentinel而不虚构零尺寸member。object/link verifier从真实bytes与relocation table重建而不信任C record。对ODR storage，这两个非空辅助atom也是同组显式member；任一producer在variant、template byte、offset、target id或actual relocation上不同都会改变definition fingerprint并在native coalesce前失败。strong registration同样由下述完整canonical record承诺这些字段。显式raw `@Global`/`@ThreadLocal`及只读immortal object使用各自表示，不能借用这两个variant绕过对应规则。

`RuntimeImageFingerprint`的canonical input不是raw C struct bytes。先定义固定字段顺序的封闭sum：

```text
CanonicalRegistrationIdentity = {
    linkage_kind, semantic_id,
    odr_group_id_or_zero, odr_member_id_or_zero,
    definition_fingerprint
}

RuntimeImageRecordKey =
    StaticStorage {
        registration, scan_kind,
        base = OwnStorageAtom(registration),
        byte_size, allocation_extent, required_alignment,
        scan_program = EmptyScan | ScanProgram(scan_fingerprint),
        scan_fingerprint, layout_fingerprint,
        initial_state = ZeroedForRuntimeUnit
                      | EncodedStaticValue {
                            template_bytes: ByteSpan,
                            relocations: Sequence<{
                                pointer_offset: u64,
                                target: PersistentImmortalObjectId
                            }>
                        }
    }
  | ImmortalObject {
        registration,
        object_start = OwnImmortalAtom(registration),
        object_size, required_alignment,
        type_registration = PersistentExactTypeId
    }
  | InitializationUnit {
        registration, schedule_kind, diagnostic_path_bytes,
        cell = OwnInitializationCell(registration),
        storage = PersistentStaticStorageId,
        failure_root = PersistentStaticStorageId,
        initializer_callable_id,
        initializer_entry = CallableEntry(initializer_callable_id),
        ensure_callable_id,
        ensure_entry = CallableEntry(ensure_callable_id),
        startup_gateway = None
                        | UnitGateway(startup_gateway_callable_id,
                                      startup_gateway_definition_fingerprint)
    }
  | TypeRegistration {
        registration, runtime_type_id,
        descriptor = TypeDescriptorAtom(PersistentExactTypeId),
        descriptor_fingerprint, layout_fingerprint
    }
  | SafepointRegistration {
        registration, safepoint_id, site_role, root_pair_count,
        owner_callable_id, normalized_stackmap_fingerprint
    }
  | CallableRegistration {
        registration,
        body_definition_fingerprint,
        entry = OwnCallableEntry(registration.semantic_id)
    }
```

canonical encoder v1逐byte固定如下：`u32`/`u64`均为little-endian；persistent id与digest恰写32原字节；byte span写`u64 length`后接原字节；sequence写`u64 count`后接各element；product无padding并按上述声明顺序；sum先写`u32 tag`再写该variant payload。`RuntimeImageRecordKey`的这个最外层sum tag就是record kind的唯一编码；`CanonicalRegistrationIdentity`从`linkage_kind`开始，绝不再写kind。domain tag同样作为byte span编码，不带NUL。所有长度/count先受限再转换，unknown/reserved tag一律拒绝。C ABI与hash-only discriminant统一冻结为：

| sum/enum | v1 tag |
| --- | --- |
| record kind | `StaticStorage=1, ImmortalObject=2, InitializationUnit=3, TypeRegistration=4, SafepointRegistration=5, CallableRegistration=6` |
| linkage | `Strong=1, Odr=2` |
| storage scan | `None=0, Recursive=1` |
| static initial state | `ZeroedForRuntimeUnit=1, EncodedStaticValue=2` |
| init schedule | `EagerStartup=1, LazyAccess=2` |
| scan-program choice | `EmptyScan=0, ScanProgram=1` |
| startup-gateway choice | `None=0, UnitGateway=1` |
| typed address/entry role | `OwnStorageAtom=1, OwnImmortalAtom=2, OwnInitializationCell=3, CallableEntry=4, UnitGateway=5, TypeDescriptorAtom=6, RootGateway=7, OwnCallableEntry=8` |
| safepoint site role | `ManagedPoll=1, ManagedCall=2, ManagedInvoke=3, NativeSafeTransition=4, NativeBorrowedTransition=5` |

每个table分别按`(semantic_id, linkage_kind, odr_group_id, odr_member_id)`的byte序严格递增；image stream按record-kind tag顺序写六个**各自带count**的table sequence，不能把空table省略。direct dependency identity按digest bytes严格递增、去重并带count。raw pointer永不进入key：own atom/cell/callable entry只编码对应role tag且owner是当前registration；scan program编码choice与scan fingerprint；Encoded初值直接编码恰为`allocation_extent`的template byte span及按offset排序的`{pointer_offset, target immortal semantic id}`，不编码template/relocation-array地址、target record地址或storage中ASLR后的pointer bits；跨record pointer必须先解析到目标kind-specific registration id；`CallableEntry`、`UnitGateway`、`TypeDescriptorAtom`与`RootGateway`编码role tag后再按伪代码写其typed id/fingerprint payload，其中所有callable/gateway owner均为已重算的`PersistentCallableBodyId`。link verifier还要用object relocation证明每个pointer确实指向该typed symbol；runtime以callable registration的双向map重验每个entry/gateway/stackmap function address、producer image与body id，不能只检查“落在某段可执行内存”。TypeDescriptor内部parent/itable/callable/scan pointer的完整语义已由`descriptor_fingerprint`承诺；runtime按shape与已登记type relation作防御性验证，不尝试从ASLR地址反推symbol名。

`normalized_stackmap_fingerprint`也使用同一scalar/count encoder，不能hash raw LLVM section slice。每条已匹配registration的record先转成下列无地址schema：

```text
CanonicalStackmapRecordV1 = {
    format_version: u32 = 3,
    site_id: PersistentSafepointSiteId,
    safepoint_id: u64,
    owner_callable_id: PersistentCallableBodyId,
    site_role: u32,
    root_pair_count: u32,
    instruction_offset: u32,
    stack_size: u64,
    locations: Sequence<CanonicalStackmapLocationV1>,
    live_outs: Sequence<CanonicalStackmapLiveOutV1>
}

CanonicalStackmapLocationV1 =
    Register { size: u32, dwarf_register: u32 }                 // tag 1
  | Direct   { size: u32, dwarf_register: u32,
               signed_offset_bits: u64 }                       // tag 2
  | Indirect { size: u32, dwarf_register: u32,
               signed_offset_bits: u64 }                       // tag 3
  | Constant { size: u32, value_bits: u64 }                    // tag 4

CanonicalStackmapLiveOutV1 = { dwarf_register: u32, size: u32 }

StackmapRecordFingerprint =
    SHA-256(ByteSpan("scoop-stackmap-record-v1") ||
            canonical(CanonicalStackmapRecordV1))
```

raw LLVM v3 location顺序原样保留，包括前三个statepoint header location及之后每对base/derived root；live-out也按raw顺序保留并各自带count。raw `Constant`的signed i32 small value先符号扩展为64-bit two's-complement `value_bits`，`ConstantIndex`必须是非负且在pool内，再以pool中的原始u64 bits编码成同一个canonical `Constant` tag；constant-pool index、pool顺序及未引用entry不进入hash。Register要求raw offset为0，Constant/ConstantIndex要求raw DWARF register为0；Direct/Indirect的i32 offset先符号扩展并以其i64 two's-complement u64 bits编码。所有raw reserved、record flags和alignment padding必须为0，unknown kind/越界constant index直接拒绝，不产生canonical record。

function table的ASLR地址由匹配`safepoint_id`的registration解析到其callable record，并必须逐bit等于该record的entry；它与最终`return_pc = function_address + instruction_offset`只用于segment、唯一性和运行期root lookup，二者都不进入fingerprint。`instruction_offset`和`stack_size`仍进入fingerprint。Darwin/AArch64 profile另验证location总数精确为`3 + 2 * root_pair_count`、前三项为8-byte constant且statepoint flags/deopt count为0、每对root为相同的8-byte SP/FP `Indirect`可写slot；registration中的site/body/role/root count与canonical字段必须逐项相等。Rust object/final verifier与C runtime共享Constant/ConstantIndex等价、pool重排/ASLR不变，以及逐字段篡改会改变digest的golden vectors。

RuntimeImage hash stream依次为domain byte span `scoop-runtime-image-v1`、runtime ABI digest、target profile digest、`ConeRecord { group byte span, name byte span, version byte span, identity }`、direct-dependency sequence及六个record table sequence；`RuntimeImageFingerprint = SHA-256(stream)`。它不是`ArtifactFingerprint`；其自身slot不是输入，且绝不输入`code.o` bytes、ASLR地址、program-level core binding或该digest回填后的bytes。`DigestFinalizationPlan`要求runtime-image node显式依赖record key中实际出现的definition/layout/scan/descriptor/gateway/callable/stackmap节点；object finalizer计算并回填，manifest保存同值，link verifier核对两份，runtime再从链接record和program ABI输入重算。

最终program另使用两个封闭key，不能把entry/core pointer写进hash：

```text
RootEntryKey = {
    owner_cone_identity, callable_id, source_signature_fingerprint,
    gateway_callable_id, gateway_definition_fingerprint,
    failure_root = PersistentStaticStorageId,
    gateway = RootGateway(gateway_callable_id)
}

RuntimeCoreBindingsKey = {
    core_image = ConeIdentity,
    string_capability_id = SCOOP_CORE_STRING_CAPABILITY_ID_V1,
    string_type_id,
    string_type_registration = PersistentExactTypeId,
    object_header_size, string_length_offset, string_bytes_offset,
    string_minimum_size, string_alignment, string_scan_fingerprint
}
```

link verifier与runtime都先把entry的`failure_root`/`gateway`及core的`core_image`/`string_type`四个pointer解析到对应kind-specific id，并验证gateway body、producer与core image的owner关系，再编码key。final-program builder把已经验证的core String `Scan` digest逐byte复制到`RuntimeCoreBindingsV1.string_scan_fingerprint`，这不是新的digest node或新的hash domain。Graph hash stream按同一encoder依次为domain byte span `scoop-program-graph-v1`、runtime ABI、target profile、root Cone identity、按伪代码声明序的`RootEntryKey`、`RuntimeCoreBindingsKey`，最后是带count的canonical-topological `{ConeIdentity, RuntimeImageFingerprint}` product sequence；其SHA-256即`GraphFingerprint`。它不属于任何Cone的`DigestFinalizationPlan`，由final-program builder在program slot为全零时唯一计算/写入，再由link verifier/runtime重算。只排除program record中的`graph_fingerprint`自身，不排除entry/core内已经完成的上游digest。Rust producer/finalizer/link verifier与C runtime共享至少包含空/全variant/边界长度的逐byteencoder与digest golden vectors；code变化由`CodeFingerprint`、`ArtifactFingerprint`与最终link key覆盖，不与runtime image fingerprint或program graph建立自引用。

每个Cone image descriptor symbol由其`ConeIdentity`mangle且为hidden strong symbol；相同Cone在钻石图只允许一个artifact instance。image必须携带完整canonical coordinate，runtime重新验证第1.1节grammar、重算`ConeIdentity`，并以coordinate bytes验证同层排序；coordinate因此是ABI验证/排序输入，不只是诊断字符串。generic ODR member不产生额外伪Cone image。

六类metadata table使用**指向独立descriptor record的pointer span**，而不是把record按值嵌入每个image。一个image只列出由自己实际发射的strong producer或ODR producer record；普通external reference不重复登记。两个consumer各自发射同一generic specialization时，两边表项经link relocation必须指向已coalesce的同一ODR record。runtime分别建立kind-specific `semantic id -> record address`及反向map；callable table还建立`PersistentCallableBodyId ↔ entry address`双向map，要求entry非null、位于可执行segment且同一entry不能属于不同body。所有registered callable atom都是address-significant：不得带`unnamed_addr`，codegen禁用跨不同body id的MergeFunctions/function alias folding，M23 final-link profile拒绝function ICF；只有已经验证为同一ODR callable member的winner可以共址。strong id只允许一个producer；ODR重复只在record地址、group/member、registration/body definition fingerprint及全部关键storage/cell/entry/TD地址相同后去重。每个type registration还必须满足非零`runtime_type_id == descriptor->type_id`。不同persistent exact type id共享一个TypeDescriptor地址、同一type id出现两个地址、或body id/entry不是一一对应都必须fatal，不能依赖constant merge/ICF碰运气。每个LIR `RegisteredCallableBody`即使没有safepoint也必须出现在callable producer表；init/root entry与stackmap raw function address均通过该表验证，不能只靠全局text权限。native extern/runtime/`bridge.o` body不属于该集合，按3.1的独立边界验证。

`ScoopRootEntryDescriptorV1.gateway`是compiler生成、C-callable、**不得展开异常**的精确`uint32_t(void)`runtime gateway，不是把任意Scoop函数指针强转给C。link verifier要求`callable_id`是root image中源码签名恰为ordinary/non-generic/non-suspend `() -> Unit`的main body；`gateway_callable_id`必须由3.1的`RootGateway { root_cone, main }`重算，gateway pointer逐bit等于该callable registration的entry，`gateway_definition_fingerprint`逐byte等于同一registration的`body_definition_fingerprint`；该record必须由root image的strong callable producer表拥有，pointer位于最终程序任一executable segment。v1不含per-Cone text range，不能伪称验证“位于root Cone text”。gateway以自己的body identity生成并登记入口poll、内部managed call/异常路径的全部safepoint，再调用该namespaced main。成功返回0；未捕获Scoop异常必须在gateway内部catch、物化到已登记的`failure_root`、结束native catch后返回1；其他值是fatal ABI错误。runtime只经6.2的逐次native→managed transition wrapper调用它；收到失败status后从rooted Throwable输出既有uncaught诊断并终止，异常绝不穿越C frame。

同理，`initializer_entry`/`ensure_entry`是供generated managed代码相互调用的Scoop managed entry，C runtime不得直接调用；两个id都必须解析为对应entry的`PersistentCallableBodyId`且pointer逐bit等于callable registration entry。每个eager unit另有精确`uint32_t(void)`的`startup_gateway`：`startup_gateway_callable_id`必须由3.1的`InitializationStartupGateway { unit }`重算，pointer逐bit等于该callable registration的entry，`startup_gateway_definition_fingerprint`逐byte等于其`body_definition_fingerprint`；runtime对每次调用都执行6.2的transition wrapper，gateway以自己的body owner登记入口poll和内部site并调用`ensure_entry`。成功返回0；未捕获异常在边界内结束catch并要求unit的已登记`failure_root`已经发布后返回1。runtime据此终止startup。lazy访问仍从managed代码调用普通`ensure_entry`并按M21语义向源码抛出，不能错误复用no-throw startup gateway。

IR中schedule必须是封闭`InitializationSchedule::{EagerStartup { gateway }, LazyAccess}`。固定C record的编码矩阵为：Eager的gateway callable id/fingerprint均非零、pointer非null且位于最终程序executable segment，并由当前image strong callable表或与unit相同的已验证ODR owner闭包生产；Lazy的这两个digest全零、pointer为null且runtime永不调用。其余`cell`、`storage`、`failure_root`、initializer/ensure id与entry在两种schedule都必须非null/非零；两个storage registration必须由当前image strong producer或与unit同一ODR group拥有，entry也必须由相同owner闭包发射。root entry的`failure_root`同样必须由root image producer表拥有且初始为null value。这里的tagged null是schedule编码的一部分，不适用“空span使用sentinel”的规则。

`failure_root`在LIR中不能是任意`StaticStorageDescriptorRef`，而是`FailureRootStorageRef`，其identity key的封闭role为`InitializationFailureRoot { unit }`或`RootEntryFailureRoot { root_cone, main }`。两种role都机械编码成一条专用static-storage registration：当前64-bit profile要求`byte_size == allocation_extent == 8`、`required_alignment == 8`、`scan_kind == Recursive`、scan恰为canonical `References { offsets: [0] }`（word program `[1, 0]`），其scan/layout fingerprint必须由该单managed-ref slot重算，且在任何managed代码前8 bytes全零。一个failure root不能兼作property/published/ZST token或另一个不同unit/root的failure root；只有同一已coalesce ODR identity可重复指向同一registration/range。link verifier与runtime在允许gateway/coordinator写入前逐项验证这些条件，否则是fatal metadata error。

`cell`同样在LIR中是owner绑定的`InitializationCellRef`，不是裸可复用metadata pointer。共享header已冻结其v1 layout为16/8；启动验证要求整个range位于writable segment、按8对齐、初始精确为`{ state = Uninitialized(0), owner_thread = null }`。不同unit的cell range互不重叠，也不得与任何static-storage allocation/failure-root range重叠；只允许同一已验证ODR unit的coalesced record共享同一cell。runtime先完成range、初始状态与反向`cell address -> unit identity`唯一性检查，之后coordinator才可读写它。

init descriptor的普通`storage`也必须是owner绑定的`InitializationValueStorageRef`，其persistent storage key包含该unit及property/published-object role；不能指向另一个unit的合法storage registration。两个不同unit不得共享该registration或allocation range，`storage`与本unit的`failure_root`也必须不同；唯一例外仍是同一ODR unit经完整record验证后的重复引用。该关系由LIR类型保证，并由link verifier/runtime的`storage id/address -> initialization unit`反向map防御性重验。

`ScoopRuntimeCoreBindingsV1`只包含C runtime主动使用的能力。trusted core Export HIR/LIR非可选地提供`RuntimeCoreCapability::String { exact_type_id, layout_contract }`；linker必须证明binding中的`string_type_id`精确等于该well-known relation重算出的`PersistentExactTypeId`，不能用另一个同布局core class替代。共享ABI header中的v1 capability-kind常量严格按6.1 encoder计算为`SHA-256(ByteSpan("scoop-runtime-core-capability-v1") || ByteSpan("String")) = d69647675041ab4e7a2550b9a5a27b49741c08b30ab07b259d998e606f218b77`，并以上述`SCOOP_CORE_STRING_CAPABILITY_ID_V1[32]`字节数组声明；final-program builder和C runtime直接使用这32 bytes参与Graph key/golden，不以C字符串NUL、host endian或symbol地址重算。该常量只标识versioned String capability kind，不单独编码runtime ABI；Graph已把`runtime_abi`作为独立必需输入，core声明/契约变化必须反映在其typed relation与runtime ABI中。

String binding还必须指向reserved core image**自身producer表**中的type registration，`string_type_id == string_type->registration.semantic_id`且其TD地址相同；runtime逐项要求`sizeof(ScoopObjectHeader) == object_header_size == 16`、`offsetof(ScoopString, len) == string_length_offset == 16`、`offsetof(ScoopString, data) == string_bytes_offset == string_minimum_size == 24`、alignment为8，且TD shape为`InlineBytes { minimum_size/inline_offset: 24, instance_alignment: 8, inline_size/stride/alignment: 1 }`、两个scan均为None并匹配fingerprint。这样String分配不会只因拿到“某个看似可用的TD指针”就越界。compiler生成代码继续使用普通typed external refs，这个表不是运行期名称服务。

### 6.2 启动注册阶段

进程启动顺序固定为：

1. runtime/thread/callback的纯native基础状态；
2. **envelope phase**：只读取唯一program的固定prefix及已验证范围内的顶层字段；逐层验证program/image/entry/core/六类record的prefix、exact `struct_size`、pointer/count乘加、alignment与segment权限后才读取对应其余字段，不跟随尚未验证的nested pointer；
3. **provisional-index phase**：遍历全部image/table，验证coordinate/dependency结构、record排序与表内typed registration identity，建立尚未对外可见的`semantic id ↔ record address`、callable body与object-address maps及dependency DAG；此时不重算或信任RuntimeImage/Graph fingerprint，也不调用任何登记副作用；
4. **semantic phase**：解析最终section中串接的全部stackmap v3 blob并核对safepoint/body owner；验证TypeDescriptor/scan/layout、ODR、static-storage/immortal range、init cell、专用failure root、initializer/ensure/gateway、root entry与core String binding，再用provisional maps解析所有cross-record pointer；
5. **image-hash phase**：只从步骤4已验证并类型化的record key按canonical encoder重算每个`RuntimeImageFingerprint`，与object/manifest值逐一核对；
6. **graph-hash phase**：验证canonical dependency-first image顺序及root/core owner，重算`RootEntryKey`、`RuntimeCoreBindingsKey`与最终`GraphFingerprint`并核对program slot；
7. 全部成功后一次性commit并freeze static metadata registry；任何失败丢弃整个provisional state；
8. 初始化GC heap/handle等需要完整root universe的状态；
9. attach主线程，初始为native-safe且没有活动managed栈段；
10. 按6.3对每个eager unit独立执行下述transition wrapper，经no-throw startup gateway运行初始化，返回后恢复native-safe；
11. 再独立执行一次transition wrapper，通过program descriptor的no-throw root gateway调用typed root `main`，返回后恢复native-safe。

每次C startup coordinator调用gateway，都在本次wrapper的栈底建立独立`NativeToManagedBoundary`并按LIFO保存previous mode/boundary。新段状态是封闭`EntryPending | ActiveManagedSegment`：`EntryPending`证明gateway尚未执行、段内没有managed frame，不能伪造C return PC作为anchor。wrapper发布完整boundary及managed mode后按runtime spec 3.5复查GC phase/epoch；若需要park，使用该empty-segment证明，醒来重验后才执行gateway。gateway的typed `NativeGatewayEntry`强制第一个可握手操作为本body的入口poll，发布准确managed anchor并把段变为`ActiveManagedSegment`，此后使用普通managed call/invoke/root plan。collector扫描始终止于本次boundary，不能跨入C coordinator。gateway在仍处managed段时完成catch、异常物化和failure-root发布、结束native catch，只返回GC-free status；wrapper收到返回后不再保留该段managed frame，以release语义发布native-safe，再按握手协议移除本次boundary并恢复外层记录。transition链的修改与collector snapshot通过同一epoch/发布协议同步，不能在collector可读取时销毁record。每次调用都独立enter/leave，不能用attach时的一条boundary覆盖整个eager loop；普通callback重入继续沿相同LIFO规则另建段。

任何managed allocation、initializer、callback或Scoop exception构造都不能发生在步骤1–8。尤其不能为了提早验证Graph而解引用尚未通过步骤2–4的record/scan/gateway pointer；metadata验证失败是稳定fatal artifact/runtime ABI error，source构建流程应由link verifier更早捕获同类错误。

root登记从“恰好调用一次的单表”改为registry。不同identity的storage allocation range不得重叠；即使logical `byte_size == 0`也必须通过1-byte identity token取得不同地址。ODR重复按record variant比较：storage的record/base/size/extent/alignment/scan/layout，immortal的record/start/size/alignment/type，init的record/schedule/path/cell/storage/failure/entries/gateway，type的record/TD/runtime id/descriptor/layout，safepoint的record/site/owner/root count/stackmap，callable的record/body id/entry/body definition，以及各自registration identity/definition fingerprint都必须完全相同才去重一次。已确认的同一ODR immortal/storage/callable winner可共享对应range/address；其他range/address不得重叠。相同stable id对应不同地址说明linker未正确coalesce，必须fatal，不能登记两份来掩盖TypeDescriptor/static/callable identity分裂。

### 6.3 初始化顺序、重复与失败

runtime独立验证driver给出的topological order。每轮ready set是“全部direct dependency都已出现在此前prefix”的剩余image，从中按canonical coordinate byte order取最小；因此dependency总在dependent之前且不受edge存储方向影响。每个Cone中eager unit按`PersistentInitializationUnitId` bytes严格递增。runtime对每个unit经6.2的独立transition wrapper调用no-throw `startup_gateway`，gateway内部再调用generated ensure；因此显式直接依赖可以提前初始化目标，cell/Failed/cycle语义完全沿用M21，同时不会让异常穿过C frame。

init descriptor全程序登记时：

- non-ODR unit stable id/key必须唯一；
- generic delegated extension等ODR unit可重复出现在多个image table，但link后descriptor/cell/storage/failure root/entries必须已coalesce为同一组地址且fingerprint一致，registry只保留一次；
- lazy unit登记但不在eager loop运行；第一次访问仍通过同一coordinator；
- dependency Cone initializer失败或未捕获cycle时立即终止startup，所有dependent Cone和`main`均不执行；已完成副作用不回滚；
- cycle message使用带coordinate的stable unit path，跨Cone/ODR unit不因相同短名合并；
- metadata登记本身不触发object/companion或generic delegate initialization。

### 6.4 stackmap与“image”范围

M23的image是一个Cone codegen产物对应的**logical Scoop image descriptor**。v1所有logical image静态链接进一个Mach-O executable，但Darwin linker对各object的`__LLVM_STACKMAPS,__llvm_stackmaps`只是byte concatenation，不会合成为单个LLVM stackmap v3 header。platform adapter可以返回主image中的一个native section span；通用parser必须把它迭代解析为一个或多个连续v3 blob：从每个header的function/constant/record count及对齐精确计算该blob消费长度，再从下一byte开始验证下一个header，直到恰好耗尽section，不能让只认识第一块header的现有parser静默忽略后续Cone。

同一ODR weak函数可使多个输入blob在最终链接/ASLR relocation后描述相同`{SafepointId, return PC}`。runtime/final verifier只在full persistent site id、owner/group/member、normalized location payload和`normalized_stackmap_fingerprint`全部相同时去重；相同ID或PC但payload不同、不同full key映射同一64-bit ID/PC、或registration没有且仅有一个去重后的stackmap record都为fatal。root定位以该**原始return PC**为主键，不做`PC-4`、nearest-symbol或地址范围修正；SafepointId只用于完整性/诊断，不能代替PC。

Darwin/AArch64 M23最终link profile明确禁止`-dead_strip`：raw stackmap section没有由program/image pointer graph产生的symbol relocation保活关系，实测dead-strip会整段删除它。未来只有在target提供并经artifact测试证明的section retention/重写机制时才能重新开放dead-strip；“descriptor强引用SafepointId表”不能当作证明。最终link测试必须同时证明多object concatenation后每个Cone的managed frame都能命中记录、ODR重复按上述规则去重，且故意启用dead-strip会被profile拒绝。

M15的platform API继续保留span list，为未来dylib准备，但M23不登记dyld callback，也不允许运行期新增/删除**静态image descriptor、其静态storage/root table或stackmap native image**。native root frame、exception stable external root、handle与pin仍按既有runtime API动态变化，不受static metadata registry冻结影响。静态multi-Cone通过不等于动态loading完成。

### 6.5 终止

program descriptor与image metadata在整个进程生命周期稳定。shutdown沿用M13/M21/M25顺序：拒绝新attach/callback，验证无非主线程/token/caught exception/active initialization，再销毁GC与thread状态。M23不卸载Cone、不撤销image root、不运行global destructor，也不逆序执行initializer清理。

## 7. Generic delegated extension property

M23开放M21延期的形态：

```kotlin
val <T> Box<T>.cached: Value by cacheFor<T>()
```

既有约束不变：全部type parameter只能由receiver exact静态type与bound唯一确定；property/delegate role ordinary、non-suspend；getter/setter在定义处已经绑定为typed template。新增规则为：

- 每个成功使用的exact receiver application产生`GenericDelegateStorageSpecializationId`，key为origin `PersistentExtensionPropertyId`加完整receiver concrete type arguments；result expected type、setter value或使用Cone不进入key；
- specialization是一个ODR group，整体包含effective delegate storage、managed root descriptor、init cell、failure root、initializer/ensure entry与init descriptor；
- delegate expression和可选`provideDelegate`按template substitution在consumer Cone concretize，不重新解析name/import/operator；generic body可沿hidden support closure调用origin Cone的narrower helper；
- unit固定为`LazyAccess`。第一次任一Cone访问该specialization时exactly once求值并发布；M23不因最终程序恰好知道所有使用点而把它改成eager startup；
- 多个Cone产生同一specialization时使用同一ODR group，link/runtime按3.4和6.3验证真正只有一个storage/cell。不同receiver application各有独立storage和失败记忆；
- moving GC把delegate/failure storage当普通registered root；初始化中间值留在managed frame root，不pin；
- re-export property不改变specialization origin/key。visibility决定候选能否使用，不决定storage复制；
- 不生成type-erasedmap、runtime dictionary、按type name查表或一份跨所有T共享delegate。

non-generic delegated extension继续按M21普通top-level eager storage；两者用不同typed init schedule/storage id，不能靠“type argument Vec是否为空”区分。

## 8. 诊断、健壮性与可复现性

至少提供以下稳定错误：

- manifest schema/coordinate/kind/dependency/locator错误，reserved core伪造、path/artifact coordinate不匹配；
- dependency cycle、同`group:name`多version、同Cone identity不同semantic artifact、executable dependency；
- source path逃逸/重复/非UTF-8、空Cone、executable entry缺失/重复/签名错误；
- package/import位置或path错误、star alias、非direct import、不可见target、exact/split-package歧义；
- public import当前Cone/internal/private/protected target、destination binding冲突、star展开冲突；
- imported overload/extension/property/alias按M16–M22普通规则产生的无适用/MSC/访问错误，诊断同时显示声明source coordinate；
- public inheritance surface缺失protected slot/constructor或witness损坏；
- `.slib` magic/hash/schema/ABI/target不兼容、section缺失/重复/超限、typed ref/kind/index/origin错误；
- stale direct dependency fingerprint、HIR/MIR/LIR bridge不完整；
- symbol、persistent id、runtime type id、SafepointId碰撞；ODR同key不同member/fingerprint或link后多地址；
- program/image descriptor缺失/重复、DAG/order/table/root/TD/init record不完整；
- generic delegated extension不能由receiver定型或specialization ODR不一致。

driver/reader错误不得包含host绝对路径进入golden中；CLI可在定位本地manifest时显示用户传入路径，但artifact内部diagnostic source一律显示`group:name:version/src/...`。diagnostic排序为graph phase/order、Cone coordinate、source path/span；不能依赖HashMap、archive member或filesystem枚举顺序。

可复现门禁使用两个不同absolute checkout/temp output路径、打乱文件创建顺序与dependency input枚举，比较`.slib`逐byte相同及最终symbol/metadata集合相同。最终executable本身若受系统linker UUID影响可在剥离已知非语义字段后比较；不能因此放弃`.slib` bitwise门禁。

## 9. 测试与验收

### 9.1 manifest、graph与parser

- library/executable、path/artifact/search dependency三种locator、implicit core；
- chain、diamond、无依赖siblings、cycle、自环、多version、same coordinate different artifact；
- recursive multi-file source、不同package、root package、source排序/路径归一化；
- package/import/public import/alias/star语法与多错误恢复；
- library无main、dependency main不抢entry、root main缺失/重复/ordinary/Unit规则。

### 9.2 resolver与visibility

- exact import → same package → star → prelude的层级；高层全不适用时继续、同层MSC；
- explicit receiver member优先，各extension scope层，property-like六分区与operator/iterator/component role；
- exact alias、多个star同origin去重、diamond re-export同origin去重；不同origin同FQN/type歧义与function overload；
- direct dependency可见、未re-export transitive不可见、链式public import可见；
- public/internal/file-private/member-private/protected跨Cone矩阵；下游subclass可访问protected inheritance surface但普通lookup不能；
- public interface default/override/property accessor/object/companion/const/non-generic alias跨Cone；
- M17 default经exact/star/re-export使用都保持定义处target与origin，调用方同名声明不能改变body；
- core prelude中的普通function/type与`Option.*`，删除任何user/core file-index特判后结果不变。

### 9.3 `.slib`与stage round-trip

- Export HIR每类surface/closure独立golden；hidden support没有binding entry，protected没有ordinary import入口；
- HIR/MIR/LIR metadata encode → decode → typed remap → re-encode canonical bytes相同；
- generic concretization predicate逐variant wire round-trip；unknown tag、缺字段路径、binder/persistent type ref越界均拒绝，predicate或字段路径改变必须改变HIR semantic fingerprint；
- 同一artifact由两个direct path加载只intern一个origin world id；
- LocalConcrete、solver scratch、失败candidate、无关private body与absolute path不在archive；
- 每个section删减、bit flip、truncation、bad hash/schema/kind/index/arity/witness/bridge/ODR/source span均返回结构化error且不panic；
- section/count/nesting/string/body resource limits有边界测试；
- 隔离目录与随机input顺序bitwise reproducibility；
- HIR-only变化、layout变化与object-only变化分别触发正确compile/relink fingerprint；
- typed digest graph以不同合法topological traversal得到同一结果；缺边、环、双writer、错patch owner/offset、非零provisional slot均拒绝。`DigestNodeId`与strong registration使用固定domain/kind/input排序，逐项篡改strong record canonical field、own-slot归零规则或任一typed input都改变结果；逐项篡改六种`RuntimeImageRecordKey`、stackmap fingerprint、`RootEntryKey`和`RuntimeCoreBindingsKey`时对应image/program digest必须变化，不能被peer-slot归零掩盖；
- Rust producer/finalizer/link verifier与C runtime消费同一canonical encoder golden bytes/hash：覆盖空与非空dependency/六张table、每个record/choice/role/site tag、Eager/Lazy、byte-span及count边界；缺失空table count、错误endian/padding/order、unknown/reserved tag均稳定拒绝；
- scan normal-form golden覆盖None/References/Sequence/Array及共享DAG；Recursive `[0]`、count 0、singleton/nested Sequence、None child、未合并References、乱序/重复offset/child、cycle以及刚好超过depth/node/word/expanded-node/canonical-byte五项预算均在reader和runtime一致拒绝；另用共享大References leaf被多条语义路径引用的DAG锁定memoized subtree cost与按路径重计，不能退化为重复编码/比较的二次工作。

### 9.4 generic、layout与link

- 上游generic function/type + 下游local value/ref type实例化；
- 两个sibling Cone实例化同一上游`Box<Int>`/function，最终只有一个function/TypeDescriptor/dispatch/scan group；
- 相同短名不同origin不合并，相同origin不同type arguments不合并；
- downstream class继承upstream class、实现upstream interface、继承default、扩展vtable/itable并通过`is/as`；
- generic closure/coroutine/callback/box/array/tagged enum含ref layout跨Cone；
- provider导出`@CLayout G<T>`并由consumer分别以C-safe non-ZST、ZST、ref及其他无C表示类型具体化；只允许第一种，失败诊断位于consumer application并带provider字段路径。predicate wire round-trip、unknown-tag拒绝及semantic-fingerprint变化由9.3锁定；
- 人工制造同ODR key不同fingerprint在link前失败；禁止让native linker任选；
- 后端生成memory/TLS/EH helper与C bridge entry分别具有typed requirement；源码声明公开`memcpy`等support symbol时，完整ABI/library一致可共用、冲突须在native linker前失败，任意私有runtime/Scoop/bridge符号别名或无capability的undefined symbol均拒绝；
- 全部Scoop-owned symbol符合persistent mangling，两个Cone声明同package/name的non-generic实体无link碰撞；
- native extern/library/bridge requirements从transitive closure完整汇总；同symbol的canonical contract相同则去重，library、function/data/TLS/mutability、C/Scoop ABI、calling convention、参数/结果classifier或storage type任一差异均在native linker前失败，诊断包含双方origin及不同字段。未调用的声明同样参与验证；`code.o`/`bridge.o`的`SourceExtern` relocation漏contract或匹配多个contract时拒绝。只改变contract而不改变object bytes也必须使code/link key失效。

### 9.5 ZST、ABI与array

- `Unit`、空ordinary struct、全ZST嵌套struct/tuple与`Phantom<A>`/`Phantom<B>`分别锁定`ZstStatus`、0/正alignment、field offset和distinct `PersistentExactTypeId`/TypeDescriptor；`Option<ZST>`继续使用非零tagged layout。另以LIR layout单测覆盖schema允许的大于1 alignment及超过`maximum_managed_alignment`的拒绝路径；
- TypeDescriptor的五种shape逐variant做C/LLVM `sizeof/offsetof`、encode/decode及非法矩阵测试。`BoxedValue`同时覆盖普通含ref value的payload-relative `inline_scan`到object-relative `object_scan`平移，以及ZST的size 0 payload/nonzero minimum allocation；`FixedObject`与`AbstractRef`不能再通过同一个0 size混淆，allocation LIR不能构造`AbstractRef` target且伪造C record由runtime拒绝；
- 两个sibling Cone分别经re-export/import alias spelling与完全展开underlying spelling materialize同一nominal application、tuple/function及pointer exact type，必须重算出逐byte相同的canonical `diagnostic_name`并coalesce为一个TypeDescriptor/diagnostic atom；篡改name byte、长度、origin/owner tag或让descriptor span指向consumer-local display string时，reader或ODR/object verifier稳定拒绝；
- generic template在两个consumer生成的closure/adapter/coroutine nominal必须通过固定generated role/id分支得到相同诊断名，不得读取arena ordinal或临时name；篡改generated role/id或构造超过16 MiB的共享type DAG展开名时，producer/reader在分配前稳定拒绝；
- provider/consumer Cone之间调用带多个ZST参数并返回用户ZST，锁定`ElidedZst`及完整source signature/mangle/call fingerprint；两个物理LLVM signature相同但exact ZST不同的callable不得合并，参数producer的副作用与异常顺序仍执行；
- `Array<Unit>`、`MutableArray<Phantom<T>>`覆盖length 0、1和大但合法值，literal/assembly/spread计数与副作用、越界`set`仍先执行RHS、get/set bounds、整数index iterator、clone/互转fresh ref identity、allocation size不随length变化、无payload copy/barrier/scan且collector工作量不随length增长；长度/assembly/index overflow仍拒绝。另构造alignment大于8且含managed ref的非ZST element，证明`first_element_offset != 24`时moving GC仍更新正确slot，并篡改scan的length/first-offset/stride逐项稳定拒绝；
- String、非ZST InlineArray与ZST InlineArray分别在mul/add/alignUp、`INT64_MAX` count、target `size_t`及`maximum_managed_object_size`的恰好边界与越界处测试；任意负内部count、overflow、wrap/truncate都稳定fatal，且没有虚构的源码`IllegalArgumentException`路径；
- ZST boxing每次产生fresh ref，经过interface dispatch及`is/as`仍使用对应exact TD，并在moving GC压力下保持合法；LIR/runtime调用必须命中无payload pointer/destination的ZST box/unbox分支。普通over-aligned/含ref value box验证只使用NonZero place、不重复传size/scan、不固定`+16`且collector从object base扫描正确；
- address-taken ZST parameter/local/value-typed `this`分别验证non-null、alignment、有效期及同一时间不同place不共址；不重叠生命周期允许复用但不要求，token不可作为payload读写。compiler-owned static/ODR token与初值由下一项覆盖；
- compiler-owned static初值分别覆盖：有unit的GC-free/Recursive storage与published/failure root只能使用`ZeroedForRuntimeUnit`；无unit的非零integer、全零`Option.None`、ZST token及immortal String ref必须使用`EncodedStaticValue`。Rust/C golden锁定template恰为allocation extent、padding/pointer leaf归零、relocation按offset排序且target使用immortal typed id；object/link/runtime negative逐项覆盖错tag/length/byte、乱序/重复/非leaf/越界offset、None带relocation、Recursive非null leaf漏项/多项、target未登记/非object-start/任意heap ref，以及同ODR storage的template或target不同。逐项变化必须改变strong/ODR definition及RuntimeImage fingerprint，并在managed startup前失败；
- unsafe `Ptr<ZST>`覆盖正负offset都保持pointer bits、load/store的receiver/offset/value求值、alignment/lifetime前置条件与禁止pointer-progress；`Ptr<Unit>` byte arithmetic必须显式转成正确的`Ptr<UInt8>`；
- C ABI positive只保留`Unit` result、`Ptr<Unit>`/`Option<Ptr<Unit>>` opaque pointer；negative分别覆盖ZST by-value参数/result/callback、extern global/TLS、空/含ZST `@CLayout`、`Ptr<非Unit ZST>`及`Option<Ptr<非Unit ZST>>`。

### 9.6 runtime与端到端fixture

- 3+ Cone roots/immortal objects/TypeDescriptors全部登记后在各Cone代码中触发moving GC；
- dependency-first初始化与无依赖coordinate tie-break，Cone内stable key顺序；direct ensure提前、lazy object与generic delegate不提前；
- 上游initializer失败阻止下游和main；same/cross-thread cycle path含Cone identity；
- 在每次startup/root gateway的enter、首次poll、managed body与leave边缘请求moving GC，验证epoch握手不丢失、每次调用单独建立/移除boundary且C coordinator始终不进入managed stack walk；失败root须先发布并结束catch再离开，嵌套callback按LIFO恢复外层段；
- sibling Cone重复generic delegate specialization只有一次求值、一个storage/cell/root，失败记忆共享；不同application各一次；
- concatenated multi-object stackmap v3 blob在普通/moving-stress下覆盖上游、下游、generic instance、exception、closure、coroutine与foreign callback frame；
- program/image/storage/TD/init/safepoint descriptor的prefix、size、identity、双向地址、fingerprint或pointer range损坏时runtime unit测试稳定fatal；两个strong零尺寸global/init storage分别拥有不可合并token，ODR零尺寸storage只coalesce同一member，伪造共址或错误allocation extent稳定失败；root/init no-throw gateway保证源码异常不穿越C frame；
- 两个以上object产生连续stackmap v3 blob时全部解析；ODR重复record精确去重、不同payload拒绝；Darwin profile拒绝`-dead_strip`并证明无stackmap丢失；
- 最终artifact无重复TypeDescriptor、旧固定`scoop_main`/`scoop_image_*`或未namespaced Scoop symbol，并继续满足M25 EH导入门禁。

新增fixture建议分为：

- `tests/fixtures/m23-imports/`；
- `tests/fixtures/m23-reexports/`；
- `tests/fixtures/m23-generics/`；
- `tests/fixtures/m23-zst/`；
- `tests/fixtures/m23-initialization/`；
- `tests/fixtures/m23-errors/`；
- `tests/slib/`的format/corruption/reproducibility测试。

每个多Conefixture自身含多个Cone目录和manifest，runner以graph root为单位快照每个Cone AST/Export HIR/LocalConcrete HIR/MIR/LIR及最终stdout/stderr。至少一个组合fixture串联re-exported generic default → downstream local type specialization → interface dispatch → delegated generic extension → eager/lazy globals → moving GC/exception；M1–M22及M25全部单Conefixture通过内部synthetic manifest迁移后原样回归。

## 10. 实现顺序与完成门

1. 同步language/runtime/impl spec与ROADMAP，固定术语、manifest/import语义、persistent identity、ODR和program descriptor；
2. 建立manifest/resolved graph/source discovery与library/executable entry模型，fixture runner迁到typed synthetic Cone input；
3. 引入persistent typed id、definition key、全量mangler和runtime/safepoint id门禁，先消除跨Cone必碰撞的旧symbol；
4. parser加入package/import，HIR建立semantic world、binding/re-export index与精确candidate layer；
5. 把Export HIR拆成public/inheritance/hidden/default闭包，落地三层显式wire schema、deterministic `.slib` writer/reader及corruption/reproducibility测试；
6. 分离并预编译trusted `scoop.core`，删除core/user同unit、`user_file_index`与Option/prelude模拟；
7. MIR/LIR接入selected imported meta，完成cross-Cone inheritance/layout/external target、ZST/typed ABI/array shape和consumer-side generic specialization；
8. 建立ODR group完整member/fingerprint检查、generic delegated extension lazy storage与native linker coalesce验证；
9. codegen改发namespaced image descriptor，driver生成program descriptor；runtime改为registry、全image root/TD/init登记与canonical初始化；
10. 完成multi-object stackmap/final artifact/moving-GC组合、缓存失效矩阵和全量回归。

每一批先`cargo fmt --all`与`cargo clippy --workspace`，再执行对应unit/golden/fixture。中间提交不得保留两套生产入口（single-file core拼接与真实Cone）、两套symbol mangler、raw arena id wire格式、固定`scoop_image_*`weak fallback或“ODR不一致让linker选择”的路径。

M23只有在以下条件同时满足时完成：任意library Cone无需`main`即可独立生成可重复`.slib`；direct/re-export/import层在下游使用同一M16/M17 resolver/default协议；non-generic alias/protected inheritance/generic hidden closure跨artifact边界不泄漏权限且信息完备；reader对完整损坏矩阵无panic；上游generic与下游local type可实例化，重复specialization的全部runtime identity真正coalesce；ZST从HIR status到LIR layout、Scoop/C ABI、box、static token及`Array`/`MutableArray`均通过9.5矩阵；generic delegated extension全程序exactly once；core作为trusted独立`.slib`消费；最终程序先登记全部image/storage/root/TD/stackmap/callable/init metadata，再按canonical DAG顺序初始化并从no-throw typed root gateway运行；多Cone moving-GC/exception/closure/coroutine/FFI组合通过。

## 11. 明确不做

1. package registry、网络下载、version range/SAT求解、lockfile生成、dependency feature与workspace发布；M23只消费exact resolved graph；
2. shared library ABI、`dlopen`/`dlclose`、hot reload、plugin discovery、image unload/global destructor及运行期新增Cone；
3. 跨不同target、language/runtime ABI或wire schema的兼容适配；v1只接受exact capability match；
4. 加密签名、publisher trust、sandboxed compiler或对恶意producer的真实性认证；reader仍必须内存安全并拒绝损坏结构；
5. generic/nested typealias、interface method自身type parameter、declaration/use-site variance、star projection或runtime generic dictionary；
6. dependency-coordinate-qualified源码name与split-package显式disambiguation语法；不同origin冲突先给稳定歧义诊断；
7. wildcard public API的lint、semver/API compatibility checker、dead public code分析或自动生成re-export；
8. cross-Cone LTO、thin-LTO、whole-program devirtualization或根据最终图选择唯一generic emitter；v1使用可验证ODR，Darwin/Mach-O按同名per-member weak/linkonce coalesce，未来target才可在等价验证之外使用COMDAT；
9. binary-only隐藏源码的强保密保证。`.slib`为generic concretization必须携带必要typed template/hidden support metadata；optional source text可省略，但semantic body不是加密格式；
10. M24 String byte API与字符串插值，以及M23之后其他语言backlog。

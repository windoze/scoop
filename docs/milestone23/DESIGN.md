# M23 系列设计：多 Cone、导入系统与 `.slib`

版本：0.4（设计完成，待实现；2026-09-09）

对应`docs/ROADMAP.md`的M23-1…M23-11。M23系列在M16/M17统一调用决议与default template、M20 exact generic application、M21 typed access domain/全局初始化以及M22非generic typealias之上，把“core源码与用户源码同一编译单元”的过渡模型替换为真正的独立Cone编译：低层编译器`scoopc`每次只把一个Cone编译为可复用`.slib`，umbrella build tool `scoop`负责解析多Cone图、缓存与调度，独立link stage再把依赖闭包中的link input和image descriptor静态链接成最终程序。下游编译只通过版本化metadata消费上游语义接口。本文仍是这些子里程碑的唯一总体设计，避免拆分完整的identity/wire/runtime契约或制造重复真源。

本里程碑不是给现有arena id外面套一层Cone编号。跨artifact identity、import provenance、generic specialization、TypeDescriptor唯一性、初始化登记与缓存失效是同一个闭环：只要其中一处仍以FQN、link symbol、输入顺序或host路径补猜，钻石依赖就可能把同一实体复制成两个运行期类型，或把不同实体错误合并。

## 0. 关键决策与范围

- **Cone是module/distribution/build unit，package只是源码namespace。** 二者不互相推导；一个Cone可包含多个package，同一package也可由多个依赖Cone贡献声明，冲突由typed origin与普通名称规则诊断；
- M23 v1只支持最终链接前已完整解析的、**静态无环Cone图**。不实现registry下载、版本范围求解、lockfile生成、动态库加载、`dlopen`、image卸载或ABI-stable plugin；
- `Cone.toml` v1使用exact coordinate与library/executable kind。dependency locator可以是source path、`.slib`路径或搜索路径中的exact artifact，但locator不进入Cone identity和产物metadata；
- **`scoop`与`scoopc`的关系类比Cargo与rustc。** `scoop`拥有manifest locator解析、完整DAG、缓存与多Cone调用顺序；`scoopc`只发现并编译命令行指定的当前Cone，要求完整上游`.slib`闭包作为显式输入，不递归定位、构建或重建任何上游Cone；
- **`scoop build <file.scoop>` / `scoop run <file.scoop>`是正式单文件模式。** 该文件被建模为只含一个source、只具有trusted core Cone依赖的synthetic executable Cone，仍通过普通single-Cone `scoopc` → `.slib` → program-link路径。它不扫描相邻manifest、其他`.scoop`、C/C++源码或blob；FFI native library requirement不是Cone dependency，只能由`.slib`中已有的typed requirement经显式`--library-path`解析；
- `ConeIdentity`由canonical `group:name:version`产生，artifact content另有fingerprint；同coordinate不同metadata不能伪装成同一个可替换artifact；
- source `package`、exact/star import、`as` alias和`public import`由M23-1先冻结语法与本Cone结构，再由M23-5接入跨Cone semantic world。re-export只建立新的公开binding，保留最终实体的origin identity，不复制声明或改写type identity；
- import只给M16/M18既有resolver增加typed candidate source。每个候选仍独立完成M17参数映射、constraint、postponed argument、applicability和MSC；不存在“来自`.slib`所以先选中”的远程捷径；
- Export HIR的序列化闭包分成**public lookup surface**、**inheritance/slot surface**与**generic hidden support closure**。后两者可包含不能普通import的实体；语言可见性与native link visibility严格分离；
- `.slib` v1是target-specific、可复现的确定性archive，包含manifest、Export HIR metadata、MIR metadata、LIR metadata及一个类型化成员目录。目录可承载任意多个由Scoop、generated bridge、未来C/C++或其他producer产生的`LinkObject`、诊断附件与由显式capability标识的embedded blob；只有显式`LinkObject`才作为object交给link stage，opaque blob即使扩展名或内容看起来像object也不能被提升。成员名、扩展名、顺序和数量都不承担语义。wire schema显式版本化，不直接`serde`当前Rust struct或持久化`la_arena::Idx`；
- 跨artifact实体使用按kind隔离的persistent typed id；reader验证后重映射成consumer session-local typed id。canonical key和link symbol只分别用于identity生成/发射，不能在缺失typed relation时作为lookup fallback；
- 上游generic template在每个实际需要它的下游Cone完成concretization、MIR/LIR lowering与发射。相同specialization的函数、layout、TypeDescriptor、dispatch table、adapter及generic static storage使用同一ODR group和fingerprint整体coalesce；
- M21延期的generic delegated extension property在M23开放：每个exact receiver application拥有program-wide唯一、lazy exactly-once的delegate storage/init unit，identity由property template与完整concrete receiver arguments决定；
- `.slib`首次冻结跨Cone layout与typed Scoop ABI，因此M23同时收口ZST：logical size保持0，address-taken/static place使用独立token，Scoop ABI显式elide payload，ZST array以logical index而非pointer推进，C ABI拒绝无portable C object representation的零尺寸值；
- 每个Cone artifact的全部linkable object合起来恰好定义一个名称唯一的hidden strong `ScoopImageDescriptorV1`；manifest以typed member/definition owner指出其所在成员，其他object与blob不得重复发射。独立link stage另生成唯一`ScoopProgramDescriptorV1`，显式引用依赖闭包中每个image、root executable entry与well-known core binding；runtime不扫描weak symbol或按符号前缀猜image；
- runtime先验证并登记全部image、stackmap、callable、TypeDescriptor、static storage/root、immortal object和init unit，之后才初始化heap、attach主线程并执行任何managed initializer。eager顺序为依赖优先、同层Cone coordinate稳定排序、Cone内`PersistentInitializationUnitId` bytes排序；
- reserved core Cone `scoop:scoop.core:0.1.0`从用户编译单元中移出，作为sysroot中由`scoop`解析、由`scoopc`验证authority的独立library Cone预编译/缓存。普通Cone对它具有隐式direct dependency；core prelude来自其typed export metadata，不再由user/core file index或`Option`短名特判模拟；
- M23包含M22**非generic**typealias的跨Coneimport、re-export与展开；generic/nested typealias、interface method自身type parameter、动态image与跨版本ABI承诺继续留在后续。

### 0.1 子里程碑与契约冻结点

本文中未带后缀的“M23”表示M23-1…M23-11全部完成后的最终基线；`.slib v1`、`PersistentV1`等名称中的`v1`是协议版本，不是子里程碑编号。各子里程碑的详细完成门见第10章：

| 里程碑 | 主要范围 | 首次冻结的边界 |
| --- | --- | --- |
| M23-1 | source `package`/`import`/`public import`/alias/star语法、parser与当前编译单元lookup（[详细设计](stage1/DESIGN.md)） | AST表面、当前单元名称语义与诊断 |
| M23-2 | persistent typed identity与`.slib` wire基础（[详细设计](stage2/DESIGN.md)） | identity（含callable application与ODR group/member key）、mangler、container/member envelope与schema演进规则 |
| M23-3 | single-Cone artifact与core分离 | `scoopc`请求、strong-only image/digest与基础Compile/Link view |
| M23-4 | resolved build graph与调度 | locator、DAG、artifact cache与child orchestration |
| M23-5 | 多Cone名称语义 | cross-Cone semantic world、import/re-export与access provenance |
| M23-6 | 跨Cone layout、typed ABI与ZST | layout/scan/ABI section与运行时表示 |
| M23-7 | 跨Cone generic、ODR与generic delegated extension | 完整ODR member closure、ABI/definition proof、materialization与跨Cone等价验证 |
| M23-8 | runtime multi-image registry与启动 | program descriptor ABI、image消费契约、登记/初始化顺序与stackmap |
| M23-9 | 基础artifact-only program-link | fixed target/runtime闭包、runtime-build、program object与真实链接 |
| M23-10 | general native requirement闭包与link evidence hardening | extern/archive/provider解析、snapshot、完整evidence与final verifier |
| M23-11 | umbrella CLI、单文件模式与总验收 | 正式工具边界、fixture迁移与旧路径删除 |

M23-2结束后不再为后续实现便利修改persistent identity、mangler、`.slib` container/member envelope或section versioning规则；它冻结generic需要的application/ODR **identity**，但不假装已经完成layout、generic hidden payload、ODR definition proof、runtime或final-link能力。per-Cone image、cross-Cone semantics、layout、完整generic/runtime及link相关section在负责它们的M23-3、M23-5…M23-10分别冻结版本，M23-10结束时封闭`.slib v1`的最终内建capability集合与Link view。M23-6结束后不再修改layout/typed ABI。后续只能填充已经预留且语义完整的typed variant，或提升对应section/schema版本显式演进。每个中间里程碑都必须是封闭、可验证的子集：尚未开放的capability稳定拒绝，不保留unchecked、`TODO`或暗中回退到旧ABI的分支。

## 1. Cone identity、manifest与构建图

### 1.1 三种身份不得混用

M23先冻结全协议共用的两种字节原语；后文没有另一套同名“canonical”编码：

```text
ByteSpan(bytes) = little_endian_u64(checked_u64(bytes.len)) || bytes

DomainSeparatedCborHash(domain, value) =
    SHA-256(ByteSpan(ASCII(domain)) || WireCborV1(value))

ConeCoordinate = { group, name, version }
ConeIdentity = DomainSeparatedCborHash("scoop-cone-id-v1", ConeCoordinate)
ArtifactFingerprint = 第4.2节定义的多片段framed hash
```

`WireCborV1`是第4.1节唯一的RFC 8949 deterministic CBOR子集。它输出一个自定界item，因此在`DomainSeparatedCborHash`中不再套`ByteSpan`；凡公式需要拼接其他可变长raw片段，必须显式写`ByteSpan`，固定32-byte typed id则显式写`raw(id)`。domain无NUL且ASCII bytes逐字固定，任何长度转换先checked到`u64`。callable body与runtime registration使用第6.1节runtime canonical encoder，不得把其bytes换成CBOR。

- `ConeCoordinate`是用户、诊断与依赖声明中的稳定文本身份。`group`/`name`各由一个或多个`.`分隔的lowercase ASCII段组成，每段匹配`[a-z][a-z0-9-]*`；`version`是SemVer 2.0.0的唯一合法文本（含pre-release/build metadata时原样参与identity，禁止前导零与多余`v`）。不做trim、大小写折叠、Unicode归一化或路径别名解析；不满足canonical grammar直接拒绝；
- `ConeIdentity`包含version。两个版本即使源码相同也拥有不同nominal/type/callable identity；M23的`scoop` build graph要求一个resolved graph内同一`group:name`只出现一个version，版本调停仍由更上层的依赖管理能力完成；
- `ArtifactFingerprint`描述这个coordinate的一次具体产物，按4.2排除自引用字段后计算。相同`ConeIdentity`而artifact/semantic fingerprint不同的两个`.slib`不能同时进入一次构建，也不能根据搜索路径顺序任取其一；
- source package、FQN、manifest/path、artifact digest、compiler session中的provider slot都不能代替`ConeIdentity`。源码无法构造或声明一个内部`ConeIdentity`值；
- SHA-256输入严格使用上述domain framing或其所在章节明确给出的其他typed framing，不用裸字符串、分隔符或未标明encoder的“canonical bytes”占位符拼接。完整coordinate与canonical identity record保存在manifest中，reader重新计算digest；digest不匹配或同digest对应不同canonical record是损坏artifact，禁止以文本名继续工作。

稳定排序显示用`(group UTF-8 bytes, name UTF-8 bytes, canonical version)`，不按digest排序。这样无依赖Cone和诊断路径对人可预测，同时identity仍由typed digest承载。

### 1.2 `Cone.toml` v1

manifest-backed生产Cone根目录必须有一个`Cone.toml`和固定`src/`目录。第1.3节的single-file root是唯一不从用户manifest取得Cone语义的正式例外。最小schema为：

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
"org.other:log" = "3.1.0" # 由scoop从artifact search roots解析
```

规则如下：

- `schema`、`[cone]`四字段必需，unknown required table/field直接诊断；未来可新增明确标为optional且不影响语义的字段，v1实现不能静默忽略拼错的semantic字段；
- dependency key是exact `group:name`，value给出exact version。字符串短式等价于只写`version`；不接受`^`、`~`、区间、`latest`或可选dependency；
- table value至多有一个locator：`path`指向source Cone根，`artifact`指向`.slib`，二者都省略时由`scoop`依次检查每个`--cone-path`下的`<group>/<name>/<version>/cone.slib`（三项均使用canonical文本，不拆`.`）。所有存在的候选必须具有相同完整artifact fingerprint，否则报告ambiguous artifact；root顺序不能决定选取不同内容。relative path相对当前manifest，仅用于定位；它不写入`.slib`、stable key、diagnostic source identity或缓存语义。`scoopc`读取同一manifest中的exact dependency声明，但不解释或跟随这些locator；
- path Cone或artifact manifest中的coordinate必须与dependency key/version逐字canonical相等，否则在读取源码前失败；
- 除reserved coordinate `scoop:scoop.core:0.1.0`外不存在隐式dependency。这里的Cone name `scoop.core`与源码package `scoop.core`只是当前core发布约定，不存在由package推导Cone的规则。用户manifest不能声明、覆盖或用path伪造该coordinate；`scoop`只从当前sysroot的trusted core slot注入direct edge，`scoopc`再验证显式传入的core artifact确实来自该slot；
- `scoop:single-file:0.0.0`同样是reserved coordinate，只能由第1.3节的typed `SingleFileRoot`产生。用户manifest不能声明该coordinate，dependency locator不能指向该artifact，也不能因artifact自报同coordinate而获得single-file身份；
- v1没有dev/build dependency、feature、platform条件、dependency alias或native link option。native library仍由已经验证的FFI declaration进入LIR/link metadata；
- executable不能作为另一个Cone的dependency。build root可以是library或executable：root为executable时graph中恰有一个executable且只能是root；root为library时graph中没有executable。library产物不含program descriptor或C `main`；
- manifest自身不是源码语义表达式。locator、graph与cache错误由`scoop`给出manifest path和TOML span；当前Cone coordinate/kind/dependency声明错误由`scoopc`给出同类诊断，二者都不伪造成HIR file index诊断。

### 1.3 source discovery、single-file root、package与entry

- `scoopc`只为当前Cone递归收集`src/`下扩展名精确为`.scoop`的regular file，以标准化Cone-relative `/`路径按UTF-8 byte顺序排序；空source set、非UTF-8路径、两个路径归一化到同一identity、或symlink逃出Cone root均为single-Cone compiler错误；
- `scoop`的root operand是封闭sum `BuildRootInput::ManifestCone | SingleFile`。operand basename的扩展名精确为`.scoop`且跟随symlink后的目标是已存在regular file时选择`SingleFile`；symlink cycle、dangling link或最终目标非regular file稳定失败，resolved host path仍只作locator。即使该文件所在目录含`Cone.toml`或其他source，也不能切换为manifest mode或发现旁文件。目录operand要求其下存在`Cone.toml`，regular file operand只有精确命名的`Cone.toml`或满足上述`.scoop`规则时合法；不存在、错类型或其他扩展名不作启发式回退；
- `SingleFile`构造固定semantic projection：`ConeCoordinate = scoop:single-file:0.0.0`、`ConeOutputKind::Executable`、source set恰好为该文件且logical path固定为`main.scoop`、direct Cone dependency恰好为trusted `scoop:scoop.core:0.1.0`。它不接受dependency locator、`--direct-slib`、`--support-slib`或相邻source discovery，源码中对非core Cone的import按普通不可达诊断；
- single-file `.slib`是local executable-root artifact，可进入content-addressed cache，并可由产生它的`scoop build/run`或显式`scoop link --root-slib`作为唯一root消费；它不能作为可分发artifact发布、作为另一Cone的dependency或由manifest `artifact` locator引用。一个resolved graph中最多有一个该synthetic root；缓存键除固定projection外必须包含source content digest、core三层fingerprint、compiler/schema/target/toolchain身份，因而不同文件不会因共用Cone identity而命中同一artifact；
- host绝对路径、调用时的相对/绝对/symlink spelling、目录枚举顺序、inode、mtime和canonicalized临时目录不进入任何semantic identity。manifest Cone的source path identity是`(ConeIdentity, normalized Cone-relative path)`；single-file root使用相同pair形态，但第一项固定为由reserved `scoop:single-file:0.0.0`计算的`ConeIdentity`，第二项固定为`main.scoop`。当次CLI可用用户输入路径装饰当前source诊断，但该display locator不写入`.slib`、persistent id、`SourceLocation.file`或cache key；
- 文件路径不决定`package`。一个文件可声明任意合法package，省略则属于root package；跨package引用必须按第2章规则导入或限定；
- library Cone不要求entry。它可以声明名为`main`的普通函数，但不获得entry linkage；pipeline各层以封闭的`ConeOutputKind::Library | Executable { local_entry }`表示产物种类，不能用`Option<Entry>`让library/executable与entry presence形成可矛盾状态；
- executable root必须恰有一个top-level ordinary、non-generic、non-suspend、无参数、返回`Unit`的`main`。只在root Cone中发现，dependency中的同名声明不参与竞争；`Executable`分支的`local_entry`非可选且entry可保持internal；
- root实际entry也使用普通namespaced stable symbol。最终program descriptor保存typed code pointer，runtime不再调用固定`scoop_main`符号。

现有单文件fixture不需要为每个历史目录手写manifest：runner必须调用正式`scoop` orchestration的`SingleFile`分支，不能直接调用`scoopc` pipeline library或使用仅测试可见的第二套synthetic manifest。测试allowlist仍只能用于intrinsic authority单元测试，不会赋予single-file root额外依赖或authority。

### 1.4 resolved DAG与缓存输入

`scoop`先把root operand解析为`BuildRootInput::ManifestCone | SingleFile`。前者只读解析全部source manifest与prebuilt `.slib`的bounded manifest summary；后者构造第1.3节固定的synthetic source node，并与trusted core组成恰好两个节点的graph，不运行locator发现。随后统一建立以`ConeIdentity`为key的`ResolvedBuildGraph`。manifest summary或synthetic root projection只是locator/调度输入，不是`ValidatedGraphArtifact`，不能授予metadata或link API，也不能据此把artifact标成可复用、可发布或可交给下游：

1. 仅对非core Cone注入trusted `scoop:scoop.core:0.1.0` direct edge；core bootstrap root不依赖自身；
2. 验证coordinate/content唯一、dependency kind、exact version与target/schema兼容；
3. 以typed DFS/SCC检测cycle并打印完整coordinate edge path；
4. 验证同一`group:name`没有多个version；
5. 得到唯一canonical topological order：初始及每轮ready set精确定义为“全部direct dependency都已输出”的剩余Cone，从中按coordinate byte order取最小者；因此dependency总在dependent之前，不依赖edge在实现中的存储方向；
6. 才按该顺序处理每个节点：prebuilt/cache候选必须先完整读取全部payload并分别成功构造Compile与Link view，之后才可复用；source节点在其全部上游artifact已经完成同一双view门禁后，以一次独立`scoopc build`调用生成且只生成当前Cone的`.slib`，父进程对输出再次执行同一门禁后才将节点标为完成。任一调用都不得收到尚未完成的source Cone，也不得通过进程内IR绕过`.slib`边界。

一个`.slib`的dependency table记录其编译时每个direct dependency的`ConeIdentity`与三层semantic fingerprint。`scoop`在调度前据此决定source节点的cache命中或重编译；只有prebuilt artifact时报告stale dependency。`scoopc`收到的上游`.slib`集合若缺失、含不可到达的额外artifact或fingerprint不匹配，只报告single-Cone输入错误，不自行解析locator或重编译上游，因而旧typed id不可能被接到新metadata。

M23的缓存归`scoop`所有，key至少包含：normalized manifest semantic fields、全部source content digest、compiler language/schema/runtime ABI、target profile fingerprint及当前Cone实际消费的三层**Merkle semantic fingerprint**。每层fingerprint除本Cone canonical section外，还按typed support/re-export edge纳入对应origin artifact的同层Merkle fingerprint；因此`C -> A(re-export B)`即使A自己的binding bytes未变，B的相关HIR/MIR/LIR变化也会沿A传播到C。`scoopc`不读写全局artifact cache；它只把指定输出写到临时路径、round-trip验证并原子发布，是否复用该结果由调用方决定。

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
- import selector与qualified type path都先取能形成可见package binding的最长前缀，再沿static nested nominal/object/companion scope走typed owner edge；最长package前缀一旦选定便不回退到较短前缀重猜。exact import终点必须是importable binding，star import终点必须是importable namespace；两者都不把点连接的字符串直接当FQN查全图；
- v1的value/function表达式仍通过import后的短名或普通receiver语法访问顶层实体，不新增dependency-coordinate-qualified表达式语法。

### 2.2 import可到达性与origin

普通import selector只能解析：

1. 当前Cone中按visibility允许该file访问的声明；
2. manifest的direct dependency公开binding；
3. direct dependency公开binding中已经解析好的re-export target；
4. trusted core dependency的公开/prelude binding。

`scoopc`虽然从显式输入的上游`.slib`闭包加载transitive support metadata，但未被direct dependency re-export的transitive public surface不会自动成为源码候选；link stage则单独消费同一闭包中的link input。因而“artifact已经提供”或“metadata已在内存中”都不是源码可见性证明。

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
PersistentGenericTypeId
PersistentFunctionId
PersistentGenericFunctionId
PersistentCallableApplicationId
PersistentConstructorId
PersistentPropertyId
PersistentExtensionPropertyId
PersistentPropertyAccessorId
PersistentObjectValueId
PersistentTypeAliasId
PersistentFieldId
PersistentEnumVariantId
PersistentEnumVariantFieldId
PersistentGeneratedCallableId
PersistentSourceContextId
PersistentExportBindingId
PersistentLocalBindingId
PersistentLocalValueId
PersistentCallbackRegistrationId
PersistentCallbackApplicationId
PersistentSourceNativeExternalContractId
PersistentNativeExternalSymbolId
NativeLinkRequirementId
PersistentDispatchSlotId
PersistentInitializationUnitId
PersistentLayoutId
PersistentScanId
PersistentDispatchTableId
PersistentExactTypeId
PersistentCallableBodyId
PersistentStaticStorageId
PersistentImmortalObjectId
PersistentSafepointSiteId
GeneratedBridgeUnitId
GeneratedBridgeAtomId
ObjectDefinitionPlanId
ObjectDefinitionAtomId
OdrGroupId
OdrMemberId
```

不能定义一个可随意cast的`PersistentEntityId([u8; 32])`公共API。wire上即使payload同为32字节，field tag与decoder目标kind也必须匹配。

除下文明确使用runtime encoder的callable body外，persistent id都由`DomainSeparatedCborHash(kind domain, definition key)`产生；helper负责唯一的Wire-CBOR编码。definition key至少含：origin `ConeIdentity`、package、typed owner chain、实体kind、源码名称以及语言duplicate-declaration规则采用的normalized signature key。signature使用alias展开后的persistent type refs与binder位置，不含body、default表达式、visibility、arena ordinal、source枚举顺序或link symbol。改变body保持声明identity但会改变semantic/code fingerprint；改变合法重载签名产生新identity。

generic/owner-dependent callable的declaration、concrete application与machine body是三个不同实体：

```text
CallableTemplateOrigin =
    Function(PersistentFunctionId)                 // tag 1
  | GenericFunction(PersistentGenericFunctionId)   // tag 2
  | Constructor(PersistentConstructorId)           // tag 3
  | Accessor(PersistentPropertyAccessorId)          // tag 4
  | VariantConstructor(PersistentEnumVariantId)    // tag 5

CallableInstantiationOwner =
    NoOwner                                          // tag 1
  | ExactNominalOwner(PersistentExactTypeId)         // tag 2
  | EnclosingCallableApplication(PersistentCallableApplicationId) // tag 3
  | EnclosingInitializationApplication(PersistentInitializationUnitId) // tag 4

CallableArguments = NoCallableArguments            // tag 1
                    | Arguments(NonEmpty<PersistentExactTypeId>) // tag 2

CallableApplicationKey = {
    origin: CallableTemplateOrigin,
    instantiation_owner: CallableInstantiationOwner,
    callable_arguments: CallableArguments,
}

PersistentCallableApplicationId =
    DomainSeparatedCborHash("scoop-callable-application-id-v1",
                            CallableApplicationKey)
```

ordinary method/nominal constructor/enum variant constructor/accessor处于generic nominal时使用其声明宿主的`ExactNominalOwner + NoCallableArguments`；generic method再带自身Arguments；top-level generic function使用NoOwner+Arguments；generic extension-property accessor以receiver binder arguments填Arguments。local callable从普通callable lexical parent继承substitution时使用`EnclosingCallableApplication`；若其nearest enclosing materialization是generic delegated initializer/ensure（包括其中lambda/anonymous callable），则使用`EnclosingInitializationApplication`，且unit必须是同一extension property与完整receiver arguments的`GenericDelegatedExtensionApplication`。local callable若自身generic还要同时带自身Arguments，不能让outer owner吞掉phantom binder。两项都不存在时不构造application，直接使用declaration id。validator沿source/generated lexical parent逐边证明tag 3/4恰好是nearest enclosing materialization；owner不是调用点receiver或动态派生类型。所有arity包含phantom binder且argument必须是concrete exact type；application id与declaration/body/unit id之间没有cast。

initializer/ensure的generated **template** identity始终由声明级unit固定：只接受`TopLevelProperty`、`ExtensionProperty`、`Object`或`Companion`；generic delegated template仍使用声明级`ExtensionProperty` unit取得唯一template id。具体`GenericDelegatedExtensionApplication` unit只进入`CallableMaterializationContext::InitializationApplication`，实际initializer/ensure implementation以该context的`MaterializationRoot`求Cone/ODR root。不得为每个receiver application重造一份generated template，也不得只读template key中的声明级unit决定具体body归组。

foreign callback同样把source conversion site与concrete materialization分成两个identity：

```text
CallbackRegistrationKey = {
    parent: LexicalCallableParent,                // field 1
    path: StructuralDefinitionPath,               // field 2
    source_c_signature: SourceCAbiFunctionSignature, // field 3
    context_index: CallbackParameterIndex,          // field 4
    managed_shape: SignatureCallableShape,        // field 5
    mode: CallbackMode,                           // field 6
}

PersistentCallbackRegistrationId =
    DomainSeparatedCborHash("scoop-callback-registration-id-v1",
                            CallbackRegistrationKey)

CallableMaterializationContext =
    NoSubstitution                                      // tag 1
  | Application(PersistentCallableApplicationId)       // tag 2
  | InitializationApplication(PersistentInitializationUnitId) // tag 3

CallbackApplicationKey = {
    registration: PersistentCallbackRegistrationId,    // field 1
    context: CallableMaterializationContext,          // field 2
}

PersistentCallbackApplicationId =
    DomainSeparatedCborHash("scoop-callback-application-id-v1",
                            CallbackApplicationKey)
```

HIR registration的signature/managed shape允许合法binder，只标识一次source conversion site；MIR application必须按context完成全部替换。`NoSubstitution`只接binder-free site，普通generic callable使用能回溯registration parent的`Application`，generic delegated initializer/ensure使用同property与完整receiver arguments的`InitializationApplication`。MIR以application为主键保存exact managed signature、mode及固定`{closure, result storage, roots, throwable} -> u32 status` storage ABI；LIR再把该application的source C signature按`ValidatedLirTargetSelectionV1`正规化，并保存`{application, CanonicalCAbiSignatureFingerprint, GeneratedBridgeUnitId}`。不同application可以复用同一个`CallbackTrampoline(signature,index)` unit，不得复制unit identity或把registration误作concrete application。

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

PersistentExactTypeId =
    DomainSeparatedCborHash("scoop-exact-type-v1", ExactTypeKey)
```

primitive、`Unit`及其他core nominal type走`Nominal`；`T?`先按语言规则脱糖成core `Option<T>` application；non-generic typealias先透明展开，alias identity不进入exact type key。`RawPointer`专门表示13.10的`Ptr<T>`，不能伪装成普通core nominal application；`NativeFunctionPointer`专门表示`FunPtr<F>`的native code pointer，不能与managed `Function`混用。后者v1的`NativeCallingConvention`封闭为`C`，参数/结果是完成C-FFI-safe验证后的exact type，ordinary/non-suspend约束在构造该key前检查；未来增加calling convention必须新增稳定tag而不能复用display string。template中的binder ref不是exact type，仍以带binder位置的signature type表达；只有替换完成的concrete type才能产生`PersistentExactTypeId`。tuple与managed function是结构类型；pointer两种variant则保留其不同provenance/ABI。相同有序结构跨Cone得到同一identity，不同形状或不同pointer/function category即使当前target layout相同也不能合并。wire保存按dependency-first顺序编码的`ExactTypeRecord { id, key }`，reader在把nominal ref视为叶节点后验证结构边无环并重算id；不能用递归字符串hash或遍历插入顺序决定结果。

`TypeDescriptor.diagnostic_name`不采用源码、import/re-export alias或consumer本地display spelling，而由已验证的`ExactTypeRecord`通过唯一的`CanonicalExactTypeDiagnosticName` printer产生。printer只输出ASCII，先定义`esc(UTF-8 bytes)`：`[A-Za-z0-9._-]`原样保留，其余每个byte（包括`%`）写成`%HH`大写十六进制。nominal declaration atom固定写为`n(c=<esc(canonical ConeCoordinate)>;p=<esc(package)>;o=<owner>;k=<kind>;x=<esc(source name)>)`；`kind`是封闭ASCII tag `C=class | I=interface | S=struct | E=enum | O=object/companion | A=annotation class`，typealias没有tag因为进入printer前已展开。空owner写`-`，非空typed owner chain按从外到内的`<kind>:<esc(source name)>`以`/`连接并使用同一tag表。它只读取声明的canonical origin coordinate/package/typed owner/name，不读取使用点qualifier、alias路径或可读mangle。递归variant grammar固定为：`Nominal = <declaration atom>`、`NominalApplication = a(<origin atom>;[<D(arg)>,...])`、`Tuple = t([<D(element)>,...])`、ordinary/suspend managed function分别为`f(o;[<D(param)>,...]-><D(result)>)`/`f(s;[...]->...)`、`RawPointer = r(<D(pointee)>)`、C native function pointer为`x(c;[<D(param)>,...]-><D(result)>)`；`,`、`;`、`[]`、`()`和`->`都是字面ASCII delimiter，元素顺序不变，空parameter list写`[]`。这里`D`按dependency-first exact-type graph递归；transparent alias在进入key/printer前已展开。producer/reader在分配或写出字符串前，以memoized checked subtree byte cost按每条展开路径重计共享type DAG，并复用4.5单个text/bytes语义字段16 MiB上限；cycle、算术溢出或超过上限均拒绝，不能先递归展开再检查长度。reader必须从key重算这些UTF-8 bytes，并拒绝同一`PersistentExactTypeId`对应不同bytes。

上段`n(...)`只适用于拥有source declaration atom的nominal。compiler-generated nominal不能伪造source name，固定写为`g(r=<8位小写十六进制role tag>;i=<64位小写十六进制PersistentTypeId>)`。v1 `GeneratedNominalRole`冻结为`ClosureEnvironment=1, CallableAdapterEnvironment=2, CoroutineFrame=3, ContinuationAdapterEnvironment=4, CoroutineStep=5, BoxedValue=6, CoroutineSlot=7, ObjectBackingClass=8`；stable structural definition path及callable/exact owner已经进入该`PersistentTypeId`的canonical key，printer不得读取arena ordinal、临时name或mangle，新增role必须扩展identity schema。递归grammar中的`Nominal`与`NominalApplication.origin`接受且只接受上述source/generated atom两分支。

`PersistentLayoutId`由exact type、target ABI profile和封闭representation role派生，只标识target-specific layout record；它不是language type identity。`PersistentStaticStorageId`由owner declaration或specialization identity加封闭storage role派生，`PersistentImmortalObjectId`由owner declaration/specialization、stable definition path与object role派生，`PersistentSafepointSiteId`由concrete callable identity加CFG site role/ordinal派生。它们分别用于layout bridge、static address、immortal range与stackmap site，互不转换；constant内容进入definition fingerprint而不替换owner identity。TypeDescriptor本身直接以`PersistentExactTypeId`作为语义登记key，再由strong/ODR member id标识具体record，不另设会与exact type产生双重真相的`PersistentTypeDescriptorId`。

每个由LIR定义、在当前Cone某个linkable object成员中实际发射的Scoop callable body另有统一但不擦除来源的owner identity，供entry pointer、stackmap与safepoint site共同引用：

```text
CallableBodyKey = Strong { owner: StrongCallableDefinitionOwner }
                | Odr { member: CallableOdrMemberId }
                | RootGateway { root_cone: ConeIdentity,
                                main: MainCallableBodyId }
                | InitializationStartupGateway {
                      unit: PersistentInitializationUnitId
                  }

PersistentCallableBodyId =
    SHA-256(ByteSpan("scoop-callable-body-v1") ||
            RuntimeEncode(CallableBodyKey))
```

`RuntimeEncode`就是6.1的runtime metadata canonical encoder：variant tag为小端`u32`、product按声明序编码，绝不是CBOR。v1变体严格冻结为：`Strong=1`，payload只有`owner`；`Odr=2`，payload只有`member`；`RootGateway=3`，payload依次为`root_cone, main`；`InitializationStartupGateway=4`，payload只有`unit`。不得按Rust enum ordinal、字段名、display string或host布局编码；unknown/reserved tag一律拒绝。

`StrongCallableDefinitionOwner`与`CallableOdrMemberId`是只接受callable atom的typed refinement，不能塞入storage/TD member；`MainCallableBodyId`则只接受root Cone中top-level ordinary/non-generic/non-suspend `() -> Unit` main的Strong body id。source/generated initializer/ensure等普通body使用前两种，两个C-callable no-throw gateway必须使用后两种专门variant，不能伪装成被调用的main/ensure。所有C record中的`*_callable_id`与safepoint的`owner_callable_id`均精确表示`PersistentCallableBodyId`；对应identity record保存完整tagged key供reader/link verifier重算。gateway primary symbol由该body id的专用mangler kind产生；root gateway始终`ConeStrong`，init startup gateway的strong/ODR linkage及group/member归属与其unit一致。这样runtime可直接用root record暴露的main body id重算gateway id，而无需反推出source function id；gateway内部managed call及异常物化路径上的每个site都以`{ body id, typed site role, stable local ordinal }`产生自己的`PersistentSafepointSiteId`，不会与main、ensure或另一Cone的wrapper共享编号。

这里的注册全集精确等于LIR的`RegisteredCallableBody`集合：包含当前Cone linkable object成员中的普通managed/NoGC Scoop body、compiler-generated **managed adapter/内部Scoop thunk**及root/init gateway，不包含只有声明而没有本Cone body的`@Extern` target、`ValidatedRuntimeArtifact`中的runtime函数，也不包含generated-C bridge/callback trampoline或未来C/C++ translation unit body。后三类分别使用extern/native-member/GeneratedBridge typed identity与对应object verifier；尤其generated-C trampoline是`GeneratedBridgeUnitId` recipe及producer-specific atom，不获得`PersistentCallableBodyId`，也不进入callable table。因此“每个body登记”不要求在`.slib`封装后回头patch其他已经验证的成员。

凡concrete exact type进入当前Cone LIR的layout/type closure，或作为param-free exported LIR bridge，就属于**runtime-materialized type**并必须生成TypeDescriptor registration；只存在于尚未替换的Export HIR template/binder中的type尚不materialize。tuple、managed function、raw/native pointer等非nominal exact type以其`PersistentExactTypeId`建立3.4的`StructuralType` ODR group，多个Cone重复materialize时整体coalesce。因而每个最终程序中materialized exact type恰有一个TypeDescriptor地址，而纯template type不会为了“可能未来使用”提前生成伪descriptor；是否materialize不改变语言type identity。

### 3.2 session-local remap

serialized table index只是wire压缩索引，不是semantic id。reader分两阶段：

1. 验证所有identity record并按kind建立`Persistent*Id → Imported*Id`映射；
2. 分配consumer semantic world的fresh typed arena id，再解析全部edge、body、bound、origin与witness。

同一origin从钻石路径再次出现时复用已有world id，并要求schema payload/semantic fingerprint一致；不同origin永不因同FQN或相同结构intern成一个declaration。current Cone source实体、imported export实体和LocalConcrete实体仍使用互不兼容id家族；从imported template到local specialization必须通过显式`ConcretizationOrigin` relation。

`IntrinsicProviderId`在M23后退为compile-session authority slot，不再承担持久Cone身份。`scoopc`从调用方提供且经trusted sysroot slot验证的core artifact把其`ConeIdentity`映射到`IntrinsicAuthority::Core`；测试allowlist仍是内部typed capability。普通artifact不能通过写同coordinate、provider整数或manifest flag取得intrinsic authority。

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

PersistentV1的21个kind对linkage是穷尽关系，不允许实现按visibility或symbol spelling自行推断。下表中C/H/O分别为`ConeStrong`/`TemplateSupportHidden`/`OdrWeak`；`RuntimeAbi`对全部行非法：

| kind | typed owner/root | allowed | ODR时唯一member/primary约束 |
| --- | --- | --- | --- |
| `cb` | callable body | C/H/O | body自身member；generic startup为CallableBody/InitializationUnit；M24 hook为ReleaseHook/ExactType |
| `ss` | static storage | C/H/O | StaticStorage/StaticStorage |
| `io` | immortal object | C/H/O | ImmortalObject/ImmortalObject |
| `td` | exact type | C/H/O | TypeDescriptor/ExactType |
| `ly` | layout的exact type | C/H/O | Layout/Layout |
| `sp` | scan的layout root | C/H/O | ScanProgram/Scan |
| `dt` | table implementor exact type | C/H/O | DispatchTable/DispatchTable |
| `ds` | dispatch declaration origin Cone | C/H | 只表示slot anchor，implementation thunk走`cb` |
| `ic` | initialization unit | C/H/O | InitializationCell/InitializationUnit |
| `id` | initialization unit | C/H/O | InitializationDescriptor/InitializationUnit |
| `rr` | root storage | C/H/O | RegistrationRecord/StaticStorage |
| `ir` | immortal object | C/H/O | RegistrationRecord/ImmortalObject |
| `nr` | initialization unit | C/H/O | RegistrationRecord/InitializationUnit |
| `tr` | exact type | C/H/O | RegistrationRecord/ExactType |
| `sr` | safepoint owner body | C/H/O | RegistrationRecord/SafepointSite且site属于同body |
| `cr` | callable body | C/H/O | RegistrationRecord/CallableBody |
| `im` | key中的Cone | C | producer必须是artifact Cone |
| `br` | atom key中的producer Cone | C | 只接可物理materialize的atom tag 1…3 |
| `od` | ODR member | O | 只接没有kind-specific primary的member |
| `bs` | ObjectDefinitionPlan root | C/H/O | 精确继承plan primary |
| `be` | 与同atom的`bs`相同 | C/H/O | linkage/root必须与`bs`逐项相同 |

Cone-owned subject默认只能证明C。M23-2 foundation拒绝H且不可发布，M23-3 strong-only production profile拒绝H/O，M23-5只建立visibility/access eligibility；直到M23-7同时验证实际template body的support reachability与整个subject的LIR/object派生闭包，才可把`cb/ss/io/td/ly/sp/dt/ds/ic/id/rr/ir/nr/tr/sr/cr/bs/be`整组收窄为H。root gateway、image、bridge永远不能hidden；initialization startup继承unit，M24 param-free hook继承exact source subject。Odr-owned只能为O，Cone-owned不能为O。

### 3.4 specialization、layout与ODR group

program-wide specialization key以封闭sum区分被实例化的语义实体：

```text
SpecializationKey = Nominal { origin: PersistentGenericTypeId, exact arguments }
                  | Callable { application: CallableApplicationKey }
                  | DelegatedProperty { origin: PersistentExtensionPropertyId,
                                        exact receiver arguments }
                  | StructuralType { exact_type: PersistentExactTypeId }
OdrGroupId = DomainSeparatedCborHash("scoop-odr-v1", SpecializationKey)
OdrMemberKey = { group: OdrGroupId,
                   role: OdrMemberRole,
                   discriminator: OdrMemberDiscriminator }
OdrMemberId = DomainSeparatedCborHash("scoop-odr-member-v1", OdrMemberKey)
```

四个Specialization variant的wire tag固定为1…4。`OdrMemberRole`的tag 1…16依次是`CallableBody, GeneratedNominal, Layout, ScanProgram, TypeDescriptor, DispatchTable, DispatchAdapter, StaticStorage, ImmortalObject, InitializationCell, InitializationDescriptor, RegistrationRecord, DiagnosticBytes, AddressTakenConstant, ObjectSupport, ReleaseHook`；tag 16已经属于identity schema v1，但M23 outer schema/profile v1不得产生或消费其semantic member，只有M24 schema/profile v2启用。member discriminator与完整role矩阵由M23-2详细设计冻结。所有argument/application必须是persistent exact type identity；没有owner或没有callable arguments由第3.1节typed sum表达，不能靠空Vec猜实体类别。member role标识group内member而不进入group key；否则function、TypeDescriptor和storage会被错误拆成互不关联的多个“组”。validator还必须从每个discriminator的canonical key沿typed owner/application关系回溯到当前group root：Callable application逐字段等于group key，nominal exact application、delegated-property unit和structural exact type分别是其variant唯一root；合法role不能搭配任意别组typed id。producer Cone、symbol、object分片或相同layout都不构成provenance。

- param-free source-anchored type/callable默认由其定义Cone以`ConeStrong`发射，layout/dispatch/TypeDescriptor从同一个exact/source subject继承；只有M23-7同时取得实际Export template-body reachability与LIR/object派生闭包证明后，整个Cone-owned subject才可统一收窄为`TemplateSupportHidden`。`param-free`本身不是Cone ownership证明：`FunctionShape`等真正非nominal exact shape才进入`StructuralType`组；box/coroutine step、slot、shell/start等以`ExactOwnerRoot`求根，source nominal回到定义Cone，nominal application进入`Nominal`组，只有非nominal exact owner才进入`StructuralType`。adapter再按其冻结的source/target shape root规则判定，不能把所有compiler helper一概归为Structural；
- 上游generic template与下游local type组成的新application在**实际消费Cone**完成HIR concretization、layout、MIR/LIR与发射。定义Cone不可能预计算`Upstream<DownstreamLocal>`，因此“layout只在定义Cone计算一次”只适用于param-free实体；
- 多个Cone产生相同specialization时，各自发射同一`OdrGroupId`，每个同role member具有相同`OdrMemberId`、symbol和`OdrWeak`属性。nominal group包含它产生的layout、TypeDescriptor、vtable/itable、scan与相关box/adjust member；callable group包含body及它产生的closure/coroutine/adapter；delegated-property group包含storage/root/cell/failure/init/ensure/descriptor；structural-type group包含materialized layout、scan、box与TypeDescriptor。任何specialization-owned的runtime registration record、address-taken string/constant、diagnostic bytes或其他relocation target都必须是同组的显式member，不能指向consumer-local private定义，也不增加未列入`SpecializationKey`封闭sum的“content group”旁路。唯一例外是第3.4/4.1节generated bridge：canonical relation只引用共享unit recipe，实际object relocation可指向当前producer ConeStrong primary atom，但object verifier必须再规范化回相同unit，atom id绝不进入ODR fingerprint。group可以typed-ref引用另一个由既有封闭variant产生的canonical group，但不能把自身产生的runtime identity漏在组外，尤其不能只coalesce函数而留下两个TypeDescriptor、registration或static storage；
- Darwin/Mach-O按每个`OdrMemberId`发射同名`linkonce_odr`/`weak_odr` coalesced symbol，并不假设Mach-O存在原子COMDAT group；支持COMDAT的未来target可以把同组member放入COMDAT。两条路径都先验证完整member set与definition fingerprint，使native linker逐member任选winner也只能得到等价的一组定义；
- 每个producer在`.slib` manifest记录`OdrRecord { group, members, abi_fingerprint, definition_fingerprint }`。`OdrAbiFingerprint = DomainSeparatedCborHash("scoop-odr-abi-v1", {1=member_abi_shapes_sorted_by_member_id})`；每个受检definition同时由LIR meta保存`CanonicalLirDefinition { owner, canonical payload }`，reader以`LirDefinitionFingerprint = DomainSeparatedCborHash("scoop-lir-definition-v1", CanonicalLirDefinition)`重算。最终ODR digest本身不进入该payload或LIR semantic own-layer，避免反向自引用。`OdrDefinitionFingerprint = DomainSeparatedCborHash("scoop-odr-definition-v1", {1=group, 2=按OdrMemberId排序的{role, LirDefinitionFingerprint, 按DigestNodeId排序的{ObjectDefinition node id, fingerprint}, 按PersistentSafepointSiteId排序的{site id, StackmapRecordFingerprint}}})`；没有对应leaf的列表使用typed empty variant，不能靠遗漏字段表示。`SlibMemberId`与range/offset只负责在当前artifact中定位和验证这些leaf，绝不进入跨Cone ODR内容或排序；否则同一specialization仅因producer Cone或物理分片不同就无法coalesce。ABI fingerprint只用于接口诊断；link前必须比较完整member set与definition fingerprint，只有ABI相同而body、initializer、constant、scan/vtable、stackmap或relocation不同仍必须失败；
- actual-object范围不能只覆盖ODR，也不能默认全部definition位于同一个object，否则strong root/init gateway、param-free TypeDescriptor和strong registration会退回“相信LIR”。`CurrentConeLir`非可选地携带member-independent `ObjectDefinitionPlan`；plan覆盖每个参与派生fingerprint的strong definition和每个ODR member，却不决定物理object分片。codegen/native producer另输出`MemberMaterialization { plan_id, member: SlibMemberId, emitted boundary symbols }`，把每个plan恰好绑定到一个`LinkObject`；这个relation属于object verification projection而非LIR semantic projection。Mach-O `nlist`没有可靠symbol size，因此producer为每个primary atom及address-taken constant/registration发射plan指定的stable start/end boundary symbol，设置`MH_SUBSECTIONS_VIA_SYMBOLS`，形成不重叠named subsection atom。object verifier逐个解析真实symbol/section/relocation table并产生`VerifiedObjectDefinitionIndex { member, DefinitionAtomRange, associated records, ObjectDefinitionFingerprint... }`；manifest保存materialization relation及按member分区的全部definition range，`OdrRecord`只引用其中owner为该组ODR member的精确子集，artifact reader再从对应object独立重算。range不能跨object member；每个member内要求边界同section、严格有序、padding按规范全零且归属于前一atom，并拒绝未被任何range归属的受检relocation；
- LIR另输出member-independent `DigestFinalizationPlan { nodes }`，每个`DigestNode { typed node id, DigestKind, canonical direct inputs, patch intents }`。`DigestKind`封闭为`SourceSignature`、`Layout`、`Scan`、`LirDefinition`、`ObjectSupport`、`ObjectDefinition`、`StackmapRecord`、`OdrDefinition`、`StrongRegistration`、`RuntimeImage`；前四类从canonical LIR metadata产生语义leaf，允许没有object落槽，其余leaf来自声明的object range或canonical runtime record。`DigestInputRef`也是封闭sum，只能引用上述typed node id，不能把裸digest、object offset或“当前已算出的值”当依赖。`DigestPatchIntent`非可选指明typed intent id、target definition owner/atom role、固定width 32与唯一source node，不含member或offset；producer的materialization relation再产生`MaterializedPatchSite { intent_id, member: SlibMemberId, checked_offset }`。即使digest源atom与落槽descriptor属于不同owner，也通过这两层typed relation关联而不靠裸offset；
- patch intent的identity精确为`DigestPatchIntentKeyV1 {1=source: DigestNodeId, 2=target_owner: ObjectDefinitionPlanOwner, 3=atom_role: DefinitionAtomRole, 4=semantic_field_role: DigestSemanticFieldRoleV1}`与`DigestPatchIntentId = DomainSeparatedCborHash("scoop-digest-patch-intent-id-v1", DigestPatchIntentKeyV1)`。plan/intent/atom的role都是封闭tag，且每个owner/field组合唯一；boundary symbol只由plan id与start/end/associated role派生。以上identity均不含`SlibMemberId`、object ordinal、section number或offset，所以重新分片只改变materialization relation与code/artifact验证面；重复/缺失plan或intent、一个intent对应多个site、一个physical site被多个intent认领都在finalization前拒绝；
- 允许边按kind封闭：四种semantic leaf与object support无依赖；stackmap可依赖owner的source signature与object support；object definition可依赖其semantic leaf、associated stackmap及object support；ODR definition依赖完整member set的LIR definition、object definition与stackmap leaf；strong registration依赖自己的LIR/layout/scan/source、object definition及适用的stackmap leaf；runtime image必须显式依赖其每个canonical record key中出现的definition/layout/scan/descriptor/gateway/stackmap digest。交叉record仍只引用对方typed registration identity，除非其digest确实作为本record字段出现。peer、descendant、反向边或kind不允许的shortcut一律非法；
- digest node自身也有可重算的persistent identity：`DigestKind`按上段顺序冻结为`1..10`，key精确为`DigestNodeKeyV1 {1=kind: DigestKind, 2=owner_and_role: DigestOwnerAndRoleKeyV1}`，`DigestNodeId = DomainSeparatedCborHash("scoop-digest-node-id-v1", DigestNodeKeyV1)`。owner-and-role key使用LIR wire的persistent typed owner与封闭role，不含arena id、patch offset或任何digest值。一个node的direct input sequence按`(DigestKind tag, DigestNodeId bytes)`严格递增且无重复，每项编码`{ u32 kind, DigestNodeId[32], digest[32] }`并带`u64 count`；plan缺少必需input、增加kind不允许的input或同id对应不同owner key均拒绝；
- strong registration的算法固定为`StrongRegistrationFingerprint = SHA-256(ByteSpan("scoop-strong-registration-v1") || RuntimeEncode(StrongRegistrationFingerprintInputV1))`，其中`StrongRegistrationFingerprintInputV1 { record: CanonicalStrongRecordSansOwnDefinition, direct_inputs: Sequence<StrongRegistrationDirectInputV1> }`按声明序编码，sequence带小端`u64` count，每项恰为`{ kind: u32, node_id: DigestNodeId[32], digest: Digest256[32] }`并按`(kind,node_id)`严格递增。`CanonicalStrongRecordSansOwnDefinition`恰为6.1完整`RuntimeImageRecordKey` sum：最外层sum tag作为record kind只写一次，variant payload中的registration不再重复kind；linkage必须为Strong、ODR group/member全零、当前registration的`definition_fingerprint`写32个零字节。其他source/layout/scan/descriptor/gateway/callable/stackmap digest字段保留已完成的直接依赖值，pointer按typed role/identity替换。它不读取`MaterializedPatchSite`、RuntimeImage或最终object地址。finalizer与reader从record、LIR plan和verified object index独立重建该输入；不得在完整variant bytes前再写第二份kind，只能把own definition slot归零，不能按遍历时机再归零已经声明的上游digest；
- digest计算是纯函数：每个node只读取其canonical source与声明的直接input digest；对object-backed source，`normalize(node, member, ranges)`只保留该node**传递依赖**所写且位于该成员受检range中的digest，node自身及所有非依赖（peer、descendant、无关image）patch slot一律归零，再规范化relocation槽。不能用“当前遍历时尚未计算”定义hash视图。`ProvisionalObjectSet`中全部native object成员的graph-managed digest slot初始为零；plan verifier在codegen前与各object产出后都验证node id/order canonical、allowed-edge DAG无环、每个object-resident digest槽恰有一个writer、所有应有槽被覆盖、patch的member/owner/range无误、槽无relocation且精确32 bytes。finalizer按任意合法topological order计算后一次性核对/回填对应成员；manifest保存可重放的typed graph，reader从canonical source与逐成员bytes重算，而不是信任历史写入顺序；
- per-Cone C字段与writer node固定一一映射：source/layout/scan字段分别由`SourceSignature`/`Layout`/`Scan`写；TypeDescriptor的`descriptor_fingerprint`及每条callable registration的`body_definition_fingerprint`由对应atom的`ObjectDefinition`写；root及**Eager** init的gateway definition字段不是独立承诺，而是同一gateway `ObjectDefinition` node的额外`DigestPatchIntent`及其`MaterializedPatchSite`，必须逐byte等于其callable registration的body字段。Lazy init的gateway body id/fingerprint是固定全零tagged encoding，不是graph-managed slot、没有node/patch intent/site；`normalized_stackmap_fingerprint`由对应`StackmapRecord`写；registration的`definition_fingerprint`由strong record的`StrongRegistration`或ODR record所属`OdrDefinition`写；image字段由`RuntimeImage`写。`LirDefinition`与`ObjectSupport`可以只存在verifier index而无C落槽。禁止为这些字段另造同名hash算法，且一个node写多个等价slot时所有slot最终byte必须相同；
- canonical relocation target是封闭sum：`NamedPersistentSymbol { kind, persistent/ODR/runtime id }`、`GeneratedBridgeUnit { id: GeneratedBridgeUnitId }`、`OwningAtomOffset { owner, checked offset }`、`AssociatedRecordOffset { owner, record role, checked offset }`、`SectionBasePair { canonical section role, associated owner/support id, checked addend }`。其中bridge是唯一例外：LIR canonical definition与ODR fingerprint只保存producer-independent unit；codegen把它映射到当前artifact Cone的`PrimaryEntry(unit)` atom，object verifier核对实际`br` relocation后再规范化回该unit。defined-symbol owner仍是producer-specific atom。其余后两项覆盖LLVM正常产生的section-relative compact-unwind与`SUBTRACTOR(section base) + UNSIGNED(function)` EH pair；reader把Mach-O composite relocation先解析成typed form，要求offset/addend落在已声明range内，绝不能hashsection number、temporary symbol或object-local index。AArch64 relocation pair作为一个有序typed relocation归一化，unknown或跨range pair直接失败；
- 任一function owner的associated-record闭包都必须覆盖与其语义绑定的`__gcc_except_tab` LSDA、`__eh_frame` FDE/CIE relation、`__compact_unwind`记录及LLVM stackmap function/callsite payload；记录按上述typed owner和relocation关联，而不是只hash`__text`。共享CIE等以`ObjectSupportFingerprint = SHA-256(ByteSpan("scoop-object-support-v1") || ObjectSupportRuntimeEncodeV1(role, normalized bytes/relocations))`成为support node；definition leaf使用domain`scoop-object-definition-v1`，stackmap leaf使用`scoop-stackmap-record-v1`并覆盖完整site/owner/v3 payload。三者的encoder都由其typed producer capability唯一指定，不能换成裸拼接或Wire CBOR。safepoint registration的normalized fingerprint必须等于对应stackmap node。object-level测试从真实Mach-O重算definition graph，并人工篡改strong/ODR primary atom、support与每类associated metadata证明finalization会失败；
- native linker完成coalesce后，最终artifact verifier确认每个TypeDescriptor identity只有一个地址、dispatch引用指向winner、无重复strong定义；runtime再做防御性验证；
- same source template但不同exact arguments、same arguments但不同template origin必须是不同group；同group的不同generated role必须是不同member而不是不同group。

`ObjectDefinitionPlan`的identity schema不能用“kind-specific id + role”自然语言代替，v1精确为：

```text
StrongDefinitionEntity =
    CallableBody(PersistentCallableBodyId)             // tag 1
  | StaticStorage(PersistentStaticStorageId)           // tag 2
  | ImmortalObject(PersistentImmortalObjectId)         // tag 3
  | ExactType(PersistentExactTypeId)                   // tag 4
  | Layout(PersistentLayoutId)                         // tag 5
  | Scan(PersistentScanId)                             // tag 6
  | DispatchTable(PersistentDispatchTableId)           // tag 7
  | DispatchSlot(PersistentDispatchSlotId)             // tag 8
  | InitializationUnit(PersistentInitializationUnitId) // tag 9
  | SafepointSite(PersistentSafepointSiteId)           // tag 10
  | ConeImage(ConeIdentity)                            // tag 11
  | GeneratedBridgeAtom(GeneratedBridgeAtomId)         // tag 12

StrongDefinitionRole =
    CallableBody=1 | StaticStorage=2 | ImmortalObject=3
  | TypeDescriptor=4 | Layout=5 | ScanProgram=6
  | DispatchTable=7 | DispatchSlot=8 | InitializationCell=9
  | InitializationDescriptor=10 | RootRegistration=11
  | ImmortalRegistration=12 | InitializationRegistration=13
  | TypeRegistration=14 | SafepointRegistration=15
  | CallableRegistration=16 | ImageDescriptor=17
  | GeneratedBridge=18

ObjectDefinitionPlanOwner =
    Strong { producer: ConeIdentity,
             entity: StrongDefinitionEntity }       // tag 1, fields 1..2
  | Odr { member: OdrMemberId }                        // tag 2, field 1

ObjectDefinitionPlanRole =
    Strong { role: StrongDefinitionRole }            // tag 1, field 1
  | OdrMemberPrimary                                   // tag 2

ObjectDefinitionPlanKey = {
    owner: ObjectDefinitionPlanOwner,                // field 1
    definition_role: ObjectDefinitionPlanRole,       // field 2
}

ObjectDefinitionPlanId =
    DomainSeparatedCborHash("scoop-object-definition-plan-v1",
                            ObjectDefinitionPlanKey)

ObjectDefinitionAtomKey = {
    plan: ObjectDefinitionPlanId,          // field 1
    role: DefinitionAtomRole,            // field 2
    subkey: DefinitionAtomSubkey,        // field 3
}

DefinitionAtomRole =
    Primary=1 | Lsda=2 | EhFrame=3 | CompactUnwind=4
  | Stackmap=5 | RuntimeRecord=6 | AddressTakenConstant=7

DefinitionAtomSubkey =
    Singleton                                            // tag 1
  | CallableBody(PersistentCallableBodyId)              // tag 2
  | StaticStorage(PersistentStaticStorageId)            // tag 3
  | ImmortalObject(PersistentImmortalObjectId)          // tag 4
  | InitializationUnit(PersistentInitializationUnitId)  // tag 5
  | ExactType(PersistentExactTypeId)                    // tag 6
  | SafepointSite(PersistentSafepointSiteId)            // tag 7
  | StructuralPath(StructuralDefinitionPath)          // tag 8

ObjectDefinitionAtomId =
    DomainSeparatedCborHash("scoop-object-definition-atom-v1",
                            ObjectDefinitionAtomKey)
```

合法矩阵为：CallableBody entity只接role 1/16，StaticStorage接2/11，ImmortalObject接3/12，ExactType接4/14，Layout/Scan/DispatchTable/DispatchSlot接5…8，InitializationUnit接9/10/13，SafepointSite接15，ConeImage接17，GeneratedBridgeAtom接18；后者只接受`PrimaryEntry`、`SignatureDescriptor`或`ContextDescriptor`，`StaticAssertSupport`没有物理plan。Strong owner只能配Strong role，ODR owner只能配`OdrMemberPrimary`。每个ODR member恰有一个primary，选择按kind固定：CallableBody/DispatchAdapter/ReleaseHook用`cb`，Layout用`ly`，ScanProgram用`sp`，TypeDescriptor用`td`，DispatchTable用`dt`，StaticStorage用`ss`，ImmortalObject用`io`，InitializationCell/Descriptor用`ic`/`id`，RegistrationRecord按discriminator唯一使用`rr/ir/nr/tr/sr/cr`。`od`只给GeneratedNominal及没有专用primary的DiagnosticBytes、AddressTakenConstant、ObjectSupport；同member不得再发`od` alias，避免native linker对两个名字分别选不同producer。

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
- `Array<Z>`/`MutableArray<Z>`的对象仍有独立ref identity、header和源码可见的`size: Long`；offset 16处的物理count仍是受`0..=INT64_MAX`约束的独立typed `u64` machine metadata，不把源码`Long`当作内部layout/count类型。data offset为`alignUp(24, alignOf<Z>())`，exact allocation size就等于该offset并满足element alignment，与logical length无关。`get`按receiver → index求值后做整数bounds check并产生typed ZST；`set`遵守普通call顺序，先求值receiver → index → RHS，再由intrinsic执行bounds check，成功后不写payload，因此越界也不能跳过RHS副作用/异常。assembly仍对各part计数并保留全部求值，clone/互转仍分配新array但不发`memcpy`；
- array iterator永久使用`{ array ref, Long index }`，以`index < size`终止并按1递增；不能使用`current += stride`或`current == end`，否则stride 0不前进。LIR/runtime对ZST或其他GC-free element把object scan直接规范化为`None`；非空array object scan使用自包含的`RefScan::Array { length_offset: 16, first_element_offset: data_offset, stride: NonZeroU64, element: NonEmptyRefScan }`，两个offset和stride均进入scan fingerprint，不能把over-aligned element的data offset默认为24。collector工作量不随ZST logical length增长；
- boxing ZST仍分配普通managed object：其TypeDescriptor使用`BoxedValue { inline_storage: ZeroSized }`，allocation采用6.1的对齐后`inline_offset/minimum_size`，side metadata登记精确非零object size，payload不占字节。LIR以`BoxPayload::{ZeroSized, NonZero { source_place }}`和对应`UnboxResult::{ZeroSized, NonZero { destination_place }}`封闭区分；ZST分支不制造/传递/解引用payload pointer，non-ZST的size/alignment/scan只从exact TD读取而不由callsite重复提交。NonZero的`source_place`必须是跨本次runtime call地址稳定、满足value alignment的caller temp；若TD的inline scan非空，caller必须先把完整value写入该temp，再用封闭`GeneratedRootFrameOp::PushRecursiveRegion`调用compiler-private `GeneratedNoGcLeafTargetRef`，以TD的inline scan登记覆盖该temp的root。该leaf不分配、不park、不握手、不回调且不产生statepoint；push必须在调用`scoop_rt_box_value`及其managed-entry handshake之前完成。region root贯穿entry handshake、allocation与payload copy，moving collector原地更新temp中的ref slots；runtime从同一已登记temp复制，返回caller后才以匹配的typed pop按LIFO移除。CFG verifier要求push/pop在所有非fatal出口配对，runtime入口只验证root已经活跃，不能在入口内补登记；空scan可省略该frame。不能在push前后缓存并改从旧AS1 leaf/旧副本复制。每次boxing照常产生ref identity，不使用shared singleton或place token；ZST unbox产生typed logical value，只有后续真正取址才另建place token；
- `Ptr<ZST>`在Scoop unsafe代码中合法：offset的byte displacement恒为0、pointer bits不变，load/store保持全部operand求值与pointer validity/alignment/lifetime前置条件但不访问payload。算法不得用pointer变化表达进度；`Ptr<Unit>`是opaque `void *`，需要byte arithmetic时使用`Ptr<UInt8>`。C ABI只把`Unit` result映射为`void`并保留`Ptr<Unit>`例外；ZST by-value参数/result、C-boundary extern global/TLS、空或含ZST字段的`@CLayout`及其他`Ptr<ZST>` C pointee在v1拒绝，本Cone内部compiler-owned/raw ZST storage仍按token规则存在。

这些规则必须进入LIR wire schema、layout/ABI fingerprint与artifact tests；否则同一个generic template可能在provider/consumer分别按empty aggregate、1-byte占位或zero-stride array生成不兼容代码。

### 3.7 native extern contract closure

native linker只按目标符号工作，不会检查两个Cone是否把同一`@Extern`声明成不同ABI。M23因此把extern声明加入构建期typed closure；它不是runtime metadata，不增加第七张image table。

target选择不是一个可由各stage自行补字段的`TargetProfile`袋子。driver/registry原子构造以下完整product并验证canonical triple、object format、deployment、calling convention、pointer/storage ABI、compiler输出与link输入互相相容：

```text
ResolvedTargetProfileV1 {
    lir_target: ValidatedLirTargetProfileV1,               // field 1
    backend: ValidatedBackendProfileV1,                    // field 2
    c_bridge_toolchain: ValidatedCBridgeToolchainProfileV1,// field 3
    runtime_build: ValidatedRuntimeBuildProfileV1,         // field 4
    final_link: ValidatedTargetLinkProfileV1,               // field 5
}

ValidatedLirTargetSelectionV1 = {
    lir_target: ValidatedLirTargetProfileV1,
    backend: ValidatedBackendProfileV1,
}
```

M23-2只构造并持久化前两项组成的`ValidatedLirTargetSelectionV1`，不伪造完整profile；后三项由后续required capability冻结：`c_bridge_toolchain`进入generated-bridge Code fingerprint，`lir_target + c_bridge_toolchain + runtime_build`进入`RuntimeArtifactFingerprint`并只经该validated产物继续影响RuntimeImage/Graph，`lir_target + final_link`进入`ResolvedLinkPlanFingerprint`。`lir-lower`只接收`lir_target`，Scoop codegen接收`lir_target + backend`，generated-C producer接收`lir_target + c_bridge_toolchain`，runtime-build接收`lir_target + c_bridge_toolchain + runtime_build`，program-link接收`lir_target + final_link`以及已经验证的其他stage产物。任一stage不得从host默认、另一projection或既有object反推缺少的项。

HIR先解析省略的`name`并保存source-level extern contract；LIR在`ValidatedLirTargetSelectionV1`下完成native symbol/calling-convention与canonical storage正规化后生成：

```text
NativeExternalSymbolKey = {
    target_profile: TargetProfileWireId,
    native_link_symbol: bytes
}

PersistentNativeExternalSymbolId =
    DomainSeparatedCborHash("scoop-native-link-symbol-v1",
                            NativeExternalSymbolKey)

NativeLibraryBinding = DefaultNativeNamespace                         // tag 1
                     | Requirement(NativeLinkRequirementId)           // tag 2

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
    DomainSeparatedCborHash(
        "scoop-native-external-contract-v1",
        { 1: PersistentNativeExternalSymbolId,
          2: NativeExternalContract })
```

`native_link_symbol`是target profile规范化后真正进入object symbol table的bytes；当前Darwin object中的undefined reference不携带声明的`lib`，所以只按该symbol id分组，`library`是必须相等的contract内容。`NativeLinkRequirementId`引用4.2中不含host绝对路径的target-tagged逻辑requirement；空`lib`只能编码为`DefaultNativeNamespace`。M12 v1的function calling convention只有规范化后的`Cdecl`，省略与显式`cdecl`得到同一值；data variant不携带calling convention且ABI固定为C。

source层先保存`SourceExternFunctionAbi = C { SourceCAbiFunctionSignature } | Scoop { SourceScoopAbiFunctionSignature, GcEffect }`，其中`GcEffect`严格为`Managed=1 | NoGc=2`；它与ordinary/suspend callable effect是正交轴。C source `Unit` result唯一正规化为Void，parameter不能是Unit；Scoop ABI result不使用该特例。alias在进入signature前透明展开，binder-free source extern必须拒绝残余binder；callback registration则复用允许binder的source C signature，到concrete application才替换。

target层的`CanonicalCAbiFunctionSignature`精确为map `1=TargetCallingConvention, 2=array<CanonicalCAbiParameter>, 3=CanonicalCAbiReturn`；parameter为`{1=source_exact_type, 2=CanonicalCStorageType}`，return为`Void=1 | Value=2 {1=source_exact_type, 2=storage}`。`CanonicalCStorageType`封闭区分integer的signedness/width、Boolean、带data-pointee与nullable-wrapper provenance的data pointer、带signature fingerprint的code pointer，以及引用`CanonicalCAbiLayoutFingerprint`的`@CLayout` struct。它描述canonical generated-C源码的storage contract，**不**持久化平台register class、integer extension、`byval`/`sret`等手写C ABI分类；所有C function/global/callback都必须经profile指定的canonical generated-C bridge与system C compiler产生C侧lowering。Scoop侧桥接入口只消费已经类型化的storage ABI，object capability再验证canonical C source/object闭包；不能从host ABI默认值补pass mode。

```text
CanonicalCAbiParameter = {
    source_exact_type: PersistentExactTypeId,     // field 1
    storage: CanonicalCStorageType,              // field 2
}

CanonicalCAbiReturn =
    Void                                           // tag 1
  | Value { source_exact_type: PersistentExactTypeId,
            storage: CanonicalCStorageType }     // tag 2, fields 1..2

CanonicalCAbiFunctionSignature = {
    calling_convention: TargetCallingConvention,// field 1
    parameters: array<CanonicalCAbiParameter>,  // field 2
    result: CanonicalCAbiReturn,                 // field 3
}

CanonicalScoopAbiFunctionSignature = {
    exact_signature: ExactCallableSignature,    // field 1
    arguments: array<ScoopAbiArgument>,         // field 2
    result: ScoopAbiReturn,                      // field 3
    gc_effect: GcEffect,                        // field 4
}
```

`CanonicalCAbiSignatureFingerprint`与`CanonicalCAbiLayoutFingerprint`分别对完整signature/layout preimage使用`scoop-c-abi-signature-v1`与`scoop-c-abi-layout-v1`的`DomainSeparatedCborHash`。Scoop variant则使用每个`PersistentExactTypeId`及完备`AbiArgument::{ElidedZst, Direct, Indirect}`/`AbiReturn::{UnitVoid, ElidedZst, Direct, Indirect}`，并把source `GcEffect`原样放入`CanonicalScoopAbiFunctionSignature` field 4；Managed与NoGc即使物理shape相同仍是不同contract，且两者都保持M15 `NativeBorrowed` transition与caller-root publication，`NoGc`不能偷降为ordinary NoGC callsite。因此相同size或LLVM function type不构成相同contract；只有canonical native signature逐字段相同才相等。

每个Cone的HIR/MIR/LIR section保留相应typed relation，manifest保存按`(PersistentNativeExternalSymbolId, contract fingerprint)`排序的完整record与local declaration origin集合。public import/re-export保留origin contract；下游extern call/global use及任一`LinkObject`中由这些extern目标产生的undefined relocation只能引用已有contract id。object verifier把每个`SourceExtern` undefined symbol连同其`SlibMemberId`反向关联到一个record；漏record、一个relocation匹配多个contract或record的symbol/library requirement与真实relocation/link input不一致都拒绝。合法`FunPtr`取址只指向非extern Scoop-owned `@NoGC` body，继续使用callable/object identity；`FunPtr`仅在它作为extern参数/结果的C code-pointer type时进入该contract signature。

每条object undefined-symbol use还必须以`{ member: SlibMemberId, relocation }`唯一归属于封闭`UndefinedSymbolRequirement::{ScoopOwned(persistent_entity), SourceExtern(contract_id), RuntimeEntry(runtime_target), GeneratedBridge(unit: GeneratedBridgeUnitId), TargetSupport(profile_capability), MemberCapability(capability_requirement_id)}`。前五种分别由typed IR external ref、extern contract、runtime ABI registry、C bridge semantic plan和target profile的完备compiler-support能力表产生；最后一种只允许known `LinkObject.verifier_capability`为未来native source等扩展提供已经规范化的symbol/library contract，不能成为“任意undefined都接受”的escape hatch。`GeneratedBridge`只保存producer-independent unit，不能保存当前Cone atom；object verifier把实际指向本Cone`PrimaryEntry(unit)`的relocation规范化回该unit。consumer按artifact target/runtime ABI独立验证该表，不能把任意符号、名字前缀、成员扩展名或“C compiler生成”当作豁免。manifest保存完整typed requirement index并由LIR/object verifier交叉验证；无分类、同一use多分类或找不到producer/support capability都拒绝。`SourceExtern`才要求源码contract，其他分支继续核对各自typed definition/ABI与link input。同一公开native symbol可以同时被源码extern和target support/公开runtime API使用（例如`memcpy`）；此时support/API能力表必须提供同样完整的canonical native ABI/library contract，linker逐字段合并后才允许共用symbol。Scoop-owned、generated bridge及compiler-private runtime symbol仍禁止被源码extern别名占用，不相容的多种requirement在native linker前失败。

undefined闭包不能替代definition闭包。每个`LinkObject`中的non-local strong/weak/common definition也必须唯一归属于封闭`DefinedLinkSymbolOwner::{ScoopDefinition(typed owner), GeneratedBridge(atom: GeneratedBridgeAtomId), TargetSupport(profile definition id), MemberCapability(capability definition id)}`并带其linkage/visibility/kind与canonical contract；bridge definition必须是artifact Cone拥有且可materialize的atom，不能以共享unit充当物理owner。local/debug symbol由对应object capability的allowlist单独约束。M23内建capability只能接受已经由LIR definition plan、generated bridge plan或target profile列出的定义，未认领或多重认领的external/weak definition直接拒绝；未来C/C++ producer必须定义自己的typed native-definition contract与跨Cone冲突/ODR策略，不能让native linker按输入顺序选择。manifest保存按member分区的完整defined-symbol index，program-link在启动native linker前合并验证。

`DefinedLinkSymbolOwner`与上述member-aware requirement只描述Cone `.slib`中的`LinkObject`，不能伪装成program descriptor、runtime或host native input也具有`SlibMemberId`。program-link先把每条逻辑native需求及其来源归一化；一个物理输入可同时满足多个来源，不能只保留任意一个“主需求”：

```text
NativeInputRequestOrigin =
    ConeMetadata {
        cone: ConeIdentity,
        artifact: ArtifactFingerprint,
        requirement: NativeLinkRequirementId,
        provider_contract: NativeProviderContractFingerprint,
    }
  | ConeLinkExtension {
        cone: ConeIdentity,
        artifact: ArtifactFingerprint,
        member: SlibMemberId,
        requirement: NativeLinkRequirementId,
        provider_contract: NativeProviderContractFingerprint,
    }
  | Runtime {
        runtime_artifact: RuntimeArtifactFingerprint,
        requirement: RuntimeNativeRequirementId,
        provider_contract: NativeProviderContractFingerprint,
    }
  | TargetProfile {
        target_profile: TargetProfileWireId,
        capability: CapabilityId,
        requirement: TargetNativeRequirementId,
        provider_contract: NativeProviderContractFingerprint,
    }

NativeProviderContract =
    Static {
        target: TargetProfileWireId,
        kind: DirectObject | StaticArchive,
        verifier: NativeObjectVerifierProfileFingerprint,
    }
  | Dynamic {
        target: TargetProfileWireId,
        kind: SharedLibrary | Framework,
        identity: NativeProviderIdentityContract,
        load_command_contract: DynamicLoadCommandContractFingerprint,
    }

NativeProviderIdentityContract =
    ContentDigest
  | PlatformIdentity { scheme: CapabilityId }

NativeProviderContractFingerprint =
    DomainSeparatedCborHash("scoop-native-provider-contract-v1",
                            NativeProviderContract)
```

`RuntimeNativeRequirementId`与`TargetNativeRequirementId`分别由runtime registry、target profile对其canonical requirement record使用独立domain hash产生，不能和`NativeLinkRequirementId`互换。`request_origins`按完整variant payload排序并去重；同一resolved input上的每个origin都必须与该input的provider contract逐字段相容，诊断保留全部origin，不能因去重丢掉“是谁要求了这个库”。profile隐式default library也只能从`TargetProfile`分支产生。

静态native input不能使用“任意合法object”能力。每个known verifier capability必须冻结一个`NativeObjectVerifierProfile`；其中每个effect字段都是`Reject | Verify { capability: CapabilityId }`的封闭sum，unknown capability或未归类effect均失败：

```text
NativeObjectVerifierProfile {
    capability: CapabilityId,
    target: TargetProfileWireId,
    object_format: ObjectFormatId,
    object_kind: Relocatable,
    load_commands: NativeSemanticEffectPolicy,
    sections: NativeSemanticEffectPolicy,
    local_and_debug_symbols: NativeSemanticEffectPolicy,
    constructors_and_destructors: NativeSemanticEffectPolicy,
    eh_and_unwind: NativeSemanticEffectPolicy,
    tls: NativeSemanticEffectPolicy,
    language_runtime_metadata: NativeSemanticEffectPolicy,
}

NativeSemanticEffectPolicy =
    Reject
  | Verify { capability: CapabilityId }

NativeObjectVerifierProfileFingerprint =
    DomainSeparatedCborHash("scoop-native-object-verifier-profile-v1",
                            NativeObjectVerifierProfile)

CanonicalRelocationUse {
    atom: CanonicalNativeAtomId,
    section_role: CanonicalSectionRole,
    relocation_role: CanonicalRelocationRole,
    offset_within_atom: u64,
    checked_addend: i64,
}

CanonicalNativeDefinition {
    definition_id: NativeDefinitionId,
    native_link_symbol: CanonicalNativeLinkSymbol,
    linkage: CanonicalNativeLinkage,
    visibility: CanonicalNativeVisibility,
    kind: Function | ReadOnlyData | MutableData | ReadOnlyTls | MutableTls,
    contract: NativeDefinitionContractFingerprint,
}

CanonicalNativeUndefinedUse {
    use_id: NativeUndefinedUseId,
    native_link_symbol: CanonicalNativeLinkSymbol,
    relocation: CanonicalRelocationUse,
    contract: NativeExternalContractFingerprint,
}

VerifiedNativeObjectSurface {
    byte_length: u64,
    sha256: Digest256,
    target: TargetProfileWireId,
    object_format: ObjectFormatId,
    verifier_profile: NativeObjectVerifierProfileFingerprint,
    definitions: CanonicalSet<CanonicalNativeDefinition>,
    requirements: CanonicalSet<CanonicalNativeUndefinedUse>,
    semantic_effects: CanonicalNativeSemanticEffectSet,
    verification_fingerprint: NativeObjectVerificationFingerprint,
}
```

`NativeDefinitionId`、`NativeUndefinedUseId`与`NativeObjectVerificationFingerprint`分别从所标识record移除自身id后以独立domain hash重算；set按相应id排序。verifier必须把每段会进入最终image的bytes及每个load command、section、symbol、relocation、constructor/destructor、EH/unwind、TLS和语言runtime metadata恰好归入上述typed surface或profile的一个`Verify` handler；不能归类就拒绝。M23内建的plain native-static capability拒绝constructor/destructor、EH/unwind、TLS、Objective-C/Swift metadata、自定义linker section以及任何autolink/embedded linker option；以后开放C/C++或更丰富native input时必须增加新的versioned contract，不能放宽为generic object pass-through。`CanonicalNativeAtomId`由verifier的canonical atom partition产生，relocation定位不使用raw section ordinal、symbol-table index、host path或临时文件名。

resolved native input是下列封闭sum；key不含host path，id只由key产生。direct object与static archive只能使用内容身份；只有target profile登记的dynamic identity scheme可以使用platform identity：

```text
CanonicalContentIdentity = { byte_length: u64, sha256: Digest256 }

CanonicalNativeProviderIdentity =
    ContentDigest(CanonicalContentIdentity)                            // tag 1
  | PlatformIdentity { scheme: CapabilityId, canonical_value: bytes } // tag 2

ResolvedNativeInputKey =
    DirectObject {
        target: TargetProfileWireId,
        object_format: ObjectFormatId,
        content: CanonicalContentIdentity,
    }
  | StaticArchive {
        target: TargetProfileWireId,
        archive_format: ArchiveFormatId,
        content: CanonicalContentIdentity,
    }
  | DynamicProvider {
        target: TargetProfileWireId,
        kind: SharedLibrary | Framework,
        identity: CanonicalNativeProviderIdentity,
    }

ResolvedNativeInputId =
    DomainSeparatedCborHash("scoop-resolved-native-input-v1",
                            ResolvedNativeInputKey)

NativeArchiveMemberRef {
    archive_input: ResolvedNativeInputId,
    physical_ordinal: u64,
    header_offset: u64,
    payload_offset: u64,
    payload_length: u64,
    payload_sha256: Digest256,
}

VerifiedNativeArchiveCandidate {
    member: NativeArchiveMemberRef,
    object: VerifiedNativeObjectSurface,
}

CanonicalResolvedNativeInput =
    DirectObject {
        input_id: ResolvedNativeInputId,
        key: ResolvedNativeInputKey::DirectObject,
        request_origins: NonEmpty<NativeInputRequestOrigin>,
        provider_contract: NativeProviderContractFingerprint,
        object: VerifiedNativeObjectSurface,
    }
  | StaticArchive {
        input_id: ResolvedNativeInputId,
        key: ResolvedNativeInputKey::StaticArchive,
        request_origins: NonEmpty<NativeInputRequestOrigin>,
        provider_contract: NativeProviderContractFingerprint,
        archive_layout_fingerprint: Digest256,
        candidates_in_physical_order: CanonicalVec<VerifiedNativeArchiveCandidate>,
        verification_fingerprint: NativeArchiveVerificationFingerprint,
    }
  | DynamicProvider {
        input_id: ResolvedNativeInputId,
        key: ResolvedNativeInputKey::DynamicProvider,
        request_origins: NonEmpty<NativeInputRequestOrigin>,
        provider_contract: NativeProviderContractFingerprint,
    }
```

`NativeArchiveVerificationFingerprint = DomainSeparatedCborHash("scoop-native-archive-verification-v1", {1=input_id, 2=archive_layout_fingerprint, 3=candidates_in_physical_order})`。static archive必须在调用native linker前一次性、有界地解析完整archive：拒绝thin/external/path member、nested archive、bitcode/LTO、unknown special member、非object ordinary member和越界/重叠布局；允许的symbol/long-name table也必须由`ArchiveFormatId`解释并进入layout fingerprint。每个ordinary candidate都按non-optional verifier profile检查，即使linker最后不抽取它。`NativeArchiveMemberRef`只在其精确archive digest内有意义；ordinal、offset、length与digest共同区分同名甚至相同bytes的重复member。target/linker profile的load trace若不能唯一恢复这个ref，该profile不合格。post-link trace只能选择该preverified candidate index中的记录，绝不能在链接后才发现并“补验”一个member。

所有`ContentDigest`输入都必须在plan定稿前从**同一批已验证bytes**物化到program-link私有、create-new、不可经symlink替换的只读snapshot；plan中的identity从该snapshot重算，native linker也只能打开同一snapshot。等价的already-open fd方案必须由profile提供identity-pinning proof。Cone提取对象、program descriptor、runtime对象与target startup/support对象也遵守同一规则。`PlatformIdentity`的scheme必须同时定义bounded canonical bytes、链接时pinning以及链接后/缓存命中时的revalidation proof；做不到就不能用于M23。

direct object和每个实际抽取的archive member分别形成static contribution；选择证据绑定精确plan action occurrence，而不是只绑定库名或member basename：

```text
NativeContributionSource =
    DirectObject {
        input: ResolvedNativeInputId,
        action: LinkActionIndex,
    }
  | ArchiveMember {
        input: ResolvedNativeInputId,
        action: LinkActionIndex,
        member: NativeArchiveMemberRef,
    }

ValidatedNativeContribution {
    contribution_id: NativeContributionId,
    plan: ResolvedLinkPlanFingerprint,
    source: NativeContributionSource,
    object_verification: NativeObjectVerificationFingerprint,
    selection_evidence: NativeSelectionTraceEventFingerprint,
}

NativeContributionId =
    DomainSeparatedCborHash("scoop-native-contribution-v1",
                            {1=plan, 2=source, 3=object_verification})

TargetSyntheticInputKey =
    RelocatableObject {
        target_profile: TargetProfileWireId,
        capability: CapabilityId,
        role: TargetSyntheticRole,
        content: CanonicalContentIdentity,
    }
  | LinkerGenerated {
        target_profile: TargetProfileWireId,
        capability: CapabilityId,
        role: TargetSyntheticRole,
        generation_rule: Digest256,
    }

VerifiedTargetSyntheticInput =
    RelocatableObject {
        input_id: TargetSyntheticInputId,
        key: TargetSyntheticInputKey::RelocatableObject,
        object: VerifiedNativeObjectSurface,
    }
  | LinkerGenerated {
        input_id: TargetSyntheticInputId,
        key: TargetSyntheticInputKey::LinkerGenerated,
        definitions: CanonicalTargetDefinitionSet,
        requirements: CanonicalTargetRequirementSet,
        verification_fingerprint: Digest256,
    }
```

`TargetSyntheticInputId`以domain `scoop-target-synthetic-input-v1`从key重算。profile的default static archive/direct object必须进入`CanonicalResolvedNativeInput`并带`TargetProfile` request origin，default shared library/framework必须进入dynamic-provider分支；都不能伪装成`TargetSynthetic`。该分支只表示profile明确声明的startup/support relocatable object或linker-generated contract。

native linker与artifact inspection完成后，program-link把每个实际受控贡献或dynamic provider归一化为封闭sum；未抽取任何member的static archive仍由plan/evidence覆盖，但不会伪造空contribution：

```text
FinalLinkInput =
    ConeObject(VerifiedLinkObject)
  | ProgramDescriptor(VerifiedProgramDescriptorObject)
  | RuntimeObject(ValidatedRuntimeObject)
  | NativeStaticContribution(ValidatedNativeContribution)
  | NativeDynamicProvider(ValidatedDynamicProvider)
  | TargetSynthetic(VerifiedTargetSyntheticInput)

FinalLinkInputKey =
    ConeObject { cone: ConeIdentity,
                 artifact: ArtifactFingerprint,
                 member: SlibMemberId,
                 object_verification: Digest256 }
  | ProgramDescriptor { object_digest: Digest256 }
  | RuntimeObject { artifact: RuntimeArtifactFingerprint,
                    object: RuntimeObjectId }
  | NativeStaticContribution { contribution: NativeContributionId }
  | NativeDynamicProvider { provider: ResolvedNativeProviderId }
  | TargetSynthetic { input: TargetSyntheticInputId }

FinalLinkInputId =
    DomainSeparatedCborHash("scoop-final-link-input-v1",
                            FinalLinkInputKey)

FinalLinkSymbolOwner =
    Cone { cone, member, owner: DefinedLinkSymbolOwner }
  | ProgramDescriptor { schema, symbol_role }
  | Runtime { runtime_artifact, runtime_object, runtime_symbol }
  | NativeStatic {
        contribution: NativeContributionId,
        definition: NativeDefinitionId,
    }
  | TargetSynthetic {
        input: TargetSyntheticInputId,
        definition: TargetSyntheticDefinitionId,
    }
```

static owner只引用一个contribution中的一条definition；该contribution对应的全部request origin、resolved input与provider contract由typed relation反查，不能在owner中再复制一份可能不一致的单个requirement。undefined use也必须保存完整、类型化且可重算的origin：

```text
FinalUndefinedOrigin =
    Cone {
        cone: ConeIdentity,
        member: SlibMemberId,
        requirement: UndefinedSymbolRequirement,
        relocation: CanonicalRelocationUse,
    }
  | ProgramDescriptor {
        program_object: Digest256,
        requirement: CanonicalProgramRequirement,
        relocation: CanonicalRelocationUse,
    }
  | Runtime {
        runtime_artifact: RuntimeArtifactFingerprint,
        runtime_object: RuntimeObjectId,
        requirement: CanonicalRuntimeRequirement,
        relocation: CanonicalRelocationUse,
    }
  | NativeStatic {
        contribution: NativeContributionId,
        requirement: NativeUndefinedUseId,
        relocation: CanonicalRelocationUse,
    }
  | TargetSynthetic {
        input: TargetSyntheticInputId,
        requirement: TargetSyntheticRequirementId,
        relocation: CanonicalRelocationUse,
    }

FinalUndefinedSymbolRequirement {
    requirement_id: FinalUndefinedSymbolRequirementId,
    origin: FinalUndefinedOrigin,
    native_link_symbol: CanonicalNativeLinkSymbol,
    contract: NativeExternalContractFingerprint,
}

FinalUndefinedSymbolRequirementId =
    DomainSeparatedCborHash("scoop-final-undefined-symbol-v1",
                            {1=origin, 2=native_link_symbol, 3=contract})

FinalUndefinedResolution =
    Controlled {
        requirement: FinalUndefinedSymbolRequirementId,
        owner: FinalLinkSymbolOwner,
    }
  | DynamicImport {
        requirement: FinalUndefinedSymbolRequirementId,
        provider: ResolvedNativeProviderId,
        binding: ValidatedDynamicImportBindingId,
    }
```

每个origin中的typed requirement与relocation必须反查到所属object verification surface中的同一symbol/contract，不能靠symbol name重新配对。dynamic provider只出现在resolution中，从不成为definition owner或undefined origin：

```text
ResolvedNativeProviderId =
    SHA-256(ByteSpan("scoop-resolved-native-provider-v1") ||
            raw(ResolvedNativeInputId))

ValidatedDynamicProvider {
    provider_id: ResolvedNativeProviderId,
    resolved_input: ResolvedNativeInputId,
    bindings: CanonicalSet<ValidatedDynamicImportBinding>,
}

ValidatedDynamicImportBinding {
    binding_id: ValidatedDynamicImportBindingId,
    undefined_requirement: FinalUndefinedSymbolRequirementId,
    native_link_symbol: CanonicalNativeLinkSymbol,
    contract: NativeExternalContractFingerprint,
}

ValidatedDynamicImportBindingId =
    DomainSeparatedCborHash(
        "scoop-dynamic-import-binding-v1",
        {1=undefined_requirement, 2=native_link_symbol, 3=contract})
```

provider id按上式只表示dynamic resolved input身份，不随本次程序实际imports改变；只有`CanonicalResolvedNativeInput::DynamicProvider`的id可用于该公式。target/kind/identity全部从`resolved_input`反查并作为provider record的一致性证明，不进入provider id preimage，也不能在post-link record中复制第二份。binding id从移除自身id后的完整binding重算。完整request origins与provider contract同样只由`resolved_input`反查。`PlatformIdentity.canonical_value`必须是对应scheme定义的有界canonical bytes，unknown scheme不能进入plan。bindings按binding id严格排序，可以为空（例如profile明确保留但当前没有import的load command）；但每个实际允许external resolution的requirement必须在全部provider中恰绑定一次。未绑定export不是`FinalLinkSymbolOwner`。每个实际shared-library/framework load command都有一个dynamic-provider origin；仅作为候选而未形成load command的input只保留plan/evidence。dynamic provider不得满足、抢占或interpose任何最终解析类别为`Controlled`的requirement，而不只是compiler-private符号。外部provider在运行机器上是否真实遵守声明的C ABI仍是FFI作者责任，但不能因此跳过输入identity、contract或provenance验证。

program descriptor verifier只允许schema声明的program record、image/root/core引用和profile支持引用；`ValidatedRuntimeArtifact`携带`lir_target + c_bridge_toolchain + runtime_build`三项validated projection及各自fingerprint、runtime ABI、任意非空数量的verified runtime object record、封闭runtime defined/undefined contract和`RuntimeArtifactFingerprint`，不含backend/final-link projection，也不含或指向第二种runtime archive容器。local/debug symbol仍只由各输入capability的有界allowlist处理。

```text
ValidatedRuntimeArtifact {
    lir_target: ValidatedLirTargetProfileV1,
    c_bridge_toolchain: ValidatedCBridgeToolchainProfileV1,
    runtime_build: ValidatedRuntimeBuildProfileV1,
    runtime_abi: RuntimeAbiFingerprint,
    objects: NonEmpty<ValidatedRuntimeObject>,
    definitions: CanonicalRuntimeDefinitionSet,
    requirements: CanonicalRuntimeRequirementSet,
    fingerprint: RuntimeArtifactFingerprint,
}

VerifiedProgramDescriptorObject {
    graph_fingerprint: GraphFingerprint,
    object_digest: Digest256,
    definitions: CanonicalProgramDefinitionSet,
    requirements: CanonicalProgramRequirementSet,
}
```

runtime-build不引入第二种可分发容器。target profile以normalized toolchain-relative UTF-8 path列出完整runtime source set与构建规则，但输出是任意非空数量的verified relocatable object collection；source数、object数和二者映射都不是协议基数：

```text
ValidatedRuntimeObjectRecord {              // canonical map field 1..5
    object_id: RuntimeObjectId,              // 1
    byte_length: u64,                        // 2
    sha256: Digest256,                       // 3
    definitions: CanonicalRuntimeDefinitionSet, // 4
    requirements: CanonicalRuntimeRequirementSet, // 5
}

RuntimeObjectId =
    DomainSeparatedCborHash("scoop-runtime-object-v1", {
        1: sha256,
        2: definitions,
        3: requirements,
    })

RuntimeArtifactFingerprint =
    DomainSeparatedCborHash("scoop-runtime-artifact-v1", {
        1: lir_target,
        2: c_bridge_toolchain,
        3: runtime_build,
        4: runtime_abi,
        5: object_records_sorted_by_object_id,
        6: canonical_merged_definitions,
        7: canonical_merged_requirements,
    })
```

source path不含host sysroot前缀；source/build-rule closure、C compiler identity/version、canonical generated-C模板与完整flags分别由`c_bridge_toolchain`及`runtime_build`的typed contract/fingerprint承诺，`lir_target`则承诺runtime object必须匹配的target/storage ABI。三项都按上述field 1…3直接进入同domain preimage，不能压成未定义的单一toolchain digest，也不能从object bytes或当前host反推。重复object id、同id不同record、缺失source build evidence、未分类额外object或跨object definition/requirement冲突都拒绝；相同bytes/contract产生相同object id，构建分片改变则允许改变runtime artifact fingerprint。M23每次为executable构建该集合，不接受外部prebuilt runtime bundle，也不设runtime-object cache；以后若引入可分发/cacheable runtime bundle，必须先单独冻结其container/manifest与信任协议。

runtime definition封闭为C process entry、公开`RuntimeEntry`与带typed private id的runtime helper；requirement封闭为`ProgramDescriptorV1`和`runtime_build`列出的support/native contract。runtime artifact必须定义三项projection共同要求的C entry与runtime entry，不能定义`ScoopProgramDescriptorV1`、Cone image、Scoop-owned或generated-bridge symbol。program object从封闭`ProgramObjectPlan`生成后立即反解析；其唯一default-visible strong definition是`scoop_program_descriptor`，其余hidden/local record和每条image/root/core/support relocation必须与plan一一对应，不能产生额外native requirement。program/runtime/Cone/target relocatable object以及静态archive的**全部preverified candidate**都在native linker启动前逐object拒绝`LC_LINKER_OPTION`、autolink、embedded linker script/option或其他隐式扩展link输入；是否最终被archive算法抽取不能改变验证结果，M23也不尝试把这类副作用递归“发现后补进”closure/cache key。

因此pre-link Cone闭包验证、program/runtime/native/profile各自的input verifier与post-link map/image verifier共同覆盖最终输入全集；任一实际受控贡献或dynamic provider没有唯一`FinalLinkInput` origin、绕过typed contract、抢占Scoop/program/runtime保留symbol，或linker trace/dynamic-import evidence与最终image不一致都拒绝。linker/`cc` driver隐式加入的startup object、default library或参数也必须由target profile列出、归一化为pre-link plan及相应final origin并进入实际输入identity；无法取得完整archive-member load trace、link map、dynamic-import与platform-identity pinning/revalidation证据的target/linker profile不合格。shared provider未绑定的其他export不塞进Cone的`DefinedLinkSymbolOwner`或`FinalLinkSymbolOwner`，也不能interpose任何解析为`Controlled`的requirement。

最终linker在调用native linker前合并完整transitive Cone closure中的全部record。同一symbol id必须先证明canonical symbol key相同，再要求library、kind/TLS/mutability、ABI、calling convention及完整signature/storage逐字段相等；相等者去重为一份contract和native requirement，不同者报告全部declaration origin及第一个不同字段。即使其中某个声明在当前root未调用也不交给native linker任选。该闭包只能证明程序内部声明一致；外部binary是否真实实现该contract仍是FFI作者责任。

## 4. `.slib` v1格式与reader

### 4.1 deterministic archive

`.slib` v1使用标准deterministic `ar`容器，但archive只是字节容器，语义入口是manifest中的通用typed member directory。除唯一bootstrap成员`manifest.cbor`外，每个成员都有一条：

```text
SlibMemberId = SHA-256(ByteSpan("scoop-slib-member-v1") ||
                       raw(ConeIdentity) || WireCborV1(MemberStableKey))

SlibMemberRecord {
    id: SlibMemberId,
    stable_key: MemberStableKey,
    role: SlibMemberRole,
    byte_length: u64,
    sha256: Digest256,
}

CapabilityId = { namespace: CanonicalCapabilityNamespace,
                 name: CanonicalCapabilityName,
                 major_version: NonZeroU32 }

MemberPurposeSet = u32 bit set {
    Graph       = 0x0000_0001,
    Compile     = 0x0000_0002,
    Link        = 0x0000_0004,
    Diagnostics = 0x0000_0008,
}
```

M23 v1对上述目录类型统一使用第1.1节`WireCborV1`，而不是Rust内存布局。unsigned integer使用最短CBOR编码，digest/id使用恰好32 byte的byte string，`logical_key`使用1…4,096 byte的byte string，ASCII名字使用text string；record/product是只含已声明integer key的map，sum的key `0`是非零variant tag，其余payload field按下表的integer key编码。map key按canonical顺序排列，缺失、重复、额外key、错误major type、indefinite item、浮点、非最短整数或unknown tag都拒绝。`SlibMemberRecord`的field key固定为`1=id, 2=stable_key, 3=role, 4=byte_length, 5=sha256`；`byte_length`必须能在4.5的`u64`预算内通过checked计算。

`MemberStableKey`的v1封闭编码是：

| tag | variant | payload field |
|---:|---|---|
| 1 | `HirMetadata` | 无 |
| 2 | `MirMetadata` | 无 |
| 3 | `LirMetadata` | 无 |
| 4 | `LinkObject` | `1=verifier_capability, 2=logical_key` |
| 5 | `DiagnosticAttachment` | `1=capability, 2=logical_key` |
| 6 | `ExtensionBlob` | `1=capability, 2=logical_key` |

`SlibMemberRole`的v1封闭编码是：

| tag | variant | payload field | implicit purpose |
|---:|---|---|---|
| 1 | `HirMetadata` | `1=wire_schema: NonZeroU32` | Compile |
| 2 | `MirMetadata` | `1=wire_schema: NonZeroU32` | Compile |
| 3 | `LirMetadata` | `1=wire_schema: NonZeroU32` | Compile, Link |
| 4 | `LinkObject` | `1=target_profile: TargetProfileWireId, 2=object_format: ObjectFormatId, 3=verifier_capability: CapabilityId` | Link |
| 5 | `DiagnosticAttachment` | `1=capability: CapabilityId` | Diagnostics请求时可解释，但永不成为语义必需输入 |
| 6 | `ExtensionBlob` | `1=capability: CapabilityId, 2=required_for: MemberPurposeSet` | 见下文 |

metadata stable key只与同tag role配对；其余stable key还要求key与role中的完整capability逐byte相等。`LinkObject` key中的capability就是producer/verifier contract，不从object bytes或文件名反推。`logical_key`是该capability定义的canonical shard/translation-unit/bridge/owner key；已知producer必须做到decode后重新encode逐byte相同，未知capability的reader仍能按有界opaque bytes计算id。key不含payload hash、host path、临时名或目录ordinal；不同成员不能共享id，同一artifact内同一id也不能换成不同role。

`CapabilityId`本身固定编码为map `1=namespace, 2=name, 3=major_version`。namespace总长1…255 ASCII byte，grammar为`[a-z][a-z0-9-]{0,62}(\.[a-z][a-z0-9-]{0,62})*`；name总长1…63 byte，grammar为`[a-z][a-z0-9-]{0,62}`；major version为`1..=u32::MAX`。`TargetProfileWireId`和`ObjectFormatId`是在类型上不可互换的`CapabilityId` wrapper，wire仍用同一三字段编码。M23 registry精确内建：

| typed id | `namespace / name / major` | contract |
|---|---|---|
| `TargetProfileWireId::DarwinAarch64V1` | `org.scoop-lang.target-profile / darwin-aarch64 / 1` | M15冻结、M23-2精确化的Darwin/AArch64 LIR-target/layout contract；manifest另带其profile fingerprint，不代表C-bridge/runtime-build/final-link三项projection |
| `ObjectFormatId::MachORelocatableV1` | `org.scoop-lang.object-format / mach-o-relocatable / 1` | Mach-O 64-bit relocatable object envelope |
| Scoop LIR verifier | `org.scoop-lang.link-object / scoop-lir / 1` | LIR plan、definition/digest/image/stackmap与symbol contract |
| generated C bridge verifier | `org.scoop-lang.link-object / generated-c-bridge / 1` | generated bridge plan与完整defined/undefined native contract |

两个内建object capability的`logical_key`也不留给实现自由选择，并且不把任意多unit id内联到4,096-byte key中。generated bridge把producer-independent recipe unit与producer-specific physical atom严格分开：

```text
GeneratedBridgeUnitKey =
    OutboundFunction { contract: NativeExternalContractFingerprint }       // tag 1
  | GlobalRead       { contract: NativeExternalContractFingerprint }       // tag 2
  | GlobalWrite      { contract: NativeExternalContractFingerprint }       // tag 3
  | GlobalAddress    { contract: NativeExternalContractFingerprint }       // tag 4
  | CallbackTrampoline { c_signature: CanonicalCAbiSignatureFingerprint,
                         context_parameter: CallbackParameterIndex }         // tag 5

GeneratedBridgeUnitId =
    DomainSeparatedCborHash("scoop-generated-bridge-unit-v1",
                            GeneratedBridgeUnitKey)

GeneratedBridgeAtomKey = {
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

GeneratedBridgeAtomId =
    DomainSeparatedCborHash("scoop-generated-bridge-atom-v1",
                            GeneratedBridgeAtomKey)

GeneratedBridgeSemanticTarget = {
    unit: GeneratedBridgeUnitId,                    // field 1
}

CanonicalCAbiSignatureFingerprint =
    DomainSeparatedCborHash("scoop-c-abi-signature-v1",
                            CanonicalCAbiFunctionSignature)

CallbackParameterIndex = typed zero-based u32

ScoopLirObjectUnitSetDigest =
    DomainSeparatedCborHash(
        "scoop-lir-object-unit-set-v1",
        strictly_sorted_unique_nonempty<ObjectDefinitionPlanId>)

GeneratedBridgeObjectUnitSetDigest =
    DomainSeparatedCborHash(
        "scoop-generated-bridge-object-unit-set-v1",
        strictly_sorted_unique_nonempty<GeneratedBridgeUnitId>)
```

sum仍按本节规则编码为`0=tag`及声明顺序payload field；contract/signature/layout id都是对应schema的固定32-byte typed digest。`CallbackParameterIndex`是独立于源码`Long`的typed newtype，在key中编码为Wire CBOR unsigned `u32`，必须小于canonical C signature的参数数且精确指向registration指定的`Ptr<Unit>` context槽；签名中存在别的普通`Ptr<Unit>`参数并不构成歧义。callback trampoline unit严格按`(c_signature, context_parameter)`复用，不包含具体closure、callback application、managed adapter或`PersistentCallableBodyId`；同一签名不同context槽得到不同unit，同一pair可由不同application复用同一个unit recipe。每个实际producer Cone仍为所用unit恰好生成一个`PrimaryEntry(unit)` atom，并以该atom的`GeneratedBridgeAtomId`产生自己的ConeStrong `br` symbol；跨Cone不共享物理symbol。

`GeneratedBridgeSemanticTarget`是canonical LIR definition、ODR fingerprint与undefined requirement可引用的唯一bridge target。codegen把它映射到当前artifact Cone的primary atom；object verifier核对真实symbol后规范化回unit。`SignatureDescriptor`与`ContextDescriptor`可在需要时物理materialize；`StaticAssertSupport`只标识canonical generated-C source中的编译期assert recipe，成功object中没有对应section bytes，绝不能产生`br` request、`ObjectDefinitionPlan`或defined-symbol owner。缺少预期assert proof、为它制造sentinel atom、跨producer引用atom或LIR直接保存atom id都拒绝。

`scoop-lir/1` logical key精确为map `1=unit_count: NonZeroU32, 2=ScoopLirObjectUnitSetDigest`；`generated-c-bridge/1`使用同样field形状，但key 2为`GeneratedBridgeObjectUnitSetDigest`。manifest的materialization relation保存完整、按id排序的unit/plan → `SlibMemberId`映射，atom则由producer Cone与unit/role key重算；reader要求每个plan/unit恰出现一次、不同member集合不相交，并证明object内全部可物理atom/record及共享CIE/layout support均为这些unit的typed派生闭包，不允许无owner的standalone support。这样单个object可含任意多unit而logical key仍定长；打包分组改变可以改变member id/code fingerprint，却不改变LIR semantic fingerprint，枚举或线程完成顺序不能改变key。未来producer capability必须冻结自己的unit key、set digest、覆盖和canonicality，不能只分配capability名字后使用临时ordinal。

这张表没有“任意C object”能力：未来C/C++ producer新增自己的versioned verifier capability和完整contract，但仍使用同一个`LinkObject` role。M23没有内建`ExtensionBlob`的required capability handler。

reader的基础语义purpose由类型参数选择`Graph`、`Compile`或`Link`之一，`Diagnostics`只是正交的显式decorator。effective closure固定为Compile→{Graph, Compile}、Link→{Graph, Link}、Graph→{Graph}；Diagnostics不授予其他能力。v1 `ExtensionBlob.required_for`只允许`0`或恰好`Link(0x4)`：前者是任何语义purpose都可跳过的opaque attachment，后者要求Link reader认识capability并运行handler；Graph/Compile/Diagnostics bit及多bit组合虽然可从envelope解码，但在v1属于非法role/purpose组合并拒绝。这样未知blob可以安全保存，而不会出现尚无typed输出/fingerprint落点的Graph/Compile输入。v1 link handler的封闭输出只有`VerifiedLinkExtensionInputV1::NativeLibraryRequirements(CanonicalNativeLibraryRequirementSet)`（tag 1）；没有object/raw-bytes/argv/path/script variant。需要参与链接的Scoop、C、C++或其他producer object必须直接声明成一个或多个`LinkObject`，不能藏在blob中绕过member-aware验证；以后扩展handler出口必须提升container或对应wire schema并让旧reader fail closed。

- archive物理顺序固定为`manifest.cbor`，随后按`SlibMemberId` bytes严格递增的目录项；对应物理名只由从0开始的directory ordinal唯一派生为`m`加8位、零填充的十进制数（`m00000000`…`m00065535`），不写入`SlibMemberRecord`，也不保留输入basename或扩展名。reader从已验证目录独立推导expected name并与raw ar header逐项核对，拒绝绝对路径、`..`、separator、重复名/id、未声明成员和缺失成员；物理名不是member identity/fingerprint的第二真源；
- 容器必须是self-contained普通`ar`，拒绝thin archive及任何外部文件引用。因为`.slib`不会整体交给native linker，writer不生成、reader不接受`/`、`//`、`__.SYMDEF*`或其他archive symbol/long-name special member；唯一bootstrap名和全部`mNNNNNNNN`均可由普通短名header表示，不能让special table形成目录外payload或host-path/TOCTOU旁路；
- `HirMetadata`、`MirMetadata`、`LirMetadata`各恰有一个。raw解码只返回`DecodedSlibEnvelope`，保证canonical container/directory、兼容头、长度与全部payload hash，不授予任何semantic API。purpose-specific validation返回不可混用的`ValidatedGraphArtifact`（identity、dependency/kind/target与完整member envelope）、`ValidatedCompileArtifact`（在Graph proof上增加HIR/MIR/LIR decode、跨层结构验证与`Imported*Set`）或`ValidatedLinkArtifact { graph, lir_verification_surface, link_objects: NonEmpty<VerifiedLinkObject>, image_owner: VerifiedImageOwner, link_extension_inputs, ... }`。Graph/Compile view只保存hash-valid object envelope，不能取得object bytes的verified link API；Link view不要求HIR/MIR可供compiler使用，但必须解码Link所需LIR verification surface。M23可发布Cone artifact必须分别成功构造Compile与Link view，因此结构上至少有一个link object与恰好一个image；任何view都不得假定更具体的object数量或固定文件名；
- `LinkObject.verifier_capability: CapabilityId`隐含`required_for = { Link }`，决定该object必须经过哪种验证；effective purpose包含Link的consumer不认识它时必须在object解析和native linker调用前fail closed，不能退化成generic object pass-through。纯Graph/Compile consumer仍须验证envelope/length/hash，但可以把该payload保持opaque，不能由此声称它已经link-valid。当前Darwin profile的known verifier要求每个member确为Mach-O `MH_OBJECT`，load command、section/flags、symbol/relocation/string table均在对应capability的有界allowlist内，明确拒绝`LC_LINKER_OPTION`、autolink directive、embedded linker script/options及其他绕过typed native requirement的输入；所有linker-visible definition和undefined use分别满足3.7的typed owner/requirement闭包。M23内建Scoop LIR object与generated C bridge object能力，但不规定它们各自的成员数；未来Cone内C/C++ translation unit仍使用同一个`LinkObject`外层role，只增加对应的known verifier/compile-contract capability，不改变archive、提取或link算法。真正开放C/C++源码前仍须另行定义flags、undefined-symbol/native-library contract、静态构造与M25异常边界；成员目录本身不伪装成这些语言规则已经完成；
- `DiagnosticAttachment.capability`是版本化opaque capability id，不是reader必须穷举的封闭format enum；该role固定为对Graph/Compile/Link均optional，只能被认识该capability的Diagnostics consumer解释。旧reader仍验证envelope、长度、hash与资源上限，但不认识capability时跳过；诊断附件缺失或无法解释不得改变编译、链接或runtime语义；
- `ExtensionBlob`可以承载任意非object blob；v1的optional与Link-required语义、唯一handler输出在上文已经封闭。所有reader仍检查名称、长度、hash和资源上限；unknown optional capability可跳过，unknown Link-required capability在payload解码或native linker调用前拒绝。blob永不因扩展名、内容探测或“看起来像object”成为link input；
- native linker只接收目录中已验证`LinkObject`的bytes；known Link-required extension handler只向resolved plan增加上述canonical native-library requirement。一个名为`.o`的附件/blob不会被链接，一个物理名没有`.o`后缀的`LinkObject`照常链接。任何层都不得把“除metadata外的成员”、整个`.slib`或固定ordinal盲传给linker；
- canonical writer唯一输出SysV/GNU short-name `ar`：global magic为ASCII `!<arch>\n`；每个60-byte header的name field为logical physical name加`/`再以space填满16 byte，mtime/uid/gid分别为十进制`0`再填满12/6/6 byte，mode为八进制ASCII `100644`再填满8 byte，size为无前导零的十进制payload长度再填满10 byte，结尾恰为`` `\n``；payload为奇数byte时追加一个不计入size的`0x0A` pad。writer不得输出其他等价拼法；v1 reader为保证唯一bitstream也逐field要求该canonical encoding。archive不记录build目录、producer host或临时文件名；metadata CBOR遵守上文唯一codec；
- 同样输入、compiler/schema/target与dependency semantic fingerprints必须逐byte产生相同`.slib`。两次隔离临时目录构建的bitwise equality属于验收门。

选择标准archive只解决容器边界，不把IR wire schema委托给`ar`。实现应使用成熟archive/CBOR/hash库，不手写通用TOML、archive或CBOR parser。

### 4.2 manifest与兼容头

`manifest.cbor`至少包含：

- magic `SCOOPSLIB`、container version与每个HIR/MIR/LIR wire schema version；
- producer compiler版本（仅诊断）、language ABI、runtime ABI、mangling/identity version；
- canonical Cone coordinate、`ConeIdentity`、Cone kind；
- exact direct dependency records及编译时三层semantic fingerprints；
- `ResolvedTargetProfileV1`五投影中的LIR target id/fingerprint与backend id/fingerprint进入compatibility；C-bridge toolchain、runtime-build与final-link profile由其后续required capability、runtime artifact或resolved link plan分别承诺，不能伪装成前两项。每个native object的object format由自己的member record携带并必须与对应投影兼容；
- 每个**非manifest** member的完整`SlibMemberRecord`，包括typed id/stable key、role/capability/purpose、byte length与SHA-256；manifest不能包含自身member hash，物理archive name只由canonical目录ordinal派生。由目录可重算的required-capability摘要若为快速拒绝而冗余保存，reader必须逐项证明相等；
- public/re-export/prelude index摘要、source table摘要；
- ODR records、每个ObjectDefinition/patch intent/generated-bridge unit到`SlibMemberId`/actual offset或member的完整materialization relation、按member分区的全部strong/ODR `DefinitionAtomRange`、每个ODR record引用的精确range子集、typed digest graph、runtime type/Safepoint/callable-body full-id映射、六类runtime registration record摘要，以及每条记录都带member id的完整`DefinedLinkSymbolOwner`/`UndefinedSymbolRequirement` index；
- 唯一`image_owner_member: SlibMemberId`，它必须指向`LinkObject`并与该object中由当前Cone identity派生的hidden strong image descriptor定义互证。目录其余成员都不得定义同一或另一Scoop image descriptor；
- 完整`NativeExternalContractRecord`表及独立的target-tagged native **library** link requirements；contract按`(symbol id, contract fingerprint)`排序，保存canonical symbol key/payload/local declaration origins，library requirement保存逻辑库名、kind、顺序/分组约束和target条件。二者都不记录producer机器的绝对搜索路径，均沿完整transitive link closure传播；
- `hir_semantic_fingerprint`、`mir_semantic_fingerprint`、`lir_semantic_fingerprint`、`code_fingerprint`、`runtime_image_fingerprint`与whole-artifact fingerprint。`graph_fingerprint`只存在最终program descriptor，因为library `.slib`尚无最终闭包。

M23 v1对required schema、language/runtime ABI、identity/mangling version及target fingerprint采用exact match。producer patch版本不同但这些值相同时可以读取；不能简单用“编译器版本字符串相同”替代各层兼容检查，也不能对unknown enum variant猜默认。

`MemberFingerprint = DomainSeparatedCborHash("scoop-slib-member-content-v1", SlibMemberRecord)`只描述完整envelope成员；`LinkMemberFingerprint = DomainSeparatedCborHash("scoop-slib-link-member-v1", SlibMemberRecord)`使用同一份4.1五字段record编码，但只允许对role=`LinkObject`或`ExtensionBlob { required_for={Link} }`调用。capability/purpose只在`role`内编码一次，不另加一份派生字段。两者都与物理archive ordinal/name无关。whole-artifact使用独立的多片段framing：

```text
ArtifactFingerprint = SHA-256(
    ByteSpan("scoop-artifact-v1")
    || ByteSpan(WireCborV1(ArtifactManifestInputV1))
    || Concat(i in [0, n), ByteSpan(raw(MemberFingerprint[i])))
)
```

`ArtifactManifestInputV1`是与bootstrap manifest field 1…10相同、但结构上不存在field 11的独立closed product，不是把完整manifest的fingerprint槽归零或遗漏required field。member fingerprints按目录`SlibMemberId`顺序；`n=0`时Concat为空，所有count/index checked。writer计算后再增加field 11并重新编码，reader投影回该input重算。这样manifest/member校验没有自引用，archive header的规范化值也不会成为另一套identity。fingerprint只做完整envelope的一致性与cache验证，不是发行者签名。

同coordinate的两个完整`ArtifactFingerprint`不同的候选仍是ambiguous artifact，即使差异只来自optional attachment；这样search-root顺序永远不能暗中选择不同envelope。但编译与链接失效不使用whole-artifact fingerprint：三层semantic fingerprint和4.6的code fingerprint只覆盖对应用途的成员，所以optional diagnostic/opaque blob变化不会使下游重编译或重链接。

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

每个param-free exported source exact subject还必须携带有限、可闭合的shape-support边：其source nominal definition、exact ancestry/field/variant shape、必要的layout/scan/dispatch与native-boundary witness都由M23-6 required capability按typed id给出，使下游能验证并引用已物化定义。该闭包不复制param-free private body，也不允许consumer按FQN、相同layout或当前唯一候选补猜；缺少任一shape edge时Compile proof失败，而不是由codegen临时生成第二个TypeDescriptor。

`concretization predicate`不是自由文本或nullable flag，而是版本化封闭sum；M23至少包含既有`RequiresGcFreePointee`与`CLayoutFieldRequirement::CFieldSafeAndNonZst { signature_type, declaration_field_path }`。predicate引用binder位置和persistent type ref，consumer只能在完整替换后得到Success或带provider origin的typed diagnostic；unknown variant使reader拒绝artifact，不能被当成“无需检查”。

### 4.4 MIR与LIR metadata

M23-2先冻结三层共同outer envelope与identity foundation；这部分不是后续stage可以用当前Rust DTO替换的摘要：

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

HIR/MIR/LIR magic分别为`SCOOPHIR`、`SCOOPMIR`、`SCOOPLIR`，M23全部`outer_schema=1`。foundation capability精确为`org.scoop-lang.hir/identity-foundation/1`、`org.scoop-lang.mir/identity-foundation/1`与`org.scoop-lang.lir/identity-foundation/1`，三者都严格`required_for={Compile}`且每层恰好一条；artifact profile为`org.scoop-lang.slib-profile/identity-foundation/1`。该profile的Code/RuntimeImage unavailable、publication=`FoundationOnly`、Link forbidden，只能构造`ValidatedCompileArtifact<IdentityFoundationProfile>`，不能发布、链接或作为dependency。section按`CapabilitySortKey`严格递增；unknown optional完成outer/hash验证后才可跳过，unknown Compile-required在分配IR arena前失败。

三个inner payload都是Wire CBOR v1 closed product；同一种identity只在首次产生它的stage进入一张delta table，跨层重复`(kind,id)`即使key相同也拒绝。HIR foundation的field 1…30精确为：

| field | table |
| ---: | --- |
| 1 | `SourceRecordV1` |
| 2 | HIR source non-generic `PersistentTypeId` |
| 3 | `PersistentGenericTypeId` |
| 4 | `PersistentFunctionId` |
| 5 | `PersistentGenericFunctionId` |
| 6 | `PersistentConstructorId` |
| 7 | `PersistentPropertyId` |
| 8 | `PersistentExtensionPropertyId` |
| 9 | `PersistentObjectValueId` |
| 10 | `PersistentTypeAliasId` |
| 11 | `PersistentPropertyAccessorId` |
| 12 | HIR `PersistentFieldId` |
| 13 | HIR `PersistentEnumVariantId` |
| 14 | HIR `PersistentEnumVariantFieldId` |
| 15 | HIR `PersistentExactTypeId` |
| 16 | 当前Cone direct-public `PersistentExportBindingId`；M23-5在同一identity表增加resolved re-export，route/provenance进入新surface section |
| 17 | HIR `PersistentCallableApplicationId` |
| 18 | HIR `PersistentGeneratedCallableId` |
| 19 | HIR generated `PersistentTypeId` |
| 20 | `PersistentDispatchSlotId` |
| 21 | `PersistentInitializationUnitId` |
| 22 | `PersistentSourceContextId` |
| 23 | `PersistentLocalBindingId` |
| 24 | HIR `PersistentLocalValueId` |
| 25 | `PersistentCallbackRegistrationId` |
| 26 | `SourceNativeExternalContractRecord` |
| 27 | HIR `OdrGroupId` |
| 28 | HIR `OdrMemberId` |
| 29 | `DefinitionOriginRecord` |
| 30 | `NativeBoundaryTypeDefinitionRecordV1` |

MIR foundation的field 1…12精确为：

| field | table |
| ---: | --- |
| 1 | MIR `PersistentExactTypeId` |
| 2 | MIR `PersistentGeneratedCallableId` |
| 3 | MIR generated `PersistentTypeId` |
| 4 | MIR generated class/struct `PersistentFieldId` |
| 5 | MIR generated enum `PersistentEnumVariantId` |
| 6 | MIR generated enum `PersistentEnumVariantFieldId` |
| 7 | `MirCallableSignatureRecordV1` |
| 8 | MIR `PersistentLocalValueId` |
| 9 | `PersistentCallbackApplicationId` |
| 10 | `MirCallbackApplicationRecordV1` |
| 11 | MIR `OdrGroupId` |
| 12 | MIR `OdrMemberId` |

`MirCallbackApplicationRecordV1`恰为map `1=application, 2=managed_adapter: CallableSignatureSubjectV1, 3=managed_signature: ExactCallableSignature, 4=ForeignCallbackStorageAbiV1, 5=CallbackMode`；v1 storage ABI唯一tag为`ClosureResultRootsThrowableToU32=1`。它按application id排序并从registration/context重放替换，不得命名为Registration record或提前混入target-specific C bridge。

LIR foundation的field 1…22精确为：

| field | table |
| ---: | --- |
| 1 | LIR `PersistentExactTypeId` |
| 2 | `PersistentLayoutId` |
| 3 | `PersistentScanId` |
| 4 | `PersistentDispatchTableId` |
| 5 | `PersistentStaticStorageId` |
| 6 | `PersistentImmortalObjectId` |
| 7 | LIR `OdrGroupId` |
| 8 | LIR `OdrMemberId` |
| 9 | `PersistentCallableBodyId`，使用`RuntimeIdentityRecord` |
| 10 | `PersistentSafepointSiteId` |
| 11 | `RuntimeTypeMappingRecordV1` |
| 12 | `SafepointMappingRecordV1` |
| 13 | `PersistentSymbolRequest` |
| 14 | `NativeExternalContractRecord` |
| 15 | `CanonicalCAbiSignatureFingerprintRecord` |
| 16 | `CanonicalCAbiLayoutFingerprintRecord` |
| 17 | `GeneratedBridgeUnitId` |
| 18 | `GeneratedBridgeAtomId` |
| 19 | `LirCallbackBridgeRecordV1` |
| 20 | `NativeLinkRequirementId` |
| 21 | `ObjectDefinitionPlanId` |
| 22 | `ObjectDefinitionAtomId` |

除callable body外，identity table使用`CborIdentityRecord {1=typed id, 2=nested Wire-CBOR key}`；body独占`RuntimeIdentityRecord {1=body id, 2=RuntimeEncode(key) bytes}`，不得提供CBOR mirror。LIR callback record恰为`{1=application, 2=canonical C signature fingerprint, 3=bridge unit}`；三层validator逐项证明MIR application、正规化后signature和`CallbackTrampoline(signature, context_index)` unit一致。field 15/16还必须携带canonical preimage供reader以对应domain重算，不能信任bare digest。

foundation中的layout/scan/dispatch table只有identity key，不是通用ABI证明；它也不包含public lookup、template body、dispatch implementation、ODR definition/member闭包、object range或runtime image record。M23-6必须增加独立、required的完整layout/ABI/scan section并在`ValidatedArtifactClosure<Compile>`上取得跨Cone witness后，Compile API才可暴露这些能力；不得悄悄扩写foundation `/1` payload或把native-boundary最小witness冒充通用layout。

MIR section提供下游MIR需要的、不从HIR name重建的关系：

- HIR persistent source id到external callable/global/constructor/accessor symbol的typed bridge；
- exact ancestry、virtual/itable slot identity、default implementation/adjust thunk与dispatch table schema；
- param-free concrete entity、已发射specialization及其ODR/linkage record；
- callback/coroutine/closure等可跨Cone引用的hidden ABI relation。

LIR section提供：

- param-free exact layout、field offsets/alignment、canonical C storage/layout contract与recursive scan；
- TypeDescriptor/parent/interface/callable typed refs和symbol；
- exported exact generic application若已经materialize的layout/ODR record；
- external target完整calling convention、return convention、effect/root-plan类别；
- current image descriptor所需的root/immortal/init/type/safepoint摘要。
- `DefinitionVerificationSurface`：每个受检strong/ODR owner的canonical LIR/constant definition payload、`LirDefinitionFingerprint`，以及member-independent `ObjectDefinitionPlan`/`DigestFinalizationPlan`。payload使用与wire schema同样的typed id和canonical field tag，不能保存session arena顺序；最终ODR/registration/runtime-image digest不进入payload或LIR own-layer。LIR section中的`LirSemanticProjection`只含canonical定义、layout/ABI/scan与typed external relation；packager随后把producer/object verifier输出的member assignment、atom range/boundary和materialized patch site保存为manifest中的`ObjectVerificationProjection`。两种投影通过typed plan/intent id连接，但codegen不能回写已定稿LIR section，也不得把后者的物理分片/placement字段混进LIR semantic fingerprint；

consumer新建的generic specialization由本次LocalConcrete HIR逐层生成本地MIR/LIR；只有引用既有上游param-free/materialized实体时才消费external meta。LIR lower不能因找到上游layout就跳过specialization key验证，也不能为同一个persistent type建立第二个non-ODR TypeDescriptor。

三层reader分别返回`ImportedHirSet`、`ImportedMirSet`、`ImportedLirSet`；不存在返回无类型map的通用`read_metadata`，也不存在Export template id到Concrete id、HIR callable到link symbol的unchecked cast。

### 4.5 decode验证与资源上限

`.slib`是编译输入，即使来自本地cache也不能使compiler panic或越界。reader至少验证：

- archive/member count、section byte size、CBOR nesting、string/source length、arena/table entry count和递归type/body深度上限；
- member directory的canonical id/name/order、role与purpose组合、required capability、未声明/缺失payload及全部payload length/hash；先按4.1求effective purpose closure，unknown optional capability只允许在完整envelope验证后跳过，与effective purpose相交的unknown required capability必须在解码该payload前拒绝；
- index范围、typed kind、identity hash、owner/decl/application关系、non-empty与exact-arity不变量；
- 每条external ref都能由manifest direct/transitive support graph定位到唯一origin artifact；
- public/inheritance/hidden/default用途不被非法互转，access witness引用完整；
- MIR/LIR bridge覆盖HIR宣称的每个external executable/layout需求且不存在额外同id矛盾记录；
- canonical LIR definition leaf、typed digest DAG/patch单writer、每个plan/intent恰有一个合法member/offset materialization、带`SlibMemberId`的actual object leaf、ODR member/fingerprint、runtime type id、SafepointId、symbol/mangling重新计算一致；
- 构造`ValidatedGraphArtifact`只验证manifest graph identity/dependency/kind/target、目录、全部payload envelope/hash与Graph预算；构造`ValidatedCompileArtifact`再解码并结构验证HIR/MIR/LIR及其跨层bridge；只有构造`ValidatedLinkArtifact`才解码LIR verification surface、运行Link-required extension handler，并要求每个`LinkObject`符合目录声明的target/object format和verifier capability，包括target profile的relocatable-kind/load-command/section/symbol/relocation allowlist与无autolink门禁。Link view中全部object联合起来恰有一个manifest指定的image owner，任一definition range、patch、stackmap contribution、defined symbol与undefined-symbol use都落在声明的同一member内并有唯一typed owner/requirement；非link blob没有object解析或link路径。较弱view类型上不能调用更强view的metadata/object API；
- source path为normalized relative path，span/line offsets在声明的source长度范围内；
- target/layout/ABI与当前profile完全一致。

M23 v1的默认`SlibDecodeLimits`固定为：archive总长不超过2 GiB、bootstrap `manifest.cbor`不超过64 MiB、非manifest member数不超过65,536、单个metadata section不超过256 MiB、单个link object或extension blob不超过1 GiB、单个diagnostic attachment不超过512 MiB、CBOR nesting不超过128、任一table不超过16,777,216项、单个text/bytes语义字段不超过16 MiB、type/body/owner/path递归验证深度不超过1,024。member上限只是集中式资源门禁，不表达“一个Cone通常有几个object/blob”。所有长度先以checked `u64`运算，再验证可转换为host `usize`；不预分配声明值而未核对剩余bytes。除逐字段上限外，reader还必须对decoded heap、node/edge/text总数及验证work建立集中式累计预算；object capability handler共享同一member budget并对section/symbol/relocation/string-table数量做有界解析，不能各自重新获得一套无限预算。v1没有压缩member，因此不存在解压后尺寸旁路。未来放宽limit可以是reader capability变化，但当前实现、错误和边界测试必须使用同一集中常量，不能各section散落不同默认值。

上述累计预算也属于`SlibDecodeCostModelV1::DeterministicLogicalCostV1`，默认值精确为：decoded logical heap 2,147,483,648 bytes、node 16,777,216、edge 67,108,864、owned/copied bytes 1,073,741,824、validation work units 268,435,456。record/sum/collection node各计64 logical bytes，collection element slot 32、graph edge slot 16、ready-set element 40、pending remap entry 96，owned byte/text/canonical temporary按请求长度1:1另计；同一对象承担多个角色逐项累加，replace/shrink不返还额度。每个CBOR item occurrence、archive header/member、interner probe各扣1，每次capability dispatch固定扣32，foundation handler另各扣64；SHA-256长度`L`一次预扣`floor((L+72)/64)`，stable Kahn对`n`个node与`e`条边固定扣`n*ceil_log2(max(n,2))+e`。计费不读取allocator rounding、hash seed、实际比较/heap probe次数或worker时序。

`MetadataSection.payload`、`ManifestSection.payload`、semantic contribution projection与member backing range是carrier bytes，分别受section/member/manifest/archive上限，不重复套16 MiB semantic-leaf限制；known handler进入inner schema后，每个真实text/bytes leaf仍受该限制。foundation projection必须借用同一immutable inner range或stream hash，不能复制整份payload；把大leaf包成bstr、拆成相邻字段或在handler私有buffer复制都仍扣同一个累计meter。outer envelope与inner payload各自从CBOR depth 1开始，但node/heap/work meter绝不重置；所有输入驱动分配先checked预扣，再用fallible exact reserve，失败返回typed resource error且不提交partial world。

单个artifact通过不代表完整显式闭包可以无限放大资源。`scoop`、直接`scoopc`与program-link共用一份`SlibClosureDecodeLimits`，在打开全部payload前累计Cone数、manifest/member总bytes、decoded entity/edge/text与各capability handler work；默认值由toolchain profile集中版本化并进入兼容配置。超过预算返回确定的resource-limit诊断，不通过并行度、重复路径、diamond引用或先逐个解码再commit绕过；同一`ArtifactFingerprint`在闭包内只计一次，冲突artifact仍在去重前报告。

解析分“wire decode → structural validation → typed remap → semantic-world commit”四步；任何失败丢弃整个artifact，不留下半注册entity。错误包含artifact coordinate、section和field/index路径，但不把不受信任字符串当格式串或执行建议。

### 4.6 fingerprint与失效

- HIR fingerprint使用domain `scoop-hir-semantic-v1`，覆盖public/inheritance/template/default/const/re-export/prelude、declared source extern contracts及其typed closure；
- MIR fingerprint使用domain `scoop-mir-semantic-v1`，覆盖external symbol、selected extern contract bridge、dispatch、ancestry、ODR executable relation；
- LIR fingerprint使用domain `scoop-lir-semantic-v1`，覆盖`LirSemanticProjection`中的target ABI、layout、scan、TypeDescriptor、call signature、完整target-native extern contract与canonical LIR definition payload；它明确排除`SlibMemberId`分配、object分片、boundary/range、patch offset、archive placement及`DigestFinalizationPlan`中的纯物理关系。这些字段仍由member hash、code/artifact fingerprint与object/link verifier覆盖；仅在object之间重分片而不改变LIR语义不能迫使dependent重编译；
- code fingerprint由packager使用domain `scoop-code-v1`，依次覆盖目录中每个`LinkObject`的`LinkMemberFingerprint`、每个known Link-required extension按同样排除物理archive name计算的link-member fingerprint及handler产生的canonical `NativeLibraryRequirements`、canonical target-tagged native library link requirements、`CanonicalDefinedLinkSymbolOwnerSet`、`CanonicalUndefinedSymbolRequirementSet`和`CanonicalNativeExternalContractSet`。成员按`SlibMemberId`排序；后四张表按各自typed key排序并编码完整canonical payload，不编码诊断origin。object finalizer只能产出逐member finalized/verified结果，不能在全部link-purpose成员、definitions、requirements与contracts就绪前计算composite code fingerprint。optional blob或diagnostic attachment不进入该值；因此插入一个排序更早的optional blob即使改变后续物理ordinal/name也不会使code fingerprint变化。把同一payload从optional改成Link-required用途则role/purpose改变并必然改变code fingerprint。

前三项都是Merkle fingerprint，并统一使用一个Wire-CBOR product：`LayerFingerprintInputV1 {1=context, 2=contributions, 3=support_edges}`，其中contribution为`{1=section location, 2=capability, 3=sink, 4=validated canonical inner payload bytes}`，support edge为`{1=origin ConeIdentity, 2=edge role CapabilityId, 3=corresponding dependency layer fingerprint}`。`layer_fingerprint = DomainSeparatedCborHash(layer_domain, LayerFingerprintInputV1)`，三个domain分别为`scoop-hir-semantic-v1`、`scoop-mir-semantic-v1`与`scoop-lir-semantic-v1`。contribution按`(location tag, CapabilitySortKey, sink tag)`排序，edge按`(origin raw id, role CapabilitySortKey)`排序并以完整key去重。support edge至少覆盖re-export target、signature/inheritance closure、generic hidden/default target及MIR/LIR external bridge；不能只保存persistent id而漏掉其内容fingerprint。`CrossConeUseSet`给出精确边集合；保守实现可纳入全部direct dependency同层fingerprint。由此A re-export B时，即使A本地binding bytes不变，B的相关层变化仍改变A，再使只依赖A的C失效。

下游compile key依赖所消费dependency的三层Merkle fingerprint；最终link key另依赖完整transitive code/native-library/extern-contract fingerprint与全部`OdrDefinitionFingerprint`。optional full-source、opaque attachment或producer timestamp等非语义数据只改变完整`ArtifactFingerprint`，不得使下游重编译或重链接；任何缓存命中都不能跳过artifact envelope、required capability、Merkle edge、extern contract或ODR验证。

## 5. Pipeline、core分离与工具边界

### 5.1 AST与parser

以下是M23完成后的稳定输入形状。AST `SourceFile`新增完整header；文件的跨stage来源由外层typed input承载，不把host path塞进语法节点：

```text
IdentifiedSourceInput { identity: SourceIdentity, text: SourceText }
IdentifiedParsedSource { identity: SourceIdentity, ast: SourceFile }

SourceFile {
    package: PackageSyntax,
    imports: Vec<ImportSyntax>,
    declarations: Vec<Decl>,
    span: Span,
}

ImportSyntax = Exact { exposure: Local | PublicReexport, path, alias }
             | Star  { exposure: Local | PublicReexport, path }
```

省略package使用显式`RootPackage` variant，不用空字符串。qualified path是非空identifier sequence，package/owner分界留给HIR基于typed namespace解析。parser按package、每条import、顶层declaration分别恢复；任一诊断时该file AST仍按既有规则整体丢弃。

分阶段时，M23-1的外层先使用经验证、不可序列化的request-local `Stage1SourceHandle`与`AllParsedSources`；它只是当前编译请求的typed handle，不是identity。M23-2再把外层原子替换为上述`SourceIdentity { cone, logical_path }`，不修改M23-1已经冻结的`SourceFile` header AST。详细契约见`docs/milestone23/stage1/DESIGN.md`。

### 5.2 HIR semantic world与resolver

`scoopc`把当前Cone所有AST、manifest semantic projection、direct dependency `ImportedHirSet`、显式传入的transitive support artifact map及trusted core capability交给hir-lower。HIR先建立只读`SemanticWorld`：

- 每个provider有session-local `WorldConeId`和persistent `ConeIdentity`；
- public binding只从direct surfaces导入，hidden support只可沿已绑定template edge访问；
- current package、exact/star/prelude scope预先解析为typed binding groups；
- 同一origin经钻石路径只intern一次；
- 当前Cone声明和imported声明都投影成同一种`CallableView`/nominal/property view，来源不改变applicability。

输出仍严格为当前Cone的`ExportHir`与`LocalConcreteHir`。此外产生结构化`CrossConeUseSet`：`LookupObservationSet`保存HIR实际观察的完整候选/binding surface及负查询，`SelectedExternalSet`保存后续stage实际使用的external semantic id、materialized specialization和link requirement。M23 v1 compile key仍保守消费全部direct dependency HIR Merkle fingerprint；MIR/LIR只按selected set投影，不扫描所有dependency meta猜哪些需要导入。

所有**源码语义错误**必须在parser/HIR结束；locator/DAG/cache由`scoop`、当前manifest与dependency-input集合由`scoopc`、wire/member/capability损坏或不兼容由slib reader、ODR/final artifact冲突由独立link stage报告。MIR以后不补源码名称、visibility、generic inference或import错误。

### 5.3 stage输入边界

```text
slib-read:  ExplicitSlibPaths + ArtifactPurpose<P> + DiagnosticsPolicy
            -> ValidatedArtifactClosure<P>
parser:    IdentifiedSourceInput -> IdentifiedParsedSource
hir-lower: CurrentConeParsedSources + ImportedHirSet -> ExportHir + LocalConcreteHir + CrossConeUseSet
mir-lower: LocalConcreteHir + SelectedImportedMir -> Mir + MirMeta
driver:     TargetSelectionRequest -> ResolvedTargetProfileV1
lir-lower: Mir + SelectedImportedLir + ResolvedTargetProfileV1.lir_target
            -> Lir + LirMeta
codegen:   CurrentConeLir(with member-independent definition/digest plans)
           + ResolvedTargetProfileV1.{lir_target, backend}
           -> ProvisionalLinkObjectMembers + MemberMaterializationIndex
              + GeneratedNativeMemberInputs
native:    GeneratedNativeMemberInputs
           + ResolvedTargetProfileV1.{lir_target, c_bridge_toolchain}
           -> ProvisionalLinkObjectMembers + MemberMaterializationIndex
object:    ProvisionalLinkObjectMembers + CurrentConeLir.ObjectDefinitionPlan
           + MemberMaterializationIndex
           -> VerifiedLinkObjectMembers + VerifiedObjectDefinitionIndexes
              + CanonicalDefinedLinkSymbolOwnerSet
              + CanonicalUndefinedSymbolRequirementSet
slib:      manifest semantic inputs + ExportHirMeta + MirMeta + LirMeta
           + VerifiedLinkObjectMembers + TypedAuxiliaryMembers
           + CanonicalNativeLinkRequirements
           + CanonicalDefinedLinkSymbolOwnerSet
           + CanonicalUndefinedSymbolRequirementSet
           + CanonicalNativeExternalContractSet
           + VerifiedObjectDefinitionIndexes -> .slib
scoop:     RootManifest + Locator/Search/CachePolicy
           -> ResolvedBuildGraph
              + dependency-first sequence<ScoopcInvocation> + RootSlib
runtime:   ResolvedTargetProfileV1.{lir_target, c_bridge_toolchain,
                                    runtime_build}
           -> ValidatedRuntimeArtifact
program-link:
           ValidatedArtifactClosure<Link> + ValidatedRuntimeArtifact
           + ResolvedTargetProfileV1.{lir_target, final_link}
           + NativeLocatorPolicy
           -> VerifiedProgramDescriptorObject + ResolvedLinkPlan
              + FinalLinkEvidence + VerifiedExecutable
```

- `.slib`容器读取只在slib crate；完整dependency graph/cache只在`scoop`，当前Cone stage调度只在`scoopc`，最终closure/link只在program-link组件。stage implementation crate不打开archive、manifest或上游source；
- `ArtifactPurpose<P>`的marker只允许`Graph`、`Compile`、`Link`；返回值保留同一个`P`，API没有擦除proof的无参数`ValidatedArtifactClosure`。`ResolvedBuildGraph`只持有source projection与bounded prebuilt manifest summary，不能冒充任何validated artifact view；对每个实际`.slib`，`scoop`必须从同一`DecodedSlibEnvelope`独立构造并保留`ValidatedArtifactClosure<Compile>`与`ValidatedArtifactClosure<Link>`，两者都成功后节点才可成为cache hit、已发布上游或最终library结果。`scoopc`只消费前者，program-link只接受后者；单独构造`<Graph>`只可用于已存在artifact的graph检查，绝不提升为Compile/Link。diagnostic解码结果作为`DiagnosticsPolicy`产生的decorator附着在任一view上，不能转换或提升base purpose；
- imported meta类型定义在对应IR/meta crate。`scoopc`按`CrossConeUseSet`从完整reader结果投影出`SelectedImportedMir`/`SelectedImportedLir`；mir-lower只依赖HIR输入/MIR输出类型，lir-lower只依赖MIR输入/LIR输出类型；
- codegen仍只接收本Cone完整LIR，上游target是typed external ref；
- fingerprint finalization消费3.4已经验证的typed DAG。LIR metadata提供canonical ordinary source signature、exact type/target layout、`RefScan`和`CanonicalLirDefinition`；finalizer分别以`scoop-source-signature-v1`、`scoop-layout-v1`、`scoop-scan-v1`与`scoop-lir-definition-v1`计算四类semantic leaf，object verifier同时证明实际descriptor/scan bytes与这些语义值一致。全部`ProvisionalLinkObjectMembers`中由`MemberMaterializationIndex`绑定的graph-managed patch site（包括semantic leaf落槽）初始为零，其他字段不得被finalizer改写；
- verifier按member及node的纯`normalize`视图计算object support、definition与stackmap leaf并回填其patch sites，未直接落槽的leaf保存在带member id的`VerifiedObjectDefinitionIndexes`。随后以完整LIR/object/stackmap input计算ODR节点，以record kind/semantic id、6.1封闭static shape、referenced registration identity及声明的semantic/object/stackmap input计算strong registration节点；交叉record只按typed identity引用，除非对方digest就是当前record的显式字段；
- runtime-image节点最后从6.1的canonical `RuntimeImageRecordKey`序列、Cone/runtime/target/dependencies计算。每个node都只观察声明的传递依赖digest；own、peer、descendant与无关槽固定归零，因此同一个graph以任意合法topological traversal得到相同结果。回填完成后逐member重跑graph、atom、EH、stackmap、layout/scan与patch-slot verifier，才产出不可修改、按`SlibMemberId`排序的`VerifiedLinkObjectMembers`；
- packager在全部verified link object、known Link-required blob handler输出、canonical defined-symbol owner、undefined-symbol requirement、native library requirement及完整extern contract set就绪后计算4.6的composite `CodeFingerprint`，再计算member/manifest hash与`ArtifactFingerprint`；optional/Diagnostics handler输出不得混入link fingerprint。最终program link时才计算`GraphFingerprint`。artifact reader从LIR verification surface、逐member object与manifest graph逐层重算到runtime image/code/artifact，而不是信任producer历史顺序。不能让codegen回写已经定稿的LIR meta，也不能在pack之后修改任何成员；
- packager只消费各stage正式输出、typed member、verified object与typed verifier index，不反向读取implementation crate内部arena；artifact reader不信任manifest range，仍从对应member bytes重算；
- dump必须区分`local`/`external(origin@coordinate)`/`odr(group)`，但显示字符串不参与语义。

### 5.4 `scoop.core`独立编译

sysroot固定提供reserved core coordinate `scoop:scoop.core:0.1.0`与source/artifact slot。它的Cone name与源码package当前同为`scoop.core`只是发布约定，不建立通用推导。`scoop`在需要时通过受信任的内部bootstrap请求调用`scoopc`编译core；普通single-Cone调用只接受并验证既有core artifact，不能自行重建另一个Cone：

- core不隐式依赖自身；
- 只有来自trusted sysroot locator的该Cone获得`IntrinsicAuthority::Core`；
- core HIR export包含普通public surface、typed prelude binding表、全部well-known compiler/runtime core relation；
- 每个普通Cone把已验证core `.slib`作为direct dependency加载，不再把core source与用户AST拼进一个scope；
- core source/artifact版本、semantic fingerprints与compiler ABI不匹配时由`scoop`先重建trusted source，再把结果作为trusted input传给后续`scoopc`；不能退回旧的“同单元编译”旁路；
- runtime需要的String TypeDescriptor等well-known地址由最终program descriptor的`ScoopRuntimeCoreBindings`传入，不再依赖硬编码`scoop_td_String`源码符号。

core prelude是其typed metadata的一部分，至少能表示package star与`Option` variant scope。用户Cone不能在manifest自定义全局prelude，也不能伪造core marker；普通library仍可由调用方显式/star import。

### 5.5 `scoop`、`scoopc`与独立program-link

M23固定三层工具边界。面向用户的umbrella binary是`scoop`；`scoopc`是可直接调用的低层single-Cone compiler；program-link是独立library/stage，由`scoop build`、`scoop run`和`scoop link`共用，M23不再提供“最后一次`scoopc`顺带链接”的第二条生产路径：

workspace中的实现映射也固定下来：现有`compiler/driver` package继续产出`scoopc` bin/lib，但lib只暴露single-Cone请求；新增`compiler/scoop` package产出薄`scoop` bin与可单测的build orchestration lib；新增`compiler/linker` package承载artifact-only program-link；新增`compiler/runtime-build`只消费`lir_target + c_bridge_toolchain + runtime_build`三项validated projection，把它们指定的受信任runtime source set构建成`ValidatedRuntimeArtifact`；`compiler/manifest`与`compiler/protocol`分别承载两端共享的manifest projection和版本化子进程消息。`scoop`可以依赖manifest/protocol/slib/linker/runtime-build，但不得依赖`scoopc` lib、parser/lower/codegen implementation crate；`scoopc`不得依赖`scoop`、runtime-build或program-link。这个Cargo dependency方向把“每个Cone必须跨进程、跨`.slib`边界”变成可审计的结构约束，而非调用习惯。

```text
scoop build <root-input> [--cone-path <root> ...]
            [--library-path <root> ...] [-o <artifact-or-binary>]
scoop run <root-input> [--cone-path <root> ...]
          [--library-path <root> ...] [-- <program-arg> ...]
scoop link --root-slib <root.slib> --dependency-slib <upstream.slib>...
           [--library-path <root> ...] -o <binary>

scoopc build <Cone-root-or-Cone.toml-or-file.scoop>
    --direct-slib <direct.slib>...
    --support-slib <transitive-support.slib>...
    --out-slib <current.slib>
```

`root-input`是第1.3节的封闭sum：目录或`Cone.toml`选择manifest Cone，regular `.scoop`文件选择single-file root；显式文件operand始终优先，不查找相邻manifest。`scoop run`对library root报错，对executable manifest root与single-file root采用同一路径。target profile与sysroot仍是每个命令的显式、规范化配置，示意中省略。所有CLI path只作定位或当次诊断展示，不进入Cone/member/entity identity、fingerprint或semantic source identity；大量dependency以后可以增加与上述参数逐项等价的response-file传输形式，但它不能成为第二套graph/artifact协议。host native library search path只属于`scoop`/program-link；`scoopc`仅把源码与member capability产生的逻辑native requirement写入`.slib`，不能让当前机器的`-L`选择改变single-Cone semantic/code artifact。单文件旁边的`.c`/`.cc`、archive或blob不会被自动发现；需要它们的调用方或fixture runner必须先显式构建出满足已有typed requirement的library，再只通过`--library-path`把候选搜索根交给program-link。CLI不提供无来源的raw object/archive注入参数。

`scoop build <file.scoop>`省略`-o`时，把最终binary原子物化到调用者cwd下的`.scoop/build/single-file/<target-profile-id>/<ResolvedLinkPlanFingerprint>/<sanitized-input-stem>`；fingerprint使用完整值建立目录，示意路径可缩写显示，stem只影响展示文件名而不进入semantic/cache identity。同一输出位置由build lock串行发布，因而并行fixture不会因共享reserved Cone coordinate、相同basename、不同core/toolchain或不同native解析结果而覆盖。显式`-o`只改变materialization destination，不改变artifact或link plan。`scoop run <file.scoop>`默认直接执行本次build得到的已验证binary；它可以位于受检cache或私有临时物化目录，但不额外承诺一个用户可见输出路径，需要保留binary时使用`build -o`。manifest root的默认输出仍位于其自己的build目录，不能与single-file默认路径混用。

`scoop run` 等价于完整执行一次`scoop build`后启动其已验证binary，不存在interpreter、JIT、跳过`.slib`或跳过program-link的快速路径。`--`之后的参数原样作为program argv，不进入build/cache key；child继承调用者的cwd、environment与stdin/stdout/stderr。`scoop`自身的diagnostic/warning只写stderr，不污染program stdout；启动成功后透传program的exit status或signal结果，启动失败则使用独立的tool error状态。

`scoopc build`的输入输出契约如下：

1. 先把当前输入解析为`CurrentConeInput::Manifest | SingleFile`。manifest分支只解析当前manifest的semantic projection并发现当前`src/**/*.scoop`；`path`、`artifact`和search locator对它是不可跟随的数据，缺artifact时不得搜索或编译上游source。single-file分支验证第1.3节固定projection并只读取指定的一个regular file；
2. manifest分支的`--direct-slib`与当前manifest的非core direct dependency逐identity一一对应；`--support-slib`恰好提供这些direct artifact递归引用的其余transitive support closure。两组互斥，argument顺序不参与结果；single-file分支要求两组均为空。reserved core在两个分支中都从trusted sysroot slot单独取得，普通dependency参数不能授予core/intrinsic authority；
3. reader按`.slib`自报的typed identity重建闭包，并对显式artifact graph重新执行完整一致性验证：缺失/重复identity、同identity不同fingerprint、direct/support错分、不可从direct边到达的额外artifact、stale edge、cycle、自环、同一`group:name`多version、executable dependency、非法root/kind、core authority或target/schema/ABI不兼容都在parse当前源码前失败。这里验证的是调用者给出的封闭输入，不解析locator、不选择版本，也不等于构建或调度图；`scoopc`绝不重编译其中任何节点；
4. 对当前Cone执行parser → HIR → MIR → LIR → codegen/native-member production → per-member verification/finalization → pack。当前实现产生多少native object不是接口不变量；packager接收typed member集合并按4.1建立canonical目录；
5. 所有link object联合起来恰好定义本Cone的一个image descriptor；全部undefined relocation按`{SlibMemberId, use}`验证、收集contract/requirement。之后原子写`--out-slib`临时文件，使用Compile与Link purpose各做一次完整round-trip验证，再rename；
6. 无论manifest kind是library还是executable，唯一产物都是当前Cone `.slib`。executable分支结构上额外携带非可选typed main/root gateway metadata，但`scoopc`不生成program descriptor、不读取或构建runtime输入，也不调用最终native linker。

直接生成的single-file `.slib`可交给`scoop link --root-slib`，但linker必须验证它是闭包中唯一root，从trusted sysroot slot取得core，并要求其identity、root dependency edge记录的HIR/MIR/LIR semantic fingerprint及ABI全部匹配；本次core的code/artifact fingerprint另行进入link plan，不要求等于编译root时某个未被dependency table记录的byte-exact artifact。single-file `.slib`继续禁止出现在`--dependency-slib`或manifest locator位置。这样低层`scoopc build <file.scoop>`有完整后续路径，又不会把reserved identity变成可发布library identity或破坏object-only更新只需relink的规则。

`scoop build`拥有完整构建生命周期：先按1.4解析并验证整个DAG，确保cycle、multiversion或ambiguous artifact在启动第一个compiler子进程前失败；随后按canonical dependency-first顺序处理节点。single-file graph仅有trusted core与synthetic root，但也执行同一artifact/view/cache门禁，不得简化为把core source拼回当前AST。prebuilt与cache候选不调用`scoopc`，但只有完整envelope/hash验证及Compile、Link两种独立strong view都成功后才算命中并可复用；unknown LinkObject capability、损坏object或只通过Graph view的候选都不是成功节点。每个source cache miss恰好启动一次独立`scoopc`进程，并只把已经通过同一双view门禁、成功发布的direct/support `.slib`传入。`scoop`必须调用同一toolchain安装中、ABI/schema身份匹配的配套`scoopc`，不能从任意`PATH`挑选另一个版本；子进程请求/诊断使用版本化结构化协议，输出artifact由父进程重新构造Compile与Link view并核对计划。`scoop`不得把两个source Cone的AST/IR放进同一进程，或以调用`scoopc`内部pipeline library的方式绕过artifact边界。M23先串行执行；未来并行ready set也不能让完成时序改变诊断、目录、program graph或runtime初始化顺序。

每次`scoopc`成功后，`scoop`重新读取完整输出，分别构造Compile与Link view，并核对计划中的coordinate、dependency fingerprint、target与cache key，再原子发布cache entry。子进程失败时不再启动其dependent，也不运行link；已经完成的独立artifact可留在content-addressed cache。`scoopc`以结构化诊断通道报告typed error/warning，`scoop`只排序、标注Cone并汇总，不解析或改写面向人的stderr文本。library root只有在同一双view门禁通过后才以其`.slib`结束；executable root也先完整生成并验证同类`.slib`，再把保留的Link closure交给同一个program-link stage。

对executable root，`scoop`还负责取得runtime输入：它从同一个`ResolvedTargetProfileV1`投影出`lir_target + c_bridge_toolchain + runtime_build`，选择受信任runtime source set并调用`compiler/runtime-build`。该组件按这三项固定的target/storage ABI、C compiler identity/flags与source/build rules独立构建，逐object验证并按3.7的精确七字段preimage计算`RuntimeArtifactFingerprint`，最终只返回不可伪造的`ValidatedRuntimeArtifact`。M23不接受外部prebuilt runtime bundle或raw `.a`，也不缓存这层结果。program-link从不打开runtime源码、不调用C compiler；显式`scoop link`同样先走这一步，而`scoopc`完全不参与。

program-link只消费一个`ValidatedArtifactClosure<Link>`（其中root已证明为executable）、`ValidatedRuntimeArtifact`、target/link profile与输出路径；不读取source manifest、locator、cache或编译残留IR。`scoop link`的dependency参数顺序同样不参与结果，stage自行验证root唯一、闭包完整及canonical graph。它按`ConeIdentity`去重，比较全部ODR record，按3.7合并并逐字段核对extern/native member contract，再按canonical Cone order及每个目录的`SlibMemberId`顺序提取每个`LinkObject`恰好一次；known Link-required blob handler只能产生4.1封闭的native library requirement。opaque/diagnostic/unknown optional成员绝不提取或传给native linker，archive物理名与`.o`后缀也不参与选择。物化member时使用program-link创建的私有临时根，以Cone identity分区并以完整`SlibMemberId`生成create-new文件；不同artifact中相同`mNNNNNNNN` raw name不得覆盖，symlink、既有文件或路径逃逸一律失败。native linker argv的稳定顺序仍来自typed Cone/member key，而不是临时路径枚举顺序。

program-link随后稳定合并传递native library requirements：静态archive/object的实际受控贡献封装为3.7的`ValidatedNativeContribution`，shared library/framework封装为`ValidatedDynamicProvider`及逐symbol binding；contract冲突必须在调用native linker前报告，上游member的native unresolved symbol不能因root Cone没有直接声明该FFI而漏掉依赖。它生成并验证一个小型`VerifiedProgramDescriptorObject`，唯一导出`scoop_program_descriptor`，其中按canonical topological order引用每个Cone的image descriptor、root entry和core bindings。Cone object、该program object、validated runtime object、native static contribution、native dynamic provider与profile声明的target-synthetic input共同构成封闭`FinalLinkInput`集合；program descriptor object不是任何Cone `.slib`成员，`GraphFingerprint`也只在此阶段产生。

native library search path与host绝对路径不进入任何`.slib` identity。program-link解析逻辑library requirement、完成3.7的全部pre-link input verification并冻结content snapshot后，构造唯一canonical plan：

```text
ConeLinkPlanInput {
    cone: ConeIdentity,
    artifact: ArtifactFingerprint,
    code_fingerprint: CodeFingerprint,
    link_objects: CanonicalVec<{ member: SlibMemberId,
                                 byte_length: u64,
                                 sha256: Digest256,
                                 verification_fingerprint: Digest256 }>,
    odr_verification_surface: Digest256,
}

PlanRelocatableInputRef =
    ConeObject { cone: ConeIdentity, member: SlibMemberId }
  | ProgramDescriptor { object_digest: Digest256 }
  | RuntimeObject { artifact: RuntimeArtifactFingerprint,
                    object: RuntimeObjectId }

LinkActionIndex = typed zero-based u32

ResolvedLinkAction =
    RelocatableInput { input: PlanRelocatableInputRef }
  | DirectNativeObject { input: ResolvedNativeInputId }
  | StaticArchive {
        input: ResolvedNativeInputId,
        extraction: Ordinary | WholeArchive | ForceLoad,
    }
  | DynamicProvider { input: ResolvedNativeInputId }
  | TargetSyntheticObject { input: TargetSyntheticInputId }
  | TargetSyntheticGenerated { input: TargetSyntheticInputId }
  | GroupStart { group: LinkGroupId }
  | GroupEnd { group: LinkGroupId }
  | ProfileOption {
        capability: CapabilityId,
        canonical_value: bytes,
    }

ResolvedLinkPlan {
    graph_fingerprint: GraphFingerprint,
    cones: CanonicalVec<ConeLinkPlanInput>,
    program_object: VerifiedProgramDescriptorObject,
    runtime_artifact: ValidatedRuntimeArtifact,
    native_inputs: CanonicalMap<ResolvedNativeInputId,
                                CanonicalResolvedNativeInput>,
    target_synthetic_inputs: CanonicalMap<TargetSyntheticInputId,
                                          VerifiedTargetSyntheticInput>,
    ordered_link_actions: CanonicalVec<ResolvedLinkAction>,
    linker_and_toolchain: LinkToolchainFingerprint,
    target_and_deployment: CanonicalTargetDeployment,
    link_profile: LinkProfileFingerprint,
    normalized_global_options: CanonicalLinkOptionSet,
}

ResolvedLinkPlanFingerprint =
    DomainSeparatedCborHash("scoop-resolved-link-plan-v1",
                            ResolvedLinkPlan)
```

`cones`使用canonical topological order，每个Cone的`link_objects`按`SlibMemberId`排序；`CodeFingerprint`与`odr_verification_surface`保证contract/owner/requirement只改metadata而object bytes不变时仍改变plan。input table只保存identity与verified surface；每一项必须被至少一个action引用，也不能引用表外项。`ordered_link_actions`才是native linker语义顺序：其vector ordinal就是`LinkActionIndex`，重复input、archive重复出现、group边界、whole-archive/force-load与profile option occurrence都保留且进入fingerprint；不能把它们去重为map或排序后的set。group必须正确嵌套，option capability是target profile的封闭variant，不能夹带raw argv、响应文件或host path。实际snapshot path/fd只存在于与plan input id一一对应的进程内execution table，不进入canonical plan。

link完成后必须构造与plan一一闭合的evidence，而不是仅保存一份不可关联的link map：

```text
FinalContentInputRef =
    Relocatable(PlanRelocatableInputRef)
  | DirectNativeObject { input: ResolvedNativeInputId }
  | StaticArchive { input: ResolvedNativeInputId }
  | DynamicProviderContent { input: ResolvedNativeInputId }
  | TargetSyntheticObject { input: TargetSyntheticInputId }

FrozenContentSnapshotProof {
    input: FinalContentInputRef,
    content: CanonicalContentIdentity,
    binding: PrivateCreateNewReadOnly
           | PinnedOpenFile { capability: CapabilityId },
    proof_fingerprint: Digest256,
}

PlatformIdentityProof {
    input: ResolvedNativeInputId,
    scheme: CapabilityId,
    canonical_value: bytes,
    pinning_and_revalidation_capability: CapabilityId,
    proof_fingerprint: Digest256,
}

ResolvedLinkActionOutcome =
    RelocatableIncluded {
        action: LinkActionIndex,
        input: PlanRelocatableInputRef,
        final_input: FinalLinkInputId,
    }
  | DirectNativeIncluded {
        action: LinkActionIndex,
        input: ResolvedNativeInputId,
        contribution: NativeContributionId,
    }
  | StaticArchiveSelection {
        action: LinkActionIndex,
        input: ResolvedNativeInputId,
        selected_in_trace_order: CanonicalVec<NativeContributionId>,
    }
  | DynamicProviderOutcome {
        action: LinkActionIndex,
        input: ResolvedNativeInputId,
        result: Loaded(ResolvedNativeProviderId)
              | NotLoaded(NoReferencedImport)
              | NotLoaded(DuplicateOf { first_action: LinkActionIndex }),
    }
  | TargetSyntheticObjectIncluded {
        action: LinkActionIndex,
        input: TargetSyntheticInputId,
        final_input: FinalLinkInputId,
    }
  | TargetSyntheticGenerated {
        action: LinkActionIndex,
        input: TargetSyntheticInputId,
        final_input: FinalLinkInputId,
        generation_evidence: Digest256,
    }
  | GroupBoundaryObserved { action: LinkActionIndex, group: LinkGroupId }
  | ProfileOptionApplied { action: LinkActionIndex, capability: CapabilityId }

FinalLinkEvidence {
    plan: ResolvedLinkPlanFingerprint,
    content_snapshot_proofs: CanonicalMap<FinalContentInputRef,
                                          FrozenContentSnapshotProof>,
    platform_identity_proofs: CanonicalMap<ResolvedNativeInputId,
                                            PlatformIdentityProof>,
    action_outcomes: CanonicalVec<ResolvedLinkActionOutcome>,
    native_input_trace_fingerprint: Digest256,
    inspected_link_map_fingerprint: Digest256,
    dynamic_load_command_fingerprint: Digest256,
    executable_digest: Digest256,
    final_verifier_fingerprint: Digest256,
}
```

每个action按同一index恰有一个同variant outcome；static archive的空`selected_in_trace_order`就是显式零抽取结果，不得省略。每条archive trace event唯一命中同一input的一个preverified `NativeArchiveMemberRef`并生成一个contribution；direct object不能伪装成archive member，相同basename、相同bytes或相同member名也不能跨archive配对。每个实际load command唯一映射到一个planned dynamic input；每个plan input都有至少一个action outcome，link trace、link map、load command与最终image中不得出现plan外object、archive member、provider、target synthetic input、option或其他事件。每个受控definition/use还须经3.7的owner/requirement/resolution逐项闭合。

M23可以每次都重新链接而不缓存最终binary；若`scoop`实现final-link cache，key恰为`ResolvedLinkPlanFingerprint`，只有link后的contribution、binding与action outcome属于value。命中时，`ContentDigest`分支重新读取并hash当前解析到的完整bytes、重建或验证同一只读snapshot proof；`PlatformIdentity`分支重跑scheme的pinning/revalidation capability proof，不能对它虚构“重新hash文件”。随后重验plan/evidence/executable digest并运行同一个final artifact verifier，或验证target profile明确声明为语义等价的受检CAS proof；任一步失败都按miss重新链接。只有逻辑`-l`名、search path、`.slib`旧code fingerprint、source mtime或link后trace都不足以构成key；无法稳定取得任一实际input、snapshot/platform proof或toolchain identity时必须禁用cache。路径别名选中同一verified content可以命中，plan冻结后把原host path切换到其他bytes也不能改变linker实际消费的snapshot。

linker dead-strip不能丢失未被普通call graph引用但需要登记的image/global/init/callable metadata：program descriptor到每个image是强引用，image再强引用其storage/immortal/init/type/safepoint/callable六张表。`public`语言声明是否被调用不决定metadata存活。

最终artifact verifier至少检查：一个v1 program descriptor、一个no-throw root entry gateway、每个expected image恰好一次且与其artifact的`image_owner_member`一致、无unexpected Scoop image、每个受控object/static contribution及dynamic provider都有且只有一个`FinalLinkInput` origin并与plan/evidence逐项对应、非link blob没有进入最终link、全部linker-visible definition由唯一`FinalLinkSymbolOwner`解释、每个undefined symbol具有唯一`FinalUndefinedSymbolRequirement`并精确解析为受控owner或validated dynamic binding、无unexpected dynamic import、ODR winner唯一、TypeDescriptor双向地址identity唯一、runtime type/safepoint/body id到full key为一对一（同key重复已coalesce）、每个LIR `RegisteredCallableBody`恰有一个entry且不存在不同body id的共址、全部Cone link object贡献并串接的stackmap blob覆盖去重后的site全集，以及M25 EH依赖门禁仍成立。

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
- `InlineBytes`只用于String：`minimum_size == inline_offset == 24`、instance alignment 8、`Inline`的size/stride/alignment均为1，两个scan均null；源码可见的length/index/slice boundary是`Long`，offset 16处的UTF-8 byte length仍是受`0..=INT64_MAX`约束的独立typed `u64` machine count，不是源码`Long`字段；
- `InlineArray`用于`Array`/`MutableArray`：`minimum_size == inline_offset == alignUp(24, inline_alignment)`，instance alignment为`max(8, inline_alignment)`。`Inline`要求`inline_size == inline_stride > 0`；`ZeroSized`要求size/stride为0。只有element `inline_scan`非空时，`object_scan`才是`Array { length_offset: 16, first_element_offset: inline_offset, stride: inline_stride, element: inline_scan }`，GC-free/ZST element时二者均null；
- `AbstractRef`用于interface、managed function type、`Any`/`Nothing`等不可直接分配的exact reference type：minimum/alignment/全部inline字段与两个scan均为0/null；任何allocation入口看到该shape都fatal。

`minimum_size == 0`因此只合法于`AbstractRef`；ZST的“零”存在于`ValueStorageLayout::ZeroSized`与`BoxedValue.inline_size == 0`，其box仍有非零minimum allocation。registration的descriptor/layout fingerprint覆盖整个shape、两个scan及按typed role规范化的pointer内容；reader、link verifier与runtime拒绝其他组合。target profile还给出非零`maximum_managed_alignment`与`maximum_managed_object_size`，任何fixed/box/array layout超过alignment上限都在LIR layout阶段失败，任意allocation超过object-size上限都失败；allocator不能静默降级、wrap或截断。

每个TypeDescriptor的canonical LIR definition非可选地内嵌该exact type重算后的`CanonicalExactTypeDiagnosticName` byte span；其`ObjectDefinition` node把这份LIR semantic leaf作为直接输入，因而`descriptor_fingerprint`承诺name的length与bytes，而不只承诺一个relocation target。实体中的span必须指向内容完全相同的只读diagnostic atom；该atom对ODR type是同组显式member并以自身object fingerprint进入`OdrDefinitionFingerprint`，对strong type则是descriptor definition plan覆盖的typed associated atom。codegen不得用当前import alias、source pretty-printer或consumer-local string替换这些bytes。

共享header同时冻结scan program v1：null只表示empty scan；普通固定偏移节点编码`[count, offset...]`且`0 < count < UINT64_MAX-1`；sequence编码`[UINT64_MAX-1, child_count, child_ptr...]`且`child_count >= 2`；array编码`[UINT64_MAX, length_offset, first_element_offset, nonzero_stride, nonempty_element_scan_ptr]`。canonical `RefScan` normal form要求References offset严格递增且无重复；Sequence不含None或直接Sequence child，先把同层References合并成一个offset并集，再将**包含该合并References节点在内**的全部已规范化child统一按`(child ScanFingerprint, canonical child bytes)`严格递增、去重，零/单child分别折叠为None/该child；Array element必须是已规范化NonEmptyRefScan。producer在fingerprint和发射前规范化到fixed point，reader/runtime拒绝任何非canonical tree。所有offset/stride/count做checked乘加并验证alignment/object allocation range，且每个top-level scan固定受`SCOOP_SCAN_V1_MAX_DEPTH=64`（root为1）、`SCOOP_SCAN_V1_MAX_NODES=65536`、`SCOOP_SCAN_V1_MAX_WORDS=1048576`、`SCOOP_SCAN_V1_MAX_EXPANDED_NODES=1048576`与`SCOOP_SCAN_V1_MAX_CANONICAL_BYTES=16777216`限制；node/word预算对物理共享child按地址只计一次，depth按active path计数，expanded-node与canonical-byte预算则按展开每条parent→child路径重计。validator以memoized checked subtree node/byte cost在继续展开/比较/编码前拒绝超限，active recursion stack仍拒绝cycle，因此共享DAG不能制造指数或重复大leaf的二次工作；常量全部进入runtime ABI fingerprint。

scan的`ScanRuntimeEncodeV1`使用本节下述little-endian scalar/count规则且没有pointer：`None = u32(0)`；`References = u32(1) || u64(count) || each u64(offset)`；`Sequence = u32(2) || u64(child_count) || each encoded child`；`Array = u32(3) || u64(length_offset) || u64(first_element_offset) || u64(stride) || encoded element`。`ScanFingerprint = SHA-256(ByteSpan("scoop-scan-v1") || ScanRuntimeEncodeV1(scan))`；这不是Wire CBOR。共享物理child按semantic tree路径重复编码，但expanded预算保证工作有界。static storage的`scan_kind == None`使用指向canonical `[0]`的非null sentinel以维持record字段契约；该sentinel不是合法Recursive root，TypeDescriptor中的empty `object_scan/inline_scan`则按上述矩阵使用null。

variable instance的exact normalized allocation size也属于ABI：`InlineBytes(len) = alignUp(24 + checked len, 8)`；`InlineArray::Inline(count) = alignUp(inline_offset + checked count * inline_stride, instance_alignment)`；`InlineArray::ZeroSized(count) = minimum_size`。M23没有接收任意signed length的public array constructor：literal/assembly/vararg的logical count由非负component count经checked加法得到且位于`0..=INT64_MAX`；clone/互转则先验证source exact array TD、side-metadata allocation与`0 <= source.size <= INT64_MAX`一致，再读取该size作为target count并使用target refined InlineArray TD分配，不能把它伪装成component求和。任一内部入口看到负count都是compiler/runtime invariant failure，不按名称构造源码异常。String的physical byte count同样位于`0..=INT64_MAX`，再以checked zero-extension形成上述内部typed `u64` count；其长度、乘法、加法与`alignUp`全部使用checked `u64`并要求结果可转成target `size_t`且不超过target profile的`maximum_managed_object_size`。算术溢出、对象过大或底层资源耗尽沿用当前fatal allocation failure，不得wrap、截断或把ZST length当0。未来若增加接收signed length的普通core API，其源码可见异常由该API自行声明/实现。collector读取array count时还用side metadata的exact size反证`first_element_offset + count * stride`位于对象内；损坏对象/descriptor是fatal runtime invariant error。

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
    DomainSeparatedCborHash("scoop-stackmap-record-v1",
                            CanonicalStackmapRecordV1)
```

raw LLVM v3 location顺序原样保留，包括前三个statepoint header location及之后每对base/derived root；live-out也按raw顺序保留并各自带count。raw `Constant`的signed i32 small value先符号扩展为64-bit two's-complement `value_bits`，`ConstantIndex`必须是非负且在pool内，再以pool中的原始u64 bits编码成同一个canonical `Constant` tag；constant-pool index、pool顺序及未引用entry不进入hash。Register要求raw offset为0，Constant/ConstantIndex要求raw DWARF register为0；Direct/Indirect的i32 offset先符号扩展并以其i64 two's-complement u64 bits编码。所有raw reserved、record flags和alignment padding必须为0，unknown kind/越界constant index直接拒绝，不产生canonical record。

function table的ASLR地址由匹配`safepoint_id`的registration解析到其callable record，并必须逐bit等于该record的entry；它与最终`return_pc = function_address + instruction_offset`只用于segment、唯一性和运行期root lookup，二者都不进入fingerprint。`instruction_offset`和`stack_size`仍进入fingerprint。Darwin/AArch64 profile另验证location总数精确为`3 + 2 * root_pair_count`、前三项为8-byte constant且statepoint flags/deopt count为0、每对root为相同的8-byte SP/FP `Indirect`可写slot；registration中的site/body/role/root count与canonical字段必须逐项相等。Rust object/final verifier与C runtime共享Constant/ConstantIndex等价、pool重排/ASLR不变，以及逐字段篡改会改变digest的golden vectors。

RuntimeImage hash stream依次为domain byte span `scoop-runtime-image-v1`、runtime ABI digest、target profile digest、`ConeRecord { group byte span, name byte span, version byte span, identity }`、direct-dependency sequence及六个record table sequence；`RuntimeImageFingerprint = SHA-256(stream)`。它不是`ArtifactFingerprint`；其自身slot不是输入，且绝不输入raw link-object member bytes、ASLR地址、program-level core binding或该digest回填后的bytes。`DigestFinalizationPlan`要求runtime-image node显式依赖record key中实际出现的definition/layout/scan/descriptor/gateway/callable/stackmap节点；object finalizer计算并回填其所在member，manifest保存同值，link verifier核对两份，runtime再从链接record和program ABI输入重算。

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

六类metadata table使用**指向独立descriptor record的pointer span**，而不是把record按值嵌入每个image。一个image只列出由自己实际发射的strong producer或ODR producer record；普通external reference不重复登记。两个consumer各自发射同一generic specialization时，两边表项经link relocation必须指向已coalesce的同一ODR record。runtime分别建立kind-specific `semantic id -> record address`及反向map；callable table还建立`PersistentCallableBodyId ↔ entry address`双向map，要求entry非null、位于可执行segment且同一entry不能属于两个body。所有registered callable atom都是address-significant：不得带`unnamed_addr`，codegen禁用跨不同body id的MergeFunctions/function alias folding，M23 final-link profile拒绝function ICF；只有已经验证为同一ODR callable member的winner可以共址。strong id只允许一个producer；ODR重复只在record地址、group/member、registration/body definition fingerprint及全部关键storage/cell/entry/TD地址相同后去重。每个type registration还必须满足非零`runtime_type_id == descriptor->type_id`。不同persistent exact type id共享一个TypeDescriptor地址、同一type id出现两个地址、或body id/entry不是一一对应都必须fatal，不能依赖constant merge/ICF碰运气。每个LIR `RegisteredCallableBody`即使没有safepoint也必须出现在callable producer表；init/root entry与stackmap raw function address均通过该表验证，不能只靠全局text权限。native extern、runtime及其他不由LIR定义的native member body不属于该集合，按其member verifier capability的独立边界验证。

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

runtime独立验证program-link写入的topological order。每轮ready set是“全部direct dependency都已出现在此前prefix”的剩余image，从中按canonical coordinate byte order取最小；因此dependency总在dependent之前且不受edge存储方向影响。每个Cone中eager unit按`PersistentInitializationUnitId` bytes严格递增。runtime对每个unit经6.2的独立transition wrapper调用no-throw `startup_gateway`，gateway内部再调用generated ensure；因此显式直接依赖可以提前初始化目标，cell/Failed/cycle语义完全沿用M21，同时不会让异常穿过C frame。

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
- root operand不存在、不是regular file/目录、文件名与扩展名无法唯一分类，single-file伪造reserved coordinate、传入非core Cone artifact或将single-file `.slib`当作dependency；
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

`scoop`/`scoopc`/program-link/reader错误不得把host绝对路径写入artifact或可复现golden的semantic部分。manifest Cone的canonical diagnostic source是`group:name:version/src/...`，single-file root则恒为`scoop:single-file:0.0.0/main.scoop`；`SourceLocation.file`也使用该canonical source path。CLI可用当次用户传入路径装饰本地manifest、artifact或single source诊断，但装饰值不参与排序、identity、wire、structured semantic snapshot或cache。测试把snapshot分为两层：typed/structured diagnostic snapshot只含canonical source；CLI presentation snapshot可另含经过fixture runner规范化的relative display locator，绝不保存host绝对路径。diagnostic排序为graph phase/order、Cone coordinate、canonical source path/span或`SlibMemberId`；不能依赖HashMap、archive member物理顺序或filesystem枚举顺序。

可复现门禁使用两个不同absolute checkout/temp output路径、打乱文件创建顺序与dependency input枚举，比较`.slib`逐byte相同及最终symbol/metadata集合相同。最终executable本身若受系统linker UUID影响可在剥离已知非语义字段后比较；不能因此放弃`.slib` bitwise门禁。

## 9. 测试与验收

### 9.1 manifest、graph与parser

- library/executable、path/artifact/search dependency三种locator、implicit core；
- root operand精确区分Cone目录、`Cone.toml`与regular `.scoop`文件；不存在、错扩展名、非regular file和歧义输入稳定失败。显式`.scoop`文件位于某Cone目录中时仍只编译该文件，不读取旁边manifest、其他Scoop/C/C++源码或blob；
- single-file root在相对路径、绝对路径与等价symlink spelling下产生相同Cone/source/entity identity与`.slib` bytes，只有CLI diagnostic decorator不同；不同源码内容必须产生不同cache entry。该artifact只能作唯一executable root（包括显式`scoop link --root-slib`），不能作为可分发artifact发布、作dependency或由manifest locator引用；
- single-file source的Cone dependency闭包恒为trusted core；对非core Cone的import、direct/support `.slib`参数均稳定失败。另行覆盖`@Extern`所需native library只由artifact中的typed requirement加显式`--library-path`成功解析，证明它不被误当作Cone dependency，也不能用raw object注入绕过requirement；
- chain、diamond、无依赖siblings、cycle、自环、多version、same coordinate different artifact；
- 用recording/fake compiler runner锁定`scoop`按canonical dependency-first顺序为每个source cache miss调用配套`scoopc`恰好一次，prebuilt/cache hit不调用；但候选必须完整构造Compile与Link view后才能标为命中、交给dependent或作为library root返回，只通过Graph、Compile失败、unknown LinkObject capability或Link verifier失败都稳定拒绝。cycle等全图错误发生在首次调用前，上游失败后dependent与program-link均不启动；
- 直接运行`scoopc`时，direct/support参数乱序仍产生同一artifact；缺失、重复、错分、stale、额外不可达、cycle、自环、同名多version、executable dependency与伪造core authority的`.slib`闭包稳定失败，并证明它没有跟随manifest locator、读取上游source或产生上游artifact。手工按序调用`scoopc`得到的`.slib`逐byte等于`scoop build`的对应产物；
- `scoop`拒绝来自任意`PATH`的错版本`scoopc`及不兼容结构化协议；child成功后篡改coordinate/fingerprint的输出在cache发布前被父进程拒绝；
- recursive multi-file source、不同package、root package、source排序/路径归一化；
- package/import/public import/alias/star语法与多错误恢复；
- library无main、dependency main不抢entry、root main缺失/重复/ordinary/Unit规则。
- `scoop run <file> -- ...`与对同一输入`scoop build`后执行的结果一致；program argv不影响build key，cwd/environment/stdio继承，stdout不含tool diagnostic，正常退出码、signal与启动失败分别被正确透传/区分。

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
- typed member directory覆盖一个及多个`LinkObject`、diagnostic attachment、optional/Link-required extension blob与混合顺序；raw container缺少link object时仍可构造`DecodedSlibEnvelope`，metadata完备时也可构造Compile view，但不能构造`ValidatedLinkArtifact`或作为M23可发布artifact。Graph/Compile/Link三个`ValidatedArtifactClosure<P>`在编译期不可互换，Diagnostics decorator不能提升purpose。物理名、输入端basename/扩展名和枚举顺序不决定role。把原始标签为`foo.o`的bytes登记为opaque blob时它绝不进入link view，把无`.o`标签的bytes登记为`LinkObject`时它仍进入；archive内二者照常使用canonical `mNNNNNNNN`名。重复id/name、path-like/reserved/special ar成员、thin archive、未声明/缺失payload及member/hash/target错配均拒绝；
- exact wire golden覆盖`SlibMemberRecord`的1…5 field key、stable-key/role tag 1…6、purpose bit 0x1/0x2/0x4/0x8、Capability ASCII边界/NonZero major、四个内建typed id，以及从logical key到member id的固定hash vector；内建object key另覆盖1个、128个与接近table预算的plan/unit集合，锁定count + sorted-set digest、乱序等价、重复/遗漏/跨member重用失败及key长度恒定。callback unit覆盖同一C签名不同合法context index得到不同id、不同closure使用同一pair复用同一id，以及负数、超过`u32`、越界或非`Ptr<Unit>`槽在生成unit前失败。unknown/额外tag或field、reserved bit、非canonical CBOR、capability大小写/空label/过长logical key均拒绝。canonical ar golden逐byte锁定global magic、`name/`、mtime/uid/gid/mode/size padding、odd payload `0x0A` pad和十进制`m00000000`…名；任何等价但非canonical header spelling也拒绝；
- unknown optional capability在完整长度/hash验证后可跳过且不能影响Graph/Compile/Link结果；unknown Link-required capability只在Link view解码前失败，Graph/Compile仍把payload保持opaque。ExtensionBlob上的Graph/Compile/Diagnostics bit、多bit组合和unknown purpose bit都是v1非法role/purpose组合；插入或修改optional blob只改变`ArtifactFingerprint`，不得通过物理ordinal/name间接改变三层semantic fingerprint、`LinkMemberFingerprint`或`CodeFingerprint`；
- unknown `LinkObject.verifier_capability`、Mach-O非`MH_OBJECT`、`LC_LINKER_OPTION`/autolink、未认领external/weak definition，以及Link extension handler试图输出object/raw bytes/argv/path/script/unchecked input都在native linker前拒绝；Scoop、generated bridge、未来C/C++等所有object都只能经显式`LinkObject` role进入统一object/defined/undefined/image验证；
- 多个link object的definition range、patch、stackmap、undefined-symbol use及diagnostic都必须携带正确`SlibMemberId`；跨member range、错owner、零/重复image descriptor或manifest `image_owner_member`错指均在pack/read/link前失败；
- generic concretization predicate逐variant wire round-trip；unknown tag、缺字段路径、binder/persistent type ref越界均拒绝，predicate或字段路径改变必须改变HIR semantic fingerprint；
- 同一artifact由两个direct path加载只intern一个origin world id；
- LocalConcrete、solver scratch、失败candidate、无关private body与absolute path不在archive；
- 每个section删减、bit flip、truncation、bad hash/schema/kind/index/arity/witness/bridge/ODR/source span均返回结构化error且不panic；
- section/member/count/nesting/string/body及各类blob resource limits有边界测试；65,536只是资源上限，不把测试矩阵缩成固定两个object；
- 隔离目录与随机input顺序bitwise reproducibility；
- HIR-only变化、layout变化与object-only变化分别触发正确compile/relink fingerprint；只改变object分片/member assignment/range/patch placement而保持canonical LIR语义时，LIR semantic fingerprint不变但code/artifact fingerprint按实际bytes变化；
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
- 两个Cone把同一ODR member放在不同`SlibMemberId`/object分片时仍得到相同ODR definition fingerprint并可coalesce；member id或range只用于定位，交换canonical definition/stackmap内容仍必须改变ODR fingerprint并失败；
- 后端生成memory/TLS/EH helper与C bridge entry分别具有typed requirement；源码声明公开`memcpy`等support symbol时，完整ABI/library一致可共用、冲突须在native linker前失败，任意私有runtime/Scoop/bridge符号别名或无capability的undefined symbol均拒绝；
- 全部Scoop-owned symbol符合persistent mangling，两个Cone声明同package/name的non-generic实体无link碰撞；
- native extern/library/member-capability requirements从transitive closure完整汇总；同symbol的canonical contract相同则去重，library、function/data/TLS/mutability、C/Scoop ABI、calling convention、参数/结果canonical storage或storage type任一差异均在native linker前失败，诊断包含双方origin及不同字段。未调用的声明同样参与验证；任一link object的`SourceExtern` relocation漏contract或匹配多个contract时拒绝。只改变contract而不改变object bytes也必须使code/link key失效；
- program-link在全新进程中只凭`ValidatedArtifactClosure<Link>`与`ValidatedRuntimeArtifact`成功链接，证明不依赖root编译内存状态或runtime源码；fake native linker argv精确等于按Cone/member canonical order形成的typed link view，每个object一次且没有任何opaque/diagnostic成员。两个Cone内部相同raw archive member名必须物化为不同create-new路径且不可借symlink覆盖；多个object之间互相引用可正常解析，任一link member变化触发relink但不在semantic metadata不变时重编译dependent；
- runtime target/ABI/digest/entry/contract不符、定义program/Cone/Scoop保留symbol、program object多定义或少/多image/root/core relocation，以及Cone/program/runtime/native/target object中的autolink/linker directive都在native linker前失败。专门构造**不会被抽取**但含autolink/directive的static archive candidate，必须在fake linker runner被调用前失败；thin/external/path member、nested archive、bitcode/LTO、未知ordinary member，以及constructor/destructor、EH/unwind、TLS或语言metadata缺少对应capability contract也同样失败；
- static archive trace必须覆盖：零member抽取产生显式空selection并可成功；同名member、相同bytes的不同ordinal仍按parent input + ordinal/offset/length/digest唯一定位；引用另一archive member、未知/未验证candidate、重复trace event或把direct object伪装成archive member均拒绝。shared provider允许空binding，但缺失/重复/错provider binding、意外load command、target默认动态库没有`TargetProfile` request origin，或provider尝试满足/interpose任一`Controlled` requirement都失败；
- `ResolvedLinkPlan` golden覆盖相同archive在不同位置或重复出现、group边界、ordinary/whole-archive/force-load及profile option occurrence；任何顺序变化都改变plan fingerprint。每个planned action/input都有且只有一个evidence outcome，archive未抽取与dynamic未加载也不能省略；extra trace/load/target-synthetic input拒绝。target synthetic还覆盖缺capability、额外definition/use、错误generation rule和content digest变化；
- TOCTOU测试在plan定稿后切换native search path目标、替换原文件或交换symlink，fake linker仍只能读取已验证私有snapshot；snapshot digest/pinning proof不符必须在启动linker前失败。`PlatformIdentity`在首次link及每次cache hit重跑scheme proof，identity或proof失效按miss处理；
- 若启用final-link cache，更换`-L`解析到内容不同的完整native input、runtime artifact、target synthetic input或linker profile必须miss，路径别名指向相同已验证bytes可命中；只改变任一Cone的owner/requirement/contract metadata而object bytes不变也必须通过`CodeFingerprint`改变plan key。伪造只在link后才产生的contribution trace不能改变pre-link key；截断/替换cache value、executable digest不符、trace/evidence与`ResolvedLinkPlan`不符或命中产物未通过同一final verifier时按miss重链，不发布该binary。

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
- 同一或不同Cone的多个link object产生的concatenated stackmap v3 blob在普通/moving-stress下覆盖上游、下游、generic instance、exception、closure、coroutine与foreign callback frame；
- program/image/storage/TD/init/safepoint descriptor的prefix、size、identity、双向地址、fingerprint或pointer range损坏时runtime unit测试稳定fatal；两个strong零尺寸global/init storage分别拥有不可合并token，ODR零尺寸storage只coalesce同一member，伪造共址或错误allocation extent稳定失败；root/init no-throw gateway保证源码异常不穿越C frame；
- 两个以上object产生连续stackmap v3 blob时全部解析；image descriptor位于任意合法`image_owner_member`而非固定文件名时结果不变；ODR重复record精确去重、不同payload拒绝；Darwin profile拒绝`-dead_strip`并证明无stackmap丢失；
- fake runtime compiler分别产生一个、多个及与source unit非一一对应的relocatable object，runtime-build均按typed object record验证并得到与3.7一致的fingerprint；固定object records不变时分别只改变`lir_target`、`c_bridge_toolchain`、`runtime_build`或`runtime_abi`必须得到不同fingerprint，调换field、压成旧的“两字段target/toolchain”preimage或从host补项的golden必须拒绝；重复object id、缺失build evidence、额外未分类object、跨object contract冲突、外部prebuilt runtime bundle与raw `.a`都在program-link前拒绝；
- 最终artifact无重复TypeDescriptor、旧固定`scoop_main`/`scoop_image_*`或未namespaced Scoop symbol，并继续满足M25 EH导入门禁。

新增fixture建议分为：

- `tests/fixtures/m23-imports/`；
- `tests/fixtures/m23-reexports/`；
- `tests/fixtures/m23-generics/`；
- `tests/fixtures/m23-zst/`；
- `tests/fixtures/m23-initialization/`；
- `tests/fixtures/m23-errors/`；
- `tests/slib/`的format/corruption/reproducibility测试。

每个多Conefixture自身含多个Cone目录和manifest，runner以graph root为单位快照每个Cone AST/Export HIR/LocalConcrete HIR/MIR/LIR及最终stdout/stderr。至少一个组合fixture串联re-exported generic default → downstream local type specialization → interface dispatch → delegated generic extension → eager/lazy globals → moving GC/exception。M1–M22及M25全部历史单文件fixture改走正式`scoop build <file>` orchestration后保持语言结果与诊断覆盖；涉及路径的snapshot允许一次受控更新。runner从版本化child protocol或同一orchestration library的typed test result取得AST/HIR/MIR/LIR、warning与diagnostic，不直接调用`scoopc` pipeline library，也不把dump混入program stdout。diagnostic presentation layer可把当次operand规范化为fixture-relative display locator以保持可读性，但canonical semantic source仍为`scoop:single-file:0.0.0/main.scoop`。现有FFI fixture继续由runner显式把配套native source构建成满足既有`@Extern` requirement的library，并仅以`--library-path`传入其搜索根，不引入“发现相邻`.c`”或注入raw object/archive的隐式规则。

## 10. 实现顺序与完成门

### M23-1：source表面、parser与当前编译单元lookup

详细设计见`docs/milestone23/stage1/DESIGN.md`。

依赖M22。实现第2.1节和5.1节的`package`、exact/star `import`、`as`与`public import` AST/parser，以及文件头顺序、恢复边界、root package和当前编译单元内的package/name lookup。本阶段不制造伪artifact或用FQN模拟跨Cone；尚无direct dependency可用时，`public import`和外部import以完整typed诊断结束，不被静默忽略。

完成门：全部新语法均有parser golden、组合fixture与定位精确的negative fixture；当前编译单元内不同package的explicit/star/alias lookup可用；无成功AST分支携带“稍后填充”节点，旧fixture全量回归。

### M23-2：persistent identity与`.slib` wire基础

详细设计见`docs/milestone23/stage2/DESIGN.md`。

依赖M23-1。实现第1.1、3.1–3.3、3.5的identity部分，以及第4章的container基础：`ConeIdentity`、按kind分离的persistent id、exact type/source/unit/callable application/body/safepoint identity、当前generic body所需的最小ODR group/member identity、`PersistentV1` mangler、session-local remap、deterministic CBOR/`ar`、三层metadata的versioned envelope、typed member directory、资源上限、section/member fingerprint、`DecodedSlibEnvelope`、`ValidatedGraphArtifact`及现有payload的Compile decoder。第1.3节single-file reserved coordinate、logical `main.scoop`与diagnostic display locator的分离也在此冻结。后续layout、generic hidden closure、ODR definition、runtime和link payload必须各自使用显式section/capability version加入，不能改写M23-2已经赋义的tag；本阶段foundation artifact不可发布，也不宣称尚无object/image verifier支撑的最终Link view已经完成。

完成门：固定hash/wire golden在arena分配、输入枚举、绝对checkout与临时输出路径改变后保持一致；encode → decode → typed remap → re-encode逐byte相同；corruption/budget/unknown-capability矩阵无panic；目录从第一天起支持任意数量的`LinkObject`、诊断附件和opaque/required blob，不存在“一个`.o`”、固定扩展名或固定producer的格式假设。

### M23-3：single-Cone artifact与core分离

依赖M23-2。实现`Cone.toml` semantic projection、source discovery、library/executable entry sum、`compiler/manifest`与`compiler/protocol`；把`compiler/driver`收缩为只产生当前一个Cone `.slib`的`scoopc`，并建立`--direct-slib`/`--support-slib`/`--out-slib`契约。第5.4节trusted `scoop.core`独立编译和消费同步落地：core prelude经普通typed `ImportedHirSet`/bridge进入用户编译，不增加core专用AST拼接或名称fallback。本阶段成功的普通/SingleFile Cone只允许trusted core依赖；含其他Cone dependency的请求可完成闭包输入验证，但以稳定的阶段能力诊断结束，M23-5才开放其源码语义。

本阶段同时冻结并实现编译器侧`ScoopImageDescriptorV1`布局/producer、当前Scoop LIR/generated bridge两种内建`LinkObject` verifier及基础`ValidatedLinkArtifact`；M23-8实现同一ABI的runtime consumer，不在那时回改布局。这个基础Link view不是只验object envelope：M23-3一并产生并验证当前非generic/strong-only程序所需的六类registration record、`StrongRegistrationFingerprint`/`RuntimeImageFingerprint`及其digest finalization、member-aware strong definition和全部undefined requirement（包括只记录、不在此解析provider的`SourceExtern` contract）。production profile发现M23-2可表达的任意ODR group/member/body/symbol必须拒绝，不能在缺少完整member/definition proof时让native linker任选winner。M23-7在已冻结identity上增加完整ODR member closure、ABI/definition digest与验证，M23-10增加native provider resolution；两者都不能补造M23-3 artifact中缺失的owner、requirement或image relation。

producer可输出任意非空数量的object，验证在全部member的联合定义空间中证明恰有一个image owner；不能把当前通常的object数量固化为协议。由此本阶段范围内成功产出的`.slib`已经能分别通过Compile与Link round-trip，而不是等待后续里程碑补洞。`SingleFile`请求可经同一protocol产生local executable-root `.slib`，公开umbrella CLI在M23-11切换。

完成门：core bootstrap、core-only library和core-only executable均产生可重复、双view有效的`.slib`；空/非空registration table、strong/image digest、跨member owner及extern requirement任一被篡改都会使Link view失败。library无`main`，executable的entry metadata非可选；普通`scoopc`不再把core source与用户AST拼接，也不读取runtime或最终链接。缺core、上游artifact错配或尚未开放的非core语义只稳定失败，不搜索、递归或重建。stage snapshot可通过typed single-file request取得，但M23-11前旧executable fixture driver只能作为明确标记的迁移路径保留，不得被新component调用或当作artifact协议的备用实现。

### M23-4：resolved build graph与调度

依赖M23-3。只实现第1.4与5.5节的构建侧：exact locator、manifest/prebuilt summary、resolved DAG、dependency-first顺序、per-Cone artifact cache和版本化child orchestration。`compiler/scoop` orchestration library在本阶段落地，以recording/fake compiler runner与M23-3的core-only真实节点验证调度；它可以规划含普通dependency的图，但在M23-5前不得把这些artifact暴露成源码候选。所有cache/prebuilt/source节点继续走M23-3已有的Compile/Link双view门禁。

完成门：用typed recording artifacts覆盖chain/diamond/cycle/multiversion、ambiguous locator、stale dependency、cache失效与确定性调度；全图错误在第一个compiler child前失败，上游失败不启动dependent；对本阶段可成功编译的core-only真实节点，手工调用`scoopc`与orchestrator得到逐byte相同的artifact。cache key、诊断顺序和child调用顺序不受manifest枚举或并发完成顺序影响。

### M23-5：多Cone名称语义

依赖M23-4。实现第2.2–2.5与5.2节的`SemanticWorld`、direct/support closure、package binding、exact/star/alias import、re-export、public/internal/private access provenance、default template和non-generic alias，以及相应cross-Cone HIR/MIR/LIR引用投影。本阶段只开放不需要M23-6新增layout/dispatch bridge的成功集合；跨Cone value layout、继承/slot dispatch、receiver-dependent protected access和generic application分别以明确诊断关闭，不允许HIR接受后再由下游stage报“尚未支持”。

完成门：direct与transitive可见性、split package、exact/star/alias冲突、链式re-export、public/internal/private access、default origin/evaluation source、non-generic alias和negative lookup observation/cache失效矩阵通过；所有成功用例产生双view有效artifact，所有暂未开放形态在HIR边界有唯一稳定诊断，不产生残缺IR。

### M23-6：跨Cone layout、typed ABI与ZST

依赖M23-5。实现第3.6节的`ValueStorageLayout`、Scoop ABI payload elision、C ABI拒绝规则、boxing、address/static token与`Array`/`MutableArray<ZST>`，并完成param-free跨Cone layout/scan/TypeDescriptor、inheritance/slot/dispatch bridge及receiver-dependent protected access witness。本阶段新增独立required layout/ABI/scan capability，在完整`ValidatedArtifactClosure<Compile>`上验证依赖witness后才返回通用layout API；M23-2 foundation中服务现有extern/callback闭包的`NativeBoundaryTypeDefinitionRecordV1`、canonical C storage/signature/layout leaf仍只是一条受限native-boundary proof，不能提前冒充本阶段能力。本阶段冻结新增section版本、layout、scan与typed Scoop ABI；既有canonical generated-C storage contract与extern fingerprint bytes不回改，M23-7的specialization只能实例化这些规则，不能重写它们。

完成门：第9.5节的compiler/layout/object级矩阵通过，包括ZST参数/返回、box、static初值/token、array length/index/scan和C ABI negative；provider/consumer对同一exact type得到相同layout/scan/TD identity与fingerprint，跨Cone继承/dispatch产出完备artifact。可由现有单image harness覆盖的runtime行为继续回归；真正多Cone链接后的moving-GC矩阵留到M23-9/M23-11，不在本阶段虚构完成。

### M23-7：跨Cone generic、ODR与generic delegated extension

依赖M23-6。消费M23-2已冻结的specialization/group/member key，实现consumer-side concretization、generic hidden support closure、第3.4节完整ODR member closure、ABI/definition digest DAG、root provenance与跨Cone member-set/definition一致性验证、object materialization relation，以及第7章generic delegated extension property。codegen/native producer、definition/patch/undefined verifier与fingerprint finalizer全部改为collection输入，所有物理定位经`SlibMemberId`，但member id/object分片不进入ODR等价。

完成门：两个sibling Cone对同一specialization产生完整且逐byte相同的group/member/fingerprint，不同定义由纯artifact/object合并验证器拒绝；上游generic可用下游local type实例化；delegate storage/root/cell/failure/init形成一个闭合ODR group。真正的地址coalesce、全程序exactly-once与运行期失败共享由M23-9/M23-11验证，不作为当前无生产linker阶段的伪完成门。

### M23-8：runtime multi-image registry与启动

依赖M23-7。实现第6章runtime侧完整descriptor C ABI、六类registration table、provisional/semantic/image/graph验证、canonical初始化顺序、native → managed no-throw gateway、多个连续LLVM v3 stackmap blob与终止协议；冻结`ScoopProgramDescriptorV1`。本阶段用typed synthetic program descriptor builder驱动runtime测试，不依赖生产linker或扫描weak symbol；M23-3已发射的per-Cone image在这里首次由多image runtime消费。

完成门：3+个synthetic image在任何managed initializer前完成登记；dependency-first/Cone tie-break/unit-id顺序、lazy/eager、failure/cycle、moving GC、exception gateway、损坏metadata fatal、ODR重复record和multi-blob stackmap矩阵通过。这里证明runtime算法和ABI，不声称真实linker已经生成program object。

### M23-9：基础artifact-only program-link

依赖M23-8。实现`compiler/linker`的controlled-input骨架、`compiler/runtime-build`、`ValidatedRuntimeArtifact`、program descriptor object和基础final artifact verifier。为了能够真正调用system linker，本阶段同时闭合**固定target/runtime slice**：target profile非可选地枚举runtime所需的startup/support object、默认system dynamic provider、完整symbol contract、provider/content identity、ordered action和target-synthetic输入；这些输入在基础plan/evidence中逐项出现并使用同一snapshot或platform pinning，禁止依赖linker隐式default、autolink或未追踪输入。program-link只接受`ValidatedArtifactClosure<Link>`、typed runtime/target/profile与输出路径，不读源码、locator、cache或compiler内存状态；源码产生的用户native-library requirement、任意search-root解析或尚未登记的Link-required capability仍稳定拒绝，留给M23-10。

完成门：全新进程只凭无用户native requirement的`.slib`闭包与trusted runtime profile成功链接、验证并运行真实多Cone程序；固定startup/support/system-provider/target-synthetic输入全部在plan/evidence中有typed origin，关闭任一linker default后结果不依赖隐式补全。任意数量的内建`LinkObject`按typed directory各提取一次，opaque blob不进入linker；真实地址coalesce、multi-object stackmap、初始化、moving GC与exception gateway通过。不得以读取source或允许raw额外object来绕过尚未落地的general native闭包。

### M23-10：general native requirement闭包与link evidence hardening

依赖M23-9。把M23-9只接受固定target/runtime slice的plan/evidence推广到第3.7节完整用户native extern contract closure，实现direct object/static archive/general dynamic provider解析、known Link-required blob handler、content snapshot/TOCTOU hardening、完整`ResolvedLinkPlan`/`FinalLinkEvidence`、link trace核对、cache revalidation与最终artifact verifier；在本阶段封闭`.slib v1`的内建Link capability集合。CLI或fixture只能用`--library-path`为已经存在于artifact中的逻辑requirement提供解析候选，不能注入没有typed origin的raw `.o`、archive、linker option或script。

完成门：archive零/多member抽取、dynamic binding、native contract冲突、全部final-input origin、target synthetic、snapshot替换攻击、trace/evidence与可选final-link cache重验矩阵通过；第9.4/9.6节除公开CLI/历史fixture迁移外的完整link闭包得到验证。Cone内未来C/C++ producer仍须新增自己的versioned verifier capability，但不会改变typed member directory或引入单object假设。

### M23-11：umbrella CLI、single-file mode与总验收

依赖M23-10。正式启用`compiler/scoop` bin、配套`scoopc` child protocol、artifact cache发布、`scoop build`、`scoop run`和`scoop link`；实现第1.3/5.5节的manifest/single-file root分流、默认输出物化与执行语义。final-link cache仍是可选优化，不是完成门。M1–M22及M25全部历史fixture在本阶段一次性切到正式`scoop build <file>` orchestration，保留stage dump/warning/diagnostic与运行结果覆盖；FFI companion native code由runner显式构建为满足已有`@Extern` requirement的library，并仅通过显式`--library-path`参与解析。切换与删除旧driver为同一批变更，不留两条生产路径。

完成门：`scoop build/run <file>`对相邻manifest/source/native文件不做隐式发现，只构建一个source + core的synthetic executable graph；`run`只是build + execute，正确处理`--`、stdio/cwd/environment和exit/signal；单文件和多Cone综合fixture、cache失效矩阵、corruption/reproducibility与moving-GC/exception/closure/coroutine/FFI全量回归通过。诊断snapshot允许一次受控迁移到semantic source identity与独立display locator模型，不能把host路径重新写进persistent identity。

每个子里程碑均先`cargo fmt --all`与`cargo clippy --workspace`，再执行对应unit/golden/fixture。M23-3到M23-10期间允许的旧fixture executable driver只是临时保持回归的迁移边界，不能向新pipeline提供artifact、runtime或link能力；M23-11完成时必须删除。最终不得保留旧的“`scoopc`直接把裸文件与core source合并并链接”入口、让`scoopc`递归构建或最终链接的旁路、固定`code.o`/`bridge.o`成员判断、两套symbol mangler、raw arena id wire格式、固定`scoop_image_*` weak fallback或“ODR不一致让linker选择”的路径。

M23系列只有在以下条件同时满足时完成：`scoop`可只经配套`scoopc`的单Cone artifact边界按DAG构建，而直接`scoopc`在缺上游artifact时只失败、不递归；任意library Cone无需`main`即可独立生成可重复`.slib`，executable也先生成同类`.slib`再由artifact-only program-link产生binary；`scoop build/run <file>`使用固定synthetic identity、唯一source和core-only Cone dependency，却不禁止已有typed FFI requirement经显式library search root解析；typed member directory可同时承载多个link object和非link blob，unknown required/optional capability、fingerprint分层与link view均按4.1/4.6工作，任何代码都不按名称、扩展名或数量猜成员；direct/re-export/import层在下游使用同一M16/M17 resolver/default协议；non-generic alias/protected inheritance/generic hidden closure跨artifact边界不泄漏权限且信息完备；reader对完整损坏矩阵无panic；上游generic与下游local type可实例化，重复specialization的全部runtime identity真正coalesce；ZST从HIR status到LIR layout、Scoop/C ABI、box、static token及`Array`/`MutableArray`均通过9.5矩阵；generic delegated extension全程序exactly once；core作为trusted独立`.slib`消费；最终程序先登记全部image/storage/root/TD/stackmap/callable/init metadata，再按canonical DAG顺序初始化并从no-throw typed root gateway运行；多Cone moving-GC/exception/closure/coroutine/FFI组合通过。

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
10. M24 GC-free release hook、M26 String byte API与字符串插值，以及M23之后其他语言backlog；
11. Cone内C/C++源码自动发现、compiler flags/header dependency、native symbol/library contract与static constructor/destructor语义。这不禁止由已验证`@Extern`契约产生的逻辑native library requirement，也不禁止调用方向`scoop`/program-link显式提供用于解析它的library search root；它们不是Cone dependency。CLI不接受没有typed requirement来源的raw native input。4.1已经保证未来C/C++ object与其他blob可进入通用目录，但真正开放该producer前必须另行修订M25门禁：C++异常绝不能穿越Scoop/C ABI边界，若仍禁止`libc++abi`与global destructor，则只能接受满足相应受检profile的native object，不能因成员role为`LinkObject`而豁免。

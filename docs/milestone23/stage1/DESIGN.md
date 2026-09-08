# M23-1 设计：source 表面、parser 与当前编译单元名称语义

版本：0.1（设计完成，待实现；2026-09-08）

依赖：M22

上位设计：`docs/milestone23/DESIGN.md`

规范依据：

- `docs/specs/SCOOP-SPEC.md` 第 9.1.5、12.4.1～12.4.4 节；
- `docs/specs/SCOOP-IMPL-SPEC.md` 第 2.1、2.2、2.11 节；
- `docs/milestone23/DESIGN.md` 第 0、2.1、2.3、5.1、10 章。

本文只定义 M23-1；上位设计与实现规范同步区分 M23-1 的 request-local source handle 和 M23-2 起的 persistent `SourceIdentity`。语言语义仍以上述规范及总设计为准。

## 0. 结论

M23-1 是一个封闭的前端纵切，不是仅能生成新 AST、却无法进入现有 pipeline 的 parser-only 提交。完成本阶段后：

1. 每个 source file 都具有结构完备的 root/qualified package header 和 exact/star import header；
2. parser 完整支持 `package`、`import`、`as`、`public import`，并在文件头层次完成顺序检查与错误恢复；
3. HIR 以当前编译请求中的全部用户 source 共同建立 namespace，完成当前编译单元内的 exact/star/alias lookup；
4. 名称候选层切换到 M23 的最终顺序，并继续复用 M16～M22 的 applicability、MSC、property、typealias、extension 等既有决议规则；
5. 语法合法但本阶段没有能力完成的外部 import 与全部 `public import`，在 HIR 给出稳定、精确定位的错误；
6. 任一成功 AST/HIR 都不含“等待 Cone、等待 `.slib`、等待 persistent id”之类的占位状态。

M23-1 不引入 Cone manifest、source discovery、persistent identity、`.slib`、`scoop`/`scoopc` 新边界或单文件 CLI。当前历史 fixture 仍可把一个用户文件作为仅含一个 source 的编译单元送入同一个前端 API；正式的 `scoop build <file>` / `scoop run <file>` 在 M23-11 接管 fixture 时落地。

## 1. 范围与阶段边界

### 1.1 本阶段交付

- 文件头 grammar、AST、parser 与 parser golden；
- 显式 `RootPackage` 与非空 qualified package path；
- exact import、exact alias、star import 和三种 `public import` 的完整语法表示；
- package/import/declaration 的文件头状态机及独立恢复边界；
- 当前编译单元的多 source namespace 收集；
- 当前编译单元上的 import selector 解析；
- 当前文件 exact scope、current package scope、star scope 与 core prelude 的候选层接入；
- 当前编译单元内的 visibility、重复来源去重、overload group 与歧义处理；
- qualified type path 的“最长 package 前缀 + typed static owner edge”解析；
- 外部 import 和 `public import` 的阶段能力诊断；
- AST/HIR/MIR/LIR golden、positive/negative/组合 fixture 与 M1～M22 回归。

### 1.2 本阶段明确不做

- `Cone.toml`、manifest semantic projection、source discovery 或正式`ConeOutputKind` entry模型；仅以第8.3节legacy adapter维持现有executable回归；
- `ConeIdentity`、persistent entity/source id、mangler、hash 或 wire schema；
- `.slib` 读写、artifact、object、blob、link member 或成员数量约束；
- direct/transitive dependency、cross-Cone `SemanticWorld` 或 split-package provider 合并；
- 成功的 re-export、re-export snapshot、provenance witness 或 public surface 发布；
- 自定义 `annotation class` 声明及其 import target；M12 的 compiler-known 注解不是源码 namespace binding，未来交付通用注解声明时再接入同一 typed import 机制；
- `scoop` umbrella binary、`scoopc` single-Cone protocol、多 Cone DAG、cache 或 link stage；
- 正式 single-file mode；
- dependency-coordinate-qualified 源码名称；
- generic/nested typealias 或任何不属于 M23-1 的新语言表面。

M23-1 不生成伪 `.slib`，不把当前 source 包装成 synthetic Cone，也不以字符串 FQN、host path 或 link symbol模拟尚未存在的跨 Cone identity。

### 1.3 与后续子里程碑的交接

| 后续阶段 | 接收 M23-1 的内容 | 后续阶段新增的能力 |
| --- | --- | --- |
| M23-2 | 不再改变的 header AST 形状 | persistent typed identity、source identity 与 wire/container 基础 |
| M23-3 | 接受一组已解析 source 的前端入口 | manifest/source discovery、独立 core `.slib` 与 single-Cone artifact |
| M23-5 | 已存在的 import scope 与候选层 | `DirectDependency` source、跨 Cone lookup、成功的 `public import`/re-export |
| M23-11 | 单 source 也是合法编译单元这一性质 | `scoop build <file>` / `scoop run <file>` 及 fixture runner 迁移 |

后续阶段可以替换输入载体和增加新的、完整的 source capability，但不能修改本阶段已经冻结的源码 grammar，或让 M23-1 的成功节点重新变成待解析状态。

## 2. 源码表面

### 2.1 Grammar

每个 source file 的顶层顺序固定为：

```text
sourceFile     = packageHeader? importHeader* declaration*
packageHeader  = package QualifiedName
importHeader   = public? import ImportSelector (as Identifier)?
ImportSelector = QualifiedName | QualifiedName . *
QualifiedName  = Identifier (. Identifier)*
```

其中 `QualifiedName` 至少含一个 identifier segment。省略 `packageHeader` 表示 root package；不存在 `package` 后接空路径来表示 root 的写法。

合法形态如下：

```kotlin
package dev.example.app

import dev.example.model.User
import dev.example.render.render as renderUser
import dev.example.model.State.*
public import dev.example.api.Result
public import dev.example.api.render as publicRender
public import dev.example.api.errors.*
```

| 形态 | parser | M23-1 HIR |
| --- | --- | --- |
| ordinary exact | 接受 | 当前编译单元中可解析时成功 |
| ordinary exact + alias | 接受 | 同上，alias 只建立当前文件短名 |
| ordinary star | 接受 | 当前编译单元中的 namespace 可解析时成功 |
| public exact | 接受 | 稳定拒绝：本阶段没有 direct dependency source |
| public exact + alias | 接受 | 稳定拒绝：本阶段没有 direct dependency source |
| public star | 接受 | 稳定拒绝：本阶段没有 direct dependency source |
| star + alias | 语法错误 | 不进入 HIR |
| `internal import` / `private import` | 语法错误 | 不进入 HIR |

### 2.2 关键字与消歧

- `public` 只在顶层 header 位置、且下一个非 trivia token 是 `import` 时，作为 import 的上下文修饰符；
- `public fun`、`public class` 等仍按既有 declaration visibility 解析；
- 即使已经进入 declaration 区，`public import` 仍应被识别为“位置错误的 import”，不能误报成损坏的 public declaration；
- `as` 只在 exact import selector 后作为 alias delimiter。star selector 后出现 `as` 必须定位到 `as` 报错；
- `internal import` 与 `private import` 不是 declaration 加损坏 token 的普通错误，parser 应识别该二 token 形态并给出“不支持此 import 修饰符”的定向诊断；
- comment、空白、换行和 token 终止继续使用现有 lexer/trivia 规则；M23-1 不引入分号或新的换行语义；
- identifier segment 是否合法完全复用现有 Identifier 规则，不为 package/import 建立第二套 identifier lexer。

### 2.3 package 语义

- 一个文件至多有一个 package header；
- package header 必须先于所有 import 和 declaration；
- 文件路径、目录名和 display locator 不推导 package；
- 同一个编译单元可以包含多个 package；多个文件也可以属于同一 package；
- `package a.b` 同时建立 namespace tree 中的 `a` 与 `a.b` 节点，但 `a.*` 只枚举直接声明在 `a` 中的 importable binding，不把子 package `b` 当成被导入实体，也不递归枚举 `a.b`；
- 仅命中 package namespace 的 exact selector 不是合法 exact import target。导入 package 内容必须写 star selector，导入具体实体必须继续写到实体 binding。

## 3. AST 设计

### 3.1 冻结形状

M23-1 冻结下面的逻辑形状。代码块描述语义字段，不要求 Rust AST 按相同方式物理存储每个 token span；实现可以保存独立 span，也可以从 lossless syntax range 精确取得。variant、非空性和可观察的精确位置是实现契约：

```text
SourceFile {
    package: PackageSyntax,
    imports: Vec<ImportSyntax>,
    declarations: Vec<Decl>,
    span: Span,
}

PackageSyntax =
    RootPackage
  | QualifiedPackage {
        package_keyword_span: Span,
        path: QualifiedNameSyntax,
        span: Span,
    }

QualifiedNameSyntax {
    segments: NonEmpty<IdentifierSyntax>,
    span: Span,
}

ImportSyntax =
    Exact {
        exposure: ImportExposureSyntax,
        selector: QualifiedNameSyntax,
        alias: Option<ImportAliasSyntax>,
        import_keyword_span: Span,
        span: Span,
    }
  | Star {
        exposure: ImportExposureSyntax,
        namespace: QualifiedNameSyntax,
        import_keyword_span: Span,
        star_span: Span,
        span: Span,
    }

ImportExposureSyntax =
    Local
  | PublicReexport { public_keyword_span: Span }

ImportAliasSyntax {
    as_keyword_span: Span,
    name: IdentifierSyntax,
    span: Span,
}
```

`RootPackage` 是真实 variant，不使用空字符串、空 segment vector 或 `Option<PackageSyntax>` 表示。需要锚定 root package 诊断时使用 source/file span 的起点，不制造一个看似来自源码 token 的 package span。

`ImportSyntax` 在外层区分 `Exact` 与 `Star`，因此成功 AST 从类型上不能表达 star alias。`ImportExposureSyntax` 区分本地导入与 public re-export 语法意图，不用多个 bool 拼出非法状态。M23-1 虽然不会成功建立 re-export，仍必须无损保存合法的 `public import` 语法，供诊断及 M23-5 直接消费同一 AST。

### 3.2 source identity 位于 AST 外

M23-1 尚未拥有 `ConeIdentity` 或 persistent `SourceIdentity`。parser 的逻辑接口为：

```text
Stage1SourceInput<Id> {
    source_handle: Id,
    text: SourceText,
    display_locator: DisplaySourceLocator,
}

ParsedSource<Id> {
    source_handle: Id,
    ast: SourceFile,
}

AllParsedSources<Id> {
    request: Stage1RequestId,
    sources: NonEmpty<ParsedSource<Id>>,
}
```

用户 source 的 `Id` 为 `Stage1SourceHandle`：它只是当次请求内唯一的 typed handle，不是 identity，不得序列化，也不得被解释为未来的 persistent source id。`AllParsedSources` 字段私有，并提供检查 non-empty、handle 两两不同且全部带同一 `Stage1RequestId` 的验证构造器。生产 source-text 路径只能把 `parse_all` 的成功结果交给 HIR；parser error 不产生该结果。由于 parser 与 AST/IR 位于不同 crate，Rust 没有 friend-crate 可见性，验证构造器也供 legacy adapter 与直接构造结构完备 AST 的 stage 单测使用；这不伪造 parser provenance：`SourceFile` 本身没有 recovery/error node，而生产编排仍必须以 `parse_all` 的 `Result` 为原子门。`display_locator` 只用于当次诊断，不进入 AST、package、entity identity、lookup 判等或 golden 的语义部分。M23-2 用 `(ConeIdentity, logical_path)` 的最终 source identity 替换外层 handle 时，`SourceFile` 本身不变。

### 3.3 AST 成功不变量

一个成功的 `SourceFile` 同时满足：

- package 恰为一个 `RootPackage` 或一个 segment 非空的 `QualifiedPackage`；
- imports 只含 grammar 合法的 exact/star variant；
- 所有 import 都位于 declarations 之前；
- exact alias 的 name 必然存在且是合法 identifier；
- star import 不可能携带 alias；
- 所有 keyword、segment、alias、star 及完整节点 span 都已确定；
- 不含 error node、missing token、unresolved path、future Cone id 或 deferred import target。

parser 为恢复而构造的临时节点不是 AST API 的成功值。

## 4. Parser

### 4.1 文件头状态机

parser 必须显式维护以下状态，而不是先把所有顶层项解析后再按节点文本重排：

```text
Start
  package -> AfterPackage
  import  -> AfterImport
  decl    -> InDeclarations

AfterPackage
  package -> duplicate package error
  import  -> AfterImport
  decl    -> InDeclarations

AfterImport
  package -> misplaced package error
  import  -> AfterImport
  decl    -> InDeclarations

InDeclarations
  package -> misplaced package error
  import  -> misplaced import error
  decl    -> InDeclarations
```

若已经见过合法 package，后续任意 package 优先报告 duplicate package；否则在 import/declaration 后首次出现的 package 报顺序错误。出错项恢复完成后仍保持已经进入的最严格状态，不能让后续 header 被错误地接纳到前面。

顶层 dispatcher 在所有状态中都识别 `import`、`public import`、`internal import`、`private import` 的 token 前缀，借此产生针对 header 的诊断。只有不属于这些前缀的 `public`/`internal`/`private` 才交给 declaration parser。

### 4.2 selector 解析

解析 import 时按以下顺序提交 variant：

1. 读取 `import` 及至少一个 identifier segment；
2. 重复读取 `.` + identifier；
3. 若某个 `.` 后为 `*`，立即提交 terminal star selector；`*` 后不再接受 segment；
4. exact selector 可选读取 `as Identifier`；
5. star selector 后若读取到 `as`，报告 star alias 错误并恢复本条 import；
6. selector 或 alias 任一部分损坏时，不构造成功 `ImportSyntax`。

至少覆盖以下畸形输入：

```text
package
package .a
package a.
import
import *
import .a
import a.
import a..b
import a.*.b
import a.* as x
import a.B as
public import
internal import a.B
private import a.B
```

### 4.3 恢复与失败契约

恢复边界分为三类：

- package header；
- 每一条 import header；
- 每一个顶层 declaration。

同步时只在顶层、delimiter 平衡的位置识别下一个 header/declaration 起点，不能因坏 import 吞掉后面的函数、类型或另一条 import。每个边界可以产生多个局部诊断，但同一根因不得因逐 segment 重试而重复报告。

parser 按 source 顺序、span 顺序提交诊断。多个 source 的最终展示顺序沿用调用方的诊断展示顺序；该顺序只服务展示，绝不能进入 namespace 判等或 overload tie-break。

只要某个文件出现任一 parser error，该文件的恢复树就整体丢弃。编译请求可以继续解析其他文件以收集独立错误，但只要任一 source 解析失败，就不调用 HIR，也不产生任何可消费 AST 集合、HIR、MIR 或 LIR 输出。warning 不属于本阶段新增的 parser 行为。

## 5. 当前编译单元输入模型

### 5.1 输入不是 Cone

M23-1 的前端聚合接口接收调用方已经给出的 source 集合：

```text
Stage1CompilationInput {
    user_sources: AllParsedSources<Stage1SourceHandle>,
    m22_core: ExistingM22CoreInput,
}
```

这里的“当前编译单元”精确定义为 `user_sources` 中的全部用户 source。它没有 coordinate、manifest、dependency edge、source discovery 或 artifact identity，因此不能被命名为 Cone，也不能被写入缓存或 `.slib`。

`Stage1CompilationInput` 接收已经由验证构造器封闭的同一 request `AllParsedSources`，因此 HIR 从类型上不会收到空集合、重复 source handle、不同请求拼接的 source 或 parser recovery node。生产调用方还必须先通过 `parse_all` 的成功门；legacy/test adapter 可以直接提供结构完备 AST，但同样不能构造 error node。现有单文件 fixture 用一个元素构造 `user_sources`；多 package 测试可直接传入多个 source。这个接口只接收已经选定的 source，不负责从目录、相邻文件或 manifest 发现文件，因而不会与 M23-3/M23-11 的正式 build root 规则形成第二套 source discovery。

### 5.2 M22 core 兼容边界

M23-1 必须保持 M22 已有的 core prelude及其 MIR/codegen backing 可用，但不提前实现 M23-3 的独立 core provider。`ExistingM22CoreInput` 在本文中只是对现有、已经闭合的 core 编译输入/下游 backing 的逻辑称呼，不冻结一个新的 crate API，也不增加可序列化类型。

- 新 package/import namespace collector 只枚举 `user_sources`，绝不把现有 core source 当成 `CurrentUnit`；
- core 名称只通过 M22 已有 prelude 候选入口出现，并继续携带现有 pipeline 所需的完整 typed target/body/backing；
- M23-1 不为 core 建立 exact/star import surface，也不增加 `BootstrapCore` import-source variant；
- 因此显式 core exact/star import与其他外部 selector 一样稳定拒绝，旧的无 import prelude 行为保持不变；
- M23-3 原子删除这条 M22 core 输入/backing 路径并换成 trusted core `.slib` 的 typed Compile/Link view，不能只替换名字索引而继续偷用旧 body；
- M23-5 再把 validated core direct-dependency public surface接入普通 exact/star/`public import`，与其他 direct dependency 走同一 source/provenance 类型。

这个边界避免两种错误过渡：既不把 core `internal` 当成用户当前单元可见，也不实现一套两阶段后即废弃的 core 专用 import API。

## 6. HIR namespace 与 import 决议

### 6.1 总体流程

```text
AllParsedSources ------------------> current-unit declaration collection
                                                |
ExistingM22CoreInput --------------> existing prelude + complete backing
                                                |
                                    resolve each file's imports
                                                |
                                    freeze per-file candidate scopes
                                                |
                                resolve typealiases + typed signatures
                                                |
                                  validate normalized duplicate signatures
                                                |
                                    existing HIR body/type lowering
                                                |
                                  complete HIR or diagnostics, never both
```

HIR 按五个逻辑 pass 工作：

1. **collect names**：为全部用户 source 的 package、owner、名称与provisional overload group分配session-local、kind-specific typed id，并建立完整current-unit namespace；此时只检查不依赖类型解析的非可重载名称/非法shape冲突；
2. **imports**：只在完整 current-unit namespace 上解析每个文件的 import，冻结 exact/star scope；
3. **signatures**：使用每个文件冻结后的scope解析typealias、base/bound、receiver、parameter/result及其他declaration header type，完成typealias展开/循环诊断；
4. **signature validation**：在全部相关signature均typed-resolved后，按normalized callable signature检查重复，并原子冻结可供body lookup的declaration surface；
5. **bodies**：使用冻结后的 candidate layer 与declaration surface完成body lowering。

collect names 必须先覆盖所有 source；后续pass不依赖 `user_sources` 容器顺序或内部 namespace map 的枚举顺序。源码中 declaration 的书写顺序仍按既有初始化、诊断及其他语言规则保留；这里禁止的只是用容器迭代顺序决定 lookup winner。

### 6.2 package namespace

package path 由 identifier segment 序列在当次 session 的 namespace interner 中取得 typed `PackageId`。文本 segment 是语言声明的 namespace key，不是 entity identity；同名 declaration 仍使用各自 kind-specific typed id。

namespace index 穷尽区分：

- package namespace；
- nominal/static owner namespace；
- class、interface、struct、enum、non-generic typealias 与 object binding；nominal binding携带其既有constructor surface，不把constructor另造为可独立import的名称；自定义 `annotation class` 尚无源码声明，因而不在 M23-1 target 集合中；
- enum variant binding；
- 当前 package/static namespace 中的 function binding；ordinary、operator 与 extension 角色继续由对应 function view携带；
- 当前 package/static namespace 中的 property binding；ordinary/extension及read-only/read-write能力继续由对应 property view携带。

package 本身不是 exact-import target；constructor、instance member、property accessor/backing field、local declaration、type parameter、label、storage、initialization unit及compiler/runtime内部实体都不可独立import。nested nominal/object、companion/static成员和enum variant不是额外target kind：它们通过typed owner namespace到达后仍落入上述class/interface/struct/enum/object/function/property/variant分支。

不得把全部实体压进一个可任意 cast 的 `EntityId`，也不得通过拼接 `package + owner + name` 后扫描全局表来恢复 kind 或 owner relation。

空文件声明的 package 仍是有效 namespace。父 package 作为路径前缀存在，但子 package 不自动成为父 package 的 star-import member。

import selector 从 root namespace 开始，先选择可见的最长 package namespace 前缀，再沿 nominal/object/companion 的 typed static owner edge 前进；exact selector 的最后一步必须得到 importable binding，star selector 的最后一步必须得到 importable namespace。最长 package 前缀一旦选定便不回退到较短前缀重猜含义。单段 exact selector可直接选择 root package 中的 binding。整个过程不构造或查询字符串 FQN。

duplicate检查分两步。collect names可立即拒绝同scope非可重载名称及其他无需type的非法shape；callable的`normalized signature`可能引用当前文件import、alias或typealias，因此只能在imports与全部declaration signature完成typed resolution/alias展开后检查。后一步按既有duplicate-declaration规则使用`(PackageId, typed owner, namespace role, name, normalized callable signature)`，并在body lowering前原子完成。top-level private scope额外携带声明source handle，不能因两个文件共用package而丢失file-private隔离。不同package中的同短名实体合法，必须获得不同的session-local definition id和codegen symbol。M23-1只要求现有本地symbol allocator消费typed package/owner/source信息避免碰撞，不冻结任何persistent mangling bytes；M23-2再整体切换到`PersistentV1`。

### 6.3 当前编译单元 visibility

对当前编译单元 target：

- top-level `private` 只对声明所在 `Stage1SourceHandle` 可见；
- `internal` 与 `public` 可在当前编译单元的其他 source 中访问；
- member/protected 等情形继续使用 M21 已有的 typed access domain/witness；
- package 相同不扩大 private，可访问性也不由 host directory 决定；
- ordinary exact import 明确命中不可见实体时，在 selector 上报告 visibility error，并以声明位置作 note；
- star import 只枚举当前 file 有权访问的 binding。namespace 合法但筛选后为空仍是合法空 scope，不因其中存在 private declaration而逐项报错。

### 6.4 ordinary exact import

exact import 必须解析到一个 importable typed binding 或合法 overload group，而不是 package namespace本身。成功结果保存：

```text
ResolvedExactImport {
    local_name: Identifier,
    targets: NonEmpty<Stage1ImportedTargetBinding>,
    syntax_origin: ImportSyntaxOrigin,
}

ImportSyntaxOrigin {
    source: Stage1SourceHandle,
    import_span: Span,
}

Stage1ImportedTargetBinding {
    target: Stage1ImportedTarget,
    sources: NonEmpty<CurrentUnitImportWitness>,
}

CurrentUnitImportWitness {
    source_binding: CurrentUnitBindingId,
    access: CurrentUnitAccessWitness,
}

Stage1ImportedTarget =
    Class(CurrentClassId)
  | Interface(CurrentInterfaceId)
  | Struct(CurrentStructId)
  | Enum(CurrentEnumId)
  | TypeAlias(CurrentTypeAliasId)
  | Object(CurrentObjectId)
  | EnumVariant(CurrentEnumVariantId)
  | Function(CurrentFunctionId)
  | Property(CurrentPropertyId)
```

这些id均指向M22已有的declaration-side kind-specific实体；generic declaration application、concrete specialization、accessor、constructor application等id家族不能混入import target。function target 的ordinary/operator/extension属性及property target的ordinary/extension、getter/setter能力由其现有typed view非可选地携带，不能从名字猜测。source 位于每个 target binding 上，而不是假定整组 selector 只有一个source；同一 target重复到达时合并其typed witness，不能按声明或加载顺序覆盖。

- 无 alias 时 `local_name` 为 selector 的目标短名；
- 有 alias 时只替换当前文件 exact layer 中的 `local_name`；
- alias 不修改 target name、package、typed origin、签名、layout 或 codegen identity；
- function/extension 等可重载 selector保留整个 typed group，不能先按声明顺序挑一个；
- selector 仅命中 namespace、找不到 target 或目标类别不可导入时，在 selector 上报错；
- 每条 import 以事务方式解析，失败时不向文件 scope 提交任何 binding。

同一 typed target 由多条 exact import 到达同一 local name 时去重；不同 target 保持在同一 exact 层，由普通 overload/歧义规则处理。import 声明的先后顺序不是优先级。

### 6.5 ordinary star import

star selector 必须解析到一个具有 importable surface 的 typed package/static namespace。成功结果保存该 namespace 的 typed snapshot，而不是在每次名称查询时重新按字符串扫描：

```text
ResolvedStarImport {
    namespace: Stage1ResolvedNamespace,
    bindings: CanonicalBindingSnapshot,
    syntax_origin: ImportSyntaxOrigin,
}

Stage1ResolvedNamespace =
    Package(CurrentPackageId)
  | ClassStatic(CurrentClassId)
  | InterfaceStatic(CurrentInterfaceId)
  | StructStatic(CurrentStructId)
  | EnumStatic(CurrentEnumId)
  | ObjectStatic(CurrentObjectId)
  | CompanionStatic(CurrentCompanionId)

CanonicalBindingSnapshot {
    entries: Vec<Stage1StarBinding>,
}

Stage1StarBinding {
    local_name: Identifier,
    targets: NonEmpty<Stage1ImportedTargetBinding>,
}
```

- resolved namespace 来自 current unit 的 typed package/static owner；snapshot 中每个 target binding 分别保存自己的非空 source witness 集合；
- snapshot 只含该 namespace 的直接、对当前 file 可访问的 binding；
- 不递归展开子 package；
- 多条 star import 属于同一个候选层，没有“后写覆盖先写”；
- 同一 typed origin 经多个 star header 到达 star 层时去重；若它也由 exact import 到达，exact 与 star 两个候选层都保留；
- 不同 function/extension origin 可组成 overload set；
- 同层多个不同的非可重载 origin 在实际 lookup 处报告歧义；
- canonical snapshot 排序仅是当次 session 的确定性输出规则，不冻结 persistent/wire key，也不得成为 winner tie-break；
- 合法但为空的 namespace产生空 snapshot；不存在或不具备 importable surface 的 namespace 报 selector error。

### 6.6 `public import` 的阶段门禁

最终语言中，`public import` 要求每个 target 都由 direct dependency 的公开 surface 授权，并建立 re-export。M23-1 的 import 输入域只有 current unit，没有可构造的 `DirectDependency` witness，所以不存在任何成功分支。

处理顺序固定为：

1. parser 正常生成 `PublicReexport` AST；
2. HIR 在 current-unit namespace 中做结构化 selector 解析，以区分 unresolved 与阶段能力错误；
3. selector 若不可解析，报告 ordinary selector error；
4. selector 若结构化解析到 current-unit target/namespace，无论其声明visibility或star展开结果是否为空，都在 `public` span 报告“public import requires a direct dependency target”；
5. 整条 import 事务回滚，不建立本地 exact/star binding，也不建立 re-export surface；
6. public star 每条 header 至多产生一个该能力错误，不能按展开成员逐项轰炸诊断。

本阶段不得把 `public import` 降级为普通 import，不得复制当前单元声明，不得构造空 re-export record，也不得留下等待 M23-5 回填的 witness。

### 6.7 外部 selector 的阶段门禁

不在 current-unit namespace 中的 ordinary selector，统一以“import target is not available in the current compilation unit”类别诊断结束；这也包括对 core package 的显式 exact/star import。M23-1 不根据文本猜测它是拼写错误、未来 dependency 还是某个 host path 中的声明。

错误必须锚定完整 selector，并可列出已经成功解析的最长 namespace 前缀。它不能创建 external placeholder id、隐式搜索相邻 source、读取 `.slib` 或延迟到 MIR/link。M23-5 引入 validated direct dependency surface 后，同一解析入口增加成功来源；M23-1 的拒绝路径无需被后续 stage 补洞。

## 7. 名称候选层

### 7.1 无显式 receiver

候选层从高到低固定为：

1. lexical binding 与 local function；
2. implicit `this` 的真实 member；
3. 当前文件 exact import，包括 alias；
4. current package 中对当前 file 可见的声明；
5. 当前文件全部 star import；
6. M22 既有 core prelude；
7. 由唯一 expected exact enum application 产生的 contextual variant fallback。

current package 层只含当前用户编译单元的声明；M22 core 不会因内部源码具有某个同名 package 而变成 current-package binding。失败的 `public import` 不进入第 3 或第 5 层，显式 core import 在本阶段也不会进入 exact/star 层。core 的既有短名只从第 6 层 prelude 到达。

### 7.2 显式 receiver 与 extension

显式 receiver 先查真实 member。extension scope 再按下列顺序分层：

```text
exact import -> current package -> star import -> M22 core prelude
```

真实 member 始终高于 extension。property-like `invoke`、property getter/setter、operator、constructor、variant、callable reference 与 typealias qualifier继续走各自既有 typed 入口；import 只增加候选 source，不创建专用调用决议捷径。

### 7.3 层内规则

- callable 在每层继续执行 M16 的 shape filter、candidate-local applicability 与 MSC；
- 只选择第一个至少含一个适用候选的层；更高层同名候选全部不适用时必须继续下一层；
- function-like/property-like c-level partition 继续遵循 M18；
- type、object、property name 等非可重载 lookup 在同层出现多个不同 typed origin 时歧义；
- 同一 typed origin 在同一层重复到达时去重；
- declaration、source、import 或 namespace 的枚举顺序不能作为 tie-break；
- visibility/access witness 在 applicability 前完成，非法候选不能靠 overload 选择变得可访问。

### 7.4 qualified type path

对 `a.b.C.Nested` 一类 type path：

1. 在当前可见 package binding 中选择最长匹配前缀；
2. 从该 package 的 typed namespace 开始，沿 nominal/object/companion 的 static owner edge 逐段解析；
3. 最长 package 前缀一旦选定，不因后续 owner 解析失败而退回较短 package 前缀另猜含义；
4. 最终必须得到可用于该 type 位置的 typed target。

不得把整个路径拼成 FQN 后扫描所有声明，也不得新增用 package-qualified 语法直接调用顶层 value/function 的入口。顶层 value/function 仍通过短名 import 或普通 receiver 语法访问。

## 8. Pipeline 完备性

### 8.1 阶段结果

```text
parse-all:
    NonEmpty<Stage1SourceInput<Stage1SourceHandle>>
    -> AllParsedSources<Stage1SourceHandle> | ParserOrInputDiagnostics

hir-lower:
    Stage1CompilationInput {
        user_sources: AllParsedSources<Stage1SourceHandle>,
        m22_core: ExistingM22CoreInput,
    }
    -> CompleteHirOutput | HirDiagnostics

mir/lir/codegen:
    只接收 CompleteHirOutput，沿用 M22 契约
```

M23-1 不改变 MIR、LIR 或 codegen 的职责。成功 import 在 HIR winner 中已经变成唯一 typed target/完整 overload group；源码 path、alias 语法、unresolved selector、recovery node 或 stage capability marker 都不得进入 MIR。

`CompleteHirOutput` 继续包含 M22 执行路径原本需要的全部 core concrete backing；M23-1 只阻止新 import namespace把它当作current-unit declaration，不删除、复制或由driver补造这些body。由此prelude target一旦被选中，MIR/LIR/codegen仍读取同一条已有typed definition链。M23-3必须把名称surface与这些backing一起切换到core artifact，不能形成“名字来自`.slib`、body仍来自旧源码”的混合状态。

### 8.2 错误原子性

- 任一 parser error：整个编译请求没有 AST 集合输出，不进入 HIR；
- 任一 HIR error：可以继续做受控诊断收集，但最终没有 `ExportHir`/`LocalConcreteHir`，也不进入 MIR；
- 一条失败 import：不向任何 exact/star/current-package scope 提交部分 binding；
- body 中因失败 import 导致的重复未解析错误可以由独立的 diagnostic suppression state 抑制，但该 state 不是 AST/HIR 节点，也不进入成功输出；
- warning 不阻止成功输出，仍遵循现有独立 journal 规则。

本阶段可暂时沿用 M22 的程序执行路径完成回归，但不得将该路径包装成新的 artifact 协议或 CLI。`.slib` 与 link 输入的任何格式、数量和 producer 假设都不属于 M23-1。

### 8.3 过渡期 entry

M23-1 的前端输出本身不引入 `ConeOutputKind`。只有需要继续进入 M22 既有 executable 测试/driver 路径的请求，才通过一个明确标记为 legacy 的 adapter选择entry：

- 只在全部 `user_sources` 中查找，不把 M22 core declaration计入；
- package 不参与entry身份，qualified package中的合法 `main` 与root package中的合法 `main` 等价；
- 候选必须是top-level ordinary、non-generic、non-suspend、无参数且返回 `Unit` 的函数；
- executable 请求必须恰有一个候选；不同package各有一个合法 `main` 仍是multiple-entry错误；
- 只做parser/HIR golden或库级前端测试的请求不要求entry。

该 adapter 不构造 library/executable sum、manifest 或synthetic Cone，M23-3由正式`ConeOutputKind`替换，M23-11删除旧driver入口。

## 9. 诊断契约

具体 diagnostic code 沿用项目统一命名方案；本阶段冻结下列原因分类、主 span 与必要 note：

| 原因 | 主 span | 必要信息 |
| --- | --- | --- |
| package 缺路径/坏路径 | 损坏或缺失位置 | 期望非空 qualified name |
| duplicate package | 第二个 `package` | 首个 package 位置 note |
| package 位于 import/declaration 后 | 后出现的 `package` | package 必须位于文件头最前 |
| import 位于 declaration 后 | `import`；有 `public` 时覆盖完整前缀 | import 只能位于文件头 |
| star alias | `as` | star import 不能 alias |
| alias 缺 identifier | `as` 后的缺失位置 | 期望 identifier |
| `internal/private import` | modifier | 只允许 ordinary 或 `public import` |
| exact selector 只命中 package | selector | exact import 需要 importable binding |
| namespace/target 不可用 | selector | 当前编译单元无可用 target，可附最长前缀 |
| exact target 不可见 | selector 最后一段 | 声明位置与 access reason note |
| `public import` 非 direct source | `public` | 本阶段 current-unit target/namespace 不可 re-export |
| 同层非可重载歧义 | 使用处名称 | 按稳定顺序列出所有 typed origin 的声明位置 |
| callable 歧义 | 使用处 call/name | 复用现有 candidate trace 与 MSC 诊断 |

parser 诊断按源码 span 排序；HIR 同一使用点列举候选时按稳定的 source/display order 和 declaration span 展示，但该排序只影响消息展示，不影响语义选择。host 绝对路径不能进入 semantic golden。

## 10. 测试设计

### 10.1 Parser golden

至少覆盖：

- 省略 package 的 `RootPackage`；
- 单段与多段 qualified package；
- ordinary exact、exact alias、star；
- public exact、public exact alias、public star；
- 多条 import 后接各种 declaration；
- `public import` 与 `public` declaration 相邻，验证上下文关键字消歧；
- 每个 keyword、path segment、dot、star、alias 与完整节点 span；
- comment/空白/换行组合不改变 AST 语义。

### 10.2 Parser negative 与恢复

至少覆盖：

- duplicate/misplaced package；
- declaration 后的 ordinary/public import；
- 空、前导点、尾随点、连续点和 star 后继续 segment 的 path；
- `import *`；
- star alias、缺失 alias identifier；
- `internal import`、`private import`；
- 同一文件包含坏 package、多条坏 import 与坏 declaration，断言独立诊断均被发现且顺序稳定；
- 任一 parser error 后断言没有成功 AST/HIR/MIR/LIR dump；
- `AllParsedSources` 构造器拒绝重复 source handle，多个source中任一解析失败时整个集合不进入HIR；
- 显式输入一个source时，即使display locator相邻目录还有其他`.scoop`文件也绝不自动吸收。

### 10.3 当前编译单元语义 fixture

至少覆盖：

- 同 package 多文件无需 import 的短名访问；
- 不同 package 间 exact、alias、star import；
- package 与物理/display path 完全无关；
- root package 与多个 qualified package 组合；
- function/extension overload group 不被 exact import 压成单实体；
- type、object、property、function、extension、non-generic typealias 的导入；
- current package、exact、star 同名时的层级；
- exact 层同名 callable 全部不适用时落到较低层；
- 多个 star 到达同一 typed origin 时去重；
- 不同非可重载 origin 在同层的歧义；
- 真实 member 高于 extension，extension 遵循 exact → package → star → prelude；
- qualified type path 的最长 package 前缀与 static nested owner；
- exact/star/alias分别覆盖imported constructor-through-type、enum variant、operator、callable reference、property read/write、property-like `invoke`与typealias qualifier，证明它们仍进入既有typed resolver；
- 跨文件同package的重复non-overloadable declaration、重复展开签名callable稳定报定义错误；至少一个negative让一条signature使用import alias、另一条使用其展开目标type，断言在typed signature validation而非语法文本比较时报重复；不同package同短名可同时生成并运行，file-private同名保留各自source owner；
- qualified package中的唯一合法`main`可由legacy executable adapter运行，不同package各有一个合法`main`报告multiple entry；
- 在保持同一组source handle和每个source内源码书写顺序不变时，置换 `user_sources` 容器顺序和namespace map插入/枚举顺序；对raw request-local id做alpha-normalization后，名称winner与歧义集合不变。

### 10.4 visibility、core 与阶段门禁

至少覆盖：

- 跨文件访问/import `internal`、`public` 成功；
- 跨文件 exact import top-level `private` 给出精确 visibility error；
- star import 自动排除其他 source 的 private binding；
- M22 core prelude及其完整 MIR/LIR/codegen backing 保持全部旧 fixture 行为；
- core 中不属于 prelude 的名字不能无 import 以短名访问，core internal/private 也不会泄漏进 current package；
- ordinary exact/star import core package与其他未提供的外部路径都稳定失败；
- public exact/public alias/public star 分别指向 current unit 与未知/core外部路径；
- public star 每条 header 只有一个阶段能力主诊断；
- 所有失败路径均无本地导入降级、无 re-export、无下游 IR 输出。

`public import` 的诊断优先级由下表冻结；exact alias 与 exact 使用同一行：

| selector 形态 | 结构化目标 | M23-1 主诊断 | 提交结果 |
| --- | --- | --- | --- |
| exact | current-unit `public`/`internal` | `public`处direct-dependency capability error | 无binding、无HIR |
| exact | current-unit同文件或其他文件`private` | `public`处同一capability error；本阶段不追加visibility error | 无binding、无HIR |
| star | current-unit非空、空或private-only namespace | 每个header恰好一个`public`处capability error | 无snapshot、无HIR |
| exact/star | unknown或显式core/external namespace | selector处unavailable-target error | 无binding、无HIR |
| 任意 | malformed selector | parser error | 不进入HIR |

每一格都必须断言没有ordinary-import降级、没有re-export record、没有HIR/MIR/LIR/executable。M23-5加入真实`DirectDependency`后才新增public import成功格，并另行检查target是否public/exportable。

### 10.5 回归与 snapshot

- M1～M22 fixture 全量通过；
- 原有无 package/import 文件统一显示为 `RootPackage`，允许 AST golden 做一次受控更新；
- 原有程序 stdout、运行期行为和非相关诊断不得变化；
- 新成功 fixture 同时锁定 AST、HIR、MIR、LIR 与最终程序结果；
- 新错误 fixture 锁定 diagnostic reason、主 span、note 及“无下游 dump”；
- M23-1 不要求 fixture 改走尚未存在的 `scoop build <file>`。

fixture 迁移按用途区分，不能把多source测试误塞进single-file mode：

| 阶段/fixture | 输入方式 |
| --- | --- |
| M23-1历史单文件回归 | legacy runner显式传一个source |
| M23-1新增parser/HIR多source测试 | 前端test harness显式传`AllParsedSources`，永不做目录发现 |
| M23-3～M23-10 end-to-end迁移期 | 正式新pipeline使用manifest/typed SingleFile request；旧executable runner只维持历史回归，不得被新生产组件调用 |
| M23-11历史单文件fixture | 迁至正式`scoop build <file>` / `scoop run <file>` |
| M23-11多source集成fixture | 迁至manifest-backed Cone；parser/HIR的direct unit/golden harness可继续保留 |

M23-11完成后删除legacy executable runner；源码无需为迁移改写package/import语义。

## 11. 实现顺序

1. 在 AST 中加入完整 header sum type、非空 path 与 span，并一次性更新 root-package golden；
2. 实现 parser 文件头状态机、selector parser、上下文关键字和恢复测试；
3. 将 HIR 输入收敛为验证后的 `AllParsedSources` 加现有 `ExistingM22CoreInput`，先完成仅针对用户source的全单元name/provisional-group collection；
4. 实现 current-unit namespace selector、事务式 exact/star scope 与 visibility；
5. 在冻结import scope后解析typealias与declaration signature，完成typed normalized-signature duplicate检查，再开放body lowering；
6. 把 exact/current-package/star/prelude 接入既有 type/name/call/extension resolver，补齐顺序、fallback、dedupe 与歧义测试；
7. 实现 `public import` 与 external selector 的阶段门禁及无下游输出断言；
8. 依次运行`cargo fmt --all`、`cargo clippy --workspace`、parser/IR golden、新fixture与M1～M22全量回归。

每一步合入时都必须维持成功 IR 结构完备；不得先加入 `Option<ResolvedImport>`、unknown target、字符串 FQN fallback 或暂时忽略错误，等待后一步回填。

## 12. 完成门

M23-1 只有同时满足以下条件才算完成：

- grammar 表中的全部合法形态均有 parser golden；
- header 顺序、path、modifier、alias 和多错误恢复均有定位精确的 negative fixture；
- `SourceFile` 的 package/import 结构从类型上不能表达空 path、缺 alias identifier 或 star alias；
- 当前编译单元内多 package exact/star/alias lookup 可用，且winner不依赖source容器、map插入或namespace枚举顺序；源码declaration顺序仍按既有语言规则保留；
- 最终候选层、callable applicability fallback、MSC、member/extension 优先级均有组合 fixture；
- visibility、M22 core prelude兼容、external import 与全部 `public import` 的阶段行为均被锁定；所有语法合法的`public import`在M23-1都必须于HIR失败，不存在成功或ordinary-import降级分支；
- 成功 HIR 中不存在 unresolved import、源码 path target、future Cone/persistent id 或待补 witness；
- 任一 parser/HIR error 都阻断全部下游 IR 与 executable 产出；
- 不新增 `.slib`、artifact、object/member 数量或 host path 假设；
- `cargo fmt --all`与`cargo clippy --workspace`通过；
- 对应unit、AST/HIR/MIR/LIR golden及positive/negative/组合fixture通过；
- M1～M22 全量回归通过。

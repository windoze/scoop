# M23-6a 设计：统一 HIR 语义模型、产物消费与具体化

状态：已完成并验收（2026-10-01），见 [实际验收](ACCEPTANCE.md) 和 [进度记录](PROGRESS.md)。位于 M23-6 与 M23-7 之间，是 M23-7 及后续阶段已落实的共同 HIR 前置条件；[M23-6 的实际验收](../stage6/ACCEPTANCE.md) 保持。

权威合同为 [语言规范](../../specs/SCOOP-SPEC.md) 3.2、8.5、12.4、12.5，[实现规范](../../specs/SCOOP-IMPL-SPEC.md) 2.2–2.4、2.13，以及 [运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md) 的类型身份、初始化与 GC 契约。实施时同步修订这些合同，不能以旧阶段限制、格式分区或局部测试已经通过为理由保留重复语义路径。

## 1. 目标与范围

同一份声明、类型和已检查正文，无论由当前源码产生还是由依赖 `.slib` 解码，都使用相同语义结构，并由同一套查询、调用决议、默认值展开和具体化算法消费。已有语言能力不因声明来源改变支持范围。

本阶段修正 `scoop-hir`、`scoop-hir-lower` 及其实际产物编解码、driver 和 MIR 输入适配。保留 HIR→MIR→LIR 的职责划分，不重新实现后端，不新增通用查询引擎、插件层、来源凭证或预算框架。数据结构与格式由 IR crate 拥有，语言分析与替换由 `hir-lower` 完成，driver 只编排输入和输出。

M23-7 已实现的泛型、默认值、构造、closure、数组与迭代代码和真实 fixture 是迁移基线。统一这些路径、修复其 HIR 缺口属于 6a；完整 ODR 定义比较、生成机器实体闭包和泛型委托存储的运行闭环由修订后的 [M23-7](../stage7/DESIGN.md) 继续完成。若 HIR 回归需要适配已有机器表示、调用或产物接口，应在 6a 一并修正，不能因为原工作被列在 Stage 7 而延后。

收尾审计确认了生成机器实体闭包的一项具体需求：消费方首次取得外来参数自由 `@NoGC` 函数的 C ABI `FunPtr`。现有普通跨 Cone 调用已经成功，但未在提供方取地址的函数没有发布 C storage bridge；现有取地址 lowering 只持有本次生成的 bridge 与 trampoline，尚无完整的跨 Cone C 地址发布／引用服务。该服务由 M23-7 随实际机器桥闭包补齐，不能由消费方复制提供方 Strong 定义或伪造本地源码函数绕过；已有本地 C 地址、普通外部调用、指针条件及函数值能力继续作为本阶段回归。

## 2. 已确认的缺口与修补落点

下表记录设计时的实际入口。迁移可以移动或删除文件，完成标准是替代旧路径，而非保留名称。

| 缺口 | 当前证据 | 修补方式 |
| --- | --- | --- |
| 同一种 nominal 被本地／导入来源拆成不同类型 | [`types.rs`](../../../compiler/hir/src/types.rs) 的 `Struct/ImportedStruct`、`Class/ImportedClass` 等 | 每种 nominal 使用一种 application 表示，按原 typed 声明 ID 查询完整定义；删除来源专用类型分支 |
| 依赖侧已有共享查询，当前声明仍从另一套 arena 取数据 | [`semantic_world.rs`](../../../compiler/hir/src/semantic_world.rs) 与 [`entities.rs`](../../../compiler/hir/src/entities.rs) | 扩展现有查询，同时覆盖当前声明与可达依赖；算法消费相同的声明、成员、字段、父类型和 conformance 记录 |
| 普通候选仍要求本地函数 ID | [`call_resolution/candidates.rs`](../../../compiler/hir-lower/src/call_resolution/candidates.rs) | 候选保留原 kind-specific typed target，全部调用形态共用完整实参映射、上下文推断、MSC 与提交过程 |
| 导入泛型调用独立组织推断流程 | [`generic/arguments.rs`](../../../compiler/hir-lower/src/expr/named_calls/imported_dependency/generic/arguments.rs) | 将 postponed arguments、partial solver、默认类型种子与失败事务纳入共同调用过程，删除导入专用调度 |
| 同一默认值有两套正文替换 | [`defaults/instantiate`](../../../compiler/hir-lower/src/defaults/instantiate/mod.rs) 与 [`defaults/materialize`](../../../compiler/hir-lower/src/expr/named_calls/imported_dependency/defaults/materialize.rs) | 本地与读出正文使用共同节点、局部值环境和替换操作，保留定义位置、求值位置及求值顺序 |
| 语义正文先转换成 transport，再重建 imported template | [`production/default_templates`](../../../compiler/hir/src/production/default_templates/body/mod.rs)、[`imported_generics.rs`](../../../compiler/hir-lower/src/imported_generics.rs) | wire 只编码／解码共同语义节点；具体化直接查询原正文，不再建立导入专用语义图 |
| 具体化请求和入口继续按来源分流 | [`concretize.rs`](../../../compiler/hir-lower/src/concretize.rs) 的 `Free/Method/Imported/ImportedMethod` | 按原定义和完整 application 统一请求、查询及固定点，来源只参与定义归属与外部实现关联 |
| 合法解构因 imported 类型进入不匹配分支 | [`patterns/binding.rs`](../../../compiler/hir-lower/src/patterns/binding.rs) | 从共同 nominal/field/member 查询直接展开普通绑定、字段及 component 调用；覆盖直接源码、默认值和泛型正文 |
| 参数自由声明被旧物化闭包排除 | [`concretize/automatic.rs`](../../../compiler/hir-lower/src/concretize/automatic.rs)、[`nominal_materialization.rs`](../../../compiler/hir/src/semantic_world/nominal_materialization.rs) | 按完整应用和实际需求闭合，消除封闭泛型父类型、签名、默认参数导致的 source-only gate |
| 协议操作的语言处理与模板适配仍可能重复 | 已有 `for`、数组、指针、Option、bound 和派生相等路径 | 在定义处选定目标并正规化；共同正文只保存已选操作和必要条件，具体化不重新做语言选择 |

已记录的直接回归包括：普通宿主继承／实现封闭泛型父类型、整数范围 iterator 候选去重、消费方直接解构外来 struct/class，以及外来函数值默认参数。它们归入本阶段共同模型的验收，不继续作为逐语法追加 imported 实现的清单。

## 3. 语义 HIR、导出投影与具体 HIR

### 3.1 三者的关系

`SemanticHir` 表示当前 Cone 已检查的完整语义图；名字用于明确职责，不要求新增 crate。`ExportHir` 是该图的导出投影，与原图使用同一声明、类型、正文结构及原 ID；它不是另一种需要重新 lowering 的语言。`LocalConcreteHir` 是完成替换、供当前 Cone MIR 使用的另一阶段 IR，具有独立的实体 ID 家族。

```text
源码分析：CurrentCone AST + 共同 HIR 查询 -> 当前 Cone SemanticHir
导出投影：SemanticHir + 公开／继承／模板支持需求 -> ExportHir
产物解码：.slib HIR section -> 同结构的 ExportHir
具体化：当前 Cone 物化需求 + 共同 HIR 查询 -> LocalConcreteHir
```

共同查询覆盖当前语义声明与全部可达依赖导出。源码签名收集、递归声明构建和候选失败状态留在 builder 内部；成功输出不存在未完成签名、可选类型或 pending 实例化。依赖普通实现可以只提供完整声明与 typed 外部定义引用；需要在消费方替换的模板正文必须存在于导出闭包，缺失是产物错误。

### 3.2 身份与类型

声明、泛型定义、符号化 application、exact application 和具体发射实体使用各自原有的 typed ID。不同实体种类不能共用裸编号，临时 arena index 不成为跨 Cone 身份；按持久 ID 查询同一原声明，不能复制为当前 Cone 同名实体或按 FQN/layout 回退。

struct、class、enum、interface 各使用一种 nominal application；application 保存原声明身份和完整的、可能含定义域 binder 的参数。具体化之后使用 exact type identity。字段、variant、constructor、getter/setter 与 dispatch slot 保留各自身份；统一来源不抹除这些语言区别。

primitive、Ptr、函数类型等实际表示族继续使用现有语义区分，并关联真实 core 声明。core 自身编译与依赖 core 的角色解析都得到同一种 typed 引用，不在后续语义算法中保留另一套 core 候选或具体化规则。

### 3.3 共同正文

普通函数、泛型函数、默认值、构造初始化、lambda、局部函数和委托初始化共用表达式、语句、模式、局部值、捕获、引用与类型替换。根记录仍分别保存各自 owner、参数协议、初始化序列和条件；不把默认值伪装成普通函数，也不复制一套 `GenericExpression` 或 `ImportedExpression`。

表达式的类型、调用目标、receiver、effects 和定义位置必须完整；默认展开同时保存实际求值位置。局部值和捕获使用现有 body-scoped selector 与词法身份，替换生成新的使用点局部值，不能把提供方裸 arena index 当成消费方变量。初始化中的局部函数、跨层捕获和再次发布保持同一词法归属。

abstract、intrinsic、source extern、外部普通定义及需要替换的正文采用穷尽的实现类别。普通外部定义没有正文不等于缺失模板，abstract/intrinsic 不能用空正文填充。导出只保留下游实际需要的正文，不发布整个 provider 的普通实现。

## 4. 共同查询与唯一语言处理入口

### 4.1 查询与可见性

统一现有的声明、类型、字段、父类型、成员、conformance、默认来源和正文查询。源码建立的完整记录与解码记录具有同一语义含义；存储可以分属不同 Cone，算法不据此分流。不能仅在两套实现上加一个同名 facade 后视为完成。

direct dependency 的公开包参与源码名称查找，support provider 供已绑定的模板、类型和成员引用解析；沿已选 nominal owner 的公开静态成员查询遵守原规则。统一查询不扩大 public/internal/private/protected 可见范围。re-export 保留原实体，菱形引用按 typed origin 去重，候选优先级只来自语言规定的 scope 和 c-level 分区。

### 4.2 调用决议

普通函数、成员、扩展、构造、variant、operator、property-like invoke 与 callable reference 共用完整决议过程：候选分层、形态预过滤、source argument mapping、独立约束环境、postponed arguments、固定点推断、MSC、winner 提交和参数物化。目标种类仍类型化，不把构造器当作普通函数。

宿主与 callable 参数分组，显式参数、`_`、期望结果、默认类型种子和失败诊断使用同一规则。实际声明签名参与候选比较，不能以当前实参推断结果替换签名。失败候选不提交实体、依赖使用或诊断副作用。

receiver 及所有显式实参按源码顺序求值；随后按形参顺序展开实际省略的默认值和构造 vararg。推断顺序不能改变运行顺序。`addressOf` 的原 place、循环条件的 setup 区域、默认值的双来源与 suspend 上下文沿原语言合同处理。

### 4.3 定义处分析与具体化的边界

定义处完成名字、重载、访问域、默认来源、bound 成员／槽选择及语言脱糖。`for`、解构、属性操作和参数展开使用已选 typed 目标；intrinsic 正规化为现有操作。`when`、closure 和 suspend 可以保留完整的 typed 节点，由 MIR 做 CFG、closure conversion 和协程变换，不强行提前到 HIR。

具体化只替换类型、receiver、局部值和捕获，沿已选 bound 契约完成 exact 派发，并检查确实依赖实参的既有 GC-free、NoGC、CLayout/FFI 等条件。不能因实际类型新增了某个方法而重新做 overload resolution，也不能把完整类型检查复制进 meta crate。

## 5. 统一具体化与物化需求

具体化器通过共同查询直接读取本地或依赖的同结构正文。请求 key 使用原定义、完整宿主／callable 实参和既有词法身份；不包含 Local/Imported 语义分支。相同请求复用同一结果，新发现的实际调用、类型和初始化需求进入同一固定点。

| 需求 | 处理 |
| --- | --- |
| 当前 Cone 普通实现、初始化及必需表示 | 用完整签名和空或既定实参环境生成本地 concrete 实体 |
| 上游普通函数、存储、初始化服务和已定义类型 | 保留原实体和 provider，关联既有外部定义，不复制实现或状态 |
| 当前实际使用的泛型 application | 从共同正文生成本次 concrete 实体，保留原 application/ODR 身份 |
| 未被执行或物化需求使用的模板、默认值、查询结果 | 保持语义数据，不因位于 arena 或可见集合而建立机器根 |

公开普通实现、必要继承/dispatch、实际类型操作和 hidden 普通 helper 形成相应物化需求。`Box<Int>`、`I<Int>`、含封闭 application 的签名和函数值默认参数均按完整类型继续闭合；不得保留“声明自身无参数但依赖闭包含泛型，所以整个 owner 不可用”的 gate。

泛型声明未被实例化、普通实现属于另一 Cone、或某项操作确实不需要实体，是合法的不发射原因；曾属于哪个 M23 子阶段不是原因。对引用类型字段不递归物化 referent 全部方法，对 helper 不递归产生全部支持族。

保留原泛型递归 SCC、按值循环和继承环规则。只有实际完整替换产生的新类型事实需要计算；已验证依赖的 exact facts 可复用。MIR 输入不含 binder、推断变量、缺失 GC facts 或待完成请求；MIR→LIR 的 layout、ABI、Strong/ODR 与定义方存储归属保持原合同。

## 6. 编解码、验证与缓存

共同 HIR 类型及 wire 定义位于 `scoop-hir`；`scoop-slib` 负责归档与格式边界，不能调用上游 stage 重编译。producer 从已检查语义图选择导出记录，reader 解码为同结构记录并检查必要的格式、引用、binder、签名连接、局部数据流及来源位置。编解码不重新执行名字查找、重载、默认来源或访问域选择。

语义导出闭包、源码可见集合和机器定义闭包分别从真实需求取得，不用一张 param-free 白名单兼任。正文引用 private/internal helper 时保留原 typed 支持引用；它不成为普通 import binding，也不复制声明方存储。

同一边界完成且数据未变化的验证结果由后续查询和发布复用。新实例、新跨记录关系及新的外部输入在对应边界检查；不引入“已验证”凭证链，不为当前编译器输出重新读取所有依赖并完整重放。

内存表示统一不必自动改变 bytes；实际 wire payload 改变时才提升对应 section major，更新 required inventory、profile fingerprint、固定向量和缓存，重建 core/provider/consumer。现有 HIR `/40` 等编号仅记录迁移前基线，不能作为保持双语义模型的理由。6a 完成时使用一套正式编解码和 publisher；旧格式保留明确拒绝测试，不维持长期新旧路径。

HIR fingerprint 来自共同语义记录及实际导出内容。实例化结果、布局和机器对象沿原 MIR/LIR/Code 投影计算；不新增按声明来源区分的 cache、whole-program 实例选优或重新发射上游普通实现的策略。

## 7. 迁移顺序与删除要求

1. **共同实体与查询。** 修订 spec，统一 typed nominal/application 和声明查询；迁移类型关系、成员、继承、conformance 与模式，删除 `ImportedStruct/Class/Enum/Interface` 等来源专用语义分支。
2. **共同候选与语言处理。** 让全部目标进入现有调用决议与绑定处理，合并上下文推断、参数协议和默认来源；修复实际回归，删除导入专用推断调度。
3. **共同正文与产物。** 统一普通、默认、泛型和初始化正文及其编解码；consumer 直接取得语义节点，删除 transport→imported template 的第二次语义构造。
4. **共同具体化与需求。** 改为查询原定义和 application 的单一固定点；移除本地 arena 假设、source-only gate 和来源分支，适配实际 MIR/产物消费者。
5. **正式切换与回归。** 升级实际改变的格式，重建依赖，删除被替代代码和仅服务旧路径的工厂；保留所有真实语言、损坏产物与运行回归。

步骤按数据依赖推进，不按语法逐项追加第二套支持。迁移中临时适配只允许机械的身份／存储转换，不增加语言算法；对应步骤完成时删除。不能让旧 imported 类型、旧默认值执行器或复制声明的入口继续作为正式 fallback。

## 8. 验证与完成门

沿用现有 fixture、stage golden、真实配套 `scoopc` 和单 image 运行机制，不建立平行测试编译器。每批代码先格式化和 lint，再执行相关验证；阶段结束使用实际配套编译器完成 workspace/runtime 回归。

| 验证 | 必须证明 |
| --- | --- |
| 同一导出图的内存／wire 等价 | 同一原身份的导出投影直接消费或 encode/decode 后消费，得到一致的声明、正文、选择和具体化结果；比较不要求包含未导出的 provider 私有实现 |
| 声明位置变化 | 在保持合法可见性的前提下，类型／函数位于当前 Cone 或依赖时，调用选择、默认值、解构和求值顺序一致；必要的 Cone 身份差异不作为误报 |
| 双向泛型组合 | provider 泛型配 consumer-local 类型、本地泛型配外来类型，以及类型和模板来自不同依赖；完整宿主／方法实参、bound、默认值和再次发布均正确 |
| 当前明确缺口 | 封闭泛型父类型、整数范围 iterator 去重、外来 struct/class 解构和函数值默认参数通过共同路径 |
| 真实语义组合 | constructor/common initialization、属性、数组/vararg、Option、异常/finally、closure/capture、callable reference、pointer/FFI 条件与已有 suspend 语义保持 |
| 可见性与定义处绑定 | support 不成为公开 import；public default 的非法引用、不可见调用、错误 bound 和定义处重载诊断正确，消费方新增同名方法不改变已选目标 |
| 物化与归属 | 查询不产生多余根，未用模板不发射，普通外部存储/初始化不复制，实际 application 与机器引用完整 |
| 格式与运行 | 缺失模板、错 owner/binder/引用被明确拒绝；真实源码发布后移走源码，消费者可再次发布，并在适用的普通／移动 GC 场景链接运行 |

完成时，上表及以下条件必须同时成立：共同查询由实际生产调用；源码与解码使用相同语义节点；默认值和具体化只有共同替换路径；最终 HIR 不再包含来源专用语义类型／操作；MIR 输入结构完备；已有机器能力支撑的真实跨 Cone 回归全部通过。只增加 facade、保留旧分支或仅通过手工 metadata 测试均不算完成。

尚属于 M23-7 的新增物理 helper、完整 ODR 合并和泛型委托运行能力按其真实缺口继续交付；不能反向作为 6a 共同 HIR 的前置依赖，也不能以其未完成掩盖本阶段的语义／输入适配缺口。

## 9. 后续阶段合同

- **M23-7** 消费已统一的 HIR、模板与具体化结果，完善 ODR 定义和引用、机器 helper、泛型委托存储及运行；不重新安排逐语法 imported 支持。
- **M23-8** 消费真实 MIR/LIR 和六类 registration 完成多 image 登记与启动，不重新解释 HIR、推断泛型或补造类型定义。
- **M23-9** 的 program-link 只消费完整 Link 数据、runtime 与目标输入；缺失定义由生产/输入错误报告，不回调 HIR lower 修补。
- **M23-10** 从已有 typed native requirement 解析真实 native 输入；本地、模板和依赖调用共享前端已选签名、effects 与边界契约。
- **M23-11** 迁移 CLI 与历史 fixture 时只编排同一编译管线，复用上述等价与真实运行矩阵，删除 core/source 拼接和旧编译入口。

各阶段继承 6a 的共同语义模型。后续新增语言操作应扩展共同 HIR 及其必要编解码和 lowering，不再为 `.slib`、core 或泛型正文另建语义实现。

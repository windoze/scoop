# M23-6a 实现进度

目标：完成 [设计](DESIGN.md) 第 8 节的全部完成门；每个功能先格式化、lint 和验证，再提交。当前为实现中，已有 Stage 7 功能与 fixture 仅作为迁移基线，不代表共同 HIR 已完成。

## 实施顺序

1. 统一 nominal application 与声明查询，迁移类型关系、成员、继承、conformance 和模式。
2. 统一候选、参数映射和上下文推断，删除导入专用的语言处理调度。
3. 统一普通、默认、泛型和初始化正文，接入正式编解码，删除 imported template 的二次语义构建。
4. 统一具体化请求和实际物化需求，移除 source-only gate 并适配 MIR 输入。
5. 完成内存／wire 等价、声明位置变化、双向泛型、真实产物再次发布和普通／移动 GC 回归，删除被替代路径。

## 接续基线

- 工作区已有 `for` 在 Export HIR 前展开的实现和 `hir/cross-cone-interface/40` 迁移；沿用实际声明、调用、Option 和 while 节点，删除专用 For／binding-plan 运输及重复遍历。
- 三份 spec、M23 总设计和 Stage 7 职责已先行修订，明确声明来源不改变语言语义，普通封闭泛型 application 不能被历史阶段门排除。
- 本次接续已完成 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和 `cargo build --workspace`，均通过。启用本次实际配套 `scoopc` 的 workspace 回归为 5208 passed、1 failed、0 ignored；唯一失败是 `/40` 迁移遗漏的两份 Link Code 指纹快照。
- 更新这两份快照后，关闭更新开关，以同一配套编译器和原测试二进制复验 `property_initialization_uses_close_source_mir_lir_and_both_artifact_views`，1 passed、0 failed。基线共 5209 个测试均已覆盖通过；原完整回归日志和专项复验日志分别为 `/tmp/scoop-m23-6a-baseline-test.log`、`/tmp/scoop-m23-6a-baseline-code-verify.log`。
- 已清理约 13 GiB 的 `target/debug/incremental` 缓存；阶段验证使用禁用增量编译和调试符号的构建配置。

## 尚未完成

设计中的共同实体、正文与具体化迁移仍须实现。不能把基线回归或某个共同查询通过，作为删除来源专用语义模型的替代验收。

## 共同字段查询与绑定展开

- struct 字段访问、不可失败解构、`when` 模式与穷尽性分析使用同一完整 application 字段查询；模式直接保留完整 owner 类型，适用于当前及依赖声明。迁移期查询内部仍适配两种存储，后续须随 nominal 模型统一而删除。
- val／var、lambda 与 for 绑定直接生成普通声明、字段访问和已选 component 调用，删除只为重放前一步而建立的 `IrrefutableBindingPlan`、shape 与 action。失败按原事务回滚；被解构表达式求值一次，class 的 `_` 仍按顺序调用 component。
- 解构主文件由 829 行降至 194 行，聚合处理 106 行、struct 模式名称解析 69 行。没有新增 wire 节点或 section 版本。
- `m23-shared-bindings` 包含 5 组正例、11 组反例与 HIR／MIR／LIR、诊断快照；覆盖字段／component／扩展、nested／rest／alias、lambda／for 捕获、默认值／when、宽值／引用／Unit。使用真实产物、移走源码、再次发布和普通／移动 GC 运行。
- 验证：全仓 fmt／clippy 通过；HIR、HIR lowering、MIR lowering 与 slib 的 2840 项回归通过；新增两个完整 fixture 测试在关闭更新开关后通过，日志 `/tmp/scoop-m23-6a-bindings-verify.log`。
- 组合探测另确认依赖 Boolean／String 字面量模式的 ordinary `equals` 仍被旧 imported 调用入口限制（例如 `Pair<Boolean>` 中的 `Pair(true, _)`）。该缺口归共同候选／调用及正文迁移，未用阶段限制作为最终合法性规则；本批穷尽性回归使用嵌套 Option 及 guarded struct 模式。

## 共同父类型与接口查询

- 子类型、约束推断、nominal invariance 诊断和 Iterator application 复用完整直接父类型关系，接口闭包共用去重与继承遍历。`relations.rs` 从 470 行降为约 210 行，声明存储适配单列 127 行；后续统一 nominal 存储时继续删除适配分支。
- 新增 `m23-iteration/inference.scoop`，覆盖本地菱形接口 bound 到父接口的调用推断，并与依赖 Cursor／Iterator 和再次发布组合。
- 该组合发现抽象成员在本地保存 `Owner.member`、导入后只保存 `member`，导致同一 trap stub 的字符串常量与 ODR definition 不一致；导入记录现保留原 owner 与成员名称。
- 全仓 fmt／clippy、1288 项 HIR lowering 回归及 49 项泛型产物测试通过；快照变更仅涉及原抽象成员名称、调用和 trap 常量，关闭更新开关后复验覆盖全部变更快照的 6 项完整 fixture 测试通过。配套编译器与测试二进制保存在独立临时目录，本轮后续源码修改不影响结果；日志 `/tmp/scoop-m23-6a-parents-generic-fixtures.log`、`/tmp/scoop-m23-6a-parents-verify.log`。
- 旧基线的 `target/debug` 已清理，后续统一使用 `target/m23-6a`，另外释放约 15 GiB。

## 共同上界约束传播

- 唯一解选择前，将既有等式、下界和声明上界沿同一关系约简器传播；`R : Reader<T>` 与实参的完整父类型可以确定 `T`，不再要求返回结果的额外类型注解，也不把声明上界当作缺省解。
- invariant 诊断与约束求解共用完整 nominal application 查询；删除按本地／依赖来源分别遍历父类型的分支，补齐 rigid 参数到本地 generic class bound 的关系。存储适配暂存于 67 行的 `types/nominal.rs`，继续归后续实体统一迁移。
- 求解主文件降至 441 行，完成解验证为独立 212 行模块，约束传播为 156 行模块。新增 `m23-shared-bounds` 的 3 组正例、4 组反例，覆盖菱形、本地与依赖 class/interface、参数拓宽、lambda 上下文及无解；真实再次发布与普通／移动 GC 已通过。原 iteration 推断 fixture 也已移除结果类型注解。
- fmt／clippy 与 1288 项 HIR lowering 回归通过。关闭快照更新开关的完整泛型产物回归为 50 passed、1 failed；唯一失败是上界传播后错误原因从整个 application 不满足 bound 细化为 `U` 的 `Int`／`String` 冲突，报错位置不变。核对并更新该诊断后，原配套二进制的专项复验通过，51 项测试均已覆盖。日志 `/tmp/scoop-m23-6a-shared-bounds-regression.log`、`/tmp/scoop-m23-6a-shared-bounds-bound-diagnostic.log`。

## 语义查询使用原声明身份

- nominal 身份在完整声明树收集后建立，供 import、约束与正文查询使用，Export HIR 直接接收同一记录。应用查询已删除 `NominalTemplate::Imported` 等来源／本地 arena 身份分支，统一使用原 `SourceNominalId`；本次构建的反向表只将当前声明身份定位到原 arena 记录。
- 源码位置、嵌套 owner、声明种类与元数沿既有身份规则，wire 格式和身份算法未改变；本地 arena 索引不作为共同查询的声明 key。约束单元测试补齐实际声明来源与 namespace，沿生产身份建立方法构造测试输入。
- 全仓 fmt／clippy、HIR 与 HIR lowering 的 2143 项单元测试，以及 8 项真实产物组合通过，所有快照更新开关关闭。组合涵盖泛型名义类型、上界、同包／限定名称、嵌套重导出和再次发布；日志 `/tmp/scoop-m23-6a-nominal-identity-unit.log`、`/tmp/scoop-m23-6a-nominal-identity-fixtures.log`。
- 清理编辑器后续生成且当时未被构建使用的 `target/debug/incremental`，再释放约 2.2 GiB；当前阶段仍使用 `target/m23-6a`。底层来源专用 Type／声明存储及正文路径仍待后续迁移，不能以查询 key 统一代替完整完成门。

## 共同上界记录

- class／interface 上界统一保存完整类型和声明位置，删除 Local／Imported 上界变体及仅保存本地 application 的中间结构。kind 约束与 nominal 约束仍由互斥结构表达，class 上界仍最多一个。
- 约束推断、bound 诊断、访问域、父类型、dump 与导出签名直接读取共同类型；声明顺序继续按原位置保留。成员收集和完整 application 校验中的旧声明存储适配仍随 nominal 迁移继续删除，没有增加 wire 格式或来源资格规则。
- 全仓 fmt／clippy、2143 项 HIR／HIR lowering 单元测试，以及 9 项真实泛型产物回归通过；覆盖 class／interface bound、函数引用、基本值派发、迭代、本地与依赖声明组合和再次发布，快照更新开关关闭。日志 `/tmp/scoop-m23-6a-bound-records-unit.log`、`/tmp/scoop-m23-6a-bound-records-fixtures.log`。

## 共同字段引用与初始化投影

- struct／class 普通字段引用及初始化字段投影统一保存原字段 ID 和完整宿主类型，删除 Imported 字段变体及初始化字段的 Declared／Imported 分支。默认值只代换宿主类型，具体化按原字段 ID 映射到同一 concrete 布局；派生 struct 相等复用共同字段查询。
- 字段身份在实际字段可用后按需建立，委托初始化仍在确定实际存储类型后建立字段。已建立的 property／field 身份由导出复用；最终表补齐未被正文引用的声明并保留完整索引。原 359 行字段身份模块拆为记录、构建和查询，分别约 195、171、96 行。
- fmt／clippy、2840 项 HIR／前端／MIR lowering／slib 单元测试、51 项真实泛型产物回归均通过。核对 27 份快照，仅字段显示和派生相等的声明字段路径发生变化；错误位置与 MIR／LIR／wire 快照不变。关闭更新开关，使用保存的配套二进制复验覆盖全部变更快照的 11 项完整组合，全部通过。日志 `/tmp/scoop-m23-6a-field-refs-unit.log`、`/tmp/scoop-m23-6a-field-refs-fixtures.log`、`/tmp/scoop-m23-6a-field-refs-verify.log`。
- 本批完成字段引用统一；底层 nominal 声明、调用与正文的其他来源分支仍按阶段设计继续迁移。

## 共同 enum 引用与模式查询

- variant 构造、判别、payload 投影、默认构造目标与模式统一保存原 variant／payload 字段 ID 及完整 enum 类型，删除 Imported variant 正文和模式分支。身份在变体收集完成后建立，导出复用；具体化按原 ID 取得 concrete 表示，默认替换只改变宿主类型。
- 字段形状、派生相等、模式覆盖共用 variant 查询；Export HIR 的 enum 穷尽性记录只保存完整 subject，删除冗余本地 enum application，源码与依赖均保留同一种记录。wire 字段及 section 版本未改变。变体形状处理、声明查询与具体化分别为 97、111、73 行。
- 新增 `m23-shared-enums` 的 3 组正例、4 组反例与三阶段／诊断快照；覆盖位置／命名／默认构造变体、嵌套 enum／Option、guard、类型别名、派生相等、宽值／引用／Unit、泛型再次发布及普通／移动 GC。
- 完整回归暴露候选事务深复制不可变 core 协议表导致默认测试线程栈溢出；Lowerer、Export HIR 与 concrete HIR 改为共享同一 Arc 数据，未扩大线程栈或引入预算。原嵌套泛型用例在默认栈下已通过。
- 全仓 fmt／clippy、2840 项 HIR／HIR lowering／MIR lowering／slib 单元测试及 54 项真实泛型／enum 产物回归通过。核对 19 份旧快照，仅 HIR 构造／模式／穷尽性显示和一条派生相等字段路径改变，MIR／LIR 与诊断位置不变；关闭更新开关的 13 项专项复验覆盖全部变化，全部通过。日志 `/tmp/scoop-m23-6a-variant-refs-unit.log`、`/tmp/scoop-m23-6a-variant-refs-fixtures.log`、`/tmp/scoop-m23-6a-variant-refs-verify.log`。
- 此前已清理编辑器生成且未在构建中使用的 `target/debug/incremental`，释放约 1.23 GiB；阶段构建仍使用独立的 `target/m23-6a`。底层来源类型、声明与调用／正文迁移仍在继续。

## 共同限定类型名称与模式前缀

- 模式前缀和普通 qualified 类型引用共用包、import 分层、透明别名与静态嵌套声明查找。名称结果保存原 nominal ID 或完整 alias 类型；泛型声明前缀沿用 subject 实参，alias 仍要求完整 application 相等。删除依赖 enum 专用模式入口，以及被替代的 Current／Imported 类型限定名遍历。
- 完整声明查询补齐泛型嵌套 nominal；错误不会根据 subject 同名声明重试或绕过 import 遮蔽。可见性诊断统一显示并标注完整限定名，缺失嵌套类型保留完整前缀。名称选择与存储绑定模块分别为 161、68 行，模式形状主模块由 327 行降至 231 行。
- 新增 6 项源码测试、`m23-shared-enums/qualified.scoop` 及两个名称前缀反例；覆盖当前／exact／star／alias／包／嵌套类型、不同泛型实参、错误 exact 遮蔽、star 歧义与未导入名称。真实泛型默认值、再次发布、下游本地引用类型／Int／Unit 和普通／移动 GC 组合通过。
- 全仓 fmt／clippy、1294 项 HIR lowering 测试与 10 项相关真实产物测试通过。两份既有 qualified 类型诊断快照按完整前缀更新；新增三阶段与反例快照已核对，关闭更新开关的 5 项专项复验通过。日志 `/tmp/scoop-m23-6a-qualified-names-unit.log`、`/tmp/scoop-m23-6a-qualified-names-fixtures.log`、`/tmp/scoop-m23-6a-qualified-names-verify.log`。
- 确认没有构建占用后，再清理约 1.71 GiB 的 `target/debug/incremental`。当前名称解析已共用；nominal 存储、候选和正文其他路径继续按设计迁移。

## 按原定义统一具体化函数请求

- 函数请求统一为原函数／泛型函数／访问器／生成正文身份、完整宿主和完整实参；删除 Free／Method／Imported／ImportedMethod 请求分支及单独 MethodRequest。当前与依赖记录位置单独保存，不参与请求去重；同一队列处理普通、方法与词法正文，宿主参数和方法参数保持完整顺序，包含 `Ptr<T>` 的 pointee。
- 初始化、接口 dispatch、派生相等、core 协议、词法 parent 与默认局部值查询已适配共同请求。请求模块 170 行，函数 lowering 主模块由 271 行降至 155 行；现有正文存储适配仍待后续统一，本批未把它包装成阶段完成。
- 新增 `m23-shared-requests` 的 2 组源码与三阶段快照，覆盖 import alias 重复请求、Int／String／Unit、当前和依赖泛型方法、lambda／局部泛型与完整继承实参；单元测试确认同一 application 复用且不同宿主／方法实参分别具体化。
- 全仓 fmt／clippy、2848 项 HIR／HIR lowering／MIR lowering／slib 单元测试，以及关闭快照更新开关的 55 项真实泛型与 enum 产物回归全部通过。真实回归包含移走源码、再次发布、下游组合和普通／移动 GC；既有快照未变。日志 `/tmp/scoop-m23-6a-function-requests-unit.log`、`/tmp/scoop-m23-6a-function-requests-fixtures.log`。
- 新组合同时复现外来函数值默认参数的旧阶段限制；本批显式传入函数值验证请求身份，该缺口继续归默认正文与词法上下文迁移，不作为合法程序的最终限制。

## 词法 parent 保留原 callable 身份

- 删除依赖模板专用的 Function／Constructor parent handle，统一保存原 `CallableTemplateOwner`。局部函数、闭包与函数引用沿原定义及完整继承实参关联父 application；本地和依赖函数复用同一请求身份查询，内部 arena 只定位正文。
- 默认值／构造正文加载已不再传递仅用于 parent 归属的 arena ID，构造 helper 的多层透传参数一并删除。现有源码 binder、捕获、派生身份与 wire 格式保持。
- 全仓 fmt／clippy、2848 项 HIR／HIR lowering／MIR lowering／slib 单元测试，以及 10 项真实产物回归通过。覆盖构造器内闭包、局部泛型、函数引用、委托初始化、相同实例汇合、再次发布及普通／移动 GC；快照更新开关关闭。日志 `/tmp/scoop-m23-6a-lexical-parent-unit.log`、`/tmp/scoop-m23-6a-lexical-parent-fixtures.log`。
- 确认没有构建占用后，清理本轮临时生成的 `compiler/target` 和编辑器增量缓存，共约 391 MiB；继续复用 `target/m23-6a`。默认值正文统一与无父函数机器实例的词法归属继续迁移。

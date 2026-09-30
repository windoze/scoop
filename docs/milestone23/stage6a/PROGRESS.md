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

## 共同默认值正文展开与捕获绑定

- 默认值的局部值和语句使用普通 `Body`，源码与依赖共用同一展开器；依赖默认定义按原 root／path 加载一次，调用点只替换类型、receiver、参数和捕获。删除两块重复预检查遍历，加载入口改名为 `prepare`；加载主模块、定义映射和捕获绑定分别约 165、160、76 行，展开及 wire 子模块均控制在 500 行内。
- 默认 lambda、匿名函数、局部函数和函数引用保留原词法 owner；普通默认值中零参数词法正文使用空 application。发布闭包描述符时把实际词法实现加入既有正文闭包，普通默认函数引用在展开点建立 invoke，保留完整泛型宿主实参。
- 捕获记录分离实际读取来源与原绑定。默认展开造成两者不同的情况下，wire 保留原 callable、默认作用域、selector 和定义来源；读取默认正文与捕获描述符复用同一原绑定。新增 capture field 4，HIR interface 升至 `/41`，同步三份规范、Stage 7 格式表、required inventory、profile 固定向量和旧 `/40` 拒绝回归。
- 修正空 application 的外来词法定义查询、默认语句的实际调用位置，以及派生相等替换时的完整表达式来源；正常 `FunctionCoercion` 与装箱路径用于函数值默认参数。真实嵌套泛型回归另暴露声明查询中多余的 Lowerer 复制，移除该复制后原用例在默认线程栈下通过。
- 新增 `m23-shared-default-bodies` 的普通／泛型函数、方法和构造组合，包含局部函数、lambda、匿名函数、函数引用、型变适配、前序默认参数、Int／宽值／引用／Unit，以及两个诊断反例。真实发布后移走源码，再次发布、链接和普通／移动 GC 运行均通过。
- 全仓 fmt／clippy、2851 项相关单元回归通过，来源修复后追加的 1298 项 HIR lowering 回归通过。89 项真实产物与格式回归均已覆盖成功；关闭更新开关的 13 项语义组合与 3 项格式复验全部通过。核对旧快照：默认临时值名称、部分 arena 编号与捕获布局变化符合共同展开；core 与依赖布局快照的变化仅为完整产物指纹，Code／RuntimeImage 指纹保持。日志前缀 `/tmp/scoop-m23-6a-default-bodies-`，主要结果见 `verify.log`、`format-verify.log`、`fixed-cases.log` 和 `core-update.log`。
- 两次清理未被构建使用的 `target/debug/incremental`，共释放约 3.34 GiB；继续复用 `target/m23-6a`。本批完成默认展开共用，声明／类型来源分支、正文的 wire 到 imported 构造和其余调用调度仍按阶段设计继续删除。

## 共同上下文实参推断

- 函数重载、名义构造与依赖泛型调用共用 `infer_contextual_arguments`：同一候选持续积累约束，部分解提供完整期望类型，固定点停滞后才加入允许的外层结果约束和尝试默认类型。删除依赖专用 `generic/arguments.rs` 及源码重载、构造中的重复循环，不再为每轮推断重建约束环境。
- 部分替换按原声明 binder 查询，保留已解实参中的外层类型参数；结果分别保留宿主／callable 实参、源码顺序参数值及其 setup。spread 和命名整数组与源码调用共用精确相等约束，普通实参保留子类型约束。
- 空数组等输入的无上下文失败不会阻止后续完整标注匿名函数提供类型。新增 `shared-fixed-point` fixture，对照源码／依赖函数、struct／enum 构造、泛型宿主方法、双向函数引用和命名重排，并经再次发布覆盖引用、Int、Unit 应用及普通／移动 GC 运行。函数值字段调用改用已有共同字段引用，修正误走导入 accessor 路径的问题。
- 默认类型尝试的完整候选副本只存在于专用尝试调用中，通过交换提交成功状态；固定点自身不在递归栈帧中保存完整 Lowerer。原嵌套泛型用例在默认线程栈通过。共同固定点、表达式尝试、重载探测分别约 250、107、228 行；名义候选处理与上下文分类按职责分为约 192、422 行。
- 全仓 fmt／clippy 通过；1299 项 HIR lowering 单元回归与 22 项相关真实产物回归均已覆盖通过。关闭快照更新开关复验新推断组合、上界反例和数组／vararg 全组成功。一个单元断言按约束加入顺序调整类型列举；四份诊断快照分别反映正常结果类型检查和整数组精确相等，源码位置未变。日志前缀 `/tmp/scoop-m23-6a-contextual-`，最终结果见 `unit-final.log` 加 `diagnostic-verify.log`、`artifacts.log`、`regression.log` 加 `verify.log`／`arrays-verify.log`。
- 确认没有进行中的编译后清理 `target/debug/incremental`，释放约 496 MiB，继续复用约 3.6 GiB 的 `target/m23-6a`。本批删除了三套上下文推断循环；来源专用参数映射、普通依赖调用调度和 HIR 类型／正文来源分支仍需继续合并。

## 共同参数映射

- `CandidateArgumentMap` 直接服务源码与依赖声明：参数形状仅传递名称及必需／默认／vararg 协议，默认来源保持各自已有的类型化引用。删除依赖侧的命名、重复、遗漏、vararg 和 operator `set` 映射算法，以及 `ImportedParameterInput`；依赖默认值准备、实参类型模式和调用提交直接消费共同映射结果。
- 明确区分普通实参、vararg 元素／spread、命名整数组、空 vararg 和默认值；源码输入顺序与声明参数顺序分别保留。operator `set` 的最后值参数在共同形状上预留，不再复制并改写整份实参 AST。
- 原 662 行参数映射文件按职责拆为数据／查询 218 行、映射算法 217 行、源码适配 110 行和测试 187 行；依赖适配与原单测共 219 行。默认引用的类型参数只用于两个实际存储来源，不增加语义执行器或发布格式。
- 新增 `shared-mapping` 对照源码／依赖泛型函数的命名重排、前序参数默认值、混合 spread、整数组引用身份、空 vararg 和求值顺序；再次发布覆盖引用、Int、Unit，并在普通／移动 GC 下运行。全仓 fmt／clippy、1300 项 HIR lowering 单元测试、7 项相关真实产物回归及关闭更新开关的组合复验全部通过，既有快照无需修改。日志前缀 `/tmp/scoop-m23-6a-argument-mapping-`，结果见 `unit.log`、`artifacts.log`、`regression.log`、`verify.log`。
- 确认没有进行中的构建后再清理约 251 MiB 的旧增量缓存，继续复用约 3.6 GiB 的工作 target。本批完成实参映射统一；普通非泛型依赖调用、候选身份，以及 HIR 类型／正文来源分支继续按阶段设计合并。

## 普通依赖实参检查与结果类型边界

- 普通非泛型依赖调用以空推断环境进入共同实参引擎，删除独立的逐实参 lowering／子类型检查循环。完整签名与实参模式复用同次解析结果；签名解析失败直接诊断，不再回退为 `Any`。普通签名适配独立为 175 行模块，原 probe 主模块降至 326 行。
- 删除外层期望结果类型对普通依赖候选的淘汰。重载与作用域先按参数选择，结果不匹配在实际使用处报告；不能因此选择较宽参数重载或更低优先级的扩展函数。源码、泛型及普通依赖共用已有完整形参上下文，初始约束未变化时复用部分解。显式标注的 lambda／匿名函数保留自身签名并按既有函数型变适配。
- 新增 `m23-shared-native-calls` 的 2 组正例、7 组反例和 HIR／MIR／LIR、诊断快照，覆盖窄整数分支与 callable 结果、逆变／协变、默认值、尾随 lambda、命名／spread／整数组、普通成员和返回类型重载反例。真实发布后移走源码，再次发布并以引用／Int／Unit 应用进行普通／移动 GC 运行。
- 全仓 fmt／clippy 通过；1302 项 HIR lowering 单元覆盖通过（整套运行中两项旧断言修正后定向复验，新 fixture 与这两项共 4 项再次通过）。13 项真实产物覆盖通过，更新后的两组 fixture 均关闭更新开关复验。既有普通构造器的 14 份阶段快照仅同步此前共同默认值名称及字段显示，5 份诊断改为外层实参错误；`Any` 返回错误改由函数正文类型检查报告。日志前缀 `/tmp/scoop-m23-6a-native-calls-`，结果见 `unit.log`、`diagnostics.log`、`regression.log`、`artifacts.log`、`verify.log`、`constructors-verify.log`。
- 无进行中的构建时清理约 487 MiB 的旧增量缓存。本批完成普通依赖实参检查统一；共同候选身份、剩余 MSC 调度、nominal 类型与正文存储仍继续迁移。

## 共同 MSC 筛选与实际接收者声明闭包

- 普通函数、成员、混合 function-like 候选及构造委托共用 55 行的 MSC 集合筛选：声明转发关系、非参数化优先，以及仅对互相可转发集合应用的默认值数量／vararg 优先。删除三处重复矩阵和集合算法；原 typed 目标、提交、诊断顺序及整数字面量偏好继续由调用入口持有。声明视图直接提供 binder 和映射后的声明类型，不使用本次推断实参比较候选。
- 重载筛选文件由 133 行缩至 37 行，构造决议降至 159 行；删除仅包装声明视图的两种转发类型。新增 `m23-shared-selection`，对照当前源码和依赖的严格参数选择、泛型／普通优先、默认值、vararg、同名函数／构造器、成员及 this／super 委托。3 组正例带 HIR／MIR／LIR 快照，6 组反例锁定不可比较声明的歧义及位置。
- 真实组合发现并修复私有派生接收者的发布缺口：实际普通依赖调用的静态 receiver 与其他物化类型在同次遍历收集，沿已有共享声明闭包保留其原继承和表示依赖。私有声明保留原身份及可见性，不产生 public binding，不把整个私有类型 arena 作为发布根。
- 共同筛选切换后全仓 fmt／clippy、1305 项 HIR lowering 测试及 6 项相关真实产物回归通过；接收者修复后再次 fmt／clippy，856 项 HIR 测试、125 项相关 lowering 测试和 2 项继承产物回归全部通过。新增 fixture 经过真实发布、移走源码、再次发布、引用／Int／Unit 应用及普通／移动 GC 运行，关闭快照更新后复验通过。日志前缀 `/tmp/scoop-m23-6a-selection-` 与 `/tmp/scoop-m23-6a-selection-receivers-`。
- 无进行中的构建时清理约 243 MiB 的旧增量缓存。本批完成共同 MSC 集合筛选；候选原身份、整数字面量偏好调度、nominal 类型及正文的来源分支仍需继续删除。

## application 之前的原声明身份

- nominal 收集在建立 self application 之前生成原声明身份，词法父声明直接复用已有记录；object 和 generated backing class 分别保留自身身份。四类 application 缓存改用原 `SourceNominalId` 与完整实参，不再以当前声明 arena 索引作为缓存键。
- 删除完成声明树后递归重建身份的 builder、访问集合和重复 owner 解析；完成声明收集后只整理已建立的记录，原有字段、variant、导出及具体化查询继续复用同一身份。该调整不改变 wire payload 或身份算法，`Type::Imported*` 及 application 的剩余存储迁移继续推进。
- 身份模块由 435 行降至 208 行。原声明收集按 struct／enum／class／interface 拆为 116～184 行模块，共同名称检查保留在 72 行主模块；object 收集与其查询／解析分开为 254、266 行。
- 全仓 fmt／clippy、1305 项 HIR lowering 单元测试及 8 项真实产物回归全部通过；限定名、嵌套 re-export、原函数请求、默认值、bound 和共同 MSC 的现有快照均无需更新。日志前缀 `/tmp/scoop-m23-6a-nominal-identities-`，结果见 `unit.log` 和 `artifacts.log`。

## 普通宿主的封闭泛型父类型

- 普通宿主的基类与接口沿原 nominal 声明和完整 receiver application 查询父边、槽顺序及已经选定的实现。删除“父类型含泛型 application”导致整个普通宿主不可物化的门；公开根、私有表示依赖及 core 的四类整数范围继续使用既有实际需求闭包。不同实参的继承节点按完整 exact type 区分，不补造普通声明身份。
- 槽根和目标从完整签名 receiver 取得原宿主与实参，删除重复 owner field 2；原 domain field 5 继续退役。type-semantics 升至 `/11`，同步三份规范、Stage 6／7 格式说明、required inventory 与 profile 固定向量，保留旧 major 和旧 slot payload 的明确拒绝测试。HIR interface 仍为 `/41`，runtime ABI 未改变。
- producer、reader 与 MIR 复用实际应用的签名和 Strong／ODR 目标连接；接口应用槽展开与既有槽验证共用原父顺序／override 展开。删除两份共 219 行的源码 callable 投影，以及 MIR 的重复 Strong-only 分支。本批修改的 Rust 文件均在 500 行内。
- 新增 `m23-shared-parents` 的 class、interface、组合三组正例与三个反例，覆盖本地／依赖的公开泛型父类、未覆写的继承方法、接口默认实现、super 调用、嵌套实参和私有表示依赖。源码发布后移走，再次发布、引用／Int／Unit 应用以及普通／移动 GC 运行均通过；新快照已关闭更新开关复验。
- 全仓 fmt／clippy 通过，3202 项 HIR／HIR lowering／MIR／MIR lowering／slib 单元测试均已覆盖通过。36 项相关真实产物回归均已覆盖成功；关闭更新开关后的 core 22 项（含初始化 Link 组）和继承／受保护成员 4 项专项复验全部通过。旧快照已核对：新增范围与迭代器表示、普通宿主的必需支持、函数编号顺序、共同字段显示和完整限定名诊断；Link 对象、注册项与指纹反映实际新增物化。日志前缀 `/tmp/scoop-m23-6a-shared-parents-`。
- 本批清理约 1.73 GiB 旧增量缓存，继续复用 `target/m23-6a`。共同 nominal 类型、候选原身份、普通／模板／初始化正文及其 wire 适配仍按阶段设计继续迁移；本批通过不代表 6a 已完成。

## 接口槽选择与整数范围

- 成员查询按完整接口 application 和原派发 slot 复用声明中已经选定的实现；当该实现作为可见成员进入同一候选层时，删除重复的接口声明入口。移除未替换 binder 的原签名比较，保留真正不同的重载及接口默认实现。primitive 查询直接读取原选择，不要求提前物化完整接口实现。
- 新增 `m23-shared-ranges` 与 `m23-shared-interface-members`，覆盖四类整数范围、开闭区间、break／continue、普通 class 与泛型 struct 的接口成员、同名重载、默认方法及错误实参诊断。全部使用真实库发布、移走源码、再次发布、引用／Int／Unit 应用和普通／移动 GC 运行；新 HIR／MIR／LIR 及诊断快照已关闭更新开关复验。
- 共 1309 项 HIR lowering 单元测试通过。接口候选查询与原槽选择查询分别保持在 250 行内及 73 行；没有增加 wire 字段、物化根或重复的 override 验证。
- 全仓 fmt／clippy 与 21 项相关真实产物回归均已覆盖通过。更新两组既有测试中的七份旧快照：共同字段编号、enum 构造节点及默认局部值名称；两个完整测试组关闭更新开关复验通过。日志前缀 `/tmp/scoop-m23-6a-ranges-`，最终结果见 `unit-verified.log`、`final-artifacts.log` 加 `conformance-verify.log`／`values-verify.log`。
- 无进行中的 cargo／rustc 时清理约 1.19 GiB 的旧增量缓存，继续复用 `target/m23-6a`。本批完成既定接口选择的候选复用，声明／类型及正文的剩余来源表示继续迁移。

## 共同函数正文具体化

- 原定义请求在声明查询边界取得完整签名、实现类别、接收者和捕获绑定，之后共用正文、参数、结果、捕获及方法派发降低。源正文直接借用已完成的记录，删除逐实例复制整个源码／依赖正文的处理，以及 145 行的依赖专用函数降低文件；实际 application 请求继续进入同一原定义固定点。
- 声明派发共用原虚方法 family／接口 slot 表示，源接口成员在查询边界取得原 slot，实际槽号从完整接口 application 的已有映射获取。含正文的参数必须从局部值映射取得编号，取消缺失时回退原 arena 编号；显式 intrinsic／extern 仍沿无正文签名处理，初始化协调函数保持所属单元的 MIR 生成契约。
- 扩展 `m23-shared-requests/captured-parameters`，对照源码／依赖中的多层局部函数、泛型局部调用和四项捕获；实际发布、移走源码、再次发布及引用／Int／Unit 的普通／移动 GC 运行全部通过。新 HIR／MIR／LIR 快照关闭更新开关复验通过，既有快照无需修改。
- 全仓 fmt／clippy、1310 项 HIR lowering 单元测试和 8 项相关真实产物回归均已覆盖通过，包含方法／接口 bound、构造、委托初始化局部函数、派生相等、函数引用和默认正文。函数替换、声明读取、请求队列分别为 169、174、219 行。日志前缀 `/tmp/scoop-m23-6a-function-definitions-`，结果见 `unit.log` 加 `captures-verified.log`、`artifacts.log` 和 `artifacts-verify.log`。
- 本批统一实际替换算法；当前／读入声明的存储定位仍有机械适配，正式语义图中的 nominal、候选及正文存储来源表示继续按阶段设计迁移。wire payload 和 runtime ABI 未改变。

## 原声明子类作用域与共同祖先查询

- `protected` 的子类访问约束统一保存原 `SourceNominalId`，删除依赖专用约束及当前／跨来源／依赖的三套继承遍历。作用域包含、覆写覆盖与实际访问共用一套祖先查询；现有声明索引只用于定位记录，完整类型实参、词法作用域和显式接收者规则保持。
- 原访问域构建与查询保留在 327 行主模块；公开声明投影、类型访问域和祖先查询分别为 164、223、97 行。没有增加 wire 字段、声明副本或额外访问证明。private 词法 owner 的剩余 arena 表示继续按阶段设计迁移。
- 全仓 fmt／clippy、856 项 HIR 与 1310 项 HIR lowering 单元测试全部通过。复用既有独立和组合 fixture 的 7 项真实产物回归全部通过，包含泛型 protected、封闭父类型、函数引用、嵌套受保护类型、setter、普通继承正例及负例；全部快照无需修改。日志前缀 `/tmp/scoop-m23-6a-visibility-`，结果见 `unit.log` 与 `artifacts.log`。

## 共同函数实现类别、抽象槽与初始化协调函数

- 当前和读入函数共用 `FunctionKind`，抽象方法／访问器保留完整参数局部值并使用明确的 Abstract 类别；初始化协调函数使用 InitializationEnsure 类别，只有实际初始化正文属于 User。删除依赖专用实现枚举及具体化的二次适配枚举，避免把无正文声明伪装为空函数。局部值身份、类型需求、core Iterator 合同、effect 投影和 MIR 消费同步使用真实类别。
- 抽象槽具体化沿共同参数替换，MIR 保留既有 trap；初始化协调实现从所属初始化单元生成，未改变顺序、失败缓存或重入语义。真实再发布验证发现的抽象属性名称差异已按原 getter／setter 身份修正，源码与读入声明产生相同的 trap 字符串及 ODR 定义。
- `globals.rs` 从 1481 行按存储、初始化、常量 image、扩展属性职责拆为 514 行主模块及四个 209–303 行子模块；dump 主模块降至 489 行，具体局部值身份主模块降至 427 行。函数替换、声明读取和抽象读入分别为 172、146、133 行，没有新增语义验证框架或 wire 字段。
- 新增 `m23-shared-abstract` 的源码／依赖两组正例、六份阶段快照和两个反例，覆盖泛型抽象方法、getter／setter、默认实现、局部覆写、函数引用、引用／Int／Unit 组合及移动 GC。真实发布后移走源码，再次发布、ODR 比较、链接和运行通过；新快照关闭更新开关复验通过。
- 全仓 fmt／clippy、2282 项 HIR／HIR lowering／MIR lowering 单元测试均已覆盖通过。15 项真实产物回归均已覆盖成功，包含抽象继承正反例、泛型 bound、成员、委托初始化及完整初始化 Link 组；最后一项包含初始化顺序、失败缓存、物理定义与损坏产物验证。五份旧成员快照仅修正访问器名称及 trap 字符串，另同步一份早期共同字段显示快照；两个完整测试组均关闭更新开关复验通过。日志前缀 `/tmp/scoop-m23-6a-implementations-`，结果见 `unit.log`、`singleton-verified.log`、`accessors-unit.log`、`artifacts-verified.log`、`members-verified.log` 及 `core-initialization.log`。
- 确认没有 cargo／rustc 占用后，两次清理旧增量缓存，共约 3.6 GiB；继续复用 `target/m23-6a`。本批统一实现类别及消费，当前／读入声明的完整存储、nominal 类型、候选及正式正文编解码继续按阶段设计迁移。

## private 词法访问域使用原声明身份

- 删除保存 Class／Interface／Struct／Enum／Object arena ID 的 `VisibilityOwner`，成员 private 约束统一保存原 `SourceNominalId`。词法包含、定义文件和 protected 词法上下文继续通过既有声明索引定位；域正规化直接按完整约束排序，删除手写的 arena 排序键。相关实现净减少 55 行，没有扩展依赖可见性或增加身份映射表。
- 全仓 fmt／clippy、856 项 HIR 和 1312 项 HIR lowering 单元测试全部通过；既有 private setter 结构断言同步验证原 class 声明身份。4 项真实产物回归全部通过，覆盖泛型 protected／private、嵌套词法作用域、函数引用、继承访问及错误诊断；快照无需改动。
- 日志前缀 `/tmp/scoop-m23-6a-private-owners-`，结果见 `unit.log` 和 `artifacts.log`。nominal application 与完整声明／正文中的其余来源表示继续迁移，本批不宣告 6a 完成。

## 构造调用保留完整期望 application

- 修复真实 Option 回归发现的别名目标丢失：依赖构造器和 variant 在检查实参前，将同一原 nominal 的完整期望 application 加入既有等式约束。与当前声明构造保持一致，不再把别名固定实参当成可被 payload 推断覆盖的普通函数结果提示；普通泛型函数的延期结果推断保持。
- 新增四个独立反例，连同既有错误 payload 反例，覆盖别名链、命名 payload、结果转为 Any、直接期望 application 和 class 构造器。诊断定位错误实参；原反例快照无需改动，新诊断快照关闭更新开关复验通过。
- 全仓 fmt／clippy、1314 项 HIR lowering 单元测试通过。Option 完整组和泛型构造器完整组的真实发布、源码移走、再发布及运行均通过；同时执行的 enum／接口成员等九项不同真实产物测试均已覆盖成功。日志前缀 `/tmp/scoop-m23-6a-constructor-applications-`，结果见 `unit.log`、`options-verified.log` 和 `verified-artifacts.log`。
- 未增加推断器、wire 字段或 runtime ABI；修改后的泛型调用主模块为 470 行。确认没有 cargo／rustc 占用后清理约 0.72 GiB 旧增量缓存，继续复用 `target/m23-6a`。

## 共同 enum 具体化与实例缓存

- enum 具体化按原声明身份和完整类型实参复用同一实例缓存。声明读取后统一分配、替换 variant payload、计算 GC 属性及降低既定接口实现，删除当前／依赖的两套处理；递归引用先登记身份，字段与 GC 完成后再降低方法和接口。普通外部方法保留提供方定义归属，没有复制方法或重新选择接口实现。
- 声明读取直接借用既有字段类型和原 variant／payload 字段身份，不复制整个声明。当前定义与已应用的依赖记录分别传递其实际替换环境，保证嵌套泛型和 consumer-local 实参对应正确；已在前端检查的参数数目与 NoGC 规则不在具体化中重复断言。
- 新增 `m23-shared-enums/instances` 及 HIR／MIR／LIR 快照，覆盖本地／依赖 enum、别名、嵌套 Option 与 enum payload、引用／Int／Unit、泛型调用及下游再次发布。原四组快照无需修改；完整五组正例关闭更新开关复验通过，单元测试同时验证原身份加完整实参唯一、payload 替换及 GC 属性。
- 全仓 fmt／clippy、最终 1314 项 HIR lowering 单元测试及九项不同真实产物回归均已覆盖成功，包含普通 enum、Option、重建 core、派生相等、构造和接口成员组合。最终日志前缀 `/tmp/scoop-m23-6a-constructor-applications-`，结果见 `unit.log`、`verified-artifacts.log` 和 `enums-verified.log`；早期新 fixture 验证见 `/tmp/scoop-m23-6a-enum-definitions-shared-verified.log`。
- 具体化主文件从 631 行按实际编排职责拆为 384 行主模块和 253 行运行模块，enum 具体化与声明读取分别为 126、110 行。前端 nominal application 与完整声明存储的来源表示仍须继续迁移，本批只统一已经消费已检查声明的实际具体化算法；wire 与 runtime ABI 未改变。

## 共同 struct 具体化、内建表示与 C ABI

- struct 的声明读取后共用实例分配、字段替换、GC 属性及接口记录，缓存统一使用原声明身份与完整实参；构造器身份和 core 协议查找同步使用该键。递归字段先取得同一实例身份，字段与 GC 完成后再请求方法、接口和既有自动构造器；删除两份来源专用流程及重复的 NoGC／参数数目断言。
- 整数、Boolean、Ptr、FunPtr 的既定机器表示进入同一 struct 分配过程，保留 canonical type、实际 C 布局和 C ABI 投影。接口闭包从已具体化的父类型读取；primitive 普通方法与装箱适配仍由提供方拥有，未复制外部实现、增加物化根或重复选择成员。
- 新增 `m23-shared-structs` 两组正例及两个反例，覆盖本地／依赖字段、嵌套 Option、别名、接口默认方法与继承、C 布局、引用／Int／Unit 和下游本地 class。真实发布、源码移走、再次发布及普通／移动 GC 运行全部通过；六份阶段快照和两份诊断快照关闭更新开关复验通过。
- 全仓 fmt／clippy、1315 项 HIR lowering 单元测试及 11 项不同真实产物回归全部通过，包含泛型构造器、接口成员、解构、指针、interior mutability、装箱与普通 C ABI、派生相等及完整 core 源码三组导出闭包。既有快照无需修改。日志前缀 `/tmp/scoop-m23-6a-struct-definitions-`，结果见 `unit.log`、`shared-verified.log` 和 `artifacts.log`。
- struct 具体化、声明读取和 primitive 请求分别为 179、177、67 行；原 nominal 文件降至 156 行。完整声明／application 的前端来源表示与 class／interface 具体化继续迁移，wire 与 runtime ABI 未改变。

## 共同 class 具体化与对象重定位身份

- class 使用原声明和完整实参复用实例，当前与依赖声明在读取后共用字段、基类、既定虚方法和接口记录的具体化。String／Array／MutableArray 保留既定表示；当前 object 的 backing class 在递归前登记，外部普通虚方法保留原 family 和定义方实现。删除 134 行的依赖 class 具体化，主文件从 454 行降至 295 行，声明读取和全局实体分别为 149、213 行。
- 新增 `m23-shared-classes` 两组正例与两个反例，覆盖封闭与泛型继承、字段、虚方法、接口、Array 转换、object 状态及引用／Int／Unit。真实发布后移走源码，下游使用本地 Payload 再次实例化；六份阶段快照、两个诊断及普通／移动 GC 运行均关闭更新开关复验通过。
- 再次发布组合揭示同一静态存储在本地与依赖重定位中编码不同，导致机器对象相同却被 ODR 比较拒绝。按三份 spec 与 Stage 7 设计先修合同：所有 Strong shape 重定位共用实体与定义角色编码，provider 仍保留在普通依赖引用和符号连接中；没有放宽 ODR 合并或复制状态。复用已有 `definition_parts`，删除旧 tag 12 分支；link-identity-closure 从 `/6` 升至 `/7`，更新 required profile 与固定向量，保留旧版本明确拒绝测试。
- 全仓 fmt／clippy 通过，1316 项 HIR lowering、466 项 LIR、584 项 slib 单元测试均已覆盖成功。14 项不同非 core 真实产物回归及完整 core 22 项均关闭更新开关通过，覆盖继承、成员、构造、初始化、委托、数组与 Link 损坏产物。core 新变化的 47 份快照只含 artifact／Code／runtime 摘要；函数顺序、共同字段显示和默认局部值名称的既有快照也已按完整组复验。
- 最终日志前缀 `/tmp/scoop-m23-6a-class-odr-`，结果见 `unit-verified.log`、`updated-verified.log`、`other-verified.log` 和 `core-verified.log`；原 HIR lowering 验证见 `/tmp/scoop-m23-6a-class-definitions-`。确认没有 cargo／rustc 占用后清理约 375.5 MiB 旧增量缓存，继续复用 `target/m23-6a`。完整 nominal application、候选与正文存储继续迁移，本批不代表 6a 完成。

## 共同 interface 具体化与继承参数替换

- interface 实例按原声明和完整实参复用，family 按原声明复用。当前与依赖定义共用父类型、原槽、覆写去重和具体方法签名生成；声明读取只适配已有记录，保留普通外部默认实现的定义归属。删除依赖 interface 具体化及空的来源专用 nominal 模块，主流程、声明读取、继承查询分别为 169、112、97 行。
- 继承遍历保存到达各方法声明时的完整实参，修复本地父接口交换参数后仍以最外层实参替换外来方法的错误。父链和菱形去重使用原 slot 与完整 application，不重做 override 选择，也不新增物化根。
- 新增 `m23-shared-interfaces` 两组正例、六份阶段快照及两个反例，覆盖父接口参数交换、恢复顺序、菱形继承、默认方法、getter／setter、缺失实现与错误返回类型。源码移走后再次发布，下游本地类型、引用／Int／Unit 和普通／移动 GC 均通过；新快照关闭更新开关复验通过。
- 全仓 fmt／clippy、1317 项 HIR lowering 单元测试及 13 项不同真实产物回归全部覆盖成功，包括普通值／primitive 接口、抽象访问器、限定 super、泛型成员、三类 nominal 实例及实际重建 core 声明。四份旧 super 快照仅同步共同字段显示和默认局部值名称，完整组关闭更新开关复验通过。
- 日志前缀 `/tmp/scoop-m23-6a-interface-definitions-`，结果见 `unit.log`、`shared-verified.log`、`artifacts.log` 与 `super-verified.log`。本批统一接口的实际具体化算法；完整前端声明／application 和正文的来源存储继续按设计迁移，wire payload 与 runtime ABI 未改变。

## 共同字面量模式调用与完整来源位置

- Boolean／String 字面量模式保存定义处选择的共同 `CallableTarget`，当前声明与普通依赖的 `equals` 进入同一模式、默认值、effect 和 MIR 调用处理；整数继续使用既定的 typed 比较。实际模式的隐式调用随原有正文遍历进入依赖闭包，未使用模板不因此成为发射根。
- 修复真实再次发布发现的来源位置缺口：字面量模式复用完整 `DefaultExpressionV1`，同时保留定义位置、求值位置及类型，不再从 arm 位置重建常量。producer、reader、引用收集与普通表达式共用路径；增加非法表达式与旧常量 payload 的格式反例，往返测试刻意使用不同的定义／求值位置。
- 实际 wire 字段改变，HIR `cross-cone-interface` 从 `/41` 升至 `/42`；同步三份 spec、Stage 7 说明、required profile、固定向量与旧版本拒绝测试。43 份既有快照经核对只更新完整摘要，没有引入新的 runtime ABI 或机器行为。
- 新增 `m23-shared-literals`，覆盖 Boolean／String、嵌套 struct 模式、guard、默认表达式、泛型正文、别名与 Option，包含两组正例和两个反例。源码移走后再次发布，下游本地 Payload 与引用／Int／Unit 的普通／移动 GC 运行均通过；六份阶段快照和两个诊断关闭更新开关复验通过。
- 全仓 fmt／clippy、2873 项 HIR／HIR lowering／MIR lowering／slib 单元测试均已覆盖通过，最后的引用收集修复后再次执行全部 1317 项 HIR lowering 测试。9 组真实产物回归和完整 core 22 项均关闭所有快照更新开关通过。最终日志前缀 `/tmp/scoop-m23-6a-literal-wire-`，结果见 `lower-verified.log`、`slib-verified.log`、`artifacts-verified.log` 与 `core-verified.log`。
- 按实际职责拆出 pattern 格式错误、默认模式读取和 callable effect 查询，相关主文件控制在 502 行以内。无 cargo／rustc 占用时清理约 2109.6 MiB 旧增量缓存，继续复用 `target/m23-6a`。共同 target 是声明／application 存储迁移的一步，本批不代表全部候选或完整语义图已经统一。

## class 上界属性与已选访问器

- 类型参数通过其完整 class 上界查询属性，保留原静态 receiver 的访问域与实际值。继承中的宿主实参按原声明替换；计算 getter／setter 复用普通 class-bound 调用，存储属性继续使用原字段身份。默认值、复合赋值、函数值属性和安全访问沿既有正文、类型替换与具体化处理，没有新增 wire 或 runtime 入口。
- 新增 `m23-shared-bound-properties` 两组正例及三个反例，覆盖当前／依赖声明、交换参数的继承、存储与虚访问器、默认读取、安全访问、函数值调用、复合赋值，以及错误值类型、只读属性和超出定义处上界的成员。真实发布后移走源码，下游本地 Payload 与引用／Int／Unit 的普通／移动 GC 运行均通过；六份阶段快照和三份诊断关闭更新开关复验通过。
- 全仓 fmt／clippy 与全部 1318 项 HIR lowering 单元测试通过；新增单元检查 getter／setter 保留声明方完整实参和原 receiver。9 组相关真实产物回归全部覆盖通过，包含泛型成员、扩展属性、受保护成员、默认值、class／interface 及继承初始化 ABI。
- 两组旧快照差异经上一个已验证编译器复现，实际输出与本批一致；14 份旧快照只同步抽象访问器名称及既定函数顺序，完整两组更新后再次关闭开关通过。最终日志前缀 `/tmp/scoop-m23-6a-bound-properties-`，结果见 `all-unit.log`、`artifact-verified.log`、`regressions.log` 和两个 `snapshot-verified-*.log`。
- 属性主文件从 625 行拆为 394 行；查找、存储读写和访问器目标按实际职责分别为 125、130、26 行。继续复用 4.0 GiB 的 `target/m23-6a`，没有删除活动构建目录。完整 nominal application 与声明存储迁移继续推进。

## 名义应用保留原声明身份

- struct／enum／class／interface 的符号化 application 直接保存原 `SourceNominalId` 和完整实参，四种 application ID 保持独立。原身份通过已有声明关系定位存储；字段、variant、构造与 core 协议的引用核对同步使用原身份。新增声明 arena 重排测试，证明 application 仍定位同一原 enum，并拒绝重排后落在旧位置的其他声明。
- 完整替换、方法参数替换、部分推断提示和求解器的类型项物化共用 nominal application 查询与重新应用入口，删除四种 nominal 在各替换器中的重复分支。类型替换文件从 325 行降至 176 行；FunPtr 的完整函数签名继续正规化为既定表示，没有新增机器根或修改 wire。
- 原 1500 行左右的 nominal 声明文件按 struct、enum、class、字段、成员、构造、intrinsic 和测试职责拆分，生产子模块均不超过 325 行。MIR 测试构造器使用相同的原声明关系，未增加另一套编译或身份生成路径。
- 全仓 fmt／clippy、859 项 HIR、1318 项 HIR lowering 和 114 项 MIR lowering 单元测试全部通过。13 组真实产物回归全部关闭快照更新开关通过，覆盖四类 nominal、构造、指针、函数引用、默认值、数组、bound、属性及完整 core 的三组 MIR／LIR 导出闭包；既有快照无需改动。日志前缀 `/tmp/scoop-m23-6a-application-origins-`，结果见 `unit.log`、`build.log` 与 `artifacts.json`。
- 确认没有 cargo／rustc 占用后清理 1764.0 MiB 旧增量缓存，继续复用 `target/m23-6a`。本批完成 application 原身份和共同替换；其余来源专用类型及完整声明存储继续迁移，不据此宣告 6a 完成。

## 求解器共用原声明应用与 intrinsic 表示

- 删除求解器 `NominalApplication` 的 Struct／Class／Enum／Imported 分支，约束统一保存原声明和完整类型项；求解后的类型应用使用现有共同入口。构造器与 variant 的目标 ID 保持独立，未把构造目标改成普通函数。
- Ptr／FunPtr 的表示从真实声明取得，删除求解器中的另一套指针应用分配。数组类型信息按原声明的 intrinsic 类别和实际元素实参查询，移除当前／依赖的重复数组判定。既有求解器测试改用真实 intrinsic 声明表示，并验证非法 `FunPtr<Int>` 被拒绝。
- 全仓 fmt／clippy、2291 项相关单元测试与 6 组真实产物回归全部通过；覆盖指针及下游 pointee、数组／vararg、泛型构造、bound 和完整 core 三组 MIR／LIR 导出闭包。快照无需修改，wire 和 runtime ABI 保持。日志前缀 `/tmp/scoop-m23-6a-nominal-constraints-`，结果见 `unit.log`、`build.log` 与 `artifacts.json`。

## 调用与函数引用共用候选约束环境

- 候选环境直接接收宿主／callable 形参组、已知宿主实参、显式类型实参及已绑定 receiver 关系，不再要求源码 `CallableView`。普通源码调用、依赖普通／泛型调用和 callable reference 共用该入口，删除各自重建形参上界、宿主绑定和显式参数约束的代码；构造结果与函数引用的精确签名约束保持原角色。
- 全仓 fmt／clippy、全部 1318 项 HIR lowering 单元测试与 7 组真实产物回归全部通过。覆盖构造、bound、函数引用、默认值、数组、指针 intrinsic 与 consumer-local pointee，已有快照无需修改。日志前缀 `/tmp/scoop-m23-6a-callable-environments-`，结果见 `unit.log`、`build.log` 与 `artifacts.json`。
- 本批生产代码净减少 34 行；依赖泛型调用主文件降至 446 行。剩余候选目标和声明存储仍按阶段设计继续统一，没有将环境合并等同于全部候选迁移完成。

## enum 共用 application 与带形参的 payload 定义

- 删除 `Type::ImportedEnum`、`ImportedEnumType` 及依赖专用的单位 variant 常量。当前和依赖 enum 统一使用原声明身份加完整实参的 `EnumApplication`，变体、payload、直接接口及既定接口实现保存在共同 `EnumDefinition` 中；依赖记录在声明的形参环境中解码一次，应用不再复制预替换的 payload。
- 统一 variant 查询、字段替换、类型身份、GC 分类、父接口查询和具体化入口。递归签名先登记同一原身份，再完成字段与接口记录；具体化使用声明实参替换，普通外部方法保持原归属。单位 variant 的静态 image 与普通表达式共用原 variant 身份和完整 owner，常量探测复用已有表达式处理及失败事务。
- 新增 `m23-shared-enum-definitions` 两组正例和六份阶段快照，覆盖 enum／class 递归引用、静态单位 variant、泛型接口及默认方法、嵌套 Option、本地与依赖声明对照。发布后移走源码，下游再次发布，consumer-local 类型、引用／Int／Unit 及普通／移动 GC 运行均通过；新快照关闭全部更新开关复验通过。原 enum 实例单元同时验证依赖 payload 保留声明 binder，并由多组完整实参复用。
- 全仓 fmt／clippy、859 项 HIR、1319 项 HIR lowering、114 项 MIR lowering 单元测试全部通过。15 组不同真实产物回归均通过，包括新递归定义、既有 enum 正反例、Option 与重建 core、普通 enum、派生相等、完整成员模板、bound、指针、数组、构造及完整 core 三组 MIR／LIR 导出闭包；既有快照无需修改。日志前缀 `/tmp/scoop-m23-6a-shared-enum-types-`，结果见 `unit.log`、`build.log`、`new-verified.log` 与 `artifacts.json`。
- enum 声明、依赖定义解码、常量处理和具体化主文件分别为 356、145、228、122 行。确认没有 cargo／rustc 占用后清理 204.7 MiB 旧增量缓存，继续复用 `target/m23-6a`。本批未修改 wire payload 或 runtime ABI；其余 nominal 类型、完整声明成员／条件、候选及正文存储继续按 6a 设计统一。

## struct 共用 application 与带形参的字段定义

- 删除 `Type::ImportedStruct`、`ImportedStructType` 与依赖专用 struct 常量。当前与依赖声明使用原身份和完整实参的 `StructApplication`，字段、直接接口及既定接口实现保存在共同 `StructDefinition` 中；依赖定义只在声明形参域内解码一次，应用不再复制预替换的字段。类型身份、投影、解构、GC 分类、接口查询和具体化统一消费该定义，intrinsic 表示及实际 C 布局属性继续保留。
- 拷贝更新改用原字段身份，共用字段替换、基值与更新表达式的求值顺序和结果组装。修复产物中已有 `StructConstruct` 在读入默认值／泛型正文时被拒绝的缺口：解码直接恢复共同节点，后续仍由已有正文替换处理，不再次选择构造器，也不复制普通外部实现。
- 新增 `m23-shared-struct-definitions` 两组正例、三组反例及六份阶段快照，覆盖 struct／class 递归字段、接口默认方法、当前／依赖拷贝更新、具副作用的逆序更新、原值保持、泛型默认值、closure 和再次发布。消费方本地类型及 String／Int／Unit 在普通／移动 GC 场景运行通过；重复字段、未知字段和错误字段类型保留消费方准确位置。新快照与诊断全部关闭更新开关复验通过。
- 全仓 fmt／clippy 无警告，859 项 HIR、1319 项 HIR lowering、114 项 MIR lowering 单元测试通过。15 组不同真实产物回归均已覆盖通过，包含既有 struct、递归 enum、默认值、完整成员与构造、派生相等、bound、指针、数组、Option 及完整 core 三组 MIR／LIR 导出闭包。既有 bound 默认值 HIR 快照只调整新增声明形参带来的一个临时 TypeId（23→25），完整组已严格重跑。日志前缀 `/tmp/scoop-m23-6a-shared-struct-types-`，结果见 `unit.log`、`build.log`、`new-verified.log` 与 `verified-artifacts.json`。
- struct 声明、依赖定义解码、具体化和拷贝更新主文件分别为 150、124、166、237 行；正文解码主文件为 471 行。确认没有 cargo／rustc 占用后清理 1076.3 MiB 旧增量缓存，继续复用 `target/m23-6a`。本批没有改变 wire payload 或 runtime ABI；其余 nominal 类型、完整声明条件／成员、候选及正文存储仍按 6a 设计继续统一。

## 跨依赖泛型包装器的按值循环检查

- 共同 struct／enum 定义提供实际参与内联布局的形参关系；字段检查把依赖包装器的完整实参代入当前声明图，拒绝直接、间接、混合及不断增长实参的按值循环。提供方已检查的声明不重复成为本地 SCC 顶点；Phantom 参数、class／interface／函数引用及指针继续构成实际布局边界，没有展开无限应用或引入资源预算。
- 新增 `m23-shared-value-layouts` 两组正例、四个反例及六份阶段快照，覆盖依赖与本地包装器组合、Phantom、经 class 的合法递归、接口装箱和嵌套模式。修复前真实源码单元已复现非法布局被接受；修复后四个诊断均准确定位消费方声明，源码移走后再次发布及普通／移动 GC 运行通过。新快照与诊断全部关闭更新开关复验通过。
- 全仓 fmt／clippy 无警告，859 项 HIR、1320 项 HIR lowering、114 项 MIR lowering 单元测试全部通过。七组不同真实产物回归通过，包括新布局规则、共同 struct／enum、bound、泛型构造与完整 core 三组 MIR／LIR 导出闭包；既有快照无需修改。日志前缀 `/tmp/scoop-m23-6a-shared-value-layouts-`，结果见 `before.log`、`unit.log`、`new-verified.log` 与 `verified-artifacts.json`。
- 循环检查主文件缩至 244 行，内联参数分析为 141 行。确认没有 cargo／rustc 占用后清理约 1122.7 MiB 旧增量缓存，继续复用 `target/m23-6a`。本批未改变 wire payload 或 runtime ABI；完整 nominal、条件与成员、候选和正文存储继续按 6a 设计推进。

## class 共用 application 与带形参的字段、父类型定义

- 删除 `Type::ImportedClass`、`ImportedClassType` 和预替换的依赖字段记录。当前与依赖 class 共用原身份加完整实参的 `ClassApplication`，字段、基类、直接接口及既定接口实现保存在共同 `ClassDefinition` 中；依赖定义只在原声明形参域内解码一次。类型身份、父类型、字段查询和具体化消费同一 application，String／Array／MutableArray 与 object backing 的既定表示及原身份保持。
- 源码属性和构造初始化元数据按共同字段位置关联，删除其重复字段类型；后期确定的委托存储也加入同一字段表。初始化读取外来泛型基类字段时取得实际声明宿主及完整实参，保留原字段身份。普通外部虚方法与实现继续关联提供方，具体化不重新选择继承和 dispatch。
- 真实组合揭示继承默认值在交换泛型参数后丢失发布映射。默认来源现保留原提供方到当前声明域的完整实参，每层继承组合代换；再次发布只更新归属与 binder 映射，实际调用使用同一已加载正文及共同展开器。单元通过完整产物接口投影确认两层覆写分别保留 `[B, A]`、`[X, Y]`，且指向同一原定义；没有改写原正文、伪造局部函数身份或放宽合同校验。
- 新增 `m23-shared-class-definitions` 的两组正例、三个诊断反例与 HIR／MIR／LIR 快照，覆盖递归 class、struct 包装、类型别名、泛型基类初始化、多层交换参数与默认值、接口默认方法、数组、普通委托存储及 singleton。源码移走后再次发布、consumer-local 类型、String／Int／Unit 与普通／移动 GC 运行均通过；反例准确定位 final 基类、错误字段实参与泛型基类字段初始化。
- 全仓 fmt／clippy 无警告；859 项 HIR、1321 项 HIR lowering、114 项 MIR lowering 单元测试全部通过。20 组不同真实产物回归均覆盖通过，包括完整 core MIR／LIR 闭包、class／struct／enum／interface、构造、成员、属性、默认值、数组、继承初始化、抽象 conformance、companion 与 singleton。默认映射修复后，使用最终配套编译器关闭更新开关复验相关八组全部通过；既有快照仅三份 HIR 中的四处 arena 编号改变。日志前缀 `/tmp/scoop-m23-6a-shared-class-types-`，结果见 `unit.log`、`build.log`、`default-mapping.log` 与 `verified-results.json`。
- 属性声明主文件由 1225 行降至 186 行，拆出的 class、访问器规则和函数构造模块分别为 350、399、324 行；新 class 解码为 148 行，继承默认映射为 103 行。确认没有 cargo／rustc 占用后清理 1777.6 MiB 旧增量缓存，继续复用 `target/m23-6a`。本批未改变 wire payload 或 runtime ABI；interface 类型、完整 nominal 条件与成员、候选和正文存储继续按 6a 设计迁移。


## interface 共用 application 与带形参的父类型、递归签名

- 删除 `Type::ImportedInterface`、`ImportedInterfaceType` 及每个实际应用上预替换的方法表。当前和依赖接口共用原身份加完整实参的 `InterfaceApplication`，形参与父接口保存在共同 `InterfaceDefinition`；依赖方法签名只在原声明域内解码一次。继承签名先组合到当前声明域，查询和具体化再代入本次实参，递归参数和结果复用同一 application。
- 父类型、约束、属性、conformance、默认来源及具体化读取共同定义；loaded 方法保留真实定义位置和原槽身份。修复本地泛型接口覆写外来槽时按未替换宿主比较造成的漏消除：既定覆写关系按原槽声明身份应用，与具体化槽表保持一致，没有重新选择 override、复制普通外部实现或扩大物化根。
- 新增 `m23-shared-interface-definitions` 两组正例、四个诊断反例及六份阶段快照，覆盖递归签名、交换／恢复参数的多层继承、菱形、默认参数、class／struct 实现、bound、装箱、Array／MutableArray、别名和引用／Int／Unit／复合值。源码移走后再次发布和普通／移动 GC 运行通过，关闭全部更新开关复验通过；单元同时验证 loaded 定义唯一、递归 self application 和继承签名仍在原 binder 域。
- 全仓 fmt／clippy 无警告；859 项 HIR、1322 项 HIR lowering、114 项 MIR lowering 单元测试全部通过。17 组不同真实产物回归全部覆盖成功，包括完整 core 三组 MIR／LIR 导出闭包、四类名义定义、成员、构造、默认值、属性、数组、抽象实现、本地接口继承与限定 super。仅一份旧 HIR 快照的两处函数类型 arena 编号变化，核对更新后完整组严格复验通过。日志前缀 `/tmp/scoop-m23-6a-shared-interface-types-`，结果见 `unit.log`、`build.log` 和 `verified-artifacts.json`。
- 接口解码、具体化主流程、继承查询和声明模块分别为 185、158、89、154 行。确认没有 cargo／rustc 占用后清理 740.6 MiB 旧增量缓存，继续复用 `target/m23-6a`。本批未改变 wire payload 或 runtime ABI；完整名义条件、成员候选和正文存储继续按 6a 设计迁移。

## 共同名义定义保留 NoGC 与 pointee 条件

- struct／enum／class／interface 的共同定义保留原声明的 GC-free pointee 条件，struct 的完整属性与 enum 的 NoGC 标记不再依赖来源记录。读入时在原 binder 域中解码一次，前端按实际类型使用检查；仅在泛型代换后形成的应用复用具体化已经计算的 GC 属性，错误沿原请求队列定位实际调用或类型使用，没有重算布局或增加通用条件框架。
- 名义声明 wire 新增必需的 NoGC 与原 binder 条件记录，HIR `cross-cone-interface` 从 `/42` 升至 `/43`，同步三份 spec、required profile、固定向量及旧版本拒绝测试。条件往返保留原 binder，并拒绝错误深度、越界、重复、非 binder 及缺失字段；runtime／MIR／LIR ABI 未改变。
- 新增 `m23-shared-nominal-conditions` 两组正例、16 个反例、六份阶段快照与 16 份诊断快照，覆盖默认值、局部与泛型正文、别名、嵌套包装、class／enum 根、Phantom、指针、NoGC 与 consumer-local 类型。源码移走后的再次发布和普通／移动 GC 运行通过，所有新快照关闭更新开关复验通过。
- 真实 CLayout 指针组合揭示 producer 与 reader 对每个字段强求独立布局的错误。两端改为按既有 C ABI 合同仅收集实际嵌套 C struct 的布局；标量与指针沿原字段布局处理，保留原有边界核对。修复后新增组合及四组既有指针／构造回归均严格通过。
- 全仓 fmt／clippy 无警告；862 项 HIR、1324 项 HIR lowering、114 项 MIR lowering、584 项 slib、466 项 LIR 与 142 项 LIR lowering 单元测试均已覆盖通过。34 组不同真实产物回归全部关闭更新开关成功，其中包含完整 core 的 22 组；43 份既有快照经核对只修改 `artifact=` 摘要，机器代码、runtime 和布局输出均保持。最终证据前缀 `/tmp/scoop-m23-6a-shared-nominal-conditions-`，严格产物汇总为 `verified-artifacts.json`。
- 从原 813 行的具体化表达式模块拆出 116 行的来源位置处理，主文件降至 700 行；新的具体化条件、前端 NoGC、wire 条件模块分别为 76、119、126 行。确认没有 cargo／rustc 占用后清理约 2220.4 MiB 旧增量缓存，继续复用 `target/m23-6a`。共同可调用签名、完整候选与正文存储继续按 6a 设计迁移。

## 共同可调用签名与挂起上下文

- 当前函数和依赖模板直接保存同一 `CallableSignature`，保留名称、完整参数／结果、调用修饰、执行方式、属性和原位置；实现继续共用 `FunctionKind`。删除来源记录上的重复签名字段及具体化中的逐字段镜像，具体化直接借用原签名与实现。依赖效果只在解码时转换一次，泛型、接收者和词法归属保留各自原身份，wire payload 与 runtime ABI 未改变。
- 新反例复现普通函数错误接受依赖泛型 `suspend` 调用。提交处按已选声明复用既有挂起上下文检查，普通调用、成员调用及默认参数均准确定位实际调用；该检查不参与候选筛选，不改变定义处的重载选择。原源码调用继续使用同一诊断入口。
- 新增 `m23-shared-callable-signatures` 两组正例、五个反例、六份阶段快照及五份诊断快照，覆盖当前／依赖签名、NoGC／Unsafe／suspend、接口成员、泛型方法、默认闭包、局部泛型与捕获。再次发布后以 consumer-local class、String、Int 和 Unit 实例化，普通／移动 GC 运行通过；新增快照全部关闭更新开关复验成功。
- 全仓 fmt／clippy 无警告，862 项 HIR、1326 项 HIR lowering、114 项 MIR lowering 单元测试全部通过。13 组不同真实产物回归全部严格成功，包括完整 core 三组 MIR／LIR 闭包、抽象与泛型成员、扩展属性、受保护成员、初始化、函数引用、默认值、共同请求和名义条件；既有快照无需修改。最终日志前缀 `/tmp/scoop-m23-6a-shared-callable-signatures-`，修复前证据为 `effects-before.log`，严格产物汇总为 `verified-artifacts.json`。
- 函数声明、读入模板根、具体化声明视图和新增单元模块分别为 425、231、128、76 行；确认没有 cargo／rustc 占用后清理约 911.3 MiB 旧增量缓存，继续复用 `target/m23-6a`。完整候选、共同调用节点与正文存储继续按 6a 设计迁移，本批不代表里程碑完成。

## LocalConcrete HIR 共用直接调用节点

- 删除 LocalConcrete HIR 的 `ImportedGenericCall` 和 `ImportedDependencyCall`，直接调用共用 `Call` 与已有 `CallableTarget`；目标仍分别引用本次物化函数或普通外部定义。具体化不再按模板的来源根选择不同调用节点，MIR 从同一个入口按实际目标生成调用，普通方法与限定 super 继续保留各自派发语义。
- 可选名称绑定保留在原调用点；依赖调用记录按原泛型声明和完整 application 查询，当前模板不会因为也有 application 而变成依赖使用。共同遍历、静态 receiver 收集和原调用位置检查同步迁移，保留缺失目标／闭包、重复物化根、错误定义与求值位置的原负例。
- 新增 `m23-shared-concrete-calls` 两组正例和六份阶段快照，覆盖别名复用、当前／依赖泛型、默认值、嵌套调用、闭包、原提供方状态及 consumer-local class／String／Int／Unit。源码移走后再次发布和普通／移动 GC 运行通过，六份快照关闭更新开关复验成功。
- 全仓 fmt／clippy 无警告；862 项 HIR、1327 项 HIR lowering、114 项 MIR lowering 单元测试均已覆盖成功。既有 receiver 测试按实际目标区分本次物化调用与外部定义，修正其旧节点选择器后单独复验通过，外部 receiver／参数断言保持。14 组不同真实产物回归全部严格通过，包含完整 core 三组 MIR／LIR 闭包、签名与挂起约束、成员、访问器、初始化、引用、默认值和名义条件；既有快照无需修改。
- 最终日志前缀 `/tmp/scoop-m23-6a-shared-concrete-calls-`，结果见 `unit.log`、`receiver-verified.log`、`mir-unit.log` 与 `verified-artifacts.json`。具体化表达式模块降至 692 行，依赖调用查询和新单元模块分别为 286、83 行；确认没有 cargo／rustc 占用后清理约 879.1 MiB 旧增量缓存，继续复用 `target/m23-6a`。wire 与 runtime ABI 未改变，Export HIR 调用、完整候选和正文存储继续迁移。


## Export HIR 共用直接调用与原使用点绑定

- 删除 Export HIR 的 `ImportedDependencyCall`；当前 callable、普通外部定义及非成员模板 application 共用 `Call` 与已有 typed target。默认值替换、GC／pointee 条件、引用收集、委托访问器及具体化消费共同目标，原名称绑定和静态 receiver 随替换保留。成员和限定 super 仍保留既定派发，本批未把剩余成员操作或全部候选迁移记为完成。
- 新增 `m23-shared-source-calls` 两组正例、三个反例与九份快照，覆盖别名、当前／依赖泛型、默认值、扩展、局部函数、closure 和具副作用的逆序命名实参。真实源码移走、再次发布及 consumer-local class／String／Int／Unit 在普通／移动 GC 运行通过；反例保留 managed 调用与泛型 GC 条件的准确源码诊断。
- 全仓 fmt／clippy 无警告，862 项 HIR、1329 项 HIR lowering 和 114 项 MIR lowering 单元全部通过。74 组泛型真实产物测试关闭所有快照更新开关复验通过；完整 core 三组 MIR／LIR 导出闭包亦通过。378 份既有快照中，375 份只修改调用标签；另外三份同步两处临时 TypeId 与静态 receiver 类型先处理带来的 class／layout 枚举顺序，原身份、签名、布局内容与机器指令保持。
- 扩展回归中的包可见性反例暴露 reader 的缺失 exact type 错误；上一批冻结编译器复现同一错误，证据为 `baseline-visibility.log`。后续通过原身份模块确认缺失类型并非 Unit，该回归继续单独定位，没有删掉反例或放宽产物校验。其余最终证据前缀 `/tmp/scoop-m23-6a-shared-source-calls-`，结果见 `unit.log`、`verified-artifacts.json` 与 `snapshot-audit.json`。
- 按实际职责拆出 96 行的共同调用效果检查和 48 行的调用名称输出；原效果表达式降至 508 行，默认引用收集为 455 行。wire payload 与 runtime ABI 保持。确认没有 cargo／rustc 占用后清理约 1697.0 MiB 旧增量缓存，继续复用 `target/m23-6a`。

## 按实际初始化入口读取 Unit 返回类型

- MIR reader 先从实际 Strong generated callable 收集初始化单元，只有非空需求才读取并核对 canonical Unit 返回类型。删除调用方无条件查找 Unit 的前置步骤；全部实际 initializer／ensure 仍须匹配原 source key、provider、成对角色和完整签名，同一 Unit 身份只检查一次。未物化声明不成为类型或机器根，wire 与 runtime ABI 保持。
- 新增 `m23-demanded-initialization` 两组正例和六份阶段快照，覆盖只有 Int 返回值的普通库、泛型调用与实际 object 初始化组合。再次发布后以 consumer-local class、Int 和 Unit 实例化，普通／移动 GC 运行通过；全部新快照关闭更新开关复验成功。
- 全仓 fmt／clippy 无警告，341 项 MIR 与 584 项 slib 单元测试全部通过。8 组既有真实产物回归严格通过，包含完整 core 三组 MIR／LIR 闭包、初始化来源和 ABI、object、构造、泛型 dispatch 及共同调用；实际初始化缺来源、错 provider、缺角色和签名反例保持。包可见性回归仍报另一个 exact type 缺失，不计入本批通过项。
- 最终日志前缀 `/tmp/scoop-m23-6a-demanded-initialization-`，结果见 `unit.log`、`new-verified-results.json` 和 `verified-results.json`。初始化重放模块为 169 行，继续复用 `target/m23-6a`。

## 共有继承图按槽所在产物读取类型

- 定位并修复包可见性扩展回归：共有继承图重放全部 provider 的槽签名时，错误地使用依赖集合末尾产物的身份表。现按槽实际所在产物读取 receiver、完整成员代换与签名，不要求无关普通库复制其它 provider 的类型；固定 Unit 的比较直接使用原内建身份。仍只构建一次共有继承图，各产物原有引用、槽、签名与依赖检查保持。
- 在 `m23-qualified-types` 补充 `independent` 正例，将菱形重导出、嵌套类型、泛型、值类型与独立普通函数库组合。三份新阶段快照、普通／移动 GC 运行以及三个既有包可见性反例全部关闭更新开关复验成功；原 `bad-direct-longest` 现准确报告不可访问的 `Nested`，不再提前误报产物类型缺失。
- 全仓 fmt／clippy 无警告，862 项 HIR 与 584 项 slib 单元测试通过，另有 10 组真实产物回归严格通过，包括完整 core、槽与 callable 反例、类型策略、继承 ABI／初始化、泛型 dispatch、限定类型和初始化需求。临时定位输出已移除，旧快照无需改动，wire 与 runtime ABI 保持。
- 最终日志前缀 `/tmp/scoop-m23-6a-inheritance-scope-`，结果见 `unit.log`、`new-verified-results.json` 与 `verified-results.json`；原失败的定位证据为 `/tmp/scoop-m23-6a-missing-type-diagnostic.log`。槽重放与组合测试模块分别为 107、233 行；旧 `target/debug/incremental` 已为空，继续复用 `target/m23-6a`。

## 共同成员调用与实际接收者适配

- 当前成员、依赖泛型成员、解码正文、默认值与函数引用共用 `MethodCallee` 和 `BoundCallableRef`；上界记录保存实际 receiver 类型、完整 class／interface application、原成员引用、已选目标与完整签名。默认展开共同代换这些字段，删除 `ImportedMethodCall`、`ImportedGenericCall`、`ImportedMethodCallee` 和单独的导入上界记录及替换、效果检查与投影分支。
- 具体化按已有完整 conformance 和原接口槽取得实际实现；普通外部实现继续引用原声明。本地与依赖的调用、引用共用接收者上转型／装箱，外部 callable use 从原签名保存所需 receiver，调用点原静态类型独立保留。修正外部父类实现被本地接口上界选中后的参数 ABI，并让成员与扩展的连接检查共用完整 exact receiver 关系，接受合法泛型 application 的父类型链。
- 新增 `m23-shared-method-calls` 的独立／组合源码、6 份阶段快照和 4 份错误诊断。覆盖 class／interface 上界、完整宿主与方法实参、默认值、限定 super、函数引用、当前接口与外部父类实现，以及通过基类静态类型调用子类覆写。源码移走后再次发布，下游 String／Int／Unit 组合及普通／移动 GC 运行通过。
- 全仓 fmt／clippy、2306 项 HIR／HIR lowering／MIR lowering 单元测试，以及关闭全部快照更新开关的 78 项真实泛型产物与完整 core 回归全部通过。122 份既有阶段快照变化中，97 份 HIR 仅调用标签变化，另 7 份 HIR 展示共同上界／派生目标；11 份 MIR 对应明确上转型或函数编号变化。7 份 LIR 按稳定符号还原调用后，正文、调用目标和元数据相同，仅函数排列与临时编号变化；既有诊断快照保持不变。
- 最终日志前缀 `/tmp/scoop-m23-6a-shared-methods-`，结果见 `unit.log`、`all-verified-results.json`、`hir-review.json` 和 `lir-review.json`。成员具体化模块 206 行，默认实体替换模块 328 行；本批清理未使用的 `target/debug` 约 1475.1 MiB，继续复用 `target/m23-6a`。wire 和 runtime ABI 不变，完整候选与剩余词法／构造节点的共同表示继续迁移。

## 共同函数引用目标与原局部声明

- 删除 `ImportedCallableReferenceTarget`，当前与解码正文的命名、局部、成员、扩展和 intrinsic 引用使用同一目标及替换、投影和具体化过程。局部目标保留原定义路径和已选 typed callable；Concrete 局部声明按实际函数实体复用，重复引用共享实现，各次创建的捕获值仍独立。
- 解码局部声明不再被当作无运行时效果的语句提前删除。共同 `LocalFunction` 记录保存声明、出现处签名、原路径、宿主 binder 数与捕获，正文存储只负责定位；原 parent 从已有 typed declaration owner 读取，实际调用和引用继续保存完整实参。声明本身不触发机器实例，具体化执行正文时才擦除该声明语句。
- 新的再次发布组合复现默认值引用的来源位置错配：调用点产生新的 invoke 身份，外层表达式仍保留默认值定义位置。读取时现从既有生成声明记录恢复 invoke 位置，当前与解码默认展开共用调用点规则；没有增加 wire 字段或放宽 reader。
- 新增 `m23-shared-reference-targets` 两组正例、四个反例、六份阶段快照与四份诊断快照。覆盖局部泛型、完整且未使用的 owner 实参、重复引用、不同捕获值、可变接收者快照、成员／扩展／intrinsic 引用及函数值默认参数；源码移走后再次发布，下游本地 class／String／Int／Unit 和普通／移动 GC 运行通过。单元还核对原定义 span、捕获参数身份与未用局部声明不发射。
- 全仓 fmt／clippy 无警告，2307 项 HIR／HIR lowering／MIR lowering 单元测试通过；79 项真实泛型产物及完整 core 回归已全部关闭更新开关覆盖成功。18 份既有 HIR 快照的 55 处变化仅为目标显示，MIR／LIR／诊断快照不变。最终证据前缀 `/tmp/scoop-m23-6a-shared-references-`，结果见 `unit.log`、`all-verified-results.json`、`snapshot-verified-results.json` 和 `snapshot-review.json`。
- 共同词法记录 192 行，默认值 closure／引用替换 314 行，新增局部声明读取 89 行，引用目标具体化 121 行；清理闲置增量缓存约 1605.4 MiB，继续复用 `target/m23-6a`。wire 与 runtime ABI 保持，剩余词法引用根和完整候选继续按 6a 设计迁移。

## 共同引用节点与递归接收者捕获

- 删除 `ImportedCallableReference` 及其独立表达式、默认替换、效果遍历、投影和具体化分支。当前与解码引用直接进入同一 `CallableReference` arena；存储根只保存当前 typed 根或原 persistent parent，目标、路径、完整宿主实参、签名和捕获使用共同记录。读取时保留原生成声明的位置和 parent，不复制一份生成记录，不以消费者的词法根代替原定义。
- 局部递归调用的隐藏捕获补齐现在同时遍历共同引用实体的接收者及捕获表达式。新真实源码在捕获首次出现之前，以递归调用结果创建成员引用；修复前接收者只传一个参数，而实际函数需要两个，修复后源码和产物两条路径均保留最终捕获。默认展开仍按原规则产生调用点 invoke 身份，模板展开保留原身份与完整代换。
- 扩展 `m23-shared-reference-targets` 的独立／组合正例和六份阶段快照，覆盖递归接收者创建引用，再次发布以及消费方本地类型、String／Int／Unit 和普通／移动 GC 运行。单元检查共同 arena 中两种存储根，以及实际递归目标的参数数量和捕获签名；既有四个反例诊断保持。
- 全仓 fmt／clippy 无警告，2308 项 HIR／HIR lowering／MIR lowering 单元测试通过。79 项真实泛型产物及完整 core 回归全部关闭更新开关覆盖成功；仅一份旧 HIR 快照的两个引用编号发生变化，更新后完整组严格复验通过。最终证据前缀 `/tmp/scoop-m23-6a-reference-roots-`，结果见 `before.log`、`unit.log`、`all-verified-results.json` 与 `snapshot-verified-results.json`。
- 共同词法记录 201 行、默认 closure／引用替换 263 行、引用读取 166 行、引用具体化 84 行、局部调用捕获补齐 336 行。确认无 cargo／rustc 占用后清理约 225.4 MiB 闲置增量缓存，继续复用 `target/m23-6a`。wire 与 runtime ABI 保持，剩余 closure／构造节点和完整候选继续迁移。

## 局部自身引用保留最终捕获

- 真实源码复现局部函数正文中 `::自身函数` 提前取得空捕获列表，而最终目标函数需要隐藏捕获参数。正文完成后的补齐过程现按已选函数身份处理自身引用，以本次函数执行中的捕获绑定构造引用环境；不读取外层声明处局部变量，也不按名称或路径重选目标。
- 扩展 `m23-shared-reference-targets` 的普通递归和泛型递归正例：引用之后首次使用外层绑定、不同类型的多项捕获、同一泛型局部函数的 Int／Unit 外部实例，以及原宿主实参均保留。复用既有捕获 ABI 单元断言，修复前明确失败为 0 对 1；修复后源码与产物路径均通过，六份阶段快照同步更新。
- 全仓 fmt／clippy 无警告，2308 项 HIR／HIR lowering／MIR lowering 单元测试通过；15 项相关真实产物回归关闭更新开关全部通过，覆盖再次发布、普通／移动 GC、默认值、局部函数、closure、bound 与函数引用。既有其它快照和四个引用反例保持。证据前缀 `/tmp/scoop-m23-6a-recursive-references-`，结果见 `before.log`、`focused-verified.log`、`unit.log` 与 `all-verified-results.json`。
- 捕获补齐模块 358 行。本批检查闲置增量目录为空，继续复用 `target/m23-6a`；wire 和 runtime ABI 保持，后续继续统一 closure 正文与完整候选。

## lambda 与匿名函数共用正文和创建点记录

- 删除 `ImportedClosure`、对应表达式及独立的默认替换、效果遍历、投影和具体化分支。源码与解码后的 lambda／匿名函数分别进入同一 arena，并与局部函数共用 `LexicalFunctionDefinition` 存储定位；创建点保留原定义、完整正文实参、函数类型与捕获。解码实参显式保存已恢复的 binder 映射，不再额外创建导入 application。
- 默认值、GC／pointee 条件和捕获来源表达式使用共同遍历，具体化直接进入原定义与完整实参的共同请求队列。当前声明身份、词法 parent 和导出根只登记当前正文；读入闭包继续引用原生成声明，没有复制普通实现或把读入声明伪装成本地函数。
- 新增 `m23-shared-closure-bodies` 两组正例、四个反例、六份阶段快照和四份诊断快照。覆盖多层 lambda／匿名函数、默认闭包重复创建与不同捕获值、完整且未使用的宿主实参、本地／外来类型组合，以及源码移走后的再次发布和普通／移动 GC 运行。单元检查源码对照只使用当前声明、读入默认值复用原正文、两种闭包均保留完整实参。
- 全仓 fmt／clippy 无警告，2309 项 HIR／HIR lowering／MIR lowering 单元测试通过；源码存储断言加强后单独严格复验通过。80 项真实泛型产物及完整 core 回归全部关闭更新开关覆盖成功。六份旧 HIR 快照的 39 行变化仅为共同闭包标签、arena 编号与捕获显示；既有 MIR／LIR／诊断快照保持不变。证据前缀 `/tmp/scoop-m23-6a-shared-closures-`，结果见 `unit.log`、`source-storage-verified.log`、`all-verified-results.json` 与 `snapshot-review.json`。
- 共同词法实体 209 行、默认 closure／引用替换 238 行、closure 具体化 240 行、closure 读取 202 行、词法正文投影 280 行。确认无 cargo／rustc 占用后清理约 1026.7 MiB 旧 `target/debug`，继续复用 `target/m23-6a`。wire 和 runtime ABI 保持；剩余 singleton／构造操作与完整候选继续按 6a 设计迁移。

## 单例读取共用节点与初始化顺序

- 删除 Export HIR 与 LocalConcrete HIR 的 `ImportedSingletonValue`，两者分别以共同 `SingletonValue` 节点保存当前记录或依赖原 object value 身份。默认替换、引用投影、效果与类型遍历消费同一目标；MIR 取得实际 ensure 和已发布根后，共用先初始化、再读取的降低过程。实际依赖根选择保留原提供方，不复制单例状态与存储。
- 新增 `m23-shared-singleton-reads` 两组正例、三个反例、六份阶段快照和三份诊断快照，覆盖当前／依赖 object、默认参数、泛型、lambda／匿名函数、初始化一次与对象身份。源码移走后再次发布，下游本地 class／String／Int／Unit 在普通和 moving GC 下运行通过；以实际对象分配触发压力收集，运行夹具核对 GC epoch。单元使用同一源码检查共同节点及未复制外部存储。
- 全仓 fmt／clippy 无警告，2894 项 HIR／HIR lowering／MIR lowering／slib 单元已覆盖成功。81 项真实泛型产物及完整 core 回归全部关闭更新开关覆盖通过。31 份既有 HIR 快照的 82 行变化仅为单例节点标签，MIR／LIR／诊断快照不变。证据前缀 `/tmp/scoop-m23-6a-shared-singletons-`，结果见 `unit.log`、`source-verified.log`、`remaining-unit.log`、`all-verified-results.json`、`snapshot-verified-results.json` 和 `snapshot-review.json`。
- 新单元模块 60 行，MIR 删除重复初始化／读根分支；wire 与 runtime ABI 保持。确认无 cargo／rustc 占用后清理约 1454.4 MiB 闲置 `target/debug`，继续复用阶段构建缓存。构造 application／正文和完整候选继续按 6a 设计迁移。


## 共同构造 application 与初始化节点

- 删除导入专用的 constructor application arena、ID 和 `ImportedConstructorInit`。class／struct 分别共用完整 application 与 `ClassInit`／`StructInit`；目标保留已选定义的存储定位和完整 nominal owner。默认值、委托、效果条件、引用投影与具体化读取相同目标，普通外部 initializer 继续引用原 provider。
- `@NoGC` 构造检查按主／次构造的实际角色执行，修复依赖泛型的 GC-free struct 主构造被当成 managed 次构造而拒绝的问题。遵循既有语言规则，当前与依赖的 managed 次构造仍报准确诊断；没有改变 wire 或 runtime ABI。
- 新增 `m23-shared-constructor-applications` 两组正例、四个反例、六份阶段快照和四份诊断快照，覆盖完整且未使用的 owner 实参、重复 application、主／次构造、默认值、本地继承依赖构造及闭包。单元核对共同 application 的去重与原定义；源码移走后的再次发布、本地 class／String／Int／Unit 以及普通／moving GC 运行通过。
- 全仓 fmt／clippy 无警告，2895 项 HIR／HIR lowering／MIR lowering／slib 单元已覆盖通过。82 项真实泛型产物及完整 core 回归全部关闭更新开关覆盖成功。161 份旧 HIR 快照的 341 行变化逐行核对为共同构造标签及完整 class owner 的显示；既有 MIR／LIR／诊断快照不变。证据前缀 `/tmp/scoop-m23-6a-constructor-nodes-`，结果见 `unit.log`、四份 `dump-*-verified.log`、`remaining-unit.log`、`all-verified-results.json`、`snapshot-verified-results.json` 与 `snapshot-review.json`。
- application 声明模块 44 行，新增单元模块 76 行；继续复用 `target/m23-6a`。构造正文与完整候选尚在迁移，本批不代表 6a 完成。


## 共用构造正文与初始化片段

- 当前与解码构造器共用 class／struct 的主／次构造正文、已选 `this`／base 委托、实参计划和公共初始化步骤。主参数存储与普通／委托字段初始化直接保存完整 owner 和原字段身份；属性记录继续保存委托存储关系。删除 `ImportedConstructorKind`、提前把导入公共初始化展开为另一种正文的路径、独立导入构造正文 lowering，以及源码专用字段偏移计算。
- 构造正文通过轻量存储视图进入同一参数、字段、公共初始化和委托降低过程；各初始化片段的局部值环境保持独立，原定义位置和执行上下文保留。wire 字节结构和 runtime ABI 不变。
- 新组合复现初始化中的匿名函数／局部函数 `return` 被误当作退出构造器。按既有函数边界规则修复：初始化体自身仍拒绝返回，嵌套函数返回自身；lambda 裸返回和捕获未完成 `this` 仍拒绝。新增 `m23-shared-constructor-bodies` 两组正例、五个反例、六份阶段快照及五份诊断快照，检查 struct 委托、class 主／次构造、多个公共片段、委托字段、局部函数和多层闭包的顺序与捕获。
- 全仓 fmt／clippy 无警告，2895 项 HIR／HIR lowering／MIR lowering／slib 单元测试通过。83 项真实泛型产物与完整 core 回归全部关闭更新开关覆盖成功，包括新增源码移走、再次发布、消费方本地 class／String／Int／Unit 及普通／moving GC 运行。16 份旧 MIR／LIR 快照的 90 行变化仅是片段内局部变量显示名称；寄存器、机器指令、原身份以及旧 HIR／诊断快照保持。
- 证据前缀 `/tmp/scoop-m23-6a-shared-constructor-bodies-`，失败复现为 `before.log`，最终结果见 `unit.log`、`generate-results.json`、`all-verified-results.json`、`snapshot-verified-results.json` 和 `snapshot-review.json`。共同 class 构造降低、存储视图和解码模块分别为 293、109、352 行；确认缓存闲置后清理约 2370.2 MiB `target/debug`，继续复用 `target/m23-6a`。完整请求和候选决议继续迁移，尚未完成 6a。


## 构造请求与公共初始化共用原词法身份

- class／struct 构造请求按原构造声明和完整 concrete owner 去重；源码 class 构造和生成零参数适配器保留不同 typed 身份。当前／解码位置仅用于读取正文，不进入请求 key。构造发射与词法 parent 查询共用 application 身份计算，查询从原名义声明取得归属，不要求加载或发射未使用的父构造器正文。
- 公共初始化实际捕获的局部值与默认值共用词法值记录；初始化在不同构造器中的运行时存储使用独立 `Initializer` 路径。原定义位置、选择器与完整宿主实参保持，求值上下文绑定实际执行构造器。已读入公共片段的绑定按原定义关联，修复再次发布后模板加载顺序改变导致捕获绑定缺失的问题，不增加构造发射需求。
- 删除 lambda、匿名函数和函数引用按原节点／类型实参缓存整条创建记录的路径。每个实际创建点使用自己的局部映射和捕获表达式；原函数正文、invoke 和环境类型继续按同一身份复用。真实双构造器用例在修复前由 MIR 明确拒绝错误的捕获字段类型，修复后完整产物闭环通过。
- 新增 `m23-shared-constructor-requests` 四组正例、一个反例、12 份阶段快照及一份诊断快照。覆盖只调用后置构造器、同时调用两个终止构造器、条件委托、公共初始化中的局部函数／引用／匿名函数／lambda、完整且未使用的 owner 参数、重复与别名请求、本地继承依赖以及声明顺序变化。两项单元核对构造去重、词法父构造器不发射和实际捕获 ABI；源码移走后的再次发布、本地 class／String／Int／Unit 及普通／moving GC 运行通过。
- 全仓 fmt／clippy 无警告，2897 项 HIR／HIR lowering／MIR lowering／slib 单元测试全部通过；84 项真实泛型产物与完整 core 回归全部关闭快照更新开关通过。既有 HIR／MIR／LIR／诊断快照均保持不变，wire payload 与 runtime ABI 不变。证据前缀 `/tmp/scoop-m23-6a-constructor-requests-`，最终结果见 `unit.log`、`generate-results.json`、`all-verified-results.json`；失败复现保存在 `before.log`、`fragment-selectors-before.log`、`capture-creation-before.log`、`evaluation-context-before.log` 和 `binding-order-before.log`。
- 公共初始化降低、词法值查询、构造身份和 closure 具体化模块分别为 86、188、159、223 行，构造正文主模块 254 行。确认无文件占用后清理闲置 `target/debug` 与 `compiler/target` 共约 2706.8 MiB，继续复用 `target/m23-6a`。局部调用节点与完整候选决议仍需继续统一，6a 尚未完成。


## 局部函数直调共用完整调用参数

- 删除语义 HIR 与 LocalConcrete HIR 的 `LocalFunctionCall`，源码和解码局部直调都使用共同 `Call`，先传隐藏捕获、再传源码形参。默认值替换、效果检查、可执行表达式遍历、具体化和 MIR 共用已有直接调用处理，不再重复维护局部函数直调分支。
- 递归补齐根据原已选函数身份和完整形参数量更新捕获，保留引用及其接收者中的递归调用。默认值克隆只替换实际目标与值，不再维护局部直调描述符的额外映射。只有函数引用需要 concrete 局部函数值记录；直接调用保留真实函数、完整未使用的宿主参数与捕获身份，不产生额外函数值实体。
- 导出按原局部声明的捕获数量投影既有 wire 局部调用格式，读入恢复共同 `Call`；普通默认值引用集合仍保留原局部声明和实际 callee。wire payload 与 runtime ABI 保持。
- 新增 `m23-shared-local-calls` 两组正例、三个反例、六份阶段快照及三份诊断快照。覆盖递归后首次发现捕获、泛型局部函数、自身引用、局部默认参数与前置形参、命名实参源码求值顺序、空及 spread vararg；源码移走后的再次发布、本地 class／String／Int／Unit 及普通／moving GC 运行通过。单元核对实际直调的捕获数量和参数类型，既有词法值身份与未使用宿主实参断言改为检查真实被调用函数。
- 全仓 fmt／clippy 无警告，2898 项 HIR／HIR lowering／MIR lowering／slib 单元测试全部通过；85 项真实泛型产物与完整 core 回归均已关闭快照更新开关通过。18 份旧 HIR 快照的 43 行只改共同调用标签，两份 MIR 快照的 12 行仅调整删除多余函数类型记录后的显示编号；LIR、原函数身份、调用参数和既有诊断快照不变。证据前缀 `/tmp/scoop-m23-6a-local-calls-`，最终结果见 `unit.log`、`generate-results.json`、`all-verified-results.json`、`snapshot-verified-results.json` 和 `snapshot-review.json`；初轮快照差异保存在 `all-before-snapshot-results.json`。
- 调用投影模块 181 行，递归捕获补齐 367 行，新增单元模块 60 行，其余语义消费者均删除重复分支。确认无文件占用后清理约 2347.2 MiB 闲置 `target/debug`，继续复用 `target/m23-6a`。完整候选、参数物化与 probe／commit 调度仍需统一，6a 尚未完成。

## 共同参数物化与依赖值的 invoke 查询

- 当前函数、成员、构造、variant 与依赖候选的 winner 共用参数物化：接收者先保存，各显式实参及 setup 按源码顺序执行，再按形参顺序展开实际默认值、空／整数组／元素与 spread vararg。默认值取得此前已物化参数；已经降低的内部调用复用原求值结果。源码 recipe 与已读取默认正文只适配真实存储，删除两套重复求值循环，原 typed 目标和 intrinsic place 出口保持。
- 新组合复现依赖类型的局部值被 `operator invoke` 预查询漏掉。预查询现沿已有可见成员与扩展候选读取真实角色，再交普通决议完成访问域、infix 和适用性检查；普通同名方法仍被拒绝。修复前诊断保存在 `invoke-before.log`，没有新增另一套 invoke 选择算法。
- 新增 `m23-shared-argument-materialization` 两组正例、四个反例、六份阶段快照与四份诊断快照。覆盖接收者副作用、逆序命名实参、前置参数默认值、成员／扩展 invoke、泛型构造与 variant、vararg 组合，以及随循环条件重复执行的默认值 setup。源码移走后再次发布，下游本地 class／String／Int／Unit 与普通／moving GC 运行通过。
- 全仓 fmt／clippy 无警告，2898 项 HIR／HIR lowering／MIR lowering／slib 单元测试全部通过；86 项真实泛型产物与完整 core 回归全部关闭快照更新开关通过。既有 HIR／MIR／LIR／诊断快照均未变化。证据前缀 `/tmp/scoop-m23-6a-argument-materialization-`，结果见 `unit.log`、`generate-results.json`、`all-verified-results.json` 和 `invoke-before.log`。
- 参数协议、声明适配、共同求值模块分别为 103、117、119 行，依赖提交模块降为 373 行；wire payload 与 runtime ABI 保持。确认无文件占用后清理约 1483.8 MiB 闲置 `target/debug`，继续复用 `target/m23-6a`。完整候选和 probe／commit 调度仍需统一，6a 尚未完成。

## 共同候选类型探测与失败事务复用

- 当前 callable、nominal、普通依赖及泛型依赖共用完整约束输入、上下文固定点、声明实参代换与值适配；`infer_contextual_arguments` 仅由共同探测器调用。普通函数显式参数绑定 callable 组，构造器与 variant 绑定 owner 组；依赖构造不再伪装成函数参数组，期望 nominal application、声明上界与完整结果检查走相同约束过程。已由求解器验证的普通实参不再逐个重做子类型检查，intrinsic 的额外实际签名要求保留。
- 当前调用的形态失败也直接返回原事务，命名调用不再为了诊断重新运行 singleton resolver。实际表达式探测记录诊断是否依赖期望类型，诊断只复用结果；未知变量继续给出聚焦错误，enum 上下文和重载相关错误保留候选信息。两项既有诊断单元在迁移中发现的回归已修复，最初证据保存在 `unit-before-diagnostics.log`。
- 新增 `m23-shared-call-probes` 两组正例、四个反例、六份阶段快照与四份诊断快照，覆盖无实参构造的期望类型、完整宿主／方法参数、`_`、延迟 lambda、函数值与匿名函数、失败重载事务、generic variant 和 kind bound。源码移走后再次发布，下游本地 class／String／Int／Unit 与普通／moving GC 运行通过。
- 全仓 fmt／clippy 无警告，2898 项 HIR／HIR lowering／MIR lowering／slib 单元测试全部通过；87 项真实泛型产物与完整 core 回归全部关闭快照更新开关通过。既有 HIR／MIR／LIR／诊断快照均未变化。最终证据前缀 `/tmp/scoop-m23-6a-call-probes-`，结果见 `unit.log`、`generate-results.json` 和 `all-verified-results.json`。
- 共同探测器 115 行，声明约束模块 225 行；函数引用签名检查与依赖诊断分别拆为 130、144 行，依赖泛型探测模块降至 294 行。确认全部验证结束且无文件占用后清理约 1484.0 MiB 闲置 `target/debug`。wire payload 与 runtime ABI 不变；完整声明参数视图、引用适用性与最终内存／wire 等价及 workspace／runtime 完成门仍需落实。

## 完整声明参数视图与共同函数引用适用性

- 当前 callable、nominal 与已读取依赖共用完整声明视图，保存有序 owner／callable binder、参数名、完整参数及结果类型、Required／Default／Vararg 协议和原默认来源。实参映射、共同推断与声明 MSC 直接消费这一视图；删除依赖分支按源码位置重复保存的 wire 参数类型及另一套 variant 参数映射。getter／setter 从真实访问器签名取得隐式 Required 参数，普通函数继续使用完整源码参数表。
- 函数引用共用完整宿主绑定、期望签名、效果限制和约束检查，声明 MSC 使用共同比较过程，保留绑定／非绑定 receiver 与原函数目标。引用不使用调用点的默认参数或 vararg tie-break；普通调用的整数 literal 偏好统一在 MSC 后作逐实参 Pareto 比较。默认来源、具体目标和 dispatch 均保持原身份，没有新增语义验证边界。
- 新增 `m23-shared-declaration-views` 两组正例、六个反例、六份阶段快照与六份诊断快照，覆盖 generic／closed 重载、默认值与 vararg 的完整函数引用签名、泛型扩展、绑定／非绑定引用、宿主与方法参数、命名／spread／空 vararg，以及引用创建不执行默认表达式。源码移走后再次发布，下游本地 class／String／Int／Unit 与普通／moving GC 运行通过；反例分别锁定默认参数 arity、vararg 数组签名和缺少泛型引用期望类型，均只有对应的一条诊断。
- 全仓 fmt／clippy 无警告，2898 项 HIR／HIR lowering／MIR lowering／slib 单元测试全部通过；88 项真实泛型产物与完整 core 回归全部关闭快照更新开关通过。最后精简反例后再次严格复验新增组通过，既有 HIR／MIR／LIR／诊断快照均未变化。初轮访问器迁移失败已修复，相关单元、扩展属性与委托产物回归均通过；证据前缀 `/tmp/scoop-m23-6a-declaration-views-`，最终结果见 `unit.log`、`all-verified-results.json` 和 `fixture-verified-results.json`，初轮证据为 `unit-before-accessors.log`。
- 完整参数视图为 42 行，依赖存储适配约 160 行，函数引用共同检查约 200 行；普通重载主模块降至 414 行，引用依赖适配降至 378 行。确认所有验证完成且无文件占用后清理约 1676.5 MiB 闲置 `target/debug`，继续复用 `target/m23-6a`。wire payload 与 runtime ABI 不变；最终内存／wire 消费等价性、声明位置变化审计及 workspace／runtime 完成门继续执行，6a 尚未完成。

## 同一导出图的内存／wire 消费等价

- 新增正式消费对照测试，复用已有 core、provider 投影、身份注册、wire 编解码、HIR 与 MIR lowering；同一 provider 导出图在内存中直接导入，或经正式 foundation／interface 编解码恢复后导入。两次消费使用同一原 Cone 与实体身份、相同的 consumer 源码和依赖，不归一化身份，也不比较未导出的 provider 私有实现。
- 四组真实 fixture 覆盖泛型正文与重载、两个构造器的公共初始化及捕获、Option 和指针默认值。逐项精确比较原导出 foundation、声明／正文接口、消费方 HIR dump、具体 MIR dump、重新发布的 foundation／interface 字节及 MIR foundation 字节，确认 wire 往返不改变选择、词法捕获或具体化身份。测试没有新增生产工厂、平行管线或重复语义验证。
- 全仓 fmt／clippy 无警告，对照测试四组全部通过；证据为 `/tmp/scoop-m23-6a-wire-equivalence-verified.log`。本批只增加验证，生产行为与旧快照保持。声明位置审计及启用配套编译器的完整 workspace／runtime 验收继续执行。

## 隐式 receiver 属性查询不再假定本地 class

- 声明位置审计用实际源码复现依赖 class 的扩展函数在 `item.read()` 上访问本地 class arena 而 panic。隐式 receiver 的无诊断预查询现复用完整 nominal application 的属性与可见性查询，再查询实际依赖成员；删除按裸 class ID 遍历继承和 struct／enum 的重复属性判断。实际访问继续使用已有 getter 与成员决议，不复制依赖声明或增加候选副作用。
- 新增 `m23-shared-host-properties` 两组正例、两个反例、六份阶段快照和两份诊断快照，覆盖本地／依赖宿主、继承泛型属性、本地派生 class、函数值属性和属性接收者上的函数引用。私有属性在两个声明位置都准确拒绝；源码移走后再次发布，下游本地 class／String／Int／Unit 与普通／moving GC 运行通过。
- 全仓 fmt／clippy 无警告，2900 项 HIR／HIR lowering／MIR lowering／slib 单元通过；新增组及六组既有 bound、接口、扩展属性、函数引用产物回归全部关闭更新开关通过。既有快照没有变化。证据前缀 `/tmp/scoop-m23-6a-host-properties-`，原 panic 为 `before-verified.log`，最终结果见 `unit.log`、`fixture-verified-results.json` 和 `related-verified-results.json`。
- 单独抽查 core bootstrap 测试发现旧固定数量断言期望 7 个 ODR member、实际为 91；本批修复前的冻结编译器也同样失败，证据为 `bootstrap-baseline.log`。该基线断言与实际成员的对应关系将在完整 workspace 验收中核对，不能据此宣称全仓已通过。成员决议模块由 238 行降至 188 行，新增单元仅 11 行；wire 与 runtime ABI 不变。

## 函数值与此前实参先于后续前置语句求值

- 真实运行复现函数值调用的后续默认实参越过 callee 和前一实参求值。共同函数值入口先收集每个实参的完整表达式及前置语句，仅在存在后续前置语句时保存此前结果；函数值自身在第一个实参前保存。直接 `f(...)` 与显式 `f.invoke(...)` 共用该处理，异常继续终止后续执行，函数值的参数形态、类型与挂起规则保持。
- 新增 `m23-shared-callable-order` 两组正例和六份阶段快照，覆盖 callee 与实参副作用、嵌套默认值、异常、实参期间重绑定函数变量以及后续实参修改此前实参读取的变量。源码移走后再次发布，下游本地 class／String／Int／Unit 与普通／moving GC 均通过；修复前冻结编译器实际运行返回用例错误，证据为 `/tmp/scoop-m23-6a-callable-order-before-generate-function_values_preserve_callee_and_argument_evaluation_order.log`。
- 全仓 fmt／clippy 无警告，2900 项 HIR／HIR lowering／MIR lowering／slib 单元通过；90 项真实泛型产物与完整 core 回归全部使用同一最终配套编译器、关闭更新开关覆盖通过。29 组受到快照变化影响的场景全部严格复验成功，161 份旧快照的类型、函数与 callback 声明头逐项保持一致；HIR／MIR 中增加普通临时保存，LIR 与局部编号相应变化，既有诊断快照不变。证据前缀 `/tmp/scoop-m23-6a-callable-order-`，结果见 `unit.log`、`all-verified-results.json`、`snapshot-verified-results.json` 与 `snapshot-review.json`。
- 函数值调用拆为 123 行的独立模块，调用分类主模块降至 252 行。所有验证完成后清理约 1725.8 MiB 闲置 `target/debug`，前一宿主属性批次另清理 1725.0 MiB，继续复用热缓存。wire payload 结构与 runtime ABI 保持；完整 workspace／runtime 验收和已确认的 core bootstrap 旧断言仍待完成。

## core bootstrap 按真实封闭应用验证

- 核对实际 core 源码和 LIR 确认旧断言已过时：四个整数范围族各自实现完整 `Iterable<T>`／`Iterator<T>`，其迭代结果使用对应 `Option<T>`，再加既有 `Option<String>`，共 13 个真实泛型应用。原测试只允许一个 application、固定七个 ODR member，并试图按旧诊断名称排除 iterator，不能验证已经实现的共同父类型与迭代能力。
- 测试现在按实际 exact ID 关联 MIR 与 LIR，对全部预期应用核对原 specialization group、HIR 归属、OdrWeak linkage 和匹配布局，保留 HIR foundation 的角色边界检查。删除过时数量和 iterator 排除断言，没有只把常量七改成九十一。检查独立放入 `bootstrap/application_shapes.rs`，主 bootstrap 模块同步缩短；生产代码不变。
- 全仓 fmt／clippy 无警告，最终配套编译器上的 core bootstrap 检查通过，证据为 `/tmp/scoop-m23-6a-workspace-bootstrap-verified.log`。完整 `cargo test --workspace --no-fail-fast` 已启动，快照更新开关全部关闭，继续完成 runtime 与其余 workspace 检查。

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

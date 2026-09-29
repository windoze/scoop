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

# M36 设计：编译器输入与 IR 的类型完备性

状态：已完成（2026-10-11）。12 项功能均已实现并分别提交，跨平台验收见 [ACCEPTANCE.md](ACCEPTANCE.md)，逐项实现与实际验证见 [PROGRESS.md](PROGRESS.md)。下文审查结论描述实施前状态。

日期：2026-10-10。

前置：[M35](../milestone35/DESIGN.md) 的正式验收。后续：[M37 语言语法与类型能力补齐](../milestone37/DESIGN.md)。原 M36 的语言功能设计已顺延到 M37，其功能范围与已确定决策保留。

设计稿最初仅调整 M36/M37 目录；本里程碑已按本文范围同步规范、编译器、fixture 与路线图，M37 语言功能不进入本里程碑。

## 0. 目标与范围

M36 收紧当前编译器实际使用的输入和 IR 数据结构，使必要内容不能缺失、互斥形态不能混搭、已确定的实体种类不会在传递中丢失。优先消除依赖“调用方保证输入满足前提”的 `panic!`、`unreachable!`、`assert!`、`expect`，并保持现有语言行为、求值顺序、实体身份、ABI、异常和 GC 契约。

依据为[语言规范](../specs/SCOOP-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.2～2.5、2.11～2.12，以及[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)。HIR 结束后应交付完整语义信息；后续阶段通过 IR/meta 通信，并复用已验证且未变化的数据。

本里程碑遵守以下边界：

- 按实际操作、实体和消费者所需的区别，使用封闭枚举、必填字段、具名参数与已有专用 ID。抽象只服务当前调用路径，不把整个 IR 泛型化或建立通用 typestate 框架。
- 重构应实际减少可表示的错误状态。仅把原有断言移入新的 `Checked<T>` 构造器、给相同数据增加包装层，不能单独视为完成。
- 外部字节的格式、引用闭合、实际类型关系、布局、ABI、内存边界与 GC 契约仍在相应边界检查；成功后传递完整结果。不得在 IR/meta crate 重新实现上游语言语义，或在每个 stage 重放相同验证。
- 不新增语言功能、来源授权、防伪凭证、通用资源预算、插件层或独立证明体系。普通未找到符号、类型错误、产物错误和目标限制继续走现有诊断路径。
- 不以 panic 数量归零作为验收指标。类型系统不能替代外部 I/O、分配失败、目标范围溢出、动态索引有效性或后端错误的处理。

## 1. 审查口径与结论

审查搜索范围覆盖 `compiler/`，重点沿生产代码的类型定义、构造路径和消费点核对。测试断言、自定义 `Parser::expect` 返回语法诊断、固定常量转换，以及同一局部检查后立即解包的分支，不直接归类为类型完备性缺口。

以下条目说明实施前内部表示或接口能够接收违反前提的组合，并指出可收紧的结构。部分对象已经通过私有构造和不可变访问保证实际不变量，因此不能仅凭存在 `unreachable!` 就认定合法 Scoop 源码可以使编译器崩溃。最初审查仅做静态阅读，未作穷尽性审计承诺；实施后的构建、测试与产物验收记录在 [PROGRESS.md](PROGRESS.md)。

编号用于定位审查项，优先级表示实施收益与顺序，不表示已确认漏洞的严重程度。

| 编号 | 现有缺口 | 建议表示 | 实施顺序 |
| --- | --- | --- | --- |
| R01 | `CallKind` 与 `Callee` 独立，允许矛盾组合和重复签名 ID | 按调用方式保存允许的目标、签名和 receiver | 优先 |
| R02 | `When.subject` 可空，条件与覆盖信息仍可要求 subject | 有 subject 与纯条件的两种完整结构 | 优先 |
| R03 | 最终 MIR 仍允许 Cast、协程临时 callee 和 pending chain | 临时需求留在 lowering 私有数据，最终 MIR 只含完成态 | 优先 |
| R04 | 固定元数 intrinsic 的操作和参数形态独立 | 一元、二元及具名操作分支各自携带完整参数 | 优先 |
| R05 | 函数、enum 等已知种类退化成通用 `TypeId` | 保留 `FunctionTypeId`、enum application ID 等已有窄身份 | 与相关 IR 重构一起完成 |
| R06 | declared/C/Scoop 专用接口接收通用 nominal 或 struct | 独立分支数据及实际需要的窄引用 | 与相关 IR 重构一起完成 |
| R07 | 语义 return 的 value 是否存在依赖函数返回类型 | 语义 return 携带完整值，物理 void 在 LIR 分类 | 与控制流重构一起完成 |
| R08 | NativeBorrowed call 与结果 root 可独立传入 | 按返回约定一次构造调用及结果发布信息 | 调用链收紧 |
| R09 | codegen 将已分类调用拆成互相关联的 `Option` | 完整 callee 与已准备 transition 的局部结构 | 调用链收紧 |
| R10 | core protocol 用位置数组保存通用 entry 枚举 | 具名且类型明确的 protocol 字段 | 局部收紧 |
| R11 | parser 修饰符的存在标志与 span 分离 | `Option<Span>` 或携带 span 的 modifier | 局部收紧 |
| R12 | AST literal pattern 接受任意表达式 | 专用 literal pattern 数据 | 局部收紧 |

## 2. 逐项结论与建议

### 2.1 R01：调用目标、调用方式与 receiver

现状：[MIR CallTarget](../../compiler/mir/src/control_flow/calls.rs) 同时保存 `kind: CallKind` 和 `callee: Callee`。Closure、FunctionBridge 两侧还分别保存 `FunctionTypeId`。因而数据结构允许 User 配 Closure、Extern 配 Virtual、两份函数类型 ID 不一致等组合。动态调用的 receiver 还依赖实参列表的首项存在。

[LIR call lowering](../../compiler/lir-lower/src/function/call.rs) 对 extern 断言 Direct，对普通本地 callee 配 Closure/FunctionBridge 使用 `unreachable!`；动态 receiver 从 `args[0]` 取得。

建议按 Direct、Virtual、Interface、Closure、FunctionBridge 组织 `Call/CallTarget` 的封闭分支。各分支只保存其允许的 callee 类别；closure/bridge 签名只保存一份；需要 receiver 的调用将其作为必填字段。具体字段归属由当前构造与优化调用方决定，不引入独立的调用插件或资格状态。

普通用户函数的参数数量和实际类型匹配仍是动态事实，应在原有决议或输入边界检查。重构不得改变 receiver 与参数从左到右、各求值一次的规则，也不能因拆出 receiver 而改变隐藏参数顺序。去虚拟化与内联改写应一次生成完整的新调用分支。

### 2.2 R02：when 的 subject 与允许的条件

现状：[LocalConcrete When](../../compiler/hir/src/concrete/body.rs) 保存 `subject: Option<Expr>`，同时允许 `WhenCondition::Case` 和带 subject 类型的穷尽性信息；[Export HIR](../../compiler/hir/src/body.rs) 也有对应组合。[具体化](../../compiler/hir-lower/src/concretize/body.rs) 与 [MIR when lowering](../../compiler/mir-lower/src/body/when.rs) 因此都需要 `expect("a case has a subject")`，覆盖信息还要求 `expect("coverage describes a subject")`。

建议区分有 subject 与纯条件两种 when。有 subject 的分支必填 subject，允许普通谓词和 case；纯条件分支只保存允许的谓词/后备条件。依赖模式匹配的覆盖信息归入有 subject 的结构，所需类型从实际 subject 的完整数据取得，避免独立记录互相矛盾的 subject 状态。

实施采用有 subject 与纯条件的完整分支，共享 arm 的 guard、正文与位置；覆盖类别不另存可从 subject 取得的类型或 enum 身份。导出模板在原有格式绑定边界归一化，wire 标签保持不变。共用 arm body、guard 或控制流实现时复用现有 helper，不复制整套 when lowering。保持 subject 快照一次、条件和 guard 顺序、guard 失败继续后续 arm、合法语句落空，以及 Nothing 路径不提供结果的语义。

### 2.3 R03：最终 MIR 不携带 lowering 临时态

现状：[MIR call 数据](../../compiler/mir/src/control_flow/calls.rs) 明确注明 `Callee::CoroutineSuspend` 仅供 CFG 构造至协程变换之间使用，最终 call 的 `pending` 必须为 Root，但类型仍允许这些临时状态。[MIR ExprKind](../../compiler/mir/src/control_flow.rs) 还保留 `Cast`。

消费方的 [call lowering](../../compiler/lir-lower/src/function/call.rs) 遇到 CoroutineSuspend 时 `unreachable!`，[expression lowering](../../compiler/lir-lower/src/function/expression.rs) 遇到 Cast 时 `unreachable!`；[MIR 验证](../../compiler/mir/src/validation.rs) 检查 pending 已归 Root。

核对实际生产路径后，语言 Cast 已在 mir-lower 私有正文构造中展开，因此删除最终 MIR 中未被生产的 Cast 类别；将协程注册操作和 pending chain 放入协程 pass 的私有输入或工作数据。最终 `scoop-mir` 只暴露可以交给 LIR 的调用和表达式，不为恒定为 Root 的状态保留一个可变枚举字段。

实施前核对每个临时状态的真实生产者与消费时段；按需要局部拆分，不复制完整 MIR 或建立跨阶段通用状态机。协程正常/异常恢复、finally 的待定转移和 Context 清理必须保留。

### 2.4 R04：固定元数 intrinsic 的完整操作

现状：[HIR IntegerOperation](../../compiler/hir/src/concrete/body.rs) 将 operation 与 `HirIntegerOperationArguments` 独立保存。二元 div/rem 可以在结构上配 Unary 参数，随后在 [整数 lowering](../../compiler/mir-lower/src/body/operators/primitive.rs) 触发 `unreachable!`。普通 intrinsic 还常用 `Vec<Expr>` 传入固定元数参数，再通过 `try_into().expect(...)` 解包。[Data borrow lowering](../../compiler/mir-lower/src/body/data_borrow.rs) 要求输入 slice 恰好是 object 与 block。

建议表示为携带一元操作与单个 operand 的分支、携带二元操作与 lhs/rhs 的分支；操作种类本身也按允许的元数收窄。data borrow 保存具名 object、block 及其已选定的对象/签名信息。源码参数列表在 HIR 决议时完成映射，后续传递固定形态；一般用户函数继续使用实际签名和参数列表。

实际路径中，整数 normalized expression 使用一元／二元的窄操作与完整 payload，默认模板保留同一结构，wire 仍投影既有操作和参数标签。普通源码／导出模板中的数据借用是含实际声明的泛型调用；在具体化解析 intrinsic 与实例形参时构造完整借用操作，对象类别和回调 FunctionTypeId 来自形参，随后 MIR 无需解包参数列表或重查值的类型。方法引用按已选操作直接分配固定形参；普通函数继续使用其签名参数列表。

Nothing 操作数仍按既有终止语义处理。必须先完成有序求值与终止传播，只有正常产生值的路径才建立整数操作；不能为了满足新操作数类型而构造零值、Unit 或其他占位值。

### 2.5 R05：保留已确定的类型种类与身份

现状：[intern_function_type / instantiate_function_type](../../compiler/hir-lower/src/types/interning.rs) 返回通用 `TypeId` / `Option<TypeId>`。调用方又在 [函数类型替换](../../compiler/hir-lower/src/types/substitution.rs) 等路径匹配 `Type::Function`，以 `unreachable!` 排除其他形态。[EnumVariantApplication](../../compiler/hir/src/body.rs) 的 owner 是通用 `TypeId`，[enum variant/field 查询](../../compiler/hir/src/declarations/nominals/enums/queries.rs) 再断言 owner 是 enum。

建议函数签名 API 直接返回已有 `FunctionTypeId`，需要普通类型时从签名的 `canonical_type` 取得。enum variant owner 使用对应的 enum application ID，保留原 variant/field 的 persistent identity。其他已确定种类的转换沿用同样原则，只在真正需要通用类型的边界转换。

泛型替换是否完整、引用是否属于正确 owner、导入 ID 是否存在，仍按既有语义和引用规则处理。专用 ID 解决实体类别混用，不能单独证明任意 ID 在当前 arena 中有效；不增加 FQN fallback、跨 stage 实现依赖或重复身份系统。

### 2.6 R06：declared nominal 与表示专用接口

现状：[HIR nominal](../../compiler/hir/src/concrete/nominals.rs) 和 [MIR types](../../compiler/mir/src/types.rs) 的 `declared_fields` / `declared_fields_mut` 挂在同时允许 Declared 与 Intrinsic 的定义上；传入 Intrinsic 就 panic。[LIR StructDefs::set_scoop_fields](../../compiler/lir/src/metadata.rs) 接受通用 `StructDefId`，对 C 或 intrinsic struct 同样 panic。

[Codegen C payload helper](../../compiler/codegen/src/llvm_types.rs) 与 [C bridge layout](../../compiler/codegen/src/c_bridge/layout.rs) 接受通用 struct，再调用 `c_fields().expect(...)`。LIR 已有 `CStructRef`，但部分消费接口没有保留这一类别。

建议将 Declared 分支的字段、属性等数据抽成独立 payload，相关方法接收该 payload 的引用；需要跨表持有 ID 的入口使用实际所需的窄引用。Scoop 字段更新可采用与 CStructRef 对应的引用，C 专用 helper 直接接收 C 视图及完整字段。仅在入口匹配一次宽枚举，后续传递具体分支。

共同的 Intrinsic 表示在 struct/class 消费中仍可能包含不适用的变体，实施时随实际调用方决定是否进一步区分两类 payload。字段边界、布局、C ABI 与 nominal 身份检查保留，不能把“不适用”伪装成空字段或普通 ZST。

### 2.7 R07：语义 return 必须具有完整结果

现状：[LocalConcrete Return](../../compiler/hir/src/concrete/body.rs) 与 [MIR Return](../../compiler/mir/src/control_flow.rs) 使用 `Option<Expr>`，是否必须存在依赖所属函数的返回类型。[CFG prepare_return](../../compiler/mir-lower/src/cfg/transfers.rs) 断言 Unit return 不含表达式，并对非 Unit 缺值 `expect`；[LIR terminator lowering](../../compiler/lir-lower/src/function/statements.rs) 还有 `non-Unit return without a value` 分支。

建议语义 IR 统一使用携带完整表达式的 return。Unit return 保存真实的 Unit 值；原表达式的副作用继续按原顺序执行。到 LIR 根据物理 ABI 再决定 void、elided ZST、直接结果或间接结果。

Unit 与 Nothing 必须继续区分：正常 Unit 返回具有语义值；终止路径不产生 return value，不得用 Unit 掩盖。返回值与函数声明结果的类型匹配仍由语义边界检查。先同步现有 bare Unit return 的阶段合同，再调整具体化、CFG cleanup、协程保存和优化中的相关消费者。

### 2.8 R08：NativeBorrowed 结果发布与调用一次构造

现状：[CallTargets::bind_native_borrowed_call](../../compiler/lir/src/calls/typed.rs) 分别接收 `TypedCall` 和 `NativeBorrowedResultRoot`，然后检查返回约定、scan 是否非空、root storage 是否等于间接返回 storage；不一致时 assert 或 panic。[调用方](../../compiler/lir-lower/src/function/call/protocol.rs) 已经依据同一 call 的签名完成一次分类。

输出侧已有私有封装，问题主要在构造入口接受了两份可能矛盾的信息。建议 lowerer 按返回约定一次生成完整调用与发布信息：间接返回直接复用调用本身的 storage；直接返回有 managed scan 时分配相应 root storage；void/ZST/GC-free 返回使用各自完整分支。IR crate 继续只保存数据，存储分配和转换仍由 lowerer 完成。

必要的 native ABI、scan 和 storage 关系在原有边界确认。类型收紧不能省略跨 safepoint 的保活、结果发布、reload 或 root frame 生命周期，也不增加独立的调用认证对象。

### 2.9 R09：codegen 保留已分类的调用数据

现状：[emit_typed_invoke](../../compiler/codegen/src/function/call/managed/invoke.rs) 接收 destination 与独立的 `Option<FunctionValue>`，直接调用分支需要 `expect`。[typed call emission](../../compiler/codegen/src/function/call/managed.rs) 还依赖 native、transition、safepoint 等相关 Option 同时存在。

当前调用路径已经维持这些关系，不能据此认定存在源码触发的 panic。建议使用 codegen 局部的完整 callee 分支，例如 Direct 携带 FunctionValue、Dispatch 携带分派所需数据；native transition 的准备分支一并取得 safepoint 与 roots，发射入口握手后保留退出所需的 roots、frame 与结果存储。后续消费具体分支，避免先拆散再恢复关联；已消费且后续不再使用的状态不必额外保存。

保持现有 LIR 的调用分类，复用已有准备步骤，不另建平行调用实现。LLVM builder、后端验证或目标发射的实际失败继续返回现有错误，不能假定它们都能由 Rust 类型提前排除。

### 2.10 R10：Core protocol 从位置数组改为具名字段

现状：[ImportedCoreProtocolProduct](../../compiler/hir/src/foundation/imported_protocols.rs) 保存 `[ImportedCoreProtocolEntry; N]`，保证数量但不保证每个位置的类别。concrete nominal、generic nominal、callable、variant、field、slot 与 exact type 的访问 helper 依赖此前的类别检查，并以 `unreachable!` 解包。[生产侧 product](../../compiler/hir/src/production/core_protocol_surface.rs) 也保存通用 entry 数组。

建议内存表示使用具名、类型明确的字段。例如 Option protocol 直接保存 owner、Some variant、Some payload、None variant 的专用身份；其他 protocol 按当前真实角色保存对应数据。wire 编码可保持既有字段/顺序，解码时检查一次，再交付具体字段。

已有私有构造和不可变访问已经限制了错误来源，因此这是局部结构收紧。真实 provider、owner 关系、完整签名与引用合法性继续沿普通规则验证；不会因为字段类型具体化就引入 core 来源资格或把普通库成员升级为 compiler protocol。

### 2.11 R11：Parser 修饰符的存在与位置

现状：[Modifiers / MemberPrefix](../../compiler/parser/src/decl/mod.rs) 分别保存 `is_suspend` 与 `suspend_span`、`is_const` 与 `const_span`、method modifier 与 modifier span。诊断分支在 [函数解析](../../compiler/parser/src/decl/functions.rs) 和 [成员解析](../../compiler/parser/src/decl/nominals/members.rs) 中通过 `expect` 取出位置。

建议只有存在性与位置的修饰符使用 `Option<Span>`；带实际值的修饰符使用携带 span 的 modifier。是否存在由同一个值决定，省去同步维护 bool 和 Option 的义务。既有重复修饰符、错误声明位置与准确源码诊断保持不变。

本项只整理已有修饰符的数据表示。M37 的兼容修饰符语法及其完全忽略规则属于后续语言工作。

### 2.12 R12：AST 字面量模式的封闭值域

现状：[Pattern::Literal](../../compiler/ast/src/statements.rs) 保存任意 `Expr`，[literal pattern dump](../../compiler/ast/src/dump/patterns.rs) 只处理字面量及允许的数值符号语法，其他表达式触发 panic。

建议使用专用 literal pattern 数据，明确包含整数、浮点、Char、String、Boolean、Unit，以及语言允许的带符号数值语法。保留原始数字拼写/精确表示与源码 span；需要与普通表达式共用解析或格式化时，复用真正相同的 literal payload。

parser 继续在源码边界报告非法模式，AST 消费者不再接受任意调用、字段访问或控制表达式作为 literal pattern。不得顺带扩展模式语法或为其新增表达式执行机制。

## 3. 只能部分静态化的关系与保留的检查

[MIR Expr](../../compiler/mir/src/control_flow.rs) 公开保存 `ty` 与 `kind`，整数构造器断言操作数具有操作所需的精确整数种类；[指针 lowering](../../compiler/lir-lower/src/function/pointers.rs) 还断言 raw pointer 类别、ULong carrier 以及 pointee 的一致性。

这些位置适合随相应操作族局部收紧，但应区分两类事实：

| 事实 | 处理方式 |
| --- | --- |
| 操作必须是一元/二元、必须有 receiver 或结果、只接受某类实体 | 用封闭分支、必填字段和专用 ID 表达 |
| 整数实际位宽相等、pointee 相等、某字段属于某 owner、参数类型满足签名 | 在已有语义或引用边界检查，成功后保留完整关系并复用 |
| 某条路径没有正常结果 | 先传播 Nothing/终止控制流，只在正常路径构造值操作 |
| CFG 支配关系、循环目标合法性、异常/cleanup 与 GC roots | 保留实际转换或消费边界所需检查，不能仅凭换了字段类型删除 |
| 外部产物 bytes、未知 tag、实际索引范围、大小/对齐溢出、LLVM 发射失败 | 返回既有边界诊断，不承诺通过内部类型完全消除 |

M36 不要求为每种动态 exact type 建立 Rust 泛型参数，不建立全程序静态证明框架。同一不变关系一旦在明确边界确认，应复用结果；数据被优化或 CFG 改写后，只核对本次变换实际影响的合同。

已有的 `LocalConcreteHir::TypeKind` 不含 Param、LIR 的调用分支与 typed result、非空序列和私有不可变引用等结构，应继续使用。无需为了替换其内部已经受保证的分支，增加另一套相同功能的数据通道。

## 4. 实施顺序与规范迁移

以下为建议批次。实施前先核对 M35 完成后的实际代码，确认各审查项仍存在；若已被其他必要修复消除，记录事实即可，不重新制造改动。

| 批次 | 内容 | 完成条件 |
| --- | --- | --- |
| M36-1 | R01 调用目标与 R04 固定元数操作；同步相关 R05 身份接口 | 构造、优化、dump、产物投影与后续消费统一使用完整分支，重复目标/参数形态不再可独立填写 |
| M36-2 | R02 when、R03 临时态边界、R07 return | 最终 MIR 无临时标记，when/return 的必需内容由结构保存；正常、终止、异常和协程路径保持 |
| M36-3 | 完成 R05、R06 与 R10 | 已知实体类别、表示专用 payload 和 protocol 字段贯通生产/消费，原引用与布局验证只在必要边界保留 |
| M36-4 | R08 与 R09 | native 结果发布及 codegen 准备数据一次构造，调用/返回/GC 生命周期的现有合同保持 |
| M36-5 | R11 与 R12；完成相关回归和记录 | 解析与 AST 消费使用完整数据，既有语义、诊断和 CLI 闭环经过实际验证 |

批次不是建立新通用框架的理由。小范围且独立的身份/修饰符收紧可提前完成；跨阶段表示变更应作为完整的生产到消费路径落地，不能留下供后续 stage `unwrap` 的中间缺口。

实施按 spec 先行处理以下文档关系：

- 实现规范 2.2～2.5：明确 HIR/MIR/LIR 成功输出中的调用、when、return、临时态与 codegen 消费职责。接口收紧改变阶段合同的部分先更新规范。
- 实现规范 2.6、2.11～2.12：核对真实 metadata 编码、typed identity 与 protocol 数据。仅 Rust 内存表示变化且 wire 合同不变时，不要求格式升版；实际编码或可消费合同变化时只升级受影响 section，并走既有兼容诊断。
- 语言规范：核对现有语义保持。M36 不新增 M37 的语言能力，也不通过 IR 改写改变合法程序集合。
- 运行时规范：核对调用、结果发布、异常与 GC 契约保持。只有实际 runtime 合同变化才需要修改，不因 Rust 类型重构统一提升 ABI。
- ROADMAP：已登记 M36 类型完备性里程碑，并将语言能力补齐对应到 M37；完成状态随实际验收更新。

## 5. 验收与回归

验收围绕当前真实语言程序、实际产物消费和受影响的转换边界。内部无效组合由 Rust 数据结构排除后，不为构造这些组合保留宽接口，也不新增只有测试使用的语义工厂。外部非法输入仍需要对应的格式/引用错误测试；源码已有的编译错误规则继续有 negative fixture，并断言位置与信息。

| 范围 | 独立及组合验证 |
| --- | --- |
| 调用与 intrinsic | direct/virtual/interface/closure/bridge，本地与导入、泛型、receiver 顺序、固定元数运算、Nothing 实参、debug/release 优化 |
| when 与 return | 有/无 subject、普通条件/case/guard、覆盖与合法落空、Unit/非 Unit、泛型替换为 Unit、终止分支、try/finally/Context |
| 协程与临时态 | 挂起/恢复、成功/失败、pending return/throw/loop cleanup；最终 MIR golden 不再包含已移出的临时节点 |
| 类型与 nominal | Function/FunPtr、enum variant/payload、declared/intrinsic、C/Scoop layout、别名与完整泛型 application、跨 Cone 模板及再次发布 |
| NativeBorrowed/codegen | void/ZST/直接/间接结果、GC-free/含引用值、managed/native/invoke、移动 GC 下结果发布与 reload |
| Protocol | 实际 core 产物构建/导入、固定角色类型与引用错误、修改并重建 core 后的正常消费；wire 不变时核对编码仍一致 |
| Parser/AST | 已有修饰符的合法与错误位置、重复诊断、各 literal pattern 及带符号数值、AST/HIR dump 和准确 span |

每批代码完成后先按仓库要求格式化与 lint，再运行受影响的 Rust 测试和文件 fixture。涉及 Python 时先使用仓库规定的 Ruff 版本。最终验收使用配套 scoop/scoopc/scoop-linker CLI、fixture runner unittest 与完整 fixture 流程；平台和优化模式结果按实际执行情况记录。

完成记录说明每项消除了哪一种可表示的错误状态、哪些边界检查仍保留，以及测试覆盖了哪些真实组合。panic 数量下降、仅能通过 Rust 编译或仅更新 golden 都不能代替受影响能力的编译、产物消费、链接与运行验证。各项实际执行情况与最终验收结果见 [PROGRESS.md](PROGRESS.md)。

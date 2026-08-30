# Scoop 实现大纲

版本：0.2（草案）

配套文档：`SCOOP-SPEC.md`（语言规范）、`SCOOP-RUNTIME-SPEC.md`（运行时规范）。本文引用其章节号。

## 1. 总体技术路线

- 编译器实现语言为 **Rust**，LLVM 绑定使用 **inkwell**（feature `llvm22-1`）；个别 inkwell 未覆盖的 LLVM 子系统（statepoint / stackmap 等）可降落到 `llvm-sys`；
- 使用 **LLVM 22.1** 作为编译器后端（与当前 Rust 工具链自带的 LLVM 版本一致）；
- 使用 **LLVM stackmap** 生成 GC 所需的 metadata；
- 使用 **LLVM landingpad** 作为 exception 的基础设施（Windows / catchpad 留待后续）；
- runtime library 使用 **C 语言**，包含一个基于 **Immix 的分代式 GC** 以及其他必须的 runtime 功能；
- GC register root 的操作使用平台相关汇编。

## 2. 编译器 pipeline

**模块边界**：stage 之间只通过 IR / meta crate 交换数据——AST、HIR、MIR、LIR 的定义（含各自的 `.slib` meta 格式）独立成 crate，作为 stage 之间的通道。每个 stage crate 只负责把输入变成输出，只依赖其输入/输出的 IR crate，不了解、不依赖上游 stage 的实现；stage 之间的编排由 driver 负责（见 2.7）。

### 2.1 parser / AST

解析源代码，为每个源文件生成语法树。

lexer 对坏字符及可恢复的字面量错误继续扫描；parser 分别以顶层声明、类型成员、块内语句为同步边界，在一次解析中收集同一文件的多个独立诊断。诊断按源码顺序输出；只要存在任一诊断，恢复得到的残缺 AST 必须整体丢弃，不得进入 HIR。

### 2.2 HIR

负责 desugaring、type check 和 overload resolution；综合上游 Cone 的 generic HIR representation；解析每个表达式/子表达式的 type，解析每个 callable 的 target。输出：

- **generic HIR function/type list**：包含 generic 信息的 IR，供下游 Cone 的 HIR 阶段使用，支持 export generic（spec 12.5）；
- **instantiated function/type list**：涵盖所有 non-generic / instantiated type/function，其中所有 type 已完全解析、所有 generic type parameter 已完全填好。

**所有编译期错误都在 HIR 层报告**，之后的 stage 不再做源代码错误处理。

HIR 负责解析所有 type parameter：确定每个 generic 调用的具体类型实参（含对上游 Cone generic HIR 的实例化请求），输出完整的实例化需求清单，但**不生成实例体**——实例体由 MIR 生成（见 2.3）。

调用与值构造的泛型推导对整组实参执行固定点约束求解，不得按从左到右的一次遍历决定成败。依赖期望类型的实参可延迟到其他实参完成绑定后再检查；为每个实参产生的 desugaring 语句必须分开缓存并最终按源码实参顺序拼接，类型检查顺序不得改变运行期求值顺序。

泛型 interface 的 HIR 类型必须同时携带 interface id 与完整类型实参；实现列表同样保存已解析的 interface 应用而非裸 id。HIR 按声明点 `in` / `out` 计算子类型关系并递归校验类型参数的使用位置。MIR 为每个实际使用的具体 interface 应用建立独立的单态化 interface 实体、TypeDescriptor 与 itable 身份；不得把不同类型实参的应用擦除到同一个 interface id。

此外归属 HIR 的语义工作：

- 字符串插值脱糖（spec 6.2）、`?.` / `?:` 脱糖（spec 7.3）、`for` 脱糖（spec 11.8）等；
- 在 callable 签名、调用目标与 override / interface 实现关系中保留 `suspend` 标志；以显式、可嵌套的上下文状态检查挂起调用只出现在挂起函数或已登记的协程构建器中，不能把“当前无函数”当作默认允许。进入顶层属性、object/companion、实例属性、delegate、`init`、构造函数/构造委托及普通属性访问器等声明自身拥有的初始化/访问体时，必须压入带原因的 forbidden context；离开后恢复调用者上下文，所以 suspend caller 的显式构造实参仍可挂起。MIR 不为这些初始化入口或属性访问器生成 frame / continuation ABI；
- `const val` 在 HIR 做常量表达式求值与依赖环检查；其值进入可供下游 Cone 使用的 HIR meta，不生成 runtime initializer。普通/挂起 call、构造、分配及普通属性读取均不能进入 const expression IR；
- 对非 `Unit` 块体执行组合式控制流分析，证明所有可达路径均以有值 `return` 或 `throw` 结束；`finally` 的必退出路径覆盖 try/catch 的待执行结果（spec 第 8 章）；
- 默认参数值的调用处实例化（spec 8.5）；`getCurrentSourceLocation` 在缺省参数中的常量化（spec 11.12）；
- 装箱/拆箱的插入（spec 4.4.4 的 O(1) 规则）；
- `when` 的穷尽性检查（spec 第 5 章）。

### 2.3 MIR

接收**本 Cone** 的 HIR output 中的 instantiated function/type list 部分，以及**上游 Cone 的 MIR meta**（见下），负责：

- 为每一个（generic 定义 + 已确定 type param 组合）生成特定的单态化实例体，并为 function/type 做 name mangling；
- 为每个 call 标注 virtual / interface / direct call 类型；
- 为每个具体类型建立 vtable / itable（见 2.9）。本 Cone 的类型实现上游接口或继承上游类时，表结构、槽位布局与 TypeDescriptor 符号取自上游的 MIR meta；
- 把结构化 HIR 降为类型化 CFG：调用从嵌套表达式中按源码求值顺序正规化出来，控制边与异常 unwind 边显式化，`try` / `catch` / `finally` 的 cleanup 路径在 MIR 固定。LIR 只负责把这些边映射为目标相关的 landingpad 结构；
- **先单态化、后协程变换**：对每个具体 suspend function，把 CFG 在挂起调用后切分为恢复状态，计算跨挂起点活跃的值并生成堆上 frame；每个挂起点生成与其结果类型精确匹配的 `Continuation<T>` 实际类型。frame、continuation adapter 与内部完成结果均有独立的类型化 id、TypeDescriptor 与递归引用扫描描述，不允许用 FQN 或 `Any` 作为实体/结果回退；
- suspend callable 的内部 ABI 是 `(source args..., Continuation<R>) -> CoroutineStep<R>`，其中编译器内部值 enum `CoroutineStep<R>` 只有 `Completed(R)` / `Suspended` 两个变体。立即完成走 `Completed`；返回 `Suspended` 后只能由传入的 continuation 恢复。变换完成后 MIR output 不再含源码级 suspend callable 或 suspend call；
- 恢复失败在对应恢复状态入口重新注入为 `throw`，沿原调用点的 unwind / cleanup 边传播。挂起不是作用域退出，不能触发 `finally`；跨挂起的 pending return / exception / cleanup 动作必须作为有判别的 frame 字段保存，不能依赖未初始化槽或原生栈状态；
- 把 `when` 模式匹配降级为 decision tree / 跳转序列；
- 输出 MIR type/function list，其中不再包含任何 generic 和 suspend（诊断信息除外）。

**MIR meta**：每个 Cone 的 MIR 同时输出一份 metadata，随 `.slib` 导出（见 2.6），内容包括：各单态化实例的符号、泛型来源与具体类型实参，各导出类型的 vtable / itable 结构、TypeDescriptor 符号与 name mangling 结果。类型布局不在其中——布局由 LIR 生产、经 LIR meta 导出（见 2.4）。HIR 的 generic function、已解析类型实参的 generic function 与 MIR 的单态化实例是三类实体，分别使用不同的类型化 id。

### 2.4 LIR

接收**本 Cone** 的 MIR output，以及**上游 Cone 的 LIR meta**（见下），负责：

- 为每个 type 生成布局信息（struct/enum/tuple 布局、`@CLayout` 的 pack/align、`Option` 的 niche 编码——spec 7.4），并生成布局完备的递归引用扫描描述：普通引用偏移、同起点 sequence、按 tag 分派的 enum 子扫描、数组元素子扫描。需要上游布局的场景：本 Cone 的类继承上游类（继承字段的偏移）、跨 Cone 嵌套的值类型（上游 struct/enum/tuple 嵌入本地类型、作为数组元素、按值传参的 ABI）；
- statepoint 的落地形态（M9 定稿）：LIR 保持 statepoint 无关的指令形态，由 codegen 给每个 function 设置 GC strategy（`statepoint-example`）并执行 `rewrite-statepoints-for-gc` pass，同时在函数入口与循环回边插入 safepoint poll（详见 codegen 的实现与注释；statepoint 的"插入职责在 LIR"是早期表述，以此为准）；
- 把 MIR 已显式化的 normal / unwind / cleanup CFG 机械映射为 landingpad + personality function；`throw` 接到 runtime 入口。对 suspend function 的 handler，进入可挂起的 Scoop catch/finally 代码前必须把 ABI 异常物化为 managed 对象并结束原生 catch，任何原生 EH 状态都不得跨挂起点；
- 输出 LIR type/function list，不再包含任何 Scoop 特有的内容，可以机械翻译成目标 IR 或其他格式。

**LIR meta**：每个 Cone 的 LIR 同时输出各导出类型的布局与递归引用扫描描述，随 `.slib` 导出（见 2.6）。布局及扫描描述只在定义它的 Cone 计算一次，下游直接消费、不重算，保证全程序一致；结构上不能用可缺失字段把 tagged enum 或聚合元素的扫描责任推迟给 codegen。

### 2.5 codegen

只接收**本 Cone** 的 LIR output，负责：

- 将 LIR output 机械翻译成目标 IR（本阶段为 LLVM IR），然后用 LLVM 编译成 `.o`；
- 生成每个具体类型的 `TypeDescriptor`（runtime spec 2.2：类型标识、实例大小、递归引用扫描描述、父类型表、`equals`/`hashCode`/`toString` 分发入口）；
- 展开登记表中归属 codegen 的 `@Intrinsic`（见 2.10）；
- extern 声明的符号发射与 calling convention 属性（spec 13.4）、`addressOf` 的 lvalue 语义（spec 13.10）。

codegen **不需要任何上游 meta**：上游信息已逐层吸收进本 Cone 的 LIR（布局经 LIR meta、符号经 MIR meta），对上游函数/TypeDescriptor 的引用一律发射为外部符号，链接期解析。两个链接层规则：

- **重复实例去重**：不同 Cone 可能各自单态化出同一个实例（如两个 Cone 都实例化上游的 `foo<Int>`），同名实例符号必须以 `linkonce_odr` / COMDAT 形式发射，由 linker 去重（前提是 name mangling 全程序一致）；
- **符号可见性**：`internal` 符号可本地化，但被导出的泛型体 / 默认参数表达式引用的 `internal` 符号必须保留可链接名字（spec 8.5、12.5）。

### 2.6 `.slib` 打包

把 codegen 产出的 `.o` 与 metadata 打包成 `.slib`（spec 12.5）。metadata 包括三层：HIR 的 generic 输出（供下游 HIR）、MIR meta（供下游 MIR，见 2.3）、LIR meta（供下游 LIR，见 2.4）。包含一个 `.slib` reader，作为下游 Cone 各阶段的输入。

### 2.7 build driver

不属于编译器 stage，但为必需组件：

- 依赖图解析与无环检查（spec 12.3）；
- 上游 `.slib` metadata 变化时触发下游重编译（spec 12.5）；
- 调度各 Cone 的编译与最终链接。

### 2.8 linker

收集所有 Cone 编译产生的 `.o`，与 runtime lib 链接，生成可执行程序。

### 2.9 虚/接口调用分派

- MIR 为每个具体类型建立 **vtable**（类层次分派）与 **itable**（接口分派）；标注 call kind 时，virtual / interface call 的 target 指向对应 table entry，direct call 指向具体函数符号。
- 表的内容由 MIR 定义，由 codegen 以数据形式发射，并从 `TypeDescriptor` 引用：TypeDescriptor 内嵌 vtable 指针与 itable 数组（见 runtime spec 2.2）。`Any` 的 `equals` / `hashCode` / `toString` 即 vtable 的固定前三个槽位。
- **装箱值类型的 this 调整**：值类型装箱后对象为 header + payload，而值类型成员函数以 payload 为 `this`；vtable / itable 中对应装箱值类型的表项指向 MIR 生成的 **adjust thunk**（`this` 加 header 偏移后 tail-call 真正的成员函数）。
- **跨 Cone 的槽位识别**：初版 itable 采用（接口 TypeDescriptor 指针 → 方法表）的键值查找，调用点按接口 TypeDescriptor 地址查找，不需要跨 Cone 的全局槽位编号；槽位编号等优化留待后续。
- **泛型成员函数不参与虚分派**：带自身类型参数的成员函数不进入 vtable / itable（单态化实例无法枚举）；interface 可保留这类方法的声明与实现约束，但分派表布局必须跳过它，经 interface 静态类型调用由 HIR 拒绝。class 上的泛型成员必须为 final，值类型成员本来即为 final，因此合法调用一律标为 direct。泛型宿主的参数与方法自身参数使用同一类型参数编号空间，前缀为宿主参数、后缀为方法参数；`ResolvedGenericFunction` 与 MIR 单态化键携带同序的完整实参向量。把 interface 签名代入某个实现宿主时，必须在替换 interface 参数后把方法参数重基化到实现宿主参数前缀之后，避免两组 `TypeParamId` 碰撞。
- **结构化表达式降级**：AST 在表达式位置直接表示 `if` / `when` / `try`。HIR lower 先确定所有正常分支的共同结果类型，再分配一个类型完备的隐藏结果 local，在每个可正常结束的分支尾写入该 local，并把原有 HIR 结构化语句追加到表达式的 desugaring sink；表达式本身成为该 local 的读取。结果为 `Unit` 时无需结果 local，但仍须保留分支尾表达式的求值。MIR / LIR 继续只接收已有的结构化控制语句，不新增内嵌 CFG 的表达式节点。

### 2.10 intrinsic 的分阶段处理

不同的 intrinsic 在其**信息就绪的最早 stage** 展开，而不是集中在 codegen：

- 编译器内置一张 **intrinsic 登记表**：`name → （展开 stage、种类、签名约束）`。`@Intrinsic("name")` 声明只携带 name；name 必须在表中，否则 HIR 报编译错误——用户不能用自定义 name 声明 intrinsic。
- **检查与展开分离**：无论在哪一阶段展开，类型检查、`value` / `ref` 约束检查、注解共存检查一律在 HIR 完成（与"所有编译期错误在 HIR 报告"一致）。
- 晚于 HIR 展开的 intrinsic 在中间 IR 中以"完全实例化的已知符号调用"形式存在；各 IR 的不变量（如 MIR 输出"不含 generic"）对这类调用设例外——其 type argument 已全部确定，只是调用节点留待后续 stage。
- 每个 intrinsic 恰好展开一次；其后的 stage 只看到普通 IR。

初始分类：

| 展开 stage | intrinsic | 理由 |
|---|---|---|
| HIR | `current_source_location` | 编译期常量，直接折叠为 `SourceLocation` 值实例（spec 11.12） |
| LIR | `size_of` / `align_of` | 依赖 2.4 计算的布局信息 |
| codegen | 算术/位运算（`int_add` 等）、`ptr_load` / `ptr_save` / `ptr_cast` | 直接映射到目标 IR 指令 |
| codegen | `address_of` | 需要后端的寻址模型与 lvalue 物化 |

## 3. 待明确事项

1. **异常穿越 FFI frame 的最终规则**（runtime spec 第 5/9 章的 TBD）。
2. **runtime spec 同步**：第 3/9 章原写"GC 方案待定"，现定为 Immix 分代 + C runtime + 汇编 register root；分代意味着写屏障 / remembered set，需与 runtime spec 3.6 的屏障预留对齐后更新该文档。
3. **Windows 异常**（catchpad）与调试信息（line table 等）留待后续。

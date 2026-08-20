# Scoop 实现大纲

版本：0.1（草案）

配套文档：`SCOOP-SPEC.md`（语言规范）、`SCOOP-RUNTIME-SPEC.md`（运行时规范）。本文引用其章节号。

## 1. 总体技术路线

- 编译器实现语言为 **Rust**，LLVM 绑定使用 **inkwell**（feature `llvm22-1`）；个别 inkwell 未覆盖的 LLVM 子系统（statepoint / stackmap 等）可降落到 `llvm-sys`；
- 使用 **LLVM 22.1** 作为编译器后端（与当前 Rust 工具链自带的 LLVM 版本一致）；
- 使用 **LLVM stackmap** 生成 GC 所需的 metadata；
- 使用 **LLVM landingpad** 作为 exception 的基础设施（Windows / catchpad 留待后续）；
- runtime library 使用 **C 语言**，包含一个基于 **Immix 的分代式 GC** 以及其他必须的 runtime 功能；
- GC register root 的操作使用平台相关汇编。

## 2. 编译器 pipeline

### 2.1 parser / AST

解析源代码，为每个源文件生成语法树。

### 2.2 HIR

负责 desugaring、type check 和 overload resolution；综合上游 Cone 的 generic HIR representation；解析每个表达式/子表达式的 type，解析每个 callable 的 target。输出：

- **generic HIR function/type list**：包含 generic 信息的 IR，供下游 Cone 的 HIR 阶段使用，支持 export generic（spec 12.5）；
- **instantiated function/type list**：涵盖所有 non-generic / instantiated type/function，其中所有 type 已完全解析、所有 generic type parameter 已完全填好。

**所有编译期错误都在 HIR 层报告**，之后的 stage 不再做源代码错误处理。

HIR 负责解析所有 type parameter：确定每个 generic 调用的具体类型实参（含对上游 Cone generic HIR 的实例化请求），输出完整的实例化需求清单，但**不生成实例体**——实例体由 MIR 生成（见 2.3）。

此外归属 HIR 的语义工作：

- 字符串插值脱糖（spec 6.2）、`?.` / `?:` 脱糖（spec 7.3）、`for` 脱糖（spec 11.8）等；
- 默认参数值的调用处实例化（spec 8.5）；`getCurrentSourceLocation` 在缺省参数中的常量化（spec 11.12）；
- 装箱/拆箱的插入（spec 4.4.4 的 O(1) 规则）；
- `when` 的穷尽性检查（spec 第 5 章）。

### 2.3 MIR

接收**本 Cone** 的 HIR output 中的 instantiated function/type list 部分，以及**上游 Cone 的 MIR meta**（见下），负责：

- 为每一个（generic 定义 + 已确定 type param 组合）生成特定的单态化实例体，并为 function/type 做 name mangling；
- 为每个 call 标注 virtual / interface / direct call 类型；
- 为每个具体类型建立 vtable / itable（见 2.9）。本 Cone 的类型实现上游接口或继承上游类时，表结构、槽位布局与 TypeDescriptor 符号取自上游的 MIR meta；
- 把所有 suspend function 变换成状态机，并生成对应的 Continuation 实际类型（Continuation 类型必须携带 TypeDescriptor / 引用位图，挂起帧在堆上时可被 GC 扫描）；
- 把 `when` 模式匹配降级为 decision tree / 跳转序列；
- 输出 MIR type/function list，其中不再包含任何 generic 和 suspend（诊断信息除外）。

**MIR meta**：每个 Cone 的 MIR 同时输出一份 metadata，随 `.slib` 导出（见 2.6），内容包括：各导出类型的 vtable / itable 结构、TypeDescriptor 符号与 name mangling 结果。类型布局不在其中——布局由 LIR 生产、经 LIR meta 导出（见 2.4）。

### 2.4 LIR

接收**本 Cone** 的 MIR output，以及**上游 Cone 的 LIR meta**（见下），负责：

- 为每个 type 生成布局信息（struct/enum/tuple 布局、`@CLayout` 的 pack/align、`Option` 的 niche 编码——spec 7.4）。需要上游布局的场景：本 Cone 的类继承上游类（继承字段的偏移）、跨 Cone 嵌套的值类型（上游 struct/enum/tuple 嵌入本地类型、作为数组元素、按值传参的 ABI）；
- 给每个 function 插入 statepoint（spec 14.2）；
- try / catch / finally 降级为 landingpad + personality function；`throw` 接到 runtime 入口；
- 输出 LIR type/function list，不再包含任何 Scoop 特有的内容，可以机械翻译成目标 IR 或其他格式。

**LIR meta**：每个 Cone 的 LIR 同时输出各导出类型的布局信息，随 `.slib` 导出（见 2.6）。布局只在定义它的 Cone 计算一次，下游直接消费、不重算，保证全程序布局一致。

### 2.5 codegen

只接收**本 Cone** 的 LIR output，负责：

- 将 LIR output 机械翻译成目标 IR（本阶段为 LLVM IR），然后用 LLVM 编译成 `.o`；
- 生成每个具体类型的 `TypeDescriptor`（runtime spec 2.2：类型标识、实例大小、引用字段位图、父类型表、`equals`/`hashCode`/`toString` 分发入口）；
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
- **泛型成员函数不参与虚分派**：带类型参数的成员函数不进入 vtable / itable（单态化实例无法枚举）；通过 interface 或父类引用调用它是编译错误（HIR 检查，见 spec 3.2），静态类型的直接调用不受影响。

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

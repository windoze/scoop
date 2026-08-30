# Scoop 实现路线图

版本：0.1（草案）

配套文档：`docs/specs/SCOOP-IMPL-SPEC.md`（pipeline 与各 stage 职责）、`AGENTS.md`（编码准则）。

## 1. 策略：纵向主线 + 显式语言子集

不采用"逐 stage 完整实现"的横向推进，而是先打通最小端到端主线，再逐里程碑扩大语言子集。理由：

- **风险前置**：statepoint/stackmap、landingpad、GC pin/handle、suspend 状态机是本项目的新链路，越晚碰代价越大；
- **IR 设计需要下游反馈**：每个 stage 输出的真实需求由其消费者发现，孤立地"完整"实现某个 stage 几乎必然返工；
- **测试基建一次到位**：fixture runner、golden dump、negative fixture 断言在第一个里程碑建好，后续特性直接落入现成框架。

与 AGENTS.md"不留占位符"准则的调和：每个里程碑定义一个**显式的语言子集**，子集内的规则完整实现（结构完备、无 TODO 分支）；子集外的语法由 parser/HIR 报**正式的不支持诊断**（有位置、有信息）——这是面向用户的错误报告，不是代码中的占位分支。随着里程碑推进，这些诊断逐一消除。

## 2. 里程碑

每个里程碑都是全链路可运行的（parser → HIR → MIR → LIR → codegen → 可执行文件）。

### M0 技术 spike ✅（2026-08-21 完成）

- ~~独立一次性程序验证 inkwell 的 statepoint + stackmap + landingpad 全链路（不进主线代码）~~——`spikes/llvm-gc/`（独立 workspace），inkwell 0.10 + LLVM 22.1.8 验证通过：`rewrite-statepoints-for-gc` 正常改写、`.o` 含 `__llvm_stackmaps` 与 `gcc_except_tab`；
- ~~建立 fixture runner、golden dump 设施与 CLI 骨架~~——`compiler/driver/tests/fixtures.rs`（insta 快照），`scoopc` CLI 拆为 lib + 薄 bin（clap），冒烟 fixture `tests/fixtures/m0-smoke/hello.scoop` 端到端通过。

### M1 hello world ✅（2026-08-22 完成，设计见 `docs/milestone1/DESIGN.md`）

顶层函数、`String` 字面量、`fun main`、调用 runtime 的 print。GC 用 always-leak 实现（分配即 malloc、不回收）；单 Cone；不插 statepoint。

### M2 值类型基础 ✅（2026-08-22 完成，设计见 `docs/milestone2/DESIGN.md`）

struct / tuple、字段访问、`val` / `var`、if / while、结构相等。

### M3 泛型与 Option ✅（2026-08-22 完成，设计见 `docs/milestone3/DESIGN.md`）

单态化、`enum Option<T>`（暂为编译器内建）、`T?` 脱糖与 `?.` / `?:` / `!!`（spec 第 7 章）。Option 是核心设施，越早越好。

### M4 enum 与模式匹配 ✅（2026-08-22 完成，设计见 `docs/milestone4/DESIGN.md`）

enum 变体、when 扩展模式、守卫、穷尽性、解构声明与 `..`（spec 第 4、5 章）。同时建立了 sysroot 框架（`sysroot/lib/scoop.core`），`Option` 与 `print`/`println` 的硬编码定义正式迁移入 core 库。

### M5 数组 ✅（2026-08-27 完成，设计见 `docs/milestone5/DESIGN.md`）

`Array<T>` / `MutableArray<T>`（暂为编译器内建）、字面量与推导规则、下标读写、`size`、构造函数形式互转（memcpy 快照）；越界 trap（M8 改异常）。

### M6 引用类型层级 ✅（2026-08-28 完成，设计见 `docs/milestone6/DESIGN.md`）

class / 继承 / interface / 方法、vtable / itable 分派、装箱（spec 3、4.4、9.1；impl spec 2.9）。落地后顺带解锁：数组的 `toArray` / `toMutableArray` 方法形式、core 的 `class StringBuilder` 声明、`add<T>` 依赖的 `toString` 分发基础。

### M7 函数重载 ✅（2026-08-28 完成，设计见 `docs/milestone7/DESIGN.md`）

顶层函数与方法的 overload resolution（候选集分层 + 可应用性 + MSC，按 Kotlin 规范）；`print` / `println` 已迁移为 `scoop.core` 的普通重载定义（三个 `@Intrinsic` 原语支撑）。

### M8 异常 ✅（2026-08-30 完成，设计见 `docs/milestone8/DESIGN.md`）

try / catch / finally / throw，landingpad 落地（runtime spec 第 5 章）。四条 trap 路径已全部改接真实异常：`!!` → `UnwrapException`、数组越界 → `IndexOutOfBoundsException`、`as` → `ClassCastException`、整数除零 → `ArithmeticException`（spec 11.7 已同步新增后者）。`scoop_rt_throw` 按 `__cxa_allocate_exception` + 拷贝的 ABI 正确形态实现。

### M9 真 GC ✅（2026-08-30 完成，设计见 `docs/milestone9/DESIGN.md`）

真 GC 替换 always-leak：Immix 核心（32KB block / 128B line、bump 分配、free-line 复用、标记-区域回收，1 GiB mmap arena）；statepoint 打开（GC strategy + `rewrite-statepoints-for-gc` + safepoint poll + stackmap）；对象头扩为 16B（td + gc_word）；pin（对象头标志位）与 GcHandle 表落地；`scoop.core.gc` 包（暂为 intrinsic）；写屏障卡片表（预偏置指针，为分代预留）；M1–M8 全部 fixture 在真 GC 下原样通过。

### M10 协程

suspend 状态机变换、Continuation（spec 8.2；impl spec 2.3）。

### M11 FFI 注解族

`@Extern` / `@NoGC` / `@Unsafe` / `@Safe` / `@CLayout` / `@CallingConvention` / `@Global` / `@ThreadLocal` / `@InteriorMutable`、`Ptr` / `FunPtr`（spec 第 13、14 章）。

### M12 泛型上界约束与接口化（ToString / Hash / equals）

- 泛型上界约束：`T : Interface` 与 `where` 子句（spec 2.1/3.2 的既有语法落地）、有界类型参数上的方法解析（bounded method resolution）；
- `ToString` / `Hash` 接口落地（spec 11.11）：值类型派生实现，`print` / `println` 改造为 `fun <T : ToString> print(v: T)`（单态化静态分发，退役 M7 的 `Any.toString()` 分发形态）；
- equals 的 operator fun 化（成员限定，spec 11.11）：class 的 `==` 走 `equals` 运算符，值类型的条件派生 `==`；vtable 前三槽（Any 方法）拆除；
- 受益方：M13 字符串插值的 `add<T : ToString>`。

### M13 字符串插值

f-string 与 `StringBuilder` 脱糖（spec 第 6 章，设计见 `docs/milestone13/DESIGN.md`）。低优先级语法糖；`add<T : ToString>` 由 M12 支撑。

### M14 多 Cone 与 `.slib`

`Cone.toml`、依赖图、`.slib` 打包与 reader、三层 meta（impl spec 2.6）、re-export（`public import`）。

## 3. 备注

- 里程碑内的特性验收标准：独立 fixture + 组合 fixture + 相关编译错误规则的 negative fixture + 各 stage 的 golden dump（见 AGENTS.md 编码准则）。
- 里程碑顺序可按实现中发现的依赖调整，但 M0 不推迟、M3 不晚于任何依赖 `Option` 的特性。
- 2026-08-28 顺序调整：字符串插值由 M6 后移至 M12（低优先级语法糖）；引用类型层级提前为 M6，新增 M7 函数重载；原 M8–M12 顺延为 M8–M13。其后（同日）再调整：新增 M12"泛型上界约束与接口化"（ToString/Hash/equals，spec 11.11 已定稿），字符串插值顺延为 M13、多 Cone 顺延为 M14。

## 4. 待补齐清单（backlog）

各里程碑"涵盖但只实现了部分"的事项，按来源里程碑整理。标注→的为目标里程碑（已知时）；未标注的待排期。

### 来自 M1

- always-leak GC → M9 替换（runtime spec 第 3 章已定契约）；
- parser 错误恢复（当前 fail-fast 于第一个错误）；
- 单文件单 Cone 编译 → M13。

### 来自 M2

- struct 字段默认值、命名参数调用、次构造函数（spec 4.1）；
- 副本更新表达式 `s.{ f: v }`（spec 4.5）；
- `break` / `continue` / `for` 循环（`for` 与区间见 M5 行）；
- 定宽整数族 `Int8/16/32/64`、`UInt*`（spec 11.2；`Int` 已固定 i64）；
- 整数溢出语义（spec 未定，需先回 spec 补充）；
- 内建 print 重载 → M7 转为 core 普通重载（设计已含）。

### 来自 M3

- ~~`!!` 失败 trap → `UnwrapException`~~（M8 已完成）；
- `while` 条件中禁用 `?.`/`?:`（诊断拒绝；待 `break` 或循环重组方案，需先回 spec 讨论）；
- `f(None, 1)` 式"先 None 后绑定"的推断（M3 的实参顺序限制）；
- 显式类型实参 `f<Int>(x)`（`<` 消歧方案待定）；
- `value` / `ref` 类型约束（spec 13.9）；
- 非 Unit 函数返回的分支穷尽分析（当前要求函数体以 `return` 结尾）；
- `if` 作为表达式；
- 泛型 **struct** 声明（泛型 enum 已在 M4 完成）。

### 来自 M4

- core 与用户代码同单元编译 → M13 的 `.slib` 与 Cone 隔离；
- 注解仅 `@Intrinsic` 且仅 sysroot → M11 扩展为完整 FFI 注解族；
- 构造函数式变体的默认值只支持常量表达式（完整 spec 8.5"定义处解析、调用处求值"随函数默认参数一起做）；
- `when` 的表达式形态（产生值）；
- 命名字段模式的子模式（`S { f1: 0, .. }` 字面量匹配——ast::FieldPattern 需扩展）；
- 表达式位的裸变体名解析推广到所有 enum（当前仅 `Option` 的 `Some`/`None`；spec 4.2/5.1 的"上下文可确定类型时可省略前缀"在表达式位只对 Option 生效）；
- tuple/struct 的穷尽性按"穷尽模式组合"判定（当前要求 catch-all 或 `else`；spec 5.2/5.3 的组合判定是保守简化）；
- **tagged enum 嵌入 struct/tuple/class 字段时引用偏移不压平**（LIR Plain 布局不记录嵌套 enum 的引用——M9 GC 前必须解决，runtime spec 2.2 已补按 tag 扫描契约）；
- `for` 循环变量与 lambda 参数的解构（随 `for`/lambda）。

### 来自 M5

- `toArray` / `toMutableArray` 方法形式（spec 10.4；方法体系已在 M6 就位）；
- `for` 循环与区间 `IntRange` 等（spec 11.8；含 `..` 区间运算符与 rest 的共存验证）；
- `String` 下标/切片；
- 数组 `==` 语义（spec 缺口，需先回 spec 第 10 章补充）；
- 数组字面量混合引用类型的 LOB 推导（spec 10.3 完整规则）；
- ~~数组越界 trap → 异常~~（M8 已完成）。

### 来自 M6

- 次构造函数、`init` 块、body 属性（非构造函数属性）、`super` 调用；
- interface 的属性与默认实现；
- `equals` / `hashCode` / `toString` 的用户覆写——已改道为接口化设计（spec 11.11）：`equals` 走 operator fun、`ToString` / `Hash` opt-in 接口、vtable 前三槽拆除（→ M12）；
- companion object、`object` 声明、`sealed`、委托（`by`）；
- 可见性修饰符（`internal` 语义 → M13 前）；
- `?.` 后随方法调用（`a?.foo()`）；
- smart cast 完整 flow analysis（当前简化：仅不可变局部变量、仅 `is`/`!is` 与 `&&`）；
- 基类构造委托实参不可引用构造函数属性（`class B(val x: Int) : A(x)` 中 `x` 暂不可用于委托实参——hir-lower 在空作用域降级）；
- class 字段按 8 字节槽索引的约定与连续 sub-8 字段（如两个相邻 `Boolean`）的布局协调（lir-lower/codegen 约定需要随布局一般化复审）；
- 泛型成员函数（静态调用；spec 3.2 虚分派排除规则已生效）；
- `Any` 的 core 库形态（spec 11.1；当前编译器内建）。

### 来自 M7

- 候选集分层的完整层级：局部函数层、显式 import / 星号 import 分层（随相应机制落地后插入；当前为"成员 → 调用点同侧顶层 → 对侧隐式导入"三层）；
- 泛型候选的 MSC 比较改用 Kotlin 的 fresh-variable 约束系统（当前为"推断后类型实参参与比较"的简化，复杂多泛型场景随用例扩展）；
- `write` 的 `@Intrinsic` 退役（→ M11 经 `@Extern` 由 core 普通 FFI 实现）；
- `print` / `println` 的 `Any.toString()` 分发形态为过渡基线（→ M12 改造为 `fun <T : ToString> print(v: T)` 单态化分发，并拆除 vtable 前三槽）；
- 歧义/无匹配诊断的候选明细展示（首版只报主消息）；
- 默认参数/vararg 的决议规则、运算符重载（`operator fun`）、`context` 参数（spec 8.3）——随各自特性落地时补齐。

### 来自 M8

- try 的表达式形态（`val x = try {...}`）；
- catch 遮蔽降级为警告（当前为错误；待警告级别诊断基础设施）；
- finally 内的路径分析（finally 内 return/再抛异常的精细语义）；
- 未捕获异常打印类型名（当前为通用消息；需要 TD 增加 name 字段，runtime spec 2.2 同步）；
- 异常穿越 Scoop ABI FFI frame 的规则（维持 runtime spec 第 5 章的暂定“初版禁止”）。

### 来自 M9

- 分代（nursery、晋升、remembered set 消费卡片表、代间引用检查）；
- evacuation / defragmentation（Immix 的碎片整理；arena 扩容与多段管理）；
- 并行/并发回收与 STW 线程协调（线程注册/握手，safepoint poll 已是握手点形态）；
- x86_64 栈扫描汇编（当前仅 aarch64，v1 为保守栈扫描；statepoint 精确栈扫描替换点已在 gc.c 预留）；
- 异常 ABI 缓冲的精确释放（v1 为 pin 原对象 + thrown 列表登记的保守近似）；
- tagged enum 的 per-variant 扫描表发射（SCOOP_REFS_ENUM 的 LIR meta 扩展；当前保持 M4 边界）；
- hir-lower 的泛型 struct 字段类型形参作用域（当前字段里的 T 需要进一步支持；core GC struct 的按名识别 stopgap 可摘除）；
- 其余定宽整数族（Int8/16/32、UInt8/16/32，spec 11.2；UInt/UInt64 已落地）。

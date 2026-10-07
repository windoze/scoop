# M32 设计：Any / Nothing 的源码定义与顶底类型闭环

状态：已完成。设计、源码定义、编译器实现及第 7 节验收均已完成，实际平台、覆盖范围与结果见[验收记录](ACCEPTANCE.md)。

日期：2026-10-07。

基线：M31 已完成的编译、普通依赖与产物链接、debug/release 和分代 GC。对应[语言规范](../specs/SCOOP-SPEC.md) 3.1、4.4.4、8、11.1、13.1，[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.2～2.4、2.10～2.12，以及[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 2.2、2.6。历史 M6/M8 对编译器内建 Any 和缺少 Nothing 的记录由本设计替代。

## 0. 交付目标

M32 同时交付真实 core 源码声明和支撑它们的编译器行为。用户能够从重建后的 core 导入、使用、发布并再次消费 Any/Nothing；单纯增加 Rust enum variant、文档名字或没有被编译的 Scoop 文件不构成完成。

1. Any 从没有源码声明的内建根迁移到真实的 core intrinsic class，保留现有顶类型、装箱、identity 与无成员语义。
2. Nothing 成为具有实际 nominal identity 的源码底类型，完成签名、子类型、结果合并、转换和无正常返回控制流。
3. 普通依赖、泛型／默认值、函数值及其动态类型关系、异常／finally、GC 和产物链接使用相同声明和规则。

不新增根对象方法、用户自定义 intrinsic、来源授权或另一个类型检查框架。根类型的特殊关系由已有 intrinsic 机制承担，普通成员、ABI 和格式仍通过既有通道表达。

## 1. 当前缺口

| 基线现状 | 必要改动 |
| --- | --- |
| 语言规范 3.1/11.1 只有 Any/Nothing 的简短语义条目 | 给出正式源码形态、构造限制、底类型求值与 ABI 规则 |
| `sysroot/lib/scoop.core/src` 没有两个声明 | 增加参与正常 core 构建的 `roots.scoop` |
| Any 在 HIR 为特殊类型，名称解析在普通查询失败后直接返回内部 ID | 名称来自普通 public binding，保留必要的内部顶类型分类 |
| Any 的共享声明与 machine shape 有“没有源码 arena entry”的专门合成分支 | 让实际声明进入普通名义类型、发布和依赖查询；删除无声明兜底 |
| Nothing 尚未进入编译器；throw 只通过专门语句表示终止 | 让真实结果类型驱动调用与表达式的正常完成分析 |
| 结果合并排除 return/throw，但普通调用一律视为可能返回 | 将 Nothing 求值接入现有控制结果集合与 MIR CFG |

主要起点为 [intrinsic 类型](../../compiler/hir/src/declarations/nominals/intrinsics.rs)、[core 契约](../../compiler/hir-lower/src/core_contract/intrinsics.rs)、[类型关系](../../compiler/hir-lower/src/types/relations.rs)、[控制流分析](../../compiler/hir-lower/src/stmt/flow.rs)、[MIR 表达式规范化](../../compiler/mir-lower/src/cfg/expression.rs) 和 [runtime 类型描述](../../compiler/lir-lower/src/metadata/descriptors.rs)。这些是实施定位，不是要求建立平行实现。

## 2. core 源码与声明身份

`sysroot/lib/scoop.core/src/roots.scoop` 正式包含：

```scoop
@Intrinsic("core_any")
public abstract class Any {}

@Intrinsic("core_nothing")
public final class Nothing {}
```

两者都是无类型参数、无源码构造入口的 intrinsic class。登记表分别规定源名称、class target、Any 的 abstract 与 Nothing 的 final，以及不允许字段、成员、父类型、companion 和 release block。错误形状在源码位置报错；不从无字段推导空对象或零大小值。

抽象 Any 表达可承载其他类型引用的根；final Nothing 表达没有可构造实例的底类型。它们不是供用户显式继承的普通基类。类型系统自动给出相应顶／底关系，普通继承列表中写 Any/Nothing 是错误。`Any()`、`Nothing()` 和相应别名的构造调用均报错。

声明必须具有实际 source identity、nominal identity、public export binding 和 ordinary declaration metadata。core 定义边界各要求一个登记项；消费方从实际选中的 core 产物取得它们。缺失、重复、错误类型形状由现有声明／格式边界报告，不生成替代声明。

名称查找遵守现有规则：core prelude 最低优先级，显式 import、alias 和用户同名声明按普通规则处理。普通 `class Nothing` 不会成为底类型。编译器生成的失败根、动态 closure 适配等引用已解析的实际根声明，不能被使用处同名类型替换。

## 3. 顶类型与底类型行为

### 3.1 Any

所有类型都是 Any 的子类型。引用提升保留原对象，单个值提升按现有规则装箱；数组字面量的批量位置仍不自动装箱。Any 没有 equals、ToString、Hash 或其他成员，不增加 vtable 固定槽。`===` 比较已有对象身份，`==` 仍需要静态类型上的成员 equals。

### 3.2 Nothing

Nothing 是所有类型的子类型，但没有一个表达式能正常完成并提供 Nothing 值。`fun fail(): Nothing { throw ... }` 是普通源码函数；`fun bad(): Nothing {}`、裸 return 和返回普通值均非法。已有专用 throw/return/break/continue IR 可继续表达控制转移，不要求为了统一名称把每个控制语句改写成函数。

Nothing 的 class 类别属于 ref kind，不能构造有效引用。它可写在参数、返回、字段、alias 和泛型实参中；这些位置不因此获得一种默认值。包含该类型的构造只有在全部实参正常求值后才能完成，因此不能制造不可居留的 payload。

从 Nothing 到任何类型的适配没有正常值操作。保留原表达式求值及异常／挂起行为，禁止补 Unit、null、零值、box 或转换结果。原表达式的实际类型和调用签名必须保留。

条件、when guard 和 `&&`／`||` 的 Boolean 位置接受 Nothing。短路运算只在所选分支上求值；泛型替换使 receiver 成为 Nothing 时，保留 receiver 求值，不为不可达的成员调用生成接口实现或继续求值实参。

无 expected type 时，`LUB(Nothing, T) = T`，所有分支均无正常结果时取 Nothing。普通 nominal 泛型继续不变：`Option<Nothing>` 不是 `Option<Int>` 的子类型；`None` 的上下文推断规则不变。不能为无解的泛型参数随意填 Nothing。

元组字面量的 Nothing 元素可以使用对应的预期槽位类型；数组字面量推断时 Nothing 不限制其他元素的类型，也不为无解的泛型元素提供默认类型。两者都保留原元素求值；遇到 Nothing 后不求值后续元素或完成构造。已有元组和数组的类型关系不因此改变。

### 3.3 检查与转换

`e is Nothing` 和 `e !is Nothing` 在先求值 e 后分别为 false/true。`e as Nothing` 求值后抛出现有 ClassCastException；`e as? Nothing` 返回 Option<Nothing> 的 None。常量结果不允许省略源表达式的副作用、异常或挂起。

Any 的检查、拆箱与原 exact type 身份不变。Nothing 不需要成功拆箱路径或对象分配入口；不能把某个 AbstractRef 对象伪装为 Nothing 实例。

### 3.4 函数、求值顺序与清理

直接调用、成员调用、函数值、默认实参以及跨 Cone／泛型实例均以实际结果类型判断能否正常返回。receiver 与实参严格按既有顺序求值；某一步无正常结果时，后续实参和外层调用不执行。

跨 Cone 默认模板保留参数的预期类型及正文的实际类型。例如 `<T> demand(value: T = fail())` 的模板结果契约为 T，正文调用仍返回 Nothing。源码类型检查和提供方参数契约各在原边界完成，格式构造器不以类型键必须相等取代底类型规则。

`Nothing` 不等于 NoGC、不抛异常或进程退出。调用可以分配、抛出后被外层 catch 捕获；finally、Context 恢复和 native root 生命周期沿原异常边执行。只有调用的正常出口不可达。

普通 `() -> Nothing` 按结果协变可用于 `() -> T`；函数值转换及动态检查保留底类型关系。挂起函数仍按 `Continuation<R> -> CoroutineStep<R>` 的既有 ABI：`suspend () -> Nothing` 可返回 Suspended，不能成功产生 Completed 的 payload，不能给整个 lowered callable 错加 LLVM noreturn。

## 4. 编译器与产物落点

| 层次 | 交付与输出不变量 |
| --- | --- |
| core / AST / parser | 实际 `roots.scoop`，复用 intrinsic class 与普通类型语法；诊断保留真实源码位置 |
| HIR 声明与导出 | 两个声明具有实际 owner、application、binding 与表示；内部分类不替代声明 |
| hir-lower | 普通名称查询、缺失与形状诊断；底类型子类型／LUB／转换／返回检查；无正常返回求值顺序 |
| LocalConcreteHir | 默认值、模板、函数适配与依赖保留原根身份及完整结果类型；不伪造成功结果 |
| MIR | 底类型求值后明确结束正常 CFG 边；保留 EH、finally、suspend 与 Context 关系 |
| LIR / codegen | 不可分配的根 TD、普通引用 ABI 和精确 GC；底类型正常后继不可达，保留合法异常边 |
| metadata / slib / linker | 发布、读取、再次发布与独立链接能查到真实根声明、表示和必要 TD；按实际编码升级版本 |

Any 保留专门的 `Type::Any` 顶类型分支，其身份来自实际 core class；Nothing 使用普通 `Type::Class` application 和封闭的 `IntrinsicTypeRepresentation::Nothing`，复用 nominal identity 和泛型通道。成功输出必须保留两个实际声明，不以显示名字反推语义。

现有 Any 的 fixed-key、无源码声明、合成 public shape 和原 nominal 查询豁免必须逐一核对。能够由实际声明提供的记录改走普通通路，编译器主动使用的角色引用只保存对该声明的 typed reference；不能保留“删除 roots.scoop 仍能编译”的降级路径。

共享 IR/meta crate 只表达数据、格式和引用约束。源码的成员、bound、完成性质只在前端确定，不在 reader 或后端再建一套语义检查器。

## 5. 表示、runtime 与兼容性

Any 的值仍为指向原动态类型对象的 managed reference；Nothing 没有合法的非空引用。两个根的 TD 使用 AbstractRef，不能分配。已有 Any 在函数关系中的顶类型表示可保留；Nothing 必须以实际 TD 和明确底类型关系参与结构化函数检查，不能与普通 interface 或 AbstractRef 混同。

Nothing 的物理签名允许使用既有引用 carrier，但不存在成功返回或消费该 carrier 的执行路径。不能用 UnitVoid 或 ElidedZst 冒充其源码语义。MIR/LIR 在求值后结束正常路径，LLVM 只消费这个已经确定的事实；是否附加 noreturn 优化属性不能改变 EH 和挂起协议。

Option<Nothing> 可以持有 None；零长度 Array<Nothing> 可以存在。为这些真实语言组合复用现有引用 niche／元素布局，不引入新的通用居留性或布局优化框架。非空 payload／元素必须经过不可能正常完成的 Nothing 求值，不能以零初始化制造一个有效值。

新增 intrinsic 表示、core 协议字段和 runtime 类型关系编码后，以下版本随实际编码一起提升，旧 core／依赖和缓存按正常版本规则重建。源码身份不因 debug/release、host 路径或消费者而改变。

| 契约 | M31 → M32 | 变化 |
| --- | --- | --- |
| HIR core-bootstrap-interface | 11 → 12 | fundamental types 增加实际 Any/Nothing 角色 |
| HIR cross-cone-interface | 60 → 61 | 根 intrinsic 表示、默认值及调用中保留实际底类型 |
| HIR cross-cone-type-semantics | 22 → 23 | 根表示及对应的类型关系 |
| MIR cross-cone-type-bridge | 15 → 16 | 实际根来源与 intrinsic 表示 |
| LIR cross-cone-layout-abi / cone-production | 10 → 11 | 不可分配的根描述符及 Bottom 关系 |
| runtime metadata ABI | 5 → 6 | TypeDescriptor relation kind 4 表示底类型 |
| RuntimeAbiContract | 9 → 10 | runtime 类型关系合同更新 |

root TD 均为 AbstractRef，不产生 class object storage。Nothing 的 Bottom 关系没有 operands 或 result；函数 TD 引用它表示结果协变或参数逆变。旧版拒绝、固定 wire 编码和派生指纹测试随上述变化更新。

## 6. 实施批次

| 批次 | 交付 | 验收出口 |
| --- | --- | --- |
| M32-1 源码根 | 两个 intrinsic 登记项、实际 core 声明、Any 普通导出与 Nothing nominal 表示 | 重建 core；导入、alias、真实声明查询；错误形状与构造诊断 |
| M32-2 底类型求值 | 子类型、结果合并、调用完成、转换和 CFG | Nothing 返回函数、分支／实参／默认值、catch/finally；HIR/MIR/LIR golden |
| M32-3 组合与产物 | 函数值、泛型、Option/数组、依赖再次发布和 runtime 类型关系 | A→B→C 与 artifact-only 链接运行；移动 GC 与 debug/release |
| M32-4 回归与收尾 | 格式／缓存迁移、旧 Any 兜底清理、正式测试和实际记录 | 所有必需 fixture 通过，明确平台范围及实际结果 |

每批代码先 fmt/clippy 再测试；若实际实现揭示规范空白或冲突，先修订本设计和对应 spec。状态以真实代码和测试为准，不以文档存在、Rust 编译通过或内部 ID 可构造代替交付。

## 7. 验收矩阵

| 类别 | 必需验证 |
| --- | --- |
| 源码声明 | core 普通编译包含两者；删除／重复／错误 intrinsic target、modality、字段、成员、继承与构造均给出源码诊断 |
| 名称与身份 | 显式导入、alias、用户同名类型、重导出；将两个根移到另一 package 并由根 package 的 public alias 公开，重建 core 后消费者只使用实际声明 |
| Any 回归 | primitive/struct/enum/tuple 装箱、引用上转型、is/as/as?、identity；Any 没有 equals/ToString/Hash |
| Nothing 基础 | 参数／结果／alias、Nothing 函数抛出并由调用方捕获；非法正常返回／落空、构造与显式继承 |
| 求值与控制流 | receiver／多个实参／默认值顺序，分支 LUB、全终止分支、返回转发、finally 覆盖与 Context 清理 |
| 类型组合 | 不变泛型、Option<Nothing> 的 None、空数组、函数结果协变、动态检查、挂起而不成功完成 |
| 转换 | is/!is 的常量结论仍求值；as 抛出；as? 为 None；不分配底类型 box 或制造 payload |
| 产物 | 参数自由和泛型 A→B→C、再次发布、artifact-only 链接／运行、错误或旧格式诊断 |
| ABI / GC | Any 引用与含引用 box 跨 safepoint；Nothing 的异常边、minor/full moving、debug/release；挂起 ABI 不误标 noreturn |
| golden | 两个真实 source nominal、typed representation、Nothing call 的不可达正常边及保留的异常／清理边 |

正式完成门为 `cargo fmt --all`、`cargo clippy --workspace --all-targets`、适用 workspace 测试，构建 `scoop`／`scoopc`／`scoop-linker`，公共 runner 单元测试及 `python3 tests/run_fixtures.py --all`。Python 改动遵守 AGENTS.md 的固定 ruff 版本。新 fixture 使用现有 schema，不建立 M32 专用 runner。

Darwin/AArch64 执行完整适用回归；Linux GNU/musl 使用现有可用环境验证受影响的源码／产物／异常／函数适配与 GC 组合。记录实际 target、profile、命令、覆盖范围和结果，不把交叉编译或未运行的平台写成通过。2026-10-07 已按上述范围完成验收，实际记录与普通模式报告见 [ACCEPTANCE.md](ACCEPTANCE.md)。

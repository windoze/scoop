# 包限定表达式与函数声明引用设计

状态：实施中。日期：2026-10-28。currentSourceLocation 的不可引用诊断及错误下的声明导航已实现；包限定表达式仍待实施。验证记录见[实施记录](../compile-time-intrinsics/PROGRESS.md)。

权威合同见[语言规范](../specs/SCOOP-SPEC.md) 8.1.4～8.1.5、11.12、12.4.5、13.1、13.10，以及[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.2.1、2.10。本设计与[编译期 intrinsic 设计](../compile-time-intrinsics/DESIGN.md)配合，但适用于所有可见 package 的普通声明。

## 1. 当前缺口与目标

当前 Scoop 已支持 import selector 和限定类型路径，但顶层函数／值表达式不能直接通过 package 路径定位。`demo.api.answer()` 仍会把 `demo` 当普通表达式名称；导入 `demo.api.answer` 后调用 `answer()` 才可用。`import demo.api` 也不表示导入 package 对象：exact import 必须终止于可导入的 binding。

函数引用的 parser 在 `::` 后只读取一个标识符；AST 保存可选 receiver 和单个 name。因此不能只补普通函数调用：`::demo.api.answer` 也必须作为完整声明引用进入解析，而不能被拆成 `::demo` 的字段访问。

目标是让原本可通过 import 短名访问的声明，在相应位置也能通过完整 package 路径定位。目标的类型、调用、访问和初始化规则保持一致；package 只参与名称解析，不成为运行期值。

## 2. 覆盖范围

| 入口 | 示例与语义 |
| --- | --- |
| 顶层函数调用 | `demo.api.f(args)`、`demo.api.f<T>(name = value)`；支持原默认参数、vararg、spread、尾随 lambda |
| 顶层属性／常量读取 | `demo.api.VALUE`、`demo.api.state`；沿原 const、getter、初始化规则 |
| 属性赋值与更新 | `demo.api.state = value`、`demo.api.count += 1`、`++demo.api.count`、`demo.api.count++` |
| 值的后续操作 | `demo.api.callback(args)`、`demo.api.values[index]`、`demo.api.optional?.member()`、已有 struct 的 copy update |
| 构造与 enum 变体 | `demo.api.Type<T>(args)`、`demo.api.Alias(args)`、`demo.api.Enum.Variant(args)` 与 unit variant |
| object／companion／嵌套声明 | `demo.api.Singleton`、`demo.api.Type.Companion.member`、`demo.api.Box<Int>.create(value)`、`demo.api.Outer.Nested(args)` |
| 函数声明引用 | `::demo.api.f`；重载、普通／挂起函数和泛型实例化沿原期望类型规则 |
| 构造器引用 | `::demo.api.Type`、`::demo.api.Alias`；仍只接受存在合法普通构造入口的目标 |
| 未绑定扩展函数引用 | `::demo.api.extension`；接收者成为函数类型的首参数 |
| 成员引用的限定左侧 | `demo.api.Type::method`、`demo.api.Type<T>::method`、`demo.api.Singleton::method` |
| native 地址上下文 | `val fp: FunPtr<(Int) -> Int> = ::demo.api.callback`；沿原 NoGC、签名及目标条件 |
| 合法 place 的地址 | `addressOf(demo.api.rawGlobal)`；只扩展定位，不使普通 property 或 const 变得可取址 |

属性取得函数值后可调用；函数声明名本身仍不能省略 `::` 隐式变成函数值。安全访问、下标、更新和 copy update 都先取得真实值，再执行原操作，不对 package 生成运行期求值。

类型注解、返回类型、类型实参、bound、typealias、继承列表、扩展 receiver 类型、is／as、注解名及 enum 模式前缀中的既有限定路径继续沿原规则。本次对这些位置做回归，不另建一套类型或模式名称解析。

## 3. 名称解析与遮蔽

### 普通点表达式

1. 先按原有表达式规则查询首段的普通值、类型和其他可用限定目标，保留局部绑定、import alias、隐式 receiver 等现有查找层次。首段已经绑定时，后续路径沿该实际目标解析。
2. 只有首段没有普通限定目标时，才查询可见 package 的最长前缀。可见集合沿语言规范 12.4.1：当前 Cone、直接依赖的公开 binding 及 re-export；support dependency 不扩大源码可见域。
3. 最长 package 前缀一旦选定，不因后续成员缺失、不可访问或不适用而回退到较短 package。普通首段已经绑定但后续失败时，也不改用同名 package 重猜。
4. package 后通过真实 static owner edge 和 namespace 候选定位目标，再按当前表达式需要执行类型、值、调用或 place 解析。遇到真实值后，其余链段进入原成员规则。

例如局部 `val demo = ...` 存在时，`demo.api.f()` 继续表示该值的成员链。新增一个名为 `demo.api` 的依赖 package 不应夺走原表达式；需要避开这一遮蔽时可以显式 import 函数并设置 alias。

相同最长 package 中来自不同 Cone 的 binding 按实际 typed origin 合并、去重，并沿正常候选选择与歧义规则处理。FQN 不能选择特定版本／来源，不能绕过 private、internal 或生产／测试来源限制。源码字符串不是实体身份。

### 前置声明引用

```text
declarationReference := "::" identifier ("." identifier)*
memberReference      := receiver "::" identifier
```

此处 identifier 沿既有词法规则。单段 `::name` 保留当前短名解析；多段选择器优先按最长可见 package 前缀继续解析，没有 package 前缀时从可见静态类型／alias owner 起步。选定前缀后失败不回退重猜，也不把局部值当 receiver 求值。它与普通点表达式的值遮蔽分支分开，但复用相同的可见包、static owner 和末端声明查询。

`::demo.api.f` 的全部点分名称属于声明路径。若要操作引用结果，显式括起整个引用，例如 `(::demo.api.f)(value)`、`(::demo.api.f).invoke(value)`。原来将 `::f.member` 理解成引用结果成员访问的代码应改写为 `(::f).member`；formatter 不得删除用于确定结合关系的括号。

后置 `receiver::member` 右侧保持单个成员名。左侧可以通过 package 路径定位类型、object 或普通值；类型产生未绑定引用，真实值产生绑定引用，后者在创建时求值一次并保存。`demo.api.Type.Companion::method` 同样使用真实 singleton receiver。

`::demo.api.Type` 可以选构造器；`::demo.api.Type.method` 不用于隐式构造或绑定成员 receiver，真实成员引用仍写 `demo.api.Type::method`。package 本身不是 receiver，不支持 `demo.api::f`。单段与多段声明引用都不增加 property reference：函数类型属性直接读取 `demo.api.callback`，`::demo.api.callback` 仍报错。

泛型函数／构造器引用继续通过期望函数类型确定完整实例化；本次不增加 `::f<T>` 或 `::demo.api.f<T>` 语法。类型作为成员限定符的既有实参语法保持，例如 `demo.api.Box<Int>::get`。扩展引用继续把接收者放在首参数位置，不同时引入 `demo.api.extension(receiver, args)` 形式的扩展调用。

## 4. 与现有语义组合

### 调用、属性和初始化

限定调用和 import 短名调用选择同一声明，并保留命名／默认参数、vararg、参数求值顺序、重载、泛型约束、suspend 与 unsafe effect 检查。泛型正文和默认参数只保存已选 typed 身份，下游不根据 FQN 文本重新查找。

属性读写使用既有 getter／setter、可见性、delegate 和存储类别。复合赋值、前后自增及下标更新保持原 receiver／index／getter／setter 的求值次数和顺序；不能为了尝试不同名称路径执行多个读取。普通 object 和 companion 值仍在原访问点确保初始化；const 读取本身仍不初始化 singleton。

### managed 函数值与 FunPtr

普通／挂起函数类型上下文继续生成原 managed callable reference；无期望类型时也不自动推导 FunPtr。期望为 FunPtr 时，`::demo.api.callback` 属于无 receiver 声明引用，直接进入 native-address resolution，不先创建 closure。

FunPtr 仍要求非泛型、非挂起、非 extern、具有普通 Scoop 正文的 NoGC 顶层函数及精确 C-safe 签名。成员、扩展、局部函数、构造器、已有函数值和绑定引用不因带 package 前缀变得合法。

### 编译期 intrinsic

通用能力完成后可以直接使用 `scoop.compiler.env("SCOOP_BUILD_TAG")`、`scoop.compiler.includeStr("../LICENSE")`；四个入口都沿解析后的同一 registry kind 展开成常量。include 路径及 symlink 目标仍须在定义 Cone 内，includeDir 逐项检查；env 首版只接受精确 SCOOP_ 前缀，使用 FQN 不改变这些输入限制。core 仍不会把该 package 加入 prelude，未 import 的裸 `env` 仍不可用。

`::scoop.compiler.env`、`::scoop.compiler.includeStr` 等是合法的声明引用语法，但目标 intrinsic 不支持引用，属于 HIR 语义错误。短名和限定名一致：这些操作在编译期求值，没有运行期 callable 入口，不能生成 managed 函数值或 FunPtr。无需为这组 intrinsic 建立特别的 FQN 查找路径。

### 不可引用 intrinsic 的诊断

`currentSourceLocation()` 也只允许直接调用；当前 core 中的正式名称就是 `currentSourceLocation`，本次不改名或迁移 package。它依赖调用表达式的 evaluation origin，普通函数值的间接调用不传递动态调用点源码位置。因此 `::currentSourceLocation` 必须拒绝，不能悄悄返回引用创建点、生成 adapter 的位置，或依靠调试信息补偿。

禁止引用由解析后的 typed intrinsic kind 决定。先按正常规则解析名称与可见性，再保留该候选的专用失败原因；短名、import alias、re-export、FQN、本地／依赖以及 managed／无期望类型／FunPtr 上下文一致。不是按 `env`／`currentSourceLocation` 字符串禁用同名普通函数，也不把所有 intrinsic 都禁用。多候选继续按原规则选择，并在失败诊断中保留不可引用的具体原因；不能静默删除该候选后只报告找不到函数、无匹配重载或签名不符。

诊断主位置覆盖原引用表达式，名称显示实际解析到的声明；使用 alias 时可同时标出源码名称与实际目标。期望的信息如下，英文沿编译器现有诊断风格：

```text
error: compiler intrinsic `scoop.compiler.env` does not support callable references
note: this intrinsic is evaluated at compile time and has no runtime callable entry
help: call `scoop.compiler.env(...)` directly with a constant name starting with `SCOOP_`
```

```text
error: compiler intrinsic `currentSourceLocation` does not support callable references
note: this intrinsic requires the source location of a direct call; indirect calls do not provide it
help: call `currentSourceLocation()` where the location is needed, or pass a `SourceLocation` value explicitly
```

直接调用仍然合法，包括显式 lambda 或普通函数正文中的直接调用；它们沿原求值来源规则取值，不自动观察未来的间接调用者。位置 intrinsic 的默认参数调用仍按语言规范 8.5、11.12 取得求值来源。诊断不得自动改写为 lambda／adapter；普通包装函数是用户显式编写的正文，语义来自其中实际调用，不能充当原 intrinsic 引用的透明替代。

实现复用现有 IntrinsicFunctionKind，在 HIR 的引用候选检查中封闭匹配上述五个 kind 及拒绝原因；不需要新的源码注解、持久能力字段或通用策略框架。检查先于 managed adapter 和 native FunctionAddress 构造；LSP 保留真实声明绑定以支持跳转，不把语义错误退化成不完整语法。

## 5. 编译器改动面

| 层 | 必要改动 |
| --- | --- |
| parser／AST | `compiler/parser/src/expr/callables.rs` 与 `compiler/ast/src/expressions.rs` 区分前置非空声明路径、后置 receiver 加单名；保留逐段 span |
| 语法树／格式化 | `compiler/parser` 的恢复解析和 `compiler/syntax`、`compiler/fmt` 保留完整引用、泛型调用及括号结合；不破坏编辑中的不完整路径 |
| 名称解析 | 在 `compiler/hir-lower/src/imports/` 的可见 package、候选和 owner 查询上补齐表达式入口，保留 `lookup/calls/qualifiers.rs` 的普通首段遮蔽 |
| 表达式 lowering | `expr/members.rs`、`expr/imported_namespaces/` 及既有 call、place、构造路径消费已解析目标；不逐个复制 package 查找算法 |
| callable reference | `expr/callable_references.rs` 及子模块接入完整声明目标；native 分支按是否真实 receiver 区分，不把 package 编码成 receiver；两种上下文共享不可引用 intrinsic 的具体失败原因 |
| LSP | `compiler/lsp` 与 HIR editing 数据保留每段 namespace／声明来源，支持诊断、补全、签名、导航、引用和重命名 |

AST 保证声明路径非空；成功 HIR 仍使用完整的 typed call、property、place、构造或引用，不携带待解析 selector、package 伪值或备用 FQN。既有 type／annotation／pattern 查询不套用普通表达式的值遮蔽策略。

此项主要改变 parser 和 HIR 名称解析，MIR／LIR／codegen 继续消费原已选操作。没有新的 runtime ABI、初始化协议、链接规则或实体身份体系；只有实际持久格式发生变化时才升级相应格式，避免为语法路径长度引入另一套表示。

## 6. 实施与验收

1. 补齐前置声明路径的 AST、parser、恢复语法和 formatter，确定与 postfix 的结合规则。
2. 在既有 namespace 查询中实现普通点表达式的 package 分支，接通 call、value、place、构造、variant 和 object／companion。
3. 接通构造／函数／扩展声明引用、限定成员引用和 FunPtr 上下文；补齐编译期输入／位置 intrinsic 的不可引用语义诊断，沿同一目标完成 LSP 查询。
4. 完成下表验收，并与编译期 intrinsic 组合验证。通用 package 解析可先使用普通库测试，不依赖新 intrinsic 已落地。

| 范围 | 必须覆盖 |
| --- | --- |
| parser／formatter | 前置多段引用、后置引用、结果括号、调用／下标 postfix、显式调用类型实参、尾随 lambda、不完整路径恢复、格式化幂等 |
| 名称解析 | 无 import、exact／star／alias 并存、局部值与类型遮蔽、最长包不回退、同包多 Cone 合并／歧义、嵌套声明与 typealias |
| 调用与构造 | 普通／泛型／重载、命名与默认参数、vararg、普通函数值 invoke、构造及 enum variant、object／companion 转发 |
| 属性与 place | const、普通／computed／delegated property、可见 setter、复合赋值、自增、下标更新、合法 raw storage 的 addressOf；验证求值顺序和次数 |
| 引用 | managed／suspend／泛型期望类型、构造器、未绑定扩展、绑定与未绑定成员、虚分派、创建点 receiver 快照 |
| native 上下文 | 合法限定 NoGC callback；拒绝 generic／suspend／extern／member／extension／局部函数、错误 C-safe 签名及已有函数值 |
| 访问与诊断 | 未声明直接依赖、private／internal、support-only 声明、包作为值／import 终点、错误路径段、property reference 与无运行期 intrinsic 引用 |
| intrinsic 引用诊断 | 四个编译期输入及 currentSourceLocation：短名／alias／re-export／FQN、managed／无期望类型／FunPtr；断言引用位置、真实名称与具体原因，不接受 parser／unknown name 泛化错误；同名普通函数和原合法 intrinsic 引用回归 |
| 位置 intrinsic | 直接调用、默认参数、显式 lambda／普通包装函数保持原 evaluation origin；拒绝引用而非捕获创建位置；LSP 语义错误下仍可导航 |
| 既有路径回归 | 限定类型、alias、bound、注解、is／as、enum 模式；原短名和 receiver 形式 |
| 产物与工具 | 跨 Cone 泛型／默认值保留真实绑定；LSP 每段导航、引用与重命名；scoop.compiler 四个限定直接调用 |

普通语言行为用独立测试 Cone 和 scoop.test，采用 release。编译错误／诊断位置、FunPtr ABI 及跨产物身份使用对应编译器测试或正式 fixture；少量 parser／typed HIR 测试检查路径和已选身份，不保存完整 IR 快照。完成代码变更后先格式化／lint，再运行相关测试。

当前编译器仍不能使用本节新增的包限定表达式；不以文档中的规划示例作为已实现能力。

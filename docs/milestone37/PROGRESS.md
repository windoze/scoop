# M37 实施记录

基线：`ec9d8849d`（M36，2026-10-11）。工作分支：`codex/m37`。既有未跟踪设计稿为本次实现范围，随首项功能纳入版本控制；本文件只记录实际完成和验证的工作。

## 实施与验证约定

每项功能先同步权威规范，完成 parser/HIR、必要后续阶段与产物路径，再运行独立、negative 和组合 fixture，完成后单独提交。遵守范围纪律，不引入授权、防伪、通用预算或假想扩展框架；按实际职责控制文件长度。

Rust 每批先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再运行相关 Rust 测试并构建配套 CLI。本机使用 `--release` 构建配置，复用既有产物并避免与编辑器的 debug 测试枚举争用构建锁。本机定向验证通过后，在 `nuc12:~/repos/scoop` 完成 Linux 全量 Rust 与正式 CLI 验收，并验证 musl 受影响路径；用户已明确允许更新该目录。不重复执行无关全量。

构建及测试使用仓库 `target/m37-tmp` 作为 TMPDIR；日志、报告与临时脚本保存在 `target/m37-validation`，不使用 `/tmp`。定期清理已验证的 fixture 工作区和过期编译缓存，保留可复用依赖、工具及报告。开始时本机 target 约 20 GiB。

## 功能进度

### 1. 兼容修饰符与语言边界（完成）

规范固定五个修饰符完全忽略，明确禁止 interface 方法自身泛型、泛型虚方法及外层局部 var 捕获；runtime 契约和 wire 编码无需变化。

parser 在函数、property/accessor、值参数及类型参数的合法位置消费修饰符，保留语法诊断所需位置后丢弃，不增加 HIR/LLVM hint。支持混合/重复标记、非递归 tailrec、无 inline 的 reified、constructor/enum/annotation 参数及局部函数；同名普通参数、函数和 variant 继续合法。共享现有 vararg 解析，语法辅助模块 88 行，减少函数与 nominal 文件中的重复代码。

本机 fmt、workspace clippy（release）通过；parser 479 项与 doc-test 1 项通过。12 个正式 CLI fixture 普通比较全部通过，15 个配置、23 个进程、18 个 HIR/MIR/LIR golden；包括 debug/release、接口宿主泛型、final 泛型方法、显式引用对象状态、异常/finally 与 moving GC。9 个 negative fixture 固定真实诊断位置和完整信息。快照已检查普通分派、初始化和捕获结构；release 的内联仍来自既有优化器，与兼容标记无关。

普通报告见 [01-modifiers-darwin.json](validation/01-modifiers-darwin.json)。生成快照不计入普通验证次数。完成后清理本批 fixture cases，保留共用缓存、sysroot 和报告供后续增量验证。

### 2. 单语句控制体与 else-if（完成）

if/else/while/for 使用同一个控制体入口，单语句形成保留源码位置的普通 block，沿用现有 HIR 作用域和 MIR 控制流。直接 else-if、最近 if 结合、else 前分号、无值 return 的 else 边界和 body 内尾随 lambda 均闭合；没有新增后端分支或 wire 形态。控制结构 parser 170 行，新测试文件 59 行。

本机 fmt、workspace clippy（release）通过；parser 482 项及 doc-test 1 项通过。8 个新增 CLI fixture 的普通比较全部通过，11 个配置、17 个进程、18 个阶段 golden，涵盖 debug/release、循环跳转、嵌套 if 值、Elvis return、异常/finally、lambda 与作用域错误；另有两个 M35 尾随 lambda 正式回归通过，2 个配置、6 个进程、8 个 golden。两条旧 parser 负例已按新规范替换为条件括号检查及已有正向覆盖。

报告见 [02-control-bodies-darwin.json](validation/02-control-bodies-darwin.json) 和 [02-regression-darwin.json](validation/02-regression-darwin.json)。已审阅嵌套 if 的正常值合流及原循环条件入口，原 M35 快照无需迁移。

### 3. Unicode 与反引号标识符（完成）

规范固定 Unicode XID 与非空反引号名称，以实际码点序列比较，不做规范化；标识符、package、注解路径、成员与跨 Cone 名称均保留原有分段。使用持续维护的 unicode-ident 1.0.26（核对 crates.io 更新日期为 2026-09-17），没有自建 Unicode 字符表。lexer 区分关键字与带引号 token，AST 只保存实际文本；持久化名称继续用原有 UTF-8 text 编码，已有合法实体身份不变。

注解改用已有非空 QualifiedNameSyntax，删除按点号恢复路径的逻辑；成员查找与 override 使用真实声明名称，限定 enum 构造沿用原有 MethodCall AST，移除重复的拼接名称构造入口。访问器使用已有类别区分，允许源码方法与访问器显示名称相同而不互相代替。修复 `$名称` 错误插值诊断扫描中的 UTF-8 字节边界，并在非法反引号名称中只报告首个非法字符。新 lexer 子模块 43 行、parser 名称测试 85 行；没有新增 runtime 契约或 wire 标签。

本机 workspace fmt/clippy（release）通过；parser 486 项、identity 341 项、HIR 890 项、HIR lower 1,398 项及 2 个 doc-test 通过；提交前另补验限定 enum variant 与同名顶层 Array 构造器保持独立推导的回归。新增名称用例含 4 个正向 fixture（其中跨 Cone 产物链 1 个）和 9 个 negative fixture；普通 CLI 比较连同 2 个 M29 属性与静态产物回归共 15/15 通过，18 个配置、38 个进程、26 个 HIR/MIR/LIR golden。覆盖 debug/release、UTF-8 位置、码点不规范化、关键字/空格/点号/反斜杠名称、泛型与别名、接口/继承/引用、注解路径、重导出、仅产物消费与 moving GC。

报告见 [03-names-darwin.json](validation/03-names-darwin.json)。已清理之前完成的 fixture cases（52.1 MiB），并在确认没有 Rust 构建运行后清理闲置的 debug incremental（14.7 GiB）；保留 release 依赖与增量 fixture 缓存。Linux LLVM 22.1.2 的实际前缀已确认为 `/usr/lib/llvm-22`，最终验证将显式配置，避免系统默认 LLVM 21.1。

### 4. 循环标签与 break/continue 表达式（完成）

while/for 支持 Unicode 与反引号标签、最近同名标签遮蔽及当前 callable 内的目标解析。标签与值名称独立；while 条件位于自身循环控制区内，for iterable 在进入自身控制区前求值。break/continue 在 Elvis、调用参数、数组元素等表达式位置产生 Nothing，并保留已执行的副作用；未知、越界、跨 callable 及不可达代码中的非法目标均诊断。

HIR 使用既有 LoopId；默认参数和泛型产物保存类型化的包围循环距离，实例化时重新分配目标。无标签 tag 10/11 的原编码不变，非零距离增加字段，reader 检查实际嵌套。默认 body 的控制流合并按目标保存 abrupt outcome，内层循环不会吞掉外层跳转；concretization、MIR 及 finally/Context/协程清理均按实际目标执行。标签解析模块 57 行、目标编码模块 58 行，旧固定四槽控制结果改为实际目标映射后减少了重复代码。

本机 workspace fmt/clippy（release）通过；parser 489、HIR 893、HIR lower 1,399、MIR lower 118、slib 560 项 Rust 测试和 1 项 doc-test 通过。更新了旧编码负例及直接构造 HIR 的 MIR 测试声明名称，原有分派断言保持不变。16 个新增 fixture（5 个正向、11 个 negative）连同 4 个回归共 20/20 普通比较通过，25 个配置、53 个进程、43 个 HIR/MIR/LIR golden。覆盖外层 break/continue、同名遮蔽、条件 setup、求值顺序、finally 覆盖与嵌套 finally 循环、Context 还原、挂起/恢复、moving GC，以及含默认参数的泛型 provider → bridge → consumer 仅产物链。

报告见 [04-loop-jumps-darwin.json](validation/04-loop-jumps-darwin.json)。审阅了 HIR 的外层目标及 bridge 默认参数中的重新映射；既有 M22 循环组合与 M35 return/throw 操作数和清理快照保持不变。完成后清理本批已通过的 fixture 工作目录，保留共用缓存、sysroot 和报告。

### 5. do/while 与条件初始化检查（完成）

do body 与 condition 共享最外层局部作用域，每轮闭包捕获本轮 val；嵌套块及循环后的可见性保持普通规则。复用既有控制结果分析，按目标 LoopId 合并真正到达条件的 continue；被 finally 改为 break/return 的路径不计。变量读取、函数值调用及 lambda/局部函数捕获均在源码使用处报告可能未初始化，包含 initializer 中的 Elvis continue 和外层标签跳转。

AST、HIR、concrete HIR 和结构化 MIR 保留显式 LoopKind；默认/泛型 body 使用 tag 9 保存 do/while，普通 while 的 tag 8 编码不变。reader 按真实执行顺序检查局部定义。CFG 分别建立 do body、condition 和 exit，首轮直接进入 body，continue 进入完整条件；条件入口保留 GC poll 和协程恢复，永远无法到达的条件完整封闭为 unreachable。新增实现模块分别为 63、89、104 行，现有控制流大文件移出对应循环实现。

本机 fmt、workspace clippy（release）通过；parser 490、HIR 895、HIR lower 1,400、MIR lower 118、slib 560 项 Rust 测试及 1 项 doc-test 通过。16 个新增正式 fixture（6 个正向、10 个 negative）和 3 个既有循环回归全部完成普通比较，合计 19 个用例、26 个配置、66 个进程、48 个阶段 golden。覆盖单语句、至少执行一次、条件内 continue/break、每轮捕获、finally/Context、挂起恢复、moving GC，以及默认参数与泛型 body 的 provider → bridge → consumer 纯产物链。旧 HIR/concrete/default dump 仅补充普通 while 的 LoopKind 字段；保留原有执行与 wire 身份。

报告见 [05-do-while-darwin.json](validation/05-do-while-darwin.json)。报告合并普通验证批次：已通过且未变化的用例没有重复执行，只重跑新增清理正例与受结构快照字段迁移影响的回归。完成后清理本批 fixture cases，保留增量 cache、sysroot 和报告。

### 6. 局部 return 标签（完成）

支持具名函数自身标签、显式 lambda/匿名函数标签，以及直接具名调用 lambda 参数的隐式标签；显式标签覆盖同一 lambda 的隐式标签，同名内层循环遮蔽外层 callable 标签。标签只解析到当前 callable，未知目标、循环目标、跨 callable 与裸 lambda return 均在实际使用处诊断。下一行注解不会被解析为前一表达式的标签，局部函数标签使用源码名称。

无期望结果类型的 lambda 独立合并自身的带标签 return 与正常末尾结果，内层 callable 不影响外层推导；全无正常结果时为 Nothing。显式返回按目标类型检查，Unit 期望只允许丢弃末尾值。复用既有 HIR Return/异常清理与 coroutine 路径，标签不进入 wire；补齐 ContextScope、when predicate setup 和 guard setup 内返回值的统一适配。标签 parser 子模块 63 行，lambda 返回推导子模块 43 行，lambda 主模块 260 行。

本机 fmt、workspace clippy（release）通过；parser 493 项、HIR lower 1,402 项及 doc-test 1 项完成验证。HIR lower 初轮唯一剩余的局部函数标签回归修复后以两个相关单元测试补验，没有重复执行不受影响的全量。22 个新增正式 fixture（6 个正向、16 个 negative）与 5 个旧裸 lambda return 回归全部普通比较通过，共 27 个用例、32 个配置、60 个进程、33 个阶段 golden。覆盖 debug/release、返回值合并与装箱、默认参数、泛型、成员/infix/Unicode 名称、finally/Context、挂起恢复、moving GC 和 provider → bridge → consumer 纯产物链；旧负例更新为精确的裸 return 位置及完整诊断。

报告见 [06-return-labels-darwin.json](validation/06-return-labels-darwin.json)。完成后清理本批 fixture cases，保留共用缓存、sysroot、日志和报告。

### 7. 具名表达式体函数结果推导（完成）

顶层、成员、扩展、局部、泛型、挂起、override 及 operator 的表达式体在定义处推导结果，显式结果优先，块体和无 body 声明仍按省略即 Unit 处理。泛型参数保留原身份，无正常结果推导 Nothing；调用者的期望类型不能反向改变声明。活动 FunctionId 依赖栈诊断递归推导，显式签名可以打断环；重载按参数选中实际目标后才查询结果，既不会把无关同名候选当作环，也不会因真正的环退回较不具体的重载。

声明查询复用普通表达式 lowering，保存完整 typed body，隔离调用处的局部、捕获、初始化和默认表达式状态。继承目标和默认参数来源先按已知部分建立，只有依赖结果的相等约束延后检查；重复约束合并，原有继承和布局验证不重放。完成后按原有访问域检查公开签名，再导出完整结果；没有新增 HIR/wire 占位或 runtime ABI。局部函数 body 移入 68 行子模块，声明查询、作用域隔离、结果约束分别为 178、58、104 行。

本机 fmt、workspace clippy（release）通过；HIR lower 1,407 项测试全部通过。26 个新增正式 fixture（9 个正向、17 个 negative）与 M33 errno、M35 when 产物回归普通比较共 28/28 通过，38 个配置、96 个进程、69 个阶段 golden。覆盖 debug/release、泛型宿主/方法/接口默认实现、继承默认参数、super 与动态分派、函数引用、Context、局部捕获、异常/finally、挂起恢复、moving GC、可见性，以及移除源码后的 provider → bridge → consumer 链；消费方同名函数不会重绑定定义处默认调用。

报告见 [07-function-inference-darwin.json](validation/07-function-inference-darwin.json)。现有 M33/M35 快照保持不变。完成后清理本批 fixture cases，保留共用 cache、sysroot 和验证日志。非局部 property 推导作为下一项功能单独实现与提交。

### 8. 非局部 property 类型推导（完成）

顶层、class、object、struct/enum 计算属性及泛型扩展属性可以从 initializer、表达式 getter 或唯一委托 getter 推导完整类型；const 复用原标量常量求值。显式类型优先，stored property 的自定义 getter、setter 和后续赋值不反推声明类型。缺少推导来源、依赖环、空数组/None、未定型 lambda、override 不匹配及公开类型泄漏均在源码处诊断。

函数和属性共享按 typed ID 记录的声明依赖栈，查询隔离定义处作用域并复用完整 typed body。class/object 初始化在真实构造上下文检查 constructor 参数、此前字段和 receiver 发布规则，最终按源码位置复用初始化步骤；顶层初始化同样只执行一次。继承的已知部分沿用原有决议，结果类型等式完成后统一检查；逻辑属性的可见性结果由访问器复用，避免相同类型重复诊断。raw storage 按最终类型检查零映像与 GC-free 契约，并导出正确的 NoGC 访问器。没有新增 HIR/wire 形态或 runtime ABI。声明查询主模块 227 行，属性签名与初始化子模块分别 131、151 行；class 初始化主模块拆为 251 行，globals 主模块为 408 行。

本机 fmt、workspace clippy（release）通过；HIR lower 1,410 项、parser 493 项及 doc-test 1 项通过，可见性去重后补验 22 项相关测试全部通过。36 个新增正式 fixture（11 个正向、25 个 negative）与 8 个回归普通比较共 44/44 通过，58 个配置、154 个进程、120 个阶段 golden。覆盖 debug/release、构造参数与闭包、初始化顺序/只执行一次、provideDelegate、泛型 receiver、继承与动态分派、const/raw storage、Nothing/Unit、Context、挂起恢复、异常/finally、moving GC，以及移除源码后的 provider → bridge → consumer 链和跨 Cone NoGC 读取。

报告见 [08-property-inference-darwin.json](validation/08-property-inference-darwin.json)。仅迁移一个既有构造器产物用例的两份 AST 快照，补充功能 6 的 lambda 隐式标签；其余阶段和执行结果保持不变。普通报告合并已通过且未变化的用例与该用例复验，生成快照不计入通过数。已清理本批旧 fixture cases（575 MiB），保留共用 cache、sysroot 与日志。

### 9. 构造器与未绑定成员引用（完成）

`::TypeName` 从普通可见的 primary/secondary constructor 中选择目标，支持泛型与透明 alias；`Type::member` 将 receiver 作为第一个函数参数，按正常名称和类型语法区分值 receiver、类型 qualifier、嵌套类型与同名变量。源码函数与构造器进入相同候选层，期望函数类型参与既有适用性和 MSC；无期望类型须有唯一完整签名。普通绑定引用继续在创建时只读取一次 receiver。virtual/interface 成员每次调用均按实际 receiver 分派。

已选定的构造器或未绑定成员生成完整 typed lambda 适配 body，复用现有闭包、调用、捕获和 GC 路径。普通依赖构造器调用已有产物入口，泛型依赖请求已有构造模板，不复制初始化 body；默认参数与再次发布保存定义处绑定。创建点 receiver 类型作为刚性输入进入引用约束，保留外层泛型参数身份。数组构造结果与异常类型准备集中到普通调用和引用共用的完成入口。默认值、参数名与 vararg 协议不进入函数值，vararg 保留 Array 参数；unsafe/NoGC/FunPtr 边界保持原规则。没有新增 HIR/wire 形态或 runtime ABI。原 565 行引用文件拆分后主入口 215 行，构造器、构造候选准备、未绑定入口分别 296、195、112 行。

本机 fmt、workspace clippy（release）通过；69 项相关 Rust 测试通过，最终普通依赖构造分支和数组异常准备以正式 CLI 定向补验。39 个新增 fixture（12 个正向、27 个 negative）与 10 个回归普通比较共 49/49 通过，61 个配置、156 个进程、117 个阶段 golden。覆盖 debug/release、secondary/struct/Array 构造器、源码与依赖重载、泛型宿主及方法、类型参数上界、alias、嵌套类型、原始类型成员和扩展、receiver 快照、Context、挂起恢复、moving GC，以及移除源码后的 provider → bridge → consumer 产物链。数组同时验证普通构造、函数值构造、复制与负长度异常。

报告见 [09-references-darwin.json](validation/09-references-darwin.json)。6 个旧“不支持引用”负例按新语义迁移为正向回归，其余旧阶段快照不变。报告合并已通过且未变化的用例和最后 8 个定向复验；生成快照不计入普通通过数。本批多次清理 fixture cases，保留共用 cache、sysroot 与日志；release 构建目录约 1.6 GiB。

### 10. receiver 函数类型与隐式 receiver（完成）

支持 `A.(P) -> R`、挂起形式、带 receiver 的匿名函数，以及直接、invoke 和扩展式函数值调用。lambda 的 this 由期望 receiver 类型提供，it 只取决于普通参数数量；显式局部名称优先，隐式 receiver 从内向外参与原有成员和扩展候选决议。this@label 按 class、扩展函数和 callable 的实际词法标签定位，透过普通闭包的传递捕获仍禁止自由引用外层 var。构造初始化中的 this 标签和具有自身 receiver 的闭包分别按实际实例检查。

receiver 槽在 AST、HIR、MIR、共享签名和 exact identity 中独立保留；ABI 沿用展平后的首参数与既有 closure environment。普通无 receiver 签名保留原 wire 编码和持久化身份。receiver 与首参数形式按展平后的方差双向适配，包括动态 is/as、装箱桥和 managed C callback；native FunPtr 本身不接受 receiver 签名。推导保留单一来源的完整函数形状，混合形状无唯一解时报错；Array/Option 的 nominal 不变性和重载规则分别保持准确。

新增 receiver 查找、匿名函数签名、扩展候选和重载签名比较子模块，主匿名函数实现 257 行，声明 callable 主模块 297 行。没有新增 runtime 执行机制或调用 ABI。

本机 fmt、workspace clippy（release）无警告通过；parser 496 项及 doc-test、identity 343 项及 doc-test、约束求解 19 项、HIR lower M11 26 项、MIR adapter 3 项和 LIR function 4 项通过。最终推导修复另以两个定向单元测试补验。49 个新增 fixture（15 个正向、34 个 negative）和 56 个回归普通比较共 105/105 通过，126 个配置、248 个进程、188 个阶段 golden。覆盖 debug/release、真实挂起恢复、moving GC、值类型复制、ZST、求值顺序、FFI 回调、泛型与默认参数，以及删除源码后的 provider → bridge → consumer → 独立链接完整产物链。

报告见 [10-receivers-darwin.json](validation/10-receivers-darwin.json)。旧 HIR 快照补充 receiver 字段，定向回归另迁移此前的隐式标签及类型收集顺序差异；普通函数身份和编码保持不变。报告只合并普通通过批次，不计入生成快照的运行。已清理本批完成的 fixture cases 817 MiB，保留共用 cache、sysroot 与日志；C callback 的 Linux 目标 LIR 快照将在最终平台验证时生成并普通复验。

### 11. 泛型、嵌套与局部 typealias（完成）

alias 的类型参数、inline/where bound 与目标在定义处绑定，使用处替换完整实参；构造调用和引用沿原有推导与 MSC 选择真实目标，固定目标实参不能被覆盖。支持 nominal nested、companion 宿主参数和局部声明后可见性；普通 static nested 不捕获宿主参数。完整展开依赖检查数组、函数与泛型实参内部的循环，局部 alias 不产生运行期状态或导出名称。

enum variant 的构造、普通值和模式共享实际 alias application，保留 payload、默认参数、vararg 与期望类型推导。alias 目标本身为类型参数时，先从显式参数或期望结果确定真实构造类型。泛型构造与非泛型同名函数进入原有重载规则；参数置换、重复目标参数、未使用参数及构造器引用均映射回真实声明，不生成 alias nominal 或转发构造器。

共享接口保存规范化 binder/bound、目标与必要的宿主参数前缀，旧无参四字段编码保持不变。发布路径使用真实 owner chain 区分顶层和嵌套名称；公开目标及 bound 的外部引用进入原有闭包。新增声明、应用、推导、构造与发布子模块分别为 171、148、85、108、243 行，原构造候选大文件按准备、探测与完成职责拆分。

本机 fmt、workspace clippy（release）无警告通过；parser alias 6 项、HIR alias 接口 25 项、HIR lower alias 51 项、引用 69 项及导出绑定 6 项定向测试通过。52 个新增 fixture（13 个正向、39 个 negative）和 52 个回归普通比较共 104/104 通过，121 个配置、293 个进程、198 个阶段 golden。覆盖 debug/release、构造协议、receiver 类型、边界与循环诊断、moving GC，以及删除源码后的 provider → bridge → consumer → 独立链接产物链。

报告见 [11-aliases-darwin.json](validation/11-aliases-darwin.json)。138 份旧 golden 中的 745 个无参 alias 记录仅机械补充空 binder/宿主前缀字段，移除新增文字后已逐份确认与原记录一致；定向回归另更新构造诊断位置、少量类型 arena 编号及此前隐式标签的 AST 输出。普通报告合并未变化的通过项与受影响用例复验，不计入生成快照的运行。清理已完成的 fixture cases 1.4 GiB 后，5 个因旧只读产物覆盖失败的用例普通重跑全部通过；保留共用 cache、sysroot 和日志。

### 12. sealed 继承与 when 穷尽性（完成）

支持 sealed class/interface、默认 protected 的 sealed 构造器，以及同 origin Cone、同 package、具名非局部声明的直接继承约束。sealed class 隐含 abstract，普通成员仍默认 final。完整直接子类型关系进入 HIR 和共享元数据，私有子类型参与覆盖检查但不增加源码可见性；普通类型保留原有 11 字段编码，sealed 增加第 12 字段。

when 按真实继承 application 递归展开 sealed 家族，保留非 sealed 子类型的开放继承范围；泛型参数沿父子关系替换，额外未确定参数保留整个家族，不能用几个具体实例冒充穷尽。guard 不贡献无条件覆盖，用户 equals 不覆盖整个家族，互补条件及互斥类型的否定条件可以联合覆盖。类型条件检查已保存 subject 的静态类型，先前失败路径的 smart cast 继续服务普通条件、guard 和 body，避免把合法的后续否定条件误报为无意义检查。

子类型关系、泛型 application 和 when predicate 实现分别为 109、205、42 行；共享短路求值移入 109 行子模块，原 operators 主文件缩为 415 行。没有增加 runtime 契约或额外的继承授权机制。

本机 fmt、workspace clippy（release）通过；parser sealed 2 项、HIR nominal 接口 35 项（含新增 wire 测试）及 HIR lower when 47 项定向测试通过。41 个新增 fixture（14 个正向、27 个 negative）和 44 个回归完成普通比较，共 85/85 通过、99 个配置、204 个进程、140 个阶段 golden。覆盖 class/struct/enum/object、泛型与 bound、菱形继承、私有子类型、构造器、guard、短路副作用、跳转与 finally、真实挂起恢复、moving GC，以及删除源码后的 provider → bridge → consumer → 独立链接产物链。

报告见 [12-sealed-darwin.json](validation/12-sealed-darwin.json)。1,518 份旧 golden 中的 12,808 个普通 nominal 记录机械补充空 sealed 子类型字段，迁移时已逐份确认移除新增字段后与原文件一致；when 条件改读保存的 subject，另外更新了相应 HIR/MIR/LIR 快照。生成快照不计入普通验收结果。已清理最后一批通过的 fixture cases 549.6 MiB，保留共用 cache、sysroot 与日志；Linux 目标快照随最终平台验证更新。

### 13. 接口实现委托（完成）

class/object 支持 `I by expression`，按继承列表顺序在 base/primary store 后、body 初始化前求值并保存一次。primary 参数可见，当前未完成初始化的实例及任一 secondary 的局部参数不可见；异常、safe 上下文和 singleton 初始化沿普通构造规则处理。即使所有 slot 都被显式实现覆盖，delegate 表达式仍按顺序执行。

显式成员与继承的具体 class 实现优先，唯一 delegate 补齐其余 slot，并优先于 interface default；多个 delegate 竞争时要求显式 override。属性逐 getter/setter 转发，保留整体类型与 mutability 检查，并支持父类 getter 与委托 setter 的组合。转发成员覆盖所有相关 slot 的访问域，保留默认值、参数名、vararg、operator/infix、Context、挂起及 safety contract；实际转发只传已物化实参，delegate 内部 this 不改变。

HIR 生成普通隐藏字段、方法及访问器；物理字段不再强制关联逻辑 property。字段身份按真实宿主与继承项位置区分，object 和泛型 companion 的字段源码位置关联原 object。跨 Cone 的普通接口调用保留动态分派，父类 getter 使用直接 super 调用；泛型正文继续使用已有模板。没有增加运行期代理或 ABI。委托的字段、选择、可见性、属性、签名和正文分别放在独立模块；AST 的类和构造语法拆为 263 行模块，原声明文件缩为 536 行。

本机 fmt、workspace clippy（release）无警告通过；parser 3 项、字段身份 8 项、HIR 字段 2 项、继承 36 项和产物读取 22 项定向测试通过。36 个新增正式 fixture（16 个正向、20 个 negative）和 30 个回归普通比较共 66/66 通过，82 个配置、315 个进程、360 个阶段 golden。覆盖 debug/release、值类型 delegate 装箱、默认参数与变参、函数引用、初始化顺序及异常/finally、object/泛型 companion、真实挂起恢复、moving GC，以及删除源码后的 provider → bridge → consumer → 独立链接产物链。

报告见 [13-delegation-darwin.json](validation/13-delegation-darwin.json)。旧回归快照无需修改；生成快照的运行不计入普通验收结果。完成后清理本批 fixture cases，保留共用 cache、sysroot 和验证日志；release 构建目录保持约 1.6 GiB。跨库 fixture 的 Linux 目标 LIR 快照随最终平台验证生成并普通复验。

### 14. inner class（完成）

支持 class 直接成员中的 inner class、完整 `Outer<A>.Inner<B>` 类型、显式与隐式外层实例构造、`this@Outer` 以及绑定外层实例的构造器引用。外层 binder 作为完整前缀保留，自身参数按普通约束检查；静态嵌套类型隔断外层实例和类型参数，inner companion 只继承完整类型参数。普通重载、alias、class bound、可见性、继承与 this/base 委派复用已有规则。

每个 inner 实例保存普通 typed 外层引用字段；构造入口先保存 outer，再求值显式实参、default 和 vararg。最终构造器在 base 后、primary store 和 delegate 前写入 outer；委派与默认值直接读取隐藏参数，尚未就绪的 inner 仍不可访问。普通成员及多层闭包从外层 receiver 读取或捕获，构造器引用在创建时保存 receiver 快照，沿已有字段、闭包和 GC 路径保活。

外层字段与构造输入使用独立 typed identity/selector。源码构造签名以 outer 为 receiver，实际 initializer 以 inner 为 receiver、outer 为首个值参数，普通源码参数索引保持原值；跨 Cone 的 HIR/MIR 签名、模板、源码位置和参数对应关系同步保留。没有增加 runtime 入口或对象模型。新增模块最长 188 行，类型解析主文件拆为 410 行。

本机 fmt、workspace clippy（release）无警告通过；parser、字段与参数身份、HIR 来源与 inner shape、MIR 构造签名的定向 Rust 测试通过。44 个新增正式 fixture（19 个正向、25 个 negative）与 26 个旧回归完成普通比较，共 70/70 通过、100 个配置、250 个进程、207 个阶段 golden。覆盖 debug/release、求值顺序、多条 terminal secondary、默认参数、vararg、泛型、alias、继承、接口委托、闭包的传递捕获、真实挂起恢复、moving GC，以及删除源码后的 provider → bridge → consumer → 独立链接产物链。

报告见 [14-inner-darwin.json](validation/14-inner-darwin.json)。2,138 份旧 HIR golden 机械补充无 outer 参数字段及普通参数 selector，迁移时逐份确认去掉新增字段后与原文一致；inner 新快照保存最终源码参数位置及 ABI。报告合并普通通过批次，生成快照不计入验收；未变化的通过项不重复运行。已清理完成的 fixture cases，保留共用 cache、sysroot 和验证日志；跨库 Linux LIR 快照随最终平台验证生成并普通复验。

### 15. 局部类型与匿名 object（完成）

支持局部 class/struct/enum/interface、匿名 object、声明后可见性、嵌套类型和外层泛型参数。局部类型沿实际词法声明路径生成 typed identity，不导出局部名称；默认参数和泛型正文保存必要定义，删除提供方源码后仍可消费。推导结果不得使局部或匿名类型逃出有效作用域，显式父类型和 Any 返回沿普通适配处理。

局部和匿名 class 只按实际自由引用捕获不可变 binding，在构造时复制到一个普通隐藏 tuple 字段，空环境使用 Unit；构造器引用保存创建时的环境快照。自身及外层 receiver、局部函数、默认值、smart cast 视图、接口委托和多层闭包使用同一捕获路径。外层 var、值类型/interface 捕获、静态嵌套越界捕获与条件未初始化读取均在源码位置诊断。隐藏构造上下文以封闭类型区分 outer 实例和捕获环境，源码参数位置保持不变。

捕获集合在声明正文检查完成后一次确定，并完成相应字段、构造输入和闭包来源类型；没有增加 runtime 对象种类或 GC 入口。构造器引用使用普通函数参数逆变和结果协变，保留真实构造签名后生成既有适配。新增模块最长 214 行；原局部函数补丁遍历与捕获完成共用已有 HIR 的结构遍历，避免复制整套表达式分支。

本机 fmt、workspace clippy（release）无警告通过；新增 parser 2 项、identity 343 项及 doc-test、HIR lower 闭包 26 项定向 Rust 测试通过。47 个新增 fixture（27 个正向、20 个 negative）与 83 个构造器引用/inner 回归完成普通比较，共 130/130 通过、185 个配置、423 个进程、351 个阶段 golden。覆盖 debug/release、每轮与每实例捕获、名称遮蔽、默认值、多条初始化路径、函数引用、方差、moving GC，以及 provider → bridge → consumer → 独立链接的无源码产物链。

报告见 [15-local-types-darwin.json](validation/15-local-types-darwin.json)。旧 HIR golden 机械迁移构造上下文字段名，并将已有 outer 包装为明确类别；普通引用回归仅额外更新一条方差诊断及一个未使用类型实例。报告合并未变化的普通通过项与两条定向复验，不计快照生成次数。已多次清理完成的 fixture cases，并清除约 2.66 GiB 的旧 debug 可执行文件；保留共用 cache、sysroot、release 依赖和日志，Linux LIR 快照随最终平台验证生成。

### 16. 正常出口与流敏感 smart cast（完成）

类型事实沿实际正常出口传播，覆盖 if/when、短路条件、循环回边、break/continue、try/catch/finally 和挂起恢复。普通局部 var 可以在当前 callable 内收窄，赋值、复合赋值和更新撤销旧值事实；可写地址暴露及外层 var 捕获仍遵守原限制。不可变快照与 Boolean 条件值保存自己的事实，原变量写入不会改变已经保存的值。

分析使用已有 BindingId、TypeId 和 LoopId，在有限的现有事实中求循环稳定结果，并核对实际读取所用的类型视图。do/while 条件分别合并正常 body 出口及指向自身的 continue，条件中的 continue 保持条件回边；finally 按真实控制结果替换原跳转。只合并实际可达的 catch 出口。显式 is/as 和可变 when subject 读取声明存储类型，不能把首轮视图错误用于后续迭代；when 普通匹配继续读取进入时的快照。

复合赋值与更新读取当前视图，写回按声明存储类型适配；独立的 ++/-- 语句允许丢弃结果。成功 HIR 只保留完整 typed 读取、必要拆箱及既有控制结构，不增加 MIR、wire 或 runtime 流分析状态。新增分析模块最长 153 行，替换了原先只针对 AST 局部分支的事实推导。

本机 fmt、workspace clippy（release）无警告通过；47 项相关 Rust 测试通过。48 个新增 fixture（28 个正向、20 个 negative）与 75 个旧 when/do/循环跳转/局部捕获回归完成普通比较，共 123/123 通过、163 个配置、363 个进程、290 个阶段 golden。覆盖正常出口、循环写失效、finally 覆盖、异常尾值、别名快照、局部捕获、真实挂起恢复、moving GC，以及删除源码后的泛型 provider → bridge → consumer → 独立链接产物链。

报告见 [16-smart-casts-darwin.json](validation/16-smart-casts-darwin.json)。旧 mutable when 负例改为写入后失效的精确诊断；五个旧用例只迁移 HIR 中不再使用或改变登记顺序的实例，MIR/LIR 保持不变。报告合并普通通过批次与受影响项复验，不计快照生成次数。已清理 122 个完成的 fixture 工作目录，保留共用 cache、sysroot 和报告；Linux LIR 快照随最终平台验证生成。

### 17. 整型枚举（完成）

支持八种定宽整数父类型、显式常量 variant、整数边界常量、透明 alias 与局部/泛型枚举。定义处检查位宽、符号、范围、重复值、缺少初始化和 payload；E 是其整数父类型 I 的子类型，算术、比较和方法沿 I 的签名，结果仍为 I。复合赋值与更新不能把 I 隐式写回 E，Array/Option/Ptr 保持原有不变性；when 按 E 的 variant 域检查穷尽性。

HIR、共享接口与 MIR/LIR 保留枚举、variant 和完整 application 身份，以及底层整数种类和原始位型。未装箱 E 使用与 I 相同的标量表示，无额外 tag 或 niche；C ABI、native callback、Ptr、extern global 和 C-layout aggregate 保留精确的源码类型。Box<E> 使用普通 boxed descriptor，以 Box<I> 为父并共享一个标量 payload；按 E、I 及继承接口查询/拆箱沿既有路径完成，Box<I> 不能转成 E。

上转节点改为 Upcast，值到整数父类型保持标量，值到引用类型才装箱。补全局部 enum 的外层泛型参数、普通 when 覆盖及泛型 bound 方法接收者适配。局部枚举组合测试暴露的 LLVM 常量 phi/select 根计数差异已修复：已知不可移动的选择不要求 relocation，动态输入继续保留移动根检查。没有增加 runtime 入口、授权机制或资源预算。新增实现模块最长 97 行。

本机 fmt、workspace clippy（release）无警告通过，79 项相关 Rust 测试通过。34 个新增正式 fixture（8 个正向、26 个 negative）与 26 个旧回归完成普通比较，共 60/60 通过、73 个配置、158 个进程、146 个阶段 golden。覆盖 debug/release、moving/minor GC、全部整数位宽、空 enum、继承方法与接口、局部类型、普通/case when、泛型不变性、C ABI，以及删除源码后的 provider → bridge → consumer → 独立链接产物链。

报告见 [17-integral-enums-darwin.json](validation/17-integral-enums-darwin.json)。旧 HIR 快照机械迁移 Upcast 名称和 Algebraic 表示字段，逐份反向核对原字节；受影响回归另迁移 core 新增 16 个边界常量带来的局部 ImportedIdentityId 索引、少量 HIR 类型登记顺序及一个已有 lambda 标签的 AST。旧回归 MIR/LIR 不变。报告只合并普通通过记录，不计生成快照的运行。完成后清理 fixture cases，保留共用 cache、sysroot、release 依赖和日志；跨平台快照在最终 Linux 验收中生成并普通复验。

### 18a. 注解静态值域（完成）

注解参数支持整型枚举、嵌套 annotation 应用及这些原子类型的一维 Array。枚举保留完整 application、variant identity 和底层整数位型；嵌套应用按定义处参数与默认值形成静态数据。数组保留顺序、重复元素及各元素的源码身份和位置，默认数组继续指向定义处。普通函数、对象、多维或可变数组、错误枚举身份与递归参数/default 依赖均在源码处诊断。

annotation class 不进入 runtime 类型系统，参数 schema 以独立的 typed annotation identity 表达嵌套值。数组 alias 沿现有类型解析检查实际 core Array，使用已有形式参数身份确认元素被完整转发，拒绝丢弃或替换元素的 alias。跨 Cone 保存同一 canonical 静态值及必要的类型/variant/annotation 引用，reader 在既有边界检查 schema、值类型和循环；不生成运行期对象或执行任意代码。cross-cone-interface 升为 73，新模块最长 169 行。

本机 fmt、workspace clippy（release）无警告通过；57 项相关 Rust 测试通过。33 个新增 fixture（6 个正向、27 个 negative）与 78 个旧注解回归完成普通比较，共 111/111 通过、117 个配置、168 个进程、69 个阶段 golden。覆盖整数位宽、泛型与 alias、前向和局部声明、默认值位置、逐元素错误、moving GC，以及删除源码后的 provider → bridge → consumer → 独立链接产物链。

报告见 [18a-annotation-values-darwin.json](validation/18a-annotation-values-darwin.json)。旧负例只迁移诊断类别或措辞，位置不变；旧正例 HIR 更新数组静态表示、元素来源，以及上一项 core 边界常量产生的索引与位置变化，MIR/LIR 不变。报告合并普通通过批次和受影响项复验，不计快照生成次数。清理本批 fixture cases，保留共用缓存、sysroot、release 依赖和日志。注解目标/use-site、type-use 与 Target/Repeatable 继续分别实现验证，尚未标记完成。

### 18b. 注解声明目标与 use-site（完成）

注解支持 annotation class、具名函数/方法、constructor、完整源码参数、accessor、receiver 和 typealias。property、field、get/set、param、setparam、receiver、delegate 只选择已存在实体；主构造 val/var 默认目标保持原字段/property。局部函数、局部 alias、lambda/匿名函数参数及解构完整参数保留实际位置，不增加局部变量或任意表达式目标。核心 accessor 效果注解沿原函数属性与正文检查处理。

HIR 和共享接口分别使用原声明身份、callable 与源码参数序号、property 与存储类别；局部 callable 复用 lexical generated identity，局部 alias 保留原源码身份，不进入公开名称查找。静态查询入口覆盖全部 typed target；reader 检查实际声明引用，不复制局部签名表或重放 body。cross-cone-interface 升为 74。新增模块最长 181 行，alias 发布模块约 305 行。

组合回归修复了固定外层参数的 alias 构造器候选比较：目标签名中的既有词法参数以固定等式进入原推导会话，声明自身 binder 继续推导。普通构造调用与构造器引用共用这一规则；不同外层实参不能因此互相替代。

本机 fmt、workspace clippy（release）无警告通过；149 项不同的相关 Rust 测试通过。26 个新增正式 fixture（5 个正向、21 个 negative）及旧注解、alias、局部类型回归完成普通比较，共 203/203 个适用用例通过，227 个配置、393 个进程、221 个阶段 golden；另一个 musl 专用用例在 Darwin 不适用。覆盖 debug/release、默认和显式目标、核心 NoGC、局部声明、泛型与 delegate、moving GC，以及删除源码后的 provider → bridge → consumer → 独立链接产物链。

报告见 [18b-annotation-targets-darwin.json](validation/18b-annotation-targets-darwin.json)。五个旧目标负例迁移为正例；其余快照保留新增局部 alias 和此前 enum/注解值格式变化。除增加普通构造调用的 alias 引用组合外，旧回归 MIR/LIR 不变。报告合并普通通过记录，21 个迁移用例已单独普通复验，不计生成快照的运行。完成后清理 fixture cases 与旧工具的编译缓存，保留当前缓存、sysroot、release 依赖和报告。Target/Repeatable 与类型位置注解继续实施。

### 18d. 注解目标限制与可重复性（完成）

core 增加 AnnotationTarget 整型枚举、Target 和 Repeatable；按真实 core 声明身份绑定固定语义，同名用户注解不获得特殊作用。允许目标按实际实体分类，包含 receiver/setparam、getter/setter 和 backing/delegate storage。默认 use-site 不受允许目标影响，空集合拒绝所有应用位置。显式可重复注解保留源码顺序与重复值，导入别名和 re-export 保持原身份。

annotation 参数/default 和元注解先完成绑定，再按最终规则检查前向应用。局部名义类型的 member alias 在该作用域元注解规则完成后绑定，块内 alias 沿既有声明顺序绑定。core 自身编译从本 Cone 的真实声明取得规则；使用 core 的编译从既有 prelude binding 取得相同身份。

允许目标集合与 repeatable 属性成为普通 HIR/共享声明数据。reader 在既有边界检查实际 target 类别、重复和已绑定属性，不重放元注解名称解析或求值。cross-cone-interface 升至 75，MIR/LIR 与 runtime ABI 不变；没有 container annotation 或运行期注解对象。新增模块最长 134 行，alias 注解绑定移到 40 行子模块，发布模块缩为 282 行。

本机 fmt、workspace clippy（release）无警告通过；59 项相关 Rust 测试通过。34 个新增正式 fixture（6 个正向、28 个 negative）及旧注解回归完成普通比较，共 182/182 个适用用例通过、197 个配置、324 个进程、170 个阶段 golden；1 个 musl 专用用例在 Darwin 不适用。覆盖全部已开放声明类别、前向/局部声明、同名用户注解、alias/re-export、重复顺序和重复值、moving GC，以及删除源码后的 provider → bridge → consumer → 独立链接和仅产物错误检查。

报告见 [18d-annotation-rules-darwin.json](validation/18d-annotation-rules-darwin.json)。40 份旧 HIR 快照逐份核对，差异仅为默认规则字段与 core 新声明引起的 ImportedIdentityId 索引；旧 MIR/LIR 全部不变。一条旧 Rust 测试按已开放函数注解目标改为未知名称诊断。报告不计快照生成次数。已更新测试工作区的 core 副本，并清理本批 fixture cases 与旧工具的编译缓存；保留共用 cache、sysroot、release 依赖和报告。类型位置注解的后续实现与验证见下一节。

### 18c. 类型位置注解（完成）

类型注解覆盖参数、结果、receiver、field/property、显式局部类型、父类型与 bound、alias 目标和显式调用类型实参；泛型实参、tuple、函数与 pointer 子节点分别保留位置。可空 receiver 与普通可空类型使用同一括号规则。TYPE 目标、use-site 禁止、核心效果边界、静态参数和 Repeatable 均在真实使用位置检查；注解不改变 TypeId、重载、布局或 ABI。

类型使用以 SourceIdentity 和实际类型范围形成独立身份，保存完整解析类型、binder、稀疏注解路径和 typed 声明位置关联。正常类型解析记录实际关系，schema/常量完成后绑定一次静态值；annotation class 及其 Array 参数保持独立静态类型，不泄露用于解析的临时参数。别名按原 binder 位置重排、复制或丢弃实参注解，以模板、实参、当前前缀的顺序合并；两个实参的 TypeId 相同也不会混淆注解位置。

共享接口与静态查询保留完整类型、各节点注解及原来源，支持普通依赖、re-export 和再次发布。reader 复用既有引用、参数值和签名检查，核对路径、binder、TYPE 与重复规则，不重跑别名展开或布局。cross-cone-interface 为 76，MIR/LIR 和 runtime ABI 不变。新增模块最长约 300 行，别名组合、作用域保存、路径和 wire 编码分别放入实际职责对应的模块。

组合回归同时修正了 imported alias 缓存保留首次引用位置的问题。另以 `af79df007` 独立修复构造器 receiver 类型位置检查：只有已记录的 EnclosingInstance 或 CapturedEnvironment 输入构成 context receiver，普通构造器不因拥有 class owner 而获得该位置。

本机 fmt、workspace clippy（release）无警告通过；288 项不同的注解、alias 和 receiver Rust 测试通过。32 个新增正式 fixture（11 个正向、21 个 negative）与旧注解、alias、receiver、inner、局部类型及声明位置回归完成普通比较，329/329 个适用用例通过，另 1 个 musl 专用用例不适用；合计 390 个配置、754 个进程、509 个阶段 golden。新增用例自身覆盖 42 个配置、95 个进程、75 个 golden，包括 debug/release、moving GC、真实 C callback、静态查询、可空层次，以及删除源码后的 provider → bridge → consumer → 独立链接与错误诊断。

报告见 [18c-type-annotations-darwin.json](validation/18c-type-annotations-darwin.json)。81 份旧 HIR 快照逐份审阅：77 份只有 ImportedIdentityId 编号变化，已核对完整内容及引用等价关系；另两份补充默认注解规则，两份仅移动同一完整类型描述块。旧 MIR/LIR 全部不变。报告只合并普通通过批次，不计快照生成次数；已通过且未变化的用例没有重复执行。多次清理已完成的 fixture cases，保留共用 cache、sysroot、release 依赖与日志。全部功能批次已完成，Linux 总验收继续进行。

## Linux 总验收（进行中）

补充设计第 14 节完整组合链之前，M37 的 675 个 fixture 已完成 GNU 与 musl 普通验收，每个目标均为 675 passed、0 failed，888 个配置、1,878 个进程、1,422 个阶段 golden。逐个用例选择最后一次普通执行结果，排除快照生成和中断记录；统计与逐项来源见 [M37 Linux 覆盖](validation/m37-linux.json)。真实目标 ABI 不同的 LIR 已按目标保存，41 份相关 fixture 声明同步选择目标文件。既有功能的完整 GNU 回归及其跨平台快照迁移仍在进行。

Darwin 已补齐一批受 core 声明及控制流转储影响的旧用例：212 项普通比较通过、1 项按平台不适用，218 个配置、2,207 个进程、2,606 个 golden，见 [旧功能平台回归](validation/legacy-platform-darwin.json)。358 份变更逐份审阅，包括 315 份符号表、34 份 LIR 与 9 份 HIR；保留真实 AArch64 ABI，普通符号表只增加对应平台已核对的 core 符号，协程生命周期用例另包含已审阅的 Nothing 函数适配。生成快照不计入普通通过次数。

已将全部功能提交同步到 `nuc12:~/repos/scoop`。首轮 Linux workspace fmt/clippy 通过；完整 Rust 测试发现三条旧 wire 测试仍将整型枚举的新 tag 当作未知标签，或保存了扩展前的空注解表编码。按现有规范迁移这三条断言，未知标签改用未定义的 255，固定编码继续精确比较；没有修改生产代码或放宽 reader 检查。本机 fmt/clippy 与 HIR 全部 905 项测试通过，Linux 继续验证其余 workspace 与正式 CLI。

后续全量中的两条旧 HIR lower 断言同步到现行源码规则：局部 var 的负例改为赋值后撤销 smart cast；固定类型 alias 构造的诊断位置与已验收的四条正式 CLI negative 保持一致，仍检查完整类型错误与源码区间。本机 fmt/clippy 后，两项定向 Rust 回归通过；不改生产代码。

另迁移三条旧底层断言：Retype 的非法 machine-scalar 输入继续拒绝，断言同步实际错误文字；两份 profile 固定描述逐字节确认只有 cross-cone-interface 版本 72 → 76，随后更新相应固定指纹。本机 fmt/clippy、25 项 profile 测试（HIR 4 项、slib 21 项）及一项 Retype 拒绝测试通过；编码与指纹仍由完整固定向量检查。

Linux 完整 Rust 覆盖已闭合：首轮加剩余 workspace（`--exclude scoop-hir --no-fail-fast`）执行全部目标，八条旧断言迁移后定向补验通过，43 个目标共 5,553 项不同测试通过、0 failed、0 ignored。driver 的 82 项完整组合全部通过，没有重复这组重型测试。最终 fmt/workspace clippy、公共 runner 44 项及三个 release CLI 构建通过。环境为 i7-12700H、Rust 1.99.0、LLVM 22.1.2、GCC 15.2.0；逐项来源和初跑原始日志见 [Linux Rust 记录](validation/rust-linux.json)。正式 GNU 全量和 musl 选集验收继续进行。

### 验收回归：单例成员引用（本机完成）

Linux 全量发现完整泛型 companion 限定名被误判为未绑定 receiver。修正后，object/companion 及其透明别名按已解析的单例身份创建绑定引用；实例只在创建引用时求值一次，普通类型的未绑定引用规则不变。跨 Cone 物化泛型 companion 时，字段类型位置沿字段声明方查询 backing property，继续检查真实属性身份与来源。

新增三个独立 fixture，覆盖本地与跨库别名、完整宿主 application、默认值、泛型正文、转导出、单例初始化次数、普通与 moving GC，以及错误地多传 receiver 的精确负例。workspace fmt/clippy 通过，22 项函数引用、6 项 companion 与 14 项类型位置 Rust 定向测试通过；44 个正式 CLI 用例普通验收全部通过，包含原有 m29 companion 与 m34 MaybeUninit 回归。旧用例仅两组 HIR 局部编号迁移，MIR/LIR 未改变。报告见 [单例引用回归](validation/singleton-reference-regressions-darwin.json)。Linux 将在本机修订完成后对受影响范围复验。

### 验收回归：捕获值与构造器参数的 smart cast（本机完成）

when 保存 subject 时，原实现只关联普通局部来源，遗漏不可变捕获值与构造器参数。修复复用已有稳定 binding 查询及快照关系，使闭包内的多 interface 检查和构造初始化中的值类型检查保留事实；分支合流、赋值失效和可变捕获限制仍按原规则处理。生产改动位于 132 行的既有条件分析模块，没有新增分析框架或后续 stage 状态。

新增三个 fixture，覆盖 lambda、匿名函数、局部函数、显式快照、when subject 声明、构造器参数与闭包、普通/moving GC，以及只在部分分支成立的事实不得越过合流。workspace fmt/clippy 通过，捕获与 when 两组 Rust 定向测试执行 77 项全部通过。52 个正式 CLI 用例普通比较全部通过，81 个配置、206 个进程、187 个阶段 golden，包含原有 M35 捕获组合回归。该旧组合用例补充 lambda 标签，并将类型检查改为读取已经保存的 subject；既有静态类型结构保持不变，LIR 相应省去一次重复捕获读取。报告见 [smart cast 回归](validation/smart-cast-capture-regressions-darwin.json)。Linux 受影响范围复验继续进行。

另有八个旧负例按现行规则完成迁移：接口实现诊断顺序、带宿主的 companion 名称、单语句 for body，以及 GCLeaf 目标的语义检查。旧 return-label 负例改为 Elvis 中使用不存在的标签，继续精确检查错误文字与源码位置。workspace fmt/clippy 后八例本机普通比较全部通过，生产代码未因这些快照差异改变。报告见 [旧负例迁移](validation/gnu-diagnostic-migrations-darwin.json)。

### 验收回归：函数 smart cast 与原存储值（完成）

函数 smart cast 的读取可能创建 ABI 适配闭包，不能把它当作 binding 原本保存的函数。修正显式 is/as 操作及引用相等，使其读取原存储值；有期望类型且声明类型已满足它时避免多余适配。不同签名的适配结果不再与来源共享快照事实，动态函数适配器的来源通过既有 Retype 满足 Any 捕获字段。改动复用当前读取、事实关联和 MIR 引用转换，五个生产文件共增加 43 行、删除 11 行；读取与条件模块分别为 137、135 行。

此前将协程检查交换顺序的处理未解决上述问题，现已恢复两份旧用例的原顺序，并替换相应验收记录。新增五个 fixture，覆盖多个返回接口视图、显式转换、捕获值、构造器参数、引用身份、typed function 转换失败、错误的适配值事实传播，以及泛型 receiver 函数值、默认 lambda、转导出、移除源码后的独立链接和 moving GC。完整 HIR 检查保留原 Any 读取，MIR 锁定物理捕获字段的引用转换。

本机与 Linux fmt/workspace clippy、35 项 HIR lower 与 4 项 MIR lower 定向 Rust 测试通过；Darwin、GNU、musl 三个目标各有 9 个正式 CLI 用例普通比较全部通过，每个目标为 13 个配置、70 个进程、77 个阶段 golden。包含原有两组动态函数适配及两组协程回归。报告见 [Darwin](validation/function-view-regressions-darwin.json)、[GNU](validation/function-view-regressions-gnu.json) 和 [musl](validation/function-view-regressions-musl.json)。目标 LIR 已逐份比较已验收的结构及真实 ABI，符号快照只增加对应平台已审阅的 325 个 core 符号；共有 HIR/MIR 没有平台差异。

Darwin 另有 18 个独立链接、原有 M35 组合及源码类型描述回归完成普通比较，33 个配置、179 个进程、336 个阶段与链接计划 golden 全部通过。20 份目标快照已逐份审阅：6 份 LIR 保留实际 AArch64 ABI，14 份链接计划仅反映已核对的 core 元数据变化；未改变执行断言。报告见 [Darwin 补充回归](validation/legacy-platform-extra-darwin.json)。

### 设计验收的完整组合链（完成）

新增五个 fixture 覆盖设计第 14 节的六条完整组合链：泛型 sealed 根与匿名开放子家族、guard/smart cast 和 receiver lambda；do/while 与标签、Elvis continue、finally、Context 和真实挂起恢复；inner/局部委托类与按值捕获、老对象持有新对象；Unicode/反引号与命名参数、尾随 lambda、整型枚举、嵌套与可重复类型注解；整型枚举、父类型参数/结果、函数值适配、不同装箱身份和独立 C callback。跨库注解链同时覆盖泛型 alias、结果推导、构造器/未绑定成员引用、re-export、再次发布，以及删除源码后的独立链接。

跨库组合发现整型枚举的成员查找将整数父类型误当作 interface application。先明确实现规范的遍历规则，再在既有模块内沿整数父类型继续遍历接口；修改后模块为 91 行，没有增加类型、分派或产物格式。C ABI fixture 保留已声明的函数引用签名，再通过普通函数值赋值验证参数逆变；原有引用候选规则不变。

本机 fmt/workspace clippy、6 项相关 Rust 分派测试及三个 release CLI 构建通过。五个新增 fixture 与既有整型枚举跨库、默认参数回归共 7/7 普通比较通过，12 个配置、73 个进程、71 个 golden。新增链均运行 debug/release 和 normal/moving/minor GC；静态描述保留嵌套注解身份、数组顺序与源码位置，C ABI 保留 Int16 符号扩展及上下界，GC 捕获字段为普通 Snapshot 值且无 mutable box。旧默认参数 HIR 仅按完全相同的实体记录重排依赖引用。报告见 [完整组合链本机验收](validation/complete-combinations-darwin.json)。M37 fixture 增至 680 个。Linux 同样完成 fmt/workspace clippy、6 项相关 Rust 测试和配套 CLI 构建；GNU、musl 各有这 7 个 fixture 的普通比较全部通过，配置、进程和 golden 数与本机相同，见 [GNU](validation/complete-combinations-gnu.json) 和 [musl](validation/complete-combinations-musl.json)。两目标的全部 680 个 M37 fixture 普通覆盖至此闭合，各为 898 个配置、1,938 个进程、1,476 个 golden；既有功能全量回归另行收尾。

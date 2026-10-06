# M29 实施记录

以 `a727ac93c` 为实现基线，在 `codex/m29` 逐功能提交。目标及完成门以
[设计](DESIGN.md) 和三份当前 spec 为准；只在实际通过验收后记录完成。

## 2026-10-06：显式 codec 协议与核心库迁移

`Encodable<T>` 现在接收显式 value 和 Encoder；标量编码方法移至各自 companion，
Unit 使用独立的 UnitEncoder。新增普通 EncodeFunction<T> 与保存元素 codec 的
Option/Array/MutableArray/ArrayList encoder，encodeList 同样显式接收元素 codec。
Json.encode 接收所选 codec；data 值本身不再得到编码接口。

删除容器条件编码字段、专用成员筛选/具体化/导入分派，以及 tuple 编码模板、生成键
和额外装箱接口表，保留普通类型、闭包与接口路径。HIR identity/interface/semantics
版本分别升至 8/56/21，MIR type bridge 升至 14；descriptor、指纹和旧版本拒绝测试
同步更新。core bootstrap、LIR 与 runtime ABI 未变。

macOS 已通过 7 项显式 codec HIR 测试和 8 个协议正式 fixture（22 个进程、4 份
stage golden），包括精确诊断、普通/移动 GC、删除源码后的独立产物链接运行。
扩展后的容器组合 fixture 又通过 5 个进程，覆盖四种核心容器、encodeList 和 tuple
函数值 codec 的普通/移动 GC 运行。
HIR/identity/MIR/slib 的相关回归分别通过 877/340/345/601 项；格式化与 workspace
all-targets clippy 通过。自动派生及其 fixture 尚在迁移，本节不代表新版 M29 完成。

此前 GNU/musl 旧协议的全量正式进程已中断；报告保存在 /tmp，清理约 63.7 GiB 的
fixture 工作目录。两端源码和 golden 与已提交版本逐文件核对后备份现场，并同步到
协议修订基线，保留 musl/M28 worktree 与 native unwind 文件。Linux 新协议验证待
后续同步实现提交后执行；历史旧协议通过数不并入本次验收。

## 2026-10-06：codec 协议修订，待实现迁移

当前设计改为 companion/普通 codec 实现 `Encodable<T>` 与 `Decodable<T>`，
encode 显式接收数据值；Json 的两个入口都接收 codec。泛型的两个方向均注入字段
codec，数据类型不再需要编码 bound；容器/tuple 的旧条件编码改用普通 helper 和
函数值组合。父子数据类型的 companion 独立，字段不会向基类 companion 回退。

本次只修订 M29 设计、三份 spec 和路线图，尚未修改编译器、core、JSON 或 fixture。
截至修订前提交 `795a4b26e`，以下实施记录中的实例 Encodable、条件编码及其
验收结果均属于旧协议，不代表新 `Encodable<T>` 已实现。已有泛型 companion、
annotation/shape、普通构造、JSON 格式处理及独立正确性修复继续作为迁移基础。

后续按 [设计第 9.3 节](DESIGN.md#93-2026-10-06-协议迁移) 逐功能实现并提交：
先迁移协议/标量/Unit/JSON，再迁移 codec 上的派生与依赖，随后替换容器和 tuple
编码并删除专用条件关系、生成键和分派记录。按实际产物变化升级兼容版本，同步
正负 fixture、stage golden，并重新完成 macOS、Linux glibc/musl 的正式验收。
此次文档修改不预先记录新协议的测试通过数。

## 修订前的实施顺序与验收约定

1. 普通编码/解码协议、标量 codec 和 JSON 库，建立显式 codec 的真实运行闭环。
2. 完整宿主限定和泛型 companion，贯通单态化、独立初始化及跨 Cone ODR。
3. 自定义 annotation、共有静态 shape 及产物读写。
4. record 派生、字段定制和正常 constructor/default 求值。
5. enum、tuple、核心容器及显式 generic decoder 组合。
6. final class、递归/异常/Context/GC 组合与正式总验收。

各功能先格式化、lint，再进行相应正式 CLI、negative 和 stage golden 验证。
完整验收包括 workspace tests、fixture runner 单测及全部文件 fixture，并使用
`nuc12:~/repos/scoop` 验证 Linux glibc/musl。

## 构建环境与目录

macOS 使用 `/opt/homebrew/opt/llvm@22`（22.1.8）；默认 PATH 的 LLVM 23.1
不用于 Scoop 后端。开发构建关闭增量缓存与调试信息，定期检查并清理构建中间文件。
`target/m28-darwin` 是已有 Git worktree，清理 Cargo 缓存时不删除该 checkout。

## 普通协议、标量 codec 与 JSON

已实现 `Encodable`、`Decodable<T>`、三种容器协议、带路径的编码/解码异常、
`UnitDecoder` 和普通函数值适配类 `DecodeFunction<T>`。Boolean、八种整数、
Char 和 String 使用普通成员方法编码，各自 companion 实现具体解码接口。
整数源码按类型拆分，每个文件不超过 131 行；原 fixture 的文件路径与源码替换同步调整。

`scoop.json` 是独立普通库，使用有序成员列表和 typed JSON 值树，完成对象、数组、
单值、整数精确范围、JSON 数字语法、转义/Unicode、重复 key、完整消费及路径检查。
新增实现文件不超过 124 行。JSON 与 core 均无新增 runtime C 入口或反射表。

独立 provider/JSON/consumer 验收发现并修复共有继承查询遗漏依赖 identity graph 的问题：
某个依赖的 `Decodable<Identifier>` 不一定被最后一个产物直接引用；共有查询现在按
typed ID 读取原已验证图。修复没有复制机器物化根、增加 wire 字段或修改 runtime ABI。

macOS 已实际通过标量和容器 fixture 的普通/移动 GC 运行、三项带精确位置和消息的
negative，以及删除源码后的 artifact-only link/run。四份 HIR/MIR/LIR golden 保留普通
interface、companion、函数值和构造调用。另通过 40 项共有 HIR 测试、38 项 fixture
runner 单测、workspace 格式化/lint 和三个 release CLI 构建。整数源码拆分相关的
35 项 core operations fixture 与 core builtin binding fixture 也已通过，共 134 个进程。

阶段性清理了旧 `target/m29-scalars` 工作目录和手工 core/JSON `.slib`，保留实际报告。
后续仍须实现完整宿主 companion、annotation/shape、自动派生与核心组合 codec；
本节不代表整个 M29-1 或 M29 已完成。

首批实现提交为 `754e85b17`。该提交在 nuc12 使用 LLVM 22.1.2 完成 release CLI
构建和 workspace 格式化/lint；glibc、musl 各通过全部 6 个 M29 fixture，分别为
20 个进程、4 份 golden。各平台 HIR/MIR 一致，新增两份 Linux LIR golden。
随后清理已验收的 macOS core operations、core binding 和首次 artifact 工作目录，
回收约 1.6 GiB，JSON 报告另存 `/tmp`；保留当前 M29 fixture 结果及既有 worktree。


## 完整宿主限定与泛型 companion

已贯通 `Box<Int>.Companion` 的类型、值、转发成员和方法引用；命名 companion、
透明别名及不同宿主种类沿同一查询处理。companion 使用直接宿主的 binder/bound，
方法自身参数保持独立。普通 nested 类型继续具有独立作用域；缺少宿主实参、`_`、
非法宿主 bound、方法参数重名、实例/词法值遮蔽及不同 application 的赋值均诊断。

每个完整 application 拥有独立 exact 类型、存储、初始化 gate 和失败缓存。参数不影响
字段布局的空 companion 也保持类型及实例区别。隐藏构造模板、默认值、局部泛型函数、
闭包、Context 和直接初始化依赖使用原声明身份及完整实参；跨 Cone 物化不重复生成
源码声明。多个使用方的相同 application 经已有 nominal ODR 合并为同一个 singleton。
泛型初始化依赖和普通外部 property/object 的依赖均保留在已有初始化数据中。

新增 13 个正式 fixture：10 个独立 negative，以及 application、失败缓存和删除源码后的
artifact-only 场景。macOS、Linux glibc/musl 均已逐项通过普通及 moving GC 运行；
包含 struct/class/enum/interface 宿主、空字段、泛型方法、初始化中的闭包和局部泛型
默认值、接口分派、绑定函数引用与 Context。HIR/MIR/LIR golden 保留普通调用与完整
application；三平台分别提供 LIR golden。另通过 81 个相关旧 fixture 和 38 个 runner 单测。

产物兼容版本按实现规范 2.17 更新，缓存及 profile 固定向量同步迁移；未修改 runtime C
ABI。旧测试中无实参访问泛型 companion 的源码已迁移，静态 nested import 仍保留。
新增实现按类型限定、字段/调用、具体化身份和初始化、导入模板分模块；既有初始化文件
从 544 行降为 481 行。清理已验收的回归和 artifact 工作目录，报告移至 `/tmp`，保留
`target/m28-darwin` worktree。注解、静态 shape、派生与最终完整 fixture 验收仍待后续批次。

Workspace 的完整 release 测试与文档测试已通过（5292 项），随后对限定名解析
补充并运行 parser 回归，确保 `Unit.names.Item` 仍按普通包路径解析。

## 编译期注解与产物保存

已实现普通 `annotation class`、Boolean/String/Char/定宽整数参数、常量 default，
以及位置/命名参数绑定。应用补齐参数后保存实际标量常量，并保留声明身份与源码顺序。
`const val` 引用复用普通访问及常量检查，包括完整的泛型 companion 限定；读取常量
不触发 singleton 初始化。注解没有运行期类型、实例或 constructor。

注解使用独立 `PersistentAnnotationId`，名称按普通类型名称域处理；顶层、嵌套、
别名、跨 Cone 导入和 re-export 保留同一个声明。type、variant、struct/variant 字段
及 class/interface logical property 使用原 typed target。primary 普通参数、函数、
全局属性及值类型 computed property 的不合法应用均报错。注解不会复制到 accessor
或隐藏存储，也不会使私有声明变为可导入名称。

`SerialName` 和 `Transient` 是 core 中的普通注解声明，前端按实际 prelude binding
识别其身份并检查目标及共存规则；用户同名注解仍是普通静态数据。字段 default 和
wire 名唯一性的派生检查属于后续方法合成批次。本批没有新增 runtime C 接口。

HIR dump 展示注解声明、实际参数类型、完整常量及原目标。共有接口保存声明与应用，
foundation 保存独立声明键；兼容版本为 HIR foundation `/5` 和 interface `/52`。
reader 在原源码接口边界检查常量种类、参数个数、目标及引用，复用既有来源和依赖表。
公开类型上的私有注解作为所需静态数据保留，消费方仍不能按源码 import 访问它。

新增 40 个正式 fixture，macOS、Linux glibc/musl 均已通过，分别为 51 个进程和 4 份
stage golden。包含全部标量边界、别名、嵌套、泛型 companion 常量、私有注解支持，
以及删除 provider/facade/consumer 源码后的产物消费、链接和普通/移动 GC 运行。
FFI 注解继续只接受原有字面量形式；用户注解的常量路径不会放宽该规则。

workspace 完整回归及四个修正包的复跑共验证 5299 项 Rust/文档测试，fixture runner
的 38 项单测通过。新增 Rust 实现最长为 256 行。Linux 的旧 companion 验收目录已
保存报告后清理，回收约 487 MiB；既有 worktree 与当前使用的构建输出保留。

90 个相关旧 fixture 已通过，覆盖 FFI、const、property、companion、初始化、可见性、
import 和 enum（97 个变体、139 个进程、152 份 golden）。HIR/MIR/LIR 快照同步保留新增
核心编码协议及内部引用索引；原有运行行为与 negative 诊断均按预期完成。
统一静态 shape 查询、构造/default 关联和编码/解码方法合成仍待后续功能批次。


## class 主构造参数与 property 关联

共有名义声明现在保存 class 主构造的实际 constructor identity，以及按参数序排列的
logical property identity；普通参数保留空项，无主构造与零参数主构造明确区分。
名称、类型、可见性和 default 沿原 callable/source parameter 数据读取，computed
property、类体存储及继承字段不会混入主构造参数映射。没有复制 default 正文或布局图。

producer 从原 class 字段的 primary parameter 关系投影；reader 在已有名义声明边界
检查构造归属、参数数量、property 与普通 backing field 的类型关联。新增数据位于
`NominalDeclarationDetailsV1` field 11，共有接口兼容版本升至 `/53`，其余 section
及 runtime ABI 不变。格式固定向量和旧版本拒绝断言已同步。

新增两个正式 fixture，分别覆盖本地编译和删除 core/provider/consumer 源码后的
产物消费、独立链接、普通及 moving GC 运行。泛型 class 的普通参数、`val`/`var`、
private 存储、computed property、继承、次构造器、空主构造及仅次构造 class 均覆盖。
macOS、Linux glibc/musl 各通过 10 个进程和 4 份 stage golden；HIR/MIR 跨平台一致，
LIR 分别保留三平台快照。相关 HIR、HIR lowering 和 slib 共 2810 项 Rust 测试通过，
workspace 格式化/lint 通过。统一静态 shape 查询及编码/解码方法合成继续在后续批次实施。

## 统一静态 shape 查询与 HIR dump

共有 HIR 提供借用式静态查询，覆盖名义类型、核心 intrinsic、tuple、Unit、函数、
指针及带原始 bound 的类型参数。字段、variant、logical property、annotation 和
主构造参数/default 均沿原 typed identity 与声明关系读取；class 基类不扁平化，
enum payload 保留分支层级，companion 保留完整宿主实参及自身存储。

泛型字段复用原签名映射执行一次参数替换，开放实参保留调用方 binder，递归字段只
返回类型引用。尚未载入类型 arena 的核心父接口直接借用依赖产物的共有签名。
导入字段保留普通字段、property backing、delegate 与生成字段类别；object backing
字段名称从原 property 声明读取。查询不扩张源码访问域、不触发初始化或机器物化，
不新增 wire 版本、runtime metadata 或类型/布局图。

`--emit=hir` 的原 HIR dump 增加 Static Shapes 区段，展示字段类型与身份、注解、
variant、父类型、property 存储类别，以及原主构造/default 关系。driver 复用本次
编译已建立的依赖上下文。签名映射中的 nominal 辅助函数拆分为子模块，新查询实现
按名义类型、字段、property、注解、构造和参数划分；新增源码模块最长为 219 行。

六项查询测试分别走本地多文件与独立产物路径；相关 HIR、HIR lowering、slib 共
2816 项 Rust 测试通过，两端 workspace lint 通过。新增两个正式 fixture，覆盖
泛型/递归字段、scalar/intrinsic、函数与指针、委托 property、接口、companion、
缺省构造和注解；删除 core/provider/consumer 源码后独立链接并普通/移动 GC 运行。
macOS、Linux glibc/musl 各通过 10 个进程、5 份 golden；共有 HIR/MIR 完全一致，
三平台 LIR 分别保存。已有 63 个 M29 fixture 回归通过（115 个进程、23 份 golden）。

两端已清理调试增量缓存，保留当前 CLI 和既有 worktree。编码/解码方法合成、核心
条件组合 codec，以及完整 workspace/文件 fixture 总验收仍在后续批次实施。

## 名义类型的自动编码

struct、enum 和没有 class 基类的普通 final class 可通过显式 `Encodable` conformance
请求缺失的 `encode`。继承选择先保留合法用户方法、基类方法和 interface default，
仅在普通 obligation 分支登记合成成员；无关 overload 不阻止合成，错误 override
与 default 冲突仍走原诊断。core 接口从普通绑定取得 typed identity，同名用户接口
不触发派生。

合成成员使用原源码 owner、完整签名、参数接口与普通方法身份；全部签名建立后，
按共有 HIR 的字段、variant、logical property 和注解生成普通块，复用原 body lowering。
Export HIR 保留完整 body/template，跨 Cone 不重新派生。没有新增 IR 执行指令、
wire 版本或 runtime 入口。公开存储 property 的生成 accessor 与用户自定义 accessor
按已有来源类别区分；computed property 不参与，Transient 排除存储字段，SerialName
按实际 core annotation identity 更改 wire 名称。重复名称与缺失字段能力在定义处报错。

新增实现按登记、字段/property、普通语法块和 variant 划分，最长文件 167 行。
八项语义测试及全部 1346 项 HIR lowering 测试通过，两端 workspace lint 通过。
新增 23 个正式 fixture：19 项精确诊断，以及 record、enum、实现选择和独立产物组合。
覆盖泛型 bound、接口调用、四种 variant、递归 class/enum、default 不被编码重复求值、
删除 core/JSON/provider/consumer 源码后的链接，以及普通和 moving GC 运行。
macOS、Linux glibc/musl 各通过 41 个进程、10 份 golden；七份共有 HIR/MIR 字节一致，
三平台 LIR 分别保存。macOS 的全部 86 个 M29 fixture 回归通过，共 156 个进程、
33 份 golden。已保存报告并清理两端四个完成的测试工作目录，回收约 956 MiB。

本批完成名义类型的自动 encode；自动 decode、核心容器/tuple 的条件组合以及
完整 workspace/文件 fixture 总验收继续按后续批次实施。

## 核心容器的显式解码器

`Option<T>`、`Array<T>`、`MutableArray<T>` 和 `ArrayList<T>` 的完整宿主 companion
提供普通 `decoder(element: Decodable<T>)` 方法。每次调用返回持有该元素解码器的
普通对象，容器自身的 T 保持无 bound，也不把元素策略保存到 singleton。Option
沿 enum 的单 variant record 格式读取 None/Some；数组沿 unkeyed sequence 读取，
三个数组实现共用普通库循环，再按结果类型完成正常构造或数组转换。

两个新增核心源码文件分别为 24、22 行；接口调用、泛型模板、异常和 GC 全部沿既有
编译路径，没有新增编译器协议或 IR/runtime 表示。用户异常保留其具体类型、消息和
所携带的引用，遵守原 throw/catch 对异常 payload 的复制与物化规则。

新增四个正式 fixture，覆盖空容器、嵌套 Option/数组、整数边界、Unicode、错误路径、
不完整元素 codec、Unit、无 Encodable 的对象及函数值、独立元素策略、用户异常，
以及删除 core/JSON/provider/consumer 源码后的产物消费和独立链接运行。macOS、
Linux glibc/musl 各通过 22 个进程、10 份阶段 golden，包含普通与 moving GC 运行；
三平台均完成不更新快照的复验，七份共有 HIR/MIR 字节一致，六份 Linux LIR 独立保存。
两端 workspace 格式化/lint 通过，测试报告保存后清理本批工作目录。自动 decode、
核心容器/tuple 的条件 Encodable 和完整总验收继续实施。


## 名义类型、tuple 与组合字段的自动解码

companion 或普通 class/struct/object 对 `Decodable<R>` 的缺失实现现在生成普通
`decode` 方法。合法手写、继承和 interface default 仍优先；无关 overload 不阻止
合成，非法 override 和额外 requirement 保留原诊断。结果 R 的普通 struct、enum、
tuple 和合格 final class 按实际 shape 展开，singleton 明确拒绝自动构造。

字段解码器在定义处按 primary val 依赖、递归 this、完整 companion application、
核心容器或 tuple 组合选择，并保存实际 typed 引用。两个 binder 具体化成同一类型后
仍使用原依赖；tuple 闭包沿普通 lambda 的参数、捕获、词法来源和正文登记流程，
共享实现抽取为独立模块。合成方法登记与 encode 共用现有成员签名建立流程。
没有新增 IR 指令、跨阶段解码计划、wire 格式或 runtime ABI。

生成代码先读取所有提供的字段，并结束该层容器，再按构造参数序补 default 和
Transient 参数；缺失字段不求值其 codec。enum 在补 default 前同时完成外层检查，
unit variant 直接构造普通值。正常 primary/variant 构造、访问域、泛型替换、vararg
整数组及 default 模板路径共用原实现；class 完整初始化后才返回。

共有依赖数据支持消费方为导入类型合成方法，保留字段/annotation、主构造关联及
原 default 绑定。独立产物用例删除 core、JSON、两个 provider 和 consumer 源码后
链接运行，并区分两个 Cone 中具有相同 FQN 的类型、companion 和构造器。

新增解码模块最长 186 行，共享 lambda 主模块由 347 行缩为 268 行，参数处理
单列为 112 行模块。八项新语义测试与全部 1354 项 HIR lowering 测试通过；后续
unit variant 和 singleton 修复在两端通过全部 16 项自动编解码语义回归。

新增 43 个正式 fixture，其中 34 项核对精确错误位置和信息，9 项覆盖源码与组合运行。
涵盖四种 enum variant、嵌套 Option/数组/tuple、泛型依赖、原始及缺省 vararg、普通
接口分派、companion 惰性初始化、default/constructor/codec 异常、动态 Context、
独立产物和 moving GC。macOS、Linux glibc/musl 各通过 82 个进程、28 份阶段 golden；
三平台又分别通过全部 9 项 stage fixture 的不更新快照复验。19 份共有 HIR/MIR
字节一致，18 份 Linux LIR 独立保存。两端 workspace 格式化/lint 通过。

本批完成自动 decode；核心容器与 tuple 的条件 Encodable，以及完整 workspace/
文件 fixture 总验收继续实施。报告已归档，完成的测试工作目录按批清理。


## Unit 的普通核心编码实现

Unit 由五行普通 core 源码声明提供 Encodable，encode 调用
`singleValue().writeNull()`。其 intrinsic 声明复用原固定 Unit identity；
`Unit`、`()`、零大小布局和返回 ABI 保持不变，UnitDecoder 仍是独立 object。
成员查找、泛型上界、接口装箱、绑定引用和跨 Cone 分派沿既有 intrinsic struct
路径，共有声明与机器布局消费同一份实际接口信息。

组合用例同时修复普通类型测试的操作数表示：值通过既有装箱转换为引用，
智能转换形成的 Unbox 则复用原引用，保留短路和单次求值。新增独立及产物 fixture
覆盖 Unit 方法/接口、普通函数引用、泛型包装、自动 encode/decode、异常、数组、
源码删除后的独立链接及 moving GC。macOS、Linux glibc/musl 各通过 13 个进程、
7 份阶段 golden，5 份共有 HIR/MIR 字节一致。

workspace 回归还修正了测试夹具中的 Unit 固定身份与 intrinsic family 清单，
并区分泛型 companion 的源码初始化记录和实际机器初始化单元。相应 HIR、全部
114 项 MIR lowering 与 driver 定向回归通过；两端 workspace 格式化/lint 通过。
三平台均完成不更新快照的正式复验，报告已归档并清理本批 fixture 工作目录。
核心容器与 tuple 的条件 Encodable、完整 workspace/文件 fixture 总验收继续实施。


## 普通列表编码 helper

核心库新增七行普通 `encodeList<T : Encodable>`，按 List 的迭代次序向 unkeyed
容器逐项编码，最后结束容器；空列表同样结束，元素异常按普通调用传播。List 与
MutableList 自身没有增加 Encodable 接口；泛型实例、函数引用、跨 Cone 正文和
GC 复用既有路径，没有新增 IR 指令、格式版本或 runtime ABI。

三个正式 fixture 覆盖空列表、Array/MutableArray/ArrayList、逻辑元素与清空后的
空闲容量、Unicode、Unit、嵌套编码、列表视图、绑定泛型函数引用、元素求值及异常，
并以独立 provider/consumer 测试删除所有源码后的链接、运行和 moving GC。
负例锁定无 Encodable 元素的泛型上界诊断及精确位置。macOS、Linux glibc/musl
各通过 13 个进程、7 份阶段 golden，均完成不更新快照的复验；5 份共有 HIR/MIR
字节一致，4 份 Linux LIR 独立保存。两端格式化/lint 通过，报告归档后清理本批
fixture 工作目录。容器与 tuple 的条件 Encodable 和正式总验收继续实施。


## 四种核心容器的条件 Encodable

Option、Array、MutableArray、ArrayList 保留无约束的原始类型参数，只有元素满足
实际 core Encodable 时才提供该接口及 encode 成员。合成方法由原容器声明拥有，
正文以独立的有界 binder 检查，并按宿主参数位置正常替换。三个序列容器复用普通
encodeList，Option 调用十二行普通 helper，保留 None/Some 的 enum 编码形式。

条件以完整的元素类型、接口与普通方法选择保存在 HIR 名义声明中；成员查找、
泛型上界、绑定引用、具体化与共有继承查询使用同一条件。正常实例物化就形成
所需接口表，没有静态 encode 调用的值经 Any 擦除后也可测试和分派。递归名义类型
读取完整声明中的接口事实；依赖正文闭包复用原导入缓存，补齐实际 application
所用标量的 core 声明，不依赖偶然的静态成员调用。

共有名义 details 增加 field 12，HIR interface/semantics 分别升至 55/19，旧产物
与缓存重建。MIR 类型桥复用已有共有继承查询，删去重复的父接口投影；没有新增
MIR/LIR 指令或 runtime ABI。新增生产模块最长 161 行，导入泛型主模块由 361 行
缩为 291 行，正文完成逻辑按职责独立为 76 行模块。

新增 11 个正式 fixture，其中 9 项锁定精确错误位置和信息；组合覆盖空容器、
逻辑 size、嵌套/递归、Unit、接口元素、绑定方法、异常及普通 List 视图。
独立产物用例删除 core、JSON、provider、peer、consumer 全部源码后链接运行，
验证重复泛型实例的 ODR、动态接口和 moving GC。macOS、Linux glibc/musl 各通过
22 个进程、8 份阶段 golden，并完成不更新快照的复验；6 份共有 HIR/MIR 字节一致，
4 份 Linux LIR 独立保存。

两端 workspace 格式化/lint 通过，HIR 877 项、HIR lowering 1358 项、MIR lowering
114 项，以及 slib 的 macOS 601/Linux 602 项全部通过。报告已归档，本批 fixture
工作目录清理后提交；tuple 的条件 Encodable 和完整 workspace/文件 fixture
总验收继续实施。

## tuple 的条件 Encodable

非空 tuple 仅在全部元素的静态类型满足 Encodable 时取得该接口。直接成员、泛型
上界、接口调用、绑定引用和 Any 擦除使用同一关系；按实际出现的元素数生成带完整
receiver、元素上界和普通 typed body 的方法模板，没有名义包装或固定 arity 上限。
模板与具体方法使用独立 typed identity，具体方法按完整 tuple 类型进入既有 ODR
合并，生成局部值使用普通 Synthetic 来源。

MIR 在完成实际 box 时统一读取 HIR 的全部接口并生成 adjust thunk；正文装箱、
函数适配和 shape support 的重复补表逻辑已删除。结构 tuple 保持原 exact key，
其 box 拥有普通 dispatch schema。实际装箱所需的私有元素声明沿原支持闭包保留，
包括无编码能力的元素；不增加公开查找名，不收集无关的私有类型。跨库同一 tuple
从静态调用或仅 Any 擦除进入时生成一致的表示和接口表。

本批 HIR identity-foundation 升至 7，interface/semantics 为 55/20，MIR type bridge
升至 13；旧产物与缓存重建，runtime ABI 不变。新增生产模块最长 169 行；成员
查找由 488 行整理为 350 行及 147 行候选模块，装箱分派由 486 行整理为 381 行及
113 行 conformance 模块。

新增 10 个正式 fixture，其中 8 个 negative 锁定诊断位置和信息。组合覆盖单元素、
嵌套和十二元素 tuple、Unit、接口元素、递归容器、泛型闭包、绑定方法、自动解码、
副作用次序、异常、动态接口与 moving GC。独立产物用例删除 core、JSON、provider、
peer 和 consumer 全部源码后链接运行，并覆盖私有非编码元素与跨库重复实例。
macOS、Linux glibc/musl 各通过 21 个进程、8 份阶段 golden，并完成不更新快照的
复验；6 份共有 HIR/MIR 字节一致，各平台另保存 2 份 LIR。

两端 workspace 格式化/lint 通过；identity 340、HIR 877、HIR lowering 1361、MIR
345、MIR lowering 114，以及 slib 的 macOS 601/Linux 602 项回归通过。报告归档并
清理本批 fixture 工作目录后提交，完整 workspace、公共 runner 单测及全部正式
文件 fixture 的总验收继续实施。

## 总验收中的依赖收集回归

M29 的普通值编码方法带来了实际的外部 callable 依赖。将 materialization 根及
本地 dispatch 表目标的收集合并到 driver 原有的 MIR 引用收集入口，正式 CLI
与产物回归辅助路径使用同一结果；不增加阶段验证或第二套选择逻辑。

产物测试继续核对这些 callable 的实际 provider 和物理导入，并更新两组值用例
新增的编码签名类型。泛型正文的位置按现行规范保留定义方来源，默认参数使用点
仍核对消费方来源；测试分别统计 core 支持正文和用例自己的求值位置。

macOS 的 workspace 格式化、lint 和完整 5,331 项 Rust 测试通过，没有失败或
忽略项。Linux workspace 及三平台完整文件 fixture 验收继续进行。

## tuple 条件接口的依赖载入

全量 CLI 暴露默认参数推断、空值相等性和重载组合中的同一问题：tuple 查询只
登记了 Encodable application，依赖接口定义尚未载入。条件查询与模板生成现在
复用普通名义类型解析及其缓存，在查询父接口前取得完整定义，规范 §2.17 同步
明确该顺序。三个原有正式 fixture 均已通过源码、产物和运行回归。

10 个 M29 tuple fixture 在固定工具副本上以不更新快照的方式通过，共 21 个
进程、8 份 golden；macOS 的完整 5,331 项 workspace 测试也覆盖此修复。涉及的
生产文件分别为 155、165、68 行，未新增阶段或运行时机制。

## 既有二进制损坏向量迁移

使用当前正式 CLI 重新生产原 generic delegate 与 program-link 产物，再用原
Slib writer 重建损坏及 optional/required 成员向量。删除的四条 registration
记录与旧向量逐字节相同，交换仍针对原初始化单元；所有片段唯一匹配，组合结果
与 writer 完整输出一致。更新实际摘要、archive 长度和 cone-production 7 的
诊断位置，保留原错误类别、缺失数量、字段角色及禁止发布产物的检查。

三项文件 fixture 通过，覆盖五种 registration 错误、损坏 member、截断归档、
magic 错误、optional 成员正常链接与 required 成员拒绝，以及普通和 moving GC
运行。生成工具只用于此次测试数据迁移，没有进入生产 crate 或公共 runner。

## enum 声明命名空间与 generic companion

导入 enum 的限定调用先读取实际值绑定，变体继续通过原 typed 变体路径推断宿主
实参；只有真正的 companion 成员进入完整宿主 application 检查。该修复不按
Option 名称特判，也不改变变体构造或 companion 初始化规则，语言规范 4.2 同步
明确二者的边界。

新增跨库回归同时验证 payload/expected/显式实参推断、unit variant、未使用
companion 不物化，以及 companion 成员缺少宿主实参仍报错。HIR lowering 的
1,362 项测试通过；macOS 的 37 个相关文件 fixture 全部通过，共 166 个进程、
126 份 golden，覆盖原 23 个 Option 消费用例及完整 companion 矩阵。

既有 NoGC 负例同步记录值类型测试产生的显式装箱及 Any 值诊断，原错误记录和
位置断言全部保留。生产查找模块为 129 行，没有新增数据格式或 runtime 契约。

## 核心继承用例的源码替换维护

六个既有 primitive inherited fixture 按当前 Boolean、Long、String 声明加入测试
接口，并保留类型原有的 Encodable。只更新精确匹配的声明片段，没有改变原继承
方法、默认实现或 NoGC/参数错误规则。六项 macOS 文件验收全部通过，覆盖重新
生产 core、独立 provider/consumer、正常与 moving GC 运行及两项精确负例。

## 跨库回归补充与旧测试输入迁移

Linux glibc、musl 分别通过 43 个相关文件 fixture，各有 218 个进程、146 份
golden，覆盖 enum 命名空间修复、companion 正反例、NoGC 诊断和六个核心继承
用例；与 macOS 同步的 16 份改动文件逐字节一致。

搬迁核心 String 的原有用例同步当前完整声明和编码接口，继续验证角色随实际
声明与透明 alias 迁移。protected nested 用例改用完整 Generic<Int>/Generic<String>
companion，并在方法中使用宿主参数；原保护域、构造与运行断言保留。产物图两条
stale 诊断只更新实际 HIR/MIR/LIR 摘要，仍检查同一 Cone/provider 与相同错误规则。

这三项 macOS 文件 fixture 全部通过，共 33 个进程、53 份 golden。最新完整
workspace 的 5,332 项 Rust 测试通过，没有失败或忽略项；Linux workspace 与三平台
最终文件验收继续运行。

## companion 的完整宿主约束

名义约束解析完成后，同步 companion backing type 及 initializer/ensure 的宿主
参数，保留原参数 ID 和 generic callable ID。字段、父接口、方法及初始化复用
同一结果；约束验证跳过共享宿主参数的 object backing，避免再次检查相同声明。
该阶段整理为 103 行模块，主 pipeline 从 617 行缩至 567 行，无新格式或 ABI。

新增独立运行 fixture 覆盖 struct/class/enum/interface companion、接口和类上界、
value/ref kind、F-bound、受约束字段/父接口、默认参数与 exactly-once 初始化。
原 source-nominal 的两个工厂用例迁移到完整宿主实参，保留私有构造、保护域、
跨 Cone 产物消费与 moving GC 断言。

四项 companion 单测通过；macOS 的全部 160 个 M29 fixture 及这两个组合用例
共 162 项全部通过，实际执行 347 个进程、132 份 golden。新完整 workspace 和
Linux 验证继续进行，最终结果在总验收后记录。

## Darwin 正式全量验收

2026-10-06：Darwin 的普通 `--all` 已完整通过。发现并选择全部 2,480 个声明，
2,474 个适用 fixture 全部通过，6 个目标不适用；没有失败、配置错误、环境错误
或中断。实际执行 2,564 个变体、12,409 个进程和 12,409 次快照检查，包含全部
160 个 M29 fixture；本轮没有使用更新快照、筛选或缩小 suite 的选项。

已将报告的通过及不适用集合与当前声明逐项比较，并保存 11,864 个已验证快照路径的
内容摘要，供 Linux 目标完成后比较共有输出。正式报告归档在
`/tmp/scoop-m29-saved-reports/m29-final-darwin-all.json`，完整日志为
`/tmp/scoop-m29-final-darwin-all.log`。

最新 macOS workspace 的 5,333 项 Rust 测试全部通过，零失败、零忽略。测试使用
`CARGO_PROFILE_DEV_OPT_LEVEL=1` 和 `CARGO_PROFILE_TEST_OPT_LEVEL=1`，保留默认的
debug assertions 与 overflow checks；完整日志为
`/tmp/scoop-m29-final-darwin-workspace4.log`。格式化、workspace clippy 和公共
fixture runner 的 38 项单测也已通过。

GNU 和 musl 的 14 个 companion fixture、两个 source-nominal 工厂用例均已通过。
新增宿主约束用例的输入和 HIR/MIR 在三平台逐字节一致，两个 Linux LIR 已提交。
两种 Linux 目标的三个旧产物失效用例也已在更新实际摘要后完整复验通过；诊断的
错误类别、Cone/provider、消息结构和位置均保持原断言。

Linux workspace 及 GNU/musl 最终文件验收继续进行；本节只记录已经取得的结果，
M29 的完成状态将在全部验收结束后更新。

## Linux workspace 与原生链接复验

最新 Linux workspace 已完整通过：43 组、5,361 项测试，零失败、零忽略。同样使用
dev/test `opt-level=1` 并保留 debug assertions 和 overflow checks。完整日志已在
两个宿主保存为 `/tmp/scoop-m29-final-linux-workspace5.log`。

GNU/musl 文件 fixture 与 Rust 端到端测试同时运行时，GNU 的十个原生链接用例
达到原有 120 秒构建时限。停止该更新轮后保存所有完成结果；将代表性的
`native-link-cabi-packed` 改为单用例复验，在原时限内通过普通与 moving GC 的
构建、运行及删除源码后的独立链接，共 2 个变体、10 个进程、10 次快照检查。
没有修改 fixture 的时限或断言。GNU 后续更新使用最新固定工具和 4 个并发任务，
只运行尚未取得通过结果的用例；其余超时项及最终普通 `--all` 仍继续验证。

已归档中断更新轮和复验报告，清理约 33 GB 的旧 GNU 工作目录。Darwin 正式全量
工作目录约 79 GB 也已清理，日志、报告、已验证的快照摘要和最新工具保留；原有
M28 worktree 不在清理范围内。

## 显式观察的缓存锁范围

Linux 原生链接复验进一步发现：显式 stage dump 即使已命中缓存，仍在同一编译键
的独占锁内重新编译。同一 core 的观察请求因此串行等待，普通命中也会等待这些
不用于缓存去重的编译。实现规范 §2.7 先明确锁的范围，再调整执行：观察请求独立
完成编译、产物检查和 dump 输出，只在缓存比较及原子发布时获取独占锁；普通
未命中仍持锁再次查询并去重编译，普通命中继续复用已有产物。相同产物复用、
nondeterminism 诊断和失败不发布的规则保持，无新格式或协议。

新增 71 行独立测试模块，用另一文件描述符直接尝试获取实际缓存键的锁，确定性
检查观察请求可并发、普通未命中仍互斥，并验证失败不留下缓存条目。执行模块共
257 行。两端先通过格式化和 workspace clippy，再通过全部 scoop 包测试：macOS
95 项、Linux 97 项，包含已有缓存等价、冲突及损坏检查。

三平台各 5 个相关 CLI fixture 全部通过，每个平台执行 9 个变体、52 个进程和
108 次快照检查，覆盖冷/热缓存、显式 dump、失败保留旧产物，以及原生 object、
archive、TLS callback 的正常和 moving GC 运行。Linux 中原先超时的四项在原有
120 秒单步时限内完成，没有放宽测试规则。macOS 已开始修复后的普通全量复验，
Linux 用新工具继续剩余快照更新和最终验收。

已保存旧 Linux 更新轮的全部完成结果并清理约 50 GiB 工作目录；独立检查发现
GNU 的 6,841 个、musl 的 6,657 个已通过快照路径与 Darwin 逐字节相同。最终跨平台
比较会在全部目标完成后记录。

## 缓存锁修复后的 Darwin 正式全量

修复后的固定三工具再次完整通过普通 `--all`：全部 2,480 个声明中，2,474 个
适用 fixture 通过，6 个目标不适用；包含全部 160 个 M29 用例，没有失败、错误
或中断。共执行 2,564 个变体、12,409 个进程及 12,409 次快照检查，未更新快照、
筛选用例或缩小 suite。已再次将报告中的两个集合与当前声明逐项核对。

本轮报告为 `/tmp/scoop-m29-saved-reports/m29-final-darwin-all2.json`，日志为
`/tmp/scoop-m29-final-darwin-all2.log`。既有 Darwin 快照保持原内容，继续用已保存的
11,763 个实际文件的摘要作为 Linux 共有输出的基线。GNU/musl 完成的更新结果均已保存，
两目标各使用 6 个并发任务继续剩余用例；最终普通全量验收随后运行。

## GNU 全量快照更新与共有输出比较

GNU 全部 2,455 个适用 fixture 已取得通过结果，最后一批 333 项无失败。共有输出
与已通过正式全量验收的 Darwin 逐字节比较一致；同步 Linux 目标快照后，本次
提交更新 1,939 个实际文件。GNU 已用最新固定三工具启动普通 `--all`，不更新
快照、不筛选用例、不缩小 suite，最终结果另行记录。

清单中的 `../goldens/` 引用曾使同一文件按多个路径统计。按实际路径合并后，
Darwin 的 11,864 个路径对应 11,763 个文件，GNU 的 11,701 个路径对应 11,645 个
文件；GNU 与 Darwin 共有的 9,635 个文件全部相同，其余 2,010 个为目标所需的
文件。路径别名均指向相同内容，没有改变 fixture 的进程、变体、快照检查次数或
通过集合；原路径清单和摘要备份保留，后续统计采用去重后的文件数量。

## musl 全量快照更新与三平台共有输出

musl 全部 2,457 个适用 fixture 已取得通过结果，最后一批 465 项全部通过。去重
后的 11,645 个快照文件中，与 Darwin、GNU 各自共有的 9,635 个文件全部逐字节
一致；本次同步其余目标文件，更新 1,939 个实际快照文件。正式普通 `--all`
已启动，使用最新固定三工具和 6 个并发任务；最终报告仍需完成后确认。

两个 Linux 目标的更新报告与比较结果均已归档。共享文件沿用经过确认的内容，
只有目标所需的快照进入对应提交，没有复制一份平行的 fixture 或编译实现。

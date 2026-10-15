# M38 实施记录

基线：`f75704bac`。M46 与 M48 已完成；本期只在 M48 最终提交之后开始实施。当前完整范围见 [设计](DESIGN.md)，未完成部分不能以基础库通过代替。

## M38-0：文本 writer

先交付 debug 与 CLI 共用的普通文本接口。语言规范 11.16 定义同步 String 写入、普通异常传播、显式换行及 StringWriter 的快照行为；core 使用 13 行普通 Scoop 实现，复用既有 StringBuilder。

独立 fixture 覆盖空值、Unicode／零字符、接口调用、重复 build、后续追加不改变已有 String 和独立实例。组合 fixture 经真实 provider `.slib` 消费泛型 writer 函数和捕获接口的 closure，验证 GC、异常原样传播、已写内容保留及后续正常写入；两者都声明普通和 moving-GC 运行。negative fixture 精确检查非 String 实参的位置与类型诊断。

本机 fmt 与整仓 Clippy 通过，无警告；当前 release 工具执行 3 个新 fixture 和 3 个 StringBuilder 相关回归，最终 6 项全部通过，6 variants、18 processes，未运行无关全量测试。组合首轮误断言 catch 的 managed 对象与 throw 实参引用相同；按运行时规范 5.3～5.4 的异常物化合同改为检查异常类型与消息，只补验该组合，普通／moving-GC 均通过。编译器和 runtime 未因该测试期望改变。

Linux 留待本机完整功能批次稳定后统一验收；本批不宣称 M38 完成。原始日志与逐次报告保存在仓库外。已结束的 fixture case 目录在保存结果后清理，共享编译缓存和 Rust／release 增量结果继续保留。

## TypeInfo 普通数据层

语言规范 9.6 明确类型引用、当前层描述、成员／构造映射、类型使用树和注解数据的 API；实现规范 2.17 与运行时规范 2.2 同步说明普通值／闭包表示及 GC 行为。实际相等接口为 `Equals<T>`，已同步修正设计中的旧名称。八种整数用 IntegerValue 保留类型，FloatBits／DoubleBits 保留原始位型。

core 新增六个普通 Scoop 模块，提供 TypeInfo／TypeRef／TypeShape、七种独立成员／声明引用、全部形状记录、类型使用位置、AnnotationKey／AnnotationValue 与 findAnnotations。引用的 equals 只比较其 identity 数据；展示名、对象地址和描述闭包不参与。模型没有 runtime 专用入口、TD 扩展或新的 wire 格式。describe 只执行所持普通闭包，不主动遍历子引用；普通数据构造不赋予声明或成员访问能力。

本机 fmt、Clippy 通过，无警告。3 个新 fixture 与 3 个 writer 回归最终均通过；新增独立运行覆盖同名不同身份、同身份不同展示名、20 层按需递归、闭包异常和 GC。跨 Cone 组合通过实际 provider `.slib` 传递泛型 TypeInfo 包装、field／constructor 参数映射、声明／字段／type-use 注解分离、repeatable 筛选顺序、空数组元素种类、嵌套注解及标量边界；两组运行均通过 moving-GC。negative 精确检查 PropertyRef 不能传给 FieldRef 参数；记录诊断后只补验该项。日志和报告保存在仓库外，共享缓存继续保留，结束的 case 工作目录清理。

本批仅交付可运行的普通数据层。用例主动构造描述值以检验库合同，**不把它计作编译器自动查询的验收**；typeInfo／annotationKey 源码查询及描述生成仍待实现，M38-1 尚未达到退出条件。Linux 仍留待本机完整功能批次稳定后统一进行。

## TypeInfo 自动查询

公开 typeInfo<T>() 由普通 core 泛型函数实现，内部引用数据和当前层描述 intrinsic 在 concrete HIR 生成普通值、数组、闭包和函数调用。子类型使用惰性 TypeRef；精确类型字符串从既有 exact-type 身份表读取，MIR 不重算身份，也不增加 runtime ABI 或运行期注册表。七种成员引用及 AnnotationKey 使用普通 struct。

生成逻辑复用静态声明视图，覆盖全部类型形状、字段／property／accessor、基类／接口、构造参数映射、原声明 Option 事实、类型使用路径和注解参数／默认值。property override 的已绑定身份随共有接口保存；描述依赖的注解参数、计算属性结果和未调用构造器参数类型走正常加载流程。注解声明的惰性辅助函数保留实际 annotation identity 与查询所在 callable template，并继承其实例化上下文；跨 Cone 复用现有 ODR 归属，避免重复 Strong 定义。

本机 5 个新 fixture 最终均通过，6 个 writer／普通数据模型回归也通过。正例实际运行普通和 moving-GC 两种模式；总计 11 项、11 variants、33 processes，不保存普通功能 IR 快照。覆盖同型递归、透明 alias、泛型原声明 Option、enum payload／integer enum、私有／计算／委托属性、override、构造关联、泛型 companion、suspend receiver／FunPtr／原生指针、Nothing／atomic、局部泛型／捕获／inner／anonymous 类型。真实 provider .slib 与 consumer 验证普通和泛型查询、嵌套／repeatable 注解、空数组元素种类、浮点原始位型，以及仅由描述引用的类型。负例精确检查 annotation 不能作为 typeInfo 的普通类型实参。

fmt、workspace Clippy 与 release 构建通过。受影响 Rust 测试覆盖 generated callable 15 项、property interfaces 43 项、core protocol surface 10 项、static shapes 9 项、artifact profile 17 项；旧字段数／未知 tag／版本指纹断言按新格式修正后只补验对应测试，全部闭合。HIR core-bootstrap-interface 为 16，cross-cone-interface 为 77；HIR／MIR／LIR identity-foundation 为 10／8／9，旧产物须重建。原始日志与报告继续放在仓库外。

这批交付自动 typeInfo 查询；公开 annotationKey<A>() 尚未实现，不能据此宣称 M38-1 或 M38 完成。Linux 按用户要求留待本机完整功能稳定后统一验收。

## 注解声明查询

公开 annotationKey<A>() 绑定实际 core 查询声明，以正常名称与可见性规则解析注解实参；alias、限定类型路径及 re-export 保留原声明身份，同名普通函数不获得特殊语义。HIR 及共享正文保存 PersistentAnnotationId，默认参数引用表保存对应声明依赖；没有把注解声明伪装成值类型，也没有新增 runtime 入口。声明描述复用已有惰性数据生成，辅助函数沿实际函数或构造器归属及实例化上下文发布。

15 个新 fixture 最终全部通过：独立查询覆盖注解元数据、参数默认值、repeatable 顺序、相等、递归描述及同名声明；三 Cone 组合覆盖私有声明的泛型正文、默认参数、re-export、closure、泛型 class 初始化及 struct 构造器。两个正例均通过普通与 moving-GC 运行。13 个负例精确检查实参数量、类型参数／普通类型／未知声明、推断参数、值参数、nullable／type-use、private 访问、公开默认参数的访问域、callable reference 和 NoGC。只对失败或修改的用例补验；跨库用例的顶层函数调用按规范改用导入短名，NoGC 期望保留结果类型与分配两项诊断。

跨库闭包暴露的未使用泛型 binder 缺少规范类型记录已修复：创建 TypeParamId 时同时 intern 对应 Type::Param；13 项泛型正文／默认参数 Rust 回归通过。212 项共享表达式、引用闭包与 core protocol 测试及 3 项 profile 测试通过。fmt、workspace Clippy 和 release 构建通过。cross-cone-interface／type-semantics 为 78／30，core protocol、identity foundation 与 runtime ABI 保持不变，旧产物须重建。

受共享辅助函数归属影响的 TypeInfo 独立及跨库两个 fixture 也通过，2 variants、8 processes。原始日志与报告仍保存在仓库外，仅在本文保留结果概要；Linux 留待本机完整功能稳定后统一验证。

## Inspectable 与 DebugPrinter

语言规范 11.17 定义普通值视图、显式 inspector、惰性读取和 debug 输出；实现规范 2.17 与运行时规范 2.2 同步普通闭包、引用身份及 GC 合同。core 提供 ValueObservation、各类形状和成员视图、标量／Option／序列 inspector、显式 opaque 策略及 DebugPrinter。Compact／Pretty、Unicode scalar 截断、Ignore／Redact、深度／项数控制和当前祖先路径的循环检测均由普通 Scoop 实现；基类投影复用已进入对象，不误报循环。

HIR 在正常 requirement 选择后补全缺失 inspect，复用 codec 的实际 core 声明、显式依赖、成员访问和普通调用。支持 struct、三种 enum payload、整型 enum、tuple 与无 class 基类的 final class。合成器不读取 computed property；不合法的存储 accessor／delegate、private／protected／跨 Cone internal 读取、字段能力缺失及歧义在定义处报错。已绑定读取闭包及泛型依赖沿现有共享正文发布，不增加 wire 格式或 runtime 入口。core 自身的类型描述 intrinsic 同时登记普通来源上下文，支持库内标量 inspector 查询类型。

本机 20 个新 fixture 覆盖独立观察、自动派生、3 Cone 再发布及精确负例；三个正例包含普通与 moving-GC 运行。枚举构造器式 payload 与命名式 payload 统一使用 Named 元数据，位置式仍使用 Positional。组合包含私有字段 companion、定义处泛型依赖、JSON codec 与 debug 同时派生、延迟读取计数、深度／项数／redact 跳过读取、显式基类投影、自环／互环／共享对象、GC 中的读取和 writer、异常传播与已有输出保留。跨库负例同时证明 inline、调用者移入目标类型和 false 分支不能改变非法读取的定义处访问规则。

fmt、workspace Clippy、release 构建及 47 项 codec／注解／类型描述 Rust 回归通过。20 个新 fixture 最终全部通过；新增跨库负例记录精确诊断后，只补验其所在组合。4 个既有 TypeInfo／annotationKey 独立与跨库 fixture 也通过，连同补验组合共 5 项、25 processes。没有为期望文件更新重跑无关套件。原始日志与报告保存在仓库外；清理已结束的 fixture case 目录约 451 MiB，保留共享缓存和 Rust 增量结果。Linux 留待本机整体功能稳定后统一验收。

## 独立 scoop.cli

语言规范 12.7 与实现规范 2.17 定义独立 CLI Cone、显式依赖、schema、参数语法、稳定 Decoder、help 与错误。库由约 800 行普通 Scoop 分成 10 个模块实现，只依赖 core；没有 JSON 中间表示或 compiler／runtime 专用 CLI 入口。Float／Double 以普通 extern 复用已有目标精度转换 ABI。schema 构造一次检查名称、wire key、role、choices 和位置布局，保存只读结果；parser 接收现成 schema，归一化节点保留 token 来源，Decoder 不记录全局消费状态。

自动模型使用实际 annotationKey、nominal identity、primary constructor 映射、hasDefault 和原声明 Option 事实；Serialize.rename／ignore／required／emitNull 与 CLI 命名、help 和输入缺失保持各自职责。ClassInfo 新增 isFinal，支持正常拒绝 open／abstract 目标。跨库组合发现普通 getter 物化为函数体后曾暴露内部 StorageBody；现统一映射为公开 Storage 并删除该实现细节的公开枚举项，本地和跨 Cone 的 getter／setter 保持同一源码事实。

6 个新 fixture 最终全部通过，6 variants、21 processes；5 个正例均包含普通与 moving-GC。覆盖长短选项、分组／attached／负数／终止符、完整 argv、help 提前返回、重复／缺值／未知参数、配置错误、整数与浮点边界、直接 Float 舍入、Char／Unicode、Option 与嵌套序列规则、unit enum choice、默认值和 constructor 次数、显式 wrapper codec、错误 path 和 token、字段／标量复读、容器失败后换视图、独立／共享序列游标及读后异常恢复。真实 provider／bridge／consumer 消费独立 CLI 产物，组合泛型再次发布、注解 alias／re-export／type-use、JSON 和 debug；必填性与实际解码同时核对。注解负例精确检查 target、重复和参数类型。

初轮只修正用例中的 Array 相等比较与遗漏的 enum inspector；跨库普通 getter 的公开描述修复后，集中复验 CLI 全组和相关 TypeInfo／debug 用例。3 个相关 fixture 通过，3 variants、16 processes；fmt、workspace Clippy 与 release 构建通过。原始日志／逐次报告在仓库外，本文只保存范围与结论；清理本批已结束的 fixture case 目录约 497 MiB，继续复用缓存。

## 完整复验发现的导入构造器复用问题

本机完整 Rust 首轮发现 27 项失败：23 份既有 HIR 快照缺少新增的 TypeRef 与内部工厂，另 4 项共享构造初始化暴露了重复展开问题。元数据依赖补齐后再次推进正文队列时，函数已跳过完成项，构造器却重新分配局部绑定，导致既有闭包的捕获身份悬空。现在构造器同样复用已完成的正文；实现规范明确这一既有绑定保持要求，没有增加重放或验证机制。

修复后 fmt、workspace Clippy、release 构建及全部 1,465 项 HIR lowering 测试通过，包含四项共享构造器／跨库闭包回归。首轮 CLI 在 155 通过、18 不适用、零失败时中断，未完成的 3,986 项不计通过；修复后使用 10 并发重新执行完整 CLI。已清理中断后结束的 case 工作目录约 9.3 GiB，保留共享缓存。原始日志和报告均保存在仓库外。

## 剩余验收

本机完整 Rust 范围 5,302 项已全部通过，零忽略；50 项 fixture runner 规则也通过。完整执行后只补验失败项：HIR 期望和构造器修复见上节，后续同步 HIR identity-foundation 10 的固定向量，并清理 core 产物测试对整份内部闭包／泛型实例名单及 singleton／初始化单元总数的绑定。每个实际描述符的 exact layout、特化组、链接属性和导出边界仍逐项检查；初始化单元按真实声明身份核对函数对，缺失类型负例仍精确核对被删除的类型，允许引用检查或依赖检查先发现它。bootstrap 额外检查 Inspectable、TypeInfo 和 TypeRef 的实际泛型应用。

core 组合的 16 项在 debug 补验通过，剩余基础产物、初始化单元和 singleton 三项使用优化测试构建通过；较重的属性初始化组合在两种构建中均通过。格式化、workspace Clippy 及后续受影响 crate 的 lint 均通过。正式 CLI 的旧 builtin-binding 用例改为直接复用当前 Int 源码，删除 154 行重复声明，新增 inspector 所需导入；该项单独补验通过，1 variant、2 processes，并清理结束的 case 目录约 71 MiB。

CLI 完整首轮另发现 6 项 core 根类型负例的旧诊断期望：新增 inspector 改变 ArrayList 的源码范围，并在故意缺失或破坏 Any 时产生额外的连带错误。首条被测根类型诊断不变；同步全部诊断后只补验这 6 项，6 variants、6 processes 全部通过。没有修改编译器、runtime 或 sysroot，也没有放宽诊断匹配。

`program-link-rebuilt-core` 同样依赖旧 String 正文的完整文本替换；现直接复用当前 sysroot 源码，只调整实际被测的类型位置及新增方法，删除两份重复正文。原有类型身份、重建 core、显式产物链接及普通／moving-GC 断言保持；格式与 lint 通过后只补验此项，1 variant、9 processes 全部通过。

本机正式 CLI 已完整闭合：4,141 项通过、18 项不适用，4,665 variants、16,463 processes、88 项 stage／plan golden。完整执行用时 3,253.56 秒；其中 8 项旧测试配置／期望问题按上文只补验受影响项，汇总逐项替换失败结果，没有用未执行项计数。M38 新增 52 项全部通过。原始日志、完整及补验 JSON 报告保存在仓库外；清理本轮已结束的 case 目录约 176 GiB，保留共享编译缓存与近期 Cargo 结果。

本机完整验收完成后，再统一进行 Linux Rust 与 GNU／musl CLI 验收。三目标完成前不宣称 M38 完成。

## Linux 验收与 manifest 大小写修正

Linux fmt、整仓 Clippy、release 工具构建及 50 项 runner 规则通过；优化构建的 Rust 全量 5,338 项一次通过，零失败、零忽略，用时 2,736.26 秒。GNU 与 musl 各 8 并发执行完整 CLI，工具使用固定副本，原始记录位于仓库外的磁盘目录，避开远端容量有限的 `/tmp` 内存盘。

CLI 首轮在两个目标各发现 10 个 M38 用例无法读取 manifest：新增文件使用小写 `cone.toml`，目录入口要求 `Cone.toml`；本机文件系统未暴露大小写差异。统一 20 个文件名及 5 处显式 fixture 输入，内容保持不变。格式与整仓 lint 通过后，本机只补验受影响的 10 项，10 variants、45 processes 全部通过；编译器、runtime 和 Scoop 库源码未改变。Linux 将只补验同一组，完整回归继续执行。

Linux 补验中，CLI 基本组合的冷编译触及默认 300 秒上限；本机完整轮次的同一用例耗时 169.93 秒。仅将该用例 build 步骤的上限设为 600 秒，保留全部编译、普通及 moving-GC 运行断言；格式与 lint 通过，本机复用缓存单独补验 1 项、3 processes 通过。Linux 只重跑这一项，其余补验结果继续保留。


## 冷编译的 GC 指针查询开销

600 秒的补验仍未完成，连续调用栈采样定位到 RS4GC 后的指针活跃性检查：原实现按每个 safepoint、每个派生指针重复扫描整函数的所有指令及 operand。现按实现规范 2.5 直接遍历 LLVM 的实际使用链，包含 operand bundle；函数内的指令位置与 CFG 后继收集一次并复用，block 内使用按实际定义区间判断。PHI incoming edge、循环重新定义、旧根与派生地址跨 safepoint 的拒绝规则保持不变，没有删除 GC 合同检查。

移除临时的 600 秒放宽，CLI 基本组合恢复默认 300 秒。fmt、整仓 Clippy、release 工具构建和本机 codegen 全组 377 项通过；新增三项专测覆盖 operand bundle 重复使用、statepoint 调用参数和 PHI 两条分支的正反例。随后集中补验 M38 全组及 gc 标签的 315 个正式 CLI 用例：308 项通过、7 项目标不适用，M38 的 52 项全部通过；CLI 基本组合在全新工作目录中冷编译及普通／moving-GC 运行共 35.01 秒。本机闭合后再安排 Linux 同批验证。原始记录仍全部位于仓库外。

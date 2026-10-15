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

## 剩余交付

Inspectable 合成与普通访问检查、DebugPrinter、独立 scoop.cli、稳定 CliDecoder、更完整的跨 Cone 组合及最终三目标验收均未完成。下一步完成值观察、debug 与 CLI；不缩减原设计范围。

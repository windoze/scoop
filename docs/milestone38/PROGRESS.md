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

## 剩余交付

TypeInfo／annotationKey 自动查询和全部描述生成、Inspectable 合成与普通访问检查、DebugPrinter、独立 scoop.cli、稳定 CliDecoder、功能跨 Cone 组合及最终三目标验收均未完成。先继续类型描述和其源码／产物消费闭环，再完成值观察、debug 与 CLI；不缩减原设计范围。

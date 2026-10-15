# M38 实施记录

基线：`f75704bac`。M46 与 M48 已完成；本期只在 M48 最终提交之后开始实施。当前完整范围见 [设计](DESIGN.md)，未完成部分不能以基础库通过代替。

## M38-0：文本 writer

先交付 debug 与 CLI 共用的普通文本接口。语言规范 11.16 定义同步 String 写入、普通异常传播、显式换行及 StringWriter 的快照行为；core 使用 13 行普通 Scoop 实现，复用既有 StringBuilder。

独立 fixture 覆盖空值、Unicode／零字符、接口调用、重复 build、后续追加不改变已有 String 和独立实例。组合 fixture 经真实 provider `.slib` 消费泛型 writer 函数和捕获接口的 closure，验证 GC、异常原样传播、已写内容保留及后续正常写入；两者都声明普通和 moving-GC 运行。negative fixture 精确检查非 String 实参的位置与类型诊断。

本机 fmt 与整仓 Clippy 通过，无警告；当前 release 工具执行 3 个新 fixture 和 3 个 StringBuilder 相关回归，最终 6 项全部通过，6 variants、18 processes，未运行无关全量测试。组合首轮误断言 catch 的 managed 对象与 throw 实参引用相同；按运行时规范 5.3～5.4 的异常物化合同改为检查异常类型与消息，只补验该组合，普通／moving-GC 均通过。编译器和 runtime 未因该测试期望改变。

Linux 留待本机完整功能批次稳定后统一验收；本批不宣称 M38 完成。原始日志与逐次报告保存在仓库外。已结束的 fixture case 目录在保存结果后清理，共享编译缓存和 Rust／release 增量结果继续保留。

## 剩余交付

TypeInfo／TypeRef／TypeShape、全部成员与注解描述、Inspectable 合成与普通访问检查、DebugPrinter、独立 scoop.cli、稳定 CliDecoder、跨 Cone 组合及最终三目标验收均未完成。先继续类型描述和其源码／产物消费闭环，再完成值观察、debug 与 CLI；不缩减原设计范围。

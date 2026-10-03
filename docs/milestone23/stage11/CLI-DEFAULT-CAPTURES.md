# 默认闭包捕获的直接引用集合

正式产物消费发现，默认值中的局部函数引用同时记录捕获描述符的首次使用
位置和创建点局部读取位置。导出正文只有前者，后者使 reader 正确报告
`ReferenceClosure(Extra { kind: Type, index: 3 })`。原受保护默认值组合
因此只能生产 provider，不能由下游完成编译与运行。

引用收集现在按实际导出正文遍历捕获描述符，不再把创建点局部读取当作
额外正文表达式。真实 receiver 和正文表达式继续正常遍历，访问域、引用
闭包检查、持久格式与 runtime ABI 保持。

`m23-cli-default-captures/closure-references` 独立验证局部函数引用、lambda、
匿名函数与显式实参覆盖；运行时条件分别走两条捕获路径。
`m23-cli-protected-domains/protected-default-combinations` 保留原源码，组合
受保护成员、泛型嵌套类型、tuple、enum 和默认值调用。两者都保留 provider
和消费程序的完整四阶段输出，移走源码后独立链接，并在普通与移动 GC 下运行。

这两项用例只读通过 13 次进程执行、22 份阶段与计划 golden、7 个产物指纹。
直接引用集合、默认值生产和展开的 22 项 Rust 测试通过；原闭包组合回归也
只读通过。受保护域的其余 11 项用例一并复验，通过后将迁移记录单独提交。
这轮 118 份 golden 中，105 份完全相同，10 份属于原先失败后未能产生的
程序阶段与链接计划，3 份 HIR 只删除上述重复类型引用。

继续复验 `default-preparation/capture-combinations` 与
`source-defaults/nested-identities`，三份 HIR 各删除一个同类的 String 捕获
重复引用，AST/MIR/LIR 与链接计划不变。同步三个受影响的产物指纹后，两项
用例只读通过 11 进程、14 份 golden、5 个指纹。

常量条件在 LLVM statepoint 改写时暴露的另一处 CFG 问题由
[常量分支修复](CLI-CONSTANT-BRANCHES.md) 单独处理。
本记录不代表 M23-11 全仓验收完成。

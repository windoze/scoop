# 包限定表达式范围决定

状态：取消。日期：2026-10-28。

本次不实现普通包限定函数／值表达式（如 `scoop.compiler.env(...)`），也不实现前置点分声明引用 `::a.b.f`。点表达式的包查找需要协调 receiver、首段遮蔽、最长包前缀、重载、类型别名和属性读写，用户据此决定取消这两项扩展。

跨包函数和值使用既有 exact／star import 或 import alias；函数引用继续使用 `::name`、`receiver::member`。类型、注解与模式中的既有限定路径保留。相关未提交的 AST／parser／HIR 改动与试验测试均已撤回。

[编译期 intrinsic 设计](../compile-time-intrinsics/DESIGN.md)继续实施，四个入口保留在 core Cone 的 `scoop.compiler` package，导入后调用短名或 alias。不可引用 intrinsic 的诊断是独立的语义规则，见语言规范 13.1；`currentSourceLocation` 的诊断及错误下声明导航已实现。完整范围与验收见[实施记录](../compile-time-intrinsics/PROGRESS.md)。

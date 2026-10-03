# 导入默认值的继承接收者

正式产物消费暴露了原默认调用签名 fixture 未执行到的错误：`Child.read(2)`
在定义处选择 `Parent.read`，导入默认正文却提前改成普通 Call，跳过接收者
适配。共有 HIR 正确拒绝了声明参数 Parent 与实际参数 Child 不一致的调用。

默认值物化现在保留普通／super 成员调用和原声明，复用已有成员具体化路径。
LocalConcrete 生成引用上行转换，同时用 SourceCallReceiver 保存原 Child
静态类型。泛型、bound、装箱与共有签名校验继续使用既有实现，产物字段、
实体身份和 runtime ABI 不变。

`m23-cli-default-receivers/inherited` 独立用例先在修复前复现同一错误；修复后
检查普通调用、导入默认值和显式实参覆盖。程序移走源码后独立链接，并在普通
与移动 GC 下运行。结构单元测试直接检查 Child 静态 receiver、Parent 实参、
ReferenceUpcast 和默认值的原定义／新求值来源；原接收者组合单元测试也通过。

`m23-cli-default-operations/operation-signatures-standalone` 保留原源码作为
组合回归，实际调用继承／泛型／挂起方法、getter/setter、vararg、class/struct/
enum 构造及泛型默认构造。两项 CLI 用例只读通过 **13 进程／22 份阶段与计划
golden／7 个产物指纹**，两项 Rust 测试通过。

同时复验其余五个默认操作组合，已有 76 份 golden 完全相同；唯一已有文本
变化是静态嵌套成员在 Export HIR 中从 Call 恢复为 MethodCall，MIR/LIR 不变。
新增的十份 golden 来自此前失败用例缺少的程序阶段和链接计划。

继续复验 `m23-cli-default-access/value-access-combined`，四处单例默认成员
调用在 Export HIR 中同样恢复为 MethodCall；其余阶段与产物指纹不变。
同步该文本预期后，用例只读通过 6 进程、9 份 golden、3 个指纹。
本记录只覆盖本项修复，不代表 M23-11 全仓验收完成。

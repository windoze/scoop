# M33 范围外的既有问题

## 泛型结构比较的具体化正文选择

恢复基线为 `23cbfb7de`，即 `00d4e5858` 开始将 operator 与 core Equality 绑定之前的提交。

基线的 `compiler/hir-lower/src/concretize/equality.rs` 按具体宿主类型缓存结构比较函数；`derived_functions` 的键只有 `concrete::TypeId`。若同一个具体宿主同时来自直接比较和带不同 operator bound 的泛型正文，两处可能复用首先生成的比较正文。例如直接比较 `Box<Key>` 与在 `<T : Wide>` 中比较 `Box<T>`，具体化为相同宿主后，不能保证分别保留窄参数与宽参数的字段重载选择。

`git diff 23cbfb7de d2ac0cfaa -- compiler/hir-lower/src/concretize/equality.rs` 为空，相关生成身份、导入具体化和默认正文投影也未改变。此前在 `d2ac0cfaa` 上的定向探查观察到两个比较共用一个 `Box.equals` 正文；没有对旧基线另做完整构建或宣称其运行输出已经验证。

这是原有结构比较实现的问题，本次恢复 operator、添加普通 `Equality<T>.equalTo` 不改造该算法，也不把它列为 Equality 或 M33 的新增验收门槛。尚未提交的上下文相关生成身份、模板、ODR 和配套升级测试已经撤回；旧的 `DerivedEquality` 及 HIR `DerivedEqualityApplication` 数据仍服务原有 operator。

# M23-11 可执行类型使用与依赖 callable 的 CLI 迁移

`m23-cli-executable-sites` 覆盖原 `m23-executable-type-sites` 十五份和 `m23-executable-dependency-callables` 七份源码。19 项通过正式 CLI 生产 library／executable；library 移走源码后由实际下游编译，再从独立 CLI 位置、仅凭产物与 runtime index 链接和运行。普通与 moving-GC 配置均通过，最终只读验证共 129 次进程执行、231 份阶段及 link plan golden。

原 driver 的六个类型用例、五个 callable 用例和四个 core／provider／facade，共 15 份 `.slib` 与 CLI 产物逐字节一致。声明固定检查 54 个原源码节点的 artifact fingerprint。现有 typed reader 继续检查源位置、定义／求值 Cone、声明类型用途、默认值、实际机器引用和缺失／多余 shape 的拒绝；没有新增产物检查 API。

十五份旧摘要中十三份逐字一致，另外两份的差异已逐项核对：

- `storage-combined` 的公开 `StoragePair.equals` 按既有 `edde2400a` 修复实际生成。它增加七处表达式类型使用、一个 Boolean 结果签名，以及两个字段相等调用；签名计数 34→35、表达式计数 5→12、MIR／LIR selected 1→3，其余摘要相同。旧 producer 与 CLI 产生的完整归档仍逐字一致。
- `casts` 的 HIR 摘要相同。旧内部 MIR 的本地函数编号整体移动三项；CLI 的六个当前函数由旧 `@fn34..39` 对应到 `@fn0..5`，`ClassCastException` 构造从旧本地 `@fn45` 变为真实依赖 `external0`，正文其余文本一致。实际运行除成功 cast 外还覆盖依赖默认值中的错误 cast，并捕获原 `ClassCastException`。

| 功能 | 正式 CLI 用例 | 进程／golden |
| --- | --- | --- |
| 表达式类型位置、定义来源与未展开默认值 | `types-standalone`、`types-combined`、`types-type-test` | 20／35 |
| 完整签名、局部值与声明类型位置 | `types-declaration-standalone`、`types-declaration-combined` | 14／26 |

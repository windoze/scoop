# 函数值静态存储的完整布局

迁移原 `m23-property-initialization/inactive.scoop` 时，实际 CLI 在发布
`.slib` 时报告 `CanonicalShapes(MissingContent(...))`。缺失成员是顶层函数值
属性的 ODR value layout：全局存储已经引用该布局，LIR 元数据原来只为装箱和
数组元素补充结构类型的 inline payload，遗漏仅出现在静态存储中的函数值。

LIR lowering 现将实际 `Managed` 全局的值类型纳入同一个 payload 遍历，继续
按原 exact type 去重、计算布局和扫描。未改变布局编码、ODR 身份、外来 Strong
归属或 runtime ABI，也没有在 canonical shape reader 中放宽完整性检查。

正式 suite `tests/fixtures/m23-cli-function-storage` 的两项回归覆盖：

- `standalone`：普通库只导出一个顶层函数值，源码移除后，程序从 `.slib`
  读取该值并调用，结果为 42。
- `combined`：另一 Cone 同时保存闭包、匿名函数与函数数组，与库共享同一个
  `() -> Int` exact type。完整产物独立链接成功，函数值与数组调用均返回预期结果。

两个用例均保留 provider、program 的 AST/HIR/MIR/LIR 和链接计划 golden；
移除源码并隔离编译器后执行独立链接，再分别进行普通与移动 GC 运行。
关闭快照更新的正式运行通过 2 项、12 个进程和 18 份阶段／计划 golden。
已先执行 `cargo fmt --all`、Python 格式与 lint、全 workspace/all-targets Clippy。

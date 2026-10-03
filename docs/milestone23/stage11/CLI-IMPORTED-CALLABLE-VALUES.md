# 外来属性与单例的命名调用

从真实 `.slib` 读取 `public val callback: (Int) -> Int` 后，`callback(40)`
原来错误地报告未知函数。命名调用的 import 层已有该绑定，但值候选收集只接受
当前 Cone 的属性和 object，遗漏了依赖中的对应值。

值候选现在保留实际依赖绑定，并复用普通变量读取的 `ResolvedValueTarget`
路径，再进入已有函数值／成员 `operator invoke`／扩展 invoke 分区。
getter、初始化服务、访问域和求值顺序继续由原路径处理。值分区的公共辅助方法
移入 `expr/named_calls/values.rs`，主文件未继续增长。

正式 suite `tests/fixtures/m23-cli-imported-callable-values` 覆盖：

- `standalone`：移除库源码后直接调用导入的函数属性。
- `combined`：exact alias 与 star import 混合，getter 返回函数值、实参函数的
  副作用顺序为 12，另调用外来 class 实例属性和 object 单例的 `operator invoke`。
- 四个 negative：错误类型、错误数量、命名参数、spread 参数。逐项核对
  `SCOOPC_HIR_ERROR`、完整消息、canonical 源码路径和精确 span，要求无输出产物。

两个正例保留 provider/program 四阶段及链接计划 golden，删除源码后独立链接，
普通和移动 GC 运行均成功。四个 negative 也保留 provider 的四阶段 golden。
关闭快照更新的正式运行通过 6 项、24 个进程、34 份阶段／计划 golden；
原命名调用决议的 22 个 Rust 单元测试全部通过，未使用快照更新。
格式化、Python lint 和全 workspace/all-targets Clippy 均先于验证执行。

原 `m23-property-initialization/inactive.scoop` 不变，其普通库发布及下一层
直接调用三个函数属性的完整 CLI 流程也已通过。

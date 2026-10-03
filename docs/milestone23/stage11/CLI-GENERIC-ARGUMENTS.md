# M23-11 泛型参数、重载与定义点诊断

九个用例保留原源码，以原 test:generic-provider:1.0.0 和 test:scoop-hir-lower:0.0.0 坐标经过正式 CLI 发布／消费。成功用例移走源码后独立链接，运行普通和 moving GC 变体；错误用例检查完整 canonical JSON 与失败后不发布产物。诊断在保存前逐项核对原消息及源码字节 span。

只退役文件 golden 比较和由新用例覆盖的 negative 文件入口；局部 typed HIR 断言继续留在原 crate。原始 `.scoop` 未被删除或内联到 Rust。

| 功能 | 用例 | 只读进程／golden | 保留的断言 |
| --- | --- | --- | --- |
| materialization | `consumer` | 12／9 | 原 HIR／MIR 两份快照逐字相同；显式／推断类型、局部值、relay、extension、默认值、NoGC 与递归实际运行，原 HIR template 数量与 typed 正文断言继续保留 |
| overload | `overload` | 12／9 | tuple 选择双参数泛型声明，运行结果为 2；原单一胜出 template 与 tuple 参数断言继续保留 |
| `imported_generic_kind_bound_reports_the_consumer_argument` | `bad-kind` | 3／0 | value 约束错误定位完整 valueOnly 调用，保留额外的公开签名可见性错误 |
| `imported_generic_ambiguity_reports_both_declared_signatures` | `bad-overload` | 3／0 | conflict(1, 2) 的精确 span；错误仍包含两个完整候选签名 |
| `imported_generic_effects_and_pointee_predicates_reach_consumer_calls` | `bad-nogc`、`bad-nogc-argument`、`bad-pointee` | 9／0 | managed 调用、GC-free 类型参数及 Ptr pointee 条件均保留；完整诊断集合还锁定签名与两处 Ptr 类型使用错误 |
| `ordinary_core_call_uses_shared_generic_argument_diagnostics` | `bad-core-type-arity` | 2／0 | print 的精确 span 和 expects 1 type argument(s), found 2 |

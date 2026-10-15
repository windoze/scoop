# M45 实施记录

M38 已完全验收后才开始本里程碑。本机批次逐项完成、提交并复用缓存；Linux GNU／musl 验收安排在所有本机修改稳定后。原始日志与机器报告保留在仓库外，本文只记录结果概要。

## M45-1：泛型扩展回调推断

2026-10-16 完成本机实现与回归。已知 receiver／普通实参先确定 lambda 参数及可选 receiver，再由正文确定返回类型；外层结果与其他延期实参仍可提供缺少的约束。失败探测保留候选隔离，最终 HIR 不包含部分函数类型，不引入库名特判或新的 IR／产物格式。

| 验证 | Darwin/AArch64 结果 |
| --- | --- |
| Rust 格式化与 workspace all-targets clippy | 通过 |
| scoop-hir-lower 全 crate | 1470 通过，含 5 项新增推断测试 |
| 相关正式 CLI 回归 | 148 通过；177 variants、497 processes、1 项既有结构 golden |
| 新增 M45 CLI | 8 通过；普通与挂起回调、构造／变量 receiver、隐式 it、嵌套调用、跨 Cone `.slib`、真实挂起恢复、debug/release 与 moving GC |
| 必须拒绝的调用 | 重载歧义、错误参数数目、缺少约束、普通 lambda 调用挂起函数、普通上下文调用挂起重载；精确诊断位置与原因通过 |

CLI 回归还覆盖既有 contextual inference、泛型约束、receiver function type、声明结果推导与重载专测。没有重复全 workspace／全 fixture 验收；Linux 未在本批执行，纳入最终组合验收。

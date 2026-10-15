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

## M45-2：platform Cone、系统字符串、路径与错误

2026-10-16 完成本机实现。普通 Cone 依赖 core／std，测试目标额外依赖 scoop.test；OsString／Path 的复制、UTF-8、字节比较、词法路径操作与错误对象均由 Scoop 实现。一个 C11 shim 负责依据本机 errno／EAI 常量分类，不读取延后的 errno，不携带 runtime 私有实现。

- `scoop test sysroot/lib/scoop.platform`：12 项通过，覆盖无损非 UTF-8 数据、严格／显式有损转换、NUL、路径组件／父路径／归一化、原始错误快照。
- native 错误码专测通过：实际 C 常量分类、EAI_SYSTEM 的 errno 快照与不改写线程 errno；作为普通 Scoop 测试无法直接读取 C macro 的窄 ABI 专测保留。
- 下游 `.slib` 消费 fixture 通过普通运行与 moving GC，源文件和工具始终存在；production build 使用普通 native requirements。
- C 格式化、`-Wall -Wextra -Werror`、Rust 格式化和 workspace all-targets clippy 通过；本批没有编译器／runtime 改动，未重跑 Rust 测试或全 fixture。debug/release 组合与 Linux 留到最终验收。

## M45-3 前置修复：异常对象身份

资源双失败测试暴露 throw/catch 复制 Throwable 的旧合同。已先修订语言、runtime 与 std 规范：throw、catch、重抛和跨挂起保存都保留原对象及字段别名。native record 通过现有 GcHandle 保活；删除两次对象复制、异常物化入口、无生产使用者的堆外对象 root 注册与扫描分支。runtime contract 升至 13，metadata 布局不变；旧产物按既有兼容性规则重建。

本机格式化、workspace all-targets clippy 与 C 严格告警检查通过；受影响的 MIR／LIR、lowering、codegen/runtime 与 slib 共 1732 项 Rust 测试通过。独立源码回归覆盖字段别名、catch 返回、重抛、实际挂起恢复、debug/release 和 moving GC。旧 ABI 固定向量及必要 LIR 期望已核对更新；11 份 MIR 代表快照只保存各自被测函数，从 5775 行缩至 605 行，异常与协程的专门 typed 断言仍保留。组合 CLI 与资源库结果在下一节记录，Linux 留到 M45-10。

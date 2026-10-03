# 实际 Link 用途与 Code 重放

独立源码覆盖外来属性初始化、普通 callable、String 和 native 调用；组合源码
继续加入 default、重复 getter/setter 及多种初始化路径。

正式 CLI 声明位于 `../m23-cli-layout-dependencies/core-symbols-*`，保留完整
AST/HIR/MIR/LIR 和产物 fingerprint。移走 core 与当前库源码后，下一层继续
读取实际 `.slib`；产物 fingerprint 同时固定原对象、登记、relocation 分区、
Code 与 native contract/library 内容。

driver 内部测试继续核对定义与用途分区、最终对象及登记、Compile/Link 读取，
并在重建 archive hash 后定点破坏 distribution、output 分支、两个 Code 字段、
native 表和 dependency owner。两个 Code 值一致但都错误的反例也必须被拒绝。
原重复的文件快照和更新环境变量已删除，迁移对照见
`../../../docs/milestone23/stage11/CLI-LAYOUT-DEPENDENCIES.md`。

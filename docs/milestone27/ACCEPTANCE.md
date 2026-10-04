# M27 实施与验收记录

状态：实现进行中；以下为分批证据，不代表整个 M27 已完成。

## M27-1 同步作用域

实现了 AST/HIR/MIR/LIR 的 context requirement 和 scope、实际 core MissingContextException、入口 lookup、结构化 push/restore cleanup、root gateway 的 task 初始化、可移动 GC 的 TaskContext/四叉持久树，以及沿现有 callable registration 和 associated atoms 发布 key cell 的对象与 runtime 路径。普通 callable ABI 保持原有参数。

新增实现文件按职责拆分，当前均不超过 170 行；较大的 metadata 发射模块拆出了既有 digest 处理。构建使用外部 LLVM 22.1，阶段性清理 Cargo target，并关闭本次构建的增量缓存和调试信息以控制临时产物体积。

正式 fixture：

- `tests/fixtures/m27-context/basic`：基本 binding、入口取得、退出恢复、缺失异常与 moving GC；
- `tests/fixtures/m27-context/scopes`：中间普通调用、延迟函数引用、入口 local 稳定、不同 key、anonymous/unused requirement、词法捕获、value 求值一次和异常消息；
- `tests/fixtures/m27-context/exits`：return/break/continue/throw/finally、绑定表达式抛出、GC 中保留结果，以及 caller 能捕获而正文 try 不能捕获入口失败。

三套 fixture 均保存 HIR/MIR/LIR golden，并各运行普通和 `SCOOP_GC_STRESS_MOVE=1` 两条路径。runtime 的 `task_context_test.c` 通过现有 codegen C 测试入口验证较高 slot、分支边界、嵌套恢复、fork snapshot、每次分配时 relocation，以及 native-safe 线程的 current task 扫描。

本批通过 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，以及 parser 450、HIR 874、HIR lowering 1314、identity 337、MIR 345、MIR lowering 114、LIR 477、LIR lowering 146、codegen 312、slib 590 项库测试。driver 的 88 项库测试中，87 项在批量运行通过；剩余 core 产物闭合测试修正其对 compiler-generated Context 类型的选择条件后，单独复验通过。三个正式 fixture 共执行 9 个进程并核对 9 份 stage golden。

## 后续批次

M27-2 完整源码契约、M27-3 泛型与独立产物、M27-4 协程任务传播、M27-5 callback snapshot 和 M27-6 总验收仍在实施计划内。全部完成前不标记路线图 M27 完成。

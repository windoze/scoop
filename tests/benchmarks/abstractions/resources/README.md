# 资源作用域的抽象成本

`usingLease` 通过预编译 core 的 `Disposable.use` 读取字段、捕获本轮输入并清理；`directLease` 按相同顺序直接执行同样操作。清理会修改字段，结果包含清理前的读取和最后的字段值。

复用 `../../interfaces/phases.c` 的计时入口。将其编译为 `libm34_interfaces.a`，用 `scoop build --release --library-path DIR` 构建。`SCOOP_BENCH_KIND=0` 选择 `use`，`1` 选择直接写法；`SCOOP_BENCH_COUNT` 是迭代次数。推荐长样本 20000000，短样本 1。

用同一源码和预编译 sysroot 分别构建两版工具的产物，再用 `tests/benchmarks/compare.py` 交错运行。计时阶段包含资源创建、循环和最后读取，输出在阶段后完成。GC 统计另行开启，不混入普通计时；同时检查实际内核的闭包分配、调用、异常路径与对象分配。

优化、跨 Cone 与 GC 合同由 `compiler-correctness/inlining/exception-paths` fixture 覆盖；普通资源语义由 core consumer Cone 测试覆盖。

# 跨 Cone 普通成员内联

provider 提供用户定义的 struct／class 方法与 getter；consumer 分别循环调用它们，并提供 `Long.hash`、未知接口调用及直接标量计算作为对照。正文选择依据结构与完整调用签名，不按这些声明的名称选择。`SCOOP_BENCH_KIND` 的 0～5 分别选择值方法、引用方法、getter、primitive hash、未知接口方法、标量循环，`SCOOP_BENCH_COUNT` 指定迭代次数。

使用正式 `scoop build --release` 分别构建 provider 和 consumer，先将 consumer 复制到工作目录的 `consumer/`，provider 发布到该工作目录的 `artifacts/dev.performance/member-bench-provider/0.1.0/cone.slib`，再构建复制后的 consumer，其清单通过显式 artifact 消费。两者使用同一组配套编译器、sysroot 与目标选项。复用 [接口基准的 phases.c](../../interfaces/phases.c)，将其编译为 `libm34_interfaces.a` 并通过 `--library-path` 链接，不重建计时设施。

计时区间只包含所选循环，stderr 输出 kernel wall／CPU 纳秒，stdout 输出 checksum。0、1、2、4、5 的 checksum 应相同；3 从 7 开始重复固定 64 位 hash。先验证 0、1、2、3、4097 次的输出，再用 [compare.py](../../compare.py) 对照已经构建的两个程序，按 AB／BA 交替运行。记录原始内核时间、整个进程时间、产物体积和编译成本，GC 统计单独采集。未知接口对照不应因已导出正文而绕过动态分派；检查实际 MIR 与机器码后再解释性能变化。

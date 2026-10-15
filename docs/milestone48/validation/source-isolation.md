# 历史源码隔离包装迁移验收

迁移原清单 1,646 个用例：源码、工具与原始产物保持存在，通过真实构建、运行、诊断与调用记录覆盖原语言／ABI／GC 行为。删除 304 个普通语言用例中 306 次重复显式链接；真实 ODR、native、ABI、runtime、独立链接和失败不发布专测保留。逐项映射见 [CLEANUP](../CLEANUP.md) 与 [SOURCE-ISOLATION](../SOURCE-ISOLATION.tsv)。

本次提交 1,644 份 fixture 描述、4 份诊断期望、4 个 Rust helper／测试文件及 runner 说明／docstring；其余两份 `core-default` 和 `program-link/input-errors` 已随此前实现提交。测试仍验证实际缺失输入、输入变化、失败不发布、输出缺失后的生成与正常并发。

## 证据范围

Darwin 代表组与定向复验最终覆盖 75 个不同用例，90 variants、557 processes、21 goldens，包括 17 个 Darwin 专属用例；Rust core 发现、直接 core 输入、类型位置、泛型 ODR／优化及机器对象均已通过，记录在 [PROGRESS 第 17 节](../PROGRESS.md)。

Linux 受影响全组使用独立冻结 release 工具运行，避免与后续实现构建相互覆盖。初次临时磁盘不足与一次复用数字源路径的问题、复验落点和全部分批报告在同节说明；失败和未完成项不作为通过结果。完成后 GNU／musl的成功或声明不适用结果分别覆盖原清单全部 1,646 个名称，无遗漏。最后 musl 第 8 批、报告为 200 项通过、201 variants、1,150 processes；第 9 批、报告为 36 项通过、36 variants、123 processes、1 golden。后台任务已结束，已完成批次的临时 case 目录由脚本清理。

提交前核对 1,654 个变更文件与 Linux 被测文件完全一致；两端已有 fmt／workspace clippy 记录。Ruff 格式、lint与 49 项 runner 测试再次通过。重新扫描所有 fixture 描述，剩余 25 份 remove／move 用例与已审阅的正常输入变化、输出缺失、源码正常搬迁或测试清理清单一致。

这是 fixture 迁移的验收，不是 M48 当前最终实现的全量证据。Compile／Link 语义与布局重放、dump、最终当前代码全量回归和五轮性能验收仍待完成；M38 未启动。

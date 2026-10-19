# M58 实施与验收记录

状态：实施中。分支 `codex/m58`，主仓库 `/Users/chenxu/repos/scoop`；保留工作区中其他任务的 M47／M57 未提交内容，不纳入 M58 提交。按 [DESIGN.md](DESIGN.md) 完成全部 A–D 后再做整体验收；当前结果不代表里程碑完成。

## 分项状态

| 功能 | 当前状态与证据 |
| --- | --- |
| A：年轻 small block 整块退役 | 已实现；本机 GC 专测 32 项、新源码 fixture 四个变体通过，Linux 待验证 |
| A：无引用大对象跳过 remembered scan | 已实现；本机 nursery 专测 10 项、新源码 fixture 三个变体通过，Linux 待验证 |
| A：full 必要容量早判 | 已实现；本机 region 专测 7 项、新源码 fixture 三个变体通过，Linux 待验证 |
| A：starts bitmap 枚举 | 已实现；本机 GC 专测 35 项、M58 runtime 四个源码 fixture／13 变体通过，Linux 待验证 |
| B：统一优化迭代 | 已实现三轮上限与无变化早停；本机 MIR 单测 122 项、新旧优化源码回归通过，Linux 待验证 |
| B：函数内来源／逃逸分析、box／closure 消除与支持产物清理 | 待实施，接入同一迭代 |
| C：tail completion 转交、状态机生成前同步内联 | 待实施 |
| D：外层 poll 循环分块、受限内层 SIMD | 待实施 |
| 性能与跨平台 | A 的本机生产前后矩阵已记录于 [PERFORMANCE.md](PERFORMANCE.md)；B–D 性能、GNU／musl 完整验收及最终清理待完成 |

## 2026-10-20：年轻 block 退役

先补 runtime 规范 3.9，再在收尾阶段为普通 minor 的全死／已搬空年轻 small block 使用现有 block 退役流程。先统计晋升量；源存储已完成回写与验证才释放 forwarding，复用依赖现有 activation 清空元数据。含 pin 的存活 block、evacuation target、release-hook 旧对象和 moving stress 保持原路径。

新增 C 专测反复检查全死块与混合存活源块的退役／复用、旧起点不可见、晋升计数和链内容；新增正式源码 fixture 以持续分配和 old→young 链覆盖 debug／release 及普通／stress minor。运行参数显式选择当前 runtime 源码，避免误测旧 sysroot runtime。

本机验证：C 格式化与 `-Wall -Wextra -Werror` 语法检查、`cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；`scoop-codegen` 的 GC 专测 32 项通过，包含 pin／handle、release、移动 stress、并行回写、晋升失败和线程握手；新 fixture 1 项／4 变体／12 进程通过。最初 fixture 使用 Option 结构比较触发 Node 缺少 equals 的诊断，已改为模式匹配并闭合。

构建与原始验证输出位于仓库外 `/Users/chenxu/repos/scoop-work/m58-validation`；临时目录显式使用仓库的 `tmp/m58/tmp`，不使用 `/tmp`。开始时定向 `cargo clean -p scoop-codegen` 释放约 1.4 GiB，保留可复用依赖。后续继续按实际磁盘占用清理，Linux 尚未开始。

## 2026-10-20：跳过无引用大对象的卡表

在 shared remembered 枚举入口读取已发布旧 large head 的 TD；没有 object scan 时不遍历其卡表。标记与回写共用此路径，卡表分配／屏障／清零和对象本身的保活规则保持原样。

新增 C 专测将 1 MiB 无引用对象全部标脏，与真正含引用的 large array 同轮收集；四轮均只计入两个必要脏卡、128 个旧引用槽，两个年轻对象正常搬迁、内容和晋升统计保持。正式源码 fixture 组合大值数组的 scoped pointer borrow、native 范围屏障和旧引用数组，使用当前 runtime 源码。

格式化、C 严格语法检查、workspace clippy 通过；nursery 专测 10 项、新 fixture 1 项／3 变体／9 进程通过。fixture 的回调补上显式 unsafe 作用域；无引用存储使用 Array 的 scoped borrow，避免当前显式 MutableArray 借用重载解析到 Array 的无关诊断。没有修改语言或重载行为，目标运行时路径由 C 专测和源码闭环共同覆盖。性能测量留到 GC 改动完成后统一对照。

## 2026-10-20：full 容量早判

在普通 full 的单 source 情况下，使用现有 region 存活统计与普通 block 容量，计算已占用目标 region 的可用容量上界。明显不足时恢复 source 状态，直接做非移动收尾；通过时仍使用实际预留／回滚。没有新增堆列表、对象扫描、公开参数或 ABI。

C 专测在真实 STW、mark 与 plan 边界检查：两个 block 的 source 无法放入仅一个 block 的容量时，目标死亡对象仍未 release、起点仍存在；正常收尾只 release 一次。额外释放两个目标 block 后，同一 source 的两个对象成功搬迁，引用和内容保持。正式源码 fixture 用 55 万个含引用节点、共享尾节点和大引用数组，覆盖密集全存活、稀疏回收及再次 full 的图正确性。

C 格式化／严格语法检查、Rust fmt／workspace clippy 通过；region 专测 7 项通过，包含失败回滚、pin、discard、region 压缩；源码 fixture 1 项／3 变体／3 进程通过，覆盖 debug、release 与四 worker。此处确认正确性和提前退出路径，尚未给出暂停加速比。清理约 16 GiB 的 `target/debug/incremental`，保留依赖与正式 CLI 工具。

## 2026-10-20：按 starts 位图枚举

small block 收尾按 starts 置位枚举对象；目标准备直接枚举 `starts & ~marks`。每次取得 bitmap word 快照后清位，维持地址顺序与原有 release／pin／forwarding／size／line 逻辑。没有增加对象列表或持久索引。

新增 C 专测覆盖同 word 多起点、相邻 word、空 word、block 末尾的 24-byte 对象、含 pin 的目标与另一 source region；压缩前后的存活资源内容保持，全部资源只 release 一次。独立源码 fixture 以 512 个 release 资源保留稀疏子集，覆盖普通 full 与移动 stress 的生命周期。

C 格式化／严格语法检查、Rust fmt／workspace clippy 通过；全部 GC 专测 35 项通过。M58 runtime 的四个源码 fixture 共 13 变体／33 进程通过，包含前面三项改动的组合回归。A 的四项实现已分别闭合本机正确性验证；生产性能对照与 Linux 验证尚未完成。

## 2026-10-20：A 的生产对照

固定编译器，仅替换 `c3b8942db`／`8df65ae17` runtime，使用真实 Scoop 编译运行的 full、minor 与连续分配程序。五对独立进程、事件级 full 100 样本／minor 250 样本，另做预热后的七对连续分配；方法、阶段、RSS、原始离群值处理和限制见 [PERFORMANCE.md](PERFORMANCE.md)。四 worker 下，90 万节点的失败压缩准备 plan 5.495 → 0.074 ms、总暂停 p50 20.788 → 11.729 ms；真实稀疏压缩 27.764 → 20.480 ms，仍有长暂停。四 worker、128 MiB 无引用旧数组的 minor p50 814 → 413 µs。连续分配总时间 105.234 → 67.775 ms，主要由累计暂停减少解释。

全部测试输出与临时 benchmark 副本位于仓库外；已清理约 1.8 GiB 的已完成 fixture 产物。Linux、多个 mutator 的响应尾部、GUI 帧循环及 B–D 性能仍待完成。

## 2026-10-20：统一普通优化迭代

release 使用最多三轮 devirt／inline／simplify，实际无变化时早停。内联的稳定模板、原始成本、caller 剩余增长及展开祖先跨轮保留；当前直接调用图每轮刷新递归判断，隐藏递归另受实际展开祖先约束。只重新分析上轮改变的函数，LocalValueRegistry 在整个阶段内共用，结束时一次写回。没有新增持久 IR／产物字段或通用 pass 框架。

新增 typed MIR 专测区分一轮仍间接、两轮已展开、第三轮观察到固定点；12 轮测试验证累计增长与六层深度不重置、开放 receiver 中隐藏的递归保留，以及新 exact receiver 暴露的直接递归触发重新判断。独立源码 fixture 组合 interface helper、两层 closure 包装、参数副作用、可变字段、实际递归与移动 GC；此时 closure 分配仍保留，环境消除属于下一项。

Rust fmt、workspace clippy、MIR lowering 全部 122 项单测通过；8 项直接覆盖普通优化。新源码 fixture 3 变体／3 进程；M34 内联五项 10 变体／44 进程、M31 优化五项 12 变体／38 进程通过。M34 去虚拟化中，跨库与 slots 两项先通过；locals 的 release MIR 因新增展开和常量分支化简更新已有优化专测期望，debug 期望保持，复跑两变体／8 进程通过。隐藏递归单测原先预期停在 method，实际由展开祖先停在 helper；修正预期并单独增加新直接 SCC 的专测，二者均闭合。

这笔仅完成有实际去虚拟化／内联消费者的迭代功能，不代表 B 的分配消除或 C 的协程前置内联已完成。GC benchmark 的临时源码、基线 runtime、可执行文件与缓存已清理；保留文档概要和仓库外的小型验证日志，完成的优化 fixture 产物继续定期清理。

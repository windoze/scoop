# 搬迁时更新存活清单

本项从 `746fb125a` 开始。新内联版本的 trees 采样仍包含大量 GC 元数据查询；实现中，每个搬迁后需要扫描的对象先由存活清单取得旧地址，再查询 source 元数据、读取 forwarding、重新查询目标元数据。复制阶段已经拥有相同对象的目标地址，可以在此保存，减少后续重复工作。

## 采用方案

沿用稳定分块的存活清单，每项仍为一个对象指针。搬迁计划将原 source 指针字段替换为对应清单条目的地址，48 B 的计划项保持不变。全部目标预留成功后，从条目读取原地址，依次复制、登记目标元数据、发布 forwarding，再将条目改为目标地址。预留失败保持原清单；pin、原地 survivor 和其他未搬迁对象仍指向原存储。

引用回写直接消费清单中的当前地址，claim 只定位一次对象元数据。保留 collection、起点、mark、当前副本和重复扫描检查；对象字段、roots、remembered slots 仍按原 forwarding 规则更新。mark worker 完成后才修改清单，update worker 发布前完成全部复制和清单更新。单／多 worker 共用此规则。

这是 GC 私有执行顺序调整，使用新版 runtime 重新链接后，全部 profile 默认采用；对象、block、TLAB、静态元数据和 `.slib`／调用 ABI 不变，无需重编译 provider。没有新增测量代码或优化开关。

## 定向验证

本机与 Linux 先执行格式检查、相关 C 的 `-Wall -Wextra -Werror` 语法检查和 `scoop-codegen` 的全 target Clippy，再各运行 9 项 native GC 测试。新增用例在真实 collector 的复制完成、引用回写开始之前检查清单，覆盖单／四 worker、普通对象、大对象、无引用对象及 pin；既有用例覆盖 survivor、minor 晋升预留失败、full 预留失败、零搬迁、并行共享图、别名 roots、worker 创建失败和真实 moving collector。

Darwin ARM64、Linux GNU／musl 各通过三组正式 GC fixture，共 7 个构建变体、33 个进程：`nursery/full-live-graph`、`gc-writes`、`parallel/values`。组合包括 debug／release、移动与 minor stress、不同 worker 数及值内引用。没有运行完整验收。

运行对照使用前项 `746fb125a` 产物与仅替换 runtime 的候选，复用原六个程序源码和系统库。普通耗时与现有 GC 统计分开测量，各预热一对、交错运行三对 AB／BA；NUC 固定 CPU 4，延续原比较方式。短 JSON 的 Darwin 初测随预热下降，针对该项补三对预热、31 对正式样本；未重复整组验证。原始日志、汇编和报告保留在两机 `tmp/program-performance/live-set-relocation/`。

## 运行结果

普通进程耗时中位数的变化如下，负值表示耗时降低。Darwin 使用 M3 Ultra／LLVM 22.1.8；Linux 使用 i7-12700H／LLVM 22.1.2。

| 程序 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| trees | −13.98% | −9.09% | −2.97% |
| strings | +0.36% | −0.43% | −0.27% |
| strmap | −1.25% | +0.97% | +2.07% |
| dispatch | −1.88% | −0.05% | −0.79% |
| values | +1.02% | +0.05% | −0.04% |
| json | +0.12% | −2.00% | −0.36% |

trees 分别从 5.433→4.673 s、9.483→8.620 s、9.885→9.592 s。独立 GC 统计测量确认减少的主要是引用回写阶段：

| 目标 | update 总计（s） | update 变化 | GC 总暂停（s） | 普通运行峰值 RSS（MiB） |
| --- | ---: | ---: | ---: | ---: |
| Darwin ARM64 | 1.459→0.738 | −49.42% | 3.188→2.466 | 468.23→469.64 |
| Linux GNU | 2.377→1.479 | −37.76% | 6.524→5.629 | 450.99→450.32 |
| Linux musl | 1.952→1.603 | −17.89% | 7.089→6.827 | 447.93→447.99 |

三目标均保持 297 次 minor、12 次 full、70,981,978 个追踪对象及相同晋升字节数；并行装填使部分样本的复制总字节有小幅变化。musl 的 plan 中位数从 1.597→1.657 s、copy 从 1.254→1.265 s，部分抵消了 update 的改善。

strmap 未体现稳定收益，GNU／musl 的小幅退化保留在上表；该程序的 update 总时间仍约 0.12～0.15 s。其余小幅差异也不视作普遍加速。Darwin JSON 初始三对样本随预热明显下降，31 对补测为 5.670→5.677 ms，未复现初测约 20% 的退化。以上均为描述统计，不作显著性结论。

## 机器码与存储

最终程序的 claim 路径直接消费当前对象。三目标汇编中，搬迁对象所需的 `scoop_heap_object_meta` 调用从两处减为一处；Darwin 的 `scoop_heap_forward_object` 调用和 x86 原内联的 source forwarding 路径均消失。真实字段和 roots 的 forwarding 路径保持。

Clang 布局结果确认搬迁计划项保持 48 B，1024 项存活清单分块保持 8208 B，block 元数据保持 144 B。没有为每个对象同时保存 source 与 destination，也没有新增 managed root、静态 GC 描述或对象字段。六个程序构建记录的 root 产物指纹及依赖保持一致，源码编译与库生产不涉及新的优化工作；本项构建成本单独对照实际改变的 runtime C 冷构建。

| 目标 | 每程序代码段变化 | 完整程序大小变化 | runtime 冷构建墙钟（s） | 墙钟变化 |
| --- | ---: | ---: | ---: | ---: |
| Darwin ARM64 | +4 B | 全部不变 | 4.548→4.505 | −0.96% |
| Linux GNU | −192 B | 全部不变 | 5.387→5.383 | −0.07% |
| Linux musl | −192 B | JSON −4096 B，其余不变 | 5.626→5.638 | +0.22% |

冷构建使用相同 builder 与 vendor，每轮独立空缓存，一对预热、三对正式交错测量；CPU 时间变化 −1.08%～+0.23%，构建峰值 RSS 变化 −0.98%～−0.22%。搬迁计划和存活清单没有新增存储，目标负载的三目标耗时均下降，构建成本基本持平，因此采用所有 profile 默认的单一实现；保留上述程序差异作为后续优化对照。

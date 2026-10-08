# M34 实施记录

开始日期：2026-10-09。基线为 `e97b40f5c9c408e2ff785e5d09c6bf9222c78979`（M33）；工作分支为 `codex/m34`。

本文件区分目标规范与已实现能力；设计和路线图的原始 M34 草稿是本轮输入。各功能完成必要验证后分别提交，不以文档修订代替实现。

| 批次 | 状态 | 实际出口 |
| --- | --- | --- |
| M34-1 规范与基线 | 已完成 | 三份规范先行修订；两地 M33 基线、混合 AS1／metadata 返回及目标 C ABI 实验完成，详见下文。 |
| M34-2 双字接口 | 待实施 | 表示、slot receiver、niche、GC、AtomicRef、native、closure／coroutine、跨 Cone。 |
| M34-3 MIR 优化 | 待实施 | 实际类型传播、去虚拟化、三档调用点内联与累计增长控制。 |
| M34-4 poll 与分配 | 待实施 | 条件 poll、pending 激活、线程局部统计及汇总。 |
| M34-5 小值与 DirectC | 待实施 | DirectParts 与两套目标 C aggregate ABI。 |
| M34-6 多 region | 待实施 | 稀疏地址索引、扩容、large mapping、cards。 |
| M34-7 搬迁与归还 | 待实施 | source／target 规划、完整回写、discard／unmap。 |
| M34-8 并行 mark | 待实施 | 首次标记、分块任务、全局终止、存活集合复用。 |
| M34-9 容器 | 待实施 | MaybeUninit 与 ArrayList／StringBuilder。 |
| M34-10 总验收 | 待实施 | 三 target 的功能、CLI、ABI、GC、性能与实际版本清单。 |

## 验证与磁盘使用

优先运行受影响 crate／fixture；最终汇总必要覆盖，不因小改动重复全量。基线运行与其他编译／清理串行，三个 target 分别记录，不跨宿主相除。

本机初始无 target，Linux 初始 target 为 46 GiB。分批清理已链接的中间对象及过期 incremental，保留仍有效的库、CLI、测试产物和构建缓存；记录实际回收字节。构建、运行和清理不并发操作同一 target。

本机默认 LLVM 已升级为 23.1.3；首次基线运行按项目合同拒绝，未计为测量。随后显式使用 `/opt/homebrew/opt/llvm@22`（22.1.8）；Linux scoopc 链接 `libLLVM.so.22.1`。每次 Rust 构建保留显式 LLVM 22 prefix，避免误选其他已安装版本。

## ABI 与格式迁移

目标为 runtime contract 12／metadata ABI 8，旧 ABI 11／7 产物需重建。当前规范已描述目标，代码和 section versions 随实际批次逐项迁移；开发中间批次不作为稳定发布 ABI。persistent 声明身份不因机器表示或优化而改变。

## M34-1：规范、基线与后端可行性

三份规范先行规定双字接口／slot object projection、DirectParts／目标 C 分类、MIR 优化顺序、条件 poll、多 region／cards／归还、并行 mark、线程局部统计和 MaybeUninit；同步移除旧单字／全 aggregate 间接／full 必须搬迁等冲突条文。代码仍以 M33 ABI 11／7 运行为基线，后续实际 ABI 迁移单独记录。

两地各完成八个原有工作负载 × 五次运行，实际输出与 GC 直方图均由原脚本检查；[性能记录](PERFORMANCE.md) 保留全部样本和测量限制。[独立 C 实验](experiments/README.md) 确认整数、混合、浮点 HFA 与寄存器耗尽的两套分类；没有将编译器实验写成尚未实现的 DirectC 功能。

LLVM 22.1 的最小实验确认，双字结果必须经过已有的精确 managed leaf 回写协议；只拆分再组合原 SSA aggregate，可能被 InstCombine 合并并遗漏 relocation。实验结合已有 codegen 路径保存接口存储，跨 site 只对 object 发射根标记和 volatile leaf 回写，再通过 volatile object reload 与原 metadata 重建；仅有 volatile store 不能阻止 LLVM 转发旧值。PHI／select 同样经过这一普通路径，M34-2 将落实对应接口发射。invoke 保留原有零 gc-live／显式根协议，不引入另一套 aggregate SSA 优化或降低验证标准。

验证：工作区 fmt／all-targets clippy 通过；Darwin 与 nuc12 各 24 项 statepoint 定向回归通过，包含原子引用、GC 根合并、循环派发及 amd64 固定调用栈。新增实验覆盖三个 target × debug／release × PHI／select，执行 Scoop 根验证、LLVM safepoint verifier 和真实目标 object 生成。实验代码分为独立 Rust／LLVM 文件，各不足 110 行；本批不重复运行无关工作区测试或完整文件 fixture。

已清理本机 target 96 项、3,132,545,645 bytes；nuc12 52 项、1,997,877,943 bytes，保留库、CLI 和有效缓存。详细记录位于两地 `tmp/m34/batch1-target-cleanup.json`。

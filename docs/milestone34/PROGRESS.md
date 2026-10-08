# M34 实施记录

开始日期：2026-10-09。基线为 `e97b40f5c9c408e2ff785e5d09c6bf9222c78979`（M33）；工作分支为 `codex/m34`。

本文件区分目标规范与已实现能力；设计和路线图的原始 M34 草稿是本轮输入。各功能完成必要验证后分别提交，不以文档修订代替实现。

| 批次 | 状态 | 实际出口 |
| --- | --- | --- |
| M34-1 规范与基线 | 已完成 | 三份规范先行修订；两地 M33 基线、混合 AS1／metadata 返回及目标 C ABI 实验完成，详见下文。 |
| M34-2 双字接口 | 已完成 | 表示、slot receiver、niche、GC、AtomicRef、native、closure／coroutine、跨 Cone；三 target 的六组 CLI fixture 通过。 |
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

M34-2 开始启用 runtime contract 12／metadata ABI 8。MIR `cross-cone-type-bridge` 为 18，记录具体对象 receiver 的 interface adapter；LIR `cross-cone-param-free-bridge` 为 2、`cross-cone-layout-abi` 为 13、`cross-cone-layout-link-closure` 为 6，保存 DirectParts 与双字 interface exact layout。目标 ABI 分类合同同步区分 interface parts；其他未改变编码的 section 保留原版本。

## M34-2：双字接口

普通 interface 与同构 niche enum 已使用 16-byte／8-byte 对齐的 `{AS1 object, AS0 itab}`；参数展开为两个 physical parts，结果直接返回两分量。动态调用直接读取携带的 slot table，隐藏 receiver 只传 object。默认方法和装箱方法使用真实生成的 typed adapter，直接 class 实现保持原入口；object 的源码与 backing 视图共用 adapter。普通参数、结果、字段、数组、枚举、捕获和 suspend ABI 均保留完整值。

转换按完整目标 interface identity 选择表，已知具体类型复用实际 descriptor 目录顺序，未知转换在边界查询。None 清零两个分量，空接口仍按 object 判断存在；引用比较、pin、handle 与抛异常只使用 object。GC 只扫描 offset 0，跨 statepoint 从回写后的 object 与原 metadata 重建；LLVM 优化所需的 volatile leaf store／load 已落实，invoke 延续显式 roots。

AtomicRef 的原子槽保持单 object，load／exchange／CAS 结果在 NoGC 路径恢复接口表。Context 内部 binding 使用独立 generated managed-pointer 类型。协程 receiver slot 使用真实 box identity，普通 nominal 与泛型 application 分别保留 Strong／ODR 归属；内部私有 slot 不扩大发布闭包。消费者复用普通依赖类的 descriptor 和 adapter，本地子类及实际泛型实例生成自己的表。

新增六组正式 CLI fixture：值与转换、原子操作、Scoop native ABI、四线程发布与 CAS、挂起／恢复、跨 Cone 产物。覆盖 debug／release、普通／full-moving／minor-stress；跨 Cone fixture 交叉使用 provider／consumer profile，并删除源码后仅凭产物重新链接。挂起用例同时包含 lambda、普通私有值类型和公开值类型；跨库 object 验证共享 default adapter。macOS、Linux GNU 和 Linux musl 的六组均通过；GNU／musl 每个 target 为 12 variants、60 processes、42 stage goldens。两地共用的 14 份 HIR／MIR 快照逐份校验一致，全部 target-specific LIR 快照均已保存。

受影响 IR、ABI、codegen、metadata 与 native 边界的定向回归已通过；最近的 HIR→MIR 生产路径 26 项、协程 lowering 10 项均通过。另运行原有接口 default／super／属性 helper 和函数变型／移动 GC 文件 fixture，3 variants、6 processes、12 stage goldens 通过。新实现按 interface lowering、ABI parts、receiver adapter 和身份记录分文件，未将新逻辑继续堆入已有大型测试文件。

本批两次清理中间对象与 incremental：本机共 38,752,299,448 bytes，Linux 共 2,075,523,097 bytes（按删除文件的逻辑大小记录）。最近一次分别删除 19,386 项与 119 项；有效库、CLI、测试程序及 M33 基线保留。记录在两地 `tmp/m34/batch2-target-cleanup*.json`。

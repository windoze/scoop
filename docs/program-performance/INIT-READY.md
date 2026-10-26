# 初始化完成后的就绪快路径

基线为 `7e49855d6`，复用已经完成的接口 SSA、根恢复与 runtime 可见化能力。本项只处理初始化状态检查；Context 访问器仍按后续热点测量安排。

codegen 对真实 typed `ManagedRuntimeFunction::InitializationEnter` 的普通调用生成 acquire 状态读取。Initialized 直接得到 Ready，其余状态进入原 `scoop_rt_init_enter`；只有慢边物化、恢复该站点的根。ensure 调用及其入口 poll 仍然保留，未就绪、失败、等待、循环检测和 invoke 均保留原协议。普通 extern 不按符号拼写替换。后续 singleton root 继续重新读取，不缓存跨 safepoint 的对象地址。

cell 仍为 16 B、对齐 8，state 位于 offset 0；没有对象、接口值或静态状态大小变化。已发布的 state 全部采用原子访问；成功先写 storage，失败先写 failure root，再分别 release 发布终态。锁内 state 读取／Initializing 写入使用 relaxed，owner、依赖栈与等待图保持锁保护。runtime contract 升为 18，metadata ABI 9 和 `.slib` 容器保持；旧库、runtime 与开发 sysroot 必须重建，避免新原子读取搭配旧普通写入。

两机格式化、Clippy 和 C warnings-as-errors 通过；runtime ABI 各 5 项、初始化定向单测 ARM64 20 项／Linux 21 项通过。覆盖生成的 acquire、慢边根恢复／relocate、一次性初始化、并发等待、同线程／跨线程环及等待中的移动 GC。本机 ThreadSanitizer 通过四读者发布测试：读者没有 coordinator 锁，也没有其他从写者到读者的同步，仅以 acquire state 读取普通 storage。

三目标各一个正式 GC fixture、debug／release 两个变体通过，覆盖单例身份、带存活输入的首次／重复访问、失败 Throwable 身份及循环异常缓存。debug 强制搬迁实际复制 43,016 B；release minor 模式实际复制 24 B，发生一次 minor 和 41 次显式 full。两种模式分开运行，避免强制 full 掩盖 minor。没有完整验收。

性能对照使用相同源码、布局和 runtime C 代码；各平台双方的 92 个 runtime 目标文件逐字节一致，仅按各自编译器生成兼容的 ABI 索引。消费配套预编译库，完整 LIR 相同。普通 release，一次预热、五对 AB／BA；Linux 固定在 nuc12 的 CPU 4。每模式两百万次，失败路径两万次，输入及首次初始化在计时前完成。下表为包含结果消费的 ns／次中位数，不代表整个程序或首次启动的加速比。

| 路径 | Darwin ARM64，旧 → 新 | GNU x86-64，旧 → 新 | musl x86-64，旧 → 新 |
| --- | ---: | ---: | ---: |
| 顶层运行期属性读取 | 18.819 → 1.728 | 18.832 → 0.856 | 22.066 → 0.855 |
| 单例字段读取 | 19.387 → 1.728 | 18.652 → 1.122 | 22.406 → 1.069 |
| 单例字段递增 | 19.260 → 1.728 | 19.077 → 1.298 | 22.277 → 1.210 |
| 泛型 companion 字段读取 | 17.189 → 1.730 | 17.049 → 1.119 | 20.768 → 1.069 |
| 普通对象字段读取对照 | 0.186 → 0.186 | 0.1382 → 0.1444 | 0.1351 → 0.1327 |
| 重抛已缓存的失败 | 5530.1 → 5548.3 | 3007.953 → 3017.075 | 3207.803 → 3222.877 |

已就绪目标减少约 90%～96% 耗时。实际机器码在状态读取后跳过 `scoop_rt_init_enter`，省去该次调用中的 TLS、managed anchor、锁及协调器工作；ARM64 使用 `ldar`，x86-64 使用满足 acquire 的普通机器 load。循环本身的指令数量、调用位置和 poll 没变，ensure 每个增加 ARM64 8／x86-64 5 条指令，完整慢路径仍在。GNU 普通对象对照增加 4.48%（0.0062 ns／次），其指令数量保持；失败路径增加 0.30%～0.47%。保留这些差异，不将短对照归为已获益或推算 forwarding 周期。

另行 GC 统计中，双方就绪／普通对象模式总分配均为 176 B，失败模式均为 224 B，均无 collection；没有新增每次访问的分配或分配次数探针。静态 cell、storage、根／stackmap 段大小保持。成本对照如下，编译为三对独立冷 consumer 构建中位数：

| 成本 | Darwin ARM64 | GNU x86-64 | musl x86-64 |
| --- | ---: | ---: | ---: |
| text 增量 | +1,184 B | +576 B | +576 B |
| 完整程序文件增量 | 0 B | 0 B | 0 B |
| core.slib 增量 | +976 B | +802 B | +1,092 B |
| 编译墙钟变化 | +6.80% | −1.95% | −0.03% |
| 编译 CPU 变化 | +1.01% | −2.22% | −0.56% |
| 编译峰值 RSS 变化 | +0.20% | −0.05% | −0.07% |

ARM64 编译墙钟的增加保留记录，CPU／RSS 没有相同比例的增长；不把 Linux 的小幅负差解释为稳定编译加速。该小型快路径在全部 profile 默认生成，无新开关、诊断或测量代码。它解决反复访问已完成单元的固定开销，不省去首次初始化、失败处理、实际 ensure 入口或后续根重读。

原始日志、逐次数据与机器码在仓库外 `scoop-performance-results/2026-10-26/init-ready`，ARM64 在本机，GNU／musl 在 nuc12；临时基准和构建输入位于忽略的 `tmp/program-performance/init-ready`。

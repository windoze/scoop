# 普通 managed poll 与有界叶函数

本项承接 [poll 本线程读取](POLL-OWNER-READS.md)。新编译器对所有 profile 默认采用，不增加公开选项、线程参数或专用寄存器，也不改变 runtime ABI 16、metadata ABI 9 与 `.slib` 合同。

## 选择与正确性依据

- 普通 Managed 函数从已经激活的线程进入。Full poll 只以 acquire 读取全局 world phase，RUNNING 时继续；不读取 TLS、mode、全局 epoch 或 observed epoch。ARM64 每个 Full poll 从上一步的两条 `ldar` 加两条 `ldr` 降为一条 `ldar`。更早四条 `ldar` 的基线见上项记录。
- root／initialization startup gateway 按完整 callable body key 识别，保留 Gateway 入口检查与 MANAGED_PENDING 的有锁激活。callback 由 runtime 先激活，native 返回保留原握手。没有按源函数名称选择优化。
- collector 在 STOPPING 后仍等待每个 MANAGED 线程停稳。线程暂时读到 RUNNING 只会推迟停稳，不能允许 collector 提前扫描或移动它的引用。共享 phase 的原子读取不能被提升出循环；慢边、anchor、epoch 协议、精确 roots 和 relocation 保持。
- 原有界叶函数资格保持：最终 CFG 是正常返回的无环直线路径，最多 64 条操作与终结指令，每个值不超过 64 B，无调用、分配、循环、异常或 native／全局存储访问。这些普通函数省去入口 poll；不扩展到任意 NoGC 调用，不向 caller 传播新效果承诺，callable 仍为 Managed。
- 有界叶函数没有自身 safepoint 也保留 callable registration。旧库多做检查仍相容；接口、装箱、值 ABI 与对象大小不变，没有新增对象引用或 GC root。

## 定向验证

本机格式化与 workspace Clippy 通过。LIR lowering 的 189 项测试通过，包括有界／超界叶函数、root／初始化 gateway、循环 roots 与 NoGC；一个代码生成专测在 Darwin、GNU、musl 上分别验证 Full／Gateway 的原子读取、TLS 缺席、仅慢边物化 roots 及 statepoint rewrite。

三个目标运行同一组正式专测：`poll-threads`、`poll-thread-roots`、`poll-codegen-roots`、`small-values-artifacts`、`small-values-native`、`ssa-roots`。覆盖 debug／release、跨 profile 的预编译依赖、值参数／结果、callback、并发停止、moving／minor、接口／tuple／可空值／closure roots 和重新链接。未运行完整 fixture 验收。LIR 单元快照只有 poll 删除与 Gateway 标记变化；部分 CLI 快照同时补齐此前内联变更的旧期望，已用保留的基线编译器对同一输入核对，本次实际 LIR 差异仍只有 poll。

ARM64 六个成员基准内核每个有入口、循环两个 Full poll：`ldar` 总数 4→2，均省去一次 TLS resolver 间接调用；动态调用内核仅保留真正的方法间接调用。`Mixer.apply`／`Bias.apply`／`Bias.amount` 分别从 20／27／22 条指令降到 6／7／5 条，callee 内没有 poll 或调用。系统库重编译后，`Long.hash` 从 28 条降到 14 条，保留实际 native hash 调用与 Full 慢边，入口不再取得 TLS。

## 性能与成本

以 `457a54f4a` 的编译器为基线，固定相同预编译系统库、runtime 与输入，只重编成员基准的 provider／consumer。使用既有 A/B 工具：一次预热、七对交错运行，每种负载 2000 万次；Linux 固定 CPU 4。六种负载分别核对 0、1、2、3、4097 次的独立预期值，GC 诊断与计时分开运行，分配／收集／root slots 等 17 项计数保持。

| 目标 | 动态调用内核，基线 → 当前 | 内核耗时降低 | 整进程耗时降低 |
| --- | ---: | ---: | ---: |
| Darwin ARM64 | 38.820 → 32.405 ms | 16.52% | 14.21% |
| Linux GNU | 36.252 → 26.278 ms | 27.51% | 23.93% |
| Linux musl | 39.136 → 23.670 ms | 39.52% | 28.76% |

已内联的对照主要不再支付 callee 入口成本，收益较小。首轮其余五组内核变化：Darwin −0.29%～+0.51%，GNU −6.36%～+2.45%，musl +0.01%～+3.41%；这些正负变化全部保留，不把它们当作稳定收益。

只对首轮偏差较大的三个对照复测，将次数提高到 2 亿，仍用一次预热、七对交错。GNU reference／accessor 仍分别为 +2.11%／+2.58%，每次迭代增加约 0.0093／0.0114 ns；musl value 变为 −4.35%，首轮 +3.41% 未复现。保留 GNU 的实际小幅回退；动态调用在三个目标上收益稳定，且本项没有代码／静态空间增长，因此采用同一默认实现，不为这些对照另设布局或函数特判。

| 目标 | text 减少 | GC 静态记录减少 | 完整程序减少 | provider / consumer `.slib` 减少 |
| --- | ---: | ---: | ---: | ---: |
| Darwin ARM64 | 1,924 B | const 720 B、stackmaps 464 B | 2,032 B | 8,708 / 1,616 B |
| Linux GNU | 1,536 B | 合并 metadata 1,184 B | 10,288 B | 17,808 / 3,610 B |
| Linux musl | 1,536 B | 合并 metadata 1,184 B | 6,192 B | 16,012 / 3,520 B |

编译成本按同一输入直接调用 `scoopc`，一次预热、五对交错测量，排除链接。provider／consumer 的墙钟变化分别为 Darwin −0.45%／−1.26%、GNU −2.13%／−1.40%、musl +1.43%／+0.24%；峰值 RSS 变化 −0.58%～+0.22%。本项减少指令与静态记录，不引入分析框架或额外运行时存储，作为默认能力采纳。

随后按正常流程重编并发布开发 sysroot 的 Darwin 22、GNU 21、musl 21 个系统库，另检查新库的启动入口与 GC 组合。全部 `.slib` 合计分别减少 9,828,210／14,575,722／14,702,200 B；相同成员程序相对基线分别减少 139,408／131,120／127,024 B。补充的 Darwin 零次循环启动对照为三次预热、十五对交错，中位数 8.901→8.765 ms；它包含系统库中 poll／safepoint 记录减少的影响，不代表完成 callable 可达性裁剪。

清理本机 67 份被更新会话替代的旧 Rust 增量缓存，移除约 6.74 GiB 逻辑文件大小；保留每个 crate 目录的最新完成会话及工作中会话。

诊断／测量代码没有新增。原始命令、产物、计时与复测保留在两机仓库忽略的 `tmp/program-performance/active-polls/`。

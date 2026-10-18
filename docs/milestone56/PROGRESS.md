# M56 实施记录

状态：实施中，2026-10-19 开始。范围与完成条件以 [DESIGN.md](DESIGN.md) 第 9、10 节为准；未完成项不得以设计或历史性能数据代替验收。

## 交付检查表

| 项目 | 状态 | 验证／取舍 |
| --- | --- | --- |
| A：当前 release 工具、开发 sysroot 与真实程序基线 | 本机基线就绪 | 完整库 sysroot 准备成功；分配与拆箱基线已建立，后续按项目追加 |
| B1：内部 TD 验证结果复用、native 边界保留 | 本机通过 | 30 项 collector 测试、8 个 CLI 变体及完整分配对照；Linux 最终复验 |
| B2：unbox／clone 单次对象定位 | 本机通过 | 单次 exact-size 定位；非法地址／类型、large clone、移动与含引用值 |
| B3：String 有界表示 intrinsic | 待完成 | byteLength、byte-at、cursor 解码；UTF-8、越界与 GC 组合 |
| C1：NoGC／无 site 循环优化及 Managed 保守优化 | 待完成 | 实际向量化、唯一 site／roots、EH／原子与响应 |
| C2：局部 card 查询／mark 复用 | 待完成 | 同 card／跨 card、同步边界、多 mutator、minor／moving |
| D1：nursery／refill 对照与策略取舍 | 待完成 | 1／4／8／16 MiB，存活率与 1–8 mutator；吞吐／内存／停顿 |
| D2：有搬迁 GC 并行 update 候选 | 待完成 | 串行 claim、槽分区、完整阶段屏障；单／多 worker 同义性及净收益 |
| E：声明构建、局部 box／协程消除、跨 Cone 内联 | 待评估 | 逐项记录设计规定的进入条件与实测，不以无依据延期替代调查 |
| 本机阶段验证 | 待完成 | 每批先格式化／lint，相关 crate／runtime 与正式 CLI fixtures |
| Linux 完整验收 | 待完成 | nuc12 独立 worktree：workspace、fixture runner 规则和完整 CLI；GNU／musl 合同 |
| 逐功能提交、临时资料与 target 清理 | 进行中 | 只提交本任务变更，原始日志在仓库外，不使用绝对 `/tmp` |

## 工作环境与验证策略

本机 Apple M4 Pro／macOS 27.0.1，LLVM 22.1.8；Linux 从 `nuc12:~/repos/scoop` 新建独立 worktree，并为本任务使用独立 target、TMPDIR 和测试缓存，不修改其他 agent 的 checkout／构建输出。开始时远端 HEAD 为 `3cbc339f8`，完整验收前将本次提交导入独立 worktree。开始时本机 target 为 4.8 GiB、远端原 checkout target 为 1.6 GiB；按阶段清理无用构建产物，避免每个小改动反复全量构建。B1 验证后删除 2.6 GiB 的旧 incremental 缓存，本机 target 从约 6.6 GiB 降至 4.0 GiB；本任务设置 `CARGO_INCREMENTAL=0`。

仓库外工作目录为 `../scoop-work/m56-20261019`，包含 scratch、基线／候选测量与原始日志；命令显式设置 TMPDIR。性能样本先完成构建，再串行交替运行。正常应用使用显式准备的开发 sysroot，runtime 专测继续用已有源码构建测试入口。

已有的 M47 两份文档和 M55 设计不属于本次提交。初始内容摘要保存在仓库外，结束时核对；不使用全仓 reset／checkout，也不删除／隐藏源码或工具来验收缓存。

## 实施与验收记录

### B1：复用已登记的 TD

先补充运行时规范 2.2。`value_shape` 直接消费 generated-code、对象头和内部已登记 TD，保留动态 kind／size／count／alignment／overflow 检查；公开 native `scoop_rt_alloc` 在进入内部实现前检查登记。没有新增 unchecked 公共入口、凭证或第二套尺寸算法。C 回归新增合法 native 分配控制组及未登记、null、不可读 TD 的拒绝覆盖。

验证：C 使用 LLVM 22.1.8 clang-format（4 空格、100 列），Rust `cargo fmt --all` 与 `cargo clippy --workspace --all-targets` 通过；`scoop-codegen` 的 30 项 runtime collector 测试通过，包含 normal／moving 表示用例与 native 边界。fixture runner 的 50 项规则测试通过；新增 `m56-runtime-allocation`、`m56-runtime-representation` 共 8 个 debug／release／moving／minor 变体通过，无 golden。首次分配用例误用要求 Node.equals 的 Option 比较，改为已知长度遍历后通过，没有改语言规则迁就测试。

真实程序对照使用同一 release 编译器和 core，分别显式选择原始 runtime 副本与当前源码。循环分配两百万个 24-byte Node，写入 1024 项保留环并累加数值；末尾检查保留值和 checksum `1999999000000`。原生 monotonic clock 只在循环外取时，诊断 `SCOOP_GC_STATS=1`。先完成构建再串行测量，一轮预热、七轮交替独立进程，不固定 affinity：

| 版本 | kernel 中位 [最小, 最大] ms | ns／分配 |
| --- | ---: | ---: |
| `28fab1812` runtime | 71.363 [69.767, 75.752] | 35.682 |
| B1 | 43.332 [42.524, 43.915] | 21.666 |

该工作负载约减少 39.3% 时间；全部样本均为 45 次 minor／0 次 full、48,008,432 allocated bytes、1,114,160 promoted／copied bytes，dirty cards／root slots／traced objects 等工作量也一致。这是包括循环、引用写入与实际 GC 的 kernel 成绩，不是所有应用的加速比；Linux 尚待复验。

环境准备：release 工具、core、runtime 和第三方 native 后端已准备。完整 sysroot 首次在尚未准备好的 IDNA header 处停止；依赖已随后补齐，继续复用已有结果准备其余 Cone。原始构建、测量 JSON 与 fixture 工作目录均在仓库外。

### B2：拆箱与克隆复用单次对象定位

先补充运行时规范 2.3。两个表示入口在原有 heap lock 内调用一次 exact-size 查询，在读取对象头前确认对象起点，并用所得尺寸校验动态 shape；删除重复的 object-start 查询，没有增加新查询接口或跨 safepoint 缓存。保留类型／枚举父类型匹配、长度与实际分配尺寸检查。非法地址仍明确 abort，诊断沿用 exact-size 查询。

格式化、workspace clippy 和 descriptor-driven representation 专测通过；该专测在 normal／moving 两种模式覆盖内部地址、null、非对象地址、未匹配类型及动态 size。正式 CLI 的 3 个 fixture 共 12 个变体通过，新增整数枚举按原身份与父整数拆箱、错误转换异常、large array 克隆独立性；原含引用／ZST 用例继续通过。完整开发 sysroot 已准备成功。

同一 release 编译器／core 对照 B1 与 B2 runtime：一个 `Any` 中的 Pair 在循环内拆箱两百万次并累加两字段，checksum `30000000`；一轮预热、七轮交替独立进程，构建完成后串行测量。B1 中位 112.989 ms [112.512, 114.101]，B2 102.124 ms [101.818, 102.482]，约减少 9.6%。全部样本 allocated bytes 为 224，无 collection／复制／晋升；该结果包含类型测试、锁与循环成本，不代表所有 Any 使用方式。Linux 最终复验。

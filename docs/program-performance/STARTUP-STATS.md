# 编译期开关控制的启动测量

在 `cd40662f2` 的 runtime 上增加可选启动阶段测量。`scoop build/run/test/link --enable-startup-stats` 和 `scoop-link --enable-startup-stats` 选择编入统计的 runtime；debug、release 均默认不编入。默认版本通过 C 预处理排除全部新增计时、阶段状态、报告、诊断字符串和专用计数访问，不能通过环境变量启用，也不依赖优化器消除死代码。

这是测量基础设施，供后续通用静态优化和 runtime 优化使用；没有新增 PGO 消费者、按函数名称的例外或自动调参。

## 记录范围与构建合同

开启后，`scoop_rt_run_program` 从读取 platform／加载镜像范围前开始计时，在 eager 初始化完成、root gateway 之前向 stderr 输出一行 JSON。`phases_ns` 的七个互斥阶段为 `loaded_images`、`collect_records`、`code_and_types`、`storage_and_units`、`stackmaps`、`runtime_init`、`eager_init`。使用八次单调时钟读取，每条记录的阶段之和精确等于 `total_ns`。

`counts` 直接读取已有索引：Cone image、物理 image、type、callable、safepoint、合并后 stackmap、不同地址 scan program、static storage／root、immortal、initialization／eager unit、context key 的数量。专用 scan program 查询仅在统计版编译，其他数量直接读取已有字段；不为诊断再遍历或验证图，不增加对象字段或 managed roots。

计时不包含 runtime 入口前的 dyld／CRT、root gateway 的 managed 参数构造、main、shutdown 和打印报告；阶段失败不输出伪造的完整记录。eager 自身的 stderr 可以出现在记录之前。完整进程成本仍用外部 A/B 测量，不能把这一时钟解释为全部启动时间。

runtime 的 `SCOOP_ENABLE_STARTUP_STATS=0/1` 进入编译 flags、构建记录和 index 配置。index schema 7 的配置 field 5 保存该布尔值，开发缓存使用 `runtime-v8`。普通／GC stress 与启动统计的两个布尔配置分别选择 `runtime`、`runtime-stress`、`runtime-startup-stats`、`runtime-stress-startup-stats`。准备与打包脚本都覆盖四者；显式 index 也检查请求一致，不能默认误链统计版。该配置只影响 runtime 与最终链接，不改变 Scoop Cone 编译结果、`.slib` 或 ABI。

## 定向验证

- 两机 workspace Clippy、本机与 Linux helper Clippy、固定 Ruff，以及 Clang／GCC 的宏 0、1 配置 `-Wall -Wextra -Werror` 均通过。
- runtime 构建测试实际编译四种组合，验证 index／fingerprint 独立、再次命中和双向配置拒绝；扫描实际对象，默认及仅 stress 配置均无新增报告、计时错误字符串和专用查询符号。CLI 转发测试覆盖开关位于子命令前后，以及程序参数开始后的普通文本转发。
- `compiler-correctness/startup-stats` 在 Darwin ARM64、Linux GNU、musl 的 debug／release 共 6 个 variant、54 次外层进程调用和 48 次 probe 子进程运行通过。检查最终二进制的代码标志、开关切回后的排除、七阶段与计数、eager／报告／main 顺序、非零正常返回、GC 统计组合、两个独立链接入口及配置不符诊断。
- 开关变化时 root `.slib` fingerprint 相同、编译子进程为零，最终链接 fingerprint 按实际配置变化；切回默认恢复原链接 fingerprint。
- 三目标均已重新准备四种开发 runtime，复用原系统 `.slib`，没有重复构建系统库或运行全量验收。打包脚本的 4 个现有单元测试通过。

## 机器码与空间

两个实际程序分别仅输出整数，以及调用 `scoop.json` 将 `"7"` 解码为 Int；三目标输出均为 `7`。与保存的无启动测量源码重新构建的基线相比，六对默认可执行程序的**整个代码段逐字节相同，完整文件大小相同**，启动函数大小也相同。默认产物没有新增启动诊断代码。

开启后的增量如下，均为 bytes；完整文件增量包含节对齐、符号表和平台签名影响：

| 增量 | Darwin ARM64 | GNU x86_64 | musl x86_64 |
| --- | ---: | ---: | ---: |
| `.text`，两个程序相同 | 548 | 1,024 | 896 |
| 最小程序完整文件 | 80 | 8,248 | 56 |
| JSON 程序完整文件 | 80 | 56 | 4,152 |

默认最小程序代码段为 1,064,936／1,206,113／1,275,774 B，JSON 为 1,233,388／1,388,321／1,457,982 B。Scoop 元数据和方法机器码没有因测量复制。

## 时间与编译成本

M3 Ultra／LLVM 22.1.8 和 NUC i7-12700H／LLVM 22.1.2；Linux 固定 CPU 4。每项三组预热、31 组交错 AB／BA，不并发编译。以下为完整进程墙钟中位数，单位 ms；开启后的结果已经包含诊断输出开销：

| 负载 | Darwin 关闭 → 开启 | GNU 关闭 → 开启 | musl 关闭 → 开启 |
| --- | --- | --- | --- |
| 最小程序 | 11.189 → 11.242 | 13.946 → 14.013 | 34.746 → 34.747 |
| JSON | 12.512 → 12.564 | 17.057 → 17.324 | 41.492 → 41.360 |

这组差值包含计时／输出及代码布局的观测扰动，不作为程序优化收益；musl 的负差值也不解释为统计使程序更快。默认版本对无测量基线的中位数变化为 −1.22%～+0.88%，代码段完全相同，不归因于本项。

阶段统计直接复用上述开启样本的 stderr，无额外采样运行。主要三阶段和总量的中位数如下，单位 ms；各列独立取中位数，表中列值无需相加等于总量：

| 目标／程序 | 汇集注册记录 | 代码与类型 | stackmap | runtime 内启动总量 |
| --- | ---: | ---: | ---: | ---: |
| Darwin 最小 | 2.143 | 1.801 | 2.334 | 6.348 |
| Darwin JSON | 2.541 | 2.111 | 2.775 | 7.531 |
| GNU 最小 | 4.048 | 3.138 | 4.231 | 11.560 |
| GNU JSON | 4.989 | 3.786 | 5.267 | 14.367 |
| musl 最小 | 15.552 | 7.752 | 10.008 | 33.606 |
| musl JSON | 18.636 | 9.320 | 11.838 | 40.108 |

三目标计数一致：最小程序有 3 个 Cone image、2,090 个 type、3,011 个 callable、7,229 个 site／stackmap、1,250 个 scan program；JSON 分别为 4、2,314、3,402、8,689、1,409。两者均没有 eager unit，专测另覆盖非空 eager 初始化。后续先优化注册汇集、不可变 owner 关系的重复查询及 stackmap 合并；本项并未消除这些工作。

Scoop Cone 无额外编译成本。单次冷构建完整 runtime 的关闭／开启时间为 Darwin 5.03／4.72 s、GNU 5.16／4.98 s、musl 6.00／5.92 s；它们是准备成本记录，未做重复统计，不据此宣称编译提速。发行准备新增两个可选资源目录，默认程序不链接它们。

首次专测修正了独立链接器标准 `link-plan` 输出的预期；没有更改产品行为。机器码提取曾重新写出一份临时基线文件，最终对照改用保留的无测量 runtime 源码重建，重新完成受影响的默认比较；最终代码提取使用独立输出文件。原始日志、样本和临时工具保存在两机忽略的 `tmp/program-performance/startup-stats/`。

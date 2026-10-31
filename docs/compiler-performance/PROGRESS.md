# 编译速度与 `.slib` 尺寸优化

开始日期：2026-11-01。源码基线 `427ef37cccc67ef4b95942ef31ea7c51ec73410c`。先测量实际编译与当前产物，再按收益安排实施；不降低语言检查、生成代码的优化等级、GC／ABI 正确性或调试能力。每轮完成后更新本文、清理无用临时产物、提交并推送，再合并 `origin/main`。

已有工作见[编译性能修复](../COMPILER-PERFORMANCE.md)和[存储精简与压缩](../slib-storage/DESIGN.md)。本任务从这些优化之后的当前版本继续，不将过去的收益再次计入。

## 基线与方法

本机为 Apple M3 Ultra、macOS 27.2、Rust 1.99.0、LLVM 22.1.8。显式设置 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`；PATH 中默认 LLVM 已是 23，不用于这些对照。编译器与 Scoop 库均使用 release。

以真实的 `scoop.std` 和 `scoop.http.client` 源码执行 `scoop build --release --emit-slib`。每次源码重编译使用独立的新缓存和输出目录，复用同一套预编译依赖；随后用原命令测量输入未变的缓存命中。三组旧／新交替执行，报告中位数并保留全部样本。操作系统文件缓存保持正常，不将其称为整机冷启动；采样运行与正式计时分开。`/usr/bin/time -l -p` 记录墙钟和最大单进程 RSS，计时不包含事后产物比较。

首次未采样观测为标准库 13.74 秒、HTTP 客户端 102.78 秒；正式交替样本更稳定，以下收益均以同组对照计算，不用首次较慢的数值放大收益。采样中标准库约 50%、HTTP 客户端约 60% 的主线程样本落在对象生成；元数据分区分别约占总样本的 26%／23%，其中反复建立机器代码生成 pass 占明显成本。

当前 Darwin/AArch64 sysroot 的 29 个库合计 419.13 MiB、238,683 个归档成员：对象 171.75 MiB、LIR 130.28 MiB、HIR 44.66 MiB、manifest 41.87 MiB、MIR 16.80 MiB，其余为 ar header／padding。该清单只读现有产物，库集合不同于此前 25 库报告，不能直接作跨报告尺寸增减比较。

## 第一轮：静态元数据直接由 LLVM 汇编器发射

只有数据、别名及函数声明的分区使用 LLVM 的目标 AsmPrinter／MC；不为每个小对象建立无实际函数可处理的指令选择、寄存器分配及机器函数优化流水线。目标配置、物理分区、符号、TLS、COMDAT、DWARF、module assembly 和原生对象处理保持原合同。函数对象继续使用完整流水线，无新增依赖、用户开关或自行实现的对象编码器。规范见实现规范 2.5。

| 实际源码重编译 | 原版中位数 | 新版中位数 | 耗时降低 | 原版／新版范围 |
| --- | ---: | ---: | ---: | --- |
| `scoop.std` | 11.649 s | 10.364 s | 11.0% | 11.581～11.739 / 10.332～10.375 s |
| `scoop.http.client` | 95.492 s | 88.899 s | 6.9% | 94.471～96.355 / 84.745～89.549 s |

两库三组新旧产物全部逐字节相同，完整 `.slib` 仍为 12,054,970 和 85,426,772 bytes。本轮是编译速度收益，没有尺寸收益。输入未变的缓存命中中位数约 20～26 ms，未变化的缓存路径没有新增工作。RSS 中位数分别为 1,448,673,280 → 1,482,440,704 bytes、12,054,560,768 → 12,136,710,144 bytes；未观察到内存下降，不将小幅差异解释为内存优化。

验证：本机格式化、workspace all-targets Clippy 与 release 编译通过。新增独立测试以 LLVM 完整流水线为参照，在七种目标、debug／release 两种配置下逐字节比较数据对象，覆盖初始化数据、BSS、TLS、别名、外部函数／数据引用、COMDAT 与 DWARF。标准库完整生产另作逐字节对照。

本机后端整组为 388 项通过、1 项失败：`windows_statepoints_record_the_return_pc_before_seh_padding` 直接调用 LLVM 旧入口，本机 Homebrew LLVM 缺少仓库已有的 `llvm-windows-statepoint-return-pc.patch`，得到 16 而非 15 的返回 PC。该测试不经过本轮新增路径；补丁要求见 vendor 的 LLVM 说明。本轮保留此失败记录，不改断言、不宣称 Windows 原生运行已验收。

Linux 在 `nuc12` 的独立目录 `~/repos/scoop-compiler-speed-20261101` 使用 Rust 1.99.0、LLVM 22.1.2，格式检查、release workspace all-targets Clippy 通过；后端 401 项通过，显式排除上述依赖未安装 LLVM 补丁的检查。Linux 包含实际原生运行与对象合同测试，新增逐字节对照同样通过。

正式 CLI 的 `generic-reader`、`runtime-images`、`direct-c/artifacts`、`interfaces/artifacts`、`gc-correctness/native/tls-threads` 五组全部通过：20 个 fixture、23 个模式组合、151 次实际进程调用，含 5 份专用 golden 对照。覆盖预编译泛型消费、初始化顺序与失败、跨 profile 的 C ABI 和接口产物、TLS／回调／线程及定向 moving GC。普通验收复用同一 sysroot 和共享 Cone cache；并未执行全仓库全部 fixture。

## 后续优先级

1. 复用函数对象发射的 LLVM pass manager。采样确认逐对象建立完整流水线仍占主要成本；LLVM 的 AsmPrinter、MCStreamer 和 MachineModuleInfo 已有逐模块收尾／清空逻辑。先用不同模块连续发射的独立对照验证状态隔离，再决定落地，保持每个函数的原优化与物理分区。
2. 减少真实依赖闭包内重复的 ODR 物化与持久化记录。现有 29 库对象存储中，约 38.19 MiB 的对象 bytes 在多个库重复；这是全库集合的离线上限，尚未扣除不存在实际依赖关系的重复，不能直接宣称可全部删除。需沿当前 typed 导入、ABI、可达性和发布职责量化可复用部分。
3. 继续从对象和 LIR 的实际大项评估内容精简与存储方式。普通 LIR 字符串表投影仅使 core／std／HTTP client 的 LIR 成员减少约 0.22／0.04／2.08 MiB，占完整归档更低，暂不为它增加格式变更。已有 HIR 来源索引的收益也较小；在更大方案完成前不优先实施。

本机原始命令、采样、逐轮计时和验收日志保留在仓库外的 `/Volumes/Data/home/chenxu/scoop-performance-results/compiler-20261101`；Linux 日志在 `~/repos/scoop-work/compiler-20261101`。工作缓存与对照二进制暂放本仓库 `tmp/compile-speed-20261101`，只保留下一轮实际需要的内容。

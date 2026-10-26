# 最终程序符号清理

## 实现与开启条件

普通 release 和 artifact-only link 默认清理普通内部符号；debug 或 `--keep-symbols` 保留。Darwin 在已有符号本地化之后使用 linker `-x -S`，GNU／musl 使用 `--strip-all`。参与链接的 `.slib`／`.o`、持久 ID、可达性选择、GC／EH 数据和必要动态符号保持。清理在最终链接和签名流程中完成，无运行时开关或诊断代码。

debug 默认旁置 `<output>.map`，release 和 artifact-only link 通过 `--emit-symbol-map` 显式开启，保存真实链接地址与范围；与 `--emit symbols` 的逻辑函数观察配合做离线归因。请求的 map 与 executable 一起缓存和发布，`executables-v3` 更新键包含符号保留策略及 map 开关，命中不读取文件内容。map 开关独立于 `--keep-symbols`，默认 release 不产生这份额外文件。下文既有体积和性能测量使用带 map 的配置。

## 文件体积

同一份应用 `.slib`、系统库和 runtime，分别保留／清理符号。Darwin 为 M3 Ultra、LLVM 22.1.8；Linux 为 NUC12 x86_64、LLVM 22.1.2。前后工具均含身份图优化，使用正常 release。

| 目标／程序 | 保留符号 B | 默认清理 B | 文件变化 |
| --- | ---: | ---: | ---: |
| Darwin arm64 hello | 2,922,056 | 1,357,992 | -53.5% |
| Darwin arm64 nbody | 3,950,136 | 1,855,576 | -53.0% |
| Linux GNU x86_64 hello | 3,397,312 | 1,572,624 | -53.7% |
| Linux musl x86_64 hello | 3,301,432 | 1,461,640 | -55.7% |

Darwin hello 的普通符号 17,642→415，条目 282,272→6,640 B，字符串表 1,286,632→10,320 B，`__LINKEDIT` 1,611,336→47,272 B。16,323 个 Scoop 哈希符号全部消失，其中 11,512 个是定义边界标记，不是函数。nbody 的 21,920 个哈希符号也全部消失。GNU／musl 的 `.symtab`、`.strtab` 消失，GNU 的动态符号表和动态字符串表保持；静态 musl 没有动态符号表。

剩余体积仍以代码和实际数据为主。Darwin hello 的 `__text` 310,724 B、只读 metadata／常量 640,128 B、stackmap 220,144 B、EH frame 21,096 B、unwind 7,344 B 均保持；nbody 对应为 430,316／887,864／326,344／21,424／8,480 B。GNU hello 的 text 372,705 B、metadata 859,864 B（含 stackmap）、动态 relocation 178,344 B 保持。本批没有为进一步缩小文件删减这些运行时合同。

## 地址归因与正确性

- Rust 定向测试在 ARM64／GNU 比较实际链接结果：内部名字消失，动态导入／导出地址不变，text、stackmap、EH／unwind 的地址及内容相同，带 native companion 的程序仍输出 42。
- 三目标 map 与保留符号版的全部 486 个 hello callable 地址一致；Darwin nbody 为 658 个，零缺失。保留／清理两种结果的范围集相同。Darwin 实际 stripped nbody 的 sample 中，124 行明确 PC 可由 map、主镜像 slide 和 symbols 观察定位，全部 658 个 callable 有逻辑名字。
- map 不包含指令源码行表，不推测内联栈或歧义同址函数。Darwin 支持按 sample 的主镜像地址消除 ASLR；Linux PIE 的运行地址须由真实加载信息转换，未新增 perf.data reader。
- 本机 executable-record 29 次与 stage-observations 15 次进程调用通过，GNU executable-record 29 次通过；覆盖 debug／release、显式保留、只重链接及缓存复用。ARM64／GNU 的接口 SSA 双 profile moving GC 专测通过。嵌套 enum 返回另有三目标双 profile 回归。
- Rust fmt、release workspace Clippy、Ruff 与 14 项符号化 Python 专测通过。Linux nbody 缺少独立 sysroot 的 math 产物，本批未测，不从 ARM64 外推。

旁置 map 大小约 2.23 MB（Darwin hello）、2.92 MB（nbody）、5.32／5.51 MB（GNU／musl hello），不计入上述 executable。没有把剥离后的 Scoop 与带内嵌 DWARF 的 Go 或不同符号配置的 Kotlin/Native 作体积排名。

## 成本与最终整合对照

最终链接各预热一次、五轮交替。此前身份优化工具到当前整合工具，hello 中位数 0.7282→0.7936 s，nbody 0.9649→1.0550 s。当前工具的保留符号＋map 为 0.8023／1.0497 s，说明清理本身没有明显额外代价，map 及整合流程仍有成本。两个工具之间也合入了 main 更新，这个总差值不能严格归因于单一 linker 选项。

为计入该成本，再以身份优化前的 `6a5a5cb34` 正常工具与当前整合工具做五对 AB／BA 端到端对照。沿用固定系统库、真实正文／公开签名修改和独立冷项目 cache；每对产物指纹和运行结果相同，executable 因清理符号不再要求字节相同。

| 场景 | 原基线 s（范围） | 最终整合 s（范围） | 变化 |
| --- | ---: | ---: | ---: |
| hello，无改动 | 0.0120（0.0114～0.0240） | 0.0118（0.0114～0.0165） | 毫秒级持平 |
| hello，正文修改 | 2.0988（2.0822～2.1611） | 1.5019（1.4534～1.5350） | -28.4% |
| hello，冷项目 | 2.4231（2.4159～2.5173） | 1.8044（1.7911～1.8402） | -25.5% |
| nbody，无改动 | 0.0125（0.0115～0.0226） | 0.0125（0.0120～0.0151） | 毫秒级持平 |
| nbody，正文修改 | 4.3142（4.2302～4.3849） | 2.5848（2.4290～2.7247） | -40.1% |
| nbody，冷项目 | 4.6358（4.5663～4.6870） | 2.8712（2.8397～2.9313） | -38.1% |
| 依赖公开签名修改 | 2.2893（2.2793～2.3282） | 1.4670（1.4216～1.5413） | -35.9% |

无改动确认为零 compiler child，正文／冷项目为一个，公开签名为两个。编译峰值 RSS 中位数：hello 正文 645.5→619.2 MB，nbody 正文 1053.1→985.9 MB，签名修改 488.3→463.8 MB。身份处理分项归因见[独立阶段记录](COMPILER-IDENTITIES.md)，其单独收益不当作最终整合收益。

程序运行使用同机五对正常 release。Darwin nbody 25M 墙钟 1.50375→1.50847 s、CPU 1.50114→1.50619 s，峰值 RSS 7,274,496→7,290,880 B；hello 墙钟约 5.08→5.07 ms。Linux hello 为毫秒级，GNU／musl 均无稳定运行收益。文件体积改善没有被报告成执行速度、分配量或 RSS 改善；后续 GC 工作单独建立运行基线。

原始日志、逐轮 JSON、sample 和汇编位于仓库外 `scoop-performance-results/2026-10-27/compile-size`；Linux 原始记录在 `~/repos/scoop-compile-size-results-20261027`。此次不重复全量验证。

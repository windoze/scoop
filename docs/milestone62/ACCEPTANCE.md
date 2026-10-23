# M62 验收

状态：已完成。按用户最新要求停止尚未完成的完整 fixture 运行，以已通过结果和失败项定向补测收尾；不宣称完整 fixture 验收通过。

## 交付范围

已实现：类型确定后的 intrinsic 统一下降（含泛型／跨 Cone、函数引用、去虚化及内联后暴露目标）、普通跨 Cone 小函数内联、拷贝清理与冷路径成本、intrinsic 标量复制成本、已登记 TD 的热路径复用、有界 core C leaf、Managed 叶子入口激活检查、TLAB 完成路径简化、普通 GC 扫描中的 immortal 重复查找移除，以及 Darwin 内部边界导出精简。系统库完整构建暴露的两类 GC site／物理根计数问题均已闭合；阶段职责、类型化身份、异常、GC 与 ABI 合同继续保留。

各功能提交及独立验证见 [实施记录](PROGRESS.md)，未采用的 nursery、回写、父接口表、HFA 与 HashMap／String 候选见 [对照评估](EVALUATION.md)。

## macOS ARM64 组合验证

- 最终 fmt／workspace clippy 通过；根计数修改后全部 256 项 codegen 测试通过，含 GNU／musl 的目标机器产物检查。
- 标量成本修正后的正式 CLI 组合 11 项、21 个配置变体、60 个进程通过，覆盖 intrinsic、普通跨 Cone／V3 内联、core 浮点／String 以及 moving／minor 多线程根。另有内联增长、实参／异常语义和 direct C ABI 专测通过。std 的 104 项及 core consumer 的 34 项 Cone 测试通过。
- 标量成本修正后全部 21 个系统库及普通／stress runtime 完成重建，最终 http.client 通过；Darwin 真链接测试确认定位符本地保留、动态导出移除、native 入口和执行结果正确。
- immortal 查询调整后，fmt／workspace clippy、C 警告检查、37 项 collector 测试通过，其中配置专测覆盖优化／未优化普通 runtime、stress 与显式堆验证。普通／stress runtime 重建后四项正式 GC fixture、八个变体、40 个进程通过。

设计调查时的原 release 产物与标量成本修正后的 M62 产物，在同一机器预热后交替运行三轮；下表为进程墙钟中位数，单位秒。相同工作负载输出一致，未开启 stress。这是组合结果，不能将单项加速相加或归因给某一项优化；测量发生在后续 immortal 查询调整前，不包含该项收益。

| 用例 | 原产物 | M62 |
| --- | ---: | ---: |
| listIter | 2.6132 | 0.5345 |
| forEach | 2.6479 | 0.5347 |
| listIndex | 0.2465 | 0.1677 |
| mapFilter | 1.4070 | 0.3263 |
| hashLong | 0.9561 | 0.7731 |
| hashString | 0.3216 | 0.2548 |
| struct | 1.3071 | 0.1707 |
| trees | 0.4689 | 0.4555 |
| refStore | 0.4068 | 0.3267 |

struct 的约 7.7 倍改善来自修正 Double 临时值的复制成本：V3 add／scale 使用原阈值即可内联。ARM64 最终机器码确认循环中保留三次 fmul、三次 fadd 与 Managed poll，add／scale 调用消失；未引入浮点融合。独立五轮对照中，自动内联 0.170 秒、手工展开 0.170 秒、严格浮点 C 0.134 秒。trees 收益仍小；HFA ABI 与 nursery 默认值保持。最终文件的独立链接体积对照见实施记录。

immortal 查询的独立对照使用同一 Scoop 产物、单 worker、65536 个引用槽、601 次非移动 full；五轮交替中位数分别为全 immortal 0.629 → 0.219 秒、全堆对象 0.311 → 0.307 秒、半数 immortal 0.480 → 0.276 秒。每组输出、分配量、标记对象数和引用槽访问数一致；此结果只描述该标记负载。

## Linux 验证

使用独立目录 `nuc12:~/repos/scoop-m62`；原 `~/repos/scoop` 未参与本任务。记录与临时文件位于两端仓库外 `../scoop-work/m62`，不使用 `/tmp`。已完成 release workspace 5,170 项单元／集成测试覆盖和 2 项 doc tests；两处旧测试断言修正后重测通过。最终 statepoint 41 项、MIR lowering 158 项及 Python fixture runner 55 项通过。环境为 Ubuntu 26.04.1 amd64、Rust 1.99、系统 LLVM 22.1.2；这是 LLVM 22.1 开发环境验证，并非固定 LLVM 22.1.8 发行打包验收。

完整 GNU fixture 按用户要求提前结束。保留已执行结果并合并定向复测后，4,204 项中累计 2,217 项通过、33 项不适用、1,954 项未完成；没有遗留的已知失败。此前 15 个失败项均已正常补测通过：缓存摘要一项、去虚化／异常路径三项、归档成员列表十一项。修正分别涉及旧 cache 目录名、优化后的 MIR／external 编号与固定 `manifest.zst` 名称，未放宽语义或产物断言。

GNU 全部 26 个内置／consumer 测试 Cone 完成：1,339 项通过、零失败、零未运行；另外六项 readline PTY 子用例按设计在独立枚举中标记 ignored，已由通过的父测试在真实 PTY 内启动并检查结果。

musl 已完成 30 项定向 fixture，覆盖 intrinsic、内联、泛型产物消费与分配／多线程 GC；core consumer 34 项及 std 104 项 Cone 测试通过。immortal 查询调整分别通过 GNU／musl 各四种 runtime 配置验证，普通构建的两个扫描入口已无 immortal 查询引用，stress／显式验证构建保留查询。

最终 GC 源码同步后，Linux fmt／release workspace clippy 和 37 项 collector 回归通过，包含新增配置测试。GNU／musl 的普通与 stress runtime 均重新发布至独立开发 sysroot；两目标各四项正式 GC fixture、八个变体、40 个进程通过，覆盖分配统计、minor／moving 与多线程根。完整 fixture 和 Cone 测试期间的 runtime 增量由这组定向验证补齐，未重复全量执行。

三目标各十二项归档成员专测通过，确认固定 `manifest.zst` 名称及其他成员列表；本机与 GNU 的四项去虚化／局部值／异常路径专测也在同步 MIR 期望后通过。均为正常断言复测，不使用更新快照模式作为通过依据。

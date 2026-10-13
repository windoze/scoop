# M42 验收记录

状态：已完成。日期：2026-10-13。实施基线 `89da1b35`，验证代码包含 `0f712304`；之后的收尾提交只更新文档。

## 交付范围

M42-1～12 全部交付，公共行为见 [标准基础库规范](../specs/SCOOP-STDLIB-SPEC.md)，实现取舍见 [设计](DESIGN.md)，分批提交与诊断过程见 [进度记录](PROGRESS.md)。

- 普通 `scoop:scoop.std:0.1.0` Cone，仅直接依赖 core。124 个 Scoop 源文件、5218 行，最大文件 149 行；没有 std 专用 Extern／Intrinsic、native helper 或 runtime ABI。
- HashMap／HashSet、LinkedHashMap／LinkedHashSet、SortedMap／SortedSet、TreeMap／TreeSet、ArrayDeque、PriorityQueue、比较器、工厂、快照及集合算法。Tree 使用最小度数 8 的 B-tree，非根节点常态保存 7～15 个内联键值，TreeSet 复用树逻辑，不逐 entry 分配对象。
- 惰性 Sequence、短路与 once-only 来源、Result／Option 组合；文本处理、定宽整数与准确浮点解析、基础格式化；堆内字节构建／读取、大小端、Hex 和 Base64。
- 显式 Random、具名 seeded PRNG、无偏随机集合操作和 Uuid；没有隐式 RNG、系统熵或密码学安全承诺。
- Duration、Instant、Clock／TimeSource／TimeMark、LocalDate／LocalTime／LocalDateTime、UtcOffset／OffsetDateTime、YearMonth、Month／DayOfWeek、DatePeriod／DateTimePeriod。仅数据、运算、格式及来源接口，不提供系统取时、时区数据库或调度。
- 集合、Result、Uuid 和时间日期的显式 codec，与独立 JSON Cone 组合。三个业务 Cone 的 release provider／debug consumer／release application 经移除源码后独立链接运行通过。

普通 core 源码增加 Collection 接口和浮点位型转换方法；原 List／ArrayList／StringBuilder 行为、single-file 的 core-only 入口及显式 std 依赖保持。math（含 BigInteger／Decimal）、regex、完整 Unicode、压缩、密码学、更多格式、持久化集合和系统库仍为后续独立范围，没有加入占位实现。

## 三目标定向验证

以下是按 fixture 名去重后的实际通过覆盖。M42 三目标使用同一组 130 个 fixture；额外回归只来自被修改的 core API、比较展开、优化／GC 和最终链接路径。

| 目标 | M42 fixture／variant／process | 连同受影响既有回归的 fixture／variant／process |
| --- | --- | --- |
| aarch64-apple-darwin | 130／139／536 | 150／165／623 |
| x86_64-unknown-linux-gnu | 130／139／536 | 150／165／623 |
| x86_64-unknown-linux-musl | 130／139／536 | 149／163／615 |

GNU／Darwin 额外包含 C++ values 的 debug/release、普通／moving／minor GC 回归；musl 不支持 C++，不将它计为通过。其余 19 项既有回归覆盖 List／Array、StringBuilder、core sequence codec、泛型派生相等和 M31 的局部作用／SSA roots。M42 本组没有 IR 或链接计划 golden；既有底层专测保留其必要结构断言。

所有 M42 独立功能、精确 negative 诊断、组合、代表 debug/release、moving GC 和移除源码后的产物重链接均通过。PRNG 固定向量、独立生成的浮点／Duration／日历向量、B-tree 结构及模型对照在三个 target 上通过，没有未闭合的功能失败。

没有运行 `cargo test --workspace` 或全仓库 `run_fixtures.py --all`。每批按实际变更先格式化／lint，再执行对应模块测试、必要既有回归和 M42 定向套件。

## 底层缺陷与独立修复

| 提交 | 实际缺陷 | 必要回归 |
| --- | --- | --- |
| `61eb2a41` | 开放结构比较展开来源混用 | HIR equality 33 项、executable_type_sites 4 项，跨 Cone 来源与产物消费 |
| `ae0ab619` | 嵌套 enum 比较在 guard 外读取 payload | MIR 118 项，嵌套值、泛型 defaults、字段比较及 debug/release／moving GC |
| `ddfa354b` | LLVM 机器 tail duplication 复制 safepoint | codegen statepoint 28 项、三目标对象验证、独立 core 用例及 M31 回归 |
| `e6c06155` | LLVM 引用查找表缺少物理对象归属 | codegen object_partition 9 项、LIR foundation 45 项、production 184 项、slib relocation 6 项及 core-only release 用例 |
| `0f712304` | ELF 对象列表超过系统 argv 上限 | 旧实现独立复现；gcc／g++／musl-gcc 长列表、特殊字符路径、动态库、原失败用例和 C++ 回归 |

这些修复均先建立独立失败证据、修复并验证后再继续库工作。没有用库侧替代分支隐藏缺陷，也没有新增授权、配额或通用验证框架。Collection 本身属于普通 core API 补齐，不是新增底层集合特判。

## 报告与执行环境

本机报告保存在 `target/m42/`，按用例名核对的汇总为 `acceptance-summary.json`。Darwin 的 23 项 debug 工具通过记录在 `final-darwin-debug-report.json`，其余 107 项使用同一代码的 release 工具，见 `final-darwin-release-report.json`；Scoop 程序自身的 debug/release 变体没有减少。既有回归沿用实施阶段的通过结果；ELF 修复后另外复验 Darwin 三 Cone 应用和 C++ values。

Linux 在 `nuc12` 的独立检出执行，复用原仓库的构建目录和 LLVM 22／GNU／musl 工具。独立检出初次遗漏被 Git 忽略的 LLVM unwind 文件，补齐后重试受影响用例；随后实际暴露 argv 缺陷并独立修复。每次恢复只选择尚未通过的 fixture，已通过项保留结果，不将环境失败或主动中断计为通过。

Linux 分批报告已回收至 `target/m42/linux/`：`final-{gnu,musl}-{debug,environment,argv,fixed}-report.json` 与 `elf-args-{gnu,musl}-fixtures-report.json`。最后一批 GNU 124 项、musl 125 项全部通过，与先前通过项合并后精确覆盖上述清单。原失败与修复后的 linker 日志保存在 `target/m42/elf-args-gap/`；其他底层修复日志位置见进度记录。

构建、测试和临时文件均使用仓库内的 TMPDIR，没有使用 `/tmp`。完成后清理 fixture cases、一次性检查／传输文件及旧 incremental 缓存，保留具名报告、当前工具和可复用构建缓存。

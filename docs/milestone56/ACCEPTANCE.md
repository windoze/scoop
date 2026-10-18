# M56 最终验收

日期：2026-10-19。状态：本机组合回归、Linux workspace 与规则测试通过，Linux 完整 CLI 验收进行中。实现范围与候选边界见 [DESIGN.md](DESIGN.md)，逐项过程及本机性能数据见 [PROGRESS.md](PROGRESS.md)，桌面短暂停调研见 [DESKTOP-GC.md](DESKTOP-GC.md)。

## 1. 交付范围

已实现内部 TD 查询复用、拆箱／克隆单次对象定位、三个 String 有界表示 intrinsic、按函数 GC effect 选择的 LLVM 优化、普通字段写入的局部 card 查询复用，以及复用现有 worker pool 的并行引用回写。公开 native 输入、精确根、异常、原子和移动 GC 合同保持。默认 nursery 经容量／寿命矩阵比较后保留 1 MiB。

分块 poll、年轻 block 整块退役、metadata 缩减、局部装箱／同步协程包装消除、跨 Cone 内联和声明裁剪仍是设计或实验候选；没有作为生产默认启用。M56 不承诺任意存活堆下的 60 fps，也未引入增量／并发 collector。

## 2. 环境与正确性

- Darwin：Apple M4 Pro，macOS 27.0.1，LLVM 22.1.8，AArch64 generic target。
- Linux：nuc12，Intel i7-12700H，LLVM 22.1.2，amd64 GNU／musl。仅使用本任务新建的 worktree、target、scratch、sysroot 与缓存；第三方 native 构建及 Cargo 并行数限制为四。
- 两端显式准备 release 三个工具、16 个 Cone 和对应 runtime／native sysroot，正式 fixture 不隐式执行准备或发行打包。本机最终 sysroot 使用最终生产编译器重建。

本机逐功能先格式化／lint，再运行相关 crate、runtime 和 CLI 测试；范围与失败闭合见 PROGRESS。最终生产工具加重建 sysroot 的组合回归：M56 的 10 个 fixture／48 个变体全部通过；M26 String 的 9 个 fixture／9 个变体／21 个进程与 M33 UTF-8 的 6 个 fixture／9 个变体／35 个进程全部通过。这些 CLI 验收没有更新 golden。

Linux workspace 首轮的 HIR lowering 有 1452 项通过，23 份早期快照仅因 B3 的三个新增 core intrinsic 声明失败。对照确认用户 HIR 未变后，将期望收敛到被测声明、完整正文、入口与泛型实例，删除约 2500 行无关 core 重复内容；没有改生产 dump。格式化／clippy 与 23 项修正后的测试、String intrinsic 签名拒绝专测在两端全部通过。首轮已通过的 suite 不重跑，仅继续剩余 crate；单独补齐首轮尚未执行的文档测试。

Linux workspace 最终覆盖 **4975 项通过、0 失败、0 忽略**（含 2 项文档测试；重跑同一项不重复计数），fixture runner 的 **50 项规则测试通过**。driver 首轮未优化测试程序的 11 项已完成用例保留结果；其余 72 项使用 `CARGO_PROFILE_TEST_OPT_LEVEL=2`，同时显式保留 debug assertions 和 overflow checks，全部通过，随后补齐 7 项 CLI 参数单元测试和 workspace 文档测试。该配置仅加速 Rust 测试程序，不修改 Scoop 编译模式、输入或断言。

GNU／musl 完整 CLI 结果在完成后补录；性能在全部构建与正确性验证结束后串行运行。

## 3. Linux 性能复核

待记录实际结果。对照保持同一目标、输入、编译选项和结果校验；先预热，再交替运行七个独立进程。跨平台不直接比较绝对时间，GNU／musl 正确性与 GNU 性能分别报告。

## 4. 清理与限制

编译计时、旧优化管线和额外 GC 诊断仅用于实验。生产源码与工具恢复、任务临时目录／远端 worktree 清理及既有用户改动核对，在最终完成时记录。

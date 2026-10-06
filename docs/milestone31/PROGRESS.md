# M31 实施记录

日期：2026-10-07。基线：`6e62514da`（M30）。设计见 [DESIGN.md](DESIGN.md)。

## 实施顺序

1. 配置与基线：保存 M30 可复跑数据；贯通 debug/release、child 请求、producer 与缓存。
2. ODR 与 image：删除正文判等，按身份和 ABI 选择物理实现，迁移 metadata ABI 5。
3. 首批优化：局部传播、LLVM 函数内 pass、最终 GC 发射计划。
4. 分配与屏障：跨 line 普通分配、精确 size、引用范围写屏障。
5. nursery：年轻代 TLAB、minor、首次存活晋升、pin 与 full fallback。
6. 三目标验收及性能对照。

每项功能完成后单独提交；先格式化与 lint，再执行受影响的单元测试、正式 CLI fixture 和必要平台组合。复用未变化的通过结果，不逐批重跑全量。阶段快照保留实际类型、ABI、GC 与引用结构，不固定无关代码摘要。

## 当前状态

- 已读取设计与对应规范；工作区原有 M31 文档作为实施基准保存。
- Linux `nuc12:~/repos/scoop` 留有 M30 测试变更；Linux 验证将使用独立目录，保留原目录内容。
- 尚未完成 M31 功能；后续在此记录每批实际改动、版本与验证结果。

## 性能基线与构建清理

- 从 `6e62514da` 重建 release 配套命令，连同 M30 runtime/core 保存到 `/tmp/scoop-m31-baseline/`，可在后续变更后独立复跑。
- 新增六个普通 Scoop 性能程序与 100 行内的 CLI 测量脚本。Darwin 每项运行五次，核对确定输出，记录冷/热构建时间、执行时间、可执行文件和 `.slib` 大小；数据见 [M30-DARWIN.json](M30-DARWIN.json)。旧 runtime 没有完整 GC 统计，不填造分配/扫描/停顿数据。
- Python 按 Ruff 0.16.10 格式化与 lint；六项真实编译、链接及运行通过。用例准备阶段的 `gc.collect()` 改用现有公开测试入口 `gcCollect()` 后，相关四项重新执行，报告仅保留成功运行。
- 清理闲置 `target/debug/incremental` 与 M28/M29 专用目录约 11 GB；保留配套 release 命令与可复用依赖。
- Linux glibc/musl 各完成同一组六个程序、每项五次运行，记录见 [M30-LINUX-GNU.json](M30-LINUX-GNU.json) 与 [M30-LINUX-MUSL.json](M30-LINUX-MUSL.json)。Linux 工具来自原 M30 `9e4c411f2` 构建，runtime/core 源码来自 `6e62514da`；报告保留这两个 revision，后续比较使用相同环境。

## M31-1：实际优化配置

- 增加封闭的 `OptimizationMode::{Debug, Release}`，沿 umbrella、graph、child 协议、普通 `scoopc` 请求和 producer 贯通。Scoop machine 使用 O0/O2，generated-C 使用 O0/O2；runtime 保持现有 O2。LLVM 普通 IR pass 与最终 GC 计划尚属 M31-3。
- child protocol 升为 4；single-cone production capability 升为 4，manifest field 12 保存模式并进入 Code/Artifact 内容摘要；compile-cache domain 升为 `scoop-cone-compile-cache-v2`。优化模式不进入语言身份或共享 ABI。
- LLVM O2 实际产生 Mach-O `LC_LINKER_OPTIMIZATION_HINT`；reader 接受此标准命令并验证长度、文件范围及与已占用数据的非重叠，新增合法／越界／重叠测试。将 request 解码与 Mach-O 表读取按职责拆成子模块。
- Darwin 正式 profile 用例的两个变体通过，覆盖 C bridge、冷／热构建、普通与 child 产物一致、release full-moving GC；旧 CLI 用例与四份阶段快照通过。
- `nuc12:~/repos/scoop-m31` 使用 LLVM 22.1.2 构建，glibc/musl 同一 profile 用例各两个变体、12 个进程均通过。原 `~/repos/scoop` 源码未修改；复用其 target 与 native sysroot。
- 格式化及 workspace clippy 通过；定向 protocol 24、C bridge 16、manifest production 6、cache key 5、scoopc CLI 7、Mach-O optimization hint 1 项通过。后续批次复用这些结果，不重复全量测试。

# M58 实施与验收记录

状态：实施中。分支 `codex/m58`，主仓库 `/Users/chenxu/repos/scoop`；保留工作区中其他任务的 M47／M57 未提交内容，不纳入 M58 提交。按 [DESIGN.md](DESIGN.md) 完成全部 A–D 后再做整体验收；当前结果不代表里程碑完成。

## 分项状态

| 功能 | 当前状态与证据 |
| --- | --- |
| A：年轻 small block 整块退役 | 已实现；本机 GC 专测 32 项、新源码 fixture 四个变体通过，Linux 待验证 |
| A：无引用大对象跳过 remembered scan | 已实现；本机 nursery 专测 10 项、新源码 fixture 三个变体通过，Linux 待验证 |
| A：full 必要容量早判 | 已实现；本机 region 专测 7 项、新源码 fixture 三个变体通过，Linux 待验证 |
| A：starts bitmap 枚举 | 待实施 |
| B：统一优化迭代、函数内来源／逃逸分析、box／closure 消除与支持产物清理 | 待实施 |
| C：tail completion 转交、状态机生成前同步内联 | 待实施 |
| D：外层 poll 循环分块、受限内层 SIMD | 待实施 |
| 性能与跨平台 | 生产前后性能矩阵、GNU／musl 完整验收及最终清理待完成；历史原型数字不替代新结果 |

## 2026-10-20：年轻 block 退役

先补 runtime 规范 3.9，再在收尾阶段为普通 minor 的全死／已搬空年轻 small block 使用现有 block 退役流程。先统计晋升量；源存储已完成回写与验证才释放 forwarding，复用依赖现有 activation 清空元数据。含 pin 的存活 block、evacuation target、release-hook 旧对象和 moving stress 保持原路径。

新增 C 专测反复检查全死块与混合存活源块的退役／复用、旧起点不可见、晋升计数和链内容；新增正式源码 fixture 以持续分配和 old→young 链覆盖 debug／release 及普通／stress minor。运行参数显式选择当前 runtime 源码，避免误测旧 sysroot runtime。

本机验证：C 格式化与 `-Wall -Wextra -Werror` 语法检查、`cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；`scoop-codegen` 的 GC 专测 32 项通过，包含 pin／handle、release、移动 stress、并行回写、晋升失败和线程握手；新 fixture 1 项／4 变体／12 进程通过。最初 fixture 使用 Option 结构比较触发 Node 缺少 equals 的诊断，已改为模式匹配并闭合。

构建与原始验证输出位于仓库外 `/Users/chenxu/repos/scoop-work/m58-validation`；临时目录显式使用仓库的 `tmp/m58/tmp`，不使用 `/tmp`。开始时定向 `cargo clean -p scoop-codegen` 释放约 1.4 GiB，保留可复用依赖。后续继续按实际磁盘占用清理，Linux 尚未开始。

## 2026-10-20：跳过无引用大对象的卡表

在 shared remembered 枚举入口读取已发布旧 large head 的 TD；没有 object scan 时不遍历其卡表。标记与回写共用此路径，卡表分配／屏障／清零和对象本身的保活规则保持原样。

新增 C 专测将 1 MiB 无引用对象全部标脏，与真正含引用的 large array 同轮收集；四轮均只计入两个必要脏卡、128 个旧引用槽，两个年轻对象正常搬迁、内容和晋升统计保持。正式源码 fixture 组合大值数组的 scoped pointer borrow、native 范围屏障和旧引用数组，使用当前 runtime 源码。

格式化、C 严格语法检查、workspace clippy 通过；nursery 专测 10 项、新 fixture 1 项／3 变体／9 进程通过。fixture 的回调补上显式 unsafe 作用域；无引用存储使用 Array 的 scoped borrow，避免当前显式 MutableArray 借用重载解析到 Array 的无关诊断。没有修改语言或重载行为，目标运行时路径由 C 专测和源码闭环共同覆盖。性能测量留到 GC 改动完成后统一对照。

## 2026-10-20：full 容量早判

在普通 full 的单 source 情况下，使用现有 region 存活统计与普通 block 容量，计算已占用目标 region 的可用容量上界。明显不足时恢复 source 状态，直接做非移动收尾；通过时仍使用实际预留／回滚。没有新增堆列表、对象扫描、公开参数或 ABI。

C 专测在真实 STW、mark 与 plan 边界检查：两个 block 的 source 无法放入仅一个 block 的容量时，目标死亡对象仍未 release、起点仍存在；正常收尾只 release 一次。额外释放两个目标 block 后，同一 source 的两个对象成功搬迁，引用和内容保持。正式源码 fixture 用 55 万个含引用节点、共享尾节点和大引用数组，覆盖密集全存活、稀疏回收及再次 full 的图正确性。

C 格式化／严格语法检查、Rust fmt／workspace clippy 通过；region 专测 7 项通过，包含失败回滚、pin、discard、region 压缩；源码 fixture 1 项／3 变体／3 进程通过，覆盖 debug、release 与四 worker。此处确认正确性和提前退出路径，尚未给出暂停加速比。清理约 16 GiB 的 `target/debug/incremental`，保留依赖与正式 CLI 工具。

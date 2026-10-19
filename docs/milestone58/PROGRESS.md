# M58 实施与验收记录

状态：实施中。分支 `codex/m58`，主仓库 `/Users/chenxu/repos/scoop`；保留工作区中其他任务的 M47／M57 未提交内容，不纳入 M58 提交。按 [DESIGN.md](DESIGN.md) 完成全部 A–D 后再做整体验收；当前结果不代表里程碑完成。

## 分项状态

| 功能 | 当前状态与证据 |
| --- | --- |
| A：年轻 small block 整块退役 | 已实现；本机 GC 专测 32 项、新源码 fixture 四个变体通过，Linux 待验证 |
| A：无引用大对象跳过 remembered scan | 待实施 |
| A：full 必要容量早判、starts bitmap 枚举 | 待实施，各自提交和验证 |
| B：统一优化迭代、函数内来源／逃逸分析、box／closure 消除与支持产物清理 | 待实施 |
| C：tail completion 转交、状态机生成前同步内联 | 待实施 |
| D：外层 poll 循环分块、受限内层 SIMD | 待实施 |
| 性能与跨平台 | 生产前后性能矩阵、GNU／musl 完整验收及最终清理待完成；历史原型数字不替代新结果 |

## 2026-10-20：年轻 block 退役

先补 runtime 规范 3.9，再在收尾阶段为普通 minor 的全死／已搬空年轻 small block 使用现有 block 退役流程。先统计晋升量；源存储已完成回写与验证才释放 forwarding，复用依赖现有 activation 清空元数据。含 pin 的存活 block、evacuation target、release-hook 旧对象和 moving stress 保持原路径。

新增 C 专测反复检查全死块与混合存活源块的退役／复用、旧起点不可见、晋升计数和链内容；新增正式源码 fixture 以持续分配和 old→young 链覆盖 debug／release 及普通／stress minor。运行参数显式选择当前 runtime 源码，避免误测旧 sysroot runtime。

本机验证：C 格式化与 `-Wall -Wextra -Werror` 语法检查、`cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；`scoop-codegen` 的 GC 专测 32 项通过，包含 pin／handle、release、移动 stress、并行回写、晋升失败和线程握手；新 fixture 1 项／4 变体／12 进程通过。最初 fixture 使用 Option 结构比较触发 Node 缺少 equals 的诊断，已改为模式匹配并闭合。

构建与原始验证输出位于仓库外 `/Users/chenxu/repos/scoop-work/m58-validation`；临时目录显式使用仓库的 `tmp/m58/tmp`，不使用 `/tmp`。开始时定向 `cargo clean -p scoop-codegen` 释放约 1.4 GiB，保留可复用依赖。后续继续按实际磁盘占用清理，Linux 尚未开始。

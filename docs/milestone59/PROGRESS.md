# M59 实施记录

状态：实施中。依据：[设计](DESIGN.md)、[终端库规范](../specs/SCOOP-TERMINAL-SPEC.md)。按功能完成本机验证后提交，最后进行 Linux 三目标范围内的完整验收；Windows 不在本期实现范围。

## 1. 功能进度

| 功能 | 状态 | 验证 |
| --- | --- | --- |
| 公共合同与普通 Cone | 规范已建立，开始实现 | 文档链接与差异检查 |
| 命令／光标／屏幕 | 实施中 | 待本机纯编码、真实缓冲输出和独立消费者验证 |
| 样式与颜色策略 | 待实施 | — |
| 事件数据与增量 parser | 待实施 | — |
| Unix 会话、raw、resize、read／poll、查询 | 待实施 | — |
| PTY host 与交互 fixture | 待实施 | — |
| Kitty／OSC 52 与组合覆盖 | 待实施 | — |
| asyncio TTY 与 terminal.asyncio | 待实施 | — |
| Linux GNU／musl 验收与清理 | 待实施 | — |

## 2. 工作约定与环境

- 工作分支 `codex/m59`，起点 `2f7411e5`；已有 M47／M57 文档修改保持独立。
- 本机 LLVM 22.1.8；Linux 可通过 `ssh nuc12` 的 `~/repos/scoop` 使用，开始时 checkout 为旧的 `e548b9ebe` 且干净，功能本机通过后再同步。
- 临时源码、构建缓存与工具 scratch 使用仓库忽略的 `tmp/m59`；原始日志位于仓库外，不写入绝对路径 `/tmp`。
- 开始时用 Cargo 清理旧 dev 产物（21.7 GiB），构建配套命令时关闭 Rust incremental，后续按功能检查与清理不用的 target。
- 本期优先执行新库和相关 fixture；涉及底层变更时再增加对应回归，不逐项重复整个编译器全量测试。

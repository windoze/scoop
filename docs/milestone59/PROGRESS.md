# M59 实施记录

状态：实施中。依据：[设计](DESIGN.md)、[终端库规范](../specs/SCOOP-TERMINAL-SPEC.md)。按功能完成本机验证后提交，最后进行 Linux 三目标范围内的完整验收；Windows 不在本期实现范围。

## 1. 功能进度

| 功能 | 状态 | 验证 |
| --- | --- | --- |
| 公共合同与普通 Cone | 已建立；后续功能逐项补规范 | 文档检查，开发 sysroot 已发布新 Cone |
| 命令／光标／屏幕 | 本机完成，PTY／Linux 随后验收 | 14 项单测 debug 与 release＋moving GC 均通过；3 项消费者 fixture、6 个变体通过 |
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

## 3. 首批输出功能

实现普通 Command、TextWriter／ByteSink 的 queue、ByteSink.execute、Print，以及光标移动／保存／形状和屏幕清除／滚动／模式／标题命令。文件按职责拆分，生产文件最大 55 行；未引入 compiler 或 runtime 特判。

本机验证包括纯编码、零基／一基及最大坐标转换、短写、无进展、错误传播、flush 次数、真实 BufferedSink、Unicode 和非法参数。三个独立 CLI fixture 消费预编译 `.slib`，分别覆盖光标、屏幕及自定义 Command／缓冲输出／GC 的组合；均在 debug、release 下启用 moving GC，完整原始字节匹配。Rust fmt 检查及 workspace Clippy 通过。

首轮测试中的混合值类型数组改用规范要求的显式接口装箱，并去掉普通数组字面量不接受的尾逗号；未改动语言语义。保存／恢复光标核对上游后采用 `ESC 7/8`。这批结果只证明输出编码与流组合，实际屏幕效果仍由后续 PTY 用例覆盖。

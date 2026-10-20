# M60 实施记录

状态：实施中。开始日期：2026-10-21。基线：`abf17c32`；分支：`codex/m60`。范围与完成条件见 [DESIGN.md](DESIGN.md)。

## 分项状态

| 分项 | 状态与验证 |
| --- | --- |
| 设计与路线图 | 已提交；已核对本地引用、M52 当前状态及源码布局 |
| M60-A：文档前端与注解 | 文档注释、Markdown／tag 语法及普通 core 注解已完成；typed HIR 投影待接入 |
| M60-B：单 Cone 闭环 | 待实施 |
| M60-C：多 Cone 与更新复用 | 待实施 |
| M60-D：实际库及跨平台验收 | 待实施 |

## 工作与验证约定

临时文件、测试 TMPDIR 和原始日志放在仓库外的本次工作目录，不使用 `/tmp`。按功能先格式化／lint，再做本机定向验证并提交；Linux 使用 nuc12 的 `~/repos/scoop`，在本机功能闭合后完成全量验收。开始时本机 target 约 3.4 GB；阶段间检查占用并清理不再使用的构建结果，保留仍用于验证的配套工具和 sysroot。

## A1：文档注释保留与附着

同一 lexer 保存块／行文档的原始文本和 byte span，普通注释与字符串保持原行为；parser 在实际声明、参数、variant、构造器及 accessor 语法入口建立精确关联，悬空片段保留为后续文档诊断输入。AST 的关联是文件内语法位置，后续 HIR 仍须绑定真实 typed 声明。长主构造器解析移入现有 constructors 模块。

组合测试发现无 body 的 class 会误吞下一声明的可见性／注解；修正换行处主构造器前缀的解析边界，保留跨行显式 constructor，增加独立回归。

本机 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；AST 15 项、parser 520 项及 parser doctest 1 项通过，其中新增 10 项文档独立／组合测试。正式 `scoop doc`、文档 warning、HIR 投影及 Linux 验收尚未完成，不从 parser 结果推定完成。

## A2：普通 core 文档注解

新增 `DocHidden` 和可重复的 `DocAlias(String)`，复用普通注解的 Target、静态参数、use-site、alias、re-export 与产物保存。Intrinsic 类型及函数允许实际 core 文档注解；其他普通注解仍被拒绝，原有 NoGC／Unsafe 要求不变。为真实 Int、String 和 currentSourceLocation 添加代表性说明与搜索别名。

本机 fmt／workspace clippy 通过；47 项注解相关 HIR 测试通过（包含新增 6 项），11 个三目标通用 CLI fixture 在 Darwin 通过，共 14 个进程。用例检查错误消息与准确 byte span，并实际编译／运行普通注解、同名用户注解及跨 Cone 重导出。配套 release 命令已重建，开发 sysroot 的 19 个库与 runtime 已按维护脚本完整更新。Linux 验收留到功能闭合后进行。

清理约 729 MiB 本阶段不再使用的 MIR／LIR／codegen 增量缓存，保留当前前端测试及配套命令。原始日志与 fixture 工作区保存在仓库外。

## A3：Markdown、tag 与原始位置

新增 `scoop-doc` 文档 stage；正文、tag、待绑定路径、跨 tag 的显式 Markdown 引用定义及 byte 映射使用 AST 纯数据。仅消费 lexer／parser 保留的片段，不重新发现声明。pulldown-cmark 负责 Markdown 结构，代码、图片和显式 URL 不成为符号候选；反引号标识符中的点与空格保持原意。未知／缺参 tag、非法符号语法和悬空片段产生带原始位置的文档 warning，语义目标仍留给 HIR 绑定。

引入前已核对 pulldown-cmark 0.13.4 的维护状态及 Rust 1.71.1 要求，低于 workspace 的 1.85 基线；仅启用实际需要的 HTML 功能。MiniJinja 3.0.1 的 Rust 1.70 要求也已核对，待渲染阶段实际引入。

本机 fmt／workspace clippy 通过；14 项文档语法测试与 15 项 AST 测试通过，覆盖 Unicode／CRLF、装饰前缀／缩进、连续与分隔片段、8 类 tag、代码／HTML 边界、显式引用优先、quoted path、精确 warning span。此阶段尚未提供 `scoop doc` CLI，正式文档 warning fixture 随 CLI 闭环接入。

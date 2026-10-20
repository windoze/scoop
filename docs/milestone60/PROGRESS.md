# M60 实施记录

状态：实施中。开始日期：2026-10-21。基线：`abf17c32`；分支：`codex/m60`。范围与完成条件见 [DESIGN.md](DESIGN.md)。

## 分项状态

| 分项 | 状态与验证 |
| --- | --- |
| 设计与路线图 | 已提交；已核对本地引用、M52 当前状态及源码布局 |
| M60-A：文档前端与注解 | 词法保留、语法附着及普通 core 文档注解已完成；文档语法与 typed HIR 投影待接入 |
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

本机 fmt／workspace clippy 通过；47 项注解相关 HIR 测试通过（包含新增 6 项），11 个三目标通用 CLI fixture 在 Darwin 通过，共 14 个进程。用例检查错误消息与准确 byte span，并实际编译／运行普通注解、同名用户注解及跨 Cone 重导出。配套 release 命令已重建，core 已按维护脚本更新；其他开发 sysroot 库继续由同一次显式准备更新。Linux 验收留到功能闭合后进行。

清理约 729 MiB 本阶段不再使用的 MIR／LIR／codegen 增量缓存，保留当前前端测试及配套命令。原始日志与 fixture 工作区保存在仓库外。

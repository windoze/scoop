# 公共语言服务基础实施记录

开始日期：2026-10-22。实施基线：`033dd9e1d`。第一分支：`codex/language-service-foundation`。

按[设计](DESIGN.md)完整交付 A～E 后，才从第一分支的完成提交创建独立 M61 分支，实施[文本后端迁移](../milestone61/DESIGN.md)。每个完成的功能经过对应格式化、lint 和验证后独立提交；当前记录不缩减任一阶段的原完成条件。

## 交付状态

| 范围 | 状态与剩余工作 |
| --- | --- |
| A：共同源码／固定查询 | 已完成源码、坐标与诊断提取；完整片段入口、共享语义上下文与实际 generator 闭环仍待实现 |
| B：可恢复语法 | 未开始：保留原文的 CST、缺失／错误项、局部恢复与完整 AST 投影 |
| C：首个 LSP | 未开始：stdio、文档同步、位置编码、语法诊断和结构查询 |
| D：编辑态语义 | 未开始：独立声明／正文分析、源码绑定、hover／定义／补全／签名帮助 |
| E：索引与编辑 | 未开始：工作区引用与实现、重命名／修复、版本失效与取消 |
| 公共基础验收 | 未开始：按功能本机验证，随后 Linux 完整验证；完成后核对设计各项条件 |
| 第二分支与 M61 | 未开始：保留其全部字符集、IDNA、NFC、ABI、三目标及分发要求 |

## 已完成的功能

- 共同源码：`scoop-source` 提供共享不可变文本、按需行索引、UTF-8／16／32 坐标和诊断；AST 重导出保持调用兼容，当前 Cone 与 driver 诊断报告共享同一份文本。parser 在 lex 前拒绝无法由 Span 表示的长度。CLI 原有 LF／一基标量列保持，编辑器使用 LF／CRLF／CR 与编码单位列。
- 本机验证：`cargo fmt --all`、全 workspace／all-targets Clippy（拒绝 warnings）通过；source／AST／parser 的 548 项测试及 driver 诊断报告的 4 项定向测试通过。覆盖 Unicode、换行、EOF、无效字符边界、文本共享以及既有诊断显示兼容。LLVM 明确使用 Homebrew 22.1.8。

## 环境与验证策略

工作文件和原始日志位于仓库外的 `/Volumes/Data/home/chenxu/work/scoop-language-service/`；命令显式设置该目录内的 scratch 为 TMPDIR，不使用 `/tmp`。保留源码、工具和普通缓存，按真实输入变化与调用结果验证更新。Linux 环境为 `ssh nuc12` 的 `~/repos/scoop`，本机功能完成后再同步和验证。

初始 target 约 105 GiB，其中 debug incremental 约 49 GiB；无活动 Cargo 构建时使用 Cargo 清理命令回收旧开发产物。后续开发检查关闭增量并减少调试信息，保留普通 release 工具与实际复用缓存；定期在构建空闲时检查空间。格式化和 lint 先于测试；Rust 编译器测试与正式 CLI／LSP 进程 fixture 按职责选择，普通库行为遵守当前 AGENTS.md 的 Cone 测试规则。

# fmt／check 验证记录

日期：2026-10-26。本机 macOS/AArch64，LLVM 22.1。

## check

- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。
- protocol 全部 39 项测试、低层 check 与文档 HIR 回归、公共 check 参数解析、18 项图准备／并行调度测试通过。
- native 工具记录与对象更新的 5 项定向测试通过；fixture runner 的 Ruff 与公共规则测试通过。
- `tests/fixtures/driver/check/` 的 12 项 fixture（含 generator 错误变体）通过：库、可执行文件、单文件、默认输入、direct 输入、Production/Test 分离、生成测试入口、测试依赖隔离、Ignore 正文、普通／生成片段 warning、语法／名称／effect／具体化条件／递归诊断、依赖机器产物、native 头文件更新、generator 更新、同键并发及失败后修复。
- 受影响的 `doc-cli-basic`、`doc-cli-multiple`（含变体）通过。

未优化的完整 HIR Rust 测试使用 `RUST_MIN_STACK=33554432`；源码行为 fixture 使用 release。源码、配套工具、sysroot 和既有产物始终保留；通过实际调用观察和输入变化检查复用。

## 格式库

- workspace 格式化和 Clippy 通过；6 组定向测试覆盖格式输出、操作符／控制体、注释／字符串／CRLF、Unicode 标量行宽和敏感换行。
- 7,546 份可成功解析的仓库源码通过非空白 token、完整位置归一化 AST、文档注释目标和幂等性检查；有意损坏的 UTF-8 或语法输入不属于格式库成功输入。
- `cargo test -p scoop-parser -p scoop-analysis -p scoop-lsp` 全部通过，覆盖新增 CST 角色及其现有消费者。

## 格式化 CLI

- workspace 格式化／Clippy、27 项源码选择测试、公共 CLI 参数及文件写回测试通过；覆盖普通可执行权限、符号链接与第二次写回不改 mtime。有效非 UTF-8 文件名的写回用例在 Linux 验证，APFS 不接受该文件名。
- fixture runner 的 Ruff 和公共规则测试通过；8 项格式化 fixture、15 次进程调用通过，覆盖所有平台的声明并集、重复选择、默认与显式输入、空集合、只检查、语法／UTF-8 错误聚合、整个 root 选择失败、目录循环、实际写入失败及退出码 2。
- `cli-external-command`、`cli-run-process-contract` 共 21 次进程调用通过，覆盖命令结果分发、运行参数、退出码、信号及中断。

## 尚待验收

Linux 完整 workspace 测试及 GNU／musl 文件 fixture 验收。原始日志和工作目录保存在仓库外，不提交逐次报告。

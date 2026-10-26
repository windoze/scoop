# fmt／check 验证记录

日期：2026-10-26。本机 macOS/AArch64，LLVM 22.1。

## check

- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。
- protocol 全部 39 项测试、低层 check 与文档 HIR 回归、公共 check 参数解析、18 项图准备／并行调度测试通过。
- native 工具记录与对象更新的 5 项定向测试通过；fixture runner 的 Ruff 与公共规则测试通过。
- `tests/fixtures/driver/check/` 的 12 项 fixture（含 generator 错误变体）通过：库、可执行文件、单文件、默认输入、direct 输入、Production/Test 分离、生成测试入口、测试依赖隔离、Ignore 正文、普通／生成片段 warning、语法／名称／effect／具体化条件／递归诊断、依赖机器产物、native 头文件更新、generator 更新、同键并发及失败后修复。
- 受影响的 `doc-cli-basic`、`doc-cli-multiple`（含变体）通过。

未优化的完整 HIR Rust 测试使用 `RUST_MIN_STACK=33554432`；源码行为 fixture 使用 release。源码、配套工具、sysroot 和既有产物始终保留；通过实际调用观察和输入变化检查复用。

## 尚待验收

格式库、格式化 CLI，以及两项功能完成后的 Linux 完整测试与文件 fixture 验收。原始日志和工作目录保存在仓库外，不提交逐次报告。

# M41 实施记录

基线：`9e715a3c`，M39 的验收、提交及清理已完成。分支为 `codex/m41`。

## 当前状态

- 已读取设计、AGENTS.md 及三份权威规范，开始按功能细化合同。
- 第一批规范明确 Add 的完整 operand/result 签名、11 种 core 类型的显式实现、普通扩展与名义上界的区别，以及既有 NoGC/Managed slot 适配。
- IEEE 四态 API 定名为 IeeeComparison / ieeeCompare；Equals 迁移、其余接口与浮点语义实施尚未完成。
- 临时目录和日志使用 `target/m41/`，测试命令设置 TMPDIR；保留 M39 编译依赖以避免重复构建。

## 验证安排

每批先 fmt、workspace clippy，再针对改动执行 Rust 与正式 CLI fixture；每个完成的功能单独提交。
最后在 nuc12 的 `~/repos/scoop` 执行 Linux 完整适用范围验收，按成功完成状态清理 fixture 工作目录。
尚未执行的测试不计为通过，不使用批量快照更新代替诊断核对。

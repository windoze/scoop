# M41 实施记录

基线：`9e715a3c`，M39 的验收、提交及清理已完成。分支为 `codex/m41`。

## 当前状态

- 已读取设计、AGENTS.md 及三份权威规范，按功能细化合同。
- Add 已实现：八种整数、Float、Double、String 显式提供 `Add<Owner, Owner>`；具体化后的已知运算保持 intrinsic，实际接口调用使用既有装箱与 NoGC/Managed slot 适配。
- LocalConcrete intrinsic 保存完整物理参数类型；实际分派入口复用 callable materialization 和 primitive lowering，生产、导入两侧按声明的 slot/dispatch selection 发布与选择入口，没有新增 runtime ABI 或产物 wire tag。
- 跨库扩展正文的隐式 struct 字段读取复用字段身份，避免选择没有函数体的 Storage getter。
- IEEE 四态 API 定名为 IeeeComparison / ieeeCompare；Equals 迁移、其余接口与浮点语义实施尚未完成。
- 临时目录和日志使用 `target/m41/`，测试命令设置 TMPDIR；保留 M39 编译依赖以避免重复构建。

## 验证安排

每批先 fmt、workspace clippy，再针对改动执行 Rust 与正式 CLI fixture；每个完成的功能单独提交。
最后在 nuc12 的 `~/repos/scoop` 执行 Linux 完整适用范围验收，按成功完成状态清理 fixture 工作目录。
尚未执行的测试不计为通过，不使用批量快照更新代替诊断核对。

## Add 本机验证

- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；三个正式 CLI 的 release binary 构建通过。
- 新增 typed IR 测试 2 项通过，覆盖 11 个 intrinsic 的完整签名、GC effect、可发布入口及具体 Int bound 调用保留直接运算。
- HIR callable record 16 项、nominal callable classifier 10 项、原 intrinsic 测试 8 项、MIR lowering 118 项通过。
- `tests/fixtures/m41-add` 全部 11 项通过：15 个变体、43 次进程执行。包括 debug/release、异构结果、多 bound、alias、扩展边界、仅产物链接及正常/moving/minor GC 模式；7 个负例逐项核对位置与诊断。
- fixture runner 公共规则 47 项通过。
- core bootstrap 和跨库 shape 产物结构测试各 1 项通过；既有 M33 接口效果回归 3 项通过（5 个变体、21 次进程执行）。
- 本批定期删除早期失败 fixture 的 cases/cache/sysroot 与诊断采集工作目录，保留报告和日志；未使用 `/tmp`。

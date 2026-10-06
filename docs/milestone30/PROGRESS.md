# M30 实施记录

目标与验收矩阵见 [设计](DESIGN.md)。按独立功能完成实现、针对性验证并提交，最后执行正式整体验收；未经实际运行的项目不记作通过。

## 实施计划

- A：Float/Double 标量、literal、alias、完整 IR/meta/ABI 与最短字符串化。
- B：算术、IEEE 比较、分类、totalOrder、显式转换与 const。
- C：候选隔离与直接舍入、默认值、annotation、递归 pattern、泛型及派生相等。
- D：值存储、GC/协程与 C/Scoop FFI、fmod native support。
- E：single-value 协议、companion codec 与 JSON。
- F：跨 Cone/ODR、artifact-only、Darwin 与 Linux glibc/musl 验收。

## 2026-10-06：开始实施

以 `93ae1e671`（M29 完成）为代码基线，保留并采用工作区已有的 M30 设计、调研及三份规范修订，分支为 `codex/m30`。

本机 `target` 初始约 31 GiB，其中旧 debug incremental 约 16 GiB；确认没有活动构建后清理该 incremental 目录，保留现有编译依赖与历史验收资料。开发期优先受影响 crate 和选定文件 fixture，不反复运行全量测试。Linux 验证使用 `nuc12:~/repos/scoop`。

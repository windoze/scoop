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

## A：浮点标量闭环

实现 Float/Double 的真实 intrinsic nominal、透明 alias、未舍入十进制 AST、目标精度 literal 与负零，贯通 HIR/MIR/LIR、持久化记录、标量 ABI、C storage 及 LLVM `float`/`double` 常量。以固定版本 `rustc_apfloat 0.2.3` 直接解析目标精度，以 Ryu 固定 commit 提供最短字符串化；新增自有模块按职责拆分，第三方源码保留原样。

实际 CLI 暴露并补齐了 intrinsic struct application、具体化以及 LLVM 局部常量池的对象记录。常量池沿真实重定位保存字节并进入已有对象摘要，不增加源码实体或来源认证。受影响 section 及兼容版本见实现规范 §2.18。

Darwin/AArch64 验证：

- `cargo fmt --all` 与 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22 cargo clippy --workspace --all-targets` 通过。
- 三个配套 CLI 的 release 构建通过。
- 选定单元测试通过：parser 浮点语法 3 项、identity 浮点 wire 2 项、slib profile 19 项、Mach-O section 7 项、HIR persistent function 3 项。
- `m30-floating` 的 10 个正式文件 fixture 通过，包括普通／移动 GC 运行、四阶段 golden 和 9 个精确诊断负例。直接舍入反例得到 F32 `0x4b800001`，正负零、最小 subnormal 和下溢均经实际执行验证。
- Char 基础组合回归通过普通／移动 GC 与四阶段 golden；HIR golden 仅同步新增 core 声明导致的临时 imported identity index 变化，已确认其余内容相同。

本批尚不声明算术、const 求值、pattern、完整 FFI 或 JSON 已完成；它们继续按 B～F 实施。未运行全量测试，Linux 留待后续组合与最终验收。

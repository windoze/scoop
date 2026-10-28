# 原生调试实施记录

分支：`codex/native-debugging`。开始日期：2026-10-29。

## 范围与顺序

本轮实现 DESIGN.md 的基础源码调试、静态变量和物理栈、LLDB 值展示、VS Code 集成、Throwable 历史栈快照和 trap 栈打印，以及发行打包。完整 Scoop 表达式求值和写入 managed 变量留在后续独立阶段。协程逻辑栈按现有 continuation 信息与 LLDB frame provider 的真实能力评估。

1. 更新编译职责、调试级别、独立文件、缓存和发布规范。
2. 贯通源码位置与类型／变量描述，发射标准 DWARF，维护优化和 GC 后的位置。
3. 完成平台调试文件、CLI／缓存／发布，迁移性能符号化与 map 消费者。
4. 完成 Throwable 首次抛出快照、保留原栈、printStackTrace、未捕获异常和 trap 诊断，验证 GC 与异常传播。
5. 完成 LLDB formatter、VS Code 构建和调试入口，评估协程栈。
6. 更新发行脚本，打包 LLDB、lldb-dap、Python 支持及必要依赖。
7. 本地定向验收完成后，在 nuc12 的 `~/repos` 新目录验证 Linux GNU／musl 并闭合问题。

每个可验证的变更完成后提交并推送；随后 fetch 并合并远端 main 的新增提交。原始日志、临时程序和报告存放仓库外。

## 当前状态

已建立分支并提交设计基线，保留原工作区中的相关规范变更。用户进一步确认本轮包含 Throwable 历史栈快照及 trap 栈打印。

独立调试文件的底层生成／发布已完成：linker 可显式生成 dSYM 或 ELF `.debug`；最终 basename 进入调试结果更新键，伴随文件参与结果存在判断和输出发布，缓存命中不启动调试工具。Darwin 在 dsymutil 后裁剪并重新签名，目录包支持替换发布。map 通路暂时保留，待源码调试与性能消费者迁移后删除。

本地已通过 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，以及调试文件生成与 CLI 参数的两项定向测试。生成测试使用保留源码和对象的真实 C 程序，覆盖符号保留／裁剪、含空格 basename、行表及 Darwin 最终签名。此结果尚不代表 Scoop 源码调试已完成；Linux 分支仅通过编译检查，按约定待本地全部功能完成后在 nuc12 执行。

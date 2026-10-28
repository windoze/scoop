# 原生调试实施记录

分支：`codex/native-debugging`。开始日期：2026-10-29。

## 范围与顺序

本轮实现 DESIGN.md 的基础源码调试、静态变量和物理栈、LLDB 值展示、VS Code 集成，以及发行打包。完整 Scoop 表达式求值、写入 managed 变量和 Throwable 历史栈快照留在后续独立阶段。协程逻辑栈按现有 continuation 信息与 LLDB frame provider 的真实能力评估。

1. 更新编译职责、调试级别、独立文件、缓存和发布规范。
2. 贯通源码位置与类型／变量描述，发射标准 DWARF，维护优化和 GC 后的位置。
3. 完成平台调试文件、CLI／缓存／发布，迁移性能符号化与 map 消费者。
4. 完成 LLDB formatter、VS Code 构建和调试入口，评估协程栈。
5. 更新发行脚本，打包 LLDB、lldb-dap、Python 支持及必要依赖。
6. 本地定向验收完成后，在 nuc12 的 `~/repos` 新目录验证 Linux GNU／musl 并闭合问题。

每个可验证的变更完成后提交并推送；随后 fetch 并合并远端 main 的新增提交。原始日志、临时程序和报告存放仓库外。

## 当前状态

已建立分支并保留工作区中的调试设计和相关规范变更。尚未声明任何新增调试能力通过验收。

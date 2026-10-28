# 编译期 intrinsic 与包限定表达式实施记录

目标：完整实现本目录及 `../qualified-expressions/DESIGN.md` 的设计，按功能完成验证并提交。当前处于实施中，不能将单项完成视为整组能力完成。

## 交付清单

- [ ] 不可引用 intrinsic 的明确语义诊断，包含 currentSourceLocation、alias、依赖与 FunPtr。
- [ ] 包限定调用、值与 place、构造／variant／object／companion、声明与成员引用，以及 formatter／LSP。
- [ ] enum／Option 常量及 Elvis／when、独立静态常量身份、泛型／跨 Cone 共享与发射。
- [ ] core 的 scoop.compiler 声明、includeStr／includeBytes／env、Cone 目录范围及 SCOOP_ 名称限制。
- [ ] includeDir、glob、逐项目录／symlink 范围检查与 EmbeddedDir API。
- [ ] 构建／check／doc／LSP 的外部输入更新和缓存复用，资源／环境的定义来源。
- [ ] Darwin 与 Linux 必要组合验证、对应规范状态更新及功能提交。

## 验证与工作目录

代码在独立 worktree 的 `codex/compiler-intrinsics` 分支实现。原工作区其他设计变更不纳入本分支。临时输入、原始日志放在仓库外的 `scoop-work/compiler-intrinsics` 中，不使用 `/tmp`，不提交逐次原始报告。

每批代码先格式化及 lint，再运行与变更相关的 Rust／正式 CLI 测试。内置库变化后显式准备开发 sysroot；普通测试复用未变化产物。阶段完成后记录实际命令、覆盖与结果摘要。定期检查 target 占用，只在没有相关构建进程时清理可再生缓存。

## 已完成

设计与规范已从原工作区提取，包含用户追加的不可引用诊断、Cone 内路径／symlink 和 SCOOP_ 环境限制；编译器功能尚待交付。

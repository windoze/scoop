# 编译期 intrinsic 与包限定表达式实施记录

目标：完整实现本目录及 `../qualified-expressions/DESIGN.md` 的设计，按功能完成验证并提交。当前处于实施中，不能将单项完成视为整组能力完成。

## 交付清单

- [x] currentSourceLocation 的明确语义诊断，包含 alias、依赖与 FunPtr；错误下保留声明导航。新 intrinsic 随后接入。
- [ ] 包限定调用、值与 place、构造／variant／object／companion、成员引用的左侧限定目标，以及 formatter／LSP。
- [ ] enum／Option 常量及 Elvis／when、独立静态常量身份、泛型／跨 Cone 共享与发射。
- [ ] core 的 scoop.compiler 声明、includeStr／includeBytes／env、Cone 目录范围及 SCOOP_ 名称限制。
- [ ] includeDir、glob、逐项目录／symlink 范围检查与 EmbeddedDir API。
- [ ] 构建／check／doc／LSP 的外部输入更新和缓存复用，资源／环境的定义来源。
- [ ] Darwin 与 Linux 必要组合验证、对应规范状态更新及功能提交。

## 验证与工作目录

代码在独立 worktree 的 `codex/compiler-intrinsics` 分支实现。原工作区其他设计变更不纳入本分支。临时输入、原始日志放在仓库外的 `scoop-work/compiler-intrinsics` 中，不使用 `/tmp`，不提交逐次原始报告。

每批代码先格式化及 lint，再运行与变更相关的 Rust／正式 CLI 测试。内置库变化后显式准备开发 sysroot；普通测试复用未变化产物。阶段完成后记录实际命令、覆盖与结果摘要。定期检查 target 占用，只在没有相关构建进程时清理可再生缓存。

## 已完成

设计与规范已从原工作区提取，包含用户追加的不可引用诊断、Cone 内路径／symlink 和 SCOOP_ 环境限制。

### currentSourceLocation 引用诊断

managed／无期望类型／FunPtr 均先按真实 intrinsic 身份报告不能引用及直接调用来源要求；本地和依赖声明共用拒绝原因。未创建 adapter 或 native 地址，普通同名函数、显式包装函数和 lambda 不受此限制。编辑器保留被拒绝引用的声明绑定，不把错误函数体当成完整的类型检查结果。

Darwin 验证：`cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；`cargo test -p scoop-hir-lower references` 69 项通过；正式 CLI 的 `source-location-reference-diagnostics`、`annotation-query-callable-reference`、`source-locations-legacy-defaults-coroutines` 三项通过。覆盖精确错误位置、原 annotationKey 诊断及直接／默认／泛型／协程求值来源。Linux 组合验证留到后续平台验收。编译器和配套 CLI 已构建，本项无 core 或产物格式变化。

## 范围调整

2026-10-28：按用户要求取消前置点分声明引用 `::a.b.f`，撤回尚未提交的 AST／parser 改动和关联机械测试修改。保留既有 `::name`／`receiver::member`；跨包声明引用通过 import／alias 使用。包限定直接调用、值／place 和本组编译期 intrinsic 的实现目标继续保留。撤回后的编译器代码与已验证提交 `74f4c2eb8` 完全一致，本次仅更新设计与规范，不重复构建测试。

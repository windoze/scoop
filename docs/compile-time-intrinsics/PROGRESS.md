# 编译期 intrinsic 实施记录

目标：完整实现本目录的编译期 intrinsic 与常量设计，按功能完成验证并提交。当前处于实施中，不能将单项完成视为整组能力完成。

## 交付清单

- [x] currentSourceLocation 的明确语义诊断，包含 alias、依赖与 FunPtr；错误下保留声明导航。新 intrinsic 随后接入。
- [x] 带 payload 的 enum 静态初值数据通路：HIR／MIR／LIR、LLVM 发射与既有静态存储 relocation。
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

### 静态 enum 初值数据通路

将各阶段的单位 enum 初值扩展为实际 variant 与完整字段序列；保留 exact application，递归传递 String、标量和嵌套 enum。niche 直接使用原指针表示，tagged 初值按既有字段 offset 写入；padding 和 inactive GC slot 为零，String 字段进入既有 immortal relocation。静态初值编码与 LLVM enum 常量发射分别放入独立模块，避免继续增长原文件。没有改动 runtime ABI 或 `.slib` 编码格式。

Darwin 验证：格式化、全 workspace lint 通过；四个相关 crate 的 `static_enum_constants` 测试共 4 项通过，覆盖 Export HIR → concrete HIR → MIR → LIR、嵌套 Option、String relocation、浮点位型与 LLVM verifier；HIR 的 `static_variants` 编辑器回归 5 项通过；正式 CLI `native-raw-globals-and-tls` release 用例通过。编译器及配套 CLI 已构建。

本项仅完成常量数据表示与发射，不宣称源码 enum const／env 已可用。前端初值求值早于默认参数模板注册，enum 构造、默认 payload、const property 与控制表达式需在后续常量求值项中统一接通；当前未加入提前解析默认值的特殊路径。

## 范围调整

2026-10-28：按用户条件取消普通包限定表达式及前置点分声明引用 `::a.b.f`。点表达式的包分支需要协调现有 receiver、遮蔽、重载、类型别名和 property／place 流程，超出本组 intrinsic 所需的导入能力。相关未提交代码及测试已全部撤回，编译器代码回到已验证的 `d1b83f4e6`；本次仅同步文档，不重复构建测试。`scoop.compiler` package、显式 import／alias、intrinsic 引用诊断及常量实现目标保留。

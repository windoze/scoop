# 编译期 intrinsic 实施记录

目标：完整实现本目录的编译期 intrinsic 与常量设计，按功能完成验证并提交。当前处于实施中，不能将单项完成视为整组能力完成。

## 交付清单

- [x] 用户追加的[块级 import](../block-imports/DESIGN.md)：作用域、parser／formatter、普通查找与 LSP，作为 intrinsic 的就近导入方式。

- [x] currentSourceLocation 的明确语义诊断，包含 alias、依赖与 FunPtr；错误下保留声明导航。新 intrinsic 随后接入。
- [x] 带 payload 的 enum 静态初值数据通路：HIR／MIR／LIR、LLVM 发射与既有静态存储 relocation。
- [x] Unit／tuple 静态初值数据通路，以及显式类型 ordinary top-level property 的递归字面量初值求值。
- [ ] struct／tuple／Unit／enum／Option 聚合常量及字段读取、Elvis／when、独立静态常量身份、泛型／跨 Cone 共享与发射。
- [ ] core 的 scoop.compiler 声明、includeStr／includeBytes／env、Cone 目录范围及 SCOOP_ 名称限制。
- [ ] includeDir、glob、逐项目录／symlink 范围检查与 EmbeddedDir API。
- [ ] 构建／check／doc／LSP 的外部输入更新和缓存复用，资源／环境的定义来源。
- [ ] Darwin 与 Linux 必要组合验证、对应规范状态更新及功能提交。

## 验证与工作目录

代码在独立 worktree 的 `codex/compiler-intrinsics` 分支实现。原工作区其他设计变更不纳入本分支。临时输入、原始日志放在仓库外的 `scoop-work/compiler-intrinsics` 中，不使用 `/tmp`，不提交逐次原始报告。

每批代码先格式化及 lint，再运行与变更相关的 Rust／正式 CLI 测试。内置库变化后显式准备开发 sysroot；普通测试复用未变化产物。阶段完成后记录实际命令、覆盖与结果摘要。定期检查 target 占用，只在没有相关构建进程时清理可再生缓存。

## 已完成

设计与规范已从原工作区提取，包含用户追加的不可引用诊断、Cone 内路径／symlink 和 SCOOP_ 环境限制。

### 块级 import

支持块内 exact／alias／star import：声明后生效、内层优先、离块恢复，复用现有 selector、可见性、真实目标去重和调用候选规则。局部 callable／类型保留定义处的导入；泛型与跨 Cone 正文携带已绑定身份。parser、formatter、LSP 引用／补全／参数提示／折叠同步支持，局部 public import 明确报错。

Darwin 的格式化、workspace lint、定向及相关回归测试通过；5 项普通 Cone release 测试覆盖跨 Cone 泛型／默认值、局部类型、重载与扩展 invoke；正式诊断 fixture 通过。具体范围见块级 import 设计。无 runtime ABI 或 `.slib` 格式变化，Linux 组合验收仍按后续平台项执行。

### currentSourceLocation 引用诊断

managed／无期望类型／FunPtr 均先按真实 intrinsic 身份报告不能引用及直接调用来源要求；本地和依赖声明共用拒绝原因。未创建 adapter 或 native 地址，普通同名函数、显式包装函数和 lambda 不受此限制。编辑器保留被拒绝引用的声明绑定，不把错误函数体当成完整的类型检查结果。

Darwin 验证：`cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；`cargo test -p scoop-hir-lower references` 69 项通过；正式 CLI 的 `source-location-reference-diagnostics`、`annotation-query-callable-reference`、`source-locations-legacy-defaults-coroutines` 三项通过。覆盖精确错误位置、原 annotationKey 诊断及直接／默认／泛型／协程求值来源。Linux 组合验证留到后续平台验收。编译器和配套 CLI 已构建，本项无 core 或产物格式变化。

### 静态 enum 初值数据通路

将各阶段的单位 enum 初值扩展为实际 variant 与完整字段序列；保留 exact application，递归传递 String、标量和嵌套 enum。niche 直接使用原指针表示，tagged 初值按既有字段 offset 写入；padding 和 inactive GC slot 为零，String 字段进入既有 immortal relocation。静态初值编码与 LLVM enum 常量发射分别放入独立模块，避免继续增长原文件。没有改动 runtime ABI 或 `.slib` 编码格式。

Darwin 验证：格式化、全 workspace lint 通过；四个相关 crate 的 `static_enum_constants` 测试共 4 项通过，覆盖 Export HIR → concrete HIR → MIR → LIR、嵌套 Option、String relocation、浮点位型与 LLVM verifier；HIR 的 `static_variants` 编辑器回归 5 项通过；正式 CLI `native-raw-globals-and-tls` release 用例通过。编译器及配套 CLI 已构建。

本项仅完成常量数据表示与发射，不宣称源码 enum const／env 已可用。前端初值求值早于默认参数模板注册，enum 构造、默认 payload、const property 与控制表达式需在后续常量求值项中统一接通；当前未加入提前解析默认值的特殊路径。

### Unit／tuple 静态初值

HIR／MIR 保存 Unit 和递归 tuple 元素，LIR 使用 aggregate 初值；完整存储类型逐层提供元素类型。直接发射 ordinary top-level property 中可静态求值的 tuple／Unit 字面量，包含运行期调用时保留完整 initializer。LLVM 发射复用目标布局取得嵌套 tuple offset；静态模板保留 padding、零大小 storage token 和嵌套 String relocation。无 runtime ABI 或 `.slib` 格式变化，尚不扩大源码 `const val` 求值。

Darwin 验证：格式化、全 workspace lint 通过；6 项定向 Rust 测试通过，覆盖 HIR 至 LLVM、tuple／enum 嵌套、单位值、浮点位模式、引用重定位及静态／运行期初值选择。`tests/cones/aggregate-constants` 的 2 项 release 测试与正式 CLI `native-raw-globals-and-tls` fixture 通过。编译器及配套 CLI 已构建。编译期间磁盘空间不足已通过清理旧增量缓存解决；失败构建后相应验证通过。Linux 已确认 LLVM 22.1.2 可用，实际运行仍留在平台验收项。

## 范围调整

2026-10-28：按用户要求去掉只扩展 enum 的类型限制；常量按完整类型与实际值组成判断，包含 struct、tuple、Unit、空 struct 及编译期输入的不可变对象引用。enum 只检查选中 variant，不因其他分支携带运行期对象而拒绝 None。类型能表示常量不等于任意 initializer 都能编译期执行，保留本版不解释用户函数／次构造器正文的求值边界。尚未完成源码实现，不能将本次规范修订计为聚合常量功能交付。

2026-10-28：按用户条件取消普通包限定表达式及前置点分声明引用 `::a.b.f`。点表达式的包分支需要协调现有 receiver、遮蔽、重载、类型别名和 property／place 流程，超出本组 intrinsic 所需的导入能力。相关未提交代码及测试已全部撤回，编译器代码回到已验证的 `d1b83f4e6`；本次仅同步文档，不重复构建测试。`scoop.compiler` package、显式 import／alias、intrinsic 引用诊断及常量实现目标保留。

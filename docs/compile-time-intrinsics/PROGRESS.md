# 编译期 intrinsic 实施记录

目标：完整实现本目录的编译期 intrinsic 与常量设计，按功能完成验证并提交。当前处于实施中，不能将单项完成视为整组能力完成。

## 交付清单

- [x] 用户追加的[块级 import](../block-imports/DESIGN.md)：作用域、parser／formatter、普通查找与 LSP，作为 intrinsic 的就近导入方式。

- [x] currentSourceLocation 的明确语义诊断，包含 alias、依赖与 FunPtr；错误下保留声明导航。新 intrinsic 随后接入。
- [x] 带 payload 的 enum 静态初值数据通路：HIR／MIR／LIR、LLVM 发射与既有静态存储 relocation。
- [x] Unit／tuple 静态初值数据通路，以及显式类型 ordinary top-level property 的递归字面量初值求值。
- [x] const property 与静态初值共用聚合值表示；Unit／tuple／无负载 enum／None、前向引用、字段读取与跨 Cone 值编码。
- [ ] Option Elvis／when 常量求值、intrinsic 产生的聚合值及泛型／跨 Cone 传递。
- [ ] 常量存储合并优化：尽量共享可以安全合并的数据，保持不同常量实体身份；不作为源码合法性或唯一对象身份的强制要求。
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

本项仅完成常量数据表示与发射，不宣称源码 enum const／env 已可用。按后续范围简化，普通构造调用及默认 payload 不进入本版常量求值，不调整构造器默认值注册顺序；const property 与控制表达式仍需接通。

### Unit／tuple 静态初值

HIR／MIR 保存 Unit 和递归 tuple 元素，LIR 使用 aggregate 初值；完整存储类型逐层提供元素类型。直接发射 ordinary top-level property 中可静态求值的 tuple／Unit 字面量，包含运行期调用时保留完整 initializer。LLVM 发射复用目标布局取得嵌套 tuple offset；静态模板保留 padding、零大小 storage token 和嵌套 String relocation。无 runtime ABI 或 `.slib` 格式变化，尚不扩大源码 `const val` 求值。

Darwin 验证：格式化、全 workspace lint 通过；6 项定向 Rust 测试通过，覆盖 HIR 至 LLVM、tuple／enum 嵌套、单位值、浮点位模式、引用重定位及静态／运行期初值选择。`tests/cones/aggregate-constants` 的 2 项 release 测试与正式 CLI `native-raw-globals-and-tls` fixture 通过。编译器及配套 CLI 已构建。编译期间磁盘空间不足已通过清理旧增量缓存解决；失败构建后相应验证通过。Linux 已确认 LLVM 22.1.2 可用，实际运行仍留在平台验收项。

### 聚合 const 值与跨 Cone 传递

const property 复用 HirConstantImage，保存完整类型下的 Unit／tuple／struct／enum 值。源码求值接通 Unit、tuple、无负载变体、const 引用和实际字段／元素读取；前向引用和依赖环覆盖 tuple 及字段 receiver。类型按实际常量值判断，`None: Option<RuntimePayload>`、`Choice<RuntimePayload>.Empty` 不检查未选择分支。普通 struct／enum payload 构造及任意用户函数仍拒绝，没有引入新 literal 语法。

导出值使用独立的 canonical 聚合表示，保留原标量编码及注解标量语义，新增 Unit／tuple／struct／enum tag；枚举引用沿 typed identity 解码，消费时按完整 signature 匹配字段和实际 variant。移除无生产调用者的 const meta 语义重放 authority，保留实际边界的类型／格式／引用检查。多个 String 字段以 property 和元素序号区分；泛型正文与默认参数可传递这些值。常量存储合并仍是后续优化项，不把当前未合并的结果视为语言语义错误。

Darwin 验证：格式化、全 workspace lint 通过；329 项常量相关 Rust 测试通过，随后补充的聚合 wire／泛型默认值组合和 String owner 编码测试通过。原 Cone 的 4 项及实际 `.slib` consumer 的 2 项 release 测试通过；新增诊断 fixture 覆盖构造调用拒绝、元素类型／数量和字段依赖环；3 项既有诊断 fixture 已按实际求值范围更新并通过。静态运行期 ABI 不变；新聚合值和默认模板 String owner 扩展 HIR metadata 的 tag，`cross-cone-interface` 升至 82，旧产物需要重建。Linux 仍留在平台验收项。

格式版本补充验证：17 项 profile 测试通过，覆盖旧 section 拒绝、必要 section 清单与实际编码／指纹；新版 CLI 下的跨 Cone consumer 两项 release 测试通过。已显式准备 Darwin 开发 sysroot，25 个内置 Cone 和 runtime 全部完成。

## 范围调整

2026-10-28：按用户澄清，相同值的 const object 的运行期 identity 不保证相同或不同。不同泛型特化、泛型类 companion 及其中的常量实体保持独立；编译器可在语义正确的前提下共享底层数据。语言规范 9.1.2 增加此规则，11.19、实现／运行时规范及本设计同步去掉强制全局唯一对象、固定物理副本数量和各 profile 必须一致合并的要求。普通常量与 intrinsic 结果均可参与实际优化，不预建强制跨阶段常量身份体系。

2026-10-28：按用户要求简化本版，不支持构造器求值，只组合允许的常量表达式；struct 主／次构造、enum payload 构造和默认实参求值均移出本版。聚合常量表示、intrinsic 直接生成的 struct／enum 值及字段／模式读取保持。当前语言没有独立 struct／enum 花括号字面量，本次尚未引入新语法。

2026-10-28：按用户要求去掉只扩展 enum 的类型限制；常量按完整类型与实际值组成判断，包含 struct、tuple、Unit、空 struct 及编译期输入的不可变对象引用。enum 只检查选中 variant，不因其他分支携带运行期对象而拒绝 None。类型能表示常量不等于任意 initializer 都能编译期执行，保留本版不解释用户函数／次构造器正文的求值边界。尚未完成源码实现，不能将本次规范修订计为聚合常量功能交付。

2026-10-28：按用户条件取消普通包限定表达式及前置点分声明引用 `::a.b.f`。点表达式的包分支需要协调现有 receiver、遮蔽、重载、类型别名和 property／place 流程，超出本组 intrinsic 所需的导入能力。相关未提交代码及测试已全部撤回，编译器代码回到已验证的 `d1b83f4e6`；本次仅同步文档，不重复构建测试。`scoop.compiler` package、显式 import／alias、intrinsic 引用诊断及常量实现目标保留。

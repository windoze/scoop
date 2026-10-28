# 编译期 intrinsic 实施记录

目标：实现本目录的编译期输入 intrinsic，按功能完成验证并提交。按用户 2026-10-29 的范围调整，常量与常量表达式的进一步扩展留到后续专项；本轮只继续 intrinsic 所需的静态表示与发射。当前处于实施中，不能将单项完成视为整组能力完成。

## 交付清单

- [x] 用户追加的[块级 import](../block-imports/DESIGN.md)：作用域、parser／formatter、普通查找与 LSP，作为 intrinsic 的就近导入方式。

- [x] currentSourceLocation 的明确语义诊断，包含 alias、依赖与 FunPtr；错误下保留声明导航。新 intrinsic 随后接入。
- [x] 带 payload 的 enum 静态初值数据通路：HIR／MIR／LIR、LLVM 发射与既有静态存储 relocation。
- [x] Unit／tuple 静态初值数据通路，以及显式类型 ordinary top-level property 的递归字面量初值求值。
- [x] const property 与静态初值共用聚合值表示；Unit／tuple／无负载 enum／None、前向引用、字段读取与跨 Cone 值编码。
- [x] core 的 scoop.compiler.env、Option<String> 常量、SCOOP_ 名称限制、泛型／默认值及跨 Cone 传递。
- [x] env 的 build／check／doc 缓存更新、定义处求值及语言服务的共享 HIR 输入宿主。
- [x] includeStr、Cone 目录范围检查与文件输入更新。
- [ ] includeBytes 及静态字节数组。
- [ ] includeDir、glob、逐项目录／symlink 范围检查与 EmbeddedDir API。
- [ ] 文件／目录输入的构建／check／doc／LSP 更新和缓存复用、资源的定义来源。
- [ ] Darwin 与 Linux 必要组合验证、对应规范状态更新及功能提交。

## 验证与工作目录

代码在独立 worktree 的 `codex/compiler-intrinsics` 分支实现。原工作区其他设计变更不纳入本分支。临时输入、原始日志放在仓库外的 `scoop-work/compiler-intrinsics` 中，不使用 `/tmp`，不提交逐次原始报告。

每批代码先格式化及 lint，再运行与变更相关的 Rust／正式 CLI 测试。内置库变化后显式准备开发 sysroot；普通测试复用未变化产物。阶段完成后记录实际命令、覆盖与结果摘要。定期检查 target 占用，只在没有相关构建进程时清理可再生缓存。

按用户 2026-10-29 的补充要求，不涉及 LIR／codegen 的改动不跑完整测试套件；Linux 同样只做相关定向测试。Linux 独立验证环境位于 `nuc12:~/repos/scoop-compiler-intrinsics`，不修改该机器原有源码 checkout。

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

### env 编译期输入

core 新增 `scoop.compiler.env(name: String): Option<String>`，通过显式 import／alias 使用。普通名称和重载解析选中实际 intrinsic 后，在定义处直接展开为完整 Option 常量；泛型正文、默认值及 const property 沿既有聚合值通路发布，消费预编译依赖时保留 provider 的值。普通同名函数及构造调用不取得编译期执行资格。引用 env 给出无运行期入口的语义诊断，非法前缀、`=`／NUL、非常量参数在查询宿主前拒绝；非 Unicode 值报错，缺失与空值分别保存为 None 和 Some("")。

driver 和语言服务供给同一窄输入宿主，同次编译内按实际变量复用查询结果。成功编译返回实际环境输入；build／check／doc 在源码读取和工具探测前比较记录，只查询已使用的名字。父 driver 原样传递可查询的 SCOOP_ 环境，内部对象缓存改由 child 命令参数传递，避免覆盖用户设置的 SCOOP_SYSROOT／SCOOP_CACHE_DIR。环境记录不成为纯预编译消费者的输入。

本项新增 callable intrinsic tag 30，HIR `cross-cone-interface` 升至 83；machine protocol 升至 19，编译／check／doc 缓存域同步升级。没有改动 LIR、codegen 或 runtime ABI。Option Elvis／when 的常量求值仍是独立待完成项。

Darwin 验证：格式化及全 workspace lint 通过；env 定向 Rust 测试、protocol、profile、缓存及 child transport 测试通过。正式 CLI 的 env 值／缓存用例共 15 步、定义处跨 Cone 产物用例共 4 步、3 项 env 诊断 fixture 和 5 项既有 const 诊断 fixture 均通过。值／缓存用例覆盖缺失、空值、变化、未查询变量变化、运行期环境不影响产物、泛型／默认值，以及 build／check／doc 复用。已显式重建 25 个 Darwin 内置 Cone，并完成 runtime 准备。

Linux 验证：使用 LLVM 22.1.2，在独立目录构建所需 CLI、core／std 与普通 GNU runtime；相同 15 步值／缓存、4 步跨 Cone 产物、3 项诊断 fixture 及输入宿主定向测试均通过。初次复用的旧 runtime 索引与当前编译器不兼容，已在独立测试 sysroot 重建并复验通过。未跑全套测试或 GC stress。

### includeStr 与文件输入

core 新增 `scoop.compiler.includeStr(path: String): String`。名称解析选中实际 intrinsic 后，在定义文件处读取 regular file 并展开为静态 String；保留空文件、BOM、CRLF 与 NUL，UTF-8 错误给出字节偏移。普通调用、泛型与默认值使用同一展开路径；预编译依赖的消费者保留 provider 已嵌入的内容。引用、动态参数、空／NUL 路径给出直接的语义诊断，没有扩大常量表达式集合。

输入宿主保留原路径，分别检查词法定位与解析全部 symlink 后的目标位于定义 Cone 内，`link/..` 按真实文件系统解析。PM／driver 保留输入 Cone 根的定位拼写；single-file 按实际入口文件的父目录定位。同一编译请求复用读取结果，build／check／doc 记录实际文件与环境输入；更新判断只查询 metadata、解析后的目标及已使用的环境值，symlink 改指到 metadata 相同的文件也失效。失败构建不替换成功记录或已有产物。

语言分析保留输入宿主，后续语义查询能在源码版本不变时发现本地及源码依赖的资源更新，缺失文件补回后可恢复。LSP 的资源 watcher 注册与主动重发诊断仍在文件／目录输入更新项收尾，不能将这里的查询缓存更新等同于完整编辑器通知链。

本项增加 intrinsic tag 31，HIR `cross-cone-interface` 升至 84，machine protocol 升至 20，PM prepare 协议升至 2；相关缓存域及格式测试同步更新。未改动 LIR、codegen 或 runtime ABI。两机只重建本次需要的 core／std，沿用 ABI 26 的普通 runtime。

Darwin：格式化、全 workspace lint 通过；文件宿主、HIR 展开、协议、profile、缓存、PM prepare 与语义分析的定向测试通过；env 的定向 Rust 回归及 15 步 CLI 值／缓存回归通过。Linux：相关 lint、文件／HIR／协议／语义分析及 env 宿主测试通过。两机各自通过 includeStr 的 19 步值／缓存／单文件／失败恢复、4 步跨 Cone 产物及 1 步诊断 fixture；最后补齐 UTF-8 字节偏移后，HIR 与诊断定向复验通过。未跑完整测试套件或 GC stress。target 已检查，无超过清理期限的旧增量 session；原始记录继续保存在仓库外。

### 合并远端 main

2026-10-29 按用户要求合并 `origin/main` 的 `1de3cd2eb`，无冲突；本组 intrinsic／常量改动保留。上游变更新增 GC／分配优化，并将 runtime ABI 从 24 升至 26。两机显式更新本次验证所需的 core、std、公开头文件与普通 runtime；其他内置 Cone 和 stress 产物在后续使用前按新 ABI 准备。

合并后 Darwin 格式化、全 workspace lint、23 项分配、5 项静态数据及 1 项 runtime ABI Rust 测试通过；Linux 格式化、相关 crate lint、23 项分配和 6 项静态数据测试通过。两机各自的 env 值／缓存 15 步与跨 Cone 产物 4 步均通过，没有运行完整套件。原始日志继续保存在仓库外。

## 范围调整

2026-10-29：按用户要求停止继续扩展常量及常量表达式，撤回尚未提交的常量 Elvis／when 接线与实现方案。已提交的聚合值通路和 env 实现保留；新的常量控制表达式、通用常量能力及合并优化改由后续专项处理，不再作为本轮交付门槛。include 系列继续要求在编译期完成读取并将静态数据嵌入产物，输入参数复用已实现的常量表达式集合。

2026-10-28：按用户澄清，相同值的 const object 的运行期 identity 不保证相同或不同。不同泛型特化、泛型类 companion 及其中的常量实体保持独立；编译器可在语义正确的前提下共享底层数据。语言规范 9.1.2 增加此规则，11.19、实现／运行时规范及本设计同步去掉强制全局唯一对象、固定物理副本数量和各 profile 必须一致合并的要求。普通常量与 intrinsic 结果均可参与实际优化，不预建强制跨阶段常量身份体系。

2026-10-28：按用户要求简化本版，不支持构造器求值，只组合允许的常量表达式；struct 主／次构造、enum payload 构造和默认实参求值均移出本版。聚合常量表示、intrinsic 直接生成的 struct／enum 值及字段／模式读取保持。当前语言没有独立 struct／enum 花括号字面量，本次尚未引入新语法。

2026-10-28：按用户要求去掉只扩展 enum 的类型限制；常量按完整类型与实际值组成判断，包含 struct、tuple、Unit、空 struct 及编译期输入的不可变对象引用。enum 只检查选中 variant，不因其他分支携带运行期对象而拒绝 None。类型能表示常量不等于任意 initializer 都能编译期执行，保留本版不解释用户函数／次构造器正文的求值边界。尚未完成源码实现，不能将本次规范修订计为聚合常量功能交付。

2026-10-28：按用户条件取消普通包限定表达式及前置点分声明引用 `::a.b.f`。点表达式的包分支需要协调现有 receiver、遮蔽、重载、类型别名和 property／place 流程，超出本组 intrinsic 所需的导入能力。相关未提交代码及测试已全部撤回，编译器代码回到已验证的 `d1b83f4e6`；本次仅同步文档，不重复构建测试。`scoop.compiler` package、显式 import／alias、intrinsic 引用诊断及常量实现目标保留。

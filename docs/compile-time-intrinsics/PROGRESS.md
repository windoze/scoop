# 编译期 intrinsic 实施记录

目标：实现本目录的编译期输入 intrinsic，按功能完成验证并提交。本轮首批能力已实现；按用户 2026-10-29 的范围调整，常量与常量表达式的进一步扩展留到后续专项，只完成 intrinsic 所需的静态表示与发射。整组定向验证通过，完整回归的未闭合项见“最终验收与限制”。

## 交付清单

- [x] 用户追加的[块级 import](../block-imports/DESIGN.md)：作用域、parser／formatter、普通查找与 LSP，作为 intrinsic 的就近导入方式。

- [x] currentSourceLocation 的明确语义诊断，包含 alias、依赖与 FunPtr；错误下保留声明导航。新 intrinsic 随后接入。
- [x] 带 payload 的 enum 静态初值数据通路：HIR／MIR／LIR、LLVM 发射与既有静态存储 relocation。
- [x] Unit／tuple 静态初值数据通路，以及显式类型 ordinary top-level property 的递归字面量初值求值。
- [x] const property 与静态初值共用聚合值表示；Unit／tuple／无负载 enum／None、前向引用、字段读取与跨 Cone 值编码。
- [x] core 的 scoop.compiler.env、Option<String> 常量、SCOOP_ 名称限制、泛型／默认值及跨 Cone 传递。
- [x] env 的 build／check／doc 缓存更新、定义处求值及语言服务的共享 HIR 输入宿主。
- [x] includeStr、Cone 目录范围检查与文件输入更新。
- [x] includeBytes 及静态字节数组。
- [x] includeDir、glob、逐项目录／symlink 范围检查与 EmbeddedDir API。
- [x] 文件／目录输入的构建／check／doc／LSP 更新和缓存复用、资源的定义来源。
- [x] Darwin 与 Linux 必要组合验证、对应规范状态更新及功能提交。

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

### includeBytes（f3982705c）

core 的 `scoop.compiler.includeBytes` 复用定义处文件宿主与已有路径限制，直接保存原始 bytes；默认／泛型模板携带已读数据。MIR 的原 String 表扩展为实际 String／字节数组的共享 immortal 表，字节数组保存完整 class application。LIR／LLVM 发射只读数组 header、count 和数据，使用实际 Array<Byte> TD 与既有 immortal 登记；不运行数组构造或复制。普通数组字面量仍按原语义分配，通用 const／常量表达式资格保持本轮收缩后的范围。

callable intrinsic 新增 tag 32，默认模板表达式 tag 77；HIR `cross-cone-interface` 升至 85。ArrayConstant 对象角色／结构位置分别为 2／13，LIR `identity-foundation` 升至 10。runtime ABI 和文件输入协议不变。

Darwin／Linux 的 3 项普通 Cone 测试均通过，覆盖 256 种 byte、空数组、实际动态类型与可变副本。两平台定义处跨 Cone fixture 均通过 release 和 debug＋moving GC 两个变体，共 8 步；覆盖 global、泛型、默认值、普通函数及 provider 输入变化后消费已发布值。前端输入、字节模板编码、immortal 身份、数组登记布局和 LLVM 静态发射定向测试通过。Darwin 全 workspace lint、202 项 MIR lowering、357 项 identity、17 项 profile、92 项 driver 和 108 项 runtime 相关 Rust 测试通过；fixture runner 的 59 项规则测试通过。Linux 的完整 intrinsic 标签回归通过：9 项 fixture、10 个变体、54 个进程步骤，覆盖 env、includeStr 与 includeBytes。

按用户要求以 `30af1bdfa` 合并 origin/main 的 `726a38a96`，无冲突。全量 Rust 验收未全部通过：4 项 codegen 分配器／GC 断言已在原提交 `c9ba66107` 复现；合并后其中的 region GC 用例及相关 runtime 回归通过，其余分配器断言仍未改动。另有使用旧 CLI 参数的 linker 集成测试、runtime EH 契约与 slib 旧向量失败，留待独立处理。正式 CLI 全量验收在后续合并请求到来时停止：522 项通过、16 项失败、17 项不适用，其余 3752 项中止。失败涉及链接计划指纹不一致、优化断言、CLI／诊断差异及 JSON 库测试超时，尚未逐一归因与闭合，不能将定向验证记为全量通过。本次也修正了 driver 测试复制 core 源码时只接受平铺文件的辅助代码，以覆盖 scoop.compiler 子目录。旧对照构建 target 已清理，原始日志保留在仓库外。

### 合并 origin/main 的 ArrayList／StringBuilder 优化

2026-10-29 按用户追加要求，以 `1f35644e5` 合并 `origin/main` 的 `5c245cb7f`。解决运行时规范一处冲突：保留 include 的静态对象说明，同时采用上游新的 StringBuilder 字节存储合同。其余编译器改动自动合并，includeBytes 提交保留。

上游将 runtime ABI 升至 27。两机显式准备本次需要的 core／std／test、公开 C 头文件、普通和 stress runtime；其余旧 sysroot 资源在后续使用前重新准备。Darwin 全 workspace lint 和 15 项定向 Rust 测试通过；Linux 相关 crate lint 和 13 项定向 Rust 测试通过，覆盖输入 intrinsic、字节复制与根计划、静态字节数组和 ABI。两机各自通过 3 项 includeBytes Cone 测试，以及 9 项 intrinsic fixture（10 个变体、54 个进程步骤）。Linux 额外通过 7 项 StringBuilder 和 2 项列表增长测试，定向启用 moving／minor GC stress。未重跑完整套件。原始日志与部分全量验收记录继续保存在仓库外，清除了本轮验证产生的 16 个未跟踪 Cone.lock 文件。

### includeDir 与静态目录表

目录输入已接入共享输入宿主与成功响应／缓存记录，复用文件输入的 Cone 范围检查；glob 0.3.4 只负责字符串匹配。遍历先检查所有条目和链接，再读取选中的 regular files；保留逻辑路径、排序与原始 bytes。同一次请求复用结果，同一实际目标经多个逻辑路径出现时共用已读 bytes。更新检查重扫目录与解析目标，记录选中文件的长度／修改时间，不读取内容，也不因排除文件的内容变化重编。

machine protocol 升至 21，编译 key 域升至 v10，check／doc 缓存目录升至 v4，相关 fixture 路径已同步。Darwin 格式化与全 workspace lint、13 项输入测试、46 项协议测试通过；Linux 相关 lint、14 项输入测试、46 项协议测试通过。覆盖递归 glob／Unicode／隐藏项、空选择、深层新增／删除／改名、excluded symlink 越界、目录循环、重复目标、原始 link/..、相同 metadata 下改指及非普通文件；非法 UTF-8 文件名在允许创建该输入的 Linux 上验证，Darwin 文件系统在创建时即拒绝。随后接通的源码和静态发射验证如下。


core 增加普通 package `scoop.compiler` 下的 includeDir、EmbeddedFile 与 EmbeddedDir。名称解析沿普通命名／默认参数绑定；include／exclude 的数组字面量在 HIR 消费，结果为普通 struct 与静态文件表，不留下运行期筛选数组或文件访问。泛型、默认值和 inline 模板保存定义处已读数据，跨 Cone 消费不重新读取 provider 的资源。文件表使用真实 Array<EmbeddedFile> 布局，String／bytes 子对象及其引用沿既有 immortal 表和链接关系发射；runtime 允许只包含静态引用的 immutable immortal，继续检查大小、对齐、对象头与范围，不重放编译器生成的整个对象图。runtime ABI 仍为 27。

callable intrinsic 新增 tag 33、默认模板表达式 tag 78；HIR `cross-cone-interface` 升至 86，LIR `identity-foundation` 升至 11。运行期接口、普通构造器和通用 const 表达式范围保持。实际 debug 跨 Cone 用例暴露聚合字段读取与根恢复读取直到机器下降才合并的问题：debug 在 RS4GC 前复用 InstCombine／EarlyCSE 完成标量规范化，物理根数量与最终 stackmap 一致；没有增加独立根等价算法。

Darwin／Linux 均通过 8 项普通 Cone 测试，以及新增 3 项正式 fixture（4 个变体、26 个进程步骤）。覆盖目录顺序、原始 bytes、空结果、named／default filters、精确查找、泛型、用户构造、可变副本、引用／非法输入诊断、release 与 debug＋moving GC 下的跨 Cone 静态引用，以及 build／check／doc 的新增、删除、改指、排除项变化、失败恢复与缓存复用。两平台的 11 项 HIR 输入测试、72 项 statepoint 测试、静态发射与 runtime immortal 登记测试通过；Darwin 的 195 项默认模板测试通过。新增聚合读取回归在修复前稳定复现 stackmap 数量失配，修复后覆盖全部三个后端目标和两种 profile。

Darwin 全 workspace lint 通过；一次全量 Rust 运行报告 6209 项通过、18 项失败，另有 formatter corpus 线程栈溢出和 doc 阶段依赖产物缺失。6 项失败已闭合：缓存固定向量、shared-intrinsic 枚举、两项 profile、语义指纹，以及合并后的 StringBuilder 已改用 MutableArray<UInt8> 的 core 应用类型预期。对应 6 项缓存、5 项 shared-intrinsic、18 项 profile、8 项语义指纹与真实 core HIR→MIR→LIR 定向测试通过；前四组也在 Linux 通过。formatter 设置 RUST_MIN_STACK=16777216 后完整 corpus 通过。剩余 12 项失败涉及先前记录的 3 项 codegen 分配断言、5 项使用旧 CLI 的 linker 测试及 4 项 runtime ABI 固定向量；doc 阶段待顺序复验，不能记为全量通过。完整 CLI 验收正在显式准备其所需 sysroot，Linux 维持定向验证。LSP 资源 watcher 与主动重发诊断仍属于独立未完成项。清理了不再使用的 46 MiB 重现程序 target；原始日志和运行目录继续保存在仓库外。

### LSP 资源通知与主动诊断

共享分析将当前项目实际查询的输入宿主交给 LSP，包含源码依赖和失败请求。客户端支持动态注册时监听对应 Cone 的定位根／实际根及后代，覆盖缺失文件补建、目录新增、链接改指与目标变化；请求不再使用资源或配置改变时撤销旧注册。通知先复用输入记录判断更新，成功输入未变时不重读内容或调度分析。实际变化独立推进 revision、取消旧快照并重新调度打开文档，保留未变源码的语法缓存；没有增加单独的 intrinsic 求值器或修改编译产物格式。

Darwin／Linux 相关 crate lint 通过，LSP 共 23 项测试通过（原完整 crate 运行中两个新增测试的输入设置已修正并定向重跑），另各通过 1 项共享分析组合测试。实际 stdio 进程覆盖 includeStr 的非法 UTF-8、includeBytes 缺失／恢复、includeDir 嵌套项变化、源码依赖与 Cone 内目标更新、排除项越界链接修复、未改源码时诊断清除、无关文件写入不重发诊断，以及移除 include 后撤销监听；共享分析验证失效旧快照时仍保留 SourceVersion。字符串 glob 的转义使用真实 matcher 验证。

客户端支持 relativePatternSupport 时，资源注册使用 Cone 父目录 URI 与目录名的 RelativePattern，使 VS Code 能监听工作区外的源码依赖，并接收 Cone 根删除／重建的通知。不支持该能力时保留字符串模式。两平台格式检查与 LSP lint 通过，两个 stdio 资源测试分别通过；实际覆盖两种能力分支、外部依赖注册，以及 Cone 根缺失时报错、恢复后清除诊断。

includeDir 验收补充：全 workspace doc tests 顺序重跑通过；此前缺少依赖产物的问题由重叠 Cargo 构建导致，后续 Cargo 验证保持串行。Darwin 已显式准备当前 worktree 的 25 个内置 Cone 和四种 runtime，fixture runner 的 59 项规则测试通过。完整 CLI 正在使用固定的工具副本运行；一次误指向主工作区旧 core 源码的配置已更正并重跑，该批配置错误不计为编译器失败。全量 Rust 剩余 12 项失败仍按上节记录，不能宣称全量通过。

### 组合验收收尾

Linux 在 includeDir 与 LSP 接通后通过整组 12 项 intrinsic fixture（14 个变体、80 个进程步骤），包含四个入口、缓存与跨 Cone 组合。两项此前未闭合的正式 fixture 已定位为测试设置问题并修正：注解数组的 debug 重链接补齐与 build 相同的 keep-symbols 选项，实际链接指纹完全一致；版本冲突用例保留正常的 Cone／lockfile 相对位置，诊断仍断言原有信息和位置。两平台分别通过相应 2 个变体／10 步和 1 个变体／1 步，没有改写指纹预期或放宽诊断断言。Darwin 完整验收继续运行；已清理约 54.7 GiB Rust 增量缓存，保留可执行工具与依赖产物。

符号裁剪 fixture 同样补齐其符号表检查所需的 keep-symbols 选项，Darwin 的 debug／release 两个变体、6 步通过，包含 moving／minor GC 运行验证。Char 的常量负例同步此前字段查找已改变的诊断文字与字段位置，两平台各 1 步通过；源码仍被拒绝，没有扩展常量求值。

Linux 补齐块级 import 与位置 intrinsic 的平台验收：普通 Cone 的 5 项 release 行为测试通过，块级 import 诊断、source location 引用拒绝、annotation query 引用诊断及默认值／泛型／协程位置组合共 4 项 fixture、6 个进程步骤通过。

完整回归发现 combinations-artifacts、containers-artifacts、containers-updates 的 debug 重链接同样遗漏 keep-symbols。补齐与 consumer build 一致的选项后，两平台各自通过 3 项 fixture、6 个变体、24 个进程步骤，保留链接指纹比较与实际程序输出断言。

### 最终验收与限制

四个输入 intrinsic、块级 import、不可引用诊断、构建／check／doc 输入更新及 LSP 主动诊断均已分项提交。Darwin／Linux 的整组 intrinsic fixture、普通 Cone 行为和相关 HIR／静态发射／GC／编辑器定向验证通过；Linux 只做相关测试，没有运行完整套件。

Darwin 完整 CLI 选择覆盖 4310 项，按每项最后一次明确验证汇总为 **4143 通过、144 失败、6 项 fixture 配置错误、17 项目标不适用**，没有未完成的 fixture。为提高效率，前两批保留已完成结果，最后 2195 项仅使用当前源码构建的 Rust release 工具续跑；Scoop 的各 fixture profile 与断言不变。切换前 includeDir 跨 Cone／GC 用例通过，耗时由约 36 秒降至 7.7 秒。7 项失败经前述 fixture 修正闭合；JSON 库测试使用 release 工具在原 600 秒时限内以 307 秒通过，合计闭合 8 项。失败用例停止后的步骤不计为通过。

剩余问题按实际失败位置归组，不将全部失败推定为本分支之前已有，也不将本次结果记为全量通过：

| 类别 | 数量 | 当前证据与限制 |
| --- | ---: | --- |
| 诊断预期 | 63 | 包含 PM 接管配置诊断、core 源码位置变化及扩展候选原因增加。扩展失败合并实现与 origin/main 的 `158d94d3c` 完全一致；其余保留实际差异。 |
| 链接计划指纹比较 | 55 | 已修正的代表用例证明 build／link 符号选项不一致会造成该差异；剩余同类用例尚未逐项修正。 |
| MIR／LIR 或归档成员快照 | 12 | 保留完整差异，未批量更新快照。部分同类问题在此前 fmt／check 验收中已有记录。 |
| 优化／符号断言 | 6 | 两项 array-inlining、counted-array-bounds、两项 reference-array 和 type-pruning。counted-array-bounds 的 release 程序运行通过，其 IR 断言仍失败。 |
| 其他运行／fixture 设置 | 8 | 包括两项运行失败、缺少 scoop-pm／map 文件、旧 artifact 版本约束、registry 配置及 root 列表差异，见下文。 |
| fixture 源码替换配置 | 6 | imported-class-primitive-inherited 用例的旧 strings.scoop 替换片段无法匹配；未进入编译验证。 |

两项运行失败为 client-observer-lifetime 的 Completed／caller cancellation 类型断言，以及 generic-delegate-siblings-consumer 的 IllegalStateException。前者用已生成程序单独运行仍失败，尚未完成基线归因；后者在此前 [fmt／check 验收](../fmt-check/VALIDATION.md) 有基线复现记录。其他设置问题分别位于 cli-core-locator-cache、cli-input-manifest-roots、executable-native、installed-tools、odr-selection 和 test-selection。本轮不扩大为这些已有子系统的修复任务。

上述完整 Rust 运行当时保留 12 项失败，其后续定向闭合见下节；formatter corpus 和顺序 doc tests 已通过。完整与定向报告、失败分组和日志留在仓库外，文档只记录范围与限制。新输入 intrinsic 的 12 项正式 fixture 全部通过，未因上述失败放宽其输入限制、诊断、缓存、产物或 GC 断言。末次收尾清除 45 个测试生成的未跟踪 Cone.lock，并在 Cargo 构建锁内清理约 530 MiB 新增 Rust 增量缓存。

### Rust 遗留 12 项闭合

按用户要求，以 `150c34db9` 合并最新 origin/main `0bded827b`。保留 main 的调试信息与本分支编译期输入，合并后的 machine protocol 为 22、HIR `cross-cone-interface` 为 87；main 的栈追踪 intrinsic 保留 tag 30，env 改用独立 tag 34，并验证两者编码与完整往返。同步合并引起的接口版本、缓存、语义指纹及协议负例的字段位置预期。

原 12 项失败全部闭合：3 项 codegen 断言验证实际的常量对齐、TLAB 容量及写入后的原子 card mark；4 项 ABI 固定向量同步 main 的 runtime ABI 29；5 项 linker 测试使用 main 已迁移的 scoop 构建入口和预编译资源，实际链接、更新输入、符号裁剪及程序运行均通过。原 `final_link_strips_internal_names_and_keeps_address_map_and_runtime_data` 随 main 改名为 `final_link_strips_internal_names_and_keeps_runtime_data`。

Darwin／Linux 均通过格式检查与 workspace lint，各通过 138 项定向 Rust 测试（86 项接口／缓存／ABI／分配／写屏障检查、5 项 linker、47 项协议）。Linux 的 linker 组合以 ELF 动态库更新用例替代 Darwin 专有符号用例。两平台额外通过 env 与 includeDir 跨 Cone fixture，共各 3 个变体、12 个进程步骤，包含 debug moving GC。使用仓库外单独准备的 core／std 与 runtime，原始日志和报告不入库。本轮没有重跑完整测试套件，之前完整 CLI 的其他失败状态不变；收尾在 Cargo 构建锁内清理约 4.46 GiB 本地增量缓存。

## 范围调整

2026-10-29：按用户要求停止继续扩展常量及常量表达式，撤回尚未提交的常量 Elvis／when 接线与实现方案。已提交的聚合值通路和 env 实现保留；新的常量控制表达式、通用常量能力及合并优化改由后续专项处理，不再作为本轮交付门槛。include 系列继续要求在编译期完成读取并将静态数据嵌入产物，输入参数复用已实现的常量表达式集合。

2026-10-28：按用户澄清，相同值的 const object 的运行期 identity 不保证相同或不同。不同泛型特化、泛型类 companion 及其中的常量实体保持独立；编译器可在语义正确的前提下共享底层数据。语言规范 9.1.2 增加此规则，11.19、实现／运行时规范及本设计同步去掉强制全局唯一对象、固定物理副本数量和各 profile 必须一致合并的要求。普通常量与 intrinsic 结果均可参与实际优化，不预建强制跨阶段常量身份体系。

2026-10-28：按用户要求简化本版，不支持构造器求值，只组合允许的常量表达式；struct 主／次构造、enum payload 构造和默认实参求值均移出本版。聚合常量表示、intrinsic 直接生成的 struct／enum 值及字段／模式读取保持。当前语言没有独立 struct／enum 花括号字面量，本次尚未引入新语法。

2026-10-28：按用户要求去掉只扩展 enum 的类型限制；常量按完整类型与实际值组成判断，包含 struct、tuple、Unit、空 struct 及编译期输入的不可变对象引用。enum 只检查选中 variant，不因其他分支携带运行期对象而拒绝 None。类型能表示常量不等于任意 initializer 都能编译期执行，保留本版不解释用户函数／次构造器正文的求值边界。尚未完成源码实现，不能将本次规范修订计为聚合常量功能交付。

2026-10-28：按用户条件取消普通包限定表达式及前置点分声明引用 `::a.b.f`。点表达式的包分支需要协调现有 receiver、遮蔽、重载、类型别名和 property／place 流程，超出本组 intrinsic 所需的导入能力。相关未提交代码及测试已全部撤回，编译器代码回到已验证的 `d1b83f4e6`；本次仅同步文档，不重复构建测试。`scoop.compiler` package、显式 import／alias、intrinsic 引用诊断及常量实现目标保留。

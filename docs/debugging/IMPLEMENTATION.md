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

按用户追加要求完成 `.slib` 体积调查并把 Zstandard 统一改为标准 level `3`，已提交推送；本机 25 库现有成员的离线重编码减少约 106.82 MiB。旧来源 wire 全面索引化、LIR 重复身份和对象粒度调整的收益与实施成本保存在[体积调查](../slib-storage/FOLLOW_UP.md)，原始报告不入库。

源码位置已从 LocalConcrete HIR 贯通至 MIR 的函数、源码局部、表达式、语句和 terminator。公共 `scoop-source` 记录对文件、位置、函数显示名称和内联调用点去重，IR 节点保留共享引用；已规范化的定义来源集合也共享底层存储。CFG、常量替换、分配重写和 MIR 内联保留或更新位置，生成操作显式分类。此批只建立内存中的来源数据，不向 HIR／MIR／LIR metadata 写入新的调试快照。

共享记录及嵌套内联链、中文／CRLF 和导入位置换算、MIR 内联观测及六项基本 lowering 测试通过。DWARF 发射、变量声明／作用域的完整描述和用户调试入口继续在后续变更中完成。

LIR 已贯通函数、指令和 terminator 的来源引用。指令与来源作为同一记录移动，GC poll／协议生成指令显式无源码位置；实际托管调用测试确认插入 poll 后仍引用原调用位置，三项 roots 回归通过。公共文件记录分别保存语言身份及 debugger 路径，本地沿已有输入 locator，导入模板沿已有逻辑路径，不读取源码补信息或重复写入 metadata。

LLVM 后端已按独立级别发射 DWARF 5 行表、源码函数名／精确 linkage name、文件路径和内联位置链。生成操作使用无源码的 line 0，调试 builder 在优化前 finalize。定向对象测试覆盖 O0／O2 与 none／line-tables 的四种组合，验证 GC statepoint 变换前后 LLVM 合法、内联调用点仍存在，以及实际 Mach-O 行表的有无。此阶段仍需接通用户 CLI 和真实源码 debugger 验收。

源码 CLI、machine protocol 和编译缓存已贯通独立调试级别；目前接受 none／line-tables，完整变量发射完成后再开放 full。debug 暂默认 line-tables，release 默认 none，开启时自动生成伴随调试文件。协议升级为 19。HIR 单独保存实际 debugger 路径，避免语言的语义来源路径覆盖它。

协议往返、缓存级别隔离、CLI 参数及实际前端路径回归通过。真实 Scoop 程序使用预编译 core／std 生成含空格名称的 dSYM，程序输出正确、DWARF 验证通过、LLDB 按源码绝对路径解析断点；重复构建没有 child 调用。本机 Xcode LLDB 和上游 LLVM 22 LLDB 均在启动普通 C 对照程序时卡住，进程内断点／栈验收仍待解决，不能以离线断点解析代替。

公共记录已增加共享源码变量声明和词法父子关系。MIR 源码局部复用 PersistentLocalValueId；内联复制共享原声明，保留源码名称并更新位置链，生成结果仍显式分类。作用域由已有 HIR 嵌套正文、catch 和 pattern 绑定投影，LLVM 行表使用对应的词法块。三项共享记录测试、实际 MIR 内联变量回归及四组合 DWARF／statepoint 测试通过；此批仍未开放 full，也未增加 `.slib` 调试快照。

LIR 已把源码变量关联到原有 local slot／逻辑参数，并建立按 exact type 共享的静态类型记录，保留实际字段偏移、对象前缀、数组和 tagged／niche enum 布局。类型记录只用于当前原生 DWARF 发射，不写入 LIR metadata。结构体变量测试验证声明身份、实际栈槽及字段偏移，真实 core／普通 callable 管线回归通过。后端变量记录及 full 入口继续实施。

Full DWARF 已接通源码参数、局部、词法作用域和精确静态类型，debug 默认 full、release 默认 none；协议为 20，显式级别与优化模式独立。参数按实际 ABI 发射直接值或 fragment，普通局部观察已有存储，managed 值只记录实际读取／赋值；safepoint 后无法保证位置时结束记录，后续真实读取可恢复。调试没有增加 GC roots、存储或程序读取。类型显示名复用 MIR 的可读名称，exact id 仍用于类型关联与 DWARF 唯一性。

本地定向验证通过：六组合行表／优化／statepoint、递归类型和参数片段、GC 前后失效与根数量对比、协议／缓存／CLI，以及十项 LIR 布局回归。真实 Scoop 默认 full 程序输出正确，dSYM 校验无错误，LLDB 可读取 Long 类型和 answer 变量位置并解析源码断点；重复构建没有 child 调用。全局／TLS、跨 Cone 模板局部名及闭包／协程绑定继续实施。stock LLDB 在帧显示中优先使用精确 linkage 名，此显示问题和本机进程启动卡顿仍需闭合，当前结果不代表进程内 DAP 验收完成。

全局／TLS 绑定已随 property 与 singleton 来源传递，在定义实际存储的对象中发射 DIGlobalVariable；纯数据对象也可建立调试单元。生成的失败根与 delegate 协议存储不冒充源码变量。全局对象定向测试通过，真实 Scoop 的 counter、threadCounter、label 均出现在 dSYM 中，TLS 使用 DW_OP_form_tls_address，DWARF 校验和程序输出通过。未为此新增 `.slib` 调试快照。

跨 Cone 模板的现有局部记录增加源码名称字段，默认表达式、构造片段和泛型正文实例化保留原名称；HIR cross-cone-interface 升至 82，MIR／LIR 不增加持久化快照。接口测试 600 项、中文名称往返和生产端声明测试通过；真实 core／泛型 provider／consumer 流水线通过，并确认 catch／finally 局部名保留。该组合测试同时发现并修复生成协程 helper 的调试签名分类：内部机器参数不要求源码 exact type，frame 内部字段仅提供物理类型描述。

Throwable 已改为可继承的 intrinsic FixedObject，保留公共零参数构造；24-byte 前缀中的 offset 16 是私有快照引用，普通派生字段跟在完整前缀后，继承扫描保留这个隐藏根。实际 core 的 Throwable／Exception 布局、跨库构造与泛型消费、两层继承及 metadata 往返通过，HIR 相关规则回归 34 项通过。HIR 两项 capability 升至 83／31，MIR type-bridge 升至 20，LIR layout-abi 升至 18，runtime ABI 升至 28，metadata ABI 保持 9。编译器、runtime 与系统库必须配套重建；这一批只完成存储和构造，首次采集与打印继续实施。

LLDB 名称显示问题已定位到上游 LLVM 22 的 DWARFASTParserClang：存在 DW_AT_linkage_name 时，Function 的 Mangled 名称未保留 DW_AT_name，导致帧显示与按源码名查询使用编码后的符号。发行集成需要验证一个保留两种名称的小型兼容修补，不能通过丢弃精确 linkage 身份规避。官方 macOS 预编译 LLDB 包没有 Python 支持，发行打包也需使用启用 Python 的构建。两项尚未完成，不影响已验证的 DWARF 类型、行表和变量记录结果。

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

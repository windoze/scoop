# 原生调试实施记录

分支：`codex/native-debugging`。日期：2026-10-29。

本轮基础源码调试、Throwable 历史栈与 trap 栈打印、调试器打包和 VS Code 集成已完成。编译器与工具的具体能力、实际验收和平台限制如下；完整 Scoop 表达式求值及协程逻辑栈继续作为独立工作。

## 编译接口与产物

`scoop build/run/test` 和 `scoopc` 接受 `--debug-info none|line-tables|full`，与 O0／O2 独立。debug 默认 full，release 默认 none；release + full 保持优化并提供仍可定位的变量。仅消费产物的 `scoop link`／`scoop-link` 使用 `--emit-debug-file`，不能补造输入中没有的调试数据。

| 目标 | 产物及处理 |
| --- | --- |
| Darwin/AArch64 | DWARF 5、相邻 dSYM；dsymutil 完成后 strip，再按最终字节签名 |
| Linux/amd64 GNU、musl | DWARF 5、相邻 `.debug`、`.gnu_debuglink`；动态及静态程序均保留 SHA-1 build-id |

伴随文件参与正常结果存在判断和发布，最终 basename 进入调试结果更新键。同 basename 换目录可复用，换 basename 重新生成最终调试文件；命中不启动调试工具、不重读产物内容。旧 `--emit-symbol-map`、map 字段和性能 map parser 已移除。性能工具使用 LLVM JSON、标准调试文件与精确 linkage identity，支持 inline 链、Darwin slide 和 Linux load bias。

machine protocol 为 20，runtime ABI contract 为 29，metadata ABI 保持 9；当前 HIR cross-cone-interface 为 84。Throwable 前缀和实际模板字段已改变，编译器、runtime、core、派生异常所在库及消费者须沿正常版本机制配套重建。

## 来源共享与 `.slib`

`scoop-source` 集中保存共享的文件、位置、作用域、声明与内联调用链；HIR／MIR／LIR 节点保留共享引用和本阶段实际变换关系。MIR／LIR 没有新增完整的持久调试快照。跨 Cone 所需的局部名称进入原 HIR 模板局部表，已有来源与定义／求值语义继续复用。LIR 的具体类型、字段偏移和变量存储关联仅服务当前原生 DWARF 发射。

低成本体积优化已实施：现有 Zstandard 成员压缩统一由 `-3` 改为 `3`，reader 和容器格式保持兼容。对已有 25 个 Darwin 库离线重编码，成员合计减少约 106.82 MiB，约占 849.83 MiB 全样本的 12.6%；这是现有成员的重编码估算，不是全量重建归档的大小。来源 wire 全面索引化、LIR／manifest 身份去重、对象／DWARF 合并等较大方案及测量方法见[体积调查](../slib-storage/FOLLOW_UP.md)。

## DWARF 与移动 GC

函数同时保留源码名和精确 linkage name；line-tables 也保留 linkage，full 另提供参数、局部、全局／TLS、作用域和实际类型布局。生成操作显式没有用户位置，生成 helper 标为 artificial。内联链、模板局部名称与闭包实际读取贯通到最终记录。generated-C 桥保留展开信息和物理帧，不伪造用户源码行。

变量只观察程序已有的 SSA 值或存储，不增加调试专用的读取、栈槽、GC root 或额外保活。已有 canonical root 先通过 declare 交给 SROA 维护真实分片，GC lowering 后统一为可结束位置区间的 value 记录。safepoint 后使用该边实际恢复的根和支配当前观察的真实 relocation；不能确认的位置显示 unavailable。调用结果在旧观察失效后建立位置，normal／unwind 边分别处理。

定向后端测试比较 none／full 的 root、slot、statepoint 和 relocation 数量；真实 DAP 用例进一步检查 GC 后对象地址已改变且字段仍正确、新建对象在存活时可展开、已失效变量不可用、调用者帧不显示搬迁前地址。

## Throwable 与 trap

Throwable 是可继承的 intrinsic FixedObject，保留公共零参数构造；24-byte 前缀中 offset 16 保存私有快照引用，普通派生字段从完整前缀后开始，GC 扫描继承该隐藏根。

首次 throw 的 Managed 准备采集物理地址、分配普通 immutable byte array 并一次发布，返回潜在搬迁后的同一异常对象；重抛和延迟打印保留原栈。同对象并发采集只发布一份完整快照。`printStackTrace()`、未捕获报告及语言 trap 使用稳定数据输出；语言 trap 保持退出码 1，内部 compiler fatal 保持 abort 语义。

运行期符号化使用固定 libbacktrace 版本及源码名／DWARF 5 完整路径修补，不启动外部 LLVM 或 LLDB 进程。快照保存原始 PC、查询 PC、模块 UUID／build-id、load bias 和路径。没有调试信息时保留模块与地址；检测到 loader 变化时保守降级，避免用新模块误解释旧地址。

## LLDB、VS Code 与发行脚本

固定 LLVM 22.1.8 的 Python-enabled LLDB 构建保留源码显示名及精确身份，支持按两种名称命中同一断点；同源文件的独立编译单元全局量加入所选栈帧，TLS 使用该线程上下文。Linux/amd64 静态 TLS 从 PT_TLS 和线程指针定位，不依赖动态 link map。

VS Code 扩展 0.3.0 注册 Scoop debug type 和源码断点。构建／launch 从 JSON executable 记录取得真实路径；支持 attach、参数、cwd、环境、终端、sourceMap，以及 `scoop test --no-run` 与测试过滤。只读 formatter 显示 UTF-8／内嵌 NUL String、分页数组、tagged／niche enum、class／interface 和 raw 字段，不执行 getter／toString 或写入 managed 对象。使用方式见[扩展说明](../../tooling/vscode-scoop/README.md#原生调试)。

完整打包入口调用 LLVM／Clang SDK 与 LLDB 构建，并收集 lldb、lldb-dap、LLDB library、argdumper、debugserver／lldb-server、匹配 Python、标准库／原生模块、Clang resources、formatter 和实际非系统动态依赖。依赖改为安装根内相对加载；Darwin 最后签名并保留 debugserver entitlements。源码兼容修补随 llvm-sources 分发。维护与复用方法见[脚本说明](../../scripts/README.md#调试器发行资源)。

## 验证范围与结果

先完成本地功能和定向验证，再使用 nuc12 的 `~/repos/scoop-native-debugging-20261029` 独立目录验证 Linux。未运行无关完整测试套件，原始日志、逐轮报告和临时程序存放仓库外。

| 范围 | 结果 |
| --- | --- |
| Rust／Python／扩展 | workspace fmt／clippy、涉及来源与 lowering／ABI／后端的定向测试、fixture runner 规则测试、Python Ruff 0.16.10；扩展 Biome／TypeScript、15 项 Node 测试、构建与 VSIX 打包通过 |
| 后端与链接 | O0／O2 × none／line-tables／full、真实对象／DWARF、参数分片与递归类型、GC 位置、跨 Cone 模板／ODR、全局／TLS；独立调试文件及缓存／basename／源码更新的定向用例通过 |
| Throwable／trap | Darwin、GNU、musl 的 full／line-tables／none 实际程序通过：首次栈、catch 后保活、重抛、移动／minor GC、uncaught／trap 退出与源码行；同对象并发首次 throw 通过 |
| GNU、musl 进程内 DAP | 两目标 launch、源码映射、断点／单步、栈、全局／TLS、String／数组／enum／interface／闭包，以及移动 GC 后变量检查通过；GNU 另通过真实 PTY 终端输入和 attach |
| TLS 与离线归因 | musl 静态 TLS 的初始化／零初始化、64-byte 对齐及两个线程通过；ELF 两目标源码／精确符号查询通过，GNU PIE 使用 `/proc` 取得的真实非零 load bias 通过离线 CLI 符号化 |
| 调试器发行资源 | Darwin／Linux 调试器包位于含空格路径，以系统 PATH 启动；Python 标准模块、formatter、真实 DWARF 和源码名断点通过；dyld／ELF loader 记录确认私有依赖来自包内。Linux 正式 LLDB recipe 构建及随后复用通过 |

可重复的外部协议和静态值用例见[调试验收入口](../../tooling/vscode-scoop/test/debugger/README.md)，栈诊断用例位于 `tests/fixtures/compiler-correctness/debugging/`。

本机 macOS 的进程 launch 仍受环境限制：系统及上游 LLDB 对普通 C 对照程序也会卡住。本机验收覆盖真实 Scoop 运行、对象／dSYM 校验、调试器加载和离线观察，进程内 DAP 结论来自 Linux。未运行 VS Code extension host 的图形调试会话；扩展通过构建／单元测试和实际 adapter 协议验证。发行验证针对调试器及其依赖，没有重跑全部 25 库的完整工具链发行构建。

## 后续范围

协程已有 owner、state、completion 和 saved value identity，但缺少可消费的状态到挂起行映射、当前 frame 参数和 adapter／终端 continuation 识别。完整逻辑栈还需验证线程迁移、合成帧选择及 saved locals，不能只靠重命名物理 helper 或打印任务树完成。本轮保留物理栈，后续按[主设计 10.4](DESIGN.md#104-异常协程和数据断点) 独立实施。

完整 Scoop 求值／条件表达式、managed 写入、Scoop 异常筛选、GC 感知数据断点及 Windows/PDB 均不属于本轮交付。复杂 `.slib` 优化保留在体积调查中。

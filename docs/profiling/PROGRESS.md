# Profiling / PGO 实施记录

状态：首版实现及验收完成。开始日期：2026-11-13；完成日期：2026-11-14。设计见 [DESIGN.md](DESIGN.md)，最终范围与性能边界见 [VALIDATION.md](VALIDATION.md)。

实现分支：`codex/profiling-pgo`。每个完整变更在本地完成针对性验证后提交、推送，再合并最新 `origin/main`；跨平台验证按完整大项安排，不逐文件重复全量测试。原始日志和临时训练产物保存在仓库外，完成一轮后清理不再使用的文件。

## 交付范围

| 大项 | 当前状态 | 验收依据 |
| --- | --- | --- |
| 固定 MIR 采集位置、LLVM InstrProf 与 ODR 计数 | 已实现，六个目标的原生采集与反馈闭环通过 | 确定计数、内联映射、多线程、对象选择/裁剪 |
| off/default/full IR template 导出 | 已实现，三平台代表目标验收完成 | 完整模板、语义必需模板保留、实际 consumer 与 Strong 归属 |
| profiling profile 与 runtime 采集 | 已实现，Darwin、GNU/musl、MSVC-UCRT、MinGW-UCRT/MSVCRT 原生验收通过 | 控制流/调用计数、运行指标、退出写出、release 无探针 |
| MIR cost 反馈及 `scoop-pgo` | 已实现，六目标闭环与本机性能对照完成；Long hash 回归及归一化边界已记录 | instrument → merge → optimize → run、配置复用及覆盖诊断 |
| 可选 profiling 系统库组件 | 已实现；本机完整 44 Cone 组件独立安装、训练、优化、移除与下载复用验收通过 | 两套 full 库、自动选库、独立下载安装和移除 |
| GC phase/slice 事件 | 已实现，六目标的真实增量周期、重启与截断验收通过；整体采集开销已测量 | 时间语义、采集开销和真实进程输出 |
| 使用材料及发行配套 | 规范、帮助、编程指导、打包、安装器、官网和验收记录已同步 | 规范、帮助、编程指导、安装器和官网组件选项一致 |

以下按提交顺序保留当时的验证范围；各批次提及的后续工作，其最终状态以上表及验收文档为准。

## 已完成

- 将评审设计及路线图入口纳入实现分支，包含按目标安装的 profiling 组件、Profiling/full 训练库和配套 Release/full 无插桩库。
- 核对本机 LLVM 22.1.8 的 instrumentation profile：`llvm-profdata` 可以合并，但不提供此类数据的 JSON/YAML show 输出，读取将使用同一 LLVM reader 的适配入口。
- LLVM/Darwin 原型验证：四线程各调用 100,000 次，两个实际执行记录各得到 400,000；重复 weak_odr 对象只计一次，未使用函数和计数记录均被裁剪。只验证 LLVM 接线，尚不代表 Scoop 的 MIR 位置映射完成；原型输入、对象、程序和 profile 已清理。
- Codegen 已提供 LLVM 22.1 原子计数 lowering 和 indexed profile reader。两个定向测试通过：在 Mach-O/ELF/COFF 的 LLVM IR 中确认 64 位原子更新及 ODR 分组；用真实 indexed 数据确认零计数、全零记录保留及无效输入诊断。后续 MIR/LIR 接线见下节，实际构建入口仍待接入。

### 逻辑位置与计数通道

- 新增纯数据 crate `scoop-profile`，保存正文身份、fingerprint、位置、物理记录映射及已解码频次。先在每个实际内联副本内配对调用点和入口，再汇总；零、缺失、部分覆盖、正文变化和计数溢出分别处理，不用其他副本的入口补分母。
- MIR 在首次可选 suspend 内联前固定原正文位置，普通内联保留调用点及 callee 来源；协程状态机将原入口计数保留在启动包装函数，恢复不重复计数。生成 callable 和 release hook 建立自己的计划，计数不增加 GC roots、safepoints 或 MIR cost。字符串融合与 tail forwarding 保持可用。
- LIR 只在 Generate 时发出计数，Codegen 将每个 callable 的实际位置映射为自己的 LLVM 记录。普通关闭模式不建立计划，也不额外扫描函数来建立计数记录。
- 跨 Cone 定向测试发现并修复了导入模板丢弃 CFG 调用点附加信息造成的 fingerprint 差异；普通正文及 private helper 在 provider/consumer 两侧的正文键和位置布局现已一致。
- 本机 workspace 格式化、Clippy 通过（仅既有警告）；共享反馈数据 5 项、MIR 217 项（含 4 项新位置/协程测试）、跨 Cone 匹配 1 项通过。正式对象发射定向测试在本机生成 Mach-O、ELF、COFF，确认计数随 callable 对象保留，独立元数据对象不持有计数。尚未通过实际 CLI 生成训练程序，Linux/Windows 原生训练随完整 profiling 大项验证。

### IR template 导出

- `scoop build` 与 `scoopc build` 已传递 `--export-ir off|default|full`，该选择进入正常编译更新输入并随 HIR 产物保存。compiler protocol 升至 32，cross-cone-interface 升至 95；旧系统库需要按新格式重建。
- `off` 保留语义必需的泛型、默认实参及其支持模板。`full` 保留普通和私有 callable、闭包、Context、异常、协程及普通构造器的完整模板，使用处才降低；普通 Strong 定义仍归 provider。修复了普通优化模板中的局部值未进入闭包捕获身份表的问题。
- 完整模板与当前 MIR 内联能力分开：具有模板的普通 class/secondary 构造器和 suspend callable 在当前内联器中报告 `unsupported-body`；可降低的闭包、Context 与异常正文照常进入成本评估。未导出正文报告 `no-mir-body`。
- macOS AArch64：workspace 格式化和 Clippy 通过（有既有 lint 警告）；HIR 模板消费 102 项、产物生成 121 项、协议 47 项、slib 269 项及缓存键 7 项通过。正式 CLI 的跨 Cone 内联用例通过 4 个 profile/export 组合；新的语义组合用例分别消费 default/full core，在 off/default/full 三档 provider 下均构建、链接和运行成功，验证闭包在 GC 后仍有效、Context 缺失异常、try/catch/finally、协程实际挂起/恢复、构建复用和优化报告。
- 使用本分支正式脚本准备本机 runtime，并实际构建 default/full core。观察到两种 core 产物分别为 12,224,056 与 12,541,705 字节；这是当前构建的体积记录，不是普遍开销承诺。
- Linux GNU x86-64（LLVM 22.1.2）与 Windows MinGW-UCRT x86-64（LLVM 22.1.8）：均使用独立源码目录和正式准备入口构建 runtime、Release/full core；消费 full core 的导出语义用例通过 3 档、15 次进程调用，普通跨 Cone 内联用例通过 4 个组合、16 次进程调用。Linux 另通过 4 项完整 HIR 导出定向单测。尚未用本项单独覆盖 Linux musl、Windows MSVC/MinGW-MSVCRT，不能由上述结果推定所有发行目标已验收。
- Windows 验收发现深层缓存目录中的优化报告发布失败：`tempfile::persist` 直接调用 Windows API，没有处理超长路径。发布前将已有目录解析为扩展路径后闭合；定向回归在原实现真实失败，修复后通过，随后上述 CLI 组合全部通过。本机格式化、`scoop` 定向 Clippy 和 2 项观察缓存测试通过。修复仅影响显式阶段输出复制，不增加普通缓存命中的内容检查。
- full 系统库构建暴露出 provider 存储访问器的导入边界：当前消费方不能直接表达外来 backing slot 和初始化门。保留完整模板，在使用时保留 provider 入口并报告 `unsupported-body`；不引入另一套存储 ABI。新增动态初始化及 image 值读写后，三档正式导出 fixture 在本机再次通过。
- 全量 full 系统库进一步覆盖普通模板中的全局赋值、ThreadLocal 写入、数组写入和导入类型的派生相等性。全局写入沿 provider setter（native 全局沿指针）还原，访问域使用原定义源码身份。定向 HIR 单测通过；正式导出 fixture 扩展上述行为后，三档、15 次进程调用全部通过，full 档确认这些正文实际进入优化评估。格式化与 HIR Clippy 通过（仅既有警告）。

### Profiling 构建与运行时

- build/run/test 与 scoopc 接通 Profiling：Release 优化、Generate 计数及默认 line-tables。采集配置经 compiler protocol 33、manifest 7 和普通缓存键传递；位置映射是 Generate 的正常结果，旁置保存，构建成功 JSON 报告实际映射及未覆盖依赖。命中缓存只确认所需结果存在，不读映射内容。
- 系统库在依赖解析前选择，PM protocol 8 接收上层选择的目录；训练默认选择 `profiling/instrumented`。维护入口 `--system-library-root` 与 `prepare_sysroot.py --profiling --profile-runtime ...` 生产两套 full 库，第三方 native 资源复用原目录。发布端与安装器接线继续下一批。
- Runtime index schema 9 明确采集配置及目标 compiler-rt archive。普通 runtime 不编译采集源文件；profiling runtime 在正常返回、显式 exit 或宿主 atexit 时一次写出 raw 计数、启动和 GC 总指标，标准流保留原用途。输出目录保留原生路径编码，每次运行独立命名，不能被 `%` 当作 LLVM 路径模板解释；I/O 失败报告但不改变业务退出码。
- LLVM profile counters/data/names 作为 callable 的 associated atoms 参与对象定义、fingerprint、重定位及 ODR 选择，不跳过验证；COFF 使用既有 COMDAT 关联。显式由 Scoop runtime 管理生命周期，避免每个对象产生独立的 LLVM runtime hook 函数。`scoopc profile-summary` 复用 LLVM reader 还原逻辑次数。
- 本机正式 runtime fixture 通过 7 次外部调用，内部运行多线程程序、正常返回与 `exit(7)`，合并后 main=2、choose=6、worker=8、leaf=8004、未执行递归函数=0。覆盖中文/空格/百分号路径、默认 cwd、写出失败、系统 core 计数、零记录、构建复用、Release 无探针，以及错误混入 Generate 库时失败且保留已有输出。
- 格式化及 workspace Clippy 通过（仅既有警告）；协议 48 项、slib 270 项、缓存 17 项、LLVM 对象/reader 3 项通过，三种对象格式均执行关联定义和重定位验证。Fixture runner 84 项通过（4 项平台条件跳过）。目前真实训练的原生目标为 macOS AArch64；Linux/Windows 原生覆盖随完整 PGO 批次进行。

### MIR 反馈消费

- Use 以相同逻辑位置读取配对计数，普通内联按动态热度排序、评分并选择既有增长档位；比例保留原始整数并截断到 [1/8, 32]，使用宽整数运算避免计数乘法溢出。字符串融合消费原基本块的配对频次，内联后仍沿原标记选择正文范围。
- 优化事件保存位置、原始次数、部分覆盖、实际比例和回退原因；正常 MIR 结果另记录去重后的匹配范围和实际反馈消费者。无正文等原有理由与数据状态分别报告，读取到数据不等于实际使用反馈。
- 本机格式化与相关 crate Clippy 通过（仅既有警告）；MIR 全部 220 项与共享反馈 5 项通过。新增测试确认同一 callee 的热调用先内联、零次冷调用保留，字符串单次拼接随动态块热度改变策略，零入口和正文变化回退且不声称使用反馈。CLI 端到端与性能对照尚待下一批。

### 反馈构建与缓存

- `scoop build` 与 `scoopc build` 接通 release `--profile-use`；默认选择组件中的 Release/full 系统库。compiler protocol 34 传递索引及 UseSummary。索引、计数、映射通过普通路径、长度、修改时间参与输入更新；缓存命中复用原反馈摘要，不重读 profile。该数据不进入语言实体身份或 `.slib` ABI。
- `--profile-use-required` 在最终链接/发布前要求源码构建图中至少一个正常优化决策实际使用反馈，不要求每个依赖均有动态数据。直接 scoopc 构建则检查当前 Cone；完全未使用数据、目标不符和损坏输入均保留已有输出。
- 本机正式 fixture 完成真实采集、合并、反馈构建和运行，确认生成程序无探针，优化事件包含原始次数。重复构建复用结果；追加计数与映射更新分别触发一次子编译。空映射允许低层静态回退，但 required 在缓存命中时仍拒绝发布。固定计数、业务输出、多线程和非零退出回归一并通过。
- 格式化及相关 Clippy 通过（仅既有警告）；协议 50 项、缓存 18 项、显式构建文件更新测试通过。同步编程指导中的低层反馈入口；独立命令、GC 时间线、组件全量安装及原生跨平台训练仍在后续批次。

### 独立 PGO 命令

- 新增 host binary `scoop-pgo` 和正常 commands 安装入口。`scoop pgo instrument/optimize` 复用主驱动参数声明，成功 build JSON 保存实际根输入、target、工具和解析后的设置；外部命令分发只对子进程提供配套 driver/sysroot 环境。
- instrument 默认 full，保存各次构建的独立映射，保持原始 argv、标准流和业务退出状态；支持追加训练和 `--no-run` 外部采集。optimize 自动选择 Release/full、要求实际反馈消费，并在配置核对和构建成功后发布新 binary。普通文件锁协调同一集合，合并按输入更新判断，损坏或缺失计数不回退到旧结果。
- 本机完整 profiling fixture 通过 9 次外部调用，PGO 子流程覆盖 16 次 command 操作及真实程序运行：确定计数、空参数与 exit(7)、换 cwd 的相对路径复用、无探针输出、缓存复用、外部训练、显式输出、编译失败、配置冲突、无数据、损坏和缺失输入。workspace 默认成员变化后仍训练并优化原先选定的 Cone。前两次新增测试仅因测试目录重名及样例缺 package 声明失败，修正后最终组合全部通过。
- 格式化与相关 Clippy 通过（仅既有警告），主驱动 CLI 41 项、共享选项/原生参数 2 项、PGO 参数 2 项、fixture runner 84 项（4 项条件跳过）通过。commands 打包布局 5 项通过，正式入口安装后已实际完成 instrument → optimize。同步命令帮助、tooling/scripts 说明、编程指导和 fixture 工具选择；GC 时间线、全量组件安装与跨平台验收继续后续批次。

### GC 事件时间线

- Profiling runtime 沿现有 world phase 和 collector 记录 cycle、STW slice、八种 phase。增量周期跨片保持身份，显式 full 或 fallback 重启时保留已付出的成本；暂停、停稳等待、周期墙钟和 worker 任务 CPU 分别表达。已有 phase 时钟直接复用，计数取既有标量差值，不在暂停中扫描堆、分配托管对象或写文件。
- 固定 native 缓冲发布不可变事件前缀，正常退出输出已发布部分；满缓冲累计 dropped，原 GC 总指标继续记录。普通 runtime 不编入事件存储和记录函数。规范与命令说明同步解释嵌套阶段及时间线截断。
- 本机四种 profiling runtime 和普通 runtime 均按正式入口重建。真实程序保持 262,144 节点链表，覆盖增量周期跨片和 full 重启，验证阶段时间区间、嵌套关系、逐片计数与累计 CPU/任务/字节一致；另一进程执行 1,100 次 full GC，记录 8,192 条事件并明确报告 608 条丢失，总 collection 计数仍完整。正式 fixture 4 次外部调用通过，已有增量 GC 定向正确性测试通过。C 格式化、toolchain Clippy 与 fixture Python Ruff 通过（仅既有 Rust 警告）。原生跨平台和整体采集开销继续成组验收。

### 原生验证中的对象修正

- Linux 实际 profiling core 构建暴露了 ELF ODR 计数组与正文 COMDAT 分离的问题。LLVM 的通用 lowering 为潜在跨函数内联保留独立计数组；Scoop 按 callable 发射对象，在 LLVM 优化完成后将三个 profile 关联段统一归入该正文的现有 COMDAT，保留正常对象检查和记录格式。
- 扩展生产对象回归覆盖 Darwin ARM64、GNU、musl、MSVC-UCRT、MinGW-UCRT、MinGW-MSVCRT 各自的 Strong／ODR 组合。修复前真实复现 GNU ODR 错误，修复后十二个组合的计数保留、对象定义及重定位检查通过。格式化及 codegen Clippy 通过（仅既有警告）；继续原生训练验收。
- Linux 原生链接进一步确认 compiler-rt 引用的 `__ehdr_start` 由最终 ELF linker 生成；将其加入已有 linker-defined 符号处理，不放宽普通未定义符号检查。格式化及 linker Clippy 通过（仅既有警告）。GNU 和 musl 各自使用原生 compiler-rt、四种普通和 profiling runtime，以及两套 full core/std，通过 GC 时间线与完整 profiling/PGO 正式 fixture：每个目标 2 项、13 次外部进程调用，包含多线程确定计数、数据追加与合并、配置复用、缓存更新、反馈优化、错误输入和无探针输出。

### 完整组件与发行配套

- 发布准备流程按目标构建 LLVM compiler-rt profile archive，并生成两套完整 full 系统库及四种 profiling runtime。独立组件只包含这些产物、训练映射和 compiler-rt 许可证，沿用普通 native 资源；配套 host 工具增加 `llvm-profdata` 与正常命令别名。manifest 将 profiling 作为可选目标组件。
- compiler-rt 构建已在 Darwin ARM64、Linux GNU/musl、Windows MSVC-UCRT、MinGW-UCRT/MSVCRT 使用真实工具链完成。MSVC 在私有 LLVM 构建输入中选择与 Scoop 一致的动态 CRT；musl 使用独立 libc 头文件；输入未变化直接复用。Python 格式化、Ruff 通过，发行布局、manifest、LLVM 工具与许可证定向测试 21 项通过（1 项条件跳过）。
- 本机完成全部 44 个系统 Cone 的 Profiling/full 和 Release/full 构建，连同 44 份训练映射及四种 runtime 打包成 1,220,497,376 字节的实际组件。使用隔离安装目录、本地 HTTP 发行源、真实 host 工具和 core/std 基础库，验证基础安装不下载 profiling、缺少组件时明确失败、独立安装、训练和反馈程序均输出 55、反馈程序无采集输出、移除后普通构建仍成功，以及再次安装复用下载缓存。首次测试包遗漏 `llvm-profdata` 动态依赖，补用正式依赖打包函数后整个流程通过；没有重新压缩完整库包。
- 安装器仓库 `scoop-install` 与官网仓库 `scoop-lang.org` 的 `codex/profiling-pgo` 分支已分别提交并推送可选组件支持：安装、移除、更新保留选择，bootstrap/publisher 识别目标组件。实际 HTTP CLI、bootstrap 和 publisher 定向测试通过；原始日志保存在仓库外。

### 性能与模板成本对照

- 新增同一 callee 两个调用点的可运行基准及复现说明。真实采集得到 500 万与零次调用，反馈把热位置从静态 `not-profitable` 改为内联，冷位置保留；额外启用训练中未执行的稀疏位置后结果仍正确。结合既有算术、字符串和 GC 程序完成预热及五对 AB/BA，所有运行标准输出一致。
- 测量覆盖普通 release、profiling、PGO release，以及相同 release 产物只更换 profiling runtime 的对照；记录内核与进程墙钟、CPU、RSS、代码及计数段大小，计时之外另核对分配／GC。调用点和字符串未显示明确净收益；Long hash 约慢 63%，已定位到嵌套模板的原函数归一化热度降低，导致 helper 停止内联。完整结果和适用边界见 [VALIDATION.md](VALIDATION.md)，不以实际读取或消费反馈代替性能收益。
- 独立新缓存、单 worker 对照 default/full core/std 生产与同一 consumer 编译，保留产物体积和墙钟／CPU成本；完整组件安装后约 1.96 GB。原始逐次记录在仓库外，未引入自动调参、通用性能预算或额外验收框架。

### Windows 观测脚本适配

- Windows 原生程序已经产生正确的调用计数和 GC 事件；定向验收暴露出两个脚本假设：短任务的 `GetThreadTimes` 差值可以为零，Python 文本标准输出默认转换为 CRLF。GC 仍严格核对逐片 CPU 与累计 CPU 相等，不再要求短任务计时必然大于零；四个探测脚本使用固定 LF 状态行。
- Ruff 格式化与检查通过。本机 GC fixture 4 次外部调用再次通过，Windows MSVC-UCRT 的真实 GC 时间线、增量重启和缓冲截断也通过；完整 PGO 流程及另外两种 Windows CRT 继续使用同一组正式 fixture。

### 最终原生验收与收尾

- Windows 三种 CRT 全部通过完整正式组合，各 2 项、13 次外部进程调用。MSVC-UCRT 的 GC／完整 profiling 用例分别为 7.86／365.61 s，MinGW-UCRT 为 29.19／409.82 s，MinGW-MSVCRT 为 29.04／407.59 s。初期缺少配套 `llvm-profdata`、混用 Clang driver 与 linker 名称属于测试环境准备问题；补齐真实 LLVM 22.1.8 工具并按 CRT 选择后，程序及计数验证通过。测试中的时钟精度和换行问题已按上节修正。
- 连同 Darwin 和 Linux GNU/musl，设计要求的六个原生目标均完成 instrument → merge → optimize → run，以及正常／非零退出、缓存更新、失败不发布和无探针输出。性能收益与采集开销只在本机测量，未把跨目标正确性结果扩大为跨平台性能结论。
- 实现、规范、编程材料、独立组件和外部发行配套均已分批提交推送，并逐轮合并 `origin/main`。清理一次性压缩包、安装目录、旧的失败尝试及不再使用的辅助工具；保留可复用的 sysroot／缓存、有效验证目录和仓库外原始记录。

### 保留默认内联、PGO 只追加

- 先修订实现规范，再将普通静态选择与反馈追加分为同一会话内的两个阶段，各最多三轮。共享原模板、摘要、ancestry 和 caller／region 累计增长；默认选择不读取 profile，也不冒充反馈消费者。反馈继续选择剩余调用和新暴露的机会，普通 release 阈值、字符串独立反馈及采集行为保持。
- 新增冷计数、嵌套 callee 归一化、默认与热候选争用累计增长，以及纯默认展开不计反馈消费者的回归；MIR-lower 共 223 项通过。真实优化报告确认 Long hash 的三层默认展开保留，热 `mix` 仍追加展开。三个程序的六种输入完成五对 AB/BA，同轮旧 PGO 的约 61% hash 退化不再复现；未训练 Boolean hash 仍慢约 19%，已确认反馈标记阻止 poll 分块的结构差异。具体数据和范围见 [VALIDATION.md](VALIDATION.md)。
- 按用户要求清理 workspace 既有 Clippy 警告，固定分块使用带长度的数组视图，保留尾部长度、顺序和格式检查；协议解码仅将可选 profile 摘要间接存储，不改变 wire 格式。平台测试辅助模块按真实调用平台编译，不关闭 lint 或删除测试。workspace 测试同时发现旧 runtime 缓存路径断言，改为检查构建实际返回的目录。macOS debug 编译器的 unwind 表尺寸警告按用户要求保留，未调整开发编译配置。
- 最终 `cargo fmt --all` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过；HIR、identity、MIR-lower 及文档测试合计 1,446 项通过，linker／toolchain 83 项通过。重新构建正式命令后，完整 profiling fixture 再次通过 9 次外部调用，覆盖训练、合并、反馈构建、运行、缓存更新、失败不发布及无探针输出。
- 全 workspace 执行曾得到 6,446 项通过、53 项失败；修正本分支 schema 演进后的两处测试期望，并补齐本机 LLD 搜索路径后，相关测试复验通过。剩余失败涉及旧 TaskContext 快照、core descriptor 测试辅助逻辑，以及本机 LLVM 缺少项目要求的 Windows statepoint 补丁；前两类在独立主线 checkout 的代表测试中同样复现。本轮未改这些既有合同和工具链，范围见 [VALIDATION.md](VALIDATION.md)。
- Windows 长路径发布和 ELF image header 解析已作为独立修复由 PR #131 合入主线。合入 `origin/main` 的 `306c818a04` 后，保留 PGO section 符号处理并复用主线通用回归；格式化及 workspace Clippy 零警告，dump 2 项、linker 18 项和完整 profiling fixture 的 9 次外部调用再次通过。

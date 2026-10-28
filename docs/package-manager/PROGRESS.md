# M63 包管理实施记录

状态：实施中。更新日期：2026-10-28。目标合同见[包管理规范](../specs/SCOOP-PACKAGE-SPEC.md)，阶段完成条件见[设计](DESIGN.md#5-实施步骤与完成条件)。普通 manifest 命令、跨根构建/测试调度与文档绑定隔离已接入 PM；低层清单迁移、发布及工具/发行收尾尚未完成。

## 已完成的功能

### 原始包声明与本地 workspace

- 新建独立 `scoop-pm` crate/binary，`scoop pm` 转发 argv、标准流与退出状态，可显式选择配套程序。PM 不依赖 LLVM 或编译 stage 实现。
- `new --workspace` 创建 virtual workspace；`members` 本地展开 root/virtual workspace、显式成员、default-members、`-p`/`--workspace` 选择及共享 Cone 字段/依赖。继承路径以定义处为基准，错误保留定义与引用位置。
- manifest 数据层新增版本要求和 path/Git/GitHub/registry/artifact 声明，复用既有 source/native 解析规则；版本要求覆盖 caret、tilde、交集、pre-release 与精确 build metadata。
- 此批提供本地声明和成员能力。现有构建尚未切到 PM；构建接入、发布及多根编译调度仍待完成，第一、二实施阶段未整体完成。

### 便携锁文件与共享缓存位置

- `Cone.lock` 格式、精确坐标/来源及每个 root/purpose/target 的闭合图已落地；读取检查缺节点、重复包、循环、测试边和不可达节点。
- 锁文件写入按文件锁串行合并所选解析单位，保留其他根/用途，移除已删除成员；失败不替换旧文件，未变化不重写。
- `deps` 可只读输出已锁定图及 JSON；`--why` 的声明原因输出待 metadata 接口接入。
- PM 缓存位置按 CLI、环境变量、`~/.scoop/packages/` 的优先级选择；registry/Git 目录键包含规范化来源。

### Git 与 registry 的共享获取

- 系统 Git 获取默认分支、branch/tag/rev，并按规范化仓库与完整 commit 共享 object 库和 checkout；lock 命中不刷新 ref，离线可复用已缓存源码或对象。
- registry 按需读取单包 JSON 索引，只下载选中的 tar.zst；首次下载校验 SHA-256，解包与 manifest/index 匹配成功后发布共享目录，保留已解析清单供后续准备使用。
- 普通文件锁串行发布同一版本，缓存命中不重读源码或归档计算摘要；来源/摘要冲突和下载、解包错误不覆盖已有包。使用 `ureq 3.5`、`tar 0.4` 和仓库已有的 `zstd 0.14`，不手写 HTTP/归档协议。
- 获取模块已连接 resolve/fetch/update。真实 CLI 覆盖 Git workspace、同仓库 path、submodule 的 gitlink 固定和 selector 变化；编译消费闭环仍待 driver 接入。

### 单版本解析与 resolve/fetch/update

- 使用 PubGrub 0.4 实现每个 root/purpose/target 的单版本求解，候选包含实际来源；锁定优先、全图回溯、pre-release/build metadata、撤回版本和根覆盖遵守同一约束。PM 的 Rust 最低版本为 1.92，driver 不链接该求解库。
- 独立 CLI 支持 resolve、locked fetch、选择性 update/precise 与 offline/frozen；输出版本/来源变化及相关要求。失败保留原 lock，多个根的结果在全部准备成功后一起发布。
- Git lock 增加声明 selector 记录，防止改 branch/tag 后误用旧 commit。有效 lock 直接匹配已有选择；首次 registry 解析获取最新索引，旧选择无法满足新要求时只刷新冲突涉及的来源再回溯。
- path、Git、registry、sysroot 和显式 artifact 均经同一来源目录进入求解；同包不同版本的独立根不合并。registry 在求解成功后才下载归档；同进程多个根共享已取得的 metadata。
- manifest、用户配置和产物摘要按路径、mtime、大小与工具版本缓存。预编译依赖保持精确坐标和 semantic fingerprint，目标不匹配或同坐标产物歧义明确失败；不把源码目录变成构建输出目录。
- 本批完成独立 PM 的解析/获取/lock 流程。driver 与 child 输入、普通构建多根调度、package/静态 registry 维护、LSP/formatter 和发行迁移仍待完成。

### prepare 与展开配置

- `scoop-pm prepare --request --response` 实现版本 1 的 JSON 接口，一次准备全部所选根；成功后原子交付响应，失败保留既有成功文件。共享类型放在 `scoop-protocol`，不引入 PM 实现依赖。
- 每个源码 Cone 交付已绑定精确依赖的生产配置，只有测试根携带测试配置；保留原清单文本与 span，继承字段/依赖定位到成员的继承声明。预编译节点复用现有 `.slib` 摘要；host path 使用无损载体，JSON 解码检查路径编码和格式。
- prepare 记录先检查配置、manifest、lock、artifact 及发现候选位置的更新状态，命中后直接交付已有展开结果。新增 workspace、用户配置、search-root 产物会失效；普通源码正文改变不重新选择版本。锁文件的观察记录在 lock 文件锁释放前取得，避免把旧图与并发更新后的 lock 状态配对。
- 两个项目取得同一 registry 版本时，交付相同共享源码目录；prepare 不复制源码到项目中。普通构建与 LSP 的 PM 接入仍待完成。

### 编译器已展开输入

- `scoop → scoopc` 协议升级到 18，移除 machine request 的 `ManifestRoot`，通过 `ExpandedCone` 交付已绑定配置、原始诊断文本和无损 host 路径；CBOR frame 与现有成功摘要继续复用。child 不重新读取或解析清单，独立单文件输入保持。
- parent 保留已加载的单 Cone 配置，native 准备及 child request 共享该结果。协议 reader 检查必需格式、引用、路径和诊断 span，不重放依赖求解。
- 真实 child 进程可消费含 workspace 继承的原清单与已展开配置，原始源码/清单保持在位。此批完成编译器交接边界，尚未把普通构建的图发现替换为 PM prepare。

### 普通构建命令接入 PM

- build/run/test/check/doc 的 manifest 入口通过配套 PM 一次准备所选成员，支持 workspace、`-p`、lock 路径、locked/offline/frozen 和共享包缓存。driver 投影精确节点与边，不重新读取清单、选择版本或获取源码；single-file 与 artifact-only link 保持原有入口。
- 成员产物按 `target/<target>/<profile>/<group>/<name>/` 发布，测试产物位于其 `tests/` 子目录。同名不同 group 成员不覆盖，workspace 成员子目录默认选择对应成员。
- prepare 失败通过已有结构化诊断格式交付，保留主清单 span 与相关声明；求解错误包含依赖路径和候选被排除的原因。文档消费展开后的 readme 元数据，资源路径以 README 所在目录为基准。
- 此批最初依次执行各根；跨根构建/测试调度由下一批接入。文档中的同身份不同绑定仍需独立表示，第二阶段尚未全部完成。
- fixture runner 新增多结果 JSON 模式，保留全部构建/检查/测试记录；正式 workspace 用例通过真实 Git 依赖验证自动 PM、成员输出、冻结复用、子目录选择与指定 README。

### 跨根构建与测试调度

- build/test 的所选闭包进入同一 ready queue，共享整次命令的 jobs 上限。各根保留独立节点、精确边、完成结果和诊断顺序；不按 ConeIdentity 合并图。
- 沿用完整编译键和键锁共享相同工作；正在等待另一根同键结果的节点让出队列，父进程继续接收子进程结果。失败后停止新任务并收完已启动进程，保留成功依赖缓存，按根/拓扑顺序报告错误。
- 各根分别发布和链接，workspace test 分别运行自己的测试程序。run 可在多个所选成员中选择唯一 executable，多于一个时要求使用 `-p`。
- 正式用例为三个程序选择两个依赖版本：相同绑定共享同一库产物，同身份不同绑定产生不同产物；程序运行和各自 Cone 测试验证实际绑定。改变 jobs 后 frozen 构建不启动编译子进程。

### 文档的独立依赖绑定

- 同身份源码在不同根中有不同源码/依赖输入时，分别建立文档 catalog；每个根的页面、搜索和跨 Cone 链接保持在自己的闭包内，共同首页连接各入口。相容输入继续使用既有共同站点。
- 文档编译仍按完整输入键共享；三个根中的两份相同绑定只编译一次。路由以根身份与清单位置确定，README 更新只重绘相应首页，冻结命中不编译或渲染。
- 比较绑定时也包含 prebuilt 节点的精确边；消费已有 `.slib` 的根不会把 re-export 链接到另一个根中绑定了不同上游版本的源码文档。
- 复用既有页面记录和整体原子发布，未另建文档产物缓存或语言身份。`--no-deps` 保留所选根；不同源码的同身份 Cone 也可分别展示。

### 本地格式化与清理

- `members --json` 使用共享的版本 1 类型和无损路径，失败复用 PM 结构化诊断。build 与本地工具共用成员选项；driver 不链接 PM 实现。
- `fmt` 支持 workspace/default-members、成员子目录、重复 `-p` 与跨目标生产/测试源码并集，文件去重与写回规则保持。源码选择直接消费展开字段，不构造虚假的已绑定依赖配置。
- `clean` 从任一成员定位 workspace 的 target；显式 target-dir 可在项目配置损坏时直接清理该目录。两者不求解或联网，不读取/写入 lock；显式单文件仍独立工作。

## 验证记录

- `cargo clippy --workspace --all-targets` 通过。
- manifest 77 项、PM workspace 8 项、PM 真实 CLI 2 项测试通过；PM 依赖树无 LLVM 或编译 stage 实现。
- Scoop CLI 参数转发 4 项、fixture runner 公共规则 58 项及 `pm-local-workspace` 正式 fixture 通过。后者验证真实 `scoop → scoop-pm`、根/成员选择与失败退出，不调用编译器。
- 锁文件批次：全 workspace lint、PM 15 项单元与 2 项真实 CLI 测试通过；正式 fixture 增加 deps 只读输出和缺失测试解析记录，8 次实际进程调用通过。
- Linux 独立环境下，第一批 manifest 77 项、PM 8 项单元及 2 项真实 CLI 测试通过，无 LLVM 依赖。
- 来源批次：全 workspace lint 通过；macOS 的 PM 回归与新增索引错误测试通过，Linux 的 PM 25 项单元与 2 项真实 CLI 测试通过。真实 Git 仓库覆盖 ref 更新/锁定/离线；本地 HTTP 服务覆盖跨项目并发共享、来源隔离、撤回版本、校验/解包失败和离线缺失，不删除源码或工具来验证缓存命中。
- 解析批次：全 workspace lint 通过；macOS/Linux 的 PM 34 项单元、2 项基础 CLI 和 6 项解析 CLI 场景通过，覆盖真实 Git/submodule、HTTP、workspace、override、生产/测试隔离、旧索引/commit 不满足新要求时的刷新，以及 artifact 精确语义与目标检查。正式编译链路验收尚待后续批次。
- prepare 批次：全 workspace lint 通过；macOS/Linux 的 manifest 77 项、protocol 41 项、PM 34 项单元、2 项基础 CLI 和 12 项解析/prepare CLI 场景通过。新增覆盖多根不同版本、继承诊断位置、测试根隔离、预编译摘要传输、共享 registry 路径，以及源码更新复用/新配置与 workspace/新搜索产物失效。真实非 UTF-8 目录由 Linux 覆盖；macOS 文件系统拒绝创建该名称，两端均验证路径载体的无损编码。
- 已展开输入批次：全 workspace lint 通过；macOS/Linux 的 protocol 42 项、wire 19 项、PM/manifest 回归及真实 `scoopc` child 交接通过；driver 调度、CLI 与依赖边界检查通过。macOS 三个正式 CLI 用例共 20 次进程调用通过，覆盖 native 更新、生产/测试隔离和文档缓存。测试前发现本地预编译库 ABI 过旧，只重新准备所需 core/test 库后闭合。
- 同批发现的 HIR 栈溢出已单独修复：候选状态在堆上保存，失败诊断借用状态。macOS 的调用/上下文推断 152 项回归通过；macOS/Linux 均通过 core 文档的三个目标检查，未扩大测试线程栈。
- 自动入口批次：workspace 用例在 macOS/Linux 各 6 次实际进程调用通过；macOS 的 Scoop library 98 项、CLI 33 项、fixture runner 59 项通过，既有构建/运行/测试/检查/文档用例及 36 个边界用例通过。新诊断合同的 PM 34 项单元、2 项基础 CLI、13 项解析/prepare 场景及 protocol 42 项在两端通过。显式 artifact 版本、合法 caret 语法及循环来源的旧测试意图已按新合同修正，完整诊断继续精确断言；其中 25 个 PM 诊断用例也在 Linux 通过，native 用例前显式准备所需公开头文件。
- 跨根调度批次：全 workspace lint、macOS 的 driver 99 项单元和两端 5 项调度专测通过。两端 workspace 构建/绑定用例各 16 次正式进程调用通过，覆盖三个程序、两个依赖版本、共享编译、运行选择及多根测试；macOS 既有 test-list 和 library 发布用例也通过。Linux 显式准备所需 test/runtime 资源后验收，复用既有 libunwind，不复制完整 sysroot。
- 文档绑定批次：两端文档库 24 项测试、共享/不同绑定用例各 15 次正式进程调用通过，检查实际页面链接、依赖版本、冻结复用、单页 README 更新和 no-deps。macOS 既有 5 组文档 CLI 回归通过，覆盖多版本路由、并发发布、失败保留与资源更新；Linux 基础和路由用例通过。检查脚本的 HTML 转义误判已修正并复跑闭合。
- 本地工具批次：全 workspace lint 通过；两端 manifest 77 项、protocol 42 项、Scoop CLI（macOS 33 项、Linux 34 项）及 PM 基础 CLI 2 项通过。两端既有 8 组格式化 fixture 与新 workspace 工具用例通过，后者使用不可连接的 registry 和保留的 lock 验证纯本地操作；macOS 完整的 Cone cache/clean 用例也通过。新用例的版本要求已改为合同要求的完整 SemVer 后复跑闭合。
- 文档混合来源补充：macOS/Linux 的绑定用例扩展为各 17 次实际调用，复用真实构建出的 `.slib` 与另一个上游版本的源码一起生成文档，验证独立 catalog、re-export 不错误连接以及全部本地链接；两端通过。
- 原始日志与临时工作文件在仓库外。按阶段清理 `target/debug/incremental`，保留当前二进制与依赖构建结果。

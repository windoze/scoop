# M63 包管理实施记录

状态：本地功能与静态 registry 消费实施中。更新日期：2026-10-28。目标合同见[包管理规范](../specs/SCOOP-PACKAGE-SPEC.md)，阶段完成条件见[设计](DESIGN.md#5-实施步骤与完成条件)。普通 manifest 命令、跨根构建/测试调度与文档绑定隔离已接入 PM；低层原始清单入口已退役；LSP 已接入，工具/发行收尾仍在进行。本轮包含静态 registry 索引读取与源码包下载，源码打包/发布流程单独留至下一阶段。

## 职责边界修订

PM 生成和维护 `Cone.lock`；`scoop` 消费锁定图及其本地投影，向 `scoopc` 传递单 Cone 的具体编译参数。`scoopc` 不读取包声明/lock，也不匹配版本要求。machine request 使用展开配置，旧的直接 manifest 入口已删除，正式 Cone 调用者已迁至 `scoop`，底层专测使用具体编译请求。默认 build/run 在缺少 lock 时由 PM 自动解析并生成，保留直接开始构建的使用方式。

## 已完成的功能

### 原始包声明与本地 workspace

- 新建独立 `scoop-pm` crate/binary，`scoop pm` 转发 argv、标准流与退出状态，可显式选择配套程序。PM 不依赖 LLVM 或编译 stage 实现。
- `new --workspace` 创建 virtual workspace；`members` 本地展开 root/virtual workspace、显式成员、default-members、`-p`/`--workspace` 选择及共享 Cone 字段/依赖。继承路径以定义处为基准，错误保留定义与引用位置。
- manifest 数据层新增版本要求和 path/Git/GitHub/registry/artifact 声明，复用既有 source/native 解析规则；版本要求覆盖 caret、tilde、交集、pre-release 与精确 build metadata。
- 此批提供本地声明和成员能力。现有构建尚未切到 PM；构建接入、发布及多根编译调度仍待完成，第一、二实施阶段未整体完成。

### 便携锁文件与共享缓存位置

- `Cone.lock` 格式、精确坐标/来源及每个 root/purpose/target 的闭合图已落地；读取检查缺节点、重复包、循环、测试边和不可达节点。
- 锁文件写入按文件锁串行合并所选解析单位，保留其他根/用途，移除已删除成员；失败不替换旧文件，未变化不重写。
- `deps` 可只读输出已锁定图及 JSON；`--why` 补充本地声明原因，见下文。
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

### 分离产物编译与最终链接

- `scoop build --emit-slib`通过同一PM/lock图编译并发布根`.slib`，保留真实Cone kind，不进入runtime准备或最终链接；`--tests`选择测试图及入口。默认路径、显式输出和workspace多根沿用统一布局。
- 输出意图不进入编译键；后续普通build/test复用同一产物完成链接和运行。此能力用于将旧低层manifest调用迁回scoop，不给scoopc增加清单解析入口。
- macOS/Linux的正式用例各6次调用通过，覆盖workspace继承、范围依赖、锁文件、普通与测试产物、无重复编译的链接运行及多根输出。全workspace lint、CLI测试（34/35项）和fixture runner 59项通过；runner已识别新的`artifact`结果类型。

### 编译器原始清单入口退役

- 删除 `scoopc` 请求中的原始 manifest 分支及清单读取；machine request 只消费展开后的配置和具体 direct/support 产物。独立 build/check 只接受显式 `.scoop`，目录或 `Cone.toml` 给出使用 `scoop` 的提示；测试用途由 `scoop test/check --tests/build --emit-slib --tests` 传递。
- 原有底层编译专测改用显式已展开请求；正式产物、ABI、native 链接及测试用途 fixture 迁至 `scoop build --emit-slib`。过期依赖专测显式绑定原 `.slib`，保持先检查依赖再查 native 符号的规则。
- 默认 `scoop build/run` 在没有 lock 时自动调用 PM、成功生成 lock 后继续执行，无需手动 resolve；已有选择优先复用。首次构建/运行及 locked/frozen 缺锁拒绝分别验证，单文件模式保持独立。



### 只读 metadata

- `scoop pm metadata [--offline]` 返回与 prepare 相同的版本 1 项目模型，支持成员、用途、目标及本地输入选择；始终要求有效 lock，不生成或更新依赖选择。`--json` 错误保留现有结构化诊断。
- 命中准备记录时直接复用；离线只使用已有 Git checkout / registry 源码目录，缺少时提示 fetch，不因本机已有 objects 或归档而隐式展开。普通 fetch 的离线展开能力保持，在线 metadata 可获取已锁定的缺少来源。
- 本批提供 LSP 所需的只读接口；语言服务接入仍待后续完成。


### 锁定依赖原因说明

- `deps --why group:name` 按所选 root/purpose/target 显示实际锁定坐标和来源，列出全部入边的版本要求、声明位置及从根到各入边的一条确定路径。继承声明保留定义/引用两处，测试边单独标记；独立根不会合并成一个版本解释。
- 人类文本与 JSON 共用同一解释数据，Git 来源保留 lock 中的 commit/selector；只说明当前选择，不保存历史求解轨迹或查询最新版本。普通 deps 继续只读 lock，why 在锁定模式检查本地声明，缺少远端来源提示 fetch，陈旧 lock 提示 resolve；不隐式下载、展开或写 lock。

### 本地依赖清单编辑

- `scoop pm add/remove` 编辑唯一选中的成员，支持生产/测试用途、来源替换与 workspace 继承；path/artifact/本地 Git 路径从调用 cwd 换算为相对成员的路径。
- 使用已有 TOML 编辑库保留无关内容、键格式与条目注释；普通值和独立依赖表都可更新。先复用包声明及继承规则验证，再原子写入；相同结果不重写，不解析或修改 lock，不访问网络。
- macOS/Linux 的 PM lint、34 项单元、2 项基础 CLI、3 项编辑和 17 项解析/准备场景通过。编辑用例覆盖注释、无变化复用、非法声明失败不写入、多成员选择、路径基准、继承和来源替换；首轮发现的键注释丢失及独立表格式问题已修复并复跑闭合。

### 编辑器清单输入边界

- `members --overlays-stdin` 与 `metadata --offline --overlays-stdin` 接收版本化的未保存清单文本，复用 PM 的解析、workspace 继承和锁定匹配；新增但未保存的成员清单可以参与本地发现。
- 覆盖只存在于本次请求，不写回文件、不改 lock、不发布准备记录；关闭覆盖后恢复磁盘及既有缓存结果。metadata 强制离线，不把输入文本作为触发下载或更新依赖的理由。
- macOS/Linux 的 workspace/PM lint、protocol 43 项与 PM 回归通过，新增 2 个真实 CLI 场景覆盖 workspace/path 覆盖、依赖约束冲突、诊断位置、新成员及无 lock；测试中的继承字段和源码选择语法已按现有合同修正并复跑闭合。LSP 消费这条接口仍在实施中。

### LSP 消费锁定项目模型

- 分析宿主通过配套 PM 的 members/metadata --offline 消费精确图，移除原来的清单依赖查找；每个 workspace 成员独立取得实际绑定，根成员不受 default-members 排除。缺少适用 lock 的成员保留源码语法与 workspace 符号，不用空依赖图替代完整项目。
- 未保存清单沿 overlays 协议进入 PM；Cone.lock 变化参与监听。sysroot 源码导航只使用本地 members 的源码选择，依赖语义仍来自预编译产物。新增配套 PM、包缓存和 lockfile 配置，并同步 VS Code 配置及说明。
- macOS/Linux 的 manifest、analysis、LSP Rust 回归与 lint 通过。真实 LSP 会话覆盖源码/预编译依赖、未保存文本、安装源码导航，以及 workspace 的 Git/path 不同版本、非默认根、缺锁成员和 lock 不变；Mac 原会话与新增组合场景分别运行，Linux 同一正式 fixture 的三个进程场景全部通过。扩展定向 lint/类型检查及单元测试通过；全目录 Biome 检查会扫描现有示例 target 产物，因此使用变更文件检查。未运行 VS Code GUI 宿主验收。

### 静态 registry 读取协议与构建消费

- 固定格式 1：HTTP(S) base 下的 config.json 和 packages/<group>/<name>.json，版本记录提供完整版本、kind、生产依赖、archive URL、SHA-256 与 yanked；归档为根含独立 Cone.toml 的 tar.zst。明确相对 URL 基准、必需字段、不可变版本、HTTP 错误、lock/离线读取行为与源码包消费要求。
- 服务端只需托管静态文件；PM 自行求解并仅下载选中版本，共享源码缓存沿既有获取实现。package/publish 与索引维护留至下一阶段。
- macOS/Linux 新增正式 fixture 均通过：使用 Python 标准静态文件服务器，索引与归档在请求前生成；两个有传递依赖的项目真实编译/运行，验证相对及绝对归档 URL、最新兼容版本、一次共享下载、未选候选不下载、有效 lock 不查远端、显式 update 与 frozen 运行。测试输入压缩使用 zstd CLI；首轮测试代理环境及源码 package 声明问题修正后通过，不增加服务端接口。

### 配套程序发行与清单迁移

- 工具链组装构建并复制 scoop-pm，许可证遍历加入 PM 依赖（包括 PubGrub 的 MPL-2.0）；安装说明与开发构建命令同步为五个配套命令。
- 两个遗留产物消费清单改为显式精确版本；产物摘要更新专测转为观察实际承担摘要读取的 PM 缓存，保留真实产物、修改/touch、更新记录和编译调用次数验证。双来源冲突的 negative 清单保持原测试意图。
- macOS/Linux 发行脚本 7 项测试通过，覆盖 PM 复制、可执行位和独立依赖许可证收集。macOS 的 artifact-summary（16 个进程步骤）与 generator prebuilt（8 个步骤）通过；未重新构建完整 LLVM 发行包。

### Driver 原始清单入口退役

- driver 的库入口只接收 PM 已检查的精确图或独立单文件；删除原始清单解析缓存、源码依赖递归查找及重复的来源选择。普通命令和库调用共用相同投影与调度入口。
- PM 图直接建立编译顺序及 direct/support 产物关系，不重放版本约束、图闭合和 core 选择；单文件头部的精确产物发现保持。源码图测试经真实 PM 生成输入，独立单文件继续覆盖原产物查找。
- 全 workspace lint 与 macOS driver 110 项测试通过；Linux library 76 项通过，其中 musl 用例在显式准备对应 libunwind 后单独复跑。完整 CLI fixture 回归发现旧裸产物声明和开发 sysroot 资源需要迁移/更新，正在闭合。

### 既有产物 fixture 清单迁移

- 将 2477 份实际产物消费者清单的裸版本改为精确版本及显式 artifact 路径；路径按各用例的真实复制位置、产物输出与搜索根确定，保留传递产物的精确查找。专门验证非法字段/版本、缺少 registry 和生产/测试隔离的声明保持测试意图。
- 直接读取仓库源码的正式命令用独立测试工作目录保存 lock；开发 sysroot 准备也将 lock 放到维护工作目录，避免测试/发行准备修改随包源码。
- 全部变更 TOML 解析及差异检查通过，Python 脚本格式化/lint 与 7 项发行脚本测试通过。macOS 首批 7 组正式 fixture（8 个变体、45 个进程步骤）通过，覆盖产物传递、ODR 冲突、profile 替换、native 要求及精确语言诊断；全量回归继续完成。

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
- 编译器入口退役批次：全 workspace lint 通过；macOS/Linux 的请求与 CLI 15 项、底层 core/依赖 preflight 12 项及真实展开配置 child 用例通过。两端共 59 个相关文件 fixture 全部闭合（Linux 本批 273 次进程调用），覆盖首次 build/run 自动创建 lock、locked/frozen 缺锁拒绝、生产/测试隔离、core 自举、精确产物及 native/链接错误顺序；迁移中发现的旧声明和诊断期望已修正并仅复跑受影响用例。
- metadata 批次：workspace lint 与两端 PM 34 项单元、2 项基础 CLI、16 项解析/准备场景通过。真实命令覆盖缺锁/陈旧 lock 拒绝、共享准备记录、保留 lock、Git objects-only 与 registry archives-only 的离线读取边界，以及显式 fetch 后读取和在线获取；源码、归档和工具保持在位。首轮复用断言的测试 sysroot 路径与实际 CLI 默认值不同，改为显式共同路径后闭合。
- deps --why 批次：workspace lint、两端 17 项解析/准备 CLI 场景与既有本地 workspace fixture（各 8 次进程调用）通过。验证不同根版本、两条传递约束、继承位置、测试边、root/不存在包、缺锁/陈旧锁、Git selector，以及 objects/archives-only 缓存不被隐式展开；lock 内容和修改时间保持。
- 原始日志与临时工作文件在仓库外。按阶段清理 `target/debug/incremental`，保留当前二进制与依赖构建结果。

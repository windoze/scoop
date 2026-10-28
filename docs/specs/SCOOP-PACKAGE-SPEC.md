# Scoop 包管理规范

状态：M63 本地功能与 registry 消费实施中；源码打包/发布移至下一阶段。完成范围见[实施记录](../package-manager/PROGRESS.md)。更新日期：2026-10-28。

本规范规定独立 `scoop-pm` 的 workspace、依赖来源、版本解析、`Cone.lock`、源码 registry 及其与构建 driver 的接口。实现步骤与设计取舍见[包管理设计](../package-manager/DESIGN.md)，调度、编译和链接继续遵守[编译器与产物规范 2.7～2.8](SCOOP-IMPL-SPEC.md#27-scoop-与-single-cone-scoopc)。

Scoop 尚未发布。本次直接重定义 `Cone.toml` 的初始 schema，与仓库内清单、工具和测试一起实施，不提供旧清单兼容层。以下是完整目标合同，不表示当前命令已经支持全部能力。

本轮交付 workspace、path/Git/sysroot/artifact 依赖、版本与 lock、本地清单编辑、构建与 LSP 接入、配套工具发行，以及第 8 节静态 registry 的索引读取和归档下载消费。第 9 节源码打包/发布工具留至下一阶段；本轮以预先生成的静态文件验收消费闭环。

## 1. 职责与交付边界

| 组件 | 负责的输入与输出 |
| --- | --- |
| `scoop-pm` | 读取 manifest/workspace；选择根；解析版本；获取 Git/registry 源码；维护 `Cone.lock`；提供精确图与本地输入；打包源码及维护静态 registry |
| `scoop` | 将 `scoop pm …` 转发给配套 PM；构建命令取得已锁定的图后，判断构建更新、调度单 Cone 编译、链接与运行 |
| `scoopc` | 只接收 `scoop` 传入的单 Cone 编译配置、源码与精确 direct/support `.slib`；完成编译，不读取包声明或 lock |
| `scoop-link` / runtime | 消费既有精确产物闭包；不解释版本范围、workspace、Git 或 registry |

`scoop-pm` 是独立可执行程序和可独立测试的 Rust crate，不依赖 LLVM 或编译 stage 实现。driver 不链接 resolver、Git 或 HTTP 客户端。共享代码限于 manifest 数据、精确图/请求的数据格式和已有产物 reader；不另建任务 DSL、通用插件系统或服务进程。

`scoop pm <command> …` 等价于 `scoop-pm <command> …`，转发剩余 argv、退出状态及标准流。默认使用 `scoop` 同目录的配套程序，允许 `--package-manager <path>` 显式选择。`pm` 是明确的配套入口，优先于现有 `<SYSROOT>/commands/` 外部命令查找；该既有机制保持，不增加新的插件框架。

包管理分发单位仍是 Cone，语言中的 `package` 仍只是 namespace。首版 registry 分发源码，不分发跨工具链通用的预编译包；现有显式 `.slib` 与 sysroot 消费继续保留。

## 2. Manifest 与版本要求

### 2.1 Cone 声明

`schema = 1` 必需。包含 `[cone]` 的清单定义一个 Cone，`group`、`name`、`version`、`kind` 必需，kind 为 `library` 或 `executable`。坐标语法和完整 `ConeIdentity` 仍由[语言规范 12.2](SCOOP-SPEC.md#122-cone-identity与conetoml)定义；Cone 自己的 version 必须是精确 SemVer。

```toml
schema = 1

[cone]
group = "dev.example"
name = "server"
version = "0.1.0"
kind = "executable"
license = "MIT"
repository = "https://github.com/example/server"

[registry]
default = "https://packages.example.org/"

[dependencies]
"org.example:json" = "1.4.0"
"dev.example:model" = { path = "../model", version = "0.3.0" }
"org.example:http" = { github = "example/http", tag = "v1.2.0", subdir = "cones/http", version = "1.2.0" }
"org.vendor:codec" = { version = "=2.0.1", artifact = "../artifacts/codec.slib" }

[test-dependencies]
"scoop:scoop.test" = "=0.1.0"
```

`license`、`repository`、`description`、`readme` 为可选发布/文档元数据；license 为 SPDX 表达式文本，repository 为 URL，readme 为相对路径。文档使用显式 readme；省略时查找 Cone 根下的可选 `README.md`。显式文件缺失报错，其相对资源链接以该文件所在目录为基准。它们不改变 ConeIdentity。未知 semantic 字段及错误类型报 manifest 错误。

### 2.2 版本语义

依赖 key 是完整 `group:name`，不支持 alias。版本要求采用以下闭合语法，数值部分必须完整写出 `major.minor.patch`：

| 声明 | 允许范围 |
| --- | --- |
| `"1.4.0"` 或 `"^1.4.0"` | `>=1.4.0, <2.0.0` |
| `"0.3.2"` 或 `"^0.3.2"` | `>=0.3.2, <0.4.0` |
| `"0.0.3"` 或 `"^0.0.3"` | `>=0.0.3, <0.0.4` |
| `"~1.4.2"` | `>=1.4.2, <1.5.0` |
| `"=1.4.2"` | 只有该完整版本 |
| `">=1.4.0, <1.8.0"` | 逗号分隔比较项的交集 |

比较项支持 `>`、`>=`、`<`、`<=`、`=`。首版没有 `latest`、通配符、部分版本、逻辑或及任意版本表达式。非精确范围按 SemVer precedence 比较。pre-release 候选只有在同一要求中显式出现相同 major/minor/patch 的 pre-release 比较项时才可选；例如 `^1.4.0-rc.1` 可接受 `1.4.0-rc.2`、`1.4.0` 和兼容稳定版，不接受 `1.5.0-rc.1`。

合法 build metadata 继续参与 ConeIdentity。带 build metadata 的版本要求只允许 `=`，按完整文本匹配；不含 metadata 的 `=1.4.2` 也只匹配该完整版本。其他范围比较忽略候选的 metadata；同 precedence 的多个候选在锁定版本之后按完整版本 UTF-8 升序选择，确保行为确定，不赋予 metadata 新的版本高低语义。

`[dependencies]` 是生产依赖；`[test-dependencies]` 仅加入所选测试根，依赖本身仍以 Production 构建。同一 manifest 两张表不得重复声明同一 key。普通 build/run/check/doc 不解析或获取测试依赖，不因只改变测试配置而更新生产解析或构建结果。首版不提供 optional/build dependency、feature、依赖平台条件或安装脚本。

## 3. Workspace

### 3.1 成员与命令根

```toml
schema = 1

[workspace]
members = ["apps/server", "apps/cli", "libs/model"]
default-members = ["apps/server"]

[workspace.cone]
group = "dev.example"
version = "0.1.0"
license = "MIT"

[workspace.dependencies]
"dev.example:model" = { path = "libs/model", version = "0.1.0" }
"org.example:json" = { version = "1.4.0", registry = "https://packages.example.org/" }
```

成员清单：

```toml
schema = 1

[cone]
group = { workspace = true }
name = "server"
version = { workspace = true }
license = { workspace = true }
kind = "executable"

[dependencies]
"dev.example:model" = { workspace = true }
"org.example:json" = { workspace = true }
```

`[workspace]` 可与 `[cone]` 共存；根 Cone 自动成为成员。没有 `[cone]` 的 workspace 是 virtual workspace，不能直接编译。members 是相对根目录的显式成员目录列表，首版不支持 glob、隐式遍历或目录外成员；普通 symlink 仍按实际 I/O 使用，不审计物理来源。重复成员或同 workspace 中重复 `group:name` 报错。嵌套 workspace 独立，不能同时成为外层成员。

default-members 必须是成员子集；省略时，有根 Cone 则选根 Cone，virtual workspace 则选全部成员。从成员内部调用默认只选该成员，从 workspace 根调用使用 default-members。`--workspace` 选全部成员，重复 `-p group:name` 选指定成员；两者互斥。显式目录或 `Cone.toml` 使用该位置；省略输入时从 cwd 向上定位最近清单，再查找显式列出该成员的最近 workspace。仅位于workspace目录树中不自动获得成员身份；未列出的Cone独立处理，要求继承时则报未加入workspace。遇到独立 workspace 边界即停止，不把不同 workspace 的配置叠加。

`run` 必须最终选中唯一 executable；多个根的 build/test/check/doc 共用调度并分别报告结果。单一 `-o` 不接受多个根。成员的默认输出放在 `target/<target>/<profile>/<group>/<name>/` 下，测试位于其 `tests/` 子目录，避免不同 group 的同名 Cone 覆盖。源码依赖构建结果仍使用共享 Cone cache。

文档也保留各根的实际绑定。同身份源码在不同根中绑定不同依赖时，按根建立独立页面和搜索范围，由共同首页连接；输入相容时保留共同站点。文档编译按完整输入键共享，页面范围不进入语言身份，详见编译器与产物规范 2.7 的文档合同。

### 3.2 显式继承

可继承的 Cone 字段仅为 group、version、license、repository；name、kind、源码/native 清单不继承。依赖的 `{ workspace = true }` 必须是完整 value，不允许再混入 version/source 字段；引用 `[workspace.dependencies]` 同 key 的完整声明。该表不自动添加依赖，也不覆盖传递依赖。成员依赖彼此必须使用显式 path（可经共享依赖声明）；同名 registry 依赖不自动变成本地 member。

继承值中的相对路径以定义该值的清单为基准；成员自己的相对路径以成员为基准。PM 展开配置时保留定义和引用位置，用于诊断。成员要求继承但未找到所属 workspace 或对应键时直接报错。Git 仓库内的成员使用其所属远端 workspace 展开，不采用消费者的 workspace 字段。

### 3.3 解析隔离

解析单位是 `(root member, purpose, target)`。每个单位产生完整依赖闭包，同一闭包中每个 `group:name` 只能有一个版本和一个实际来源。不同 executable 或独立 library 根可以使用不同版本；一次 `--workspace` 不把这些闭包强制合并。

例如 server → json 1.x，cli → json 2.x 可以同时存在；若 server 同时依赖两个分别要求 json 1.x 和 2.x 的库，则解析失败。生产和测试根分别锁定，测试闭包必须整体满足生产与测试约束，不强制复用另一个生产闭包的版本。profile 不改变求解结果；target 作为解析键用于实际 sysroot/prebuilt 选择。

## 4. 依赖来源

### 4.1 来源选择

声明在 workspace 展开后恰好属于以下一种来源：

| 来源 | 声明与行为 |
| --- | --- |
| Path | `{ path = "../lib", version = "1.2.0" }`；version 可省略，版本由所选本地 Cone 确定；有要求则必须满足 |
| Git | `{ git = "https://host/repo.git", … }` 或 `{ github = "owner/repo", … }`；见 4.2 |
| Registry | 字符串要求，或 `{ version = "1.2.0", registry = "https://host/index/" }`；version 必需 |
| Artifact | `{ artifact = "../lib.slib", version = "=1.2.0" }`；必须为精确要求，消费现有 `.slib` |
| Sysroot | 无显式来源的 `scoop` group 依赖；使用所选 target 的已发布系统库，版本必须满足声明 |

path、git/github、registry、artifact 互斥。registry 使用规范化的 HTTP(S) base URL：统一 scheme/host 大小写、默认端口与结尾 `/`，不接受 query/fragment 或内嵌凭据，不按重定向猜测其他 registry 等价性。首版直接使用 URL，不需要全局 registry 别名服务。

未显式指定 registry 的普通依赖依次使用声明所在 Cone 的 `[registry].default`、所属 workspace 根的该字段、调用者配置文件中的默认值。用户配置位置为平台配置目录的 `scoop/config.toml`，使用同一 `[registry] default` 字段。没有默认值则报错，不假定尚未部署的官方站点。为保证远端清单可移植，Git Cone 的普通 registry 依赖必须在自身/所属 workspace 中固定 URL；发布的 registry 包必须展开全部 URL，不采用消费者配置。用户配置不改写已明确的来源。

无显式来源的 `scoop` group 继续使用显式 artifact search roots，找不到满足要求的候选时使用 `<sysroot>/lib/<target>/<name>.slib`，不去默认 registry 搜索。候选必须满足版本要求，且是目标兼容的 library；不同版本参与普通版本选择，同坐标的多个不同内容候选报歧义。隐式 core 固定为 `scoop:scoop.core:0.1.0`，全图复用已选显式 core，否则使用 sysroot。显式来源失败不切换其他来源。

非 `scoop` group 的普通 manifest 裸依赖改为 registry；`--cone-path` 不再充当其多源回退。单文件模式、artifact-only 链接和已有 `.slib` 传递依赖仍按原有精确 coordinate 搜索规则工作。

### 4.2 Git 与 GitHub

`github = "owner/repo"` 展开为 `https://github.com/owner/repo.git`，其他 Git 能力相同。`git` 接受系统 Git 支持的远端 URL；需要本地源码开发时使用 path，测试可使用本地 Git URL。tag、branch、rev 三选一；均省略时首次解析采用远端默认分支 HEAD。rev 接受 Git 可解析的 commit 标识，lock 一律写完整 object ID，不能假设所有仓库固定使用 40 位 SHA-1。

`subdir` 默认为 `.`，是仓库内相对 Cone 目录；禁止空路径、绝对路径或词法越出仓库的 `..`，不自动猜测同名包。选中目录必须有匹配 key 的 Cone.toml。可选 version 只检查该 commit 的 manifest version 是否满足要求，**不据此枚举或猜测版本 tag**。首次 resolve 和 update 解析 ref；日常构建使用 lock 的 commit，不刷新 branch/tag。

PM 调用系统 Git，复用 SSH、credential helper 和用户 Git 配置；不要求 GitHub API 或另建登录体系。获取选中 commit 及构建需要的 submodules；submodule 的内容由该 commit 的 gitlink 固定，不跟随分支。Git LFS 不作为首版附加协议，缺少实际构建文件按普通输入错误报告。

仓库中 path 依赖按声明位置展开；只要词法仍在同一 Git checkout 内，lock 将其记为相同 repository/commit 加不同 subdir。远端包不能通过 path 引用 checkout 外的本机目录。一个仓库可以提供多个 Cone，共用获取结果；同一闭包中，同 key 的不同 commit/subdir/来源仍是冲突，不能用相同版本号掩盖。Git URL 仅合并可明确规范化的同写法，不把 SSH/HTTPS、fork 或重定向自动判断为同源；需要替换时用 4.3。

### 4.3 根覆盖与预编译依赖

workspace 根（独立 Cone 即自身）的 `[overrides]` 允许按 key 指定一个 path、Git、registry 或 artifact 来源，来源字段规则与依赖一致。覆盖只替换来源，所有原有显式版本要求继续成立；override的version可省略（artifact仍要求精确版本），给出时进一步收紧约束。原path/Git声明未写version时不另外从被替换源推导一个版本要求。仅选中的根workspace的overrides生效，远端依赖自身的overrides和lock不影响消费者。覆盖在求解前一致应用于整个选中闭包，不按发现顺序覆盖。

```toml
[overrides]
"org.example:json" = { path = "../json-fix", version = "1.4.0" }
```

没有覆盖时，同 key 的不同来源是来源冲突；即使版本相同也不能静默选一个。来源不进入 ConeIdentity，但进入 lock 和构建更新输入。

`.slib` 内已绑定的 dependency coordinate、semantic fingerprint 和 ABI 要求保持精确；不能因新 manifest 范围允许而将其依赖换绑到另一个版本或语义不同的产物。PM 可用现有 reader 提取精确依赖以参与求解；实际产物加载/链接继续完成既有兼容性检查，复用已读结果。若新选择需要重建上游，必须存在明确的源码来源；不能假装仅更新 lock 就使旧 `.slib` 兼容。

## 5. 版本解析与诊断

求解变量是当前解析单位内的 `group:name`，取值为一个版本及实际来源。边携带要求与清单位置。path、Git ref 和 artifact 通常只提供一个候选；registry 提供已知可用版本。最终图必须无环，全部非根节点为 library，并满足所有版本和精确产物约束。

优先级为：有效 lock 中仍可用的选择优先；需要新选时，稳定版本按 SemVer 降序尝试；显式允许的 pre-release 参与对应排序。同级候选按 2.2 确定顺序。变量选择和输出排序必须确定；求解器可使用成熟的 PubGrub 或等价冲突回溯实现，算法不进入产物 ABI。

“最新兼容”指全图约束下偏好较新候选，不是先对每个包取最大值后遇到冲突就结束，也不承诺同时最大化互相制约的每个包。为使全图可解必须回溯。例如 app 要求 A `^1.0.0` 与 B `^1.0.0`；A 1.2.0 要求 B 2.x，A 1.1.0 要求 B 1.x，应选 A 1.1.0，不能强制 B 2.x 或立即报错。

`update` 忽略所选包的旧版本偏好并尝试更新；其他选择尽量保持，必要的传递变化允许发生并显示原因。`--precise` 临时固定一个 registry 精确版本或 Git 完整 commit，仅写 lock，不改变 manifest 约束。新版本仍必须满足原图要求；不存在解时保留旧 lock。

失败诊断至少包含冲突 key、两条根到依赖的路径、各要求和声明位置、已知候选为何不满足；来源冲突同时显示来源。`deps --why group:name` 可解释成功选择受哪些边约束。不要提供 nearest-wins、无条件 highest-wins 或忽略要求的 force 模式。

## 6. Cone.lock

### 6.1 存储与结构

workspace 在根目录共享一个 `Cone.lock`；独立 Cone 放在自身目录。建议应用和库仓库都提交，库被其他项目消费时忽略其 lock。文件记录每个解析单位的完整精确图；可同时包含不同程序的不同版本，也可只包含已请求的根/用途/target，不预先求解全部成员或测试依赖。

以下是一个完整的小型解析记录示例；源码和边的顺序均按 canonical coordinate 排列：

```toml
format = 1

[[resolutions]]
root = "apps/server"
purpose = "production"
target = "aarch64-apple-darwin"
root-coordinate = "dev.example:server:0.1.0"

[[resolutions.nodes]]
coordinate = "dev.example:server:0.1.0"
kind = "executable"
source = { kind = "path", path = "apps/server" }
dependencies = ["org.example:json:1.4.3", "scoop:scoop.core:0.1.0"]
test-dependencies = []

[[resolutions.nodes]]
coordinate = "org.example:json:1.4.3"
kind = "library"
source = { kind = "registry", registry = "https://packages.example.org/", archive = "archives/org.example/json/1.4.3.tar.zst", sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" }
dependencies = ["scoop:scoop.core:0.1.0"]
test-dependencies = []

[[resolutions.nodes]]
coordinate = "scoop:scoop.core:0.1.0"
kind = "library"
source = { kind = "sysroot", name = "scoop.core" }
dependencies = []
test-dependencies = []
```

每个解析记录包含 root、purpose（production/test）、canonical target、root-coordinate 和闭合 nodes；同 key 只能一条记录。root是相对workspace根（独立Cone即自身）的成员目录，根Cone用`.`。每个 node 必需 coordinate、原 manifest kind、source、dependencies、test-dependencies。test-dependencies 仅在 Test 根允许非空；effective executable kind 由用途决定。每个边值必须引用本记录中唯一节点，不能跨解析单位借用节点；不能有多余不可达节点。implicit core 也显式记入 dependencies，core 本身不依赖自己。

source是带kind的封闭记录，各分支必需字段如下，不接收其他分支字段：

| kind | 字段 |
| --- | --- |
| `path` | `path`，相对lock所在目录的Cone路径 |
| `git` | `git`，规范化仓库URL；`commit`，完整object ID；`subdir`，仓库内Cone目录；`references`，本闭包指向该节点的已解析 Git selector 列表 |
| `registry` | `registry`，base URL；`archive`，归档URL（可相对base）；`sha256` |
| `artifact` | `path`，相对lock所在目录的`.slib`路径 |
| `sysroot` | `name`，所选target下的逻辑系统库名 |

具体版本由coordinate唯一提供。Git的实际选择使用commit；references保留对应的声明意图，元素为`default`、`branch:<name>`、`tag:<name>`或`rev:<value>`，去重并排序。同仓库内path边不增加selector，因此列表可以为空。日常使用lock必须匹配当前声明的selector；从branch改成tag或其他branch时不能继续冒用旧commit。多个selector解析到同一个repository/commit/subdir可以共存，selector本身不成为新的实际来源身份。显式`--lockfile`只改变锁文件位置，不改变workspace发现和root标识；path/artifact字段按实际lock目录重新表达。

不把下载 cache 的绝对路径、工具位置、mtime、编译结果、成功收据或机器凭据写入 lock。用户显式使用的本机绝对 path/artifact 可记录但不具有可移植性；相对路径可以含 `..`。path 源码内容不被 lock 冻结，正常源码更新由构建缓存处理。lock 固定依赖选择，不承诺不同工具链产生相同二进制。

### 6.2 使用与更新

| 模式 | 解析与获取行为 |
| --- | --- |
| 默认 | 优先使用现有选择；相关要求改变或缺少记录时求解并更新；已满足时不主动查询新版本 |
| `--locked` | 必须有满足本次请求的记录；不改变图/来源/版本；可下载其中已锁定但本机缺少的源码 |
| `--offline` | 不访问网络；可用现有 lock、本地 Git objects、已缓存索引和归档求解；本地信息不足则明确失败 |
| `--frozen` | `--locked --offline` |

读取 lock 时只做必要的格式、坐标和图结构检查。对源清单的约束匹配、workspace 展开与来源选择由 PM 负责；driver 不再做第二次求解。PM 使用普通本地解析记录按 manifest/lock/config/工具的路径、mtime、大小判断更新；命中不重读清单正文、Git checkout 内容或归档摘要。输入发生变化后只更新受影响的解析单位；仅测试配置变化不使生产结果失效。

默认构建的有效 lock 不进行网络 freshness 请求。首次解析、增加 registry 来源或明确 update 时，联网模式刷新相关包索引；共享源码缓存仍直接复用同一版本。旧选择/缓存 metadata 不能满足已改变的全图要求时，只刷新冲突涉及的来源后重试；`--locked` 不走这条更新路径。离线解析只能使用本机索引和 Git 对象，可能选不到联网时的新版本，诊断应说明信息来源，不冒充已检查远端最新状态。

求解、选中来源获取及必要 manifest 匹配成功后，才发布该批解析结果；失败保留旧 lock。后续编译失败可以保留已正确生成的 lock，但不能发布构建成功记录。普通文件锁和原子替换协调写入，在锁内重查输入并保留本次未选择的解析记录；不把两个并发命令的更新互相覆盖。已删除的成员记录在下一次写入时移除。lock 冲突不可通过任选一侧静默解决，应重新 resolve。

## 7. PM 与构建 driver 的接口

### 7.1 便携 lock 与本地准备结果

driver 消费的是 lock 的精确图和本次已准备好的本地输入。portable source 不能直接当作本机文件路径，因此 PM 将 lock 投影为 `PreparedWorkspace` 返回；这份投影不是第二份 lock，不接受独立编辑，不记录另一套版本选择，也不是构建结果收据。

`scoop-pm prepare --request <file> --response <file>` 是唯一自动构建接口。请求/响应采用版本为 1 的 UTF-8 JSON，一次进程处理本次选中的全部根；响应成功后原子写文件，失败不提供部分成功响应。失败诊断写 stderr 的单个 JSON 记录，字段为 `version` 和 `diagnostic`，后者复用 `StructuredDiagnosticV1` 的 canonical CBOR 字节表示；清单主位置及相关声明保留为 host path/span，driver 直接交给现有诊断展示。其他面向人的 PM 命令仍输出文本诊断。未知协议版本明确失败，不在 driver 中降级成旧解析器。文件中的 host path 沿用既有可表示本机路径的数据载体，不能通过有损 UTF-8 转换改变路径。

请求的必需信息是：协议版本、cwd/显式 root、成员选择、purpose、canonical target、sysroot、artifact search roots、PM cache 位置和 locked/offline 策略；可选指定 lock 路径。profile、编译参数、native 工具位置和输出调度不参与求解。响应的必需信息是：协议版本、workspace/lock 位置、所选解析单位，以及每个单位的完整精确节点和边。节点用 `SourceCone` 或 `PrebuiltCone` 闭合表示：

- SourceCone：完整 coordinate、kind、本地 Cone 根、已展开的 manifest 生产配置、所选根需要的测试配置、manifest 诊断来源，以及已绑定的精确 direct 边和测试边。配置含现有源码选择、native 声明和文档元数据，不含源码正文或完整输入镜像。
- PrebuiltCone：完整 coordinate、本地 `.slib` 位置和精确依赖记录；sysroot 和 artifact 来源均落实为该形式。PM 提取过的必要摘要可随结果交付，但不能作为跳过实际 reader 所需格式/ABI 检查的授权凭据。

版本 1 的共享数据类型位于 `scoop-protocol`，不依赖 PM 的求解或获取实现。请求字段为 `version/cwd/input/members/purpose/target/sysroot/artifact_roots/package_cache/lockfile/locked/offline`；`members` 区分 `default`、`all` 和 `packages`，purpose 为 `production/test`。响应使用 `version/workspace/lockfile/resolutions`，每个 resolution 包含根相对路径、根 coordinate、purpose、target 和 nodes。节点包含 `cone`、精确 `dependencies` 与 `test_dependencies`；`cone.input` 区分 `source/prebuilt`。source 交付完整 `ParsedConeManifest`、本地根、原清单路径与文本、展开的文档 metadata。原清单文本仅服务诊断，不作为再次解析的输入；继承条目的诊断 span 指向成员的继承声明。prebuilt 交付本地路径和已有 `ArtifactManifestSummaryV1` 的 canonical CBOR 字节表示，复用既有摘要格式。

生产配置只包含生产依赖；仅所选测试根绑定测试配置，传递依赖不携带可激活的测试边。源码配置中的原始 locator 不再参与 driver 发现：所有声明都已绑定到该解析单位内的精确 coordinate。隐式 core 保留在图的精确边中，不伪造原清单中的声明位置。协议边界检查版本、host path、节点/边对应关系及完整引用；不重做版本要求、workspace 继承和来源选择。

`scoop` 将结果接入已有精确 `ResolvedBuildGraph`、缓存与调度，只做进程边界所需的数据解码/图引用检查；不重新打开原 manifest 解释 workspace/version/source，也不再次计算“这个版本是否满足范围”。实际编译仍按输入选择读取源码，实际 `.slib` 入口仍承担既有产物正确性职责。

多个根之间可共享的任务由实际源码/配置、精确依赖结果、target、profile 等现有编译输入确定，**不能仅按 ConeIdentity 合并跨解析单位节点**。同版本源 Cone 在两个程序中绑定了不同依赖版本时，必须是两个编译任务和结果。

普通构建使用一个跨根 ready queue，`--jobs` 是整次调用的编译子进程上限。每个根保留自己的闭包、完成节点和诊断顺序；就绪源码按已有完整编译键及其锁复用工作，其他根遇到正在构建的同一键时让出队列，完成后消费同一结果。父进程不能阻塞在自己持有的另一个键锁上。发生失败后停止启动新节点并收完已启动的子进程，按根选择顺序和根内拓扑顺序报告失败。多根测试分别运行所生成的测试程序；`run` 在所选成员中要求唯一 executable。

### 7.2 自动准备与低层调用

普通 manifest build/run/test/check/doc 在构建前调用配套 PM 的 prepare。`scoop build/run` 在 `Cone.lock` 不存在时自动调用 PM 解析依赖、准备来源并生成 lock，成功后继续构建／运行，无需用户先执行 resolve。已有有效 lock 时优先复用锁定选择和本地准备结果，不查询新版本。显式 `--locked` 或 `--frozen` 遇到缺少适用 lock 时失败；普通 `--offline` 仍可用足够的本地来源生成 lock。`scoop build --lockfile <file> -p group:name --locked` 是显式消费锁定图的入口；没有 `-p` 时仍按所在 workspace 的默认选择。driver 本身从不修改 lock。

首次构建也使用同一次 prepare 请求：

1. `scoop` 将本次根选择、用途、target 和锁定/离线模式交给 PM，不先要求用户创建 lock。
2. PM 在默认模式下为缺少的锁定记录求解，准备选中来源；全部成功后原子写入 `Cone.lock`，返回该图的本地准备结果。
3. `scoop` 收到成功响应后，才按精确图调度编译和链接；`run` 在本次构建成功后运行程序。

PM 启动、协议、求解或获取失败时，本次命令立即失败，不进入编译或运行。driver 不另行先执行 resolve 再解析清单，也不在 PM 失败后自行选版本。后续编译或链接失败保留已经成功生成的 lock，便于修复源码后重试；lock 的存在不表示构建成功。

`scoop` 根据 lock 的精确图安排依赖优先的构建，将每个单 Cone 的配置、源码输入、精确 direct/support 产物及目标/用途参数通过现有 machine request 交给 `scoopc`。PM 完成版本要求与来源的绑定，`scoop` 将已绑定的边落实为具体产物参数；两者都不把选择工作交回编译器。原清单文本及 span 只供诊断，不能作为 child 再次解析的输入，也不能通过生成临时源码树改变诊断路径。

`scoopc` 不读取 `Cone.toml` 或 `Cone.lock`，不处理 workspace、版本要求、来源定位或声明与产物的匹配。普通 Cone 的 build/check/test/doc 和 AST 观察均走上层 `scoop`；现有独立 manifest 入口退役，不为其增加新版清单适配。`scoopc` 保留精确 direct 集合、support 闭包、单版本、产物语义指纹和 target/ABI 的普通编译输入检查，不重复 PM 的版本范围判断。只观察 AST 时沿用不加载依赖产物的规则。

single-file build/run 和 artifact-only `scoop link` 保持现有精确输入方式，不启动 PM、不读取邻近 workspace。低层单文件 `scoopc build/check` 仍可接收一个 `.scoop` 文件和显式 direct/support 产物；头部使用现有精确坐标，不引入包版本要求。普通 core 槽位与显式 core 自举参数保持。

需要分开编译和链接时使用 `scoop build --emit-slib`：仍经 PM/lock 准备完整图和正常单 Cone 编译，但将根 `.slib` 作为输出，不准备 runtime、不执行最终链接。`-o` 指定产物位置，默认成员输出名为 `<name>.slib`；单文件使用源文件 stem。JSON 成功结果的 `kind` 为 `artifact`，保留 root/dependencies 与编译观察，不含 runtime/link 字段。`--tests` 只与 `--emit-slib` 一起使用，选择 Test 锁定图和测试入口产物，默认仍为 Production；普通测试继续使用 `scoop test`。该输出选择不进入编译缓存键，后续普通 build 可直接复用 `.slib` 后链接。

LSP使用同一已展开数据模型；已有本地准备结果可直接消费，缺少结果时可执行PM的`metadata --offline`读取，不修改lock。缺少锁定源码提示fetch，缺少适用lock则提示resolve；仍可展示本地members和语法诊断。文档编辑或补全不自动触发网络。formatter只需本地成员发现和源码清单展开，使用PM的members输出，不要求先成功求解依赖。

`members --overlays-stdin` 与 `metadata --offline --overlays-stdin` 为编辑器接收未保存的清单文本。stdin 是版本 1 JSON：`version` 与 `manifests` 数组，每项含无损绝对 host `path` 及 UTF-8 `text`；路径必须指向 `Cone.toml` 且不重复。PM 对这些本地 workspace/path/Git 清单使用同一解析、继承和锁定匹配规则，关闭 buffer 后恢复磁盘输入。覆盖文本不写回清单、lock、共享源码或持久准备记录；未覆盖的输入继续复用既有缓存。metadata 的此模式强制离线，只接受现有锁定选择；新增或改变依赖导致 lock 不适用时提示保存后 resolve。成员发现仍可在无 lock 时工作，新增但未保存的成员清单沿普通本地路径参与发现。原路径与覆盖文本保留给诊断，不生成临时源码树。

LSP 在 workspace 根打开时发现全部成员，并为各成员独立取得所选 target/purpose 的锁定图；显式成员目录只选择该成员。一个成员缺少适用 lock 或本地依赖时，仅该成员的依赖语义不可用，本地源码、语法诊断和 workspace 符号仍可使用，不能以猜测的版本或空依赖图冒充已解析项目。已安装系统库的源码导航也使用 members 提供的源码选择，不为导航解析依赖。`Cone.lock` 与清单、源码和产物共同参与文件监听；依赖准备只由用户显式 resolve/fetch/build 触发。LSP 的 `packageManager` 默认取实际 scoop-lsp 同目录的 scoop-pm，`packageCacheDir` 与 `lockfile` 可显式配置，均沿配置根解析相对路径。

`members --json` 的格式 1 同样由 `scoop-protocol` 定义：返回 workspace root、所有成员的 coordinate/kind/相对路径/本地目录/清单路径、selected 标记及生产/测试 SourceSelection；本地路径使用无损 host path 载体。formatter 消费所选成员的源码集合，clean 使用 workspace root 定位统一 target；两者不调用 prepare，不创建或读取 lock。JSON 失败使用与 prepare 相同的结构化诊断记录。`fmt` 支持 `--workspace` 和可重复 `-p`，成员选择只接受一个 workspace 输入；多个显式文件仍可合并格式化。显式 `.scoop` 文件模式独立于 PM。

## 8. 源码 registry

本节是本轮的客户端合同。registry service 只需托管普通文件，版本求解、下载、校验和缓存均在 PM 内完成；不要求服务端执行包查询、版本选择或下载重定向逻辑。发布者如何生成和上传这些文件单独由第 9 节定义。

### 8.1 静态读取协议

registry base 是第 4 节的规范化 HTTP(S) 目录 URL，例如 `https://packages.example.org/scoop/`。base 下提供 `config.json`（完整内容为 `{"format":1}`），以及 `packages/<group>/<name>.json`；group/name 使用 canonical 文本，点不拆目录。索引为 UTF-8 JSON 对象，必需字段如下，格式 1 不接受未知字段。客户端只请求实际需要的包记录，不需要全局包列表或目录浏览，也不克隆完整索引。

```text
config.json
packages/org.example/json.json
archives/org.example/json/1.4.3.tar.zst
```

其中归档目录和文件名是示例，实际下载位置完全由版本记录的 `archive` 指定。`packages/org.example/json.json` 的内容示例为：

```json
{
  "format": 1,
  "group": "org.example",
  "name": "json",
  "versions": [
    {
      "version": "1.4.3",
      "kind": "library",
      "dependencies": [
        {"key": "scoop:scoop.core", "version": "=0.1.0", "source": {"kind": "sysroot"}}
      ],
      "archive": "archives/org.example/json/1.4.3.tar.zst",
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "yanked": false
    }
  ]
}
```

每个版本记录必需完整版本、kind、生产依赖、archive、sha256、yanked。依赖条目必需key、version和source；source为`{kind="registry", registry=URL}`、`{kind="git", git=URL, commit=完整ID, subdir=目录}`或`{kind="sysroot"}`对应的JSON对象，不在索引内复制所依赖版本的归档位置。隐式core在索引中展开为普通精确边。只给消费者解析生产依赖，测试依赖保留在源包清单中但不参与传递求解。executable版本可发布，但不能作为普通依赖候选。

`archive` 是 HTTP(S) URL 或按标准 URL 规则相对 registry base 解析的引用，不能包含内嵌凭据或 fragment；允许归档位于其他静态站点。它不相对包索引文件或重定向后的地址解析。`sha256` 是实际 tar.zst 归档字节的 64 位小写十六进制摘要，示例值需替换为真实摘要。索引的 group/name 必须匹配请求，versions 可为空，完整版本文本不得重复；数组顺序不影响求解。依赖 key 不得重复，版本要求沿用第 2 节，不另设 registry 版本语法。

读取只需普通 HTTP GET：首次使用某个 registry 取得 config，求解时按需取得包索引，选定版本后取得其归档。不要求 HEAD、Range、ETag、条件请求、压缩索引或 POST API；可通过普通静态文件服务器、对象存储或 CDN 提供相同文件。服务端应返回完整 JSON 和归档内容；HTTP 客户端可跟随标准重定向，但静态站点无需配置重写或重定向。404 表示所请求的 registry 文件/归档不存在，其他 HTTP、TLS、JSON 或归档错误正常报告；不切换到另一个 registry 猜测同名包。

有效 lock 保留已选版本及 archive/sha256；已有源码缓存时不查询远端，新环境的 locked fetch 可读取索引来取得锁定版本的完整依赖 metadata。新的解析或 update 刷新所涉及包的索引，offline 只用本地已有索引/归档/源码。索引先供求解使用，不为了列出候选下载每个版本的源码。服务端应先提供归档，再原子替换对应包索引，避免索引引用尚未可下载的文件；单包索引的更新不需要全 registry 事务。

### 8.2 不可变发布与缓存

已发布版本的源码、依赖 metadata 和归档摘要不可更改；需要修复应发新版本。只有 yanked 状态可更新：新求解排除被撤回版本，现有 lock 可继续使用它，明确 `--precise` 仍不把撤回版本作为新选择。服务器应保留旧归档以支持既有 lock。

下载写入临时文件，完成一次 sha256 校验后解包并发布本地结果；正常缓存命中不重新读取归档/源码计算摘要。首次加载选中包时，检查有效 manifest 的 coordinate、kind、生产依赖与索引一致，复用该解析结果，不在 driver 或每个构建阶段重放。摘要用于下载内容完整性，不建立来源认证、签名信任链或运行时资格体系。

归档 reader 检查格式、边界和解包路径，拒绝绝对路径、路径逃逸、冲突条目与非法文件类型；这些是正常解包职责，不检查 owner、权限、inode 或输入目录的物理归属。实际缺失输入和 I/O 失败正常报告；不设计“外部破坏缓存后自动恢复”的独立流程。

### 8.3 已发布源码包的消费格式

格式 1 的归档为 zstd 压缩的 tar，根目录直接包含 `Cone.toml`、源码及随包资源，不额外包一层包名目录。归档包含普通文件和目录。清单使用 schema 1，定义一个 Cone；workspace 继承必须展开，不含 workspace、根 overrides 或 publish 配置。路径按包内普通相对路径表达，随包内容必须足够用于声明的源码/native 输入。

生产与测试依赖都须是可独立消费的显式 registry、完整 commit 的 Git，或 scoop 系统库来源，不能依赖发布者本地 path/artifact/default registry 配置。索引只列生产依赖，隐式 core 展开为精确依赖；源码中的普通 import、可见性和编译规则保持不变。消费者使用自己的 lock，归档不提供依赖版本选择。PM 在首次取得归档时将展开清单与索引匹配，然后把同一个共享源码目录交给构建；不需要先安装或调用 publish 工具。

## 9. 源码打包与静态发布

本节留至下一阶段，包含 package/list/check、发布清单转换、静态目录维护和发布流程；第 8 节客户端读取及下载消费在本轮完成。

`scoop pm package`为所选Cone在workspace（独立Cone即自身）的`target/package/<group>/<name>/`下产生`<version>.tar.zst`和`<version>.json`版本metadata，`--target-dir`可替换target根；多个成员不会因拼接包名而覆盖。归档根直接包含独立`Cone.toml`和该Cone的源码/资源。默认纳入Cone目录内的普通文件，排除`.git`、`.scoop`、`target`、`Cone.lock`及本次package输出；`[publish] include`可显式列出完整相对文件/目录清单，exclude再排除子集，首版均不解释glob。排序稳定，归档中的owner/时间不承载本机状态。

包必须包含所有已声明平台分支需要的 Scoop/native 源码及随包头文件；不能只按打包主机 target 截取。系统 native 库仍是逻辑外部依赖，不把宿主 SDK/系统库打入包。绝对本机路径、未映射的包外文件依赖和无法移植的 native 参数使 package 失败，并指出条目。

现有多个 Cone 共用 `../../vendor` 的情况使用显式文件映射：

```toml
[[publish.files]]
from = "../../vendor/llhttp"
to = "vendor/llhttp"
```

from 是相对当前 Cone 的普通文件/目录路径，to 是归档内相对路径。不同映射不得重叠或覆盖包内已选文件；PM 按最长匹配前缀重写 manifest 的 native.sources/include、readme 以及受支持的 native 路径参数。不能解析/重写的路径参数直接诊断，不猜测 shell 文本。普通复制按实际 I/O 读出文件；symlink 按实际目标读取并写成普通归档内容，遍历循环/I/O 错误照常报告，不进行文件系统来源审计。

打包后的清单展开workspace继承和默认registry URL，移除workspace、overrides、publish配置和本机locator。生产与测试两张依赖表都必须能独立消费；path依赖只有同时声明发布用version要求与可确定的默认registry时才能转换成该registry依赖，因此path表本身无需允许第二种来源。Git依赖按原声明来源解析为完整commit，浮动tag/branch不能原样发布；未写version时补入该commit清单的精确版本要求。可复用来源匹配的lock选择，不能将根override的替换源写成原Git来源的发布结果。artifact依赖没有可移植源码替代，源码package拒绝它，作者需改成明确的源码依赖。消费者不继承发布者的lock或本地覆盖。

`package --list` 输出最终文件清单和改写后的 manifest；默认 package 进行清单、依赖可发布性和归档检查，不隐式运行编译。`package --check` 先为已生成归档解包后的独立Cone解析消费者可用的来源，生成本次检查用的临时lock，再显式调用普通driver做一次`--locked`构建；不把发布者的path/override带入该检查。生产源码及工具保持原位，不通过隐藏原输入制造验收。该构建不会证明所有其他 target 都已验证，结果须标明实际 target。

`scoop-pm registry add --root <directory> <archive>` 维护符合第 8 节的本地静态目录：读归档的展开清单生成 metadata，检查目标版本未存在，先落盘归档，再原子更新对应包索引。`registry yank/unyank` 只变更状态。并发写由普通文件锁协调。静态目录可由用户已有站点部署方式托管；首版交付这组维护工具和 HTTP 消费闭环，不提供远端 `scoop publish`、托管账号或自定义上传服务。

## 10. 命令与失败行为

| 命令 | 行为 |
| --- | --- |
| `scoop pm new --workspace <dir>` | 建立 virtual workspace；不隐式创建业务成员 |
| `scoop pm members [--json]` | 本地发现和展开成员，供 CLI/LSP/formatter 使用，不求解或联网 |
| `scoop pm add/remove …` | 编辑指定成员的依赖声明，保留无关 TOML 内容与注释；不隐式下载或更新 lock |
| `scoop pm resolve` | 按根/用途/target 求解并准备所需来源，发布 lock |
| `scoop pm fetch` | 按现有 lock 获取所选闭包；相当于只准备依赖的 locked 模式，不编译 |
| `scoop pm update [--package group:name] [--precise value]` | 对所选解析单位更新版本/ref，显示 lock 的变化 |
| `scoop pm deps [--why group:name] [--json]` | 只读现有精确图和本地约束说明；缺少适用lock则提示resolve，不隐式更新 |
| `scoop pm metadata [--offline]` | 按locked规则返回第7节的数据，不改写lock；offline时也不下载，缺少必需来源则提示fetch |
| `scoop pm package [--list] [--check]` | 生成或检查源码包 |

根选择沿用第 3 节，PM 命令用 `--purpose production|test`，默认 production；update 的 `--package` 指待更新依赖，与选择根的 `-p` 区分。target 默认使用本机的 canonical Scoop target，可显式指定。resolve/update/prepare 支持第 6 节模式；members、清单编辑与 formatter 不需要依赖成功解析。

`scoop clean`在manifest/workspace模式只通过本地成员发现定位workspace的target目录，不求解、不获取依赖、不删除Cone.lock或全局PM缓存；单文件仍使用其既有target根。若用户显式`--target-dir`，按该路径清理原有输出范围。

`deps --why <group:name>` 对包含该包的每个选中解析单位输出锁定版本/来源、所有直接约束边、声明位置，以及从根到各约束边的一条确定路径；测试边明确标记。根自身说明为选中的 root。解释的是当前 lock 和有效声明，不保存历史求解轨迹，也不声称保留的版本一定是远端最新版本。该命令始终只读本地来源，不下载、不展开缺少的远端源码、不更新 lock；缺少源码/索引提示 fetch，声明已不满足 lock 则要求 resolve。`--json` 输出 `format/resolutions`，每个 resolution 包含 `root/purpose/target/selected/source/requirements`（root/selected 为精确坐标），要求记录包含 `from/path/versions/source/test/origins`；origin 使用无损 host path 与声明 span。普通不带 why 的 deps 仍只需读取锁定图。

`metadata` 返回与 prepare 相同的版本 1 `PreparedWorkspace` JSON，接受本地根/成员、用途、target、sysroot、artifact search roots、lockfile 和 PM cache 选择；始终要求已有且满足声明的锁定记录，不写 lock。`--json` 同时选择结构化错误，供 LSP 消费。在线时可获取已锁定但本机缺少的来源；`--offline` 只使用已展开的 Git checkout 与 registry 源码目录，不能从已有 Git objects 或源码归档隐式展开缺少的来源，缺少时明确提示 `scoop pm fetch`。有效的本地准备记录沿既有更新规则直接复用；普通 metadata 缓存写入不修改项目声明或依赖选择。

清单编辑使用 `scoop pm add <group:name> [input]` / `remove <group:name> [input]`，可用 `-p group:name` 选目标成员；默认发现与成员选择沿用 workspace 规则，必须唯一选中一个 Cone，多成员默认选择要求显式 `-p`。`--purpose production|test` 选择依赖表。add 的 `--version` 是完整版本要求；`--path`、`--git`、`--github`、`--registry`、`--artifact` 选择来源，Git 另接受 branch/tag/rev/subdir。无来源时按普通声明默认来源处理，必须给 version；不联网推测版本。`--inherit` 写入完整的 `{ workspace = true }`，不能与 version/source 混用，必须能从所属 workspace 继承该 key。path/artifact 及本地 Git 路径以调用 cwd 为基准，写成相对目标成员的路径。

add 新建或整体替换所选表中的该条声明，不隐式合并旧来源字段；保留该条的行注释及无关 TOML 内容、顺序和注释。remove 删除所选表中的该条，不修改另一用途或 workspace.dependencies；缺少该条时报错。编辑仅验证新清单及 workspace 继承的合法性，不要求声明的包/路径当前可获取，不编译、不读写 lock。

add/remove 先验证拟写的 TOML，再原子替换该 manifest；解析由后续 resolve/build 触发，避免承诺 manifest 与 lock 的跨文件事务。取消、网络失败、无解和工具退出均明确返回失败；已有可用 lock、下载结果或构建结果只按各自成功边界发布。没有成功准备结果时 driver 不运行旧程序充当本次成功。

## 11. 缓存与可测试性

### 11.1 跨项目共享目录

PM cache 默认根目录为 `~/.scoop/packages/`，与安装根下的 `bin`、`lib`、`src` 等发行资源分离；不在每个项目中复制下载包。CLI `--package-cache-dir` 优先于 `SCOOP_PACKAGE_CACHE_DIR`，再使用默认值，prepare请求携带最终位置。移动该目录只改变本机来源定位，不修改Cone.lock或语言身份。

```text
~/.scoop/packages/
  registry/<registry-id>/
    index/<group>/<name>.json
    cache/<group>/<name>/<version>.tar.zst
    src/<group>/<name>/<version>/
  git/<repository-id>/
    db/
    checkouts/<full-commit>/
  records/
```

registry-id和repository-id是规范化来源URL的SHA-256目录键，只用于避免名称/路径冲突，不是来源认证。registry缓存键为`(registry URL, group, name, exact version)`；不同registry的同名同版本不能共用槽位。Git object库按repository复用，checkout按完整commit复用；subdir只选择其中一个Cone，不再复制仓库。lock中的registry摘要是下载输入，在首次下载时校验，缓存命中不重新hash；同缓存键出现不同声明摘要是不可变版本记录冲突，报告错误，不覆盖已有版本或尝试修复其内容。

多个项目的相同registry版本直接使用同一个解包目录；相同Git commit直接使用同一个checkout。PM管理的源码目录作为共享输入使用，构建输出写入已有构建cache和用户target目录，不能写回共享源码目录。需要修改依赖时使用本地path/根override，不把一次项目修改传播给所有消费者。这里不引入chmod、owner、inode或symlink来源检查。

以来源/版本（Git以仓库/commit）为单位的普通文件锁协调下载和展开：获取锁后先检查结果是否已经存在，成功则复用，否则获取到临时位置、完成正常读取/下载检查后原子发布。一个版本的失败不覆盖已发布的其他版本；两个项目同时首次请求同一包只需要一份成功结果。不在每次项目clean时清空全局PM cache；构建clean仍只处理其原有输出范围。

相同源码版本不保证编译结果相同。driver继续以target、工具链、profile、源码/native有效配置和实际精确依赖结果判断共享，不能只按`group:name:version`取`.slib`。多个项目的path依赖也不会仅因坐标相同而合并源码目录。

### 11.2 更新判断与独立测试

PM 的缓存保存 manifest 展开记录、registry metadata、Git objects/checkouts、解包源码与本地准备结果；driver 保留已有 Cone 编译、native 和链接缓存。两者共享“先判断更新，再读取内容或准备输入”的规则，不互相读取命中产物作内容复核。

解析缓存区分 root/purpose/target 及实际依赖声明/来源；构建缓存继续区分实际依赖结果和 profile。配置、lock 或本地 manifest 改变才更新相应投影；源文件正文变化不应触发版本重新选择。普通缓存目录并发以文件锁协调；输入变化、缺失输入和实际失败按既有规则处理。

prepare 的本地记录按规范化请求位置、成员选择、purpose/target、sysroot/search roots、有效 PM cache 和工具版本区分。更新记录保存本次读取的配置、manifest、artifact 和 lock 的 mtime/大小，以及成员发现过程中不存在的候选清单/用户配置；新增 workspace 或配置同样使记录失效。先检查这些更新信息，命中后直接读取并交付已有准备结果，不重读 TOML、不展开继承、不遍历版本约束，不读取 Scoop/native 源码正文。locked/offline 不改变已经成功准备且输入未变的结果；源文件内容变化留给 driver 的构建缓存处理。

解析器测试使用内存 metadata 图；Git 测试使用本地仓库；registry 测试使用本地 HTTP 静态服务。绝大多数 PM 测试不需要 LLVM、runtime 或调用 `scoopc`。少量正式 CLI fixture 验证 prepare → driver → 编译/链接和离线消费；普通库行为继续放 Cone tests。不引入包授权、来源防伪、资源计费或通用任务执行框架。

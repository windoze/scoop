# Scoop 小改动设计

状态：已实现并完成三个目标的验收；用户已确认核心规则，其余边界采用下文记录的实施选择。更新日期：2026-10-22。验收结果见 [PROGRESS.md](PROGRESS.md)。

代码基线：`windoze/scoop` 的 `main`，提交 `4de3ae9e8ac3f18bc63eba4947afd8bb87ea34db`。

配套文件：[分步实施计划](PLAN.md)。

本批改动完善 Cone 的 package 约束、工程创建和单文件执行，并将测试按职责组织。普通功能与内置库测试统一使用 release；编译器和 GC 正确性由明确选择模式的专门用例覆盖。单文件继续复用现有编译、依赖和链接流程，不建立另一套脚本语言或包管理器。

## 已确认规则

| 事项 | 已确认的行为 |
| --- | --- |
| package 与 group | package 可以等于 Cone group，也可以位于其子包下。例如 `dev.example` 和 `dev.example.http` 均属于 group `dev.example` |
| 内置 Cone 例外（D02 已确认） | core 和 std 豁免 package 与 group 的前缀约束 |
| 单文件 package（D03 已确认） | 源码不需要写 package；存在声明时发出警告并忽略，继续使用单文件的内置默认 package |
| 新建工程 | 新增 `scoop new` 创建 Cone；模板选项的具体集合见 D04 |
| shebang | 文件首行的 `#!` 行按无内容注释处理，不形成声明或文档注释 |
| 单文件依赖 | 使用 `//! dep: group:name:version`，替代最初讨论的 `///!` |
| 单文件默认依赖（D10 已确认） | 始终默认直接依赖 core 和 std；显式 dep 只追加依赖，不替换默认集合；不自动增加 import |
| 普通 Cone 中的依赖注释 | 不参与依赖发现，按普通注释处理 |
| 单文件程序参数 | 保留显式 `--`；允许省略它传普通位置参数；进入程序参数后，其余参数原样转发；首个参数与 runner 选项冲突时使用 `--` |
| 内置库测试 | 能迁入 `scoop.test` 的库行为测试迁入对应 Cone；普通 Cone 测试和普通 fixtures 仅使用 release |
| 模式覆盖 | 编译器正确性用例单独分组，按实际需要覆盖模式和组合；GC stress 只用于 GC 正确性场景 |
| GC stress 构建开关 | 仅使用 `--enable-gc-stress` 构建的程序包含 stress 支持；随后由环境变量启用或禁用。未使用该参数时，相关环境变量无效 |
| 测试组织 | 按功能与回归原因组织 fixtures，不再以里程碑作为主要目录结构 |

以下各节给出实施方案。D02、D03、D10 为用户明确确认的选择；其余边界按原设计推荐方案实施，见文末决策记录。设计落地前，按仓库约定先修订相关规范，再修改实现。

## 基线基础与迁移范围

在基线提交中，[语言规范第 12 章](../specs/SCOOP-SPEC.md)允许 Cone 与 package 独立，省略 package 时属于根包；[实现规范第 2.7 节](../specs/SCOOP-IMPL-SPEC.md)将单文件模式限定为 core-only。本批已同步调整这两处规范。

基线的 [CLI](../../compiler/scoop/src/cli/args.rs)有 Build、Run、Test、Link、Doc，没有 New。`RunArgs.program_args` 使用 `last = true`，当时必须通过 `--` 传参。基线的 [词法器](../../compiler/parser/src/lexer/comments.rs)将 `//!` 作为普通注释，但不接受 shebang。

基线测试清单如下，数量表示声明及组织状态，不是本批改动后的执行结果：

| 指标 | 基线数量 |
| --- | ---: |
| 通过现有 discovery/schema 校验的 fixture | 4,532 |
| 声明的运行变体 | 5,340 |
| 同时具有名为 debug 和 release 的变体的 fixture | 648 |
| 在步骤或变体环境中显式设置 `SCOOP_GC_STRESS_MOVE=1` 的 fixture | 1,699 |
| 位于里程碑命名顶层目录下的 fixture | 4,029 |
| 主要内置 Cone | 19 |
| 已配置 `scoop.test` 测试依赖且有测试源码的主要内置 Cone | 15 |

基线中的主要内置 Cone 里，`scoop.cli`、`scoop.json`、`scoop.core`、`scoop.test` 尚无常规 `tests/` 源码。core 的 67 个生产源码文件属于根包。部分辅助 Cone 的 group 是 `scoop.tests`，package 却是 `scoop.crypto.testdata` 或 `scoop.asyncio.test_native`，也属于 package 迁移范围。

## package 与 Cone group

### 名称规则

对适用该约束的源码，将 Cone group 按 `.` 分成名称段，与 AST 中 package 的已解析名称段比较。group 的所有段必须与 package 的前缀逐段相等，package 可以有额外的子包段。

| group | package | 结果 |
| --- | --- | --- |
| `dev.example` | `dev.example` | 合法 |
| `dev.example` | `dev.example.http` | 合法 |
| `dev.example` | `dev.examples` | 非法 |
| `dev.example` | `dev.example2.http` | 非法 |
| `dev.example` | `example` | 非法 |

不能用无边界的字符串 `starts_with` 判断，也不能把一个反引号标识符内的点当作 package 分隔符。Cone group 允许连字符；例如 group `dev.my-org` 对应的源码可使用 ``package dev.`my-org` ``。模板应复用现有标识符转义规则。

约束只限制 namespace 的允许范围。ConeIdentity 仍来自完整 coordinate；声明身份继续使用现有 typed identity 规则。相同 package 仍可由多个合法 Cone 贡献声明，split package 不合并不同 origin，也不扩大 internal/private 可见性。

### 已确认的例外与单文件行为

core 和 std 豁免上述前缀约束，覆盖其生产和测试源码。按这两个内置 Cone 的身份识别例外，其 group:name 分别是 `scoop:scoop.core` 和 `scoop:scoop.std`；不按目录名判断，也不放行整个 `scoop` group。其他内置库及独立测试辅助 Cone 仍遵守普通规则。本批不要求迁移 core 的根包，也不因该约束迁移 std 的 package。

单文件模式始终使用现有内置默认 package，即 [parser 当前初始化的 RootPackage](../../compiler/parser/src/parser.rs)，不是根据文件名或保留 Cone group 推导一个 package。源码应省略 package；若出现 package 声明，警告指向该声明，说明它已被忽略，随后继续编译。名称解析、声明归属和后续 lowering 均使用默认 package，不能只发出警告却保留声明的命名空间效果。

实现保留原始源码和声明 span，在进入语义处理时选定生效 package，不删除源码行。合法 package 声明本身不会导致构建失败；声明语法损坏、重复声明或声明位置错误仍沿用现有语法诊断，不以“忽略”吞掉解析错误。普通 manifest 模式不会套用此警告和忽略规则。

### 校验入口与诊断

在 manifest 和已解析源码同时可用的前端边界校验一次，覆盖普通 build、test 和 doc。可从 [driver 的源码解析入口](../../compiler/driver/src/request/preflight.rs)接入，复用 AST 的 package 名称段及 span；parser 本身不依赖 manifest。

适用约束的测试源码遵守与生产源码相同的规则。错误指向 package 声明，报告实际 package 和期望的 group 前缀。除 core/std 外，普通 manifest 源码省略 package 时按 D01 报错；core/std 例外与单文件默认 package 已确定。后续 HIR/MIR/LIR 不重复校验同一关系，预编译产物消费也不回读其源码做来源检查。

### 迁移

D01 要求除 core/std 外的 manifest Cone 源码显式声明合法 package。core/std 的豁免不扩展到独立消费者或测试辅助 Cone；不能仅为了使旧 fixture 通过而加宽生产规则。

迁移需同步修改 package、import、re-export、测试显示名称和必要诊断期望。package 改变可能改变声明 ID；若涉及内置库公开声明，应重建受影响 sysroot 产物及下游消费者，不以旧产物成功作为迁移通过。修改名称组织本身不要求重设计 runtime ABI。

负例应保留原本被测的错误：原来测试重载、可见性或类型错误的源码，不能在新规则下先被无关 package 错误截断。新增的 package 专测则应精确断言自身诊断及位置。

现有单文件 fixture 若用 package 验证命名空间、限定名或可见性，应迁入合适的 manifest 测试载体；若声明与测试目的无关，可移除声明并调整相关名称。单文件专测保留“有声明时警告且按默认 package 编译”的覆盖，不通过批量改写诊断期望掩盖原有测试目的。

## scoop new

### 首版接口

D04 采用以下固定接口：

```sh
scoop new hello --group dev.example
scoop new utilities --group dev.example --kind library
scoop new tools --group dev.example --name tools --package dev.example.tools
```

| 输入或选项 | 含义 |
| --- | --- |
| `<path>` | 新 Cone 的目录 |
| `--group` | 必需，使用现有 coordinate 校验，不猜测用户或组织身份 |
| `--name` | 缺省取目录名；非法时要求显式提供，不静默改写 |
| `--kind` | `executable` 或 `library`，缺省 executable |
| `--version` | 缺省 `0.1.0`，使用现有 canonical SemVer 规则 |
| `--package` | 缺省 group，显式值必须满足 group 前缀规则 |
| `--no-tests` | 默认生成测试；该选项省略测试文件和测试依赖 |

固定模板生成 `Cone.toml`、executable 的 `src/main.scoop` 或 library 的 `src/lib.scoop`，以及可选的 `tests/basic.scoop`。测试模板使用当前工具链支持的 `scoop.test`，并声明测试依赖。生产依赖沿用 manifest 现有 core 默认行为；本次 core/std 双默认依赖仅适用于单文件，不顺带改变普通 manifest 的默认值。

首版不增加模板下载、模板插件、用户模板语言或自动发布功能。先校验全部选项与目标冲突，再写文件；要求目标目录不存在，拒绝覆盖已有文件。模板中的注释和标识符按仓库约定使用英文。

生成后无需修改即可 build；含测试的模板可 `scoop test --release`；executable 模板可 run。library 模板不生成假 main。

## shebang 与依赖注释

### 示例与坐标

```scoop
#!/path/to/scoop run
//! dep: scoop:scoop.cli:0.1.0
//! dep: scoop:scoop.http.client:0.1.0

fun main() {
}
```

坐标使用现有三段 `group:name:version`。原始需求示例中的 `scoop:scoop.http:client:0.1.0` 是四段，不符合当前 coordinate 规则；对应现有 HTTP client Cone 的名称是 `scoop.http.client`。

### shebang 词法

D05 由共享 lexer 处理文件开头的 shebang，所有源码模式均适用；不新增 BOM 或前导空白容忍。

仅当文件首字节开始为 `#!` 时，跳过到行尾或 EOF。行尾继续按原有换行规则处理；不从源码中删除整行、不创建改写后的临时源码，确保后续诊断、SourceLocation、文档源码页的行号和字节 span 不变。shebang 内容不进入文档注释，也不由 compiler 执行为 shell 命令。

源码注释支持与操作系统直接执行是两个验收点。直接执行要求真实可执行脚本及有效解释器路径；在支持 `env -S` 的环境可用 `#!/usr/bin/env -S scoop run`，由 `env` 拆分解释器参数，见 [GNU env 文档](https://www.gnu.org/s/coreutils/manual/html_node/env-invocation.html)。

### 依赖注释的识别

仅在显式单文件 build/run 及对应单文件 scoopc 输入中解释依赖头部。普通 manifest 编译、测试和文档生成将 `//!` 作为普通注释，不解析其中坐标，也不因其依赖文本无效而报错。

D06 采用以下规则：

- 头部从可选 shebang 之后开始，到首个语言 token 之前结束；允许空白、空行和普通注释穿插。
- 只读取真实行注释中的 `//! dep: <coordinate>`；不读取块注释、文档注释正文或字符串内看似相同的文本。
- package、import 或 declaration 一旦开始，后续 `//!` 行按普通注释处理，不再补充依赖；按此规则，单文件中被忽略的 package 声明也结束依赖头部。
- `dep` 指令缺少冒号、缺少坐标或坐标非法时，诊断对应原始行；不静默降级为无依赖脚本。
- 其他 `//!` 文本仍为普通注释；不把全部 `//!` 变成一个需注册指令的语言。
- 同一依赖在源码中重复声明时报错；同一 group:name 的不同版本进入现有版本冲突规则。显式声明与默认 core/std 相同的坐标时合并依赖边，不把默认注入视为一次源码声明；不允许通过冲突版本静默替换默认依赖。

头部读取需区分“没有 dep 声明”与“存在但非法的 dep”，后者不能回退到默认依赖。读取后生成带原始 span 的 coordinate 列表，复用现有 coordinate 与 SemVer 校验。可以在 `scoop-manifest` 的单文件输入模块增加小型头部读取函数供两个 driver 入口调用，不扫描完整语法，也不另建通用预处理框架。

### 默认依赖与显式依赖

无论是否存在 dep 声明，单文件都默认直接依赖 core 和 std。按本次代码基线，对应坐标为 `scoop:scoop.core:0.1.0` 和 `scoop:scoop.std:0.1.0`；实现复用工具链现有内置库版本选择规则，不在多个入口各自维护一组默认坐标。

默认依赖与显式依赖都不自动增加 import，也不改变既有 core prelude 行为。std 进入默认 direct set 后，其 API 仍按正常导入规则使用。找不到所需 std 产物应报定位或缺失依赖错误，不能静默退回 core-only。

D10 已确认：生效直接依赖集合为 `{core, std} ∪ 显式 dep`。dep 只追加依赖，不替换或移除 core/std。增加第一条或删除最后一条 dep 都不改变 core/std 的直接依赖身份；例如仅声明 `scoop.cli` 时，直接依赖仍包含 core、std 和 cli。

图发现、scoopc 输入校验和缓存统一使用按上述规则得到的生效直接依赖集合，不分别推断默认值。

### 依赖发现与产物

D07 首版仅支持精确 coordinate，复用显式 `--cone-path` 和现有 sysroot 默认库查找。不给注释增加 path/artifact locator、版本范围、registry 下载或新的锁文件。缺失产物按现有定位规则报错，不能隐式读取随包源码重建。

单文件构建图包含生效直接依赖及所需传递闭包；core 和 std 始终都是直接依赖。传递依赖只提供正常的编译和链接支持，不自动获得 direct import 可见性。依赖库的 native 需求沿现有产物闭包处理；文件旁边的 C/C++、blob 或 manifest 仍不被发现。

| 实现位置 | 需要贯通的行为 |
| --- | --- |
| [single_file 输入](../../compiler/manifest/src/single_file.rs) | 读取和定位依赖头部，将显式依赖合入默认 core/std 集合，保留原始源码及 span |
| [CLI 请求](../../compiler/scoop/src/cli/commands.rs) | 允许单文件使用 `--cone-path`，不改变 root 分类 |
| [构建图发现](../../compiler/scoop/src/discovery/builder.rs)及 [graph](../../compiler/scoop/src/graph.rs) | 增加默认及显式依赖边，解除“只能有一个 core 边”的限制，保留环、版本、library kind 等规则 |
| [子编译请求](../../compiler/scoop/src/snapshot/prepared/model/child_request.rs)及 [协议校验](../../compiler/protocol/src/request.rs) | 传递并接受单文件 direct/support 产物 |
| [scoopc 请求](../../compiler/driver/src/request.rs)及 [direct set 校验](../../compiler/driver/src/request/preflight/dependencies/graph.rs) | 以统一的生效直接依赖集合核对实际输入，无论是否有 dep 都包含 core/std |
| [产物与链接分类](../../compiler/slib/src/manifest/production/projection.rs) | 放宽 core-only 条件，保留唯一 source、executable 和 LocalExecutableRoot 属性 |
| [编译缓存输入](../../compiler/scoop/src/snapshot/prepared/model/cache_key.rs) | 纳入实际依赖选择及结果，验证依赖变化会触发所需更新 |

直接调用 scoopc 不新增包定位或构建依赖职责：沿用 core 的现有处理，由调用者提供其余所需 direct/support slib，scoopc 使用相同头部语义核对所需集合。所有单文件输入都需要 std；`scoop` 负责按现有查找流程提供它，直接调用 scoopc 的测试或工具调用也需提供对应产物。旧单文件的“直接依赖只有 core”断言随新默认值迁移。

单文件保留 coordinate `scoop:single-file:0.0.0` 和 logical source path `main.scoop`。允许单文件依赖库，不改变其不能被其他 Cone 依赖、不能作为普通可分发 Cone 发布的约束。显式 link 必须能消费带依赖的单文件 root。

### 缓存与读取顺序

头部依赖决定构建图，需在编译前可用。沿用 [manifest 解析记录](../../compiler/scoop/src/cache/manifest.rs)的更新原则，以实际输入路径、mtime、大小及解析规则版本查找单文件头部记录。命中先复用解析结果，不先读取源码验证；输入变化时重新读取头部并发现依赖。

编译更新判断继续使用已有源码状态、目标、profile、配套工具及依赖结果。默认依赖策略纳入现有解析规则或缓存版本更新，避免未改源码时复用旧 core-only 构建图。实际编译时读取源码，并在同次 scoopc 工作中复用该文本处理头部与语法。头部记录用于正常更新，不增加内容认证、来源审计或缓存破坏恢复。

需要覆盖无变化复用、只改源码正文、增加首条或删除最后一条 dep、替换依赖版本或产物，以及默认 core/std 产物变化的行为；增删 dep 后 core/std 始终保留。单文件有无 package 声明使用相同默认命名空间，但原始源码不同仍参与正常更新判断；不要求二者生成相同缓存指纹。package 警告的缓存展示沿用既有诊断约定。失败不发布新产物，不运行旧 executable。验证保留源码、工具和产物，不以删除或隐藏输入来证明缓存命中。

协议或产物编码是否需要升版，由最终结构变化和现有兼容规则决定；不能仅因开放依赖就预先要求 runtime ABI 变更，也不能遗漏旧工具拒绝新请求的兼容检查。

## 单文件 runner 的参数规则

### 已确认的解析模型

进入程序参数之前，保留 scoop 自身选项的解析。显式单文件 root 后，满足以下任一条件即开始转发：遇到分隔符 `--`，或遇到第一个程序位置参数。开始转发后，剩余参数不再由 scoop 解析。

| 调用 | runner 行为 | 程序收到的参数 |
| --- | --- | --- |
| `scoop run file.scoop Alice Bob` | 使用默认构建 profile | `Alice`, `Bob` |
| `scoop run file.scoop -- Alice Bob` | 消耗分隔符 | `Alice`, `Bob` |
| `scoop run --release file.scoop Alice` | release 构建 | `Alice` |
| `scoop run file.scoop --release -- Alice` | 保留既有选项位置 | `Alice` |
| `scoop run file.scoop --help` | 显示 runner 帮助 | 不运行程序 |
| `scoop run file.scoop -- --help` | 消耗分隔符 | `--help` |
| `scoop run file.scoop Alice --release` | Alice 已触发转发 | `Alice`, `--release` |
| `scoop run file.scoop Alice -- Bob` | 转发状态中不再次消费 `--` | `Alice`, `--`, `Bob` |
| `scoop run file.scoop -- --` | 消耗第一个分隔符 | 一个字面的 `--` |
| `scoop run -- Alice` | 使用当前目录的 Cone.toml | `Alice` |

shebang 直接执行遵守相同规则：`./file.scoop Alice` 和 `./file.scoop -- Alice` 均可；程序自己的首个 `--help` 用 `./file.scoop -- --help` 传入。

省略 root 的调用保留原语义：`scoop run -- Alice` 不能把 Alice 误判为 root。普通 Cone 目录及 Cone.toml 的 run 入口继续使用既有显式 `--` 规则，不随本次单文件便利写法扩大语义。

D08 规定：在转发开始前遇到 scoop 不认识的 `-` 开头参数时报 runner 参数错误；程序选项使用 `--` 开始转发。已确认的是“与 runner 已知选项冲突时用 `--`”，不以猜测用户意图消解冲突。

### 实现约束

CLI 保留 `last = true` 的分隔符约定；单文件 run 在首个普通程序位置参数前补入分隔符，再交由原参数解析处理。以本节行为表为合同，普通 Cone 与显式 `--` 的语义保持一致。

参数预处理若有必要，应限于 run 的单文件规则，复用既有选项定义；不能通过扫描任意 `.scoop` 字符串寻找 root，因为它也可能是 `--cache-dir` 等选项的值。使用 `OsString` 保留参数边界和平台字节，不拼接 shell 命令。[现有程序启动入口](../../compiler/scoop/src/cli/run.rs)可继续使用。

## GC stress 构建开关

`scoop --enable-gc-stress` 显式允许构建含 GC stress 支持的程序。该参数也可放在 build/run/test/link 子命令之后、程序参数转发开始之前；不改变已有 profile 选择。未指定时，生成程序不包含 stress 专用实现，设置 `SCOOP_GC_STRESS_MOVE` 或 `SCOOP_GC_STRESS_MINOR` 均无效。指定参数只编入能力，不自动开启压力模式；运行时对应环境变量为 `1` 时启用，为 `0` 或未设置时关闭。

当前 stress 路径位于 runtime。按构建配置编译普通和支持 stress 的 runtime 对象，默认版本在编译时排除 stress 环境读取及专用分支，不以运行时恒假的开关代替。普通 GC 的移动、safepoint、屏障和正确性检查保持原合同；仅 stress 调度、强制搬迁与其专用验证受此开关控制。没有 stress 插桩的 Scoop 库无需仅因最终 runtime 选择而重编译。

开关贯通 CLI、runtime 源码构建、预编译 runtime 选择、对象配置和最终链接缓存。开发 sysroot 准备脚本显式准备两类 runtime；普通应用只消费选定版本，不临时重建发行资源。显式 runtime 索引与请求能力不匹配时报配置错误，不能通过环境变量或缓存命中改变能力。实际产物配置和相关缓存键区分两种 runtime，普通 `.slib` 与语言身份不为此增加新语义。

验收覆盖：默认构建忽略 stress 环境；启用支持后以环境变量分别打开和关闭；相同输入切换构建开关时选择正确 runtime 并更新链接，重复相同选择可复用；普通生成程序不含 stress 环境读取及专用路径。runtime 的定向 C 测试和 GC fixtures 显式选择所需版本，普通测试使用默认版本。该规则为用户新增的已确认要求，不新增待决策项。

## 测试组织与迁移

### 职责和默认模式

| 职责 | 推荐位置 | 执行方式 |
| --- | --- | --- |
| 内置库 API 行为 | `sysroot/src/<cone>/tests/` | `scoop test --release`，正常 GC |
| 库 API、错误路径与普通 consumer | 各内置 Cone 的 `tests/`；必要时独立测试 Cone | 必须使用 `scoop test --release`；消费生产库本身不是 fixture 理由 |
| 必须从外部控制的库集成边界 | `tests/fixtures/libraries/` | release；仅保留测试进程内无法完成的实际断言 |
| 普通语言语义和诊断 | `tests/fixtures/language/` | release；负例断言真实诊断及位置 |
| CLI、输入和构建行为 | `tests/fixtures/driver/` | release；保留默认 profile 等命令合同专测 |
| 产物、链接、ABI、native 互操作 | `tests/fixtures/compiler-correctness/` 的相应功能组 | 缺省 release；跨 profile 等专测明确声明必要组合 |
| 优化和编译器正确性 | `tests/fixtures/compiler-correctness/` | 明确声明需要的 debug/release、跨 Cone、混合优化等组合 |
| GC 正确性 | `tests/fixtures/gc-correctness/` | 按场景选择正常、moving stress、minor/full 或并发组合 |
| 缺陷回归 | `tests/fixtures/regressions/<defect>/` | 缺省 release；确需编译器或 GC 矩阵时用现有 tags 标明原因 |

“编译器正确性组”指需要特定模式或结构断言的用例，例如优化前后语义一致性、跨 Cone 泛型及 ODR、ABI 混合产物、异常清理、statepoint relocation。普通语言结果测试仍保留，不因也能暴露编译器 bug 而全部扩展成矩阵。

GC 测试也不是机械地覆盖所有模式笛卡尔积：每个变体需对应具体的根、屏障、搬迁、pin、FFI 或并发正确性事实。旧的 `gc`、`moving` 标签不能直接证明测试属于 GC 专测，需审查实际断言。

release 指被测 Scoop 产物的 profile，与执行 runner 的 Rust 工具位于 `target/debug` 或 `target/release` 是不同问题。D09 只调整仓库测试调用策略，保留用户命令的 debug 能力及既有默认值。

### 内置库迁移

优先迁移 `scoop.cli`、`scoop.json` 的纯库行为 fixture，并补充现有 `scoop.std` 测试。已使用 `scoop.test` 的库，逐组消除与 fixtures 重复的 API 断言。`scoop.test` 的 [执行绑定已有当前 Cone 分支](../../compiler/hir-lower/src/test_entry/bindings.rs)，可以为库自身添加测试，而不声明对自身的依赖。

core 直接测试依赖 `scoop.test` 会形成 `core → scoop.test → core` 的图环。使用独立测试 Cone 同时消费生产 core 和 scoop.test，测试公开库行为；需要 core 编译语义、内部实现或 GC 契约的测试保留在相应专测中。

fixture 是 `scoop.test` 无法完成的验证载体，不是普通测试手段。库 API 契约、正常/错误路径、边界值及仅调用库功能的 consumer 都放入所属 Cone 的 tests；测试源码同样能够通过依赖消费生产库。`consumer`、`artifacts`、`prebuilt` 等名称、构建后运行的外壳、顺带重复的冷/热缓存检查都不构成保留理由。

编译失败及诊断位置、实际产物结构、跨 Cone 可见性规则、构建缓存、显式链接/ABI、CLI 退出行为，以及必须由外部进程控制的 PTY 或协议对端，才保留相应 fixture。使用 native 库、文件或网络本身也不是理由：能在 Cone 测试中准备输入、调用 API 并验证结果的仍使用 `scoop.test`。混合用例先拆出库行为，每个保留 fixture 应能说明测试进程内无法完成的具体断言。

普通编译错误按编译器规则集中为代表性的 expected-error fixtures，不为每个库再建 negative 组。方法不存在、参数类型错误、泛型边界、不可变接口写入、可见性和不变性等不能仅因涉及某个库就视为该库的独立契约。先核对既有语言负例的规则和必要组合，已有覆盖直接去重，缺少的补入对应语言规则组。派生编码、注解查询等编译器内建语义中独有的非法形态保留在相应编译器语义组；运行时可断言的库拒绝行为迁入 Cone 测试。

测试框架自身的过滤、列表、生命周期和统计验证，应使用小而稳定的测试 Cone；避免在 `scoop.std` 增加正常测试时，迫使不相关 CLI fixture 更新固定的全库用例数量。

### 执行策略与目录迁移

现有 runner 未指定 variants 时执行 normal，而普通 CLI 默认 profile 为 debug。删除 debug 变体并不能自动得到 release：普通 `scoop build/run/test` 和直接 `scoopc build` 调用均需显式选择 release，并同步调整输出路径断言。

[runner](../../tests/fixture_runner/execute.py) 在启动测试子进程时清除继承的 `SCOOP_GC_STRESS_MOVE` 与 `SCOOP_GC_STRESS_MINOR`，再由 GC 专测的变体或步骤显式设置。其他父进程环境继续继承，用户直接执行 scoop 的环境传递保持原有行为。

复用现有 fixture schema、steps、variants 和 tags，不新增第二套 runner。先确定专测分组及迁移对应关系，再减少普通模式覆盖；不要按目录名或是否含字符串 `moving` 批量删除用例。

fixture runner 直接使用已显式准备的 sysroot，默认不复制整棵目录。`--sysroot`、`SCOOP_SYSROOT` 和仓库 sysroot 依次选择资源位置；runner 不自动打包或重建系统库。`${cache}` 跨 fixture 和不同 work-dir 共享，通过 `--cache-dir`、`SCOOP_CACHE_DIR` 或仓库 `target/fixture-cache` 选择，输入及工具变化仍由正常构建缓存判定。临时程序、协议数据和退出状态继续在各 fixture 的 work 中隔离。

不为每个库重复执行 core/std/json 等系统库 build 或附带一次 warm build。普通源码消费者直接引用预编译依赖；编译器正确性中确实修改系统库源码或验证发布/缓存的少数用例，明确保留独立准备步骤。HTTP client 验证应分别观察系统依赖是否复用及根消费者的实际编译成本，不把仍需编译根程序误报为系统库重建；通过迁入 Cone 测试合并普通 API 场景，减少大型可执行文件重复生成。

修改 core 的专测还需要构建依赖该版本的 std，不能混用发行 std；声明中说明这个依赖关系，构建仍使用共享 cache，由正常输入变化决定是否更新。保留必要构建步骤不等于为该用例建立一套独立冷缓存。

最后按功能或缺陷搬迁目录，修改活动脚本、suite/filter、相对输入、期望文件和文档链接。以功能为主的 regression 可保留 issue/commit 说明；没有 issue 的缺陷使用明确的行为名称。历史里程碑文档保留历史语境，必要时补新位置链接，不重写旧验收结果。Rust 单元测试文件的里程碑命名不属于本次 fixtures 目录迁移范围。

## 边界决策

D02、D03、D10 来自用户明确确认。其余边界在用户授权实施整个设计后，按原推荐方案选定；本批没有等待确认的边界。

| 编号 | 采用的规则 |
| --- | --- |
| D01 | 除 core/std 外，manifest 源码必须显式声明合法 package |
| D02 | core/std 豁免 package/group 前缀约束 |
| D03 | 单文件使用内置默认 package；显式声明警告后忽略 |
| D04 | 固定 executable/library 模板；group 必需，默认 executable、0.1.0、package=group，生成测试，可用 `--no-tests`；目标目录必须不存在 |
| D05 | 所有源码模式接受首字节 `#!`；不新增 BOM 或前导空白容忍 |
| D06 | 首 token 前的注释区读取 dep，package 也结束头部；源码中重复 dep 报错；未知 `//!` 和头部外指令仍为普通注释 |
| D07 | 精确坐标，使用 `--cone-path` 和既有 sysroot 默认查找 |
| D08 | 转发前未知连字符参数报 runner 参数错误；使用 `--` 传给程序 |
| D09 | 仓库普通测试使用 release；保留产品 CLI 的 debug/default 合同 |
| D10 | 单文件始终直接依赖 core/std，dep 只追加 |

## 完成条件

- 已确认的 package 前缀规则覆盖生产、测试、文档入口，core/std 豁免且不扩散；D01 写入规范并落实迁移。
- 单文件省略 package 时使用内置默认 package；显式合法声明只产生警告，不改变命名空间，也不单独导致构建失败。
- `scoop new` 的每种交付模板可直接构建，按模板用途运行和测试；非法输入不会覆盖已有文件。
- shebang 不改变后续源码位置；依赖注释在单文件中生效、在普通 Cone 中无效，不形成 API 文档说明；源码页仍可原样显示这些注释。
- 单文件始终默认直接依赖 core/std，显式 dep 只追加依赖。依赖发现、scoopc、缓存、`.slib` 和显式链接完成闭环，保持本地 root 属性和相邻文件隔离。
- runner 的行为表成立，既有 `--` 与省略 root 的调用不回归，参数字节和边界保持不变。
- 普通库测试和普通 fixtures 只跑 release；保留必要的编译器矩阵和专门 GC 模式，父进程 stress 环境不会污染普通测试。
- `--enable-gc-stress` 控制生成程序是否包含 stress 支持；默认版本忽略 stress 环境变量，启用支持的版本可由环境变量打开和关闭，runtime 选择与缓存正确区分。
- fixture 迁移有可核对的旧新职责对应，负例仍验证原本的语言错误，活动入口和路径引用有效。
- 通过实施计划规定的分批检查及最终目标平台验收；原始日志放仓库外，仓库文档只保留结果概要和实际限制。

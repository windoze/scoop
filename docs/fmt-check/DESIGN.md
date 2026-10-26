# `scoop fmt` 与 `scoop check` 设计

状态：两项命令均已实现并通过本机定向验证，完整 Linux 验收进行中。设计日期：2026-10-26。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md) §2.1、§2.2、§2.7、§2.20，以及[语言服务公共基础](../language-service/DESIGN.md)。本设计不改变语言语义、`.slib` 格式或[运行时契约](../specs/SCOOP-RUNTIME-SPEC.md)。本文按命令合同描述目标；当前交付范围以文首状态及验证记录为准。

## 1. 目标与首版决定

提供项目日常开发和 CI 所需的源码格式化、格式检查与完整前端检查。格式化复用现有语法；检查复用正常编译的语义完成过程。

| 项目 | 首版决定 |
| --- | --- |
| 格式化入口 | `scoop fmt [input...]`，默认写回；`--check` 只判断是否需要修改 |
| 格式化输入 | 当前 Cone、多个显式 Cone 或 `.scoop` 文件；Cone 包括生产、测试及各平台声明的源码 |
| 格式化风格 | 四空格缩进、100 列软行宽、固定规则；保留非空白 token 与注释原文 |
| 检查入口 | `scoop check [root-input]`；低层提供 `scoopc check` |
| 检查范围 | 默认 Production；`--tests` 选择根 Cone 的 Test 用途 |
| 检查停止点 | 根 Cone 完成 ExportHir 和 LocalConcreteHir 后返回，不进入 MIR |
| 源码依赖 | 复用现有 `.slib` 缓存，需要更新时正常构建完整 library |
| 结果 | 复用 human／JSON 诊断；格式差异和检查成功有独立结果，不伪造编译产物 |
| 缓存 | 格式化不新增持久缓存；检查使用普通本机成功记录，并复用依赖构建缓存 |

首版不包含任意目录递归／workspace／glob 选择、stdin 格式化、格式配置文件、import 排序或删除、代码修复、语法错误文件的部分格式化，以及 LSP formatting／range／on-type 接入。整个依赖图都只生产语义结果属于后续独立设计，不能为了首版检查增加残缺 `.slib`、第二套类型检查器或通用任务框架。

## 2. 已有基础与实际缺口

| 代码位置 | 已有能力 | 本次工作 |
| --- | --- | --- |
| [公共 CLI](../../compiler/scoop/src/cli/args.rs)、[命令执行](../../compiler/scoop/src/cli/commands.rs) | New／Build／Run／Test／Link／Doc 及外部子命令 | 新增独立 FmtArgs／CheckArgs、分发与退出结果 |
| [源码发现](../../compiler/manifest/src/discovery.rs)、[选择规则](../../compiler/manifest/src/discovery/select.rs) | manifest 路径、target 条件、生产／测试角色和确定顺序 | 复用路径枚举，增加格式化的全平台文件并集入口 |
| [parser](../../compiler/parser/src/lib.rs)、[syntax](../../compiler/syntax/src/kind.rs) | 同次解析返回 CST、诊断和完整 AST，保留 trivia | 新增格式库；缺少的排版语法角色在原 grammar 中记录 |
| [HIR lowering](../../compiler/hir-lower/src/current_lowering.rs) | 完整正文、generator、Export HIR 和具体化 | 检查复用完整 lowering，保留所有前端诊断 |
| [具体化](../../compiler/hir-lower/src/concretize/run.rs) | 仍可产生类型条件与递归错误 | 首版检查不能提前停在 Export HIR |
| [driver 编译路径](../../compiler/driver/src/request/preflight/production.rs) | HIR 后 observation／documentation 停止点 | 加入正式无产物检查分支，共享诊断完成步骤 |
| [文档编排](../../compiler/scoop/src/document/compile.rs)、[目标选择](../../compiler/scoop/src/discovery/target.rs) | 前端根与机器依赖分开，按需准备生产目标 | 将实际共有的前端目标选择用于 doc／check |
| [请求输出](../../compiler/protocol/src/request/output.rs)、[成功结果](../../compiler/protocol/src/response/product.rs) | Artifact／Documentation／两者同时 | 增加 Check 闭合分支及配套协议版本 |

现有 HIR observation 仍要求 dump，普通请求仍存在主输出路径和临时产物目录假设。`check` 不能通过隐藏 `--emit hir`、解析 dump 或传入虚假 `.slib` 路径实现。

编辑态分析允许部分事实不可用，也不执行 generator。`scoop-analysis` 的诊断适合交互查询，不能直接用其可用诊断集合宣告一次完整编译检查成功。

## 3. 命令合同

### 3.1 公共输入与退出规则

无参数时，两个公共命令都使用调用者 cwd 的 `Cone.toml`，不向父目录搜索。所有显式相对路径从 cwd 解析；已有内置命令优先于 `<SYSROOT>/commands/scoop-<name>` 的规则保持，因此 fmt／check 实现后成为内置名称。

| 结果 | 退出码 |
| --- | --- |
| 格式写回全部成功、格式检查没有差异，或语义检查成功 | 0 |
| `fmt --check` 存在格式差异 | 1 |
| 格式化的输入、语法或 I/O 失败；语义检查或必要依赖构建失败 | 1 |
| CLI 用法错误或互斥参数 | 2 |

中断继续使用现有进程管理及信号退出约定，不把中断改写成成功。两命令的 stdout 均为空，诊断、文件状态和最终结果写 stderr；默认 human，`--message-format json` 使用现有 schema 1 记录流。

### 3.2 `scoop fmt`

```sh
scoop fmt
scoop fmt ./app ./libs/model
scoop fmt ./app/Cone.toml ./scratch.scoop
scoop fmt ./app --check
scoop fmt ./app --check --message-format json
```

位置参数是零个或多个 Cone 目录、恰名为 `Cone.toml` 的文件或扩展名精确为 `.scoop` 的文件。目录必须具有自己的 Cone.toml；不把 `scoop fmt src/` 隐式解释为任意目录遍历。显式 `.scoop` 文件是普通语法文件，不要求它是可运行 single-file 程序，不要求 main、有效依赖或 package/group 语义关系。

首版专属选项仅为 `--check` 和 `--message-format`。不接受 target、profile、依赖、产物输出或 generator 选项；格式化不解析 sysroot 或访问构建缓存。现有全局 `--sysroot`、`--enable-gc-stress`、`--enable-startup-stats` 可以沿 CLI 语法被接收，但对格式化无作用。

### 3.3 `scoop check`

```sh
scoop check
scoop check ./app
scoop check ./app/Cone.toml --tests
scoop check ./script.scoop --target aarch64-apple-darwin
scoop check ./app --generator model=./tools/model-generator --message-format json
scoop check ./app --release -j 4
```

只有一个可选 root，支持与 build 相同的 ManifestCone／SingleFile。manifest 按当前 target 选择源码；SingleFile 沿用现有 core、std、头部依赖及 package warning 规则，不读取相邻 manifest。

| 选项 | 行为 |
| --- | --- |
| `--tests` | 将根切换为 Test 用途；只接受 manifest，生产源码与测试源码共同检查，测试依赖仅对测试源码可见 |
| `--target`、`--sysroot`、`--cone-path` | 沿正常目标和依赖选择；不因为检查而改用宿主或源码版系统库 |
| `--cache-dir`、`--scoopc` | 沿现有本机缓存位置与配套编译器选择 |
| `--generator NAME=PATH` | 沿现有生成程序注册；实际待填正文需要生成时执行 |
| `--profile debug\|release`、`--release` | 与 build 相同默认值及互斥关系，用于必要的源码依赖机器产物；根不执行优化阶段 |
| `-j`／`--jobs` | 沿现有独立源码依赖调度，根检查在其所需依赖成功后执行 |
| `--cc`、`--native-sysroot` | 仅供需要重新构建的源码依赖选择 native 工具 |
| `--message-format` | human／json |

`--tests` 选择一次 Test 检查，不隐式串行执行 Production 和 Test 两轮。其入口为已有 generated TestEntry，故不验证 executable 的生产 main 是否存在；CI 如需覆盖两种用途，应分别调用默认检查与 `--tests`。Ignore 或测试运行过滤不缩小检查集合，检查期间不执行测试程序。

不提供 `-o`、`--target-dir`、`--emit`、dump、runtime 或最终链接参数。现有全局 runtime 开关对检查无作用，不进入检查键；`--cc` 等输入也不触发根 Cone 的 native 编译。首版没有 warning 升级为错误的新政策。

### 3.4 `scoopc check`

```sh
scoopc check ./app --direct-slib ./deps/model.slib --support-slib ./deps/base.slib
scoopc check ./app --test --direct-slib ./deps/scoop.test.slib
```

命令接受必需的当前 Cone 输入、`--target`、`--sysroot`、`--direct-slib`、`--support-slib`、`--generator` 和 `--test`。示例中的依赖路径仅展示参数形式，调用者仍须提供真实完整依赖集合。

与低层 build 一样一次只处理一个 Cone，保留已有默认 core 选择，不定位其他缺失依赖，不跟随上游源码 locator。它不需要 `--out-slib`、profile 或 C 工具参数，直接使用共享 target registry；human 诊断与父进程调用的结构化诊断来自同一结果。

## 4. 格式化设计

### 4.1 文件选择

manifest 的解析与字段、路径、target 谓词合法性使用现有实现，但格式化不建立依赖图，也不核对依赖 locator 是否能构建。

1. 生产源码未声明 `sources` 时扫描 src/；声明后只取其显式路径。
2. 测试源码使用同样的缺省／显式规则。缺省 tests/ 不存在时忽略；显式路径缺失或类型不符正常报错。
3. 显式清单中的路径全部进入候选集合；target 条件只验证合法性，不参与格式化过滤。各文件独立格式化，不把互斥源码放进一个编译 Cone。
4. 复用现有目录遍历、扩展名检查、路径解析和循环错误处理。格式化结果可以为空；缺失的默认 src/ 仍是路径错误，不等价于空目录。
5. 条件路径互相重叠、多 root 重复或显式文件已被 Cone 包含时，按解析后的实际文件路径只处理一次。输入按参数顺序展开，Cone 内按标准化相对路径排序，重复时保留首次位置。

并集入口不调用“必须组成一个合法目标编译集合”的最终合并检查。正常 build／check 对同一 target 内重叠路径、重复源身份及生产集合非空的规则保持；`fmt` 成功不表示 manifest 能编译。去重只需要已有普通路径解析，不比较 inode、不审计来源或目录权限。

不额外扫描 examples、缓存、target、依赖或 sysroot；只有显式路径清单确实选择这些位置时才遍历。首版不加入 ignore 文件或 glob 机制。

### 4.2 解析与模块边界

在 `compiler/fmt` 新增 `scoop-fmt`，作为语法数据到文本的变换库。调用方读取原文，调用现有 `parse_recovering` 一次，确认无语法错误且具有完整 AST 后，将同次解析的 CST 交给格式库。

格式库的编译器数据依赖为 `scoop-syntax`，不依赖 parser 实现、HIR、manifest、LSP 或 LLVM。原文 token 由同次解析的 CST 提供，不建立第二份语言 grammar；输出为完整格式化文本。文件读取、路径、写回、退出码均属于 `scoop` 编排。

排版所需语法角色和分组由同次解析的 CST 提供，完整 AST 的存在确认该文件解析成功。原 parser production 记录一元／二元／postfix 操作符、下标、copy update 和单语句控制体边界；格式库因此只需依赖 `scoop-syntax`。不能从 token 拼写猜测类型／表达式角色，也不能扫描另一遍源码另建解析器。仅为当前排版需要增加视图，不搭建通用语法访问框架。

排版复用 MIT 许可的 [`pretty` 0.12.5](https://github.com/marwes/pretty.rs) 文档组合库，其近期维护包括深层 append 渲染的栈修复。使用其文本、缩进、软／硬换行和成组折行，显式指定 Unicode 标量宽度；渲染器区分原 token 与布局空白，使换行风格转换不触及字符串和注释内部字节。

### 4.3 固定格式规则

| 语法／文本 | 规则 |
| --- | --- |
| 缩进 | 每层四个空格；续行增加一层，不按名称长度做垂直对齐 |
| 行宽 | 100 列软目标，按 Unicode 标量计数；长 token、注释或受语法限制的行允许超宽，不修改字面量 |
| 换行 | 原 whitespace token 中的换行若全部是 CRLF，则保留 CRLF；其他情况，包括混合、仅 CR 或无换行，使用 LF；token 内部换行不参与判定且保留 |
| 文件边界 | 去掉文件外层多余空白；纯空白文件变为空文本，其他结果以一个换行结束；shebang 始终位于 byte 0 |
| 声明与 import | 保留顺序；package、import 区及声明区之间一个空行，顶层声明之间一个空行，连续 import 不逐条插空行 |
| 块 | 非空普通声明体／语句块换行并缩进，结束括号回到所属层级；空块使用 `{}`；不自动给单语句体加大括号 |
| 表达式体与 lambda | 短表达式体和单表达式 lambda 在语法允许且行宽足够时保持单行，其余在已有安全边界折行 |
| 参数、实参、类型参数、tuple、数组 | 单行时逗号后一个空格；需要折行时按项换行并缩进，闭合符号单列；保留已有尾逗号 |
| 操作符与类型 | 二元／赋值操作符两侧一个空格，逗号和类型冒号后一个空格；一元、postfix、路径及泛型标点按 AST 角色处理 |
| 控制结构 | 控制关键字与条件括号间一个空格；保持 else、catch、finally、do/while 与所属结构的关系；已有单语句体单独缩进 |
| `when` | 非空分支区逐分支排版，箭头两侧一个空格；模式、guard、单语句体及块体保留原结构 |
| annotation／修饰符 | 保留顺序；声明前置 annotation 单独排版，参数及类型位置的 annotation 留在对应项中；不改兼容修饰符 |
| 空行 | 块内原有空行至多保留一个；不跨 token 搬运注释或拆开文档与附着目标 |
| 标点与 token | 不增删分号、逗号或括号，不排序 import，不改数字拼写、转义标识符、字符串和字符字面量 |
| 注释 | 保留每个注释 token 的全部字节和顺序，不重排／重流 Markdown；可调整 token 外部缩进，行尾注释前保留分隔空格且后续代码另起一行 |

语义约束优先于上表的普通排版。所有换行和空格规则都需服从实际 grammar，不能为满足行宽而增删标点或改变解析结果。

例如：

```scoop
fun add(a:Int,b:Int):Int {return a+b}
```

格式化为：

```scoop
fun add(a: Int, b: Int): Int {
    return a + b
}
```

### 4.4 必须保留的语法关系

- `return` 与其返回值、jump 与 label 的同一行要求保持；不能把 `return` 后原本独立的表达式接成返回值，也不能在二者之间新增换行。
- callee 与调用左括号、显式类型实参的换行条件保持；下标左括号必须保持与 receiver 的相邻关系，不能把独立数组表达式接成下标。
- 尾随 lambda、when arm 边界、单语句控制体及 dangling else 使用原 AST 的归属，不只按大括号缩进。
- 类型参数的 `<...>`、比较表达式、一元／二元操作符、range、nullable 和安全导航按实际语法角色排版；相邻 token 不能因空白变化合并成另一个 token。
- f-string 的静态文本和分隔符逐字保留；插值表达式仅调整原 grammar 允许的空白，不改变插值边界、字符串值或普通转义。
- shebang 不移动；文档 token 的目标以及未附着状态保持，普通注释不能因排版变成文档注释。

首版必须覆盖所有现有合法语法类别。某种语法缺少规则时应补齐实现和用例，不能以整文件原样返回伪装格式化完成，也不能将占位分支作为正式功能交付。保留有语义作用的空白是格式规则的一部分，不是跳过格式化。

### 4.5 错误、比较与写回

每个候选文件只读取和解析一次。语法／词法错误使用原 byte span 报告，保持该文件不变；实际 I/O 错误同样定位到该文件。已确定的其他文件继续处理，最终统一返回失败。每个 manifest 先完成自身文件选择再加入处理集合；加载或选择失败时报告该输入失败，其他独立输入可继续，不使用失败 Cone 的半份文件集合。

正常排版后与原文直接比较；相同则不创建临时文件、不写回、不改变 mtime。写回使用同目录临时文件和 rename，保留原普通权限及可执行位；显式符号链接按正常解析的实际文件路径写回，保留链接本身。失败时不发布该文件的半截文本，不提供跨文件回滚，也不增加源码并发替换检测、来源审计或持久备份系统。

`--check` 运行相同排版并收集差异文件，完全不写源码。正常文件、待格式化文件和错误文件可同时出现在批量结果中；差异不生成伪造的 parser Error。

解析结构一致、token 保留与幂等性属于格式库的测试合同。正式命令不为每个文件重复解析输出、重新执行 HIR 或反复格式化到稳定。

## 5. 检查流水线

### 5.1 图与依赖

`scoop` 继续负责 root、locator、完整依赖图、版本／target 冲突与调度；`scoopc` 继续一次只检查或编译一个 Cone。检查不把 source dependency 移入根源码，也不把低层 driver 改成项目扫描器。

预编译库正常消费已选产物；源码库先判断已有完整产物是否可复用，需要更新时沿普通构建路径生成 `.slib`。生产依赖和测试依赖都按 Production 构建，不递归启用依赖的 tests。源码依赖中的语言、native 或产物生成错误正常使本次检查失败。

把现有 `GraphTarget::Documentation` 的实际共有部分收敛为前端目标选择，doc／check 共用；只有确实准备某个未命中的机器生产节点时才取得 `ResolvedTargetProfile`。根检查只需要 canonical target 与兼容依赖，不发现 C／C++ 工具、runtime 或最终 linker。根有 native 配置也不读取、预处理或编译其 native 文件；manifest 字段解析及 HIR 中实际涉及的 FFI 语义检查仍执行。

这意味着预编译依赖齐备时根检查可完全跳过后端工作；冷的源码依赖仍会执行后端。当前 scoopc 二进制本身依赖 LLVM，跳过后端阶段不等于本期交付一份不依赖 LLVM 的安装包。

### 5.2 根的语义完成

根检查依次完成：

1. 正常输入加载、目标与 direct／support 依赖闭包消费。
2. 同一 parser 的全部源码解析，以及现有 manifest package/group 检查或 single-file warning。
3. 原名称绑定、声明／签名、类型推断、默认参数、初始化及全部必要正文检查。
4. 必要的 generator 调用、返回正文解析／绑定、测试发现与 TestEntry 生成。
5. 原 effect、FFI、泛型递归及其他源码合同检查。
6. 正常 ExportHir、LocalConcreteHir 和 native-boundary 类型结果完成。
7. 汇集源码与生成片段 warning，返回检查成功。

第 6 步必须保留：现有 concretizer 仍可能报告类型条件和递归错误。首版不通过重写这部分职责来追求更早停止，也不复用编辑态“暂不可用”结果作为成功。

检查返回完整 lowering 已取得的结果，不继续构造只供 `.slib` 发布或机器布局使用的 metadata、MIR、LIR、对象和最终程序。当前 `CurrentConeHirArtifacts::from_hir` 混合了语义结果与产物投影，实施时把共有的 lowering／诊断完成留在前端路径，将仅生产需要的投影放在相应消费者入口；同一事实不重复检查。

默认检查 executable 时保留完整生产 main 规则。检查通过仅说明当前用途下的 Scoop 前端与必要依赖工作成功，未运行根的 native 编译、实际 ABI 发射或最终链接；缺失 native 符号、系统库及链接冲突仍由 build/link 报告。

### 5.3 generator 与诊断

沿用现有 GeneratorPrograms、执行协议、固定查询与完整返回正文检查。缺失注册、生成器失败、返回语法错误或语义错误均不能跳过；没有实际待填方法时不启动进程。

保留 generator 实际使用记录、生成正文文本、逻辑来源及注解关联，使错误仍指向具体片段；不能只把所有错误压到配置位置。源码 warning、生成 warning 及此前已产生的 single-file warning 在后续失败时继续报告。

生成程序为完成类型检查而执行，检查不执行根程序或测试用例。外部生成程序输入追踪沿现有合同，不另造任意文件／环境访问跟踪系统。

### 5.4 请求、结果与协议

在现有闭合枚举中增加无路径的 Check 意图及成功结果：

- `ScoopcOutputRequestV1` 和 driver 对应的 `SingleConeOutput` 增加 `Check`。
- `ScoopcProductV1` 和 `SingleConeCompilation` 增加检查成功分支，携带或复用现有 warning、使用的 generator 数据；不携带 artifact summary 或输出路径。
- Check 支持 Production／Test；不接受 documentation、stage dump 或已编译 native 对象组合。
- 请求归一化使用 `SelectedTarget::Frontend`。移除“任何成功请求必有 primary_path／产物临时目录”的假设；只有实际需要文件输出的路径才创建其工作目录。
- 父进程核对实际响应种类后取得检查结果；不把文档或 observation 成功当作检查成功。

设计时 machine protocol 为 16。实施时递增到下一个可用版本，追加新分支编码，不复用已分配 tag；同步 capability、codec、请求形状检查和版本用例。此处不修改 `.slib` profile、HIR wire 或 runtime ABI。

## 6. 检查缓存与更新

检查使用独立的本机 `checks-v1` 成功记录。它是普通更新数据，不是可分发语义产物或供下游编译消费的接口。

| 输入 | 更新依据 |
| --- | --- |
| 根与用途 | 实际 root／single-file 路径、coordinate、kind、Production／Test |
| 源码 | 已选集合、完整逻辑路径、角色、实际文件路径、mtime 与大小 |
| 配置 | 前端实际消费的 manifest 语义、当前 target、依赖选择；Production 不纳入仅测试配置 |
| 配套编译器 | 普通工具路径、mtime、大小及协议／能力配置，复用已有工具记录 |
| 依赖 | 实际 direct／support 选择、普通依赖输入键与需要的结果位置；机器依赖按现有 profile 构建记录更新 |
| generator | 注册映射与实际使用程序的已有输入记录 |

记录保存本次成功状态、warning、诊断显示所需来源，以及实际依赖和 generator 输入。根的 native 源码内容、runtime、最终链接配置不属于根前端输入；源码依赖自身的 native 变化仍按其正常构建键处理。依赖变动可保守使根检查失效，不为避免这一次检查而重读产物推导接口是否等价。

更新顺序固定为：已有 manifest／single-file 头部记录与输入 metadata → 根检查记录匹配 → 确需更新的依赖生产 → 根检查 → 成功记录发布。使用已有图和产物摘要记录判断依赖状态；不能在命中判断前先准备全部机器工具、读取源码、解码整份依赖产物或构造临时输入镜像。

命中时直接返回已有成功和 warning，不启动 scoopc／generator，不读取命中产物内容。显示带位置的 warning 时可按已有诊断规则按需读取相关源码；这不参与更新判断。只有依赖首次消费或输入变化确需读取时，才执行其必要格式与引用检查。

首次 check 即使已有成功 build，也可建立自己的检查记录；首版共享依赖机器缓存和检查自身结果，不读取 build 产物补造缺少的检查结果。反向也不把 check 记录当作可链接的 build 产物。

同键请求使用现有普通锁，锁内重查；成功后通过临时文件和 rename 发布。检查失败或取消不保存成功，也不返回先前成功来掩盖本次输入错误。已经成功的依赖遵循普通缓存生命周期。输入缺失和实际 I/O 错误正常报告，不增加缓存损坏恢复、生产证明或多份结果交叉验收。

## 7. 输出与 CLI 接入

### 7.1 格式结果

human 模式列出实际格式化／需要格式化／失败文件和简要计数。JSON 先输出实际诊断，最后输出一个 `kind: "format"` 结果，字段包括：

- `schema: 1`、`mode: "write" | "check"`、`success`。
- `files`：按确定顺序保存 path 及 `unchanged`、`formatted`、`needs-format`、`failed` 状态。
- `counts`：各文件状态的数量。

无法展开的 root 作为普通输入诊断报告，不伪造一个源文件；最终 `success` 同时考虑输入错误和文件状态。`--check` 有差异时 `success` 为 false 并退出 1，完整保留格式结果。路径沿用现有 HostPath 表达，非 UTF-8 宿主路径不有损转换。

CLI 需要表达“已有完整格式结果，但退出非零”，不能把它强制转成 `BuildFailure` 再额外打印一个无意义工具错误。只增加当前命令实际需要的退出结果表示。

### 7.2 检查结果

human 模式在 warning 后输出根 Cone 与用途的检查成功摘要。JSON 输出 `kind: "check"`、`schema: 1`，包含实际根身份、canonical target、用途、缓存是否命中，以及本次实际 child 调用观察。源码依赖的 artifact 编译与根的 check 调用明确区分；命中不能伪造调用。

检查结果不含 executable／`.slib` 输出路径，不使用 `library` 或 `executable` 成功记录。失败沿用现有结构化诊断，不输出成功检查记录。JSON 结果继续使用 stderr，fixture runner 只扩展必要的结果识别。

## 8. 实施拆分

| 批次 | 工作与完成条件 |
| --- | --- |
| A：合同 | 本设计、实现规范 2.7／2.20 和路线图一致，明确首版依赖构建、Test 及文件并集行为 |
| B：单 Cone 检查 | 正式 Check 请求／结果、协议版本、低层 CLI 和完整 HIR 停止；诊断与普通编译一致 |
| C：项目检查 | `scoop check`、前端目标复用、依赖调度、检查缓存、human／JSON 和正式 CLI fixtures |
| D：格式库 | 必要 CST 角色、固定布局与全部现有语法覆盖；token、解析结构、附着及幂等性测试 |
| E：格式 CLI | manifest 全平台文件并集、显式文件、写回／只检查、错误聚合、退出码和正式 fixtures |

`check` 的 B／C 可先交付。格式库与 CLI 在 D／E 完成之前不把未覆盖语法留给正式入口。新增 crate 注册到 Cargo workspace；将格式代码按声明、表达式、类型／模式、注释与布局等实际职责拆分，不把所有规则塞进 CLI。

主要实现位置为 `compiler/scoop`、`compiler/driver`、`compiler/protocol`、`compiler/manifest`、必要的 `compiler/parser`／`compiler/syntax` 及新 `compiler/fmt`。只有共享 HIR 完成接口确需调整时才改 hir-lower；不另建语义规则。LSP 的能力声明、运行时及发行包工具数量不因此变化。

## 9. 验收范围

### 9.1 格式库

| 类别 | 必需覆盖 |
| --- | --- |
| 声明 | package/import、注解、函数、property／accessor、构造器、class／struct／enum／interface／object／companion、扩展、条件 interface 实现、context 与 where |
| 表达式与语句 | 运算优先级、调用、命名／spread／vararg／尾随实参、lambda／局部函数、控制体、when 模式与 guard、try、jump、copy update、解构 |
| 文本 | 普通／文档注释、未附着文档、shebang、转义名称、Unicode、数值拼写、字符／字符串／f-string、LF／CRLF／混合换行 |
| 敏感空白 | return／label、调用换行、receiver 下标相邻、独立数组、尾随 lambda、when arm、相邻操作符的 token 合并 |
| 性质 | 输出满足小型可读 golden；再次格式化不变；去除位置后 AST 结构与文档附着一致；非空白 token 字节与顺序保持 |

只比较 AST 还不够：兼容修饰符等 token 可能不进入后续语义结构，必须同时覆盖 token 保留。测试中的位置归一化只用于比较结构，不掩盖表达式种类、分组、标点、注释内容或文档目标变化。

以仓库现有合法源码作为补充语料，在临时副本中检查上述性质；不将整仓库重新排版作为本次功能交付的一部分。纯格式规则用 Rust 单元／golden 测试，真实命令行为才使用 fixtures。

### 9.2 正式 CLI

| 命令 | 必需覆盖 |
| --- | --- |
| fmt 输入 | 默认 root、多个 Cone／文件、显式清单、全平台条件、生产与测试、重叠去重、空集合、缺失输入与目录循环 |
| fmt 写回 | 只改变化文件、第二次不改 mtime、普通可执行权限和符号链接行为、语法错误文件不改、独立文件继续、实际写入失败 |
| fmt 检查 | 合规退出 0、差异退出 1、源码不写回、错误与差异混合、human／JSON、参数错误退出 2 |
| check 基础 | library／executable／single-file、main 与 package 规则、warning、源码位置、target、default core/std、direct／support 依赖 |
| check 完整性 | 代表性的语法／名称／类型／effect 错误、具体化类型条件与递归、generator 成功及错误、生成片段位置；与 build 的前端诊断相符 |
| check 测试 | Production 不加载 tests，Test 加载正确集合与依赖，生产源码不可使用仅测试依赖，Ignore 用例仍检查，不执行测试程序 |
| check 阶段 | 根不产生机器产物、不调用 native／runtime／linker；预编译依赖直接消费；冷源码依赖构建完整 `.slib` 后根检查 |
| check 复用 | 重复命中、源码／源码集合／依赖／实际 generator 变化、Production/Test 分离、失败不发布成功、修复后成功、同键并发协调 |
| 路由／协议 | 内置命令优先、低层 direct 输入与公共 CLI 一致、Check codec 及版本协商、非对应成功结果拒绝 |

从现有 expected-error 用例中选取少量代表规则交叉检查，不复制全部语言 negative fixture，也不按使用哪个库重复检查同一个前端规则。无关库 API 行为继续放在所属 Cone tests。

阶段和缓存验收保持真实源码、工具、sysroot 和原产物存在，通过实际调用观察、输出及正常输入变化断言。不得删除或隐藏编译器／源码来证明“无需后端”，不得新增缓存破坏、来源或权限审计用例。

### 9.3 仓库验证

每批 Rust 变更先执行 `cargo fmt --all` 与 `cargo clippy --workspace --all-targets`，再运行相应单元／集成测试。fixture runner 如需 Python 修改，先运行仓库指定版本的 Ruff，再运行公共规则测试。交付验收包含 `cargo test --workspace`、配套命令构建、已显式准备的开发 sysroot，以及正式 CLI fixture 规则和文件用例；普通 fixture 使用 release，模式组合仅保留合同确有需要的专测。

原始日志和逐次报告保存在仓库外。实施完成后仅新增精简记录，说明实际验证范围、结果和未完成项；设计状态及路线图据实更新。

## 10. 范围与后续边界

格式化不会使语义错误程序自动正确；完整语法是写回的前提，类型检查不是格式化前置。检查通过也不保证根的机器代码、native 源码或链接成功。

若后续实测表明源码依赖的冷构建是主要成本，应单独设计如何复用现有完整语义输入，并重新确认单 Cone driver 的依赖合同。首版不预先引入语义产物格式、长驻服务或共享内存协议。

LSP 可在后续通过同一格式库实现全文 TextEdit，但需单独处理文档版本、坐标、客户端选项和能力声明；range／on-type 与错误文件格式化不能由本期全文 CLI 的成功推导出来。

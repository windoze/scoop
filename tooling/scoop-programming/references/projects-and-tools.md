# 项目、依赖与 CLI

所有命令沿用主 skill 选中的 `scoop` 和 sysroot。选项细节以该版本的 `<command> --help` 为准。

## 选择输入形式

| 形式         | 入口与规则                                                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| 单文件       | `scoop run main.scoop --release` 只编译这个文件，不读取邻近 `Cone.toml` 或其他源码。默认依赖 core 和 std，名称仍需相应 import。 |
| 可执行 Cone  | `scoop run app --release` 或 `scoop run app/Cone.toml --release` 选择完整项目。默认从 `src/` 递归选择 `.scoop` 文件。           |
| library Cone | `scoop build library --release` 产生库产物；通过库测试或真实消费者验证，不给库添加虚假 main 来运行。                            |

build/run/check/test/doc 省略输入时从当前目录向上查找最近的 `Cone.toml` 及其 workspace；成员目录默认选该成员，workspace 根使用 `default-members`。测试与文档要求 manifest Cone。已有项目直接修改；新项目可使用：

```sh
scoop new my-app --group example.app
scoop new my-lib --group example.lib --kind library
scoop pm new --workspace my-workspace
```

目标目录必须不存在，包括空目录。`new` 默认创建生产源码、`tests/basic.scoop` 和测试依赖；只有确实不需要测试模板时才传 `--no-tests`。name 默认取目录名；目录名含空格等不符合 Cone 名称规则的字符时，用 `--name` 指定合法名称。`--version`、`--package` 也可覆盖默认值；此命令不下载模板或构建程序。

可执行入口是一个顶层、非泛型、非 suspend 的 `main`，参数为无参数或单个 `Array<String>`，返回 Unit 或 Int。**`args[0L]` 是程序启动路径，用户参数从 `args[1L]` 开始**，不同于 Kotlin／Java；无用户参数时仍有第 0 项。`cli/options` 的 `parseCommandLine(args)` 接受这份完整 argv。返回 Int 可指定退出码。

## 声明依赖与 package

单文件的额外依赖写在首个语言 token 之前：

```scoop
//! dep: scoop:scoop.json:0.1.0

import scoop.json.*
```

这是精确 coordinate，普通 manifest 模式不解释此注释。把单文件移入 Cone 时要将依赖移到 `Cone.toml`。下面展示 manifest 的字段形状，名称与版本应取实际项目和所选库：

```toml
schema = 1

[cone]
group = "example.app"
name = "report"
version = "0.1.0"
kind = "executable"

[dependencies]
"scoop:scoop.std" = "0.1.0"
"scoop:scoop.json" = "0.1.0"

[test-dependencies]
"scoop:scoop.test" = "0.1.0"
```

- 普通 manifest 默认依赖 core；std 与其他使用的库需显式声明。`"1.2.3"` 是 caret 兼容范围，`"=1.2.3"` 才是精确版本，也可使用 tilde 或比较约束；不支持 `latest`。PM 在一个程序的完整依赖图中为每个 `group:name` 选择一个版本，默认保留有效 lock，新选择优先最新兼容版本。
- 依赖是 Cone coordinate，import 是源码 package／名称，二者不必同名。例如 `scoop.std` 提供 `scoop.collections`、`scoop.time` 等包。安装根内有库不代表当前 Cone 已声明它；传递依赖也不自动成为直接可访问 API。
- 普通 Cone 的源码 package 必须等于 group 或位于其子包；同一 Cone 的文件可以互用 internal 声明，跨 Cone 需要公开 API。单文件不建立项目的 package 结构，package header 会被忽略并产生 warning。
- 本地源码依赖的写法可从 `language/visibility` 取得，例如 `"example.lib:greeter" = { version = "0.1.0", path = "../library" }`。path 相对声明它的 manifest，coordinate 必须与被引用库一致。
- 显式 `sources` 是完整源码选择清单，不会额外追加默认 `src/`。增加文件前先检查项目是否使用显式清单或平台条件。
- 随包系统库按目标预编译，普通应用复用所选 sysroot。缺失目标、版本不匹配和源码类型错误应分别处理，不自动修改依赖版本或从源码重建 sysroot。

workspace 根用 `[workspace] members = ["app", "library"]` 列出成员目录，可配置 `default-members`。成员通过 `{ workspace = true }` 显式继承根的 `[workspace.cone]` 字段或 `[workspace.dependencies]` 条目。`--workspace` 选择全部成员，`-p group:name` 选择指定成员；`run` 要求所选成员中只有一个 executable。不同程序可以锁定同一依赖的不同版本。

非 `scoop` group 的裸版本依赖要求明确的 `[registry] default`；它不会按 `--cone-path` 猜测本地产物。来源写法例如：

```toml
[dependencies]
"example.lib:greeter" = { path = "../library" }
"org.example:json" = { version = "1.4.0", registry = "https://packages.example.org/scoop/" }
"org.example:codec" = { github = "example/codec", rev = "<实际 commit>" }
"org.example:legacy" = { version = "=2.0.1", artifact = "../artifacts/legacy.slib" }
```

Git/GitHub 可指定 branch、tag 或 rev，最终完整 commit 写入 `Cone.lock`；同仓库中的包可用 `subdir` 选择。不要把示例 URL、仓库或 commit 当作已存在的包。registry 只需托管静态索引和源码归档，PM 按索引下载所选版本；源码默认共享在 `~/.scoop/packages/`，构建输出仍归项目和构建缓存管理。

缺少 lock 时，普通 build/run 自动调用配套 PM 解析并生成；`--locked` 要求已有匹配 lock，`--frozen` 还禁止联网。提交项目的 `Cone.lock`；修改声明后运行 `scoop pm resolve`，显式升级用 `scoop pm update`，获取已有锁定来源用 `scoop pm fetch`。`deps --why group:name` 解释选择原因，`add/remove` 编辑声明后不自动更新 lock。语言服务缺少 lock 时可调用 PM 初始化，之后离线读取依赖 metadata；提示缺少输入时按实际诊断 resolve/fetch，不将整个语言服务流程描述为始终离线。源码打包和 publish 流程尚未提供。

## 按目的选择命令

| 目的         | 命令示例                                       | 结果与限制                                                                           |
| ------------ | ---------------------------------------------- | ------------------------------------------------------------------------------------ |
| 格式化       | `scoop fmt .`                                  | 写回所选生产及测试源码；也可传多个 Cone 或显式文件。                                 |
| CI 格式检查  | `scoop fmt . --check`                          | 不写源码，差异或错误退出 1。                                                         |
| 生产语义检查 | `scoop check . --release`                      | 检查前端及生产入口，不生成根机器产物；需要更新的源码依赖仍可能构建。                 |
| 测试语义检查 | `scoop check . --tests --release`              | 检查生产和测试源码及生成的测试入口，不检查生产 main。                                |
| 编译／链接   | `scoop build . --release`                      | 完成当前 Cone 产物及可执行 Cone 的链接，`-o` 选择输出位置。                          |
| 仅生成产物   | `scoop build . --emit-slib --release`          | 发布根 `.slib`，不准备 runtime 或执行最终链接；测试产物可再加 `--tests`。            |
| 执行         | `scoop run . --release -- Alice --count 2`     | `--` 后原样传给程序。程序的 `--help` 也放在分隔符后。                                |
| 测试         | `scoop test . --release`                       | 编译并运行 manifest Cone 的测试，保持测试依赖在 `[test-dependencies]`。              |
| 选择测试     | `scoop test . --release --filter "zero count"` | 按完整测试名称的子串筛选；`--list` 列举，`--no-run` 仅构建，`--nocapture` 显示输出。 |
| API 文档     | `scoop doc . --no-deps --deny-doc-warnings`    | 生成所选根的离线文档；多根可同时传入，`--output-dir` 选择站点位置。                  |

普通应用验证使用 release，已有项目的明确 profile 要求优先。选择其他 target 时先确认工具包及 native 环境确实支持它，使用 `--target <triple>`，不从规划文档推断已支持的平台。

采集执行次数使用 `scoop build . --profile profiling`，它采用 release 优化并默认保留行表；普通 debug/release 不采集。程序运行时在 `SCOOP_PROFILE_DIR` 指定目录下为每次运行创建独立子目录，缺省为运行 cwd 下的 `scoop-profile/`。其中 `counts.profraw` 是 LLVM 计数，`runtime.jsonl` 保存启动、GC 总指标、cycle/slice/phase 事件与退出信息；业务标准流保留原用途。GC 事件缓冲满后报告 dropped，总指标继续记录；时间以 ns 为单位，实际时钟精度可能使短阶段或任务的差值为零。正常返回和显式 `exit` 都尝试写出，业务非零退出仍可有有效样本，崩溃或强制终止不保证完整数据。

profiling 默认需要所选目标的专用系统库，目录为 `<sysroot>/lib/<target>/profiling/instrumented`；其中所有系统 Cone 都采用 full 模板导出。缺失时按所用版本的组件安装提示准备，维护构建见仓库 `scripts/README.md`，不自动降级为普通系统库。采集计数可用配套 `llvm-profdata merge` 合并；Scoop 位置映射随 `.slib` 以 `.profile.json` 后缀保存，`scoopc profile-summary <profile-index.json> --message-format json` 读取 Scoop 数据索引并报告逻辑次数。次数不代表耗时，性能比较应使用无插桩 release 程序；独立 PGO 流程的交付状态以 `docs/profiling/PROGRESS.md` 为准。

日常 PGO 使用 `scoop pgo instrument . -- <训练参数>`，然后执行 `scoop pgo optimize -o dist/app`。先安装所选目标的 profiling 组件；命令自动选择 Profiling/full 训练库和 Release/full 无探针库。重复 instrument 会追加样本；默认数据目录为调用 cwd 下的 `target/pgo`，`--data-dir` 可覆盖，`--target-dir` 不改变它。optimize 复用保存的实际根和构建设置，生成后不自动运行；切换 cwd 时指向原数据目录。`--no-run` 可先生成训练程序，按提示使用 `SCOOP_PROFILE_DIR` 外部运行后再 optimize。程序标准流和退出状态保留原用途，非零退出仍可能产生有效样本。

已有 Scoop 反馈索引时，底层入口为 `scoop build . --release --profile-use path/to/profile.json --profile-use-required`。它消费执行频次来调整 MIR 内联和字符串融合，默认选择组件中 `profiling/release` 的无探针 full 库，生成程序不再采集。`--profile-use-required` 要求本次源码构建图至少有一个优化决策实际使用反馈，在发布最终输出前检查；缺失或正文变化的局部计数仍按静态规则优化。索引、计数和映射更新会触发正常重编译，命中缓存则复用已有反馈摘要。`--profile-use` 仅能与 release 一起使用；该选项接收 Scoop 索引，不直接接收 `.profraw` 或 `.profdata`。便利命令内部固定启用该要求，不需要手工传入这些参数。

构建库时可以用 `--export-ir off|default|full` 选择跨 Cone 优化正文的导出范围，例如 `scoop build library --release --export-ir full`。默认 `default` 保留当前可移植的普通正文及 helper 闭包；`full` 保留所有有 Scoop 正文的函数、方法、构造器和访问器及其支持声明，最终是否内联仍由 consumer 决定。`off` 关闭额外普通优化正文，泛型、默认实参等语义必需模板仍会导出。这一选项不改变公开可见性，也不能恢复预编译依赖已剪掉的正文；只影响本次实际构建的源码 Cone。

single-file run 允许第一个普通程序位置参数开始转发；为避免选项歧义，示例统一使用显式 `--`。从样例复制命令时，确认工作目录、文件名、运行前提和参数都对应当前项目。

## 语言服务查询

使用所选工具链配套的 `scoop-lsp`；先用 `scoop-lsp query --help` 确认该版本提供入口。在含 Cone.toml 的项目/workspace 目录运行：

```sh
scoop-lsp query hover src/main.scoop --line 8 --column 12 --format json
scoop-lsp query signature src/main.scoop --line 8 --column 20
scoop-lsp query completion src/main.scoop --line 9 --column 10
scoop-lsp query definition src/main.scoop --line 8 --column 12
scoop-lsp query diagnostics src/main.scoop --format json
scoop-lsp query code-actions src/main.scoop --line 8 --column 12 --format json
scoop-lsp query symbols src/main.scoop
```

行列应取实际源码；两者从 1 开始，列默认计 Unicode 标量（utf-32）。可选 `--position-encoding utf-8|utf-16|utf-32`；JSON 内 LSP range 仍为零基并附 positionEncoding/rangeBase。用 `--project` 指向另一个 Cone 或 workspace，文件与路径选项仍相对当前目录。`--sysroot`、`--target`、`--purpose production|test` 应与实际开发视图一致；`--package-manager`、`--cone-path`、`--package-cache-dir`、`--lockfile` 可选择已有依赖资源。

文档查询的 `--stdin` 读取完整未保存正文，不改磁盘。JSON 的 result 是原 LSP 结果；diagnostics 包含当前文档及项目诊断。status=empty 表示无结果，partial 表示存在错误或服务器分析问题，要同时读诊断。查询成功含 empty/partial 都退出 0；输入/协议错误退出 1，用法错误退出 2，错误在 stderr。查询不替代 build/test，也不生成机器产物。

`code-actions` 返回已展开的语法/import 编辑。多个 import 路径需要根据实际意图选择；没有动作时继续查看诊断及 API 声明。编辑中的版本 1 仅表示本次临时 buffer，应用前确认正文未变，按 positionEncoding 的零基范围编辑，再执行 `scoop check`。命令不自动修改文件，也不将普通值自动包装为 Some。

type-definition、implementation、references 提供进一步导航。workspace-symbols 只查当前工作区源码声明；查库中尚未使用的名称仍从文档/库源码入手。capabilities 不加载项目，报告实际服务器能力。每次查询使用新会话，结束会回收服务器；缺少 lock 的初始化沿用前述 PM 规则。

## 测试和诊断

从 `scoop example test/assertions --format json` 查看完整 `Cone.toml`、生产文件和测试文件。`@Test` 用于测试容器与用例；`assertEquals` 提供名义与结构相等重载，具体规则见[语言参考](language.md)。遇到候选错误先核对实际参数及内层调用，不要将精确断言改成只报告 Boolean 的断言来绕过诊断。正常返回、边界条件和预期异常用 Cone 测试验证；CLI 退出码、进程交互和编译失败才需要外部验证。

编译器 human／`--message-format json` 诊断写 stderr，stdout 留给程序；同时检查退出状态。检查成功不等于测试执行成功，也不等于所有目标都已验证。已有源码依赖、generator 或 native 工具产生的日志可能同时出现在 stderr，不把整个流假定为单个 JSON 对象。

验证可执行程序自己的标准流时，可以先用 `scoop build . --release -o ./app` 明确输出位置，再直接运行 `./app` 并分别收集 stdout、stderr 和退出码。Windows 使用 `app.exe`。输出路径由公开的 `-o` 选项确定，无需读取 `.scoop-output-*.record` 等内部构建记录。

需要排查具体编译转换或性能问题时，`scoop build . --release --emit optimizations --dump-dir observations` 可观察优化决策；`--emit` 还支持 ast/hir/mir/lir/symbols/all。只选所需阶段，普通程序开发不依赖完整 IR 快照。已有产物链接用 `scoop link --help`；`scoopc` 和 `scoop-link` 是低层入口，应用通常使用 umbrella `scoop` 完成编排。

`scoop clean` 会删除所选 target 目录及其中全部 profile／构建缓存；只在确实要清理输出时使用，不作为类型或库查找错误的常规修复。系统库准备与发行脚本属于工具链维护流程。

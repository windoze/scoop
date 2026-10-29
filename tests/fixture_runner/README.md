# Scoop fixture schema 1

运行 `python3 tests/run_fixtures.py --all` 完整验收。开发时可用
`--suite tests/fixtures/driver/cli` 或 `--filter 'cli-*'`；`--list` 使用同一发现与校验。
所有被扫描的 `.scoop` 必须归属于声明的 `inputs` 或 `support`，支持文件不单独计为通过。
缺少工具、期望文件、未知字段、嵌套类型错误、重复载体与前向步骤引用均报配置／环境错误。
Python 3.11+ 只用标准库；Python peer 和辅助进程统一使用 `${python}`（本次 runner 的解释器），避免 PATH 中另一个旧版本。格式化／lint 使用 `tests/requirements-dev.txt` 固定的 Ruff。
默认最多并行运行四个用例，可用 `--jobs N` 调整，`--jobs 1` 顺序运行。
大型 HTTP 客户端集成用例及 WebSocket 生命周期用例的编译步骤显式使用 900 秒时限，以覆盖较慢宿主上的完整编译；运行、诊断和缓存断言仍按各用例原有规则执行。
正式 CLI 验证使用按 [准备脚本说明](../../scripts/README.md) 事先准备的开发 sysroot；
`${sysroot}` 依次采用 `--sysroot`、`SCOOP_SYSROOT`、仓库 `sysroot`，直接读取已准备的资源，
不复制完整 sysroot，也不自动调用准备、LLVM 构建或打包脚本。
`${cache}` 依次采用 `--cache-dir`、`SCOOP_CACHE_DIR`、仓库 `target/fixture-cache`，
跨用例及不同 `--work-dir` 复用；清理本次 work 不清理该缓存。
Linux amd64 默认选择 glibc；用 `--target x86_64-unknown-linux-musl` 运行 musl 用例，
fixture 的 Scoop 命令通过 `${target}` 传递该选择。`${cc}` 默认分别为 GCC / musl-gcc，
可用 `--cc` 覆盖；Linux 不提供 Darwin 的 `${sdk}` / `${deployment}`。
Windows x64 可选择三个完整 Windows target，默认 Microsoft UCRT；命令默认使用 `.exe`。
Windows 安装包的主驱动为 `sco.exe`，验证安装包时传 `--scoop <安装根>/bin/sco.exe`；
`${scoop}` 与 `--scoop` 仍是 runner 的逻辑名称。未覆盖工具路径时使用 Cargo 内部构建产物 `target/debug/scoop.exe`，其他工具名不变。
`--native-sysroot` 与 `--unwind-prefix` 可指定所选目标的开发资源；省略时使用 sysroot 中的目标目录。
`--linker` 选择最终 executable linker（例如 Microsoft `link.exe`），仅加入 `${link_args}`。
Windows native companion 使用 Clang 的 GNU 风格命令行，`${cc_args}` 提供实际 machine triple 和目标资源，
MinGW 默认使用所选 prefix 的 Clang，Microsoft 使用宿主 Clang 并由 SDK／Build Tools 或 `msvc.toml` 取得开发环境。
Microsoft companion 使用动态 CRT；GNU driver 的额外静态 CRT 默认参数被省略，链接沿用对象中的正常默认库指令。
`--ar` 可覆盖 companion archiver；只为当前适用且实际使用 native 工具的用例准备这些参数。
共有 fixture 使用 `{each = "${compile_args}"}` 给 `scoopc build` 传目标参数，
使用 `{each = "${link_args}"}` 给 `scoop build/run/link` 或 `scoop-link` 传目标及最终链接参数。
普通功能 fixture 和内置 Cone 测试显式使用 release：Scoop 命令传 `--release`，
低层 `scoopc build` 传 `--profile release`。不公开 profile 参数的 `scoop doc` 按原文档/HIR 合同覆盖。不把普通用例分别在 debug/release 跑两遍。
编译器正确性专测按实际 IR、优化、跨产物合同保留所需 profile；产品默认 profile
的专测保留省略选项的调用，Scoop 产品的默认值仍为 debug。分类与例外见
[测试模式说明](../../docs/minor-fixes/TEST-MODES.md)。
Linux 的链接参数包含所选 `${sysroot}/lib/<target>/unwind`，供普通程序链接使用。
显式 `--cc` 也传给 Linux Scoop 工具；用例保留其实际源码、sysroot 和工具。
需要固定自定义 `--cc` 的工具链诊断专测直接传 `--target ${target}`，不展开包含环境 `--cc` 的公共参数数组。
native companion 用 `{each = "${cc_args}"}` 传平台编译参数：Darwin 为实际 target、
SDK 和 deployment，Linux 为 PIC/pthread；源码自己的标准、优化和告警参数保留。
Darwin deployment 采用 `MACOSX_DEPLOYMENT_TARGET` 或 `scripts/release.toml` 的明确基线，
与开发工具保持一致，不读取宿主系统版本。
`${target_profile}` 是内部 target profile 名称，core artifact 目录使用 canonical `${target}`；`${symbol_prefix}` 是 C/编译器逻辑名字
在对象符号表中的平台前缀；`${errno_eloop}` 是宿主文件系统的符号链接循环错误码。
结构／ABI／原生链接专测确有平台差异时，用 `${target}` 选择独立快照；
平台无关的专测共用期望，比较时不抹去 ABI 或符号差异。
手写 LLVM IR companion 声明 `tools = ["llc", ...]`，使用 `${llc}` 加
`-mtriple=${llvm_target} -filetype=obj -relocation-model=pic` 生成目标对象。
ELF 的 `${llvm_target}` 与 `${target}` 相同；Darwin 包含当前 deployment，以产生
原生对象需要的 `LC_BUILD_VERSION`。Scoop 命令仍使用 canonical `${target}`。
工具依次使用 `--llc`、`SCOOP_TEST_PAIRED_LLC`、`LLVM_SYS_221_PREFIX/bin/llc`，
否则查找 `llc-22` / `llc`；实际版本必须为 LLVM 22.1。
只为适用 target 的用例发现工具，目标不适用项仍单列，不计为通过。
每个用例的步骤与变体保持有序；报告按发现顺序保存。中断时清理运行中的进程，
未完成项标记 `interrupted`，退出码为 130。
Windows 执行保留完整 32-bit exit code；`signal` 退出断言只用于 POSIX。
每个 Windows fixture 进程在开始执行前加入独立 Job，结束、超时或中断时关闭 Job，
回收该进程及其后代；不设置 CPU／内存配额。参数使用 Windows Unicode 表示，标准流仍按原始字节断言。

每个用例选择一种载体：相邻 `name.fixture.toml`、目录中的 `fixture.toml`，
或源码开头 `// fixture:begin` 到 `// fixture:end` 之间每行带 `//` 的 TOML。
`tests/fixtures/.gitattributes` 固定文本 checkout 使用 LF，保持源码字节位置与 golden 的跨宿主一致性；
专门的二进制输入沿用 `-text` 属性，runner 不归一化源码或诊断中的位置。
源码注释不会被删除，编译器读取原始源码。相对文件名以描述所在目录为基准。
发现阶段仅解码内联条件注释；源码正文中的非法 UTF-8 留给实际 compiler 诊断。

```toml
schema = 1
name = "example"
root = "hello.scoop"
inputs = ["hello.scoop"]
tools = ["scoop"]
targets = ["aarch64-apple-darwin"]
tags = ["cli"]

[[steps]]
name = "build"
argv = ["${scoop}", "build", "--release", "${root}", "--sysroot", "${sysroot}",
        "--cache-dir", "${cache}", "--target-dir", "${work}/target",
        "--message-format", "json"]
exit = 0
stdout = ""
stderr = ""
json = "stderr"
diagnostics = []
checks = [{actual = "${build.result.kind}", equals = "executable"}]

[[steps]]
name = "run"
argv = ["${build.result.output}"]
exit = 0
stdout = "hello\n"
stderr = ""
```

顶层还可声明 `support`、`vars`、`variants`；每个变体只有 `name`、`vars`、`env`。
未写 variants 时运行 `normal`。变体按顺序执行，各有独立 `${work}`，共享
`${cache}` 与 `${sysroot}`。GC 正确性专测先给 `scoop build/run/test/link` 或 `scoop-link` 传 `--enable-gc-stress` 编入支持，再用 `env = { SCOOP_GC_STRESS_MOVE = "1" }` 开启对应变体；未编入支持时环境变量无效。
普通用例直接使用预编译系统库和共享 `${cache}`；仅实际验证源码构建、发布或缓存行为时，
才为相关输入保留构建步骤或私有缓存。库 API 行为用 `scoop.test` 验证，不重复冷/热构建。
多 Cone 提交真实 `Cone.toml`，按普通 `copy` 步骤布置，runner 不合成入口或 manifest。
native companion 显式作为 input，并用普通 argv 步骤调用 `${cc}`、`${ar}`，指定
`{each = "${cc_args}"}` 和输出；随后传 `--library-path`。

`${fixture}`、`${root}`、`${repo}`、`${runtime}`、`${target}` 也是内置值；Scoop
工具默认从 `target/debug` 取得，可用 CLI 参数或 `SCOOP_TEST_PAIRED_*` 指定。

LSP 进程用例声明 `tools = ["scoop-lsp", "python"]`，使用 `${scoop-lsp}` 和真实协议 peer；
先构建 `cargo build -p scoop-lsp`。可用 `--scoop-lsp` 或 `SCOOP_TEST_PAIRED_SCOOP_LSP`
选择服务器，runner 不自动构建，也不为项目加载准备 sysroot。

PM 进程用例声明 `tools = ["scoop", "scoop-pm"]`，先构建 `cargo build -p scoop -p scoop-pm --bins`。
`${scoop-pm}` 可由 `--scoop-pm` 或 `SCOOP_TEST_PAIRED_SCOOP_PM` 指定；
只做本地成员发现和清单编辑的用例无需 LLVM、编译器或预编译系统库。

PTY fixture 声明 `tools = ["scoop", "scoopc", "terminal-host"]`，以普通 argv
步骤调用 `${terminal-host}` 并传入 Scoop 产物。先按
[host 说明](../terminal_host/README.md)构建测试工具；路径依次取 `--terminal-host`、
`SCOOP_TEST_PAIRED_TERMINAL_HOST`、`target/debug/terminal-host`。
runner 仅在选中用例需要时检查该可执行文件，不自动构建工具。
`${step.result.output}` 等引用前一步实际 JSON；数组使用 `.0`，`.length` 取长度。
路径的 `.canonical` 按宿主解析绝对路径（Windows 包含 verbatim 前缀），也可表示缺失输入的诊断路径；`.filename` 返回工具或文件的末级名称。`${file_not_found_error}` 与 `${directory_write_error}` 分别保存宿主的文件不存在、向目录写文件的错误文本，包含系统本地化和 OS error 编号。

文本或 bytes 的 `.hex` 返回十六进制字节表示；文本使用文件系统编码并保留原始字节，
可组合 `${work.hex}2fff` 这类期望来比较 JSON 中的非 UTF-8 路径。字典的 `hex` 字段仍按普通字段读取。
传作 argv 时先在 vars 中声明 `raw_hex = "${work.hex}2fff"`，再写 `{hex = "${raw_hex}"}`；
静态 hex 或完整引用使用相同的预检和执行规则。
`${variants.normal.build.result.root.artifact_fingerprint}` 可比较前一变体。
普通功能 fixture 不额外保存整份产物、core 或 native 输入的固定指纹快照。
缓存命中／失效、确定性、产物读写和正常多 Cone 编译，应使用
`equals`／`not_equals` 或 `same_as`／`different_from` 比较本次实际产生的值。
固定摘要仅用于输入严格受控、需要锁定编码或指纹规则的专门测试；结构、诊断、
ABI、GC、相关符号及链接运行断言继续保留，不能用摘要归一化隐藏这些差异。
普通功能用例只请求编译和运行，不附加 `--emit all`。结构专测只请求被测阶段，
例如下面的 MIR 专测在 build 步骤中使用 `--emit mir --dump-dir ${work}/dump`：

```toml
checks = [
  {file = "${work}/dump/${build.result.root.identity}/mir.txt", snapshot = "program.mir.txt"},
]
```

该期望应锁定小程序的 CFG、求值顺序或清理路径，而非展开整个 core。
AST/HIR/MIR/LIR 的 typed IR 测试同样可承担结构职责；不要求每个功能保存四阶段输出。
代表组和迁移理由见 [M40 迁移说明](../../docs/milestone40/MIGRATION.md)。
观察模式本身的 CLI 专测检查输出文件集合、已有观察复用和缺失观察补齐。
普通语言组合用例直接运行 build 输出，检查语言结果及真实多 Cone 消费，不重复独立链接、
复制产物或固定公共内部符号。build／独立链接一致性、Cone/image/root 关系和 mangling
由少量链接与 ABI 专测覆盖；native 输入选择、归档成员与动态绑定专测保留必要计划。
整个字符串为引用时保留 JSON 类型。argv 可用 `{hex = "ff"}` 传递原始字节，
或 `{each = "${build.result.dependencies}", field = "path", prefix = "--dependency-slib"}`
展开实际清单。程序 JSON 中的非 UTF-8 path carrier 保留字节，不经过 shell。
字符串中的 `$$` 表示一个字面 `$`，只展开一遍：`$${name}` 匹配诊断中的 `${name}`，
`$$${name}` 表示字面 `$` 后接 name 的值。预检忽略已转义的引用；引用所得的值不再展开。

每个进程必须声明 `exit` 或 `signal`（如 `"SIGABRT"`）之一，以及完整 stdout/stderr。
stdin 默认空；cwd 默认 `${work}`；env 继承调用者并应用显式键；timeout 默认 300 秒，以容纳 Linux 上完整的 core 冷构建。
每个变体开始时去掉调用者环境中的 `SCOOP_GC_STRESS_MOVE` 和 `SCOOP_GC_STRESS_MINOR`，
再应用变体和步骤的显式设置；其他环境变量继续继承。GC 正确性专测显式编入支持并
设置所需环境变量。普通功能用例不启用 stress，也不保留重复的 moving/minor 运行。
依赖实际分配量或等待 minor GC 的压力循环必须让对象真实逃逸，例如先写入全局槽；只构造并读取字段可能被标量替换。若目标是制造短命垃圾，在下一次分配前清空压力槽，避免把压力对象的额外存活与晋升混入被测行为。
字节期望和 stdin 可写文本、`{hex = "..."}` 或 `{file = "expected.stdout"}`。
stdout/stderr 的地址等非确定性输出可用 `{regex = "..."}` 对完整 UTF-8 字节流匹配；不隐式添加通配符，不用于 stdin。保留固定诊断文本并限定动态字段，栈诊断用例另外验证原抛出位置及重抛后内容一致。
stdout/stderr 也可显式声明 `{snapshot = "symbols.${target}.txt"}`，沿用 snapshot
更新规则；正常验收仍比较完整原始字节。仅用于小而受控的结构或工具输出专测；普通功能用例不保存完整符号表。
`json = "stderr"` 解析 Scoop schema 1 前缀，遇到成功结果即停止，后续字节归程序。
`scoop doc` 返回 `documentation` 结果，包含站点入口、各 Cone 入口、渲染更新状态和实际编译调用。
`scoop check` 返回无产物的 `check` 结果，包含根身份、target、用途、缓存命中和实际 child 调用；调用区分根检查与源码依赖的 artifact 编译。
`scoop fmt` 返回 `format` 结果，包含写回／检查模式、成功状态、有序文件状态与计数；格式差异或部分文件失败时仍返回完整结果，进程退出 1。
`scoop clean` 返回 `clean` 结果，包含选中的 `target_dir` 和是否实际删除目录的 `removed`；目录不存在时同样成功。
这种模式必须声明 `diagnostics` 数组／JSON 期望文件，或用该步骤的 `checks` 显式检查
`${step.diagnostics...}`。通常比较完整诊断；破坏 core 根声明等会引发大量无关连带错误的
规则用例可以只精确检查首个规则诊断，保留其 code、message、notes 和实际声明的源码位置。
缺失声明的全 Cone 错误不锁定任意首个源码文件的回退位置。runner 仅剥离 display，不改变
canonical source、span、code、message 或 notes；stdout/stderr 仍严格比较剩余原始字节。
期望中的 `${input.canonical}` 可取得已存在宿主输入的真实路径；Windows 保留 Rust 文件系统 API 使用的 `\\?\` 前缀，并按宿主路径分隔符拼接。先把完整输入路径放入变量（如 `manifest = "${fixture}/program/Cone.toml"`），再在完整诊断的 message／host origin 中引用 `${manifest.canonical}`。这只构造期望，不归一化实际诊断或逻辑源码身份。
`json = "stdout"` 用于普通工具的逐行 JSON，保留原 stdout。步骤结果还包含
`records`、`diagnostics`、`returncode`、`pid`、`stdout`、`stderr`、`raw_stderr`。

`json = "test-stderr"` 用于默认捕获输出的 `scoop test` 或 `--list`：继续读取构建记录后的
`test-report`／`test-list` 与工具诊断，`result` 保留构建结果，`test_result` 保存测试结果。
同样必须声明完整 diagnostics；捕获的用户输出只作为报告字段读取，不从中识别结构化记录。
`--nocapture` 将用户 stderr 混入工具记录时不能使用这个模式。

每一步的 `checks` 选择 `actual`（JSON 引用）、`file` 或 `glob`（排序后的路径数组），并选择一个比较：
`equals`、`not_equals`、`contains`、`not_contains`；文件还支持 `exists`、`type`、
`same_as`、`different_from`。`snapshot = "expected.hir.txt"` 比较完整文本或 JSON。
不新增缓存损坏、收据伪造或文件来源／权限防御用例，也不通过删除、移走或隐藏源码、工具、原始产物来验收消费能力。混合用例保留真实语言、ABI、GC、运行与诊断断言，迁为输入保持存在的多 Cone 构建。缓存通过正常输入变化、工具实际调用及输出复用验证；真实缺失输入、失败不发布和测试结束清理继续覆盖。受控编码／指纹规则的固定向量由对应 Rust 单元测试覆盖。
普通语言用例运行正常 build 的 executable，不再为同一运行结果重复复制工具和产物后独立链接。显式链接、ODR、native 选择和失败不发布专测继续使用实际工具、sysroot 和已有产物，显式指定本次 fixture 的缓存目录。源码目录正常搬迁后以新位置重新构建属于输入位置变化，相关源码须保留到用例结束。

`${test_core}` 是本轮共享的 core 源码目录，仅加入已有 `stage3_test.scoop` 语言测试辅助声明。
普通多 Cone 用例从这个固定路径执行真实 build，共用 `${cache}` 的正常输入更新判断，
避免为每个用例在不同路径重编相同 core。它保持源码和工具存在，不预制编译结果，
用例不得修改该目录；需要修改 core 或验证输入失效的专测仍使用自己的源码副本。
`${class_core}` 同样共享导入类用例已有的固定 core 声明（RebuiltReadable、直接异常基类
和辅助值 43）。这些普通语言用例只消费该输入；core 更新前后的比较由 rebuilt-core
与需要不同 core 声明的专测覆盖，不在每个类规则中重复两次冷构建。
`${inheritance_core}` 在同一固定输入上加入 `RebuiltParent`，供继承规则用例共享。
`imported-class-inheritance-core` 保留真实修改 core 后重新编译、比较产物和运行 consumer 的覆盖。
`${member_defaults_core}` 为 Long、Boolean 和 String 加入同一个 `RebuiltDefaults` 接口，
供 primitive inherited-method 用例共享；combined 用例保留修改前后的真实 core 构建。
`normalize` 的公共规则为 `paths`、`newlines`、`native-digests` 和 `native-link-plan`。
`native-link-plan` 只允许用于显式 snapshot：要求完整 `program-link v3` 的唯一 section
边界，比较 `link inputs:` 到 `startup object references:` 之前的原生链接区段。
输入顺序、ELF 系统库、archive 成员/选择、DSO load/re-export/rpath 和符号绑定均保留，
区段内未知新字段也进入比较；缺失或重复边界直接失败。它不保存无关 core 对象数量、
startup 符号列表、生成 C 或全镜像 `ELF system imports` 总数；系统导入总数包含 runtime
和后端辅助引用，不是该 fixture 的 native 输入选择合同。真实符号解析由 linker 与程序运行验证。
`native-digests` 仅用于链接计划：
将 `native library` 的 `input`、`native object/archive` 的内容 ID、归档 member 的
`digest`、`dynamic provider` 及其 load/re-export 引用替换为按首次出现编号的标记。
同值共用标记，不同值使用不同标记；保留逻辑库、requirement、源码声明身份、符号、
成员名称／顺序／范围、选择状态、动态库路径和绑定关系。它不处理普通 IR ID 或任意哈希。
native 诊断中的 `native object` 引用及 `NativeInputId(Digest256(...))` 可通过进程步骤的
`diagnostics_normalize = ["native-digests"]` 使用同一规则；完整诊断的其他字段仍精确比较。
过期依赖诊断可用 `diagnostics_normalize = ["dependency-digests"]`，仅处理
`StaleDependency` 中 recorded HIR/MIR/LIR 与 actual 的六个内容摘要，保留它们的相等／变化关系。
该规则只允许用于诊断；Cone／provider 身份、坐标、来源、阶段、错误 code 和其他内容仍精确比较。
摘要本身是被测行为的确定性或编码测试不启用此规则。显式
`--update-snapshots` 仅更新选中的 snapshot 文件，不改 exit/signal、诊断和普通输出期望。

文件准备步骤使用 `files = [{copy = {from = "...", to = "..."}}]`。
其余动作及必需参数：`move/symlink/hardlink(from,to)`、`remove/mkdir/touch(path)`、
`write(path,data)`、`replace(path,old,new)`（必须唯一匹配）、`patch(path,offset,hex)`
（不可越界）、`chmod(path,mode)`（八进制字符串）。修改均应指向本次私有工作区。
`concat(path,parts)` 按数组顺序拼接已有字节载体，不自动添加分隔符。例如
`parts = [{file = "first.scoop"}, "\n", {file = "second.scoop"}]` 保留原单文件组合用例；
先读取所有部分再写目标，输入缺失时不覆盖已有目标。原输入仍需声明在 `inputs` 中。
`remove` 删除目录树，或解除普通文件、符号链接和命名管道，不打开文件内容。
`truncate(path,size)` 将现有文件缩短为指定非负字节数；超过当前文件长度时报配置错误，
不扩展或修改文件。普通编译缓存测试不使用它制造损坏后要求检测或恢复。

并发进程声明 `background = true`，其结果期望在后续 `wait = "step"` 时检查。
`await_file = "${work}/ready"` 等待被测程序写出的就绪文件；
`send_signal = {process = "step", signal = "SIGTERM"}` 只向那个进程发送真实信号。
所有后台步骤必须有 wait。失败或中断时 runner 清理自己启动的进程并收割。
wait 之前只有后台进程的 `.pid` 可引用；result／stdout 等要等完成后才可用。
变量按每个变体分别校验，不能借用其他变体才声明的变量。
长诊断 JSON 文件在发现时加载和校验引用，避免启动编译后才发现配置错误。文件名可含 `${target}`，按当前目标加载；fixture 必须声明 `targets`。未指定目标或目标不适用时用其首个声明目标校验格式，诊断内容仍完整比较且不受 snapshot 更新影响。
不允许逐 case Python/shell 编排脚本、动态 predicate 或专用注册表。

报告单列用例、变体、进程和 golden 数量，不将支持文件或目标不适用项计作通过。
`--work-dir` 或失败时保留 report 与工作区，`--keep` 也可用于审阅成功运行。

内置 Cone 的测试入口使用仓库 `sysroot/src` 中的当前源码；`--sysroot` 只选择预编译系统库与 native 资源。把 sysroot 指向独立的已准备目录时，不会误跑该目录中的旧版测试源码。

workspace 多根命令使用 `json = "batch-stderr"`，按顺序在 `${step.records}` 保留每个构建／检查／测试结果；`result` 与 `test_result` 分别为最后一个对应结果。单次运行程序仍用原 `stderr` 模式，在首个工具结果后保留程序的 stderr 原文。

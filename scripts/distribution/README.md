# Scoop 工具包

Windows 的主驱动名为 `sco.exe`，在 PowerShell 中使用 `sco --version`、`sco run main.scoop`、`sco pm` 和 `sco example`。macOS／Linux 仍使用 `scoop`；下文示例中的 `scoop` 在 Windows 上替换为 `sco`。其余程序名不变：`scoopc.exe`、`scoop-link.exe`、`scoop-lsp.exe`、`scoop-pm.exe` 以及 `commands/scoop-example.exe`。Windows 工具包不提供 `scoop.exe` 别名，避免与同名包管理器冲突。

工具包直接包含安装根内容，默认解压到 `~/.scoop`，然后把 `~/.scoop/bin` 加入 PATH。将下面的归档文件名替换为实际下载的版本与宿主平台：

```sh
mkdir -p "$HOME/.scoop"
tar -xJf scoop-0.0.0-dev-aarch64-apple-darwin.tar.xz -C "$HOME/.scoop"
export PATH="$HOME/.scoop/bin:$PATH"
cat "$HOME/.scoop/VERSION"
scoop --version
```

归档内没有额外的版本目录、`.scoop` 或 `sysroot` 层，无需 `--strip-components`。五个命令通过实际执行文件定位相邻资源，也支持正常软链接启动。也可解压到其他安装根；移动整个目录后可以直接继续使用。

安装根的 `VERSION` 第一行为发行版本，随后记录源码 revision 与宿主平台。`scoop --version`、`scoopc --version`、`scoop-link --version`、`scoop-lsp --version`、`scoop-pm --version` 显示同一份发行版本和源码 revision。此目录中的 `toolchain.toml` 列出宿主、目标、内置 Cone、native 版本、VSIX 和编程 skill 路径；它只说明包内容，不是命令启动时的验证清单。

Darwin/AArch64 包要求声明版本的 macOS、Xcode 或 Command Line Tools、Apple Clang、macOS SDK 和系统 linker。Linux/GNU amd64 包要求声明版本的 glibc、GCC、GNU ld 和开发文件；生成 musl 程序还需要 musl-gcc 与匹配的 musl 开发文件。GNU 宿主工具不承诺在纯 musl 系统运行。native C++ 另需配套 clang++／g++ 与标准库；musl C++ 不在当前范围。

Windows x64 包的最低版本以 `toolchain.toml` 为准，包含宿主 Clang／LLD／RC 工具、三个 Windows target 的系统库与 runtime，以及两个 LLVM-MinGW CRT 的 x86-64 开发资源。默认 MSVC UCRT 目标需要本机 C++ Build Tools 和 Windows SDK，或显式 `--native-sysroot` 所指的 `msvc.toml`；不要求完整 Visual Studio IDE。两个 MinGW 目标可直接使用随包资源，例如 `sco run --target x86_64-w64-windows-llvm-mingw-ucrt main.scoop`。应用开发不需要维护构建使用的 Rust、CMake、Perl 或 MSYS。native MSVC C++ 应用按实际依赖部署 VCRuntime／MSVC STL DLL。

内置库以 release 预编译，debug 和 release 应用都直接使用 `~/.scoop/lib/<target-triple>/*.slib`；当前应用需要的泛型实例仍正常生成。空缓存也不触发系统库重编。普通 build、run、test 和 link 不重新编译内置 Cone 或 runtime。应用构建缓存和输出写到各自正常位置，安装目录无需可写。显式 `--sysroot` 或环境变量 `SCOOP_SYSROOT` 会覆盖默认安装根；使用安装包时不需要设置这两项。

包管理器位于 `bin/scoop-pm`，可通过 `scoop pm` 调用。普通 `build/run` 在没有 `Cone.lock` 时自动解析依赖并创建 lock；`--locked` 要求已有匹配选择，`--frozen` 同时禁止联网。workspace 成员可用 `-p group:name` 或 `--workspace` 选择。

Git 和静态 registry 的源码默认共享在 `~/.scoop/packages/`，可用 `SCOOP_PACKAGE_CACHE_DIR` 或命令选项指定位置。PM 只按索引选中版本下载源码，构建输出仍留在项目 target 与构建缓存中。编辑器消费已有 lock；缺少输入时显式运行 `scoop pm resolve` 或 `scoop pm fetch`。当前发行提供 registry 读取/下载，源码打包与 publish 流程另行实施。

安装根的 `commands/` 用于外部子命令，包中提供 `scoop-example`。放入其他宿主可执行程序 `commands/scoop-somecommand` 后，运行 `scoop somecommand <args>` 即可调用它。内置子命令优先；外部子命令之后的所有参数（包括 `--help` 和 `--`）原样传递，Scoop 自身的选项应放在子命令之前，例如 `scoop --sysroot /path/to/scoop somecommand --help`。程序继承当前目录、环境和标准输入输出，退出码及终止信号保持。

`scoop example` 离线提供 24 个入门、语言、集合、文件、JSON、异步 HTTP、命令行和测试主题：

```sh
scoop example --list
scoop example --search "JSON 序列化"
scoop example language/option
scoop example http/get --format source --file client.scoop
scoop example json/typed --format json
```

默认输出完整程序、运行条件、操作步骤、预期结果和关键规则。把显示文件保存到可写目录后按说明运行；example 本身只读取材料。新示例和正文位于 `share/scoop/examples`，复用的库示例仍在 `src/`。指定其他安装根查询时，后续运行也使用同一 `--sysroot`。

面向 AI 的编程 skill 位于安装根的 [`tooling/scoop-programming/SKILL.md`](../../tooling/scoop-programming/SKILL.md)。它指导 AI 查询示例与库 API、正确处理语言差异和 Cone 依赖，并使用 fmt/check/test/build/run 验证程序。可向 AI 提供该文件，或按所用工具的方式自行添加整个 `scoop-programming/` 目录，保留 `references/` 与 `agents/`。工具包只分发这些文件，不自动安装、注册 skill 或修改 AI 工具的配置。

VS Code 扩展位于 `tooling/scoop-language-support-<extension-version>.vsix`，内含语法高亮、折叠和缩进所需代码及 WASM。在 VS Code 中选择“从 VSIX 安装”，或运行：

```sh
code --install-extension "$HOME"/.scoop/tooling/scoop-language-support-*.vsix
```

安装后使用扩展无需 Node.js、npm 或 WASM 构建工具。扩展默认启动 `~/.scoop/bin/scoop-lsp --stdio`，提供诊断、补全、签名、导航、符号、引用、重命名和快速修复；高亮、折叠与缩进由 Tree-sitter 提供。安装到其他位置时，在 VS Code 设置 `scoop.server.path` 为该安装根的 `bin/scoop-lsp` 绝对路径。`scoop.sysroot` 默认沿用服务器安装布局；工作区需受信任才能启动语言服务器。可用命令面板的 `Scoop: Restart Language Server` 重启，或 `Scoop: Show Language Server Output` 查看日志。

```sh
scoop run hello.scoop
scoop build my-cone --release
scoop test my-cone
scoop fmt my-cone
scoop fmt my-cone --check
scoop check my-cone
scoop check my-cone --tests
scoop build my-cone --target x86_64-unknown-linux-musl
```

最后一条命令用于 GNU 宿主包，并要求 musl 开发环境。非 core 库仍须在 `Cone.toml` 声明普通依赖；包中存在某个库不会自动导入它。`src` 保留库源码，`vendor` 保留相关 native 输入。显式 source／artifact 依赖和 `--sysroot` 可选用另一组兼容资源；`--runtime-root` 明确请求从另行提供的 runtime 源码构建。

`scoop fmt` 默认格式化当前 Cone 的生产、测试及各平台声明的源码，也接受多个 Cone 或显式 `.scoop` 文件。默认写回变化文件；`--check` 只检查格式，有差异时退出 1，可用于 CI。语法错误文件保留原文，其他已选文件继续处理。

`scoop check` 默认检查当前 Cone 的生产源码，也接受一个 Cone 或 single-file 输入。检查完成完整前端语义及必要的 generator，不生成根 Cone 的机器产物或执行程序；源码依赖需要更新时仍正常构建。`--tests` 选择测试用途，包含生产与测试源码，但不验证生产 main；CI 可分别运行默认检查与 `--tests`。两命令均支持 `--message-format json`，完整选项可用各自的 `--help` 查看。

用浏览器打开安装根的 `docs/index.html`，选择应用所用目标，即可浏览所有内置库的 API、说明和源码。页面之间使用相对链接，搜索及资源随包提供，无需联网或启动服务器。Darwin 包提供 Darwin 文档，Linux 包分别提供 GNU 与 musl 文档，Windows 包提供 Microsoft UCRT 及两个 LLVM-MinGW CRT 目标的文档；源码直接嵌入 HTML，移动完整安装目录后仍可阅读。

自己的 Cone 可用 `scoop doc my-cone` 生成离线文档；多个 Cone 可同时作为参数传入。`--no-deps` 只展示显式选择的根，`--deny-doc-warnings` 将文档 warning 作为生成失败处理。只有预编译产物的依赖仍可参与类型检查；没有可用源码文本时不生成源码展示部分，`.slib` 不包含完整源码。

使用统一 HTTP client 时，在应用的 `Cone.toml` 中声明：

```toml
[dependencies]
"scoop:scoop.http.client" = "0.1.0"
```

完整的 client 入门程序及其直接依赖见安装目录中的 `src/scoop.http.client/README.md`；压缩内存操作、同步文件流和异步流示例见 `src/scoop.compression/README.md`。

`scoop.idna`、`scoop.url`、`scoop.charset` 和 `scoop.compression` 也可分别作为普通依赖使用。IDNA 和 NFC 使用 Rust ICU4X，不要求 ICU4C 或 C++；URL、文本和压缩所需的 native 静态库、头文件、源码与许可已经随工具包提供。压缩的同步与异步流共用一个 Cone。

`scoop.regex` 提供 PCRE2 原生模式语法，使用独立的 PCRE2-8 C 静态库。其三个目标的 archive、头文件与许可已随包提供，应用无需另行安装 PCRE2 或 Rust。查询、替换、分割、Unicode 标量索引和 close/use 的说明见 `src/scoop.regex/README.md`。

Darwin 默认最低版本由发行配置确定；`MACOSX_DEPLOYMENT_TARGET` 可显式选择兼容的部署版本。默认 runtime 的最低要求不能高于所选版本。较老系统的支持必须另经实际验证，不能通过降低此变量得到保证。

升级时可将旧 `~/.scoop` 移到备份位置，再创建新的 `~/.scoop` 并解压完整包，避免旧版本文件残留；也可安装到另一目录并调整 PATH 或软链接。无需删除整个用户缓存。第三方许可见 `../licenses/README.md`；Scoop 自身的许可状态也列在该文件中，应以维护者确定的项目许可为准。

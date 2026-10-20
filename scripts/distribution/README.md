# Scoop 工具包

把此完整目录解压到常用位置，将其中 `bin` 加入 PATH。三个命令通过实际执行文件定位相邻资源，也支持正常软链接启动。移动整个目录后可以直接继续使用。

`scoop --version`、`scoopc --version`、`scoop-link --version` 显示发行版本和源码 revision。此目录中的 `toolchain.toml` 列出宿主、目标、内置 Cone 和 native 版本；它只说明包内容，不是命令启动时的验证清单。

Darwin/AArch64 包要求声明版本的 macOS、Xcode 或 Command Line Tools、Apple Clang、macOS SDK 和系统 linker。Linux/GNU amd64 包要求声明版本的 glibc、GCC、GNU ld 和开发文件；生成 musl 程序还需要 musl-gcc 与匹配的 musl 开发文件。GNU 宿主工具不承诺在纯 musl 系统运行。native C++ 另需配套 clang++／g++ 与标准库；musl C++ 不在当前范围。

内置库以 release 预编译，debug 和 release 应用都直接使用这些产物；当前应用需要的泛型实例仍正常生成。普通 build、run、test 和 link 不重新编译内置 Cone 或 runtime。应用构建缓存和输出写到各自正常位置，安装目录无需可写。

```sh
scoop run hello.scoop
scoop build my-cone --release
scoop test my-cone
scoop build my-cone --target x86_64-unknown-linux-musl
```

最后一条命令用于 GNU 宿主包，并要求 musl 开发环境。非 core 库仍须在 `Cone.toml` 声明普通依赖；包中存在某个库不会自动导入它。`src` 保留库源码，`vendor` 保留相关 native 输入。显式 source／artifact 依赖和 `--sysroot` 可选用另一组兼容资源；`--runtime-root` 明确请求从另行提供的 runtime 源码构建。

使用统一 HTTP client 时，在应用的 `Cone.toml` 中声明：

```toml
[dependencies]
"scoop:scoop.http.client" = "0.1.0"
```

完整的 client 入门程序及其直接依赖见安装目录中的 `src/scoop.http.client/README.md`；压缩内存操作、同步文件流和异步流示例见 `src/scoop.compression/README.md`。

`scoop.idna`、`scoop.url`、`scoop.charset` 和 `scoop.compression` 也可分别作为普通依赖使用。IDNA 不要求 ICU；URL、文本和压缩所需的 native 静态库、头文件、源码与许可已经随工具包提供。压缩的同步与异步流共用一个 Cone。

Darwin 默认最低版本由发行配置确定；`MACOSX_DEPLOYMENT_TARGET` 可显式选择兼容的部署版本。默认 runtime 的最低要求不能高于所选版本。较老系统的支持必须另经实际验证，不能通过降低此变量得到保证。

升级时解压到另一个版本目录并调整 PATH 或软链接，原版本仍可保留。无需删除整个用户缓存。第三方许可见 `../licenses/README.md`；Scoop 自身的许可状态也列在该文件中，应以维护者确定的项目许可为准。

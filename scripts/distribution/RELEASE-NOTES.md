# M52 工具链交付

本期提供 Darwin/AArch64、Linux/GNU amd64 两类宿主工具包，覆盖 Darwin、GNU、musl 三个既有目标。宿主包包含 `scoop`、`scoopc`、`scoop-link`，以及 LLVM 22.1.8 的所需动态依赖。M47 延后，不附带 generator。

统一布局保留十个内置 Cone 的源码、预编译产物、公开 C 头、runtime 对象、OpenSSL 静态库和 Linux libunwind。libuv 与 Ryu 继续编入实际使用它们的 Cone／runtime。LLVM 原始源码归档、补丁和许可证另存于同版本的 `llvm-sources` 包。

仓库中的准备、LLVM 构建和打包脚本供维护者显式调用；常规 CLI 测试和 CI 不自动运行打包，也不要求先生成归档。具体验证范围以仓库的 M52 实施记录为准。此包的部署配置、源码 revision 与实际组件列表见 `toolchain.toml`。

项目维护者尚未选定 Scoop 自身的 LICENSE 时，包只用于当前本地构建和功能验收；此说明不授予新的对外分发许可。已有第三方许可随组件保留。

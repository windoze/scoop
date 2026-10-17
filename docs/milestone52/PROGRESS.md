# M52 实施记录

基线：`43ff43117`。先完整完成 M52，再继续 M53。

已确认范围：M47 尚未定稿，延后决定；本期分发现有 `scoop`、`scoopc`、`scoop-link` 三个宿主命令，不以 generator 为前置。sysroot 准备和发行打包放在仓库脚本，不向命令或 driver 添加一次性维护功能。M53 采用 llhttp 9.4.3，并明确已知方法集合限制。

实施与验收按功能记录；原始日志、构建缓存和发布归档保存在外置盘仓库已忽略的 `tmp` 下，不提交原始记录。M52 总体验收尚未完成。

## Runtime 索引

- schema 4 按索引目录解析相对对象路径；生产记录保留完整编译信息，消费只检查目标、runtime ABI 和最低部署版本。构建记录切换为 `runtime-v4`。
- 本机 `cargo fmt --all`、`cargo clippy -p scoop -p scoop-linker --all-targets` 通过；三个配套命令以 release 构建通过。
- release 定向测试通过：runtime 构建／缓存 3 项；复制到带空格目录后真实链接运行 1 项。覆盖兼容编译器／SDK、错误目标、过低 macOS 部署版本、实际输入变化、缺失输入及失败不发布。
- Linux 完整回归留到本机各功能闭合后统一执行。


## 共享 native 源码

- manifest native 源文件允许归一化后的前置 `..`；Scoop 源码路径约束不变。native 对象键采用独立路径类型，父路径贯通对象生产和 `.slib` 消费。
- 格式化与相关 crate 的全部 target lint 通过；manifest 单元测试 61 项、路径身份定向测试 8 项通过。
- 本机正式 CLI 验证通过：8 个相关 negative fixture；2 个新增正向 fixture、3 个变体，覆盖共享 C 源码与头文件更新、跨 Cone C++／泛型、release 与 moving GC。首次正向验证发现的产物路径类型限制和用例泛型语法错误均已修复并复验。


## 源码、vendor 与目标资源布局

- 普通 Cone 迁至 `sysroot/src`；libuv、OpenSSL、Ryu 及 LLVM 22.1.8 的版本资料／补丁集中在 `sysroot/vendor`。目标 `.slib`、OpenSSL 与 unwind 的位置统一为 `sysroot/lib/<canonical-triple>`。
- runtime 构建显式接收 vendor 根，Ryu 的真实源码和头文件变化进入现有 depfile／更新记录。自建 sysroot 的 fixture 显式保留共享 vendor 输入；缺少 vendor 时报告所选路径。
- 本机全 workspace 格式化与 lint、Python Ruff 通过；构建编排单元测试 85 项、新增缺失 vendor 诊断 1 项、目标布局 1 项及 fixture runner 规则 50 项通过。
- 正式 CLI 验证通过：共享 native、C++／泛型、OpenSSL crypto／TLS、显式 core 产物共 5 组、6 个变体；asyncio 的定时器、取消与 UDP 组合 4 项通过。自建 sysroot 缺失 vendor 的首次失败已修复并复验。
- 本机 OpenSSL 在新目录完成实际构建；LLVM 宿主打包、Linux unwind 新版构建与三目标完整验收尚待后续批次。

## 仓库准备脚本

- `scripts/prepare_sysroot.py` 按实际 manifest 依赖调用现有普通 release 构建，安装公开头和 O2 runtime。独立的脚本内部 Rust 辅助程序只复用 runtime 编译库，不进入主 workspace、driver 或发行 `bin`。
- runtime 缓存升级为 `runtime-v5`，使用可直接复制的 index／objects 布局；索引仍为 schema 4，不重复解析或哈希对象。
- 本机 Rust 格式化、相关 crate 和辅助程序 lint、Python Ruff 通过。手动完成十个内置 Cone 与 runtime 的首次准备及重复准备；runtime 构建／更新测试 4 项、搬迁后真实链接运行 1 项通过。
- 打包脚本保持手工使用，不作为普通 CLI 回归的执行步骤。

## 默认预编译 Cone

- 默认 core 和其他已声明的 scoop 依赖统一读取 `lib/<canonical-triple>/<name>.slib`；显式 source／artifact／search-root 优先，默认产物缺失时诊断目标、coordinate 和路径。`scoop link` 使用相同的显式优先顺序。
- 随包源码仍保留；源码变化不影响默认产物消费，显式重建发布后按普通依赖更新。新增带空格 sysroot、独立空缓存、debug／release、泛型、moving GC、默认链接和缺失 core 覆盖。
- 本机格式化与相关 crate lint、83 项构建编排单元测试通过。默认库正式 CLI 17 组、19 个变体通过；缓存及显式 core 相关 26 组通过，另 2 组仅适用于 Linux，留到最终 Linux 验收。旧用例的默认源码边、core 编译调用数、产物摘要数量和显式路径诊断预期已更新并复验。

## 安装定位、公开头与默认 runtime

- 三个命令按显式参数、环境变量、实际执行文件的安装根选择 sysroot，配套 scoopc 跟随实际宿主命令位置。用户 native 编译读取所选 sysroot 的公开头，protocol 已有 native 对象时不重复准备头文件或源码。
- build／run／test 及两个 link 命令默认消费目标 runtime 索引，保留显式源码和对象覆盖。相对 index 输入先按调用者 cwd 定位；缺失索引诊断包含目标和路径，library 构建无需该资源。
- 本机 Rust 格式化、四个相关 crate 的全部 target lint 与配套 release 命令构建通过。正式 CLI 10 组、12 个变体通过，覆盖三个命令的软链接启动、带空格安装根、相对覆盖、真实头文件更新、测试库、泛型、moving GC，以及既有 runtime 源码更新与预编译消费。新增 fixture 的 Unsafe 块语法已修正并复验。

## 默认 native 库搜索

- 已声明的 native 库依次使用显式搜索根、目标 sysroot 及其 OpenSSL 目录、系统 provider；所选层的歧义或无效输入直接诊断。Darwin 动态依赖和 Linux ELF 闭包沿用相同优先级，默认路径只作为本机定位信息。
- 最终工具缓存切换为 `final-tools-v2`，键包含实际选择的 sysroot 搜索根；命中继续直接复用。
- 本机 Rust 格式化、相关 crate lint、Python Ruff 及三个 release 命令构建通过。正式 CLI 7 组、10 个变体通过，覆盖默认静态库、OpenSSL、显式覆盖、真实 archive 更新后仅重链、坏候选诊断、动态依赖、预编译 crypto／TLS 的 debug／release 与 moving GC。旧缓存目录 glob 和动态库真实路径预期已更新并定向复验。

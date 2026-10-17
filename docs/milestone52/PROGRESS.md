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

## 部署配置与工具版本

- `scripts/release.toml` 固定 LLVM 22.1.8、CPU 与实际系统构建基线。Darwin 默认最低版本在构建时写入工具，`MACOSX_DEPLOYMENT_TARGET` 可显式覆盖；不再通过 `sw_vers` 选择。codegen 显式接收已选 native 部署合同，LLVM、generated-C、native、runtime 和最终链接使用相同最低版本。
- OpenSSL 准备采用同一 Darwin 配置或 Linux x86-64 CPU 参数；目标资源将随本期最终 sysroot 准备统一更新。三个命令的版本输出统一包含发行版本与源码 revision，不在运行时读取 Git 或打包清单。
- 本机 Rust 格式化、相关 crate 与脚本辅助程序 lint、Python Ruff、三个 release 命令构建通过。工具链单测 32 项、codegen 定向测试 6 项、fixture runner 规则 50 项通过；正式 CLI 6 组、7 个变体通过，覆盖实际对象部署记录、配置更新／命中、非法版本、过低版本消费 runtime、版本输出及既有缓存组合。新增用例的两处 Scoop 语法及 JSON 空值表示已修正并复验。

## 受控 LLVM 与 native 准备

- `build_llvm.py` 固定 LLVM 22.1.8、AArch64／X86 后端及动态库构建，关闭不需要的外部组件，安装实际 Rust 构建入口；libunwind 准备在更新判断之后才复制／打补丁／探测工具。两个脚本复用源码、工具和实际 depfile 输入的 metadata，不读回命中产物。
- Linux 的普通 C／C++、runtime、startup、OpenSSL 和 unwind 显式采用 x86-64 CPU 基线。生成的 sysroot 资源可放在仓库外置盘的 tmp 中，以正常 symlink 接入开发树；本任务的构建、缓存、日志及生成资源均已迁到该位置。
- Rust 格式化、相关 crate lint、Python Ruff 及 C toolchain 合同测试 6 项通过。Darwin 与 GNU 宿主的受控 LLVM 均已实际构建成功，本机重复调用直接复用；Linux GNU／musl libunwind 首次构建与重复复用成功，GNU dynamic、musl static／dynamic 三种模式的真实 cleanup／catch／delete 测试通过。

## 手工打包脚本

- `package_toolchain.py` 调用仓库准备脚本，组装三个宿主工具、目标资源、实际动态依赖、源码输入和许可证，并生成宿主及 LLVM 源码归档。依赖处理与许可证收集分别放在小模块中；常规 CLI、driver、fixture 和 CI 不调用这些维护操作。
- Darwin 已实际生成两个归档。手工解压到带空格的新目录后，三个命令版本一致，实际加载包内 LLVM；空缓存下的 native／泛型／moving GC 应用、crypto／TLS、库测试及两个 link 入口均通过，重复构建无编译调用。原源码、工具与产物保持存在。
- 首次宿主构建因全局动态库路径影响 Rust 自身 LLVM 而失败，已改为仅给产出的命令设置加载路径并完整重跑成功。sysroot 头文件发布保留外置目录 symlink，fixture 复制资源时物化本轮副本；Python Ruff 和 50 项 runner 规则测试通过。
- GNU 宿主包与完整三目标常规回归仍在本期后续验收中。

## 完整回归收尾

- 本机全 workspace Rust 测试 4,929 项通过。两处输入定位 fixture 改为断言只编译用户 Cone；27 组自定义 core fixture 改为先发布实际消费的版本到目标 sysroot，并移除预编译依赖不再产生的源码警告预期，均已定向复验通过。所发现的旧路径与更新顺序假设已闭合。
- 本机保留各功能定向验证及已完成的既有 CLI 回归，完整正式 CLI 验收集中在 Linux，避免两个宿主重复全量；Darwin 的平台特有部署、动态链接、安装定位和发布包另有定向覆盖。已结束的本机 dev 构建目录清理释放 2.9 GiB。
- Darwin 归档第二次解压到另一安装位置后，三个命令版本查询和空缓存 native／泛型／moving GC 编译运行通过。GNU 十个内置 Cone 与 runtime 已完成准备；musl 的 OpenSSL 首次构建因开发环境缺 Linux UAPI 搜索路径失败，使用指向现有内核头的任务内编译器包装脚本后构建成功，继续准备其余目标资源。

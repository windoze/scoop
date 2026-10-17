# M52 设计：工具链分发与预编译 sysroot

状态：实施中。日期：2026-10-18。调研基线：`2a5a8734d`。

本期把已有编译器、内置 Cone、runtime 和必要的 native 依赖组成可搬迁的二进制工具链。用户解压后将 `bin` 加入 PATH，准备该平台声明的 C 开发环境，即可编译、运行和测试程序；第一次使用空缓存时也直接消费随包的内置 Cone 和 runtime。

依据：[语言规范](../specs/SCOOP-SPEC.md)第 12～14 章、[实现规范](../specs/SCOOP-IMPL-SPEC.md)第 1 节及 2.6～2.8、2.12、2.19、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)第 5、7 章和[路线图](../ROADMAP.md)。本设计接续 M38、M45、M46、M48～M51 的已有结果。M47 尚未定稿，本轮不以它为前置，不实现或分发 generator。本文不表示发布功能已经实现；各批实施前先同步第 9 节所列规范。

## 0. 范围与决定

1. 安装根目录同时作为 Scoop sysroot，提供 `bin`、`lib/<canonical-target-triple>`、`src`、`vendor`、`include`、`libexec` 和 `share`。仓库内置 Cone 源码从 `sysroot/lib` 迁到 `sysroot/src`，第三方源码统一归入 `sysroot/vendor`；目标构建资源统一归入 `sysroot/lib/<triple>`。
2. 普通应用构建默认选择已安装的 `.slib`、runtime 对象和 native 库。随包源码供阅读、诊断定位和显式重建使用，不因源码目录存在而加入应用的构建图，也不在目标产物缺失时自动重建。
3. 现有三个 Scoop 命令位于 `bin`，随包 LLVM 位于 `libexec`，它们都是宿主组件；各目标目录中的 `.slib`、runtime、OpenSSL 和 unwind 资源属于目标。首批发布 Darwin/AArch64 和 Linux/GNU amd64 两类宿主包，覆盖全部三个既有目标。
4. 随包提供 LLVM 22.1 后端需要的宿主动态库及非系统依赖。首版保留平台 C 编译器、链接器、SDK／libc 开发文件的明确外部要求，不在本期新增 Clang／LLD 后端或完整跨平台 C SDK。
5. 每个目标只发布一套 release 内置 Cone 和默认 O2 runtime；应用可继续选择 debug／release。跨 Cone 泛型实例化、内联和正常代码生成仍使用 `.slib` 中的已有数据。
6. 开发 sysroot 准备与发行打包由仓库脚本负责，复用普通 Cone 构建和既有 runtime 编译库。`scoop` 命令及 driver 只保留日常编译、运行、测试与链接职责，不增加 sysroot 构建、安装或打包子命令，也不承载这些操作的编排。
7. 保持 M48／M50 的职责边界：缓存先判断更新，命中直接使用已有结果；发布只组装实际需要的文件，不重放自产 IR、对象、产物及生产过程，不增加来源授权或文件系统审计。
8. 打包流程只提供脚本，供维护者手工测试和发布时显式运行，不作为常规验证步骤，也不接入常规 CI、workspace 测试或 fixture runner。常规回归使用已准备的开发 sysroot；手工测试发布包时才从实际归档解压目录执行。源码、工具和原产物保持存在，通过真实调用与程序结果验证预编译消费、直接复用和输入变化。

## 1. 当前基础与需要衔接的实现

| 现有位置 | 当前行为 | M52 的工作 |
| --- | --- | --- |
| [sysroot 路径](../../compiler/toolchain/src/paths.rs) | 默认路径来自编译时工作区位置 | 改为执行文件所在安装目录，并保留显式覆盖 |
| [core 布局](../../compiler/toolchain/src/trusted_core.rs)、[默认 core](../../compiler/scoop/src/locator/default_core.rs) | 先找 `lib/scoop.core` 源码，缺失时才找旧 `artifacts/<target-id>` | 使用统一的目标目录和预编译默认来源 |
| [其他默认库](../../compiler/scoop/src/locator/default_library.rs) | 已声明的 scoop 依赖默认从 `lib/<name>` 构建 | 从当前目标的 `.slib` 定位普通依赖 |
| [CLI 构建请求](../../compiler/scoop/src/cli/commands.rs)、[runtime 编排](../../compiler/scoop/src/runtime_build.rs) | runtime 默认从仓库源码构建，已有显式对象索引入口 | 安装环境默认使用目标 runtime；显式 runtime 源码选项继续用于普通程序构建，准备脚本复用编译库 |
| [runtime 索引](../../compiler/linker/src/runtime/index.rs) | 当前 schema 3 保存绝对对象路径，并比较完整 C toolchain 合同 | 在同一读写入口中使用可搬迁路径及必要的消费兼容条件 |
| [M49 asyncio](../milestone49/DESIGN.md)、[asyncio manifest](../../sysroot/lib/scoop.asyncio/Cone.toml) | libuv 1.53.0 按 native 源码编进 Cone | 保留其对象在 `.slib` 中的现有交付方式 |
| [M51 native 交付](../milestone51/DESIGN.md#5-native-构建链接与交付) | OpenSSL 3.5.9 静态库与生成头位于旧 `sysroot/native/<triple>`，消费者显式给 library root | 迁入统一目标目录，接通正常的默认库搜索 |
| [Ryu](../../runtime/third_party/ryu/README.scoop.md)、各 Cone 的 vendor、[unwind 准备脚本](../../scripts/build_llvm_unwind.py) | 第三方源码分别位于 runtime、Cone 内或仓库外指定目录 | 集中到 `sysroot/vendor`，更新 native source／include、runtime 构建输入和准备脚本 |

runtime 索引现在已经不再复制、哈希并重验对象内容。M52 只解决安装定位和实际兼容性，不能恢复旧的写后读回、对象证明或完整 replay。M47 延后独立决定，不属于本期交付或验收条件。

## 2. 安装布局与发布内容

### 2.1 统一目录

```text
<sysroot>/
  bin/
    scoop
    scoopc
    scoop-link
  lib/
    <canonical-target-triple>/
      <cone-name>.slib
      runtime/
        index.cbor
        objects/*.o
      unwind/                         # Linux targets
        include/
        lib/libunwind.a
      openssl/
        include/openssl/
        lib/libcrypto.a
        lib/libssl.a
  include/
    scoop_rt.h
    scoop_runtime_metadata_v1.h
  src/
    <cone-name>/
      Cone.toml
      src/
      native/                         # Where present
      tests/                          # Where present
  vendor/
    libuv/
    openssl/
    ryu/
    llvm-project/<version>/           # Build input; separate source archive
  libexec/
    llvm/lib/
      <host LLVM libraries and non-system dependencies>
  share/
    scoop/toolchain.toml
    scoop/README.md
    scoop/RELEASE-NOTES.md
    licenses/
```

目标目录只使用现有 canonical triple：`aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。CLI 接受的别名先按既有规则正规化；目录不继续使用 `darwin-aarch64` 等内部 target id 拼写。

`src/<cone-name>` 保留 Scoop 源码、manifest、项目自身维护的 native shim 和测试资源；上游第三方源码集中到同级 `vendor`，由实际使用者引用。runtime 的公开 C 头放在 `include`；runtime 私有头和实现继续由仓库维护，其 Ryu 输入也从统一 vendor 根取得，不因发布新增公共接口。

仓库与发行包采用同一源码／目标资源布局，不维护两套需要动态猜测的目录规则。更新 OpenSSL manifest 的条件 include，例如 Darwin 从 `../../native/aarch64-apple-darwin/openssl/include` 改为 `../../lib/aarch64-apple-darwin/openssl/include`；继续使用普通路径和已有 `when`，不新增变量插值。脚本、fixture helper 和有效文档链接同步迁移，生成的二进制资源仍不提交为源码。

### 2.2 内置 Cone 清单

基线中以下十个普通 library Cone 均进入每个目标的发布目录。表中的 core 包括现有规则注入的隐式依赖；最终构建顺序从实际 manifest 依赖取得。

| Cone | 当前生产依赖 | 附带的 native 内容／要求 |
| --- | --- | --- |
| `scoop.core` | 无 | 普通 core `.slib` |
| `scoop.std` | core | 无额外 native 库 |
| `scoop.cli` | core | 无额外 native 库 |
| `scoop.json` | core | 无额外 native 库 |
| `scoop.test` | core | 测试事件与输出捕获 C 对象 |
| `scoop.platform` | core、std | 当前目标的系统 C shim |
| `scoop.asyncio` | core、std、platform | libuv 与 shim 对象，普通线程等系统库要求 |
| `scoop.crypto` | core、std、json | crypto shim，static `crypto` 及线程库要求 |
| `scoop.tls` | core、std、crypto、platform | TLS shim，static `ssl` 要求 |
| `scoop.tls.asyncio` | core、std、crypto、platform、tls、asyncio | 复用 TLS 与异步依赖，不再编译一份后端 |

清单是发布输入，不授予库语言特权，也不改变用户 Cone 的依赖声明。仅 core 保持现有隐式依赖；其余库按需显式声明，single-file 继续只依赖 core。所有随包 `.slib` 都按 Production 构建；`scoop.test` 自身作为普通库发布，不把其他 Cone 的测试代码及测试入口编入生产产物。

### 2.3 宿主包与目标矩阵

| 宿主包 | 包含的目标 | 用户侧 native 开发环境 |
| --- | --- | --- |
| Darwin/AArch64 | `aarch64-apple-darwin` | 声明支持的 Xcode／Command Line Tools、macOS SDK、Apple Clang 和系统 linker |
| Linux/GNU amd64 | GNU 和 musl 两个 amd64 目标 | GNU：GCC、GNU ld、glibc 开发文件；musl：可用的 musl-gcc／对应 GCC、GNU ld 和 musl 开发文件 |

需要 native C++ 的程序继续要求 Darwin clang++／libc++ 或 GNU g++／libstdc++；musl C++ 仍按现有范围诊断。GNU 默认动态 PIE，musl 默认静态并保留已有动态模式。musl 动态程序的运行环境需提供相应 loader。

Linux 宿主包中的工具自身使用 GNU 运行环境；生成 musl 程序不表示这些宿主工具可以在纯 musl 系统中运行。本期不新增纯 musl 宿主 LLVM 构建或 Darwin→Linux 的完整交叉工具链。目录可以容纳多个目标，但可使用的组合由已有目标能力、随包资源和明确选择的 native 工具共同决定。

### 2.4 第三方源码统一归属

仓库采用 `sysroot/vendor/<project>`，安装或源码包采用 `<sysroot>/vendor/<project>`。上游源码、源码归档、版本／来源说明、许可证和所需补丁在这个根下集中维护，不再分别存放于各 Cone 或 `runtime/third_party`。Scoop 自己的 shim 仍随所属 Cone／runtime 管理，构建脚本仍放在仓库 `scripts`。

| 第三方项目 | 统一位置 | 实际使用者及迁移 |
| --- | --- | --- |
| libuv | `vendor/libuv/` | 从 asyncio 的 vendor 迁入；asyncio 按普通 `native.sources` 编译所需上游 C 源码 |
| OpenSSL | `vendor/openssl/` | 从 crypto 的 vendor 迁入固定源码归档和说明；crypto、TLS 与三目标准备共用一份上游输入 |
| Ryu | `vendor/ryu/` | 从 `runtime/third_party/ryu` 迁入；runtime 编排显式取得所选 sysroot 的 vendor 根 |
| LLVM／libunwind | `vendor/llvm-project/<version>/` | 宿主 LLVM 和目标 libunwind 的源码输入集中在上游项目目录；同一版本共用源码，确需不同版本时分别保存实际版本目录及对应补丁 |

LLVM libunwind 兼容补丁随对应 LLVM 源码版本放在其 `patches` 子目录，准备脚本显式使用该路径。其他实际由 Scoop 管理的第三方源码也按项目放在同一 vendor 根；Rust crate 依赖继续由现有 Cargo.lock 和 Cargo 流程管理。

vendor 只保存上游输入和必要维护资料。目标生成头、配置头和 archive 放在 `lib/<triple>`，宿主 LLVM 二进制放在 `libexec/llvm`；解压、配置、打补丁后的构建副本和中间结果使用仓库外工作目录。GNU／musl 等目标共用固定上游源码，不在 vendor 中各复制一份目标源码树，也不在原始上游输入里交替覆盖生成配置。

asyncio 的 native 条目改为例如 `../../vendor/libuv/src/fs-poll.c`，include 改为 `../../vendor/libuv/include` 与 `../../vendor/libuv/src`。当前 `native.sources` 复用 Scoop 源码的路径规则，不能用相对父路径访问这个位置；本期在语言规范 12.2.1 明确允许普通 Cone-relative native 源文件路径保留越出 Cone 根的 `..`，并调整对应输入表示、目标筛选和缓存记录。Scoop 源码的逻辑路径规则不因此改变，不引入新的 vendor locator、来源资格或插件机制。

runtime 构建同时接收已有的 runtime source root 和所选 sysroot 的 vendor 根，直接把 Ryu 文件与 include 交给既有 C 构建路径，不再假定所有源文件都位于 runtime source root。原有 depfile、实际输入变化和缓存复用继续适用。

二进制发行包随统一 vendor 目录提供显式重建内置 Cone／runtime 所需的 libuv、OpenSSL、Ryu 输入。完整 LLVM／libunwind 上游源码单独提供同发行版本的 vendor 源码归档，解压后使用同一 `vendor/llvm-project/<version>` 布局；不要求每个宿主二进制包重复携带整套 LLVM 源码。普通应用构建仍只消费预编译结果，不读取或准备 vendor 源码。

## 3. 默认定位、显式覆盖与源码使用

### 3.1 安装根目录

sysroot 选择顺序为：显式 `--sysroot`、`SCOOP_SYSROOT`、当前可执行程序所属 `bin` 的父目录。所有 Scoop 命令使用同一规则；通过正常文件定位找到执行文件及其相邻工具，不以调用者 cwd 或编译时仓库路径推导安装位置。

相对 CLI 路径仍按调用者 cwd 解析。`--scoopc` 保持现有覆盖含义，缺省配套工具来自当前工具安装的 `bin`；显式 sysroot 可选择另一份兼容目标资源，不自动更换已选择的宿主工具。

从 `target/debug`／`target/release` 运行仓库开发工具时，通过 `--sysroot` 或 `SCOOP_SYSROOT` 明确选择已准备的开发 sysroot。公共 fixture 配置统一传入该位置；正式路径不再依赖 `CARGO_MANIFEST_DIR` 找到本机仓库。

### 3.2 Cone 默认来源

继续先处理 manifest 显式 source／artifact locator、图中已经选定的同一依赖及显式 `--cone-path`，保留现有 exact version、唯一选择和歧义规则。只有尚未定位且 group 为 `scoop` 的普通依赖，才从当前 sysroot 的 `lib/<triple>/<name>.slib` 取得默认产物。发行清单描述随包内容，不作为库来源白名单；实际选中结果使用普通 coordinate 和依赖规则。

普通应用构建不把 `src/<name>/Cone.toml` 作为缺省来源，不为了缓存键枚举或读取这些源码。源码与 `.slib` 同时存在时仍直接使用产物；目标目录、所需 `.slib` 或版本缺失时，报告具体目标、coordinate 和路径，不切换目标或自动执行源码构建。显式来源的失败也不被默认来源掩盖。

用户可以显式构建 `src/<name>`，或在 manifest 中声明正常 source locator，沿既有构建图修改和重建内置库。修改 core 后需要更新的普通依赖按原有输入关系重建；不把替换一个文件当作全套依赖已更新。sysroot 提供默认位置，库的实体身份、可见性及 ABI 仍由普通语言和产物规则表达。

源码位置继续保存 Cone identity 和 Cone-relative logical path；诊断需要显示随包源码时映射到当前安装的 `src/<name>`，不将生产机器绝对路径变成查找要求。仅显示或读取实际需要的位置，不在普通依赖定位中扫描全部随包源码。

### 3.3 公开头、runtime 和 native 库

用户 native 编译的默认 Scoop 公开 include 根改为所选 sysroot 的 `include`，直接 `scoopc` 和父命令使用同一选择；不继续读取仓库 `runtime/include`。真正使用的头文件参与普通 native 输入更新记录。

普通程序的 runtime 输入默认是 `lib/<triple>/runtime/index.cbor`。显式 `--runtime-objects`、`--runtime-root` 保持互斥覆盖：前者使用指定对象集，后者明确请求从源码构建。library 构建无需先准备未使用的 runtime 或最终链接工具。

`build/run/test/link` 与 `scoop-link` 统一在用户显式 `--library-path` 之后加入当前目标的默认 native 搜索根，包括 `lib/<triple>` 和 `lib/<triple>/openssl/lib`；默认 unwind prefix 为 `lib/<triple>/unwind`，显式 `--unwind-prefix` 优先。`scoop link`／`scoop-link` 未给 runtime index 时也采用上述默认值；已有显式用法继续有效。

搜索与实际链接只处理依赖闭包声明的库，不要求空 main 先加载或探测 OpenSSL，不因安装包包含 TLS 而给所有程序附加 TLS 依赖。OpenSSL 保持 static 需求，默认使用目标包资源；缺失时报告真实缺失输入或链接错误，不自动下载、编译或改用系统共享 OpenSSL。声明只使用 crypto 的程序不要求 libssl。

## 4. 预编译结果与可搬迁性

### 4.1 `.slib` 与优化

保留完整 `.slib` 中下游编译和链接实际需要的 HIR／MIR／LIR metadata、泛型／默认参数正文和对象，不为减小安装包删除这些数据。release 内置库可与 debug 应用共同使用，依据实现规范 2.19；应用的 profile 不触发内置 Cone 的重新构建。

消费者使用 `List<MyType>` 等泛型时，仍按已有规则从依赖数据生成实际需要的实例、执行内联和发射当前代码。这属于当前消费者的正常编译。上游 Cone 的源码编译不因此重跑。

### 4.2 runtime 对象索引

保留已有 runtime index 和普通对象集合，不为发布新增 runtime `.slib`、证明文件或另一套 reader。把现有索引中的对象 locator 改为相对索引文件所在目录的路径，发布布局使用 `objects/<file>.o`；同一读写入口也用于开发缓存。

writer 从已有构建结果取得对象位置及已有链接数据，组装包时复制对象并写入相应相对位置；reader 只按索引目录解析实际路径并读取本次所需数据。必要的目标／ABI／格式判断在普通入口完成；不为了搬迁重新哈希、重解析所有对象或复核索引保存的完整生产信息。文件不存在时报告实际 I/O／链接错误。

索引升级为 schema 4，runtime 构建记录使用 `runtime-v5`，每批保存 index 与同级 `objects` 目录，使旧布局的记录正常失效；不保留两个发布／缓存读取模式。准备脚本直接复制这些已知文件及相对索引。发布结果不含原构建缓存、临时目录或生产机器绝对路径的运行依赖。

### 4.3 native 资源

OpenSSL 沿 M51 的固定源码和目标构建方式交付 `libcrypto.a`、`libssl.a` 与匹配的生成头。普通准备脚本从统一 `vendor/openssl` 取得源码归档，更新目标 prefix，并按新的条件 include 连接 shim 编译；源码与生成配置头继续分离，GNU／musl 资源不能混用。

libuv 从统一 vendor 位置作为 asyncio 的 native 源码进入 `.slib`，不再额外构建并链接一份 `libuv.a`；Ryu 同样直接编入 runtime 对象集合。内置 Cone 中其他 shim 也沿现有 `.slib` 对象成员交付。上游未嵌入 `.slib` 的 archive 才作为单独目标资源提供，不根据库名复制平行实现。

Linux LLVM libunwind 继续分别为 GNU／musl 构建，携带当前实际需要的兼容补丁，依据 [M28 构建说明](../milestone28/BUILDING.md)。Darwin 使用系统 provider；GNU native C++ 使用配套 libgcc_s，其他 Linux C 配置使用相应 LLVM libunwind。选择由现有工具链配置和普通链接参数完成，不恢复 M50 删除的 linker map 或最终对象复核。

## 5. 消费兼容性与系统基线

发布工具、内置库、runtime 按同一版本准备；语言实体继续使用实际 Cone coordinate，不能用发行版本或安装路径替代 typed identity。

区分以下两类信息：

| 信息 | 用途 |
| --- | --- |
| target、架构、对象格式、libc、Scoop／runtime ABI、必要格式和实际 native ABI 要求 | 决定本次编译或链接是否能够使用该结果 |
| compiler 完整版本、SDK 版本、工具路径、mtime／大小、实际编译参数 | 记录实际生产配置，并决定本机构建输入是否需要更新 |

当前 runtime 索引要求完整 C toolchain 合同相等，Darwin 的默认部署版本也来自 `sw_vers`。M52 必须把这些消费条件收敛到实际 ABI 和部署要求，允许兼容系统上的不同编译器／SDK 消费同一发布库。不能仅凭版本字符串、原始工具路径或生产配置摘要不同拒绝兼容产物；实际不兼容的目标、libc、调用及格式仍正常诊断。

部署基线须同时覆盖宿主工具和生成程序。发布构建显式设置支持的最低 macOS／glibc／musl 和 CPU 指令集，统一传给 Rust 宿主工具、LLVM、runtime、内置 native 源码与 OpenSSL；生成的 Scoop 对象、C bridge 和最终链接也使用选定的目标部署配置，不再默认跟随构建机器的系统版本。SDK 的生产版本不等同于产物最低运行版本。

具体最低版本由 M52-1 根据实际 API、上游依赖要求和可运行的验证环境确定并写入发布构建配置与说明，再开始后续产物构建。本设计不宣称尚未验证的旧系统支持。现有 OpenSSL Darwin 构建显式使用 macOS 13.0，这是一项输入事实，不能据此直接认定完整工具链也支持 13.0。

基线核对采用专门的构建／发布检查和真实运行。普通应用构建只消费已有部署信息，不每次重扫宿主工具和所有动态依赖，不以全量对象检查替代最低环境中的功能验收。

## 6. 随包 LLVM 与外部工具边界

首版沿用当前 inkwell 的 LLVM 22.1 动态链接方式，分发受控构建的宿主 `libLLVM` 及实际需要的非系统依赖。上游输入从统一的 `vendor/llvm-project/<version>` 准备；精确补丁版本和构建选项在 M52-1 固定，不能随 PATH 中任意 `llvm-config` 切到另一个 major，也不直接把开发机器 Homebrew／发行版路径作为安装合同。

LLVM 使用上游正常 distribution 构建能力，裁剪到现有后端实际使用的组件及 AArch64／X86 目标；保留 Scoop 需要的 GC、异常和目标发射能力。采用上游支持的 `LLVM_BUILD_LLVM_DYLIB` 路径，不使用仅服务开发的 `BUILD_SHARED_LIBS` 组合。现有 RS4GC 和后端选项继续由 codegen 负责，不在打包脚本中复制 LLVM 变换策略。

Darwin 通过正常 install name／rpath 使用相对于工具和私有库目录的位置；Linux 使用 `$ORIGIN` 相对 RUNPATH，并为需要它的依赖库配置各自的查找位置。随包库的传递依赖也必须能按这一布局取得。实际需要的系统签名处理在修改加载路径之后完成；不要求用户设置 DYLD_LIBRARY_PATH／LD_LIBRARY_PATH 来运行 Scoop。

本期发行包无需向用户提供 `llvm-config`、`llc`、LLVM 开发头文件或整套 LLVM 测试工具。确有生产命令调用的 LLVM 工具才随宿主私有目录交付；测试用的外部观察工具属于开发／验收环境。

Scoop 新程序的生成 C bridge、native 源码及启动代码仍需要 C 编译器，最终链接仍需要平台 linker／SDK／CRT／libc。Darwin 使用系统开发环境，Linux 沿既有 GCC 和 GNU ld 路径。本期随包 LLVM 解决后端安装依赖，不声称同时提供完整 C/C++ 开发环境；未来更换为 Clang／LLD 或增加跨平台 SDK 应按实际工具链能力另行设计。

## 7. 仓库中的 sysroot 准备脚本与开发流程

仓库提供 `scripts/prepare_sysroot.py`，接受 sysroot、目标、runtime 源码根、现有 Scoop 工具位置、cache 和实际需要的 native 选项。该脚本是维护源码树与发行包的开发工具，不安装到用户的 `bin`，也不由 `scoop` 或 driver 调用。Scoop 源码取 `<root>/src`，第三方源码取 `<root>/vendor`，后者也作为 runtime 编译的明确输入。一次准备一个现有目标，使用固定的 release 内置库配置和默认 O2 runtime。

调用前准备该目标的 OpenSSL 和 Linux unwind 资源；这些准备由仓库中的普通脚本及其上游构建工具完成。准备脚本根据实际 manifest 依赖排序，调用已有 `scoop build` 依次构建 core 与其余内置 Cone。每个 Cone 都是普通源码编译请求，已完成依赖通过当前目标的 `.slib` 提供；不拼接 core 源码或复刻 stage pipeline。runtime 由脚本旁的内部 Rust 辅助程序直接调用现有 `build_runtime` 库函数；这个实现细节不成为随包命令或 driver 的新职责。

编译结果写入当前目标的规定位置；源码与 native 文件变化按 M48 的普通构建记录更新，未变化组件直接复用。runtime 准备直接调用已有编排，不通过构建一个虚构应用触发，不重新实现 C 源码清单、对象信息提取或缓存。

开发者在迁移后的仓库里显式运行脚本准备所需目标，再运行正式 fixture；只测试 parser／HIR 等无关 crate 时不增加 sysroot 准备前置。修改内置库时，可显式构建某个源码 Cone，也可重跑脚本更新实际受影响依赖。普通应用构建始终使用已经选定的目标产物。开发 sysroot 准备不要求制作发行归档。

## 8. 发布生产、版本与安装

### 8.1 发布编排

新增一个仓库内的普通打包脚本，只供维护者手工测试和发布时显式运行。打包、压缩、归档解压与搬迁验证不纳入常规验证流程；`cargo test`、正式 CLI 回归、fixture runner 和常规 CI 均不调用打包脚本，也不以生成发行包为前置。脚本按以下流程生成归档，不自动启动完整测试套件：

1. 取得明确的源码 revision、发行版本、Cargo.lock、LLVM／native 依赖版本及已确定的 host／target 部署配置。
2. 构建该 host 的 release `scoop`、`scoopc`、`scoop-link`，并准备受控 LLVM 动态库集合。
3. 在新的组装目录中放入完整 `src`、随包的统一 `vendor`、公开 `include` 与宿主工具，按已知发布输入写入版本清单和目标部署配置；为每个目标准备 OpenSSL 和所需 libunwind，由仓库准备脚本生成目标资源。
4. 设置宿主动态库相对加载路径，补齐安装说明、发行说明和实际组件许可证。
5. 生成宿主归档及同版本的 LLVM／libunwind vendor 源码归档，供手工测试或发布使用。

手工测试发布包时，维护者将归档解压到另一个正常目录，按第 11 节的适用场景执行验证；原始源码、工具、组装目录和产物保持存在。该流程独立于常规回归，不在每次代码变更或里程碑验证时自动执行。

临时构建、cache、组装目录、原始日志及逐次报告放在仓库已忽略的 `tmp` 或显式指定的独立工作目录中，避免默认占用 HOME 所在磁盘；不使用绝对路径 `/tmp`，不提交原始记录。归档不收集用户构建缓存、绝对路径构建记录、临时对象提取目录或工具探测记录；发行中的 runtime index 仅引用随包对象。失败不更新已安装版本，已有成功发布保持可用。

### 8.2 最小发行清单

`share/scoop/toolchain.toml` 是正常的安装配置／说明，schema 从 1 开始，保存发行版本、源码 revision、host、LLVM 精确版本、实际内置 Cone coordinate，以及每个目标的部署基线和 native 组件版本。路径由本设计的固定相对布局表达，只有真实使用的可变定位才保存为相对安装根的值。

清单不保存自产产物的签发者、权限、证明链或逐文件复核收据；既有 `.slib`／ABI 信息仍承担实际消费职责。命令从清单取得安装版本和目标部署配置，并用普通输入 metadata 记录其更新；依赖仍由 manifest 和实际产物定位，不因清单包含某个库而添加依赖，也不在每次启动时读取、哈希或验证安装目录的全部文件。

配套工具的 `--version` 报告发行版本和源码 revision，安装说明列出 host、targets、native 前置依赖及实际支持的系统版本。配套命令继续使用现有协议版本协商，不通过相同版本文本替代真实协议兼容性。

### 8.3 归档、升级和许可证

首版产物为 `scoop-<version>-<host-triple>.tar.xz`，包含一个完整版本目录。Darwin 包带一个目标，GNU 包带 GNU／musl 两个目标；不把目标列表与宿主可执行格式混为一谈。追加目标资源仍需同一发行配置，本期不交付在线组件安装器。

另提供共用于两类宿主包的 `scoop-<version>-llvm-sources.tar.xz`，保存发行所用 LLVM／libunwind 上游输入、补丁、版本说明和许可证，按第 2.4 节的 vendor 相对布局组织。普通构建应用及使用预编译 runtime 无需解压该源码归档；维护者准备宿主 LLVM 或目标 libunwind 时显式使用它。

用户解压到新版本目录并选择其 `bin`；原版本可继续使用。缓存沿原有工具／配置／产物输入记录更新，无需将整份用户缓存搬进安装目录或在升级时全部删除。安装目录默认不写入应用缓存、构建输出和测试运行文件；仓库准备脚本的目标输出另行选择。

许可证按最终实际分发的组件收集，包括 Scoop 自身、Rust 依赖、LLVM、Ryu、libuv、OpenSSL 和选中的 LLVM 传递依赖。源码包中已有的许可证保留，`share/licenses` 给出可定位的副本或索引。当前未找到项目根 LICENSE，正式对外分发前需由项目维护者确定 Scoop 的许可；本设计不替维护者选择许可证，也不阻止先完成本地包的构建和功能验收。

## 9. Spec 先行与跨文档同步

本次仅新增设计并登记路线图，不提前把尚未实现的命令或布局写成现行规范。实施按下表先同步合同，再修改对应实现与测试。

| 文档／位置 | 实施前同步事项 |
| --- | --- |
| 实现规范第 1 节、2.6～2.8、2.12、2.19 | 安装布局、预编译默认来源、显式 sysroot 构建、runtime index 相对路径、消费兼容条件、部署配置、默认 native 搜索和直接复用；只在实际格式变化时更新版本 |
| 语言规范 12.1～12.3、12.5～12.7、13～14 章相关引用 | 保持 Cone／single-file／显式依赖语义；在 12.2.1 补齐 native 源文件的普通相对父路径，更新公开头和源码位置，不新增 library 特权或 FFI 行为 |
| 运行时规范第 5、7 章与 runtime README | 预编译对象交付、统一 vendor 的 Ryu 构建输入、既有 unwind 选择和启动编排；不因打包改变 GC、异常或 runtime C ABI |
| 标准库、platform、asyncio、crypto、TLS 规范及 M38／M45／M49／M51 使用说明 | 更新统一 vendor、源码与目标资源路径、OpenSSL include／archive 和 libuv `native.sources` 定位及开发准备命令，保持已有公共 API 与库依赖分层 |
| AGENTS.md、fixture runner 说明与公共工具位置配置 | 更新开发 sysroot 准备及路径，明确打包脚本只供手工测试和发布，不加入常规验证；继续保持源码／工具存在、命中直接复用、验收日志放仓库外 |

旧说明中的 `sysroot/lib/<cone>`、各 Cone 的 `vendor`、`runtime/third_party`、`sysroot/native/<triple>` 和 `artifacts/<target-id>` 在其有效使用入口迁移；历史验收记录只修正必要的失效引用，不改写过去结果。M48／M50 已退役的安全复核和源码隔离要求不能作为新发布入口的验收前置。

## 10. 实施分批

| 批次 | 交付 | 必要验证 |
| --- | --- | --- |
| M52-1 发布配置与兼容条件 | 固定两类 host／三目标的实际系统基线、LLVM 补丁版本和构建选项；同步 target／ABI 与生产配置的区别 | 选定最低环境中的工具／依赖可用性；不同兼容 C 工具／SDK 消费同一预编译 runtime，不因完整版本文本拒绝 |
| M52-2 源码布局与安装定位 | 迁移十个 Cone 到 src，第三方源码集中到 vendor；接通 native 相对父路径和 runtime vendor 输入；统一工具、公开头及目标目录 | 显式覆盖、默认定位、含空格路径及普通 symlink；集中源码的实际 native 编译、目标筛选和真实缺失源码诊断 |
| M52-3 脚本目标准备 | 实现仓库准备脚本，调用普通 Cone 构建和 runtime 编译库，不扩展 `scoop` 命令与 driver | core→其余内置库的真实构建、重复准备直接复用、实际源码／头文件变化更新，失败不发布成功结果 |
| M52-4 预编译默认消费 | 应用默认使用目标 `.slib` 与 runtime；接通 link 的默认 runtime 和 native roots，修订索引相对路径 | 空缓存首次使用不编译内置 Cone／runtime；显式来源优先、真实缺失输入和目标不匹配；搬迁后链接运行 |
| M52-5 LLVM 与宿主工具包 | 构建并组装受控宿主 LLVM 依赖，设置相对加载路径，统一版本输出 | 手工测试工具包时，在声明的系统环境运行四个宿主工具和真实编译；没有开发机 LLVM 安装路径的运行依赖 |
| M52-6 手工打包脚本与说明 | 提供生成宿主／vendor 源码归档的脚本、最小清单、native 前置说明、许可证清单与安装／升级说明 | 手工运行时检查归档布局、cache／输出位置及组装失败保留旧安装；脚本不接入常规验证 |
| M52-7 三目标功能验收与手工测试说明 | 使用开发 sysroot 完成功能组合、冷暖复用与必要回归，提供发布包手工测试说明；记录范围及限制 | 第 11 节的常规功能覆盖闭合；打包与归档验证仅供手工测试和发布，不作为常规验收前置 |

同一批同时完成对应规范、实现和必要测试。先格式化与 lint 再运行测试；Python 使用 AGENTS.md 固定的 Ruff 版本。目录迁移涉及大量共享 helper 时集中处理，实际代码逻辑按上述职责分批，不为文件数量机械拆分实现。

## 11. 功能验证与发布包手工测试

常规验证使用开发工具和已准备的 sysroot，覆盖安装定位、预编译消费、目标兼容及缓存行为，不执行打包流程。以下涉及最终归档、宿主依赖组装、安装搬迁和升级的场景供手工测试发布包时使用，不加入默认测试集合，也不要求每轮回归先生成归档。

### 11.1 功能与发布矩阵

| 场景 | 需要观察的结果 |
| --- | --- |
| 空用户缓存的单文件 build／run | 使用已准备的 core 和 runtime，只编译用户程序及必要启动代码；正确传递参数与退出结果 |
| 显式使用 std／cli／json 的多 Cone 应用 | exact 依赖按当前目标定位，生产与测试依赖仍隔离，运行结果正确 |
| 泛型集合、接口、默认参数组合 | 消费依赖模板并生成本次实例；不重新编译上游 Cone |
| `scoop test` | 生产形式的 scoop.test 可供新项目使用；列表、过滤、实际执行和失败报告保持 |
| platform 与 asyncio | 从预编译 `.slib` 使用文件／线程／TCP 及事件循环；libuv 对象直接来自产物 |
| 统一 vendor 的显式构建 | asyncio 和 runtime 分别引用集中存放的 libuv／Ryu，OpenSSL 和 unwind 准备读取指定上游输入；源文件、头文件与实际目标选择正确 |
| crypto、同步 TLS、异步 TLS | 使用随包 OpenSSL 做内存密码操作及本机回环握手；严格沿 M51 的显式信任配置，不依赖公网或系统 OpenSSL |
| 原生 C／C++ | 使用安装 include 编译用户 C；Darwin／GNU C++ 正常链接，musl C++ 保持现有诊断 |
| debug／release 和必要 moving GC 组合 | 同一套 release 内置库在两种应用 profile 下运行；泛型、异常、FFI、异步保活等代表组合正确 |
| 目标分离 | GNU／musl 分别使用各自 runtime、OpenSSL 和 unwind；GNU PIE、musl 静态及已有动态模式保持 |
| 直接 `scoopc`、`scoop link` 与 `scoop-link` | 明确的依赖输入、安装默认 core／runtime／native 路径和现有独立命令可用；工具及源码均保留 |
| 发布包安装位置与升级（手工） | 同一归档解压到两个不同位置，其中一个含空格，均可运行；正常路径定位不依赖生产缓存；新版本使用自己的组件，旧安装保持可用 |

按实际相关范围复用既有 fixture 和库测试；新增少量集成 fixture 使用开发 sysroot 覆盖目录、默认选择及冷暖调用，不调用打包脚本，也不复制每个库的全部语义测试。手工测试发布包时，复用适用用例检查归档，并在最低支持系统执行代表性的编译、运行、GC／异常和 native 组合。

### 11.2 缓存与调用观察

所有场景保留源码、工具与原产物。使用现有命令记录或窄的进程调用观察确认：

- 空缓存第一次编译应用，不启动内置 Cone 的源码编译、runtime 源码编译、OpenSSL／libuv 的准备，也不下载依赖。
- 第二次输入完全未变时，不启动 scoopc、C 预处理／编译或最终 linker；不读取命中产物来复核内容，不重建 startup、复制执行文件或准备完整 Link 闭包。
- 修改用户源码、实际 native 头文件或链接配置后，只更新真实受影响部分。安装的 src／vendor 作为参考输入时不使只使用预编译产物的应用失效；显式源码构建仍正确观察其实际引用的 vendor 源文件和头文件变化。
- 新目标产物或 native 库未准备、真实必需工具不存在时给出普通缺失输入／工具错误。不会因缺失一个未使用的组件而先拒绝无关库编译。
- 手工测试发布包时，两个安装位置使用独立正常构建记录验证定位；不承诺同一份绝对路径本地缓存搬迁后仍零工作。旧缓存只按当前输入、配置与格式正常更新。

不通过删除、移动或隐藏源码、工具、原产物来制造后续成功场景；不改坏缓存、恢复时间戳或检查 owner／权限／inode。安装目录不被普通应用构建写入的要求，通过实际写入位置验证，不建立只读封存或权限认证机制。

### 11.3 验证范围与记录

Rust／Python 格式与 lint 按实际变更执行。共享 CLI、默认定位、索引和目标兼容改动完成后，运行相关 workspace 测试、fixture runner 规则测试及正式 CLI 完整回归；在 Darwin 和 GNU 宿主验证适用范围，并覆盖 musl 目标的既有链接模式。这些常规验证不执行打包、宿主 LLVM 分发构建或归档解压验收。打包脚本及发布包按需手工测试，仅修改打包说明后不重复无关全套测试。

原始日志、逐轮 JSON、归档和工作目录放仓库外；提交的验收文档只列发布配置、验证范围、结果概要、失败闭合及必要限制。记录当前真实计数，不预填历史里程碑的通过数量，也不以重复完整验证扩大完成条件。

## 12. 完成条件与后续范围

M52 完成时，提供可手工生成两类宿主归档的打包脚本和使用说明，覆盖三个现有目标，包含十个内置 Cone、runtime、OpenSSL、Linux unwind 和所需 LLVM 宿主依赖。用户在已声明的系统开发环境中，使用生成的归档即可完成正式 build／run／test／link；第一次空缓存构建也不重建内置 Cone 或 runtime。打包和归档验证不作为本期或后续常规验证的前置，需要手工测试或发布时再显式执行。

Scoop 源码、统一 vendor、目标产物、公开头和宿主工具采用同一可搬迁布局，第三方上游输入与目标生成文件分开；release 内置库可供 debug／release 应用使用，必要泛型实例化保持。系统基线经过真实验证，runtime 索引及动态依赖没有生产机器路径的运行依赖，M48／M50 的直接复用与职责清理没有倒退。

纯 musl 宿主工具包、新 CPU／OS target、完整 Clang／LLD 工具链、跨平台 SDK、在线安装／自动升级／组件下载、包仓库与版本求解、系统 CA／时区数据库和独立调试符号包均不作为本期前置。实际应用的 TLS 证书信任继续由 M51 的 API 处理，不借工具链发布改变其语义。

参考：[LLVM 分发构建](https://llvm.org/docs/BuildingADistribution.html)、[Clang 工具链组成](https://clang.llvm.org/docs/Toolchain.html)、[M48 缓存合同](../milestone48/DESIGN.md)、[M50 职责清理](../milestone50/DESIGN.md)、[M51 native 交付](../milestone51/DESIGN.md#5-native-构建链接与交付)。

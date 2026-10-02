# M23-10 设计：一般 native 输入与链接闭包

状态：设计完成，待实施（2026-10-02）。前置条件为已验收的 [M23-9](../stage9/ACCEPTANCE.md)；代码基线为 `afa3cebe5`。本文不表示实现或运行验收已经完成。

本阶段交付：在全新 `scoop-link` 进程中，仅凭完整 `.slib` 闭包、runtime 对象索引、明确 toolchain 和 `--library-path`，定位已有 FFI requirement 对应的 native object、archive、dylib/framework，完成符号解析、实际成员选择、最终绑定检查和可执行文件发布。库的 Scoop/native 源码在链接时均不需要。

权威合同为 [语言规范](../../specs/SCOOP-SPEC.md) 12.5、13.4、13.6、13.8、14.2～14.3，[实现规范](../../specs/SCOOP-IMPL-SPEC.md) 2.7～2.8，以及 [运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md) 2.8、3.5、4.2～4.3、5.5。本文细化并同步修订 [M23 总设计](../DESIGN.md) 3.7、5.5、9.4、10；旧 native 来源凭证、通用 effect handler 和没有生产需求的链接模式不再作为完成门。

## 1. 范围与实际基线

### 1.1 交付行为

```scoop
@Extern(lib = "m23_math", name = "m23_add")
fun nativeAdd(left: Int, right: Int): Int

fun main() {
    @Unsafe {
        println(nativeAdd(20, 22))
    }
}
```

上例编译后的 artifact 可以由下列正式入口链接；例如 `native/` 提供 `m23_math.o`、`libm23_math.a` 或 `libm23_math.dylib` 中的一种：

```sh
scoop-link --root-slib app.slib --dep-slib core.slib \
  --runtime-objects runtime/index.cbor \
  --library-path native --target aarch64-apple-darwin -o program
```

`--library-path` 可重复。库里的 extern 被第三个 Cone 直接调用、经 re-export 使用、在默认参数或泛型正文中使用时，仍经 Stage 9 的 provider-owned Scoop ABI 入口；native 合同不改标为 runtime intrinsic。源码可见全局数据、TLS、CLayout、函数指针与 callback 也必须沿真实产物路径运行，不能只用 `Int -> Int` 函数代表完成。

仍只有 Darwin/AArch64、一个主 executable 和静态完整 Cone 图。一般 dylib 是原生 FFI provider；它不包含新 Scoop image，不支持 `dlopen`/卸载或动态添加 Cone。C/C++ 源码由用户或 fixture runner 预先构建；program-link 只编译 Stage 9 的 startup C。umbrella CLI 总迁移、single-file 与历史 fixture 全面迁移归 M23-11；本阶段不实现 final-link cache。

### 1.2 已有能力与需要修改的位置

| 位置 | 当前事实 | 本阶段工作 |
| --- | --- | --- |
| [`identity/entity/native_link.rs`](../../../compiler/identity/src/entity/native_link.rs)、[`lir/production/native_requirements.rs`](../../../compiler/lir/src/production/native_requirements.rs) | 已有逻辑库、kind/grouping、完整 C/Scoop contract、GC effect 和 source origin | 直接消费，补必要查询方法；不重建一份合同 |
| [`mir-lower/pipeline/functions/native.rs`](../../../compiler/mir-lower/src/pipeline/functions/native.rs) | 参数自由 extern 已由 provider 发射普通 Scoop ABI 薄入口 | 保留原 native transition、caller roots 和 Strong owner |
| [`linker/program/native.rs`](../../../compiler/linker/src/program/native.rs) | 已合并全部声明；非空 library 在解析前被 Stage 9 供应边界拒绝 | 删除该阶段拒绝，接入实际 native 输入和同一解析器 |
| [`linker/native_object.rs`](../../../compiler/linker/src/native_object.rs) | runtime/startup 共用 Mach-O reader；保存定义与 undefined 集合 | 复用读取，保留选中 native 对象的物理引用、TLS 和必要格式事实 |
| [`toolchain/system_provider.rs`](../../../compiler/toolchain/src/system_provider.rs) | 固定 libSystem、SDK v4 stub 和 re-export closure | 提取共有 export/依赖读取；固定系统选择仍由 profile 提供 |
| [`linker/program.rs`](../../../compiler/linker/src/program.rs) | object origin 只有 Cone/runtime，dynamic 为 symbol set | 增加实际 native origin 与 provider binding，保留所有引用来源 |
| [`linker/link.rs`](../../../compiler/linker/src/link.rs)、[`link/map.rs`](../../../compiler/linker/src/link/map.rs) | 私有快照、明确 ld、固定对象顺序及原子发布已完成 | 加入选中的 native 成员、动态快照映射及完整 map/trace 对照 |
| [`final_image.rs`](../../../compiler/linker/src/final_image.rs)、[`final_image/commands.rs`](../../../compiler/linker/src/final_image/commands.rs) | 最终检查假定一个 libSystem、ordinal 1，拒绝所有 RPATH | 按实际 load-command/provider 表核对 ordinal、绑定和必要 RPATH |
| [`scoop/program_link.rs`](../../../compiler/scoop/src/program_link.rs)、[`linker/main.rs`](../../../compiler/linker/src/main.rs) | 已有库调用和独立 `scoop-link` | 两个入口传递同一 search-root 参数，不新增另一条 linker |

原对象、ODR、startup 和 runtime 检查继续复用。本阶段不创建 native producer 工厂、可注册 verifier 框架或并行 final verifier；也不因外部对象来自普通 C 编译器而要求来源授权。

### 1.3 设计期可行性核对

2026-10-02 使用当前 Xcode 的 clang/ld，在仓库外临时目录编译 C dylib、标准 v4 stub 和 C executable；clang 启用 `-Wall -Wextra -Werror`，链接使用 arm64、明确 SDK/deployment、`-no_implicit_dylibs`、`-no_fixup_chains` 与实际 RPATH。两个最小程序运行成功，并由 `nm -m`／`otool -L` 核对：

- A、B 均导出 `foo`/`bar`，投影后 `_foo` 绑定 libA、`_bar` 绑定 libB，分别返回 11/22；A 的 TLS 同时正确读出 31 并写为 32。
- C re-export B，C 的本次 stub 只列 `_foo`；最终 `_foo` 的 library owner 为 libC，executable 没有直接 libB load command，dyld 实际经 C 调到 B，返回 21。

临时文件已删除。这只确认第 5.3 节的目标工具方案可行，没有运行 Scoop Stage 10，也不替代第 9 节的真实产物、线程、GC 和完整回归验收。

## 2. 输入与库定位

### 2.1 请求边界

`ArtifactLinkRequest` 增加 `library_paths: Vec<PathBuf>`；库入口接收等价的显式搜索配置。`scoop::link_built_program` 传递该配置，M23-11 再将公开 CLI 全部接入。root、依赖、runtime index 与 output 的既有意义保持；`scoopc` 不接收 library path，不检查本机是否已经安装 native 库。

搜索只由已读 artifact 的 `NativeLinkRequirementId` 或实际 dynamic load/re-export 依赖触发。固定 runtime/system 输入继续由明确 profile 提供。没有 `--object`、`--archive`、`-l`、`-Wl`、linker script、自动 `.c` 发现或全目录符号扫描入口；`CC`、`LDFLAGS`、`LIBRARY_PATH`、`DYLD_*` 不向链接进程注入输入。

读取 Link closure 后先合并全部 native 声明，报告同 symbol 的合同差异，再定位 library。缺库和 ABI 冲突是不同错误；无调用的声明也不能逃过合同比较。每条 requirement 保留全部 source Cone/声明 id；显示位置取已有 source table，不为了诊断重新构建 HIR 世界。

### 2.2 明确的候选规则

令 `L` 为现有 `CanonicalNativeLibraryName`，`R` 为一个显式搜索根：

| requirement kind | 检查的相对路径 |
| --- | --- |
| `TargetDefault` | `L.o`、`libL.a`、`libL.dylib`、`libL.tbd`、`L.framework/L` |
| `StaticArchive` | `libL.a` |
| `Dynamic` | `libL.dylib`、`libL.tbd` |
| `Framework` | `L.framework/L` |

这些是完整拼接规则，不做 `lib` 前缀或后缀猜测、大小写折叠、递归 glob。`TargetDefault` 可以选择 direct object，因此无需给 source contract 增加 direct-object kind。framework 只读取实际 binary 和其装载依赖，不扫描 Headers/Resources，不提供 Objective-C/Swift 编译能力。常见 framework symlink 按实际文件解析。

检查全部搜索根。不存在的候选不产生输入；候选路径存在但不可读、损坏或格式与 kind 不符时报告该文件，不回退到另一个库掩盖错误。多个候选只有 target、格式、内容与装载合同相同才合并；`.tbd` 和 `.dylib` 即使 export 相同也不是相同输入，需由用户提供唯一候选。歧义列出全部不同候选和原 requirement，改变 root 顺序不能改变结果。

固定 libSystem 不通过这个目录搜索重新选取。一个非空逻辑库名也不能仅因固定系统 provider 中存在同名符号就成功；它须解析到实际 library。默认命名空间只查询已引入输入，包括 runtime、这些显式 library 及固定系统 exports，不以空 `lib` 自动引入任意库。

源编译器当前只产生 `TargetDefault + Independent`。已有 `OrderedGroup { name, position }` 按 group 的 UTF-8 bytes、position 排列，同一 group/position 的不同 requirement 报冲突；Independent 按 requirement id 排序。排序单位用显式 variant/tag 区分，再排列组内位置。同一完整 requirement 的多次来源合并；不同 requirement 指向同一物理文件时保留关联，物理对象只纳入一次。grouping 不提供 whole-archive/force-load 语义，也不改变 13.4 的 library 合同相等规则。

### 2.3 输入实体与引用

只引入实际用于索引、物化和诊断的数据：

| 数据 | 必需内容 |
| --- | --- |
| `NativeInputId` | target、实际输入 kind、原文件 length/digest；由 `scoop-native-input-v1` 域计算 |
| direct object | input id、不可变 bytes、选中 target slice、原 requirement 来源与对象事实 |
| archive member | parent input id、physical ordinal、header/payload offset、payload length/digest；不以 basename 作身份 |
| `NativeDynamicProviderId` | target、输入 id、该记录的 install name；由独立 `scoop-native-dynamic-provider-v1` 域计算 |
| object origin | 原 Cone/member、runtime object、startup，或 native direct/member 的封闭分支 |
| resolved reference | 原 object/use 位置、symbol，以及 `Definition(owner)` 或 `Dynamic(provider, export)` |

hash 使用现有 WireCborV1 和 framing；这两个新 ID 分别标识文件输入与动态 provider，不能互相转换。SDK 一个 stub 文件含多个 install-name record 时，provider ID 仍不同。相同 native 文件的多个逻辑库引用不会复制其全部符号表。路径只在 I/O/诊断旁持有；第 5 节实际写入 executable 的装载路径单独作为语义数据。

native object 的普通 undefined 不携带可恢复的完整函数类型。已有 SourceExtern/compiler ABI 引用保留原合同；普通 C helper 引用只保存 object/symbol/relocation 和可观察的格式事实，两种输入结构明确区分。不能为后者补假的 SourceExtern、空 ABI 或 runtime capability。

## 3. 普通 native 对象的检查

### 3.1 格式、定义与实际效果

复用 `NativeObjectInfo` 与 `object` crate。薄文件必须为 little-endian arm64 Mach-O；若输入有标准 universal wrapper，检查 slice 表范围后选择唯一兼容 arm64 slice，再按实际 `.o`、archive 或 dylib 解析。原文件内容及 slice 位置都进入输入记录；不接受重复匹配 slice、递归 wrapper 或把 x86/arm64e 对象当作 arm64 对象。

| 内容 | 本阶段处理 |
| --- | --- |
| target/deployment、section/symbol/string table、relocation | 检查真实边界、类型、对齐、引用及整数运算；复用同次解析结果 |
| ordinary code/data/rodata/BSS | 允许；定义关联实际对象、symbol 和 section，不重建 Scoop entity |
| C unwind | 允许实际 `__compact_unwind`/`__eh_frame`；检查结构和引用，保持已有 FFI 不跨 native frame 展开规则 |
| 静态／零初始化 TLS | 允许 Darwin TLV descriptor、template/BSS 和实际 bootstrap 引用；按 TLS kind 解析 |
| native constructor/destructor、动态 TLS initializer | 对实际纳入对象拒绝；不把它们接入 Scoop eager/lazy coordinator |
| autolink、embedded linker options、bitcode/LTO | 拒绝；不能产生额外未列入解析的输入 |
| Objective-C/Swift metadata、C++ ABI/personality 依赖 | 当前输入范围拒绝；不增加对应语言 runtime 或 handler |
| common/tentative definition | 对实际纳入对象报明确输入错误，要求库提供实际定义；不建立新的 common storage 合并器 |

`@Extern val` 是只读使用，允许绑定实际可写 native storage；`var` 必须绑定可写 storage。同 symbol 的 `val`/`var` 声明合同差异仍按 13.4 拒绝。TLS 与普通 data 不可互换；symbol 类型不可从名称或 `@ThreadLocal` 声明单方面推断。可取得的对象大小／对齐与 ABI storage 要一致，但 Mach-O symbol table 不可靠提供完整 C object extent 时，不能以“下一个 symbol 的距离”伪造完整布局证明。

单一 native weak definition 可以使用；两个已纳入对象提供同一 non-local definition 时报告冲突，不按代码 bytes 相同宣称 ODR，也不以 weak/strong 优先级消除冲突。只保留已有 Scoop ODR 的逐 member 合并规则。native direct/member 不得定义 C main、Scoop image/root、String alias 或占用已有 Scoop/runtime/generated bridge 定义；普通 native helper 的名称使用正常符号规则。

### 3.2 检查职责

外部 bytes 新读入时检查一次格式与实际引用。native object 不是 runtime-index record，也不是新的 `.slib` profile；共用底层 Mach-O 读取，按其实际用途执行必要检查。runtime/startup 的已有要求保持，不能为了支持普通 native input 放宽它们的定义或入口集合。

archive 索引只需要读取容器、成员格式、target 和定义符号；选中成员时从同一不可变记录完成余下对象检查。direct object 必然进入本次链接，立即完成检查。dynamic provider 读取的是装载/export 接口，不将整个 DSO 当成 relocatable object，亦不逐函数反汇编验证 FFI 作者的实现。

## 4. Archive 成员选择与静态闭包

### 4.1 普通归档与候选

使用 `object::read::archive::ArchiveFile` 读取 normal ar，包括实际 BSD/GNU long-name 和 symbol-table 形式；native archive 不要求 `.slib` 的 canonical header/member 命名。symbol table 的 offset 必须指向实际成员；候选定义以成员自身的 symbol table 为准，不仅信任 archive TOC。

拒绝 thin/external-member、nested archive、越界／重叠记录、未知 special member、普通成员不是支持对象以及损坏的名称／索引。成员名仅用于诊断，绝不拼接为提取路径；普通归档中含路径形状的成员名也不被用于打开外部文件。两个 `same.o` 或两个完全相同 bytes 的成员由 parent+ordinal+offset 区分。

不再要求“每个永远不被抽取的成员都先通过完整 EH/TLS/constructor/autolink 检查”。本设计不会把原 archive 交给系统 linker；只有已选对象才能影响输出，检查这些成员即可。未选成员仍必须能构成有效候选索引，不能借此接受损坏容器或 bitcode 作为普通 Mach-O。

### 4.2 单一解析器中的有限工作集

1. 合并所有 SourceExtern 合同与实际 compiler support 合同；为每个非空 library 解析唯一输入。所有声明检查候选是否能提供相应 symbol/kind，即使该声明没有机器引用。
2. 纳入全部 Cone/runtime/startup/direct-native 对象和原 alias；以它们的真实 undefined 引用建立工作集。声明表本身不制造 relocation 或强制抽取；已生成的 provider extern 薄入口当然会产生真实 native 引用。
3. 每次按稳定 symbol/origin 顺序处理未解析项。已有合法定义直接复用；带 `Requirement(id)` 的 extern 只能由对应 input 或其 re-export 提供，不能被其他输入的同名定义满足。默认命名空间／普通 native helper 先检查已纳入定义，否则在本次已有 library/provider 中选择；多个不同可用 provider 报歧义。
4. 选中的 provider 是 archive 时，按物理成员顺序取第一个能满足该 symbol 的候选。同一 archive 内的重复定义不在候选阶段冒充已链接冲突；如果两个成员因不同需求都被选入、随后形成重复定义，则正常报错。
5. 对新成员完成第 3 节检查，加入其全部定义与真实 undefined，记录触发选择的原引用。一个物理成员最多加入一次；跨 archive 的递归引用继续进入同一工作集。源合同与既有 compiler 合同相遇时仍按原完整字段比较。
6. 工作集耗尽即得到完整静态闭包；找不到定义、library 不符或冲突时，报告从原 Cone/native 成员到失败 symbol 的引用链。终止性来自有限输入与成员去重，不设逻辑步数、内存预算或计费规则。

本目标的归档规则允许反复查询已有 archive，循环引用不需要重复传库。`NativeLibraryGrouping` 只保留输入排序信息；没有生产字段表达的 `whole-archive`、`force-load`、手工 linker group 不加入本阶段。

### 4.3 交给系统 linker 的内容

选中成员的 bytes 作为独立 `.o` 写到本次私有目录，以 input id 和物理成员位置命名；direct object 同样使用 input id。最终输入顺序为原 startup、canonical Cone/member、runtime/object、按规范输入／物理成员顺序排列的 native static objects，随后为动态根。闭包求解时的发现顺序不成为最终顺序。

不创建第二份 archive、不运行 `ar/ranlib`、不让 ld 再抽取一次。plan 保存原 archive digest、完整成员索引和实际选择；map/trace 中的独立对象路径精确映射回原 member，解决同名成员无法从 `archive(name.o)` 唯一识别的问题。零抽取保存空 selection，不伪造 contribution；未选中定义和 undefined 不参与最终冲突或依赖检查。

## 5. Dynamic provider、绑定与运行时查找

### 5.1 实际 export 与依赖

支持普通 Mach-O dylib、已有 SDK v4 `.tbd` 和 framework binary。reader 从真实 load commands/export trie/可用 symbol table 取得 install name、版本、target/deployment、普通／TLS／weak export、re-export 及必要依赖。复用既有 SDK stub 和 Mach-O 读取代码，只补当前输入实际需要的结构；unknown stub version、未支持的 load-command 语义有具体诊断。

固定 libSystem 的 export 集合继续来自实际 SDK，进入同一 provider 模型；系统 stub 不代表运行机器 shared cache 的字节内容。一般 provider 不要求专用身份文件或发行者证明，摘要只记录本次链接看到的文件。DSO 内部普通实现不重做 ABI/GC 语义检查；其对外合同仍由 FFI 作者负责。

load edge 与 re-export edge 必须区分：A 普通依赖 B 不使 B 的符号成为 `lib = A` 的公开候选；A re-export B 则允许该绑定，同时记录外部 library owner A 和实际导出来源 B。显式 renamed re-export 保留名字映射。递归依赖／re-export 用现有输入 id 和访问集合求闭包，不因 native 图存在回边就套用 Cone DAG 禁环规则，也不无限递归。

相同 install name 对应不同内容或装载合同是冲突。不同 provider 的未使用同名 exports可以共存；实际显式 library binding 选择确定 provider，默认／无 library 归属的引用在多个可用 provider 之间报歧义。普通 `.tbd` 只区分 symbol/TLS，不能由其 `Symbol` 分支虚构 function/data/mutability 信息；有实际 Mach-O 可观察事实时才增加这些检查。

一般 provider 的必需依赖须闭合到本次明确输入或已选 SDK 系统目录；不可见的依赖、弱库可缺失／upward／flat-namespace／dynamic-lookup 等未提供的装载模式报输入错误。weak export 的属性须保留，不能自动变成 weak import，也不能通过 stub 去掉属性来掩盖需要跨 provider weak-lookup 的输入；无法形成确定 provider binding 的情形明确拒绝。对普通 native 库不添加 C++/Objective-C/Swift runtime；现有 M25 约束保持。SDK 系统接口按其已有平台 ABI 使用，不递归证明 OS 内部实现。

### 5.2 定位与装载位置

直接 provider 的 `LC_ID_DYLIB` 必须为绝对 install name 或 `@rpath/...`。一般依赖根据明确 roots、已选 provider 的 `@loader_path` 相对位置和所选 SDK 解析；不扫描进程环境、cwd 或未声明的通用系统搜索目录。绝对 install name 的一般库仍由明确 roots 找到相应文件并核对其装载名；它不是读取任意 host 文件的额外搜索入口。

`@rpath/suffix` 必须能由一个明确根 `R` 下的 `suffix` 定位到本次选中的 provider。framework 的 `Versions/...` 路径按同一规则解析。相同内容的多个位置先按规范化真实目录排序选定一致 locator；不同内容继续报歧义。把本次实际需要的根按路径 bytes 排序、去重后生成 `LC_RPATH`。这些目录必须在 executable 正常运行时仍存在，不指向临时链接快照。

依赖自己的 `@loader_path` 相对该库的实际位置解释；它不以链接临时目录为基准。自带 `LC_RPATH` 也按相同明确位置规则解析，并作为 provider 内容保存。直接 provider 的 `@loader_path`、`@executable_path`，以及依赖解析仍需猜测输出目录／环境的形式拒绝；本阶段不复制 dylib、修改 install name 或部署 native bundle。

因此，换 object/archive locator 且内容不变时 plan 不变；换一般 dylib 的实际嵌入 runpath 时 plan 必须改变。纯输入路径与装载路径是不同用途，不把后者从摘要中删除来维持虚假的路径无关性。运行机器上的 native 库仍须由使用者按这些装载名供应；此规则不建立运行时内容 pinning。

### 5.3 标准链接 stub 与系统 linker

Mach-O relocatable 的 undefined 只有 symbol bytes，没有 SourceExtern 的 library 字段。若 A、B 都导出 `foo` 和 `bar`，源码分别声明 `foo` 来自 A、`bar` 来自 B，两个合同本身完全合法；单一库排列无法同时保证这两个绑定。因此 provider 选择必须反映到 ld 的实际可见输入，不能只在结束后发现错绑再拒绝合法程序。

program-link 为每个直接 dynamic root 从已读 export 表生成本次专用的标准 `.tbd` v4 链接 stub，保留 target、真实 install name、current/compatibility version，只列本次解析给该 provider 的实际 import，包括普通／TLS／weak 属性。固定 libSystem 使用相同投影，包含 profile 明确的 linker-generated `dyld_stub_binder` 等实际需求。writer 复用已有 YAML 库及 v4 格式，不增加新的分发格式、source declaration 或 native producer capability。

re-export 已在输入图中解析；选给公开 owner A 的 symbol 以 A 的可见名字列在 A 的 stub 中，包括 renamed re-export。生成 stub 不再发出指向 B 的 re-export library 项，避免 ld 沿子库重新选择 owner；运行时仍加载原 A，dyld 沿原真实 re-export 找到 B。普通 load edge 不贡献可见符号。stub 只是本次已解析接口的投影，不改变任何原库或其 export，所有声明与候选的可用性仍针对未裁剪的真实输入检查。

只保留本次读取的必要 native 文件／slice、依赖和 SDK stub 内容，不复制目录树。动态原文件用于构造输入事实和内容摘要，ld 只接收由这些事实生成、在私有目录 create-new 写入的链接 stub；因此不会重新打开原 locator、隐式探测间接 dylib 或读取未对应的文件。direct/static 对象继续物化原 bytes。计划包含原 provider 内容、投影规则及生成的 stub bytes，临时文件名只作 I/O 映射。

系统 ld 显式采用 `-no_implicit_dylibs` 和原 two-level 模式，直接 dynamic roots 按稳定 provider 顺序加入命令。固定 libSystem 的 load command 保持；没有机器 import 的其他库可以不加入命令，其候选检查及依赖事实仍在 plan 中。所有 ld 读取文件必须对应本次对象、生成 stub 或已有明确 target 项；无法对应的 trace、额外 load command 或试图从外部目录再读库均报错。不增加 `-dylib_file` 等不再需要的间接输入路径。

### 5.4 最终绑定

`ProgramInputs.dynamic` 从 symbol set 改为实际 provider 关联；最终 verifier 从 load commands 建立 `ordinal -> provider` 表，解析当前 profile 的 classic bind/lazy-bind/rebase。每个普通或 TLS import 必须匹配 symbol、library owner、实际 export 及必要 addend；不再硬编码 libSystem/ordinal 1。

原 Scoop ODR 的 weak coalesce 保留原内部定义路径；一般动态 export 不能使一个实际要求绑定 Scoop/runtime/program/native-static 定义的引用改成 dyld import。显式 library 的符号也不能被一个同名 static definition 静默接管。保持 two-level namespace，拒绝没有明确 provider 的 flat/dynamic-lookup binding；检查实际 load command、版本与 RPATH，没有 import 的 export 不产生失败。

## 6. FFI、全局数据与 GC 的衔接

函数签名、默认参数、unsafe、NoGC 和 C-FFI-safe 在共同 HIR 完成；本阶段不在 linker 重做。C function/global/callback 继续使用 artifact 中已生成的 C storage bridge；Scoop ABI 沿原 canonical passing，不因 native 文件换为 archive/dylib 而改用 C aggregate ABI。

跨 Cone extern 函数继续使用原 provider 薄入口及 Managed Scoop callable effect。native callee 的 C leaf、Scoop Managed/NoGc 合同保持，C native-safe 和 Scoop native-borrowed、caller-root/result publication、epoch reload 不随物理供应方式改变。

全局数据和 TLS 沿原 native symbol contract、typed global identity 与 generated-C read/write/address unit 处理。依赖中的 `@Extern val/var` 必须支持直接读取、写入、复合赋值／更新和合法取址；导入或 re-export 不复制 native storage，也不以手写 getter 包装 fixture 来替代原属性验收。若现有跨 Cone 选择还有缺项，只补共同属性／bridge 路径，禁止另建 imported-native 属性模型。

实际读写／取址缺项由共有 place 表达外部属性引用：从既有 foundation source-native contract 按原 property id 查询，具体化为普通 extern global 并复用 read/write/address bridge。消费方 foundation 保留实际使用的原始合同与必要 ABI 类型；默认值引用表和泛型正文仍分发已有 global place selector，未增加分发字段。相等比较的上下文试探同时保留其新建类型与表达式，运行时求值顺序仍为左侧先于右侧，不能把另一份 arena 的 id 当作本地类型使用。

静态 C TLS 的初始 template、BSS、TLV descriptor 和 bootstrap 是 native 存储机制，不能变成 Scoop initialization unit 或 managed root。多线程 fixture 验证不同线程的 TLS 地址／值独立，普通 global 仍共享。外部 CLayout、指针与函数指针保持 C-safe；managed ref 或 ZST 跨 C ABI 继续在前端失败。

Scoop ABI shim 的 managed ref 借用、包含引用的大值参数／结果、ZST typed elision及需要 runtime 的 native-root 协议沿原实现。static function pointer、带 context 的 closure callback、同线程重入和 foreign-thread attach 必须经真实 provider 回调；异常在原 gateway 内物化，不能穿越 native C/Scoop FFI frame。新增 native 文件不增加 registered Scoop callable、safepoint 或 runtime image。

## 7. Plan、最终检查与发布

### 7.1 普通计划

在原 plan 上保存本次真正需要的内容：

- 原 root、canonical Cone Code/objects、runtime 对象、startup C/object、alias 和 target/toolchain；
- 原 native requirement 及全部来源、实际文件内容／target slice；
- archive 的物理成员索引、选择及触发引用，direct object 的实际贡献；
- provider install name、export/load/re-export 关系、实际 library binding、生成链接 stub 与运行时搜索路径；
- 实际对象、动态根及必要 options 的有序操作。

canonical plan 使用 `DomainSeparatedCborHash("scoop-resolved-link-plan-v2", plan)`。源位置、临时路径和命令中的快照 filename 有明确 presentation/physical 映射，不混入 canonical 操作；runtime install name/RPATH 则保留。对象内容、声明合同、ABI、archive 选择、provider export/依赖、工具或装载路径改变必须改变 plan。未选 archive 成员变化可以保守使 plan 失效；不为此建立逐字节增量计费或更多证明摘要。

plan、选择记录和 map/trace 是数据，不创建 `VerifiedNativeObjectSurface`、`ValidatedNativeContribution`、binding receipt、platform identity proof 或独立发布资格状态。正常路径不实现最终 binary cache；runtime build 与 Cone cache 继续按原职责复用。以后增加 final cache 才另验输出内容与实际 plan，不以本阶段的测试虚构 cache 已交付。

### 7.2 边界分工

| 边界 | 验证责任 |
| --- | --- |
| Compile／原 Link reader | 既有语言、完整 typed ABI、Scoop 对象、Strong/ODR 和原 relocation |
| native 读入／选择 | 候选格式、选中对象实际定义与引用、必要 EH/TLS、provider export/依赖及内容快照 |
| 统一 pre-link 解析 | 全部声明合同、实际未解析引用、native static 选择、provider 归属和符号冲突 |
| 系统 linker 后 | map/trace 实际对象、最终 symbol/address、dynamic ordinal/binding/load/RPATH、TLV 及保留段 |
| Stage 8 runtime | 实际加载地址／权限、image/registration、初始化初态、精确 root/PC 与线程／GC 契约 |

最终 verifier 扩展原实现，保留 image/root array、String 同址 alias、Strong/ODR 地址、完整 stackmap/EH、签名结构和平台检查。native static 的原 relocation 结合最终符号／fixup 关联目标，动态引用结合真实 ordinal 关联 provider；不能只检查 ld 返回 0、symbol 存在或程序某次能运行。

已验证的源合同、canonical ODR body 和对象内容不重新完整计算；也不把 runtime registry 搬到 final verifier。新对象与新绑定在其边界检查，没有来源凭证、通用资源预算或重复的语义重放。

### 7.3 错误与原子输出

沿原同文件系统临时输出、检查、同步和一次 rename 发布。缺库、冲突、对象损坏、闭包未完成、ld 失败、trace/final 检查失败均保留已有输出；library 或 source locator 后续变化不能替换本次已经快照的内容。链接成功后不会自动运行程序。

诊断至少包含失败阶段、原 Cone/声明或 native input/member、symbol、期望 library/provider，以及首个实际差异。archive 链给出触发成员选择的引用；dynamic 链给出父库、load/re-export 名字与缺失位置。全部 origin 在去重后仍保留；host 路径可以展示，不成为语言 identity。

## 8. 格式、迁移与删除

| 项目 | 决定 |
| --- | --- |
| `.slib` container、outer schema、persistent identity、mangler | 保持现有版本 |
| `NativeLinkRequirementKey`、SourceExtern/C/Scoop ABI、GC effect | 保持现有字段/tag；本机候选与选择不写回 IR |
| Stage 9 `link-support/1`、既有 production/import/ODR sections | 保持；直接消费实际记录 |
| runtime metadata ABI 3、runtime index schema 1、String/GC/初始化 | 保持 |
| plan | 改用 `scoop-resolved-link-plan-v2`，覆盖第 7 节真实输入 |
| final-link profile | `scoop-final-link-profile-v2`；原八字段保留，第 9 字段为 native 输入规则 revision `1`，实际 options 包含本阶段增加项 |
| native ID 与物理索引 | 只在 linker 输入／plan 中使用，不新增分发容器或 required capability |

这些新链接规则本身不要求重编译语义和 Code 均未变化的 `.slib`，也不使 runtime ABI 未变的对象缓存失效。现有产物的同次 Link 数据直接复用。若实施中真实属性／ABI 缺项要求改动 payload，必须先修订对应 spec 并明确升级那个实际 section，不能用空值、临时 fallback 或本段“保持版本”掩盖缺失。

本阶段删除／改写以下旧要求，而非换名保留：

- native producer 来源授权、`NativeProviderIdentityContract`/`PlatformIdentity`、通用 `NativeSemanticEffectPolicy` handler，以及每层 verification/contribution/binding 的证明 fingerprint；
- 未被任何生产调用使用的 native factory、完整二次 ABI verifier、Link-required blob handler 或 Cone 内 C/C++ producer 入口；
- 未选 archive 成员的完整语义效果拒绝、依赖 ld trace 唯一猜回同名原成员，以及每个 action 必须生成 evidence outcome 的通用事件层；
- 为尚无输入表达的 whole-archive/force-load、raw linker group/script 预造模式，以及 Stage 9 的非空 library 统一拒绝和固定 ordinal 假设。

`.slib` 的 unknown Link-required capability 仍报错；optional blob、diagnostic 和普通附件永不成为 object。本阶段现有 `LinkObject` producer 仍为 Scoop/generated-C，native 搜索输入另有实际 origin。无需为测试造一个新的 producer 或 handler 来证明“格式可扩展”。

## 9. Fixture 与验收

### 9.1 真实源码与产物

在 `tests/fixtures/m23-native-link/` 增加独立及组合源码，沿已有 `compiler/scoop/tests/program_link` 运行器调用真实配套 `scoopc` 和 `scoop-link`。native companion 用明确 C toolchain 预先编译；测试准备不能成为 production 自动发现 native 源码的入口。

每个源码功能保留 provider/facade/root 的 HIR/MIR/LIR golden，并生成完整 `.slib`。移动／移走 Scoop 和 native 源码后，下游继续只消费 artifact；独立链接进程没有源码、前端状态或 LLVM locator。结果断言来自 native 实现的返回、内存写入、地址或线程行为，不能只比较手工 metadata。

| 独立能力 | 真实验收 |
| --- | --- |
| direct object | 非空 lib 定位 `.o`，C 函数、只读／可写 global、取址读写，输出值来自 native 实现 |
| archive | 同成员多 symbol、成员链、跨 archive 回边、无额外抽取、多成员抽取、未引用成员不进入输出 |
| 同名成员 | 两个相同 basename、相同 bytes 不同 ordinal，按真实成员选择并在 plan/map 精确区分；已选重复定义拒绝 |
| dylib | 普通 export、global/TLS、绝对 install name 与 `@rpath`，去掉链接快照后仍按正式装载位置运行；A/B 重名 export 下 `foo -> A, bar -> B` 两个选择同时生效 |
| re-export/framework | C 接口 framework、普通依赖与 re-export 的区别、改名 re-export，最终 ordinal 关联声明库 |
| TLS | 静态初值／零值、读写／取址、两个 foreign threads 的 TLS 独立与普通 global 共享 |
| C ABI | 定宽整数、嵌套／packed/aligned CLayout、指针、nullable/code pointer 与 C 调用的实际布局 |
| Scoop ABI | Managed/NoGc 两种 callee contract、ZST、间接大值、managed ref 参数／结果和 native roots |
| callback | 已有静态 `FunPtr`、带 context 的 closure、同线程重入、foreign-thread attach 与实际 callback failure |

组合至少覆盖：provider 声明 extern → facade 泛型／默认值或 re-export → root 本地类型／callback；同一 native 库由多个 Cone 使用；四 Cone 的 archive+dylib+ODR/eager/lazy 混合。适用场景分别普通运行与 `SCOOP_GC_STRESS_MOVE=1`，其中移动 GC 场景必须实际跨 FFI 或 callback 保留 live managed 引用。TLS native 线程在退出前 join/detach，不能依赖进程退出掩盖 runtime 生命周期错误。

archive 选择函数还应直接测试空需求、递归去重、同名索引和未选成员；不为了制造零 selection 新增语言语法、blob handler 或发布器。实际源码用例必须验证未引用 member 未被加入。Stage 9 的 core/sysroot、String alias、多 object、ODR member 并集、初始化失败／cycle、异常与原子发布回归继续保留。

### 9.2 Negative、损坏与确定性

| 错误类别 | 必需覆盖 |
| --- | --- |
| 定位 | 缺 library、多个不同内容、`.tbd`/dylib 冲突、错误 kind/target/slice、损坏或不可读候选、空 lib 不扫描 roots |
| 合同 | 同 symbol 的 library、C/Scoop ABI、GC effect、参数／结果、function/data/TLS/mutability 冲突，包含未调用声明及全部 origin |
| 真正缺定义 | native helper 缺 provider、显式库不提供 symbol 却被其他库提供、archive 成员引用链未闭合 |
| 对象／archive | thin/nested、坏 header/TOC/range、bitcode、选中 autolink/constructor/dynamic TLS/C++ 依赖、common、已选 weak/strong 冲突 |
| 未选成员 | 合法索引但包含未选 autolink/constructor 的成员不进入 ld，也不误触发完整效果拒绝；坏容器仍失败 |
| dynamic | 缺依赖、普通依赖冒充 re-export、同 install name 不同内容、默认命名空间歧义、错误 TLS export、无法解释的装载路径、生成 stub 多／少／错 owner 的 import |
| final | 额外／重复／遗漏 map object，额外 trace 文件、错 ordinal/provider/版本/RPATH、static 定义被动态接管、保留段丢失 |
| 输入一致性 | 读入后替换原文件或 symlink，实际 linker 仍消费已读 bytes；输出路径已有文件时各失败均保持原内容 |

source 错误断言位置和具体信息；artifact/native/final 错误断言其真实边界及来源，不虚构不存在的源码位置。损坏对象和伪造 final 只用于定点边界测试，不代替真实正例。对不存在完整类型的外部实现，不写“机器码证明 ABI 正确”的测试。

plan golden 覆盖库需求来源合并、路径别名、archive member ordinal、provider/re-export、实际 RPATH 和有序命令。交换依赖、搜索根、目录枚举顺序后解析结果保持；只改 native bytes、export/load contract 或运行时目录必须改变 plan。Cone/native 源码均不可用的重复链接须得到相同 plan，符合既有 target 确定性要求。

### 9.3 实施顺序与完成门

1. **direct object 闭环。** 接入 request/search roots 与现有合同解析；复用普通对象 reader，先让真实非空 lib 的 `.o` 完成独立进程运行。
2. **archive 闭包。** 完成真实成员索引、有限选择、选中对象检查与独立物化，验收成员链、回边、同名及冲突。
3. **dynamic 与部署语义。** 共用 provider reader，接通 export/re-export、load 依赖、标准链接 stub 与实际 RPATH；扩展 final ordinal/load/binding。
4. **数据、TLS 与 FFI 组合。** 沿共有属性／bridge 路径完成跨 Cone 读写取址、CLayout、Scoop ABI、callbacks、线程及 moving GC。
5. **计划、诊断与收尾。** 完成 plan/profile 版本迁移、map/trace/最终引用核对、原子输出与确定性；删除被替代拒绝和旧证明框架要求。

每批实现先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再构建实际配套 `scoopc`/`scoop-link` 并运行相关 fixture；最终关闭所有 snapshot 更新开关，启用 `SCOOP_TEST_PAIRED_SCOOPC`、`SCOOP_TEST_PAIRED_SCOOP_LINK`，执行完整 workspace 回归。单元、格式或 mock linker 通过不等于产物验收。

完成必须同时满足：三类 native 输入在正式独立链接入口运行；所有真实引用、合同、成员选择和 dynamic binding 闭合；源码／本机前端不参与链接；FFI/GC/TLS/异常组合和 negative/golden 通过；未增加 runtime ABI、来源凭证、通用预算或假想 handler；最终检查与原子发布完成。实施后单独记录 `ACCEPTANCE.md` 的实际提交、命令、结果和剩余 Stage 11 工作。

M23-11 只需把本阶段同一搜索配置与库入口接入正式 `scoop build/run/link`，迁移历史 single-file/FFI fixture，并删除旧 source/core 拼接和直链旁路；不能另实现 native resolver 或把 Stage 10 的供应能力留给 fixture 专用注入。

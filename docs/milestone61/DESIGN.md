# M61 设计：Rust 文本后端与宽松许可静态分发

状态：已完成（2026-10-22）。实现与验证结果见 [完成记录](PROGRESS.md)，字符集范围见 [迁移对照](CHARSET-COVERAGE.md)。

本期以纯 Rust 后端替换 `scoop.charset`、`scoop.idna` 引入的 GNU libiconv、libcharset、libunistring 和 libidn2，使应用静态链接这些文本能力时不再承担这条 LGPL 依赖链的再链接材料等分发义务。允许 IDNA 改用成熟库的 UTS #46 行为，在规范与库文档中明确其与原严格 IDNA2008 profile 的差异；字符集集合按 2026-10-22 用户决定收敛到成熟 Rust 库能够覆盖的编码，严格／有损、BOM 与增量合同继续保留。

现行合同已写入对应 spec；IDNA、NFC、字符集及 readline Unicode 基础操作均已迁入同一 Rust 后端。下文保留实现边界和选型依据。

依据：[语言规范](../specs/SCOOP-SPEC.md) §11.4、§12.2、§13，[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) §3.5、§4、§5.5，[实现规范](../specs/SCOOP-IMPL-SPEC.md) §1、§2.7～2.8，[字符集规范](../specs/SCOOP-CHARSET-SPEC.md)、[IDNA 规范](../specs/SCOOP-IDNA-SPEC.md)、[M52 分发设计](../milestone52/DESIGN.md)与 [M54 设计](../milestone54/DESIGN.md)。

## 1. 目标与范围

交付同一套 Rust 实现、三个目标的静态库，以及现有 Scoop API 到该静态库的普通 C ABI 适配：

- `aarch64-apple-darwin`；
- `x86_64-unknown-linux-gnu`；
- `x86_64-unknown-linux-musl`。

`scoop.charset` 继续负责名称解析、内存及增量字符集转换和完整文本 NFC；`scoop.idna` 继续负责域名转换、有效性检查和原始 Punycode。URL、HTTP、TLS 复用它们的结果，不增加独立字符转换、Unicode 校验或 IDNA 例外表。

Rust 是后端维护与预编译时使用的实现语言。普通应用消费 `.slib` 和目标 `.a`，无需执行 Cargo 或安装 Rust。编译器仍使用既有 native library 与 C ABI 通道，不新增 Rust stage、Rust 源码自动构建机制、后端插件或运行时选择框架。

本期只处理这条文本依赖链，不改变平台系统库的既有链接规则，也不把 Cargo 依赖许可清单视为所有目标本机运行库的完整许可结论。

## 2. 原有依赖与选型

### 2.1 原有依赖

| 位置 | 原后端 | 实际职责 |
| --- | --- | --- |
| `scoop.charset` | libiconv 1.19 | 名称、编码、解码及有状态转换 |
| `scoop.charset` | libunistring 1.4.2 | 普通文本 NFC |
| `scoop.idna` | libidn2 2.3.8、libunistring 1.4.2、libiconv 1.19 | IDNA、Punycode 及其 Unicode 支持 |
| GNU 后端准备 | libcharset | libiconv 附带的 locale 编码辅助能力 |

迁移前两个 Cone 没有单独声明链接 `libcharset.a`，相关辅助代码也存在于其他 GNU 归档中。替换整条后端依赖后，这项能力不需要另找库：Scoop 的公开文本接口不依赖进程 locale 自动选择编码。

原库部分选用 LGPL；这不等于静态链接应用必须采用 GPL，但分发时存在对应源码、修改说明、允许替换库后重新链接等义务。本期通过更换实现减少这项分发负担，不通过改写原库许可证或引入额外授权机制解决。

### 2.2 选定的 Rust 组合

| 包 | 设计基线 | 用途 | 上游许可 |
| --- | --- | --- | --- |
| `encoding_rs` | 0.8.42 | 常用遗留编码与增量转换 | `(Apache-2.0 OR MIT) AND BSD-3-Clause` |
| `idna` | 1.1.0 | UTS #46 与原始 Punycode | `MIT OR Apache-2.0` |
| `icu_normalizer` | 2.3.0 | NFC，与 IDNA 共用规范化实现和数据 | `Unicode-3.0` |
| `idna_adapter` | 1.2.2 | `idna` 到 ICU4X 的上游适配 | `Apache-2.0 OR MIT` |
| `icu_properties` | 2.3.0 | IDNA 字符属性及 readline Alnum | `Unicode-3.0` |
| `icu_segmenter`、`icu_casemap` | 2.3.0 | readline 扩展字素与完整大小写 | `Unicode-3.0` |
| `unicode-width` | 0.2.2 | readline 单标量列宽 | `MIT OR Apache-2.0` |

上表组件及必要的 locale 类型使用同一份 `Cargo.lock` 锁定。`idna_adapter` 已使用 ICU4X normalizer 和 properties，普通 NFC 直接复用 `icu_normalizer`；不另外加入 `unicode-normalization` 或 utf8proc 形成第二套规范化数据。ICU4X 为 Rust 实现，与需要 C++ 配套的 ICU4C 区分。

`encoding_rs` 0.8.42 和本期使用的 ICU4X 2.3 组件声明的最低 Rust 版本均为 1.88；正式准备工具链的版本还须满足锁定的完整依赖。试验使用 Rust 1.99.0，不能据此把 1.99.0 宣称为最低版本。发行准备记录实际工具链版本，普通应用不探测该版本。

现成的 C ABI 包 `encoding_c`、`icu_capi` 可供接口实现参考，但本期直接编写承接 Scoop 现有 native 函数的小包装。不会为了几个入口引入 ICU4X 的全部 C API、默认日志组件或其他国际化功能。许可与署名随实际使用的代码和数据保留。

## 3. 静态库、ABI 与库边界

### 3.1 单个共享归档

在 `sysroot/native/text_backend/` 放置独立的内部 Cargo package，包含 `Cargo.toml`、`Cargo.lock` 和按字符集、规范化、域名、Punycode 划分的 Rust 模块。该 package 不进入 compiler 的 stage workspace，也不成为新的公开 Scoop Cone。

Cargo 配置的核心形态为：

```toml
[lib]
name = "scoop_text_backend"
crate-type = ["staticlib"]

[dependencies]
encoding_rs = "=0.8.42"
idna = "=1.1.0"
icu_normalizer = { version = "=2.3.0", default-features = false, features = ["compiled_data"] }

[profile.release]
panic = "abort"
lto = true
codegen-units = 1
```

每目标输出 `sysroot/lib/<triple>/libscoop_text_backend.a`；实际 C 适配需要的头文件放入该目标的 `include`。两个 Cone 均以现有 `native.libraries` 声明静态库 `scoop_text_backend`，最终链接复用现有闭包去重规则。一个归档统一包含所需 Rust 依赖与共享 Unicode 数据，不分别为每个依赖生成会重复携带 Rust 支持代码的应用归档。

Rust LTO 在准备静态库时完成，交付普通目标机器对象。Scoop 的 LLVM codegen 与本机 linker 消费该归档，不要求它们读取 Rust bitcode，也不要求 Rust 工具链内部 LLVM 与 Scoop 的外部 LLVM 22.1 完全相同。

### 3.2 C ABI 与生命周期

尽量承接现有 `scoop_charset_*`、`scoop_idna_*` native 入口。导出使用 `extern "C"` 与显式符号名，跨边界结构使用 `#[repr(C)]`、固定宽度整数和指针／长度；Rust 的 `String`、`Vec`、引用、trait object 和默认 enum 布局不进入 C ABI。

Scoop 的 `Long` 计数与 Rust `usize` 在真实边界转换，检查负值、溢出和实际缓冲区长度。零长度输入不依赖可解引用的指针。UTF-8 String 仍允许内嵌 NUL，接口使用显式长度，不退回 C 字符串语义。

一次性调用借用输入仅限当前同步 FFI 步骤，返回独立结果；Rust 分配的结果和转换器状态由同一包装库的释放入口回收，Scoop 复制成 managed String／Array 后及时释放。增量状态跨步骤保存普通 native 字节与编码器状态，不保存未固定的 managed 指针。`close` 幂等，错误后的状态只可关闭，沿用既有借用与 GC 边界。

不增加线程、I/O、异步执行器或 managed 回调。Rust 实现语言本身不改变现有 FFI transition，也不构成新增 `@GCLeaf` 标注的理由。

### 3.3 错误、panic 与分配失败

普通格式、编码、参数和输出空间不足通过结果状态报告，不使用 panic 作为可恢复错误通道。`panic = "abort"` 防止 Rust panic 展开越过 C ABI；它不表示 Rust 内部的分配失败已经被转换为 Scoop 异常。

能显式检测的长度溢出、缓冲区错误及可恢复分配失败按现有 native 错误合同返回。现行 IDNA／NFC spec 明确：上游内部不可恢复 OOM／panic 终止进程，不保证转换为 Backend 异常；显式返回的参数、长度或后端失败仍按状态传播。普通格式错误不得使用 fatal 路径。不为捕获所有 OOM 引入通用分配预算或异常代理框架。

## 4. IDNA 的目标行为

### 4.1 Profile

采用 `idna` 的 UTS #46 non-transitional 实现，并显式选择：

| 参数 | 选择 | 效果 |
| --- | --- | --- |
| ASCII 规则 | `AsciiDenyList::STD3` | 保留 DNS 主机名的 ASCII 字符限制 |
| 连字符 | `Hyphens::Check` | 检查首尾及规定位置的连字符 |
| DNS 长度 | `DnsLength::VerifyAllowRootDot` | 检查 A-label 长度，允许末尾根点 |
| Bidi | 上游始终检查 | 继续拒绝不符合双向文字规则的域名 |
| ContextJ | 上游始终检查 | 继续验证连接控制字符的上下文 |
| Transitional | 关闭 | 保留非过渡处理，例如 `ß` 不映射为 `ss` |

不直接使用会允许更多 ASCII 字符的缺省便利函数，也不将库名或函数名中的 strict 等同于完整 IDNA2008。上游 UTS #46 接受的部分符号与 ContextO 差异属于本期明确允许的行为调整，不另写严格 IDNA2008 字符表或 ContextO 验证器。

### 4.2 与 M54 的差异

| 输入或能力 | M54 合同／实现 | M61 决定 |
| --- | --- | --- |
| `☃.example` 等 UTS #46 允许的符号域名 | 按严格 IDNA2008 拒绝 | 接受，转换为 A-label；不保证 DNS 可解析 |
| `a·b.cat` 等 ContextO 违规名称 | 拒绝 | 采用上游行为，接受；不声明执行 ContextO |
| `faß.de`、NFC 等价输入 | 非过渡处理与规范化 | 保留 |
| ASCII 下划线、控制字符、内嵌 NUL、空中间标签 | 拒绝 | 保留拒绝 |
| 非法连字符、伪 A-label、Bidi／ContextJ 违规 | 拒绝 | 保留相应检查，不要求原 GNU 数值错误码 |
| 合法名称的末尾根点 | 允许并保留 | 保留 |
| 独立根域 `.` 与单个 Unicode 分隔符的等价输入 | 完整域名入口允许，URL HTTP host 另行拒绝 | 保留现有局部边界规则；不要求后端把根域当普通标签验证 |
| 详细后端原因 | libidn2 提供多个数值错误码 | 上游有效性失败不保证细分原因，按第 4.3 节表达 |
| IDNA 数据版本 | 固定声明 15.1.0 | 记录选定 ICU4X 数据的实际 Unicode 版本，不沿用旧常量 |

符号域名和 ContextO 差异已在静态库试验中确认。其余保留项仍须完成正式 Scoop 回归；不能把本表当作这些入口已经迁移的证据。

DNS wire name 不超过 255 字节、标签不超过 63 字节，仍以转换后的 ASCII／A-label 计算。单标签入口仍拒绝标签分隔符；完整域名入口对独立根域的处理是原有 profile 边界，不扩展成另一套 Unicode 映射实现。

### 4.3 公共 API 与诊断

保留 `Idna.parse`、`toAscii`、`toUnicode`、OrNone、单标签转换以及 `IdnaName` 的使用方式。`toUnicode` 仍验证输入，不将后端用于展示错误的 U+FFFD 标记当作成功域名返回。

`parse` 复用一次处理产生的 ASCII 与 Unicode 结果；可使用 `Uts46::process` 的两个输出 sink，并在取得 ASCII 后检查本次实际需要的 DNS 长度。单一表示入口使用同一 profile。不要为生成两个视图串联两次完整 IDNA 处理，也不要为恢复旧错误分类逐标签重新验证。

保留 `IdnaException` 的操作名、类别、可选 code、detail 和 labelIndex。无法细分的上游有效性失败统一使用 `InvalidLabel`，文档将其定义扩展为通用 IDNA 有效性错误；`code`、`labelIndex` 没有实际值时为 None。只有包装层直接掌握的事实才提供更细类别和位置，例如最终 ASCII 的长度错误。`Bidi`、`Context` 等现有类别不再承诺由新后端逐项报告；不根据错误字符串或虚构数字模拟 GNU 错误码。

OrNone 仅转换正常输入无效结果，参数和后端运行故障保持既有传播原则。`backendVersion` 记录包装层、`idna` 和 ICU4X 基线；`unicodeVersion` 来自固定数据的实际版本。ICU4X 包版本不是 Unicode 数据版本，二者分别记录。

### 4.4 原始 Punycode

使用 `idna::punycode::encode_str`、`decode_to_string`，保留原始 RFC 3492 codec 与域名验证的分离。空串、纯 ASCII 编码的末尾分隔符、大小写、内嵌 NUL 与标点按现有合同处理；无效整数序列、非 Unicode 标量及算术溢出作为格式错误报告。

退役 GNU 后端的 raw Punycode 补丁，不复制其实现或另写 Bootstring 算法。域名有效性放宽不使原始 codec 接受非法编码，也不把 DNS 标签长度限制施加到原始文本编码入口。

## 5. 字符集与 NFC

### 5.1 必须保留的基础编码

仍交付字符集规范列明的 14 种编码：UTF-8、US-ASCII、ISO-8859-1、Windows-1252、UTF-16LE／BE、UTF-32LE／BE、GB18030、GBK、Big5、Shift_JIS、EUC-JP、ISO-2022-JP。UTF-16／32 无字节序名称和既有显式别名继续遵守现有选项合同。

| 能力 | 实施方式与兼容要求 |
| --- | --- |
| 遗留多字节编码和状态编码 | 复用 `encoding_rs` 的 decoder／encoder；核对与原固定映射的差异 |
| US-ASCII | 明确按 7-bit 字符集处理，高位字节／不可表示字符不能被当成 Windows-1252 |
| ISO-8859-1 | 使用 `encoding_rs::mem` 的 Latin-1 操作或等价的标量转换，保留 U+0080～U+009F 的本义 |
| UTF-16 字节输出 | 使用 Rust core 的 UTF-16 转换及显式字节序；不能使用会选择 UTF-8 输出的 WHATWG encoder 便利行为 |
| UTF-32 | 使用标准库 Unicode 标量转换与显式字节序，检查代理值和超出 U+10FFFF 的值 |
| NFC | 使用 `icu_normalizer::ComposingNormalizerBorrowed::new_nfc()`，与 IDNA 共用 ICU4X 数据 |

这些适配只覆盖实际 Unicode 字节序、标量与基础单字节规则，不自行实现另一套 CJK 或 ISO-2022 编码算法。

### 5.2 名称、覆盖与映射差异

不能将 `Encoding::for_label` 的 WHATWG 别名规则原样作为 `Charset.forName`：它会把 ASCII／ISO-8859-1 等名称解释为 Windows-1252，并具有与当前名称合同不同的空白处理。名称解析继续大小写无关，使用明确的 canonical name 与别名；未知名称失败，不回退 locale，不接受 `//TRANSLIT`／`//IGNORE` 等选项后缀。

M54 的 ENCODINGS.md 列出 108 个 canonical name；M61 的 [新集合](../../sysroot/src/scoop.charset/ENCODINGS.md) 保留 43 个。`encoding_rs` 并非 GNU libiconv 全集的等价实现。[覆盖核对](CHARSET-COVERAGE.md) 已逐项列出原集合。2026-10-22 用户明确选择“保留纯 Rust，仅支持成熟 Rust 库能覆盖的编码”；不再要求保留全部 108 种。实施时在 charset spec 与 ENCODINGS.md 明确新的集合、移除的名称、保留别名及实际映射版本，不让未实现名称继续查找成功。

GB18030 的版本对照已在编码列表明确：旧名称对应 2005，另列 2022；新名称采用 WHATWG 2022 变体并移除年份别名，记录 19 个 PUA 的非往返行为。GBK、Big5、Shift_JIS、EUC-JP、ISO-2022-JP 的标准／厂商扩展与错误恢复以固定上游实现及其文档为准。

实际映射差异先写入 charset spec，再迁移到成熟 Rust 后端。不存在的历史编码明确返回 UnsupportedCharsetException；不保留隐藏 GNU fallback、不自行补写 CJK／ISO-2022 算法、不搭建后端插件框架。测试检查 Scoop 的名称表与适配合同，不逐项复核上游映射表。

### 5.3 严格、有损与增量状态

默认严格入口拒绝非法序列和不可表示字符；显式 lossy 解码产生 U+FFFD，编码产生同一目标编码器编码出的 `?`。使用上游无隐式替换的低层接口取得实际错误，不能把 HTML 数字字符引用当作 Scoop 的编码替代策略。

`CharsetTranscoder` 保留源 decoder、目标 encoder、跨块残留与有限的中间输出，沿用 `consumed`、`produced`、NeedInput／NeedOutput／Finished 和输入结束语义。替换字符也经过当前编码器，保持 ISO-2022-JP 等状态编码的 shift 状态；不在上层跳过任意错误字节后继续假定状态有效。

UTF-16／32 的跨块单元、代理项和 BOM 由实际适配状态保存。Consume／Preserve／Require、反向 BOM 冲突、显式与缺省字节序、`emitBom` 及流中间 U+FEFF 的规则保持。暂时没有输入不表示 EOF，输出空间不足不表示格式错误，严格截断错误在明确结束时报告。

错误偏移仍对应原始输入流，不因分块或中间 UTF 转换而重置。底层接口未直接提供的信息应从本次转换的实际消费位置推导，不能扫描完整输入重放转换来定位错误。失败后只可 close，一次性转换只在完整成功时交付结果。

### 5.4 NFC

`normalizeNfc` 继续接收完整 String，保持规范分解／重排／组合与 Hangul 行为，保留 NUL 和 U+FEFF，不做兼容折叠、大小写转换或 BOM 消费。返回结果具有独立生命周期；不按任意流块分别规范化。HTTP Basic／Digest 的 UTF-8 凭据继续经该公共入口规范化，不新增协议专用实现。

## 6. 构建、目标链接与分发

准备脚本显式构建固定 Cargo package 和锁定依赖，为三个目标生成预编译资源。源代码、锁文件、许可和修改说明按已有 sysroot vendor／发行布局交付；Cargo 的普通锁定与依赖准备足够，不建立产物收据、来源授权或签名链。

更新判断沿用现有维护脚本规则：输入未变化且所需输出存在时直接复用，判断先于源码读取、解包、复制、Cargo／Rust 工具探测及链接准备。只在确需重建时执行对应工作，命中时不读取 `.a` 验证内容或生产过程。失败不发布部分资源。

每个目标的 CPU 基线、Darwin 最低部署版本及 GNU／musl 区分必须与现有发行配置一致。三个目标不能混用归档；共享相同源码和版本不等于机器产物可跨目标使用。

Rust `staticlib` 仍可能需要目标系统库。准备与链接验证使用 `rustc --print=native-static-libs` 获取实际需求，并通过现有 native 链接配置承载；该探测不进入普通应用的缓存命中路径。GNU／musl 尤其要核对 Rust 支持代码与 Scoop 既有 Level I unwind provider 的关系，不能叠加冲突的 provider，也不能因使用 Rust 而隐式开启 C++ 模式。

2026-11-02 的实际 GUI／HTTP 闭包同时包含 Scoop、Rust、C++ 和 Objective-C personality，超过 macOS compact unwind 可编码的数量。文本后端并不使用 Rust 展开，现将包装收敛为 `no_std + alloc`，关闭 idna 的默认 std feature，保留现有算法、数据和 panic=abort。系统分配器薄适配与直接 abort 的 panic handler 足以支持已有 String／Box／VecDeque，不增加另一套堆或 panic 运行时，也不切换 Scoop 最终链接器。新资源的实际消费者验证见 [UI 实施记录](../ui/PROGRESS.md)，其他平台不沿用旧归档的验收结论。

迁移完成后，准备脚本与两个 Cone 不再要求原四个 GNU 文本归档；发行包移除仅供这条链使用的源码、补丁与资源，并保留仍适用的第三方通知。不能只替换 NFC 而让 libidn2 继续把原链带入应用。其他后端的许可与资源不随本期改动处理。

## 7. 已有可行性证据

2026-10-22 在仓库外构建了一个试验 Cargo package，将 encoding_rs、idna、icu_normalizer 编入同一 release 静态库，使用 Apple Clang 编译的纯 C 消费者调用。结果确认 C aggregate 返回、同库释放和 Darwin 系统库链接可行；Cargo 依赖许可为 MIT、Apache-2.0、BSD、Unicode，无 GPL／LGPL。

该试验仅用于选型；正式 Scoop 适配、目标构建与运行结果见 [进度](PROGRESS.md)。不将上游 Unicode 算法核对或官方一致性向量作为本项目验收。

## 8. 验收计划

测试只验证本次实现的 Scoop 适配、库功能与目标资源交付，使用 scoop.test，不新增 fixture。按用户要求，不为编译器、链接器或其他底层缺口增加正例、负例，也不增加 Rust 库 Unicode 兼容性或完备性测试；不随附官方一致性数据。

| 范围 | 必须覆盖 |
| --- | --- |
| Rust／C ABI | 真实 C 消费者、布局与长度、空输入和 NUL、所有权与释放、普通失败状态；三个目标实际链接运行 |
| IDNA／Punycode／NFC | Scoop profile 边界与参数适配、状态映射、输入借用和输出复制／释放；复用现有公共 API 用例 |
| 字符集 | Scoop 名称表、退役名称、选项、BOM 与字节序适配 |
| 增量 | 任意合法分块、极短输入／输出、跨块错误与绝对偏移、明确 EOF、输出背压、严格／替换策略、finish／close 与失败清理 |
| Unicode 适配与 readline | UTF-8 字节边界转 Scoop 标量索引、长度与所有权；实际编辑、历史记录、补全与 PTY 行为 |
| Scoop 消费者 | 复用已完成的 charset／idna 与直接消费者结果；不扩展或重跑 HTTP client 组合测试 |
| 资源生命周期 | 复用库内已有的独立调用、gcCollect 与 close 清理用例，不增加 runtime 缺口专测 |
| 分发与复用 | 三目标预编译资源与复用、两个正式链接入口、真实消费、普通并发准备；Darwin 发行资源组装与许可收集 |

原严格 IDNA2008 与 UTS #46 的差异在规范中说明，上游算法与数据正确性由上游负责；不把 Bidi、ContextJ、ContextO、NFC、分段、折叠或编码映射的规则核对设为本项目验收项。

链接专测检查实际使用的目标归档和必要本机依赖，确认原 GNU 文本链已经退出；这些检查只用于定向验收，不进入消费者 reader 或缓存命中的内容复核。复用测试保持源码、工具与原产物存在，以真实调用和输出验证结果。

实现阶段按修改范围先格式化、lint，再运行 Rust 包检查、Scoop 库测试、正式 CLI 消费与组合测试。只有真实代码变更才执行对应构建测试；原设计试验不能替代正式验收。

## 9. 实施顺序与文档同步

1. 完成字符集覆盖对照、精确 Unicode 数据版本、上游分配失败行为和 Linux 本机链接需求的核对，形成可执行的迁移边界。
2. 按已确认的范围先修订 IDNA／charset spec；涉及实际 FFI 故障或目标链接合同的变更，同步对应运行时／实现 spec 及其引用。把允许的 IDNA 差异和未放宽的字符集要求分别写清。
3. 实现内部 Rust package、固定依赖与 C ABI，先完成三个目标的原生消费者；复用既有目标资源准备入口，不增加普通应用构建时的 Cargo 步骤。
4. 迁移两个 Scoop Cone、native 依赖与错误映射，完成独立测试及直接受影响的消费者回归，移除对应 GNU 使用与补丁需求。
5. 更新预编译资源准备、发行许可和使用文档，运行三目标正式消费者、链接及缓存复用验收，最后记录完成状态。

对应文档已按实际实现同步：

| 文档／配置 | 同步内容 |
| --- | --- |
| `docs/specs/SCOOP-IDNA-SPEC.md` | Rust 选型、UTS #46 profile、差异、诊断、版本与分发 |
| `docs/specs/SCOOP-CHARSET-SPEC.md` | Rust 转换与 NFC、实际支持集合、名称和失败合同 |
| 三份主 spec 中实际受影响条款 | 只同步确有变化的 native ABI／链接／失败职责及交叉引用 |
| `sysroot/src/scoop.idna/README.md`、公开注释 | 新后端与明确的兼容行为 |
| `sysroot/src/scoop.charset/README.md`、`ENCODINGS.md`、公开注释 | 实际支持集合、映射版本、BOM、严格及 lossy 语义 |
| `scripts/README.md`、`scripts/release.toml`、分发文档 | 固定 Rust 依赖、维护构建、目标资源与适用许可 |
| `docs/milestone54/DESIGN.md`、`docs/ROADMAP.md` | 标明后端由 M61 迁移，保留 M54 当时的实际验收边界 |

## 10. 完成情况

原 GNU 文本依赖链已退出目标应用链接；IDNA、Punycode、NFC、43 种字符集及 readline Unicode 基础操作均使用固定 Rust 后端。profile 差异、退役名称、错误与 OOM／panic 边界已经写入规范。

三个目标完成资源准备、Scoop 适配、两个正式链接入口和实际运行；复用及 Darwin 发行资源组装通过。发行携带锁定依赖和实际 Rust 标准库通知。验收范围按用户要求收敛，不新增底层补缺或上游 Unicode 兼容性／完备性测试；具体结果与发行验证边界见 [完成记录](PROGRESS.md)。

## 11. 上游依据

- [encoding_rs](https://github.com/hsivonen/encoding_rs)、[0.8.42 API](https://docs.rs/encoding_rs/0.8.42/encoding_rs/)：字符集、增量接口、WHATWG 名称与输出编码差异。
- [encoding_c](https://github.com/hsivonen/encoding_c)：现成 C ABI、头文件和 panic 边界说明。
- [idna 1.1.0](https://docs.rs/idna/1.1.0/idna/)、[UTS #46 API](https://docs.rs/idna/1.1.0/idna/uts46/index.html)、[上游 README](https://github.com/servo/rust-url/blob/main/idna/README.md)：检查参数、严格 IDNA2008 与详细错误分类的范围。
- [ICU4X 2.3.0](https://github.com/unicode-org/icu4x/releases/tag/icu%402.3.0)、[icu_normalizer](https://docs.rs/icu_normalizer/2.3.0/icu_normalizer/)、[许可](https://github.com/unicode-org/icu4x/blob/main/LICENSE)：Rust Unicode 后端与数据。
- [icu_capi](https://github.com/unicode-org/icu4x/tree/main/ffi/capi)：可选的官方 C ABI，本文不引入其完整默认组件。
- [Rust linkage](https://doc.rust-lang.org/reference/linkage.html)、[rustc 输出信息](https://doc.rust-lang.org/rustc/command-line-arguments.html#--print-print-compiler-information)：staticlib 与目标本机链接需求。
- [UTS #46](https://www.unicode.org/reports/tr46/)、[UAX #15](https://www.unicode.org/reports/tr15/)、[RFC 3492](https://www.rfc-editor.org/rfc/rfc3492.html)：IDNA 兼容处理、规范化与原始 Punycode。

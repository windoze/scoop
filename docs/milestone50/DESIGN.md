# M50 设计：清理普通编译器职责之外的校验与安全保障

状态：实施中；逐批进度与验证见 [实施记录](PROGRESS.md)。创建日期：2026-10-16；更新日期：2026-10-17。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[编译器与产物规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[路线图](../ROADMAP.md)与 [AGENTS.md](../../AGENTS.md)。M50 继续清理 [M48](../milestone48/DESIGN.md) 后仍存在的额外职责，覆盖整个项目及其历史测试，不以某个旧里程碑已完成为保留理由。各批实施前先同步对应 spec，再修改实现与关联测试。

编译器自身的冷／暖构建、源码变化构建和调用栈调查见 [性能调研](PERFORMANCE.md)。其中重复输出复核和整包重编码属于本设计的清理范围；Lowerer 复制、LLVM 对象粒度、依赖按需读取、并行和更细增量等普通优化保留为候选方向，不自动增加 M50 的交付门槛。

## 1. 核心决定

**所有不属于一个“普通编译器”职责的额外校验、证明和安全性保障全部清理掉，包括其关联的单元测试、集成测试、fixture、golden、测试辅助代码及文档要求。下面的已知问题是清理起点，不是范围上限。**

普通编译器的职责以 GCC 的源码编译、对象生成和链接，以及 Cargo 的依赖、构建编排和输入变化判断为参照：根据用户选定的源代码、依赖、目标和配置完成产出，需要更新时执行实际工作，输入未变化时复用已有结果。这里采用的是职责边界，不要求复制 GCC 或 Cargo 的内部实现。

1. **自身和 C/C++ 编译器成功产生的输出按正确结果使用。** 不把 AST/HIR/MIR/LIR、生成的 C、对象、`.slib` 或 executable 当作需要重新认证的安全对象；不为证明前一步生成正确而重新生成预期产物、完整重放语义／布局、比较全部字节或建立验证资格链。
2. **缓存有效性只判断输入是否改变。** 源码、依赖、实际工具、目标、有效配置与编译选项都是输入。记录不存在、请求的输出不存在或缓存格式已变时，完成相应生产工作；输出存在只表示结果可用，不触发输出内容校验。输入未变且所需结果存在时直接复用。
3. **防篡改、来源证明、生产凭证、授权和防伪均不属于职责。** 删除这类检查及其数据、状态、错误分支和证明包装；也不检查已经生成的内容是否被其他程序改坏，不承担缓存损坏检测、修复、隔离或恢复。
4. **不审计文件系统或保护构建输入。** 不核对 owner、权限、inode、可信根、私有目录或 symlink 来源，不封存只读文件，不在执行或读取前后证明文件身份稳定，不建立源码／工具隔离保证。实际文件选择、读写、进程执行和失败报告照常完成。
5. **不接管目标工具链的对象安全审查。** native 对象按正常链接输入处理；不以段名、来源语言、编译器版本字符串或安全属性黑名单拒绝目标工具链可以处理的产出，不重新实现其完整 relocation、unwind、TLS 或装载验证器。
6. **清理覆盖全部入口。** 冷构建、暖构建、对象首次生产、发布、依赖消费、最终链接、run/test、调试观察和 runtime 都遵守同一边界。不能只删除缓存命中时的检查，再在其他入口重新执行。
7. **不保留替代安全路径。** 不把退役机制转移到严格模式、另一套 reader、隐藏选项、仅供测试的生产 API 或 runtime；不为清理工作新建凭证、通用资源预算、插件或审计框架。旧 spec、design、所谓冻结条款和现有测试都不能构成例外。

## 2. 正常正确性工作的边界

正常的语言和运行契约继续成立。保留一项检查的依据必须是它直接服务当前编译、链接或执行所需的具体操作，不能仅以“更安全”“防止伪造”“此前检查过所以再确认一次”为理由。

| 工作 | M50 后的职责 |
| --- | --- |
| 源码编译 | parser/HIR 完成语法、名称、类型、可见性、泛型、effect 等语言规则；MIR/LIR/codegen 完成当前转换及实际 ABI/GC 数据生成。后续阶段使用已有完整结果，不再独立重演上游语义。 |
| 数据读取 | 按需读取实际需要的字段、section、符号和索引；对本次读取的长度、偏移、引用及计算溢出进行必要的局部处理。不能借格式读取遍历未使用内容、重编码比较整个产物或证明其来源。 |
| 对象与链接 | 提取实际定义和引用，选择依赖与 archive member，完成 Scoop 所需的符号、ODR、ABI 和元数据连接，将 native 链接交给所选工具链；实际目标不兼容、未定义或冲突符号正常报错。 |
| 工具与 I/O | 正常发现和调用所需工具、传递参数、使用退出状态、读取必需输出、报告实际失败；临时文件按正常生命周期管理，新结果成功后发布，失败不发布部分结果。 |
| 缓存 | 先按记录判断输入变化，再读取源码内容、预处理、探测工具或准备链接；使用既有结果的位置与必要摘要，不读回产物来验证其正确性、完整性或生产过程。 |
| runtime/GC/FFI | 保留语言运行语义、实际内存访问边界、根与 relocation、写屏障、线程协调及调用 ABI 所需的操作和检查。不能以 GC/ABI 名义复核未变化的编译产物或补做来源、权限防篡改和内容证明。 |

语义身份、ODR 选择和实际链接确实需要的摘要可以保留；仅服务于“这个结果可信”“它由指定步骤产生”“它与另一份证明相符”的摘要和交叉比对必须删除。文件 metadata 只用于观察普通输入变化，不防御修改内容后恢复时间戳等外部行为。依赖产物作为本次构建输入时，其正常更新参与失效判断；不因此审计它自身的生成过程。

代码生成确实需要从机器对象提取 stackmap、地址、符号或补丁位置时，正常提取一次并复用数据；不能将这项读取扩大为对编译器产出的全量复验。必要的编译器正确性回归测试可以验证 ABI、GC 和生成结果，但不把这种测试要求变成每次生产和消费产物都执行的认证流程。

## 3. 已确认的问题与处理方向

以下位置以 2026-10-16 的仓库调研为依据。实施时沿实际调用继续清理相关路径、持久字段、错误类型和测试辅助代码；仅删除列出的函数或只改名称不算完成。

### 3.1 临时文件的只读封存和权限断言

实现入口：[C bridge emission](../../compiler/codegen/src/c_bridge_emission.rs) 的 `seal_regular_file`，以及 [LLVM emission](../../compiler/codegen/src/emission/prepare.rs) 的 `verify_and_seal_object`。当前生成的 C 文件在编译前被设为只读，对象在生成后被设为只读，设置失败会导致编译失败。

删除封存、权限审计及其成功条件。生成、读取和清理工作文件，以及实际工具和 I/O 错误继续按正常流程处理。可执行输出所需的执行权限、程序实际需要的数据布局不属于只读防篡改封存。

关联测试：[c_bridge_objects.rs](../../compiler/codegen/src/tests/c_bridge_objects.rs) 中的 `generated_c_object_set_preserves_unit_authority_and_immutable_backing`，以及 [object_partition.rs](../../compiler/codegen/src/tests/object_partition.rs) 中对象必须 `readonly()` 的断言。删除这些权限／封存断言及只服务它们的辅助逻辑；保留真实编译输入输出和临时文件生命周期覆盖。

### 3.2 发布阶段的生产过程证明链

正常 [artifact assemble](../../compiler/driver/src/artifact_production/layout.rs) 会进入 [layout object verification](../../compiler/driver/src/object_production/layout/verification.rs)，构造和传递多层 `Verified...`。其中 旧 C bridge production（现为 [generated C objects](../../compiler/slib/src/link_object/generated_c_objects.rs)） 再次核对 profile ID、编译器 profile 指纹、源码模板指纹和编译参数指纹，将自己构造的生产记录当作需要证明的事实。

删除这类生产凭证核对、仅用于验证资格转换的包装和深比较，以及已无真实调用者的平行证明流水线。实际输出数据直接传递；需要的定义、引用、stackmap 和补丁位置以普通完整数据保存，不要求附带“已通过前一道认证”的对象。

关联测试：旧 `c_bridge_production/tests.rs` 的 `rejects_manifest_profile_even_when_object_deployment_would_validate`。该用例仅改变记录中的 Clang 版本指纹，就要求原本目标条件可以通过的对象被拒绝，必须移除。其他专门构造生产凭证不一致、验证资格缺失或不可达内部状态以维持证明链的测试一并清理；真实工具失败、目标和实际 ABI 测试保留。

### 3.3 跨产物的源码元数据来源核对

实现入口：[source_provenance.rs](../../compiler/slib/src/cross_cone_closure/source_provenance.rs) 的 `validate_imported_source_metadata`，由 [layout closure foundations](../../compiler/slib/src/layout_compile_closure/foundations.rs) 在依赖读取中调用。它遍历复制的 source record 和 source context，查找 provider 并要求两边记录完全相同。

删除为诊断资料建立的跨产物来源证明。SourceLocation、源码记录和 context 继续服务诊断与实际引用，不为证明“原样复制自正确 provider”再遍历比对整套资料。实际需要的字段、偏移和引用在读取边界处理，实体归属和语言访问性继续使用普通符号与类型规则。

关联测试：同文件的 `rejects_dependency_source_metadata_that_differs_from_its_provider` 通过把 `"abc"` 改为 `"abd"` 构造 `forged_record`，断言 `SourceRecordMismatch`；这类真实性断言及关联 helper 一并删除。跨 Cone 诊断位置和必要格式读取继续测试。

### 3.4 native 对象黑名单和重复的目标格式验证器

实现入口：[native_object.rs](../../compiler/linker/src/native_object.rs)、[selected.rs](../../compiler/linker/src/native_object/selected.rs)、[references.rs](../../compiler/linker/src/native_object/references.rs)、[sections.rs](../../compiler/linker/src/native_object/sections.rs)、[elf.rs](../../compiler/linker/src/native_object/elf.rs) 和 [elf_dynamic.rs](../../compiler/linker/src/native_input/elf_dynamic.rs)。

当前额外职责包括：按 `__objc_*`、`__swift*`、`__LLVM` 等段名拒绝输入；对 load command 使用封闭白名单并禁止嵌入的正常链接要求；拒绝 common/tentative definition；按 C++ 符号前缀施加拒绝；因可执行栈标志拒绝 ELF 对象或 DSO；逐段重解码 relocation 并自行检查 EH frame、compact unwind、TLV bootstrap、初始 key 和模板布局。

清理这些额外拒绝和仅返回“验证通过”的遍历。未知段名、来源语言、存在 LTO 辅助段或目标格式的正常初始化记录本身都不是 Scoop 拒绝输入的依据；是否需要某项真实工具链能力、运行库或链接输入，由实际构建与链接要求处理。当前工具确实无法完成某种输入时报告实际失败，不为尚无的能力建立安全禁令或新验证器。

Scoop 只保留自己选择对象、解析实际定义／引用和构造必要数据所需的格式读取及局部边界处理。普通 common、强／弱符号、C/C++ 运行库和 native 初始化采用目标工具链的正常规则；不能因某个内部索引尚未表示一种正常对象形态而把它永久定义为非法。新源语言入口或新 target 属于独立的真实能力工作，不以补写其元数据安全检查作为前置。

关联测试：旧 `formats-relocations` fixture（M50 已删除） 用固定字节偏移破坏对象，并要求 Scoop 输出自身的 relocation、load command、unwind 细节诊断；删除仅维持上述重复验证器的 case、golden 和 patch helper。对 Scoop 必须解析的范围保留少量直接格式测试，实际链接失败继续验证失败传播和不发布输出。

[ELF native 单元测试](../../compiler/linker/src/native_object/elf/tests.rs) 中以 `int tentative;` 和 `-fcommon` 断言必须拒绝的要求应改为普通 common 输入的链接行为。已有 common fixture 如果同时把数据符号提供给函数声明，函数／数据种类冲突仍是独立的合法 negative case，不能机械地把所有同名测试改成成功。

### 3.5 C 与 C++ 工具完整版本号必须相等

实现入口：[cxx/discovery.rs](../../compiler/toolchain/src/cxx/discovery.rs) 同时比较 `-dumpmachine` 和 `-dumpfullversion`，要求 C/C++ driver 返回完全相同的字符串。

删除仅凭完整版本号不同就拒绝的条件。目标兼容和实际编译／链接能力由正常工具选择与调用完成；版本与有效配置可以参与输入变化记录，不构成来源资格或配对认证。目标名称的不同拼写也不能代替实际目标兼容性判断。

关联测试：[version-mismatch fixture](../../tests/fixtures/m33-native-cxx/negative/version-mismatch/fixture.toml) 通过 wrapper 只改变 `-dumpfullversion` 的输出、其余操作仍转发真实工具，却要求构建失败。删除这一拒绝要求及只服务它的 wrapper/golden；保留工具缺失、实际目标不兼容、编译失败和链接依赖缺失的覆盖。

### 3.6 include 搜索目录必须预先存在

实现入口：[native/paths.rs](../../compiler/toolchain/src/native/paths.rs) 的 `validate_includes` 在预处理前对所有搜索目录执行 `canonicalize` 和存在性检查。

删除搜索目录必须存在的前置条件，把 include 参数交给 C/C++ 编译器按正常查找规则使用。实际源文件、被引用的头文件和显式 `-include` 文件读取失败仍按工具诊断处理。为增量构建观察影响头文件选择的输入变化不要求先把所有搜索目录判为有效实体。

关联测试：[include-missing fixture](../../tests/fixtures/m33-native-c/negative/include-missing/fixture.toml) 的 C 源码没有 `#include`，只是配置不存在的搜索目录，仍断言 `SourceSnapshot` 失败。该拒绝要求改为正常编译成功。调研时使用同一份 `native.c` 和缺失目录运行本机 `cc -fsyntax-only -I ...`，退出码为 0，stdout/stderr 均为空；该观察未生成对象文件。

### 3.7 仍要求拒绝普通 C constructor 的 fixture

关联入口：[direct-contracts](../../tests/fixtures/m23-cli-native-link/direct-contracts/fixture.toml) 和 [archive-conflicts](../../tests/fixtures/m23-cli-native-link/archive-conflicts/fixture.toml) 的 `reject_constructor`，以及后者 Darwin、GNU、musl 的诊断 golden。它们要求因为 `__mod_init_func` 或 `.init_array` 拒绝含空 `__attribute__((constructor))` 的普通 C 对象。

当前实现已移除这一拒绝，现行实现规范也允许目标 CRT 支持的 C 初始化／析构记录。将这些 case 改为正常链接与运行，并用可观察状态验证初始化发生；清理失效的拒绝诊断。不得为了让旧测试通过而恢复初始化段黑名单、增加 manifest 授权开关或设计 native 初始化来源证明。

### 3.8 自产 LLVM 输出复核与整包重编码比较

性能采样补充确认了两条现行路径：[prepare.rs](../../compiler/codegen/src/emission/prepare.rs) 在变换前后完整验证自产 LLVM module，并调用 [statepoint verifier](../../compiler/codegen/src/statepoint/verifier.rs) 复核工具链结果与全量函数策略；[decode_canonical](../../compiler/wire/src/cbor/decode.rs) 在完整解码后重编码整份记录，再比较全部字节。

按第 1、2 节清理仅用于证明前一步产出正确的复核和整包往返比较，不将同一套机制移到严格模式或另一条生产入口。LLVM 变换、statepoint／stackmap 的实际生成与提取，以及读取当次字段所需的局部格式和引用处理继续承担正常职责。已有输入或输出数据未变化时复用结果，不为取得下一阶段的验证资格重复遍历。

关联测试按第 4 节同步清理：移除只要求生产路径再次拒绝内部伪造产物或往返比较不一致的断言与专属 helper；保留直接检查实际 IR 变换、语言错误、格式读取及 ABI／GC 生成结果的必要回归。清理依据是职责边界，不以某个调用栈占比大小决定是否保留；采样百分比也不能直接当作可获得的提速比例。

## 4. 关联测试与历史要求必须一起清理

清理覆盖 `tests/fixtures/`、Rust 单元／集成测试、runtime 测试、公共 fixture runner/helper、golden，以及所有历史和活动设计中的相同要求。测试的位置、旧里程碑名称或是否通过不能决定其必要性。

- 完全以防篡改、来源证明、生产凭证、文件封存、权限／路径审计、缓存破坏恢复或源码／工具隔离为目标的用例，删除用例及专属 helper、输入、golden、错误类型和入口。
- 同时覆盖真实功能与额外机制的用例，保留真实编译、链接、诊断和运行行为，移除多余断言；需要时把被错误拒绝的合法程序改为 positive case。
- 正常语言功能 fixture 以语言诊断和运行结果为主；不重复保存整套 AST/HIR/MIR/LIR、符号表或生产证明快照。少量阶段转换和 ABI/GC/优化专测只锁定实际合同所需的结果。
- ABI、GC、对象读写的必要局部边界测试可以保留；仅为了维持一个已经退役的全量验证器而拼造对象、损坏字节或核对精确内部拒绝阶段的测试删除。不能把原生产验证器整体搬到测试目录后继续要求维护。
- 缓存测试保持源码与工具存在，通过真实调用和输出验证未变化输入的复用及变化输入的更新。删除改坏缓存、恢复时间戳、替换文件身份、移走输入／工具后仍要求成功等验收；保留缺失输入报错、缺失所需输出的正常生成、失败不发布和普通并发协调。
- 不要求目标 C/C++ 工具链的错误在 Scoop 中提前复刻，不因诊断由真正执行该操作的工具给出而判定回归。Scoop 自身的语言错误仍验证必要的位置与信息。
- 删除后不保留跳过、忽略、按 fixture 名特判、旧兼容 reader 或仅供测试调用的凭证状态机。没有实际使用者的数据与分支随关联机制一起删除。

本轮调研解析了 3,514 个 fixture 声明，并针对命中路径检查实现和测试，未运行 Scoop 全套验收。目录或测试名含 `escape`、`isolate`、`verify` 不自动代表仍有问题；已经改成正常输入路径、复用或语言规则测试的用例按实际行为保留。第 3 节的静态发现不冒充已执行的失败记录。

## 5. 规范同步与实施顺序

本设计记录已确定的清理方向，不表示相应代码已完成修改。实施时先修订相应规范中的反向要求，不能用旧规范保留本设计决定删除的职责。

| 文档 | 实施前同步事项 |
| --- | --- |
| 实现规范第 1 节、2.5～2.8 与相关产物／链接章节 | 明确自身和 C/C++ 成功产出的使用假设、按输入变化更新、普通按需读取；删除只读封存、生产证明链、native 黑名单和完整格式复验要求。 |
| 语言规范中源码、Cone、native 构建和 FFI 章节 | 保持语言语义及真实 ABI 要求；去掉 include 目录存在性、无实际兼容性依据的工具版本配对和来源授权等限制。 |
| 运行时规范 | 只保留实际执行、GC、内存和调用合同，不在启动或热点补做被移除的产物证明；同一不变元数据的结果按实际需要复用。 |
| AGENTS.md、fixture runner 说明、历史／活动里程碑设计 | 统一普通编译器职责与测试边界，删除要求安全认证、缓存破坏恢复、权限封存和来源隔离的验收条款。 |

实施按实际代码边界分批完成：

1. 同步总原则和涉及的 spec，逐项确认第 3 节实现及关联测试；沿正常调用继续发现同类职责，覆盖 compiler、toolchain、linker、artifact reader/writer、缓存与 runtime。
2. 每批同时简化实现和清理对应测试、错误分支、证明字段、辅助代码及旧文档要求。发布和缓存状态使用普通数据；仅当实际持久格式改变时更新格式版本并重建旧缓存，不增加证明迁移或恢复层。
3. 使用真实源码完成编译、产物消费、native 链接和运行回归；在现有完整目标上验证冷构建、直接复用和实际输入变化。新增正向组合覆盖应直接验证被恢复的正常能力，不为审计本身另建框架。

代码实施遵守仓库的格式化、lint 和测试顺序。Rust/C 及正式 CLI 回归按变更范围执行；涉及 native 目标行为时覆盖 Darwin、GNU、musl 的适用路径。最终验收记录只保留范围、结果概要、失败闭合与必要限制，原始日志和逐次报告放在仓库外。

## 6. 完成条件

- 不再存在普通编译器职责以外的校验、证明、安全性保障及其专属测试；第 3 节全部处理，实施中发现的同类问题也按同一原则清理。
- 自身和 C/C++ 编译器成功产出的对象、归档和程序直接参与后续必要工作；没有文件封存、生产凭证复核、内容防篡改或验证资格转换。
- 输入未变化且所需输出存在时直接复用；实际输入变化、输入缺失、输出缺失和真实工具失败均走普通构建流程，不检查或恢复外部改坏的缓存。
- native 对象不因段名、common、初始化记录、版本字符串或安全属性黑名单被额外拒绝；实际不兼容与链接故障由相应普通功能报告。
- 源码语义、类型与引用、真实 ABI/GC/FFI、对象实际读取边界和运行结果继续正确，现有使用者的源码编译、跨 Cone 消费、native 链接与执行闭环通过。
- 关联 fixture、单元测试、golden、helper、遗留 API 和文档同步完成；没有严格模式、隐藏分支或测试专用证明链承接已退役职责。

完成情况由上述真实功能和代码清理确认，不建立新的安全证明、来源认证或验收收据体系。

# M48：历史隔离与防御性测试清理

状态：实施清单，尚未执行删除或迁移。日期：2026-10-15。原基线：`e548b9ebe`，已在 M46 完成后的 `6571bb4a` 工作区复扫。本文件配套 [M48 主设计](DESIGN.md)，范围覆盖整个仓库，不限于 M23 或名称包含 `artifact-only` 的测试。

## 1. 清理规则

以下测试目的全部退役：先编译 provider／core／root，再删除、移走或隐藏其源码，以证明下游、独立链接器或 runtime 只依靠 `.slib` 仍能成功；以及删除原始缓存／产物后，要求已捕获的完整字节继续完成后续阶段。

同时删除以移走编译器、破坏 LLVM／sysroot 路径、移除 generator、删除 native 源码或制作隔离工具副本来证明“输入已不可用但必须成功”的验收要求。源码和工具正常存在时的缓存命中与按需调用，用实际调用记录验证。

按测试目的处理，不能机械删除所有含 `remove` 的步骤：

| 情形 | 必须处理 |
| --- | --- |
| 整个 case 只验证无源码／无工具／删除缓存后的消费能力 | 删除整个 case 和仅被其使用的输入、诊断及快照 |
| 语言、泛型、FFI、GC 或运行断言与源码隔离混在一起 | 迁到源码保留的真实多 Cone 构建或增量缓存场景；移除隔离步骤、隔离成功断言及其专属输入 |
| negative case 先删除 provider 源码，再验证语言错误 | 保留源码和原语言诊断，删除隔离包装；不得把私有访问、泛型约束等语言错误一并删除 |
| 有源码构建与无源码独立链接各运行同一程序 | 删除无源码场景；实际需要的链接功能由少量源码保留的专测覆盖，普通语言 case 只编译运行 |
| 为测试缓存变化而修改／增加／删除真正的编译输入 | 保留输入失效的功能测试，不再附加缓存内容合法性和隔离恢复断言 |
| 测试结束后清理临时目录或输出 | 正常资源清理，保留 |

仅删除 `remove_source` 一行、保留空 sysroot／单独复制工具／原始输入失效后的隔离断言，不算迁移完成。把 case 改名、标为 skip 或移到 legacy 验收组也不算完成。

## 2. Fixture 扫描基线

扫描仓库全部 fixture TOML 中的 `steps[].files`，汇总 `remove`／`move` 的实际操作数；同时检查内联描述、工具／文件操作和 Rust helper。构建输入中的故意非法 `Cone.toml` 不作为 fixture 描述解析。

基线共有 **1,632 份 fixture 描述包含 6,460 个 remove／move 操作**。按 source／sources、core-source、provider、consumer、edited-library 等实际操作路径，并补充 `m33-sysroot/combination` 的明确源码删除步骤，得到 **1,626 份源码隔离迁移候选**。内联 fixture 扫描未发现额外的 remove／move 操作。

这是需要逐项处理的描述文件数，不是应整体删除的测试数，也不是 target／variant 展开后的执行数。一个描述中可以同时有多次源码删除、正常功能断言和失效测试。完整文件名、fixture 名、命中步骤和操作见 [SOURCE-ISOLATION.tsv](SOURCE-ISOLATION.tsv)。

2026-10-15 开始实施时复扫：原 1,626 个路径全部存在，另有 M46 的 `m46-serialize/artifacts` 与 `m46-values/artifacts` 两份混合语言／隔离用例，已补入清单。因此当前候选为 **1,628 份**；包含 remove／move 的全部描述为 **1,634 份、6,474 个操作**，其他六份仍按下表逐项判断。当前正式 fixture 库共 4,091 项；M46 历史验收数量不回写。

| 范围 | 候选描述数 | 主要内容 |
| --- | ---: | --- |
| M23 | 1,501 | core/provider 发布、泛型／导入／可见性、产物 reader、独立链接及各种无源码包装 |
| M24 | 8 | release block／effect 与产物消费组合 |
| M26 | 6 | 字符、数组、集合、字符串及插值的跨 Cone 包装 |
| M27 | 1 | Context 组合 |
| M29 | 13 | 注解、静态描述、companion 和编解码产物组合 |
| M30 | 2 | 浮点产物组合 |
| M31 | 2 | ODR／优化组合 |
| M32 | 1 | roots 组合 |
| M33 | 31 | main、process、shutdown、sysroot、native C/C++、FFI 等 |
| M34 | 8 | 容器、优化、接口、DirectC、MaybeUninit 与值类型 |
| M35 | 5 | 注解、控制流及组合 |
| M37 | 28 | 新语法／推导／注解／引用等功能的隔离包装，包括 negative case |
| M41 | 4 | 运算符、可见性、浮点 |
| M42 | 14 | 标准库与真实应用的多层产物包装 |
| M43 | 1 | 测试依赖及测试程序的无源码消费 |
| evaluation-order | 1 | 派生方法求值顺序的产物包装 |
| M46 | 2 | Serialize 与 JsonValue 的多 Cone 组合包装 |
| 合计 | **1,628** | 每项须给出删除或迁移后的落点 |

迁移示例：

- [M23 equality](../../tests/fixtures/m23-cli/equality/fixture.toml) 的 `isolate` 移走整个 source，删除 sysroot／runtime，再用独立 scoop 链接。保留 equality 及语言错误断言，删除该隔离流程。
- [M33 main](../../tests/fixtures/m33-main/no-args-unit/fixture.toml) 的 `remove_source` 后才链接和运行。main／argv／返回值改为运行正常 build 的结果；需要的入口链接专测保留源码。
- [M42 application](../../tests/fixtures/m42-application/fixture.toml) 逐层删除 core、std、json、provider、consumer 和 application。保留真实多库应用的构建、运行及增量缓存行为，删除整套无源码要求。
- [M43 test dependencies](../../tests/fixtures/m43-test-artifact-dependencies/fixture.toml) 修改 locator 后删除 provider／helper，之后又删除 root 源码。改为生产与测试依赖正常存在时的编译、缓存和运行测试。

六份包含文件删除但未纳入上述源码隔离候选的描述，需要分别处理：

| 描述 | 判断 |
| --- | --- |
| `m23-cli-cache/core-default/fixture.toml` | 混合默认来源、缺失输入、损坏产物与 dangling symlink；保留正常源码来源／缺失输入规则，删除损坏与路径防御分支，按 M48 新合同调整其余部分 |
| `m28-linux/native-dso/fixture.toml` | 准备阶段清理 libs 目录及正常 DSO alias，不是源码隔离；保留真正的 native 链接覆盖 |
| `m31-optimization/profile/fixture.toml` | 清空 cache 以建立冷构建，不是隔离；保留冷／暖与优化模式对照 |
| `m33-sources/cache/fixture.toml` | 删除选中的目录以测试输入变化；保留对应更新／缺失输入行为 |
| `m33-sysroot/negative/missing/fixture.toml` | 测试真正的缺失输入；保留普通诊断，不扩展为路径来源认证 |
| `m43-test-list/fixture.toml` | 删除 tests 目录构造空测试列表；保留正常测试发现行为 |

实施时重新扫描新增或改名的描述。基线清单不是免除其他路径检查的允许名单；特别检查复制到空目录、改写 manifest locator、无效工具环境和 public helper 隐式移除来源的方式。

## 3. Rust 测试与公共 helper

| 位置 | 当前历史动作 | M48 处理 |
| --- | --- | --- |
| `scoop/tests/retained_artifacts.rs`（已删除） | `completed_graph_links_retained_artifacts_after_original_files_are_removed` 删除源码和全部原始产物后链接内存副本 | 删除这个隔离 case；普通图结果复用在输入仍存在时验证 |
| [scoop/src/artifact/tests.rs](../../compiler/scoop/src/artifact/tests.rs) | 删除 candidate 与 cached 文件后，要求 CompletedNode 持有完整归档 | 删除“文件消失后仍拥有字节”的要求；仅保留实际模型需要的数据共享测试 |
| [linker/src/test_support.rs](../../compiler/linker/src/test_support.rs) | `compile` 编译每个 Cone 后立即删除 source；native fixture 编译后删除 native.c | 删除 helper 中的隔离动作，所有调用方不再隐式继承无源码前提 |
| [linker/src/artifacts/tests.rs](../../compiler/linker/src/artifacts/tests.rs) | C++／musl 目标用例在读取闭包前删除源目录 | 保留目标工具链能力诊断，保留源目录 |
| [driver executable_type_sites](../../compiler/driver/src/request/preflight/end_to_end_tests/executable_type_sites.rs) | `published_type_occurrences_replay_from_bytes_without_dependency_sources` 删除 provider 的 src | 改为真实下游类型使用测试，删除无源码和重复 replay 的目标 |
| [driver generic siblings](../../compiler/driver/src/request/preflight/end_to_end_tests/generic_bodies/siblings/conflicts.rs) | ODR／优化组合在生成 provider 后删除其根目录 | 保留实际泛型与代码选择覆盖，去掉隔离前提 |

搜索中还有 codegen 测试在运行／检查完成后删除临时 `.o`、C source 或 binary，以及 TempDir／Drop 清理。这些按实际时序判断，不因关键词被误删。与测试无关的生产路径文件替换也不计作历史 case。

## 4. 缓存破坏与文件系统防御测试

删除只验证下列行为的测试及其专用 helper／快照：

- 修改、截断或伪造缓存 `.slib`、member、收据，要求报完整性／合法性错误；包括 [artifact-corruption](../../tests/fixtures/m23-cli-program-link/artifact-corruption/fixture.toml) 及 [cache/store/tests.rs](../../compiler/scoop/src/cache/store/tests.rs) 中 `exact_key_corruption_is_not_a_miss` 等旧约束。
- 命中后重新验证 warning origin、收据与整个依赖闭包；`cache/validation/tests.rs`（已删除） 中真正属于输入变化的部分改为普通更新判定，其余 proof 约束删除。
- 缓存 payload／锁路径为 symlink、读取中替换 inode、非私有目录、不同 owner／权限、源路径逃逸等。源码正好通过普通 symlink 或路径带空格访问成功的正常 I/O 覆盖可保留，不以路径所属区域决定是否允许编译。
- 编译过程中改写 scoopc 后要求 `CompilerChanged`；[child/tests.rs](../../compiler/scoop/src/child/tests.rs) 的 `production_runner_rejects_signal_and_compiler_mutation` 拆掉 mutation 分支，保留实际子进程 signal／失败传播。
- 写入后修改缓存、候选目录或已发布 executable，再要求 reader 发现、修复、重试或返回特定防篡改错误。

必要的源码 negative、选中输入缺失、工具真实启动失败、子进程协议读写、编译／链接失败不发布部分结果、普通构建锁协调仍保留。不能把输入失效测试改成破坏输出的恢复测试，也不通过移动到“显式验证模式”维持旧防御验收。

## 5. 文档与最终闭合

1. 按主设计同步实现规范 2.6～2.8 和相关消费边界，移除“无源码 prebuilt 的完整合法性验证”“运行前制作私有字节副本”等要求。
2. fixture runner README 删除把“源码构建／artifact-only 隔离一致性”和“文件损坏诊断”列为普遍验收目标的文字，保留普通缓存更新、源码诊断和实际链接行为。
3. M47 的计划改为源码与工具保留的缓存／生成器调用测试；其缓存输入采用 M48 的更新判定。后续里程碑不得复制旧隔离模板。
4. 历史已完成的验收记录保留其当时事实，不回写历史测试数量；当前有效的规则和新验收必须采用 M48。
5. 每批删除或迁移时记录旧描述、保留的有效断言及新落点；清理仅被删除 case 使用的输入、快照、诊断、fixture tag 和工具配置。无需新增常驻测试迁移系统，使用普通变更记录即可。
6. 最终重新扫描 TOML／内联描述、Rust 测试、公共 helper 和活动设计；不存在源码隔离成功要求、跳过的旧隔离组或用于保持旧合同的独立 reader。剩余文件删除步骤必须服务真实输入变化或测试结束清理。

通过迁移后的正式 CLI 全量和相关 Rust 测试确认语言、ABI、GC、FFI 与运行覆盖仍在；减少的执行数要能解释为历史场景退役或冗余覆盖合并，不能仅以测试总数下降声称性能改善。

活动设计复查：M47 已采用源码／工具保留和实际调用观察的合同；M38 未发现要求删除源码／工具后仍成功的条款。尚未实施的 M45 设计仍有“无源码下游链接运行”要求，列入 M48-5 的活动设计清理，不以该旧条款约束本期实现。

### 已执行批次：配套编译器

`compiler/scoop/src/child/tests.rs::production_runner_rejects_signal_and_compiler_mutation` 已去掉执行中改写工具并要求复核失败的分支，保留为 `production_runner_reports_signal`。普通工具首次运行失败、能力不匹配和协议错误仍验证。新增工具真实输入变化、重复探测复用和并发首查测试，不使用损坏工具来证明缓存命中。SOURCE-ISOLATION 清单尚未迁移，不因本批通过减少其待办范围。

### 已执行批次：Scoop 源码路径与读取

`m33-sources/negative/symlink-escape` 的唯一拒绝依据是目标位于 Cone 根外，已删除该描述、诊断与专用无效输入。正常能力改由 `m48-source-inputs/file`（外部文件、暖构建、实际源码更新）和 `m48-source-inputs/testing`（目录内 alias、外部测试目录和执行结果）验证。Rust 的两处 escape 拒绝分支改为实际读取及 logical source identity／role 断言；重复选择、缺失目标和遍历循环测试保留。此项属于路径来源审计退役，不减少 SOURCE-ISOLATION 的 1,628 个产物隔离迁移候选。

### 已执行批次：实际输入路径

`PreparedStaging` 的 symlink namespace 拒绝、输出目录额外文件拒绝两个 Rust case 随审计代码删除。prepared 的源码／prebuilt／single-file／core 用例改为实际路径、逻辑身份及真实源码更新断言，不再检查只读私有副本或改写输入后保留旧字节。`scoop/tests/retained_artifacts.rs` 的唯一隔离 case 删除；`scoop/src/artifact/tests.rs` 保留 CompletedNode 与 closure 共享数据的断言，删除文件移除步骤。

`m23-cli-program-link/diamond-ready-order` 已从 SOURCE-ISOLATION 清单迁移：保留菱形依赖的 6 个 image、10/20/30/40/50 初始化顺序、moving GC 和显式链接时重复依赖去重；源码、工具和原产物始终保留。改为真实多 Cone 构建、暖构建零 compiler child、使用已有产物路径链接及执行；删除复制独立工具、空 sysroot、隐藏工具环境、删除源码和移动产物后再次链接的隔离步骤。该清单目前 1 项已迁移，余下 1,627 项仍待逐项处理。

### 已执行批次：文件读取与 native 变更时序

`manifest/src/file_snapshot.rs` 的 no-follow 拒绝改为普通 symlink 读取，去掉 inode／时间戳是否仍与读取时一致的断言；目录输入仍检查实际读取错误。native 的 C、C++ 组合测试先编译并运行当前头文件，再修改真实头文件检查下一次构建输入变化，不再在准备与编译之间修改头文件并要求继续编译旧快照。普通 C／C++ CLI 缓存用例继续验证变更后的真实运行值；未删除语言、flags 或头文件依赖覆盖。

### 已执行批次：普通构建结果记录

删除 `cache/receipt`、`cache/validation` 及其专用测试，共 12 个收据／绑定／warning 来源与资格断言；删除 child 成功指纹逐字段复核的 2 项测试和 parent 拒绝已成功 child 产物 profile 的 1 项测试。生产时的正常语言、类型、ABI 与产物生成测试保持，parent 不再通过改写 child 结果测试另一套生产验证。

原 cache/store 的 8 项测试随收据、损坏／截断、目录 key 绑定、symlink 审计和并发产物等价性比较退役；正常发布、所需输出缺失更新、失败不发布和构建锁协调改由 3 项普通记录测试覆盖。cache-key 的真实输入／target／配置变化测试仍在，protocol round-trip 覆盖实际生产摘要；GNU／musl 正式 CLI 继续验证目标选择。scoop 单元测试从 104 项变为 84 项（删除 23 项、增加 3 项），没有以删除语言或运行断言换取通过。

新增 `m48-build-record/library` 独立 fixture：冷／暖、同一结果发布到另一输出、用户输出缺失后正常补产，以及同一位置先后发布库／程序／库。其输出缺失步骤要求正常重新发布该输出，源码、工具和原始 cache artifact 始终保留。现有 single-file、warning、dump、测试目录、native 与菱形依赖组合保留真实调用和运行断言；SOURCE-ISOLATION 的 1,627 项待迁移范围不因本批减少。

### 已执行批次：manifest 解析记录

新增 `m48-manifest-record` 保留原始源码与工具，检查实际 manifest 更新后的生产缓存选择及运行结果；M43 的测试配置／测试依赖／target 条件组合继续保留。discovery／graph 的测试 helper 将缓存从 sysroot 内移到独立目录，保持已有图结构、循环、显式 core 来源和诊断区间断言。`m23-cli-inputs/manifest-source-encoding` 同步此前源码内容读取移到 scoopc 后的 `SCOOPC_BUILD_FAILED` 文件级诊断，保留非法 UTF-8、原始路径、独立 producer 和失败不覆盖用户输出。没有删除语言测试，SOURCE-ISOLATION 的 1,627 项待迁移范围不变。

### 已执行批次：C 工具发现

删除父进程暖构建的 C 工具重复探测、child 的同次发现重放，以及 Linux discovery 对整个 GCC／cc1／assembler 的内容读取与摘要。首次发现保留实际目标／libc 编译检查，独立 scoopc 继续正常发现；更新记录只观察正常输入 metadata。协议固定帧长度与 compile key 固定向量随版本同步，不删除格式负例。新增 Linux 真实工具调用记录用例保留工具、源码与已有产物，通过工具修改触发更新；源码隔离扫描清单和其他历史测试尚待后续清理。

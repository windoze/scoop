# M43 实施记录

依据：[设计](DESIGN.md)。独立 Cone 的 integration-test 已移出本里程碑。

## 实施状态

| 批次 | 状态 | 证据 |
| --- | --- | --- |
| M43-1 断言与测试库 | 断言、四种测试标记与每用例执行库已完成 | 普通 scoop.test Cone、8 个断言函数、Test/BeforeTest/AfterTest/Ignore、分阶段失败数据与 runTestCase；语言规范第 16 章 |
| M43-2 测试输入与依赖 | manifest、源码选择、定义处候选隔离、测试构建图／缓存与输出已接入 | test.sources/test-dependencies；scoopc build --test 与 protocol 6；生产名称查找排除测试输入，生产构建与缓存不受测试配置影响 |
| M43-3 静态发现与生命周期入口 | 发现、构造选择、入口与全部用例适配、目录持久化和运行时选择已接入 | TestEntry 与 typed 目录；library/executable 测试产物、生成闭包及 runTestCase 的实际构造／hooks／正文执行、失败继续与 moving GC |
| M43-4 CLI 与运行报告 | 命令选择、独立结果通道、输出捕获与分层报告已接入；完整验收待完成 | 正式 CLI 覆盖 library/executable 根、debug/release、缓存、moving GC、生命周期失败与进程异常结束 |
| M43-5 scoop.std 应用、产物与平台验收 | scoop.std 首批同 Cone 测试已接入；完整 CLI 组合与 Linux 验收待完成 | 3 个容器、10 个用例；Darwin debug/release、生产隔离、缓存与 moving GC |

## 工作约定

- 每个可用功能完成格式化、lint、定向验证后提交；失败与修复按实际记录，不预写通过结论。
- 工作文件及工具临时目录位于仓库 `tmp/m43/`，验证产物位于 `target/m43/`；定期清理已无用途的生成目录。
- 本机先运行受影响验证；最终使用 nuc12 的 `~/repos/scoop` 做 Linux GNU/musl 完整验证，保留实际范围与结果。

## 2026-10-14：普通断言库

- 先补语言规范 16.1～16.2，再以 5 个普通 Scoop 源文件实现断言、失败数据及渲染；最长文件 47 行，没有 compiler/runtime 特判或新依赖。
- 覆盖 Boolean、Equals、引用身份、预期异常、显式失败、SourceLocation 默认实参/显式转发、失败值渲染、渲染异常保留和四个类型约束负例。6 个正例均通过 debug/release，异常、引用身份和渲染另有 moving GC 执行。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 与三个 CLI 的构建通过。Darwin 定向验收累计 10 个不同 fixture、16 个变体、34 个成功步骤通过；没有运行无关全量测试。
- 成功来源：[首轮](validation/assertions-darwin-initial.json)中的 Boolean、Equals bound、identity、location、ref bound、Throwable value bound；[修复轮](validation/assertions-darwin-fixes.json)中的 equality、exceptions；[最终补验](validation/assertions-darwin-final.json)中的 rendering、Throwable class bound。前两份保留当时失败，不能单独当作完整通过报告。
- 修复的是用例与文档：接口 override 需显式 public；含 Throwable 的 enum/Option 用模式判断，不能假设结构相等；catch 沿既有规则物化 managed 副本，用字段和引用成员验证保留；f-string 插值使用 `${...}`。断言库实现不需要底层绕行。
- 清理旧 `target/m39`、`target/m41`、`target/m42` 生成目录；保留当前构建缓存。nuc12 工作区干净，Rust、LLVM 22.1 与 musl 工具存在，非交互 SSH 需显式配置 Cargo/LLVM PATH；尚未同步或执行 Linux 验收。

## 2026-10-14：测试 manifest 与生产构建隔离

- 先补语言规范 12.2.4 与实现规范 2.7，再接入 `test.sources`、`test-dependencies` 的解码；生产与测试依赖复用相同的 coordinate/locator/span 解析逻辑。测试配置独立于生产语义投影，不增加产物字段或改变 wire 版本。
- 测试依赖重复声明生产依赖时直接诊断测试声明；覆盖相同版本、覆盖版本/locator、未知字段、非法清单/路径/平台谓词、非法 coordinate 和冲突/空 locator。
- 真实生产构建组合证明：根与依赖 Cone 的测试代码均不解析；无效 UTF-8、不可用的测试源码/产物/search-root 依赖、测试反向依赖均不影响普通 build/run。改变测试文件与配置后，生产产物指纹不变，编译器子进程调用列表为空。
- 格式化、workspace clippy、三个 CLI 构建通过；`scoop-manifest` 49 项 Rust 测试及 fixture runner 47 项公共规则通过。Darwin 新增 12 项 fixture 通过（[生产隔离](validation/manifest-production-darwin.json)、[11 项负例](validation/manifest-negative-darwin.json)），另通过 [32 项既有源码选择负例](validation/manifest-regressions-darwin.json)。没有重复断言库或无关全量验证。
- 依赖解析拆入 111 行子模块，manifest 主解析文件降至 177 行；测试配置结构为 42 行。下一步仍需测试用途的源码/依赖合并、定义处名称边界，再交付发现与合成入口；本次不宣称 `scoop test` 已可用。

## 2026-10-14：源码角色与同 Cone 声明候选

- 先补语言规范 12.2.4 和实现规范 2.7，再复用现有源码发现流程合并生产／测试文件；默认测试目录可缺失，测试集合可为空，生产集合仍须非空。跨角色的目录重叠、重复文件、symlink 别名／损坏／逃逸均按实际输入检查。
- SourceRole 随发现结果、AST 和快照保存，不改变 SourceIdentity，也不从目录名推断。合并集合按完整路径排序；driver 的解析适配保留角色和原文件诊断，HIR 前端据此收集定义处候选。
- 生产源码的类型、值、函数、扩展、const、别名、注解和 import 排除测试声明；测试专属 package 不遮蔽生产对象限定名。普通调用、泛型正文和默认实参的重载选择不受测试重载改变，测试源码仍能访问生产 internal，不能绕过文件／成员 private。
- 验证时发现 M42 新增的 Float/Double.toBits 块正文不适用于既有精简数值签名测试辅助代码。按已有 IEEE 比较／codec 的分工排除该普通方法，原始实现仍由完整 sysroot fixture 验证；独立修复提交 `36fa9d24`，22 项 imported-core 回归通过。
- 格式化、workspace clippy 及三个 CLI 构建通过。Rust 验证：manifest 56、AST 15、parser 506 及 1 项 doctest、driver 1 项发现到解析测试；前端新增 7 项组合测试、430 项 M23 回归和 72 项 import 专测通过。没有运行 workspace 全量测试。
- Darwin 正式 CLI 的 M43 manifest、M33 source selection 和跨 Cone qualified-types 回归全部通过：51 个 fixture、52 个变体、100 个进程步骤；包含缓存失效边界、生产构建隔离、准确负例、无源码依赖产物消费、链接与 moving GC 运行，见[报告](validation/source-roles-darwin.json)。
- 源码发现拆出 72 行收集模块；package／源码作用域集中于 140 行模块，namespace 主文件降至 407 行。本轮清理已归档证据的 `target/m43/assertions` 生成目录（约 216 MB）。
- 测试用途的 CLI／子编译器请求、测试依赖合并与定义处依赖候选、测试构建键、合成入口及运行器仍未交付；上述规则的正式 `scoop test` 文件 fixture 随命令接入补齐，当前不宣称 M43-2 或 M43 完成。

## 2026-10-14：定义处依赖候选隔离

- 先补语言规范 12.2.4 和实现规范 2.7，明确按当前 Cone 的直接依赖边筛选公开 binding；前端输入显式携带测试依赖身份，core 保持生产依赖。复用同一依赖语义数据，不分别加载或编译生产与测试依赖闭包。
- package 前缀、当前 package、exact／alias／star import、qualified type 和扩展候选均在冲突与重载选择前筛选；泛型正文和默认实参沿定义处规则。qualified-type 索引保留每个 provider 的贡献，筛选后才合并同一实体的 re-export 路由。
- 生产依赖 re-export 的实体仍可用，即使原始声明 Cone 同时被直接列为测试依赖；选中 nominal 后按普通规则访问其静态成员。新增 6 项依赖角色组合测试与 1 项 re-export 回归，并扩展 HIR namespace 测试以验证同一实体的路由筛选和静态成员访问。
- 格式化、workspace clippy、三个 CLI 构建通过。14 项 M43 前端测试、431 项 M23 回归、72 项 import 专测及 3 项 HIR namespace 测试通过；M23 组包含上述新增 re-export 回归。测试准备中修正了非法同名 property/function、包限定表达式和未公开构造器，未为测试改变语言行为。
- Darwin 正式 CLI 的 7 项跨 Cone 定向 fixture 全部通过，合计 7 个变体、60 个进程步骤；覆盖包前缀、冲突、support package 不可见、diamond re-export、泛型嵌套类型／别名、无源码产物消费及 moving GC 运行，见[报告](validation/dependency-roles-darwin.json)。没有运行无关全量测试。
- `scoop.std` 已明确为首个实际测试应用，设计变更独立提交 `62c1cb3b`。清理已归档报告对应的 `target/m43/source-roles`（约 593 MB），保留当前构建缓存。
- 测试构建请求、根 Cone 的测试依赖图合并、测试构建键与输出、静态目录和生成入口仍待接入；当前仅完成共享前端的依赖名称边界，正式 `scoop test` 和标准库应用尚未交付。

## 2026-10-14：生成测试入口的身份与产物传递

- 先修订语言规范 12.4.4 与实现规范 2.6～2.7，区分生产 main 与生成测试入口；源码／生成声明使用不同的 typed id，TestEntry key 由当前 ConeIdentity 与 role 构成。入口声明、完整签名与 Strong callable body 一起传递，不借用源码 main 的身份。
- HIR 输出合同、MIR bridge、LIR entry plan 和 artifact-only 入口读取保留 Source／Generated 分支；入口实现必须对应选中的声明。生成入口沿用现有 gateway、failure-root、root descriptor、GC 与异常协议，生产 main 的 body identity 保持不变。
- 按实际编码变化更新 HIR identity-foundation/9、core-bootstrap-interface/15，MIR identity-foundation/7、core-bootstrap-bridge/2，以及 LIR cone-production/12、strong-production/22。旧 section 被拒绝并要求重建，未变更 runtime ABI、manifest 或 link-identity-closure 格式。
- 格式化、workspace clippy、三个 CLI 构建通过。identity 349 项及 1 项 doctest；HIR/MIR/LIR production 分别 55/15/187 项；MIR/LIR bridge lowering 1/2 项；根网关 5 项、前端入口 9 项、codegen 6 项通过。slib 全量首轮 567 项通过，唯一失败是 section 版本改变后的语义指纹固定向量，更新该向量后定向补验通过；profile 的 57 项已通过。固定编码向量只按新的声明分支与版本更新，未放宽原有检查。完整日志见[验证记录](validation/entry-rust.log.gz)。
- 新增专测覆盖生成 key 与入口的读写、不同 identity domain 的区分、错误 generated role／签名、同 Cone 源码 main 并存、外 Cone／缺失声明、错误实现、复用生产 gateway 的拒绝、链接端恢复，以及生成入口的正常 LLVM root descriptor 与 managed/no-GC 网关。
- 入口身份实现拆为 209 行主体与 120 行声明模块；LIR/codegen 的测试 fixture 构造移入各自 support 模块，测试主文件分别为 216/167 行。清理已归档的 target/m43/dependency-roles（约 689 MB）。
- Darwin 正式 CLI 的四种入口、7 类签名负例及 4 类 shutdown 场景全部通过：15 个 fixture、23 个变体、169 个进程步骤；含 debug/release、删除源码后的 artifact-only 链接、参数／UTF-8／退出码、异常与 moving/minor GC，见[报告](validation/entry-darwin.json)。本批未重复不受影响的全量 CLI 或提前执行 Linux 验收。
- 本批尚未合成前端测试函数正文，也未接入测试构建用途、静态目录、生命周期运行器或 scoop test；这些能力与 scoop.std 首个实际应用继续按设计实施，不把本批 metadata 支持视为 M43-3 或 M43 完成。

## 2026-10-14：测试标记与声明形态

- 先补语言规范 16.3 和实现规范 2.6，在普通 scoop.test Cone 中声明 Test、BeforeTest、AfterTest 与带必需 reason 的 Ignore；使用现有 Target 和不可重复注解规则，没有新增产物格式或 runtime ABI。
- 前端按实际绑定的 typed annotation identity 识别标记，拒绝生产源码中的应用；检查容器的 class／顶层／非泛型／具体形态、直接声明的实例方法及 safe／同步／Unit／无额外参数要求、角色冲突、孤立 Ignore 和同类重复 hook。重复 hook 同时定位两处声明。未标记 helper 继续遵守普通语言规则。
- 格式化、workspace clippy、三个 CLI 构建通过。23 项 M43 定向测试与 36 项 annotation 回归通过（两组共有 9 项新增标记测试）；覆盖测试库自身无自依赖、跨 Cone 导入与 alias、同名用户注解、普通名称冲突、private 容器／方法／hooks、Unicode 名称及精确源码角色，见[验证日志](validation/markers-rust.log.gz)。测试准备中修正了 core 源文长度、声明分隔符、context 前缀及普通诊断期望，未修改语言规则迁就测试。
- Darwin 正式 CLI 的 5 项新负例与原有 10 项断言 fixture 全部通过，共 21 个变体、39 个进程步骤；新负例精确检查四种标记及 alias／跨库 re-export 的生产源码诊断和位置，断言回归包含 debug/release、跨 Cone 调用位置及 moving GC，见[报告](validation/markers-darwin.json)。复用首次构建缓存，没有运行无关全量测试。
- 形态检查主体为 200 行，身份识别为独立小模块，测试辅助与跨库用例分别拆分。核对归档报告后清理上一批 target/m43/entry（约 316 MB）；scoop.std 首个应用已同步进路线图，提交为 `f119a228`。
- 零实参构造选择与默认实参的实际求值、静态目录、调用适配和运行器仍须随前端合成接入。本批不宣称 scoop test 已可用，也未执行 Linux 最终验收。

## 2026-10-14：前端静态目录与构造选择

- 先补语言规范 16.4 与实现规范 2.6，再由已检查的注解生成当前目标的 Export HIR 测试目录。容器、构造器、用例和 hooks 保留各自 typed 声明引用；保存原始方法名、位置和 Ignore 原因，未标记 helper 与继承的方法不进入派生类的用例清单。
- 容器按规范化源码路径／声明位置排序，用例按类内位置排序；同名文件内 private 容器的显示名带源码路径，身份仍由已有 nominal/function identity 决定。空容器保留完整构造检查，没有伪造用例。
- 普通构造解析拆出原有候选选择，普通调用仍随即完成参数物化；测试目录复用同一选择流程，在容器自身词法域选择唯一 safe 零实参构造器，覆盖默认参数、空 vararg、候选优先级、private/protected 及歧义。目录保存已选构造器，默认实参模板仍属于原声明，不在发现时实例化执行调用。
- 格式化、workspace clippy、三个 CLI 构建通过；27 项 M43 定向测试及 72 项构造相关回归通过，见[日志](validation/discovery-rust.log.gz)。新增覆盖 typed 关联、hooks、忽略原因、Unicode、跨文件同名 private 容器、源序、普通继承、构造优先级，以及空／忽略容器仍拒绝缺参、歧义和 unsafe 构造。测试准备修正了输入必须按 SourceIdentity 排序和普通缺参诊断的期望。
- Darwin 的 6 项构造相关 CLI fixture 全部通过，共 6 个变体、19 个进程步骤；包含普通／跨 Cone 的重载与默认参数、继承委派、private 与 unsafe 拒绝、移除源码后的产物消费、链接运行及 moving GC，见[报告](validation/discovery-darwin.json)。没有重跑无关全量测试或提前执行 Linux 验收。
- 主发现文件 189 行，目录、形态与构造选择分别拆为 102／52／44 行模块，HIR 目录是 31 行普通数据结构；未增加通用测试框架、依赖扫描或 wire 字段。核对归档报告后清理 target/m43/markers（约 192 MB）。
- 测试目录尚未接入 CLI 的缓存／列表输出，入口函数、生命周期调用、结果通道与 scoop.std 测试应用继续实施；本批不宣称测试命令或 M43-3 已完成。

## 2026-10-14：普通 Scoop 每用例执行与失败保留

- 先补语言规范 16.5，并同步设计 6.2；`scoop.test.execution` 提供 TestAction/TestActions、TestPhase、TestFailure、TestResult 与 runTestCase。实现只依赖 core，沿用普通 closure、try/catch、SourceLocation 和 managed Throwable，不增加 runtime ABI。
- 初始化回调执行构造并返回捕获实例不可变引用的正文与可选 hooks。实例初始化失败时跳过后续动作；准备失败时跳过正文但仍清理；正文失败后仍清理；清理失败保留原失败及各自阶段和位置。没有原失败时，清理失败成为首次失败。AssertionFailure 使用断言位置，其他异常使用阶段位置；异常数据不提前渲染。
- 四组正式 CLI fixture 覆盖每用例新实例、串行顺序、缺省／单个 hooks、默认实参／字段／init 的真实构造失败、四阶段异常及 finally、双重失败、调用处与显式来源位置、Context 恢复、异常引用与 GC 保活。测试库实现分别为 25、44 行，fixture 源码各为 44～80 行。
- 格式化、workspace clippy 和三个 CLI 构建通过，见[日志](validation/lifecycle-build.log.gz)。Darwin debug/release 共 4 个 fixture、8 个变体、24 个进程步骤全部通过，含普通运行与 moving-GC 运行，见[报告](validation/lifecycle-darwin.json)。准备期间修正了 fixture 的可变局部捕获、成员可见性、数值转换、if 表达式和 enum 模式警告；据实际 closure 规则将动作创建移入初始化回调，没有放宽语言规则。
- 验证沿用本批未变化的 core／runtime 缓存；核对归档报告后清理上一批 `target/m43/discovery`（约 294 MB）。本批仅验证普通库的每用例执行，合成入口／调用适配、测试构建编排、独立结果通道与命令报告、scoop.std 应用及 Linux 最终验收仍待完成。

## 2026-10-14：前端生成测试入口与生命周期调用

- 先补实现规范 2.6，再增加独立 TestEntry HIR 函数身份及 `lower_current_tests` 入口。生成函数沿用既有 generated callable key、词法闭包、具体化和 executable entry 合同；源码 main 保留为普通声明，不参与测试入口选择。未新增 wire 编码或 runtime ABI。
- 根据已检查目录构造每用例初始化回调，物化先前选定构造器的默认／空 vararg 实参，再创建捕获同一实例不可变引用的正文和 hooks。调用普通 scoop.test.execution 的 TestAction、TestActions 与 runTestCase；按普通 typed binding 和调用决议消费当前测试库或直接依赖产物，不进行字符串反射调用。失败后继续后续用例，入口汇总失败退出码。
- 生成闭包保留测试容器的文件域与类词法域，跨文件同名 private 容器使用各自声明及位置。SourceLocation 复用 core intrinsic 与原声明来源；生成入口、闭包与已有 main 可共同完成 HIR 产物投影。运行库调用不匹配时保留普通重载诊断，合成调用失败且尚无诊断时也必须报错，不能静默跳过用例。
- 格式化、workspace clippy 与三个 CLI 构建通过，见[构建日志](validation/test-entry-build.log.gz)。M43 定向测试与新增运行库签名负例合计覆盖 33 项，生产入口 9 项、持久化身份相关 28 项通过；最终入口组 6 项全部通过，包含完整 HIR 产物投影、实际测试库依赖消费、private 多文件、默认参数、空入口进入 MIR，以及运行库签名负例的准确位置，见[日志](validation/test-entry-rust.log.gz)。完整用例的 MIR／LIR／原生执行将随 driver 接入验证，当前未用空依赖伪造测试运行库的机器调用合同。
- Darwin 正式 CLI 的四种合法入口与七类签名负例全部通过，共 11 个 fixture、15 个变体、107 个进程步骤，包含 debug/release、参数／UTF-8／退出码、移除源码后的 artifact-only 链接与 moving/minor GC，见[报告](validation/test-entry-darwin.json)。这些是共享生产入口回归，不代表 scoop test 已可运行。
- 入口主体 190 行，绑定、普通调用、用例合成分别为 99／113／112 行模块。核对归档报告后清理 `target/m43/lifecycle`（约 191 MB），保留当前编译缓存。
- 本批仅生成默认未忽略用例的生命周期调用，静态目录仍保留全部用例与 Ignore 原因。执行时过滤／ignored 选择、测试用途的编译请求与构建图、目录缓存、独立结果通道及报告尚待接入；scoop.std 首个实际应用与 Linux 最终验收继续按设计实施，不宣称 M43-3 或 M43 完成。

## 2026-10-14：single-Cone 测试编译与实际入口运行

- 先补实现规范 2.7，再为正式编译请求增加 Production／Test 用途；默认生产，`scoopc build --test` 选择测试用途，single-file 输入明确拒绝。machine protocol 升至 6，request field 11 显式编码用途，缺失字段、未知值与旧协议不被隐式接受；其他产物格式和 runtime ABI 不变。
- driver 在解析前核对生产依赖与测试依赖合并后的 direct artifact 集合，沿既有 support 闭包检查；解析复用正式测试源码发现，并将测试依赖身份传给前端的定义处候选筛选。当前 Cone 维持原 identity，以生成 TestEntry 产出 executable `.slib`，library 源 manifest 无需改 kind；生产模式继续使用原输入和输出规则。
- 生成入口已沿正式 HIR／MIR／LIR、codegen、产物发布和 artifact-only link 完成原生运行。正例覆盖 library 与 executable 根、已有 main 不被自动执行、internal 调用、跨文件同名 private 容器、private/protected 构造器和方法、默认／空 vararg 实参、每用例新实例、hooks、默认忽略，以及初始化／准备／正文／清理失败后继续后续用例并返回失败码。
- 格式化、workspace clippy、三个 CLI 构建通过。协议 24 项、正式源码解析 1 项、请求 7 项、driver 子协议 4 项、machine capability 1 项和父进程 child 6 项通过，见[日志](validation/test-purpose-rust.log.gz)。请求固定帧长度按新增的两个字节从 194 改为 196，没有放宽 reader 检查。
- Darwin 正式 CLI 共 4 个 fixture、6 个变体、31 个进程步骤通过：见[生成入口与 single-file 负例](validation/test-purpose-generated-darwin.json)、[失败流程与生产 import 负例](validation/test-purpose-additional-darwin.json)。两个运行用例均覆盖 debug/release 和 moving GC；生产文件即使处于测试编译中，也不能 import 测试专用依赖。编译失败时不产生新产物，正式链接只消费传入的 `.slib` 与既有 runtime 对象。
- 初次 CLI 验证与 Cargo 的二进制重建并行，触发了已有的配套编译器变化诊断；等待 Cargo 完成后使用配套命令副本重验通过。fixture 准备中修正了默认表达式的可见域、Option 消息参数和 `${...}` 插值写法，没有修改语言规则迁就用例。复用本批缓存，只重验受影响的用例组，没有运行无关全量测试。
- CompilationPurpose 是独立的 30 行模块，共享 HIR 产物构造复用同一实现，driver 相关主体各为 96～333 行。核对归档报告后清理上一批 `target/m43/test-entry`（约 195 MB）。umbrella 测试依赖图／缓存／输出、静态目录传递、运行时选择与报告、scoop.std 应用及 Linux 最终验收继续实施；本批不宣称 `scoop test` 已交付。

## 2026-10-14：umbrella 测试构建、缓存与基础命令

- 先补实现规范 2.7，再接通 BuildGraphRequest 的测试用途：只合并根 Cone 的测试依赖与源码，依赖保持生产用途，原 manifest 与 ConeIdentity 保留；根按 executable 规划、缓存、完成与链接。
- 编译键纳入用途、源码角色、测试源码选择及测试依赖语义，使用 cache key v3；locator 和诊断 span 不进入键。测试输入改变不影响普通生产编译键，未改变的生产依赖继续复用。测试配置的编码独立为 34 行模块，命令编排主体 193 行。
- 增加基础 scoop test 和 --no-run，复用 profile、target、工具链及缓存选项，测试程序发布到 target/<triple>/<profile>/tests/<name>；执行复用既有私有成功产物副本，拒绝单文件输入。当前仍按前端生成的默认未忽略用例串行执行，仅返回基础进程结果。
- 格式化、workspace clippy 和三个命令构建通过。scoop crate 的 102 项 Rust 用例中，首次仅新增测试的错误 manifest 写法失败；修正为 path 条目后，相关 3 项定向复验全部通过。新增覆盖测试依赖只在根展开、依赖的无效测试文件不读取、源码快照角色与不可变性、用途和配置的缓存隔离，以及子编译器用途传递。见[日志](validation/umbrella-rust.log.gz)。
- Darwin 正式 CLI 的 4 个 fixture、6 个变体、48 个进程步骤全部通过，覆盖 library/executable 根、原生产 main 不自动执行、每用例生命周期、失败继续、debug/release、moving GC、只构建、缓存命中、仅测试根重编译及生产缓存保持。见[报告](validation/umbrella-darwin.json)和[运行日志](validation/umbrella-darwin.log.gz)。
- 核对归档报告后清理 target/m43/manifest（约 48 MB）；本批仍保留当前配套工具和热缓存。静态目录持久化、运行时选择、独立结果通道、输出捕获与报告、scoop.std 应用和 Linux 最终验收继续实施，不宣称完整 scoop test 或 M43 已交付。M43 全部完成后再按同样纪律推进 M44。

## 2026-10-14：显式 core 版本的默认定位

- scoop.std 首次通过测试命令接入时，实际暴露版本声明但无 locator 的 core 边在搜索目录无候选后直接失败，未使用既有 sysroot 默认 core 来源；该问题同样影响普通生产构建。
- 先明确实现规范 2.7，再让这类 core 边等待其他声明展开。复用已发现的显式 core 来源，否则由原有默认 slot 选择源码／产物；显式 search root 的有效 core 产物仍优先，损坏候选和明确 locator 的错误不回退。
- 格式化、workspace clippy 与三个命令构建通过；18 项 discovery 用例通过，新增覆盖 production/test 两种根及稍后发现的默认库提供 core locator，既有显式产物、路径冲突、无效 locator、循环来源等回归保持。见[日志](validation/std-core-locator.log.gz)。scoop.std 的源码测试与原生验收继续进行。

## 2026-10-14：scoop.std 首个实际测试应用

- 标准库 manifest 显式增加 scoop.test 测试依赖，生产依赖仍仅 core；标准库规范同步说明同 Cone 测试目录和生产隔离。tests/ 下新增三个 33～50 行的测试文件，共 3 个容器、10 个用例。
- 集合用例覆盖每用例新实例、BeforeTest/AfterTest、排序与快照、窗口与 map/join 组合、参数异常，以及同 Cone internal ListSnapshot 的访问与索引行为；序列用例覆盖惰性窗口、take 的消费边界与非法参数；编码用例覆盖文本字节、Base64、Hex、URL 字母表和 padding 的组合及失败路径。
- 使用正式 scoop test 构建并运行标准库，新增 m43-std-application fixture。Darwin debug/release 的 2 个变体、8 个进程步骤全部通过，包含普通生产构建只依赖 core、测试构建加入 scoop.test、--no-run、缓存复用、正常执行与 moving GC。见[报告](validation/std-application-darwin.json)及[日志](validation/std-darwin.log.gz)。格式化、workspace clippy 与配套工具构建通过。
- 测试准备中把 Option<Array<Byte>> 与 None 的相等比较改为模式匹配，符合数组没有 Equals 能力的既有规则；未改变语言或标准库的相等语义。复用当前 core、测试库及生产标准库缓存，没有重跑已通过的无关 fixture。
- 当前实际应用验证基础串行执行；静态目录、过滤／忽略选择、输出捕获和完整报告仍待实现，随后补充标准库的对应 CLI 组合与 Linux GNU/musl 验收。M43 全部完成后继续 M44，当前目标保持进行中。

## 2026-10-14：测试目录持久化与静态列表

- 先补实现规范 2.6／2.7 和设计 7.2，再由 driver 将已检查的 HIR 测试目录投影为嵌套的展示数据。可选 manifest section `org.scoop-lang.manifest/test-directory/1` 保存容器、用例名称、完整显示名、Cone-relative 位置与 Ignore 原因；测试空目录也保存，生产产物不附带。实际调用仍保留原有 typed 引用，不解析显示字符串重建关系，也不重放前端形态或构造检查。
- 现有 manifest summary 在读取边界解码目录，检查 wire、路径与一基行列后复用。section 只受 ArtifactFingerprint 覆盖，不增加 Compile／Link 语义要求或更改整个归档格式。新增目录、wire、driver 投影与列表渲染分别为 106、91、45、66 行。
- `scoop test --list` 构建后读取根产物目录，不启动测试程序；human 输出到 stdout，JSON 沿现有 stderr 记录流在构建记录后输出 `test-list`。列表保留容器分组、位置、忽略原因与计数，空目录明确显示 0 用例；与 `--no-run` 互斥。
- 格式化、workspace clippy 和配套三个命令构建通过；目录编码／非法路径和位置／Unicode 与空 Ignore 原因、manifest summary 和 capability profile 的 27 项 Rust 定向测试通过。Darwin 正式 CLI 的 debug/release 共 2 个变体、8 个进程步骤通过，覆盖同名 private 容器、带 `::` 的中文用例、空容器／空目录、JSON／human、首次构建和零编译器调用的缓存命中；所有列表均无 initializer／hook／正文输出。位置期望使用前端目录中实际的 Test 标记位置。见[报告](validation/directory-darwin.json)及同名前缀日志；另验证 `--list --no-run` 返回参数错误。
- 核对已归档 umbrella 验收后清理旧 cases/1～3 输出约 62 MB，保留本批输出、sysroot 与缓存。运行时 filter／ignored、独立结果通道、输出捕获与报告、完整 scoop.std CLI 组合及 Linux 验收仍待完成；M43 全部完成后继续 M44。

## 2026-10-14：运行时过滤与忽略选择

- 先补语言规范 16.5 和实现规范 2.7。`scoop` 按完整显示名做大小写敏感的子串匹配，默认选择未忽略用例，`--ignored` 只选择 Ignore 用例；`--list` 复用同一选择函数并移除空分组，区分无测试、无匹配、无忽略用例和匹配项全部忽略。`--no-run` 始终构建完整程序。
- 全部用例（包括 Ignore）均生成 typed 调用适配。入口使用已有 Array<String>／Int ABI，以扁平目录内的序号接收本次选择；该序号不替代 callable identity。普通测试库的 isTestSelected 控制完整生命周期外层的 if，未选中用例不创建实例、不执行默认实参或 hooks。改变过滤条件只改变程序参数，不改变源码输入、编译键或测试产物。
- 20 项前端测试及 2 项选择逻辑测试通过；覆盖 Ignore 适配保留、真正的跨库调用、选择函数结果类型错误诊断、完整 argv 入口的 MIR 依赖、Unicode 匹配、空分组与空选择原因。首次原生验证发现直接运行时误将 argv[0] 当成用户参数，已按既有 12.4.4 修正；其后 7 项入口测试复验通过，debug/release 直接运行恢复默认未忽略选择。
- 格式化、workspace clippy 与三个配套命令构建通过。Darwin 最终执行／负例[报告](validation/selection-darwin.json)为 2 个 fixture、3 个变体、17 个进程步骤全部通过；[初轮报告](validation/selection-initial-darwin.json)中的列表 fixture 已通过 debug/release 共 10 步，其同轮执行 fixture 的 argv[0] 失败由最终报告覆盖修复。合计 3 个 fixture、5 个变体、27 个通过步骤，覆盖 default／filter／ignored／空匹配、同一产物指纹与缓存零编译调用、忽略用例完整 hooks 与 moving GC、构造失败、直接启动，以及过滤不隐藏未选中用例的编译错误及准确位置。
- 选择器 55 行、生成调用条件 38 行、普通库选择函数 15 行；入口主体 217 行。复用未变化的列表验证与 core 缓存，未重跑无关全量测试。独立结果通道、输出捕获与完整执行报告、scoop.std 全套 CLI 组合和 Linux GNU／musl 验收仍待完成；M43 完成后继续 M44。

## 2026-10-14：正式 fixture 读取测试报告

- 增加显式 `json = "test-stderr"`，读取构建记录之后的 test-report／test-list 与工具诊断；result 保留构建结果，test_result 保存测试结果。原 stderr 模式仍在构建结果处停止，保持用户输出的原始字节边界。
- 覆盖报告中的 JSON 形似用户输出、报告后的工具失败诊断、编译失败没有测试结果，以及重复报告拒绝。Ruff 0.16.10 格式化／lint 与 49 项公共 runner 测试全部通过，见 [日志](validation/report-python.log.gz)。执行端与报告端继续以正式 CLI 验证。

## 2026-10-14：修复常量与动态对象的 GC 根合并

- 报告库的 release 构建暴露普通 String 分支合并问题：RS4GC 将 immortal 分支的 base 置为 null，额外生成 base phi／select，与当前对象起点及 base=derived 合同不符。新增 LLVM 最小回归和不依赖测试库的 Scoop fixture；旧工具在后者的 release 构建稳定失败，见 [初始日志](validation/base-merge-before.log.gz)、[源码初始日志](validation/base-merge-fixture-before.log.gz)。
- 在最终 SSA 根分组时检查合并链中的派生地址，并使用 [LLVM 22.1 的 is_base_value 处理](https://github.com/llvm/llvm-project/blob/release/22.x/llvm/lib/Transforms/Scalar/RewriteStatepointsForGC.cpp) 保留对象起点事实；纯常量选择仍不 relocation。不放宽根数量、typed 恢复或机器 stackmap 校验，不改测试库绕过错误，也不改变公共 ABI／产物格式。
- 格式化、workspace clippy 与配套工具构建通过；codegen 全组 370 项测试通过，包括 phi／select、纯常量／null／动态组合及派生地址负例，见 [日志](validation/base-merge-codegen.log.gz)。Darwin 独立 fixture 的 debug/release 共 2 个变体、6 个进程步骤通过，覆盖完整移动与 minor GC，见 [完成日志](validation/base-merge-darwin.log.gz)。M43 报告的其余 CLI 验收仍在继续，Linux 验收尚未开始。

## 2026-10-14：独立测试结果通道、输出捕获与报告

- 先补语言规范 16.5、实现规范 2.7 与设计的提前退出输出归属。合成入口调用普通测试库的 runReportedTestCase／completeTestRun；每次运行使用独立临时目录，begin／end／complete 事件与用户 stdout／stderr 分开。父命令按已选择的目录顺序读取事件，不从用户输出识别状态，不增加公共 runtime ABI。
- 测试库通过普通 native 源码完成单调计时与 fd 捕获，覆盖实例初始化到清理的完整生命周期；结束后恢复 stdio，再渲染结果。默认隐藏通过用例输出，保留失败／中断用例及目标输出；非 UTF-8 字节在 JSON 中以 hex 保留。--nocapture 直接继承输出，与 --list／--no-run 互斥。运行目录随报告结束清理，不缓存测试结果。
- human／JSON 报告保留目标、容器和用例层级、状态、位置、耗时、忽略原因、四阶段失败、首次与清理失败、断言 expected／actual／cause 及渲染失败信息。缺失完成记录时 exit(0) 仍判失败；已开始的用例标记失败，后续未开始用例标记未运行。完整记录不能掩盖真实 shutdown 失败或进程信号。空选择不启动程序，并报告原因。
- 格式化、workspace clippy 与工具构建通过；20 项前端定向测试和 11 项 CLI 测试通过，见 [前端日志](validation/report-hir.log.gz)、[CLI 日志](validation/report-cli-rust.log.gz)、[lint](validation/report-clippy.log.gz) 与 [构建](validation/report-build.log.gz)。父命令运行主体 118 行，事件读取 75 行，human 输出 137 行；native 事件与捕获分别为 82、79 行，库的报告适配与渲染分别拆分。
- Darwin 的最终有效验收合计 7 个 fixture、14 个 debug/release 变体、98 个进程步骤通过：[主组报告](validation/report-darwin.json.gz) 的 64 步覆盖报告、选择、全部生命周期失败及常量／动态 GC 回归；[应用组报告](validation/report-applications-initial.json.gz) 中 scoop.std 的 8 步与真实 runtime shutdown 的 2 步通过；[generated 复验报告](validation/report-generated-darwin.json.gz) 的 24 步通过。标准库两个 profile 均报告 10 个用例通过；shutdown 专测在全部用例通过且 complete 后仍保留 owned callback token，确认目标失败与实际退出码 1。
- 应用组首轮 generated 失败来自不同 fixture 选择复用同一 cases 索引后残留源码，保留其失败报告及 [日志](validation/report-applications.log.gz)。全部进程结束后清理 cases，再复用同一工具和缓存完成干净复验；没有为该输入污染修改编译器。清理了旧用例输出约 411 MB 和两处临时验证产物约 14 MB，保留当前工具与热缓存。
- scoop.std 的 list／filter 组合、完整 M43 验收和 Linux GNU／musl 验收仍待完成；本批不宣称 M43 已全部交付，完成后继续推进 M44。

## 2026-10-14：修正孤立 Ignore 的诊断位置

- 正式声明负例的审阅发现：BeforeTest／AfterTest 后附加 Ignore 且没有 Test 时，发现阶段将错误指向第一个 hook 标记。现在直接使用该 Ignore 应用的源码位置；错误规则与生命周期语义保持。
- 新增精确字节区间测试，分别验证两种 hook 后的诊断确实覆盖 `@Ignore("later")`。格式化、workspace clippy、配套工具构建与 21 项测试声明／发现／入口定向测试全部通过，见 [Rust 日志](validation/ignore-location-rust.log.gz)、[lint](validation/ignore-location-clippy.log.gz) 与 [构建](validation/ignore-location-build.log.gz)。正式 CLI 负例随本批声明验收补齐。

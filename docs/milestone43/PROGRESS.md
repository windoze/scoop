# M43 实施记录

依据：[设计](DESIGN.md)。独立 Cone 的 integration-test 已移出本里程碑。

## 实施状态

| 批次 | 状态 | 证据 |
| --- | --- | --- |
| M43-1 断言与测试库 | 断言、四种测试标记已完成 | 普通 scoop.test Cone、8 个断言函数、失败数据、Test/BeforeTest/AfterTest/Ignore；语言规范第 16 章 |
| M43-2 测试输入与依赖 | manifest、源码选择与定义处声明／依赖候选隔离已完成；测试用途编排与依赖接入待实施 | test.sources/test-dependencies 解码；源码角色贯穿发现、解析与前端；生产名称查找排除测试输入，生产构建与缓存不受测试配置影响 |
| M43-3 静态发现与生命周期入口 | 入口身份与产物传递、声明检查、前端静态目录及构造选择已完成；目录持久化、入口合成与生命周期待实施 | Source／Generated 入口、TestEntry key、HIR/MIR/LIR metadata；当前目标 typed 容器／构造器／方法目录 |
| M43-4 CLI 与运行报告 | 待实施 | |
| M43-5 scoop.std 应用、产物与平台验收 | 待实施 | |

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

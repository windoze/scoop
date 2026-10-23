# M57 实施记录

状态：已完成。更新：2026-10-23。范围为 [DESIGN.md](DESIGN.md) 的普通类型与 companion 条件 interface 实现；整份类型特化不在本期，完整验收见第 18 节。

## 1. 基线与工作方式

- 基线：`f3f8ace69`；分支：`codex/m57-conditional-interfaces`。
- 已同步语言 9.1.8、companion 投影与 codec 来源合同，以及实现／运行时规范的条件关系和接口表职责。
- 分功能完成并提交。本机先做相应格式化、lint、单元／Cone／必要 CLI 验证，再在 `nuc12:~/repos/scoop` 做 Linux 完整验收。
- 临时文件使用仓库外 `../scoop-work/m57-temp`，原始日志使用 `../scoop-work/m57-validation`，不使用 `/tmp`。
- 开始实施前用 Cargo 清理旧 debug 产物，target 从约 28 GB 降至约 7.2 GB；后续在无活动构建时按实际体积继续清理。

## 2. 功能状态

| 功能 | 状态 | 验证证据 |
| --- | --- | --- |
| 语法与普通类型条件接口 | 已完成；where 必填且非空 | 5 个 parser 用例、M57 HIR 定向检查及正式 CLI 诊断通过；组合验证见下文 |
| Companion、HashMap 与 Json 闭环 | 已完成 | 6 个 JSON Cone 原生用例覆盖四种能力组合、嵌套、显式工厂及重复 key 路径 |
| 条件 codec 合成 | 本体与继承合成已完成 | 原有合成回归、JSON Cone 79 项及跨 Cone 合成 codec 验证已闭合，见第 6、12 节 |
| 继承、default、property 与效果组合 | 已完成 | 条件继承、域覆盖、默认方法／属性、suspend、NoGC、运算符及参数作用域通过，见第 7–9、17 节 |
| 导出、跨 Cone 与 ODR | 已完成 | 泛型导入、条件记录 wire 往返／再次发布与双 consumer 原生链接运行通过，包含继承、投影与 default |
| sealed 条件关系与穷尽性 | 已完成 | 6 项定向检查、42 个既有 sealed fixture、条件边界诊断、原生及跨 Cone 验证通过 |
| 类型描述与文档 | 已完成 | 具体接口、属性过滤、注解及条件文档的本地／跨 Cone 验证通过，见第 13、14 节 |
| 条件查询与循环诊断 | 已完成 | M55 负向事实、递归边界及本地／导入声明的诊断来源验证通过，见第 15 节 |
| 条件接口的 GC 正确性 | 三目标验收已完成 | 移动与 minor GC 后的值装箱、字段扫描、继承分派及协程保活通过，见第 16、18 节 |
| 完整组合与 Linux 验收 | 已完成 | 两端完整回归、GNU 库行为与 musl 定向验收通过，见第 18 节 |

第 3–17 节按实施顺序保留分阶段记录；最终状态以本表与第 18 节为准。

## 3. 基础闭环

- AST 保留独立实现块；普通类型和 companion 复用原宿主参数，方法／计算属性的条件不改变类型布局或单例身份。
- 支持条件关系、直接调用、泛型转发、方法引用、属性访问、接口转换和动态接口判断；普通 override 可满足多个条件接口义务。未证明的条件不成立为接口事实，也不能作为 M55 负向约束的证据。
- 实现重叠、重复签名、无关 override、非法存储和负向条件有正式 CLI 诊断覆盖。循环／扩张查询被拒绝，有限嵌套递减和独立成功证明有 HIR 覆盖。
- Concrete 接口关系从声明求值，不读取尚未构造完成的接口表；递归基类实例化顺序有原生覆盖。
- 本机 workspace Clippy 通过。HIR 回归首次 1,526 通过、15 失败；失败均为导入 intrinsic 的独立存储路径，修正后相关 128 个回归用例通过。后续关系求值调整通过 M55／M57 共 29 个用例。
- 已显式准备 Darwin 的 21 个系统 Cone 与 runtime；正式验证消费共享 sysroot／Cone cache。跨 Cone 条件元数据、复杂继承／default、codec 合成及 Linux 验收仍待后续步骤。

## 4. 条件关系的跨 Cone 消费

- `cross-cone-interface/79` 保存原宿主 binder 下的条件声明、成员适用性及带完整 owner application 的 slot 选择；导入后沿用定义处目标，互斥条件下的 default 选择保持独立。
- 参数的 public companion 声明在具体化前随实际泛型实参保留，避免仅使用 Any／动态判断时漏掉接口关系；查询不执行 companion 初始化。
- 无条件实现沿用类型头的普通继承边和接口槽位；实现块只承担有 where 的条件实现（见第 10 节）。
- 本机 workspace Clippy、867 个 HIR 格式测试、83 个泛型导入回归通过；16 个 M57 定向用例覆盖本地、导入及产物分派。格式版本变更导致的一项 slib 期望向量失败已更新，相关 17 个 profile 测试通过。
- 正式 ODR fixture 由 provider、两个 library consumer 和 root 构成，验证条件真／假、非泛型实现块、单例共享及不同实参独立；原链接与逆序依赖重链接均运行成功。Darwin sysroot 已按新格式显式重建。
- HashMap 默认 codec、继承／合成组合、完整 Linux 验收仍未完成。

## 5. HashMap 的默认 codec

- HashMap 原 companion 分别声明 ValueEncoder 与 ValueDecoder 条件实现，使用 K／V 的 public companion；保留原存储、键约束、显式工厂及 map helper。
- JSON Cone 的 6 项原生测试通过：编码／解码独立，两个组件均须满足对应条件；没有 companion 仍能正常使用容器及显式 codec；覆盖嵌套 HashMap、泛型转发、原 entry 表示、新建解码结果和重复 key 的 `/1/0` 路径。
- 明确验证显式 codec 优先于已有默认能力。其他容器未自动迁移，条件 codec 合成及继承组合继续实施。

## 6. 条件 codec 合成

- 空实现块的既有 codec 合成保留原条件及成员适用性，合成正文使用完整 B+C 环境；字段 codec 可以使用已证明的参数 companion 投影。
- 保持显式 primary val 依赖、递归 this、普通 companion、原容器／tuple 组合的优先级。显式方法阻止同签名合成；不具备正确字段 codec 的条件在声明处报错，诊断定位到实现块。
- 2 个 HIR 定向检查与 24 个既有 encoding／decoding 合成回归通过；正式负向 fixture 验证不能用 decoder 条件替代 encoder 条件，反向亦然。
- 完整 JSON Cone 的 76 项原生测试通过。新增 5 项覆盖独立方向、Envelope 与 HashMap 等完整泛型字段、显式依赖与方法优先、递归节点和默认值。首次失败为新用例误把应省略的默认 None 字段写成 null，按现行 wire 规则修正预期后通过，未更改编码规则。
- 跨 Cone ODR fixture 同时消费并执行 provider 合成的 Envelope codec，原链接及逆序依赖重链接均通过。

## 7. 继承条件与分派

- 基类的条件 conformance 沿完整 base application 代入，保留原 owner 与条件身份；重列接口要求在派生类基础约束下等价。普通 override 可覆盖条件 slot，带条件的 override 必须覆盖继承义务的完整域，不能用自身条件证明覆盖。
- 具体子类检查全部可能适用的 abstract 义务，已证明不可能的条件不产生义务。构造类型实参的条件使用代入后的声明事实，不反向猜测其内部参数约束。
- `cross-cone-interface/80` 增加检查后 conformance 记录，导入方直接消费原关系；空接口与继承重列也保留完整条件。普通源声明的虚表描述复用具体 HIR 实际保留的 family；无条件 override 在基类方法不可用时仍保留自己的合法成员与分派，不要求生成不适用的基类正文。
- 本机 workspace Clippy 通过。25 项 M57 定向检查、867 项 HIR 格式测试、84 项泛型导入回归、97 项普通规则回归、121 项类型／产物回归通过。格式版本对应的固定向量已更新，17 项 profile 检查通过。
- 已显式按格式 80 准备 Darwin 的 21 个系统 Cone。完整 core consumer 的 41 项原生测试通过，新增 3 项覆盖普通／条件 override、条件关闭后的虚表位置、构造实参、abstract 与 computed property。跨 Cone ODR fixture 增加相同继承组合及 marker 接口，原链接／逆序依赖重链接均通过；10 个条件接口诊断 fixture 通过。
- 继承 codec 合成、companion 出现在条件接口目标／成员签名、default 条件交叉、sealed／类型描述／文档及 Linux 完整验收仍须继续完成；本节不代表这些组合已验收。

## 8. 条件目标与成员签名中的 companion 投影

- 已确定不成立的继承条件在目标／成员签名替换前排除；仍未知的条件继续保留。判定约束之前先检查全部必需投影的存在性，支持一项参数约束引用另一参数的 companion。
- 成员签名比较不展开已知不适用的投影，调用候选仍保留原有约束失败诊断。跨 Cone 的条件槽位同样先判定适用域，避免为没有 companion 的实参解析投影接口。
- workspace Clippy 与变更后的定向 Clippy 通过，29 项 M57 检查、85 项泛型导入回归、10 个诊断 fixture 通过。完整 core consumer 的 42 项原生测试及 ODR fixture 的原链接／逆序依赖重链接通过；新增用例覆盖接口参数、方法参数／结果、继承、条件关闭及子类普通成员。
- 后续继续处理继承 codec 合成、可同时成立的 default 条件、sealed／类型描述／文档以及 Linux 完整验收。

## 9. 条件 default 的冲突与实际选择

- 定义处检查可能同时成立的接口域；无普通子接口覆写关系的默认实现要求覆盖两边条件的共同 override，不依赖当前是否已有实例。kind／不相关 class 上界可证明的互斥域保持独立。
- 默认成员查找先证明所属条件，再按普通子接口关系消歧。具体接口表按 slot 合并实际适用的选择，源码块顺序不决定实现；直接调用、接口调用和泛型转发使用同一份结果。
- 已先同步实现规范，普通外部成员的类型依赖投影复用 HIR 的成员绑定，不从无条件声明头重复证明接收者关系；保留调用签名、类型引用和声明归属检查。未在 IR／meta 中新增条件求解器，产物格式仍为 80。
- workspace Clippy、33 项 M57 检查、85 项泛型导入回归、97 项普通规则回归及 866 项 HIR 元数据测试通过。完整 core consumer 的 44 项原生测试通过，新增用例覆盖默认方法／属性、块顺序、无条件父接口、共同 override 与未知条件下的泛型转发；ODR fixture 的原链接／逆序依赖重链接通过。
- 11 个条件接口诊断 fixture 通过，覆盖未实例化时的潜在 default 冲突、继承冲突、未覆盖全部条件的成员以及条件未知／不成立时的 default 调用；已有重复成员与 override 诊断保持独立。
- 继承 codec 合成、sealed／类型描述／文档、其余效果组合及 Linux 完整验收继续保留为未完成项。

## 10. 实现块要求显式条件

- 按讨论收紧语法：`override interface` 的 where 必填且非空；无条件实现写在类型或 companion 的接口列表，override 成员留在普通 body。条件继续遵守既有重复约束规则，不增加严格缩小实参域的证明要求。
- AST 直接保存必填条件；缺省 where 的诊断定位到接口名称并指向普通接口列表写法。原无条件块示例、继承用例与 ODR provider 已改写，删除仅为该旧写法建立的专测；产物格式保持 80。
- 5 项 parser 检查与 M57 HIR 定向检查通过；正式 CLI 覆盖普通类型／companion 缺少条件和空 where，原有存储诊断保留。workspace Clippy、core consumer 原生测试及跨 Cone ODR 原链接／逆序重链接通过。

## 11. sealed 条件关系与穷尽性

- 条件实现复用直接子类型登记，检查 named／non-local、原 Cone 与 package；目标经 alias／re-export 引入也不改变限制。共享声明的直接引用检查同时识别普通父类型与条件目标，格式保持 80。
- sealed 家族按每条关系的原条件展开，未知条件及额外参数继续保留；已证明条件不成立的应用排除。覆盖检查使用当前路径的条件事实，companion 投影无法反解时保留整个可能家族，支持未公开子类型和 companion 自身的条件关系。
- workspace Clippy、38 项 M57 定向检查、87 项跨 Cone 导入回归、97 项普通规则回归和 866 项 HIR 元数据测试通过。完整 core consumer 的 46 项原生测试与 ODR 原链接／逆序重链接通过。
- 13 个条件接口诊断 fixture 和 42 个既有 sealed fixture 通过；后者首次仅有一个缺失示例从 Two 变为 One，两者均不被值比较分支覆盖，核对后更新该快照并定向复验通过。新增诊断覆盖未知条件漏分支、局部／匿名实现以及跨 package／Cone 实现。
- 继承 codec 合成、类型描述／文档、其余效果组合及 Linux 完整验收仍未完成。

## 12. 继承 codec／inspector 合成

- 具体子类和自行继承 codec class 的 companion 可按原规则合成 encode／decode／inspect。生成成员保存原条件声明与完整基类 application，`Base<Wrapper<T>>` 的条件仍作用于 Wrapper<T>；不反向推断 T 的约束，也不把生成方法变成无条件成员。
- 合成正文、直接调用、方法引用、导入及具体化使用同一条件。调用检查复用现有参数约束求解；已有本体实现块继续使用 B+C，继承合成保留实际宿主参数和独立的原基类条件。补齐 companion 基类构造检查的宿主参数作用域，遵守原有合法用法。
- `cross-cone-interface/81` 的 callable field 12 改为完整条件 application，复用 conformance 的编码；签名引用与 binder 形状沿普通元数据规则检查，runtime ABI 不变。已显式准备 Darwin 的 21 个系统 Cone 与 runtime。
- workspace Clippy、149 项 M57／导入／既有 codec 回归、866 项 HIR 元数据测试通过。格式升级的固定向量已按实际编码更新，17 项 profile 检查通过。
- 完整 core consumer 的 46 项原生测试通过。JSON Cone 首轮 79 项中 77 项通过，两项新增 inspector 预期漏写泛型实参，修正后 3 项继承测试定向复验全部通过。编码、解码、inspector、companion、嵌套实参、generic 转发与方法引用均有原生覆盖。
- 跨 Cone ODR 原链接与逆序重链接运行通过，包含 provider 生成的方法、consumer 继承合成、固定实参及两个 library consumer 共用的实例。14 个条件接口诊断 fixture 通过；额外导入检查确认未知条件和不成立条件不能调用生成成员。
- 类型描述／文档的条件展示、其余效果组合、定向 GC 与 Linux 完整验收仍未完成。

## 13. 具体类型描述与条件注解

- TypeInfo 按原顺序追加适用的直接条件接口，基类关系保持独立；条件属性在替换类型前过滤，避免展开不存在的 companion。保留原 PropertyRef、声明序号、存储与构造关系，本地和导入属性均按源码顺序排列。
- 接口表、成员具体化及描述按条件 id 与完整具体实参复用判定结果。实现块目标和 computed property／accessor 的注解进入既有 type-use／声明注解通道；互斥的同一接口目标保留各自注解，未增加产物字段或 runtime ABI。
- workspace Clippy、49 项 M57／静态描述 HIR 回归及完整 core consumer 的 49 项原生测试通过。3 项新增原生测试覆盖 class／struct／enum／companion、条件真／假、投影、注解及过滤后的稳定序号，描述不执行测试中会抛异常的 getter。
- ODR fixture 的原链接与逆序重链接均通过，验证导出的条件接口、property/getter/type-use 注解、泛型描述函数及 consumer 自行查询的一致结果。generic 文档展示、其余效果组合、定向 GC 与 Linux 完整验收继续保留为未完成项。

## 14. Generic 文档中的条件能力

- 条件接口关系、普通／companion 块成员、accessor 与合成方法展示完整适用条件。继承展示使用原 owner 的完整实参，保留 `Wrapper<T>.Companion` 等条件主体；未知关系不隐藏，已知不可能的继承成员排除，互斥条件下的同一目标保留不同关系。
- 文档沿用现有声明与成员查询，不创建实现块页面或 callable 身份。条件事实仅用于文档查询副本；导入文档前后的普通类型集合与机器调用集合保持一致。
- workspace Clippy 与 29 项文档回归通过，追加互斥关系检查后两项 M57 本地／导入测试定向复验通过。首次新增测试选中了接口声明的同名方法、且 provider 构造器未标 public，修正测试后闭合。
- 正式 `scoop doc` 成功生成 ODR provider 与 root 的文档；核对合成 encode 展示 `Envelope<T>.Companion : ValueEncoder<Envelope<T>>`，保持完整构造实参。其余效果组合、定向 GC、查询诊断边界与 Linux 完整验收继续实施。

## 15. 负向条件事实与循环诊断

- 复用既有约束不可能性判定，排除互斥 kind、不相关 class 上界、已知负向接口事实及缺失 companion 的条件实现；未知域仍不能支持 M55 的负向事实。负向查询的重复或扩张依赖保持未知，有限递减查询可以完成。
- 正向循环诊断保留实际查询链，并定位到本地原声明文件或依赖 Cone 的原源码；相同来源、位置和具体查询链的诊断不重复报告。导入声明沿用已有来源信息，不改变产物格式。
- workspace Clippy、51 项 M55／M57 定向检查、新增 core consumer 原生用例、14 个条件接口诊断 fixture 及跨 Cone 循环诊断 fixture 通过。新增 fixture 同时覆盖本地跨文件、导入和参数扩张循环；首次发现的重复诊断已修复并复验。
- 其余效果组合、定向 GC 与 Linux 完整验收仍待完成。

## 16. 接口视图与装箱的 GC 验证

- 新增定向 GC fixture，使用同一 release 构建分别运行移动 GC 与 minor GC；普通 Cone 测试不启用 stress。
- 包含引用字段的条件 struct 装箱后，通过 Any、父接口、子接口、成员引用及拆箱读取保持正确；条件 class 的继承 override、条件关闭后的字段、NoGC 到 Managed slot 适配均保持原行为。跨挂起 frame 保存接口视图，恢复后再次 GC 并读取原字段。
- 本机 fixture 三个进程全部通过，复用现有字段扫描、协程及接口表实现；未增加 runtime ABI 或 GC 特殊路径。完整本机和 Linux 验收继续实施。

## 17. 参数作用域、成员要求与效果组合

- 新增四项 core consumer 测试，覆盖 inner／nested／局部类型和匿名 object 的参数、捕获及条件真／假；requires 转发、显式 equality、NoGC 到 Managed slot 调用及引用、suspend 接口调用和成员引用保持既有语义。
- 组合验证发现 M55 补充递归检查把无关类型描述生成依赖误判为多态递归。现保存要求展开边，只在包含这类边的调用链检查改变完整实参的递归；普通有限类型描述与 requires 可以共存，真正的要求递归仍拒绝。先补充实现规范，未改变产物格式或引入查询预算。
- workspace Clippy、完整 core consumer 的 54 项原生测试、55 个 M55 requires fixture 及 16 个条件接口诊断 fixture 通过。新增诊断覆盖 suspend／Unsafe 契约和 NoGC 正文，以及显式条件 equality 不可用时不得派生兜底；条件方法尚未实例化也检查声明错误。
- 本机完整 Rust 与 CLI 回归、Linux 完整验收继续推进。

## 18. 完整验收

| 验证范围 | 已完成结果 |
| --- | --- |
| 格式化与 workspace Clippy | Darwin／Linux 通过；最后的编译器变更之后只修正 fixture 与文档 |
| workspace Rust 测试 | Darwin 5,194 项、Linux 5,224 项通过，均无失败或忽略 |
| fixture runner | 两端各 55 项通过 |
| Darwin 全量 CLI | 4,206 项通过、17 项目标不适用；由中断前结果、补跑及定向复验合并 |
| GNU 全量 CLI | 4,190 项通过、33 项目标不适用；由初始全量、剩余批次及定向复验合并 |
| GNU 库行为 | 五个内置库入口及补充 21 个 Cone 通过；补充测试计 1,127 项通过、6 个 PTY 子入口由集成测试另行启动 |
| musl 定向验收 | 30 个 fixture、33 个变体、60 个进程通过，覆盖 M57、五个内置库入口、移动／minor GC、原生 ABI、跨 profile 与重链接 |

失败闭合：

- 三项继承访问域诊断由合成参数名 T0 改为原参数名 T；两端各 18 个相关 fixture 复验通过。两项 core 非法根声明的旧快照只更新 M62 源码调整后的 18 处字节位置，两端复验通过。
- 原生动态链接快照同步基线已有的 multiple strong definitions 文案。效果冲突专测改用相同 Scoop ABI 的 allocation overflow，只制造 Managed／NoGC 差异；三目标复验通过，保留失败不覆盖输出断言。
- small-values 三目标快照同步已有优化及外部函数编号变化，声明、布局与 native 调用 ABI 不变；SSA roots 同步拷贝传播后的实际根位置。原生执行、跨 profile 消费与重链接通过，三目标普通验收模式均通过。
- Darwin 代理用例首次在 SOCKS 截止时间场景失败，同一程序完整复验通过。GNU JSON 首次超过 600 秒，单独复验约 477 秒通过；未修改源码或放宽断言。

Darwin 全量在 3,204 项通过后因磁盘耗尽中断，保留结果名单并清理已完成临时目录，仅补跑受影响及未完成用例。GNU 在补充库测试结束后切换并行度，保留初轮 2,012 项通过结果，剩余 2,174 项全部通过，另四项首轮失败按上述定向复验闭合。两端分别清理约 10.9／26.8 GiB debug 产物，保留 release 工具、已准备的 sysroot 与共享 Cone cache；原始日志及合并结果留在仓库外。

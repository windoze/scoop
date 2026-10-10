# M35 实施记录

基线：`75a534ef2`，分支 `review-20261010`；设计见 [DESIGN.md](DESIGN.md)。

## 批次

| 批次 | 状态 | 实际交付与验证 |
| --- | --- | --- |
| M35-1 规范与 case 迁移 | 已完成 | 显式 case；core/JSON/fixture/Rust 源码迁移；29 个定向 CLI fixture 与模式 Rust 测试通过 |
| M35-2 普通 when | 已完成 | subject/解构、普通条件与 case、多条件短路、guard、smart cast、落空与覆盖；42 个 M35 CLI fixture 严格通过 |
| M35-3 跳转与 Elvis | 已完成 | return/throw 表达式与合法返回目标；Elvis 正常结果合并；23 个定向 CLI 用例严格通过 |
| M35-4 完整尾随调用 | 已完成 | 分组/换行后缀、最后形参映射、所有调用入口；21 个新 CLI fixture 严格通过 |
| M35-5 注解数组 | 已完成 | 静态数组、完整参数类型、共享读写与重导出；40 个定向 CLI fixture 严格通过 |
| M35-6 组合与回归 | 本机完成，Linux 待验收 | 组合修复与本机完整回归已闭合；Rust 5,437 项、CLI 3,049 项通过，16 项不适用 |

## 验证与构建目录

每批先格式化与 lint，再运行相关 Rust 测试和文件 fixture；最终执行一次完整 workspace 与 CLI 验收。Linux GNU/musl 使用 `nuc12:~/repos/scoop`，只记录实际运行结果。

2026-10-10 开始时本机 target 约 21 GB（debug incremental 约 13 GB）；LLVM 使用已配置的 LLVM_SYS_221_PREFIX 指向的 22.1 工具。

M35-1 开发验证：解析器 459 项及 1 项 doctest、HIR 模式 34 项与 M22 103 项、MIR 模式 10 项通过；文件 fixture 验证进行中。2026-10-10 清理 debug incremental 后 target 从约 25 GB 降至 16 GB，保留已构建工具与依赖。Linux 已确认基线相同，按用户要求待本机全部完成后统一验证。

## M35-1 交付

- 同步语言与实现规范，保留原章节引用，路线图登记 M35。运行时合同未发生变化。
- when parser 独立为 108 行模块，case 仅在模式头部作为上下文关键字。原模式矩阵、guard 与类型检查保持；新反例锁定缺少前缀的位置与迁移提示。
- 迁移 233 个现有 Scoop 源码文件、Rust parser 测试与两处 fixture 动态源码。core、JSON、各历史里程碑中的绑定/模式保持原行为，声明位置不增加 case。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。`cargo test -p scoop-parser` 通过 459 项及 1 项 doctest；HIR 的 m4::when 34 项、m22_ 103 项，MIR patterns 10 项通过。
- 构建 debug/release 三个 CLI；通过 29 个定向文件 fixture，包含新 case 独立/组合/negative、M4、M22 完整整数域与递归覆盖、M26 字符、M30 浮点模式。新 fixture 及受警告诊断中断的 golden 已再次严格比较。完整记录见 [case-darwin.json](validation/case-darwin.json)。
- 首轮失败仅为添加前缀后的源码偏移与受其阻断的 golden，已按实际结果修正并仅重跑失败/新增用例。优化版编译器使相同组合 fixture 从约 26 秒降到约 6 秒。全部仓库回归留到功能完成后统一执行。

## M35-2 交付

- AST 明确区分无 subject、表达式与 val 声明，arm 区分普通条件、case 和 guarded else。普通比较复用已有 equals、is、contains；多条件按顺序短路。头部解构复用声明 binding 计划，保存完整 subject 并只求值一次。
- HIR 显式表示可选 subject、predicate/case/always 条件与合法 Fallthrough；MIR 沿用原模式决策及控制转移。分支类型事实按成功/失败路径合并，guard 失败不会误推主条件失败；值分支复用正常完成分析与期望类型合并。
- 默认表达式与泛型共享 body 格式贯通新 when 结构，cross-cone HIR interface 升至 70，type semantics 升至 28；更新规范与格式固定向量。旧 golden 中的 HIR Debug 字段按结构机械迁移，完整历史回归仍待最终验收。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。定向 Rust 验证：parser 463 项及 1 项 doctest，HIR m4::when 34 项、m22_context 15 项、m6 64 项与 flow 12 项，MIR patterns 10 项，共享 default body 96 项与 slib profile 54 项。
- 39 个新增普通 when fixture（含 33 个独立反例）及 3 个 case fixture 严格通过，覆盖求值顺序、精确错误位置、AST/HIR/MIR/LIR、挂起恢复、异常/finally、moving GC，以及去源码跨库 debug/release 消费和独立链接。记录见 [when-darwin.json](validation/when-darwin.json)。
- 第二次清理 debug incremental 约 9.2 GB，target 降至约 18 GB；保留依赖与已构建 CLI，定向测试复用 release 工具及 fixture 缓存。
- 额外复验 M26 Char 的默认值/泛型跨库用例：更新 case 前缀导致的精确源码位置后，6 个 golden 严格比较与 7 个进程步骤通过，包括移除源码消费、重链接和 moving GC。

## M35-3 交付

- parser 的语句/表达式入口共享 return/throw 解析，裸 return 正确停在换行与嵌套 delimiter。返回值使用当前 callable 的返回类型，lambda、default、初始化器、表达式体函数与非法 label 均有独立反例；初始化器内的匿名函数仍返回自身。
- Elvis 复用正常分支结果合并，允许子类型、最小上界与 Nothing 右侧。新增明确的 Unreachable/Nothing 表达式，跳转复用原 transfer，全部终止的控制表达式不再制造隐藏结果 local。废除只服务旧 Elvis 的 desugar_option/ElseBranch，跳转与 Elvis 分别拆为小模块。
- HIR 具体化、共享默认值/泛型 body、格式读写、访问遍历与 MIR CFG 保留终止语义；cross-cone interface/type semantics 递增至 71/29。更新固定编码和指纹测试。
- fmt 与 workspace clippy 通过；parser 466 项及 1 项 doctest、HIR m3 37 项、m11 24 项、m22 103 项、共享表达式 wire 11 项、slib profile 21 项通过。
- 19 个新 fixture（5 组运行组合、14 个独立反例）与 4 个定向历史/when 回归共 23 项严格通过，26 个 variant、54 个进程、40 个 golden。覆盖提前返回跳过后续实参/default、数组/tuple、短路、Some/None 一次求值、Context/finally、挂起恢复、moving GC、跨库 debug/release 与去源码独立链接。详见 [jumps-darwin.json](validation/jumps-darwin.json)。
- 第三次清理 debug incremental 7.8 GB，target 从 26.5 GB 降至 21.5 GB，保留依赖及 debug/release CLI。

## M35-4 交付

- AST 保留尾随 lambda 来源，解析器区分追加实参与分组后的返回值调用，支持省略圆括号、显式泛型与 suspend lambda。分号、return 换行、when 单语句 arm 边界保持；重复外置 lambda 给出明确诊断。
- 每个候选把尾随 lambda 固定映射到最后形参，括号内实参沿用已有默认值、命名和 vararg 规则。函数值保持精确元数；成员、extension、safe call、super、构造、别名与 operator invoke 使用同一来源规则。HIR 决议后无需新增 MIR/ABI 字段。
- 将表达式后缀、place 与尾随调用拆成独立模块，主 parser 表达式文件从约 600 行降至 270 行；新增模块均不超过 176 行。
- fmt 与 workspace clippy 通过；parser 470 项及 1 项 doctest、参数映射 8 项、HIR m11 24 项通过。
- 21 个新 CLI fixture 全部严格通过，共 22 个 variant、40 个进程步骤、28 个 golden。覆盖 16 个精确诊断反例、求值顺序、safe call、挂起成功/失败、moving/minor GC、debug/release 去源码跨库消费和独立链接。记录见 [trailing-darwin.json](validation/trailing-darwin.json)。

## M35-5 交付

- annotation class 接受合法标量的一维 core Array，普通 alias 展开后验证实际声明身份。数组支持空值、顺序/重复元素、尾逗号、位置/命名实参与默认值；元素沿用整数、浮点、Char、String、Boolean 与同类型 const 引用规则，并保存独立诊断位置。
- 新增封闭的 CanonicalAnnotationValueV1 Scalar/Array 静态表示；普通 const 值域保持标量。参数改用完整 SignatureTypeKey，保留 Array generic owner 与元素身份。静态描述查询、声明/应用、默认值与 A→B→C 重导出共用完整数据，不创建运行期数组。
- 共享 reader 在原边界校验类型、容器与元素 payload；外部类型引用复用既有 signature walker。仅 cross-cone interface 升至 72，type semantics 保持 29，MIR/LIR 与 runtime ABI 没有变化；格式向量与 profile 指纹已更新。
- fmt 与 workspace clippy 通过。parser 471 项及 1 项 doctest、数组 HIR 2 项、原标量注解 2 项、静态描述 7 项、wire 2 项、annotation reader 4 项、profile 21 项通过。
- 39 个新增 CLI fixture（含 35 个独立反例）及迁移后的 M29 MutableArray 反例，共 40 项严格通过。正例覆盖全部标量元素种类、空数组、默认值、实际 JSON 编解码、moving/minor GC、debug/release 去源码重导出和独立链接。缓存用例检查参数类型、默认值、元素内容变化引起的实际缓存键与产物变化，并验证暖缓存命中。详见 [annotations-darwin.json](validation/annotations-darwin.json)。
- 第四次清理 debug incremental 约 9.5 GB，target 从约 31.4 GB 降至 24.6 GB；保留已构建 CLI 与依赖。严格验收只重跑修正的缓存用例，复用其余 39 项通过记录。

## M35-6 组合修复

- 路径事实改用稳定 BindingId 保存多个已知类型约束；合取保留全部约束，替代路径保留共同事实，按声明身份处理成员与继承去重，普通重载选择和歧义规则保持。类型事实使用有序映射，避免推导顺序影响 stage 输出。
- 成员、属性读写、扩展、invoke 和 callable reference 选择合适的已知类型视图；一次求值的更新 receiver 保留事实。闭包、匿名函数、局部函数和构造参数沿原存储类型捕获，HIR/MIR 读取时使用现有装箱、拆箱与引用转换。
- 局部默认值展开保留替换表达式的已知类型，修复接口视图丢失导致的指针/接口布局不一致。新增模块各不超过 130 行；不新增语言 intersection type、共享格式版本或 runtime ABI。
- fmt 与 workspace clippy 通过；HIR 类型转换 64 项、闭包/引用 24 项、默认值相关 98 项通过。10 个独立/组合/反例 fixture 严格通过，11 个 variant、30 个进程步骤、24 个 golden；包括挂起成功/失败、moving/minor GC、跨库 debug/release 去源码消费和独立链接，见 [combinations-darwin.json](validation/combinations-darwin.json)。
- 第五次清理 debug incremental 约 3.0 GB，target 从约 27.6 GB 降至 25.7 GB，保留依赖与已构建 CLI。

## M35-6 本机完整回归

- Rust 工作区覆盖 5,437 项，公共 fixture runner 44 项通过。初跑的两个失败分别是旧尾随 lambda 用例和添加 case 后的警告列号；修正后只定向复验，已通过的 crate 不再重跑。两项较重 driver 用例另以 release 测试二进制验证，重复通过不计入总数。fmt 与 workspace clippy 通过，见 [Rust 验证记录](validation/rust-darwin.json)。
- 正式 `--all` 发现并执行 3,065 项：初跑 2,714 项通过、335 项需要迁移、16 项不适用。随后按未闭合结果选择 89、174、74、1 项，复用前面的通过记录；最终 3,049 项通过、16 项不适用，没有未闭合失败，覆盖 3,289 variants、14,568 processes、12,407 次阶段/链接计划快照检查。131 个 M35 新 fixture 在正式全量中全部严格通过。完整来源见 [CLI 覆盖记录](validation/cli-coverage-darwin.json)。
- 快照迁移执行仍保留原有诊断、退出、stdout/stderr、运行、GC 和链接断言。保存每次实际规范化输出，随后对最终预期严格比较：四批共 2,727 次观测全部一致。最后一批涉及的 35 份共用快照没有变化，之前通过的其他 native 用例记录仍然有效。没有为了汇总全绿重新执行整个仓库。
- 旧示例的迁移保留原测试目的：M23 的非末位 lambda 实参移回括号；M18 的泛型 invoke 将 lambda 参数放在最后，继续覆盖默认参数与尾随调用；M8 的显式 return 改用块体函数。两个 Array 反例保留主要类型错误，并更新另一构造候选对尾随重复绑定的诊断。其余诊断迁移仅改变 case 带来的源码位置。
- 抽查并核对 AST 的尾随来源、HIR 的源码位置/when 表示、MIR 的正常路径结果赋值及 LIR 的局部编号变化。新增实现文件最长 244 行，新增测试文件最长 246 行，没有新增 TODO 或 unimplemented 占位。
- 第六次清理 debug incremental，记录目录大小约 7.6 GiB；target 现约 31.8 GiB，保留已链接工具和依赖。前一次使用符号链接缓存的环境失败已终止，不计入正式验收；上述全量及复验均使用普通缓存目录。
- 本机闭合后，将同一实现提交同步到 nuc12。Linux 选择 M35 新功能、变更源码的 fixture、实际 Darwin 迁移、MIR/LIR 变化与平台专用用例，共 600 项；GNU 适用 580 项，musl 适用 583 项，执行结果另行记录。

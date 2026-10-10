# M35 实施记录

基线：`75a534ef2`，分支 `review-20261010`；设计见 [DESIGN.md](DESIGN.md)。

## 批次

| 批次 | 状态 | 实际交付与验证 |
| --- | --- | --- |
| M35-1 规范与 case 迁移 | 已完成 | 显式 case；core/JSON/fixture/Rust 源码迁移；29 个定向 CLI fixture 与模式 Rust 测试通过 |
| M35-2 普通 when | 已完成 | subject/解构、普通条件与 case、多条件短路、guard、smart cast、落空与覆盖；42 个 M35 CLI fixture 严格通过 |
| M35-3 跳转与 Elvis | 已完成 | return/throw 表达式与合法返回目标；Elvis 正常结果合并；23 个定向 CLI 用例严格通过 |
| M35-4 完整尾随调用 | 已完成 | 分组/换行后缀、最后形参映射、所有调用入口；21 个新 CLI fixture 严格通过 |
| M35-5 注解数组 | 待实施 | 静态值、完整类型、共享格式 |
| M35-6 组合与回归 | 待实施 | 适用平台与正式 CLI 验收 |

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

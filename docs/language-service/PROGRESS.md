# 公共语言服务基础实施与验收

实施开始：2026-10-22。实施基线：`033dd9e1d`；功能完成提交：`59f2aad7e`；Linux 验收输入修正：`009fca946`；产物／缓存期望与诊断修正：`f4ae78f8c`、`e3c51ac8d`。分支：`codex/language-service-foundation`。

状态：已完成，A～E 功能与验收均已闭合。按[设计](DESIGN.md)交付 A～E；各功能分别完成格式、lint 与对应测试后独立提交。本文保留当前能力、验证范围和必要限制，逐批过程可由 Git 历史查看，原始日志与逐次报告保留在仓库外。

## 已交付能力

| 范围 | 完成交付 |
| --- | --- |
| A：共同源码与固定查询 | `scoop-source` 的不可变文本、byte range、行索引与诊断；完整文件／类型／函数体入口；HIR 数据通道中的 typed 查询与来源；M47 GenerateWith 规划、双向查询、真实 generator 进程和生成正文的普通检查；build／run／test 传递配置并按实际工具输入更新缓存 |
| B：可恢复语法 | 原 lexer／grammar 构造保留全部原文的 rowan CST，错误与零宽缺失项、规则局部恢复和完整 AST 投影；probe 同步回退语法节点、诊断、文档附着及局部编号；语法错误不能进入成功编译 AST |
| C：首个 LSP | 不依赖 LLVM 的 stdio 服务器，生命周期、文档同步、UTF-8／16／32、语法诊断、大纲、折叠、选择范围；多根、目标／用途配置、编辑 manifest、文件通知和普通源码／预编译依赖输入 |
| D：编辑态语义 | 复用原声明、签名、名称、类型、重载及 effect 规则；独立错误隔离、正文与恢复输入查询、hover、定义／类型定义、receiver 成员补全、完整／半成品调用签名帮助、M60 文档；覆盖访问器、默认值、构造初始化、运行时初始化、常量与静态值 |
| E：索引与编辑 | 工作区符号、真实引用及类型／成员实现索引；构造器、类型／透明别名、精确 import、显式 alias、重导出和运算符的实际来源；prepareRename／rename 的多文件版本与绑定冲突检查；固定缺失标点的 quickfix／resolve；按实测热点优化正文定位和成员候选计算 |

M47 使用固定编译视图，LSP 使用可更新的输入宿主。它们共用事实与算法，协议和会话生命周期分别适配。M47 的 codec 迁移仍按[原设计](../milestone47/DESIGN.md)单独推进。

## 正确性覆盖

- 源码与语法：Unicode、反引号、注释、CRLF／CR／LF、EOF、未终止 token、缺失／多余分隔符、坏参数／成员／语句、嵌套插值、泛型消歧以及连续插入／删除；正确和错误语法逐字节保留原文，严格入口拒绝尾随与恢复错误。
- 语义边界：坏文件头、导入、签名、字段、别名、继承和布局循环按实际名称或类型依赖隔离；独立声明保持可查询，未知候选不能触发错误回退，失败区域不伪造成功类型或 HIR。
- 正文组合：原词法作用域、泛型 binder、闭包捕获、smart cast、分支／循环、模式、异常与清理；未知操作撤销无法继续证明的 flow 事实，失败与取消不提交试探结果。
- 来源与编辑：当前及源码／预编译依赖、重载、局部遮蔽、构造名称、property／参数、alias、重导出、默认值和运算符；引用依实际绑定形成，rename 在独立候选快照中经普通前端检查，并比较修改前后绑定。
- 输入与生命周期：打开文本覆盖磁盘、顺序增量修改、修改／关闭恢复、manifest／依赖／目标／用途更新、分段与连续 stdio 帧、取消响应、旧诊断丢弃、空诊断清除及 resolve 的版本变化。
- M47 与原生闭环：真实 generator 查询并返回普通／泛型正文，错误与中文片段按实际来源定位；生成代码保持原访问、类型和 effect 检查，经过 `.slib` 消费、链接与运行验证。

## 验证结果

| 环境与范围 | 结果 |
| --- | --- |
| 本机 macOS／AArch64 | 每批 Rust 格式化、全 workspace／all-targets Clippy（拒绝 warnings）、必要的 Ruff 0.16.10、定向 Rust 与真实 stdio／正式 CLI 验证通过；最终性能优化后的完整 release LSP fixture 通过；最终 27 项正式 CLI 定向复验全部通过 |
| Linux GNU／x86_64 工具与资源 | Rust 1.99.0、LLVM 22.1.2；独立 checkout 与 Cargo target；按维护脚本准备当前 text／compression 资源、sysroot 和 terminal-host，普通 fixture 共享预编译资源与 Cone cache |
| Linux Rust 与 Python 检查 | 格式化、全 workspace／all-targets Clippy（`-D warnings`）及 Ruff 0.16.10 均通过；最终 fixture 修正再次通过 Ruff format/check |
| Linux workspace Rust 测试 | `cargo test --workspace --locked --no-fail-fast` 通过：5,864 项单元／集成测试与 2 项 doctest，零失败、零忽略 |
| fixture runner 公共规则 | 55 项通过 |
| Linux M47／LSP 正式用例（全量子集） | 真实 LSP 工作区 stdio 用例通过（35.11 秒）；8 项生成器用例通过，覆盖双向查询与源码、测试用途、错误／完整性、日志、缓存和预编译消费 |
| Linux GNU 正式 `--all` fixture | 完整执行 4,209 项：首轮 4,151 通过、25 失败、33 不适用；修正后 27 项定向复验全部通过。结合首轮与复验，4,176 项适用用例全部闭合；未重复全量 |

Linux Rust 首轮发现 C++／musl 链接测试的普通 Cone 输入缺少所需 package，编译阶段提前失败。`009fca946` 补齐 `package dev.m33`，静态与动态链接的原诊断断言在定向复验及随后全 workspace 测试中通过。

CLI 首轮的 25 项失败分别来自压缩 manifest 首项名称、artifact summary 缓存目录、恢复诊断／声明失败隔离后的消息与顺序，以及 CLI 人类诊断的路径行列期望。两批提交只修正这些期望，保留主错误、精确位置、实际运行与复用断言；context 匿名函数负例同时改用规范要求的块正文，使其只验证不允许的 context 前缀。本机与 Linux GNU 各执行 27 项定向复验，首轮失败全部闭合。验收修正未改变语言或链接规则。

公共基础实施区间（`033dd9e1d`～`0842bec3f`）未修改 MIR、MIR lowering、LIR、LIR lowering 或 codegen，该轮 Linux 全量只选择 `x86_64-unknown-linux-gnu`。Linux 专用 Rust 测试包含其原有 GNU／musl 链接覆盖；按用户要求，不再执行 musl 全量 fixture。运行中的源码、工具和产物保持存在，日志与报告未提交到仓库。

## 性能与当前边界

真实生产项目的首次打开、编辑诊断、补全、引用、取消及峰值内存，和严格 parser 的实施前后差异，见[性能记录](PERFORMANCE.md)。优化按实际请求定位正文并计算所选成员候选；正文诊断和引用不再提前枚举无关候选，已完成查询复用同版结果。

语法仍整文件重解析，未变化文件直接复用；受影响 Cone 的语义和引用索引保守失效。没有局部重解析或正文级增量类型检查，大项目首次打开和引用索引仍有完整前端分析成本。

rename 当前要求所选项目的必要源码及语义分析完整；存在不可证明的引用或绑定时明确拒绝，缺少源码的位置不猜测。code action 仅修复已确定的缺失固定标点。semanticTokens、inlayHint、格式化和 call／type hierarchy 属于后续能力，服务器没有声明它们已实现。

启动与编辑器配置见[服务器说明](../../compiler/lsp/README.md)。

## 合并 origin/main

接入 `f3f8ace69` 的 M61 文本后端、Regex 与 M62 优化。上述公共基础全量验收截至 `0842bec3f`；本次合并的验证单独记录，不将此前结果视为合并后重新执行的全量。

- HIR 正文导出保留查询重构后的输入借用，并接入 M62 的普通小函数正文选择；另一处冲突仅为已修正 Linux 链接测试源码的空行差异。
- M47 按依赖实际提供的正文区分 Available／NotPublished。两项用例改为同时覆盖非泛型小函数的完整正文与包含字符串的未发布正文，继续保留原函数身份、来源和泛型正文检查。
- macOS 格式化、全 workspace／all-targets Clippy 及 Ruff 0.16.10 通过；受影响四个 crate 初轮 2,907 项通过，两项旧正文可用性假设修正后，15 项语义查询测试全部通过。
- 合并后的 release 工具和 macOS sysroot 构建通过；15 项正式 CLI 组合用例全部通过，包含 8 项 M47 生成器、LSP 工作区、跨产物泛型、普通跨 Cone 内联及 intrinsic，覆盖 35 个运行变体。

原始合并验证日志仍保存在仓库外；未重复执行 GNU／musl 全量。

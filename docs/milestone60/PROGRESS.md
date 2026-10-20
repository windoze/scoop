# M60 实施记录

状态：实施中。开始日期：2026-10-21。基线：`abf17c32`；分支：`codex/m60`。范围与完成条件见 [DESIGN.md](DESIGN.md)。

## 分项状态

| 分项 | 状态与验证 |
| --- | --- |
| 设计与路线图 | 已提交；已核对本地引用、M52 当前状态及源码布局 |
| M60-A：文档前端与注解 | 文档注释、Markdown／tag、普通 core 注解及 typed HIR 文档视图已完成；正式命令诊断随 B 接入 |
| M60-B：单 Cone 闭环 | 已完成渲染输入、single-Cone 协议／driver、HTML／源码页／搜索渲染；CLI 与继承关系展示继续实施 |
| M60-C：多 Cone 与更新复用 | 待实施 |
| M60-D：实际库及跨平台验收 | 待实施 |

## 工作与验证约定

临时文件、测试 TMPDIR 和原始日志放在仓库外的本次工作目录，不使用 `/tmp`。按功能先格式化／lint，再做本机定向验证并提交；Linux 使用 nuc12 的 `~/repos/scoop`，在本机功能闭合后完成全量验收。开始时本机 target 约 3.4 GB；阶段间检查占用并清理不再使用的构建结果，保留仍用于验证的配套工具和 sysroot。

## A1：文档注释保留与附着

同一 lexer 保存块／行文档的原始文本和 byte span，普通注释与字符串保持原行为；parser 在实际声明、参数、variant、构造器及 accessor 语法入口建立精确关联，悬空片段保留为后续文档诊断输入。AST 的关联是文件内语法位置，后续 HIR 仍须绑定真实 typed 声明。长主构造器解析移入现有 constructors 模块。

组合测试发现无 body 的 class 会误吞下一声明的可见性／注解；修正换行处主构造器前缀的解析边界，保留跨行显式 constructor，增加独立回归。

本机 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；AST 15 项、parser 520 项及 parser doctest 1 项通过，其中新增 10 项文档独立／组合测试。正式 `scoop doc`、文档 warning、HIR 投影及 Linux 验收尚未完成，不从 parser 结果推定完成。

## A2：普通 core 文档注解

新增 `DocHidden` 和可重复的 `DocAlias(String)`，复用普通注解的 Target、静态参数、use-site、alias、re-export 与产物保存。Intrinsic 类型及函数允许实际 core 文档注解；其他普通注解仍被拒绝，原有 NoGC／Unsafe 要求不变。为真实 Int、String 和 currentSourceLocation 添加代表性说明与搜索别名。

本机 fmt／workspace clippy 通过；47 项注解相关 HIR 测试通过（包含新增 6 项），11 个三目标通用 CLI fixture 在 Darwin 通过，共 14 个进程。用例检查错误消息与准确 byte span，并实际编译／运行普通注解、同名用户注解及跨 Cone 重导出。配套 release 命令已重建，开发 sysroot 的 19 个库与 runtime 已按维护脚本完整更新。Linux 验收留到功能闭合后进行。

清理约 729 MiB 本阶段不再使用的 MIR／LIR／codegen 增量缓存，保留当前前端测试及配套命令。原始日志与 fixture 工作区保存在仓库外。

## A3：Markdown、tag 与原始位置

新增 `scoop-doc` 文档 stage；正文、tag、待绑定路径、跨 tag 的显式 Markdown 引用定义及 byte 映射使用 AST 纯数据。仅消费 lexer／parser 保留的片段，不重新发现声明。pulldown-cmark 负责 Markdown 结构，代码、图片和显式 URL 不成为符号候选；反引号标识符中的点与空格保持原意。未知／缺参 tag、非法符号语法和悬空片段产生带原始位置的文档 warning，语义目标仍留给 HIR 绑定。

引入前已核对 pulldown-cmark 0.13.4 的维护状态及 Rust 1.71.1 要求，低于 workspace 的 1.85 基线；仅启用实际需要的 HTML 功能。MiniJinja 3.0.1 的 Rust 1.70 要求也已核对，待渲染阶段实际引入。

本机 fmt／workspace clippy 通过；14 项文档语法测试与 15 项 AST 测试通过，覆盖 Unicode／CRLF、装饰前缀／缩进、连续与分隔片段、8 类 tag、代码／HTML 边界、显式引用优先、quoted path、精确 warning span。此阶段尚未提供 `scoop doc` CLI，正式文档 warning fixture 随 CLI 闭环接入。

## A4：HIR 文档视图与引用绑定

CurrentConeSources 接收与选中源码身份、顺序一致的整理后注释；正常 HIR 完整成功后，Output 附带独立文档视图与 warning。声明沿原 AST／typed 目标关联，保存作者签名片段与已完成的推断类型，覆盖主／次构造器、字段／property／accessor、variant payload、annotation 参数、泛型 binder、context、扩展 receiver 和源码可见合成方法。局部函数及其子项不进入视图；内外泛型 binder 保持各自身份，主构造 property 说明不复制为参数说明。

文档链接复用正常 package、import、成员、继承及 direct dependency 查询，保留完整 overload 集、alias 身份及原文件作用域。tag 绑定实际目标，直接说明优先；缺失、歧义、不适用与重复描述产生准确 SourceIdentity／byte span 的文档 warning。查询使用独立语义副本，不增加普通机器调用选择；注释上下文复用现有写时复制存储，避免重载探测重复复制全文。

本机 fmt／workspace clippy、16 项 HIR 文档定向测试通过，包括真实 core 全部源码的文档请求、跨 Cone alias／成员、同名导入歧义、Unicode／CRLF、M55 条件、嵌套 binder、合成 equals、局部声明排除、普通构建无文档 warning，以及函数体错误仍拒绝输出。真实 core 的 DocHidden／DocAlias 按其 annotation identity 返回，已有说明正确关联且没有文档 warning。测试修正了 struct 字段与自动访问 property 的重复候选，并验证 context 参数直接归属 logical property。

本阶段未提供 CLI、持久化渲染输入、可见性筛选、继承关系的页面展示或 HTML；这些继续在 B／C 完成，不能从 HIR 测试推定站点验收已通过。清理约 498 MiB 不再使用的后端增量缓存，保留前端缓存、release 配套工具及开发 sysroot。Linux 完整验收仍在本机功能闭合后执行。

## B1：可缓存渲染输入与展示筛选

`scoop-doc` 直接消费完成后的 Export HIR、文档视图和已读源码，生成可序列化的展示输入；保留原签名、原 binder 名称、分段 package、源码／warning 位置、绑定链接及公开 re-export 的真实目标。路由通过现有 annotation target 的持久身份投影统一生成；参数使用实际 owner 和位置，派生 equals 使用 nominal owner 与派生角色，object 的类型／值 binding 指向同一源码条目。输入顺序与 arena 变化不改变对应路由。

公开筛选使用 effective lookup／inheritance domain，支持公开 open 类型的 protected 成员，以及 sealed 类型经公开 open 后代提供的扩展点；私有容器不泄漏公开子项。property 保存 getter／setter 的实际可见性。DocHidden 作用于真实目标及文档后代，字段与 logical property 分开，构造参数及源码原文保留；DocAlias 按 core 身份收集、合并重复关键词，并对空白关键词保存准确 warning。用户同名注解不产生文档效果，private 展示模式不取消隐藏。

本机 fmt／workspace clippy 和 24 项 HIR 文档测试通过（新增 8 项投影组合测试）；真实 core 全部源码完成展示投影且无文档 warning。覆盖 JSON 输入往返、Unicode／CRLF、泛型推断类型、M55／context、重载及稳定地址、private setter、sealed／protected、精确 use-site 与注解 alias。测试修正了 inheritance domain 自身 subclass 约束的递归解释。清理四份已被替代的前端增量缓存，约 5.16 GiB 文件逻辑大小，保留当前验证缓存及 release 工具。

此提交提供站点的渲染输入，尚未接入 single-Cone 文档协议、CLI、页面目录／HTML／搜索、继承成员页面或增量发布；完整命令与 Linux 验收继续实施。

## B2：single-Cone 文档协议与编译编排

配套协议升级至 12，以闭合的输出选择表达普通产物、仅文档及产物附带文档；成功结果对应携带真实产物摘要、文档输入位置与独立文档 warning。仅文档不使用虚构 `.slib`，不与 observation 混用，限 Production manifest 输入与 AST／HIR dump。

driver 复用完整正常 HIR，在同一次结果中投影文档；仅文档随后结束，联合输出在机器生产成功后发布文档输入。普通编译不整理文档。仅文档使用已有目标注册表的纯选择，不探测 C 工具链或进入 MIR／LIR、native 编译、runtime 和最终链接；依赖仍按原产物合同加载。文档 JSON 通过同目录临时文件替换，正文检查失败保留旧结果。

本机 fmt／workspace clippy 无警告；31 项协议测试及两组 driver 集成测试通过。集成测试以真实 core 验证 Darwin／GNU／musl 三目标的仅文档路径，以普通库验证 Production 排除测试目录、Unicode／CRLF warning 的准确 span、DocAlias、HIR dump、联合与普通编译产物一致、独立 warning 以及正文错误不覆盖旧输入。协议测试覆盖三种输出往返、非法用途／输入／dump 组合和独立文档诊断；同步更新协议 frame 的长度断言。

本次尚未提供用户侧 `scoop doc`、HTML／搜索、站点缓存与发布。正式 CLI fixture 和 Linux 完整验证继续随 B／C／D 闭合。

## B3：全站目录与静态页面渲染

先根据完整 Cone 身份、实际 typed 声明和筛选结果建立页面目录，再生成 Cone／package／类型／函数组、成员及参数锚点、源码行和共享搜索。函数重载保留独立声明地址；跨声明作用域的解析结果生成列出全部可见目标及其签名／所属 Cone 的组页。显示名称与输出路径分开，root 顺序、同名条目和不同 package 分段不覆盖页面。缺少文档目标保留文字；隐藏条目及其 re-export 不进入所选页面和搜索。

引入 MiniJinja 3.0.1；已复核官方包、发布记录和 Rust 1.70 要求。模板显式开启 HTML 转义，Markdown 原始 HTML 按文本呈现；签名、源码、标识符、tag 和 property getter／setter 能力使用实际投影。README 渲染消费调用方提供的资源路由，不自行读取或扫描文件。搜索使用本地静态索引脚本，包含名称、种类、所属 Cone／package、摘要、alias 和 re-export；无 JavaScript 时仍可从导航进入页面。

本机 fmt／workspace clippy 无警告，19 项文档测试通过（新增 5 项渲染组合测试）。验证跨 Cone／overload 链接、隐藏与 private 筛选、源文件转义、成员／accessor 锚点、README 资源和非法活动 URL；HTML parser 检查样例 6 页、46 个本地链接、唯一锚点及资源存在，搜索脚本通过 Node 语法检查。浏览器工具的 URL 安全策略拒绝 `file://`，本次未把实际离线浏览器验证计为通过。

清理 66 份已被替代的增量快照，约 3.59 GiB 文件逻辑大小，保留每个 crate 配置的最新快照。渲染层尚未接入用户命令、README 资源复制、更新记录与原子站点发布；继承／覆写关系的页面内容、真实库站点和 Linux 验收继续完成。

## 源码展示规则补充

按新增要求明确源码原文嵌入生成的 HTML，API 只链接站点内源码页和行锚点；渲染输入没有对应文本时，省略源码页、链接及空源码目录。`.slib` 继续仅保存 SourceRecord 元数据，完整源码与注释正文只进入本地文档输入。正常编译的必需输入缺失仍是 I/O 错误。

本机 fmt／workspace clippy 无警告；7 项渲染测试通过，其中新增两项覆盖嵌入文本、HTML 转义、原始文件地址不进入输出、部分／全部源码不可用。两项 driver 集成测试通过，确认联合生成文档与普通生产的 `.slib` 一致，归档不含测试的完整源码和文档正文。源码不可用用渲染输入直接表达，没有删除、移走或隐藏实际编译输入。

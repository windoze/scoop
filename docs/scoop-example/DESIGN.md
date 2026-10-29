# `scoop example` 外部子命令与示例内容设计

状态：已实现。日期：2026-10-29。命令、首批 24 个主题、安装与独立验证均已落地，后续增加编译期输入与 import 两个主题；验证结果见 [VALIDATION.md](VALIDATION.md)。

用户决定：通过附加子命令提供能力，不修改编译器。公开入口为 `scoop example`，实际程序为 `<SYSROOT>/commands/scoop-example`，沿用已有的外部命令分发。

配套文档：[主题与示例清单](TOPICS.md)、[三个输出样稿](SAMPLES.md)。依据：[语言规范](../specs/SCOOP-SPEC.md)、[库 API 风格](../specs/SCOOP-API-STYLE.md)、[实现规范 §2.7](../specs/SCOOP-IMPL-SPEC.md#27-scoop-与-single-cone-scoopc)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)。本设计使用现有语言、库与命令合同，不改变编译阶段、产物或 runtime。

## 1. 目的与交付边界

让使用 Scoop 的人和 AI 能从已安装的工具链取得一个问题的正确典型写法，包括完整程序、运行方式、可观察结果，以及修改程序时必须知道的规则。内容随同一份工具链发布，查询离线完成。

本次交付由一个只读外部命令和静态内容组成。命令列目录、搜索并展示材料；编译、运行、测试继续使用现有 `scoop check/run/test`。示例查询不启动这些命令，不生成程序、不执行示例，也不要求项目目录中存在 Cone.toml。

API 符号查询、错误码解释、项目模板生成不进入这次交付。主题可以引用已有 API 文档和正常检查命令，但不为此增加编译器接口、文档产物字段或语义查询服务。

## 2. 按问题组织主题

### 2.1 主目录与其他入口

主题 ID 使用稳定的英文小写路径：`language/option`、`collections/map`、`json/typed`、`http/get`。第一层是语言概念或用户要处理的问题领域，第二层是具体任务。标题、摘要和解释使用中文，程序标识符与注释使用英文。

库名和 API 名放入关键词、别名及来源说明。例如 `scoop.json` 指向 `json` 分类，`ValueEncoder` 可搜到 `json/typed`；`scoop.core` 会关联多个主题，作为搜索关键词，不强行设为某一个主题的别名。

目录不沿 crate、里程碑或规范章节划分。用户找“读取文件”“JSON 编解码”“可选值”时，无须先知道它们由哪个 Cone 提供。规范章节作为解释的依据保留在维护资料中。

### 2.2 分类页与内容页

- 无参数：显示建议起点，以及已发布分类的一句话摘要。
- 分类 ID：显示该分类的主题目录和推荐起点。例如 `scoop example http` 先引导应用开发者看 GET、JSON POST 和 server。
- 完整主题 ID：输出一个完整主题。
- `--list`：列出全部已发布主题的 ID、标题和一句话摘要，不展开正文。

分类从已发布内容形成；规划表中的待补主题不进入安装索引。关联主题只指向已发布条目，不展示没有内容的入口。

首版采用下列导航路径，路径是编辑建议，不是必须依序通过的前置条件：

| 使用者的问题 | 建议阅读 |
| --- | --- |
| 第一次写 Scoop | start/hello → language/overview → start/single-file |
| 写日常数据处理 | language/option → collections/list → collections/map → io/files |
| 使用 JSON 和 HTTP | json/typed → async/tasks → http/get |
| 把程序组织成项目 | start/cone → language/visibility → test/assertions |

### 2.3 一个主题的粒度

一个主题回答一个清楚的问题，并提供一个推荐的完整写法。源码通常以几十行为宜；复杂任务按实际所需文件完整展示，不通过隐藏初始化、截断代码或省略清理来缩短输出。

“创建带数据的枚举并匹配”可以同时展示声明和 when；“Option、struct、enum、集合和字符串的全部写法”只适合作为明确命名的概览。基础页解释首选做法，嵌套 Option、泛型约束组合等有独立学习目标时再拆成相关主题。

多文件主题是例外但必须完整：

- Cone 示例包括 Cone.toml、生产源码和必要测试源码。
- HTTP client 的本机演示包括实际服务端程序、两个终端的启动顺序与退出方式。
- SPA 包括实际引用的 HTML、CSS、JavaScript。
- TLS 包括证书配置和生成命令，不携带生成的私钥。

主题之间可以引用同一份文件；同一程序不因为有两个检索入口而复制。确实不同的教学任务可以编写各自简短的程序，不按行号从大型 fixture 中拼接代码。

## 3. 每个主题的内容约定

默认输出依次包含：

| 顺序 | 内容 | 编辑要求 |
| --- | --- | --- |
| 1 | ID、标题、配套版本 | 标明所属安装包；版本是说明，不作为额外兼容性检查 |
| 2 | 一句话用途 | 说明程序完成的实际任务 |
| 3 | 运行条件 | 需要的库、文件、外部工具、TTY、配套进程；普通主题只需简述正常安装条件 |
| 4 | 完整文件 | 标明相对示例根的文件名；原样展示依赖头、import、main、清理等 |
| 5 | 运行方式 | 指定工作目录、正常 Scoop 命令、参数；多终端分开列明 |
| 6 | 预期结果 | 区分 stdout、stderr、文件变化与交互行为；明确动态部分 |
| 7 | 关键说明 | 解释该例真正依赖的规则，以及用户通常需要修改的地方 |
| 8 | 常见误用 | 只列与该任务直接相关的错误假设，并给出正确处理方式 |
| 9 | 相关主题 | 少量可继续查询的 ID 和它们解决的问题 |

查询输出中的程序保持推荐的正常路径。能自然展示的缺失值、解析失败或资源清理放入完整程序；故意编译失败的代码留在必要的独立诊断 fixture 中。正文可明确指出非法写法，不把不能编译的片段混入可复制源码。

### 3.1 必须落实到内容的 Scoop 差异

以下是内容审阅重点，分配到对应主题，不在每一页重复：

| 容易从其他语言带入的假设 | 对应主题要解释的实际规则 |
| --- | --- |
| 可选值就是 null；安全调用会展平 | T? 是 Option<T>；Some/None；嵌套 Option 保留层数；Option 没有判空智能转换 |
| Kotlin data class 的全部派生能力都可用 | struct 字段不可变；副本更新用实际 Scoop 语法；相等、ToString、Hash 和接口 conformance 分别说明 |
| 声明默认为 public | 默认 internal；公开类构造器、接口成员和 override 需要实际的公开声明 |
| Kotlin 风格的 lambda 捕获和 return | 捕获不可重新绑定的 binding；共享可变状态通过显式引用对象；lambda 局部返回使用标签 |
| 常用方法在 Any 或所有类型上都存在 | 依据实际能力调用；示例不虚构通用 toString/hashCode 或隐式接口实现 |
| 写了 import 就引入了库 | 依赖声明与名称导入分别完成；单文件默认 core/std，manifest 的 std 按需要显式声明 |
| suspend 自动带来后台线程或事件循环 | 普通 main 显式进入 EventLoop；suspend lambda 显式标记；launch/async 不承诺切换线程 |
| GC 或 close 自动完成协议收尾 | use/useAsync 负责各自清理；finish、flush、shutdownGracefully 按实际 API 另行调用 |
| 字符位置、字节数和字素数相同 | String 标量、UTF-8 字节和字素簇分别处理；索引和计数使用现有 Long 合同 |

这张表以现有规范为依据，不新增语言限制。发现实现、规范或现有示例之间有冲突时，先定位并修正实际问题，不能在帮助文字里发明兼容行为。

### 3.2 典型性与输出

例子优先使用普通公共 API；用用户有理由采用的类型和数据，避免把 compiler fixture 的 ABI 布局、GC stress 或极端泛型组合带入入门材料。

每页明确哪些输入可以替换，哪些条件仍要保留。例如 HTTP GET 可修改 URL 和正文读取上限，但响应读取与关闭仍在 withResponse 作用域内；JSON DTO 可修改字段，但省略显式 codec 的入口仍有 companion 能力要求。

输出断言使用实际语义：确定性的文字给出完整结果；时间、临时路径、随机值说明变化部分；并发程序只承诺已由 join/await 建立的顺序。server 页说明请求与响应，不将启动端口或构建诊断当作固定程序输出。

## 4. 现有内容的复用与补齐

当前 `sysroot/src/*/examples/` 共 61 份 Scoop 源文件，包括两个测试 Cone 的生产与测试文件。它们已有大量短程序；这个数量不等于 61 个独立主题，也不表示已经完成本命令的内容验收。

按三种方式处理：

1. **直接复用**：JSON typed、HTTP GET、文件操作等已有程序，保留文件，补齐主题级说明和索引。
2. **拆出独立主题**：core 的 values/collections 和 std 的混合示例中，有些独立学习目标需要新的小程序。原概览可以保留，但不能把同一混合程序以多个标题冒充精确示例。
3. **新增缺口**：可见性、泛型、lambda 捕获、Cone 与依赖、资源释放等，以实际规范和已有测试为素材编写日常用法。

第一批 24 个主题的内容任务、观察结果及后续库覆盖见 [TOPICS.md](TOPICS.md)。该清单区分现有来源与计划新增文件，不将计划路径描述为已有实现。

现有库 examples/README 保留目录、共同准备方式和简单运行入口。迁移后的主题级详细解释以本设计的正文文件为准，旧 README 链接到它；不另外维护一套相同的完整解说。源码仍由原库维护。

## 5. 文件组织与内容单一来源

采用普通静态文件：

```text
tooling/scoop-example/                    独立外部工具源码
sysroot/
  commands/scoop-example                 维护构建产生的宿主可执行文件
  share/scoop/examples/
    catalog.toml                         分类、检索项、文件引用、相关主题
    topics/
      language/option.md                 运行说明、输出、规则与误用
      json/typed.md
      http/get.md
    programs/
      language/option/main.scoop         没有现有归属的新示例
      start/cone/Cone.toml
      start/cone/src/main.scoop
  src/scoop.json/examples/typed.scoop     直接引用的既有源码
  src/scoop.http.client/examples/get.scoop
```

catalog 中的 `source` 相对所选安装根；`document` 相对 catalog 所在目录；`path` 是展示给使用者、保存到临时工作目录时使用的路径。显示路径可以不同于库内路径，文件内容不作替换。例如 HTTP 主题把两个原有程序分别展示为 client.scoop 和 server.scoop。

普通单文件不改写依赖版本，不注入隐藏前导代码。多文件项目的显示路径必须保留 manifest、import 之外实际文件访问和资源引用所需的相对关系。

索引记录节选如下；关联目标也必须作为主题随同发布：

```toml
schema = 1
recommended = ["json/typed"]

[[groups]]
id = "json"
title = "JSON"
summary = "结构化数据与 JSON 文本、值树之间的转换"
aliases = ["scoop.json"]
recommended = ["json/typed"]

[[topics]]
id = "json/typed"
title = "把 struct 编码为 JSON 并解码回来"
summary = "使用 companion 提供的 codec 处理数组、可选字段和默认值。"
aliases = ["json-codec", "JSON序列化"]
keywords = ["scoop.json", "ValueEncoder", "ValueDecoder", "DTO", "companion"]
document = "topics/json/typed.md"
requirements = ["配套 sysroot 已包含 scoop.json 及其依赖。"]
related = ["language/option", "language/companion"]
files = [
  { path = "typed.scoop", source = "src/scoop.json/examples/typed.scoop", language = "scoop" },
]
```

正文是普通 Markdown，固定使用“运行”“预期结果”“关键说明”“常见误用”等小节。文件代码由 renderer 从 `files` 引用读取并插入，正文不再复制代码块。标题、摘要、运行条件与 related 由 catalog 生成，正文不重复维护这些字段。渲染器不解析 Markdown 的章节语义，也不提供任意 include 或模板执行机制。

schema 只是这个小工具的本地内容格式。它不进入语言身份、.slib、runtime ABI 或编译缓存。索引中的路径与关联在维护时做必要的格式、存在性和重复项检查；日常查询读取所需文本并报告实际 I/O 错误。

## 6. 命令与检索

```sh
scoop example
scoop example --list
scoop example language
scoop example language/option
scoop example --search "JSON 序列化"
scoop example json/typed --format source
scoop example test/assertions --format source --file Cone.toml
scoop example json/typed --format json
scoop --sysroot /path/to/scoop example json/typed
```

| 选项或输入 | 规则 |
| --- | --- |
| 主题参数 | 精确 ID 或显式别名；分类输出目录，主题输出全文 |
| --search text | 与主题参数、--list 互斥；搜索 ID、标题、摘要、别名及关键词 |
| --list | 所有已发布主题按分类与编辑顺序展示；与主题参数互斥 |
| --format markdown | 默认；普通文本与 Markdown，完整源码块，无分页器与终端控制序列 |
| --format source | 仅用于主题；单文件直接输出该文件的原文；多文件必须用 --file 选择 |
| --file path | 仅与 source 配合，精确选择 catalog 中的显示路径；缺失时列出可选文件并报错 |
| --format json | 输出当前目录、搜索结果或主题的一份 JSON；主题包含原文文件和正文 Markdown |

主题的 JSON 最少包含 `schema`、`kind`、安装版本说明、`id/title/summary`、`requirements`、`files[{path, source, language, text}]`、`body_markdown` 和 `related`。所有形式读取同一份记录与源码；JSON 不重新组织一套教学内容。

搜索对英文做大小写无关匹配，中文使用文本匹配；以空白分词，所有查询词都应在同一条目的检索字段集合中匹配。顺序为精确 ID/别名、ID 前缀、标题/关键词、摘要，再以编辑顺序消歧。不扫描程序全文或构建产物。搜索结果只给摘要和匹配依据，使用者再选择主题读取。

别名须明确且唯一。模糊名称和拼写不匹配时提供搜索候选，不静默选中相近主题；例如 `null` 同时涉及语言 Option 和 JSON null，作为搜索词，不设成单一主题别名。空搜索结果与未知主题均返回 1；正常查询返回 0；参数错误返回 2；实际 I/O 或内容格式错误返回 1。正文走 stdout，错误走 stderr；JSON 模式的错误也是 stderr 上的单独 JSON 对象，失败时不输出半份正文。

根目录和分类页的固定推荐主题来自已发布 catalog，查询不自动展开 related，不截断程序。内容较大时用明确的更细主题或 --file 选择，不能悄悄省略被示例使用的文件。

## 7. 外部工具与安装

实现为独立 Rust 宿主小工具，位于 tooling/scoop-example，使用自己的 Cargo 项目和仓库已经采用的参数/文本格式库。它不依赖 compiler crate，不调用 HIR、LLVM 或 scoopc。实现只需参数处理、读取、搜索、文本/JSON 输出。

现有 `scoop` 已直接执行选中安装根的 commands/scoop-example，且不向子进程额外注入该根。工具必须从自身可执行文件的位置定位安装根，再读取相邻的 share 与 src；不能重新按 cwd 或 SCOOP_SYSROOT 选择另一套内容。这样 `scoop --sysroot B example` 在环境变量指向 A 时仍读取 B。

维护脚本构建宿主命令并安装到 commands，复制 share/scoop/examples；现有发行流程已经携带 src。开发准备也由维护脚本显式完成。普通查询不检查或准备预编译库。所选安装包的 VERSION 文件用于显示版本；开发 sysroot 没有发行版本时明确显示“开发内容，未提供发行版本”，不调用编译器猜版本。

不修改 scoop 的内置命令列表或 --help。发现入口放在 `scoop example --help`、安装包说明、sysroot 示例总索引和仓库使用说明中。Scoop 自身的 --sysroot 放在 example 之前，符合已有外部命令规则。

输出的运行步骤以“将列出的文件保存在一个可写工作目录”为基准，使用原有 scoop 命令；不要求向安装目录写编译结果。显式选择其他安装包查询时，说明后续 run/check/test 也要使用同一 --sysroot。复制文件由调用者完成，example 本身不增加导出项目或覆盖文件功能。

## 8. 内容验证与验收

验证围绕实际要交付的内容，不在日常查询中重做：

1. 索引测试检查唯一 ID/别名、有效分类与 related、存在的正文及文件、唯一显示路径。验证记录仍放仓库外。
2. 所有发布示例使用显示给用户的文件布局，经正常 CLI 编译；涉及项目依赖、生成器或 native 的例子提供实际所需材料。
3. 首批按当时用户指定范围执行外部工具的独立测试与发布示例本机验收；后续内容按改动范围定向验证，平台和结果记入 VALIDATION.md。已有库行为测试不重复搬入本工具，不因内容更新运行编译器完整测试套件。
4. 独立脚本从真实命令的 JSON 输出还原显示文件，使用已准备的本机工具链验证已发布示例，包括 CLI 输出、跨 Cone 调用、测试 Cone 和 HTTP 两进程。只复制所需文件，保留原源码、工具与产物。
5. 网络例子使用本机回环服务，交互例子使用现有终端测试能力；验证不足的内容不标记为可运行交付。普通例子使用 release 和共享预编译 sysroot，不增加 GC stress 或全量 profile 组合。
6. 人工审阅“程序是否推荐这样写”：依赖/import 完整，正文没有不存在的方法，作用域和清理正确，输出符合所述行为，相关链接确实能帮助下一步修改。

索引/schema 测试只检查文档结构；程序正确性由正常编译和行为测试承担。正文中的输出说明与少量运行断言同步维护，不建立通用 Markdown 执行器、验证凭证或另一套源码检查器。

外部工具只需验证列表/搜索、三种输出、文件选择、错误退出，以及从实际命令位置选择安装根。发行验收检查命令、正文和所引文件进入同一安装包，并通过既有 scoop 外部入口读取真实内容。

## 9. 实施顺序

1. 按 [TOPICS.md 首批目录](TOPICS.md#1-首批-24-个主题)完成内容编辑；先落地基础页和可直接复用的库页，逐主题验证。
2. 实现薄的外部阅读器，接入静态目录、搜索及文件展示，补必要的 CLI 测试。
3. 接入维护安装/打包、旧 examples README 的导航和用户入口说明，验收首批 24 页的真实使用流程。
4. 继续按 TOPICS 的现有库映射补充内容。后续主题不成为首批交付的前置门槛。

第一批完成的标志是用户能查到内容、保存完整文件、按说明运行并正确修改程序。仅有一个搜索命令或大量主题名称不算内容交付。

## 10. 实现位置

- [独立阅读器及维护说明](../../tooling/scoop-example/README.md)：参数、索引、搜索和渲染。
- [发布索引](../../sysroot/share/scoop/examples/catalog.toml)：当前 26 个主题、文件引用与导航。
- [维护安装脚本](../../scripts/build_scoop_example.py)：仅构建此命令；完整打包入口自动调用。
- [示例验收脚本](../../tooling/scoop-example/tests/run_examples.py)：从公开输出还原文件并实际运行。

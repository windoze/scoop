# Scoop language server

构建与启动：

```sh
cargo build --release -p scoop-lsp
target/release/scoop-lsp --stdio
```

将编辑器的 Scoop language server 命令设置为该可执行文件的绝对路径，参数为 `--stdio`。标准输入／输出仅承载 LSP Content-Length 消息，启动错误写到 stderr。此命令不依赖 LLVM；语法服务也不要求 sysroot。

当前已实现 initialize／shutdown／exit、完整或增量文档同步、UTF-8／UTF-16／UTF-32 位置协商（默认 UTF-16）、语法诊断、documentSymbol、foldingRange 和 selectionRange。大纲按客户端能力返回分层或平面结构。URI 为未保存文档时同样提供语法服务；本地文件关闭后恢复普通磁盘输入，文件变更通知不会覆盖打开的 buffer。

workspace/symbol 搜索已加载项目的顶层声明与名义类型成员，支持不区分大小写的名称子串；空查询返回当前范围内的全部声明。输入版本各自保存声明索引，已知名称在签名或正文错误时仍可搜索和导航。局部声明保留在文档大纲中；不同配置中的同一文件按当前选择去重，移出项目或切换源码用途后相应结果同步更新。

每个请求读取固定分析版本，未变化文件复用语法树；输入更新使旧计算失效并重新安排尚需响应的请求。传输循环在解析期间继续处理修改和取消，取消请求仍收到响应。诊断携带当前客户端版本，修复后发布空列表清除原诊断。

工作区复用普通 Cone.toml、源码条件与依赖选择。workspaceFolders 或 rootUri 可指向 Cone 根；较大的仓库目录在打开文件时选择其最近的 Cone 根，同时加载声明的源码依赖。未归属项目的文件继续提供语法服务。打开的 Cone.toml 也覆盖磁盘；错误 manifest、缺失依赖与无效目标以 `SCOOP_PROJECT` 诊断发布。服务器响应 workspace/didChangeConfiguration、workspace/didChangeWorkspaceFolders 和文件变更通知；客户端支持动态文件监听注册时，服务器请求监听 manifest、Scoop 源码与 slib。

初始化选项支持如下配置；同样的对象可放在配置变更通知的 `settings.scoop` 中：

```json
{
  "target": "aarch64-apple-darwin",
  "sysroot": "/opt/scoop",
  "purpose": "production",
  "artifactSearchRoots": ["/work/artifacts"]
}
```

target 默认使用宿主的现有目标选择；purpose 可为 production 或 test，默认 production。Test 只将根 Cone 的测试源码和测试依赖加入输入，源码依赖仍按 production 处理。sysroot 沿用 `SCOOP_SYSROOT` 或安装布局，编辑器不自动准备系统库、运行 native 构建或启动生成器。相对 sysroot／搜索路径相对于所属 Cone 根解析。

同一文件需使用不同目标或用途时，可以在 `configurations` 中显式列出配置，并用 `activeConfiguration` 选取语义上下文：

```json
{
  "sysroot": "/opt/scoop",
  "configurations": [
    {"name": "app", "rootUri": "file:///work/app", "purpose": "production"},
    {"name": "app-tests", "rootUri": "file:///work/app", "purpose": "test"}
  ],
  "activeConfiguration": "app-tests"
}
```

每个配置还可覆盖 target、sysroot 和 artifactSearchRoots。配置名称须非空且唯一；未指定 activeConfiguration 时按配置顺序选择当前文件所属的第一个项目。同一文件共享文本和语法，项目归属与依赖保持分开。

已有语义服务包括函数／参数／局部变量及实际调用的 hover 和定义导航、receiver 成员补全、完整与半成品调用的签名帮助，以及普通声明／正文诊断。当前 Cone 和完整源码依赖复用原前端，预编译依赖读取已有 HIR；缺少源码时仍可显示已有签名，导航不猜测文件位置。错误源码中的已知事实与不可用状态按[完整设计](../../docs/language-service/DESIGN.md)区分，实际覆盖与剩余工作见[实施记录](../../docs/language-service/PROGRESS.md)。

class／struct／interface／enum／object／companion 内的方法正文不完整时，完整的类型头与成员签名仍参与普通分析，相邻完整方法可以独立查询。恢复方法保留原 receiver、访问域和泛型参数，这些类型的交叉嵌套及 inner class 保持真实 owner，缺少文件末尾右大括号时也可查询已经取得的上下文。object 保留单例类型及成员关系，companion 保留实际宿主、名称和宿主泛型参数；完整初始化式和初始化块继续沿普通单例规则处理。struct 的表示字段、默认值和构造器仍沿原规则解析，完整字段及其应用类型可以导航。interface 保留抽象 slot、默认实现和私有方法的原身份与顺序，正文不完整的默认方法仍可沿原继承关系导航到实际提供方。enum 保留全部 variant、字段、默认值和整数常量，构造、匹配穷尽性和整型父类型的规则继续由普通前端处理；当前源码和预编译依赖的限定半成品调用复用普通静态绑定与构造签名，支持 variant、嵌套类型、透明别名和泛型 companion；局部值遮蔽及已应用的宿主类型参数仍然生效。别名显示自身泛型参数，固定应用显示实际类型，variant 构造不要求访问 companion。显式类型参数与已检查实参的刚性类型冲突会排除该候选，未完成的其他推断仍保持未定。成员头部语法或表示信息未知时仍保留不可用边界，不省略坏成员来选择其他候选；已完整解析但语义失败的方法签名沿普通候选规则阻塞相关重载。

未完成的 while 条件和循环体支持已有输入的 hover、定义、成员补全及签名帮助。完整条件沿普通规则收窄循环体中的值，标签、嵌套循环和单语句 body 保留真实作用域。未知回边不沿用可变变量进入循环前的收窄；不可变值和每轮条件重新建立的事实仍可使用。条件或此前语句失败时保持不可用，循环后的未知状态不用于查询。

for 头部完整时，循环体查询沿普通 iterator 与 core `Iterator<T>` 协议取得元素类型，并使用原模式绑定；tuple 解构、泛型元素、单语句 body 和带标签跳转保持实际作用域。iterable 未完成时只查询外层已知输入，错误 iterator 或模式不产生部分绑定。模式变量与该轮循环体共享作用域，修改元素类型后 hover、定义位置和签名帮助随输入更新。

do-while 的未完成 body 保留循环作用域，后置条件不会提前收窄 body 中的值。body 完整时，未完成条件可以查询该轮直接声明的局部变量；可能被 continue 跳过的初始化仍不可用，finally 对跳转的影响沿普通 flow 分析处理。条件中的 hover、定义、成员补全与签名帮助随编辑缓冲区更新；未知或失败 body 不提供推测的条件作用域。

when 的未完成正文使用原 subject、case 模式、普通条件和 guard 的作用域与类型事实；tuple、struct、enum 模式和 subject 声明的名称可导航到实际来源。分支 body 的局部错误和写入不影响其他 arm，完整条件的否定路径继续传给后续 arm。未完成 guard 可查询已建立的模式变量，逗号条件中的部分表达式沿此前条件的否定路径查询；未知头部之后的分支仍不可用。条件、guard 和 body 中的半成品调用共用签名帮助，编辑模式类型后同步更新结果。

try、catch 和 finally 的未完成块支持已有输入与局部 binding 的查询。catch 参数沿原 Throwable 子类型和声明顺序检查，保留泛型应用及实际定义位置；无效或被前序 handler 覆盖的类型不提供参数。完整 try body 使用实际抛出路径计算 handler 的事实，finally 合并完整前置区域的各个出口；未知区域之后清除可变变量的收窄与相关 Boolean 蕴含。各块保持独立作用域，catch 参数和 body 局部不会泄漏到 finally；另一块未完成时，已有 handler 参数和不可变外层值仍可查询。

初始化式或返回位置中的 lambda／匿名函数头部完整时，未完成正文可查询参数、解构 binding、receiver 和外层捕获。初始化注记或已知返回类型可以为 lambda 提供参数类型和隐式 `it`，泛型捕获仍指向原声明位置。标签返回、suspend 调用和局部遮蔽遵守实际内部 callable 的规则，外层循环目标不会跨过该边界；参数类型未知或头部检查失败时不使用外层同名变量。hover、定义、成员补全和正文内的签名帮助随编辑缓冲区更新，关闭后恢复磁盘内容。

调用实参中的损坏 lambda／匿名函数可以使用原参数映射与泛型推断得到的上下文。当前查找层只剩一个候选、此前实参均已检查且参数／receiver 类型已知时，正文可查询真实类型与定义，返回类型仍可待推断；成员、扩展、构造器和函数值调用沿相同规则处理。括号内实参及 `call { ... }`、`call(...) { ... }` 的尾随 lambda 都保留真实来源位置，签名帮助识别当前实参；函数结果的分组调用、safe receiver、泛型、显式标签和 suspend 仍沿原规则。具名调用保留 lambda 的隐式返回标签，显式标签优先。候选歧义、必要参数类型未定或前序实参未知时不猜测内部绑定。

局部具名函数的头部完整时，损坏正文可查询自己的参数、context 名称、泛型及外层捕获，也支持内嵌 lambda 和更深的局部函数。hover 使用实际源码上下文显示局部泛型，参数和捕获导航到原定义；自递归的半成品调用使用已声明签名，尚未推断的结果保持未定。默认参数、重复声明、return 标签、suspend 和循环控制沿普通局部函数规则检查，未知前序语句或失败头部不会提供猜测绑定。

某个文件的 package 或 import 存在语法错误时，其他文件仍可查询独立的声明、正文和恢复 receiver，诊断及定义位置保留原文件编号。损坏文件中已知名称继续参与相应查找，避免同名函数、构造器、类型或属性错误地回退到其他目标；普通可见性与 Production/Test 作用域继续生效。引用查询保留已经确定的使用位置，内部索引标记整个损坏文件不可用。修复文件头或关闭未保存 buffer 后，按实际文本重新建立上下文。

完整函数正文中的 property 读取／写入显示应用后的属性类型，并导航到原属性声明；支持常量、存储、计算与扩展属性以及导入声明。构造参数属性指向原参数名称，getter／setter 和泛型实例不产生额外定义。具名 callable value 与 native 地址同样沿实际重载选择导航到原函数。

属性声明与精确 import 也可直接 hover，显示原名称、val／var、声明类型以及扩展 receiver 的泛型参数；声明名称的定义导航使用同一属性来源。声明类型已知时不检查无关正文，不执行 getter 或初始化式；尚未完成类型推断时保持不可用。局部类型属性使用所属正文的完整事实，文档选择原 property 目标，同位置构造参数的 `@param` 不覆盖属性文档。

函数、参数和 property 的 hover 同时显示 M60 文档注释。Markdown、符号链接和标签复用普通文档绑定；参数可读取直接注释或所属函数的 `@param`。源码依赖的文档与其 Export HIR 接口一起缓存，编辑注释后同步更新，关闭 buffer 后恢复磁盘文本。链接只指向项目实际提供的源码；无源码目标与重载集合保留标签。错误正文不妨碍相邻完整声明的文档，未取得真实文档附着的恢复项不猜测注释归属。

精确 import 的函数／property／类型目标名与显式 alias 名称支持定义导航；重载 import 返回多个原定义，查找直接复用导入解析。成功调用的命名实参按真实形参序号导航，支持局部函数、重载和跨 Cone 源码，参数重排不影响目标位置。命名实参 hover 使用同一映射显示实际应用类型，保留嵌套泛型名称和 vararg 元素类型，并读取原形参的 M60 文档；后续正文错误不撤销此前完整调用的事实。

构造调用和 class／struct 的构造器函数值沿原重载选择导航到实际构造器，透明别名与泛型应用保持同一源声明。enum variant 的构造、单位值和精确 import 保留原 variant 来源。构造参数的命名实参独立参与定义、文档及引用查询，同位置字段／property 的文档不会覆盖参数文档；内部生成的构造 adapter 不成为新的定义。

`textDocument/references` 查询当前文件所选项目中的源码声明、精确 import 的函数／property 目标与已完成函数正文，区分局部遮蔽和重载，合并同一原声明的泛型应用；跨 Cone 的函数与属性引用使用普通声明身份。函数参数的声明、正文引用和已绑定命名实参一起查询。选择显式 alias 时查询该 alias 的声明与实际使用，选择原声明时仍包含这些消费位置及 import 目标名；同名 alias 和静态宿主限定调用保持区分。`includeDeclaration` 控制是否包含源码声明，预编译依赖缺少源码时只返回已有使用位置。错误或尚未分析的区域不产生猜测引用；该范围目前不包括独立初始化式和 accessor 正文，alias 的跨 Cone 转导出来源仍待接入，不能据此推断符号在完整工程中未被使用。

类型名称的定义与引用区分原名义声明、透明 typealias 和类型参数。限定路径分别指向实际声明，精确 import 的显式 alias 保留自己的使用范围；泛型应用不产生新定义，局部类型的字段签名来源随原声明结果复用。源码依赖使用原类型／别名身份和对应文件的准确名称位置。

`textDocument/typeDefinition` 读取变量、参数与 property 的实际类型，以及调用的完整结果类型。类型语法中的泛型实参和透明 typealias 先沿原类型解析应用，再导航到名义声明；仍未具体化的类型参数指向原参数声明。普通函数类型、元组等没有独立声明的结构类型返回空结果；错误正文只使用已取得的事实，缺少源码时不猜测位置。名义类型的身份与来源和 M47 类型查询共用。

类型名称、typealias、类型参数以及精确 import 的类型目标支持 hover，并显示同一声明的 M60 文档。声明保留原名称和自有泛型参数，别名展示展开结果；使用处另列实际应用类型，嵌套泛型保持原参数名称。类型参数的约束读取原声明，直接注释与 `@param` 标签沿 M60 规则选择。导入 alias 仍展示被导入声明，未使用的导入无需先编译正文；编辑参数、应用类型或文档后同步更新，关闭 buffer 后恢复磁盘输入。

`textDocument/implementation` 查询声明阶段已知的名义类型关系：接口返回子接口和实现类型，class 返回子类，包含间接继承、抽象类型与 object。精确 import alias 沿原绑定选中目标，透明别名沿完整应用展开；泛型实例合并到原声明。当前项目各源码 Cone 的结果一起返回，修改继承或关闭 buffer 后同步更新，声明上的查询不检查无关正文。

方法与 logical property 的实现导航复用实际重写和接口选择，抽象中间声明保持连接，返回具体声明自身、默认实现和继承提供方的源码位置。getter／setter 合并到原 property，构造参数属性仍指向原参数名称；只有部分 accessor 提供默认实现时保留该来源。调用与绑定方法引用沿普通绑定选中目标，重载和泛型声明保持原身份；预编译中间层沿已有 slot 与 property 引用读取。待生成方法和内部 adapter 不产生导航结果。

客户端支持 code action literal 和 `workspace.workspaceEdit.documentChanges` 时，提供缺失右括号、逗号等固定标点的 quickfix；返回的 TextDocumentEdit 带实际文档 version。名称、类型与表达式缺失不生成猜测内容。支持 resolve 的客户端还可重新取得同版修复；原文变化后旧动作返回 ContentModified，原文未变而客户端 version 更新时返回最新 version。其他引用区域与重命名继续按设计实施，未实现的能力不在初始化中声明。

开发验证使用 `cargo test -p scoop-analysis -p scoop-lsp`；LSP 集成测试启动真实 `scoop-lsp` 进程，检查传输、状态变化和查询结果。运行时将 `TMPDIR` 指向仓库外工作目录，测试工作文件遵守同一设置。

预编译 core／标准库的正式进程覆盖位于 `tests/fixtures/lsp`，使用显式准备的 sysroot，按 fixture runner 的 `--scoop-lsp` 参数选择服务器。

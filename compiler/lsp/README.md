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

完整函数正文中的 property 读取／写入显示应用后的属性类型，并导航到原属性声明；支持常量、存储、计算与扩展属性以及导入声明。构造参数属性指向原参数名称，getter／setter 和泛型实例不产生额外定义。具名 callable value 与 native 地址同样沿实际重载选择导航到原函数。

函数、参数和 property 的 hover 同时显示 M60 文档注释。Markdown、符号链接和标签复用普通文档绑定；参数可读取直接注释或所属函数的 `@param`。源码依赖的文档与其 Export HIR 接口一起缓存，编辑注释后同步更新，关闭 buffer 后恢复磁盘文本。链接只指向项目实际提供的源码；无源码目标与重载集合保留标签。错误正文不妨碍相邻完整声明的文档，未取得真实文档附着的恢复项不猜测注释归属。

精确 import 的函数／property／类型目标名与显式 alias 名称支持定义导航；重载 import 返回多个原定义，查找直接复用导入解析。成功调用的命名实参按真实形参序号导航，支持局部函数、重载和跨 Cone 源码，参数重排不影响目标位置。

`textDocument/references` 查询当前文件所选项目中的源码声明、精确 import 的函数／property 目标与已完成函数正文，区分局部遮蔽和重载，合并同一原声明的泛型应用；跨 Cone 的函数与属性引用使用普通声明身份。函数参数的声明、正文引用和已绑定命名实参一起查询。选择显式 alias 时查询该 alias 的声明与实际使用，选择原声明时仍包含这些消费位置及 import 目标名；同名 alias 和静态宿主限定调用保持区分。`includeDeclaration` 控制是否包含源码声明，预编译依赖缺少源码时只返回已有使用位置。错误或尚未分析的区域不产生猜测引用；该范围目前不包括独立初始化式和 accessor 正文，alias 的跨 Cone 转导出来源仍待接入，不能据此推断符号在完整工程中未被使用。

类型名称的定义与引用区分原名义声明、透明 typealias 和类型参数。限定路径分别指向实际声明，精确 import 的显式 alias 保留自己的使用范围；泛型应用不产生新定义，局部类型的字段签名来源随原声明结果复用。源码依赖使用原类型／别名身份和对应文件的准确名称位置。

`textDocument/typeDefinition` 读取变量、参数与 property 的实际类型，以及调用的完整结果类型。类型语法中的泛型实参和透明 typealias 先沿原类型解析应用，再导航到名义声明；仍未具体化的类型参数指向原参数声明。普通函数类型、元组等没有独立声明的结构类型返回空结果；错误正文只使用已取得的事实，缺少源码时不猜测位置。名义类型的身份与来源和 M47 类型查询共用。

`textDocument/implementation` 目前查询声明阶段已知的名义类型关系：接口返回子接口和实现类型，class 返回子类，包含间接继承、抽象类型与 object。精确 import alias 沿原绑定选中目标，透明别名沿完整应用展开；泛型实例合并到原声明。当前项目各源码 Cone 的结果一起返回，修改继承或关闭 buffer 后同步更新，声明上的查询不检查无关正文。方法和 logical property 的实现导航继续实施。

客户端支持 code action literal 和 `workspace.workspaceEdit.documentChanges` 时，提供缺失右括号、逗号等固定标点的 quickfix；返回的 TextDocumentEdit 带实际文档 version。名称、类型与表达式缺失不生成猜测内容。支持 resolve 的客户端还可重新取得同版修复；原文变化后旧动作返回 ContentModified，原文未变而客户端 version 更新时返回最新 version。其他引用区域、实现与重命名继续按设计实施，未实现的能力不在初始化中声明。

开发验证使用 `cargo test -p scoop-analysis -p scoop-lsp`；LSP 集成测试启动真实 `scoop-lsp` 进程，检查传输、状态变化和查询结果。运行时将 `TMPDIR` 指向仓库外工作目录，测试工作文件遵守同一设置。

预编译 core／标准库的正式进程覆盖位于 `tests/fixtures/lsp`，使用显式准备的 sysroot，按 fixture runner 的 `--scoop-lsp` 参数选择服务器。

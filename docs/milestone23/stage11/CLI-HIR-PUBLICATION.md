# M23-11 共享 HIR 发布与消费迁移

## 正式 HIR 观察输出

`--emit hir` 在既有 Export／LocalConcrete 之后显示同次 HIR 生产的 CrossCone 共享接口、source constructor 和 imported application，LocalConcrete 追加实际 shape-support roots。读取已有 typed 数据，沿 canonical 表／arena 的现有顺序显示，不新增解析、布局校验、产物格式或语义投影。

逐份核对 720 个既有 CLI 用例中的 1313 份 HIR golden：移除新增 CrossCone 区域和 shape-support 行后，原字节完全一致；AST、MIR、LIR 与 link plan 不变。core 默认定位和委托损坏产物用例另经正式 runner 只读完整验证，共 28 次进程、17 份阶段／plan golden。

## 默认参数的嵌套 callable identity

保留 `m23-default-nested-identities` 两份原源码。独立用例的 1 个模板含 1 个 lambda；组合用例的 7 个模板包含 2 个 local function、6 个 lambda、2 个 anonymous function、1 个 callable reference。对 CLI 实际发布字节逐项重用原结构断言，5 个显式展开参数组均与其 owner binder 数量相等；这些完整记录及定义来源现由 HIR golden 锁定。

组合用例增加单独、显式的同 Cone `nestedHost()` 工厂文件，让新下游程序访问原默认私有构造器。原源码与原 Export 内容保持；相较旧 Export，只有此工厂的 3 行新正文。

两个用例均经正式 CLI 发布 provider，移走 provider 源码后消费默认参数，再移走全部源码、隔离编译器后独立链接；验证实际结果、主动 GC 及普通／移动模式运行。只读验收为 2 个用例、18 次进程、18 份阶段／plan golden。删除原 `default_nested_identities.rs`、注册和两份旧 HIR 快照。

## 共享源码闭包

保留 `m23-shared-source-closure` 两份原源码和 2／14 个默认参数模板。对 CLI 实际发布的 slib 逐项核对：模板 owner 不成为 public binding，仍有完整 declaration／source interface；顶层 callable／property support 存在，组合用例保留 extension、不同 definition root 与至少 3 个 nominal support；顶层 support 函数仍恰好为 1，不可达 `unreachableLeft`／`unreachableRight` 不被提升。完整模板、引用、support 和 definition source 由正式 HIR golden 锁定。

新下游使用显式同 Cone 工厂 `sourceContainer()`／`sourceHostInt()`／`sourceHostString()`，不修改原构造器可见性。旧 Export 的全部原字节保留，仅增加工厂正文 3／6 行。下游执行独立默认参数与 Int／String 组合默认参数，验证结果 15／55；源码移走后的重新消费、独立链接、主动 GC 和普通／移动模式均通过。

只读验收为 2 个用例、18 次进程、18 份阶段／plan golden。删除原 `shared_source_closure.rs`、注册和两份旧 HIR 快照，不保留临时结构复核入口。

## 默认参数的完整引用闭包

保留 `m23-default-reference-reader` 两份原源码。对 CLI 实际发布字节重用原断言：独立用例保留 1 个模板，组合用例保留 4 个模板；后者的 callable、constructor、type、global、singleton、field 六类引用计数分别为 3、1、17、1、1、1，全部存在。完整模板与引用现由 HIR golden 锁定，下游真实展开构造器、字段、闭包、函数、常量、单例与全局读取。

独立用例的旧 Export 内容逐字节保持；组合用例仅将 `READER_GLOBAL` 的存储访问器变为实际 getter／setter，并增加对应的两个 NoGc 正文。原源码、默认表达式及六类引用保持。删除 `default_reference_closure.rs`、注册与两份旧快照。

## 默认参数跨产物读取全局变量

真实下游消费发现 raw global 元数据有访问器声明却没有对应机器正文。现在 Scoop 定义的 `@Global`／`@ThreadLocal` 复用已有隐式访问器生成逻辑，保持 GC-free／NoGc、原存储身份与本地直接访问；默认正文中的外来 `GlobalRead` 调用定义方的实际 getter。`@Extern` 默认读取复用已有原生 pointer／place 路径，保留默认正文来源。

外来原生合同与存储不再重复发布为本地源码声明。LIR 允许引用已解析的依赖合同，共有 native boundary 从实际可达 provider 借用完整 source contract，继续执行类型闭包与 ABI 正规化；当前 source contract 的 target 完整性、重复项及未解析引用检查保留，未使用的依赖合同不生成虚构 target。原 native source closure 代码按职责移到 433 行子模块，函数正文逐字节保持，入口文件缩至 130 行；全局初始化与默认值操作符也各自拆分。

新组合用例经过 provider → consumer library → executable 三层产物：普通全局值 7→9、TLS 值 13→17、C 外部值 23→29，省略参数读取更新后的实际值，显式实参仍返回 11；所有源码移走后独立链接，验证主动 GC 与普通／移动模式。两个引用闭包用例与该组合共 3 个用例、29 次进程、27 份阶段／plan golden 只读通过。

关联 raw global／TLS／native／negative 回归共 23 个用例、131 次进程、206 份阶段／plan golden 通过。逐项审核既有变化：invoke 的 MIR／LIR 各增加 2 个访问器，raw globals 各增加 10 个，其余既有函数经唯一的函数索引平移后逐字节一致；相应 HIR 只增加这些正文与访问器关系，TLS threads 的 HIR 仅有两个 session-local imported index 移动；两个 native 组合 HIR 各删除 5 个被误记为本地的 foreign backing storage，其余字节不变。AST、诊断与已有 link plan 未改。12 项 LIR relation、22 项 HIR foundation、28 项 native boundary Rust 测试通过。

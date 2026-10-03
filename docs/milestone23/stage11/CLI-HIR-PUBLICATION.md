# M23-11 共享 HIR 发布与消费迁移

## 正式 HIR 观察输出

`--emit hir` 在既有 Export／LocalConcrete 之后显示同次 HIR 生产的 CrossCone 共享接口、source constructor 和 imported application，LocalConcrete 追加实际 shape-support roots。读取已有 typed 数据，沿 canonical 表／arena 的现有顺序显示，不新增解析、布局校验、产物格式或语义投影。

逐份核对 720 个既有 CLI 用例中的 1313 份 HIR golden：移除新增 CrossCone 区域和 shape-support 行后，原字节完全一致；AST、MIR、LIR 与 link plan 不变。core 默认定位和委托损坏产物用例另经正式 runner 只读完整验证，共 28 次进程、17 份阶段／plan golden。

## 默认参数的嵌套 callable identity

保留 `m23-default-nested-identities` 两份原源码。独立用例的 1 个模板含 1 个 lambda；组合用例的 7 个模板包含 2 个 local function、6 个 lambda、2 个 anonymous function、1 个 callable reference。对 CLI 实际发布字节逐项重用原结构断言，5 个显式展开参数组均与其 owner binder 数量相等；这些完整记录及定义来源现由 HIR golden 锁定。

组合用例增加单独、显式的同 Cone `nestedHost()` 工厂文件，让新下游程序访问原默认私有构造器。原源码与原 Export 内容保持；相较旧 Export，只有此工厂的 3 行新正文。

两个用例均经正式 CLI 发布 provider，移走 provider 源码后消费默认参数，再移走全部源码、隔离编译器后独立链接；验证实际结果、主动 GC 及普通／移动模式运行。只读验收为 2 个用例、18 次进程、18 份阶段／plan golden。删除原 `default_nested_identities.rs`、注册和两份旧 HIR 快照。

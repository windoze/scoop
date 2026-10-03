# M23-11 共有类型基础的 CLI 迁移

`m23-cli-type-foundations` 使用原 `m23-shared-type-foundations` 十份源码和坐标，分别通过正式 `scoop build` 生成 library，移走源码后由真实下游读取产物并调用成员、构造函数、继承方法和默认参数。所有用例保存 provider／consumer 四阶段输出；只读验证为 10 例、30 次进程执行、80 份阶段 golden。

迁移时直接读取十个实际 CLI 产物，逐项重建原 reader 测试的 293 行摘要。九份与原快照逐字一致；`slot-combinations` 的六行新增 getter 记录对应 `a1e63f7b3` 已修复的不同接口应用槽角色，其他行相同。旧摘要未打印 role，因而新行显示为相同 owner／target 文本；完整 HIR 与原 `(role, slot)` typed 合同均保留实际区别，没有合并或丢弃这些槽。

原入口为 `layout_exports/core/artifact/type_foundations/dependencies.rs`。这里只迁移文件快照和用例编排；provider 缺失／重复、constructor 与 protected member 来源、slot 选择及错误 mutation、默认值类型与继承定义的内部断言保留。

| 功能 | 原源码／CLI 用例 | 进程／阶段 golden |
| --- | --- | --- |
| source facts | `provider` | 3／8 |
| constructors and inherited members | `constructors`、`inheritance-members` | 6／16 |
| declaration dispatch order | `dispatch-order` | 3／8 |
| complete dispatch selections | `slot-selections`、`slot-combinations` | 6／16 |
| protected declarations | `protected-sources` | 3／8 |
| parameter protocols | `source-protocols` | 3／8 |
| inherited source defaults | `source-defaults`、`default-combinations` | 6／16 |

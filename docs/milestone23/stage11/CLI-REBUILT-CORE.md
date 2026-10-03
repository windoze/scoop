# M23-11 重建 core 后的泛型与协议消费

`m23-cli-rebuilt-core` 保留原 core 修改及 consumer／downstream 源码，通过正式 CLI 先构建原 core，再修改并重建；两个实际 artifact fingerprint 必须不同。随后删除 core 和各消费层的源码，只用产物再次消费、独立链接与运行。

各层一次编译保存完整阶段输出，consumer 的原 HIR Export、MIR、LIR 均逐字核对相同。下游使用原 `check()` 和普通 native GC epoch 检查，普通、moving GC 两种运行均通过。独立链接与构建的 fingerprint 相同，真实弱 callable 定义仍唯一。新增用例只使用现有 schema，不增加 runner 分支。

| 原功能 | 用例 | 只读进程／golden | 核对内容 |
| --- | --- | --- | --- |
| `imported_option_roles_follow_rebuilt_core_declarations` | `option-edited-core` | 12／9 | 反转 None／Some 声明顺序，保留嵌套 Option、缺省值、callback、unwrap 异常和再次泛型具体化；删除旧文件测试、注册与对应三阶段快照 |
| `imported_coroutine_roles_follow_rebuilt_core_declarations` | `coroutine-rebuilt-core` | 12／9 | 修改 core Coroutine 声明后保留协议角色、泛型 suspend 正文与再次发布；删除旧文件测试、注册与对应三阶段快照 |

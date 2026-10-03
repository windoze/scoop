# M23-11 泛型构造器、nominal shape 与初始化

`m23-cli-generic-shapes` 保留原 provider、consumer、downstream 源码及各自的 Cone coordinate。构造器 provider 仍分别编译 `src/main.scoop` 和 `src/base.scoop`，不拼接源文件；每层通过正式 CLI 发布四阶段输出并移走源码，再由下一层消费。独立链接使用正式 runtime index 和 startup，普通与 moving GC 运行继续要求原 `check()` 返回 42，并保留 GC epoch 检查。

18 个用例的 50 份原阶段输出逐字核对相同，HIR 比较原 Export 区域，同时以完整新 golden 保留 LocalConcrete／CrossCone。迁移时另对同批 CLI 生成的真实 `.slib` 执行原 Rust typed 断言，复核 callable ABI、物理 provider、shape ABI、ODR 候选和 registration；没有生成替代产物或放宽 reader 验证。真实 `nm` 输出锁定最终弱定义，初始化用例从实际提供这些定义的 provider 核对。

以下只读结果全部关闭更新开关。删除的是原 driver 的文件快照和执行编排；HIR 局部类型不变量的普通单元测试继续保留。

| 原功能 | CLI 用例 | 进程／golden | 原 typed 断言的对应 |
| --- | --- | --- | --- |
| `generic_constructor_templates_republish_and_execute_from_artifacts` | `constructors-standalone`、`constructors-structure`、`constructors-struct-secondary`、`constructors-class-secondary`、`constructors-class-terminal`、`constructors-default-base`、`constructors-captures`、`constructors-inferred-defaults`、`constructors-local-base`、`constructors-large-value`、`constructors-combined`、`constructors-nested` | 144／156 | provider 不预实例化自身 payload；每个 consumer 有真实构造器 ODR 定义及对应 ABI；default-base 保留 base.scoop 的定义 origin 和 main.scoop 的求值 origin；shape 候选与 ABI 一致 |
| `generic_nominal_consumers_create_payload_instances_from_artifacts` | `nominals-standalone`、`nominals-combined` | 24／26 | provider 无具体 shape；consumer 分别注册 1／4 个原 provider nominal 实例；ODR shape ABI 和 provider 候选一致 |
| `generic_initializations_survive_publication_and_execute_with_moving_gc` | `initialization-standalone`、`initialization-combined` | 24／26 | provider generic initialization 为 2／5 项；原类型 shape 为 12／24 项，registration 为 2／4 项，role、ABI 和候选一致 |
| `generic_dispatch_survives_publication_and_executes_with_moving_gc` | `initialization-dispatch`、`initialization-dispatch-combined` | 24／26 | 两个 generic class 各有 2 个 vtable slot，分别有 1／2 个 itable；MIR／LIR slot、签名、GC effect、target、ODR linkage 和物理 provider 全部一致 |

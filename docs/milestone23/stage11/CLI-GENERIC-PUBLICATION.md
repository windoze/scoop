# M23-11 泛型消费产物的再次发布

`generic_consumers_publish_reusable_artifacts_and_run_with_moving_gc` 迁为 `m23-cli-generic-publication` 的 `standalone`、`combined` 两个声明式用例。保留 `m23-generic-body-consumption/machine-*` 原 provider、consumer 和 downstream 源码，以及原 Cone coordinate；下游仍直接依赖 provider 与 consumer。

每层通过正式 `scoop build` 一次保存 AST、HIR、MIR、LIR，随后删除该层源码。consumer 的三份原阶段快照逐字核对相同；HIR 比较原 Export 区域，同时以新完整 golden 保留 LocalConcrete、CrossCone 和持久身份。combined 继续检查普通值、本地类型、再次传递的泛型、异常恢复和默认值，最终要求原 `check()` 返回 42。

删除源码后独立执行 `scoop link`，其 fingerprint 与构建结果一致；最终程序分别以普通和 moving GC 模式运行，保留 GC epoch 检查。真实 `nm` 输出锁定最终弱定义且同一 callable 符号只有一个定义。

关闭更新开关的只读运行通过 **2 个用例、24 次进程、26 次阶段／计划 golden 比较**。删除旧 `generic_bodies/publication.rs` 和注册。原 machine 边界测试仍使用的六份阶段快照暂留，待相应对象与 reader 断言迁移后统一清理；不以本批结果替代这些尚存断言。

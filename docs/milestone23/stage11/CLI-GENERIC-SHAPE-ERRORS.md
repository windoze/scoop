# M23-11 泛型构造器与 nominal 参数诊断

`m23-cli-generic-shape-errors` 用普通 provider 产物分别检查原八份错误源码。provider 源码在编译 consumer 前删除；构造器 provider 保持 main／base 两个独立源文件。每个 consumer 必须在 HIR 失败、不得发布 `.slib`，完整 JSON 固定全部消息、code、severity、canonical origin 和 notes。

构造器用例逐项保持原消息以及最后一个目标 token 所在的 UTF-8 byte span；nominal 用例保持原消息和精确被标记的表达式。没有以任意非零退出替代语言错误，也没有把源码复制进 Rust。仅删除被完整替代的 negative 文件测试，局部类型结构的普通单元测试保留。

| 原功能 | CLI 用例 | 只读进程 | 原断言 |
| --- | --- | --- | --- |
| `imported_generic_constructors_enforce_source_call_rules` | `constructors-bad-kind`、`constructors-bad-arity`、`constructors-private-constructor`、`constructors-immutable-property` | 12 | value kind、类型实参数量、private constructor、不可变属性写入 |
| `imported_generic_nominal_arguments_obey_invariance_and_bounds` | `nominals-bad-ref`、`nominals-bad-value`、`nominals-bad-arity`、`nominals-bad-invariance` | 12 | ref／value kind、类型实参数量、泛型不变性；精确表达式 Int／Number／Parcel／actual |

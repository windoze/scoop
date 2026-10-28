# 聚合常量

普通行为通过测试 Cone 验证；编译器 Rust 测试额外检查这些初值直接进入静态数据，含调用的 tuple 仍保留完整运行期 initializer。

```sh
target/debug/scoop test tests/cones/aggregate-constants --release --sysroot "$SCOOP_SYSROOT"
target/debug/scoop test tests/cones/aggregate-constants/consumer --release --sysroot "$SCOOP_SYSROOT"
```

覆盖 Unit／tuple 静态初值与 const 值、前向引用、tuple 元素读取、带引用类型参数的空 enum 变体，以及泛型读取中的多个 String 字段。consumer 使用实际发布的依赖 `.slib`，验证常量字段读取、泛型正文及默认参数。普通构造调用不进入常量求值；对应诊断在 `tests/fixtures/language/aggregate-constants`。

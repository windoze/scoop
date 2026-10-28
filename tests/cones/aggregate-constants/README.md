# 聚合常量

普通行为通过测试 Cone 验证；编译器 Rust 测试额外检查这些初值直接进入静态数据，含调用的 tuple 仍保留完整运行期 initializer。

```sh
target/debug/scoop test tests/cones/aggregate-constants --release --sysroot "$SCOOP_SYSROOT"
```

当前覆盖 Unit／tuple 静态初值；struct／enum 的源码 const 求值随编译期 intrinsic 后续实现扩展。

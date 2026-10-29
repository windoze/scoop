# 块级 import

使用普通测试 Cone 验证块级 exact／alias／star import 的作用域、嵌套恢复、闭包／局部函数、类型／enum、扩展 invoke，以及已发布泛型与默认参数正文的定义处绑定。

准备开发 sysroot 后，在仓库根运行：

```sh
target/debug/scoop test tests/cones/block-imports/consumer --release --sysroot "$SCOOP_SYSROOT"
```

诊断与编辑器覆盖由对应 compiler fixture 和 Rust 测试承担。

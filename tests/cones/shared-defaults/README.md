# 跨 Cone 默认值正文

三个真实 Cone 同时展开同一个参数自由默认值，用 `scoop.test` 验证构造器 suspend 闭包、嵌套闭包与局部函数捕获，以及默认函数引用。provider 自身也使用这些默认值，覆盖本地与依赖消费的身份一致性。

准备开发 sysroot 后运行：

```sh
target/release/scoop test tests/cones/shared-defaults/consumer --release --sysroot "$PWD/sysroot"
```

该测试使用普通 Cone 构建与运行，不加入 fixture runner。

修复验收：macOS 与 Linux GNU 各 3/3 通过；本机完整 HIR 测试 2055/2055 通过。Linux 仅运行这组三个最小回归测试。

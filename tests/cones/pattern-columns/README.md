# 模式矩阵未参与匹配的列

普通 `scoop.test` Cone，验证携带 32 个独立 Boolean 字段的事件被 `_` 忽略时，后面的按钮枚举和多列 Boolean 关联仍正确决定结果。它复现 GUI 输入事件暴露的实际编译器性能问题：穷尽性检查不应先展开这些字段的所有组合。

运行 `scoop test tests/cones/pattern-columns --release --sysroot <已准备的sysroot>`。需要编译、链接并执行真实源码；不依赖计时断言或 fixture。编译器单元测试另外覆盖缺失分支的稳定 witness 与空类型 product。

# 编译期输入行为测试

使用显式准备的 sysroot 运行 `scoop test tests/cones/compiler-inputs --release`。
字节值、空数组、实际类型及副本转换在普通 Cone 测试中验证；诊断、缓存更新、产物及 GC 登记的跨进程合同由对应 fixture 验证。

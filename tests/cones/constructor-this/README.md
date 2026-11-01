# 构造期间的 this

普通行为测试通过 scoop.test 验证 owner 型属性委托、已就绪属性与方法调用、闭包和函数引用、inner／匿名对象／单例、值类型次构造及失败后的已保存引用。provider 和 bridge 使用真实路径依赖产物，覆盖泛型构造正文与捕获 receiver 的跨 Cone 消费。

在仓库根目录使用已准备的开发 sysroot 运行：

```sh
SCOOP_SYSROOT="$PWD/sysroot" target/debug/scoop test --release tests/cones/constructor-this
```

直接未初始化访问、构造委派与默认参数中的 receiver 限制由 language fixtures 验证。这里不执行间接未初始化读取等未定义行为；GC 定向测试保留在编译器正确性 fixture 中。

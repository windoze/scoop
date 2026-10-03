# 具体类型继承接口默认属性

`m23-cli-source-nominals/callables` 和 `properties` 的合法工作副本在
`Reader<T>` 上直接读取接口默认 `size` 时，原前端报 class 没有该属性。
方法查找已经消费继承检查选定的接口实现，属性查找只查询自身与基类。

属性查询现复用同一个 interface implementation selection，以实际 getter
定位逻辑 property，并保留接口 application、可见性和代入后的类型。
getter/setter 继续沿普通成员调用、具体化、接口派发和 GC 路径执行；没有新增
产物字段、第二套默认实现选择或 runtime ABI。

独立用例 `m23-cli-default-properties/nominals` 覆盖 class、object、struct、enum
的直接属性访问，泛型参数与结果、最具体子接口默认 getter、基类实现优先、
接口视图，以及默认 setter 的赋值、复合赋值和后缀更新。provider 内执行本地
访问，下游只消费产物后再次访问；移走源码后独立链接，普通和移动 GC 均运行。
两个原名义声明组合继续检查保护域、私有 helper、泛型方法和属性更新。

验证：HIR 1294 项单元测试通过；上述三个 CLI 用例只读通过，共 20 次进程、
35 份阶段／链接 golden、11 项产物指纹。原名义类型批次已有的 108 份阶段输出
逐字不变，新输出只补齐此前未成功生成的部分。完整 M23-11 验收另行进行。

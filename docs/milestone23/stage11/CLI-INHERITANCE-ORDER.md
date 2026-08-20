# 多文件继承声明顺序

原名义声明组合在普通 factory 文件中增加 `Concrete<T> : Abstract<T>` 后，
factory 先于声明基类的 main 文件被读取。覆盖检查按该顺序访问方法，读取
到尚未建立 virtual family 的基类方法，触发内部 unreachable。

HIR 现在以已解析的源码父类型关系排列本地 owner，先完成父声明的属性、
accessor 和普通方法覆盖检查，再处理派生声明；同一 owner 内保持原成员
顺序。外来声明继续使用已有完整关系，身份、wire 和 ABI 不变。遍历按
实际 typed owner 进行，71 行的顺序处理放在独立模块中，主文件为 167 行。

`m23-cli-inheritance-order/multifile` 在旧二进制上复现同一失败，修复后通过
三层逆文件顺序的泛型继承、接口实现、再抽象化、方法与属性调用，分别消费
Int 和 String。原 `m23-type-source-nominals/declarations.scoop` 保留为
`m23-cli-source-nominals/declarations` 的 provider 基线，再通过实际工厂和
消费程序组合值类型、默认构造、object 与泛型抽象类。

两个用例只读通过 13 进程、22 份阶段与计划 golden、7 个产物指纹，均移走
源码后独立链接并运行普通/移动 GC。同期绑定和普通继承回归已有的 24 份
阶段、原声明组合的 4 份 provider 阶段逐字不变。

1294 项 HIR 单元测试全部通过。全量运行还定位到三个仍读取已迁移快照的
旧入口，其清理与结构断言保留单独记录在 [core 布局迁移](CLI-CORE-LAYOUTS.md)。
对应六个正式 CLI 用例也只读通过 12 进程、48 份阶段 golden。
本记录不代表 M23-11 全仓验收完成。

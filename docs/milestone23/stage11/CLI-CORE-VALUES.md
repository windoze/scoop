# core 默认导入的 object 值

`m23-cli-core-nominals/storage-roots` 在实际重建 core 后，通过 `Storage.flags()`
可以访问单例成员，裸引用 `Storage` 却报 unknown variable。默认导入收集中的
目标列表漏掉了公开 `ObjectValue`；当前修复补全该已有 binding 分支，继续使用
普通值查找、singleton identity、初始化和访问优先级。

独立用例 `m23-cli-core-values/prelude` 重建含公开单例的 core，下游不写显式
import，检查两次裸引用的对象身份、方法、Int／String 状态读写、closure 捕获
和局部同名变量遮蔽。组合用例保留原 storage-roots 源码，先生产原 core，再只
在合法的单例内部添加观察函数，检查 enum 和 Option 私有存储。两者均移走源码
后独立链接，执行普通与移动 GC。

这两项只读通过：11 次进程、22 份阶段／链接 golden、5 项产物指纹。原有
core-library 13 项、core-dependencies 3 项全部只读通过，合计 83 次进程和
104 份 golden，既有期望与指纹没有变化。HIR 1294 项单元测试通过。

此次补全不修改产物格式或 runtime ABI。原 core 名义声明批次已有的 32 份
阶段输出逐字相同；完整 M23-11 验收另行进行。

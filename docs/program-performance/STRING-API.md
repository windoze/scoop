# String 的字符扫描 API

公开字符数量属性改为 `charLength: Long`，字符定位改为普通方法 `getCharAt(idx: Long): Char`。两者仍从现有 UTF-8 内容扫描，最坏时间为 O(byteLength)；`byteLength` 继续直接读取既有字节数，时间为 O(1)。移除旧 `length`、`get` 和 `IndexGet<Long, Char>` 实现，因此 `str[n]` 不再合法。不保留旧名称别名，也不增加字符缓存、索引结构或对象字段。

本项是所有 profile 一致的 API 调整，没有优化开关。内部 `coreStringLength`、`coreStringGet` 及 runtime 入口、String 对象头和字节存储保持；异常仍为 `IndexOutOfBoundsException`，索引以 Unicode 标量计数，组合字符分别计数。顺序遍历继续使用 `Iterable<Char>`；需要随机访问快照时可用 `toCharArray()`。

三份 spec、内置库、测试、示例与仓库基准的实际 String 调用一并迁移；其他容器、buffer、range 的长度不改名。两个验证通用 `IndexGet` 的用例改用字符数组。原始性能输入保存在仓库忽略目录的副本中；脏卡组对照继续消费保存的旧 API 系统库，避免把 API 调整混入 GC 对比。

本机已通过 5 个 core consumer 测试，覆盖 Unicode、NUL、组合字符、具名参数、上下界、空串、迭代、快照与跨 Cone；std 的 text、encoding、uuid 和通用索引共 19 项通过。三个旧 API 编译失败用例核对诊断及位置；String 表示／读操作的 debug、release 专测及迁移后 15 个 smartcast 诊断合计 22 个 fixture 变体通过。22 个本机生产库完成 release 构建。

GNU／musl 各完成 21 个生产库构建，并各通过 5 项 core consumer、9 项 std 代表性测试及 8 个正式 fixture 变体。库行为仍使用所属 Cone 的测试源码，临时 manifest 仅按已有 `test.sources` 规则缩小编译集合；没有复制另一套永久行为测试或运行全量验收。三目标确认旧 API 拒绝、新名称跨预编译 core 可用，String 表示与原定位／异常 ABI 保持。

整组 core consumer 的构建在修改前的 `00b272145` 和本次版本中都遇到同一个 stackmap location-count 不一致。相关 5 项测试通过临时 manifest 的显式 `test.sources` 选择运行原始 Cone 测试源码，没有删除测试或改动编译器验证；该既有问题不在本项扩大处理。原始日志保存在仓库忽略的 `tmp/program-performance/string-api/`。

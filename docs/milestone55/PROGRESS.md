# M55 实施进度

## M55-1：负向接口条件与条件重载

2026-10-19 完成本机功能闭环：函数级 `!Interface<...>`、三态排除关系、约束转发与条件重载、规范化声明身份、完整 bound 的跨 Cone 导出和导入。负向事实不参与猜测类型实参；普通泛型正文保持定义处选择。重复和正负矛盾提供关联位置。

正式 CLI 覆盖 28 个 fixture：两个本地运行组合、25 个错误规则、一组 provider／facade／consumer 的跨 Cone 运行。组合包括静态基类、完整接口实参、kind、局部函数、泛型成员、扩展、默认值、函数引用及同类型递归。没有保存普通程序的 IR 快照。

本机验证通过：workspace 格式化与 all-target Clippy；parser 506 项、identity 351 项、HIR 862 项（含增量新增测试）、HIR lowering 回归 1,479 项与随后新增递归测试、slib 250 项。推断修复后定向重跑新特性 5 项、调用推断 34 项、既有递归检查 3 项；正式 CLI 28 项均通过。

已闭合的问题：空条件字段曾改变无条件声明身份，改为省略空集合；格式版本与固定向量同步升级；同类型递归曾被推断成 `Any`，现复用已物化的相同类型界；跨 Cone 测试的 class 构造器已显式声明 public。identity schema 为 2、metadata outer schema 为 3；runtime ABI 不变。

原始日志及运行目录保存在仓库外。Linux 环境已在 `nuc12:~/repos/scoop-m55-validation` 建立目录，完整验证留在本机实现收尾之后。M55-2／3 与里程碑最终验收尚未完成。

## M55-2：companion 投影与 JSON 默认入口

2026-10-19 完成本机功能闭环。函数自身参数可声明 `T.Companion` 的存在、正向和负向条件；投影用于签名、默认值和正文，并沿既有 bound 成员调用及 singleton 初始化路径具体化。宿主确定后，companion 的正向关系继续提供普通类型推断；投影不反向猜测宿主。JSON 新增分别要求 ValueEncoder／ValueDecoder 的单参数入口，显式 codec 重载保留。

本机格式化与 all-target Clippy 通过；受影响 crate 回归为 parser 506、identity 351、HIR 863、HIR lowering 1,483、slib 250 项。正式 CLI 共 21 个 fixture 已通过：两个本地组合、17 个错误规则、一组 provider／facade／consumer、一个 JSON 实际消费者。JSON 同时覆盖两方向独立实现、派生 DTO、命名 companion、显式替换 codec、泛型转发和强制 GC 移动；跨 Cone 覆盖消费方隐藏宿主、泛型宿主、默认投影值、成员泛型与函数引用。

已闭合的问题：开放投影具体化时保留依赖泛型 singleton 模板；跨 Cone 签名查询按 typed host 查到实际所属模块及必要的隐藏声明；wire 未知 tag 测试避开新增的投影 tag。签名投影、独立 companion binder 与默认 singleton 投影并入 M55 的整体格式迁移，最终格式／兼容性检查在 M55-4 收尾。runtime ABI 不变。

开发 sysroot 的 12 个 Cone 已显式重新准备。清理约 15 GiB 的 debug incremental 缓存，保留可运行工具；日志与工作目录均在仓库外。M55-3、声明重建优化及 Linux 完整验收继续进行。

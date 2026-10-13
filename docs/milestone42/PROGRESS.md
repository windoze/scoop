# M42 实施进度

目标范围以 [设计](DESIGN.md) 和 [标准基础库规范](../specs/SCOOP-STDLIB-SPEC.md) 为准。本文记录实际完成的增量，不代表 M42 整体已验收。

## M42-1：基础接口、比较器与列表

日期：2026-10-13。新增普通 `scoop.std` Cone，唯一直接依赖为 core。

- core 增加普通 Collection 接口，List 继承该接口；Array、MutableArray、ArrayList 自动沿原有关系取得 Collection 能力。保留 MutableList.add 返回 Unit 与原有 live iterator 行为。
- collections 再发布原集合实体，提供 isEmpty/isNotEmpty、emptyList/listOf/mutableListOf/arrayListOf、toList/toMutableList。ListSnapshot 只有只读接口，使用独立 backing；浅复制保留元素的值／引用语义。
- comparisons 提供 Comparator、comparatorOf、naturalOrder/reverseOrder、compareBy、thenBy、reversed。反向比较交换实参，避免对 Long 最小值取负；次级 selector 只在主比较相等时调用。
- 修正文档对 core package 的描述：当前 core 源码位于默认根 package，public import 使用真实根 binding，不从 Cone 名合成限定名称。显式 core 版本声明使用产物搜索，fixture 按现有 JSON 用例先构建 core 产物。

实现全部为普通 Scoop 源码，没有 compiler、driver/linker、IR/meta 或 runtime 改动。尚未发现需修复的底层实现缺陷。

### 验证与证据

使用重建后的配套 `scoop`、`scoopc`、`scoop-link`。本机目标为 `aarch64-apple-darwin`，工作目录为 `target/m42/foundation`；所有命令的 TMPDIR 都显式设为仓库内 `target/m42/scratch`。Scoop 源码按现有格式编写，各批测试前运行 `git diff --check`；未修改 Rust/C/Python 实现，无无关语言的 lint 或全量测试。

新增 `tests/fixtures/m42-foundation` 七个正式 fixture：Collection 的 core-only 消费；列表工厂、快照、值元素与嵌套 Option；比较器组合与回调异常；独立 core/std 产物构建、删除源码后的下游编译与 artifact-only link；缺失 std 依赖、Compare bound、不变泛型的精确 negative 诊断。四个运行用例均覆盖普通运行和 moving GC。

本机已覆盖通过 7 个新 fixture 和 15 个受影响的原有 fixture，没有未闭合失败。为减少重复测试，三个报告合并记录本批证据：

| 报告（仓库 target/m42 下） | 有效通过项 |
| --- | --- |
| foundation-components-report.json | lists、comparisons 两项；该轮其他失败随后修正 |
| foundation-regression-report.json | Collection、artifact-only 两项和 15 项 core 回归；三个新负例当时尚未填入诊断期望 |
| foundation-negative-report.json | 填入已核对的精确诊断后，三个新负例全部通过 |

执行入口均为 `python3 tests/run_fixtures.py --work-dir target/m42/foundation`。第一组使用 `--suite tests/fixtures/m42-foundation --jobs 4`；回归组使用 `--jobs 4`，选择五个此前待闭合的新例，以及 `m26-lists-*`、`m26-list-reject-*`、`m26-array-construction-basic`、`m26-array-construction-cross-cone`、`m26-interpolation-basic`、`m26-interpolation-aggregate-order`、`m29-container-decoding-sequences`。最后使用本组 suite 和三个负例的 exact name filter，`--jobs 3`，结果 3/3 通过。既有回归覆盖 List/Array 的接口、迭代、GC、跨 Cone、错误诊断，以及 StringBuilder 插值和 sequence codec。

本批不宣称 Linux 或整个 M42 已验收；GNU/musl 在后续本组完整定向验收中执行。

### 构建目录维护

开始实施前删除旧测试可执行文件及旧 incremental 缓存，保留配套 CLI，target 从约 39 GB 降至 33 GB。重建当前 CLI 后继续保留必要编译缓存；后续在无运行中构建时清理已完成 fixture 的临时产物，保留报告。

## M42-2：无序 Map／Set

日期：2026-10-13。新增 Map/MutableMap、Set/MutableSet、Map.Entry、HashMap/HashSet、hashMapOf/hashSetOf，以及 NoSuchElementException/ConcurrentModificationException。Map 提供 keys/values/entries 的只读 live view、缺失处理和 containsValue 扩展；键和值的 Option 层级保持完整。Set 复用 HashMap 的不替换插入，重复 add 不产生修改。

哈希存储使用线性探测和三态槽，entry 保存完整缓存 hash 与 typed key/value；rehash 复用 hash，remove/clear 重置 managed 槽。iterator 引用容器，不持有旧 backing 快照。普通写入与结构修改分开计数：替换 value 不使 iterator 失效，回调重入替换仍会被外层操作检测；计数回绕时更换局部 epoch 对象。实现文件按协议、views、存储、map、set、修改检测拆分，最大文件 127 行。

本批仍无底层宿主实现改动。算法资料与许可核对见设计 3.2。

### 验证与证据

先运行 `git diff --check`，再执行：

```sh
TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py \
  --suite tests/fixtures/m42-hash \
  --work-dir target/m42/foundation --jobs 4
```

本机 `aarch64-apple-darwin` 最终 7/7 fixture、24 个进程通过，报告另存为 `target/m42/hash-report.json`。五个运行用例均包含普通和 moving GC 执行，两个 negative 精确验证缺少 Hash／Equals bound 的位置与信息。覆盖同 hash 碰撞、跨删除槽查找、增长／墓碑清理、原 key 保留、live view、Entry 独立值、永久耗尽与失效 iterator、hash/equals 异常及重入、producer 语义、嵌套 Option、零大小 key、命名 set、引用释放，以及与独立数组模型逐次对照的 600 次操作。

复用同一工作目录时，切换 suite 前先清理已完成的 `cases` 子目录，避免旧测试留下的显式 std 产物优先于新源码；保留共享编译缓存和具名报告。源码变化时同步 runner 的 sysroot 副本。最终验收仍待 M42-3 至 M42-12 与 Linux 两目标闭合。

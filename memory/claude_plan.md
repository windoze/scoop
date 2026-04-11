# 当前执行计划

## 已确认的上下文

1. 已检查最新提交 `307477d [T0154] 支持 higher-order aggregate 返回值`。
2. 最新提交说明、`TODO.md` 与 `PLAN.md` 中未发现“必须先于新任务修复”的遗留 issue。
3. 执行开始时，`TODO.md` 中第一个未完成任务是 `T1818 [TODO] Hash-based Set/Map（Int key）`。
4. 任务规模可控，本次不拆分子任务，直接实现 `T1818`。

## 本次目标

完成 `T1818`：将当前 `stdlib/collections_set.scoop` 与 `stdlib/collections_map.scoop` 的线性扫描实现替换为基于开放寻址（linear probing）的哈希表实现，并补齐对应回归。

## 最终实现方案

1. 保持现有公开 API 名称不变：
   - `mutableIntSet` / `MutableSet.add` / `contains` / `remove` / `len` / `asSet`
   - `mutableIntIntMap` / `MutableMap.put` / `getByKey` / `getOrDefault` / `containsKey` / `removeKey` / `entryCount` / `asMapView`
2. 继续沿用当前 `typealias` 方案，避免引入新的运行时布局或编译器特殊化。
3. 将底层 `MutableArray<Int>` 的内容从“顺序元素列表 / flat kv 列表”改为“表头 + 槽位数组”的开放寻址布局：
   - Set：保存 `size`、`capacity` 与每个槽位的状态位/键值；
   - Map：保存 `size`、`capacity` 与每个槽位的状态位/键值/值。
4. 写操作保持既有“返回新集合/新映射”的表面语义；删除通过重建新表清掉 tombstone 需求。
5. `asSet()` / `asMapView()` 继续导出只读顺序视图，避免把内部槽位布局暴露给外部。
6. 为兼容当前 `typealias` surface 下只读/可变扩展可能共享路由，mutable 侧查询 API 增加“哈希 backing / 顺序视图”自动识别。
7. 回归至少覆盖：
   - 冲突探测；
   - 重复插入不增 size；
   - 更新已存在 key；
   - 删除后继续查询/插入；
   - `asSet` / `asMapView` 导出结果正确。

## 执行步骤

1. 修改 `stdlib/collections_set.scoop`。
2. 修改 `stdlib/collections_map.scoop`。
3. 新增或更新 `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 与 `.stdout`。
4. 运行格式化与定向/全量验证。
5. 更新 `TODO.md`、`PLAN.md` 与本文件。
6. 提交本次变更并停止。

## 进度

- 已完成：初始计划写入。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`，确认本次目标为 `T1818`。
- 已完成：实现开放寻址版 `HashSet<Int>` / `HashMap<Int, Int>`，并保留只读顺序视图导出。
- 已完成：新增 `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` + `.stdout`，覆盖同桶冲突、重复插入、删除后重建、map 更新与只读视图导出。
- 已完成：定向验证 `stdlib_hash_set_map_basic.scoop` 单文件 build/run 通过。
- 已完成：全量验证通过：
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (901)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- 已完成：更新 `TODO.md`、`PLAN.md` 与 `STDLIB_COMPLETENESS.md`。
- 待完成：整理变更并创建本次任务 commit，然后停止。

## 当前执行计划

1. 已检查最新提交；未发现需要先行修复的新增遗留问题说明。
2. 已定位首个未完成任务为 `T5001d1`，并判断当前可直接实施，无需先拆分子任务。
3. 已在编译器中实现函数级 explicit frame layout 规划与 descriptor/offset table 发射：
   - 进入 managed body lowering 时开始收集 layout；
   - 退出时统一发射 `ScoopExplicitRootFrame$...`、`__scoop_explicit_root_offsets__*`、`__scoop_explicit_root_desc__*`；
   - 布局按 GC leaf slots 展开，并补入 ordinary indirect GC aggregate params。
4. 已补三条编译器回归测试，覆盖 direct ref local、indirect aggregate flattening、hidden-sret caller temp。
5. 已完成 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
6. 下一步：更新任务文档并创建一次 git 提交，然后停止。

## 说明

- 这里记录的是可审阅的执行计划与进度摘要，不包含内部推理细节。

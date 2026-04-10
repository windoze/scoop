# Current Task: T0127 — 泛型验证与修复：泛型函数边界场景

## Status: IN PROGRESS

## 执行计划

### 需要验证并修复的场景
1. **多类型参数** `fun <A, B> pair(a: A, b: B): Pair<A, B>`
2. **传递实例化** `fun <T> wrap(v: T) = Box<T>(v)`
3. **泛型扩展函数** `fun <T> T.toBox(): Box<T>`
4. **泛型高阶函数** `fun <T, R> myMap(v: T, f: (T) -> R): R`
5. **泛型递归** `fun <T> foo(x: T, n: Int): T` (with base case)
6. **类型参数约束** → 推迟到 T0129/T0130

### 注意事项
- monomorph lower 中 `index_file_fun_decls` 只索引 `ast::Item::Fun`
- cross-file 泛型函数实例化存在 gap（但对当前单文件场景不影响）
- 泛型递归需验证 recursive call FQN 解析

### 当前进度
- [x] 分析完成
- [ ] Step 1: 多类型参数 fixture
- [ ] Step 2: 传递实例化 fixture
- [ ] Step 3: 泛型扩展函数 fixture
- [ ] Step 4: 泛型高阶函数 fixture
- [ ] Step 5: 泛型递归 fixture
- [ ] Step 6: 最终验证和提交

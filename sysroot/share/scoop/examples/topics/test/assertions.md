## 运行

保存 manifest、src/ 与 tests/ 的全部文件，在项目根执行：

```sh
scoop test --release .
scoop test --release --list .
scoop test --release --filter "zero count" .
```

## 预期结果

第一次执行两个测试并全部通过；--list 列出两个用例；--filter 仅执行名字包含 zero count 的用例。测试耗时等诊断可能变化，不作为固定文本。

## 关键说明

生产库是普通 library Cone，scoop.test 仅放在 test-dependencies 中。测试源码位于 tests/，package 与所属 Cone group 对齐；@Test 标注容器和用例。

第一个用例断言返回值与布尔条件，第二个用 assertFailsWith 捕获预期的 IllegalArgumentException，再检查可选 message。assertEquals 使用相应类型的相等能力，不能假设任何用户 struct 都自动满足泛型断言入口的要求。

--filter 与 --list 是 scoop test 选项，不是传给测试 main 的参数。修改 divide 业务规则时同步更新相关断言。

## 常见误用

不要用 scoop run 运行 library 或只复制 tests 文件。测试依赖不应为了方便塞进生产 dependencies；完整 manifest 让工具知道何时需要 scoop.test。

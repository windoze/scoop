## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
Found: Ada
No matching name
```

## 关键说明

Lookup 的两个变体拥有不同形状：Found 带 String，Missing 没有载荷。enum 值是带标签的值类型；构造时明确选择变体。`case Lookup.Found(name)` 解构并绑定载荷，Missing 分支处理缺失。

给 enum 增加变体时，也要更新这个需要结果的 when，让分支保持穷尽。可以替换查找逻辑，但仍通过 Lookup 返回实际找到的数据或缺失状态。

## 常见误用

不要把它当成只能列常量的 Kotlin enum class。匹配载荷使用 case；普通绑定声明不用于可能失败的变体筛选。

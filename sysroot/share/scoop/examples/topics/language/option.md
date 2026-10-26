## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
Hello, Ada!
Hello, guest!
3
```

## 关键说明

`T?` 是 `Option<T>` 的简写。`Some(value)` 携带数据，`None` 表示缺失；没有 null 值。`when` 显式解构后，value 的类型是 String。

`name?.charLength` 得到 `Long?`，`?: 0L` 提供缺失时的 Long。需要不同缺失行为时，修改 None 分支或 Elvis 的右侧即可。Option 泛型不变；嵌套 `T??` 表示两层 Option。安全调用不会把返回值原有的 Option 自动展平。

## 常见误用

不要写 `if (name != null)`；Scoop 不提供这种 null 判空或 Option 判空智能转换。需要值时用 case 解构，不要假设检查过 None 后变量本身就变成 String。

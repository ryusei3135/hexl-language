# 構造体・列挙型について

## 構造体
- 定義方法
    ```
    struct Name {
        mem: int
        mem2: int
    }
    ```
- 初期化方法
    ```
    a: Name = Name {
        mem: 1
        mem: 10
    }
    ```

## 列挙型
- 定義方法
    ```
    enum Name {
        Mem
        Mem2
    }
    ```
- 初期化方法
    ```
    a: Name = Name::Mem
    ```

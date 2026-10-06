Neither copyable:

```compile_fail,E0599
let mut table = moorings::Moorings::new();
let key = table.try_insert(1).expect("memory");
let copy = key.clone();
```

nor usable once its value is removed:

```compile_fail,E0382
let mut table = moorings::Moorings::new();
let key = table.try_insert(1).expect("memory");
table.remove(key);
table.get(&key);
```

nor usable on a table of something else:

```compile_fail,E0308
let mut numbers = moorings::Moorings::new();
let mut names = moorings::Moorings::new();
let key = numbers.try_insert(1u32).expect("memory");
names.try_insert("one").expect("memory");
names.get(&key);
```

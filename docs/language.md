# Meow Language Reference

> Complete syntax and type-system reference for the Meow smart contract language.

The Meow Language compiles to stack-based bytecode executed by the Meow VM. See [Contracts](contracts.md) for the practical contract guide (CLI, randomness, time, entry points); see [Adapter & Natives](adapter.md) for native function details, object lifecycle, and the bytecode verifier.

## Module declaration

Every source file must begin with a `mod NAME;` declaration. The name becomes the module's human-readable identifier. Only one declaration is allowed per file.

```meow
mod my_module;
```

## Imports

Use `use` declarations to import other published modules. The `@<address>` suffix is the 32-byte on-chain address of the module.

```meow
mod my_game;

use meow_object@0x01;
use meow_coin@0x10;
use math@0x1a2b3c...;
```

After importing, reference types and functions with `module_name::TypeName` or `module_name::function_name(...)`. Duplicate `use` names in the same module are rejected.

### Using imported types and functions

- **Functions**: `module_name::function_name(args)` — only `pub fn` can be called cross-module.
- **Types**: use `module_name::TypeName` as a type annotation. Receive values via `pub fn` return values or parameters. Direct construction (`module_name::TypeName { ... }`) is always rejected — use a constructor function exported by the dep module.

```meow
use shapes@0x...;
fn run() -> u64 {
    let p = shapes::make_point(3, 7); // ok — uses constructor
    // let p = shapes::Point { x: 3, y: 7 }; // rejected — cross-module construction
    return shapes::get_x(p);  // ok — uses getter
}
```

### Transitive dependencies

When publishing, declare all direct dependencies via `use`. The compiler validates the full transitive closure — every module reachable from the declared imports must be provided. Circular dependencies are rejected.

## Comments

```meow
// Single-line comment — everything from // to end of line is ignored.
```

Multi-line comments are not supported. `//` inside a string literal is not treated as a comment.

## Types

| Type | Semantics | Notes |
|------|-----------|-------|
| `bool` | Value | `true` or `false`; freely copyable |
| `u64` | Value | 64-bit unsigned integer; freely copyable |
| `address` | Value | 32-byte identifier, written as `@0x<hex>`; freely copyable |
| `string` | Value | UTF-8 string literal; freely copyable |
| `struct <Name>` | Move | User-defined named record; must be explicitly consumed |
| `(T1, T2, ...)` | — | Tuple; used only as function return types |

### Move semantics

Every struct value has move semantics. A struct held in a local variable must be consumed before the function returns. Consumption means one of:

- Passed to a function call (the callee takes ownership).
- Returned from the current function.
- Destructured: `let Name { field, .. } = value;` extracts fields; `..` discards the rest.

The compiler and bytecode verifier both enforce this. A live struct in a non-parameter local slot at `Return` is a compile error.

Function parameters are exempt from the unconsumed check at `Return` — a live struct parameter is treated as an in-place mutation and written back by the effects system.

## Structs

```meow
struct Point { x: u64, y: u64 }
pub struct Coin { id: meow_object::Id, balance: u64 }
```

- `pub` makes the struct type visible to other modules that import this one.
- Fields are always private — readable and writable only within the declaring module.
- Field types may be primitives (`bool`, `u64`, `address`, `string`) or other struct types. Tuples are not valid field types.

### On-chain objects

A struct whose first field is `id: meow_object::Id` (from `meow_object@0x01`) is an **on-chain object**. The adapter recognises this layout and tracks the struct through the object store.

```meow
use meow_object@0x01;

pub struct Hero {
    id: meow_object::Id,   // first field — marks this as an on-chain object
    name: string,
    level: u64
}
```

See [Adapter & Natives — On-chain object lifecycle](adapter.md#on-chain-object-lifecycle) for how objects are created, mutated, transferred, and destroyed.

### The `id` field is immutable

The `id` field of every on-chain object is set at creation time and cannot be reassigned anywhere — even inside the declaring module:

```meow
use meow_object@0x01;
struct Coin { id: meow_object::Id, balance: u64 }
fn bad(c: Coin, new_id: meow_object::Id) {
    c.id = new_id; // compile error: 'id' is immutable
}
```

### Struct literals

Struct literals create a new struct value. All fields must be provided in any order.

```meow
let p = Point { x: 3, y: 7 };
let hero = Hero { id: meow_vm_fresh_id(), name: "Thorin", level: 1 };
```

Construction is always module-local — you cannot write a struct literal for a type declared in another module. Use a constructor function instead.

### Struct destructuring

Destructuring extracts fields from a struct, consuming it.

```meow
let Point { x, y } = p;         // binds x and y
let Hero { id, .. } = hero;     // binds id, discards the rest
```

`..` discards unbound fields. After destructuring, the original binding is consumed and no longer accessible.

## Functions

```meow
fn add(a: u64, b: u64) -> u64 {
    return a + b;
}

pub fn spawn(name: string) {
    // ...
}
```

- `pub` makes the function callable from other modules and directly from transactions.
- Parameters are positional; each has a name and a type.
- The return type is optional; omitting it means the function returns nothing.
- Tuple return types `(T1, T2, ...)` allow returning multiple values.

### Return

```meow
return expr;      // explicit return with value
return;           // explicit void return
expr              // implicit return — last expression without semicolon
```

### Multiple return values

A function can return multiple values using a tuple `(T1, T2, ...)` as the return type. This is the standard pattern for getter functions that need to return both a (possibly mutated) struct and a derived value — since fields are private cross-module, a getter must return the struct back to the caller:

```meow
// shapes module
pub struct Point { x: u64, y: u64 }
pub fn make_point(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }
pub fn get_x(p: Point) -> (Point, u64) {
    let val = p.x;
    (p, val)   // implicit return
}

// user module
use shapes@0x...;
fn use_x(p: shapes::Point) -> u64 {
    let (p, val) = shapes::get_x(p);
    return val;
}
```

Structs can appear in return tuples, following the same move semantics — the caller receives ownership and is responsible for consuming them.

## Statements

| Statement | Syntax |
|-----------|--------|
| Variable binding | `let name = expr;` |
| Tuple destructure | `let (a, b) = expr;` |
| Struct destructure | `let Name { field, .. } = expr;` |
| Reassignment | `name = expr;` |
| Field assignment | `obj.field = expr;` |
| Return | `return expr;` or `return;` |
| Conditional | `if cond { ... }` or `if cond { ... } else { ... }` |
| Bare expression | `expr;` (value discarded) |

### `let` bindings

`let` introduces a new local variable with inferred type. Struct values have move semantics — `let` moves the value into the binding.

```meow
let x = 42;
let msg = "hello";
let coin = MeowCoin { id: meow_vm_fresh_id(), balance: 100 };
```

### Reassignment

Reassignment (`name = expr;`) updates an existing binding. Reassigning a binding that holds a live struct is a compile error — consume or destructure it first.

```meow
x = x + 1;
```

### Field assignment

Mutates a field on a struct held in a local slot. The `id` field is immutable and cannot be assigned.

```meow
hero.level = hero.level + 1;
hero.name = "Veteran";
```

### `if` / `else`

```meow
if cond { ... }
if cond { ... } else { ... }
```

Both branches must leave the stack in the same state (same types, same struct liveness). Using `if` without `else` is valid when neither branch creates struct values that survive past the branch.

## Expressions

### Literals

```meow
true            // bool
false           // bool
42              // u64
@0x01           // address (left-padded to 32 bytes)
"hello world"   // string
```

### Variables

Reading a struct variable moves it out of the binding (the binding becomes dead). Reading a primitive copies it.

### Arithmetic

`+` `-` `*` `/` `%` — operands must be `u64`; result is `u64`.

### Comparison

`==` `!=` — accepts two values of the same type; produces `bool`.  
`<` `<=` `>` `>=` — accepts two `u64` values; produces `bool`.

### Boolean logic

`&&` `||` — both operands must be `bool`.  
`!` — unary not; operand must be `bool`.

### Field access

```meow
hero.level          // read field 'level' from hero
coin.id             // read the id field (returns meow_object::Id)
```

Field access reads a field without consuming the parent struct. For primitives the result is a copy; for struct-typed fields the result is a struct value.

### Function calls

```meow
add(1, 2)                  // local function
math::scale(x, 2)          // cross-module function
meow_vm_fresh_id()         // native function
```

## Operator precedence

From highest to lowest:

1. Unary `!`
2. `*` `/` `%`
3. `+` `-`
4. `<` `<=` `>` `>=`
5. `==` `!=`
6. `&&`
7. `||`

## Native functions

These built-ins are always available and cannot be defined by user code. See [Adapter & Natives](adapter.md#native-functions) for full descriptions and gas costs.

| Function | Signature |
|----------|-----------|
| `meow_vm_fresh_id()` | `() → meow_object::Id` |
| `meow_vm_transfer(obj, owner)` | `(struct, address) → void` |
| `meow_vm_destroy(id)` | `(meow_object::Id) → void` |
| `meow_vm_sender()` | `() → address` |
| `meow_vm_rand()` | `() → u64` |
| `meow_vm_timestamp()` | `() → u64` |
| `meow_vm_abort(cond, code, msg)` | `(bool, u64, string) → void` |

## Access control

All functions and structs are **private by default**. The `pub` keyword makes them accessible from other modules. Fields are always private — readable and writable only within the declaring module.

| Declaration | Visibility |
|-------------|------------|
| `fn f(...)` | Private — callable only within this module |
| `pub fn f(...)` | Public — callable from any module that imports this one |
| `struct S { ... }` | Private — not nameable from other modules |
| `pub struct S { ... }` | Public — other modules can use `S` as a type |
| `field: T` | Always private — only readable/writable within the declaring module |

### Cross-module restrictions

| Operation | Cross-module allowed? |
|-----------|-----------------------|
| Call `pub fn` | Yes |
| Call private `fn` | No |
| Use `pub struct` as a type | Yes |
| Use private struct as a type | No |
| Construct any struct with a literal | No (always module-local) |
| Read any field directly | No (always module-local; use a getter function) |
| Write any field | No (always module-local) |
| Write `id` field | No (immutable everywhere) |

## Limits

| Limit | Default |
|-------|---------|
| Maximum struct definitions per module | 64 |
| Maximum functions per module | 256 |
| Maximum `use` declarations per module | 64 |
| Maximum transitive dependency modules | 64 |
| Maximum function parameters | 16 |
| Maximum tuple elements | 8 |
| Maximum call depth | 256 |

# Meow Language Reference

> Complete syntax and type-system reference for the Meow smart contract language.

The Meow Language compiles to stack-based bytecode executed by the Meow VM. See [Contracts](contracts.md) for the practical contract guide (CLI, randomness, time, entry points); see [Adapter & Natives](adapter.md) for native function details, object lifecycle, and the bytecode verifier.

## Module declaration

Every source file must begin with a `mod NAME;` declaration. The name becomes the module's human-readable identifier. Only one declaration is allowed per file.

```meow
mod my_module;
```

## Imports

Use `use` declarations to import other published modules. The `@<address>` suffix is the 32-byte on-chain address of the module. An optional `as alias` gives the module a different local name.

```meow
mod my_game;

use meow_object@0x10;
use meow_coin@0x20;

use math@0x1a2b3c... as m;  // reference as m:: instead of math::
```

After importing, reference types and functions with `module_name::TypeName` or `module_name::function_name(...)`, or use the alias if one was declared. Duplicate aliases in the same module are rejected. Two modules with the same name but different addresses can both be imported by giving each a distinct alias.

### Using imported types and functions

- **Functions**: `module_name::function_name(args)` — only `pub fn` can be called cross-module.
- **Types**: use `module_name::TypeName` as a type annotation. Receive values via `pub fn` return values or parameters. Direct construction (`module_name::TypeName { ... }`) is always rejected — use a constructor function exported by the dep module.

```meow
use shapes@0x... as geo;

fn run() -> (geo::Point, u64) {
    let p = geo::make_point(3, 7);        // ok — uses constructor
    // let p = geo::Point { x: 3, y: 7 }; // rejected — cross-module construction
    return geo::get_x(p);                 // ok — uses getter; returns (Point, u64)
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

## Structs

```meow
struct Point { x: u64, y: u64 }

pub struct Coin { id: meow_object::Id, balance: u64 }
```

- `pub` makes the struct type visible to other modules that import this one.
- Structs **must have at least one field**. Empty structs are rejected by the compiler.
- Fields are always private — readable and writable only within the declaring module.
- Field types may be primitives (`bool`, `u64`, `address`, `string`) or other struct types. Tuples are not valid field types.

### On-chain objects

A struct whose first field is `id: meow_object::Id` (from `meow_object@0x10`) is an **on-chain object**. The adapter recognises this layout and tracks the struct through the object store.

```meow
use meow_object@0x10;

pub struct Hero {
    id: meow_object::Id,   // first field — marks this as an on-chain object
    name: string,
    level: u64
}
```

The `id` field cannot be reassigned — the compiler rejects any attempt as a compile error. See [Adapter & Natives — On-chain object lifecycle](adapter.md#on-chain-object-lifecycle) for how objects are created, transferred, and destroyed.

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

fn use_shapes(p: shapes::Point) -> u64 {
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

Mutates a field on a struct held in a local slot.

```meow
hero.level = hero.level + 1;
hero.name = "Veteran";
hero.stats.level = 10;   // nested field path — assigns to a field of a field-typed struct
```

Fields whose type is a struct cannot be directly assigned.

```meow
hero.id = new_id;       // error — 'id' cannot be reassigned
hero.stats = new_stats; // error — struct-typed fields cannot be directly assigned; use destructuring
```

### `if` / `else`

```meow
if cond { ... }
if cond { ... } else { ... }
```

Each branch body is a separate scope — see [Variable shadowing and scoping](#variable-shadowing-and-scoping) below.

Both branches must leave the stack in the same state (same types, same struct liveness). Using `if` without `else` is valid when neither branch creates struct values that survive past the branch.

**There are no loop constructs.** `while`, `for`, and `loop` do not exist. This is intentional — every program is guaranteed to terminate and gas consumption is always bounded. The bytecode verifier also rejects backward jumps, making loops impossible at the bytecode level.

### Variable shadowing and scoping

Redeclaring a name with `let` in the same scope *shadows* the previous binding.

**Primitives** can always be shadowed — the slot is reused:

```meow
let x = 1;
let x = 2;   // ok — x is now 2
```

**Structs** can only be shadowed after the previous binding has been consumed (destructured or passed to a function). Shadowing a live struct is a compile error because it would leak the value:

```meow
let p = Point { x: 1, y: 2 };
let p = Point { x: 3, y: 4 }; // error: cannot shadow 'p' — still holds a struct value

let Point { x, .. } = p;      // consume p first
let p = Point { x: 3, y: 4 }; // ok
```

#### Block scoping

Each `if`/`else` branch body is its own block scope. The outer bindings are fully restored when the branch exits.

Variables declared inside a branch body are **not visible** after the branch:

```meow
if cond {
    let inner = 42;
}
return inner; // error: undefined variable 'inner'
```

A name declared in an outer scope can be shadowed inside a branch body — the outer binding is restored when the branch exits:

```meow
let x = 1;
if cond {
    let x = 99; // inner shadow; does not affect outer x
}
return x;       // always 1
```

The same applies to struct bindings. The inner shadow uses a new slot so the outer binding is preserved:

```meow
let p = Point { x: 1, y: 2 };
if cond {
    let p = Point { x: 9, y: 9 };  // new slot — outer p unaffected
    let Point { x, .. } = p;       // inner p must be consumed here
}
let Point { x, .. } = p;           // outer p is still alive
```

Any struct introduced inside a branch body **must be consumed before the branch ends** — leaving a live struct at the end of a branch is a compile error:

```meow
if cond {
    // error: struct 'p' introduced in if body must be consumed before the branch ends
    let p = Point { x: 1, y: 2 };
}
```

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

`==` `!=` — accepts two values of the same type; produces `bool`. Works on all types; structs and tuples are compared field by field.  
`<` `<=` `>` `>=` — accepts two `u64` values; produces `bool`.

### Boolean logic

`&&` `||` — both operands must be `bool`.  
`!` — unary not; operand must be `bool`.

### Field access

```meow
hero.level          // read primitive field 'level' from hero
```

Field access reads a primitive field (`bool`, `u64`, `address`, `string`) without consuming the parent struct — the result is a copy.

Fields whose type is a struct cannot be read via field access. They can only be extracted through destructuring, which consumes the parent struct:

```meow
let Hero { id, name, level } = hero;  // ok — destructuring extracts all fields
```

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
| `meow_vm_transfer(obj, owner)` | `(local struct, address) → void` |
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
| Read primitive field directly | No (always module-local; use a getter function) |
| Read struct-typed field directly | No (forbidden everywhere; use destructuring) |
| Write any field | No (always module-local) |

## Limits

See [Adapter & Natives — Transaction types](adapter.md#transaction-types) for the complete table of limits enforced at call time and at module publish time.

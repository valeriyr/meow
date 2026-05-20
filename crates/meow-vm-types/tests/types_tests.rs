use meow_vm_types::{
    address::Address,
    types::{FieldDef, StructDef, Type, Value},
};

//
// ─── Type::from_name ───
//

#[test]
fn type_from_name_primitives() {
    assert_eq!(Type::from_name("bool"), Type::Bool);
    assert_eq!(Type::from_name("u64"), Type::U64);
    assert_eq!(Type::from_name("address"), Type::Address);
    assert_eq!(Type::from_name("string"), Type::Str);
}

#[test]
fn type_from_name_unknown_becomes_struct() {
    assert_eq!(Type::from_name("Foo"), Type::Struct("Foo".to_string()));
    assert_eq!(
        Type::from_name("dep::Bar"),
        Type::Struct("dep::Bar".to_string())
    );
}

//
// ─── Type::name ───
//

#[test]
fn type_name_primitives() {
    assert_eq!(Type::Bool.name(), "bool");
    assert_eq!(Type::U64.name(), "u64");
    assert_eq!(Type::Address.name(), "address");
    assert_eq!(Type::Str.name(), "string");
}

#[test]
fn type_name_struct() {
    assert_eq!(Type::Struct("Foo".to_string()).name(), "Foo");
}

#[test]
fn type_name_tuple() {
    assert_eq!(
        Type::Tuple(vec![Type::U64, Type::Bool]).name(),
        "(u64, bool)"
    );
}

//
// ─── Type::is_primitive ───
//

#[test]
fn type_is_primitive() {
    assert!(Type::Bool.is_primitive());
    assert!(Type::U64.is_primitive());
    assert!(Type::Address.is_primitive());
    assert!(Type::Str.is_primitive());
    assert!(!Type::Struct("Foo".to_string()).is_primitive());
    assert!(!Type::Tuple(vec![Type::U64]).is_primitive());
}

//
// ─── Type::is_valid_field_type ───
//

#[test]
fn type_is_valid_field_type() {
    assert!(Type::Bool.is_valid_field_type());
    assert!(Type::U64.is_valid_field_type());
    assert!(Type::Address.is_valid_field_type());
    assert!(Type::Str.is_valid_field_type());
    assert!(Type::Struct("Foo".to_string()).is_valid_field_type());
    assert!(!Type::Tuple(vec![Type::U64]).is_valid_field_type());
}

//
// ─── Value::type_name — primitives ───
//

#[test]
fn value_type_name_primitives() {
    assert_eq!(Value::Bool(false).type_name(), "bool");
    assert_eq!(Value::U64(0).type_name(), "u64");
    assert_eq!(Value::Address(Address::ZERO).type_name(), "address");
    assert_eq!(Value::Str("hello".to_string()).type_name(), "str");
    assert_eq!(Value::Void.type_name(), "void");
}

//
// ─── Value::type_name — struct / tuple ───
//

#[test]
fn value_type_name_struct() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![],
    };
    assert_eq!(val.type_name(), "Foo");
}

#[test]
fn value_type_name_tuple() {
    assert_eq!(
        Value::Tuple(vec![Value::U64(1), Value::Bool(true)]).type_name(),
        "tuple"
    );
}

//
// ─── Value::as_bool ───
//

#[test]
fn value_as_bool() {
    assert_eq!(Value::Bool(true).as_bool(), Some(true));
    assert_eq!(Value::Bool(false).as_bool(), Some(false));
    assert_eq!(Value::U64(1).as_bool(), None);
    assert_eq!(Value::Void.as_bool(), None);
}

//
// ─── Value::as_u64 ───
//

#[test]
fn value_as_u64() {
    assert_eq!(Value::U64(42).as_u64(), Some(42));
    assert_eq!(Value::U64(0).as_u64(), Some(0));
    assert_eq!(Value::Bool(true).as_u64(), None);
    assert_eq!(Value::Void.as_u64(), None);
}

//
// ─── Value::as_address ───
//

#[test]
fn value_as_address() {
    let addr = Address::fill(0x42);
    assert_eq!(Value::Address(addr).as_address(), Some(addr));
    assert_eq!(Value::U64(0).as_address(), None);
    assert_eq!(Value::Bool(true).as_address(), None);
}

//
// ─── Value::as_str ───
//

#[test]
fn value_as_str() {
    assert_eq!(Value::Str("meow".to_string()).as_str(), Some("meow"));
    assert_eq!(Value::U64(0).as_str(), None);
    assert_eq!(Value::Bool(true).as_str(), None);
}

//
// ─── Value::into_str ───
//

#[test]
fn value_into_str() {
    assert_eq!(
        Value::Str("meow".to_string()).into_str(),
        Some("meow".to_string())
    );
    assert_eq!(Value::U64(0).into_str(), None);
    assert_eq!(Value::Bool(true).into_str(), None);
}

//
// ─── Value::as_tuple ───
//

#[test]
fn value_as_tuple() {
    let elements = vec![Value::U64(1), Value::Bool(true)];
    let tuple = Value::Tuple(elements.clone());
    assert_eq!(tuple.as_tuple(), Some(elements.as_slice()));
    assert_eq!(Value::U64(0).as_tuple(), None);
    assert_eq!(Value::Void.as_tuple(), None);
}

//
// ─── Value::is_void ───
//

#[test]
fn value_is_void() {
    assert!(Value::Void.is_void());
    assert!(!Value::U64(0).is_void());
    assert!(!Value::Bool(false).is_void());
}

//
// ─── Value::field ───
//

#[test]
fn value_field_returns_field_by_name() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![
            ("a".to_string(), Value::U64(1)),
            ("b".to_string(), Value::Bool(true)),
        ],
    };
    assert_eq!(val.field("a"), Some(&Value::U64(1)));
    assert_eq!(val.field("b"), Some(&Value::Bool(true)));
    assert_eq!(val.field("c"), None);
}

#[test]
fn value_field_returns_none_for_non_struct() {
    assert_eq!(Value::U64(42).field("x"), None);
    assert_eq!(Value::Void.field("x"), None);
}

//
// ─── Value::field_u64 / field_bool / field_str / field_address ───
//

#[test]
fn value_field_u64() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![("n".to_string(), Value::U64(99))],
    };
    assert_eq!(val.field_u64("n"), Some(99));
    assert_eq!(val.field_u64("missing"), None);
}

#[test]
fn value_field_bool() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![("flag".to_string(), Value::Bool(true))],
    };
    assert_eq!(val.field_bool("flag"), Some(true));
    assert_eq!(val.field_bool("missing"), None);
}

#[test]
fn value_field_str() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![("name".to_string(), Value::Str("meow".to_string()))],
    };
    assert_eq!(val.field_str("name"), Some("meow"));
    assert_eq!(val.field_str("missing"), None);
}

#[test]
fn value_field_address() {
    let addr = Address::fill(0x42);
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![("owner".to_string(), Value::Address(addr))],
    };
    assert_eq!(val.field_address("owner"), Some(addr));
    assert_eq!(val.field_address("missing"), None);
}

#[test]
fn value_field_returns_none_for_type_mismatch() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![("x".to_string(), Value::U64(1))],
    };
    assert_eq!(val.field_bool("x"), None);
    assert_eq!(val.field_str("x"), None);
    assert_eq!(val.field_address("x"), None);
}

//
// ─── StructDef::field_index ───
//

#[test]
fn struct_def_field_index() {
    let def = StructDef {
        name: "Foo".to_string(),
        fields: vec![
            FieldDef {
                name: "a".to_string(),
                ty: Type::U64,
            },
            FieldDef {
                name: "b".to_string(),
                ty: Type::Bool,
            },
            FieldDef {
                name: "c".to_string(),
                ty: Type::Address,
            },
        ],
        is_public: true,
    };
    assert_eq!(def.field_index("a"), Some(0));
    assert_eq!(def.field_index("b"), Some(1));
    assert_eq!(def.field_index("c"), Some(2));
    assert_eq!(def.field_index("z"), None);
}

//
// ─── StructDef::name and fields accessors ───
//

#[test]
fn struct_def_name() {
    let def = StructDef {
        name: "Bar".to_string(),
        fields: vec![],
        is_public: false,
    };
    assert_eq!(def.name, "Bar");
}

#[test]
fn struct_def_fields() {
    let def = StructDef {
        name: "Baz".to_string(),
        fields: vec![
            FieldDef {
                name: "x".to_string(),
                ty: Type::U64,
            },
            FieldDef {
                name: "y".to_string(),
                ty: Type::U64,
            },
        ],
        is_public: true,
    };
    assert_eq!(def.fields.len(), 2);
    assert_eq!(def.fields[0].name, "x");
    assert_eq!(def.fields[0].ty, Type::U64);
    assert_eq!(def.fields[1].name, "y");
    assert_eq!(def.fields[1].ty, Type::U64);
}

//
// ─── Type::is_cross_module ───
//

#[test]
fn qualified_struct_is_cross_module() {
    assert!(Type::Struct("dep::Foo".to_string()).is_cross_module());
}

#[test]
fn local_struct_is_not_cross_module() {
    assert!(!Type::Struct("Foo".to_string()).is_cross_module());
}

#[test]
fn primitives_are_not_cross_module() {
    assert!(!Type::Bool.is_cross_module());
    assert!(!Type::U64.is_cross_module());
    assert!(!Type::Address.is_cross_module());
    assert!(!Type::Str.is_cross_module());
}

#[test]
fn tuple_is_not_cross_module() {
    assert!(!Type::Tuple(vec![Type::U64, Type::Bool]).is_cross_module());
}

//
// ─── Value::uses_move_semantics ───
//

#[test]
fn value_uses_move_semantics_for_struct() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![],
    };
    assert!(val.uses_move_semantics());
}

#[test]
fn value_uses_move_semantics_for_tuple_containing_struct() {
    let val = Value::Tuple(vec![
        Value::U64(1),
        Value::Struct {
            type_name: "Foo".to_string(),
            fields: vec![],
        },
    ]);
    assert!(val.uses_move_semantics());
}

#[test]
fn value_does_not_use_move_semantics_for_primitives() {
    assert!(!Value::Bool(true).uses_move_semantics());
    assert!(!Value::U64(0).uses_move_semantics());
    assert!(!Value::Address(Address::ZERO).uses_move_semantics());
    assert!(!Value::Str("x".to_string()).uses_move_semantics());
    assert!(!Value::Void.uses_move_semantics());
}

#[test]
fn value_does_not_use_move_semantics_for_primitive_tuple() {
    let val = Value::Tuple(vec![Value::U64(1), Value::Bool(true)]);
    assert!(!val.uses_move_semantics());
}

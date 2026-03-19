use meow_vm_types::types::{StructDef, Type, Value};

//
// ─── Value::type_name — primitives ───
//

#[test]
fn value_type_name_primitives() {
    assert_eq!(Value::Bool(false).type_name(), "bool");
    assert_eq!(Value::U64(0).type_name(), "u64");
    assert_eq!(Value::Address([0u8; 32]).type_name(), "address");
    assert_eq!(Value::Str("hello".to_string()).type_name(), "str");
    assert_eq!(Value::Void.type_name(), "void");
}

//
// ─── Value::type_name — struct ───
//

#[test]
fn value_type_name_struct() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![],
    };
    assert_eq!(val.type_name(), "Foo");
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
// ─── Value::as_address ───
//

#[test]
fn value_as_address() {
    let addr = [0xABu8; 32];
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
// ─── StructDef::field_index ───
//

#[test]
fn struct_def_field_index() {
    let def = StructDef {
        name: "Foo".to_string(),
        fields: vec![
            ("a".to_string(), Type::U64),
            ("b".to_string(), Type::Bool),
            ("c".to_string(), Type::Address),
        ],
        is_object: false,
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
        is_object: false,
    };
    assert_eq!(def.name, "Bar");
}

#[test]
fn struct_def_fields() {
    let def = StructDef {
        name: "Baz".to_string(),
        fields: vec![("x".to_string(), Type::U64), ("y".to_string(), Type::U64)],
        is_object: false,
    };
    assert_eq!(def.fields.len(), 2);
    assert_eq!(def.fields[0].0, "x");
    assert_eq!(def.fields[0].1, Type::U64);
    assert_eq!(def.fields[1].0, "y");
    assert_eq!(def.fields[1].1, Type::U64);
}

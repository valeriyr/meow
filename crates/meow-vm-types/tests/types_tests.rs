use meow_vm_types::{
    address::Address,
    types::{FieldDef, StructDef, Type, Value},
};

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
    let addr = Address::fill(0xAB);
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
        is_object: false,
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
        is_object: false,
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
        is_object: false,
        is_public: true,
    };
    assert_eq!(def.fields.len(), 2);
    assert_eq!(def.fields[0].name, "x");
    assert_eq!(def.fields[0].ty, Type::U64);
    assert_eq!(def.fields[1].name, "y");
    assert_eq!(def.fields[1].ty, Type::U64);
}

use meow_vm_types::{
    module::{Function, Module},
    types::{FieldDef, StructDef, Type},
};

//
// ─── Module::get_struct ───
//

#[test]
fn get_struct_returns_some_for_existing_struct() {
    let mut module = Module::new("test");
    module.structs.push(StructDef {
        name: "Point".to_string(),
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
    });

    assert_eq!(module.get_struct("Point").unwrap().name, "Point");
}

#[test]
fn get_struct_returns_none_for_missing_struct() {
    let module = Module::new("test");
    assert!(module.get_struct("Missing").is_none());
}

#[test]
fn get_struct_lookup_is_case_sensitive() {
    let mut module = Module::new("test");
    module.structs.push(StructDef {
        name: "Point".to_string(),
        fields: vec![],
        is_public: false,
    });
    assert!(module.get_struct("point").is_none());
    assert!(module.get_struct("POINT").is_none());
}

//
// ─── Module::get_function ───
//

#[test]
fn get_function_returns_some_for_existing_function() {
    let mut module = Module::new("test");
    module.functions.push(Function {
        name: "add".to_string(),
        is_public: true,
        params: vec![("a".to_string(), Type::U64), ("b".to_string(), Type::U64)],
        return_type: Some(Type::U64),
        local_count: 2,
        code: vec![],
    });

    assert_eq!(module.get_function("add").unwrap().name, "add");
}

#[test]
fn get_function_returns_none_for_missing_function() {
    let module = Module::new("test");
    assert!(module.get_function("missing").is_none());
}

#[test]
fn get_function_lookup_is_case_sensitive() {
    let mut module = Module::new("test");
    module.functions.push(Function {
        name: "add".to_string(),
        is_public: true,
        params: vec![],
        return_type: None,
        local_count: 0,
        code: vec![],
    });
    assert!(module.get_function("Add").is_none());
    assert!(module.get_function("ADD").is_none());
}

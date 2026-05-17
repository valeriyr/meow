use meow_vm_types::natives::{builtin_natives, meow_vm_abort_sig};

#[test]
fn builtin_natives_is_exactly_meow_vm_abort() {
    let builtins = builtin_natives();

    assert_eq!(builtins.len(), 1);

    let sig = meow_vm_abort_sig();

    assert_eq!(builtins[0].name, sig.name);
    assert_eq!(builtins[0].params, sig.params);
    assert_eq!(builtins[0].return_type, sig.return_type);
}

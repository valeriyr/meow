use meow_types::object::object_version::ObjectVersion;

//
// ObjectVersion comparison tests.
//

#[test]
fn equality() {
    let v0 = ObjectVersion::ZERO;

    assert_eq!(v0, v0.clone());
    assert_eq!(v0.next(), v0.next());
}

#[test]
fn inequality() {
    let v0 = ObjectVersion::ZERO;
    let v1 = v0.next();

    assert_ne!(v0, v1);
}

#[test]
fn next_from_zero() {
    let v0 = ObjectVersion::ZERO;
    let v1 = v0.next();

    assert_ne!(v1, ObjectVersion::ZERO);
    assert_eq!(v1, v0.next());
}

#[test]
fn next_increments_sequentially() {
    let v0 = ObjectVersion::ZERO;
    let v1 = v0.next();
    let v2 = v1.next();
    let v3 = v2.next();

    assert!(v0 < v1);
    assert!(v1 < v2);
    assert!(v2 < v3);

    assert!(v1 > v0);
    assert!(v2 > v1);
    assert!(v3 > v2);
}

//
// ObjectVersion conversion tests.
//

#[test]
fn zero_object_version_to_string() {
    assert_eq!(ObjectVersion::ZERO.to_string(), "0");
}

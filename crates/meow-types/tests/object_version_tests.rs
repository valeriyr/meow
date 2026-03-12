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
    let v1 = v0.next().unwrap();

    assert_ne!(v0, v1);
}

#[test]
fn next_increments_sequentially() {
    let v0 = ObjectVersion::ZERO;
    let v1 = v0.next().unwrap();
    let v2 = v1.next().unwrap();
    let v3 = v2.next().unwrap();

    assert!(v0 < v1);
    assert!(v1 < v2);
    assert!(v2 < v3);

    assert!(v1 > v0);
    assert!(v2 > v1);
    assert!(v3 > v2);
}

//
// ObjectVersion constraint tests.
//

#[test]
fn next_at_max_returns_none() {
    assert!(ObjectVersion::MAX.next().is_none());
}

//
// ObjectVersion conversion tests.
//

#[test]
fn zero_object_version_to_string() {
    assert_eq!(ObjectVersion::ZERO.to_string(), "0");
}

#[test]
fn max_object_version_to_string() {
    assert_eq!(ObjectVersion::MAX.to_string(), "18446744073709551615");
}

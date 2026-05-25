use meow_types::time;

//
// ─── current_timestamp tests ───
//

#[test]
fn current_timestamp_returns_nonzero() {
    assert!(time::current_timestamp() > 0);
}

#[test]
fn current_timestamp_is_after_2024() {
    // 2024-01-01 00:00:00 UTC in milliseconds
    const JAN_2024_MS: u64 = 1_704_067_200_000;
    assert!(
        time::current_timestamp() > JAN_2024_MS,
        "timestamp must be after 2024-01-01"
    );
}

#[test]
fn current_timestamp_is_monotonically_nondecreasing() {
    let t1 = time::current_timestamp();
    let t2 = time::current_timestamp();
    assert!(t2 >= t1, "second call must not return an earlier timestamp");
}

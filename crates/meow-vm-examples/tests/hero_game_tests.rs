use std::collections::HashMap;

use meow_types::{
    address::Address,
    identifier::Identifier,
    system_framework::meow_object::{
        MEOW_OBJECT_MODULE_ADDRESS, MEOW_OBJECT_MODULE_PATH, MeowObjectId,
    },
};
use meow_vm_adapter::{
    builder,
    external_context::ExternalContext,
    runner::{self, RunResult, VmError},
};
use meow_vm_types::{module::Module, types::Value};

const HERO_SRC: &str = include_str!("../modules/hero_game.meow");

//
// ─── Tests ───
//

#[test]
fn compile_hero_game() {
    let _ = hero_module();
}

#[test]
fn spawn_creates_hero_and_transfers_to_sender() {
    let result = run("spawn", vec![Value::Str("Thorin".to_string())]).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, Address::ZERO); // sender is zero in default context
    assert!(result.destroyed.is_empty());

    let hero = &result.transfers[0].0;
    assert_eq!(field_str(hero, "name"), "Thorin");
    assert_eq!(field_u64(hero, "level"), 1);
    assert_eq!(field_u64(hero, "experience"), 0);
    assert_eq!(field_u64(hero, "wins"), 0);
}

#[test]
fn rename_updates_display_name() {
    let hero = make_hero(Address::fill(0x01), "Thorin", 1, 0, 0);
    let result = run(
        "rename",
        vec![hero, Value::Str("Thorin Oakenshield".to_string())],
    )
    .unwrap();

    assert!(result.transfers.is_empty());
    assert!(result.destroyed.is_empty());

    let final_hero = result.final_args[0]
        .as_ref()
        .expect("hero must survive in slot");
    assert_eq!(field_str(final_hero, "name"), "Thorin Oakenshield");
}

#[test]
fn duel_one_hero_wins_and_gains_xp() {
    let attacker = make_hero(Address::fill(0x01), "Attacker", 1, 0, 0);
    let defender = make_hero(Address::fill(0x02), "Defender", 1, 0, 0);
    let result = run("duel", vec![attacker, defender]).unwrap();

    assert!(result.transfers.is_empty());
    assert!(result.destroyed.is_empty());

    let a = result.final_args[0]
        .as_ref()
        .expect("attacker must survive");
    let d = result.final_args[1]
        .as_ref()
        .expect("defender must survive");

    let a_wins = field_u64(a, "wins");
    let d_wins = field_u64(d, "wins");
    assert_eq!(a_wins + d_wins, 1, "exactly one hero must win");

    // Winner gains 25 XP; loser gains nothing (loser was level 1, no level-up at 25 < 100)
    if a_wins == 1 {
        assert_eq!(field_u64(a, "experience"), 25);
        assert_eq!(field_u64(d, "experience"), 0, "loser must not gain XP");
    } else {
        assert_eq!(field_u64(d, "experience"), 25);
        assert_eq!(field_u64(a, "experience"), 0, "loser must not gain XP");
    }
}

#[test]
fn duel_winner_levels_up_when_xp_threshold_reached() {
    // Attacker at 75 XP: winning gains 25 (loser level 1) → 100 = 1×100 → level up, XP resets.
    let attacker = make_hero(Address::fill(0x01), "Veteran", 1, 75, 0);
    let defender = make_hero(Address::fill(0x02), "Rookie", 1, 0, 0);
    let result = run("duel", vec![attacker, defender]).unwrap();

    let a = result.final_args[0].as_ref().unwrap();
    let d = result.final_args[1].as_ref().unwrap();

    if field_u64(a, "wins") == 1 {
        assert_eq!(field_u64(a, "level"), 2, "attacker must level up");
        assert_eq!(
            field_u64(a, "experience"),
            0,
            "XP must reset after level-up"
        );
    } else {
        assert_eq!(field_u64(d, "level"), 1, "defender must not level up");
        assert_eq!(field_u64(d, "experience"), 25);
    }
}

#[test]
fn duel_same_hero_id_aborts() {
    let hero_id = Address::fill(0x01);
    let a = make_hero(hero_id, "One", 1, 0, 0);
    let b = make_hero(hero_id, "Clone", 1, 0, 0); // same id → abort
    let err = run("duel", vec![a, b]).unwrap_err();
    assert!(
        matches!(&err, VmError::Aborted { code: 1, .. }),
        "expected abort code 1, got {err:?}"
    );
}

#[test]
fn transfer_sends_hero_to_recipient() {
    let recipient = Address::fill(0x42);
    let hero = make_hero(Address::fill(0x01), "Thorin", 1, 0, 0);
    let result = run("transfer", vec![hero, Value::Address(recipient.into())]).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, recipient);
    assert!(result.destroyed.is_empty());
}

#[test]
fn retire_destroys_hero() {
    let hero_id = Address::fill(0x01);
    let hero = make_hero(hero_id, "Thorin", 5, 50, 3);
    let result = run("retire", vec![hero]).unwrap();

    assert!(result.transfers.is_empty());
    assert_eq!(result.destroyed, vec![hero_id]);
}

//
// ─── Utilities ───
//

fn hero_module() -> Module {
    let obj =
        builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[]).expect("meow_object must compile");
    builder::build(HERO_SRC, &[(MEOW_OBJECT_MODULE_ADDRESS, &obj)])
        .expect("hero_game.meow must compile")
}

fn make_hero(id: Address, name: &str, level: u64, experience: u64, wins: u64) -> Value {
    Value::Struct {
        type_name: "Hero".to_string(),
        fields: vec![
            (
                "id".to_string(),
                MeowObjectId::from(id).to_qualified_vm_value(),
            ),
            ("name".to_string(), Value::Str(name.to_string())),
            ("level".to_string(), Value::U64(level)),
            ("experience".to_string(), Value::U64(experience)),
            ("wins".to_string(), Value::U64(wins)),
        ],
    }
}

fn field<'a>(v: &'a Value, name: &str) -> &'a Value {
    match v {
        Value::Struct { fields, .. } => fields
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("field '{name}' not found")),
        _ => panic!("expected struct"),
    }
}

fn field_u64(v: &Value, name: &str) -> u64 {
    field(v, name)
        .as_u64()
        .unwrap_or_else(|| panic!("field '{name}' must be u64"))
}

fn field_str<'a>(v: &'a Value, name: &str) -> &'a str {
    match field(v, name) {
        Value::Str(s) => s,
        other => panic!("field '{name}' must be string, got {other:?}"),
    }
}

fn run(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let id = Identifier::new(fn_name).expect("must be valid identifier");
    runner::run(
        hero_module(),
        &id,
        args,
        HashMap::new(),
        ExternalContext::default(),
    )
}

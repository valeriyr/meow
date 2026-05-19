use std::collections::HashMap;

use meow_framework::meow_object_module;
use meow_types::{
    address::Address,
    identifier::Identifier,
    system_framework::meow_object::{
        MEOW_OBJECT_ID_FIELD_NAME, MEOW_OBJECT_MODULE_ADDRESS, MeowObjectId,
    },
};
use meow_vm_adapter::{
    builder,
    external_context::ExternalContext,
    runner::{self, RunResult},
};
use meow_vm_types::{module::Module, module_ref, types::Value};

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
    assert_eq!(hero.field_str("name").unwrap(), "Thorin");
    assert_eq!(hero.field_u64("level").unwrap(), 1);
    assert_eq!(hero.field_u64("experience").unwrap(), 0);
    assert_eq!(hero.field_u64("wins").unwrap(), 0);
}

#[test]
fn rename_updates_display_name() {
    let hero = make_hero(Address::fill(0x01), "Thorin", 1, 0, 0);
    let result = run(
        "rename",
        vec![hero, Value::Str("Thorin Oakenshield".to_string())],
    )
    .unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert!(result.destroyed.is_empty());

    let renamed_hero = &result.transfers[0].0;
    assert_eq!(
        renamed_hero.field_str("name").unwrap(),
        "Thorin Oakenshield"
    );
}

#[test]
fn duel_one_hero_wins_and_gains_xp() {
    let attacker = make_hero(Address::fill(0x01), "Attacker", 1, 0, 0);
    let defender = make_hero(Address::fill(0x02), "Defender", 1, 0, 0);
    let result = run("duel", vec![attacker, defender]).unwrap();

    assert_eq!(result.transfers.len(), 2);
    assert!(result.destroyed.is_empty());

    let a = &result.transfers[0].0;
    let d = &result.transfers[1].0;

    let a_wins = a.field_u64("wins").unwrap();
    let d_wins = d.field_u64("wins").unwrap();
    assert_eq!(a_wins + d_wins, 1, "exactly one hero must win");

    if a_wins == 1 {
        assert_eq!(a.field_u64("experience").unwrap(), 25);
        assert_eq!(
            d.field_u64("experience").unwrap(),
            0,
            "loser must not gain XP"
        );
    } else {
        assert_eq!(d.field_u64("experience").unwrap(), 25);
        assert_eq!(
            a.field_u64("experience").unwrap(),
            0,
            "loser must not gain XP"
        );
    }
}

#[test]
fn duel_winner_levels_up_when_xp_threshold_reached() {
    // Attacker at 75 XP: winning gains 25 (loser level 1) → 100 = 1×100 → level up, XP resets.
    let attacker = make_hero(Address::fill(0x01), "Veteran", 1, 75, 0);
    let defender = make_hero(Address::fill(0x02), "Rookie", 1, 0, 0);
    let result = run("duel", vec![attacker, defender]).unwrap();

    assert_eq!(result.transfers.len(), 2);

    let a = &result.transfers[0].0;
    let d = &result.transfers[1].0;

    if a.field_u64("wins").unwrap() == 1 {
        assert_eq!(a.field_u64("level").unwrap(), 2, "attacker must level up");
        assert_eq!(
            a.field_u64("experience").unwrap(),
            0,
            "XP must reset after level-up"
        );
    } else {
        assert_eq!(
            d.field_u64("level").unwrap(),
            1,
            "defender must not level up"
        );
        assert_eq!(d.field_u64("experience").unwrap(), 25);
    }
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

const HERO_GAME_SRC: &str = include_str!("../modules/hero_game.meow");

fn hero_module_name_qualified() -> String {
    module_ref::qualify(&Address::ZERO.into(), "Hero")
}

fn hero_module() -> Module {
    builder::build(
        HERO_GAME_SRC,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module())],
    )
    .expect("hero_game.meow must compile")
}

fn make_hero(id: Address, name: &str, level: u64, experience: u64, wins: u64) -> Value {
    Value::Struct {
        type_name: hero_module_name_qualified(),
        fields: vec![
            (
                MEOW_OBJECT_ID_FIELD_NAME.to_string(),
                MeowObjectId::from(id).into(),
            ),
            ("name".to_string(), Value::Str(name.to_string())),
            ("level".to_string(), Value::U64(level)),
            ("experience".to_string(), Value::U64(experience)),
            ("wins".to_string(), Value::U64(wins)),
        ],
    }
}

fn run(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let id = Identifier::new(fn_name).expect("must be valid identifier");
    runner::run(
        (Address::ZERO, hero_module()),
        &id,
        args,
        HashMap::new(),
        ExternalContext::default(),
    )
}

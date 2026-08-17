//! Ordex (OrdexArena) Practical Use Case: ECS Combat System
//!
//! This scenario demonstrates how to use Ordex as a backend for an Entity
//! Component System (ECS), showcasing batch processing (`iter_mut`),
//! safe concurrent mutation (`align!`), and ABA problem prevention.

use ordex::prelude::*;

#[derive(Debug)]
struct Character {
    name: &'static str,
    hp: i32,
    attack: i32,
    is_poisoned: bool,
}

#[test]
fn scenario_ecs_combat() {
    let mut world = OrdexArena::new();

    // 1. Spawn entities
    let hero = world.insert(Character {
        name: "Hero",
        hp: 100,
        attack: 25,
        is_poisoned: false,
    });
    let goblin = world.insert(Character {
        name: "Goblin",
        hp: 40,
        attack: 10,
        is_poisoned: false,
    });
    let slime = world.insert(Character {
        name: "Slime",
        hp: 20,
        attack: 5,
        is_poisoned: true,
    });

    // 2. Turn Start: Batch processing poison damage (Sequential access via iter_mut)
    for (_idx, chara) in world.iter_mut() {
        if chara.is_poisoned {
            chara.hp -= 2;
        }
    }
    assert_eq!(world.get(slime).unwrap().hp, 18); // Only the Slime takes poison damage

    // 3. Player Attack: Targeted interaction between two specific entities (Zero-cost concurrent access)
    // We bypass the borrow checker safely to draw out two mutable references simultaneously.
    align!(world, hero, goblin, |h, g| {
        g.hp -= h.attack;
        h.hp -= g.attack; // Counterattack
    });

    assert_eq!(world.get(goblin).unwrap().hp, 15); // 40 - 25
    assert_eq!(world.get(hero).unwrap().hp, 90); // 100 - 10

    // 4. Goblin Defeat & Respawn (ABA problem prevention & memory reuse)
    let dead_goblin = world.remove(goblin).unwrap();
    assert_eq!(dead_goblin.name, "Goblin");

    // A new enemy spawns (The memory slot previously held by the Goblin is reused).
    let dragon = world.insert(Character {
        name: "Dragon",
        hp: 999,
        attack: 100,
        is_poisoned: false,
    });

    assert_eq!(dragon.index, goblin.index);
    assert!(dragon.generation > goblin.generation);
}

//! The page's side of the game, without a browser: JSON commands in, JSON
//! views out, as `ygo_handle` runs them in WebAssembly.

use serde_json::{Value, json};
use ygo_engine::testing::{root, shared_from};
use ygo_engine::{CardIndex, GameRecord};
use ygo_play::testing::assert_no_hidden_names;
use ygo_wasm::Host;

const DECKS: [&str; 3] = [
    "save_a_owner_current_40.json",
    "save_b_dark_plasma_40.json",
    "save_a_best_overall_40.json",
];

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap()
}

fn init_command(extra_decks: &[(&str, String)]) -> Value {
    let mut decks: Vec<Value> = DECKS
        .iter()
        .map(|d| json!({ "file": d, "text": read(&format!("data/decks/{d}")) }))
        .collect();
    decks.extend(
        extra_decks
            .iter()
            .map(|(f, t)| json!({ "file": f, "text": t })),
    );
    json!({
        "cmd": "init",
        "rules": read("config/rules.toml"),
        "cards": read("data/cards.json"),
        "banlist": read("data/banlist.json"),
        "tokens": read("data/tokens.json"),
        "decks": decks,
    })
}

fn call(host: &mut Host, command: Value) -> Value {
    serde_json::from_str(&host.handle(&command.to_string())).unwrap()
}

fn ok(host: &mut Host, command: Value) -> Value {
    let answer = call(host, command);
    answer
        .get("ok")
        .cloned()
        .unwrap_or_else(|| panic!("error: {answer}"))
}

/// A host with the shipped network, initialised.
fn ready_host() -> Host {
    let mut host = Host::default();
    let net = std::fs::read(root().join("data/networks/best.bpk")).unwrap();
    host.set_network(net).unwrap();
    ok(&mut host, init_command(&[]));
    host
}

fn start(host: &mut Host, seed: u64, bot: &str, choices: &[usize]) -> Value {
    ok(
        host,
        json!({
            "cmd": "start", "deck": DECKS[0], "bot_deck": DECKS[1], "bot": bot,
            "seed": seed, "first": "random", "choices": choices,
        }),
    )
}

fn lcg(state: &mut u64) -> usize {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 33) as usize
}

#[test]
fn init_offers_the_legal_decks_and_every_bot() {
    let mut host = Host::default();
    let net = std::fs::read(root().join("data/networks/best.bpk")).unwrap();
    host.set_network(net).unwrap();
    let mut two_raigeki = read("data/decks/save_a_owner_current_40.json");
    two_raigeki = two_raigeki.replace(
        "\"name\": \"Raigeki\",\n   \"count\": 1",
        "\"name\": \"Raigeki\",\n   \"count\": 2",
    );
    let answer = ok(
        &mut host,
        init_command(&[("two_raigeki.json", two_raigeki)]),
    );
    let decks = answer["decks"].as_array().unwrap();
    assert_eq!(decks.len(), 4);
    for d in &decks[..3] {
        assert_eq!(d["legal"], true, "{d}");
    }
    assert_eq!(decks[3]["legal"], false);
    let problem = decks[3]["problem"].as_str().unwrap();
    assert!(problem.contains("2 copies of Raigeki"), "{problem}");
    assert_eq!(answer["default_bot"], "network");
    assert_eq!(
        answer["bots"],
        json!(["network", "network-search", "random"])
    );
    // Without the network only the random bot is offered.
    let mut bare = Host::default();
    let answer = ok(&mut bare, init_command(&[]));
    assert_eq!(answer["default_bot"], "random");
    assert_eq!(answer["bots"], json!(["random"]));
    assert!(bare.set_network(b"not a network".to_vec()).is_err());

    let refused = call(
        &mut host,
        json!({ "cmd": "start", "deck": "two_raigeki.json", "bot_deck": DECKS[0],
                "bot": "random", "seed": 1, "first": "me" }),
    );
    assert!(
        refused["error"].as_str().unwrap().contains("no legal deck"),
        "{refused}"
    );
}

/// The deck builder's data: every deck card with its copy limit (from the
/// ban list and the rules) and whether it is an Extra Deck card; the deck
/// rules; and a live check of a deck that is not offered for play.
#[test]
fn the_deck_builder_gets_the_pool_and_checks_decks() {
    let mut host = Host::default();
    let answer = ok(&mut host, init_command(&[]));
    let pool = answer["pool"].as_array().unwrap();
    assert_eq!(pool.len(), 535, "every deck card, no Tokens");
    let card = |id: u64| pool.iter().find(|c| c["id"] == id).unwrap();
    assert_eq!(card(55144522)["limit"], 1, "Pot of Greed is limited");
    assert_eq!(card(89631139)["limit"], 3, "Blue-Eyes White Dragon");
    assert_eq!(card(89631139)["extra"], false);
    assert_eq!(
        card(66889139)["extra"],
        true,
        "Gaia the Dragon Champion is a Fusion"
    );
    assert_eq!(
        answer["deck_rules"],
        json!({ "main_min": 40, "main_max": 60, "extra_max": 15, "default_copy_limit": 3 })
    );

    let legal = read("data/decks/save_a_owner_current_40.json");
    let checked = ok(&mut host, json!({ "cmd": "check_deck", "text": legal }));
    assert_eq!(checked["problems"], json!([]));
    let small = json!({
        "schema_version": 1, "name": "Two Pots", "main": [
            { "id": 55144522, "name": "Pot of Greed", "count": 2 }
        ]
    });
    let checked = ok(
        &mut host,
        json!({ "cmd": "check_deck", "text": small.to_string() }),
    );
    assert_eq!(checked["name"], "Two Pots");
    let problems: Vec<&str> = checked["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    assert!(
        problems.iter().any(|p| p.contains("Main Deck has 2 cards")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("2 copies of Pot of Greed")),
        "{problems:?}"
    );
    let bad = call(&mut host, json!({ "cmd": "check_deck", "text": "{}" }));
    assert!(bad["error"].is_string(), "{bad}");
}

#[test]
fn whole_games_through_json_commands_replay_from_their_record() {
    let shared = shared_from(ygo_cards::registry());
    for (seed, bot) in [(1u64, "network"), (2, "random"), (3, "network")] {
        let mut host = ready_host();
        let mut view = start(&mut host, seed, bot, &[])["view"].clone();
        let mut rng = seed;
        while view["result"].is_null() {
            let n = view["actions"].as_array().unwrap().len();
            assert!(n > 0, "the human has a choice until the game ends");
            assert!(view["prompt"].is_string());
            for a in view["actions"].as_array().unwrap() {
                if a["kind"] == "attack" {
                    assert!(a["card"].is_u64(), "an attack names its attacker: {a}");
                    let direct = a["text"].as_str().unwrap().starts_with("Attack directly");
                    assert_eq!(
                        a["targets"].as_array().unwrap().len(),
                        usize::from(!direct),
                        "{a}"
                    );
                }
            }
            view = ok(
                &mut host,
                json!({ "cmd": "play", "index": lcg(&mut rng) % n }),
            )["view"]
                .clone();
        }
        assert!(view["actions"].as_array().unwrap().is_empty());
        let refused = call(&mut host, json!({ "cmd": "play", "index": 0 }));
        assert!(refused["error"].is_string());
        let text = ok(&mut host, json!({ "cmd": "record" }));
        let record = GameRecord::from_jsonl(text.as_str().unwrap()).unwrap();
        assert_eq!(
            record.header.players,
            ["human".to_string(), bot.to_string()]
        );
        assert_eq!(
            &record.replay(&shared).unwrap(),
            host.session().unwrap().game()
        );
    }
}

#[test]
fn the_json_never_names_a_hidden_card() {
    let shared = shared_from(ygo_cards::registry());
    for seed in 10..14u64 {
        let mut host = ready_host();
        let mut answer = start(&mut host, seed, "network", &[]);
        let mut rng = seed;
        while answer["view"]["result"].is_null() && answer["view"]["turn"].as_u64().unwrap() < 30 {
            let mut text = answer["view"].to_string();
            // Log lines were checked when written (describe tests); a card
            // named there may have gone back to a hidden zone since.
            let logged = answer["view"]["log"].to_string();
            let mut names: Vec<&str> = (0..shared.pool.len() as u16)
                .map(|i| shared.pool.get(CardIndex(i)).name.as_str())
                .filter(|n| logged.contains(n))
                .collect();
            names.sort_by_key(|n| std::cmp::Reverse(n.len()));
            for n in names {
                text = text.replace(n, "#");
            }
            // Each animation step shows only what its moment's observation
            // shows (its log lines are left out).
            let steps = answer["steps"].as_array().unwrap();
            assert_eq!(steps.len(), host.step_observations().len());
            for (step, obs) in steps.iter().zip(host.step_observations()) {
                assert_eq!(step["view"]["log"], json!([]));
                assert_no_hidden_names(&step.to_string(), obs, &shared);
                ids_match_the_observation(&step["view"], obs);
                assert!(!step["cues"].as_array().unwrap().is_empty());
            }
            let obs = host.session().unwrap().observation();
            assert_no_hidden_names(&text, &obs, &shared);
            // A card's id (for its image) only when the human sees it: it
            // must match what the observation shows for that instance.
            ids_match_the_observation(&answer["view"], &obs);
            // A hidden hand card has no text; a face-up summon is public
            // from its declaration, while the card is still in the hand.
            for c in obs
                .opponent()
                .hand
                .iter()
                .filter(|c| matches!(c, ygo_engine::VisibleCard::Hidden { .. }))
            {
                let card = ok(
                    &mut host,
                    json!({ "cmd": "card", "instance": c.instance().0 }),
                );
                assert!(card.is_null(), "the opponent's hand has no readable text");
            }
            let n = answer["view"]["actions"].as_array().unwrap().len();
            answer = ok(
                &mut host,
                json!({ "cmd": "play", "index": lcg(&mut rng) % n }),
            );
        }
    }
}

/// Every move the human can see comes back as a step with cues naming
/// what to animate; over whole games every common kind turns up, and each
/// cue's card is one the step's view shows (or one that just left it).
#[test]
fn steps_carry_cues_for_each_visible_move() {
    let mut host = ready_host();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 20..24u64 {
        let mut answer = start(&mut host, seed, "network", &[]);
        let mut rng = seed;
        loop {
            for step in answer["steps"].as_array().unwrap() {
                for cue in step["cues"].as_array().unwrap() {
                    let kind = cue["kind"].as_str().unwrap().to_string();
                    assert!(
                        cue.get("name").is_none() && cue.get("id").is_none(),
                        "{cue}"
                    );
                    if kind == "summon" || kind == "set" || kind == "attack" {
                        let inst = cue["card"].as_u64().unwrap();
                        let shown = step["view"].to_string();
                        assert!(
                            shown.contains(&format!("\"instance\":{inst},")),
                            "{kind}: instance {inst} is in the step's view"
                        );
                    }
                    seen.insert(kind);
                }
            }
            if !answer["view"]["result"].is_null() {
                assert!(seen.contains("game_over"));
                break;
            }
            let n = answer["view"]["actions"].as_array().unwrap().len();
            answer = ok(
                &mut host,
                json!({ "cmd": "play", "index": lcg(&mut rng) % n }),
            );
        }
    }
    for kind in [
        "turn",
        "battle",
        "draw",
        "summon",
        "set",
        "activate",
        "attack",
        "damage",
        "destroy",
        "graveyard",
        "game_over",
    ] {
        assert!(seen.contains(kind), "no {kind} cue in four games: {seen:?}");
    }
    // A resumed game is shown as it stands: no steps.
    let resumed = start(&mut host, 20, "network", &[0]);
    assert_eq!(resumed["steps"], json!([]));
}

/// Every choice an effect asks of the human names that effect's card (on
/// the table for the human to see), and a card pick says where the cards
/// are; other decisions carry no choice.
#[test]
fn a_choice_names_the_effect_that_asks() {
    let mut host = ready_host();
    let mut from = std::collections::BTreeSet::new();
    let (mut picks, mut questions) = (0, 0);
    for seed in 30..40u64 {
        let mut answer = start(&mut host, seed, "network", &[]);
        let mut rng = seed;
        while answer["view"]["result"].is_null() && answer["view"]["turn"].as_u64().unwrap() < 40 {
            let v = &answer["view"];
            let prompt = v["prompt"].as_str().unwrap_or("");
            let choice = &v["choice"];
            if prompt == "Choose a card" || prompt == "Choose an option" {
                let name = choice["name"]
                    .as_str()
                    .unwrap_or_else(|| panic!("no choice: {v}"));
                assert!(!name.is_empty());
                assert!(
                    choice["text"].as_str().unwrap().starts_with(name),
                    "{choice}"
                );
                if prompt == "Choose a card" {
                    picks += 1;
                    if let Some(f) = choice["from"].as_str() {
                        from.insert(f.to_string());
                    }
                } else {
                    questions += 1;
                    assert!(choice["from"].is_null());
                }
            } else {
                assert!(choice.is_null(), "{prompt}: {choice}");
            }
            let n = v["actions"].as_array().unwrap().len();
            answer = ok(
                &mut host,
                json!({ "cmd": "play", "index": lcg(&mut rng) % n }),
            );
        }
    }
    assert!(
        picks > 0 && questions > 0,
        "picks {picks}, questions {questions}"
    );
    assert!(!from.is_empty(), "card picks say where the cards are");
    println!("picks {picks}, questions {questions}, from {from:?}");
}

#[test]
fn a_game_resumes_from_its_choices() {
    // The search bot thinks again on a resume and must choose the same.
    for (bot, moves) in [("network", 40), ("network-search", 5)] {
        let mut host = ready_host();
        let mut answer = start(&mut host, 21, bot, &[]);
        let mut rng = 5u64;
        for _ in 0..moves {
            if !answer["view"]["result"].is_null() {
                break;
            }
            let n = answer["view"]["actions"].as_array().unwrap().len();
            answer = ok(
                &mut host,
                json!({ "cmd": "play", "index": lcg(&mut rng) % n }),
            );
        }
        let choices: Vec<usize> = serde_json::from_value(answer["choices"].clone()).unwrap();
        assert!(!choices.is_empty());
        let mut fresh = ready_host();
        let resumed = start(&mut fresh, 21, bot, &choices);
        // A resume has no animation steps; the rest is the same.
        assert_eq!(
            (&resumed["view"], &resumed["choices"]),
            (&answer["view"], &answer["choices"]),
            "{bot}: same view and choices after a resume"
        );
        assert_eq!(
            fresh.session().unwrap().game(),
            host.session().unwrap().game()
        );
    }
}

#[test]
fn bad_commands_are_answered_with_errors() {
    let mut host = Host::default();
    for (command, expect) in [
        (json!({ "cmd": "view" }), "no game in progress"),
        (json!({ "cmd": "play", "index": 0 }), "no game in progress"),
        (
            json!({ "cmd": "start", "deck": "a", "bot_deck": "b", "bot": "random", "seed": 1, "first": "me" }),
            "data not loaded",
        ),
        (json!({ "cmd": "dance" }), "bad command"),
    ] {
        let answer = call(&mut host, command);
        assert!(
            answer["error"].as_str().unwrap().contains(expect),
            "{answer}"
        );
    }
    let mut host = ready_host();
    let answer = call(
        &mut host,
        json!({ "cmd": "start", "deck": DECKS[0], "bot_deck": DECKS[0], "bot": "lookahead:beatdown", "seed": 1, "first": "me" }),
    );
    assert!(
        answer["error"].as_str().unwrap().contains("unknown bot"),
        "{answer}"
    );
}

/// Every card object in the view (anything with an "instance") carries an
/// id exactly when the observation shows that card, and the right one.
fn ids_match_the_observation(v: &Value, obs: &ygo_engine::Observation) {
    match v {
        Value::Object(m) => {
            if let Some(inst) = m.get("instance").and_then(Value::as_u64) {
                let i = ygo_engine::InstanceId(inst as u8);
                // A question's card is shown to the one answering (a
                // Cyber Jar pick still in the Deck).
                let seen = obs.find(i).and_then(|f| f.card.card()).or_else(|| {
                    obs.asked_about
                        .as_ref()
                        .filter(|c| c.instance() == i)
                        .and_then(|c| c.card())
                });
                let id = m.get("id").and_then(Value::as_u64);
                assert_eq!(id, seen.map(|c| u64::from(c.0)), "instance {inst}: {v}");
            }
            m.values().for_each(|x| ids_match_the_observation(x, obs));
        }
        Value::Array(a) => a.iter().for_each(|x| ids_match_the_observation(x, obs)),
        _ => {}
    }
}

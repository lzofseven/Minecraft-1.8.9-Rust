// Windows: sem janela de console no executável final (veja `winapp`).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod achievement_toast;
mod atmosphere;
mod auth;
mod block_physics;
mod border;
mod camera;
mod banner;
mod chat;
mod chat_gui;
mod desktop;
mod gen_chat_tables;
mod snbt;
mod culling;
mod debug_overlay;
mod demo;
mod entities;
mod entity_net;
mod far;
mod first_person;
mod font_text;
mod fx;
mod game;
mod gen_achievements;
mod gen_banner;
mod gen_debug;
mod gen_mapcolors;
mod gen_hud;
mod gen_item_flags;
mod gui;
mod hud;
mod hud_stats;
mod hud_track;
mod interact;
mod container_net;
mod containers;
mod enchant;
mod gen_container_items;
mod gen_containers;
mod gui_container;
mod gen_creative;
mod gui_creative;
mod gui_special;
mod inventory;
mod item_use;
mod lang;
mod msa;
mod setup;
mod visuals;
mod winapp;
mod maps;
mod menu;
// Modelos gerados de todos os mobs; parte ainda sem pose/desenho (HANDOFF-mobs.md).
#[allow(dead_code)]
mod gen_mob_models;
mod gen_paintings;
mod gen_pick_block;
mod gen_block_names;
mod gen_recipes;
mod gen_special_recipes;
mod recipes;
mod gen_shadows;
mod mob_rig;
mod mobs;
mod models;
mod mth;
mod naovivas;
mod net;
mod particles;
mod pack_screen;
mod packs;
mod physics;
mod potion;
mod rain;
mod res;
mod server_pack;
mod settings;
mod skins;
mod sound;
mod tab_list;
mod test_world;
mod tile_entities;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use mc_protocol::serverbound;

use net::{BoxErr, NetEvent};

const USAGE: &str = "uso:
  mc-client [play] [servidor[:porta]] [--session auth.json | --offline [--name NOME]] [--jar minecraft-1.8.9.jar]
    sem --session/--offline: a conta salva (tela \"Conta\" no menu principal)
  mc-client --test-world --offline   (mundo plano local, sem rede; ver MINE_RS_TEST_* em test_world.rs)
  mc-client ping <servidor[:porta]>
  mc-client join <servidor[:porta]> [--session auth.json | --offline] [--name NOME]   (teste sem janela, 30 s)";

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// Quem joga: `--offline` (nick do `--name`), `--session auth.json` (um launcher), a conta salva
/// (`conta.json`, com o token renovado) ou, sem nenhuma, a tela de conta ao abrir.
struct Who {
    session: Option<auth::Session>,
    name: String,
    login: bool,
    error: Option<String>,
}

fn who(args: &[String]) -> Result<Who, BoxErr> {
    let named = flag(args, "--name").map(str::to_string);
    if args.iter().any(|a| a == "--offline") {
        let saved = match msa::Account::load() {
            Some(msa::Account::Offline(n)) => Some(n),
            _ => None,
        };
        return Ok(Who { session: None, name: named.or(saved).unwrap_or_else(|| "Jogador".into()), login: false, error: None });
    }
    if let Some(p) = flag(args, "--session") {
        let s = auth::Session::load(p)?;
        return Ok(Who { name: s.name.clone(), session: Some(s), login: false, error: None });
    }
    Ok(match msa::Account::load() {
        Some(acc) => match acc.clone().fresh() {
            Ok(a) => Who { session: a.session(), name: a.name().to_string(), login: false, error: None },
            Err(e) => Who { session: None, name: acc.name().to_string(), login: true, error: Some(format!("A sessão Microsoft não renovou: {e}")) },
        },
        None => Who { session: None, name: named.unwrap_or_else(|| "Jogador".into()), login: true, error: None },
    })
}

async fn ping(addr: &str) -> Result<(), BoxErr> {
    let (json, ms) = net::status(addr).await?;
    println!("{json}");
    println!("latência: {ms} ms");
    Ok(())
}

/// Entra sem janela por 30 s e mostra o que chegou (útil para testar rede e chunks).
fn join_headless(addr: &str, name: &str, session: Option<auth::Session>) -> Result<(), BoxErr> {
    let n = net::start(addr.to_string(), name.to_string(), session, 8);
    let deadline = Instant::now() + Duration::from_secs(30);
    let (mut columns, mut sections, mut blocks) = (0usize, 0usize, 0usize);
    let mut world = mc_world::World::new(true);
    while Instant::now() < deadline {
        let Ok(ev) = n.events.recv_timeout(Duration::from_millis(200)) else { continue };
        match ev {
            NetEvent::Status(s) => println!("{s}"),
            NetEvent::Joined { dimension } => println!("Join Game (dimensão {dimension})"),
            NetEvent::Chunk(c) => {
                columns += 1;
                sections += c.sections.len();
                world.apply(c);
            }
            NetEvent::Chunks(v) => {
                for c in v {
                    columns += 1;
                    sections += c.sections.len();
                    world.load(c);
                }
            }
            NetEvent::BlockChange { .. } | NetEvent::MultiBlockChange(_) => blocks += 1,
            NetEvent::PosLook { x, y, z, .. } => {
                let below = world.block(x.floor() as i32, y.floor() as i32 - 1, z.floor() as i32);
                println!("posição {x:.1} {y:.1} {z:.1} (bloco sob os pés: id {} meta {})", below >> 4, below & 15);
                let _ = n.out.send(serverbound::player_pos_look(x, y, z, 0.0, 0.0, false));
            }
            NetEvent::Chat { text, position, .. } if position != 2 => println!("chat: {}", hud::plain(&text)),
            NetEvent::Disconnected(r) => {
                println!("desconectado: {r}");
                break;
            }
            _ => {}
        }
    }
    println!("recebido: {columns} colunas, {sections} seções, {blocks} mudanças de bloco");
    Ok(())
}

fn main() {
    winapp::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&str> = {
        let mut v = Vec::new();
        let mut skip = false;
        for a in &args {
            if skip {
                skip = false;
            } else if matches!(a.as_str(), "--session" | "--name" | "--jar") {
                skip = true;
            } else if !a.starts_with("--") {
                v.push(a.as_str());
            }
        }
        v
    };

    let res: Result<(), BoxErr> = match positional.as_slice() {
        ["ping", addr] => tokio::runtime::Runtime::new().map_err(Into::into).and_then(|rt| rt.block_on(ping(addr))),
        ["join", addr] => who(&args).and_then(|w| join_headless(addr, &w.name, w.session)),
        [] | ["play"] | ["play", _] | [_] if !matches!(positional.first(), Some(&"ping") | Some(&"join")) => who(&args).and_then(|w| {
            let jar = setup::jar(flag(&args, "--jar"))?;
            setup::assets()?;
            winapp::progress_done();
            // Sem servidor: abre o menu principal (ou a tela de conta, sem conta).
            let server = positional.iter().copied().find(|a| *a != "play").map(str::to_string);
            let test_world = args.iter().any(|a| a == "--test-world");
            let login = w.login && server.is_none() && !test_world;
            game::run(game::Options { server, name: w.name, session: w.session, jar, test_world, login, login_error: w.error })
        }),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = res {
        eprintln!("erro: {e}");
        winapp::fatal(&format!("Erro: {e}"));
        std::process::exit(1);
    }
}

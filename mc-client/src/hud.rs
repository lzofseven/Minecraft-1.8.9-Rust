//! HUD do jogo (porte de GuiIngame, GuiNewChat, GuiChat e do placar lateral).

use std::collections::{HashMap, HashSet};

use mc_render::ui::{strip_codes, UiBatch, ICONS, PORTAL, PUMPKIN_BLUR, UNDERWATER, WIDGETS};

use crate::entity_net::EntityPacket;
use crate::gui;
use crate::hud_stats::{BossStatus, HealthBlink, HorseJump, Portal, Stats};
use crate::hud_track::Track;
use crate::potion::{self, Effects};

pub use crate::chat_gui::ChatCfg;
use crate::chat_gui::{ChatEffect, ChatScreen, HoverCtx, NewChat};

/// Posição do mouse como o chat precisa (`Mouse.getX/getY` e as coordenadas de GUI).
#[derive(Clone, Copy, Debug, Default)]
pub struct ChatMouse {
    /// Pixels da janela com a origem embaixo à esquerda (LWJGL).
    pub raw_x: i32,
    pub raw_y: i32,
    /// Fator de escala da GUI.
    pub factor: i32,
    /// Pixels de GUI (origem em cima à esquerda).
    pub gx: i32,
    pub gy: i32,
}

#[derive(Default)]
pub struct Team {
    pub prefix: String,
    pub suffix: String,
    /// `Team.EnumVisible`: 0 always, 1 never, 2 hideForOtherTeams, 3 hideForOwnTeam.
    pub visibility: u8,
    /// `getChatFormat().getColorIndex()` quando é uma das 16 cores (RESET por padrão).
    pub color: Option<u8>,
}

#[derive(Default)]
pub struct Scoreboard {
    pub objectives: HashMap<String, String>,
    /// Pontos por objetivo: jogador -> pontos.
    pub scores: HashMap<String, HashMap<String, i32>>,
    pub teams: HashMap<String, Team>,
    pub player_team: HashMap<String, String>,
    pub sidebar: Option<String>,
    /// Espaço 0 (lista do tab).
    pub list: Option<String>,
    /// Espaço 2 (abaixo do nome, `RenderPlayer.renderOffsetLivingLabel`).
    pub below_name: Option<String>,
    /// Objetivos com tipo de exibição `hearts`.
    pub hearts: HashSet<String>,
    /// Espaços 3..18: placar lateral de quem está num time da cor 0..15.
    pub team_sidebar: [Option<String>; 16],
    /// O próprio nome (`thePlayer.getName()`), para o placar lateral por cor do time.
    pub me: String,
}

impl Scoreboard {
    /// `GuiIngame.renderGameOverlay`: o espaço 3 + cor do time do jogador, senão o 1.
    pub fn sidebar_objective(&self) -> Option<&String> {
        let color = self.player_team.get(&self.me).and_then(|t| self.teams.get(t)).and_then(|t| t.color);
        color.and_then(|c| self.team_sidebar[c as usize].as_ref()).or(self.sidebar.as_ref())
    }

    /// Nome com prefixo e sufixo do time (`ScorePlayerTeam.formatPlayerName`).
    pub fn format_name(&self, player: &str) -> String {
        match self.player_team.get(player).and_then(|t| self.teams.get(t)) {
            Some(t) => format!("{}{}{}", t.prefix, player, t.suffix),
            None => player.to_string(),
        }
    }
}

/// `GuiIngame.displayedTitle`, `displayedSubTitle` e `titlesTimer`.
#[derive(Default)]
pub struct Title {
    pub title: String,
    pub subtitle: String,
    pub timer: i32,
}

/// Estado mostrado pelo HUD.
pub struct Hud {
    pub ticks: u64,
    /// `GuiNewChat`.
    pub chat: NewChat,
    /// `GuiChat` aberta (`mc.currentScreen instanceof GuiChat`).
    pub chat_screen: Option<ChatScreen>,
    /// Espelho de `chat_screen.is_some()` para quem só quer saber se o chat está aberto.
    pub chat_open: bool,
    pub chat_mouse: ChatMouse,
    pub title: Title,
    /// `titleFadeIn`, `titleDisplayTime` e `titleFadeOut` (lidos a cada quadro).
    pub title_times: (i32, i32, i32),
    pub action_bar: Option<(String, u64)>,
    pub scoreboard: Scoreboard,
    pub tab_header: String,
    pub tab_footer: String,
    pub show_tab: bool,
    pub tab: crate::tab_list::TabList,
    pub show_debug: bool,
    /// `highlightingItemStack`, o nome dele (`getDisplayName`, itálico se é nome próprio) e
    /// `remainingHighlightTicks`; `heldItemTooltips` liga o desenho.
    highlighting: Option<mc_protocol::nbt::Slot>,
    highlight_name: String,
    remaining_highlight_ticks: i32,
    pub held_item_tooltips: bool,
    /// `GuiAchievement` e as conquistas já recebidas nesta conexão.
    pub achievements: crate::achievement_toast::Achievements,
    /// `showLagometer` (F3 com Alt) e os tempos de quadro que ele desenha.
    pub lagometer: bool,
    pub frames: crate::debug_overlay::FrameTimer,
    pub health: f32,
    pub food: i32,
    pub xp_bar: f32,
    pub xp_level: i32,
    pub selected: u8,
    /// 0 sobrevivência, 1 criativo, 2 aventura, 3 espectador.
    pub gamemode: u8,
    /// Opções do chat (options.txt): visibilidade, opacidade, escala, largura e alturas (0..1).
    pub chat_cfg: ChatCfg,
    pub food_saturation: f32,
    pub hardcore: bool,
    /// `generic.maxHealth` do jogador (0x20) e `getTotalArmorValue` (calculado do inventário).
    pub max_health: f32,
    pub armor: i32,
    /// `hurtResistantTime` do jogador local (liga o pisca-pisca dos corações).
    pub hurt_resistant_time: i32,
    pub effects: Effects,
    pub track: Track,
    pub blink: HealthBlink,
    pub boss: BossStatus,
    pub portal: Portal,
    pub horse_jump: HorseJump,
    /// Entidade em que o jogador está montado.
    pub riding: Option<i32>,
    /// Entradas de cada quadro, preenchidas pelo `game.rs`.
    pub env: Env,
    /// Brilho do vinheta (`prevVignetteBrightness`).
    pub vignette: f32,
    /// Ganchos do `--test-world`: forçam entradas que o servidor/mundo daria.
    pub force: Force,
    /// OptiFine do jogador (vinheta).
    pub vis: crate::visuals::Visuals,
}

/// Entradas forçadas pelo `MINE_RS_TEST_HUD` (só no mundo de teste).
#[derive(Clone, Debug, Default)]
pub struct Force {
    pub water: bool,
    pub pumpkin: bool,
    pub horse: bool,
}

/// O que o HUD lê do jogo a cada quadro.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// 0 primeira pessoa.
    pub third_person: u8,
    /// Capacete do jogador é uma abóbora (`Blocks.pumpkin`).
    pub helmet_pumpkin: bool,
    /// `isInsideOfMaterial(Material.water)` nos olhos.
    pub eyes_in_water: bool,
    /// `Entity.getBrightness` (0..1).
    pub brightness: f32,
    /// `Minecraft.isFancyGraphicsEnabled()`.
    pub fancy: bool,
    /// Aviso da borda do mundo para a vinheta (0 longe dela).
    pub border_warn: f32,
    /// `isRidingHorse()` (montado num cavalo selado).
    pub riding_horse: bool,
    pub yaw: f32,
    pub pitch: f32,
    /// Hora do relógio do sistema em ms (pisca-pisca dos corações).
    pub now_ms: u64,
    pub spectator: bool,
}

impl Default for Hud {
    fn default() -> Self {
        Self {
            ticks: 0,
            chat: NewChat::default(),
            chat_screen: None,
            chat_open: false,
            chat_mouse: ChatMouse::default(),
            title: Title::default(),
            title_times: (10, 70, 20),
            action_bar: None,
            scoreboard: Scoreboard::default(),
            tab_header: String::new(),
            tab_footer: String::new(),
            show_tab: false,
            tab: Default::default(),
            show_debug: false,
            highlighting: None,
            highlight_name: String::new(),
            remaining_highlight_ticks: 0,
            held_item_tooltips: true,
            achievements: Default::default(),
            lagometer: false,
            frames: Default::default(),
            health: 20.0,
            food: 20,
            chat_cfg: ChatCfg::default(),
            xp_bar: 0.0,
            xp_level: 0,
            selected: 0,
            gamemode: 0,
            food_saturation: 5.0,
            hardcore: false,
            max_health: 20.0,
            armor: 0,
            hurt_resistant_time: 0,
            effects: Effects::default(),
            track: Track::new(),
            blink: HealthBlink::default(),
            boss: BossStatus::default(),
            portal: Portal::default(),
            horse_jump: HorseJump::default(),
            riding: None,
            env: Env::default(),
            vignette: 0.0,
            force: Force::default(),
            vis: Default::default(),
        }
    }
}

pub use crate::debug_overlay::DebugInfo;

impl Hud {
    /// `GuiNewChat.printChatMessage`.
    pub fn add_chat(&mut self, ui: &mut UiBatch, comp: &crate::chat::Component) {
        let open = self.chat_open;
        self.chat.print(ui, &self.chat_cfg, open, comp, self.ticks as i32);
    }

    /// `displayGuiScreen(new GuiChat(prefix))`: abre o chat com o texto inicial.
    pub fn open_chat(&mut self, prefix: &str) {
        self.chat_screen = Some(ChatScreen::new(prefix, self.chat.sent.len()));
        self.chat_open = true;
    }

    /// `GuiChat.onGuiClosed`.
    pub fn close_chat(&mut self) {
        self.chat_screen = None;
        self.chat_open = false;
        self.chat.reset_scroll();
    }

    /// `GuiChat.sendChatMessage(msg, addToHistory)`: guarda no histórico; o envio fica com o jogo.
    pub fn remember_sent(&mut self, msg: &str, add_to_history: bool) {
        if add_to_history {
            self.chat.add_sent(msg);
        }
    }

    /// Tecla com o chat aberto (`GuiChat.keyTyped`).
    pub fn chat_key(&mut self, ui: &mut UiBatch, code: winit::keyboard::KeyCode, text: Option<&str>, ctrl: bool, shift: bool) -> Vec<ChatEffect> {
        let Some(screen) = &mut self.chat_screen else { return Vec::new() };
        screen.key(ui, &mut self.chat, &self.chat_cfg, self.ticks as i32, code, text, ctrl, shift)
    }

    /// Roda do mouse com o chat aberto.
    pub fn chat_wheel(&mut self, up: bool, shift: bool) {
        if let Some(screen) = &self.chat_screen {
            screen.wheel(&mut self.chat, &self.chat_cfg, up, shift);
        }
    }

    /// Clique com o chat aberto (`GuiChat.mouseClicked`).
    pub fn chat_click(&mut self, ui: &mut UiBatch, button: i32, shift: bool) -> Vec<ChatEffect> {
        let m = self.chat_mouse;
        let span = self.chat.component_at(ui, &self.chat_cfg, true, m.raw_x, m.raw_y, m.factor);
        let Some(screen) = &mut self.chat_screen else { return Vec::new() };
        screen.mouse_clicked(ui, span.as_ref(), &self.chat_cfg, button, shift, m.gx, m.gy)
    }

    /// Resposta do servidor ao Tab (S3A).
    pub fn chat_tab_response(&mut self, ui: &mut UiBatch, names: &[String]) {
        if let Some(screen) = &mut self.chat_screen {
            screen.on_autocomplete_response(ui, &mut self.chat, &self.chat_cfg, self.ticks as i32, names);
        }
    }

    /// Dica do trecho sob o mouse (`GuiChat.drawScreen` -> `handleComponentHover`).
    pub fn draw_chat_hover(&mut self, ui: &mut UiBatch, w: i32, h: i32, ctx: &HoverCtx) {
        if !self.chat_open {
            return;
        }
        let m = self.chat_mouse;
        let Some(span) = self.chat.component_at(ui, &self.chat_cfg, true, m.raw_x, m.raw_y, m.factor) else { return };
        if let Some(ev) = &span.style.hover {
            if let Some(lines) = crate::chat_gui::hover_lines(ui, ev, ctx) {
                gui::tooltip(ui, &lines, m.gx, m.gy, w, h);
            }
        }
    }

    /// `GuiIngame.displayTitle`: só um dos casos vale por chamada (título, subtítulo ou os tempos).
    pub fn display_title(&mut self, title: Option<String>, subtitle: Option<String>, fade_in: i32, stay: i32, fade_out: i32) {
        let t = &mut self.title;
        if title.is_none() && subtitle.is_none() && fade_in < 0 && stay < 0 && fade_out < 0 {
            *t = Title::default();
        } else if let Some(s) = title {
            t.title = s;
            let (fi, st, fo) = self.title_times;
            t.timer = fi + st + fo;
        } else if let Some(s) = subtitle {
            t.subtitle = s;
        } else {
            let tt = &mut self.title_times;
            if fade_in >= 0 {
                tt.0 = fade_in;
            }
            if stay >= 0 {
                tt.1 = stay;
            }
            if fade_out >= 0 {
                tt.2 = fade_out;
            }
            if t.timer > 0 {
                t.timer = tt.0 + tt.1 + tt.2;
            }
        }
    }

    fn survival_like(&self) -> bool {
        self.gamemode == 0 || self.gamemode == 2
    }

    /// `EntityPlayerSP.setPlayerSPHealth` + `FoodStats`: vida chegando do servidor (0x06).
    pub fn set_health(&mut self, health: f32, food: i32, saturation: f32, valid_before: bool) {
        if valid_before {
            let f = self.health - health;
            if f <= 0.0 {
                if f < 0.0 {
                    self.hurt_resistant_time = 10;
                }
            } else {
                self.hurt_resistant_time = 20;
            }
        }
        self.health = health;
        self.food = food;
        self.food_saturation = saturation;
    }

    /// Pacotes de entidade que o HUD acompanha (absorção, ar, vida da montaria, chefes, dano).
    pub fn entity_packet(&mut self, p: &EntityPacket, own: Option<i32>) {
        self.track.packet(p, own);
        if let EntityPacket::Status { id, status: 2 } = p {
            if Some(*id) == own {
                self.hurt_resistant_time = 20;
            }
        }
        if let EntityPacket::Attach { entity, vehicle, leash: false } = p {
            if Some(*entity) == own {
                self.riding = (*vehicle >= 0).then_some(*vehicle);
            }
        }
    }

    /// Novo `EntityPlayerSP` (entrada no mundo ou respawn): efeitos, montaria e entidades rastreadas somem.
    pub fn on_respawn(&mut self) {
        self.effects.clear();
        self.track = Track::new();
        self.riding = None;
        self.blink = HealthBlink::default();
        self.boss = BossStatus::default();
        self.portal = Portal::default();
        self.hurt_resistant_time = 0;
    }

    /// `MINE_RS_TEST_HUD=chave=valor,chave...` (mundo de teste): simula o que o servidor mandaria.
    /// Chaves: gm, health, max, absorb, food, sat, air, armor, poison, wither, regen, hunger, hardcore,
    /// hurt, water, pumpkin, portal, horse, mount=vida/max, boss=wither|dragon:vida/max, eff=id:amp:dur;...
    pub fn apply_test(&mut self, spec: &str) {
        for kv in spec.split(',').filter(|s| !s.is_empty()) {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            let f = |d: f32| v.parse::<f32>().unwrap_or(d);
            let pair = |v: &str| v.split_once('/').map(|(a, b)| (a.parse::<f32>().unwrap_or(0.0), b.parse::<f32>().unwrap_or(20.0)));
            match k {
                "gm" => self.gamemode = f(0.0) as u8,
                "health" => self.health = f(20.0),
                "max" => self.max_health = f(20.0),
                "absorb" => self.track.absorption = f(0.0),
                "food" => self.food = f(20.0) as i32,
                "sat" => self.food_saturation = f(5.0),
                "air" => self.track.air = f(300.0) as i32,
                "armor" => self.armor = f(0.0) as i32,
                "hardcore" => self.hardcore = true,
                "hurt" => self.hurt_resistant_time = 20,
                "water" => self.force.water = true,
                "pumpkin" => self.force.pumpkin = true,
                "horse" => self.force.horse = true,
                "portal" => self.portal = Portal { time: f(0.8), prev: f(0.8) },
                "poison" | "wither" | "regen" | "hunger" => {
                    let id = match k {
                        "poison" => potion::POISON,
                        "wither" => potion::WITHER,
                        "regen" => potion::REGENERATION,
                        _ => potion::HUNGER,
                    };
                    self.effects.add(potion::Effect { id, amplifier: 0, duration: 1200, show_particles: true, duration_max: false });
                }
                "eff" => {
                    for e in v.split(';') {
                        let mut it = e.split(':').map(|x| x.parse::<i32>().unwrap_or(0));
                        let (id, amp, dur) = (it.next().unwrap_or(1), it.next().unwrap_or(0), it.next().unwrap_or(1200));
                        self.effects.add(potion::Effect { id: id as u8, amplifier: amp as i8, duration: dur, show_particles: true, duration_max: dur == 32767 });
                    }
                }
                "mount" => {
                    if let Some((h, m)) = pair(v) {
                        self.riding = Some(-1);
                        self.track.health.insert(-1, h);
                        self.track.max_health.insert(-1, m);
                    }
                }
                "boss" => {
                    let (kind, rest) = v.split_once(':').unwrap_or((v, "100/300"));
                    let (h, m) = pair(rest).unwrap_or((100.0, 300.0));
                    let k = if kind == "dragon" { crate::hud_track::ENDER_DRAGON } else { crate::hud_track::WITHER };
                    self.track.bosses.insert(-2, k);
                    self.track.health.insert(-2, h);
                    self.track.max_health.insert(-2, m);
                }
                _ => {}
            }
        }
    }

    /// Um tick do cliente (`GuiIngame.updateTick` + partes de `EntityLivingBase.onUpdate`).
    pub fn tick(&mut self, in_portal: bool, held: Option<(mc_protocol::nbt::Slot, String)>) -> bool {
        self.ticks += 1;
        if self.title.timer > 0 {
            self.title.timer -= 1;
            if self.title.timer <= 0 {
                self.title.title.clear();
                self.title.subtitle.clear();
            }
        }
        match held {
            None => self.remaining_highlight_ticks = 0,
            Some((s, name)) => {
                let same = self.highlighting.as_ref().is_some_and(|h| {
                    h.id == s.id && crate::containers::same_tags(&s, h) && (crate::containers::is_damageable(&s) || s.damage == h.damage)
                });
                if same {
                    if self.remaining_highlight_ticks > 0 {
                        self.remaining_highlight_ticks -= 1;
                    }
                } else {
                    self.remaining_highlight_ticks = 40;
                    self.highlight_name = name;
                }
                self.highlighting = Some(s);
            }
        }
        if self.hurt_resistant_time > 0 {
            self.hurt_resistant_time -= 1;
        }
        self.effects.tick();
        let confusion = self.effects.get(potion::CONFUSION).map_or(0, |e| e.duration);
        self.portal.tick(in_portal, confusion)
    }

    fn mount_health(&self) -> Option<crate::hud_stats::MountHealth> {
        self.riding.and_then(|id| self.track.mount(id))
    }

    /// Desenha tudo na área de GUI `w` x `h`.
    pub fn draw(&mut self, ui: &mut UiBatch, w: f32, h: f32, partial: f32, debug: &DebugInfo) {
        // ScaledResolution: largura e altura são inteiras (teto).
        let (w, h) = (w.ceil(), h.ceil());
        let cx = (w / 2.0).floor();
        let (wi, hi) = (w as i32, h as i32);
        self.draw_overlays(ui, w, h, partial);
        if self.gamemode != 3 {
            ui.gui(WIDGETS, cx - 91.0, h - 22.0, 0.0, 0.0, 182.0, 22.0);
            ui.gui(WIDGETS, cx - 91.0 - 1.0 + self.selected as f32 * 20.0, h - 22.0 - 1.0, 0.0, 22.0, 24.0, 22.0);
        }
        self.draw_boss(ui, wi);
        if self.survival_like() {
            self.draw_stats(ui, wi, hi);
        }
        if self.env.riding_horse {
            let (y, fill) = crate::hud_stats::jump_bar(hi, self.horse_jump.power);
            let x = cx - 91.0;
            ui.gui(ICONS, x, y as f32, 0.0, 84.0, 182.0, 5.0);
            if fill > 0 {
                ui.gui(ICONS, x, y as f32, 0.0, 89.0, fill as f32, 5.0);
            }
        } else if self.survival_like() {
            self.draw_exp(ui, wi, hi);
        }
        // O seletor do espectador (`GuiSpectator.renderSelectedItem`) não foi portado.
        if self.held_item_tooltips && self.gamemode != 3 {
            self.draw_selected_item(ui, wi, hi);
        }
        self.draw_scoreboard(ui, w, h);
        self.draw_action_bar(ui, cx, h, partial);
        self.draw_title(ui, cx, h, partial);
        self.draw_chat(ui, h);
        // GuiIngame.renderGameOverlay: updatePlayerList(false) com a tecla solta.
        self.tab.update(self.show_tab);
        if self.show_tab {
            self.draw_tab(ui, wi);
        }
        if self.show_debug {
            crate::debug_overlay::draw(ui, wi, debug);
            if self.lagometer {
                crate::debug_overlay::draw_lagometer(ui, hi, &self.frames, debug.max_fps);
            }
        }
        if let Some(screen) = &mut self.chat_screen {
            screen.draw(ui, w as i32, h as i32);
        }
    }

    /// Vinheta, abóbora, portal e água (`GuiIngame.renderGameOverlay` / `ItemRenderer.renderOverlays`).
    fn draw_overlays(&mut self, ui: &mut UiBatch, w: f32, h: f32, partial: f32) {
        self.env.eyes_in_water |= self.force.water;
        self.env.helmet_pumpkin |= self.force.pumpkin;
        self.env.riding_horse |= self.force.horse;
        let env = self.env.clone();
        // OptiFine: o ofVignette decide; sem ele, os gráficos detalhados. Perto da borda do mundo a cor é (0, f, f).
        if self.vis.vignette.unwrap_or(env.fancy) {
            // renderVignette: 1 - brilho, suavizado a cada quadro.
            let target = (1.0 - env.brightness).clamp(0.0, 1.0);
            self.vignette += (target - self.vignette) * 0.01;
            let v = self.vignette;
            let rgb = if env.border_warn > 0.0 { [0.0, env.border_warn, env.border_warn] } else { [v, v, v] };
            ui.vignette(w, h, rgb);
        }
        if env.third_person == 0 && env.helmet_pumpkin {
            ui.sprite_alpha(0.0, 0.0, w, h, PUMPKIN_BLUR.0 as f32, PUMPKIN_BLUR.1 as f32, 256.0, 256.0, 0xFF00_0000 | 0xFFFFFF);
        }
        if !self.effects.has(potion::CONFUSION) {
            let a = self.portal.alpha(partial);
            if a > 0.0 {
                let alpha = ((a.clamp(0.0, 1.0)) * 255.0) as u32;
                ui.sprite_alpha(0.0, 0.0, w, h, PORTAL.0 as f32, PORTAL.1 as f32, 16.0, 16.0, (alpha << 24) | 0xFFFFFF);
            }
        }
        if env.third_person == 0 && env.eyes_in_water && !env.spectator {
            // renderWaterOverlayTexture: cor = brilho, alfa 0.5; tex repete 4x e desliza com a direção.
            let f = (env.brightness.clamp(0.0, 1.0) * 255.0) as u32;
            let color = (128 << 24) | (f << 16) | (f << 8) | f;
            let (ox, oy) = (-env.yaw / 64.0, env.pitch / 64.0);
            let (tw, th) = (w / 4.0, h / 4.0);
            let (sx, sy) = ((ox.rem_euclid(1.0) - 1.0) * tw, (oy.rem_euclid(1.0) - 1.0) * th);
            for i in 0..6 {
                for j in 0..6 {
                    ui.sprite_alpha(sx + i as f32 * tw, sy + j as f32 * th, tw, th, UNDERWATER.0 as f32, UNDERWATER.1 as f32, 16.0, 16.0, color);
                }
            }
        }
    }

    /// `renderBossHealth`.
    fn draw_boss(&mut self, ui: &mut UiBatch, w: i32) {
        // BossStatus.setBossStatus é chamado quando o render do chefe roda: aqui, a cada quadro para
        // os chefes rastreados (o último vence).
        let mut ids: Vec<i32> = self.track.bosses.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let kind = self.track.bosses[&id];
            let name = match self.track.names.get(&id).filter(|n| !n.is_empty()) {
                Some(n) => n.clone(),
                None => crate::lang::tr(if kind == crate::hud_track::WITHER { "entity.WitherBoss.name" } else { "entity.EnderDragon.name" }),
            };
            let health = self.track.health.get(&id).copied().unwrap_or(0.0);
            let max = self.track.max_health.get(&id).copied().unwrap_or(20.0);
            self.boss.set(name, health, max, kind == crate::hud_track::WITHER);
        }
        if let Some((x, y, fill)) = self.boss.frame(w) {
            ui.gui(ICONS, x as f32, y as f32, 0.0, 74.0, 182.0, 5.0);
            if fill > 0 {
                ui.gui(ICONS, x as f32, y as f32, 0.0, 79.0, fill as f32, 5.0);
            }
            if let Some(name) = &self.boss.name {
                let tw = ui.text_width(name);
                ui.text((w as f32 / 2.0 - tw / 2.0).floor(), (y - 10) as f32, name, 0xFFFFFF, true);
            }
        }
    }

    /// Barra de experiência e nível (`renderExpBar`).
    fn draw_exp(&mut self, ui: &mut UiBatch, w: i32, h: i32) {
        let x = (w / 2 - 91) as f32;
        let (y, fill) = crate::hud_stats::exp_bar(h, self.xp_bar);
        // xpBarCap() > 0 sempre vale para níveis normais (7 + 2*nível).
        if crate::hud_stats::xp_bar_cap(self.xp_level) > 0 {
            ui.gui(ICONS, x, y as f32, 0.0, 64.0, 182.0, 5.0);
            if fill > 0 {
                ui.gui(ICONS, x, y as f32, 0.0, 69.0, fill as f32, 5.0);
            }
        }
        if self.xp_level > 0 {
            let s = self.xp_level.to_string();
            let tw = ui.text_width(&s);
            let tx = ((w - tw as i32) / 2) as f32;
            let ty = h as f32 - 31.0 - 4.0;
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                ui.text(tx + dx, ty + dy, &s, 0x000000, false);
            }
            ui.text(tx, ty, &s, 0x80FF20, false);
        }
    }

    /// Entradas de `renderPlayerStats` a partir do estado atual.
    pub fn stats(&self, blink: bool, last_health: i32) -> Stats {
        Stats {
            health: self.health,
            max_health: self.max_health,
            absorption: self.track.absorption,
            armor: self.armor,
            food: self.food,
            prev_food: self.food,
            saturation: self.food_saturation,
            poison: self.effects.has(potion::POISON),
            wither: self.effects.has(potion::WITHER),
            regeneration: self.effects.has(potion::REGENERATION),
            hunger: self.effects.has(potion::HUNGER),
            hardcore: self.hardcore,
            in_water: self.env.eyes_in_water || self.force.water,
            air: self.track.air,
            riding: self.riding.is_some(),
            mount: self.mount_health(),
            update_counter: self.ticks as i32,
            hide_food: false,
            blink,
            last_health,
        }
    }

    fn draw_stats(&mut self, ui: &mut UiBatch, w: i32, h: i32) {
        let (blink, last) = self.blink.step(self.health, self.hurt_resistant_time, self.ticks as i32, self.env.now_ms);
        let stats = self.stats(blink, last);
        for i in crate::hud_stats::layout(&stats, w, h) {
            ui.gui(ICONS, i.x as f32, i.y as f32, i.u as f32, i.v as f32, 9.0, 9.0);
        }
    }

    /// `Scoreboard.getSortedScores` sem os "#", no máximo os 15 últimos: (jogador, pontos) em ordem crescente.
    fn sidebar_scores(&self) -> Option<(String, Vec<(String, i32)>)> {
        let sb = &self.scoreboard;
        let obj = sb.sidebar_objective()?;
        let display = sb.objectives.get(obj)?.clone();
        let mut scores: Vec<(String, i32)> = sb
            .scores
            .get(obj)
            .map(|m| m.iter().filter(|(p, _)| !p.starts_with('#')).map(|(p, v)| (p.clone(), *v)).collect())
            .unwrap_or_default();
        // Score.scoreComparator: pontos crescentes; empate pelo nome ao contrário, sem diferenciar maiúsculas.
        scores.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.to_lowercase().cmp(&a.0.to_lowercase())));
        if scores.len() > 15 {
            scores.drain(..scores.len() - 15);
        }
        Some((display, scores))
    }

    fn draw_scoreboard(&mut self, ui: &mut UiBatch, w: f32, h: f32) {
        let Some((display, scores)) = self.sidebar_scores() else { return };
        let sb = &self.scoreboard;
        let lines: Vec<(String, String)> = scores.iter().map(|(p, v)| (sb.format_name(p), format!("§c{v}"))).collect();

        let mut width = ui.text_width(&display);
        for (name, pts) in &lines {
            width = width.max(ui.text_width(&format!("{name}: {pts}")));
        }
        let total = lines.len() as f32 * 9.0;
        let base = (h / 2.0).floor() + (total / 3.0).floor();
        let left = w - width - 3.0;
        let right = w - 3.0 + 2.0;
        for (i, (name, pts)) in lines.iter().enumerate() {
            let y = base - (i + 1) as f32 * 9.0;
            ui.rect(left - 2.0, y, right - (left - 2.0), 9.0, 0x50000000);
            ui.text(left, y, name, 0xFFFFFF, false);
            let pw = ui.text_width(pts);
            ui.text(right - pw, y, pts, 0xFFFFFF, false);
            if i + 1 == lines.len() {
                ui.rect(left - 2.0, y - 9.0 - 1.0, right - (left - 2.0), 9.0, 0x60000000);
                ui.rect(left - 2.0, y - 1.0, right - (left - 2.0), 1.0, 0x50000000);
                let dw = ui.text_width(&display);
                ui.text(left + (width / 2.0).floor() - (dw / 2.0).floor(), y - 9.0, &display, 0xFFFFFF, false);
            }
        }
    }

    /// `GuiIngame.renderSelectedItem`.
    fn draw_selected_item(&mut self, ui: &mut UiBatch, w: i32, h: i32) {
        if self.remaining_highlight_ticks <= 0 || self.highlighting.is_none() {
            return;
        }
        let name = self.highlight_name.clone();
        let x = (w - ui.text_width(&name) as i32) / 2;
        // shouldDrawHUD: só sobrevivência e aventura têm a barra de vida em cima.
        let y = h - 59 + if self.survival_like() { 0 } else { 14 };
        let alpha = (self.remaining_highlight_ticks as f32 * 256.0 / 10.0) as u32;
        let alpha = alpha.min(255);
        if alpha > 0 {
            ui.text(x as f32, y as f32, &name, 0xFFFFFF | alpha << 24, true);
        }
    }

    fn fade_alpha(age: f32, total: f32) -> u32 {
        let a = ((total - age) * 255.0 / 20.0).clamp(0.0, 255.0) as u32;
        a.min(255)
    }

    fn draw_action_bar(&mut self, ui: &mut UiBatch, cx: f32, h: f32, partial: f32) {
        let Some((text, start)) = &self.action_bar else { return };
        let age = (self.ticks - start) as f32 + partial;
        if age > 60.0 {
            self.action_bar = None;
            return;
        }
        let alpha = Self::fade_alpha(age, 60.0);
        if alpha > 8 {
            let text = text.clone();
            let tw = ui.text_width(&text);
            ui.text(cx - tw / 2.0, h - 68.0 - 4.0, &text, (alpha << 24) | 0xFFFFFF, true);
        }
    }

    fn draw_title(&mut self, ui: &mut UiBatch, cx: f32, h: f32, partial: f32) {
        // GuiIngame.renderGameOverlay: o alfa sai do titlesTimer com os tempos atuais (divisão por zero como o
        // Java: o `as` do Rust satura igual ao `(int)` de float).
        let t = &self.title;
        if t.timer <= 0 {
            return;
        }
        let (fi, st, fo) = self.title_times;
        let f = t.timer as f32 - partial;
        let mut a: i32 = 255;
        if t.timer > fo + st {
            a = (((fi + st + fo) as f32 - f) * 255.0 / fi as f32) as i32;
        }
        if t.timer <= fo {
            a = (f * 255.0 / fo as f32) as i32;
        }
        let a = a.clamp(0, 255) as u32;
        if a <= 8 {
            return;
        }
        let (title, subtitle) = (t.title.clone(), t.subtitle.clone());
        let cy = (h / 2.0).floor();
        let tw = (ui.text_width(&title) as i32 / 2) as f32 * 4.0;
        ui.text_scaled(cx - tw, cy - 10.0 * 4.0, &title, (a << 24) | 0xFFFFFF, true, 4.0);
        let sw = (ui.text_width(&subtitle) as i32 / 2) as f32 * 2.0;
        ui.text_scaled(cx - sw, cy + 5.0 * 2.0, &subtitle, (a << 24) | 0xFFFFFF, true, 2.0);
    }

    /// `GuiNewChat.drawChat`.
    fn draw_chat(&mut self, ui: &mut UiBatch, h: f32) {
        self.chat.draw(ui, &self.chat_cfg, self.chat_open, self.ticks as i32, h);
    }

    fn draw_tab(&mut self, ui: &mut UiBatch, screen_w: i32) {
        let sb = &self.scoreboard;
        let objective = sb.list.as_deref().filter(|o| sb.objectives.contains_key(*o)).map(|o| crate::tab_list::ListObjective { name: o, hearts: sb.hearts.contains(o) });
        self.tab.draw(ui, screen_w, sb, objective, &self.tab_header, &self.tab_footer, self.ticks);
    }

}

/// Mensagem para o terminal (sem cores).
pub fn plain(s: &str) -> String {
    strip_codes(s)
}

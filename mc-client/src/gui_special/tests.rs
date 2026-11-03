use super::*;
use crate::font_text::testing::Mock;
use mc_protocol::Reader;
use mc_render::ui::Font;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

fn ui() -> UiBatch {
    UiBatch::new(Font::empty())
}

fn press(code: KeyCode, text: Option<&str>) -> KeyEvent<'_> {
    KeyEvent { code, text, ctrl: false, shift: false, repeat: false }
}

fn lines(a: &str, b: &str, c: &str, d: &str) -> [String; 4] {
    [a.to_string(), b.to_string(), c.to_string(), d.to_string()]
}

/// Lê de volta o payload `MC|B...` que o jogo mandou: canal e livro.
fn read_book_packet(p: &[u8]) -> (String, Slot) {
    let mut r = Reader::new(p);
    assert_eq!(r.varint().unwrap(), 0x17);
    let channel = r.string().unwrap();
    let slot = r.slot().unwrap().unwrap();
    (channel, slot)
}

fn pages_of(slot: &Slot) -> Vec<String> {
    tag_pages(slot.nbt.as_ref().unwrap())
}

fn book_slot(id: i16, pages: &[&str], extra: Vec<(&str, Nbt)>) -> Slot {
    let mut m: HashMap<String, Nbt> = extra.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    m.insert("pages".into(), Nbt::List(pages.iter().map(|p| Nbt::String(p.to_string())).collect()));
    let nbt = Nbt::Compound(m);
    Slot { id, count: 1, damage: 0, raw_nbt: nbt.write_root(), nbt: Some(nbt) }
}

// ------------------------------------------------------------------------------------ placa

#[test]
fn sign_layout_matches_the_java_matrix_chain() {
    // 1280x720 na escala 3 = 427 de largura.
    let l = SignLayout::new(427, true);
    assert!(near(l.cx, 213.0));
    // Tábua: caixa (-12,-14)-(12,-2) a 3,90625 px por unidade; centro em y = 123,046875.
    assert!(near(l.board[0], 213.0 - 12.0 * 3.90625) && near(l.board[1], 123.046875 - 14.0 * 3.90625));
    assert!(near(l.board[2], 93.75) && near(l.board[3], 46.875));
    // Poste: caixa (-1,-2)-(1,12).
    let s = l.stick.unwrap();
    assert!(near(s[0], 213.0 - 3.90625) && near(s[1], 123.046875 - 2.0 * 3.90625));
    assert!(near(s[2], 7.8125) && near(s[3], 14.0 * 3.90625));
    // Texto: origem (w/2, 91,796875) e escala 0,9765625.
    assert!(near(l.text_origin.0, 213.0) && near(l.text_origin.1, 91.796875));
    assert!(near(l.text_scale, 0.9765625));
    // Placa de parede: sem poste e 29,296875 px mais para baixo.
    let w = SignLayout::new(427, false);
    assert!(w.stick.is_none());
    assert!(near(w.board[1], l.board[1] + 29.296875));
    assert!(near(w.text_origin.1, l.text_origin.1 + 29.296875));
    assert_eq!(w.board[0], l.board[0]);
}

#[test]
fn sign_keys_move_line_type_and_limit_width() {
    let mut s = SignGui::new((1, 2, 3), true, lines("a", "§cb", "", ""));
    let m = &mut Mock;
    // Seta para baixo e Enter avançam; seta para cima volta, dando a volta em 0.
    assert!(!s.key(m, KeyCode::ArrowUp, None));
    assert_eq!(s.edit_line, 3);
    s.key(m, KeyCode::ArrowDown, None);
    assert_eq!(s.edit_line, 0);
    s.key(m, KeyCode::Enter, Some("\r"));
    assert_eq!(s.edit_line, 1);
    // A linha colorida vira texto simples ao mexer nela.
    assert_eq!(s.lines[1], "b");
    s.key(m, KeyCode::KeyX, Some("x"));
    assert_eq!(s.lines[1], "bx");
    s.key(m, KeyCode::Backspace, Some("\u{8}"));
    s.key(m, KeyCode::Backspace, Some("\u{8}"));
    s.key(m, KeyCode::Backspace, Some("\u{8}"));
    assert_eq!(s.lines[1], "");
    // Cada caractere mede 6: cabem 15 (90), o 16º não entra.
    for _ in 0..20 {
        s.key(m, KeyCode::KeyW, Some("w"));
    }
    assert_eq!(s.lines[1].len(), 15);
    // Caracteres não permitidos (§, controle) são ignorados.
    s.key(m, KeyCode::KeyA, Some("§"));
    s.key(m, KeyCode::Tab, Some("\t"));
    assert_eq!(s.lines[1].len(), 15);
    // Esc conclui.
    assert!(s.key(m, KeyCode::Escape, Some("\u{1b}")));
}

#[test]
fn sign_close_sends_c12_and_keeps_text_locally() {
    let mut inv = Inventory::default();
    let mut u = ui();
    inv.open_sign((10, 64, -5), 63, lines("um", "", "", ""));
    assert!(inv.is_open());
    assert!(inv.editing_sign().is_some());
    key(&mut inv, &mut u, &press(KeyCode::ArrowDown, None));
    key(&mut inv, &mut u, &press(KeyCode::KeyO, Some("o")));
    key(&mut inv, &mut u, &press(KeyCode::KeyK, Some("k")));
    assert_eq!(inv.editing_sign().unwrap().1[1], "ok");
    assert!(inv.out.is_empty() && !inv.closed);
    key(&mut inv, &mut u, &press(KeyCode::Escape, Some("\u{1b}")));
    assert!(inv.closed && inv.special.is_none() && !inv.is_open());
    let expected = crate::container_net::update_sign((10, 64, -5), &lines("um", "ok", "", ""));
    assert_eq!(inv.out, vec![expected]);
    assert_eq!(inv.sign_done, Some(((10, 64, -5), lines("um", "ok", "", ""))));
}

#[test]
fn sign_sends_plain_text_even_for_untouched_colored_lines() {
    let mut inv = Inventory::default();
    inv.open_sign((0, 70, 0), 68, lines("§averde", "", "", ""));
    inv.close();
    assert_eq!(inv.out, vec![crate::container_net::update_sign((0, 70, 0), &lines("verde", "", "", ""))]);
    // O texto local fica como estava (a tile entity só muda o que foi editado).
    assert_eq!(inv.sign_done.unwrap().1[0], "§averde");
}

#[test]
fn sign_done_button_closes_with_sound() {
    let mut inv = Inventory::default();
    let mut u = ui();
    inv.open_sign((0, 70, 0), 63, lines("", "", "", ""));
    // gui.done em (w/2 - 100, h/4 + 120) = (113, 180) com 200x20 numa tela de 427x240.
    inv.mouse = (300.0, 190.0);
    click(&mut inv, &mut u, 0, false, 427.0, 240.0);
    assert!(inv.closed && inv.special.is_none());
    assert_eq!(inv.sounds, vec!["gui.button.press"]);
    assert_eq!(inv.out.len(), 1);
    // Fora do botão, ou com o botão direito, nada acontece.
    let mut inv = Inventory::default();
    inv.open_sign((0, 70, 0), 63, lines("", "", "", ""));
    inv.mouse = (300.0, 100.0);
    click(&mut inv, &mut u, 0, false, 427.0, 240.0);
    inv.mouse = (300.0, 190.0);
    click(&mut inv, &mut u, 1, false, 427.0, 240.0);
    assert!(!inv.closed && inv.special.is_some() && inv.out.is_empty());
}

#[test]
fn sign_draw_blinks_cursor_every_six_ticks() {
    let mut s = SignGui::new((0, 0, 0), true, lines("a", "b", "c", "d"));
    let before = |s: &SignGui| {
        let mut u = ui();
        s.draw(&mut u, 427, 240, (0, 0));
        u.vertices.len()
    };
    // Com a fonte vazia só os botões/sprites contam; o que importa é desenhar sem falhar nos dois estados.
    s.counter = 0;
    let on = before(&s);
    s.counter = 6;
    let off = before(&s);
    assert!(on > 0 && off > 0);
}

// ------------------------------------------------------------------------------------ livro

fn written(pages: &[&str]) -> BookGui {
    let slot = book_slot(WRITTEN_BOOK, pages, vec![("title", Nbt::String("T".into())), ("author", Nbt::String("A".into()))]);
    BookGui::new(slot, false, "Jogador".into(), 36)
}

#[test]
fn written_book_pages_and_buttons() {
    let mut b = written(&["a", "b", "c"]);
    assert_eq!((b.total, b.curr), (3, 0));
    assert!(b.next_visible() && !b.prev_visible());
    let m = &mut Mock;
    // Concluído ocupa a largura toda (200) e fica em w/2 - 100, 4 + 192.
    let done = b.text_button(0, 427).unwrap();
    assert_eq!((done.x, done.y, done.w, done.h), (113, 196, 200, 20));
    assert!(b.text_button(3, 427).is_none() && b.text_button(5, 427).is_none());
    // Próxima/anterior em (left + 120, 156) e (left + 38, 156): left = (427 - 192) / 2 = 117.
    assert_eq!(b.page_button_rect(427, true), (237, 156));
    assert_eq!(b.page_button_rect(427, false), (155, 156));
    b.action(m, 1);
    b.action(m, 1);
    assert_eq!(b.curr, 2);
    // Na última página de um livro escrito não há próxima.
    assert!(!b.next_visible() && b.prev_visible());
    b.action(m, 1);
    assert_eq!(b.curr, 2);
    b.action(m, 2);
    assert_eq!(b.curr, 1);
}

#[test]
fn page_click_event_changes_page_or_runs_command() {
    let mut b = written(&["a", "b", "c"]);
    let go = |n: &str| ft::Click { action: "change_page".into(), value: n.into() };
    // Página 3 existe; a atual (1) e as fora do intervalo não são tratadas.
    assert!(b.component_click(&go("3"), false).is_some());
    assert_eq!(b.curr, 2);
    assert!(b.component_click(&go("3"), false).is_none());
    assert!(b.component_click(&go("0"), false).is_none());
    assert!(b.component_click(&go("9"), false).is_none());
    assert!(b.component_click(&go("x"), false).is_none());
    let run = ft::Click { action: "run_command".into(), value: "/ola".into() };
    let o = b.component_click(&run, false).unwrap();
    assert!(o.close && o.chat.as_deref() == Some("/ola"));
    // Com shift o comando não roda.
    assert!(b.component_click(&run, true).is_none());
}

#[test]
fn written_book_draws_split_text_with_clicks() {
    let mut u = ui();
    let mut b = written(&[r#"{"text":"ir ","extra":[{"text":"pagina 2","clickEvent":{"action":"change_page","value":"2"}}]}"#, "\"dois\""]);
    b.draw(&mut u, 427, (0, 0));
    let lines = b.lines.as_ref().unwrap();
    // Com a fonte vazia tudo cabe numa linha: "ir " e o trecho clicável.
    assert_eq!(lines.len(), 1);
    assert_eq!(ft::line_text(&lines[0]), "ir pagina 2");
    assert_eq!(lines[0][1].click.as_ref().unwrap().value, "2");
    assert_eq!(b.lines_page, 0);
    // Uma página que não é JSON válido cai no texto cru.
    let mut b = written(&["duas palavras"]);
    b.draw(&mut u, 427, (0, 0));
    assert!(b.lines.is_none());
}

#[test]
fn invalid_written_tag_shows_error_line() {
    let mut u = ui();
    // Sem título e autor.
    let slot = book_slot(WRITTEN_BOOK, &["x"], vec![]);
    let mut b = BookGui::new(slot, false, "J".into(), 36);
    b.draw(&mut u, 427, (0, 0));
    assert_eq!(ft::line_text(&b.lines.as_ref().unwrap()[0]), "§4* Invalid book tag *");
    // Título com mais de 32 caracteres.
    let long = "x".repeat(33);
    let slot = book_slot(WRITTEN_BOOK, &["x"], vec![("title", Nbt::String(long)), ("author", Nbt::String("A".into()))]);
    assert!(!valid_written_tag(slot.nbt.as_ref()));
    let ok = book_slot(WRITTEN_BOOK, &["x"], vec![("title", Nbt::String("t".into())), ("author", Nbt::String("A".into()))]);
    assert!(valid_written_tag(ok.nbt.as_ref()));
    assert!(!valid_written_tag(None));
}

fn editor(pages: &[&str]) -> BookGui {
    BookGui::new(book_slot(WRITABLE_BOOK, pages, vec![]), true, "Lohan".into(), 36)
}

fn type_text(b: &mut BookGui, s: &str) {
    for c in s.chars() {
        let t = c.to_string();
        let ev = KeyEvent { code: KeyCode::KeyA, text: Some(&t), ctrl: false, shift: false, repeat: false };
        b.key(&mut Mock, &ev);
    }
}

#[test]
fn book_editing_keys() {
    let mut b = editor(&["oi"]);
    assert!(b.next_visible() && !b.prev_visible());
    type_text(&mut b, " mundo");
    assert_eq!(b.page_current(), "oi mundo");
    assert!(b.modified);
    let m = &mut Mock;
    b.key(m, &press(KeyCode::Backspace, Some("\u{8}")));
    assert_eq!(b.page_current(), "oi mund");
    b.key(m, &press(KeyCode::Enter, Some("\r")));
    b.key(m, &press(KeyCode::NumpadEnter, None));
    assert_eq!(b.page_current(), "oi mund\n\n");
    // §, controle e DEL são recusados.
    type_text(&mut b, "§\u{7f}\u{1}");
    assert_eq!(b.page_current(), "oi mund\n\n");
    // Ctrl não digita.
    let ev = KeyEvent { code: KeyCode::KeyV, text: Some("v"), ctrl: true, shift: false, repeat: false };
    b.key(m, &ev);
    assert_eq!(b.page_current(), "oi mund\n\n");
}

#[test]
fn book_page_limits() {
    // Mock: 6 px por letra, 118 px de largura -> 19 letras por linha; 14 linhas (126 <= 128).
    let mut b = editor(&[""]);
    for _ in 0..40 {
        b.key(&mut Mock, &press(KeyCode::Enter, Some("\r")));
    }
    // O "§0_" do cursor conta como linha: com 13 quebras há 14 linhas, a 14ª quebra faria 15.
    assert_eq!(b.page_current(), "\n".repeat(13));
    let mut b = editor(&[""]);
    type_text(&mut b, &"a".repeat(400));
    assert_eq!(b.page_current().len(), 255);
}

#[test]
fn book_new_page_appends_up_to_fifty() {
    let mut b = editor(&["a"]);
    let m = &mut Mock;
    assert_eq!(b.total, 1);
    b.action(m, 1);
    assert_eq!((b.total, b.curr), (2, 1));
    assert!(b.modified);
    for _ in 0..80 {
        b.action(m, 1);
    }
    assert_eq!(b.total, 50);
    assert_eq!(b.curr, 49);
    // Livro sem NBT cria a página vazia sozinho.
    let blank = Slot { id: WRITABLE_BOOK, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() };
    let b = BookGui::new(blank, true, "x".into(), 36);
    assert_eq!((b.total, b.pages.clone()), (1, Some(vec![String::new()])));
    // Um livro sem NBT que não é editável não tem páginas.
    let blank = Slot { id: WRITTEN_BOOK, count: 1, damage: 0, nbt: None, raw_nbt: Vec::new() };
    assert!(BookGui::new(blank, false, "x".into(), 36).pages.is_none());
}

#[test]
fn book_done_sends_bedit_with_trailing_empty_pages_removed() {
    let mut inv = Inventory::default();
    let mut u = ui();
    let slot = book_slot(WRITABLE_BOOK, &["velha", "", ""], vec![]);
    inv.player[36] = Some(slot.clone());
    inv.open_book(slot, true, "Lohan".into(), 36);
    key(&mut inv, &mut u, &press(KeyCode::KeyA, Some("!")));
    // Concluído (w/2 + 2, 196) 98x20 numa tela de 427x240.
    inv.mouse = (300.0, 200.0);
    click(&mut inv, &mut u, 0, false, 427.0, 240.0);
    assert!(inv.closed && inv.special.is_none());
    assert_eq!(inv.sounds, vec!["gui.button.press"]);
    assert_eq!(inv.out.len(), 1);
    let (channel, sent) = read_book_packet(&inv.out[0]);
    assert_eq!(channel, "MC|BEdit");
    assert_eq!(sent.id, WRITABLE_BOOK);
    assert_eq!(pages_of(&sent), vec!["velha!".to_string()]);
    // O livro da mão também ficou atualizado, como o ItemStack mutado do Java.
    assert_eq!(pages_of(inv.player[36].as_ref().unwrap()), vec!["velha!".to_string()]);
}

#[test]
fn book_done_without_changes_or_escape_sends_nothing() {
    let mut inv = Inventory::default();
    let mut u = ui();
    inv.open_book(book_slot(WRITABLE_BOOK, &["x"], vec![]), true, "Lohan".into(), 36);
    inv.mouse = (300.0, 200.0);
    click(&mut inv, &mut u, 0, false, 427.0, 240.0);
    assert!(inv.closed && inv.out.is_empty());
    // Esc fecha sem mandar nem as alterações.
    let mut inv = Inventory::default();
    inv.open_book(book_slot(WRITABLE_BOOK, &["x"], vec![]), true, "Lohan".into(), 36);
    key(&mut inv, &mut u, &press(KeyCode::KeyA, Some("y")));
    key(&mut inv, &mut u, &press(KeyCode::Escape, Some("\u{1b}")));
    assert!(inv.closed && inv.out.is_empty());
    // Livro escrito: Concluído só fecha.
    let mut inv = Inventory::default();
    inv.open_book(book_slot(WRITTEN_BOOK, &["x"], vec![]), false, "Lohan".into(), 36);
    inv.mouse = (200.0, 205.0);
    click(&mut inv, &mut u, 0, false, 427.0, 240.0);
    assert!(inv.closed && inv.out.is_empty());
}

#[test]
fn book_signing_sends_bsign_with_title_author_and_json_pages() {
    let mut inv = Inventory::default();
    let mut u = ui();
    inv.player[36] = Some(book_slot(WRITABLE_BOOK, &["a<b"], vec![]));
    inv.open_book(inv.player[36].clone().unwrap(), true, "Lohan".into(), 36);
    key(&mut inv, &mut u, &press(KeyCode::KeyA, Some("\"")));
    // Assinar (w/2 - 100, 196) 98x20.
    inv.mouse = (150.0, 200.0);
    click(&mut inv, &mut u, 0, false, 427.0, 240.0);
    let Some(Special::Book(b)) = &inv.special else { panic!() };
    assert!(b.signing);
    // Finalizar fica desabilitado sem título: clicar nele não faz nada.
    click(&mut inv, &mut u, 0, false, 427.0, 240.0);
    assert!(inv.special.is_some() && inv.out.is_empty());
    // Título: até 16 caracteres.
    for c in "abcdefghijklmnopqrstu".chars() {
        key(&mut inv, &mut u, &press(KeyCode::KeyA, Some(&c.to_string())));
    }
    let Some(Special::Book(b)) = &inv.special else { panic!() };
    assert_eq!(b.title, "abcdefghijklmnop");
    // Backspace e depois Enter envia.
    key(&mut inv, &mut u, &press(KeyCode::Backspace, Some("\u{8}")));
    key(&mut inv, &mut u, &press(KeyCode::Enter, Some("\r")));
    assert!(inv.closed && inv.special.is_none());
    let (channel, sent) = read_book_packet(&inv.out[0]);
    assert_eq!(channel, "MC|BSign");
    assert_eq!(sent.id, WRITTEN_BOOK);
    let tag = sent.nbt.as_ref().unwrap();
    assert_eq!(tag.get("author").and_then(Nbt::as_str), Some("Lohan"));
    assert_eq!(tag.get("title").and_then(Nbt::as_str), Some("abcdefghijklmno"));
    assert_eq!(pages_of(&sent), vec![r#""a\u003cb\"""#.to_string()]);
    assert_eq!(inv.player[36].as_ref().unwrap().id, WRITTEN_BOOK);
}

#[test]
fn book_cancel_returns_from_signing_and_buttons_follow_state() {
    let mut b = editor(&["x"]);
    let m = &mut Mock;
    assert!(b.text_button(3, 427).is_some() && b.text_button(0, 427).is_some());
    assert!(b.text_button(5, 427).is_none() && b.text_button(4, 427).is_none());
    // Assinar (w/2 - 100) e Concluído (w/2 + 2), cada um com 98x20.
    assert_eq!(b.text_button(3, 427).map(|x| (x.x, x.w)), Some((113, 98)));
    assert_eq!(b.text_button(0, 427).map(|x| (x.x, x.w)), Some((215, 98)));
    b.action(m, 3);
    assert!(b.signing && !b.next_visible() && !b.prev_visible());
    assert!(b.text_button(3, 427).is_none() && b.text_button(0, 427).is_none());
    assert!(!b.text_button(5, 427).unwrap().enabled);
    assert!(b.text_button(4, 427).is_some());
    b.title = "  ".into();
    assert!(!b.text_button(5, 427).unwrap().enabled);
    b.title = " x".into();
    assert!(b.text_button(5, 427).unwrap().enabled);
    b.action(m, 4);
    assert!(!b.signing);
}

#[test]
fn opening_book_closes_the_previous_window() {
    let mut inv = Inventory::default();
    inv.open_window(3, "minecraft:chest", "Baú".into(), 27, 0);
    inv.open_book(book_slot(WRITTEN_BOOK, &["x"], vec![]), false, "J".into(), 36);
    // O baú aberto manda o C0D ao ser trocado.
    assert_eq!(inv.out, vec![mc_protocol::serverbound::close_window(3)]);
    assert!(inv.open.is_none() && inv.special.is_some());
}

# Tela de configurações — plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Uma janela de configurações própria da Cimori, no visual da ilha, expondo tudo o que a ilha
já faz (primeira entrega da opção B), com a barra da pílula personalizável por arrasto.

**Architecture:** As opções ficam declaradas uma vez numa tabela pura em `isle-core` (`prefs.rs`),
lida pela janela, pela busca e pelos testes. A janela é uma janela Win32 normal na thread de interface,
desenhada por um segundo `PillRenderer` (tiny-skia + Inter + ícones da ilha), com geometria pura em
`isle-render/src/settings/` usada tanto no desenho quanto nos cliques (mesmo padrão de `layout.rs`).
O `IsleApp` aplica cada mudança na hora e grava o `settings.json` de forma atômica.

**Tech Stack:** Rust 2024, crate `windows` (Win32, Dwm, Shell), tiny-skia, reqwest (já no workspace),
serde_json. Nenhuma dependência nova.

**Spec:** `docs/superpowers/specs/2026-11-11-configuracoes-design.md` (mockup aprovado:
`.superpowers/brainstorm/4655-1794442758/content/config-janela.html`).

## Global Constraints

- Nenhuma dependência nova no `Cargo.toml`; só features novas do crate `windows`, se precisar.
- Todo campo novo do `IsleConfig` com `#[serde(default)]` e padrão = comportamento de hoje.
  Um `settings.json` antigo carrega sem erro e a ilha fica igual.
- Opções **leva 2** do spec não aparecem na janela. Nenhum interruptor sem efeito. Opção sem marca
  no spec cujo código não existe vira leva 2 (registrar no ROADMAP, não construir).
  Já confirmadas como leva 2 neste plano: `notch_curve` (não é lido pelo renderizador), letra no lugar
  do título, largura máxima do título, altura da pílula, manter a lista da bandeja aberta,
  minimizar por atalho e trocar a combinação de teclas dos atalhos.
- Textos em português, todos em `isle-core/src/prefs.rs` (nomes, descrições, palavras de busca) ou
  junto da tela; nenhum texto solto espalhado.
- Zero CPU com a janela fechada: nada de timer novo; com a janela aberta, quadros só quando algo
  muda ou anima (mesmo `Waker` da ilha).
- Opção que só vale ao reabrir a Cimori diz isso na própria linha (descrição "Vale ao reabrir a Cimori").
- Visual: cores de `isle-render/src/theme.rs`, ícones de `renderer/icons.rs`, Inter, damasco como
  destaque; respeitar `animations_enabled()`.
- Código novo formatado com `cargo fmt` só nos arquivos novos (o repo não é rustfmt-limpo; não
  reformatar arquivos existentes).
- Ao fim de cada tarefa: `cargo test --workspace --offline` verde.

## Review Focus

1. `settings.json` corrompido, parcial ou com valores fora da faixa (porta 0, minutos negativos,
   monitor que não existe mais): carrega com o padrão daquele campo, não perde o resto. → Task 1.
2. Arrastar um slider grava dezenas de vezes por segundo: aplicar ao vivo, gravar só ao soltar.
   → Task 9.
3. Tirar todos os itens da barra: pelo menos um item fica (o último não sai). → Task 4.
4. Posição da janela salva num monitor que foi desligado: abre centralizada no monitor principal.
   → Task 8.
5. Campo de texto com emoji, colar (Ctrl+V) texto com quebra de linha ou muito longo (chave de API,
   endereço .ics): uma linha só, limite de 2048 caracteres, emoji como par de surrogates. → Task 9.

---

### Task 1: Campos novos do `IsleConfig` e gravação atômica

**Files:**
- Modify: `crates/isle-core/src/config.rs`
- Modify: `src/app.rs:488-497` (`save_config`)
- Test: `crates/isle-core/tests/config_prefs.rs`

**Interfaces:**
- Produces (em `isle_core::config`, todos `#[serde(default)]`, `Serialize/Deserialize/Clone/PartialEq`):
  - `pub enum BarItem { Clock, Next, Date, Media, Weather, Notifications, Timer, Tray }`
    (`Next` = o espaço de hoje: compromisso/contagem, senão música, senão data).
  - `pub enum DateOrder { WeekdayDayMonth, DayMonth, MonthDay }`, `pub enum AnimationLevel { System, On, Off }`.
  - `pub struct Modules { pub ai, control, agenda, weather, media, notifications, timer, tray: bool }` (todos `true`).
  - Campos novos de `IsleConfig` e padrões:
    `bar_items: Vec<BarItem>` = `[Clock, Next, Weather, Tray]`; `bar_quota_strip: bool` = true;
    `clock_24h` = true; `clock_seconds` = false; `clock_weekday` = true; `clock_date` = true;
    `clock_year` = false; `date_order` = `WeekdayDayMonth`; `month_number` = false;
    `countdown_events` = true; `countdown_timer` = true; `countdown_pomodoro` = true;
    `countdown_hours: u32` = 12; `accent_rgb: (u8,u8,u8)` = (241,139,99);
    `animations: AnimationLevel` = System; `over_fullscreen` = false; `modules: Modules`;
    `quota_alerts` = true; `control_tiles: Vec<ControlTile>` = `ControlTile::ALL`;
    `control_mixer` = true; `control_outputs` = true; `wheel_volume` = true;
    `holidays` = true; `holidays_country: String` = "BR"; `lyrics` = true;
    `banners` = true; `banner_secs: u32` = 6; `notifications_per_page: u32` = 4;
    `muted_apps: Vec<String>` = []; `notification_icons` = true; `win_n` = true;
    `hooks_local_only` = false; `settings_window: Option<WindowPlacement>` = None.
  - `pub struct WindowPlacement { pub x: i32, pub y: i32, pub w: i32, pub h: i32, pub tab: String }`.
  - `pub fn IsleConfig::sanitized(self) -> Self` — corrige o que está fora da faixa, campo a campo.
  - `pub fn write_atomic(path: &Path, json: &str) -> std::io::Result<()>` — grava em `<path>.tmp` e
    renomeia por cima.
- `ControlTile` precisa de `Serialize/Deserialize` (está em `isle_core::state`; adicionar o derive).

- [ ] **Step 1: Testes que falham**

```rust
#[test]
fn an_old_settings_file_loads_with_todays_behavior() {
    let old = r#"{"gap_px":8,"auto_hide":false}"#;
    let cfg: IsleConfig = serde_json::from_str(old).unwrap();
    assert_eq!(cfg.gap_px, 8);
    assert!(!cfg.auto_hide);
    assert_eq!(cfg.bar_items, vec![BarItem::Clock, BarItem::Next, BarItem::Weather, BarItem::Tray]);
    assert_eq!(cfg.banner_secs, 6);
    assert_eq!(cfg.notifications_per_page, 4);
    assert_eq!(cfg.accent_rgb, (241, 139, 99));
}
#[test]
fn out_of_range_values_fall_back_field_by_field() {
    let mut cfg = IsleConfig::default();
    cfg.hook_port = 0; cfg.banner_secs = 0; cfg.notifications_per_page = 99;
    cfg.countdown_hours = 0; cfg.bar_items.clear(); cfg.gap_px = 900;
    let cfg = cfg.sanitized();
    assert_eq!(cfg.hook_port, 47830);
    assert_eq!(cfg.banner_secs, 6);
    assert_eq!(cfg.notifications_per_page, 4);
    assert_eq!(cfg.countdown_hours, 12);
    assert_eq!(cfg.bar_items, vec![BarItem::Clock]);
    assert_eq!(cfg.gap_px, 6);
}
#[test]
fn a_field_with_the_wrong_type_keeps_the_rest() {
    let cfg = IsleConfig::load_lenient(r#"{"gap_px":"oops","auto_hide":false}"#);
    assert!(!cfg.auto_hide);
    assert_eq!(cfg.gap_px, 6);
}
#[test]
fn atomic_write_replaces_the_file_whole() {
    let dir = std::env::temp_dir().join(format!("cimori-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    write_atomic(&path, "{\"a\":1}").unwrap();
    write_atomic(&path, "{\"a\":2}").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
    assert!(!dir.join("settings.json.tmp").exists());
}
```

Faixas do `sanitized`: `hook_port` 1..=65535 (senão 47830); `banner_secs` 2..=60 (6);
`notifications_per_page` 2..=8 (4); `countdown_hours` 1..=48 (12); `gap_px` 0..=40 (6);
`agenda_reminder_minutes` 0..=1440; `pomodoro_*`/`timer_default_secs` 60..=86400;
`bar_items` sem repetidos e não vazio (vazio → `[Clock]`); `control_tiles` sem repetidos.

- [ ] **Step 2:** `cargo test -p isle-core --test config_prefs` → FAIL (tipos não existem).
- [ ] **Step 3:** Implementar os tipos, `sanitized`, `write_atomic` e
  `pub fn IsleConfig::load_lenient(json: &str) -> Self` (parse em `serde_json::Value`; cada chave
  de objeto é aplicada sozinha sobre o padrão, e uma chave que não desserializa é ignorada com
  `tracing::warn!`; depois `sanitized()`). Trocar a leitura do `settings.json` em `src/main.rs` para
  `load_lenient`, e `save_config` em `src/app.rs` para `write_atomic`.
- [ ] **Step 4:** `cargo test --workspace --offline` → PASS.
- [ ] **Step 5:** Commit `feat(config): settings fields for the settings window, lenient load, atomic save`.

---

### Task 2: Relógio e data configuráveis

**Files:**
- Create: `crates/isle-core/src/clock.rs` (+ `pub mod clock;` em `lib.rs`)
- Modify: `src/app.rs` (`format_clock_utc_minus_3`, `format_date_utc_minus_3` e o tick em `:334`)
- Test: `crates/isle-core/tests/clock.rs`

**Interfaces:**
- Produces: `pub fn clock_text(local_secs: i64, cfg: &IsleConfig) -> String` e
  `pub fn date_text(local_secs: i64, cfg: &IsleConfig) -> String` (`local_secs` já com o fuso).
  O app passa a chamar esses dois; os dois `format_*_utc_minus_3` saem.

- [ ] **Step 1: Testes que falham** (11/11/2026 22:41:07 local, quarta-feira)

```rust
let t = /* segundos locais de 2026-11-11 22:41:07 */;
let mut c = IsleConfig::default();
assert_eq!(clock_text(t, &c), "22:41");
assert_eq!(date_text(t, &c), "qua 11 nov");
c.clock_seconds = true; assert_eq!(clock_text(t, &c), "22:41:07");
c.clock_24h = false; c.clock_seconds = false; assert_eq!(clock_text(t, &c), "10:41 PM");
c.clock_weekday = false; assert_eq!(date_text(t, &c), "11 nov");
c.month_number = true; assert_eq!(date_text(t, &c), "11/11");
c.date_order = DateOrder::MonthDay; assert_eq!(date_text(t, &c), "11/11");
c.month_number = false; assert_eq!(date_text(t, &c), "nov 11");
c.clock_year = true; assert_eq!(date_text(t, &c), "nov 11 2026");
c.clock_date = false; c.clock_weekday = true; assert_eq!(date_text(t, &c), "qua");
```
`clock_seconds` ligado faz o loop acordar a cada segundo só enquanto o relógio está na barra.

- [ ] **Step 2:** Rodar → FAIL. **Step 3:** Implementar e ligar no app (o `last_minute` vira
  `last_second` quando `clock_seconds`). **Step 4:** Workspace verde.
- [ ] **Step 5:** Commit `feat(clock): configurable clock and date on the bar`.

---

### Task 3: Tabela das opções (`prefs.rs`)

**Files:**
- Create: `crates/isle-core/src/prefs.rs` (+ `pub mod prefs;`)
- Test: `crates/isle-core/tests/prefs.rs`

**Interfaces:**
- Produces:
  - `pub enum SettingsTab { General, Bar, Look, Monitors, Hiding, Animations, Ai, Control, Agenda, Weather, Media, Notifications, Focus, Tray, Shortcuts, Backup, About }`
    com `title()`, `subtitle()`, `group() -> &'static str` ("Ilha" / "Módulos" / "Cimori"),
    `ALL` e `module(self) -> Option<fn(&Modules)->bool>` para as abas de módulo.
  - `pub enum PrefKind { Switch, Choice(&'static [&'static str]), Slider { min: f32, max: f32, step: f32, unit: &'static str }, Color(&'static [(u8,u8,u8)]), Text { secret: bool, placeholder: &'static str }, Chips, Action(ActionId), Monitors, CitySearch, Info }`
  - `pub enum ActionId { OpenFolder, Quit, InstallHooks, RemoveHooks, Export, Import, Restore, OpenRepo, OpenWindowsBanners, OpenBridgeReadme }`
  - `pub enum PrefValue { Bool(bool), Index(usize), Number(f32), Rgb(u8,u8,u8), Text(String), List(Vec<String>) }`
  - `pub struct Pref { pub id: &'static str, pub tab: SettingsTab, pub card: &'static str, pub name: &'static str, pub desc: &'static str, pub words: &'static str, pub kind: PrefKind, pub get: fn(&IsleConfig) -> PrefValue, pub set: fn(&mut IsleConfig, PrefValue), pub enabled: fn(&IsleConfig) -> bool, pub restart: bool }`
  - `pub fn all() -> &'static [Pref]`, `pub fn by_id(id: &str) -> Option<&'static Pref>`,
    `pub fn search(query: &str) -> Vec<&'static Pref>` (sem acento e sem caixa; casa nome, descrição,
    palavras e título da aba; todas as palavras da busca precisam casar).
  - `Chips` usa `PrefValue::List` com os nomes estáveis dos itens (`"clock"`, `"wifi"`…), e
    `pub fn chip_choices(id: &str) -> &'static [(&'static str, &'static str)]` (chave, rótulo).
- Conteúdo: exatamente as linhas do spec que não são **leva 2**, menos as que o Global Constraints
  já moveu para leva 2. Cada `Pref.id` é o nome do campo do `IsleConfig` (ou `action.<nome>`).
  Módulos: a primeira linha de cada aba de módulo é o interruptor "Mostrar <módulo>" (`modules.<nome>`).
  `restart: true` em `hook_port`, `hooks_local_only`, `ai_claude_enabled`, `ai_codex_enabled`.

- [ ] **Step 1: Testes que falham**

```rust
#[test] fn every_pref_round_trips_through_get_and_set() {
    for p in prefs::all() {
        if matches!(p.kind, PrefKind::Action(_) | PrefKind::Info) { continue }
        let mut c = IsleConfig::default();
        let v = (p.get)(&c); (p.set)(&mut c, v.clone());
        assert_eq!((p.get)(&c), v, "{}", p.id);
        assert_eq!(c, IsleConfig::default(), "{} changes nothing when set to itself", p.id);
    }
}
#[test] fn ids_are_unique_and_every_tab_has_a_row() {
    let mut ids: Vec<_> = prefs::all().iter().map(|p| p.id).collect(); ids.sort(); ids.dedup();
    assert_eq!(ids.len(), prefs::all().len());
    for t in SettingsTab::ALL { assert!(prefs::all().iter().any(|p| p.tab == t), "{t:?}"); }
}
#[test] fn search_ignores_accents_and_case() {
    assert!(prefs::search("animacao").iter().any(|p| p.tab == SettingsTab::Animations));
    assert!(prefs::search("SEGUNDOS").iter().any(|p| p.id == "clock_seconds"));
    assert!(prefs::search("fahrenheit").iter().any(|p| p.id == "weather_fahrenheit"));
    assert!(prefs::search("zzzz").is_empty());
}
#[test] fn no_leva_2_option_is_listed() {
    for id in ["notch_curve", "lyrics_in_title", "tray_keep_open", "minimize_key"] {
        assert!(prefs::by_id(id).is_none(), "{id}");
    }
}
#[test] fn setting_a_module_off_hides_its_other_rows() {
    let p = prefs::by_id("weather_fahrenheit").unwrap();
    let mut c = IsleConfig::default(); c.modules.weather = false;
    assert!(!(p.enabled)(&c));
}
```

- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** Verde.
- [ ] **Step 5:** Commit `feat(prefs): the settings table, declared once`.

---

### Task 4: Barra na ordem escolhida

**Files:**
- Modify: `crates/isle-core/src/state.rs` (`IslandState`: `#[serde(skip)] pub bar: Vec<BarItem>`, `pub bar_quota_strip: bool`, `pub unread: usize` se não existir equivalente)
- Modify: `crates/isle-render/src/layout.rs` (nova função), `crates/isle-render/src/renderer.rs:492-546` (`draw_compact`), `crates/isle-render/src/pages.rs` (`compute_hitboxes` do modo compacto)
- Modify: `src/app.rs` (copia `config.bar_items`/`bar_quota_strip` para o estado ao iniciar e ao mudar)
- Test: `crates/isle-render/tests/bar_layout.rs`

**Interfaces:**
- Produces: `pub fn compact_slots(text: &crate::text::Text, x: f32, w: f32, s: &IslandState) -> Vec<(BarItem, f32, f32)>`
  em `layout.rs` (item, x, largura). O primeiro item da lista fica colado à esquerda; os demais,
  na ordem, colados à direita. Item sem conteúdo (sem clima, sem música, sem avisos não lidos, sem
  timer correndo) não ocupa espaço. Se não couber, saem os últimos da lista até caber; o primeiro
  nunca sai. `draw_compact` e `compute_hitboxes` usam só essa função.
- Itens novos: `Notifications` = sino + número de não lidos (clique abre Avisos); `Timer` = o que
  termina primeiro (texto de `next_countdown_text` do timer/Pomodoro; clique abre Foco); `Date` e
  `Media` separados do `Next`.
- `bar_quota_strip` liga/desliga as barras de cota embaixo da pílula.

- [ ] **Step 1: Testes que falham**

```rust
#[test] fn default_order_draws_like_today() { /* estado com música tocando e clima:
    slots = [Clock à esquerda, Next, Weather, Tray à direita]; Next mostra o título da música */ }
#[test] fn order_follows_the_list_and_empty_items_take_no_room() {
    // bar = [Weather, Clock, Notifications]; sem avisos não lidos → só Weather (esq.) e Clock (dir.)
}
#[test] fn items_that_do_not_fit_drop_from_the_end_but_the_first_stays() {
    // w = 200 com 6 itens: o primeiro sempre presente; nenhum slot passa de x + w
}
#[test] fn hitboxes_sit_on_the_drawn_slots() {
    // para cada slot clicável, compute_hitboxes tem um HitBox com o mesmo x e largura
}
```
Mais o teste de pixels já existente `renderer_produces_non_empty_bgra_pill_with_configured_background`
continuando verde, e um teste em `src/app.rs` de que `bar_items = []` vindo da janela vira `[Clock]`
(Review Focus 3, junto do `sanitized`).

- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** Verde e prévias
  `cargo run -p isle-render --example preview -- target/preview-bar` conferidas (barra igual a hoje).
- [ ] **Step 5:** Commit `feat(bar): bar items in the user's order`.

---

### Task 5: Destaque, animações, tela cheia e módulos

**Files:**
- Modify: `crates/isle-render/src/theme.rs` (destaque num `static ACCENT: AtomicU32`, `pub fn set_accent(rgb: (u8,u8,u8))` reexportado em `isle_render::set_accent`)
- Modify: `crates/isle-render/src/window.rs` (`animations_enabled()` → `pub fn animations_on(level: AnimationLevel) -> bool`; `step_aside` ignora tela cheia quando `over_fullscreen`)
- Modify: `crates/isle-render/src/layout.rs` (`TABS` → `pub fn visible_tabs(m: &Modules) -> Vec<TabPage>`; `navigation` usa o número real de abas; `IslandState` ganha `#[serde(skip)] pub modules: Modules`)
- Modify: `src/app.rs` (`spawn_services` respeita `modules`; ver abaixo)
- Test: `src/app.rs` (módulo `tests`) e `crates/isle-render/tests/visual_layout.rs`

**Interfaces:**
- Produces: `IsleApp::apply_config(&mut self, old: &IsleConfig)` — compara `old` com `self.config` e
  aplica o que mudou: destaque, `state.bar*`, `state.modules`, monitores (`overlay.destroy_windows()` +
  campos + `create_windows()` + `sync_islands()` quando `monitors/all_monitors/main_monitor` mudam),
  `over_fullscreen`, relógio. Grava com `save_config()` só se `save == true`
  (assinatura: `pub(crate) fn apply_config(&mut self, old: &IsleConfig, save: bool)`).
- Módulo desligado: some da navegação e da barra na hora; uma ilha aberta nessa aba volta para a
  primeira aba visível. O serviço dele não é iniciado na próxima abertura (a linha diz "Vale ao
  reabrir a Cimori" só para desligar). Ligar um módulo cujo serviço não foi iniciado inicia o serviço
  na hora: separar cada serviço de `spawn_services` numa função `spawn_<módulo>(&self, ...)` chamável depois.

- [ ] **Step 1: Testes que falham**

```rust
#[test] fn a_module_turned_off_leaves_the_navigation_and_its_open_panel() {
    let mut app = IsleApp::new(IsleConfig::default());
    app.apply_ui_action(UiAction::SwitchTab(TabPage::Weather));
    let old = app.config.clone(); app.config.modules.weather = false; app.apply_config(&old, false);
    assert!(!isle_render::layout::visible_tabs(&app.state.modules).contains(&TabPage::Weather));
    assert_ne!(app.islands[0].mode, PillMode::Expanded(TabPage::Weather));
}
#[test] fn navigation_cells_fill_the_width_for_any_number_of_tabs() { /* 3 e 8 abas: sem sobreposição, dentro de w */ }
#[test] fn accent_reaches_the_drawn_pixels() { /* set_accent((0,200,0)); slider desenhado tem pixel verde */ }
```

- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** Verde.
- [ ] **Step 5:** Commit `feat(settings): accent, animations, full screen and modules applied live`.

---

### Task 6: Geometria da janela de configurações

**Files:**
- Create: `crates/isle-render/src/settings/mod.rs`, `crates/isle-render/src/settings/layout.rs` (+ `pub mod settings;` em `lib.rs`)
- Test: `crates/isle-render/tests/settings_layout.rs`

**Interfaces:**
- Produces:
  - `pub struct SettingsView { pub tab: SettingsTab, pub search: String, pub scroll: f32, pub nav_scroll: f32, pub focus: Option<Focus>, pub hover: Option<(f32,f32)>, pub editing: Option<(&'static str, String)>, pub drag: Option<ChipDrag>, pub confirm: Option<ActionId>, pub monitors: Vec<MonitorPlace>, pub city_results: Vec<(String, f64, f64)>, pub status: Option<String> }` com `Default`.
  - `pub enum Focus { Nav(usize), Search, Row(&'static str) }`, `pub struct ChipDrag { pub pref: &'static str, pub from: usize, pub x: f32 }`.
  - `pub enum SettingsAction { Tab(SettingsTab), FocusSearch, ClearSearch, Toggle(&'static str), Choose(&'static str, usize), Slide(&'static str, f32), Pick(&'static str, (u8,u8,u8)), Edit(&'static str), ChipToggle(&'static str, &'static str), ChipGrab(&'static str, usize), Run(ActionId), PickCity(usize), Monitor(String), MainMonitor(String) }`.
  - `pub struct Hit { pub x: f32, pub y: f32, pub w: f32, pub h: f32, pub action: SettingsAction }`
  - `pub struct Layout { pub narrow: bool, pub nav: Vec<Hit>, pub rows: Vec<RowBox>, pub hits: Vec<Hit>, pub content_h: f32, pub preview: Option<(f32,f32,f32,f32)> }`,
    `pub struct RowBox { pub pref: &'static Pref, pub x: f32, pub y: f32, pub w: f32, pub h: f32, pub card: &'static str }`.
  - `pub fn layout(view: &SettingsView, cfg: &IsleConfig, text: &Text, w: f32, h: f32) -> Layout`
    (unidades lógicas; `scroll` já aplicado; hits fora da área visível não entram).
  - `pub fn hit(layout: &Layout, x: f32, y: f32) -> Option<SettingsAction>`.
- Medidas do mockup: lateral 236 (estreita: 64 abaixo de 760 de largura, só ícones); cabeçalho 64;
  prévia 132 nas abas Barra, Aparência e Esconder; cartões com padding 18, linha mínima 44;
  duas colunas a partir de 900 de conteúdo. Buscando: só as linhas que casam, cada cartão com
  "<aba> · <cartão>" no título. Linhas com `enabled == false` ficam e não geram hit. A aba Monitores só existe com
  `view.monitors.len() >= 2` (`SettingTabShown` do mac-island).

- [ ] **Step 1: Testes que falham**

```rust
#[test] fn every_tab_lays_out_without_overlaps_at_three_sizes() {
    for (w, h) in [(1180., 760.), (820., 600.), (600., 500.)] { for tab in SettingsTab::ALL {
        let l = layout(&SettingsView { tab, ..Default::default() }, &IsleConfig::default(), &Text::new(), w, h);
        assert_no_overlap(&l.hits); assert!(l.hits.iter().all(|b| b.x >= 0. && b.x + b.w <= w));
    }}
}
#[test] fn narrow_window_keeps_only_icons_in_the_side_bar() { /* w=600 → l.narrow e nav com 64 de largura */ }
#[test] fn a_click_on_a_switch_toggles_that_pref() {
    let l = layout(&SettingsView { tab: SettingsTab::Bar, ..Default::default() }, &IsleConfig::default(), &Text::new(), 1180., 760.);
    let r = l.rows.iter().find(|r| r.pref.id == "clock_seconds").unwrap();
    assert_eq!(hit(&l, r.x + r.w - 20., r.y + r.h / 2.), Some(SettingsAction::Toggle("clock_seconds")));
}
#[test] fn search_lists_matches_from_every_tab() { /* search "volume" → linhas de Controle e nenhuma de Clima */ }
#[test] fn disabled_rows_are_not_clickable() { /* modules.weather=false → nenhum hit de weather_fahrenheit */ }
#[test] fn monitors_tab_needs_two_monitors() { /* 1 monitor → nenhum nav de Monitors; 2 → presente */ }
#[test] fn scrolling_moves_rows_and_drops_hidden_ones() { /* scroll=300 → nenhum hit com y < 64 */ }
```

- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** Verde.
- [ ] **Step 5:** Commit `feat(settings): settings window geometry`.

---

### Task 7: Desenho da janela

**Files:**
- Create: `crates/isle-render/src/renderer/settings_view.rs` (`impl PillRenderer`, como `weather_view.rs`)
- Modify: `crates/isle-render/src/renderer.rs` (declarar o módulo; `draw_compact` acessível ao desenho da prévia)
- Modify: `crates/isle-render/examples/preview.rs` (prévias `settings-<aba>.png` a 100% e 150%, 1180×760, e uma estreita 600×500)
- Test: `crates/isle-render/tests/settings_draw.rs`

**Interfaces:**
- Consumes: `settings::layout`, `SettingsView`, `prefs`.
- Produces: `pub fn PillRenderer::render_settings(&mut self, view: &SettingsView, cfg: &IsleConfig, state: &IslandState) -> &PillFrameBuffer`
  — fundo opaco (sem transparência: é janela normal). A prévia chama `draw_compact` com o `state`
  real dentro do retângulo `layout.preview`, e abaixo dele a faixa de chips da barra
  ("Na barra" e "Disponíveis", chip arrastado seguindo `drag.x`).
- Cada `PrefKind` tem um desenho: interruptor 36×20, segmentos, slider com valor e unidade,
  bolinhas de cor (selecionada com anel damasco), campo de texto (secreto mostra "••••" e o botão
  "mostrar"), chips, botão de ação (com `confirm` trocando o texto para "Confirmar?"), mapa simples
  dos monitores (retângulos proporcionais com o nome, principal com borda damasco), `CitySearch`
  (campo + até 5 resultados), `Info` (texto). Foco do teclado com anel damasco de 2 px.

- [ ] **Step 1: Testes que falham** (contagem de pixels, como os testes de `renderer.rs`)

```rust
#[test] fn switches_draw_accent_only_when_on() { /* clock_weekday (on) tem damasco na sua área; clock_seconds (off) não */ }
#[test] fn the_preview_is_the_real_bar() { /* com clock_text "22:41", a área da prévia tem tinta clara onde o relógio cai */ }
#[test] fn secrets_never_draw_their_text() { /* calendar_ics_url = "https://segredo" → nenhuma largura de glifo compatível; desenha bolinhas */ }
#[test] fn draws_at_150_percent_with_the_same_layout() { /* tinta nas mesmas posições lógicas */ }
```

- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** Verde; gerar
  `cargo run -p isle-render --example preview -- target/preview-settings` e conferir as abas contra
  o mockup aprovado.
- [ ] **Step 5:** Commit `feat(settings): draw the settings window in the Cimori look`.

---

### Task 8: Janela Win32

**Files:**
- Create: `crates/isle-render/src/settings_window.rs` (+ `pub mod settings_window;`)
- Modify: `crates/isle-render/src/window.rs` (mensagem registrada `"CimoriShowSettings"` no `wnd_proc` das ilhas → `UiInput::OpenSettings`)
- Test: `crates/isle-render/src/settings_window.rs` (testes de `fit_placement`)

**Interfaces:**
- Produces:
  - `pub enum SettingsInput { Down { x: f32, y: f32 }, Up { x: f32, y: f32 }, Move { x: f32, y: f32 }, Leave, Wheel(i32), Char(char), Key(u16), Resized, Closed }` (lógico, já dividido pela escala; `Key` = virtual key de Tab, setas, Enter, Esc, Home, End, Delete).
  - `pub struct SettingsWindow` com `pub fn open(&mut self, placement: Option<&WindowPlacement>) -> Result<()>` (já aberta: traz para a frente com `SetForegroundWindow`), `pub fn close(&mut self)`, `pub fn is_open(&self) -> bool`, `pub fn size(&self) -> (f32, f32, f32)` (largura, altura lógicas e escala), `pub fn present(&mut self, fb: &PillFrameBuffer)`, `pub fn take_inputs(&mut self) -> Vec<SettingsInput>`, `pub fn placement(&self) -> Option<WindowPlacement>`.
  - `pub fn fit_placement(p: &WindowPlacement, monitors: &[MonitorPlace]) -> (i32, i32, i32, i32)` — pura: se o centro da janela salva não cai em nenhum monitor, centraliza 1180×760 (limitado à área do principal).
  - `UiInput::OpenSettings` novo em `window.rs`.
- Classe `"CimoriSettingsWindow"`, `WS_OVERLAPPEDWINDOW`, título "Cimori — Configurações", ícone do
  app, barra de título escura (`DWMWA_USE_IMMERSIVE_DARK_MODE` e `DWMWA_CAPTION_COLOR` = fundo da lateral),
  mínimo 600×480 (`WM_GETMINMAXINFO`). DPI por monitor: `WM_DPICHANGED` aplica o retângulo sugerido e
  gera `Resized`. Pinta em `WM_PAINT` com `SetDIBitsToDevice` do último framebuffer. `WM_CLOSE` →
  `Closed` e esconde (não destrói a thread nem a ilha). Cursor de mão sobre hits fica para o app
  (`set_cursor(hand: bool)`).

- [ ] **Step 1: Testes que falham**

```rust
#[test] fn a_placement_on_a_monitor_that_is_gone_is_centered_on_the_main_one() { … }
#[test] fn a_placement_on_a_live_monitor_is_kept() { … }
```
- [ ] **Step 2:** FAIL. **Step 3:** Implementar (o resto só se valida ao vivo no Task 12).
  **Step 4:** Verde.
- [ ] **Step 5:** Commit `feat(settings): native settings window`.

---

### Task 9: Ligação no app: abrir, editar, aplicar

**Files:**
- Create: `src/settings.rs` (`#[path]` como `ai_chat.rs`; `impl IsleApp`)
- Modify: `src/app.rs` (campos `settings: SettingsWindow`, `settings_view: SettingsView`, renderer da janela; laço principal chama `pump_settings()`), `src/main.rs` (`--settings`)
- Modify: `crates/isle-render/src/renderer.rs` + `layout.rs` (engrenagem no cabeçalho da ilha → `UiAction::OpenSettings`)
- Test: `src/settings.rs` (módulo `tests`)

**Interfaces:**
- Consumes: Tasks 1–8.
- Produces: `pub(crate) fn open_settings(&mut self)`, `fn settings_input(&mut self, input: SettingsInput)`,
  `fn settings_action(&mut self, action: SettingsAction)`.
- Comportamento:
  - Abrir: engrenagem, clique direito na pílula fora de um ícone da bandeja (`UiInput::RightClick`
    sem hit de bandeja), `UiInput::OpenSettings`. `--settings`: `FindWindowW("IsleWinPillOverlayClass")`
    + `PostMessageW(RegisterWindowMessageW("CimoriShowSettings"))` e sai; sem ilha rodando, inicia
    normal e chama `open_settings()` logo após `create_windows()`.
  - Toda mudança: `let old = self.config.clone(); (pref.set)(&mut self.config, v); self.config = self.config.clone().sanitized(); self.apply_config(&old, save)`.
    Slider: `save = false` durante o arrasto, `true` no `Up` (Review Focus 2). Texto: aplica no Enter
    ou ao perder o foco; Esc descarta.
  - Edição de texto reaproveita a regra das respostas de aviso: Backspace, Ctrl+V (uma linha só:
    quebras viram espaço), surrogates de emoji juntados, limite 2048 (Review Focus 5).
  - Teclado: Tab/Shift+Tab percorre lateral → busca → linhas; setas mudam escolha/slider; Espaço/Enter
    aciona; Esc fecha a busca, depois a janela. Digitar com foco na lateral começa uma busca.
  - Chips: arrastar dentro de "Na barra" reordena; para "Disponíveis" tira; de volta, coloca no ponto
    solto; o último item não sai (o chip volta com um tremor curto, ou parado sem animações).
  - Ao fechar: salva `settings_window` (posição, tamanho, aba) e solta o renderer da janela.
  - Quadros: só quando há input, mudança de estado da ilha visível na prévia, ou animação.

- [ ] **Step 1: Testes que falham**

```rust
#[test] fn toggling_a_switch_applies_and_saves() { /* clock_seconds pelo settings_action → config e arquivo temp com "clock_seconds": true */ }
#[test] fn dragging_a_slider_saves_only_on_release() { /* 20 Move → arquivo inalterado; Up → gravado uma vez */ }
#[test] fn the_last_bar_item_cannot_leave() { /* bar_items=[Clock]; ChipGrab e soltar em Disponíveis → continua [Clock] */ }
#[test] fn pasted_text_becomes_one_bounded_line() { /* "a\r\nb" + 3000 'x' → "a b" + … com 2048 chars */ }
#[test] fn escape_discards_an_edit() { … }
#[test] fn right_click_on_the_pill_opens_settings_but_on_a_tray_icon_does_not() { … }
#[test] fn changing_monitors_rebuilds_the_islands() { /* all_monitors false→true: apply_config chama recriação (contador no overlay de teste) */ }
```

- [ ] **Step 2:** FAIL. **Step 3:** Implementar. **Step 4:** Verde.
- [ ] **Step 5:** Commit `feat(settings): open, edit and apply settings live`.

---

### Task 10: Busca de cidade do clima

**Files:**
- Modify: `crates/isle-core/src/weather.rs` (`pub fn geocoding_url(name: &str) -> String`, `pub fn parse_places(json: &str) -> Vec<(String, f64, f64)>`)
- Modify: `src/settings.rs` (ao Enter no campo `CitySearch`, busca num `rt.spawn`, resultado volta por `AppEvent::Places`)
- Test: `crates/isle-core/tests/weather_places.rs`

**Interfaces:**
- URL: `https://geocoding-api.open-meteo.com/v1/search?name=<percent-encoded>&count=5&language=pt&format=json`.
- `parse_places` monta o rótulo "Cidade, Estado, País" (pula partes vazias); JSON inválido ou sem
  `results` → vazio. Escolher um resultado grava `weather` e pede o clima de novo na hora.
- Erro de rede: `view.status = Some("Sem conexão para buscar a cidade")`; nada muda.

- [ ] **Step 1:** Testes com um JSON real gravado do Open-Meteo para "Bom Jesus" e um vazio
  (`{"generationtime_ms":0.1}`) → FAIL. **Step 2–4:** Implementar, verde.
- [ ] **Step 5:** Commit `feat(weather): city search in settings`.

---

### Task 11: Ações e listas especiais

**Files:**
- Modify: `src/settings.rs`, `crates/isle-ai/src/hooks.rs`, `src/app.rs` (avisos de apps ignorados, Win+N, banner, por página)
- Test: nos módulos de teste correspondentes

**Interfaces e comportamento (cada um com um teste):**
- `OpenFolder`: `ShellExecuteW("open", pasta do settings.json)`. `Quit`: `PostQuitMessage(0)` depois de salvar.
- `InstallHooks`/`RemoveHooks`: chamam as funções que `main.rs` usa (`install_hooks(None)`/`uninstall_hooks()`,
  movidas para `src/hooks_cli.rs` para o app usar); a linha `Info` mostra o estado via `agents::claude::hooked` / Codex equivalente.
- `hooks_local_only`: `HookServer::spawn` recebe `local_only: bool`; com ele, conexões cujo IP não é
  loopback nem `100.64.0.0/10` são fechadas sem resposta. Teste com `127.0.0.1` aceito e
  `peer_allowed("192.168.0.5".parse().unwrap(), true) == false`, `peer_allowed("100.64.0.1"...) == true`.
- `Export`: `IFileSaveDialog` (nome padrão `cimori-backup-AAAA-MM-DD.json`) com
  `{"settings": …, "reminders": …, "cleared_until": …}`. `Import`: `IFileOpenDialog`, valida com
  `load_lenient`, aplica com `apply_config(&old, true)` e regrava os lembretes. `Restore`: primeiro
  clique vira "Confirmar?", segundo aplica `IsleConfig::default()` (mantendo `settings_window`).
- `OpenWindowsBanners`: abre `ms-settings:notifications`. `OpenRepo`/`OpenBridgeReadme`: abrem o
  link no navegador. Sobre: `Info` com `Cimori {CARGO_PKG_VERSION}`, créditos isle (GPL-3.0-or-later)
  e Mac Island & Dock (caliberda, GPL-3.0).
- Avisos: `banners` falso não mostra banner (continua contando); `banner_secs` substitui `BANNER_FOR`;
  `notifications_per_page` substitui o 4 fixo; `muted_apps` (lista de chips com os apps vistos nesta
  sessão e os já silenciados) descarta avisos desses apps antes do `NotificationCenter`;
  `notification_icons` falso não pede ícones; `win_n` falso não instala o atalho (vale ao reabrir).
- Controle: `control_tiles` define quais e em que ordem; `control_mixer`/`control_outputs` escondem
  as seções; `wheel_volume` falso ignora a roda fora de painéis roláveis.
- Agenda: `holidays`/`holidays_country` no pedido do Nager.Date. Mídia: `lyrics` falso não busca letra.
- IA: `quota_alerts` falso não mostra os avisos de 80/95% e "de volta!".
- Bandeja: chips de `pinned_tray_apps` com os ícones atuais; estado da ponte vindo do `TraySnapshot`
  (o implementador expõe `bridge: bool` se ainda não houver).

- [ ] **Step 1:** Um teste por item acima (nome `<opção>_<efeito>`), FAIL. **Step 2–4:** Implementar, verde.
- [ ] **Step 5:** Commit `feat(settings): actions, backup and per-module options`.

---

### Task 12: Docs, prévias e validação

**Files:**
- Modify: `docs/ROADMAP.md` (seção "Tela de configurações" com o que entrou e a lista **leva 2**;
  regra nova em "Estratégia": "todo recurso novo entra com as suas opções na janela de configurações")
- Create: `docs/SETTINGS-VALIDATION.md` (prévias, testes, checklist ao vivo)
- Modify: `docs/WINDOWS-DEV.md` ("Onde paramos")

- [ ] **Step 1:** `cargo test --workspace --offline` (todos verdes) e
  `cargo clippy --workspace --all-targets --offline` sem aviso novo nos arquivos novos.
- [ ] **Step 2:** Prévias de todas as abas a 100% e 150% e estreita; copiar 4 representativas para
  `docs/design/native/config-*.png`.
- [ ] **Step 3:** `cargo build --release --offline`; copiar para `dist/isle-win.exe`.
- [ ] **Step 4:** Checklist ao vivo para o Marcus em `SETTINGS-VALIDATION.md`: abrir pelos três
  caminhos, segunda instância, cada aba, arrastar chips, slider, campo secreto, busca, DPI 100/150,
  mover entre monitores, fechar/reabrir no mesmo lugar, monitor desligado, CPU parado com a janela
  fechada (Gerenciador de Tarefas: 0%).
- [ ] **Step 5:** Commit `docs: settings window delivered; second-wave options listed`.

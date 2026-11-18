# Fase 9 — Finalização: plano em duas trilhas (Claude e Codex)

Data: 12/11/2026. Base: `feat/settings` @ `e47f06e` (build validado pelo Marcus).
Spec: [ROADMAP.md](../../ROADMAP.md), seções "Fase 8 — Falta portar", "Ajustes de layout e pílula" e
"Pendências conhecidas". O roadmap é a autoridade; este plano divide o trabalho.

## Regras globais (valem para as duas trilhas)

- **Portar, não inventar.** Antes de escrever, achar o equivalente nas fontes e portar:
  - isle @ a29a7c4: `.superpowers/upstream/isle/` (na raiz do checkout principal `D:\isle-win`).
  - Mac Island & Dock 3.1.1: `.superpowers/upstream/mac-island-dock-3.1.1.wh.cpp` (~121k linhas; usar grep).
  No topo de cada função portada, um comentário dizendo de onde veio (`// Port of Island::X (mac-island 3.1.1)`).
- **Toda funcionalidade nova é configurável**: uma linha em `crates/isle-core/src/prefs.rs` + campo em
  `crates/isle-core/src/config.rs` com padrão igual ao comportamento de hoje; configs antigas continuam
  carregando (`#[serde(default)]` já cobre).
- Textos da interface em português do Brasil.
- Cada tarefa termina com `cargo test --workspace` verde e `cargo build --release` compilando, e com
  commit(s) próprios na branch da trilha. Não rodar `cargo fmt` no workspace inteiro (há um commit só de
  formatação planejado para o fim).
- Prévias visuais: `cargo run -p isle-render --example preview -- <pasta>`; mudanças visuais conferidas
  nelas em 100% e 150%.
- Commits terminam com: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` (trilha Claude) ou sem
  linha extra (trilha Codex).

## Donos dos arquivos (para não atropelar)

| Trilha | Branch / worktree | É dona de |
|---|---|---|
| Codex | `feat/fin-codex` em `.worktrees/fin-codex` | `src/phase8.rs`, `crates/isle-platform-win/src/{clipboard,downloads,shelf_share*}.rs` e `shelf_share/`, `crates/isle-render/src/phase8_view.rs`, `crates/isle-core/src/phase8.rs`, `windhawk/`, `crates/isle-ai/` (servidor dos agentes) |
| Claude | `feat/fin-claude` em `.worktrees/fin-claude` | todo o resto: `isle-render` (exceto `phase8_view.rs`), `src/app.rs`, `src/settings.rs`, `isle-platform-win` (exceto os da Codex), `isle-core` (exceto `phase8.rs`) |

Compartilhados, só **acrescentando** linhas (nunca reordenando ou reformatando o que existe):
`crates/isle-core/src/prefs.rs`, `crates/isle-core/src/config.rs`, `docs/`. Se a Codex precisar tocar no
desenho da pílula para o preenchimento de downloads, mexe só na função que já desenha esse preenchimento
em `crates/isle-render/src/renderer.rs`. Se a Codex precisar de uma mudança num arquivo da trilha Claude,
deixa anotado no relatório em vez de editar.

---

## Trilha Codex

### C1 — Downloads pela barra de tarefas, com animação e aba detalhada
Roadmap: Fase 8 "Falta portar" (downloads) + Ajustes item 9.
- Portar do mac-island o progresso dos botões da barra (`ITaskbarList3`): ver o comentário em torno da
  linha 12936 (`ProgressBarWatch`, `g_progressBars`, `CTaskBand::_OnSetProgressValue/_OnSetProgressState`
  hooks, `PublishProgress`, `kProgressSureSeconds`, `ProgressBarState`) e `islandDownloadFill`.
  Isso roda dentro do `explorer.exe`: estender a ponte `windhawk/cimori-tray-bridge.wh.cpp` para publicar
  o progresso à ilha por `WM_COPYDATA` (novo `dwData`, como os de bandeja) e o lado da ilha em
  `isle-platform-win`. Sem a ponte, continua o caminho atual (BITS + parciais).
- Pílula: preenchimento animado como o `islandDownloadFill` (enche suave, estados normal/pausado/erro,
  animação de início e de fim).
- Aba Downloads (`phase8_view.rs`): lista por download com nome, %, baixado/total, velocidade e tempo
  restante quando houver; nome/tamanho/velocidade vêm do arquivo parcial crescendo na pasta Downloads.
  Sem total: animação indeterminada + velocidade.
- Atualizar `windhawk/README.md` e `docs/PHASE8-VALIDATION.md`.

### C2 — Compartilhamento por QR nos dois sentidos
Roadmap: Fase 8 "Falta portar" (QR).
- Portar de `isle/crates/isle-daemon/src/share/` o envio do celular para o PC (`PUT api/upload`,
  `api/hello`, `api/bye`, `api/picking`, `page.html`, limites de `session.rs`), mantendo a aprovação por
  código de 4 dígitos que já existe. Arquivos recebidos vão para uma pasta própria e entram na prateleira.
- Portas fixas numa faixa, como o isle. **Decisão:** usar 47831–47839 (a 47830 é do servidor dos agentes).
- Regra de firewall: documentar (e, se o isle fizer, sugerir) a liberação da faixa no Windows.

### C3 — Prateleira e histórico completos
Roadmap: Fase 8 "Falta portar".
- Arrastar arquivos da prateleira para outro app (`SHDoDragDrop`, como o mac-island; copiar ou mover).
- Salvar prateleira e histórico em disco em `%APPDATA%\isle-win\` (como `isle-daemon/src/{shelf,clipboard}.rs`
  com seu `store`): imagens em arquivos, miniaturas para os cartões; podar o que sumiu.
- Colar direto na janela da frente depois de escolher um item (isle `clipboard.js` `paste()`/
  `pressPaste`: Ctrl+V, Ctrl+Shift+V em terminais).
- Lista de apps ignorados (isle `clipboard-ignored-apps`), por executável, nas configurações.
- Miniatura das imagens no histórico (isle `THUMB_WIDTH/HEIGHT`, `clipRules.js`).

### C4 — Servidor dos agentes com token
Roadmap: Pendências ("Servidor `:47830` aceita qualquer máquina da rede, sem token").
- Ver como o isle autentica os hooks (`isle/crates/isle-daemon` e `isle-ai` locais). Exigir um token
  compartilhado gravado nas configurações e passado pelos hooks instalados (`--install-hooks`), e manter
  `hooks_local_only` (localhost + Tailscale) como padrão. Hooks antigos sem token: aviso claro no log.

---

## Trilha Claude

### T1 — Cor de destaque em tudo
Ajustes item 3. Trocar toda cor fixa de destaque (o damasco padrão) pela cor de destaque do tema em toda
a ilha e na janela de configurações: fundo da aba selecionada, saída de áudio escolhida, botões da aba
Cópias, chips e seleções. Achar as constantes de cor em `isle-render` (ex.: `polish.rs`, `renderer.rs`,
`system_view.rs`, `pages.rs`, `settings_view.rs`) e passar a ler o destaque do estado/config. Teste: trocar o
destaque nas prévias e verificar que nenhum pixel do damasco padrão sobra nesses elementos.

### T2 — Abas só com ícone
Ajustes item 2. Tirar o rótulo sob cada ícone das abas da ilha (o tooltip já existe), centralizar o ícone
no quadrado do selecionado com respiro igual nos quatro lados. Cliques e geometria em `layout.rs` continuam
batendo com o desenho.

### T3 — Ícones nítidos nos avisos
Ajustes item 1. Os ícones dos apps nos avisos (lista e banner) saem pixelados. Aplicar a mesma solução da
bandeja (`tray/`: ícone de recurso no maior tamanho, 16 px desenhado pixel a pixel, sem reescala borrada) e
pedir o ícone no tamanho da escala do monitor. Arquivos prováveis: `isle-platform-win/src/notification_icons.rs`
e o desenho do ícone no renderer.

### T4 — Pílula mais larga e estilos com notch
Ajustes itens 4 e 5. Pílula fechada mais larga. Estilos do mac-island (Ilha, Fixo com entalhes, Barra; ver
README em torno das linhas 62 e 557, `islandPillNotches`, forma do entalhe perto da linha 14806 e 2459) e o
notch do isle (`gnome-extension/src/controller.js` 113–135, `compact.js`): preso à borda de cima, abre ao
passar o mouse e cresce até o painel. No notch fechado ficam os itens escolhidos (relógio por padrão),
pela mesma barra personalizável das configurações. Opções em `prefs.rs` (estilo, largura).

### T5 — Botão de desligar e atalho das Configurações do Windows
Ajustes itens 6 e 7. Do mac-island: `IslandItem::Power` (~15120), `islandPowerSleep/Hibernate/Restart/
ShutDown/Lock/SignOut/Confirm` (~2530), "Shut down now?" etc. Botão na pílula (item da barra
personalizável) e na central; cada ação liga/desliga; confirmação opcional. Atalho "Configurações" na
central (`ms-settings:`), como `ControlId::WindowsSettings`.

### T6 — Central de controle editável
Ajustes item 8. Botão "Editar" como o do mac-island (~88850): controles com "−", lista "Adicionar
controles", arrastar para ordenar; ordem e escolha salvas nas configurações. Portar o catálogo `ControlId`
(~40372) com os controles cuja ação já existe ou é simples (atalho, tecla, `ms-settings:`, app do Windows):
hotspot, modo avião, modo escuro, captura de tela, bloquear, calculadora, emoji, área de transferência,
mostrar área de trabalho, teclados virtual/touch, Configurações, menu Win+X, microfone, brilho, bateria,
energia, pendrives, lixeira, timer, pomodoro, alarmes, cronômetro, clima, próximo evento. Os que dependem
de recurso inexistente ficam anotados no roadmap.

### T7 — Unidades e atalhos como no mac-island
Fase 8 "Falta portar" (opções). Banner (tamanho e largura) e altura da pílula em % com os valores do
mac-island (`kBannerSizes` = 50, 75, 100, 125, 150, 200; `kPillHeightPctMin/Max` = 70/130), convertendo
as configurações antigas. Atalhos de minimizar e avisos gravados com qualquer combinação ("Clique e
pressione as teclas", como o mac-island) em vez da lista fixa.

### T8 — Fuso, feriados e alarme com som
Pendências e Fase 6 "Falta". Ler o fuso do Windows em vez de UTC-3 fixo (`src/app.rs` `LOCAL_OFFSET`,
`isle-core/src/agenda.rs` `OFFSET`); país dos feriados pela região do Windows quando não escolhido; alarmes
com som (como o mac-island faz para alarmes/lembretes).

### T9 — Abrir avisos de apps fora da Store
Fase 4 "Falta": ativação de apps fora da Store (CLSID do ativador no atalho do menu Iniciar,
`System.AppUserModel.ToastActivatorCLSID`) e `activationType="protocol"`, como o `OpenNotification` do mod.

### T10 — Fila de permissões
Pendências: "Um card de permissão por vez (o isle enfileira até 8)". Portar a fila do isle (até 8) para os
cards de permissão dos agentes.

---

## Fechamento (orquestrador)
1. Mesclar `feat/fin-codex` em `feat/fin-claude`, resolver conflitos (prefs/config/docs).
2. Revisão final de toda a branch; uma rodada de correções.
3. Commit só de formatação (`cargo fmt`) e limpeza de avisos do clippy.
4. Atualizar o roadmap (o que entrou, o que ficou) e o checklist de validação ao vivo para o Marcus.
5. Build em `target/release` para o Marcus testar. Nada vai para `feat/isle-win` sem o OK dele.

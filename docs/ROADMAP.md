# isle-win — Roadmap

A ilha do topo do Windows 11 em Rust, desenvolvida no próprio Windows desde 11/11/2026 (antes
cross-compilada no Fedora; ver `docs/WINDOWS-DEV.md`), para substituir a ilha do
mod [Mac Island & Dock](https://windhawk.net/mods/mac-island-dock) e ser o hub dos agentes de IA
(Claude Code, Codex, OpenCode) rodando no Windows ou no Fedora via SSH.

> Este roadmap substitui `docs/superpowers/specs/2026-11-10-isle-win-design.md` e
> `docs/superpowers/plans/2026-11-10-isle-win.md` (gerados pelo Gemini; mantidos só como histórico).

## Estratégia: portar, não inventar

Cada recurso vem de código que já funciona:

| Fonte | Licença | O que traz |
|---|---|---|
| [zickrian/isle](https://github.com/zickrian/isle) @ `a29a7c4` (v1.7.1) | GPL-3.0-or-later | O "cérebro" em Rust: `isle-core`, agentes, chat (Claude/Codex/OpenCode), cotas de IA (`usage.rs`), letras, shelf e compartilhamento por QR |
| [Mac Island & Dock](https://github.com/ramensoftware/windhawk-mods/blob/main/mods/mac-island-dock.wh.cpp) v3.1.1 (caliberda) | GPL-3.0 | A parte Windows: central de controle, notificações, bandeja, volume por app, Wi-Fi, Bluetooth, USB, progresso de downloads, monitores |

Regras:

- **Toda funcionalidade nova da pílula deve ser configurável e personalizável** (decisão do Marcus,
  reafirmada em 12/11/2026). A entrega inclui controles para ativar ou ocultar o recurso e ajustar
  seu comportamento ou apresentação conforme se aplica. As preferências ficam na tabela
  `isle-core/src/prefs.rs`, chegam à execução e mantêm a compatibilidade com configurações antigas.
- Antes de escrever um recurso, procurar o equivalente nas duas fontes e portar. Trocar só a camada de
  plataforma (D-Bus/MPRIS/GNOME do isle → APIs do Windows; C++/Direct2D do mac-island → Rust/`windows`).
- Módulos copiados sem mudança do isle ficam idênticos ao upstream (fácil ressincronizar); os adaptados
  levam no topo de onde vieram e o que mudou.
- A interface não é copiável de nenhuma das duas (JS do GNOME Shell / C++ Direct2D): é refeita no
  renderizador do `isle-render` (tiny-skia + Inter), com a paleta e as medidas do original.
- Cada fase termina num `dist/isle-win.exe` usável e testado; o comportamento visível é conferido com as
  prévias PNG (`cargo run -p isle-render --example preview -- <pasta>`) antes do teste no Windows.
- Licença: ao entrar código do mac-island, o projeto passa a GPL-3.0 (sem "or later"), com crédito aos dois.

## Fases

Ordem pelo uso diário (opções ligadas no backup `Mac Island & Dock 2026-11-10.txt`).

### ✅ Fase 0 — Base e agentes de IA (10/11/2026)

- Núcleo do isle portado inteiro (`crates/isle-core`, 89 testes).
- Agentes portados (`crates/isle-ai/src/agents`): `isle-win --island-hook claude|codex --host IP:47830`
  instalado por `--install-hooks [--windows-host IP:47830]`, em forma exec (sem shell). Aprovação
  Permitir/Sempre/Negar na pílula, detecção de resposta dada no terminal, Codex via `hooks.json`.
- Marca `--island-hook` para conviver com o isle do GNOME (`--agent-hook`); alertas de IA do isle do GNOME
  desligados no Fedora.
- Renderizador com texto de verdade (Inter, `ab_glyph`): barra compacta, banner, card, abas.
- Testado ao vivo: Claude no Fedora → Tailscale → pílula no Windows → clique → Claude.

### ✅ Fase 1 — Fundação da janela (validada no Windows em 10/11/2026)

- DPI por monitor (Per-Monitor V2): renderizar em pixels físicos na escala de cada monitor, sem borrão.
- Uma ilha por monitor configurado (`\\.\DISPLAY1`, `\\.\DISPLAY6`), posicionada pelo nome real do
  monitor (`EnumDisplayMonitors` + `GetMonitorInfoW`), refeita quando monitores mudam
  (`WM_DISPLAYCHANGE`, `WM_DPICHANGED`).
- Loop de eventos com 0% de CPU parado: thread de UI bloqueada em `MsgWaitForMultipleObjects`, acordada
  por mensagens, timers e eventos dos serviços; quadros só enquanto a mola anima.
- Auto-hide: a pílula vira uma linha fina e volta quando o mouse chega (como o mac-island).
- Entrada: hover, roda do mouse, clique direito e do meio.
- Fonte: `window`/monitores do mac-island; `PillGeometry` (mola) já existe.
- Feito: renderizador em escala (`PillRenderer::with_scale`), `choose_monitors` por nome com o primário
  como reserva, `Waker` + `MsgWaitForMultipleObjects` (0,0% de CPU e 7 MB parado, medido no Linux),
  linha minimizada com faixa invisível de alcance na borda da tela, banners que somem em 6 s, painel
  que fecha quando o mouse sai, contador só para as próximas 12 h.
- Uma ilha independente por monitor (`iislandAllMonitors`); notificações, banners e cards na ilha
  principal escolhida (`sislandMonitor` → `main_monitor`), com o primário como reserva.
- Feito (11/11/2026): a ilha some do monitor que tem um app em tela cheia na frente (jogo em janela
  sem borda ou exclusivo, vídeo em tela cheia) e volta depois; regra `CoversMonitor`/`IsFullscreenAppOn`
  do mac-island, por monitor, acordada por eventos do Windows (sem timer).
- Falta: clique direito/do meio (a roda para volume entrou na fase 3).

### ✅ Fase 2 — Mídia (validada no Windows em 10/11/2026)

- Feito (10/11/2026, com o Codex): `isle-platform-win/src/media.rs` (GSMTC por eventos, numa thread MTA
  própria, a partir da `MediaThreadProc` do mac-island) e `lyrics.rs` (LRCLIB do isle, com cache e casamento
  por duração). Aba Mídia com título, artista, progresso, linha da letra e anterior/tocar/próxima; a barra
  compacta mostra a música tocando quando não há compromisso nas próximas 12 h. Comandos e a primeira
  leitura do Windows ficam fora da thread da interface.
- Feito (11/11/2026, polimento Cimori): **capa do álbum** (GSMTC → WIC num worker, até 512 px, releituras
  por 6 s). Título rolando entrou na segunda leva de configurações.


- GSMTC (`GlobalSystemMediaTransportControlsSessionManager`): título/artista, capa, play/pause/próxima,
  título rolando (marquee).
- Letras sincronizadas: cliente LRCLIB do isle (`media/lyrics.rs`, com cache e casamento por duração).

### ✅ Fase 3 — Central de controle (1ª rodada validada no Windows em 11/11/2026)

- Validado ao vivo: Gerenciador de Tarefas, Explorador, luz noturna aplicando de verdade, Não Perturbe
  abrindo as Configurações, Bluetooth, volume e mute do microfone.
- Feito e em uso: **trocar a saída de áudio** (fone ↔ alto-falante, chips de saída na central,
  commit `93696e5`).
- Na fase de publicação: Não Perturbe real em vez de atalho e esconder o que não existe na máquina.

- Feito (11/11/2026): CoreAudio real numa thread MTA: volume/mute da saída padrão,
  mute do microfone padrão, callbacks de volume e troca de dispositivo, mixer por aplicativo
  (agrupado pelo executável). Roda do mouse em qualquer ilha altera o volume; OSD de 2 s na
  principal, preservando painéis e cards de permissão.
- Central de 520×360 (demais abas continuam 520×260): dez controles, slider com clique/arrasto,
  mute da saída e microfone, duas linhas do mixer com slider/mute e acesso ao mixer completo do
  Windows; botões fixados preservados. Previews conferidos em 100% e 150%.
- Wi-Fi: conexão/SSID via WLAN; botão abre Rede. Bluetooth: estado e troca real via
  Windows.Devices.Radios; abre Bluetooth se indisponível/negado. Luz noturna: parser e alteração
  validados do CloudStore, timestamp monotônico; formato desconhecido abre Configurações sem
  escrever. Não Perturbe abre Notificações (interface QuietHours privada do mod não portada).
- Atalhos reais: Gerenciador de Tarefas, Explorador, DisplaySwitch para Projetar e Win+Ctrl+L
  para Legendas ao vivo (Configurações se o envio falhar). Transmitir e Acessibilidade abrem
  as páginas do Windows. Workers enviam AppEvents + Waker; mixer/WLAN/rádio/luz noturna são
  relidos a cada 3 s somente enquanto alguma central está aberta; nenhum polling novo parado.
- Fonte: Mac Island & Dock v3.1.0 — GetVolume, StatusEvents, ReadMixerApps, Island::QueryNetwork,
  RadioThreadProc, ParseNightLight/SetNightLight, RunControlShortcut/PressKeys e controles.
- Falta: validação ao vivo no Windows (dispositivos, permissões WLAN/rádio, luz noturna,
  legendas, múltiplos monitores/DPI e CPU em repouso). Win+A foi pulado: o mod intercepta o
  teclado com WH_KEYBOARD_LL e sintetiza teclas para suprimir o atalho reservado; não usa
  RegisterHotKey. Brilho e pastas extras ficaram fora (a troca de saída entrou depois).
- Verificação Fedora: testes da fase passam; o workspace é impedido de ficar todo verde por
  seis testes existentes de hooks em isle-ai que não podem abrir sockets neste sandbox
  (EPERM). isle-ai permanece intacto. Detalhes e artefatos em docs/PHASE3-VALIDATION.md.

### ✅ Fase 4 — Notificações (Win+N) (validada no Windows em 11/11/2026)

- Validado: avisos do WhatsApp e do Windows Defender, histórico, Win+N.
- Feito depois da validação: aviso que some do Windows (WhatsApp limpa ao ler a conversa) some da ilha,
  como o painel do mac-island, que lista o que ainda está no banco do Windows.
- Feito (11/11/2026, já no Windows): "Limpar tudo" vale depois de reiniciar (marca d'água
  `m_clearedUntil` em `%APPDATA%\isle-win\avisos-limpos.txt`). Os testes do workspace passam no
  Windows (os de hooks usavam `/bin/sh`; um clique no slider se perdia quando o Windows negava a captura).
- Validado (11/11/2026): mensagens seguidas na mesma conversa do WhatsApp. Cada uma substitui o
  aviso anterior no banco; os avisos substituídos ficam enquanto o app tem badge (`MergeKeptToasts`
  do mod, com o badge lido do banco). Detalhes em `WINDOWS-DEV.md`.
- Corrigido e validado (11/11/2026): **avisos novos não chegavam depois da partida.** O callback do
  WNF lia o contexto no 3º parâmetro (`TypeId`, nulo) em vez do 4º, o `SetEvent` falhava calado e
  o worker nunca acordava; só a leitura inicial aparecia.
- Validado (11/11/2026): clicar num aviso da lista abre **na conversa**, como o `OpenNotification`
  do mod: o `launch` do `<toast>` vai para o `INotificationActivationCallback` do app
  (`ToastActivatorCLSID` do AppxManifest), numa thread própria; se falhar, abre o app
  (`shell:AppsFolder\<AUMID>`). Falta: apps fora da Store (CLSID no atalho do menu Iniciar) e
  `activationType="protocol"`.
- Feito (11/11/2026): clique no banner da ilha também abre na conversa; 4 avisos por página;
  **responder pela ilha** (botão "Responder" nos avisos com caixa de resposta, Enter envia, Esc
  cancela, Ctrl+V cola; a ilha pega o teclado como no arrasto do slider e devolve depois; o painel
  não fecha enquanto se digita). Envio pelo mesmo ativador, com `NOTIFICATION_USER_INPUT_DATA`
  (`SendReply` do mod). Falta validar o envio ao vivo. Emoji colorido e ícone real do app entraram no
  polimento Cimori (r2, [NOTIFICATIONS-VALIDATION.md](NOTIFICATIONS-VALIDATION.md)).
- Banners nativos: resolvido sem código, pela opção do Windows "Mostrar faixas de notificação"
  desligada por app (grava `ShowBanner=0` em
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\<AUMID>`). Validado com o
  WhatsApp: o aviso continua indo para o banco e para a ilha. O mod esconde a janela de banners do
  shell à força; não portado.

- **Feito (11/11/2026):** leitor de `wpndatabase.db` somente leitura com o SQLite do Windows
  (`winsqlite3.dll`), sem dependências novas. App/nome, título, corpo e horário; parser de textos
  portado do mod, deduplicação, histórico limitado a 100 itens e contagens locais por aplicativo.
- **Feito:** worker MTA com o sinal WNF do mac-island; eventos de diretório como fallback. Espera
  infinita em repouso, sem polling periódico saudável. Retentativas de 150/350/700/1500 ms só após
  evento (o sinal pode chegar antes da gravação); recuperação de falha a cada 30 s, fora da UI.
- **Feito:** banner de 6 s só na ilha principal, inclusive minimizada; preserva card de permissão,
  painéis e banners de IA. Clique abre Avisos e marca aquele item como lido. Novos avisos continuam
  contados quando o banner não pode aparecer; a leitura inicial é silenciosa e marcada como lida.
- **Feito:** aba Avisos de 520×360 com grupos por app, contagem de não lidos, idade, três itens por
  página, Anterior/Próxima e Limpar tudo. Win+N portado de `ShortcutKeyProc`, em thread própria:
  ignora teclas injetadas, consome repetição/soltura e preserva cards. Limpeza/leitura são locais à
  sessão e não escrevem nem apagam notificações do Windows.
- **Falta:** validar banco/WAL, nomes, WNF/fallback, Win+N, vários monitores/DPI e CPU no Windows.
  Banners nativos **preservados**: o mod move a janela do shell para fora da tela e depende de
  restauração, UI Automation e exceções para chamadas/alarmes; não há supressão simples segura
  portada nesta fase. Respostas, ativação de ações e sincronização de leitura com o Windows não
  entram neste recorte. Detalhes, testes, artefato e checklist: `docs/PHASE4-VALIDATION.md`.

### ✅ Fase 5 — Bandeja (validada com a ponte do Windhawk em 11/11/2026)

- Portado do mac-island 3.1.1: fixados de `strayPinned` na pílula, demais no `...`, painel
  paginado e PNGs reais. Processo separado: UI Automation nos botões do shell, com registro
  `NotifyIconSettings` e processo ativo como candidatos; sem callbacks privados adivinhados.
- Esquerdo via Invoke/clique real; direito no botão real abre o menu do app junto à barra.
  Worker por eventos, atualização de 3 s só com Bandeja aberta; nenhuma execução de exe ao clicar.
- 256 testes verdes, release e previews 100%/150% conferidos. Probe: 19 candidatos e 0 ms de
  CPU em 8 s parado. Sandbox sem desktop UIA: cliques, overflow, DPI e reinício do Explorer
  ainda não validados; registro pode conter candidatos históricos. [Detalhes](PHASE5-VALIDATION.md).
- Conferido fora do sandbox (11/11/2026): a lista funciona (18 ícones, nomes e PNGs), mas nenhum fica
  "confirmado": os cliques dependem dos botões reais da barra, e com a barra escondida pelo
  mac-island nenhum clique funciona.
- **Decisão (Marcus, 11/11/2026): bandeja opcional por um mod de ponte no Windhawk.** O mac-island
  lê os ícones dentro do explorer.exe (o `WM_COPYDATA` de `Shell_NotifyIcon` para a barra, com
  `TaskbarCreated` para os que já existiam); fora do Explorer o Windows não dá janela/ID/mensagem de
  cada ícone. A ponte (`windhawk/`, portada dessa parte do mod) repassa esses dados à ilha, que
  clica como a barra (`PostTrayIconMessage`): esquerdo = ação do app, direito = menu do app
  (ex.: fechar o Vanguard). Sem a ponte, fica o caminho do Codex. Quem não usa Windhawk não perde
  nada além da bandeja.
- Validado ao vivo (11/11/2026): lista completa pela ponte, clique esquerdo e menu do app no
  direito, ícones nítidos (os de recurso do app em 32 px; os de 16 px desenhados pixel a pixel).
  Falta: grade só de ícones como no mac-island (hoje cartões com nome), se o Marcus preferir.
- Feito (11/11/2026): a ponte `windhawk/cimori-tray-bridge.wh.cpp`
  (instalação em `windhawk/README.md`) e o lado da ilha `tray/bridge.rs` (janela `CimoriTrayHost`,
  leitura no formato do `ReceiveTrayIcons`, ícone `HICON` → PNG, cliques `PostTrayIconMessage` +
  `PostTrayIconRelease`). Com a ponte, a lista e os cliques vêm dela e o painel não abre mais a
  área de ícones da barra.

### ✅ Fase 6 — Agenda, clima e contadores (validada no Windows em 11/11/2026)

- Feito (11/11/2026): agenda no estilo "dia grande" do mac-island (`iislandCalendarBigDay`): o dia grande
  ao lado do mês, navegação entre meses, clique escolhe o dia, eventos do dia e próximos, feriados em
  vermelho. Leitor `.ics` novo (`isle-core/src/agenda.rs`, de `ParseIcs`/`IcsRule` do mod): linhas dobradas,
  UTC → local, dia inteiro, repetições diárias/semanais/mensais/anuais com intervalo, COUNT, UNTIL,
  BYDAY e BYMONTHDAY, EXDATE e ocorrência movida (RECURRENCE-ID). Feriados do Nager.Date (BR, ano atual
  e o próximo). A agenda é relida a cada 15 min. O leitor antigo do Gemini (`ics.rs`) saiu.
- Feito (11/11/2026, design em `docs/superpowers/specs/2026-11-11-fase6-design.md`):
  - **Clima** (`isle-core/src/weather.rs`, do `WeatherData` do mod): uma chamada ao Open-Meteo a cada
    30 min; temperatura e céu na barra compacta (clique abre a aba **Clima**); painel com cidade,
    nascer/pôr do sol, 8 dados, 7 dias clicáveis e curva por hora (hoje a partir de agora, outros
    dias de 3 em 3 h). Ícones do céu desenhados pela ilha (11 tipos do `WeatherConceptOf`).
  - **Aba Timer**: Pomodoro (anel do isle, foco/pausa com − / +), timer (− / + e 5/10/15/30 min) e
    cronômetro, todos ao mesmo tempo; a barra mostra o que termina primeiro em m:ss, acordando a
    cada segundo só enquanto algo corre. Ajustes salvos no `settings.json`.
  - **Lembretes**: manuais na aba Timer (texto + "daqui X min", guardados em `lembretes.json`, um
    vencido com a ilha fechada aparece ao abrir) e da agenda (`ClassifyReminder` do mod: na hora,
    perdido até 12 h, 5 por vez + "e mais N"; dados em `agenda-avisados.json`).
  - **Fila de alertas**: lembretes e fins de timer aparecem um por vez na ilha principal, esperando
    Não Perturbe, painel aberto ou card de permissão; o clique abre a aba de origem.
  - Configuração nova: `weather_fahrenheit`, `agenda_reminders`, `agenda_reminder_minutes` (0).
- Falta: alarmes com som; busca de cidade (geocoding) na tela de configurações; fuso
  lido do Windows (hoje UTC-3 fixo) e país dos feriados pela região do Windows (hoje BR).

### ✅ Fase 7 — IA completa (cotas e chat validados no Windows em 11/11/2026)

- Cotas reais de Claude e Codex (`usage.rs` do isle) com aviso perto do limite e "de volta!" no reset,
  inclusive lidas no Fedora e enviadas ao Windows.
- Feito (10/11/2026): `crates/isle-ai/src/usage.rs` portado do isle — Claude pelo login do Claude Code
  (`/api/oauth/usage`, a cada 10 min, recuo de 1 h se recusado), Codex pelo `auth.json` e pelos logs de
  sessão; aba IA em duas colunas com "renova em …"; aviso de 80%/95% e "de volta!" pela janela mais cheia;
  `isle-win --usage` mostra a leitura na hora. Testado com os números reais no Fedora. O "bridge" do
  Gemini (lia um `usage.json` que não existe) foi removido: cada máquina lê as próprias cotas.
- Logos do Claude Code e do Codex na aba IA.
- Feito (11/11/2026): chat na aba IA com Claude, Codex e OpenCode Go, portado de `assistant/` e
  `chat.js` do isle. Streaming por worker + AppEvent/Waker; sem polling em repouso. Entrada reutiliza
  Avisos (Enter, Esc, Backspace, Ctrl+V); seleção da IA, Parar, Nova e histórico rolável de até 50 mensagens.
- Claude usa `stream-json` e o login do Claude Code; Codex usa `app-server` e seu `auth.json`;
  Go usa `opencode run`, modelos `opencode-go/` e `opencode_api_key` das configurações, sem chave fixa.
- Falta: validar os três serviços, login/renovação, teclado/DPI/monitores e CPU ao vivo no Windows;
  voz depois. Histórico só nesta sessão; detalhes e verificações em [PHASE7-VALIDATION.md](PHASE7-VALIDATION.md).

### ✅ Polimento visual Cimori (aprovado pelo Marcus em 11/11/2026, com o Codex)

- O app passa a se chamar **Cimori** (identidade em `docs/brand/`, `--version` mostra
  `Cimori 0.1.0 | visual 2026-11-11-r2`). O executável e as pastas continuam `isle-win`.
- Casca escura com damasco, navegação vetorial com hover/tooltips, oito módulos, fantasminha da IA,
  logos Claude/OpenAI, ícones vetoriais nos controles. Painéis maiores (até 620×470; tela em
  `CANVAS_MAX` 720×520), geometria única para desenho e cliques (`isle-render/src/layout.rs`),
  rolagem quando a área útil do monitor é menor que o painel. Respeita "efeitos de animação" do Windows.
- Capa do álbum e ícones reais dos apps nos avisos (Shell/AUMID → exe em execução → `IconUri`),
  resolvidos num worker com cache; o banner guarda uma cópia do aviso mesmo se o Windows o apagar.
- Emoji colorido nos avisos e respostas (DirectWrite).
- Detalhes: [VISUAL-VALIDATION.md](VISUAL-VALIDATION.md), [NOTIFICATIONS-VALIDATION.md](NOTIFICATIONS-VALIDATION.md),
  referências em `docs/design/`. 308 testes verdes.
- Revisão depois (Claude, 11/11/2026): a partida da mídia passou a ter limite de 2 s; se o Windows
  demorar (logo após o login) a mídia ficava desligada até reiniciar. Agora tenta de novo a cada 30 s.

### ✅ Tela de configurações — 1ª entrega (validada no uso do Marcus)

Design: `docs/superpowers/specs/2026-11-11-configuracoes-design.md`; plano:
`docs/superpowers/plans/2026-11-11-configuracoes.md`; validação: [SETTINGS-VALIDATION.md](SETTINGS-VALIDATION.md).

- Janela própria no visual Cimori, no molde da do mac-island: lateral com 17 abas em três grupos
  (Ilha, Módulos, Cimori), busca em todas as abas, prévia da pílula de verdade, cartões em duas colunas,
  modo estreito só com ícones. Lembra posição, tamanho e aba; volta ao monitor principal se o dela sumiu.
- Abre pela engrenagem da ilha aberta, pelo clique direito na pílula e por `isle-win --settings`
  (com a Cimori rodando, pede a ela; sem, abre a Cimori já com a janela).
- **Barra personalizável:** arrastar os itens da pílula para dentro, para fora e mudar a ordem
  (relógio, próximo, data, música, clima, avisos, timer, bandeja). Relógio 12/24 h, segundos, dia da
  semana, data, ano, ordem e mês em número; contagens e janela do "próximo".
- Cor de destaque, fundo, distância do topo, animações (seguir o Windows/ligadas/desligadas), ficar
  sobre tela cheia, monitores pelo mapa, módulos liga/desliga (some da ilha na hora; o serviço para ao
  reabrir), botões da central e sua ordem, mixer, saídas, roda do volume, agenda (.ics, feriados e país,
  lembretes), cidade por busca no Open-Meteo, °C/°F, letras, avisos (banners, tempo, por página, ícones,
  apps silenciados), Pomodoro/timer, fixados da bandeja, Win+N, servidor dos agentes só local/Tailscale,
  hooks, backup (exportar/importar) e restaurar o padrão.
- Tudo aplica na hora; o `settings.json` é gravado inteiro de uma vez (arquivo temporário + troca),
  e um arquivo antigo ou estragado carrega campo a campo.

### ✅ Segunda leva de configurações (no build validado em 12/11/2026)

- Recolhimento por mouse, janela maximizada ou janela sobre a ilha, atraso, desaparecimento da linha,
  reabertura por clique ou mouse, largura da zona e abertura por hover.
- Atalhos de avisos e minimização, posição no monitor do cursor, altura e arredondamento, largura dos
  painéis acompanhando uma pílula mais larga, destaque e dicas sob o mouse.
- Banners com tamanho e largura ajustáveis e opção para a pílula minimizada, controles e letras na
  barra, rolagem de títulos longos e troca de faixa com a roda.
- Dia em destaque e eventos na agenda, cartões de clima, timer na linha, volume em porcentagem,
  botão do microfone e bandeja em grade ou com painel mantido aberto.
- Iniciar com o Windows permanece na fase 9, junto do instalador.

### ✅ Fase 8 — Área de transferência, prateleira, USB e downloads (build validado no uso do Marcus em 12/11/2026)

- Área de transferência com texto, links, imagens e arquivos, busca, itens fixados e restauração.
  O histórico fica em memória e respeita as marcas de exclusão do Windows.
- Revisão contra as fontes (Claude, 12/11/2026): o modelo do histórico e da prateleira é o do isle sem
  mudança; USB segue o mac-island. Corrigido: a leitura volta à ordem arquivos → texto → imagem das duas
  fontes (células do Excel viravam imagem). Portado: **aviso "Copiado"** do `OnClipboardChanged` do
  mac-island (opção "Mostrar o que foi copiado", funciona sem o histórico, pula a Ferramenta de Captura).
- Prateleira de arquivos com seleção ou arraste, ações de copiar e abrir e compartilhamento por QR
  para o celular na rede local. A sessão exige confirmação na ilha e expira por inatividade.
- Unidades USB com aviso opcional de chegada e ações de abrir e ejetar.
- Downloads com porcentagem dos jobs BITS e indicador indeterminado para arquivos parciais recentes
  na pasta Downloads. O preenchimento da pílula pode ser desligado.
- Os quatro módulos são ativados separadamente na janela de configurações. Ver
  [PHASE8-VALIDATION.md](PHASE8-VALIDATION.md) para cobertura e testes ao vivo.

- ✅ **Falta portar — entregue na fase 9 pela trilha Codex (C1–C4) e T7** (o Codex reescreveu ou deixou
  de fora; ver a revisão acima):
  - Downloads pelo progresso dos botões da barra (`ITaskbarList3`, como o mac-island), estendendo a
    ponte do Windhawk. O caminho atual (BITS + arquivos parciais) quase não dá porcentagem nos navegadores.
  - QR nos dois sentidos como no isle (celular envia arquivos para a prateleira) e portas fixas
    47830–47839; hoje só o PC envia, em porta aleatória.
  - Arrastar arquivos da prateleira para outro app (isle e mac-island); salvar prateleira e histórico em
    disco (as duas fontes salvam); colar direto na janela da frente e apps ignorados (isle); miniatura das
    imagens no histórico.
  - Opções com as unidades do mac-island: banner e altura da pílula em %, atalhos gravados com qualquer
    tecla (hoje lista fixa de 4).

### Ajustes de layout e pílula (pedido do Marcus, 12/11/2026)

Direção: a ilha segue o estilo do isle (pílula enxuta, atalhos a um clique a mais), não a pílula cheia do
mac-island. Do mac-island vêm a central editável, o botão de desligar e os atalhos do Windows.

1. **Ícones nítidos nos avisos.** WhatsApp, Claude e os demais apps aparecem pixelados na lista e no
   banner. Aplicar a mesma correção dos ícones da bandeja (recurso do app no maior tamanho, 16 px
   desenhado pixel a pixel) e ler o ícone na escala do monitor.
2. **Abas da ilha só com ícone.** Tirar o nome sob cada ícone (o tooltip já mostra), centralizar o ícone
   no quadrado do selecionado e dar respiro interno igual nos quatro lados.
3. **Cor de destaque em tudo.** O destaque escolhido não chega a vários lugares, que ficam no damasco
   padrão: fundo da aba selecionada, saída de áudio escolhida, botões da aba Cópias e outros. Trocar toda
   cor fixa pelo destaque do tema e conferir em todas as abas e na janela de configurações.
4. **Pílula fechada mais larga.** Hoje fica uma barrinha fina demais.
5. **Forma de notch.** Portar os estilos do mac-island (Ilha, Fixo com entalhes e Barra) e o notch do
   isle: preso à borda de cima, abre ao passar o mouse e cresce até o painel. No notch fechado ficam
   sempre visíveis os itens que o Marcus escolher (relógio por padrão, como no isle; clima, música etc.),
   pela mesma barra personalizável das configurações.
6. **Botão de desligar** na pílula e na central, do mac-island: suspender, hibernar (se o Windows
   permitir), reiniciar, desligar, bloquear e sair, cada um liga/desliga, com confirmação opcional para
   reiniciar, desligar e sair.
7. **Atalho para as Configurações do Windows** na central (falta; Gerenciador de Tarefas e Explorador já
   existem).
8. **Central de controle editável** como a do mac-island: botão "Editar" que mostra os controles com
   "−" para tirar e uma lista "Adicionar controles", arrastar para ordenar. Portar o catálogo do mod
   (`ControlId`, mais de 50): hotspot, modo avião, modo escuro, captura de tela, bloquear, calculadora,
   emoji, área de transferência, mostrar área de trabalho, teclados virtual e touch, Configurações, menu
   Win+X, microfone, brilho, bateria, energia, pendrives, lixeira, timer, pomodoro, alarmes, cronômetro,
   anotações, tarefas, clima, relógios mundiais, próximo evento, papel de parede, áreas de trabalho,
   pastas e locais, entre outros. Os que dependem de algo que a Cimori não tem entram quando o recurso entrar.
9. **Downloads com animação e mais dados.** Trazer o preenchimento animado da pílula do mac-island
   (`islandDownloadFill`: a barra enche com o progresso, com estados normal, pausado e erro) e animar
   início e fim. A aba Downloads hoje mostra só "1 em andamento" e "Tamanho total indisponível"; passa a
   listar cada download com nome, porcentagem, baixado/total, velocidade e tempo restante quando a fonte
   der. Depende do porte do progresso da barra (fase 8, "Falta portar"): a barra dá a porcentagem; nome,
   tamanho e velocidade saem do arquivo parcial crescendo na pasta Downloads. Sem total, fica a animação
   indeterminada e a velocidade.

### Fase 9 — Finalização e publicação (em andamento desde 12/11/2026)

- As fases 0–8 estão entregues. Esta fase fecha os ajustes de layout acima, o que falta portar e as
  pendências abaixo, dividida entre Claude e Codex em branches separadas (plano em
  `docs/superpowers/plans/2026-11-12-finalizacao.md`). O lançamento vem só quando o Marcus achar pronto.
- Até aqui tudo é feito para o uso do Marcus: redondo, bonito, funcional e leve em RAM/CPU.
- Só então generalizar para o público (notebooks com Wi-Fi/Bluetooth, controles que se adaptam à máquina,
  instalador, configuração sem o backup do mod) e publicar no GitHub, grátis, GPL-3.0 com crédito ao isle e
  ao mac-island.
- **Preparação da versão pública** (pedido do Marcus, 13/11/2026):
  1. ✅ **Sobre** mais completo (1.0.0): versão, link público do código no GitHub, licença, créditos (isle, mac-island,
     pacotes de ícones) e onde reportar problemas.
  2. ✅ **Verificação de atualização** (1.0.0; avisa e abre a página da versão, sem instalar sozinho): consultar as releases do GitHub, avisar na ilha e baixar/instalar a nova
     versão (opção de desligar).
  3. ✅ **Boas-vindas na primeira abertura** (1.0.0; aba Boas-vindas, como o assistente do mac-island): um passo a passo que guia as configurações iniciais (estilo, aba ao
     abrir, módulos), a instalação dos hooks dos agentes com um clique e a ponte do Windhawk.
  4. ✅ **Hooks em um clique** (botão em IA e nas boas-vindas): detectar Claude Code e Codex instalados e instalar/atualizar os hooks pela própria
     interface (hoje é `--install-hooks`).
  5. **Ponte do Windhawk publicada** (1.0.0: vai no zip, tutorial e "Copiar código da ponte" nas boas-vindas, estado detectado; falta enviar ao repositório do Windhawk): enviar o mod ao repositório do Windhawk (ou publicar no GitHub) e um
     tutorial de baixar, instalar, configurar e usar; a Cimori detecta se a ponte está ativa e indica o passo.
  6. **Tutoriais** curtos dentro das configurações para cada recurso que depende de algo externo.
  7. ✅ Zip portátil (`scripts/release.ps1`) e "Iniciar com o Windows" (1.0.0); instalador fica para depois.
  8. ✅ **Nome Cimori em tudo**: executável e processo `Cimori.exe`, pasta `%APPDATA%\Cimori\` (migrando a antiga
     `isle-win` na primeira abertura), mod, hooks e repositório público **github.com/Vini-Paixao/Cimori**.
  9. **Landing page** num domínio próprio para divulgar e para o portfólio do Marcus (domínio a definir).

### Pendências conhecidas

- ✅ Servidor `:47830` com token compartilhado (C4); hooks antigos sem token são reinstalados sozinhos.
- ✅ Fila de permissões de até 8, como o isle (T10); o card mostra "+N na fila" (acréscimo nosso).
- ✅ Fuso e país dos feriados lidos do Windows; alarmes com som (T8).
- Enquanto as fases não terminam, mac-island e isle-win disputam o topo: dividir por monitor.
- (Resolvido em 11/11: a mídia tenta de novo a cada 30 s se o GSMTC demorar.)
- O código não segue `cargo fmt` (estilo compacto desde o início; o polimento deixou vírgulas e chaves
  soltas como `,\n)`). Formatar tudo de uma vez num commit só de formatação, quando não houver ramo aberto.
- Clippy: ~66 avisos de estilo restantes (nenhum de correção).

### Fase 9 — trilha Claude: ajustes 1 a 8, unidades do mac-island e pendências

- ✅ **1–3** (T1–T3): ícones nítidos nos avisos, abas só com ícone, cor de destaque em toda a ilha e nas
  configurações.
- ✅ **Avisos de apps fora da Store** (T9): o ativador vem do registro (`CustomActivator`) ou do atalho do
  menu Iniciar (`ToastActivatorCLSID`), como o `UnpackagedToastActivator` do mac-island;
  `activationType="protocol"` abre o link (esquemas perigosos bloqueados).
- ✅ **Arrastar da prateleira** e **quadros da animação de downloads** ligados na ilha.
- Revisão final (13/11) corrigida: OLE para o arraste, relógio da ponte do Windhawk (mod 1.1), redesenho
  só enquanto o preenchimento anda, salvamento sem refazer miniaturas, agenda após troca de fuso, limite
  da prateleira e clique duplo na fila de permissões.

- ✅ **4. Pílula fechada mais larga** (T4): opção "Largura da ilha fechada" (Aparência → Tamanho), 280 px
  de base em vez dos 220 fixos, mais o espaço dos itens.
- ✅ **5. Forma de notch** (T4): Aparência → Estilo → "Entalhe". Porta o `NotchGeometry` do mac-island
  (corpo preso à borda de cima, cantos de baixo redondos, curvas ao contrário na borda, que somem na
  linha minimizada) e o comportamento do notch do isle (abre ao passar o mouse e cresce até o painel).
  O entalhe fechado mostra a sua própria lista "No entalhe" (Barra; relógio por padrão, centralizado).
  Ficam de fora os entalhes múltiplos (cantos, três) e o estilo Barra do mac-island: a direção é a
  pílula enxuta.
- ✅ **6. Botão de desligar** (T5): porta o menu de energia do mac-island (`PowerAction`, `PowerActionOn`,
  `PowerAsks`, `OnlyPowerAction`, `RunPowerAction`): suspender (desliga a tela no Modern Standby),
  hibernar (só quando o Windows permite), reiniciar, desligar (com inicialização rápida, como o Iniciar),
  bloquear e sair. Item "Desligar" da barra e botão da central; cada ação liga/desliga e "Perguntar antes"
  em Central de controle → Botão de desligar. Com uma ação só, o botão a executa direto.
- ✅ **7. Atalho das Configurações do Windows** (T5): botão "Configurações" da central (`ms-settings:`).
- ✅ **8. Central de controle editável** (T6): botão "Editar" no cabeçalho da aba (vira "Concluído"), "−" em
  cada controle, lista "Adicionar controles" com o resto do catálogo e arrastar para ordenar; tudo salvo
  em `control_tiles` na hora. Catálogo portado do `ControlId` do mac-island com o que tem ação simples:
  hotspot, modo avião, brilho, bateria e energia abrem a página do Windows; modo escuro troca o tema
  (`SetDarkMode`); captura de tela, emojis, área de transferência, mostrar área de trabalho e menu Win+X
  apertam as teclas (`PressKeys`); bloquear, calculadora, teclado virtual, teclado touch (TabTip),
  lixeira e Configurações abrem o app; microfone silencia; pendrives, timer, pomodoro, cronômetro,
  alarmes (lembretes), clima e próximo evento abrem a aba da ilha.
  - **Falta portar** (dependem de recurso que a Cimori não tem): os toggles próprios do mod para hotspot
    (`HotspotRequest`), modo avião, brilho por DDC/CI, seções de bateria e modo de energia; teclado touch
    por COM (`ITipInvocation`); anotações, tarefas, atalhos, atualizações, relógios mundiais, perfis,
    idioma do teclado, privacidade, áreas de trabalho, pastas, papel de parede e komorebi.
- ✅ **Fase 8, "Falta portar": unidades e atalhos do mac-island** (T7): altura da pílula em % (70–130,
  `kPillHeightPct*`), banner em tamanho (50–200 %, `kBannerSizes`, o conteúdo cresce junto, nítido) e
  largura (80–160 %, `kBannerWidths`); configurações antigas em px viram o valor mais próximo. Os atalhos
  de minimizar e de avisos são gravados com qualquer combinação ("Clique e pressione as teclas", com Win,
  Ctrl ou Alt, como o `OnShortcutRecorded`); Win+J e Win+N seguem pelo gancho do teclado, o resto é
  registrado à parte (`RegisterHotKey`). Atalhos do Windows que o próprio shell consome não chegam a ser
  gravados (o mod usa um gancho de teclado para isso).

# Fase 4 — Notificações: validação no Fedora (11/11/2026)

Status: **🧪 pronta no Fedora, falta validar no Windows**.
Implementada exclusivamente em `/mnt/dados/projetos/isle-win-notif`, branch
`feat/notifications`. Nenhum commit, push, merge ou branch nova. O outro worktree
não foi alterado; a configuração existente do linker somente lê seu cache de SDK.

## Entrega e comportamento

- Leitura de `%LOCALAPPDATA%\Microsoft\Windows\Notifications\wpndatabase.db`
  com `sqlite3_open_v2(..., SQLITE_OPEN_READONLY, ...)`. JOIN entre `Notification`
  e `NotificationHandler`, filtro `Type='toast'`, ID incremental e ArrivalTime
  FILETIME convertido para Unix ms. Máximo de 100 registros recentes por leitura.
  Não há UPDATE/DELETE, modificação de configurações do Windows ou modo de journal.
- SQLite carregado exclusivamente de System32 (`winsqlite3.dll`), como no mod.
  A busca em `~/.cargo/registry/src` não encontrou crate SQLite/XML em cache.
  Não foi necessário buscar pacotes: DLL nativa e leitor de `<text>` do mod
  mantêm todos os builds `--offline`; Cargo.lock permanece intacto. Cargo.toml
  acrescenta apenas a feature Win32_Security da dependência Windows já instalada,
  para CreateEventW.
- Parser de texto UTF-8 com entidades XML nomeadas/numéricas, CDATA, normalização
  CR/tab, descarte de `placement='attribution'` e rejeição de payload inválido.
  É um extrator de textos de toast, não um parser XML geral; não resolve DTD ou
  entidades externas. Payload limitado a 1 MiB, título a 256 caracteres e corpo
  a 1024; strings para desenho são novamente limitadas antes de ellipsize.
- Nome do app via AppsFolder/IShellItem, com cache; sem entrada no Iniciar,
  fallback legível do AUMID/executável. Não foram portados todos os fallbacks de
  registro/recursos do mod; conferir os apps usados diariamente.
- Inicialização carrega histórico em silêncio, marcado como lido. Somente novas
  chegadas durante o processo incrementam as contagens locais. Deduplicação por
  `(Id, ArrivalTime em ms)`; grupos ordenados pelo aviso mais recente do app e
  itens do mesmo app do mais novo ao mais antigo. Retenção de 100 avisos e 512
  identidades recentes, com cursor no worker; erro/consulta parcial não avança o
  cursor. Queda do maior ID recomeça a leitura silenciosamente.
- Banner de 440×64, duração de 6 s (pausa sob hover), apenas ilha 0/principal,
  também se minimizada. Preserva cards, painéis e banners de IA; nesse caso os
  avisos entram na lista sem banner posterior. Durante rajadas, mostra o mais
  recente; todos os itens retidos entram no histórico. DND interno suprime o
  banner. Clique abre Avisos e marca somente o item exibido como lido.
- Avisos usa 520×360, paleta existente e Inter: app, não lidos por app, título,
  corpo e idade relativa; três avisos por página, Anterior/Próxima e Limpar tudo.
  Clique marca aquele registro como lido, sem abrir o app. A aba mostra o total
  não lido. A paginação permite acessar todos os 100 itens, incluindo grupos que
  continuam na próxima página.
- Leitura/limpeza/unread são locais à sessão. Limpar tudo não apaga no Windows;
  reiniciar volta a carregar os últimos 100 registros como lidos. Ler em outro
  app ou na central nativa não sincroniza o estado local. Respostas, links,
  ações, imagens, som próprio e persistência entre execuções não foram portados.
- Win+N usa WH_KEYBOARD_LL em thread própria, como o mod: somente Win+N sem
  outros modificadores, ignora entrada injetada, consome repetição e keyup,
  envia tecla dummy para evitar abrir Iniciar ao soltar Win e manda um AppEvent
  com Waker. Abre/fecha Avisos na principal; não toma um card de permissão.
  Falha ao instalar o hook preserva o atalho nativo e gera warning em runtime.
  Win+A e os demais atalhos do enum antigo não foram alterados.

## Threads e CPU em repouso

O leitor roda em thread MTA; SQLite, resolução de nomes e esperas ficam fora da
UI. Eventos chegam pelo mesmo canal mpsc/AppEvent e closure `send` + Waker das
fases anteriores. Não há comandos de escrita ao serviço: marcar/limpar são
operações de estado na UI, sem acesso ao banco.

A inscrição WNF `0x0D83063EA3BC1035`, portando StartToastWatch, acontece antes da
primeira leitura para não perder o despertar na inicialização. O callback só
sinaliza um evento auto-reset. Se a API privada estiver ausente/recusar inscrição,
o fallback é FindFirstChangeNotificationW no diretório (LAST_WRITE/FILE_NAME),
incluindo mudanças no WAL e recriação do banco. Ambos usam espera infinita.

Após um sinal, as tentativas aguardam 150, 350, 700 e 1500 ms (parando na primeira
chegada), como no mod: WNF pode anteceder a gravação. SQLite tem busy_timeout de
500 ms. Esses prazos existem apenas após evento, não são polling periódico.
Uma falha de acesso/schema/monitor exibe indisponibilidade e tenta recuperar a
cada 30 s; somente o caminho de falha tem timer. As threads duram até o processo
terminar; handles/inscrições/consultas são liberados ao reiniciar uma tentativa.

Por inspeção, não há polling novo em repouso saudável; **0% de CPU não foi medido
no Windows**. O relógio já acordava a UI a cada minuto. O hook só age sobre teclas
reais; entrada frequente e mudanças reais do banco naturalmente geram trabalho.

## O que foi portado e onde

Referência GPL-3.0: caliberda, **Mac Island & Dock v3.1.0**, arquivo local
`/mnt/dados/projetos/isle-win-ref/mac-island-dock.wh.cpp`. Os módulos adaptados
levam cabeçalhos SPDX e crédito ao autor; a UI foi redesenhada em tiny-skia.

| Arquivo | Origem e alteração |
|---|---|
| crates/isle-platform-win/src/notification_windows.rs (novo) | Sqlite/ReadNewToasts/ReadLastToastId/GetNotificationDatabasePath/ToastThreadProc/StartToastWatch, linhas 12111–12684; AppDisplayName, 93375; ShortcutKeyProc/PressUnusedKey, 104417–104541. DLL dinâmica, RAII, leitura somente leitura, sinal WNF e hook Win+N. Fallback por eventos de diretório é adaptação Rust. |
| crates/isle-platform-win/src/notifications.rs | DecodeXml (91223), ToastTexts/SplitToastTexts (92399/12327), NotificationTimeText (92444); parser, timestamp, cursor, eventos e lógica testável do atalho. Enum de atalhos existente preservado. |
| crates/isle-core/src/notifications.rs (novo) | KeptToast/kMaxKeptToasts (12299/12577), AddUnread/UnreadApp (114739), NotificationPanel e agrupamento: modelo limitado, dedupe, contagens locais, ordenação e idade. |
| crates/isle-core/src/lib.rs, state.rs | Exportação e estado de histórico, página e indisponibilidade, com defaults de serde. |
| crates/isle-render/src/pages.rs | NotificationPanel::HitTest: ações estáveis por ID/tempo, limpeza e paginação com hitboxes compartilhados. |
| crates/isle-render/src/renderer.rs | NotificationPanel::Draw e ShowToasts: lista e contagens, idade, botões; Inter/paleta existentes. Banner reutiliza AlertBanner. |
| src/app.rs | Island::ShowBanner/ShowToasts (107791) e OpenFromShortcut (104736): integração aditiva por eventos, proteção de cards/painéis, banner e interações. |
| crates/isle-render/examples/preview.rs | Cenários sintéticos de banner, vários apps, mesmo grupo, segunda página e indisponibilidade. |
| Cargo.toml | Feature Windows já disponível; nenhum pacote novo. |
| .gitignore | PNGs de scratchpad-preview-phase4 ignorados, fora de commits. |
| docs/ROADMAP.md, docs/PHASE4-VALIDATION.md | Feito/Falta, resultados e checklist desta entrega. |

`crates/isle-ai` e o backup privado não foram alterados. O script de build
existente copia o backup privado para dist/, que já é ignorado pelo Git; essa
cópia permanece local e não deve ser publicada ou distribuída. Seu conteúdo e
sua URL não foram impressos. Não houve staging de nenhum arquivo.

## Banners nativos: preservados

O mod não oferece uma simples opção pública de ocultação temporária. Em
Island::OnWindowsBanner (115747), move a janela do shell para fora da tela via
SetWindowPos e mantém timers/estado para devolvê-la se o conteúdo não puder ser
lido. Há exceções para chamadas, alarmes, lembretes e botões do Windows; UI
Automation também participa da leitura/controle das janelas. O outro caminho
usa QuietHours privada para alterar Não Perturbe e precisa restaurar estado.

Esses caminhos não foram portados: dependem da UI e do ciclo de vida do shell
que este binário não controla. Nenhuma configuração de toast/DND foi alterada.
**Os banners Windows continuam aparecendo e podem coexistir com o da ilha.**
UI Automation, respostas/ativação e supressão reversível ficam para validação e
trabalho específico posterior, conforme a exceção autorizada no pedido.

## Testes e builds

TDD: falhas observadas antes das implementações de parsing, FILETIME,
agrupamento, dedupe, retenção, leitura/limpeza, regras de banner, cursor, Win+N,
hitboxes e desenho. Inclui textos grandes/Unicode, payloads inválidos e render
nas escalas 1×/1,5×. Uma revisão independente não encontrou falhas críticas ou
importantes; o caso de CDATA com `</text>` literal e comentários foi reproduzido
em teste antes da correção. Logs red/green locais em `/tmp/phase4-*.log`.

O comando obrigatório `cargo test --workspace --offline` foi executado e falhou
somente nos seis testes existentes que tentam abrir sockets no sandbox:

- hooks::tests::a_hidden_command_is_refused_before_it_reaches_the_island
- hooks::tests::an_ask_is_shown_and_answered_from_the_island
- hooks::tests::dnd_answers_nothing_at_once
- hooks::tests::quota_sync_still_reaches_the_island
- hooks::tests::reports_are_checked_and_passed_on
- hooks::tests::the_hook_leaving_withdraws_the_ask

Todos param em `crates/isle-ai/src/hooks.rs:193`, `bind ephemeral port:
Operation not permitted (os error 1)`. Não foram alterados nem ignorados.

Rodada adicional `cargo test --workspace --offline --no-fail-fast`:

| Target | Passaram | Falharam |
|---|---:|---:|
| isle-ai | 27 | 6 (EPERM) |
| isle-core | 95 | 0 |
| isle-platform-win | 37 | 0 |
| isle-render | 27 | 0 |
| isle-win | 30 | 0 |
| **Total** | **216** | **6** |

222 testes, 23 novos nesta fase; nenhum ignorado. Doc-tests passaram.
Não é afirmado workspace totalmente verde neste sandbox.
A consulta SQL efetiva do worker também foi executada contra uma fixture SQLite
em memória no Fedora (Python stdlib), conferindo JOIN, filtro toast, cursor,
limite de 100 e ordem. Isso não executa winsqlite3/WNF no Windows.

O build `cargo build --release --target x86_64-pc-windows-msvc --offline` passou
sem warnings. `./scripts/build-win.sh` passou: build Fedora, build Windows e
empacotamento concluídos, ambos sem warnings.
Rustfmt não está instalado no toolchain disponível; `git diff --check` foi usado.
Logs: `/tmp/phase4-required-tests.log`, `/tmp/phase4-workspace-tests.log`,
`/tmp/phase4-windows-build.log`, `/tmp/phase4-package.log`.

## Artefato e previews

- `dist/isle-win.exe`: **4.069.376 bytes**.
- SHA-256, primeiros 16 caracteres: **99D9E022491E2050**.
- SHA-256 completo: `99D9E022491E2050B6EFC46BF2A9076C0754F31B907C588D27B532A137BC436B`.
- O executável empacotado é idêntico ao de `target/x86_64-pc-windows-msvc/release/`.
- Script concluído nas três etapas, sem warnings; builds Windows e Fedora offline.

Previews gerados pelo comando obrigatório nas escalas 1 e 1.5:

```text
cargo run -p isle-render --offline --example preview -- scratchpad-preview-phase4/scale-1 1
cargo run -p isle-render --offline --example preview -- scratchpad-preview-phase4/scale-1.5 1.5
```

Pasta absoluta: `/mnt/dados/projetos/isle-win-notif/scratchpad-preview-phase4/`.
Em cada escala:

- `notification-banner.png` — banner Telegram.
- `notifications.png` — Calendário, Telegram, GitHub, contagem e paginação.
- `notifications-grouped.png` — três mensagens Telegram, contagem por app.
- `notifications-page-2.png` — avisos restantes, botão Anterior.
- `notifications-unavailable.png` — falha de leitura, sem dados inventados.

Banner, lista, grupo, segunda página e indisponibilidade inspecionados visualmente; cenários preexistentes também
são gerados para conferência/regressão. PNGs ignorados pelo Git.

## Checklist no Windows 11

- [ ] Fechar/desabilitar o Win+N do mac-island durante a rodada, para não haver
  disputa entre hooks. Iniciar o exe sem privilégios extras, com usuário normal.
- [ ] Iniciar com histórico já existente: últimos 100 avisos aparecem como lidos,
  nenhum banner antigo dispara. Conferir os nomes reais (Win32, UWP, apps web).
- [ ] Receber Telegram/WhatsApp/Thunderbird/Discord/GitHub ou os apps do uso diário;
  conferir app, acentos/emoji suportados pela fonte, título, corpo e idade.
- [ ] Enviar várias mensagens do mesmo app e de apps diferentes, incluindo
  mensagens espaçadas/rajadas: grupos, contagens, dedupe e aviso mais novo corretos.
- [ ] Conferir latência em relação à gravação no WAL. Se WNF falhar nessa versão,
  validar fallback de diretório e recuperação após indisponibilidade do banco.
  Não mover/editar o banco real para testar: usar perfil de teste se necessário.
- [ ] Banner só no monitor principal escolhido, por aproximadamente 6 s;
  hover segura o banner e sair permite expirar, sem CPU ocupada.
- [ ] Manter card de Claude/Codex aberto e receber aviso/Win+N: o card/reply não
  é substituído. Repetir com abas Controle/Mídia/Avisos e banner de IA.
- [ ] Ilha minimizada recebe banner na principal; secundárias não mudam. Clique
  no banner abre Avisos e lê somente aquela mensagem.
- [ ] Win+N abre/fecha Avisos, também com foco em outro app; manter N pressionado
  abre uma vez; soltar Win antes de N não digita N no app. Não deve abrir Iniciar.
  N sozinho, Win+Shift+N, Win+A e atalhos injetados preservam comportamento.
- [ ] Clique em linhas de páginas diferentes diminui só o contador correto;
  Anterior/Próxima alcançam todos os avisos; Limpar tudo zera lista e página;
  aviso novo após limpar reaparece normalmente, sem ressuscitar os antigos.
- [ ] Conferir 100%, 125%, 150%, múltiplos monitores e mudança de DPI/resolução:
  texto, cliques e botões não sobrepõem bandeja ou barra de abas.
- [ ] CPU no Gerenciador de Tarefas após 5 min sem eventos, com painel fechado
  e depois aberto; comparar com fase 3. Conferir RAM estável após muitas chegadas.
- [ ] Fechar o processo: Win+N nativo volta. Nenhum toast/configuração/DND deve
  permanecer alterado. Banners nativos permanecem visíveis durante toda a rodada.
- [ ] Conferir Não Perturbe do Windows separadamente: fase 3 abre Configurações,
  não fornece seu estado ao `dnd_active`; esta fase não sincroniza Focus Assist.
- [ ] Repetir os seis testes de hooks fora do sandbox, além do workspace completo.

Sem execução/hardware Windows nesta sessão: comportamento ao vivo das APIs,
permissões, conflitos de hooks e consumo de CPU continuam pendentes.

# Fase 3 — validação no Fedora (11/11/2026)

Implementada diretamente em `feat/isle-win`, sem commits, push ou branches novas.
Status: **🧪 pronta no Fedora, falta validar no Windows**. As APIs Windows foram
cross-compiladas; nenhum resultado de hardware ao vivo é afirmado aqui.

## Comportamento entregue

| Controle | Implementação |
|---|---|
| Volume geral e mute | CoreAudio, saída padrão eMultimedia; slider por clique/arrasto, roda ±2 pontos por notch, callbacks IAudioEndpointVolumeCallback. |
| Microfone | Mute real do dispositivo de captura padrão eConsole, com callback. Ausência de dispositivo é exibida como indisponível. |
| Volume/mute por aplicativo | IAudioSessionManager2 + ISimpleAudioVolume; sessões agrupadas pelo executável, primeira sessão ativa representa o estado; alterações atingem todas as sessões do app. |
| Wi-Fi | WlanEnumInterfaces + WlanQueryInterface, conexão e SSID reais; botão abre ms-settings:network-status. SSID pode faltar se o Windows negar localização. |
| Bluetooth | Windows.Devices.Radios, estado e SetStateAsync reais; negado/indisponível abre ms-settings:bluetooth. |
| Luz noturna | Parser estrito e alteração do blob Data no CloudStore, timestamp monotônico e preservação dos bytes opacos; formato desconhecido não é escrito e abre ms-settings:nightlight. |
| Não Perturbe | Abre ms-settings:notifications; não exibe estado inventado. QuietHours privada do mod não foi portada. |
| Projetar | DisplaySwitch.exe. |
| Transmitir | Abre ms-settings:connecteddevices. |
| Legendas ao vivo | Win+Ctrl+L via SendInput como no mod; erro/recusa abre ms-settings:easeofaccess-closedcaptioning. |
| Acessibilidade | Abre ms-settings:easeofaccess. |
| Gerenciador / Explorador | Executa taskmgr.exe / explorer.exe. |
| Mixer completo | Abre ms-settings:apps-volume. |

A central mede 520×360 px lógicos; demais abas seguem 520×260. Dez tiles, volume,
microfone, duas linhas de mixer e bandeja fixada. Entradas são convertidas pelo DPI
da janela. O arrasto guarda a identidade do app mesmo se a lista mudar. A ilha é
ativada somente durante o arrasto, captura o mouse e restaura o foco anterior ao
terminar/cancelar; cliques comuns continuam sem ativação.

A ativação explícita durante arrasto segue as restrições documentadas de
[SetCapture](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setcapture)
e a ativação explícita permitida para
[WS_EX_NOACTIVATE](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles).
Perda de captura cancela o arrasto; a liberação intencional não cancela um arrasto posterior.
O painel não fecha por afastamento do mouse enquanto há arrasto ativo.

CoreAudio e controles usam workers MTA independentes. A UI recebe AppEvents e é
acordada por Waker. Notificações de volume, mute e troca de endpoint ficam ativas;
mixer e estados de WLAN/rádio/CloudStore atualizam a cada 3 s somente enquanto há
uma central aberta. Nenhum timer novo em repouso. OSD de volume de 2 s somente na
ilha principal, sem substituir painel, banner de IA ou card de permissão. O prazo
de um banner sob o mouse não causa espera de zero segundos nem loop ocupado.

## Arquivos e origem do porte

Todos os arquivos Rust adaptados levam SPDX GPL-3.0 e crédito a caliberda v3.1.0.
Referência local: `scratchpad/macisland/mod.cpp` fornecido no pedido.

| Arquivo | Fonte / alteração |
|---|---|
| crates/isle-platform-win/src/audio_osd.rs | GetVolume, StatusEvents, ReadMixerApps e SetAppVolume: APIs CoreAudio, callbacks, worker e lógica pura testada. Remove os antigos no-ops. |
| crates/isle-platform-win/src/system_controls.rs | Island::QueryNetwork, RadioThreadProc, ParseNightLight/SetNightLight e RunControlShortcut/PressKeys: WLAN, rádio, registro e atalhos; fallback explícito para Configurações. |
| crates/isle-core/src/state.rs | kControls/MixerApp: enum e dados próprios da central; não altera os módulos copiados byte a byte do isle. |
| crates/isle-render/src/pages.rs | ControlPanel::Layout/HitTest: geometria compartilhada de tiles/sliders e ações. |
| crates/isle-render/src/renderer.rs | ControlPanel::DrawTile/DrawMain: UI reimplementada em tiny-skia com paleta e Inter existentes. |
| crates/isle-render/src/window.rs | Island::HandleMessage: WM_MOUSEWHEEL, down/drag/up/capture e conversão por DPI. |
| crates/isle-render/examples/preview.rs | Cenários da central preenchida/indisponível, canvas suficiente para 360 px. |
| src/app.rs | Island::OnVolumeChanged/ShowStatusNotice: eventos de workers, volume/OSD, seleção por ilha, drag e tamanho da aba. |
| Cargo.toml, crates/isle-platform-win/Cargo.toml, Cargo.lock | Features Windows; dependência direta windows-core 0.58 necessária à macro implement dos callbacks COM, já presente transitivamente (nenhum pacote novo). |
| docs/ROADMAP.md | Feito/Falta e novo status da Fase 3; roda concluída nas pendências de fases anteriores. |
| docs/superpowers/plans/2026-11-10-control-center.md | Plano da execução autorizada. |
| docs/PHASE3-VALIDATION.md | Este relatório. |

`crates/isle-ai` ficou intacto. Arquivos pré-existentes não rastreados (backup do mod
e isle_win_arquitetura.md) ficaram intactos. dist/isle-win.exe foi atualizado pelo
script existente.

## Testes e builds

TDD red → green: conversões/limites de volume, agrupamento e seleção de sessão,
parser/round-trip/rejeição de CloudStore, grid sem sobreposição, slider, hitboxes,
render em 1×/1,5×, tamanho por aba, wheel por monitor, drag com identidade estável,
abertura/fechamento de serviços, proteção de cards e deadline do OSD em hover.

Comando obrigatório:

```text
cargo test --workspace --offline
isle-ai: 27 passed; 6 failed
bind ephemeral port: Operation not permitted (os error 1)
error: test failed, to rerun pass `-p isle-ai --lib`
```

Falham por EPERM em crates/isle-ai/src/hooks.rs:193, ao abrir socket local:

- hooks::tests::a_hidden_command_is_refused_before_it_reaches_the_island
- hooks::tests::an_ask_is_shown_and_answered_from_the_island
- hooks::tests::dnd_answers_nothing_at_once
- hooks::tests::quota_sync_still_reaches_the_island
- hooks::tests::reports_are_checked_and_passed_on
- hooks::tests::the_hook_leaving_withdraws_the_ask

Execução completa adicional, sem parar no primeiro crate:

```text
cargo test --workspace --offline --no-fail-fast
isle-ai:           27 passed; 6 failed (EPERM)
isle-core:         90 passed; 0 failed
isle-platform-win: 27 passed; 0 failed
isle-render:       25 passed; 0 failed
isle-win:          24 passed; 0 failed
Total: 193 passed; 6 failed; 0 ignored (199 testes, 19 novos).
Doc-tests: todos passaram.

cargo build --release --target x86_64-pc-windows-msvc --offline
Finished `release` profile [optimized] target(s) in 10.07s
Zero warnings.

./scripts/build-win.sh
Fedora release: Finished `release` profile [optimized] target(s) in 8.69s
Windows release: Finished `release` profile [optimized] target(s) in 0.07s
Packaging dist/ bundle for Windows 11... concluído
```

Não foi possível obter workspace totalmente verde sob este sandbox; os seis testes
não foram ignorados ou alterados. Logs completos: /tmp/phase3-required-tests.log,
/tmp/phase3-workspace-tests.log, /tmp/phase3-windows-build.log, /tmp/phase3-package.log.
`git diff --check` passou. rustfmt não está instalado no toolchain atual.

## Executável e previews

- dist/isle-win.exe: **4.011.008 bytes**.
- SHA-256, primeiros 16 caracteres em maiúsculas: **F24629CCF0E326D1**.
- Previews gerados com cargo run -p isle-render --offline --example preview -- DIR SCALE.
- Pasta base: /tmp/claude-1000/-mnt-dados-projetos-isle-win/e14901e3-7652-4e21-afe3-e9adac144867/scratchpad/preview-phase3/
- 100%: scale-1/control-center.png (580×380), inspecionado visualmente.
- 150%: scale-1.5/control-center.png (870×570), inspecionado visualmente.
- Ausência de serviços: scale-1/control-center-unavailable.png, também inspecionado.
- As duas pastas incluem os previews de mídia, IA, compacto, banner e card.

## Limites e validação no Windows

Shortcuts registrados no código: duas linhas de mixer (mais apps no mixer do
Windows), nomes pelo executável (sem recursos de nome amigável/UWP), refresh de
3 s somente com painel aberto e Não Perturbe pelas Configurações. Não há seletor de
saída, brilho ou pastas extras nesta entrega, fora do recorte solicitado.

Win+A **pulado**: o mod usa WH_KEYBOARD_LL e síntese de teclas para interceptar o
atalho reservado; não apresenta um caminho simples por RegisterHotKey. O atalho do
Windows foi preservado; a central abre pela aba Controle.

No Windows, conferir:

1. Teclas e roda de volume em ambas as ilhas, OSD só na principal; permanecer com
   mouse sobre OSD por mais de 2 s e confirmar CPU em repouso; sair e conferir hide.
2. Volume/mute do dispositivo padrão, troca USB/Bluetooth/HDMI e falta de saída;
   microfone padrão e mute externo. Comparar com o mixer do Windows.
3. App com várias sessões (browser/Spotify): volume e mute em todas elas, nova
   sessão aparecendo, saída do app e ausência de redirecionamento durante drag.
4. Arrastar até fora da janela e soltar, em monitores 100%/150%; confirmar captura,
   ausência de drag preso e restauração do foco do aplicativo anterior.
5. Wi-Fi conectado/desconectado, SSID e permissão de localização; Bluetooth
   ligar/desligar, adaptador ausente, modo avião ou acesso recusado.
6. Luz noturna: ligar/desligar visualmente, inclusive cliques rápidos, horário
   automático e máquinas com registro por dispositivo; fallback quando não suportado.
7. Todos os atalhos e fallback das Legendas com app elevado em primeiro plano;
   Não Perturbe nas Configurações, Projetar e Transmitir.
8. Abrir/fechar a central em dois monitores, banners/cards simultâneos, leitura de
   estados e CPU após fechar a última central. Não foi medido 0% CPU no Windows.

As chamadas WinRT podem demorar em provedores travados; ficam isoladas em workers
e não bloqueiam a UI. A luz noturna usa formato privado do Windows: embora haja
validação antes da escrita, seu efeito real precisa ser validado na máquina alvo.

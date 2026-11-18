# Fase 5 — Bandeja (11/11/2026)

Implementada em `feat/tray`, a partir de `8a587d5`, somente nesta worktree.
Validação interativa pendente; a cópia instalada em `C:\Tools\isle-win` permaneceu rodando.

**Commit bloqueado pelo sandbox:** `git add` falhou antes de alterar o índice:
`Unable to create 'D:/isle-win/.git/worktrees/isle-win-tray/index.lock': Permission denied`.
Branch continua em `8a587d5`, com alterações sem stage nesta worktree. A pasta de metadados
Git compartilhada fica fora da raiz gravável; não foi tentado contornar a restrição.
Mensagem pronta em `target/tray-commit-message.txt`, com a coautoria pedida. Não houve merge/push.

## Fonte e adaptação

[Mac Island & Dock por caliberda v3.1.1](https://github.com/ramensoftware/windhawk-mods/blob/main/mods/mac-island-dock.wh.cpp),
GPL-3.0: `TrayIcon`, `TrayIconKey`, `TrayIconName`/`GetProgramName`, `strayPinned`,
`TrayListPanel`, `OpenFromTrayIcon`, `TrayIconNewer`, `kTrayVersionUnknown`,
`PostTrayIconMessage`/`PostTrayIconRelease` e `AllowExplorerMessages`.
Referência baixada em `target/mac-island-dock.wh.cpp`, ignorada pelo Git; SHA-256
`072f86f776b46b42ec8b4f59ce8d6d7b125b0c9436acfeb25043070f01a7725c`.

O mod intercepta `Shell_NotifyIcon` dentro do Explorer e repassa owner HWND, callback, uID,
versão e imagem. O isle-win é outro processo: nenhuma API pública enumera esse conjunto.
Não há injeção, leitura de offsets de memória do Explorer, broadcast de `TaskbarCreated`,
filtro de mensagens elevado nem callback adivinhado. Por isso o envio antigo/versão 4 e
o envio duplo para versão desconhecida do mod não são reproduzidos aqui.

Escolha: **UI Automation nos botões reais do shell + metadados do registro**
`HKCU\Control Panel\NotifyIconSettings`. A árvore XAML de `Shell_TrayWnd`, barras secundárias
e `TopLevelWindowForOverflowXamlIsland` fornece nomes e ações. No modo clássico, somente
botões dentro dos toolbars de notificação com correspondência no registro são aceitos.
O registro fornece GUID/uID, caminho (inclusive prefixo de pasta conhecida), tooltip inicial
e `IconSnapshot` PNG. Sem tooltip, o nome vem de `FileDescription`, como `GetProgramName`.
Isso evita depender do layout privado do Explorer em 24H2/25H2.

O registro é histórico. Sem botão UIA materializado, só entram candidatos cujo executável
está rodando. **Processo ativo não prova que aquele ícone ainda existe**: candidatos históricos
podem aparecer, inclusive ícones antigos do Explorer. Correlação com botão exige identidade
única nos dois sentidos; versões antigas do mesmo GUID são deduplicadas depois da confirmação.
Nenhum clique inicia o executável ou escolhe um botão por substring/índice antigo.
`Shell_NotifyIconGetRect` não é usado para enumerar nem confirmar vida de ícones.

## Interface e eventos

- `pinned_tray_apps`/`strayPinned`: GUID ignora caixa e mudança de versão do caminho;
  sem GUID, caminho normalizado + uID. Ausentes não viram launchers. Até seis fixados na
  pílula; excedentes continuam acessíveis no painel.
- `...` abre painel de 520×360, duas colunas, dez itens por página; paginação se ajusta
  quando a lista diminui. O painel abre na ilha clicada e preserva cards de permissão.
- PNGs reais com cache por conteúdo; inicial do app quando não há snapshot válido.
  Hitboxes e desenho usam as mesmas posições em pixels lógicos.
- Esquerdo: `Invoke` do botão real; reserva: clique físico nele. Direito: clique físico
  no botão real. Releitura UIA antes de cada ação, verificação de cobertura, coordenadas
  físicas com DPI por monitor, restauração do cursor se ele não tiver sido movido.
  O menu/flyout surge junto à barra de tarefas, sob controle do shell/app.
- Ícones ocultos podem exigir mostrar o overflow nativo brevemente. Isso ocorre apenas
  ao abrir o painel ou ativar um ícone oculto, nunca na partida ou em repouso. O worker
  fecha somente o overflow que abriu durante a enumeração; após uma ação bem-sucedida,
  não fecha o menu/flyout do app.
- Worker MTA próprio → `AppEvent::Tray` + `Waker`. UIA estrutura/nome/visibilidade e
  `RegNotifyChangeKeyValue` acordam o worker. Eventos são agrupados; espera infinita
  com painéis fechados e atualização de 3 s enquanto alguma Bandeja está aberta.
  Falha de assinatura UIA preserva os eventos do registro. Roots são religadas ao
  mudar HWNDs do Explorer; diagnóstico tem limite de 8 s.

## Verificado

- `cargo test --workspace`: **256 testes**, zero falhas; 32 IA, 113 core, 44 plataforma,
  33 render, 34 app, além dos doctests. Log: `target/tray-tests.log`.
- `cargo build --release`: sucesso; artefato `target/release/isle-win.exe`, copiado para
  `dist/isle-win.exe` (ambos ignorados pelo Git).
- Previews gerados e conferidos visualmente em 100% e 150%: `tray-compact`, `tray-panel`,
  `tray-page-2`, `tray-unavailable`, em `target/tray-preview-100/` e `target/tray-preview-150/`.
  Usam snapshots locais reais; nomes/estado do preview são uma fixture, não prova de apps ativos.
- Probe nativo separado (`--tray-inspect --watch-seconds 12`): 54 registros, 19 candidatos
  de processos ativos, incluindo Tailscale e Google Drive. Deskflow não estava rodando.
  **Zero botões confirmados pelo shell**: a sessão restrita não expõe a barra de tarefas.
- Worker release permaneceu ativo com `registro_eventos=true`, `painel=false`, uma publicação
  inicial e **0 ms de CPU adicionais em 8 s**. Evidência: `target/tray-watch-release.log`.
  Essa medição cobre o fallback nesta sessão, não o desktop normal completo.
- Revisão independente: corrigidos inclusão de controles do shell e vários registros antigos
  reivindicando o mesmo botão; testes de regressão passam.

## Ainda precisa de desktop interativo

UIA do sandbox não vê janelas do desktop; a assinatura de eventos retorna `0x8007029C`,
e o host do Computer Use também não está disponível. Portanto **não foram validados ao vivo**:
cliques esquerdo/direito e menus dos apps, abrir/fechar overflow, mudanças de tooltip/ícone,
remoção/adicionamento, reinício do Explorer, vários monitores/DPI e CPU com UIA acessível.
Atualização de imagem depende dos snapshots mantidos pelo Explorer.
Nomes ambíguos ou ícones sem correspondência segura recusam a ação; apps elevados,
barra oculta/coberta e UIPI podem impedir cliques. Sem UIA/registro utilizável, o painel
informa indisponibilidade. Não se declara validação funcional completa da fase.

Diagnóstico seguro, sem iniciar a UI, ocupar a porta dos hooks ou sobrescrever o log instalado:

```powershell
cargo run --release -- --tray-inspect
cargo run --release -- --tray-inspect --watch-seconds 12
```

Para conferir visualmente (crie as pastas antes):

```powershell
cargo run -p isle-render --example preview -- target/tray-preview-100 1
cargo run -p isle-render --example preview -- target/tray-preview-150 1.5
```

`ISLE_PREVIEW_TRAY_ICONS=<pasta>` permite fornecer `<nome do app>.png` localmente.
Sem essa variável, previews usam iniciais. Logs normais continuam em
`%APPDATA%\isle-win\isle-win.log`; o diagnóstico escreve apenas no stdout.

APIs consultadas: [UIA GetCurrentPattern](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-getcurrentpattern)
e [protocolo Shell_NotifyIcon](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyicona).

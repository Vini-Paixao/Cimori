# Avisos: emojis e icones dos aplicativos

11/11/2026. Atualizacao `Cimori 0.1.0 | visual 2026-11-11-r2`.

## Alteracao

- Titulo, corpo, nome do app e campo de resposta usam DirectWrite/Direct2D com fallback de fontes
  do Windows e emojis coloridos. Shaping e reticencias usam a mesma API. A interface restante
  conserva Inter. Rasterizacao na escala fisica do monitor; cache ate 48 linhas/8 MiB.
- Icones resolvidos por AUMID na AppsFolder, fallback `package!App`, executavel local e IconUri
  registrado. Remetentes de sistema registrados sem IconUri usam a imagem padrao do Windows.
  Extracao em worker dedicado; fila/cache de 128 identidades e cache negativo de cinco minutos.
  Sem polling em repouso; falha de extracao conserva o simbolo generico.
- Metadados chegam antes dos icones. A chegada da imagem atualiza os itens existentes e o banner,
  sem criar novo aviso, alterar nao lidos ou prolongar o prazo. O banner guarda um snapshot para
  manter o visual mesmo quando o app remove a notificacao do banco do Windows.
- Leitura DB/WNF, ativacao, resposta, ponte Windhawk e configuracoes mantidas.

Referencia de porte: Mac Island & Dock v3.1.1, `LoadAppIconBitmap`, `RegisteredAppPicture` e
`NotificationPanel::Draw`. APIs: [fontes coloridas](https://learn.microsoft.com/en-us/windows/win32/directwrite/color-fonts)
e [icones da Shell](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellitemimagefactory-getimage).

## Evidencias

- RED observado antes da implementacao: backend de texto nativo e campo/setter de icone ausentes.
- Teste nativo confirma pixels coloridos no emoji de curativo e corte da linha com Unicode/ZWJ.
- Parser preserva emoji, tom de pele, ZWJ, bandeira e variation selector. Teste do centro confirma
  que icone tardio nao altera conteudo nem lido. Regressao de banner cobre remocao no Windows e
  chegada tardia de icone sem mudar prazo/modo/nao lidos.
- Probe da Shell extraiu o icone real de `5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App`: 128x128 RGBA.
  Imagem exportada apenas em `target/visual-audit/whatsapp-icon.png`, sem embutir logo de terceiros.
- Previas Rust em `target/visual-audit/notifications-{100,150,200}/`: lista, resposta e banner
  inspecionados. Os outros remetentes nas fixtures sao sinteticos e nao recebem icones reais.
- Revisao independente encontrou perda do visual do banner apos remocao do item no banco;
  corrigida com snapshot e teste. Segunda revisao sem problemas importantes pendentes.
- Workspace completo: **308 testes aprovados**, zero falhas, `cargo test --workspace --offline`.
  Log em `target/visual-audit/notification-workspace-tests.log` (PowerShell UTF-16).
- `git diff --check` passou. Os arquivos novos de Rust passaram no `rustfmt --check`.
  `cargo fmt --all --check` ainda aponta diferencas de formatacao no workspace, inclusive
  `build.rs` que esta inalterado; nao foi aplicada uma reformatacao global neste ajuste.

As previas usam dados ficticios e o icone extraido localmente; nao sao capturas da ilha instalada.
O usuario aprovou o resultado no monitor e autorizou o commit apos a instalacao.

## Build e instalacao

Build release offline concluido. Instalado em `C:\Tools\isle-win\isle-win.exe` e copiado para
`dist/isle-win.exe`. Release, distribuicao e instalado possuem SHA-256 identico:

`1A4A53CED277B76FC3239C3056580C58709C03A31EA76DE79253EBD3984B31D8`

5.036.032 bytes. Backup: `C:\Tools\isle-win\isle-win.before-notifications-20261111-200748.exe`.
Ilha anterior encerrada (PID 38616); versao r2 iniciada no caminho instalado, PID **740**,
respondendo e dona da porta de hooks 47830. `--version` instalado confirmou a revisao r2.
Log sem ERROR/panic; `RUST_LOG=warn` herdado e log vazio na observacao de 20:08:06.

Evidencias locais: `target/visual-audit/notification-{release-build.log,deployment.json,runtime.json}`.
Previas persistidas: [lista](design/native/avisos.png), [resposta](design/native/avisos-resposta.png),
[banner](design/native/aviso-banner.png). Commit local autorizado posteriormente pelo usuario;
merge e push nao solicitados.

Revalidacao a pedido do usuario (20:16): novo `cargo build --release --offline` passou,
instalado e reiniciado. Release, dist e instalado com SHA-256
`7CAF774AA3BD24452EEAA2FA340464FC2F7B157994B4D95BDFD3A13636CBFD89`, 5.036.032 bytes.
Processo instalado **41800**, respondendo, porta 47830. Backup desta reinstalacao:
`C:\Tools\isle-win\isle-win.before-notifications-20261111-201619.exe`.
Controle de tela novamente indisponivel (native pipe ausente); abertura da aba Avisos pelo usuario
com Win+N. A alteracao ja esta no Rust, sem novas mudancas de produto nesta revalidacao.

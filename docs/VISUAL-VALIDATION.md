# Cimori — integração visual nativa

Atualização dos avisos instalada: `visual 2026-11-11-r2`, com ícones reais dos aplicativos e
emojis coloridos. [Validação e instalação da r2](NOTIFICATIONS-VALIDATION.md).
O registro abaixo descreve a entrega inicial do layout (r1).

11/11/2026. Revisão: `Cimori 0.1.0 | visual 2026-11-11-r1` (`--version`).

## No aplicativo Rust

- Casca escura com profundidade sutil e damasco; cabeçalho, navegação vetorial, hover e tooltips.
- Oito módulos: IA, Controle, Agenda, Clima, Mídia, Avisos, Foco e Bandeja.
- Fantasminha vetorial, expressões por estado e olhar por eventos; repouso estático.
- Logos Claude/OpenAI junto aos nomes dos provedores, convertidos dos SVGs preservados
  em `crates/isle-render/assets/`, origem Isle/Simple Icons CC0.
- Microfone ligado/mudo, transporte, enviar/parar, nova conversa, limpar e reiniciar com ícones.
- Capas GSMTC/Windows, WIC no worker, compartilhadas por `Arc`: 8 MiB de stream, 4096 por eixo/16 MP,
  saída até 512 por eixo. Metadados publicados antes da imagem; identidade limpa a capa anterior.
- Prazo compartilhado de dois segundos por leitura, cancelamento e até quatro releituras em seis segundos.
- Geometria compartilhada entre desenho e cliques. Limite pela área útil, corpo rolável em altura
  menor e navegação/rodapé de ação fixos. Preferência de animações do Windows respeitada.

Não entraram novos serviços de voz, Shelf/clipboard, chamadas, USB ou downloads.
Logo Cimori, ponte Windhawk, credenciais, hooks e preferências preservados.

## Verificação

- Testes de geometria falharam inicialmente para Bandeja ausente na navegação, controle de microfone
  e transporte de mídia; passaram após a implementação.
- Testes de imagem: limites, PNG/JPEG e rejeição de bytes inválidos. Testes de ciclo: troca de identidade,
  cancelamento e ordem de publicação metadados → imagem.
- Revisão independente encontrou três pontos importantes: altura em DPI alto, retry perdido após
  leitura lenta e metadados tardios. Os três receberam correção e testes.
- Workspace: **303 testes aprovados**, incluindo os contratos existentes. Log em
  `target/visual-audit/workspace-tests.log` (PowerShell: UTF-16).
- Prévias reais: `target/visual-audit/native-{100,125,150,200}/`; viewport de 340 px e rolagem em
  `native-short-top/` e `native-short-bottom/`. A capa de Queen é fixture destas prévias,
  não imagem padrão do executável.
- Probe nativo de mídia: Windows sem sessão ativa naquele momento. Não valida reprodução real.
- Computer-use sem pipe nativo/host de navegador. Os PNGs inspecionados são do Rust, não capturas
  da ilha instalada; não substituem cliques e DPI físico ao vivo.

## Instalação para validar

Build release concluído e instalado em `C:\Tools\isle-win\isle-win.exe`.
Cópia de entrega: `dist/isle-win.exe`. Os dois arquivos e o release têm o mesmo SHA-256:

`D8B4B8EE34DA2F72C46CF30C200D9290C2772C16C9E28096B56F76C020775D86`

Tamanho: 4.981.760 bytes. Backup anterior:
`C:\Tools\isle-win\isle-win.before-visual-20261111-193821.exe`.
Processo iniciado e confirmado respondendo: PID **38616**, no caminho instalado;
porta de hooks 47830 pertencente a esse PID. `--version` da cópia instalada confirmado.
Log sem ERROR/panic; filtro herdado `RUST_LOG=warn` explica o log vazio nesta observação.

Amostra inicial de 5,06 s: 54,9 MiB de working set e 0,93% de um núcleo de CPU. Não é
benchmark controlado de repouso e não há comparação com a versão anterior, já encerrada.
Evidências: `target/visual-audit/{deployment,runtime}.json`, `release-build.log`, `final-tests.log`.
Prévias: 35 imagens por escala e por viewport, **210 PNGs** ao todo. Exemplos persistidos em
[`docs/design/native/`](design/native/); são prévias nativas com dados de teste, não capturas ao vivo.

## Validação ao vivo pendente

Conferir abertura/recolhimento, abas, sliders, microfone, menus reais da bandeja, respostas,
foco/teclado e streaming. Com um player tocando, conferir capa, pausa, troca rápida e fallback
quando o próprio app não fornecer imagem. Conferir DPIs físicos disponíveis e movimento reduzido.

Após validar o resultado no monitor, o usuário aprovou a entrega e autorizou o commit local
do refinamento visual e dos avisos. Merge e push não foram solicitados.

# Cimori — Fase 8 e segunda leva de configurações

A primeira entrega da janela de configurações foi validada por Marcus nos cenários que testou em
12/11/2026. Esta rodada acrescenta opções na mesma janela e quatro módulos que começam desligados.
Novas funcionalidades da pílula devem vir com opções no catálogo isle-core/src/prefs.rs ligadas ao
comportamento em execução. Arquivos de configurações antigos recebem os novos padrões.

| Módulo | O que testar e personalizar |
| --- | --- |
| Aviso de cópia | Ligar "Mostrar o que foi copiado" com o histórico desligado. Copiar texto longo (uma linha, 32 caracteres e …), um arquivo (nome), vários (quantidade) e uma imagem. Win+Shift+S não mostra aviso. Copiar células do Excel guarda texto, não imagem. |
| Área de transferência | Ativar o módulo. Copiar texto, link, imagem e arquivos. Buscar, fixar, excluir, limpar recentes e copiar de volta. Alterar o limite e desligar imagens e arquivos. Confirmar que uma cópia marcada para ficar fora do histórico pelo Windows continua fora. |
| Prateleira | Ativar o módulo. Selecionar ou arrastar arquivos, copiar, abrir e retirar. Ajustar o limite, desligar o QR e mudar o tempo de sessão. Escanear com o celular na mesma rede, conferir o código e autorizar. Verificar download, recusa, encerramento e bloqueio da sessão do Windows. |
| USB | Ativar, conectar uma unidade, conferir aviso e nome, abrir no Explorer e ejetar. Verificar o resultado quando um aplicativo tem o volume aberto. Desligar o aviso e conectar novamente. |
| Downloads | Ativar e iniciar uma transferência BITS com total conhecido. Conferir a porcentagem e o preenchimento. Repetir com um navegador que escreva um parcial na pasta Downloads. Desligar apenas o preenchimento e conferir que a aba segue disponível. |

O histórico da área de transferência e a prateleira ficam em memória nesta entrega. O QR compartilha
arquivos do PC para o celular. A sessão parte de um endereço privado da interface de rede local,
exige autorização por código e encerra por inatividade. Downloads BITS usam progresso em bytes.
Para arquivos temporários de navegador na pasta Downloads, a ilha indica atividade sem inventar
uma porcentagem. Transferências em outras pastas ou que não exponham nenhum desses sinais podem
não aparecer. A opção de iniciar com o Windows fica na fase 9 do roadmap.

O histórico e a prateleira partem dos modelos de [Isle](https://github.com/zickrian/isle/tree/a29a7c4)
e o QR porta o protocolo de sessão local. A detecção de USB e as preferências da ilha seguem a
referência de [Mac Island & Dock](https://github.com/ramensoftware/windhawk-mods/blob/main/mods/mac-island-dock.wh.cpp).

## Regressão ao vivo das opções

1. Conferir os três modos de esconder, atraso, linha apagada, clique, zona larga e abrir por hover.
2. Mudar altura, raio e largura com a prévia à vista. Alternar dica e destaque com a ilha aberta.
3. Mover o ponteiro entre dois monitores, inclusive com DPI diferente, e conferir onde a ilha aparece.
4. Trocar combinações de avisos e minimizar, alternar o interruptor e conferir a tecla original do Windows.
5. Reproduzir e pausar, alternar controles na barra, letra, tamanho/cor, título longo e roda sobre a música.
6. Alternar calendário, cartões de clima, timer minimizado, volume, microfone e bandeja em grade.
7. Alterar dimensões de banner, habilitar ou desabilitar banners minimizados e conferir com uma notificação real.
8. Em janela estreita, rolar a lateral até Atalhos e Backup e reabrir nas mesmas abas.

A suíte Windows com cargo test --workspace --all-targets --locked --offline passou com 422 testes.
Ela inclui formatos do clipboard, compartilhamento e aprovação em loopback, mudança de arquivos na
pasta Downloads, atalhos simulados, aplicação de preferências e geometria da interface. As imagens em
target/phase8-preview são prévias do renderizador com dados sintéticos. As verificações interativas
com telefone, mídia, USB físico, downloads e atalhos globais precisam de uma sessão real do Windows.

Build de release gerado em target/release/isle-win.exe, com 5.743.616 bytes. SHA-256
D5790B515C169140C06A4034BB28AA91682C02C9FA579C9674E6EFE5AB994056.

Validação (12/11/2026): Marcus usou o build com o aviso de cópia e a nova ordem de leitura e aprovou.
Os itens que ainda não foram portados das fontes estão em ROADMAP.md, fase 8, "Falta portar".
## Fase 9 — C1 (trilha Codex)

- Novas opções Downloads: progresso Windhawk, animação e lista detalhada, inicialmente
  desligadas para preservar configurações existentes. BITS/parciais continuam disponíveis.
- Testar Chrome/Edge/Firefox, dois downloads simultâneos, pausa, falha, cancelamento,
  conclusão, reinício do Explorer e ausência da ponte. Conferir 100% e 150%.
- Por arquivo parcial: nome, bytes e taxa de crescimento; total e tempo restante só
  aparecem quando conhecidos. O percentual agregado da barra não identifica um arquivo.
- Integração para Claude em `src/app.rs`: o agendamento de quadros deve considerar
  o tween `state.phase8.download_fill` (450 ms) e a cauda de conclusão/cancelamento
  (900 ms), além de progresso indeterminado. Esse arquivo não foi editado pela Codex.
- O exemplo `preview` pode ultrapassar a pilha padrão de 1 MB no Windows. Para gerar
  as mesmas prévias sem alterar a outra trilha: `cargo rustc -p isle-render --example
  preview -- -C link-arg=/STACK:16777216`, depois executar `target/debug/examples/preview`.

## Fase 9 — C2 (QR nos dois sentidos)

Ative **Receber arquivos do celular**, abra o QR (a prateleira pode estar vazia),
confira os quatro dígitos e permita o celular. Os arquivos recebidos ficam em
`%APPDATA%\isle-win\received\` e entram na prateleira quando completos. Limites
portados do isle: 4 GB/arquivo, 8 GB/sessão, 1 GB de reserva no disco, 30 s sem
receber bytes e 16 KiB/s mínimos após 30 s; uploads reservam vagas na prateleira.
Um nome já existente recebe sufixo numérico; cancelar remove o arquivo parcial.

Portas TCP fixas **47831–47839** na interface LAN privada escolhida. A porta 47830
é dos agentes. Quando todas estiverem ocupadas, a interface informa o erro.
Se o Firewall do Windows bloquear, Marcus pode liberar o executável apenas no
perfil privado (ou executar como administrador, ajustando o caminho):

```powershell
New-NetFirewallRule -DisplayName 'Cimori QR privado' -Direction Inbound -Action Allow -Protocol TCP -LocalPort 47831-47839 -Profile Private -RemoteAddress LocalSubnet -Program 'C:\Tools\isle-win\isle-win.exe'
```

A aplicação não muda o firewall automaticamente. Testar Android/iOS, múltiplos
arquivos, nomes com acentos, arquivo vazio, rejeição, queda de Wi-Fi, disco cheio,
todas as portas ocupadas e interrupção durante o envio.

## Fase 9 — C3 (histórico e prateleira)

As novas opções de salvar histórico/prateleira, colar ao escolher, miniaturas e
mover no arraste começam desligadas. Persistência usa `%APPDATA%\isle-win\`:
`clipboard.json`, `shelf.json`, `clips\<hash>.png`, `clips\<hash>.thumb.png` e
`shelf-previews\`. A gravação é serializada em segundo plano; referências a arquivos
locais desaparecidos são podadas. Índice corrompido é preservado em `.json.corrupt`;
imagens não são apagadas quando a lista original não pôde ser recuperada.
As opções de persistência pedem reinício para recuperar o conteúdo salvo.

Marcus: testar retomada após reiniciar, excluir uma imagem/arquivo fora da ilha,
copiar de um executável ignorado, usar o histórico no Bloco de Notas e no Windows
Terminal, manter outra janela na frente durante a cópia e conferir que ela não
recebe colagem acidental. Prévias específicas: `target/phase9-previews` em 100/150%.

**Integração pendente para Claude:** chamar `IsleApp::drag_shelf_file(index)` no
início do gesto de arraste (mouse pressionado + distância mínima) em `src/app.rs`.
A operação `SHDoDragDrop` está portada; chamá-la num clique já solto não inicia um
arraste. Usar o thread OLE da interface. O backend permite copiar/mover e retira da
prateleira somente após efeito MOVE ou desaparecimento do arquivo original.


## Fase 9 — C4 (servidor dos agentes com token)

Execute `isle-win --install-hooks` de novo: os hooks passam a levar `--token` (guardado em
`settings.json`, `hooks_token`). Um hook antigo sem token registra um aviso no log e é recusado (401).
Marcus: pedir uma permissão a um agente e ver a ilha responder; `curl -X POST http://127.0.0.1:47830/api/hook`
sem token deve dar 401; abrir ~40 conexões ociosas (`telnet 127.0.0.1 47830`) e conferir que um hook real
ainda é atendido e que as ociosas fecham em ~2 s.

## Fase 9 — correções finais (Claude)

- QR: feche o QR logo depois de um envio concluído e com a prateleira cheia; o arquivo precisa aparecer na
  prateleira (o mais antigo sai para dar vaga).
- Mark-of-the-Web: após receber um arquivo, `Get-Content "%APPDATA%\isle-win\received\<arquivo>" -Stream Zone.Identifier`
  mostra `ZoneId=3` e o IP do celular; abrir um executável recebido deve acionar o SmartScreen.

# Tela de configurações da Cimori

Data: 11/11/2026. Projeto: isle-win (marca Cimori). Mockup aprovado:
`.superpowers/brainstorm/4655-1794442758/content/config-janela.html` (rascunho 1, "muito top, é isso aí").

## Por que

Na ilha do mac-island o Marcus usa a configuração **sempre**: personaliza o que fica e o que sai da
pílula, liga e desliga módulos e ajusta detalhes. As configurações do caliberda são completas, e ele
quer o **mesmo nível** na Cimori, pensando no produto (fase 9, publicação), e não só no uso dele.

Hoje a Cimori não tem tela de configurações. As regras ficam em `IsleConfig` (`settings.json`, salvo por
`IsleApp::save_config`), e boa parte do que a ilha faz está fixa no código.

## Metas

1. Uma **janela de configurações** própria, no visual Cimori, com a mesma organização do mac-island:
   barra lateral de abas com ícones, busca e prévia da pílula no topo.
2. **Tudo o que a Cimori já faz** pode ser ligado, desligado ou ajustado ali, com base nas opções
   equivalentes do mac-island (lista por aba abaixo).
3. **Barra personalizável:** escolher quais itens aparecem na pílula fechada e em que ordem,
   arrastando (o "ícone que fica, ícone que sai").
4. Cada mudança vale **na hora**, sem botão Aplicar e sem reiniciar.
5. **Regra de projeto a partir daqui:** todo recurso novo já entra com as suas opções nesta janela.
   Vai no `ROADMAP.md` como regra.

## Fora do escopo

- Opções de recursos que a Cimori ainda não tem: dock, barra de tarefas, bateria, brilho, USB, downloads
  (pílula enchendo), shelf, clipboard, lixeira, pastas, papel de parede, áreas de trabalho virtuais,
  komorebi, perfis, anotações, tarefas, lançador, CPU/RAM, relógios mundiais, visualizador de áudio,
  idioma do teclado, privacidade (câmera/microfone em uso), botão de desligar, Win+A, entalhes, estilos
  Fixa/Barra, vidro/liquid glass. Cada um traz as próprias opções quando for feito (meta 5).
- Assistente de primeira configuração, "Novidades" e Logs do mac-island (ficam para a fase 9).
- Tradução: só português por enquanto. Os textos ficam num lugar só, para traduzir depois.
- Iniciar com o Windows: o Marcus abre a Cimori com dois cliques no `.exe`, e o mac-island já inicia
  com o Windows. Fica para a fase 9 (instalador).
- No monitor onde está o mouse (`islandFollowMouse`): mudaria como as ilhas são criadas por monitor.
- Importar o backup do mac-island pela janela (o `IsleConfig::from_windhawk_backup` continua como está).

## O que seria um fracasso

- Uma janela com cara de "configuração genérica do Windows", abaixo do nível visual que o Codex entregou
  na ilha.
- Interruptor que não faz nada, ou uma opção que só vale depois de reiniciar sem avisar.
- Gastar CPU com a janela fechada, ou a ilha travar enquanto a janela está aberta.
- Perder ou corromper o `settings.json` (gravação pela metade, campo novo que quebra um arquivo antigo).
- A prévia da pílula diferente da pílula de verdade.

## Como abre

- Engrenagem no cabeçalho da ilha aberta (ao lado da seta de recolher).
- Clique direito na pílula (hoje o clique direito não faz nada; ver ROADMAP, fase 1).
- `isle-win --settings`: se a Cimori já está rodando, pede à instância aberta para mostrar a janela;
  se não está, abre a Cimori com a janela à vista.
- Janela já aberta: só vem para a frente. Uma janela só.
- A janela lembra posição, tamanho e última aba (como `SettingsWindowState` do mac-island).

## A janela (do mockup aprovado)

- Janela normal do Windows: move, redimensiona, aparece na barra de tarefas e no Alt+Tab, fecha no X
  ou com Esc. Barra de título escura, integrada ao fundo grafite.
- **Lateral:** marca Cimori, busca e abas em três grupos (ver abaixo). Janela estreita: só os ícones,
  como `m_navNarrow` do mac-island.
- **Cabeçalho da página:** título da aba e uma linha dizendo para que ela serve.
- **Prévia ao vivo da pílula** no topo das abas Barra, Aparência e Esconder. É a pílula desenhada pelo
  mesmo código da ilha (`isle-render`), com o estado real do momento (hora, música, clima).
- **Cartões** com linhas. Tipos de linha (os do mac-island):
  - interruptor;
  - escolha em botões (segmentos), ex.: 24 h / 12 h;
  - slider com valor ao lado;
  - cores (bolinhas, com uma "própria" que abre um seletor);
  - campo de texto (cidade, endereço da agenda, chave de API);
  - lista com chips (ex.: faixa da barra, botões da central, ícones fixados da bandeja);
  - botão de ação (ex.: "Abrir pasta das configurações", "Exportar").
- Cada linha pode ter uma descrição curta embaixo. Uma opção que depende de outra fica apagada
  enquanto a outra estiver desligada.
- Em janela larga, duas colunas de cartões; em janela estreita, uma.
- **Busca:** procura em todas as abas, pelo nome da opção, pela descrição e por palavras extras
  (`SettingText` do mac-island). Enquanto se busca, a página mostra só as linhas que batem, com o nome
  da aba de cada uma.
- Visual: o da ilha (tema em `isle-render/src/theme.rs`, Inter, ícones vetoriais de `icons.rs`,
  damasco como destaque). Hover, foco por teclado (Tab, setas, Espaço/Enter) e animações respeitando
  a preferência de animações do Windows (`animations_enabled`).

## Abas e opções

Cada linha diz o que já existe em `IsleConfig` (✓), o que é só tirar do código (fixo hoje) e o que
precisa de comportamento novo (**novo**). Entre parênteses, a opção equivalente do mac-island.
Padrão = o comportamento de hoje, para nada mudar sozinho na atualização.

**Primeira entrega x levas (decisão do Marcus, opção B):** a primeira entrega traz a janela com as
opções que já têm comportamento no código (✓, ou fixo hoje e só passa a ler a configuração) e as
**novo** explicitamente pedidas (relógio, faixa da Barra, busca de cidade). As marcadas **leva 2** têm
comportamento que ainda não existe na Cimori. Elas **não aparecem** na janela até serem portadas do
mac-island, em levas seguintes, cada uma testada. Nenhum interruptor sem efeito.

### Grupo "Ilha"

**Geral**
- Fechar a Cimori: botão "Sair" (hoje o Marcus fecha matando o processo; minha decisão).
- Pasta das configurações: botão "Abrir pasta".
- Win+N abre os avisos da ilha (`islandWinN`; hoje sempre ligado).
- Servidor dos agentes: porta (`hook_port` ✓) e "aceitar só desta máquina e do Tailscale" (ver
  pendência de segurança no ROADMAP; **novo**, padrão desligado para não quebrar o Fedora → Windows).

**Barra** (o que a pílula fechada mostra)
- Faixa de itens arrastáveis (`islandOrderDrag` e a faixa de ordem do mac-island). Itens de hoje:
  relógio, data, próximo compromisso/contagem, música, clima, ícones fixados da bandeja. Itens novos
  baratos que entram: avisos não lidos (sino com número, `islandBell`), cotas da IA (`islandAiUsage`),
  timer correndo (`islandTimer`). Cada item pode sair ou voltar e mudar de ordem. O relógio pode
  sair, mas pelo menos um item fica.
- Relógio: formato 24 h / 12 h (`islandClockFormat`), segundos (`clockSeconds`), dia da semana
  (`clockWeekday`), data (`clockDate`), ano (`clockYear`), ordem da data (`islandDateOrder`), mês em
  número (`islandMonthNumber`). Todos **novos** (hoje o formato é fixo).
- Contagem até o próximo (`islandCountdown`): eventos, timer, Pomodoro
  (`islandCountdownEvents/Timer/Pomodoro`) e a janela de 12 h (hoje fixa).
- Música na barra: mostrar (`mediaTitle`), capa (`islandMediaCoverOnly` como "só a capa"), botões
  de mídia na pílula (`mediaButtons`, **novo**) — **leva 2**, sempre mesmo parada (`islandMediaAlways`) — **leva 2**, letra no
  lugar do título (`islandLyricsInTitle`), largura máxima (`islandLyricsWidth`).
- Clima na barra (`islandWeather`).
- Tamanho: altura da pílula e distância do topo (`gap_px` ✓).

**Aparência**
- Cor de destaque: damasco (padrão), algumas prontas e "própria" (`islandTheme`).
- Cor de fundo da casca (`background_rgb` ✓).
- Arredondamento (`notch_curve` ✓).
- Dicas sob os ícones (`islandTips`) — **leva 2** e destaque sob o mouse (`islandHoverHighlight`) — **leva 2**.
- Painéis na largura da ilha (`islandPanelsFollow`) — **leva 2**.

**Posição e monitores**
- Em todos os monitores (`all_monitors` ✓) ou só nos escolhidos (`monitors` ✓, lista com os nomes
  reais e um mapa simples das telas, como a aba Monitores do mac-island).
- Monitor principal: avisos, banners e cards (`main_monitor` ✓).
- Ficar sobre apps em tela cheia sem borda (`islandOverFullscreen`; hoje some sempre).
- A aba só aparece com dois monitores ou mais (`SettingTabShown`).

**Esconder**
- Esconder na linha até o mouse chegar (`auto_hide` ✓).
- Quando esconder (`islandHideWhen`) — **leva 2**, apagar a linha depois de (`islandLineFade`) — **leva 2**, o que abre a linha
  (`islandLineClick`) — **leva 2**, zona da borda tão larga quanto a pílula (`islandZoneAsPill`) — **leva 2**.
- Abrir painéis só passando o mouse (`islandHoverOpen`, **novo**) — **leva 2**.
- Atalho para minimizar (`islandMinimizeKey`, Win+J; **novo**) — **leva 2**.

**Animações**
- Nível: completas, reduzidas, nenhuma (`kChoiceAnimationLevel`); "seguir o Windows" é o padrão.

### Grupo "Módulos"

Cada módulo tem, no topo da aba, um interruptor "Mostrar <módulo>". Desligado, o módulo some da
navegação da ilha e da barra, e o serviço por trás dele para (não gasta CPU nem rede).

**IA e agentes**
- Claude Code (`ai_claude_enabled` ✓), Codex (`ai_codex_enabled` ✓), OpenCode Go (chave
  `opencode_api_key` ✓, campo escondido com "mostrar").
- Cotas: avisar em 80%/95% e "de volta!" (hoje fixos).
- Card de permissão: instalar/remover os hooks (`--install-hooks` / `--uninstall-hooks`) por botão,
  com o estado atual.

**Central de controle**
- Quais botões aparecem e em que ordem (chips; `pill*` do mac-island, só os dez que a Cimori tem:
  Wi-Fi, Bluetooth, Não Perturbe, Luz noturna, Projetar, Transmitir, Legendas, Acessibilidade,
  Gerenciador, Explorador).
- Volume por aplicativo (mostrar ou não), saídas de áudio, botão do microfone (`islandMicrophone`) — **leva 2**.
- A roda do mouse na ilha muda o volume (hoje sempre).
- Porcentagem do volume ao lado do ícone (`islandVolumePercent`) — **leva 2**.

**Agenda**
- Endereço da agenda (`calendar_ics_url` ✓, campo de texto; é segredo: escondido com "mostrar").
- Feriados (`calendarHolidays`) e país dos feriados (hoje BR fixo).
- Dia grande (`islandCalendarBigDay`) — **leva 2** e eventos no calendário (`islandCalendarEvents`) — **leva 2**.
- Lembretes (`agenda_reminders` ✓) e antecedência (`agenda_reminder_minutes` ✓).

**Clima**
- Cidade: campo de busca por nome (geocoding do Open-Meteo, **novo**), que preenche `weather` ✓.
- Unidade °C/°F (`weather_fahrenheit` ✓).
- Detalhes em cartões (`islandWeatherCards`) — **leva 2**.

**Mídia**
- Letras (`islandLyrics`; LRCLIB, acessa a internet), tamanho e cor da letra — **leva 2**.
- Título longo rolando (`islandMediaMarquee`; o marquee ainda não existe: **novo**) — **leva 2**.
- A roda do mouse sobre a música troca de faixa em vez do volume (`islandMediaWheel`, **novo**) — **leva 2**.
- Só tocar e pausar (`islandMediaPlayOnly`) — **leva 2**.

**Avisos**
- Banners na pílula (`bannerEnabled`), mesmo minimizada (`bannerMinimized`) — **leva 2**, tempo na tela
  (hoje 6 s fixos), tamanho e largura (`islandBannerSize/Width`) — **leva 2**.
- Ícones dos apps (os do polimento Cimori).
- Avisos por página (hoje 4).
- Apps ignorados: lista dos apps que já mandaram aviso, cada um com interruptor (não vai para a ilha).
- Esconder os banners do Windows: link que explica o caminho já validado (desligar "Mostrar faixas de
  notificação" por app no Windows) e abre a página certa. A janela não grava esse registro por conta
  própria.

**Foco e timers**
- Pomodoro: foco e pausa (`pomodoro_focus_secs` ✓, `pomodoro_break_secs` ✓).
- Timer padrão (`timer_default_secs` ✓).
- Timer na linha minimizada (`islandTimerLine`) — **leva 2**.

**Bandeja**
- Mostrar os ícones dos outros apps (`islandTray`).
- Fixados na pílula (`pinned_tray_apps` ✓): lista com os ícones reais, arrastar para fixar/desfixar e
  ordenar.
- Estilo dos não fixados (`islandTrayStyle`: cartões com nome ou grade só de ícones; a grade é **nova**
  e o ROADMAP já a cita) — **leva 2**.
- Manter a lista aberta (`trayKeepOpen`).
- Estado da ponte do Windhawk (ligada ou não, com link para `windhawk/README.md`).

### Grupo "Cimori"

**Atalhos:** a lista dos atalhos da Cimori (Win+N, minimizar), cada um com liga/desliga e, quando der,
outra combinação (`m_shortcutChips`).

**Backup:** exportar e importar o `settings.json` (com os arquivos ao lado: lembretes, avisos limpos),
e "restaurar o padrão" com confirmação.

**Sobre:** versão (`--version`), créditos ao isle e ao mac-island, licença GPL-3.0, link do repositório.

## Decisões técnicas (minhas, como construtor)

- **Mesmo renderizador:** a janela é desenhada com `isle-render` (tiny-skia, Inter, ícones vetoriais),
  numa janela Win32 normal, na mesma thread de interface da ilha. Nada de WebView, nada de dependência
  nova. A prévia chama o mesmo desenho da barra compacta.
- **Uma fonte da verdade:** a janela edita o `IsleConfig` em memória do `IsleApp`; cada mudança aplica
  e salva. Salvar grava num arquivo temporário e troca (nunca deixa o `settings.json` pela metade).
  Campos novos entram com `#[serde(default)]` e o padrão de hoje, como já é a regra do arquivo.
- **Lista declarada uma vez:** as opções ficam numa tabela (aba, cartão, tipo de linha, nome,
  descrição, palavras da busca, leitura e escrita no `IsleConfig`), como `kIslandPrefs` do mac-island.
  A janela, a busca e os testes leem dessa tabela.
- **Segunda instância:** `--settings` com a Cimori rodando manda uma mensagem à janela da instância
  aberta (achada pelo nome da classe) e sai.
- **Testes:** cada opção da tabela tem um teste que a liga/desliga e confere o efeito no estado ou no
  `IsleConfig` salvo; prévias PNG da janela (100% e 150%) pelo exemplo `preview`, como o resto da
  interface.

## O que fica para o construtor

- Medidas, espaçamentos, ícones de cada aba e a animação das linhas (seguir o mockup e o tema).
- Ordem das linhas dentro de cada cartão.
- Como dividir a primeira entrega em etapas (sugestão: janela + tabela + abas que só leem
  `IsleConfig` ✓ primeiro; depois a Barra arrastável; por fim as outras **novo** sem "leva 2").
- Se uma opção sem marca se revelar sem código ao planejar, ela vira **leva 2** (não se constrói
  comportamento novo sem estar listado).

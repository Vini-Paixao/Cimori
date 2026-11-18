# Cimori — refinamento visual completo

Data: 11/11/2026. Estado: integrado ao aplicativo nativo para validação local do usuário.
O pedido posterior de executar a versão nova no monitor autorizou a integração.
Evidência de build, instalação e pendências em [VISUAL-VALIDATION.md](../../VISUAL-VALIDATION.md).

## Intenção e direção escolhida

O produto já reúne funções de uso diário. A próxima entrega deve dar à ilha acabamento,
clareza e personalidade em todos os módulos, com um avatar de IA inspirado no Isle.
O usuário escolheu **Cimori com a linguagem do Isle e prévia visual** nesta conversa.

Prancha interativa: [cimori-visual.html](../../design/cimori-visual.html).
É um protótipo visual com dados fictícios, sem integrações; não comprova comportamento do aplicativo.
Algumas ações secundárias e variações de conteúdo foram omitidas na prancha para estudar a
composição. Isso não autoriza removê-las do produto: todos os atalhos existentes de Controle,
ações de avisos, controles de ajuste e navegação do histórico devem permanecer acessíveis.

## Abordagens consideradas

| Caminho | Ganho | Custo ou compromisso |
| --- | --- | --- |
| **Cimori + linguagem do Isle, escolhido** | Identidade própria, hierarquia clara, personagem expressivo e detalhes de profundidade | Construir componentes vetoriais e revisar desenho e interação juntos |
| Reprodução mais fiel do Isle | Proximidade direta à referência GNOME | Menor presença da identidade Cimori e adaptações inevitáveis à interface Windows |
| Vidro como aparência principal | Ênfase na transparência do mac-island | Exige composição/blur real de janelas e depende mais do fundo e da plataforma |

O vidro não é pré-requisito desta entrega. Preservar a opção de aparência existente;
se for apenas transparência, não apresentá-la como vidro com desfoque.

## Evidência do estado anterior à integração

- `isle-render` desenha com tiny-skia e Inter; não há HTML no produto. A prancha não será embarcada.
- `renderer.rs` centraliza superfícies, texto, desenho da barra, banners, permissões e parte dos painéis.
  `weather_view.rs`, `timer_view.rs` e `chat_render.rs` contêm layouts específicos.
- `pages.rs` e `chat.rs` calculam áreas de clique; `window.rs` recebe entrada e escala por monitor.
  `src/app.rs` escolhe tamanhos e dirige as molas, serviços e atualização da interface.
- O renderizador usa azul `#0A84FF`, fundo `#1C1C1E` e cartões `#2C2C2E`.
  A identidade Cimori já define damasco, grafite, porcelana e Inter.
- A navegação tem oito abas textuais comprimidas; Shelf aparece mesmo com conteúdo "Em breve".
  Bandeja abre por acesso separado e usa cartões com nome.
- O chat vazio apresenta informações técnicas; não tem avatar. Cotas usam duas colunas.
- Mídia tem título, artista, progresso, letra e transporte. `MediaInfo` ainda não contém capa.
- Existe `AppearanceMode::Glass`, mas o desenho atual aplica alfa; isso não prova blur do desktop.
- As prévias atuais foram geradas pelo exemplo existente em `target/visual-audit/current/`
  e inspecionadas visualmente. Não são capturas da ilha rodando no desktop.
- O usuário relata funções validadas. Há pendências históricas de validação real no
  `PHASE7-VALIDATION.md`; não reinterpretar essa documentação como prova nova de falha ou sucesso.

## Metas

1. Uma família visual coesa: pílula compacta, expansão, banners, permissões e cada módulo.
2. Hierarquia legível, menos ruído, alinhamentos consistentes e ações fáceis de reconhecer.
3. Avatar vetorial próprio, nítido em qualquer DPI, com expressões ligadas ao estado real da IA.
4. Refinamento de hover, pressionado, foco, desabilitado, carregamento, vazio e erro.
5. Manter integrações, atalhos, filas, persistência e comportamento já disponíveis.
6. Manter o loop sem atualização contínua em repouso.
7. Exibir capas reais do conteúdo em reprodução, obtidas da sessão de mídia do Windows,
   como no mac-island; usar logos de Claude e OpenAI/Codex nas cotas e ícones nos controles compactos.

## Fora desta entrega

- Novos serviços de voz, chamadas, gravação, clipboard, Shelf, compartilhamento, USB e downloads.
- Novo instalador, publicação, redesign do logo ou migração de framework/renderizador.
- Tela completa de configurações. A aparência definida aqui tem bons padrões; preferências novas
  estritamente necessárias ao avatar/movimento podem entrar no esquema existente, compatíveis com arquivos antigos.
- Histórico persistente de chat e mudanças no protocolo dos provedores.

## O que seria um fracasso

- Apenas trocar o azul por damasco, mantendo a mesma falta de hierarquia.
- Parecer um dashboard dentro da pílula: cartões para todo texto, muitas bordas e brilhos constantes.
- Usar emoji como ícone de interface, desfocar imagens pequenas ou mudar a marca para um rosto.
- Perder cliques, cortar comandos de permissão, esconder erros ou inventar estados de serviço.
- Avatar em animação permanente, novo polling ou rede disparada apenas por efeito visual.
- Considerar a prancha HTML como implementação ou as prévias como validação ao vivo.

## Sistema visual proposto

Estas medidas e decisões são propostas do assistente, não escolhas detalhadas já aprovadas pelo usuário.

### Cor, material e tipografia

- Casca quase preta `#17191D`, superfícies grafite `#20232A` e elevação `#2A2E36`.
- Texto principal porcelana `#F5F1EB`, secundário névoa `#A7ADB7`.
- Damasco `#F18B63` em ação principal, seleção e personagem; texto carvão sobre botão damasco.
- Sucesso, alerta e erro usam verde, âmbar e vermelho próprios, acompanhados de texto/ícone.
- Borda discreta de 1 px lógico, luz superior sutil e sombra externa suave. Nada de brilho geral permanente.
  Superfície principal opaca para contraste previsível; nenhum blur simulado apresentado como blur real.
- Inter: corpo 13–14, metadados 11–12, título 16–18, números protagonistas 36–56 px lógicos.
  Pesos 400/500/600 já existentes. Relógios/contadores com largura estável para evitar saltos.
- Grade de 4 px; espaçamento interno 20–24, entre grupos 16–20, entre itens 8–12.
  Raios: controles 10–12, grupos 16–18, ilha expandida 26–30.

### Geometria e navegação

- A pílula continua flutuante, com a posição, auto-hide e configuração de monitores existentes.
- Data/hora e clima seguem reconhecíveis; música, contagem, avisos e bandeja ocupam regiões próprias.
  Quando falta espaço, reduzir conteúdo secundário conforme a prioridade atual; preservar clima e contador.
  Avatar compacto só aparece quando há atividade de IA, sem adicionar um rosto permanente ao relógio.
- Cabeçalho do painel: relógio/contexto à esquerda, ações pertinentes à direita;
  navegação vetorial em linha abaixo. Aba selecionada mostra nome e destaque; demais têm ícone e tooltip.
- Acesso a IA, Controle, Agenda, Clima, Mídia, Avisos, Foco e Bandeja. Foco reúne os modos já existentes.
  Shelf não entra na navegação normal enquanto não houver implementação; documentar essa alteração visual.
- Bandeja preserva o acesso pela barra e também aparece na navegação, reutilizando a ação existente.
- Dimensões iniciais para implementação: IA 560×420; Controle 600×440; Agenda 560×420;
  Clima 620×440; Mídia 560×340; Avisos 560×420; Foco 560×420; Bandeja 440×340.
  O construtor pode ajustar esses valores para legibilidade e conteúdo real, registrando o resultado.
- O tamanho efetivo respeita a área útil do monitor em unidades lógicas, com margem mínima de 16 px.
  Em altura insuficiente, rolar o corpo do painel, preservando navegação e ação principal;
  nunca encolher tipografia para caber. Tamanhos, desenho e hitboxes vêm da mesma geometria.
- Entrada por mouse, roda e atalhos já existentes continua funcional. Não prometer semântica
  de acessibilidade de controles nativos onde o renderizador próprio ainda não a fornece.

### Componentes e estados de interação

Conjunto compartilhado de ícones vetoriais, botões, seletor segmentado, slider, campo,
linha de lista, barra de cota, badge, tooltip e estado vazio.
Ícones de 16/20/24 px com espessura óptica consistente, sem dependência de glifos ausentes.
Microfone ligado/mudo, volume, transporte de mídia, enviar/parar, nova conversa, fechar,
limpar e navegação de páginas usam ícones quando a ação for reconhecível. Cada botão mantém
tooltip, nome acessível quando suportado e área de clique adequada. Não substituir títulos,
nomes de dispositivo/aplicativo, conteúdo de mensagens ou decisões Permitir/Sempre/Negar por
símbolos ambíguos. Microfone mudo tem traço diagonal e indicação semântica além da cor;
indisponível fica desabilitado. Texto contextual pode acompanhar o ícone quando necessário.

Nas cotas, Claude usa seu símbolo próprio e Codex usa o símbolo OpenAI acompanhado do nome
**Codex**, seguindo `gnome-extension/src/pages/usage.js` do Isle. Não usar a estrela genérica
da Cimori para identificar os provedores. Preservar os caminhos e cores dos SVGs de referência,
sem redesenhar marcas de terceiros; os nomes permanecem legíveis junto aos logos.

Hover clareia a superfície; pressionado escurece ou reduz o contraste de elevação;
foco de teclado recebe contorno visível; desabilitado mantém legibilidade e não reage.
Alvos principais têm 32–40 px e separação adequada. Ícone sozinho exige tooltip e nome acessível
quando houver suporte na plataforma. Campos indicam foco e caret apenas quando o teclado é capturado.
Erros permanecem visíveis e explicam a ação possível; carregamento não esconde o último dado válido.

## Avatar da IA

- Personagem independente do símbolo Cimori: um fantasminha pequeno e arredondado, duas pupilas,
  base com três ondas suaves, volume em porcelana e reflexo damasco. Não desenhar olhos no logo.
- Desenho por paths/gradientes tiny-skia, sem GIF, vídeo, WebView ou geração de imagens em runtime.
  48–64 px no painel, 18–22 px na barra quando houver atividade.
- Estados: pronto (olhar neutro), processando (olhar concentrado), respondendo (expressão aberta),
  concluído (alegre), precisa de atenção (sobrancelha/ícone) e erro (expressão serena + mensagem clara).
- `busy`, chegada de deltas, conclusão, cancelamento, erro e permissão pendente determinam os estados.
  O avatar não sugere escuta ou acesso ao microfone: voz não faz parte da interface implementada.
- Olhar acompanha o ponteiro de forma limitada apenas enquanto ele está sobre a ilha aberta.
  Atualizar somente com eventos de movimento; não manter timer para isso.
- Entrada, troca de expressão e conclusão podem animar por 160–260 ms. Processando tem expressão
  estática e texto persistente; uma animação curta de entrada basta. Sem loop de respiração ou piscadas.
- Movimento reduzido desativa deslocamentos, olhar e brilhos animados. Ler a preferência de animações
  do Windows e permitir a configuração equivalente; cor/forma/texto continuam comunicando o estado.
- Um brilho damasco breve pode acompanhar uma nova atividade; permissões e erros recebem
  semântica própria e nunca dependem só do brilho. Repouso estático.

## Layout por módulo

| Área | Refinamento | Comportamento preservado e limites |
| --- | --- | --- |
| Compacta e minimizada | Zonas claras, alinhamento óptico, separadores leves, estado ativo legível | Prioridades, auto-hide, fixados, hora, clima e contadores |
| IA / conversa | Avatar como ponto de atenção; saudação curta; seletor de provedor discreto; mensagens com autoria e espaço; compositor destacado | Streaming, seleção dos três provedores, nova, parar, histórico, erro e captura de teclado |
| IA / cotas | Cards por provedor com logo Claude ou OpenAI/Codex junto ao nome, barras finas, percentual e reset alinhados; nota de dado antigo perto do valor | Dados reais e avisos existentes; fallback textual apenas se o asset falhar, sem refazer marcas de terceiros |
| Controle | Conexões em grupo, ações secundárias em grade compacta; volume principal maior; saídas e mixer com linhas claras | Mesmas ações e indisponibilidades; evitar afirmar que atalho de configurações é um toggle real |
| Agenda | Dia protagonista + mês legível; eventos em lista com horário e marca lateral; seleção e hoje distintos | Mês, feriados, dia selecionado, eventos e lembretes |
| Clima | Temperatura e céu protagonistas; dados secundários em linhas leves; faixa de sete dias; curva com mais respiro | Cidade, oito métricas, sete dias, seleção, hora, unidades, dados antigos e erro |
| Mídia | Capa real com cantos suaves no painel e miniatura na barra; título/artista, progresso e transporte mais fortes; letra destacada; fallback vetorial quando não há imagem | Tocar/pausar, anterior/próxima, posição e letra; imagem pertence à sessão/faixa atual |
| Avisos | App/horário discretos, título forte, corpo legível; ações de resposta/dismiss com posição estável | Contagem, páginas, agrupamento, abrir, responder e limpar; texto digitado não se perde na repintura |
| Foco | Anel grande; seletor Pomodoro/Timer/Cronômetro/Lembretes; ajustes menores e ação principal forte | Contagens simultâneas, persistência de ajustes, fila e lembretes |
| Bandeja | Grade de ícones reais com células confortáveis; tooltip com nome e fixado; fallback claro | Clique esquerdo, menu real no direito, paginação e integração opcional com ponte Windhawk |
| Banners | Ícone contextual, título e texto com hierarquia; conclusão da IA pode mostrar o avatar pequeno | Prazo, fila, espera por Não Perturbe e clique de origem |
| Permissões | Agente identificado, comando em bloco distinto e ações inequívocas | Todo comando deve estar visível; manter `command_fits`, resposta terminal e mesmas decisões. Nenhuma ação por hover |
| Vazios e falhas | Ilustração vetorial pequena + frase útil + ação já existente quando pertinente | Não acrescentar integrações, botões sem função ou texto técnico desnecessário |

## Arquitetura de implementação e restrições

- Manter Rust, tiny-skia, ab_glyph/Inter e Win32. Sem novas bibliotecas por conveniência visual.
- Extrair tokens, ícones, componentes e geometria compartilhados; separar views para evitar
  ampliar indefinidamente `renderer.rs`. Planejar arquivos exatos depois do aceite.
- Estender estado de apresentação e entrada apenas para hover/foco/pressionado/expressão;
  não misturar animação com estado dos serviços ou protocolos de IA.
- Preservar as regras de segurança e a precedência de hitboxes das permissões.
- Recalcular limites da janela/canvas e interação em conjunto; não aumentar apenas a imagem do painel.
- Configurações antigas carregam com os mesmos valores, salvo campos novos com default.
  Se o fundo foi explicitamente configurado pelo usuário, preservá-lo; a nova paleta define o padrão
  e seus tokens de superfície, sem sobrescrever arquivos de preferência para forçar o tema.
- Qualquer mudança que precise de reinício da ilha para validação deve preservar processos de agentes.
- Sem instalar mods, alterar a ponte Windhawk, publicar, fazer merge ou modificar credenciais nesta etapa.

### Capas reais: integração incluída pelo usuário nesta revisão

Esta revisão amplia explicitamente o escopo anterior: capas deixam de ser uma tarefa futura.
O caminho foi conferido no `MediaThreadProc` do mac-island v3.1.1: `properties.Thumbnail()` →
`OpenReadAsync()` → leitura dos bytes (até 8 MiB), com releitura breve após troca da faixa.
O serviço Rust já recebe `TryGetMediaPropertiesAsync`, mas ainda ignora `Thumbnail`.

- Estender `MediaStatus` → evento de mídia → `MediaInfo` para transportar a imagem da sessão
  atual e uma identidade/revisão da capa. Não buscar capas por título em serviços externos.
- Ler/decode fora da thread de UI. Usar Windows Imaging Component para PNG/JPEG e formatos
  suportados pelo Windows; normalizar em pixels RGBA com dimensões explícitas. A crate `windows`
  já existe; habilitar as features necessárias sem introduzir framework de imagens.
- As novas operações WinRT de abertura/leitura têm prazo e cancelamento cooperativo; não copiar
  `.get()` sem prazo do fluxo atual para a leitura de capa. Cancelar na troca de faixa e no shutdown,
  limitando cada tentativa a dois segundos e descartando qualquer resposta após expirar.
- Limitar o stream a 8 MiB, recusar dimensões inválidas ou acima de 4096 por eixo/16 megapixels
  antes de alocar saída grande e reduzir a imagem para no máximo 512×512 preservando proporção.
  Reutilizar imagem/cache entre quadros; não decodificar nem copiar o buffer a cada frame.
- Painel mostra capa de aproximadamente 96×96; barra compacta, 22–24 px se a região de mídia
  estiver visível. Usar recorte central com proporção preservada, máscara de cantos e redução suave;
  não esticar capas retangulares. Manter o título/artista e as prioridades de espaço existentes.
- Ao trocar app/faixa (app, título, artista e álbum compõem a identidade), limpar a capa anterior
  imediatamente. Resultados assíncronos antigos não podem sobrescrever a faixa nova.
- Algumas fontes entregam primeiro o título e depois a imagem. Reagir a eventos de metadados e
  fazer no máximo quatro tentativas adicionais dentro de seis segundos após a troca, em worker,
  canceladas por nova faixa, encerramento de sessão ou shutdown. Depois disso, aguardar novo evento;
  não criar polling permanente quando a capa está ausente.
- Sem capa, erro de leitura, stream grande ou formato inválido: usar ilustração vetorial neutra,
  continuar mostrando os metadados e manter os comandos funcionais. Nada de capa da faixa anterior.
- Pausar preserva a capa atual. Encerrar a sessão limpa imagem e metadados.
- Na prancha, a capa de Bohemian Rhapsody é um exemplo estático obtido do catálogo Apple/iTunes;
  isso não cria dependência desse serviço no produto e não é leitura da reprodução do usuário.

## Sequência e critérios de aceite

Depois da aprovação, produzir plano de implementação com lotes verificáveis:
fundação visual/geometria → casca/navegação → IA/avatar → painéis → estados/movimento → validação.
O alvo final é todos os módulos desta tabela, não só os primeiros lotes.

1. Prévias antes/depois de cada módulo e de seus vazios, erros e estados relevantes,
   a 100%, 125%, 150% e 200%, com textos longos e dados ausentes.
2. Compilação e testes apropriados do workspace. Testar geometria compartilhada e ações,
   prioridade dos cliques, permissões longas, input e estados; evitar testes que só repetem cores.
3. Inspeção visual de recorte, contraste, legibilidade e alinhamento sobre fundos claros e escuros.
   Medir contraste dos pares efetivos; mínimo 4,5:1 para texto comum e 3:1 para controles relevantes.
4. Validar ao vivo no Windows: cliques, menus da bandeja, sliders, teclado, scroll, troca de abas,
   expansão/recolhimento, monitores e DPIs reais disponíveis. Registrar escalas não disponíveis.
5. Conferir o comando completo no card e manter a recusa/fallback para comandos que não cabem.
6. Conferir timers simultâneos, alertas em fila, notificações/respostas e streaming/cancelamento
   dos provedores disponíveis sem mudar contratos de serviço.
7. Medir CPU/memória em repouso antes/depois, mesma condição, e verificar que não há deadline de
   animação quando a interface está parada. Interação não justifica polling permanente.
8. Conferir movimento reduzido. Não declarar acessibilidade completa por ter apenas foco desenhado.
9. Entrega separa arquivos, prévias, testes e validações ao vivo, com pendências explícitas;
   atualizar docs e roadmap para o estado verificado. Nenhum sucesso de runtime inferido de PNG.
10. Capas: PNG/JPEG, dimensões/bytes inválidos, ausência, atraso, troca rápida de faixa/app,
    pausa e fim de sessão. Validar com os players disponíveis no Windows, incluindo navegador,
    e registrar quando o próprio app não fornece imagem. Garantir ausência de polling após as tentativas.
11. Ícones: microfone ligado/mudo/indisponível, tooltips e foco; logos dos dois provedores em
    dados presentes/ausentes/antigos e nas escalas de DPI previstas.

## Referências

- [Isle, fonte e documentação](https://github.com/zickrian/isle), imagens `docs/images/voice.png`,
  `bot-looks.png`, `music.png`, `calendar.png`, `glass.png` e `gnome-extension/stylesheet.css`,
  consultadas em 11/11/2026. Inspiram contenção, hierarquia e expressividade do personagem.
- [Mac Island & Dock](https://windhawk.net/mods/mac-island-dock), fonte v3.1.1 de
  [caliberda](https://github.com/ramensoftware/windhawk-mods/blob/main/mods/mac-island-dock.wh.cpp),
  consultada em 11/11/2026. Referência para densidade funcional, controles e bandeja no Windows.
- [Guia da identidade Cimori](../../../assets/brand/cimori/GUIA.md) e `tokens.json` existentes.
  O personagem não altera os arquivos da marca.
- [Propriedade Thumbnail da sessão de mídia Windows](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmediaproperties.thumbnail),
  consultada em 11/11/2026; fornece a referência de stream da imagem do conteúdo.
- Logos copiados de `gnome-extension/icons/isle-claude.svg` e `isle-openai.svg` do Isle;
  os próprios assets identificam a origem Simple Icons/CC0. Proveniência da capa demonstrativa
  em `docs/design/cimori-visual-assets/SOURCES.md`.

## O que fica com o construtor

Organização exata dos arquivos, paths de ícones/personagem, caches de rasterização,
curvas de animação, divisão em commits e pequenos ajustes de medidas dentro dos critérios acima.
Decisões de produto que ampliem funcionalidades voltam ao usuário.

## Revisão do construtor

Revisão independente somente leitura concluída em 11/11/2026: nenhuma pergunta importante
pendente para planejar a implementação. A revisão reforçou a preservação das ações secundárias
que a prancha simplifica, incorporada acima. O aceite visual do usuário continua pendente.
Revisão adicional de capas, ícones e logos concluída em 11/11/2026: nenhuma lacuna importante.
O revisor destacou o cancelamento de operações WinRT em andamento, explicitado acima.
Integração nativa concluída depois desta revisão de design, com revisão independente de código e
correção dos três achados importantes. O aceite visual no monitor continua com o usuário.

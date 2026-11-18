# Cimori — direção de identidade, rodada 1

Data: 11/11/2026. Nome **Cimori escolhido pelo usuário** nesta conversa.
Registro da exploração: o usuário escolheu depois **A — Coroa suspensa** para
finalizar o kit. Abaixo, a pesquisa e as alternativas permanecem como histórico.
Guia de entrega: [IDENTIDADE-CIMORI.md](IDENTIDADE-CIMORI.md).

## O que o repositório revela

O produto é uma central nativa para Windows que ocupa o topo da tela, reunindo
agentes de IA e recursos do desktop. A síntese da marca é **um ponto de encontro
no alto da tela**. A origem do nome proposto é uma criação a partir de “cimo”; não
é uma tradução nem uma etimologia estabelecida.

Fontes locais analisadas:

- `docs/ROADMAP.md`: documento atual; identifica as duas bases, separa fases
  concluídas de pendências e substitui a especificação inicial.
- `crates/isle-core/src/state.rs`: estados compacto/expandido, páginas e agentes.
- `crates/isle-core/src/config.rs`: pílula escura, múltiplos monitores e integração.
- `crates/isle-render/src/renderer.rs`: formas arredondadas, superfícies escuras,
  azul de ação e cores de estado.
- `crates/isle-render/src/text.rs`: Inter embutida e já disponível no projeto.
- `crates/isle-platform-win/src/tray.rs`: atalhos de apps fixados; esse módulo
  não constitui um ícone próprio da marca na área de notificação.
- `crates/isle-render/src/window.rs`: a classe de janela usa os valores padrão
  de ícone. A raiz do repo ainda não tem recurso `.ico` nem `build.rs` próprio.

Mídia, agenda, notificações, controles e IA constituem o território do produto;
esta análise de identidade não comprova que todas as funções planejadas estejam
concluídas. O documento histórico de design foi lido como contexto, não como
estado atual nem como prova de desempenho.

## Posicionamento e personalidade

**Proposta:** sua central, no alto da tela.

**Assinatura proposta:** “Tudo perto. Espaço para você.”

**Descritor:** central de controle e agentes para Windows.

Precisão, discrição, prontidão, leveza e proximidade. A identidade deve servir
às pessoas que usam o Windows diariamente, inclusive quem trabalha com agentes,
sem depender da estética de uma ferramenta de IA ou de uma cópia visual do macOS.

O símbolo identifica o aplicativo inteiro. Não deve tentar ilustrar cada recurso.
O nome público pode mudar para Cimori sem renomear imediatamente crates, caminhos
de configuração, hooks, classes Win32 ou executáveis.

## Pesquisa e diferenciação

As referências fornecidas foram consultadas:

- [Isle, de zickrian](https://github.com/zickrian/isle): referência funcional de
  ilha no topo e integração de recursos. A forma visual da marca será própria.
- [Mac Island & Dock, de caliberda](https://windhawk.net/mods/mac-island-dock):
  referência Windows; a página não retornou texto legível nesta consulta. A
  relação técnica está documentada no roadmap e nos créditos do código local.
- [Skill logo-design](https://github.com/kaankiziltug/logo-design-skill), instalada
  em `~/.codex/skills/logo-design`: biblioteca, processo e auditoria.
- [Microsoft: design de ícones](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-design):
  formas simples, silhueta legível, atenção aos tamanhos pequenos e fundos claros/escuros.

A biblioteca da skill reúne 81 referências classificadas em produtividade e
46 em plataformas. Azul e formas geométricas recorrentes são convenções fortes;
a paleta proposta se afasta do azul predominante sem depender da cor para funcionar.
Foram estudadas as construções de Framer, Pagekit e Rocky Linux, sem copiar trajetos.
Buscas por ilha e recorte não bastam para comprovar originalidade jurídica.

Clichês evitados: estrela de IA, cabeça de robô, balão de chat, quatro quadrados
do Windows, ilha com palmeira, cápsula simples de liga/desliga e gradiente azul-roxo.

A pesquisa inicial por nomes descartou opções com usos evidentes em software,
como [Nivra](https://nivraapp.github.io/nivra/index.html),
[Velto](https://velto.id/), [Cimora](https://cimora.fr/) e
[Olume](https://apps.apple.com/us/app/olume/id6816714568).
A busca inicial por “Cimori” não revelou um app desktop equivalente entre os
resultados consultados; há outros usos do termo. Não foi realizada pesquisa
completa de marcas, domínios, lojas ou imagens semelhantes, nem se afirma exclusividade.

## Exploração de ideias

Pontuação editorial de 1 a 5, não resultado de pesquisa com usuários.
Critérios: clareza, diferenciação, simplicidade, pertinência e leitura pequena.

| Ideia | Tipo | Pontuação /25 | Decisão |
| --- | --- | ---: | --- |
| Coroa suspensa: uma ilha paira sobre um corpo que acolhe | Abstrato | 23 | A, recomendada |
| Fresta: um C compacto abre uma faixa no horizonte | Letra / espaço negativo | 21 | B |
| Confluência: duas massas deixam um espaço comum entre elas | Abstrato | 21 | C |
| Um farol reduzido a feixe e base | Pictórico | 16 | Depende de detalhes |
| Monograma CM encaixado numa cápsula | Monograma | 15 | Perde legibilidade pequena |
| Um arco com ponto de presença | Abstrato | 17 | Muito próximo de radar/conectividade |
| Uma dobra que se abre em painel | Abstrato | 18 | Tende a pasta/documento |
| Um cursor recolhendo sinais | Pictórico | 15 | Faz parecer launcher ou automação |
| Três trilhos convergindo no topo | Abstrato | 16 | Parece equalizador/métrica |
| Um horizonte cortado por pequena ilha | Pictórico / abstrato | 18 | Paisagem genérica |

### A — Coroa suspensa, recomendada

Uma faixa flutua acima de uma base aberta: o topo da tela e um lugar que reúne
o que importa, em duas peças. O capuz horizontal pode originar recortes,
alinhamentos e movimento da identidade. A forma funciona sem a inicial do nome.

Risco: pode lembrar um sorriso ou recipiente. A separação entre faixa e base
é parte essencial do desenho; não conectar como uma alça nem acrescentar olhos.

### B — Fresta

Uma massa arredondada com uma abertura horizontal deslocada. Evoca C e uma
abertura de acesso rápido. Tem a leitura mais compacta e exige pouco espaço.

Risco: a simplicidade aproxima o desenho de carteira, C/G e outras marcas de software.
É a direção menos forte em diferenciação e não representa um C inequívoco.

### C — Confluência

Duas formas opostas delimitam um corredor central: sistema e agentes se encontram.
É a direção mais abstrata e dinâmica.

Risco: pode lembrar S, intercâmbio ou sincronização; a relação com o topo é menos
imediata. O canal precisa permanecer aberto em tamanhos pequenos.

## Proposta de identidade visual

Ainda não é um manual aprovado. A proposta pode acompanhar A ou ser adaptada
à direção escolhida.

| Papel | Nome | HEX | Uso |
| --- | --- | --- | --- |
| Marca | Damasco | `#F18B63` | Símbolo, destaques de comunicação e seleção |
| Base | Grafite | `#20232A` | Suporte do ícone e superfícies escuras |
| Claro | Porcelana | `#F5F1EB` | Fundos editoriais e texto sobre grafite |
| Texto | Carvão | `#17191D` | Texto e versão monocromática sobre claro |
| Apoio | Névoa | `#A7ADB7` | Informação secundária sobre grafite |

O damasco não é adequado para texto pequeno sobre porcelana; usar carvão.
Seu contraste sobre grafite deve ser medido no relatório de testes. Para ícone
no desktop claro, prever suporte grafite ou um desenho de dois valores; não usar
somente o símbolo damasco sobre qualquer wallpaper.

**Tipografia:** Inter, já distribuída pelo repo com licença SIL OFL.
Proposta de logotipo em minúsculas, `cimori`, peso 600, espaçamento revisado
opticamente. Textos de produto: Inter 400/500; títulos: 600. No kit final,
o logotipo será convertido em curvas; texto vivo de estudos não é master.

**Geometria:** massas simples e cantos suaves; os espaços vazios são tão
importantes quanto as formas. Derivar recortes e alinhamentos da faixa superior,
em vez de repetir o símbolo como decoração em todos os componentes.

**Movimento proposto:** entrada curta por aproximação vertical e acomodação suave;
preservar o comportamento por mola do produto. A marca permanece estática em
repouso; não usar pulsação contínua. Respeitar preferência por movimento reduzido
quando a animação for implementada. Esta rodada não altera animações do aplicativo.

**Voz:** frases curtas e humanas, ação explícita (“Abrir central”, “Agente precisa
de você”, “Tudo pronto”). Sem personificação robótica, jargão de infraestrutura
ou promessas de recursos ainda não implementados.

**Estados:** a cor de marca não substitui sucesso, atenção ou erro; manter cores
semânticas e acompanhar a cor com texto ou forma. O apoio cinza não deve ser usado
como única forma de distinguir um alerta.

## Escopo desta rodada e próxima entrega

Esta rodada entrega a imagem exploratória do image-gen, três reconstruções
vetoriais de estudo, comparação em tamanhos reais, auditoria e esta direção de
identidade. A imagem gerada é referência criativa; os SVGs são desenhos manuais
correspondentes, com ajustes para escala, não uma vetorização automática exata.

Depois da escolha de A, B ou C: refino final; logotipo em curvas; versões principal,
monocromática e invertida; ícone Windows `.ico` multirresolução; PNGs; variante
pequena para bandeja; guia compacto de uso e aplicações de contexto.

A etapa de escolher antes do kit está explícita na skill solicitada:
“Build the kit (Phase 7) only after they pick a direction and say yes.”
Assim, os estudos não são promovidos silenciosamente a marca final nem incorporados
ao executável nesta rodada. A integração futura exige recurso de ícone do binário,
caminho de build e eventual `hIcon`/`hIconSm`, além da verificação no Explorer,
atalhos e bandeja em DPI real. Criar um arquivo `.ico` por si só não altera o `.exe`.

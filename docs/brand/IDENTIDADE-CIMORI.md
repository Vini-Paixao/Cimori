# Cimori — identidade visual

Nome e direção **A — Coroa suspensa** escolhidos pelo usuário em 11/11/2026.
Paleta e tipografia desenvolvidas a partir dessa escolha. Arquivos em
[`assets/brand/cimori`](../../assets/brand/cimori/).

## A ideia

Uma faixa flutua sobre uma base aberta: um ponto de encontro no alto da tela.
A marca representa a central inteira — agentes, controles e informações do dia.
“Cimori” foi criado a partir de “cimo”, em referência ao topo; o usuário escolheu
esse nome na conversa. O símbolo pode ser reconhecido sem escrever o nome dentro
do ícone e funciona em uma só cor.

**Posicionamento:** sua central, no alto da tela.

**Assinatura de comunicação:** Tudo perto. Espaço para você.

**Personalidade:** discreta, precisa, pronta, leve e próxima.

## Arquivos e usos

| Arquivo/pasta | Uso |
| --- | --- |
| `windows/cimori.ico` | Ícone de app Windows, dez imagens de 16 a 256 px |
| `windows/cimori-tray-white-*.png` | Bandeja em superfície escura |
| `windows/cimori-tray-black-*.png` | Bandeja em superfície clara |
| `svg/cimori-app-icon.svg` | Master do ícone sobre grafite, cantos transparentes |
| `svg/cimori-app-icon-small.svg` | Ajuste óptico usado no `.ico` até 32 px |
| `svg/cimori-symbol-*.svg` | Símbolo isolado, cores no nome do arquivo |
| `svg/cimori-symbol-small-*.svg` | Símbolo para 16–32 px, abertura ampliada |
| `svg/cimori-horizontal-light.svg` | Símbolo e nome em carvão, sobre fundo claro |
| `svg/cimori-horizontal-dark.svg` | Símbolo damasco e nome porcelana, sobre grafite |
| `svg/cimori-horizontal-black.svg` / `white.svg` | Assinaturas monocromáticas |
| `svg/cimori-stacked-*.svg` | Composição vertical |
| `svg/cimori-wordmark-*.svg` | Nome sem símbolo, convertido em curvas |
| `png/` | Ícones 16–1024 px e assinaturas horizontais de 1400 px |
| `tokens.json` | Cores, tipografia, geometria e regras de movimento |
| `presentation/identidade.html` | Prancha e seis aplicações simuladas |

Os SVGs finais são vetoriais, com cores explícitas, nome convertido em curvas e
sem filtros, raster embutido, texto vivo ou dependência de uma fonte instalada.
O ícone usa o desenho com escala apropriada a cada tamanho; não foi ampliado de
um PNG pequeno. O `.ico` tem imagens PNG RGBA independentes de
16, 20, 24, 32, 40, 48, 64, 96, 128 e 256 px.

## Cor

| Nome | HEX | RGB | Papel |
| --- | --- | --- | --- |
| Damasco | `#F18B63` | 241, 139, 99 | Símbolo sobre grafite, seleção e comunicação |
| Grafite | `#20232A` | 32, 35, 42 | Ícone e superfícies escuras |
| Porcelana | `#F5F1EB` | 245, 241, 235 | Fundos editoriais e texto claro |
| Carvão | `#17191D` | 23, 25, 29 | Marca e texto sobre fundo claro |
| Névoa | `#A7ADB7` | 167, 173, 183 | Informação secundária sobre grafite |

Pares medidos: damasco/grafite **6,45:1**; porcelana/grafite **13,98:1**;
carvão/porcelana **15,64:1**; névoa/grafite **6,97:1**.
Damasco/porcelana tem apenas **2,17:1**: usar carvão no fundo claro e suporte
grafite para o ícone. A cor da marca não substitui os estados de sucesso, atenção
e erro. Acompanhar estados por texto/forma.

O kit é RGB para telas. Não há equivalência Pantone nem prova CMYK; para impressão,
pedir conversão e prova ao fornecedor a partir dos SVGs monocromáticos.

## Tipografia e composição

**Inter**, já embutida no aplicativo, pesos 400 e 500 para texto, 600 para títulos.
O logotipo em minúsculas usa Inter 600 com tamanho óptico 32 e ajustes no
espaçamento. Os arquivos finais já estão em curvas; não redigitar o logotipo.
Licença da fonte: SIL OFL 1.1, incluída em `LICENSE-Inter.txt`.

Reservar espaço livre de **uma altura da faixa superior** ao redor da assinatura:
40 unidades no desenho base de 256. Essa reserva se aplica à comunicação; o
master de ícone já tem margens próprias para o seu suporte.

Mínimos para tela: símbolo **16 px** com o desenho pequeno; assinatura horizontal
**112 px de largura**; assinatura vertical **96 px de largura**. Esses mínimos
são orientações desta entrega, não resultados de estudo de reconhecimento com usuários.

Preservar proporções, orientação e separação entre faixa e base. Usar as versões
fornecidas; evitar esticar, conectar as peças, desenhar olhos, girar, acrescentar
brilho/sombra ou refazer as cores. Sobre fotografia, usar o ícone com suporte
grafite ou a assinatura em área calma e de contraste suficiente.

## Sistema visual e voz

Superfícies simples, curvas suaves e bastante espaço. Usar a faixa horizontal
como origem de alinhamentos e pequenos recortes, sem transformar todo componente
em uma repetição do logo. Uma única cor de marca por composição é suficiente.

Movimento proposto: aproximação vertical curta, com acomodação suave em cerca
de 240 ms. Repouso estático, sem pulsação constante. Respeitar movimento reduzido.
São diretrizes de design; esta entrega não muda o comportamento da interface.

Voz direta e humana: “Abrir central”, “Seu agente terminou a tarefa”, “Tudo pronto”.
Descrições públicas devem refletir recursos efetivamente disponíveis.

## Produção e integração

Os arquivos estão prontos para uso como assets. O aplicativo em Rust continua com
seu nome técnico e seus caminhos existentes; o `.exe` não foi recompilado nem teve
o recurso de ícone incorporado nesta tarefa de identidade.

Para a integração futura: incorporar `windows/cimori.ico` como recurso `ICON` no
build do executável e associar `hIcon`/`hIconSm` quando apropriado. A bandeja deve
carregar o desenho claro/escuro conforme a superfície. Conferir Explorer, atalhos,
barra de tarefas e bandeja nos DPIs reais; arquivos no repo não equivalem a essas
validações. Nenhum instalador ou página pública foi publicado.

Reprodução dos arquivos:

```powershell
python -m pip install --target target/brand-tools fonttools==4.68.0
python scripts/build-brand.py --skill-dir "$env:USERPROFILE/.codex/skills/logo-design"
```

O image-gen nativo foi usado na exploração inicial; os símbolos finais foram
reconstruídos e refinados em vetor. A imagem original e o prompt estão em
`assets/brand/concepts/round-01/`. O logotipo tipográfico foi gerado da Inter
embutida com fontTools 4.68.0. As renderizações de entrega usam o renderizador da
skill instalada e Chromium, sem chamada paga por CLI/API de geração de imagem.

A busca inicial de nomes e a biblioteca de referências orientaram a criação;
não constituem liberação de marca, domínio ou exclusividade. A entrega não define
uma licença nova para a marca, e preserva os créditos e licenças do código de origem.

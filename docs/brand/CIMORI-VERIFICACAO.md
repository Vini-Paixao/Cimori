# Cimori — verificação da entrega

11/11/2026. Escopo: arquivos de identidade, sem alteração de código do aplicativo.

## Resultado

- **Nome:** Cimori, escolhido pelo usuário.
- **Símbolo:** A — Coroa suspensa, escolhido pelo usuário para finalizar o kit.
- **22 SVGs finais:** auditoria entre **99 e 100/100**, sem WARN/FAIL. A pontuação
  é heurística da skill, não certificação externa. O aviso informativo de centro
  óptico levemente acima do centro geométrico é intencional.
- Masters conferidos por XML: sem texto vivo, raster embutido, filtros ou máscaras.
- Logotipo em curvas derivado da Inter 600 / tamanho óptico 32, com espaçamento
  ajustado. Coordenadas de letras arredondadas a duas casas; canvases inteiros.
- **29 PNGs** de entrega, dimensões dos ícones verificadas e canal alfa carregado
  pela biblioteca nativa System.Drawing. O canto da imagem de 16 px pode ter alfa
  **1/255** por antialiasing subpixel; não há fundo opaco introduzido na exportação.
- `.ico`: diretório, offsets e dimensões dos dez PNGs internos verificados.
- **Windows `LoadImageW`: as dez resoluções carregaram com sucesso**, de 16 a 256 px.
- Renderização e inspeção visual das pranchas `presentation/identidade.png` e
  `presentation/producao.png`: assinaturas clara/escura, composição vertical,
  escala real 16–256 px e bandeja clara/escura. A forma continua separada a 16 px.
- Conceitos: duas rodadas de vetores auditadas e inspecionadas; A ganhou mais espaço
  entre faixa e base e C ganhou um corredor mais aberto. B teve a fresta ampliada.
- Contrastes calculados em luminância sRGB: damasco/grafite 6,45:1;
  porcelana/grafite 13,98:1; carvão/porcelana 15,64:1;
  névoa/grafite 6,97:1. Damasco/porcelana 2,17:1, combinação desaconselhada.

SHA-256 de `windows/cimori.ico`:

```text
9884C1A19C146AA586159DCA793A65F3808DBF9A778D5CA3929620712FE63345
```

## Arquivos de evidência

- `docs/brand/cimori-svg-audit.txt`: saída completa da auditoria final.
- `assets/brand/concepts/round-01/comparacao-v1.png` e `comparacao-v2.png`.
- `assets/brand/concepts/round-01/verificacao-v2.png`: inversão, espelhamento e rotação.
- `assets/brand/cimori/presentation/producao.png`: prova dos arquivos finais.
- `assets/brand/cimori/presentation/identidade.png`: apresentação com seis contextos.
- `assets/brand/concepts/round-01/imagegen-prompt.txt`: prompt exato da exploração.
- `assets/brand/concepts/round-01/imagegen-exploration.png`: imagem do image-gen nativo.

## Limites práticos

As seis aplicações de marca são simulações identificadas, não capturas da versão
integrada do aplicativo. O carregamento nativo do `.ico` comprova sua decodificação;
não substitui a inspeção de Explorer, atalhos, barra de tarefas e bandeja depois
da incorporação ao `.exe` e em diferentes monitores/DPI.

Não houve recompilação do app, instalação de um novo executável, alteração das
cores da interface ou renomeação dos crates, hooks e diretórios. As modificações
preexistentes em `src/app.rs`, `notification_windows.rs`, `notifications.rs` e
`.claude/` foram deixadas no estado encontrado. Nenhuma operação de commit foi feita.

A skill `logo-design` está instalada em
`~/.codex/skills/logo-design` e foi lida/aplicada nesta conversa;
fica disponível para descoberta automática no próximo turno. A criação combinou
o image-gen nativo para exploração, vetor manual para a forma, fontTools 4.68.0
para o nome em curvas e Chromium para PNGs. Não foi usada a CLI paga de image-gen.

A pesquisa inicial de referências e nomes não é pesquisa jurídica de marcas.

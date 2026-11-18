# Tela de configurações — validação

11/11/2026, ramo `feat/settings`. Design em
`docs/superpowers/specs/2026-11-11-configuracoes-design.md`, plano em
`docs/superpowers/plans/2026-11-11-configuracoes.md`.

## Já verificado (sem a ilha rodando)

- `cargo test --workspace --offline`: **392 testes, 0 falhas** (eram 308). Os novos cobrem:
  `settings.json` antigo, corrompido e fora da faixa; gravação atômica; relógio e data; a tabela de
  opções (ida e volta de cada uma, busca sem acento); a barra na ordem escolhida; módulos saindo da
  navegação; geometria da janela em três tamanhos sem sobreposição; desenho (interruptor ligado em
  damasco, prévia com a pílula real, segredo nunca desenhado, tudo opaco, 150%); posição salva num
  monitor que sumiu; abrir, editar, arrastar slider (grava só ao soltar), arrastar chips, último item que
  não sai, colar texto, Esc/Enter, Tab, clique direito; busca de cidade; avisos (banner desligado, tempo,
  por página, apps silenciados); roda; letras; cotas; restaurar; backup; servidor só local/Tailscale.
- `cargo clippy`: nenhum aviso nos arquivos novos.
- Prévias de todas as abas a 100% e 150% e da janela estreita (`cargo run -p isle-render --example
  preview -- <pasta> [escala]`, arquivos `settings-*.png`). Quatro guardadas em `docs/design/native/config-*.png`.
- Release: `dist/isle-win.exe`, 5.371.392 bytes, SHA-256
  `159f7cb475562f681d8cafab3a26da194b7a103ac5032c30652828ad4a0c057d`. **Não instalado** em
  `C:\Tools\isle-win` (a Cimori em uso não foi tocada).

## Checklist ao vivo (Marcus)

Em 12/11/2026, Marcus confirmou que os cenários que experimentou no Windows funcionaram sem bugs.
Os itens abaixo ficam como referência de regressão. Ver também [a validação da fase 8](PHASE8-VALIDATION.md).

Instalar: fechar a Cimori em uso, copiar `dist/isle-win.exe` para `C:\Tools\isle-win\`, abrir.

1. Abrir pelos três caminhos: engrenagem na ilha aberta, clique direito na pílula fechada,
   `isle-win.exe --settings` num terminal (com a Cimori rodando deve só trazer a janela; sem ela, abre
   a Cimori com a janela).
2. Clique direito num ícone da bandeja na pílula: continua abrindo o menu do app, não as configurações.
3. Cada aba: nada cortado, nada sobreposto, textos legíveis. Janela estreita (arrastar a borda até
   ~600 px): lateral só com ícones, busca no topo.
4. Barra: arrastar Relógio para o fim, tirar Bandeja arrastando para fora, pôr Data pelo "+";
   a pílula muda na hora. Tirar todos: o último fica.
5. Relógio: segundos (a pílula passa a contar segundo a segundo), 12 h, ano, mês em número.
6. Slider (ex.: Avisos → Tempo na tela): arrastar e soltar; o valor segue o mouse.
7. Campo secreto (Agenda → endereço .ics): mostra bolinhas; clicar, digitar, Ctrl+V, Enter grava, Esc desfaz.
8. Busca: digitar "volume" em qualquer lugar da janela; Esc limpa; outro Esc fecha a janela.
9. Clima → Cidade: buscar "Lisboa", escolher; a pílula mostra o clima novo em segundos.
10. Aparência → cor de destaque: sliders e anéis da ilha mudam na hora.
11. Módulos: desligar Clima (some da ilha e da barra), ligar de novo (volta sem reiniciar).
12. Posição e monitores (com dois monitores): clicar no mapa troca o principal; desligar "Em todos" e
    escolher as telas.
13. Avisos: desligar banners (aviso chega à lista sem banner); silenciar o WhatsApp na lista de apps.
14. Backup: exportar (diálogo do Windows), mudar algo, importar; Restaurar pede "Confirmar?".
15. IA → Hooks: o estado aparece; Instalar/Remover mudam o estado.
16. Bandeja → "Como instalar a ponte" abre a pasta com o README e o `.wh.cpp`.
17. Fechar e reabrir: mesma posição, tamanho e aba. Mover a janela para o 2º monitor, desligá-lo,
    reabrir: aparece no principal.
18. DPI: arrastar a janela entre monitores de escalas diferentes; nítida nos dois.
19. CPU parado com a janela **fechada**: 0% no Gerenciador de Tarefas. Com ela aberta e parada: 0%.
20. `Sair da Cimori` (Geral) fecha a ilha de verdade.

## Revisão final (fresca, sobre o ramo inteiro)

Corrigido com teste que falhava antes: opções que só valiam ao reabrir sem avisar (agenda, feriados,
país, chave do OpenCode, ícones dos avisos); serviços que continuavam com o módulo desligado (clima,
agenda, cotas, bandeja agora param); `settings.json` ilegível guardado como `settings.json.bad` antes
de ser regravado; espaço na busca; slider preso quando outra janela toma o mouse; posição salva com a
janela maximizada ou minimizada.

## Limites conhecidos

- Com um módulo desligado, os leitores de eventos do Windows de mídia, avisos e central continuam
  esperando eventos (sem rede e sem CPU parado); os eventos são ignorados.
- Porta do servidor, "só desta máquina e do Tailscale", Claude/Codex e Win+N valem ao reabrir (a linha diz).
- "Página do projeto" saiu de Sobre até o repositório ser público (fase 9).

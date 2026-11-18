# Cimori — Fase 9, trilha Claude (validação ao vivo)

Cada item abaixo foi coberto por testes automáticos (`cargo test --workspace`) e por prévias
(`cargo run -p isle-render --example preview -- <pasta> [1.5]`). Falta conferir no Windows, no uso real.

## T4 — Pílula mais larga e entalhe

1. Abrir a Cimori com um settings.json antigo: a pílula continua pílula, um pouco mais larga.
2. Aparência → Tamanho → "Largura da ilha fechada": arrastar de 160 a 560 px e ver a pílula seguir.
3. Aparência → Estilo → "Entalhe": a ilha encosta na borda de cima, com as curvas para fora nos dois lados.
   Conferir em 100% e 150% e num segundo monitor.
4. Levar o mouse até o entalhe: em ~¼ s ele abre e cresce até o painel; tirar o mouse fecha no tempo de
   recolhimento. Clicar no entalhe também abre.
5. Barra → a lista sob a prévia passa a ser "No entalhe": pôr Clima e Música, reordenar arrastando, tirar
   tudo menos o relógio. Só o relógio fica no meio.
6. Com "Esconder na linha" ligado, o entalhe vira a linha na borda (sem curvas) e volta ao passar o mouse.
7. Banners de aviso, volume e card de permissão saem do entalhe, presos à borda.
8. Voltar para "Pílula": distância do topo e itens da barra voltam como estavam.

## T5 — Botão de desligar e Configurações do Windows

1. Barra → pôr "Desligar" nos itens: o ícone aparece na pílula; clicar abre o menu com as seis ações
   (Hibernar só se o Windows permitir: `powercfg /a`).
2. Clicar em "Desligar": aparece "Desligar agora?" com Cancelar e Desligar. Cancelar fecha a pergunta;
   clicar de novo na ação também. Repetir com Reiniciar e Sair. **Só confirmar quando puder perder a sessão.**
3. Bloquear: tranca na hora. Suspender: dorme (ou só apaga a tela num PC com Modern Standby).
4. Central de controle → Botão de desligar: desligar "Perguntar antes" e conferir que Desligar não pergunta
   mais (cuidado). Deixar só uma ação ligada (Bloquear): o botão executa direto, sem menu.
5. Central de controle → "Botões da central": adicionar "Configurações" e "Desligar". Configurações abre
   o app Configurações do Windows; Desligar abre o mesmo menu.
6. Com um app com trabalho não salvo aberto, Desligar/Reiniciar: o Windows ainda pergunta pelo app.

## T6 — Central de controle editável

1. Abrir a central: "Editar" no cabeçalho. Clicar: cada controle ganha um "−", o som some e aparece
   "Adicionar controles" com o resto do catálogo (rolar com a roda até o fim).
2. Tirar Transmitir pelo "−"; pôr Configurações, Modo escuro e Captura de tela pelo "+". Conferir que
   Configurações → Central de controle → "Botões da central" mostra a mesma lista, e que reabrir a
   Cimori mantém a ordem.
3. Arrastar Wi-Fi até a posição de Projetar: o controle segurado acende, o destino ganha contorno, e ao
   soltar ele fica ali.
4. "Concluído" volta ao normal; fechar a ilha no meio da edição também sai dela.
5. Usar cada controle novo: Modo escuro (troca o tema dos apps e do Windows), Captura de tela (a ilha some
   antes do recorte), Emojis e Área de transferência (abrem no app da frente), Mostrar área de trabalho,
   Menu Win+X, Bloquear, Calculadora, Teclado virtual, Teclado touch, Lixeira, Hotspot/Modo avião/Brilho/
   Bateria/Energia (páginas do Windows), Microfone, Pendrives (aba USB, ou Este Computador com o módulo
   desligado), Timer/Pomodoro/Cronômetro/Alarmes (aba Foco no modo certo), Clima e Próximo evento.
6. Com muitos controles fora do modo de edição, o som desce e a central rola.

## T7 — Unidades e atalhos do mac-island

1. Abrir com o settings.json atual (em px): Aparência → "Altura da pílula fechada" mostra o % mais próximo
   e a pílula fica do mesmo tamanho; Avisos → Banners mostra Tamanho e Largura em %.
2. Mudar a altura de 70 % a 130 %: a pílula acompanha (no entalhe também).
3. Banners em 50 %, 150 % e 200 %, largura 80 % e 160 %: um aviso, o volume e um lembrete saem no tamanho
   escolhido, com o texto nítido em 100 % e 150 %.
4. Atalhos → "Minimizar e restaurar": clicar e pressionar Ctrl+Alt+K; a pílula some e volta com ele.
   Pressionar só uma letra ou Shift+letra: aparece "Use Win, Ctrl ou Alt com uma tecla".
5. Esc durante a gravação cancela sem fechar a janela; o × limpa o atalho.
6. Gravar Win+N no minimizar: "Abrir avisos" fica sem atalho e Win+N passa a minimizar (pelo gancho).
7. Gravar uma combinação já usada por outro app: o log avisa e o atalho não funciona (escolher outra).

## T8 — Fuso, país dos feriados e som nos avisos

1. Mudar o fuso do Windows (ex.: UTC-5) com "Fuso horário do Windows" ligado: o relógio e a data da ilha
   passam a mostrar a hora nova em até um segundo, e "às HH:MM" dos lembretes segue o fuso. Desligar a opção:
   volta a Brasília (UTC-3).
2. "País dos feriados" vazio: os feriados são os da região do Windows (mudar a região em Configurações →
   Hora e idioma e reabrir o Cimori); digitar "PT" e Enter: troca para Portugal; apagar: volta à região.
3. Agenda → "Som nos avisos" ligado: um lembrete, o fim do timer e um evento da agenda tocam o som do Windows
   quando o banner aparece (e não durante o Não Perturbe); desligado, ficam mudos.

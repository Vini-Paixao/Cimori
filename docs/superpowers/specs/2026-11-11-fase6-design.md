# Fase 6 — Clima, timer, cronômetro, Pomodoro e lembretes

Data: 11/11/2026. Projeto: isle-win (marca Cimori). Agenda e feriados da fase 6 já estão prontos e
validados; este documento cobre o resto da fase.

## Por que

O Marcus usa o **clima todo dia**: a temperatura fica na pílula, e clicar nela abre o painel de clima
do mac-island (print de referência na conversa de 11/11/2026). É o que mais importa nesta fase.

Timer, cronômetro e Pomodoro ele **não usa**, mas entram porque são bons para o produto e o isle do
GNOME tem. Lembretes valem tanto os da agenda (automáticos) quanto os manuais ("me lembra daqui 30
min de fazer X").

Tudo que hoje é uma regra do Marcus (antecedência do lembrete, cidade, unidade, tempos do Pomodoro)
deve virar um valor configurável, porque a próxima etapa é a **tela de configurações** (projeto à
parte, fora deste documento). Aqui só se garante que os valores existam em `IsleConfig` e no
`settings.json`, com o padrão do mac-island.

## Metas

1. Painel de clima igual ao do mac-island, aberto ao clicar na temperatura da barra compacta.
2. Aba **Timer** com três modos: Pomodoro, timer (contagem regressiva) e cronômetro.
3. Lembretes manuais com texto e prazo, que sobrevivem a reiniciar.
4. Lembretes dos compromissos da agenda, como banner.
5. Todas as regras como valores de configuração, com padrões.

## Fora do escopo

- A tela de configurações em si (próximo projeto).
- Alarmes por horário com som (o roadmap os cita; ficam para depois, e o banner não toca som).
- Localização automática pelo Windows: a cidade vem da configuração (hoje do backup do mod:
  Bom Jesus, São Paulo, com latitude e longitude).
- Busca de cidade por nome (geocoding do Open-Meteo): fica para a tela de configurações.

## O que seria um fracasso

- O painel de clima parecer pior que o do mac-island ou com ícones borrados/quadrados (a fonte Inter
  não tem emoji: nada de emoji no clima).
- Gastar CPU parado: nenhum timer de interface rodando quando nada conta. O clima atualiza a cada
  30 min fora da thread da interface; a contagem só redesenha por segundo enquanto algo roda.
- Um lembrete sumir porque o PC reiniciou ou estava bloqueado.

## Restrições do projeto (já decididas)

- Rust, renderização própria em tiny-skia + Inter (`crates/isle-render`); serviços em threads
  próprias mandando `AppEvent` + `Waker` para a thread da interface (ver `spawn_services` em
  `src/app.rs`).
- Portar antes de inventar: dados e comportamento do mac-island 3.1.1
  (`mac-island-dock.wh.cpp`, código em `raw.githubusercontent.com/ramensoftware/windhawk-mods/main/mods/`),
  Pomodoro do isle (`zickrian/isle`, `gnome-extension/src/pages/timer.js` e `isle-core/src/pomodoro.rs`,
  este já portado).
- Textos da interface em português do Brasil.
- Paleta e medidas da ilha atual (abas de 520×360, superfícies `#2c2c2e`, destaque azul).

## 1. Clima

### Dados (porte de `WeatherData` e do pedido do mod)

Uma chamada ao Open-Meteo, a cada **30 minutos** (`kWeatherRefreshSeconds`), mais uma ao abrir a ilha:

```
https://api.open-meteo.com/v1/forecast?latitude=…&longitude=…
  &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,cloud_cover,
           pressure_msl,wind_speed_10m,is_day
  &hourly=temperature_2m,weather_code,wind_speed_10m,is_day
  &daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,uv_index_max,
         precipitation_probability_max
  &timezone=auto&forecast_days=7
```

Guardar: cidade, temperatura, sensação, umidade, código do tempo, dia/noite, nuvens, pressão,
vento, chance de chuva (de hoje), UV (de hoje), nascer/pôr do sol (de hoje), horas e 7 dias.
O código atual (`isle-platform-win/src/weather.rs`) só pede temperatura e código; é estendido.
Sem rede: mantém o último dado; erro só no painel ("Clima indisponível"), nunca trava.

### Tipos de céu e ícones

Os 11 tipos de céu do mod (`WeatherConceptOf`): limpo dia/noite, parcial dia/noite, nublado, chuva,
garoa, neve, granizo/chuva congelada, trovoada, neblina, com o mesmo agrupamento de códigos WMO.
**Decisão do Claude:** os ícones são desenhados pela própria ilha com formas vetoriais simples
(sol, lua, nuvem, gotas, flocos, raio, faixas de neblina) em cores suaves, nítidos em qualquer
escala. Os nomes do céu em português ("Céu limpo", "Garoa", "Trovoada"…) vêm da mesma tabela.

### Barra compacta

Temperatura + ícone do céu (hoje o dado existe mas não é desenhado; ver decisão 2 abaixo). Clicar
abre o painel de clima.

### Painel (como no print do mac-island, 620 px de largura no mod; aqui no tamanho de aba da ilha)

De cima para baixo:
1. Cidade e temperatura grandes; abaixo, nascer e pôr do sol.
2. Grade de 8 cartões (4×2): Sensação, Umidade, Vento, Índice UV / Chance de chuva, Pressão,
   Nuvens, Céu.
3. Sete dias (Hoje, Amanhã, nomes dos dias): ícone, máxima, mínima. Clicar num dia o seleciona.
4. Linha de temperatura por hora do dia selecionado (a partir de agora, para hoje): uma curva com
   a temperatura em cima, ícone, vento e hora embaixo ("Agora" na primeira).
5. Rodapé: "Dados do clima: Open-Meteo".

O painel é uma aba nova (`TabPage::Weather`) ou um modo próprio — escolha do construtor; precisa
caber e abrir ao clicar na temperatura. O tamanho pode crescer além de 520×360 se o conteúdo pedir
(o mod usa 620 px de largura).

## 2. Aba Timer

Três modos, escolhidos por um seletor no topo da aba: **Pomodoro · Timer · Cronômetro**.

- **Pomodoro** (porte da página do isle): anel que se enche no sentido horário com o tempo
  restante e a fase ("Foco", "Pausa") no meio; trabalho e pausa ajustáveis com − / + (1–180 e
  1–60 min); botões Iniciar / Pausar / Continuar e Reiniciar (só depois de iniciado). A lógica é
  `isle_core::pomodoro::Pomodoro`, já no projeto. Fim de fase: banner.
- **Timer:** contagem regressiva com duração ajustável (− / + de minuto em minuto e atalhos 5, 10,
  15 min), Iniciar / Pausar / Reiniciar. No fim: banner "Timer terminou".
- **Cronômetro:** Iniciar / Pausar / Zerar, com tempo em minutos:segundos (e horas quando passar
  de 60 min).

Enquanto qualquer um corre, a barra compacta mostra a contagem (como o Pomodoro já faz hoje).
O que está rodando sobrevive a fechar o painel; não precisa sobreviver a fechar a ilha.

## 3. Lembretes

- **Manuais:** na aba Timer, uma seção (ou um quarto modo — escolha do construtor) com campo de
  texto (mesma entrada de teclado da resposta dos avisos: a ilha pega o teclado ao clicar no campo
  e devolve depois) e prazo: "daqui X min" com − / + e atalhos (15, 30, 60 min). Lista dos
  pendentes, com um X para apagar. Lógica: `isle_core::reminders::Reminders` (já portado, até 50).
  Guardados em `%APPDATA%\isle-win\lembretes.json`; um lembrete vencido enquanto a ilha estava
  fechada aparece ao abrir.
- **Da agenda:** para cada compromisso com hora das próximas horas, um banner na ilha principal
  quando chega a hora menos a antecedência configurada (padrão **0 min**, como o mod: avisa na
  hora). Regras do mod (`ClassifyReminder`): aparece se o atraso for de até 2 min ("agora"); se a
  ilha estava fechada, aparece como perdido até 12 h depois; mais que isso é descartado. Dias
  inteiros não avisam (a ilha não tem o horário de aviso de dia inteiro do mod). Cada evento avisa
  uma vez (lembrar quais já avisaram por título + dia + minuto, em memória, e os de hoje em disco
  para não repetir ao reabrir).
- Banners: o mesmo `AlertBanner` dos avisos, respeitando Não Perturbe e cards de permissão, 6 s.
  Clicar no banner de compromisso abre a aba Agenda no dia.

## 4. Configuração (`IsleConfig`, salva em `settings.json`)

Valores novos ou reaproveitados, com padrão:

| Valor | Padrão |
|---|---|
| Cidade do clima (nome, latitude, longitude) | do backup do mod (já existe `weather`) |
| Temperatura em °F | não (°C) |
| Antecedência do lembrete da agenda (min) | 0 |
| Lembretes da agenda ligados | sim |
| Pomodoro: foco / pausa (min) | 25 / 5 (já existem em segundos) |
| Timer: duração inicial | 5 min (já existe `timer_default_secs`) |

Arquivos `settings.json` antigos continuam carregando (campos novos com `#[serde(default)]`).

## Decisões do Claude (depois da revisão do documento)

1. **Fila de alertas.** Lembretes (manuais e da agenda) e fim de timer/Pomodoro entram numa fila
   única no `IsleApp` e aparecem um de cada vez (6 s cada) assim que a ilha principal puder mostrar
   um banner: compacta ou minimizada, sem card de permissão. Nada é descartado por estar ocupado.
   - Com Não Perturbe, os lembretes **esperam** e aparecem quando ele acaba; o fim de timer também
     espera (é o que o usuário pediu para saber).
   - Vários vencidos de uma vez (ex.: compromissos perdidos ao abrir a ilha) aparecem em sequência,
     o mais antigo primeiro, no máximo 5; o resto vira um banner "e mais N lembretes".
   - O lembrete manual sai do `lembretes.json` quando o banner **aparece** (como o isle: "kept until
     shown").
   - Com o painel aberto, o banner espera o painel fechar; a contagem na aba já mostra o fim.
2. **Temperatura na barra compacta.** Elemento fixo logo depois da data e da hora, como no
   mac-island ("qua 11 nov 16:45 · [céu] 24°"), em todas as ilhas. Tem área de clique própria, que
   abre o painel de clima; o resto da barra continua abrindo as abas como hoje. Com pouco espaço,
   sai primeiro o texto da música, depois os ícones da bandeja; a temperatura e a contagem ficam.
   Hoje `weather_summary` existe mas não é desenhado: passa a guardar temperatura e tipo de céu.
3. **Ajustes da aba Timer são salvos.** Mudar foco/pausa do Pomodoro ou a duração do timer grava
   no `settings.json` (`pomodoro_focus_secs`, `pomodoro_break_secs`, `timer_default_secs`), por uma
   função única de salvar a configuração que a tela de configurações vai reaproveitar.
4. **Os três modos correm ao mesmo tempo**, independentes. A barra mostra o que termina primeiro
   (Pomodoro ou timer); sem nenhum deles, o cronômetro. Formato m:ss (h:mm:ss acima de 1 h),
   acordando a interface **uma vez por segundo só enquanto algo corre**; parado, nada acorda.

## O que fica com o construtor

- Layout exato dentro dos tamanhos, desde que siga a ordem e o conteúdo acima e a paleta da ilha.
- Se o clima é aba ou modo; se lembretes manuais são seção ou modo.
- Formas dos ícones do céu.
- Como dividir em commits; testes para lógica pura (parse do Open-Meteo, tipos de céu, regras de
  lembrete, timer/cronômetro) e prévias PNG (`cargo run -p isle-render --example preview`).

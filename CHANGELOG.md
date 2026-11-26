# Changelog

Todas as mudanças relevantes da Cimori. Versões no formato [SemVer](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Novo
- **Sumir só na frente destes apps** (Esconder): marque os apps (ex.: Chrome, Edge) em que a ilha
  atrapalha; ela some só quando um deles é a janela maximizada da frente no monitor, e continua
  visível nos outros (VS Code, Claude...). A lista mostra os apps abertos agora. "Sumir com janela
  maximizada" segue valendo para todos os apps.
- **Mostrar com a ilha escondida por um app** (Avisos › Banners, ligada por padrão): com a ilha
  sumida na frente de uma janela maximizada ou de um app marcado, banners e pedidos de permissão
  aparecem mesmo assim. Jogos e vídeos em tela cheia continuam sem.

### Corrigido
- Foco e Pausa do Pomodoro e o Tempo padrão do Timer, mudados nas configurações, só valiam depois de
  reiniciar a Cimori; agora valem na hora. Os sliders vão até onde os botões da ilha vão (180 e 60 min).
- Os switches **Claude Code** e **Codex** (IA › Agentes) não faziam nada sozinhos: desligar um deles
  agora tira as cotas dele da ilha e da aba Uso, na hora e sem reiniciar.
- Teste novo muda cada configuração, grava o settings.json e lê de volta, para nenhuma se perder.

## [1.0.1] — 2026-11-23

### Novo
- Aba **Atualizações**: buscar atualizações na hora e **baixar e instalar** a versão nova com um
  clique (o instalador vem do GitHub e só roda se o SHA-256 conferir; a Cimori fecha, atualiza e abre
  de novo com as suas configurações). A busca automática avisa na ilha, e o aviso abre esta aba.
- **Sumir com janela maximizada** (Esconder): a ilha some de vez, como em jogos e vídeos em tela
  cheia, enquanto houver uma janela maximizada no monitor dela, como um navegador com as abas no topo.
- **Renovar o login do Claude sozinha** (IA e agentes, desligada por padrão): com o Claude Code usado
  só em outro computador (por SSH), as cotas do Claude seguem certas neste PC.

### Corrigido
- "Verificar agora" dizia "Não deu para verificar agora" enquanto nenhuma versão estava publicada.
- Com o login do Claude Code vencido neste PC, o painel de uso diz como voltar a ver as cotas, em vez
  de parecer travado.
- Descrições das configurações quebram em várias linhas em vez de serem cortadas.

## [1.0.0] — 2026-11

Primeira versão pública.

### Ilha
- Pílula ou entalhe no topo de cada monitor, com relógio, data, contagem até o próximo evento, cor de
  destaque, Liquid Glass, animações de mola e auto-ocultar na linha.
- Pacotes de ícones on-line (Fluent, Ionicons, Lucide, Tabler, Phosphor, Material, Bootstrap,
  Iconoir, Heroicons) ou uma pasta sua.

### Módulos
- Música com capa, controles e letras sincronizadas (lrclib.net).
- Avisos do Windows na ilha, banners e apps silenciados.
- Agenda (.ics do Google ou do Outlook), feriados (Nager.Date) e lembretes.
- Pomodoro, timer, cronômetro e alarmes.
- IA e agentes: hooks do Claude Code e do Codex com permissões na ilha, cotas de uso e chat.
- Central de controle editável, volume por app, saídas de áudio e botão de desligar.
- Clima (Open-Meteo), histórico da área de transferência, prateleira com QR, USB, downloads e a
  bandeja dos outros apps (com a ponte opcional do Windhawk).

### Cimori
- Janela de configurações com busca, backup e restauração.
- Página **Sobre** com versão, código-fonte, onde reportar problemas, licença e créditos.
- **Verificação de atualizações** no GitHub ao abrir e uma vez por dia, com aviso na ilha (pode
  desligar; nada é instalado sozinho).
- **Iniciar com o Windows** (opcional).
- **Instalador** por usuário (sem administrador) e versão portátil em zip, com `SHA256SUMS.txt`.
- Card de permissão dos agentes com prioridade: os outros avisos esperam a resposta, e um clique
  no card traz a janela do agente.
- **Boas-vindas** na primeira abertura: aparência, hooks dos agentes em um clique e a ponte do
  Windhawk.
- `Cimori.exe` com ícone e informações de versão; pasta `%APPDATA%\Cimori` (a antiga `isle-win` é
  movida na primeira abertura).

[1.0.1]: https://github.com/Vini-Paixao/Cimori/releases/tag/v1.0.1
[1.0.0]: https://github.com/Vini-Paixao/Cimori/releases/tag/v1.0.0

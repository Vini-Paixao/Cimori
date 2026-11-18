# Changelog

Todas as mudanças relevantes da Cimori. Versões no formato [SemVer](https://semver.org/lang/pt-BR/).

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

[1.0.0]: https://github.com/Vini-Paixao/Cimori/releases/tag/v1.0.0

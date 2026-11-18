# Design Document: `isle-win` — Dynamic Island & AI Agent Control Center para Windows 11

**Data:** 2026-11-10  
**Autor:** Marcus Paixão + Antigravity (Superpowers + Ponytail `full`)  
**Status:** Pronto para revisão

---

## 1. Intenção e Motivação (O "Porquê")

O usuário utiliza o Windows 11 **sem a barra de tarefas inferior nativa**, substituída por um dock estilo macOS no rodapé e uma pílula/ilha dinâmica no topo da tela (atualmente via mod `Mac Island & Dock` do Windhawk). Paralelamente, todo o ambiente de desenvolvimento e uso de agentes de IA (**Claude Code**, **Codex**, **OpenCode Go**) roda em uma máquina **Fedora Linux** acessada via SSH, Deskflow (teclado/mouse compartilhados) e Tailscale/LAN.

O objetivo do **`isle-win`** é construir uma pílula superior nativa, ultraleve (~8–15 MB de RAM, 0% de CPU em repouso), que una o melhor de dois mundos para permitir desligar o mod atual do Windhawk sem perder nada do uso diário:
1. **Hub Nativo de Agentes de IA (`zickrian/isle`):** Receber notificações, alertas de parada (`Stop`) e cards interativos de aprovação (`Allow` / `Deny` / `Always`) do Claude Code e Codex em tempo real na pílula do Windows (vindos tanto do Fedora via rede/Tailscale quanto localmente), monitorar cotas de uso (janelas de 5h e semanais) usando as próprias contas já logadas nos CLIs (`claude`, `codex`) + chave de API do **OpenCode Go**, e permitir conversar (texto/voz) direto na pílula.
2. **Central de Controle Superior do Windows (`Mac Island & Dock`):** Cobrir todos os itens que o usuário realmente utiliza no dia a dia no lugar da barra de tarefas do Windows, importados diretamente do seu backup real (`Mac Island & Dock 2026-11-10.txt`).

---

## 2. Goals, Non-Goals e Anti-Goals

### Goals (O que será construído)
1. **Cross-Compile 100% no Fedora → Teste no Windows 11:**
   - Todo o código é compilado no Fedora (`/mnt/dados/projetos/isle-win`) gerando `isle-win.exe` (para rodar no Windows 11) e `isle-agent-bridge` (modo leve Linux para ler sessões/cotas e enviar hooks do Fedora para a pílula no Windows via LAN/Tailscale).
2. **Pílula Multi-Monitor com Física de Mola e 0% CPU Parada:**
   - Posicionada no topo central de `\\.\DISPLAY1` e `\\.\DISPLAY6` (`gap = 6px`, cor `#1C1C1E` opaca, curvas suaves estilo notch/pílula, auto-hide configurável).
   - Click-through dinâmico (`WM_NCHITTEST` retornando `HTTRANSPARENT` fora da geometria atual da pílula).
   - Animação por mola criticamente amortecida (`spring.rs`); quando `is_settled() == true`, o loop de quadros para completamente (0 FPS / 0% CPU).
3. **Estado Compacto / Fechado (Substituto da Barra Superior):**
   - Exibe: **Relógio + Dia da semana/Data**, **Status do Agente IA** (ocioso / trabalhando / aguardando permissão / alerta de cota 80%/95%), **Ícones Fixados da Bandeja** (**Tailscale**, **Deskflow**, **Google Drive**), **Indicadores Rápidos** (Rede, Volume/Mic, Energia, % de RAM, Drives USB conectados), **Contadores ativos** (próximo evento da agenda, Pomodoro 25/5m, Timer 5m, Alarmes) e **Mini-player com marquee** quando houver mídia tocando.
4. **Integração Completa de IA (`isle-ai`):**
   - **3 Provedores (Chat + Monitor de Cotas + Alerta Nativo de Reset):**
     - **Claude Code:** Reaproveita o login/sessão do CLI `claude` (`~/.claude/`) para responder no chat da pílula, exibir barras de cota (5h / semanal) com alertas automáticos em **80%** e **95%**, contagem regressiva automática para o horário de renovação (`reset_at`) e **notificação nativa "Claude de volta!"** assim que a cota reseta (eliminando a necessidade de criar alarmes manuais).
     - **Codex:** Reaproveita o login/sessão do CLI `codex` (`~/.codex/`) para chat, barras de cota e alerta nativo de reset.
     - **OpenCode Go:** Configurado via chave de API em `settings.json` para chat, monitoramento de cota e alerta nativo de reset.
   - **Hooks de Ciclo de Vida (`PermissionRequest`, `Notification`, `Stop`):**
     - Servidor HTTP local/LAN/Tailscale em `0.0.0.0:47830` no `isle-win.exe`.
     - Comando/script de hook não-destrutivo em `~/.claude/settings.json` e `~/.codex/hooks.json` (no Fedora e no Windows).
     - Quando o agente pede permissão, a pílula abre um card com **`Allow` / `Deny` / `Always`**; se o usuário responder no terminal antes ou estiver em Não Perturbe (DND), a pílula faz fallback silencioso sem travar o agente.
5. **Central de Controle do Windows & Produtividade (`isle-platform-win`):**
   - **System Tray:** Atalhos diretos para **Tailscale**, **Deskflow** e **Google Drive** + grade expansível de 6 colunas (até 12 itens) para ícones da bandeja.
   - **Áudio, Mic & OSD:** Controle de volume, mute de som/microfone (via CoreAudio `IAudioEndpointVolume`) e substituição de banners OSD de volume/energia/USB na pílula.
   - **Toggles & Indicadores do Sistema:** Rede/Wi-Fi, Bluetooth, DND, Luz Noturna, Projetar (`Win+P`), Cast (`Win+K`), Legendas ao Vivo, Acessibilidade, Energia, uso de **RAM** (`GlobalMemoryStatusEx`) e **Drives USB** (notificação ao conectar + botão de abrir/ejetar).
   - **Atalhos Rápidos & Pastas Fixadas:** Gerenciador de Tarefas (`taskmgr.exe`), Explorer e pastas fixadas (`Desktop`, `Documents`, `Downloads`, `This PC`, `Pictures`, `Screenshots`).
   - **Clima (Open-Meteo):** Pré-configurado para **Bom Jesus, São Paulo (`-23.5505, -46.6333`)** com temperatura atual e cards de previsão.
   - **Calendário `.ics` e Contadores:** Sincroniza com o feed privado `.ics` do Google Calendar do usuário, destaca feriados e dia atual, e exibe contadores regressivos (próximo evento da agenda, tempo para reset da cota de IA, Pomodoro `1500s/300s` e Timer `300s`).
   - **Mídia & Letras Sincronizadas (`.lrc`):** Controles Play/Pause/Next/Prev, título com marquee e letras sincronizadas via LRCLIB.
   - **Notificações & Clipboard Shelf:** Captura notificações e atalhos `Win+N` / `Win+A`, exibe banners na pílula e mantém histórico de Clipboard (ignorando cofres de senha) + Shelf de arquivos com servidor QR Code temporário na rede local.

### Non-Goals (Fora de escopo)
- **Dock inferior de aplicativos:** Será mantido por um mod dedicado apenas ao Dock estilo macOS.
- **Módulos desativados no backup do usuário (`=0`):** Medidor de CPU/VRAM, notas adesivas, lixeira, integração Komorebi, relógios mundiais e troca automática de papel de parede ficam de fora (YAGNI).

### Anti-Goals (O que tornaria o projeto uma falha mesmo funcionando)
- **Consumo alto de recursos ou polling constante:** A pílula jamais pode rodar a 60 FPS quando estiver parada nem consumir centenas de MB de RAM como Electron/WebView2.
- **Exigir instalar Visual Studio / C++ Build Tools no PC Windows do usuário:** Todo o processo de compilação deve funcionar diretamente no Fedora Linux e gerar um `.exe` autocontido.
- **Travar o terminal do Claude Code/Codex:** Se o `isle-win.exe` estiver fechado, em tela cheia/DND ou inacessível na rede, o hook deve cair em fallback imediato (timeout curto de conexão) para o prompt normal do terminal.

---

## 3. Restrições (Constraints)

1. **Ambiente de Build:** Fedora Linux (`/mnt/dados/projetos/isle-win`), sem toolchain instalado inicialmente — precisaremos provisionar Rust + target de cross-compilação Windows (`x86_64-pc-windows-gnu` via MinGW-w64) no próprio Fedora.
2. **Ambiente de Execução:** Windows 11 com dois monitores ativos (`\\.\DISPLAY1` e `\\.\DISPLAY6`), conectado ao Fedora na mesma rede local e via Tailscale + Deskflow.
3. **Zero Dependências C++ Pesadas no Cross-Compile:** Evitar bibliotecas que exigem compilar C++ complexo ou linkar binários MSVC proprietários durante o cross-compile (`skia-safe` e `whisper.cpp`).

---

## 4. Abordagens Consideradas e Escolha

| Abordagem | Descrição | Trade-offs | Decisão |
| :--- | :--- | :--- | :--- |
| **1. Híbrida Rust Nativo + Win32 Layered/DWM + Ponte Fedora/Windows** | Usa janelas Win32 `WS_EX_LAYERED \| WS_EX_TOPMOST \| WS_EX_TOOLWINDOW \| WS_EX_NOACTIVATE` com buffer BGRA pré-multiplicado via `UpdateLayeredWindow` (DWM compõe na GPU a 0% CPU parada) e renderizador 2D 100% Rust (`tiny-skia` + `ab_glyph`), mais ponte HTTP/CLI entre Fedora e Windows. | Zero dependências C++, cross-compila do Fedora para `.exe` em segundos, permite testar a renderização e o `isle-core` no próprio Fedora, binário de ~3–5 MB. | **Escolhida** (pelo usuário por recomendação técnica) |
| **2. Direct2D COM Puro (`ID2D1DeviceContext`)** | Usa bindings COM de baixo nível do Direct2D/DirectWrite via crate `windows`. | Leve no Windows, mas exige centenas de linhas de boilerplate `unsafe` COM/vtable e impede testar o desenho das abas fora do Windows. | Descartada (complexidade desnecessária para o mesmo resultado visual) |
| **3. `skia-safe` + `whisper-rs` via `cargo-xwin`** | Segue literalmente o rascunho inicial com bindings C++ do Skia e Whisper. | Quebra facilmente no cross-compile Linux→Windows via CMake/bindgen e gera binário >50 MB. | Descartada |

---

## 5. Arquitetura e Decisões Técnicas Consolidadas

### 5.1. Estrutura do Workspace Cargo

Mantemos a separação limpa de 4 crates do documento de arquitetura, adaptada para cross-compilar tanto o `.exe` do Windows quanto o binário da ponte no Fedora:

```text
isle-win/
├── Cargo.toml                        # Workspace: isle-core, isle-render, isle-platform-win, isle-ai
├── config/
│   └── default_settings.json         # Pré-carregado com os dados do backup Mac Island & Dock do usuário
├── scripts/
│   ├── build-win.sh                  # Compila isle-win.exe no Fedora e disponibiliza para o Windows
│   └── claude-hook.sh                # Script leve de hook para ~/.claude/settings.json no Fedora
├── src/
│   ├── main.rs                       # Binário principal (Windows: UI + Servidor; Linux: Ponte CLI/Agentes)
│   ├── app.rs                        # Orquestrador de estado, timers, eventos Win32 e rede
│   └── config.rs                     # Persistência em %APPDATA%\isle-win\settings.json + importador .txt Windhawk
└── crates/
    ├── isle-core/                    # 100% Rust puro (testável no Fedora):
    │   └── src/
    │       ├── lib.rs
    │       ├── spring.rs             # Física de mola criticamente amortecida (PillGeometry)
    │       ├── state.rs              # Máquina de estados (Closed, CompactBar, AlertPopup, ExpandedTab)
    │       ├── lrc.rs                # Parser .lrc e limpeza de faixas
    │       ├── ics.rs                # Parser Google Calendar (.ics), eventos, feriados e alarmes (21:30)
    │       ├── timer.rs              # Pomodoro (25/5m), Timer (5m) e contadores regressivos
    │       └── shelf.rs              # Histórico de clipboard (filtro de senhas) e itens da Shelf
    ├── isle-render/                  # Motor visual 2D (`tiny-skia` + `ab_glyph`) + Janelas Win32 Multi-Monitor:
    │   └── src/
    │       ├── lib.rs
    │       ├── window.rs             # HWND por monitor (DISPLAY1, DISPLAY6), WM_NCHITTEST e UpdateLayeredWindow
    │       ├── renderer.rs           # Desenho da pílula (#1C1C1E), curvas de notch, glow IA e tipografia
    │       └── pages.rs              # Layouts: CompactBar, AI/Chat/Quotas, ControlCenter/Tray, Calendar/Weather, Media, Shelf, Notifications
    ├── isle-platform-win/            # Integrações Windows 11 (com stubs seguros para compilação/teste no Linux):
    │   └── src/
    │       ├── lib.rs
    │       ├── audio_osd.rs          # Volume, Mute, Microfone (CoreAudio) e OSD
    │       ├── system_controls.rs    # RAM (GlobalMemoryStatusEx), Rede, Energia, USB Drives (listar/ejetar), Toggles e Pastas
    │       ├── tray.rs               # Pinned Apps (Tailscale, Deskflow, Google Drive) + bandeja
    │       ├── weather.rs            # Cliente Open-Meteo (Bom Jesus, SP: -23.5505, -46.6333)
    │       ├── media.rs              # Controles de mídia + cliente LRCLIB
    │       └── notifications.rs      # Captura de notificações, Win+N / Win+A e Clipboard listener
    └── isle-ai/                      # Integração Claude Code, Codex e OpenCode Go + Servidor HTTP :47830:
        └── src/
            ├── lib.rs
            ├── providers.rs          # Chat e leitura de cotas via CLI (`claude`, `codex`) e API (`OpenCode Go`)
            ├── hooks.rs              # Servidor HTTP :47830 (PermissionRequest Allow/Deny/Always, Notification, Stop)
            └── bridge.rs             # Ponte Fedora <-> Windows para executar chat/cotas usando o login do Fedora
```

### 5.2. Fluxo da Ponte de IA (Fedora ↔ Windows 11)
1. **No Fedora:** O binário compilado para Linux (`isle-win --fedora-bridge --windows-host <IP_WINDOWS>:47830`) ou o script `claude-hook.sh`:
   - Injeta de forma não-destrutiva os hooks `PermissionRequest`, `Notification` e `Stop` em `~/.claude/settings.json` (e `~/.codex/hooks.json`).
   - Lê as cotas da conta assinada do Claude Code e do Codex diretamente das credenciais/CLI locais do Fedora e publica para a pílula no Windows.
   - Recebe mensagens do chat da pílula e executa `claude -p` / `codex` (ou chama a API do OpenCode Go), devolvendo a resposta em streaming/texto para a pílula no Windows.
2. **No Windows 11 (`isle-win.exe`):**
   - Quando um `PermissionRequest` chega em `:47830/api/hook`, a mola expande a pílula imediatamente no topo do monitor, aplica o brilho azul/laranja de alerta e exibe a ferramenta/comando solicitado (ex.: `Bash: cargo test`) com 3 botões clicáveis: **`Allow`**, **`Deny`** e **`Always`**.
   - O clique responde a requisição HTTP pendente e o Claude Code no Fedora continua na mesma hora.

### 5.3. Decisões Definidas pelo Construtor ("Builder's Calls")
- **Fonte Tipográfica:** Carregará `Segoe UI Variable` / `Segoe UI` diretamente de `C:\Windows\Fonts\segoeui.ttf` no Windows (com fallback embutido para testes no Linux).
- **Importação Automática do Backup:** O arquivo `Mac Island & Dock 2026-11-10.txt` já presente no repositório será lido automaticamente na primeira execução para popular monitores (`DISPLAY1`, `DISPLAY6`), coordenadas de Bom Jesus/SP, URL `.ics` do Google Calendar, tempos de Pomodoro (`1500s`/`300s`) e apps fixados da bandeja (Tailscale, Deskflow, Google Drive). O alarme manual das 21:30 é substituído pelo disparo nativo baseado no timestamp real de reset da cota do Claude Code/Codex.
- **Atalhos Globais no Windows:**
  - `Win+I` (ou clique no ícone de IA): Abre direto a aba de Chat/Voz e Cotas de IA na pílula.
  - `Win+N`: Abre a aba de Notificações & Calendário na pílula (`iislandWinN=1`).
  - `Win+A`: Abre a Central de Controle Rápida na pílula (`iislandWinA=1`).
  - `Alt+M` (código `330` do backup): Minimiza/oculta a pílula manualmente (`iislandMinimizeShortcut=330`).

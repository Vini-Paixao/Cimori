# Cimori

**Uma ilha dinâmica para o Windows 11.** Uma pílula no topo da tela que mostra a música, os avisos,
a agenda, os timers e os seus agentes de IA, e abre em painéis quando você passa o mouse ou clica.

Cimori é software livre (GPL-3.0), escrito em Rust, sem Electron e sem navegador embutido: leve em
RAM e CPU. Ela nasceu da união de dois projetos livres: o [isle](https://github.com/zickrian/isle),
de zickrian, e o [Mac Island & Dock](https://island.caliberda.com.br), mod do Windhawk de caliberda.

> English summary at the end.

## Recursos

- **Pílula ou entalhe** no topo de cada monitor, com relógio, data, contagem até o próximo evento,
  cor de destaque, Liquid Glass e pacotes de ícones (Fluent, Lucide, Tabler, Phosphor e outros).
- **Música**: o que toca em qualquer player do Windows, capa, controles e letras sincronizadas.
- **Avisos do Windows** na ilha, com banners, lista por página e apps silenciados.
- **Agenda**: calendário do Google ou do Outlook (endereço .ics), feriados e lembretes.
- **Foco e timers**: Pomodoro, timer, cronômetro e alarmes.
- **IA e agentes**: Claude Code e Codex avisam a ilha quando terminam ou pedem permissão (você
  aprova ou recusa pela ilha), mostra as cotas de uso e um chat rápido.
- **Central de controle**: volume por app, saídas de áudio, Wi-Fi, Bluetooth, botão de desligar e
  atalhos editáveis.
- **Clima**, **área de transferência** com histórico, **prateleira** de arquivos com envio pelo
  celular (QR na rede local), **dispositivos USB**, **downloads** e a **bandeja** dos outros apps.
- **Configurações** completas numa janela própria, com busca, backup e "Sobre".
- **Atualizações**: avisa quando sai uma versão nova no GitHub (pode desligar). Nada é instalado
  sozinho.

## Requisitos

- **Windows 11** (64 bits). O Windows 10 pode funcionar, mas sem garantia.
- Para os agentes: [Claude Code](https://claude.com/claude-code) e/ou Codex instalados.
- Opcional: [Windhawk](https://windhawk.net), para a ponte da bandeja e do progresso dos downloads.

## Baixar e instalar

Em [Releases](https://github.com/Vini-Paixao/Cimori/releases) há duas formas:

- **Instalador** (`Cimori-<versão>-setup.exe`, recomendado): instala só para o seu usuário, sem pedir
  administrador, em `%LOCALAPPDATA%\Programs\Cimori`, com atalho no menu Iniciar. Para atualizar, rode
  o instalador da versão nova por cima; para remover, use **Configurações do Windows → Aplicativos**
  (os hooks dos agentes saem junto; as suas configurações ficam).
- **Portátil** (`Cimori-<versão>-win-x64.zip`): extraia numa pasta sua e abra o `Cimori.exe`.

Para abrir com o Windows, ligue **Configurações → Geral → Iniciar com o Windows** (as boas-vindas
perguntam). O `SHA256SUMS.txt` da release confere os arquivos baixados.

O Windows SmartScreen pode avisar na primeira vez, porque o executável não é assinado: clique em
"Mais informações" e "Executar assim mesmo".

As configurações ficam em `%APPDATA%\Cimori\settings.json` (com o registro `Cimori.log` ao lado).

## Primeira abertura

Na primeira vez, a janela de configurações abre nas **Boas-vindas**: um passo a passo para escolher o
estilo e a cor, instalar os hooks dos agentes com um clique e configurar a ponte do Windhawk. Dá para
rever tudo depois em **Configurações → Sobre → Mostrar boas-vindas de novo**.

Clique com o botão direito na ilha (ou na engrenagem do painel aberto) para abrir as configurações.

## Agentes de IA (hooks)

Em **Configurações → IA e agentes → Instalar os hooks** (ou nas boas-vindas), a Cimori acrescenta
seus hooks ao `~/.claude/settings.json` do Claude Code e ao `~/.codex/hooks.json` do Codex. Os
hooks chamam o próprio `Cimori.exe`, que fala com a ilha por `127.0.0.1:47830` com um token
compartilhado. **Remover os hooks** tira só os da Cimori. Pela linha de comando:
`Cimori.exe --install-hooks` e `Cimori.exe --uninstall-hooks`.

Se você mover o `Cimori.exe` de pasta, a Cimori reaponta os hooks sozinha na próxima abertura.

## Ponte do Windhawk (opcional)

O Windows só entrega os ícones da bandeja e o progresso da barra de tarefas ao próprio Explorer. A
ponte é um mod pequeno do Windhawk que lê esses dados dentro do Explorer e os repassa à Cimori. Com
ela, a ilha mostra e clica todos os ícones da bandeja e enche a pílula durante os downloads.

1. Instale o [Windhawk](https://windhawk.net).
2. No Windhawk, clique em **Criar um novo mod**.
3. Apague o código de exemplo e cole todo o `cimori-tray-bridge.wh.cpp` (vem no zip; nas
   boas-vindas há o botão **Copiar código da ponte**).
4. **Compilar o mod** e depois **Sair do modo de edição**: o mod fica ativo.

Detalhes técnicos em [`windhawk/README.md`](windhawk/README.md). Sem a ponte, a Cimori funciona igual.

## Privacidade

A Cimori não tem conta, telemetria nem servidor próprio. Ela só acessa a internet para:

| O quê | Para onde | Quando |
|---|---|---|
| Clima e busca de cidade | Open-Meteo (`api.open-meteo.com`, `geocoding-api.open-meteo.com`) | Módulo Clima ligado, a cada 30 min |
| Letras das músicas | lrclib.net | Opção "Letras" ligada, a cada faixa nova |
| Feriados | Nager.Date (`date.nager.at`) | Opção "Feriados" ligada |
| Sua agenda | O endereço .ics que você colar | Módulo Agenda com endereço |
| Pacotes de ícones | jsDelivr (`cdn.jsdelivr.net`) | Uma vez, ao escolher um pacote on-line |
| Atualizações | GitHub (`api.github.com`) | Ao abrir e uma vez por dia; pode desligar |
| Cotas dos agentes | `api.anthropic.com`, `chatgpt.com`, com o login que o Claude Code e o Codex já têm | Módulo IA ligado |

O chat da IA roda os programas `claude`, `codex` ou `opencode` instalados na sua máquina. O
servidor dos hooks e o compartilhamento por QR da prateleira ficam na sua rede local.

## Compilar

Requer [Rust](https://rustup.rs) estável (1.88 ou mais novo) e as Build Tools do Visual Studio
(MSVC), no Windows:

```powershell
git clone https://github.com/Vini-Paixao/Cimori
cd Cimori
cargo build --release          # target\release\Cimori.exe
cargo test --workspace
.scripts
.\scripts\release.ps1          # dist\: zip portátil e, com o Inno Setup 6, o instalador
```

## Créditos e licenças

- **Cimori** — Marcus Paixão, [GPL-3.0](LICENSE).
- **[isle](https://github.com/zickrian/isle)** — zickrian, GPL-3.0: a base da ilha, dos agentes, do
  Pomodoro e muitos módulos (`crates/isle-core` traz partes portadas sem mudança).
- **[Mac Island & Dock](https://island.caliberda.com.br)** — caliberda, GPL-3.0: o mod do Windhawk de
  onde vêm as opções, os módulos, o entalhe, a central de controle e a ponte.
- **Pacotes de ícones** (baixados só ao escolher): Fluent UI System Icons (Microsoft), Ionicons,
  Tabler, Phosphor, Bootstrap Icons, Iconoir e Heroicons (Tailwind Labs) — MIT; Lucide — ISC;
  Material Symbols (Google) — Apache 2.0.
- **Bibliotecas Rust**: tokio, reqwest, serde, tiny-skia, resvg, ab_glyph, windows-rs e outras, cada
  uma com a sua licença (MIT, Apache 2.0 e semelhantes); a lista completa está no `Cargo.lock`.

---

## English

**Cimori** is a dynamic-island overlay for Windows 11, written in Rust: a pill at the top of the
screen for media, Windows notifications, calendar, timers and AI coding agents (Claude Code and Codex
hooks with permission prompts, usage quotas), plus a control center, weather, clipboard history, a
file shelf, USB drives and downloads. The interface is in Brazilian Portuguese.

- **Requirements:** Windows 11 x64 (Windows 10 is best-effort).
- **Install:** from [Releases](https://github.com/Vini-Paixao/Cimori/releases), run
  `Cimori-<version>-setup.exe` (per user, no admin) or extract the portable zip anywhere and run
  `Cimori.exe`. A welcome flow opens on first run.
- **Optional:** the [Windhawk](https://windhawk.net) bridge (`cimori-tray-bridge.wh.cpp`) lets the
  island show and click every tray icon and read taskbar download progress.
- **Privacy:** no telemetry. Network use is limited to the features listed in the table above
  (weather, lyrics, holidays, your calendar, icon packs, a daily GitHub update check that can be
  turned off, and the agents' quota APIs).
- **Build:** `cargo build --release` with Rust and MSVC.
- **License:** GPL-3.0. Based on [isle](https://github.com/zickrian/isle) by zickrian and
  [Mac Island & Dock](https://island.caliberda.com.br) by caliberda, both GPL-3.0.

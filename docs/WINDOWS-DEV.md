# Desenvolvimento no Windows

A partir de 11/11/2026 o isle-win é desenvolvido direto no Windows 11. Antes era cross-compilado no
Fedora com cargo-xwin. O produto só existe no Windows, e assim os testes rodam o código do Windows de
verdade e a ilha pode ser aberta e inspecionada na hora.

## Instalação (uma vez)

1. **Visual Studio Build Tools 2022**, com a carga "Desenvolvimento para desktop com C++", que traz o
   linker MSVC e o Windows SDK. Pelo terminal:

   ```powershell
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```

2. **Rust**, pelo rustup (o instalador oficial é o de https://rustup.rs):

   ```powershell
   winget install Rustlang.Rustup
   ```

   Feche e abra o terminal e confira com `cargo --version`.

3. **Git** (se ainda não houver): `winget install Git.Git`.
4. **VS Code** com a extensão **rust-analyzer**. A extensão de C/C++ não é necessária, porque o Rust
   usa só o linker do Build Tools.

## O repositório

```powershell
git clone https://github.com/Vini-Paixao/Cimori.git
cd Cimori
```

Copie o backup do mod para a raiz do repositório: `Mac Island & Dock 2026-11-10.txt`. Ele é privado,
porque tem o endereço secreto da agenda do Google, e o `.gitignore` impede que ele entre num commit.
Sem esse arquivo, o teste de importação em `config.rs` não compila (`include_str!`).

## Uso diário

```powershell
cargo test --workspace
cargo run --release
```

- O `cargo run` abre a ilha. Pare antes a cópia instalada (`Stop-Process -Name Cimori`).
- Logs: o exe não tem console; a ilha grava em `%APPDATA%\Cimori\Cimori.log` (recomeça a cada
  abertura) e também mostra no terminal quando rodada por um (`cargo run`).
- Prévias: `cargo run -p isle-render --example preview -- <pasta> <escala>`.
- Hooks do Claude Code e do Codex: `Cimori.exe --install-hooks`. No Fedora, o hook aponta para o
  Windows pela opção `--windows-host 100.64.0.1:47830`.
- O exe é `target\release\Cimori.exe`. Na primeira abertura, a pasta antiga `%APPDATA%\isle-win` (e a de
  `%LOCALAPPDATA%`) vira `Cimori`, e os hooks que rodavam o `isle-win.exe` passam a rodar o `Cimori.exe`.
- `scripts/build-win.sh` e o cargo-xwin ficam só para quem quiser compilar no Linux.

## Onde paramos

O roadmap e o status de cada fase estão em `docs/ROADMAP.md`. Fases 0 a 7 validadas no Windows em
11/11/2026 (a 5, bandeja, com a ponte opcional do Windhawk). Depois veio o polimento visual
**Cimori** (r2, aprovado; `docs/VISUAL-VALIDATION.md`) e uma revisão do repo (308 testes verdes).

Marcus validou a primeira entrega da tela de configurações no uso ao vivo. A fase 8 e a segunda
leva de opções têm um roteiro de verificação próprio em [PHASE8-VALIDATION.md](PHASE8-VALIDATION.md).

Histórico desta fase (avisos):

- ✅ **Avisos limpos voltam ao reiniciar** (11/11/2026). "Limpar tudo" guarda uma marca d'água (o
  `m_clearedUntil` do mod) em `%APPDATA%\Cimori\avisos-limpos.txt`, ao lado do `settings.json`.
  "Lido" já sobrevive, porque o histórico lido na partida entra como lido. A lista `m_dismissed` fica
  para quando existir dispensar um aviso só.
- ✅ **Avisos novos não chegavam** (11/11/2026): o callback do WNF lia o contexto no parâmetro
  errado e o worker nunca acordava depois da leitura inicial. Diagnóstico: rodar o exe pelo
  terminal com logs e ver que o banco ganhou um aviso sem nenhuma leitura no log.
- ✅ **Só a última mensagem de cada conversa do WhatsApp ficava** (11/11/2026, validado ao vivo).
  Observado no banco com 5 mensagens: cada aviso tem `Tag` = conversa (`...@g.us`, `...@lid`), e
  cada mensagem nova **apaga o aviso anterior da conversa e grava outro com `Id` novo** (mesmo
  `"Order"`). A linha `badge` do app (`<badge value="2"/>`, conversas não lidas) é regravada junto.
  Correção portada de `MergeKeptToasts` (mod 3.1.1): avisos que o app tirou do banco continuam na
  ilha enquanto o app tem badge, lido da linha `badge` do banco (o mod lê da barra de tarefas). Sem
  limite pelo número do badge, porque o WhatsApp conta conversas e não mensagens.
- ✅ Clique num aviso abre o WhatsApp **na conversa** (ativador do app com o `launch` do aviso;
  reserva: `shell:AppsFolder\<AUMID>`).
- ✅ Clique no banner abre a conversa, 4 avisos por página, responder pela ilha (falta validar o
  envio ao vivo). Banners do Windows: desligar "Mostrar faixas de notificação" por app.
- O mac-island do Windhawk pode rodar junto: os dois só **leem** o `wpndatabase.db`; o que se
  sobrepõe é a tela (banners dos dois).
- **Fase 5 (bandeja):** código do Codex juntado ao `feat/isle-win`. Lista os ícones; os cliques só
  funcionam com a barra de tarefas visível, ou com a ponte opcional do Windhawk instalada
  ([`windhawk/README.md`](../windhawk/README.md)), que clica como o mac-island.
  Diagnóstico sem parar a ilha instalada: `cargo run --release -- --tray-inspect` (opcional
  `--watch-seconds 12`). Não inicia UI/hooks nem sobrescreve o log instalado.
  Validação interativa pendente; ver [PHASE5-VALIDATION.md](PHASE5-VALIDATION.md).
- O código do mod para consulta: `raw.githubusercontent.com/ramensoftware/windhawk-mods/main/mods/mac-island-dock.wh.cpp`
  (3.1.1, ~120 mil linhas; `ReadNewToasts`, `ToastThreadProc`, `NotificationPanel::MergeKeptToasts`).

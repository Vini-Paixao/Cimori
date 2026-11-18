# Ponte da bandeja (Windhawk) — opcional

A ilha mostra os ícones da bandeja (Steam, Tailscale, Google Drive, Vanguard…) e os clica como a
barra de tarefas: **esquerdo** faz a ação do app, **direito** abre o menu do próprio app (Sair,
Configurações…). Para isso ela precisa saber, de cada ícone, a janela do app, o número do ícone e a
mensagem que ele espera. O Windows só entrega isso à barra de tarefas, dentro do `explorer.exe`;
por isso existe esta ponte, um mod pequeno do [Windhawk](https://windhawk.net) que lê esses dados
dentro do Explorer e os repassa à ilha.

Sem a ponte a ilha funciona igual; a bandeja só clica quando a barra de tarefas está visível.

## Instalar

1. Abra o Windhawk e clique em **Criar um novo mod**.
2. Apague o código de exemplo e cole todo o conteúdo de [`cimori-tray-bridge.wh.cpp`](cimori-tray-bridge.wh.cpp).
3. **Compilar o mod** e depois **Sair do modo de edição**. O mod fica ativo para o `explorer.exe`.
4. Com a ilha aberta, a bandeja passa a vir pela ponte (o log da ilha, em
   `%APPDATA%\Cimori\Cimori.log`, diz `bandeja pela ponte do Windhawk: N ícones`).

A primeira vez que a ilha fala com a ponte, ela pede a todos os apps que recriem seus ícones (a
mensagem que o Windows manda quando a barra de tarefas reinicia), para conhecer os que já estavam
abertos. Pode rodar junto com o Mac Island & Dock.

## Como funciona

- O mod faz subclass da `Shell_TrayWnd` e lê os `WM_COPYDATA` de `Shell_NotifyIcon` depois que a
  barra os tratou (portado de `OnTrayNotify` do Mac Island & Dock 3.1.1).
- Manda a lista inteira à janela escondida `CimoriTrayHost` da ilha, por `WM_COPYDATA`
  (`0x43494D01`), quando algo muda; a ilha diz "oi" com `0x43494D02` ao abrir e quando o Explorer
  reinicia. O formato é o do `SendTrayIcons` do mod.
- A ponte acha a ilha pela classe da janela (`CimoriTrayHost`, `CimoriDownloadHost`), não pelo nome do
  processo: serve ao `Cimori.exe` e ao `isle-win.exe` antigo.
- Os cliques saem da ilha direto para o app (`PostTrayIconMessage`): a ponte não clica nada.
- Lado da ilha: `crates/isle-platform-win/src/tray/bridge.rs`.
# Fase 9: progresso dos downloads

A ponte também porta `GetTaskbarXamlRoot`, `ProgressBarWatch`, `ProgressBarState`,
`PublishProgress` e os hooks `CTaskBand::_OnSetProgressState/_OnSetProgressValue`
do Mac Island & Dock 3.1.1 (caliberda, GPL-3.0). Ative **Ler o progresso da barra
de tarefas** em Downloads. O valor vem do `RangeBase` visual, nunca do DWORD bruto
do hook. A maior prioridade é erro, pausa, normal, indeterminado; entre iguais,
vale o menor percentual. Aguarda cinco segundos para ignorar operações breves.

IPC: `WM_COPYDATA`, `dwData=0x43494D03`, destino `CimoriDownloadHost`;
16 bytes little-endian: versão u32=1, estado TBPF u32, permille u32
(`0xffffffff` indeterminado), número de barras u32. Heartbeat de um segundo;
após cinco segundos sem mensagem a ilha usa BITS/arquivos parciais. A bandeja
continua usando seu host e protocolo anteriores. Símbolos ausentes desativam
o progresso da ponte. O porte inicial lê a barra principal do Windows 11;
barras secundárias e builds Insider exigem validação ao vivo.


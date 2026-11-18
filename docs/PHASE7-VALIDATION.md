# Fase 7 — chat na ilha

Porte de `zickrian/isle @ a29a7c4`: `assistant/{claude,codex,opencode,chat,child,sign_in}.rs`
e `gnome-extension/src/chat.js`. Os arquivos adaptados têm crédito e cabeçalho GPL.
O stub antigo `providers.rs`, sem streaming e com endpoint Go incorreto, foi removido.

Arquivos alterados:

- `Cargo.toml`, `Cargo.lock`, `crates/isle-ai/Cargo.toml`: acesso ao Job Object do Windows, sem
  biblioteca nova baixada.
- `crates/isle-ai/src/lib.rs`, remoção de `providers.rs`, novos
  `assistant/{mod.rs,backends.rs,protocol.rs,process.rs,codex-config.toml}`.
- `crates/isle-core/src/{lib.rs,state.rs,chat.rs}`: sessão e histórico limitado.
- `crates/isle-render/src/{lib.rs,pages.rs,renderer.rs,chat.rs,chat_render.rs}` e
  `crates/isle-render/examples/preview.rs`: ações, hitboxes e desenho.
- `src/app.rs`, `src/ai_chat.rs`: teclado, worker e eventos.
- `docs/ROADMAP.md` (somente Fase 7) e este relatório.

## Uso e autenticação

Na aba **IA**, **Conversar com IA** abre o chat de 520×360. Escolher Claude/Codex/OpenCode Go
inicia uma conversa nova. **Cotas** volta ao painel existente; **Nova** limpa a sessão.
Clique na caixa para retomar o teclado. Enter envia, Backspace apaga, Ctrl+V cola,
Esc solta o teclado e preserva o rascunho. **Parar** cancela a execução; botões e roda navegam
no histórico. Perda de foco/troca de aba devolvem o teclado. Um chat compartilhado pelas ilhas,
com uma única janela dona da entrada; Avisos continua usando a mesma captura e edição `Reply`.

| Serviço | Protocolo portado | Login |
|---|---|---|
| Claude | `claude -p --input-format stream-json --output-format stream-json --include-partial-messages`; deltas de texto e resultado final | Login local do Claude Code (`CLAUDE_CONFIG_DIR` ou `~/.claude`), incluindo helpers/env de autenticação das configurações. Só esses campos são passados em arquivo temporário, apagado ao terminar. |
| Codex | `codex app-server`; initialize, thread/start, turn/start; deltas de agentMessage e turn/completed | `CODEX_HOME/auth.json` ou `~/.codex/auth.json`, copiado para o home isolado da ilha. Renovação volta se o login original não mudou nem foi apagado; cópia temporária é removida. Login apenas no keyring, sem auth.json, não está contemplado pelo porte. |
| OpenCode Go | `opencode models`, escolha do primeiro modelo `opencode-go/` listado, `opencode run --format json`; texto por etapa | `IsleConfig.opencode_api_key`, em `OPENCODE_AUTH_CONTENT` como provedor `opencode-go`, tipo `api`. Não usa o endpoint HTTP do stub. Exige OpenCode ≥1.4.8; 2.x usa `--standalone`. |

Os CLIs devem estar no PATH, em `~/.local/bin`, `~/.cargo/bin` ou no npm do AppData.
Entradas npm conhecidas são executadas com Node diretamente, sem interpolar texto/chaves no shell.
Prompt vai pelo stdin, chave Go pelo ambiente; nada é fixado no código nem escrito nos logs da ilha.
Configurações isoladas do Codex/OpenCode; ferramentas locais/hooks/MCP/plugins desabilitados como
na referência. Só busca web é permitida (Claude também WebFetch); ações do assistente GNOME não
fazem parte deste chat.

## Adaptações e limites

- Um processo por envio, encerrado no final, com transcrição JSON das mensagens recentes; não
  mantém os processos nativos/resume entre envios. Até 50 mensagens em memória; respostas até 32 KiB,
  entrada até 2000 caracteres, evento até 1 MiB e timeout de 3 minutos. Sessão limpa ao trocar de IA.
- Tokio aguarda um canal quando ocioso. Deltas viram snapshots com atualização limitada a cada
  30 ms e final garantido; AppEvent acorda o Waker. Sem timer de animação ou consulta periódica do chat.
- Processos ocultos em Job Object com kill-on-close no Windows; cancelamento, timeout e saída da ilha
  encerram os filhos. IDs de geração descartam eventos antigos após Parar/Nova/troca de IA.
- Texto simples e quebras de linha; sem seleção/cópia de resposta, Markdown rico, seletor de modelo,
  persistência/restauração do histórico ou voz. Go entrega texto por etapa, conforme o CLI de origem.
- Nenhuma alteração em Timer, clima, lembretes ou layout compacto. Integração compartilhada restrita
  a estado/ações/entrada/eventos e dispatch do desenho da IA; `config.rs` e `window.rs` intactos.

## Verificação

`cargo test --workspace --offline`: 278 testes passaram (43 IA, 115 core, 46 plataforma,
36 renderizador, 38 app), mais doctests sem falhas. Inclui as alterações finais de credenciais.
`cargo build --release --offline`: concluído em 2m53s; artefato `target/release/isle-win.exe`,
compilado sem executar a ilha.
Comandos offline e resultados finais registrados em `target/ai-tests.log` e `target/ai-release.log`.
Previews em `target/ai-previews/100` e `target/ai-previews/150`: `expanded.png`, `ai-empty.png`,
`ai-typing.png`, `ai-streaming.png`, `ai-history.png`, `ai-error.png`, conferidos visualmente nas
duas escalas. As demais telas do exemplo também são geradas.

Testes cobrem pedidos JSON, deltas/finais/erros dos três protocolos, UTF-8 dividido, linhas grandes,
saída inválida/código de erro de processo de teste, credenciais renovadas/logout concorrente,
histórico limitado, geração antiga, hitboxes, scroll, digitação Unicode, Esc, troca de aba,
monitores e cancelamento. O teste de processo usa somente `cmd.exe` com texto fixo, sem rede.

Não executado: `isle-win.exe`, serviços reais ou chamadas externas. Continuam pendentes login e
respostas reais dos três provedores, atualização de tokens, captura de teclado no desktop,
monitores/DPI reais, distribuição dos CLIs instalados e medição de CPU em repouso.
Previews e testes não substituem essas validações.

Mensagem de commit: `target/ai-commit-message.txt`; nenhum `git add`/`git commit` executado.

# Phase 3 — plano de implementação

Especificação: pedido Phase 3 e docs/ROADMAP.md. Execução direta em feat/isle-win, sem commits.

Arquitetura: portar GetVolume/StatusEvents/ReadMixerApps e Island::QueryNetwork/RadioThreadProc/ParseNightLight/SetNightLight do Mac Island & Dock v3.1.0. Workers MTA recebem comandos por canal; notificações de áudio acordam a UI. Leitura do mixer e controles a cada 3 segundos somente com a central aberta. APIs indisponíveis reportam erro ou abrem Configurações, sem sucesso simulado.

1. Escrever testes falhando para conversão/roda de volume e parser/alteração de luz noturna; portar APIs Windows com stubs Linux que retornam erro.
2. Acrescentar estado próprio, grade, sliders e hitboxes compartilhados; testes primeiro para limites e drag; central de 520x360, demais abas de 520x260.
3. Conectar workers, AppEvents, abertura/fechamento, wheel e OSD sem substituir cards de permissão nem fechar uma central aberta; testar comportamento na aplicação.
4. Renderizar e inspecionar previews em 1x/1,5x, testar workspace, compilar Windows sem warnings, empacotar e registrar tamanho/hash; atualizar roadmap.

Verificar especialmente: dispositivo de áudio ausente/trocado; drag fora da janela; dados CloudStore inválidos; permissões de localização/rádio negadas; mudanças de volume durante um card de permissão. Não alterar isle-ai ou módulos upstream byte-idênticos. Sem pacotes novos: features de windows e referência direta a windows-core 0.58, já transitivo, exigida pela macro implement dos callbacks COM.

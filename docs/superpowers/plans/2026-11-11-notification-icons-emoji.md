# Avisos: ícones reais e emojis coloridos

Pedido: corrigir apenas a identificação visual dos remetentes e os emojis, mantendo o layout Cimori.
Referência: Mac Island & Dock v3.1.1, `LoadAppIconBitmap`, `NotificationPanel::AppBitmap/Draw`.

## Causas confirmadas

- O leitor já porta o banco/WNF do mac-island e mantém Unicode; não transporta o ícone do remetente.
- O renderizador usa Inter/ab_glyph sem fallback colorido e desenha `Icon::Message` para todo app.

## Implementação

- [x] Testes RED: backend de texto e campo/setter ausentes; GREEN com pixels coloridos e ícone tardio preservando lido/conteúdo. Parser preserva emojis/ZWJ; candidatos Shell respeitam AUMID/package/local path.
- [x] Texto de título, corpo e resposta dos avisos: DirectWrite/Direct2D com fallback do Windows e `ENABLE_COLOR_FONT`, rasterizado em WIC e composto no tiny-skia. Medida, corte e shaping na mesma API. Cache limitado, por DPI/tamanho/cor/peso. Resto da interface continua Inter.
- [x] Ícones: Shell `AppsFolder` por AUMID, fallback `package!App`, executável local/registro e imagem padrão de sistema para registro sem IconUri. Extração em worker dedicado, cache/negative cache limitado, sem rede nem polling de UI. NotificationEvent de imagem atualiza itens existentes e snapshot do banner sem novo aviso.
- [x] Testes focados, workspace (308/308), prévias nativas em 100/150/200% e revisão independente.
- [x] Release offline concluído, revisão `visual 2026-11-11-r2`.
- [x] Backup e atualização da instalação em `C:\Tools\isle-win`, hash/versão/processo verificados: PID 740 respondendo, porta 47830 e cópias idênticas. Evidências em `docs/NOTIFICATIONS-VALIDATION.md`.

Sem alterações na ponte Windhawk, nas credenciais, nas configurações ou nos protocolos de recebimento/ativação/resposta.

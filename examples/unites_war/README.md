# Unites War

Estratégia em tempo real com visão lateral, inspirada em **Clan Wars: Goblin
Forest**. Recrute um exército goblin, defenda sua fortaleza e destrua a torre rival.

O cenário, as tropas e o herói são feitos no Blender e exportados como sprites
incorporados ao executável. O jogo usa renderização 2D; não carrega malhas ou
esqueletos 3D em tempo real. Blender só é necessário para editar e exportar a arte.

## Executar e testar

Execute os comandos deste README na raiz do repositório:

```sh
cargo run --release --bin unites_war
```

No menu, clique em **Jogar** e escolha a dificuldade; `Enter` confirma.
As configurações permitem ajustar resolução e tela cheia.

| Dificuldade | Comportamento inimigo | Multiplicador de renda |
| --- | --- | ---: |
| Batedor | Decisões mais lentas, composição simples e melhorias tardias | 0,72 |
| Guerreiro | Adapta tropas, protege arqueiros e investe quando tem segurança | 1,0 |
| Senhor da Guerra | Reage e investe mais cedo; concentra disparos em alvos vulneráveis | 1,3 |

O inimigo usa ouro, população, XP e os mesmos custos e limites de melhorias do jogador.

Para entrar diretamente em uma cena de teste, acrescente uma opção ao comando:

```sh
cargo run --release --bin unites_war -- --test-army
```

| Opção | Atalho no jogo | Cena |
| --- | --- | --- |
| `--test-runner` | `F6` | Saqueadores dos dois exércitos |
| `--test-map` | `F7` | Floresta a partir da torre inicial |
| `--test-army` | `F8` | Todas as tropas e herói, com ouro e XP para testes |
| `--test-cannons` | — | Tropas próximas aos dois castelos para testar os canhões |

## Interface e controles

Os botões **Tropas**, **Melhorias**, **Magias**, **Herói** e **Torre** ficam no canto
inferior esquerdo e abrem um painel por vez na lateral. Clique novamente ou use
`Esc` para fechar. Passe o mouse sobre os comandos para ver suas descrições.
Cadeados indicam requisitos de desbloqueio; recargas mostram o tempo restante.

O topo mostra ouro, XP, população e reputação. A vida das bases aparece no campo,
e o minimapa fica no canto inferior direito. A fila de treinamento aparece enquanto
há unidades em preparação. A aba Herói reúne convocação, avanço, recuo e Fúria.

| Entrada | Ação |
| --- | --- |
| `1`–`4` | Recrutar saqueador, guardião, arqueiro e ogro |
| `5` | Convocar o herói |
| Botões Avançar / Recuar na aba Herói | Controlar o movimento do herói |
| `T` / `M` / `G` / `C` / `U` | Alternar Tropas / Melhorias / Magias / Herói / Torre |
| `Q` / `E` | Selecionar Raio / Meteoro; clique no campo para conjurar |
| `W` | Ativar Fúria |
| Setas ou mouse nas bordas do campo | Mover a câmera |
| Clique no minimapa / `Home` | Ir à região escolhida / voltar à base |
| `Espaço` | Pausar ou continuar |
| `R` | Reiniciar na mesma dificuldade |
| `Esc` | Cancelar magia, fechar painel ou voltar ao menu |

## Combate e progressão

- A partida começa com 120 de ouro, renda de 6 por segundo e limite de 20 de população.
- Recrutar cobra ouro e reserva população imediatamente; o treinamento leva 0,85 s por unidade.
- Abates concedem ouro, XP e reputação. XP é acumulativo e não é gasto nos desbloqueios.
- Aliados podem se ultrapassar durante a marcha. Ao alcançar aliados em combate,
  os reforços formam fila; arqueiros disparam da retaguarda quando estão ao alcance.
  Essa regra vale para os dois exércitos.
- Flechas causam dano no impacto; ataques corpo a corpo usam o marcador da animação.
  Os canhões acompanham o alvo e disparam bolas que explodem no ponto visado,
  causando dano em área sem fogo amigo. Sair da área durante o voo evita a explosão.
- A pausa congela economia, combate, treinamento, reparos e recargas.

| Tropa | Ouro | População | XP necessário | Papel |
| --- | ---: | ---: | ---: | --- |
| Saqueador | 30 | 1 | 0 | Infantaria rápida |
| Guardião | 50 | 2 | 0 | Linha de frente com escudo |
| Arqueiro | 65 | 1 | 120 | Apoio à distância |
| Ogro | 100 | 3 | 360 | Unidade pesada |

### Melhorias

A aba **Melhorias** oferece três níveis de **Armas** (+20% de dano por nível) e
**Armadura** (+20% de vida por nível, preservando a proporção de vida das tropas).
A população permanece em 20; não há melhoria de mineração.

A aba **Torre** oferece cinco melhorias independentes, com três níveis cada,
ao custo de **120 / 220 / 320 de ouro**:

| Melhoria | Efeito por nível |
| --- | --- |
| Tiro múltiplo | +1 bola por salva em dispersão, até quatro, mesmo com um único inimigo |
| Velocidade de ataque | +20% na frequência de disparos |
| Área de dano | +15 no raio da explosão, partindo de 55 |
| Reparo | Recupera 3 de vida por segundo, até o máximo; não revive torres |
| Vida máxima | +200 de vida máxima e atual, preservando o dano já sofrido |

As melhorias de torre não alteram as tropas.

### Herói e magias

O **herói arqueiro** é liberado com 250 XP e não ocupa população. Cada flecha
atinge um único alvo, com dano adicional ao mirar diretamente na torre.
Avanço e recuo são controlados pelos botões da aba Herói. Ao recuar até o próprio
castelo, ele entra e sai do campo; após **30 segundos**, pode ser convocado
novamente com vida completa. A mesma espera vale após sua derrota.

| Habilidade | Efeito | XP necessário | Recarga |
| --- | --- | ---: | ---: |
| Raio | Dano em área | 0 | 15 s |
| Meteoro | Área maior; também danifica a torre rival | 500 | 32 s |
| Fúria | Dobra o dano do herói por 8 s | Herói em campo | 28 s |

Raio e Meteoro ficam na aba Magias e exigem selecionar um alvo no campo.
Fúria fica na aba Herói. As habilidades não custam ouro.

## Editar a arte no Blender

Edite e salve as fontes integradas antes de exportar:

| Conteúdo | Fonte editável | Exportador em `tools/` |
| --- | --- | --- |
| Saqueador | [assets/goblin_saqueador/source.blend](assets/goblin_saqueador/source.blend) | [export_goblin_sprites.py](tools/export_goblin_sprites.py) |
| Arqueiro goblin, guardião e ogro | [assets/tropas/source.blend](assets/tropas/source.blend) | [export_character_sprites.py](tools/export_character_sprites.py), grupo `troops` |
| Herói com arco preto recurvo | [assets/arqueiro_heroi/source.blend](assets/arqueiro_heroi/source.blend) | [export_character_sprites.py](tools/export_character_sprites.py), grupo `hero` |
| Floresta e castelos | [assets/forest/source.blend](assets/forest/source.blend) | [export_forest_sprites.py](tools/export_forest_sprites.py) |

Exporte apenas o conteúdo alterado. Para tropas e herói:

```sh
blender --background examples/unites_war/assets/tropas/source.blend \
  --python-exit-code 1 \
  --python examples/unites_war/tools/export_character_sprites.py -- --group troops
blender --background examples/unites_war/assets/arqueiro_heroi/source.blend \
  --python-exit-code 1 \
  --python examples/unites_war/tools/export_character_sprites.py -- --group hero
```

Para o saqueador ou cenário:

```sh
blender --background examples/unites_war/assets/goblin_saqueador/source.blend \
  --python-exit-code 1 \
  --python examples/unites_war/tools/export_goblin_sprites.py -- \
  --output examples/unites_war/assets/goblin_saqueador
blender --background examples/unites_war/assets/forest/source.blend \
  --python-exit-code 1 \
  --python examples/unites_war/tools/export_forest_sprites.py -- \
  --output examples/unites_war/assets/forest
```

Esses exportadores preservam o `.blend` e geram PNGs, manifestos e referências
Rust. Use `--samples` após `--` para ajustar a qualidade. Depois, recompile para
incorporar os novos sprites:

```sh
cargo build --release --bin unites_war
```

O canhão e a bola ficam em [assets/cannon/source.blend](assets/cannon/source.blend),
com GLBs separados. [build_cannon.py](tools/build_cannon.py) recria os modelos e
as imagens usadas pelo jogo; o exportador da floresta oculta os canhões fixos
para permitir a mira móvel no jogo.

Os scripts de construção e animação originais permanecem em [tools/](tools/).
Reaproveite as fontes atuais, materiais e rigs; após edições manuais, use os
exportadores. Construtores e migrações antigas podem recriar ou substituir
partes já ajustadas. Para prévias leves, use o viewport sólido e menos amostras.

Os ícones de espada, escudo e cadeado ficam em [assets/ui_icons/](assets/ui_icons/).
A origem e os prompts de geração estão em [generation.json](assets/ui_icons/generation.json).

## Validação

```sh
cargo test -p unites_war
cargo clippy -p unites_war --all-targets -- -D warnings
```

Os testes cobrem combate, projéteis, formação, população, progressão, melhorias,
IA, interface, pausa e recuperação do herói.

Referência de mecânicas: [Clan Wars — Goblins Forest](https://www.kongregate.com/en/games/ffgameplayer/clan-wars-goblins-forest).

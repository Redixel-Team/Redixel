# Unites War

A playable side-view strategy prototype inspired by the unit production and
fortress combat loop of classic browser war games.

The battlefield mixes projected 2.5D geometry with pixel-art textures. The
Shadow Archer and clan fortresses extrude their own transparent silhouettes in
layers, while units, controls, projectiles, shadows, and effects retain the
low-poly shape language without requiring a native 3D model pipeline.

## Controls

- `Enter` — open the stage selection and start the available stage
- `1` — recruit a runner (30 coins)
- `2` — recruit a guard (50 coins)
- `3` — recruit an archer (65 coins)
- `4` — recruit a brute (100 coins)
- `Q` — cast free lightning at the mouse cursor (15-second cooldown)
- `U` — upgrade the clan (cost increases each level)
- `H` — open or close the skills panel
- `A` / `D` or arrow keys — retreat or advance with the Shadow Archer hero
- `R` — restart the battle
- `Esc` — return from stage selection or exit during battle

The initial menu can also be controlled with the mouse through the `JOGAR` and
`SAIR` buttons. `JOGAR` opens a stage-selection screen where Stage 1 contains
the current battle and the future stage slots are marked `EM BREVE`. Its labels
use a built-in 5x7 bitmap font rendered entirely with Redixel rectangles.

Coins are generated passively during battle and awarded whenever an opposing
unit is defeated. Stronger units grant larger bounties. The four coloured cards
at the bottom can be clicked to add units to a FIFO recruitment queue. Their
cost is paid immediately and one queued unit is summoned every 0.85 seconds.
The golden fifth card buys a base upgrade. Clicking the battlefield casts
lightning when it is ready and enough coins are available.

During battle, the `HABILIDADES` button opens a paused shop where coins can buy
one passive for each unit type: Momentum, Bulwark, Piercing Shot, and Rage.
Purchases affect current and future player units for the rest of that battle.
Close the panel with `H`, `Esc`, or the `FECHAR` button.

Stage 1 also starts with **Arqueira Sombria**, a controllable hero with low
health and the same attack range as a regular archer. Her arrows pierce up to
four enemy troops and deal bonus damage to the enemy fortress when she advances
close enough. The player only controls her movement direction; targeting and
firing are automatic. Her targeting reticle cycles through three visual styles
after every shot.

## Run

```sh
cargo run --release --bin unites_war
```

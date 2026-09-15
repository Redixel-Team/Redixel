use super::*;

include!("../../assets/forest/tiles.rs");

#[derive(Clone, Copy)]
pub(super) struct ForestSprites {
    tiles: [TextureId; TILE_PNG.len()],
}

impl ForestSprites {
    pub(super) fn load(ctx: &mut dyn GameContext<Action>) -> Self {
        debug_assert_eq!(MAP_WIDTH, WORLD_WIDTH);
        debug_assert_eq!(MAP_HEIGHT, WORLD_HEIGHT);
        Self {
            tiles: TILE_PNG.map(|png| ctx.load_texture_filtered(png, TextureFilter::Linear)),
        }
    }

    fn tile_bounds(index: usize) -> (f32, f32) {
        let start = index as f32 * TILE_WIDTH - BLEED;
        (start, start + TILE_WIDTH + 2.0 * BLEED)
    }

    fn visible(index: usize, camera: f32, visible_width: f32) -> bool {
        let (start, end) = Self::tile_bounds(index);
        end > camera && start < camera + visible_width
    }

    pub(super) fn draw(&self, ctx: &mut dyn GameContext<Action>, camera: f32, width: f32, height: f32) {
        let scale = UnitesWar::view_scale(width, height);
        // Extend the soil below the exported map in windows taller than 16:9.
        ctx.clear_color(Color::from_rgba8(76, 71, 52, 255));
        for (index, texture) in self.tiles.iter().enumerate() {
            if !Self::visible(index, camera, width / scale) {
                continue;
            }
            let (start, end) = Self::tile_bounds(index);
            // Overlapping edge pixels come from one render, so linear filtering
            // cannot expose a crack when the camera falls between game pixels.
            ctx.draw_sprite(
                Vec2::new((start - camera) * scale, 0.0),
                Vec2::new(end - start, MAP_HEIGHT) * scale,
                *texture,
            );
        }
    }
}

impl UnitesWar {
    pub(super) fn start_map_test(&mut self) {
        self.start_runner_test();
        self.clan.camera = 0.0;
        self.clan.notice = "TESTE DA FLORESTA: SETAS MOVEM A CAMERA | ESPACO PAUSA | F7 REINICIA".into();
        self.clan.notice_time = 10.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_tiles_cover_the_world_with_matching_scale_and_filter_bleed() {
        assert_eq!(MAP_WIDTH, WORLD_WIDTH);
        assert_eq!(MAP_HEIGHT, WORLD_HEIGHT);
        assert_eq!(TILE_WIDTH * TILE_PNG.len() as f32, WORLD_WIDTH);
        assert!(ForestSprites::tile_bounds(0).0 <= 0.0);
        assert!(ForestSprites::tile_bounds(TILE_PNG.len() - 1).1 >= WORLD_WIDTH);
        for (index, bytes) in TILE_PNG.iter().enumerate() {
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert_eq!(width as f32, (TILE_WIDTH + 2.0 * BLEED) * 2.0);
            assert_eq!(height as f32, MAP_HEIGHT * 2.0);
            assert!(width <= 2048 && height <= 2048, "Keep WebGPU's baseline texture limit");
            assert_eq!(bytes[25], 2, "Backgrounds must be opaque RGB");
            if index > 0 {
                let previous_end = ForestSprites::tile_bounds(index - 1).1;
                assert_eq!(previous_end - ForestSprites::tile_bounds(index).0, 2.0 * BLEED);
            }
        }
    }

    #[test]
    fn camera_culling_keeps_the_tiles_on_both_sides_of_a_seam_at_every_resolution() {
        for (width, height) in [(960.0, 540.0), (1280.0, 720.0), (1366.0, 768.0), (1920.0, 1080.0)] {
            let visible = width / UnitesWar::view_scale(width, height);
            for camera in [0.0, 799.5, WORLD_WIDTH - visible] {
                for offset in [0.0, visible / 2.0, visible - 0.01] {
                    let x = camera + offset;
                    assert!((0..TILE_PNG.len()).any(|index| {
                        let (start, end) = ForestSprites::tile_bounds(index);
                        ForestSprites::visible(index, camera, visible) && x >= start && x <= end
                    }));
                }
            }
        }
        assert!(!ForestSprites::visible(2, 0.0, 960.0));
        assert!(!ForestSprites::visible(0, 1440.0, 960.0));
    }

    #[test]
    fn restarts_keep_the_map_loaded_for_normal_and_test_battles() {
        let mut game = UnitesWar::new();
        let tiles = std::array::from_fn(|index| TextureId::new(100 + index as u32));
        game.forest_sprites = Some(ForestSprites { tiles });
        for restart in [
            UnitesWar::start_map_test,
            UnitesWar::start_battle,
            UnitesWar::start_runner_test,
        ] {
            restart(&mut game);
            assert_eq!(game.forest_sprites.unwrap().tiles, tiles);
            assert_eq!(game.screen, ScreenState::Battle);
        }
    }
}

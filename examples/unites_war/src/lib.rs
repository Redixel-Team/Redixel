mod game;

use game::UnitesWar;

redixel::entry_point! {
    game: UnitesWar::new(),
    log_tag: "UNITES_WAR",
}

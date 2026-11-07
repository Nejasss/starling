// `/wp ...`, before Horizon's own commands see it.
fn waymap_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::waymap::command(game, command, ran);
}

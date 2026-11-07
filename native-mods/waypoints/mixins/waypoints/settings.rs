// The mod's settings page: labels in the world and death waypoints on or off.
fn waypoints_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::waypoints_mod as m;
    if mod_id != m::ID {
        return;
    }
    let labels = game.native_setting(m::ID, "labels");
    let deaths = game.native_setting(m::ID, "deaths");
    let (labels, deaths) = m::with(|s| {
        let labels = *s.labels.get_or_insert_with(|| m::read_on(labels));
        (labels, *s.deaths.get_or_insert_with(|| m::read_on(deaths)))
    });
    buttons.push(m::labels_label(labels));
    buttons.push(m::deaths_label(deaths));
}

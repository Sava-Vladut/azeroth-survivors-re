//! The settings page: camera drag and wheel-zoom sensitivity (with inverts), the four sound
//! volumes, the interface size and the picture polish. It opens from a button on the pick screen
//! and on the pause screen, edits [`Settings`] live, and saves on release to
//! `benilla-config/survivors-settings.txt` (`localStorage` on the page), one `key=value` per line.
//!
//! The panel is a full-screen overlay, not a [`super::Phase`]: opening it never moves the game
//! between screens, so the run stays paused behind it and closing it lands back where it came from.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use super::hud::{self, DIM_GREY, GOLD_TONE};
use crate::sound::SoundConfig;

/// The interface sizes the button cycles through, times the reference's 768-unit-tall layout. Past
/// 110% the canvas gets too short for the pick screen's zone frame (657 tall), so it stops there.
pub(super) const UI_SCALES: [f32; 4] = [0.75, 0.9, 1.0, 1.1];

/// What the sound sliders start at when the page has nothing saved: the reference's own
/// registrations ([`SoundConfig`]'s defaults).
const SOUND_DEFAULTS: [f32; 4] = [1.0, 0.4, 1.0, 0.6];

const EDGE: Color = Color::srgb(0.55, 0.45, 0.25);
const PANEL: Color = Color::srgba(0.05, 0.035, 0.025, 0.97);
const TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
const FILL: Color = Color::srgb(0.78, 0.6, 0.12);
const BUTTON: Color = Color::srgba(0.18, 0.06, 0.04, 0.95);
const BUTTON_HOT: Color = Color::srgba(0.36, 0.12, 0.06, 0.98);
const ON: Color = Color::srgba(0.16, 0.34, 0.12, 0.95);
const ON_HOT: Color = Color::srgba(0.22, 0.46, 0.16, 0.98);

/// Everything the page edits. A sound volume is `None` until the player moves it, which leaves the
/// engine's own value (and whatever the client's config set) alone.
#[derive(Resource, Clone, PartialEq, Debug)]
pub(super) struct Settings {
    /// Camera drag speed, a multiple of the base rate.
    pub camera_sens: f32,
    /// Drag down tilts the camera up instead of down.
    pub invert_y: bool,
    /// Wheel zoom speed, a multiple of the base rate.
    pub zoom_speed: f32,
    /// Wheel up zooms out instead of in.
    pub invert_zoom: bool,
    /// The interface size, a multiple of the screen-height layout; one of [`UI_SCALES`].
    pub ui_scale: f32,
    /// The picture polish ([`super::grade`]); `G` flips it in play too.
    pub polish: bool,
    pub master: Option<f32>,
    pub music: Option<f32>,
    pub sfx: Option<f32>,
    pub ambience: Option<f32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            camera_sens: 1.0,
            invert_y: false,
            zoom_speed: 1.0,
            invert_zoom: false,
            ui_scale: 1.0,
            // `?grade=0` / `WOW_GRADE=0` starts it off.
            polish: crate::webenv::var("WOW_GRADE").is_none_or(|v| v != "0"),
            master: None,
            music: None,
            sfx: None,
            ambience: None,
        }
    }
}

/// Whether the page is showing.
#[derive(Resource, Default)]
pub(super) struct SettingsOpen(pub bool);

/// A row of the page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    CameraSens,
    InvertY,
    ZoomSpeed,
    InvertZoom,
    Master,
    Music,
    Sfx,
    Ambience,
    UiScale,
    Polish,
}

impl Field {
    /// `(min, max, step)` of a slider row.
    fn range(self) -> (f32, f32, f32) {
        match self {
            Self::CameraSens | Self::ZoomSpeed => (0.25, 3.0, 0.05),
            _ => (0.0, 1.0, 0.01),
        }
    }

    fn is_sound(self) -> bool {
        matches!(self, Self::Master | Self::Music | Self::Sfx | Self::Ambience)
    }
}

impl Settings {
    /// A slider's value; a sound volume not set yet reads the engine's current one.
    fn get(&self, f: Field, snd: Option<&SoundConfig>) -> f32 {
        match f {
            Field::CameraSens => self.camera_sens,
            Field::ZoomSpeed => self.zoom_speed,
            Field::Master => self.master.or(snd.map(|s| s.master)).unwrap_or(SOUND_DEFAULTS[0]),
            Field::Music => self.music.or(snd.map(|s| s.music)).unwrap_or(SOUND_DEFAULTS[1]),
            Field::Sfx => self.sfx.or(snd.map(|s| s.sfx)).unwrap_or(SOUND_DEFAULTS[2]),
            Field::Ambience => self
                .ambience
                .or(snd.map(|s| s.ambience))
                .unwrap_or(SOUND_DEFAULTS[3]),
            _ => 0.0,
        }
    }

    fn set(&mut self, f: Field, v: f32) {
        let (lo, hi, _) = f.range();
        let v = v.clamp(lo, hi);
        match f {
            Field::CameraSens => self.camera_sens = v,
            Field::ZoomSpeed => self.zoom_speed = v,
            Field::Master => self.master = Some(v),
            Field::Music => self.music = Some(v),
            Field::Sfx => self.sfx = Some(v),
            Field::Ambience => self.ambience = Some(v),
            _ => {}
        }
    }

    fn flag(&self, f: Field) -> bool {
        match f {
            Field::InvertY => self.invert_y,
            Field::InvertZoom => self.invert_zoom,
            Field::Polish => self.polish,
            _ => false,
        }
    }

    /// A toggle flips; the interface size steps to the next of [`UI_SCALES`], wrapping.
    fn press(&mut self, f: Field) {
        match f {
            Field::InvertY => self.invert_y = !self.invert_y,
            Field::InvertZoom => self.invert_zoom = !self.invert_zoom,
            Field::Polish => self.polish = !self.polish,
            Field::UiScale => {
                let now = UI_SCALES
                    .iter()
                    .position(|&s| (s - self.ui_scale).abs() < 1e-3)
                    .unwrap_or(2);
                self.ui_scale = UI_SCALES[(now + 1) % UI_SCALES.len()];
            }
            _ => {}
        }
    }

    /// Every row back at its default, the sound ones set outright so the engine follows.
    fn reset() -> Self {
        Self {
            master: Some(SOUND_DEFAULTS[0]),
            music: Some(SOUND_DEFAULTS[1]),
            sfx: Some(SOUND_DEFAULTS[2]),
            ambience: Some(SOUND_DEFAULTS[3]),
            ..Self::default()
        }
    }

    #[cfg(test)]
    fn to_text(&self) -> String {
        self.to_text_with(self.polish)
    }

    /// The file's text with `polish` written as given, so a session override is not saved.
    fn to_text_with(&self, polish: bool) -> String {
        let b = |v: bool| u8::from(v);
        let mut s = format!(
            "camera_sens={:.2}\ninvert_y={}\nzoom_speed={:.2}\ninvert_zoom={}\nui_scale={:.2}\npolish={}\n",
            self.camera_sens,
            b(self.invert_y),
            self.zoom_speed,
            b(self.invert_zoom),
            self.ui_scale,
            b(polish),
        );
        for (key, v) in [
            ("master", self.master),
            ("music", self.music),
            ("sfx", self.sfx),
            ("ambience", self.ambience),
        ] {
            if let Some(v) = v {
                s.push_str(&format!("{key}={v:.2}\n"));
            }
        }
        s
    }

    /// Lenient: unknown keys, bad numbers and out-of-range values leave the default (or clamp).
    fn parse(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let (key, val) = (key.trim(), val.trim());
            let num = val.parse::<f32>().ok().filter(|v| v.is_finite());
            let flag = match val {
                "1" | "true" => Some(true),
                "0" | "false" => Some(false),
                _ => None,
            };
            match key {
                "camera_sens" => num.into_iter().for_each(|v| out.set(Field::CameraSens, v)),
                "zoom_speed" => num.into_iter().for_each(|v| out.set(Field::ZoomSpeed, v)),
                "invert_y" => flag.into_iter().for_each(|v| out.invert_y = v),
                "invert_zoom" => flag.into_iter().for_each(|v| out.invert_zoom = v),
                "polish" => flag.into_iter().for_each(|v| out.polish = v),
                "ui_scale" => {
                    // Snap to the nearest size on offer, so a hand edit cannot wedge the cycle.
                    if let Some(v) = num {
                        out.ui_scale = *UI_SCALES
                            .iter()
                            .min_by(|a, b| (*a - v).abs().total_cmp(&(*b - v).abs()))
                            .unwrap_or(&1.0);
                    }
                }
                "master" => num.into_iter().for_each(|v| out.set(Field::Master, v)),
                "music" => num.into_iter().for_each(|v| out.set(Field::Music, v)),
                "sfx" => num.into_iter().for_each(|v| out.set(Field::Sfx, v)),
                "ambience" => num.into_iter().for_each(|v| out.set(Field::Ambience, v)),
                _ => {}
            }
        }
        out
    }
}

fn path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("survivors-settings.txt"))
}

/// The polish the file held when `?grade=0` overrode it for this session, so saving other
/// changes does not turn the saved choice off for good.
#[derive(Resource, Default)]
struct SavedPolish(Option<bool>);

/// Read the saved settings once at startup. A missing or unreadable file keeps the defaults.
fn load(mut settings: ResMut<Settings>, mut saved: ResMut<SavedPolish>) {
    let Some(text) = path().and_then(|p| crate::local_state::read_to_string(&p).ok()) else {
        return;
    };
    let mut loaded = Settings::parse(&text);
    // `?grade=0` is explicit for this session and outranks the saved choice.
    if crate::webenv::var("WOW_GRADE").is_some_and(|v| v == "0") {
        saved.0 = Some(loaded.polish);
        loaded.polish = false;
    }
    *settings = loaded;
}

/// Save once the player lets go of the mouse, not on every frame of a slider drag.
fn save(
    settings: Res<Settings>,
    saved: Res<SavedPolish>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut seen: Local<bool>,
    mut dirty: Local<bool>,
) {
    if settings.is_changed() {
        // The first sighting is the resource's own insertion and the startup load.
        if *seen {
            *dirty = true;
        }
        *seen = true;
    }
    if *dirty && !mouse.pressed(MouseButton::Left) {
        *dirty = false;
        if let Some(p) = path() {
            let text = settings.to_text_with(saved.0.unwrap_or(settings.polish));
            let _ = crate::local_state::write_atomic(&p, &text);
        }
    }
}

/// Push the sound volumes the player has set into the engine's config.
fn apply_sound(settings: Res<Settings>, snd: Option<ResMut<SoundConfig>>) {
    if !settings.is_changed() {
        return;
    }
    let Some(mut snd) = snd else { return };
    let have = (snd.master, snd.music, snd.sfx, snd.ambience);
    let want = (
        settings.master.unwrap_or(have.0),
        settings.music.unwrap_or(have.1),
        settings.sfx.unwrap_or(have.2),
        settings.ambience.unwrap_or(have.3),
    );
    // Only a real change touches the config, so its own change detection stays quiet.
    if want != have {
        (snd.master, snd.music, snd.sfx, snd.ambience) = want;
    }
}

// --- The panel. ---

#[derive(Component)]
pub(super) struct SettingsRoot;

#[derive(Component)]
struct SliderTrack(Field);

#[derive(Component)]
struct SliderFill(Field);

#[derive(Component)]
struct ValueText(Field);

#[derive(Component)]
struct Pick(Field);

#[derive(Component)]
struct PickText(Field);

#[derive(Component, Clone, Copy)]
enum PanelBtn {
    Reset,
    Back,
}

fn row(
    parent: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    label: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(30.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                hud::text_bundle(f, label, 18.0, Color::WHITE),
            ));
            build(r);
        });
}

fn heading(parent: &mut ChildSpawnerCommands, f: &Handle<Font>, label: &str) {
    parent.spawn((
        Node {
            margin: UiRect::top(Val::Px(10.0)),
            ..default()
        },
        hud::text_bundle(f, label, 20.0, GOLD_TONE),
    ));
}

fn slider(r: &mut ChildSpawnerCommands, f: &Handle<Font>, field: Field) {
    r.spawn(Node {
        align_items: AlignItems::Center,
        column_gap: Val::Px(12.0),
        ..default()
    })
    .with_children(|c| {
        c.spawn((
            Button,
            SliderTrack(field),
            RelativeCursorPosition::default(),
            Node {
                width: Val::Px(230.0),
                height: Val::Px(16.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(TRACK),
            BorderColor::all(EDGE),
        ))
        .with_children(|t| {
            t.spawn((
                SliderFill(field),
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(FILL),
                // The track takes the press, not the bar inside it.
                FocusPolicy::Pass,
            ));
        });
        c.spawn((
            ValueText(field),
            Node {
                width: Val::Px(62.0),
                ..default()
            },
            hud::text_bundle(f, "", 16.0, GOLD_TONE),
        ));
    });
}

fn pick(r: &mut ChildSpawnerCommands, f: &Handle<Font>, field: Field) {
    r.spawn((
        Button,
        Pick(field),
        Node {
            width: Val::Px(110.0),
            height: Val::Px(26.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(BUTTON),
        BorderColor::all(EDGE),
    ))
    .with_children(|b| {
        b.spawn((PickText(field), hud::text_bundle(f, "", 16.0, Color::WHITE)));
    });
}

fn panel_button(r: &mut ChildSpawnerCommands, f: &Handle<Font>, label: &str, action: PanelBtn) {
    r.spawn((
        Button,
        action,
        Node {
            width: Val::Px(200.0),
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(BUTTON),
        BorderColor::all(EDGE),
    ))
    .with_children(|b| {
        b.spawn(hud::text_bundle(f, label, 20.0, Color::WHITE));
    });
}

/// The page, hidden until [`SettingsOpen`]; spawned with the rest of the HUD.
pub(super) fn spawn_panel(commands: &mut Commands, f: &Handle<Font>) {
    commands
        .spawn((
            SettingsRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(450),
            Visibility::Hidden,
            // Nothing under the page, the pick screen's buttons included, takes the click.
            FocusPolicy::Block,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(620.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(28.0), Val::Px(20.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
            ))
            .with_children(|p| {
                p.spawn(hud::centered_bundle(f, "Settings", 40.0, GOLD_TONE));

                heading(p, f, "Camera");
                row(p, f, "Camera drag speed", |r| slider(r, f, Field::CameraSens));
                row(p, f, "Invert camera tilt", |r| pick(r, f, Field::InvertY));
                row(p, f, "Mouse wheel zoom speed", |r| slider(r, f, Field::ZoomSpeed));
                row(p, f, "Invert wheel zoom", |r| pick(r, f, Field::InvertZoom));

                heading(p, f, "Sound");
                row(p, f, "Master volume", |r| slider(r, f, Field::Master));
                row(p, f, "Music", |r| slider(r, f, Field::Music));
                row(p, f, "Effects", |r| slider(r, f, Field::Sfx));
                row(p, f, "Ambience", |r| slider(r, f, Field::Ambience));

                heading(p, f, "Display");
                row(p, f, "Interface size", |r| pick(r, f, Field::UiScale));
                row(p, f, "Picture polish (G)", |r| pick(r, f, Field::Polish));

                p.spawn(Node {
                    margin: UiRect::top(Val::Px(16.0)),
                    column_gap: Val::Px(14.0),
                    ..default()
                })
                .with_children(|b| {
                    panel_button(b, f, "Reset to defaults", PanelBtn::Reset);
                    panel_button(b, f, "Back", PanelBtn::Back);
                });
                p.spawn(hud::centered_bundle(
                    f,
                    "Changes apply at once and are saved.",
                    13.0,
                    DIM_GREY,
                ));
            });
        });
}

/// Show the page while it is open; a hidden one leaves the layout, so it takes no clicks.
fn show(open: Res<SettingsOpen>, mut roots: Query<(&mut Visibility, &mut Node), With<SettingsRoot>>) {
    if !open.is_changed() {
        return;
    }
    for (mut v, mut node) in &mut roots {
        *v = if open.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if open.0 { Display::Flex } else { Display::None };
    }
}

/// Sliders follow the cursor while held; toggles flip on the press; the two buttons act.
#[allow(clippy::too_many_arguments)]
fn interact(
    mut settings: ResMut<Settings>,
    mut open: ResMut<SettingsOpen>,
    snd: Option<Res<SoundConfig>>,
    tracks: Query<(&SliderTrack, &Interaction, &RelativeCursorPosition)>,
    picks: Query<(&Pick, &Interaction), Changed<Interaction>>,
    buttons: Query<(&PanelBtn, &Interaction), Changed<Interaction>>,
) {
    if !open.0 {
        return;
    }
    for (track, interaction, cursor) in &tracks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(pos) = cursor.normalized else { continue };
        let (lo, hi, step) = track.0.range();
        let raw = lo + (pos.x + 0.5).clamp(0.0, 1.0) * (hi - lo);
        let v = (raw / step).round() * step;
        if (settings.get(track.0, snd.as_deref()) - v).abs() > 1e-4 {
            settings.set(track.0, v);
        }
    }
    for (pick, interaction) in &picks {
        if *interaction == Interaction::Pressed {
            settings.press(pick.0);
        }
    }
    for (button, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            PanelBtn::Reset => *settings = Settings::reset(),
            PanelBtn::Back => open.0 = false,
        }
    }
}

/// Keep the bars, numbers and toggle faces in step with [`Settings`].
#[allow(clippy::type_complexity)]
fn refresh(
    settings: Res<Settings>,
    open: Res<SettingsOpen>,
    snd: Option<Res<SoundConfig>>,
    mut fills: Query<(&SliderFill, &mut Node)>,
    mut values: Query<(&ValueText, &mut Text), Without<PickText>>,
    mut picks: Query<(&Pick, &Interaction, &mut BackgroundColor)>,
    mut faces: Query<(&PickText, &mut Text), Without<ValueText>>,
) {
    if !open.0 {
        return;
    }
    let snd = snd.as_deref();
    for (fill, mut node) in &mut fills {
        let (lo, hi, _) = fill.0.range();
        let frac = ((settings.get(fill.0, snd) - lo) / (hi - lo)).clamp(0.0, 1.0);
        let w = Val::Percent(frac * 100.0);
        if node.width != w {
            node.width = w;
        }
    }
    for (value, mut text) in &mut values {
        let v = settings.get(value.0, snd);
        let s = if value.0.is_sound() {
            format!("{:.0}%", v * 100.0)
        } else {
            format!("{v:.2}x")
        };
        if text.0 != s {
            text.0 = s;
        }
    }
    for (face, mut text) in &mut faces {
        let s = match face.0 {
            Field::UiScale => format!("{:.0}%", settings.ui_scale * 100.0),
            f if settings.flag(f) => "On".to_string(),
            _ => "Off".to_string(),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
    for (pick, interaction, mut bg) in &mut picks {
        let on = pick.0 == Field::UiScale || settings.flag(pick.0);
        let hot = *interaction != Interaction::None;
        let want = match (on, hot) {
            (true, true) => ON_HOT,
            (true, false) => ON,
            (false, true) => BUTTON_HOT,
            (false, false) => BUTTON,
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Hover colour on the panel's two buttons.
fn hover(mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<PanelBtn>, Changed<Interaction>)>) {
    for (interaction, mut bg) in &mut buttons {
        bg.0 = if *interaction == Interaction::None {
            BUTTON
        } else {
            BUTTON_HOT
        };
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Settings>()
        .init_resource::<SettingsOpen>()
        .init_resource::<SavedPolish>()
        .add_systems(Startup, load)
        .add_systems(
            Update,
            (interact, apply_sound, show, refresh, hover, save).chain(),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips() {
        let s = Settings {
            camera_sens: 1.75,
            invert_y: true,
            zoom_speed: 0.5,
            invert_zoom: true,
            ui_scale: 1.1,
            polish: false,
            master: Some(0.8),
            music: None,
            sfx: Some(0.0),
            ambience: Some(1.0),
        };
        assert_eq!(Settings::parse(&s.to_text()), s);
    }

    #[test]
    fn unset_sound_stays_out_of_the_file() {
        let text = Settings::default().to_text();
        for key in ["master", "music", "sfx", "ambience"] {
            assert!(!text.contains(&format!("{key}=")), "{key} written: {text}");
        }
    }

    #[test]
    fn parse_clamps_and_ignores_garbage() {
        let s = Settings::parse(
            "camera_sens=99\nzoom_speed=-4\nmaster=7\nmusic=abc\nnonsense\n=\nui_scale=1.12\ninvert_y=maybe\nfuture_key=1\nsfx=NaN\n",
        );
        assert_eq!(s.camera_sens, 3.0);
        assert_eq!(s.zoom_speed, 0.25);
        assert_eq!(s.master, Some(1.0));
        assert_eq!(s.music, None);
        assert_eq!(s.sfx, None);
        assert_eq!(s.ui_scale, 1.1);
        assert!(!s.invert_y);
    }

    #[test]
    fn empty_file_is_the_defaults() {
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn interface_size_cycles_and_wraps() {
        let mut s = Settings::default();
        assert_eq!(s.ui_scale, 1.0);
        s.press(Field::UiScale);
        assert_eq!(s.ui_scale, 1.1);
        s.ui_scale = 1.1;
        s.press(Field::UiScale);
        assert_eq!(s.ui_scale, 0.75);
    }

    #[test]
    fn sliders_clamp_and_read_the_engine_until_set() {
        let mut s = Settings::default();
        let snd = SoundConfig::default();
        assert_eq!(s.get(Field::Music, Some(&snd)), 0.4);
        assert_eq!(s.get(Field::Music, None), 0.4);
        s.set(Field::Music, 5.0);
        assert_eq!(s.music, Some(1.0));
        s.set(Field::CameraSens, 0.0);
        assert_eq!(s.camera_sens, 0.25);
    }

    #[test]
    fn reset_sets_every_sound_volume_outright() {
        let r = Settings::reset();
        assert_eq!(
            [r.master, r.music, r.sfx, r.ambience],
            SOUND_DEFAULTS.map(Some)
        );
        assert_eq!(r.camera_sens, 1.0);
    }
}

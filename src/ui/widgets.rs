//! Small reusable UI pieces: fonts, buttons, text fields, panels.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::{observe, Activate, Button};

use crate::render::audio::{PlaySfx, Sfx};

/// Everything a button can do. Buttons only *emit* actions; one system per
/// screen decides what they mean.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    Solo,
    Host,
    Join,
    Quit,
    Start,
    Leave,
    BackToLobby,
    Resume,
    SensDown,
    SensUp,
    VolDown,
    VolUp,
    FovDown,
    FovUp,
    /// Join the n-th game of the public list shown in the menu.
    JoinListed(u8),
    /// Copy the host code to the clipboard.
    CopyCode,
    /// Lobby (host): previous / next map and difficulty.
    MapPrev,
    MapNext,
    DiffPrev,
    DiffNext,
}

pub const TEXT: Color = Color::srgb(0.95, 0.93, 0.9);
pub const TEXT_DIM: Color = Color::srgb(0.72, 0.7, 0.76);
pub const ACCENT: Color = Color::srgb(1.0, 0.62, 0.35);
pub const PANEL: Color = Color::srgba(0.1, 0.08, 0.14, 0.82);
const BUTTON: Color = Color::srgb(0.24, 0.18, 0.32);
const BUTTON_HOVER: Color = Color::srgb(0.38, 0.27, 0.48);
const BUTTON_PRESS: Color = Color::srgb(0.95, 0.55, 0.3);

/// The two UI fonts, loaded once at startup (see `load_fonts`).
static FONTS: std::sync::OnceLock<(Handle<Font>, Handle<Font>)> = std::sync::OnceLock::new();

/// Loads Segoe UI (regular + bold) straight from Windows' font folder: full
/// French accents support. We used `FontSource::sans_serif()` (generic system
/// font) before, but with Bevy 0.20 it allocated a new 512x512 glyph atlas
/// texture *every frame* until the graphics card ran out of memory and the
/// game closed. A fixed font asset is resolved once and cached properly.
pub fn load_fonts(mut fonts: ResMut<Assets<Font>>) {
    let mut load = |file: &str| -> Handle<Font> {
        let path = std::path::Path::new(r"C:\Windows\Fonts").join(file);
        match std::fs::read(&path) {
            Ok(bytes) => fonts.add(Font::from_bytes(bytes)),
            Err(e) => {
                warn!("font {path:?} not found ({e}), using Bevy's default font");
                Handle::default()
            }
        }
    };
    let _ = FONTS.set((load("segoeui.ttf"), load("segoeuib.ttf")));
}

fn font_with(bold: bool, size: f32) -> TextFont {
    let handle = FONTS.get().map(|(r, b)| if bold { b.clone() } else { r.clone() }).unwrap_or_default();
    TextFont::from(FontSource::from(handle)).with_font_size(FontSize::Px(size))
}

pub fn font(size: f32) -> TextFont {
    font_with(false, size)
}

pub fn bold(size: f32) -> TextFont {
    font_with(true, size)
}

pub fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), font(size), TextColor(color), TextShadow::default())
}

#[derive(Component)]
pub struct MenuButton;

pub fn button(text: &str, action: UiAction, width: f32) -> impl Bundle {
    (
        Button,
        MenuButton,
        Hovered::default(),
        Node {
            width: Val::Px(width),
            height: Val::Px(46.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(8.0)),
            margin: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(BUTTON),
        observe(move |_: On<Activate>, mut actions: MessageWriter<UiAction>, mut sfx: MessageWriter<PlaySfx>| {
            sfx.write(PlaySfx::ui(Sfx::Click));
            actions.write(action);
        }),
        children![(Text::new(text), bold(20.0), TextColor(TEXT))],
    )
}

pub fn panel() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(20.0)),
            row_gap: Val::Px(6.0),
            border_radius: BorderRadius::all(Val::Px(14.0)),
            ..default()
        },
        BackgroundColor(PANEL),
    )
}

/// Full-screen centered column.
pub fn screen_root() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(10.0),
        ..default()
    }
}

fn button_colors(mut q: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<MenuButton>>) {
    for (hovered, pressed, mut bg) in &mut q {
        bg.0 = match (hovered.get(), pressed) {
            (_, true) => BUTTON_PRESS,
            (true, false) => BUTTON_HOVER,
            _ => BUTTON,
        };
    }
}

/// A one-line text input. Click to focus, type, Backspace, Ctrl+V to paste.
#[derive(Component, Debug)]
pub struct TextField {
    pub value: String,
    pub max_len: usize,
    pub focused: bool,
}

#[derive(Component)]
struct TextFieldLabel;

/// Marks the field that has focus by default on a screen.
#[derive(Component)]
pub struct DefaultFocus;

pub fn text_field(initial: &str, max_len: usize, width: f32) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        TextField { value: initial.to_string(), max_len, focused: false },
        Node {
            width: Val::Px(width),
            height: Val::Px(40.0),
            padding: UiRect::horizontal(Val::Px(10.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            margin: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BorderColor::all(TEXT_DIM),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        observe(|activate: On<Activate>, mut fields: Query<(Entity, &mut TextField)>| {
            for (e, mut f) in &mut fields {
                f.focused = e == activate.entity;
            }
        }),
        children![(Text::new(initial), font(20.0), TextColor(TEXT), TextFieldLabel)],
    )
}

fn focus_default(mut fields: Query<&mut TextField, Added<DefaultFocus>>) {
    for mut f in &mut fields {
        f.focused = true;
    }
}

fn type_in_fields(
    mut keys: MessageReader<KeyboardInput>,
    buttons: Res<ButtonInput<KeyCode>>,
    mut clipboard: ResMut<bevy::clipboard::Clipboard>,
    mut fields: Query<&mut TextField>,
) {
    let ctrl = buttons.pressed(KeyCode::ControlLeft) || buttons.pressed(KeyCode::ControlRight);
    let Some(mut field) = fields.iter_mut().find(|f| f.focused) else {
        keys.clear();
        return;
    };
    for ev in keys.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        match &ev.logical_key {
            Key::Backspace => {
                field.value.pop();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("v") => {
                if let Some(Ok(text)) = clipboard.fetch_text().poll_result() {
                    for ch in text.trim().chars().filter(|c| !c.is_control()) {
                        if field.value.chars().count() < field.max_len {
                            field.value.push(ch);
                        }
                    }
                }
            }
            Key::Character(c) if !ctrl => {
                for ch in c.chars().filter(|c| !c.is_control()) {
                    if field.value.chars().count() < field.max_len {
                        field.value.push(ch);
                    }
                }
            }
            Key::Space if !ctrl && field.value.chars().count() < field.max_len => field.value.push(' '),
            _ => {}
        }
    }
}

fn render_fields(
    time: Res<Time>,
    fields: Query<(&TextField, &Children, &mut BorderColor)>,
    mut labels: Query<&mut Text, With<TextFieldLabel>>,
) {
    let blink = (time.elapsed_secs() * 2.0) as i32 % 2 == 0;
    for (field, children, mut border) in fields {
        *border = BorderColor::all(if field.focused { ACCENT } else { TEXT_DIM });
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child) {
                let cursor = if field.focused && blink { "|" } else { "" };
                let value = format!("{}{}", field.value, cursor);
                if text.0 != value {
                    text.0 = value;
                }
            }
        }
    }
}

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiAction>()
            .add_systems(PreStartup, load_fonts)
            .add_systems(Update, (button_colors, focus_default, type_in_fields, render_fields));
    }
}

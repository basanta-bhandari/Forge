use crossterm::{
    cursor::MoveTo,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use forgetui::{
    formats,
    graphics::{self, GraphicsMode},
    math::Vec3,
    pixel_renderer::{self, PixelStyle},
    renderer,
    scene::{PrimitiveKind, Scene, ShadingMode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    error::Error,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    time::Duration,
};

const HELP: &str = "REMEMBER THESE\n  A = Add    G = Grab/move    R = Rotate    S = Scale\n  D = Duplicate    E = Export    O = Open    ? = Help\n\nMOUSE\n  Click object/outliner    select\n  Left-drag viewport       orbit\n  Right/middle-drag        pan\n  Wheel                    zoom\n\nQUICK KEYS\n  G/R/S   opens a guided command   Delete removes\n  Shift + arrows   nudge X/Z        Shift + PgUp/PgDn nudge Y\n  1 Front   3 Side   7 Top   0 Iso   5 Perspective\n  Space shading   F frame   T theme   V graphics mode\n\nPLAIN COMMANDS (short aliases also work)\n  move x 10       g x 10          rotate y 45 / r y 45\n  scale 2         s z .5          add cone / a torus\n  duplicate       mirror x        reset\n  snap .25        theme amber     graphics kitty\n  view top        mode wire       rename Bracket\n  save job.forge  open job.obj    export job.step\n\nGRAPHICS  auto · kitty · sixel · ascii\nTHEMES    midnight · amber · matrix · light · mono\n\nEsc or ? closes this panel.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeChoice {
    Midnight,
    Amber,
    Matrix,
    Light,
    Mono,
}

impl ThemeChoice {
    fn name(self) -> &'static str {
        match self {
            Self::Midnight => "midnight",
            Self::Amber => "amber",
            Self::Matrix => "matrix",
            Self::Light => "light",
            Self::Mono => "mono",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Midnight => Self::Amber,
            Self::Amber => Self::Matrix,
            Self::Matrix => Self::Light,
            Self::Light => Self::Mono,
            Self::Mono => Self::Midnight,
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "midnight" | "dark" | "blue" => Some(Self::Midnight),
            "amber" | "orange" => Some(Self::Amber),
            "matrix" | "green" => Some(Self::Matrix),
            "light" => Some(Self::Light),
            "mono" | "gray" | "grey" => Some(Self::Mono),
            _ => None,
        }
    }

    fn colors(self) -> Theme {
        match self {
            Self::Midnight => Theme::new(
                Color::Rgb(10, 15, 23),
                Color::Rgb(20, 29, 40),
                Color::Rgb(222, 230, 238),
                Color::Rgb(94, 220, 220),
                Color::Rgb(120, 135, 150),
            ),
            Self::Amber => Theme::new(
                Color::Rgb(20, 14, 8),
                Color::Rgb(43, 30, 14),
                Color::Rgb(255, 225, 170),
                Color::Rgb(255, 174, 66),
                Color::Rgb(180, 130, 75),
            ),
            Self::Matrix => Theme::new(
                Color::Black,
                Color::Rgb(5, 28, 14),
                Color::Rgb(166, 255, 180),
                Color::Rgb(45, 255, 105),
                Color::Rgb(50, 145, 75),
            ),
            Self::Light => Theme::new(
                Color::Rgb(232, 236, 239),
                Color::Rgb(210, 217, 223),
                Color::Rgb(25, 32, 40),
                Color::Rgb(0, 105, 148),
                Color::Rgb(85, 95, 105),
            ),
            Self::Mono => Theme::new(
                Color::Black,
                Color::Rgb(28, 28, 28),
                Color::White,
                Color::White,
                Color::DarkGray,
            ),
        }
    }
}

#[derive(Clone, Copy)]
struct Theme {
    background: Color,
    panel: Color,
    foreground: Color,
    accent: Color,
    muted: Color,
}

impl Theme {
    const fn new(
        background: Color,
        panel: Color,
        foreground: Color,
        accent: Color,
        muted: Color,
    ) -> Self {
        Self {
            background,
            panel,
            foreground,
            accent,
            muted,
        }
    }
}

#[derive(Default)]
struct UiRegions {
    outliner: Rect,
    viewport: Rect,
}

#[derive(Clone, Copy)]
struct DragState {
    button: MouseButton,
    column: u16,
    row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Help,
    Add,
    Command,
}

struct App {
    scene: Scene,
    path: Option<PathBuf>,
    history: Vec<Scene>,
    future: Vec<Scene>,
    overlay: Overlay,
    command: String,
    status: String,
    dirty: bool,
    quit: bool,
    theme: ThemeChoice,
    snap: f32,
    regions: UiRegions,
    pick_frame: Option<renderer::RenderedFrame>,
    drag: Option<DragState>,
    graphics: GraphicsMode,
    kitty_image_id: u32,
}

impl App {
    fn new(scene: Scene, path: Option<PathBuf>) -> Self {
        Self {
            scene,
            path,
            history: Vec::new(),
            future: Vec::new(),
            overlay: Overlay::None,
            command: String::new(),
            status: "Ready — press ? for help, : for commands".into(),
            dirty: false,
            quit: false,
            theme: ThemeChoice::Midnight,
            snap: 0.25,
            regions: UiRegions::default(),
            pick_frame: None,
            drag: None,
            graphics: std::env::var("FORGETUI_GRAPHICS")
                .ok()
                .and_then(|value| GraphicsMode::parse(&value.to_ascii_lowercase()))
                .unwrap_or(GraphicsMode::Auto),
            kitty_image_id: 0,
        }
    }

    fn checkpoint(&mut self) {
        if self.history.len() == 100 {
            self.history.remove(0);
        }
        self.history.push(self.scene.clone());
        self.future.clear();
        self.dirty = true;
    }

    fn undo(&mut self) {
        if let Some(previous) = self.history.pop() {
            self.future
                .push(std::mem::replace(&mut self.scene, previous));
            self.dirty = true;
            self.status = "Undo".into();
        } else {
            self.status = "Nothing to undo".into();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.future.pop() {
            self.history.push(std::mem::replace(&mut self.scene, next));
            self.dirty = true;
            self.status = "Redo".into();
        } else {
            self.status = "Nothing to redo".into();
        }
    }

    fn add(&mut self, kind: PrimitiveKind) {
        self.checkpoint();
        self.scene.add_primitive(kind);
        self.status = format!("Added {}", kind.label());
        self.overlay = Overlay::None;
    }

    fn remove(&mut self) {
        if self.scene.selected().is_some() {
            self.checkpoint();
            let removed = self.scene.remove_selected().expect("selection checked");
            self.status = format!("Removed {}", removed.name);
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        match formats::save(&self.scene, &path) {
            Ok(()) => {
                if matches!(
                    path.extension().and_then(|e| e.to_str()),
                    Some("forge" | "json")
                ) {
                    self.path = Some(path.clone());
                    self.dirty = false;
                }
                self.status = format!("Saved {}", path.display());
            }
            Err(error) => self.status = format!("Error: {error}"),
        }
    }

    fn open(&mut self, path: PathBuf) {
        match formats::load(&path) {
            Ok(scene) => {
                self.history.clear();
                self.future.clear();
                self.scene = scene;
                self.path = matches!(
                    path.extension().and_then(|e| e.to_str()),
                    Some("forge" | "json")
                )
                .then_some(path.clone());
                self.dirty = false;
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => self.status = format!("Error: {error}"),
        }
    }

    fn execute_command(&mut self) {
        let input = self.command.trim().to_string();
        self.command.clear();
        self.overlay = Overlay::None;
        if input.is_empty() {
            return;
        }
        let mut parts = input.split_whitespace();
        let command = parts.next().unwrap_or_default().to_ascii_lowercase();
        let args: Vec<&str> = parts.collect();
        let result = match command.as_str() {
            "add" | "a" => self.command_add(&args),
            "move" | "grab" | "g" => self.command_vector(&args, VectorOperation::Move),
            "rotate" | "r" => self.command_vector(&args, VectorOperation::Rotate),
            "scale" | "s" => self.command_scale(&args),
            "duplicate" | "dup" | "d" => {
                self.checkpoint();
                self.scene.duplicate_selected();
                Ok("Duplicated selection".into())
            }
            "reset" => {
                self.checkpoint();
                if let Some(object) = self.scene.selected_mut() {
                    object.transform = Default::default();
                }
                Ok("Reset transform".into())
            }
            "mirror" => self.command_mirror(&args),
            "snap" => self.command_snap(&args),
            "theme" => self.command_theme(&args),
            "graphics" | "renderer" => self.command_graphics(&args),
            "view" => self.command_view(&args),
            "rename" => {
                let name = input
                    .split_once(char::is_whitespace)
                    .map(|(_, rest)| rest.trim())
                    .unwrap_or_default();
                if name.is_empty() {
                    Err("usage: rename NAME".into())
                } else {
                    self.checkpoint();
                    if let Some(object) = self.scene.selected_mut() {
                        object.name = name.into();
                    }
                    Ok(format!("Renamed selection to {name}"))
                }
            }
            "delete" | "remove" => {
                self.remove();
                return;
            }
            "hide" | "show" => {
                self.checkpoint();
                if let Some(object) = self.scene.selected_mut() {
                    object.visible = command == "show";
                }
                Ok(format!(
                    "Selection is now {}",
                    if command == "show" {
                        "visible"
                    } else {
                        "hidden"
                    }
                ))
            }
            "mode" => self.command_mode(&args),
            "frame" => {
                if let Some(object) = self.scene.selected() {
                    self.scene.camera.target = object.transform.position;
                }
                Ok("Framed selection".into())
            }
            "save" => {
                let path = argument_path(&input)
                    .or_else(|| self.path.clone())
                    .ok_or_else(|| "usage: save FILE.forge".to_string());
                match path {
                    Ok(path) => {
                        self.save_to(path);
                        return;
                    }
                    Err(error) => Err(error),
                }
            }
            "export" | "e" => match argument_path(&input) {
                Some(path) => {
                    self.save_to(path);
                    return;
                }
                None => Err("usage: export FILE.obj|stl|step|blend".into()),
            },
            "open" | "o" => match argument_path(&input) {
                Some(path) => {
                    self.open(path);
                    return;
                }
                None => Err("usage: open FILE".into()),
            },
            "new" => {
                self.checkpoint();
                self.scene = Scene::default();
                self.path = None;
                Ok("New scene".into())
            }
            "undo" => {
                self.undo();
                return;
            }
            "redo" => {
                self.redo();
                return;
            }
            "help" => {
                self.overlay = Overlay::Help;
                return;
            }
            "quit" | "q" => {
                self.quit = true;
                return;
            }
            _ => Err(format!("unknown command '{command}' (press ? for help)")),
        };
        self.status = match result {
            Ok(message) => message,
            Err(error) => format!("Error: {error}"),
        };
    }

    fn command_add(&mut self, args: &[&str]) -> Result<String, String> {
        let kind = match args.first().copied().unwrap_or_default() {
            "cube" | "box" => PrimitiveKind::Cube,
            "sphere" | "uvsphere" => PrimitiveKind::Sphere,
            "cylinder" => PrimitiveKind::Cylinder,
            "cone" => PrimitiveKind::Cone,
            "torus" | "donut" => PrimitiveKind::Torus,
            "wedge" | "ramp" => PrimitiveKind::Wedge,
            "plane" => PrimitiveKind::Plane,
            _ => return Err("try: add cube|sphere|cylinder|cone|torus|wedge|plane".into()),
        };
        self.checkpoint();
        self.scene.add_primitive(kind);
        Ok(format!("Added {}", kind.label()))
    }

    fn command_vector(
        &mut self,
        args: &[&str],
        operation: VectorOperation,
    ) -> Result<String, String> {
        if args.len() != 2 {
            return Err(format!("usage: {} x|y|z NUMBER", operation.name()));
        }
        let axis = args[0];
        let amount: f32 = args[1]
            .parse()
            .map_err(|_| "value must be a number".to_string())?;
        if !matches!(axis, "x" | "y" | "z") {
            return Err("axis must be x, y, or z".into());
        }
        self.checkpoint();
        let Some(object) = self.scene.selected_mut() else {
            return Err("nothing selected".into());
        };
        let vector = match operation {
            VectorOperation::Move => &mut object.transform.position,
            VectorOperation::Rotate => &mut object.transform.rotation,
        };
        match axis {
            "x" => vector.x += amount,
            "y" => vector.y += amount,
            "z" => vector.z += amount,
            _ => unreachable!(),
        }
        Ok(format!(
            "{} {} by {amount}",
            operation.name(),
            axis.to_uppercase()
        ))
    }

    fn command_scale(&mut self, args: &[&str]) -> Result<String, String> {
        let (axis, factor) = match args {
            [factor] => (None, factor.parse::<f32>()),
            [axis @ ("x" | "y" | "z"), factor] => (Some(*axis), factor.parse::<f32>()),
            _ => return Err("usage: scale NUMBER | scale x|y|z NUMBER".into()),
        };
        let factor = factor.map_err(|_| "scale must be a number".to_string())?;
        if factor.abs() < 0.0001 {
            return Err("scale cannot be zero".into());
        }
        self.checkpoint();
        let Some(object) = self.scene.selected_mut() else {
            return Err("nothing selected".into());
        };
        match axis {
            None => object.transform.scale = object.transform.scale * factor,
            Some("x") => object.transform.scale.x *= factor,
            Some("y") => object.transform.scale.y *= factor,
            Some("z") => object.transform.scale.z *= factor,
            _ => unreachable!(),
        }
        Ok(format!("Scaled by {factor}"))
    }

    fn command_mode(&mut self, args: &[&str]) -> Result<String, String> {
        self.scene.shading = match args.first().copied().unwrap_or_default() {
            "solid" => ShadingMode::Solid,
            "wire" | "wireframe" => ShadingMode::Wireframe,
            "xray" | "x-ray" => ShadingMode::XRay,
            _ => return Err("usage: mode solid|wire|xray".into()),
        };
        Ok(format!("{} shading", self.scene.shading.label()))
    }

    fn command_mirror(&mut self, args: &[&str]) -> Result<String, String> {
        let axis = args.first().copied().unwrap_or_default();
        if !matches!(axis, "x" | "y" | "z") {
            return Err("try: mirror x".into());
        }
        self.checkpoint();
        let scale = &mut self
            .scene
            .selected_mut()
            .ok_or("nothing selected")?
            .transform
            .scale;
        match axis {
            "x" => scale.x *= -1.0,
            "y" => scale.y *= -1.0,
            "z" => scale.z *= -1.0,
            _ => unreachable!(),
        }
        Ok(format!("Mirrored on {}", axis.to_uppercase()))
    }

    fn command_snap(&mut self, args: &[&str]) -> Result<String, String> {
        let value = args.first().copied().unwrap_or_default();
        self.snap = if value == "off" {
            0.0
        } else {
            value
                .parse::<f32>()
                .map_err(|_| "try: snap .25 or snap off".to_string())?
                .abs()
        };
        Ok(if self.snap == 0.0 {
            "Snapping off".into()
        } else {
            format!("Snap step: {}", self.snap)
        })
    }

    fn command_theme(&mut self, args: &[&str]) -> Result<String, String> {
        self.theme = ThemeChoice::parse(args.first().copied().unwrap_or_default())
            .ok_or("themes: midnight, amber, matrix, light, mono")?;
        Ok(format!("Theme: {}", self.theme.name()))
    }

    fn command_graphics(&mut self, args: &[&str]) -> Result<String, String> {
        self.graphics = GraphicsMode::parse(args.first().copied().unwrap_or_default())
            .ok_or("graphics modes: auto, kitty, sixel, ascii")?;
        Ok(format!(
            "Graphics: {} (active {})",
            self.graphics.label(),
            self.graphics.detect().label()
        ))
    }

    fn command_view(&mut self, args: &[&str]) -> Result<String, String> {
        let view = args.first().copied().unwrap_or_default();
        match view {
            "front" => {
                self.scene.camera.yaw = 0.0;
                self.scene.camera.pitch = 0.0;
            }
            "side" | "right" => {
                self.scene.camera.yaw = 90.0;
                self.scene.camera.pitch = 0.0;
            }
            "top" => {
                self.scene.camera.yaw = 0.0;
                self.scene.camera.pitch = 88.0;
            }
            "iso" => {
                self.scene.camera.yaw = -35.0;
                self.scene.camera.pitch = 24.0;
            }
            _ => return Err("views: front, side, top, iso".into()),
        }
        Ok(format!("{view} view"))
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match self.overlay {
            Overlay::Command => self.handle_command_key(key),
            Overlay::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
                ) {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::Add => self.handle_add_key(key),
            Overlay::None => self.handle_normal_key(key),
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                self.command.clear();
            }
            KeyCode::Enter => self.execute_command(),
            KeyCode::Backspace => {
                self.command.pop();
            }
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.command.push(character)
            }
            _ => {}
        }
    }

    fn handle_add_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') => self.add(PrimitiveKind::Cube),
            KeyCode::Char('s') => self.add(PrimitiveKind::Sphere),
            KeyCode::Char('y') => self.add(PrimitiveKind::Cylinder),
            KeyCode::Char('o') => self.add(PrimitiveKind::Cone),
            KeyCode::Char('t') => self.add(PrimitiveKind::Torus),
            KeyCode::Char('w') => self.add(PrimitiveKind::Wedge),
            KeyCode::Char('p') => self.add(PrimitiveKind::Plane),
            KeyCode::Esc | KeyCode::Char('q') => self.overlay = Overlay::None,
            _ => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('s') => {
                    if let Some(path) = self.path.clone() {
                        self.save_to(path);
                    } else {
                        self.overlay = Overlay::Command;
                        self.command = "save ".into();
                    }
                }
                KeyCode::Char('r') => self.redo(),
                _ => {}
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            let step = if self.snap > 0.0 { self.snap } else { 0.25 };
            let delta = match key.code {
                KeyCode::Left => Some(Vec3::new(-step, 0.0, 0.0)),
                KeyCode::Right => Some(Vec3::new(step, 0.0, 0.0)),
                KeyCode::Up => Some(Vec3::new(0.0, 0.0, -step)),
                KeyCode::Down => Some(Vec3::new(0.0, 0.0, step)),
                KeyCode::PageUp => Some(Vec3::new(0.0, step, 0.0)),
                KeyCode::PageDown => Some(Vec3::new(0.0, -step, 0.0)),
                _ => None,
            };
            if let Some(delta) = delta {
                self.checkpoint();
                if let Some(object) = self.scene.selected_mut() {
                    object.transform.position += delta;
                }
                self.status = format!("Nudged by {step}");
                return;
            }
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Char(':') => {
                self.overlay = Overlay::Command;
                self.command.clear();
            }
            KeyCode::Char('a') => self.overlay = Overlay::Add,
            KeyCode::Char('g') => self.prompt("move ", "Move: type axis and distance, e.g. x 10"),
            KeyCode::Char('r') => {
                self.prompt("rotate ", "Rotate: type axis and degrees, e.g. y 45")
            }
            KeyCode::Char('s') => self.prompt("scale ", "Scale: type factor, e.g. 2 or z .5"),
            KeyCode::Char('e') => self.prompt(
                "export ",
                "Export: type a filename ending in .stl, .obj, .step, or .blend",
            ),
            KeyCode::Char('o') => self.prompt(
                "open ",
                "Open: type a .forge, .obj, .step, or .blend filename",
            ),
            KeyCode::Char('d') => {
                self.checkpoint();
                self.scene.duplicate_selected();
                self.status = "Duplicated selection".into();
            }
            KeyCode::Char('t') => {
                self.theme = self.theme.next();
                self.status = format!("Theme: {}", self.theme.name());
            }
            KeyCode::Char('v') => {
                self.graphics = self.graphics.next();
                self.status = format!("Graphics: {}", self.graphics.label());
            }
            KeyCode::Char('u') => self.undo(),
            KeyCode::Delete | KeyCode::Backspace => self.remove(),
            KeyCode::Char('j') => self.scene.select_next(1),
            KeyCode::Char('k') => self.scene.select_next(-1),
            KeyCode::Left | KeyCode::Char('h') => self.scene.camera.yaw -= 5.0,
            KeyCode::Right | KeyCode::Char('l') => self.scene.camera.yaw += 5.0,
            KeyCode::Up => self.scene.camera.pitch = (self.scene.camera.pitch + 4.0).min(88.0),
            KeyCode::Down => self.scene.camera.pitch = (self.scene.camera.pitch - 4.0).max(-88.0),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.scene.camera.distance = (self.scene.camera.distance * 0.88).max(0.5)
            }
            KeyCode::Char('-') => {
                self.scene.camera.distance = (self.scene.camera.distance * 1.14).min(1000.0)
            }
            KeyCode::Char('p') | KeyCode::Char('5') => {
                self.scene.camera.perspective = !self.scene.camera.perspective
            }
            KeyCode::Char('1') => {
                self.scene.camera.yaw = 0.0;
                self.scene.camera.pitch = 0.0;
            }
            KeyCode::Char('3') => {
                self.scene.camera.yaw = 90.0;
                self.scene.camera.pitch = 0.0;
            }
            KeyCode::Char('7') => {
                self.scene.camera.yaw = 0.0;
                self.scene.camera.pitch = 88.0;
            }
            KeyCode::Char('0') => {
                self.scene.camera.yaw = -35.0;
                self.scene.camera.pitch = 24.0;
            }
            KeyCode::Char(' ') => {
                self.scene.shading = match self.scene.shading {
                    ShadingMode::Solid => ShadingMode::Wireframe,
                    ShadingMode::Wireframe => ShadingMode::XRay,
                    ShadingMode::XRay => ShadingMode::Solid,
                }
            }
            KeyCode::Char('f') => {
                if let Some(object) = self.scene.selected() {
                    self.scene.camera.target = object.transform.position;
                }
            }
            _ => {}
        }
    }

    fn prompt(&mut self, prefix: &str, guidance: &str) {
        self.overlay = Overlay::Command;
        self.command = prefix.into();
        self.status = guidance.into();
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.overlay != Overlay::None {
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scene.camera.distance = (self.scene.camera.distance * 0.88).max(0.5)
            }
            MouseEventKind::ScrollDown => {
                self.scene.camera.distance = (self.scene.camera.distance * 1.14).min(1000.0)
            }
            MouseEventKind::Down(button) => {
                if button == MouseButton::Left
                    && contains(self.regions.outliner, mouse.column, mouse.row)
                {
                    let index = mouse.row.saturating_sub(self.regions.outliner.y + 1) as usize;
                    if index < self.scene.objects.len() {
                        self.scene.selected = index;
                    }
                } else if contains(self.regions.viewport, mouse.column, mouse.row) {
                    if button == MouseButton::Left {
                        let x = mouse.column.saturating_sub(self.regions.viewport.x) as usize;
                        let y = mouse.row.saturating_sub(self.regions.viewport.y) as usize;
                        if let Some(index) = self
                            .pick_frame
                            .as_ref()
                            .and_then(|frame| frame.object_at(x, y))
                        {
                            self.scene.selected = index;
                        }
                    }
                    self.drag = Some(DragState {
                        button,
                        column: mouse.column,
                        row: mouse.row,
                    });
                }
            }
            MouseEventKind::Drag(button) => {
                if let Some(last) = self.drag.filter(|drag| drag.button == button) {
                    let dx = mouse.column as i32 - last.column as i32;
                    let dy = mouse.row as i32 - last.row as i32;
                    if button == MouseButton::Left {
                        self.scene.camera.yaw += dx as f32 * 1.5;
                        self.scene.camera.pitch =
                            (self.scene.camera.pitch - dy as f32 * 2.0).clamp(-88.0, 88.0);
                    } else {
                        let scale = self.scene.camera.distance * 0.015;
                        let yaw = self.scene.camera.yaw.to_radians();
                        self.scene.camera.target +=
                            Vec3::new(-yaw.cos(), 0.0, yaw.sin()) * (-dx as f32 * scale);
                        self.scene.camera.target.y += dy as f32 * scale;
                    }
                    self.drag = Some(DragState {
                        button,
                        column: mouse.column,
                        row: mouse.row,
                    });
                }
            }
            MouseEventKind::Up(_) => self.drag = None,
            _ => {}
        }
    }
}

fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.right() && y >= area.y && y < area.bottom()
}

#[derive(Clone, Copy)]
enum VectorOperation {
    Move,
    Rotate,
}

impl VectorOperation {
    fn name(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Rotate => "rotate",
        }
    }
}

fn argument_path(input: &str) -> Option<PathBuf> {
    input
        .split_once(char::is_whitespace)
        .map(|(_, path)| PathBuf::from(path.trim_matches(|c| c == ' ' || c == '\"' || c == '\'')))
        .filter(|path| !path.as_os_str().is_empty())
}

fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let theme = app.theme.colors();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.background).fg(theme.foreground)),
        area,
    );
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(if app.overlay == Overlay::Command {
                3
            } else {
                2
            }),
        ])
        .split(area);
    draw_header(frame, app, root[0]);

    if area.width >= 90 {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(25),
                Constraint::Min(30),
                Constraint::Length(31),
            ])
            .split(root[1]);
        app.regions.outliner = body[0];
        draw_outliner(frame, app, body[0]);
        draw_viewport(frame, app, body[1]);
        draw_inspector(frame, app, body[2]);
    } else if area.width >= 55 {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(22), Constraint::Min(25)])
            .split(root[1]);
        app.regions.outliner = body[0];
        draw_outliner(frame, app, body[0]);
        draw_viewport(frame, app, body[1]);
    } else {
        draw_viewport(frame, app, root[1]);
    }
    draw_status(frame, app, root[2]);

    match app.overlay {
        Overlay::Help => draw_popup(frame, theme, " Help ", HELP, 72, 31),
        Overlay::Add => draw_popup(
            frame,
            theme,
            " Add primitive ",
            "[c] Cube       [s] Sphere\n[y] Cylinder   [o] Cone\n[t] Torus      [w] Wedge\n[p] Large plane\n\n[Esc] Cancel",
            34,
            12,
        ),
        _ => {}
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme.colors();
    let dirty = if app.dirty { "*" } else { "" };
    let file = app
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "Untitled.forge".into());
    let left = format!(" FORGETUI  {file}{dirty}");
    let right = format!(
        "{}  {}  {}  ",
        app.scene.shading.label(),
        if app.scene.camera.perspective {
            "PERSP"
        } else {
            "ORTHO"
        },
        app.graphics.detect().label(),
    );
    let fill = area.width.saturating_sub((left.len() + right.len()) as u16) as usize;
    frame.render_widget(
        Paragraph::new(format!("{left}{}{right}", " ".repeat(fill))).style(
            Style::default()
                .bg(theme.panel)
                .fg(theme.foreground)
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
}

fn draw_outliner(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme.colors();
    let items: Vec<ListItem> = app
        .scene
        .objects
        .iter()
        .map(|object| {
            let icon = if object.visible { "◉" } else { "○" };
            ListItem::new(format!(" {icon}  {}", object.name))
        })
        .collect();
    let mut state =
        ListState::default().with_selected((!items.is_empty()).then_some(app.scene.selected));
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Scene "))
        .highlight_symbol("▸")
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_viewport(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme.colors();
    let inner_width = area.width.saturating_sub(2);
    let inner_height = area.height.saturating_sub(2);
    let rendered = renderer::render_scene(&app.scene, inner_width, inner_height);
    let rows = if app.graphics.detect() == GraphicsMode::Ascii {
        rendered.rows.clone()
    } else {
        vec![" ".repeat(inner_width as usize); inner_height as usize]
    };
    let text = Text::from(rows.into_iter().map(Line::from).collect::<Vec<_>>());
    let title = format!(" Viewport · {} tris ", rendered.triangles);
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(theme.accent).bg(theme.background)),
        area,
    );
    app.regions.viewport = Rect::new(area.x + 1, area.y + 1, inner_width, inner_height);
    app.pick_frame = Some(rendered);
}

fn draw_inspector(frame: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(object) = app.scene.selected() {
        let p = object.transform.position;
        let r = object.transform.rotation;
        let s = object.transform.scale;
        let d = object.world_dimensions();
        format!(
            "{}\n{}\n\nDIMENSIONS\n  X {:>9.3}\n  Y {:>9.3}\n  Z {:>9.3}\n\nTRANSFORM\n  Position\n    X {:>8.3}\n    Y {:>8.3}\n    Z {:>8.3}\n  Rotation\n    X {:>8.2}°\n    Y {:>8.2}°\n    Z {:>8.2}°\n  Scale\n    X {:>8.3}\n    Y {:>8.3}\n    Z {:>8.3}\n\nMESH\n  {:>6} vertices\n  {:>6} triangles",
            object.name,
            object.kind.label(),
            d.x,
            d.y,
            d.z,
            p.x,
            p.y,
            p.z,
            r.x,
            r.y,
            r.z,
            s.x,
            s.y,
            s.z,
            object.mesh.vertices.len(),
            object.mesh.triangles.len()
        )
    } else {
        "No selection\n\nPress a to add an object.".into()
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Inspector "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme.colors();
    if app.overlay == Overlay::Command {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    ":",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.command),
            ]))
            .block(Block::default().borders(Borders::TOP)),
            area,
        );
        let cursor = area.x + 1 + app.command.chars().count() as u16;
        frame.set_cursor_position((cursor.min(area.right().saturating_sub(1)), area.y + 1));
    } else {
        let hint = " A Add  G Move  R Rotate  S Scale  ? Help ";
        let fill = area
            .width
            .saturating_sub((app.status.len() + hint.len()) as u16) as usize;
        frame.render_widget(
            Paragraph::new(format!(" {}{}{hint}", app.status, " ".repeat(fill)))
                .block(Block::default().borders(Borders::TOP))
                .style(Style::default().fg(if app.status.starts_with("Error:") {
                    Color::Red
                } else {
                    theme.muted
                })),
            area,
        );
    }
}

fn draw_popup(
    frame: &mut Frame,
    theme: Theme,
    title: &str,
    content: &str,
    width: u16,
    height: u16,
) {
    let area = centered_rect(
        width.min(frame.area().width.saturating_sub(2)),
        height.min(frame.area().height.saturating_sub(2)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_alignment(Alignment::Center),
            )
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(theme.foreground).bg(theme.panel)),
        area,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn demo_scene() -> Scene {
    let mut scene = Scene::default();
    scene.objects[0].transform.position = Vec3::new(-1.8, 0.0, 0.0);
    scene.add_primitive(PrimitiveKind::Sphere);
    scene.objects[1].transform.position = Vec3::new(1.6, 0.0, 0.0);
    scene.add_primitive(PrimitiveKind::Cylinder);
    scene.objects[2].transform.position = Vec3::new(0.0, 0.0, -2.3);
    scene.objects[2].transform.scale = Vec3::new(0.7, 1.4, 0.7);
    scene
}

fn print_help() {
    println!(
        "ForgeTUI — terminal-native modeller and renderer\n\nUSAGE:\n  forgetui [FILE]\n  forgetui --demo\n  forgetui --render [FILE]\n  forgetui --formats\n\nFILE FORMATS:\n{}\n\nInside the TUI, press ? for all controls.",
        formats::format_matrix()
    );
}

fn color_rgb(color: Color) -> [u8; 3] {
    match color {
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Black => [0, 0, 0],
        Color::White => [255, 255, 255],
        Color::DarkGray => [90, 90, 90],
        Color::Gray => [160, 160, 160],
        Color::Red | Color::LightRed => [230, 70, 70],
        Color::Green | Color::LightGreen => [70, 220, 100],
        Color::Blue | Color::LightBlue => [70, 130, 230],
        Color::Yellow | Color::LightYellow => [235, 205, 70],
        Color::Cyan | Color::LightCyan => [70, 220, 220],
        Color::Magenta | Color::LightMagenta => [210, 80, 210],
        _ => [180, 180, 180],
    }
}

fn render_pixel_overlay(
    backend: &mut CrosstermBackend<io::Stdout>,
    app: &mut App,
) -> io::Result<()> {
    let mode = app.graphics.detect();
    if mode != GraphicsMode::Kitty && app.kitty_image_id != 0 {
        backend.write_all(&graphics::kitty_delete(app.kitty_image_id))?;
        app.kitty_image_id = 0;
    }
    if mode == GraphicsMode::Ascii || app.regions.viewport.width == 0 {
        return Ok(());
    }
    let window = crossterm::terminal::window_size().ok();
    let cell_width = window
        .as_ref()
        .filter(|size| size.width > 0 && size.columns > 0)
        .map_or(8, |size| u32::from(size.width / size.columns).max(1));
    let cell_height = window
        .as_ref()
        .filter(|size| size.height > 0 && size.rows > 0)
        .map_or(16, |size| u32::from(size.height / size.rows).max(1));
    let theme = app.theme.colors();
    let image = pixel_renderer::render(
        &app.scene,
        u32::from(app.regions.viewport.width) * cell_width,
        u32::from(app.regions.viewport.height) * cell_height,
        PixelStyle {
            background: color_rgb(theme.background),
            accent: color_rgb(theme.accent),
            muted: color_rgb(theme.muted),
        },
    );
    backend.write_all(b"\x1b7")?;
    execute!(
        backend,
        MoveTo(app.regions.viewport.x, app.regions.viewport.y)
    )?;
    let previous_kitty_id = app.kitty_image_id;
    let payload = match mode {
        GraphicsMode::Kitty => {
            let image_id = if previous_kitty_id == 0 {
                std::process::id().saturating_mul(4096).saturating_add(1)
            } else {
                previous_kitty_id.wrapping_add(1).max(1)
            };
            app.kitty_image_id = image_id;
            graphics::kitty_image(
                &image.rgb,
                image.width,
                image.height,
                app.regions.viewport.width,
                app.regions.viewport.height,
                image_id,
            )
        }
        GraphicsMode::Sixel => graphics::sixel_image(&image.rgb, image.width, image.height),
        _ => Vec::new(),
    };
    backend.write_all(&payload)?;
    if mode == GraphicsMode::Kitty && previous_kitty_id != 0 {
        backend.write_all(&graphics::kitty_delete(previous_kitty_id))?;
    }
    backend.write_all(b"\x1b8")?;
    backend.flush()
}

fn process_event(app: &mut App, event: Event) -> bool {
    match event {
        Event::Key(key) => {
            let changed = key.kind == KeyEventKind::Press;
            app.handle_key(key);
            changed
        }
        Event::Mouse(mouse) => {
            let changed = !matches!(mouse.kind, MouseEventKind::Moved | MouseEventKind::Up(_));
            app.handle_mouse(mouse);
            changed
        }
        Event::Resize(_, _) => true,
        _ => false,
    }
}

fn run_tui(mut app: App) -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut redraw = true;
        loop {
            if redraw {
                terminal.draw(|frame| draw(frame, &mut app))?;
                render_pixel_overlay(terminal.backend_mut(), &mut app)?;
                redraw = false;
            }
            if app.quit {
                break;
            }
            redraw |= process_event(&mut app, event::read()?);
            for _ in 0..64 {
                if !event::poll(Duration::ZERO)? {
                    break;
                }
                redraw |= process_event(&mut app, event::read()?);
            }
        }
        Ok(())
    })();
    disable_raw_mode()?;
    if app.kitty_image_id != 0 {
        terminal
            .backend_mut()
            .write_all(&graphics::kitty_delete(app.kitty_image_id))?;
    }
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    result
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        print_help();
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--formats") {
        println!("{}", formats::format_matrix());
        return Ok(());
    }
    let render_only = args.first().is_some_and(|arg| arg == "--render");
    let demo = args.first().is_some_and(|arg| arg == "--demo");
    let file = if render_only {
        args.get(1)
    } else if demo {
        None
    } else {
        args.first()
    }
    .map(PathBuf::from);
    let scene = if demo {
        demo_scene()
    } else if let Some(path) = &file {
        formats::load(path)?
    } else {
        Scene::default()
    };

    if render_only || !io::stdout().is_terminal() {
        let rendered = renderer::render_scene(&scene, 100, 36);
        for row in rendered.rows {
            println!("{row}");
        }
        return Ok(());
    }
    let native_path = file.filter(|path| {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("forge" | "json")
        )
    });
    run_tui(App::new(scene, native_path))
}

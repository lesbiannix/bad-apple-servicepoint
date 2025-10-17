mod config;
mod theme;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::{DynamicImage, GenericImageView};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};
use rodio::{Decoder, OutputStream, Sink};
use servicepoint::{Bitmap, BitmapCommand, CompressionCode, GridMut, Origin, UdpSocketExt};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufReader},
    net::UdpSocket,
    time::{Duration, Instant},
};
use theme::Theme;

use crate::config::{load_settings, save_settings, Settings};

const FRAME_RATE: f64 = 30.0;
const SERVICEPOINT_ENABLED: bool = true;
const SERVICEPOINT_ADDR: &str = "127.0.0.1:2342";

struct AppState {
    playing: bool,
    settings: Settings,
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>> {
    let mut app_state = AppState {
        playing: true,
        settings: load_settings(),
    };
    let themes = Theme::default_themes();
    let mut current_theme = themes.get(&app_state.settings.theme).unwrap_or_else(|| themes.get("default").unwrap());

    let sp_socket = if SERVICEPOINT_ENABLED {
        UdpSocket::bind_connect(SERVICEPOINT_ADDR).ok()
    } else {
        None
    };

    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(tuple) => (Some(tuple.0), Some(tuple.1)),
        Err(e) => {
            eprintln!("Warning: Could not open audio stream: {}. Continuing with video only.", e);
            (None, None)
        }
    };

    let sink = stream_handle.as_ref().and_then(|handle| Sink::try_new(handle).ok());
    if let Some(s) = &sink {
        if let Ok(file) = File::open("assets/bad-apple.ogg") {
            if let Ok(source) = Decoder::new(BufReader::new(file)) {
                s.append(source);
            }
        }
        s.set_volume(app_state.settings.volume);
        s.play();
    }

    let frame_paths: Vec<_> = fs::read_dir("assets/frames")
        .map_err(|e| format!("Failed to read frames directory: {}", e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();

    if frame_paths.is_empty() {
        return Err("No frames found in assets/frames directory.".into());
    }

    let frames: Vec<DynamicImage> = frame_paths
        .into_iter()
        .map(|path| image::open(path).expect("Failed to load frame"))
        .collect();

    let mut frame_index = 0;
    let mut last_frame_time = Instant::now();

    loop {
        let frame_duration = Duration::from_secs_f64(1.0 / (FRAME_RATE * app_state.settings.speed));
        terminal.draw(|f| ui(f, &frames[frame_index], &app_state, &current_theme))?;

        if let Some(socket) = &sp_socket {
            let command = image_to_bitmap_command(&frames[frame_index]);
            socket.send_command(command);
        }

        if app_state.playing {
            let elapsed = last_frame_time.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
            last_frame_time = Instant::now();
            frame_index = (frame_index + 1) % frames.len();
        }

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        save_settings(&app_state.settings);
                        break;
                    }
                    KeyCode::Char(' ') => {
                        app_state.playing = !app_state.playing;
                        if let Some(s) = &sink {
                            if app_state.playing { s.play(); } else { s.pause(); }
                        }
                    }
                    KeyCode::Up => app_state.settings.volume = (app_state.settings.volume + 0.1).min(1.0),
                    KeyCode::Down => app_state.settings.volume = (app_state.settings.volume - 0.1).max(0.0),
                    KeyCode::Left => app_state.settings.speed = (app_state.settings.speed - 0.1).max(0.1),
                    KeyCode::Right => app_state.settings.speed = (app_state.settings.speed + 0.1).min(2.0),
                    KeyCode::Char('t') => {
                        let theme_names: Vec<_> = themes.keys().cloned().collect();
                        let current_theme_index = theme_names.iter().position(|r| r == &current_theme.name).unwrap_or(0);
                        let next_theme_index = (current_theme_index + 1) % theme_names.len();
                        let next_theme_name = theme_names[next_theme_index].clone();
                        app_state.settings.theme = next_theme_name.clone();
                        current_theme = themes.get(&next_theme_name).unwrap_or_else(|| themes.get("default").unwrap());
                    }
                    _ => {}
                }
                if let Some(s) = &sink {
                    s.set_volume(app_state.settings.volume);
                    s.set_speed(app_state.settings.speed as f32);
                }
            }
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, img: &DynamicImage, app_state: &AppState, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.size());

    let image_paragraph = render_image_to_paragraph(img, chunks[0], theme);
    f.render_widget(image_paragraph, chunks[0]);

    let status_text = format!(
        "{} | Speed: {:.1}x | Volume: {:.0}% | Theme: {} | Controls: [Space] Play/Pause, [↑/↓] Volume, [←/→] Speed, [t] Theme, [q] Quit",
        if app_state.playing { "▶ Playing" } else { "⏸ Paused" },
        app_state.settings.speed,
        app_state.settings.volume * 100.0,
        theme.name
    );
    let status_line = Paragraph::new(Line::from(status_text))
        .style(theme.status_bar);
    f.render_widget(status_line, chunks[1]);
}

fn image_to_bitmap_command(img: &DynamicImage) -> BitmapCommand {
    let resized = img.resize_exact(64, 64, image::imageops::FilterType::Triangle);
    let mut bitmap = Bitmap::new(64, 64).unwrap();

    for y in 0..64 {
        for x in 0..64 {
            let pixel = resized.get_pixel(x, y);
            let luma = pixel[0] / 3 + pixel[1] / 3 + pixel[2] / 3;
            if luma > 128 {
                bitmap.set(x as usize, y as usize, true);
            }
        }
    }

    BitmapCommand {
        origin: Origin::ZERO,
        bitmap,
        compression: CompressionCode::default(),
    }
}

fn render_image_to_paragraph<'a>(img: &'a DynamicImage, area: Rect, theme: &Theme) -> Paragraph<'a> {
    let mut lines = Vec::with_capacity(area.height as usize);
    let scale_x = img.width() as f32 / area.width as f32;
    let scale_y = img.height() as f32 / area.height as f32;

    for y in 0..area.height {
        let mut spans = Vec::with_capacity(area.width as usize);
        for x in 0..area.width {
            let img_x = (x as f32 * scale_x) as u32;
            let img_y = (y as f32 * scale_y) as u32;

            let pixel = img.get_pixel(img_x.min(img.width() - 1), img_y.min(img.height() - 1));
            let luma = pixel[0] / 3 + pixel[1] / 3 + pixel[2] / 3;
            let char_index = (luma as usize * (theme.video.characters.len() - 1)) / 255;
            let character = theme.video.characters[char_index].to_string();
            spans.push(Span::styled(character, theme.video.style));
        }
        lines.push(Line::from(spans));
    }

    Paragraph::new(lines)
}
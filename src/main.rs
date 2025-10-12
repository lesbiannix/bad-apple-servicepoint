use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::{DynamicImage, GenericImageView};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};
use rodio::{Decoder, OutputStream, Sink};
use servicepoint::{Bitmap, BitmapCommand, CompressionCode, GridMut, Origin, UdpSocketExt};
use std::{
    error::Error,
    fs::File,
    io::{self, BufReader},
    net::UdpSocket,
    time::{Duration, Instant},
};

const FRAME_RATE: f64 = 30.0;
const SERVICEPOINT_ENABLED: bool = true;
const SERVICEPOINT_ADDR: &str = "127.0.0.1:4242";

struct AppState {
    playing: bool,
    speed: f64,
    volume: f32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            playing: true,
            speed: 1.0,
            volume: 1.0,
        }
    }
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

fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>> {
    let mut app_state = AppState::default();

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
        s.set_volume(app_state.volume);
        s.play();
    }

    let frame_paths: Vec<_> = (1..=2)
        .map(|i| format!("assets/frames/frame_{:04}.png", i))
        .collect();
    let frames: Vec<DynamicImage> = frame_paths
        .into_iter()
        .map(|path| image::open(path).expect("Failed to load frame"))
        .collect();

    let mut frame_index = 0;
    let mut last_frame_time = Instant::now();

    loop {
        let frame_duration = Duration::from_secs_f64(1.0 / (FRAME_RATE * app_state.speed));
        terminal.draw(|f| ui(f, &frames[frame_index], &app_state))?;

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
                    KeyCode::Char('q') => break,
                    KeyCode::Char(' ') => {
                        app_state.playing = !app_state.playing;
                        if let Some(s) = &sink {
                            if app_state.playing { s.play(); } else { s.pause(); }
                        }
                    }
                    KeyCode::Up => app_state.volume = (app_state.volume + 0.1).min(1.0),
                    KeyCode::Down => app_state.volume = (app_state.volume - 0.1).max(0.0),
                    KeyCode::Left => app_state.speed = (app_state.speed - 0.1).max(0.1),
                    KeyCode::Right => app_state.speed = (app_state.speed + 0.1).min(2.0),
                    _ => {}
                }
                if let Some(s) = &sink {
                    s.set_volume(app_state.volume);
                    s.set_speed(app_state.speed as f32);
                }
            }
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, img: &DynamicImage, app_state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.size());

    let image_paragraph = render_image_to_paragraph(img, chunks[0]);
    f.render_widget(image_paragraph, chunks[0]);

    let status_text = format!(
        "{} | Speed: {:.1}x | Volume: {:.0}% | Controls: [Space] Play/Pause, [↑/↓] Volume, [←/→] Speed, [q] Quit",
        if app_state.playing { "▶ Playing" } else { "⏸ Paused" },
        app_state.speed,
        app_state.volume * 100.0,
    );
    let status_line = Paragraph::new(Line::from(status_text))
        .style(Style::default().add_modifier(Modifier::REVERSED));
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

fn render_image_to_paragraph<'a>(img: &'a DynamicImage, area: Rect) -> Paragraph<'a> {
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
            let character = match luma {
                0..=32 => " ",
                33..=64 => ".",
                65..=96 => ":",
                97..=128 => "-",
                129..=160 => "=",
                161..=192 => "+",
                193..=224 => "*",
                _ => "#",
            };
            spans.push(Span::styled(character, Style::default()));
        }
        lines.push(Line::from(spans));
    }

    Paragraph::new(lines)
}
mod bot;
mod mem_reader;
use iced::{
    executor,
    widget::{
        canvas::{self, stroke, Cursor, Geometry, Path, Stroke},
        column, row, Button, Canvas, Text,
    },
    Application, Color, Command, Element, Length, Rectangle, Settings, Subscription, Theme,
};
use libwmctl::WmCtl;
use mki;
mod libzuma;

fn main() -> iced::Result {
    AiInterface::run(Settings {
        antialiasing: true,
        ..Settings::default()
    })
}

#[derive(Clone, Debug)]
pub enum Message {
    AttachedChanged(bool),
    TryAttach,
    UpdateZumaGameState,
    RefreshCanvas,
    PlayBot,
}

pub struct AiInterface {
    attached: Option<bool>,
    win_manager: WmCtl,
    zuma_reader: mem_reader::ZumaReader,
    bot_move: bot::BotMove,

    // Time that the bot took to play/think its move
    window_find_time: std::time::Duration,
    bot_time_mem_read: std::time::Duration,
    bot_time_think: std::time::Duration,
    bot_time_play: std::time::Duration,
    bot_time_total: std::time::Duration,

    graphics: canvas::Cache,
}

impl Application for AiInterface {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                attached: None,
                win_manager: WmCtl::connect().unwrap(),
                zuma_reader: mem_reader::ZumaReader::new(),
                bot_move: bot::BotMove::Nothing,
                window_find_time: std::time::Duration::from_secs(0),
                bot_time_mem_read: std::time::Duration::from_secs(0),
                bot_time_think: std::time::Duration::from_secs(0),
                bot_time_play: std::time::Duration::from_secs(0),
                bot_time_total: std::time::Duration::from_secs(0),
                graphics: Default::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "ZUM-AI-STEAM".to_string()
    }

    fn update(&mut self, event: Message) -> Command<Message> {
        match event {
            Message::TryAttach => match self.zuma_reader.find_zuma_process() {
                Some(_) => self.attached = Some(true),
                None => self.attached = Some(false),
            },
            Message::AttachedChanged(new_attached) => {
                self.attached = Some(new_attached);
            }
            Message::UpdateZumaGameState => {
                if let Some(true) = self.attached {
                    self.zuma_reader.update_frog_follow_eyes();
                    self.zuma_reader.update_balls();
                    self.zuma_reader.update_mouse_coords();
                    self.zuma_reader.update_frog();
                }
            }
            Message::RefreshCanvas => {
                self.graphics.clear();
            }
            Message::PlayBot => {
                if let Some(true) = self.attached {
                    let before = std::time::Instant::now();

                    // Find the zuma window
                    let zuma_win = self.win_manager.active_win().unwrap();
                    let (zuma_win_x, zuma_win_y, _, _) =
                        self.win_manager.win_geometry(zuma_win).unwrap();

                    self.window_find_time = before.elapsed();

                    self.zuma_reader.update_balls();
                    self.zuma_reader.update_frog();
                    if self.zuma_reader.frog.is_none() {
                        return Command::none();
                    }

                    self.bot_time_mem_read = before.elapsed() - self.window_find_time;
                    let bot_shot = bot::suggest_shot_color(
                        self.zuma_reader.frog.unwrap(),
                        &self.zuma_reader.game_state,
                    );
                    self.bot_move = bot_shot;

                    self.bot_time_think =
                        before.elapsed() - self.window_find_time - self.bot_time_mem_read;

                    self.zuma_reader.update_paused();
                    if self.zuma_reader.paused {
                        return Command::none();
                    }

                    match bot_shot {
                        bot::BotMove::Shoot(point) => {
                            // Click on the coordinates
                            let click_x = zuma_win_x - 1 as i32 + point.x.clamp(0., 640.) as i32;
                            let click_y = zuma_win_y - 38 + point.y.clamp(0., 480.) as i32;
                            mki::Mouse::Left.click_at(click_x as i32, click_y as i32)
                        }
                        bot::BotMove::SwapShoot(point) => {
                            mki::Mouse::Right.click();
                            let click_x = zuma_win_x - 1 as i32 + point.x.clamp(0., 640.) as i32;
                            let click_y = zuma_win_y - 38 + point.y.clamp(0., 480.) as i32;
                            mki::Mouse::Left.click_at(click_x as i32, click_y as i32)
                        }
                        _ => {}
                    }

                    self.bot_time_play = before.elapsed()
                        - self.window_find_time
                        - self.bot_time_mem_read
                        - self.bot_time_think;
                    self.bot_time_total = before.elapsed();
                }
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let attached_text = match self.attached {
            Some(true) => "Attached to Zuma",
            Some(false) => "Failed to attach",
            None => "Not attached",
        };

        let mut content = row!(attached_text).padding(10).spacing(10);

        if let None | Some(false) = self.attached {
            let button = Button::new("Try attaching to Zuma")
                .padding(12)
                .on_press(Message::TryAttach);

            content = content.push(button);
        }

        let stats = column![
            Text::new(format!(
                "Finding the window took: {}ms",
                self.window_find_time.as_micros()
            )),
            Text::new(format!(
                "Memory reading took: {}ms",
                self.bot_time_mem_read.as_micros()
            )),
            Text::new(format!(
                "Thinking took: {}ms",
                self.bot_time_think.as_micros()
            )),
            Text::new(format!(
                "Playing the move took: {}ms",
                self.bot_time_play.as_micros()
            )),
            Text::new(format!("Total: {}ms", self.bot_time_total.as_micros())),
        ];

        let ball_display = Canvas::new(self)
            .width(Length::Fixed(640.))
            .height(Length::Fixed(480.));

        column![content, stats, ball_display].into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let refresh_screen =
            iced::time::every(std::time::Duration::from_millis(50)).map(|_| Message::RefreshCanvas);

        let bot_sub =
            iced::time::every(std::time::Duration::from_millis(235)).map(|_| Message::PlayBot);

        Subscription::batch([refresh_screen, bot_sub])
    }
}

impl<Message> canvas::Program<Message> for AiInterface {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        vec![self.graphics.draw(bounds.size(), |frame| {
            frame.fill(
                &Path::rectangle(iced::Point::ORIGIN, bounds.size()),
                Color::BLACK,
            );

            let reachable_balls = bot::reachable_balls(
                &libzuma::Frog {
                    location: libzuma::Point { x: 320., y: 240. },
                    active_ball: libzuma::BallColor::Blue,
                    next_ball: libzuma::BallColor::Blue,
                },
                &self.zuma_reader.game_state,
            );

            for (i, ball) in self.zuma_reader.game_state.balls.iter().enumerate() {
                draw_ball(
                    frame,
                    ball,
                    Some(format!("{}", i)),
                    reachable_balls.contains(ball),
                );
            }

            if let Some(frog) = self.zuma_reader.frog {
                let frog_pos = iced::Point::new(frog.location.x, frog.location.y);
                frame.fill_text(canvas::Text {
                    content: format!("{:?}", frog.active_ball),
                    position: frog_pos,
                    color: Color::WHITE,
                    ..Default::default()
                });

                match self.bot_move {
                    bot::BotMove::Shoot(bot_coords) => {
                        let coords = iced::Point {
                            x: bot_coords.x,
                            y: bot_coords.y,
                        };
                        let line = &Path::line(frog_pos, coords);
                        let stroke = Stroke {
                            width: 5.,
                            style: stroke::Style::Solid(Color::from_rgb8(255, 255, 0)),
                            ..Stroke::default()
                        };

                        frame.stroke(&line, stroke);
                    }
                    _ => {}
                }
            }
        })]
    }
}

fn draw_ball(
    frame: &mut iced::widget::canvas::Frame,
    ball: &libzuma::Ball,
    ball_text: Option<String>,
    is_reachable: bool,
) {
    let coords = iced::Point {
        x: ball.coordinates.x,
        y: ball.coordinates.y,
    };

    let circle = Path::circle(coords, 15.0);

    let mut color = match ball.color {
        libzuma::BallColor::Blue => Color::from_rgb8(0, 0, 255),
        libzuma::BallColor::Yellow => Color::from_rgb8(255, 255, 0),
        libzuma::BallColor::Red => Color::from_rgb8(255, 0, 0),
        libzuma::BallColor::Green => Color::from_rgb8(0, 255, 0),
        libzuma::BallColor::Purple => Color::from_rgb8(255, 0, 255),
        libzuma::BallColor::White => Color::from_rgb8(255, 255, 255),
    };

    if !is_reachable {
        color.a = 0.03;
    }

    frame.fill(&circle, color);
    if let Some(text) = ball_text {
        frame.fill_text(canvas::Text {
            content: text,
            position: iced::Point::new(coords.x - 10., coords.y - 10.),
            ..Default::default()
        })
    }
}

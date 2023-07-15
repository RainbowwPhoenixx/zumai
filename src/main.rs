mod bot;
mod mem_reader;
use iced::{
    executor,
    widget::{
        canvas::{self, stroke, Cursor, Geometry, Path, Stroke},
        checkbox, column, row, Button, Canvas, PickList, Slider, Text,
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

const BACK_TO_MENU_COORDS: (i32, i32) = (320, 360);
const NEW_GAME_COORDS: (i32, i32) = (320, 450);

#[derive(Clone, Debug)]
pub enum Message {
    AttachedChanged(bool),
    EnabledChanged(bool),
    AutoResetChanged(bool),
    ShootFreqChanged(u32),
    ModeChanged(bot::BotMode),
    TryAttach,
    UpdateZumaGameState,
    RefreshCanvas,
    PlayBot,
}

pub struct AiInterface {
    attached: Option<bool>,
    zuma_reader: mem_reader::ZumaReader,
    bot_move: bot::BotMove,

    win_manager: WmCtl,
    win_coords: Option<(i32, i32)>,

    enabled: bool,
    auto_reset: bool,
    shoot_frequency: u32, // in ms
    mode: bot::BotMode,
    memo: Vec<bot::Shot>,

    // Time that the bot took to play/think its move
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
                zuma_reader: mem_reader::ZumaReader::new(),
                bot_move: bot::BotMove::Nothing,
                win_manager: WmCtl::connect().unwrap(),
                win_coords: None,
                enabled: true,
                auto_reset: false,
                shoot_frequency: 250,
                mode: bot::BotMode::ColorBot,
                memo: vec![],
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
            Message::EnabledChanged(state) => self.enabled = state,
            Message::AutoResetChanged(state) => self.auto_reset = state,
            Message::ShootFreqChanged(freq) => self.shoot_frequency = freq,
            Message::ModeChanged(mode) => self.mode = mode,
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

                    // Find the zuma
                    if self.win_coords.is_none() {
                        let zuma_win = self.win_manager.active_win().unwrap();
                        if !"Zuma Deluxe 1.0"
                            .contains(&self.win_manager.win_name(zuma_win).unwrap_or("".into()))
                        {
                            return Command::none();
                        }
                        let (zuma_win_x, zuma_win_y, _, _) =
                            self.win_manager.win_geometry(zuma_win).unwrap();
                        self.win_coords = Some((zuma_win_x, zuma_win_y))
                    }

                    self.zuma_reader.update_balls();
                    self.zuma_reader.update_frog();
                    if !self.enabled || self.zuma_reader.frog.is_none() {
                        return Command::none();
                    }

                    self.bot_time_mem_read = before.elapsed();
                    let bot_shot = bot::suggest_shot(
                        &self.zuma_reader.frog.unwrap(),
                        &self.zuma_reader.game_state,
                        self.mode,
                        &mut self.memo,
                    );
                    self.bot_move = bot_shot;

                    self.bot_time_think = before.elapsed() - self.bot_time_mem_read;

                    self.zuma_reader.update_paused();
                    if self.zuma_reader.paused {
                        if self.auto_reset && self.zuma_reader.game_state.balls.len() == 0 {
                            // We've lost, attempt to restart automatically
                            let click_x =
                                self.win_coords.unwrap().0 - 1 as i32 + BACK_TO_MENU_COORDS.0;
                            let click_y = self.win_coords.unwrap().1 - 38 + BACK_TO_MENU_COORDS.1;
                            mki::Mouse::Left.click_at(click_x as i32, click_y as i32);

                            std::thread::sleep_ms(1000);

                            let click_x = self.win_coords.unwrap().0 - 1 as i32 + NEW_GAME_COORDS.0;
                            let click_y = self.win_coords.unwrap().1 - 38 + NEW_GAME_COORDS.1;
                            mki::Mouse::Left.click_at(click_x as i32, click_y as i32);
                        }
                        return Command::none();
                    }

                    match bot_shot {
                        bot::BotMove::Shoot(point) => {
                            // Click on the coordinates
                            let click_x = self.win_coords.unwrap().0 - 1 as i32
                                + point.x.clamp(0., 640.) as i32;
                            let click_y =
                                self.win_coords.unwrap().1 - 38 + point.y.clamp(0., 470.) as i32;
                            mki::Mouse::Left.click_at(click_x as i32, click_y as i32)
                        }
                        bot::BotMove::SwapShoot(point) => {
                            mki::Mouse::Right.click();
                            let click_x = self.win_coords.unwrap().0 - 1 as i32
                                + point.x.clamp(0., 640.) as i32;
                            let click_y =
                                self.win_coords.unwrap().1 - 38 + point.y.clamp(0., 470.) as i32;
                            mki::Mouse::Left.click_at(click_x as i32, click_y as i32)
                        }
                        _ => {}
                    }

                    self.bot_time_play =
                        before.elapsed() - self.bot_time_mem_read - self.bot_time_think;
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

        let mut attached_options = row!(attached_text).padding(10).spacing(10);

        let bot_options = if let None | Some(false) = self.attached {
            let button = Button::new("Try attaching to Zuma")
                .padding(12)
                .on_press(Message::TryAttach);

            attached_options = attached_options.push(button);
            column![]
                .padding(10)
                .spacing(10)
                .width(Length::FillPortion(1))
        } else {
            let enabled_checkbox = checkbox("Bot enabled", self.enabled, Message::EnabledChanged);
            let reset_checkbox = checkbox("Auto reset", self.auto_reset, Message::AutoResetChanged);
            let mode_text = Text::new(format!("Bot mode: "));
            let mode_choice =
                PickList::new(bot::BotMode::ALL, Some(self.mode), Message::ModeChanged);
            let freq_text = Text::new(format!("Shoot every: {} ms", self.shoot_frequency));
            let freqslider =
                Slider::new(200..=1000, self.shoot_frequency, Message::ShootFreqChanged);
            column![
                enabled_checkbox,
                reset_checkbox,
                row![mode_text, mode_choice],
                freq_text,
                freqslider
            ]
            .padding(10)
            .spacing(10)
            .width(Length::FillPortion(1))
        };

        let stats = column![
            Text::new("Stats"),
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
        ]
        .width(Length::FillPortion(1));

        let ball_display = Canvas::new(self)
            .width(Length::Fixed(640.))
            .height(Length::Fixed(480.));

        column![attached_options, row![stats, bot_options], ball_display].into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let refresh_screen =
            iced::time::every(std::time::Duration::from_millis(50)).map(|_| Message::RefreshCanvas);

        let bot_sub = iced::time::every(std::time::Duration::from_millis(
            self.shoot_frequency.into(),
        ))
        .map(|_| Message::PlayBot);

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

            for point in &self.zuma_reader.game_state.curve.points {
                frame.fill(
                    &Path::circle(
                        iced::Point {
                            x: point.x,
                            y: point.y,
                        },
                        1.,
                    ),
                    Color::WHITE,
                );
            }

            let mut reachable_balls = vec![];

            if let Some(frog) = self.zuma_reader.frog {
                let frog_pos = iced::Point::new(frog.location.x, frog.location.y);
                frame.fill_text(canvas::Text {
                    content: format!("{:?}", frog.active_ball.color),
                    position: frog_pos,
                    color: Color::WHITE,
                    ..Default::default()
                });

                reachable_balls = bot::reachable_balls(&frog, &self.zuma_reader.game_state);

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

            for (i, ball) in self.zuma_reader.game_state.balls.iter().enumerate() {
                draw_ball(
                    frame,
                    ball,
                    Some(format!("{}", i)),
                    reachable_balls.contains(ball),
                );
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

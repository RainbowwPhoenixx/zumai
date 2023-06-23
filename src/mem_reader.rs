use std::io::{Error, ErrorKind, Read};

use crate::libzuma::*;
use process_memory::*;
use sysinfo::{Pid, ProcessExt, SystemExt};

const FROG_EYES_FOLLOWING_OFFSET: usize = 0x59D5FC;
const STREAM_PARENT_OFFSETS: [usize; 2] = [0x83FE00, 0x0]; // I think this is a stack address? But it appears very consistent

const MOUSE_COORDS_OFFSETS: [usize; 4] = [0x59F4A4, 0x320, 0x10, 0xE0];

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct MemBall {
    _padding_0x0: [u8; 4],
    global_ball_number: u32,
    color: u32,
    _distance_along_path: f32,
    _padding_0x10: [u8; 12],
    x: f32,
    y: f32,
    _padding_0x24: [u8; 64],
    effect: u32,
    _padding_0x68: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct MemFrog {
    _padding_0x0: [u8; 4],
    frog_angle: f32,
    anim_x: u32,
    anim_y: u32,
    target_x: u32,
    target_y: u32,
    recoil_max_x: u32,
    recoil_max_y: u32,
    _padding_0x18: [u8; 12],
    ptr_active_ball: u32,
    ptr_next_ball: u32,
    _padding_0x34: [u8; 16],
    ball_exit_speed: f32,
}

// Balls are stored as a doubly-linked-list
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct MemBallLinkedList {
    ptr_first_elem: u32,
    ptr_last_elem: u32,
}
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct MemBallLinkedListElement {
    ptr_next_elem: u32, // closer to end
    ptr_prev_elem: u32, // closer to start
    ptr_ball: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct Game {
    __padding_0x0: [u8; 140],
    ballstream_ptrs: [u32; 6],
    __padding_0xa4: [u8; 24],
    ballstream_count: u32,
    __padding_0xc0: [u8; 13 * 4],
    igt: u32, // igt in frames (i think)
    __padding_0xf8: [u8; 4],
    game_state: u32, // 0 if running, 1 if paused, 2 if unfocused or game done
    __padding_0x100: [u8; 0x198],
    ptr_frog: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct MemBallStream {
    ptr_global_resources: u32,
    _padding_0x4: [u8; 12],
    ptr_level_data: u32,
    _padding_0x14: [u8; 8],
    stream_id: u32,
    _padding_0x20: [u8; 16],
    ptr_ball_linked_list: u32,
    ball_count: u32,
    _padding_0x38: [u8; 28],
    stopped_cooldown: u32,
    slowed_cooldown: u32,
    reverse_cooldown: u32,
    balls_shot: u32,
    speed_thing: f32,
    distance_from_start: u32,
    _padding_0xx: [u8; 4],
}

#[derive(Debug)]
pub struct ZumaReader {
    pub process_handle: Option<ProcessHandle>,
    pub game_state: BallSequence,
    pub frog_follow_eyes: Option<bool>,
    pub mouse_coords: Option<(u32, u32)>,
    pub frog: Option<Frog>,
    pub paused: bool,
}

impl ZumaReader {
    pub fn new() -> Self {
        Self {
            process_handle: None,
            game_state: BallSequence { balls: vec![] },
            frog_follow_eyes: None,
            mouse_coords: None,
            frog: None,
            paused: true,
        }
    }

    pub fn find_zuma_process(&mut self) -> Option<Pid> {
        let system = sysinfo::System::new_all();
        let process = system
            .get_process_by_name("wine-preloader")
            .into_iter()
            .find(|p| {
                p.cmd()
                    .iter()
                    .any(|command| command.contains(&"popcapgame1".to_string()))
            })?;

        self.process_handle = process
            .pid()
            .try_into_process_handle()
            .ok()
            .and_then(|p| Some(p.set_arch(Architecture::Arch32Bit)));

        // self.update_frog_struct();

        Some(process.pid())
    }

    fn get_frog_follow_eyes(&self) -> Option<bool> {
        DataMember::new_offset(self.process_handle?, vec![FROG_EYES_FOLLOWING_OFFSET])
            .read()
            .ok()
    }

    fn get_mouse_coords(&self) -> Option<(u32, u32)> {
        DataMember::new_offset(self.process_handle?, MOUSE_COORDS_OFFSETS.to_vec())
            .read()
            .ok()
    }

    fn read_ball(&self, ball_address: Vec<usize>) -> Option<Ball> {
        let mem_ball: MemBall = DataMember::new_offset(self.process_handle?, ball_address)
            .read()
            .ok()?;

        Some(Ball {
            color: number_to_color(mem_ball.color).ok()?,
            coordinates: Point {
                x: mem_ball.x,
                y: mem_ball.y,
            },
            effect: number_to_effect(mem_ball.effect).ok()?,
            is_reachable: true,
            id: mem_ball.global_ball_number,
        })
    }

    pub fn update_frog_follow_eyes(&mut self) {
        self.frog_follow_eyes = self.get_frog_follow_eyes();
    }

    pub fn update_balls(&mut self) {
        self.game_state.balls.clear();

        let mem_stream_parent =
            DataMember::new_offset(self.process_handle.unwrap(), STREAM_PARENT_OFFSETS.to_vec())
                .read();

        let mem_stream_parent: Game = match mem_stream_parent {
            Ok(thing) => thing,
            _ => return,
        };

        for i in 0..mem_stream_parent.ballstream_count {
            let mem_stream: MemBallStream = DataMember::new_offset(
                self.process_handle.unwrap(),
                vec![mem_stream_parent.ballstream_ptrs[i as usize] as usize],
            )
            .read()
            .unwrap();

            // Get the linked list manager thingymajig
            let ball_linked_list: MemBallLinkedList = DataMember::new_offset(
                self.process_handle.unwrap(),
                vec![mem_stream.ptr_ball_linked_list as usize],
            )
            .read()
            .unwrap();

            // Get the balls!
            let mut next_mem_ball = ball_linked_list.ptr_first_elem;
            for _i in 0..mem_stream.ball_count {
                let elem: MemBallLinkedListElement = DataMember::new_offset(
                    self.process_handle.unwrap(),
                    vec![next_mem_ball as usize],
                )
                .read()
                .unwrap();
                next_mem_ball = elem.ptr_next_elem;

                match self.read_ball(vec![elem.ptr_ball as usize]) {
                    Some(ball) => self.game_state.balls.push(ball),
                    None => return,
                };
            }
        }
    }

    pub fn update_paused(&mut self) {
        let mem_stream_parent =
            DataMember::new_offset(self.process_handle.unwrap(), STREAM_PARENT_OFFSETS.to_vec())
                .read();

        let mem_stream_parent: Game = match mem_stream_parent {
            Ok(thing) => thing,
            _ => {
                self.paused = false;
                return;
            }
        };

        self.paused = mem_stream_parent.game_state != 0;
    }

    pub fn update_frog(&mut self) {
        let mem_stream_parent =
            DataMember::new_offset(self.process_handle.unwrap(), STREAM_PARENT_OFFSETS.to_vec())
                .read();

        let mem_stream_parent: Game = match mem_stream_parent {
            Ok(thing) => thing,
            _ => return,
        };

        let mem_frog: MemFrog = DataMember::new_offset(
            self.process_handle.unwrap(),
            vec![mem_stream_parent.ptr_frog as usize],
        )
        .read()
        .unwrap();

        let active_ball_maybe = self.read_ball(vec![mem_frog.ptr_active_ball as usize]);
        let next_ball_maybe = self.read_ball(vec![mem_frog.ptr_next_ball as usize]);
        if let (Some(active_ball), Some(next_ball)) = (active_ball_maybe, next_ball_maybe) {
            self.frog = Some(Frog {
                location: Point {
                    x: mem_frog.target_x as f32,
                    y: mem_frog.target_y as f32,
                },
                active_ball: active_ball.color,
                next_ball: next_ball.color,
            })
        }
    }

    pub fn update_mouse_coords(&mut self) {
        self.mouse_coords = self.get_mouse_coords();
    }
}

fn number_to_color(num: u32) -> Result<BallColor, String> {
    match num {
        0 => Ok(BallColor::Blue),
        1 => Ok(BallColor::Yellow),
        2 => Ok(BallColor::Red),
        3 => Ok(BallColor::Green),
        4 => Ok(BallColor::Purple),
        5 => Ok(BallColor::White),
        _ => Err(format!("color read: 0x{:X}", num)),
    }
}

fn number_to_effect(num: u32) -> Result<BallEffect, String> {
    match num {
        0 => Ok(BallEffect::Bomb),
        1 => Ok(BallEffect::Slow),
        2 => Ok(BallEffect::Visor),
        3 => Ok(BallEffect::Reverse),
        4 => Ok(BallEffect::None),
        _ => Err(format!("effect read: 0x{:X}", num)),
    }
}

struct ScannableProc {
    handle: Option<ProcessHandle>,
    reader_cursor: usize,
}

impl Read for ScannableProc {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let proc = if let Some(handle) = self.handle {
            handle
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                "Not connected to ZUMA process",
            ));
        };

        const READ_STEP: usize = 32;
        let remainder = buf.len() % READ_STEP;

        // If it doesn't work, go byte by byte
        for idx in (0..(buf.len() - remainder)).step_by(READ_STEP) {
            if self.reader_cursor >= 0xA000000 {
                return Ok(idx);
            }

            let data = DataMember::<[u8; READ_STEP]>::new_offset(proc, vec![self.reader_cursor]);
            buf[idx..idx + READ_STEP].copy_from_slice(&data.read().unwrap_or([0; READ_STEP]));
            self.reader_cursor = self.reader_cursor + READ_STEP;
        }

        for idx in 0..remainder {
            if self.reader_cursor >= 0xA000000 {
                return Ok(buf.len() - remainder + idx);
            }

            let data = DataMember::new_offset(proc, vec![self.reader_cursor]);
            buf[idx] = data.read().unwrap_or(0);
            self.reader_cursor = self.reader_cursor + 1;
        }

        Ok(buf.len())
    }
}

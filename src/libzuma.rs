use binrw::BinRead;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BallColor {
    Blue,
    Yellow,
    Red,
    Green,
    Purple,
    White,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BallEffect {
    None,
    Slow,
    Reverse,
    Bomb,
    Visor,
}

#[derive(Clone, Copy, Debug, PartialEq, BinRead)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball {
    pub coordinates: Point,
    pub is_reachable: bool, // false if it is in a tunnel for example
    pub color: BallColor,
    pub effect: BallEffect,
    pub distance_along_path: f32,
    pub id: u32,
}

impl Ball {
    pub fn dist_sq(&self, other: &Ball) -> f32 {
        (self.coordinates.x - other.coordinates.x).powf(2.)
            + (self.coordinates.y - other.coordinates.y).powf(2.)
    }
}

#[derive(Clone, Debug)]
pub struct GameState {
    pub balls: Vec<Ball>,
    pub curve: Curve,
    pub forward_speed: f32,
    pub back_speed: f32,
    pub backwards_time_left: u32,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            balls: vec![],
            curve: Curve::new(),
            forward_speed: 0.,
            back_speed: -0.5,
            backwards_time_left: 0,
        }
    }

    fn clear_at(&mut self, index: usize) -> Option<()> {
        let color_to_clear = self.balls[index].color;

        // Determine the adjacent balls of the same color
        // Minimum index
        let mut min_clear = index;
        while min_clear > 0 && self.balls[min_clear - 1].color == color_to_clear {
            min_clear -= 1;
        }
        // Maximum index
        let mut max_clear = index + 1;
        while max_clear < self.balls.len() && self.balls[max_clear].color == color_to_clear {
            max_clear += 1;
        }

        // If more than 3 balls are contiguous, actually clear them
        if max_clear - min_clear >= 3 {
            self.balls.drain(min_clear..max_clear);
        }

        Some(())
    }
}

#[derive(Debug)]
pub enum FrogType {
    Static(Point),        // If the frog does not move
    Jumper(Vec<Point>),   // If the frog has multiple possible positions
    Slider(Point, Point), // If the frog can move along a slider
}

#[derive(Clone, Copy, Debug)]
pub struct Frog {
    pub location: Point,
    pub active_ball: BallColor,
    pub next_ball: BallColor,
    pub ball_exit_speed: f32,
}

// Represents a curve that the balls follow along
#[derive(Clone, Debug)]
pub struct Curve {
    last_loaded: String,
    // Vector of (distance along path, point)
    pub points: Vec<Point>,
}

impl Curve {
    pub fn new() -> Self {
        Self {
            last_loaded: "".into(),
            points: vec![],
        }
    }

    // Read curve data from given file path
    pub fn read_from_file(&mut self, path: String) -> Option<()> {
        use std::fs;

        if self.last_loaded == path {
            return Some(());
        }

        let mut file = fs::File::open(path.clone()).ok()?;
        let curve = BinCurve::read_ne(&mut file).ok()?;

        self.last_loaded = path;
        let mut current_point = curve.start_point.0;
        self.points = curve
            .deltas
            .iter()
            .map(|p| {
                current_point.x += (p.x as f32) / 100.;
                current_point.y += (p.y as f32) / 100.;
                current_point
            })
            .collect();

        Some(())
    }

    // Return the position at the given distance from the start
    pub fn get_pos_from_dist(&self, dist: f32) -> Point {
        // This trick works only because the points are exactly 1 unit apart
        let idx = (dist as usize).min(self.points.len() - 1);
        self.points[idx]
    }

    pub fn get_normal_at_dist(&self, dist: f32) -> Point {
        let idx = (dist as usize).min(self.points.len() - 2);
        // The delta between consecutive points rotated by 90 degrees
        // is a pretty good normal
        Point {
            x: self.points[idx+1].y - self.points[idx].y,
            y: self.points[idx].x - self.points[idx+1].x,
        }
    }
}

#[derive(BinRead)]
struct BinCurveTunnelData {
    is_tunnel: u8,
    unk2: u8, // Possibly a z thing to define tunnels
}

#[derive(BinRead)]
struct BinCurvePoint {
    x: u32,
    y: u32,
    tunnel_data: BinCurveTunnelData,
}

#[derive(BinRead)]
struct BinCurveDelta {
    x: i8,
    y: i8,
    tunnel_data: BinCurveTunnelData,
}

#[derive(BinRead)]
#[br(magic = b"CURV")]
struct BinCurve {
    // Header
    unkown1: i32,
    unkown2: i32,
    size: u32,

    // Points section
    _point_count: u32,
    #[br(count = _point_count)]
    points: Vec<BinCurvePoint>,

    // Deltas section
    _deltas_count: u32,
    start_point: (Point, BinCurveTunnelData),
    #[br(count = _deltas_count-1)]
    deltas: Vec<BinCurveDelta>,
}

#[test]
fn clear() {
    use crate::libzuma::*;
    let red_ball = Ball {
        coordinates: Point { x: 0.0, y: 0.0 },
        is_reachable: false,
        color: BallColor::Red,
        effect: BallEffect::None,
        distance_along_path: 0.,
        id: 0,
    };

    let blue_ball = Ball {
        coordinates: Point { x: 0.0, y: 0.0 },
        is_reachable: false,
        color: BallColor::Blue,
        effect: BallEffect::None,
        distance_along_path: 0.,
        id: 0,
    };

    let seq = GameState {
        balls: vec![
            blue_ball.clone(),
            red_ball.clone(),
            red_ball.clone(),
            red_ball.clone(),
            blue_ball.clone(),
        ],
        ..GameState::new()
    };

    let mut seq_cleared = seq.clone();
    seq_cleared.clear_at(3);

    // We test that the balls have been cleared
    assert_eq!(5, seq.balls.len());
    assert_eq!(1, seq_cleared.balls.len());
}

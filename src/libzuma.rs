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

#[derive(Clone, Copy, Debug, PartialEq)]
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
}

#[derive(Clone, Debug)]
pub struct BallSequence {
    pub balls: Vec<Ball>,
}

impl BallSequence {
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
}

#[test]
fn clear() {
    use crate::libzuma::*;
    let red_ball = Ball {
        coordinates: Point { x: 0.0, y: 0.0 },
        is_reachable: false,
        color: BallColor::Red,
        effect: BallEffect::None,
    };

    let blue_ball = Ball {
        coordinates: Point { x: 0.0, y: 0.0 },
        is_reachable: false,
        color: BallColor::Blue,
        effect: BallEffect::None,
    };

    let seq = BallSequence {
        balls: vec![
            blue_ball.clone(),
            red_ball.clone(),
            red_ball.clone(),
            red_ball.clone(),
            blue_ball.clone(),
        ],
    };

    let mut seq_cleared = seq.clone();
    seq_cleared.clear_at(3);

    // We test that the balls have been cleared
    assert_eq!(5, seq.balls.len());
    assert_eq!(1, seq_cleared.balls.len());
}

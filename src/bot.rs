use crate::libzuma::*;

#[derive(Clone, Copy)]
pub enum BotMove {
    Nothing,
    Shoot(Point),
    SwapShoot(Point),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotMode {
    ColorBot,
    PalindromeBreaker,
}

impl BotMode {
    pub const ALL: &[Self] = &[Self::ColorBot, Self::PalindromeBreaker];
}

impl std::fmt::Display for BotMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BotMode::ColorBot => "Color matcher",
            BotMode::PalindromeBreaker => "Simple palindrome breaker",
        }
        .fmt(f)
    }
}

pub struct Shot {
    ball_id: u32,   // Id of the ball that was shot
    target_id: u32, // Id of the target ball
    shot_time: std::time::Instant,
    expected_travel_time: std::time::Duration,
}

pub fn suggest_shot(
    frog: &Frog,
    state: &GameState,
    mode: BotMode,
    memo: &mut Vec<Shot>,
) -> BotMove {
    match mode {
        BotMode::ColorBot => suggest_shot_color(frog, state, memo),
        BotMode::PalindromeBreaker => suggest_shot_palidrome_simple(frog, state, memo),
    }
}

pub fn reachable_balls(frog: &Frog, balls: &GameState) -> Vec<Ball> {
    let mut reachable_balls = vec![];

    for ball_src in balls.balls.iter() {
        let mut ball_has_line_of_sight = true;
        for ball_obstacle in balls.balls.iter() {
            if ball_src == ball_obstacle {
                continue;
            }

            let src_vec = Point {
                x: ball_src.coordinates.x - frog.location.x,
                y: ball_src.coordinates.y - frog.location.y,
            };
            let obstacle_vec = Point {
                x: ball_obstacle.coordinates.x - frog.location.x,
                y: ball_obstacle.coordinates.y - frog.location.y,
            };

            let k_leeway = 27. / (src_vec.x * obstacle_vec.x + src_vec.y * obstacle_vec.y).sqrt();

            let k = (src_vec.x * obstacle_vec.x + src_vec.y * obstacle_vec.y)
                / (src_vec.x.powi(2) + src_vec.y.powi(2));
            let projected_point = Point {
                x: frog.location.x + k * src_vec.x,
                y: frog.location.y + k * src_vec.y,
            };

            let dist_sq_circle_to_line = (projected_point.x - ball_obstacle.coordinates.x).powi(2)
                + (projected_point.y - ball_obstacle.coordinates.y).powi(2);
            let radius_sq = 32_f32.powi(2);
            if dist_sq_circle_to_line < radius_sq && k_leeway < k && k < 1. - k_leeway {
                ball_has_line_of_sight = false;
                break;
            }
        }

        if ball_has_line_of_sight {
            reachable_balls.push(*ball_src);
        }
    }

    reachable_balls
}

#[derive(Debug)]
struct Palindrome {
    center: usize,
    sequence: Vec<(BallColor, u32)>,
}

impl Palindrome {
    fn get_breaking_len(&self) -> f32 {
        let mut count = if self.sequence[0].1 > 1 {
            1.
        } else {
            return 0.;
        };

        for item in &self.sequence {
            if item.1 > 3 {
                count += 1.;
            } else {
                count += 0.5;
                break;
            }
        }

        count
    }
}

fn find_palidromes(balls: &GameState) -> Vec<Palindrome> {
    // Transform the ball sequence into a [(color, count, ball_idx)]
    let mut rle_balls = vec![];
    for (i, ball) in balls.balls.iter().enumerate() {
        match rle_balls.last() {
            Some(&(color, count, _)) if color == ball.color => {
                (*rle_balls.last_mut().unwrap()).1 = count + 1
            }
            _ => rle_balls.push((ball.color, 1, i)),
        }
    }

    // Find the palindromes and the counts
    let mut palindromes = vec![];
    for (i, &(color, count, ball_idx)) in rle_balls.iter().enumerate() {
        let mut radius = 1;
        let mut sequence = vec![(color, count)];

        loop {
            match (rle_balls.get(i - radius), rle_balls.get(i + radius)) {
                (Some(&before), Some(&after)) if before.0 == after.0 => {
                    sequence.push((before.0, before.1 + after.1))
                }
                _ => break,
            }
            radius += 1;
        }

        palindromes.push(Palindrome {
            center: ball_idx,
            sequence,
        });
    }

    palindromes
}

// Compute the future position of the ball based on travel time
// TODO: add an adjustment along the normal of the track towards the frog
// (to make aim better in situations where the track isn't perfectly perpandicular to the frog)
pub fn adjust_for_travel_time(
    frog: &Frog,
    state: &GameState,
    target_idx: usize,
    memo: &Vec<Shot>,
) -> (Point, std::time::Duration) {
    let target_ball = &state.balls[target_idx];

    // Compute travel time
    let dist_sq = (frog.location.x - target_ball.coordinates.x).powi(2)
        + (frog.location.y - target_ball.coordinates.y).powi(2);
    let dist = dist_sq.sqrt();
    let travel_time = dist / frog.ball_exit_speed;

    // Determine the start and end indices of the ball group
    let mut comparison = &state.balls[target_idx];
    let gap_before_start = state.balls[..target_idx].iter().rposition(|ball| {
        let res = comparison.dist_sq(ball) > 32.5_f32.powi(2);
        comparison = ball;
        res
    });
    let mut comparison = &state.balls[target_idx];
    let gap_before_end = state.balls[target_idx..].iter().position(|ball| {
        let res = comparison.dist_sq(ball) > 32.5_f32.powi(2);
        comparison = ball;
        res
    });
    let ball_speed = match (
        state.backwards_time_left > 0,
        gap_before_end.is_some(),
        gap_before_start.is_some(),
    ) {
        (true, false, _) => state.back_speed,
        (false, _, false) => state.forward_speed,
        _ => 0.0,
    };

    // Compute number of balls that will be inserted
    let mut inserted_balls = 0;
    for ball in &state.balls[gap_before_start.unwrap_or(0)..target_idx] {
        if memo.iter().any(|shot| shot.target_id == ball.id) {
            inserted_balls += 1;
        }
    }

    // Compute ball distance within that time
    let point = state
        .curve
        .get_pos_from_dist(target_ball.distance_along_path + ball_speed * travel_time + (inserted_balls as f32) * 32.1);

    (point, std::time::Duration::from_millis((travel_time * 16.) as u64))
}

pub fn suggest_shot_color(frog: &Frog, state: &GameState, memo: &mut Vec<Shot>) -> BotMove {
    if state.balls.len() == 0 {
        return BotMove::Nothing;
    }

    // Update memo:
    // If the id of the ball that was shot matches one of the balls, remove it
    // If the ball was shot too long ago, remove it
    for ball in &state.balls {
        memo.retain(|shot| {
            shot.ball_id != ball.id && shot.shot_time.elapsed() < shot.expected_travel_time
        });
    }

    // Transform the ball sequence into a [(color, count, ball_idx)]
    let mut rle_balls = vec![];
    for (i, ball) in state.balls.iter().rev().enumerate() {
        let i = state.balls.len() - 1 - i;
        match rle_balls.last() {
            Some(&(color, count, _)) if color == ball.color => {
                rle_balls.last_mut().unwrap().1 = count + 1;
                rle_balls.last_mut().unwrap().2 = i;
            }
            _ => rle_balls.push((ball.color, 1, i)),
        }
    }
    rle_balls.sort_by_key(|k| -(k.1 as i32));
    let reachable_balls = reachable_balls(&frog, state);

    let mut ball_to_shoot = None;
    for (color, count, idx) in rle_balls {
        if color == frog.active_ball.color {
            let ball_group = &state.balls[idx..idx + count];
            let lowest_reachable_pos = reachable_balls
                .iter()
                .position(|ball| ball_group.contains(&ball));
            if let Some(best_pos) = lowest_reachable_pos {
                ball_to_shoot = Some(best_pos);
                break;
            }
        }
    }

    let ball_to_shoot = ball_to_shoot.unwrap_or(reachable_balls.len() - 1);
    // Transform the index into an index of state.balls
    let ball_to_shoot = state
        .balls
        .iter()
        .position(|&ball| ball == reachable_balls[ball_to_shoot])
        .unwrap();

    let (target_point, travel_time) = adjust_for_travel_time(&frog, state, ball_to_shoot, memo);

    // Add ball to memo
    memo.push(Shot {
        ball_id: frog.active_ball.id,
        target_id: state.balls[ball_to_shoot].id,
        shot_time: std::time::Instant::now(),
        expected_travel_time: travel_time,
    });

    BotMove::Shoot(target_point)
}

pub fn suggest_shot_palidrome_simple(
    frog: &Frog,
    state: &GameState,
    memo: &mut Vec<Shot>,
) -> BotMove {
    if state.balls.len() < 4 {
        return BotMove::Nothing;
    }

    let reachable_balls = reachable_balls(&frog, state);
    let mut palindromes = find_palidromes(state);
    palindromes.sort_by_key(|k| -(k.get_breaking_len() as i32));

    let mut target = None;
    for palindrome in palindromes {
        let palindrome_center = state.balls[palindrome.center];
        if palindrome_center.color == frog.active_ball.color
            && reachable_balls.contains(&palindrome_center)
        {
            target = Some(palindrome_center);
            break;
        }
    }

    let target = target.unwrap_or(reachable_balls[reachable_balls.len() - 1]);
    // Transform the index into an index of state.balls
    let ball_to_shoot = state.balls.iter().position(|&ball| ball == target).unwrap();

    BotMove::Shoot(adjust_for_travel_time(&frog, state, ball_to_shoot, memo).0)
}

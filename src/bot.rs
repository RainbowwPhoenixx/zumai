use crate::libzuma::*;

#[derive(Clone, Copy)]
pub enum BotMove {
    Nothing,
    Shoot(Point),
    SwapShoot(Point),
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
            let radius_sq = 31_f32.powi(2);
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
    fn get_breaking_len(&self) -> u32 {
        let mut count = if self.sequence[0].1 > 1 { 1 } else { return 0 };

        for item in &self.sequence {
            if item.1 > 3 {
                count += 1;
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
pub fn adjust_for_travel_time(frog: &Frog, state: &GameState, target_idx: usize) -> Point {
    let target_ball = &state.balls[target_idx];

    // Compute travel time
    let dist_sq = (frog.location.x - target_ball.coordinates.x).powi(2)
        + (frog.location.y - target_ball.coordinates.y).powi(2);
    let dist = dist_sq.sqrt();
    let travel_time = dist / frog.ball_exit_speed;

    // Determine ball speed
    // TODO: only compute stuff when needed
    // If reversing and touching the end: backwards speed
    // else if touching the start: forward speed
    let mut comparison = &state.balls[0];
    let gap_before_start = state.balls[..target_idx].iter().any(|ball| {
        let res = comparison.dist_sq(ball) > 32.5_f32.powi(2);
        comparison = ball;
        res
    });
    let mut comparison = &state.balls[0];
    let gap_before_end = state.balls[target_idx..].iter().any(|ball| {
        let res = comparison.dist_sq(ball) > 32.5_f32.powi(2);
        comparison = ball;
        res
    });
    let ball_speed = match (state.backwards_time_left > 0, gap_before_end, gap_before_start) {
        (true, false, _) => state.back_speed,
        (false, _, false) => state.forward_speed,
        _ => 0.0,
    };

    // Compute ball distance within that time
    state
        .curve
        .get_pos_from_dist(target_ball.distance_along_path + ball_speed * travel_time)
}

pub fn suggest_shot_color(frog: Frog, state: &GameState) -> BotMove {
    // Memo contains the last 2 shots that resulted in balls popping
    // (to avoid reshooting in the same place)
    if state.balls.len() == 0 {
        return BotMove::Nothing;
    }

    // Transform the ball sequence into a [(color, count, ball_idx)]
    let mut rle_balls = vec![];
    for (i, ball) in state.balls.iter().enumerate() {
        match rle_balls.last() {
            Some(&(color, count, _)) if color == ball.color => {
                (*rle_balls.last_mut().unwrap()).1 = count + 1
            }
            _ => rle_balls.push((ball.color, 1, i)),
        }
    }
    rle_balls.sort_by_key(|k| -(k.1 as i32));
    let reachable_balls = reachable_balls(&frog, state);

    let mut ball_to_shoot = None;
    for (color, count, idx) in rle_balls {
        if color == frog.active_ball {
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
    let ball_to_shoot = state.balls.iter().position(|&ball| ball == reachable_balls[ball_to_shoot]).unwrap();

    BotMove::Shoot(adjust_for_travel_time(&frog, state, ball_to_shoot))
}

pub fn suggest_shot_palidrome_simple(frog: Frog, balls: &GameState) -> BotMove {
    if balls.balls.len() < 4 {
        return BotMove::Nothing;
    }

    let reachable_balls = reachable_balls(&frog, balls);
    let mut palindromes = find_palidromes(balls);
    palindromes.sort_by_key(|k| -(k.get_breaking_len() as i32));

    let mut target = None;
    for palindrome in palindromes {
        let palindrome_center = balls.balls[palindrome.center];
        if palindrome_center.color == frog.active_ball
            && reachable_balls.contains(&palindrome_center)
        {
            target = Some(palindrome_center);
            break;
        }
    }

    // Determine move
    match target {
        Some(ball) => BotMove::Shoot(ball.coordinates),
        None => BotMove::Shoot(reachable_balls[reachable_balls.len() - 1].coordinates),
    }
}

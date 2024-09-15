use std::fmt::{Display, Formatter};

pub struct AsciiCanvas {
    width: usize,
    height: usize,
    canvas: Vec<Vec<bool>>,
}

impl AsciiCanvas {
    pub fn new(width: usize, height: usize) -> Self {
        AsciiCanvas { width, height, canvas: vec![vec![false; width]; height] }
    }

    /// Draws a line on a 2d character grid using Bresenham's line algorithm.
    pub fn draw_line(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
        let mut x_curr = x1 as i64;
        let mut y_curr = y1 as i64;
        let x_end = x2 as i64;
        let y_end = y2 as i64;

        let dx = (x_end - x_curr).abs();
        let dy = -(y_end - y_curr).abs();
        let sx = if x_curr < x_end { 1 } else { -1 };
        let sy = if y_curr < y_end { 1 } else { -1 };
        let mut err = dx + dy;

        let canvas = &mut self.canvas;
        while x_curr != x_end || y_curr != y_end {
            draw_point(canvas, x_curr, y_curr);
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x_curr += sx;
            }
            if e2 <= dx {
                err += dx;
                y_curr += sy;
            }
        }

        draw_point(canvas, x_end, y_end)
    }
}

/// Draws a point on a 2d character grid.
fn draw_point(canvas: &mut Vec<Vec<bool>>, x: i64, y: i64) {
    if x >= 0 && x < canvas[0].len() as i64 && y >= 0 && y < canvas.len() as i64 {
        canvas[y as usize][x as usize] = true;
    }
}

impl Display for AsciiCanvas {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut output = String::with_capacity(self.height * (self.width + 1));
        (&self.canvas).into_iter().for_each(|row| {
            row.iter().for_each(|&ch| {
                let char = if ch {
                    '*'
                } else {
                    ' '
                };
                output.push(char);
            });
            output.push('\n');
        });
        write!(f, "{}", output)
    }
}
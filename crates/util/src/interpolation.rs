#[derive(Debug)]
pub struct InterpolationBuffer<T: Interpolate, const N: usize> {
    data: [T; N],
    last: usize,
}

pub trait Interpolate {
    fn interpolate(a: Self, b: Self, t: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(a: Self, b: Self, t: f32) -> Self {
        a * (1.0 - t) + b * t
    }
}

impl<T: Interpolate + Copy + PartialEq, const N: usize> InterpolationBuffer<T, N> {
    pub fn new(value: T, last: usize) -> Self {
        Self {
            data: [value; N],
            last,
        }
    }

    pub fn insert(&mut self, x: usize, y: T) {
        if x > self.last {
            let last_y = self.data[self.last % N];
            for i in self.last + 1..x + 1 {
                let t = (i - self.last) as f32 / (x - self.last) as f32;
                self.insert_old(i, T::interpolate(last_y, y, t));
            }
        } else {
            self.insert_old(x, y);
        }
        self.last = x;
    }

    fn insert_old(&mut self, x: usize, y: T) {
        if x >= self.last.saturating_sub(N - 1) {
            self.data[x % N] = y;
        }
    }

    pub fn last(&self) -> &T {
        &self.data[self.last % N]
    }

    pub fn get(&self, x: f32) -> T {
        let ix = x as usize;
        if ix >= self.last {
            self.data[self.last % N]
        } else if ix >= self.last.saturating_sub(N - 1) {
            let t = x - ix as f32;
            T::interpolate(self.data[ix % N], self.data[(ix + 1) % N], t)
        } else {
            self.data[(self.last + 1) % N]
        }
    }
}

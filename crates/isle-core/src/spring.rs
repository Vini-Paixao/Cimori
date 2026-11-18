/// The longest step a spring integrates at once (s).
const SUBSTEP: f32 = 1.0 / 240.0;
/// The most time one frame moves a spring on (s): past a hitch this long it slows down instead.
const MAX_STEP: f32 = 0.1;

#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    pub fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
            stiffness,
            damping,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Moves the spring on by `dt` seconds, in steps of at most `SUBSTEP`: a late frame puts it where
    /// it would be by then. (One Euler step of 50 ms covered k*dt^2 = 80% of the way at once.)
    pub fn step(&mut self, dt: f32) -> bool {
        let mut left = dt.clamp(0.0, MAX_STEP);
        while left > 0.0 {
            let dt = left.min(SUBSTEP);
            left -= dt;
            let displacement = self.value - self.target;
            let spring_force = -self.stiffness * displacement;
            let damping_force = -self.damping * self.velocity;
            let acceleration = spring_force + damping_force;

            self.velocity += acceleration * dt;
            self.value += self.velocity * dt;
        }

        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            false
        } else {
            true
        }
    }

    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < 0.25 && self.velocity.abs() < 0.5
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PillGeometry {
    pub width: Spring,
    pub height: Spring,
    pub corner_radius: Spring,
    pub glow_intensity: Spring,
}

impl Default for PillGeometry {
    fn default() -> Self {
        Self {
            width: Spring::new(220.0, 320.0, 28.0),
            height: Spring::new(34.0, 320.0, 28.0),
            corner_radius: Spring::new(17.0, 320.0, 28.0),
            glow_intensity: Spring::new(0.0, 180.0, 22.0),
        }
    }
}

impl PillGeometry {
    pub fn morph_to(&mut self, width: f32, height: f32, radius: f32) {
        self.width.set_target(width);
        self.height.set_target(height);
        self.corner_radius.set_target(radius);
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        let w = self.width.step(dt);
        let h = self.height.step(dt);
        let r = self.corner_radius.step(dt);
        let g = self.glow_intensity.step(dt);
        w || h || r || g
    }

    pub fn is_settled(&self) -> bool {
        self.width.is_settled()
            && self.height.is_settled()
            && self.corner_radius.is_settled()
            && self.glow_intensity.is_settled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_settles_to_exact_target_and_stops_ticking() {
        let mut geom = PillGeometry::default();
        geom.morph_to(420.0, 180.0, 24.0);
        let mut moving = true;
        for _ in 0..300 {
            moving = geom.tick(1.0 / 60.0);
            if !moving {
                break;
            }
        }
        assert!(!moving);
        assert_eq!(geom.width.value, 420.0);
        assert_eq!(geom.height.value, 180.0);
    }

    /// Where the pill is follows elapsed time, not how many frames drew it: 30 fps, 60 fps and a
    /// late 50 ms frame all put it at the same place.
    #[test]
    fn the_spring_follows_elapsed_time_whatever_the_frame_rate() {
        let at = |dts: &[f32]| {
            let mut s = Spring::new(34.0, 320.0, 28.0);
            s.set_target(450.0);
            for dt in dts { s.step(*dt); }
            s.value
        };
        let sixty = at(&[1.0 / 60.0; 6]);
        assert!((at(&[1.0 / 30.0; 3]) - sixty).abs() < 1.0, "{} vs {sixty}", at(&[1.0 / 30.0; 3]));
        assert!((at(&[0.05, 0.05]) - sixty).abs() < 1.0, "{} vs {sixty}", at(&[0.05, 0.05]));
        // The first frame does not leap: after one 60 fps frame it has gone a few percent of the way.
        assert!(at(&[1.0 / 60.0]) < 34.0 + 0.1 * 416.0, "{}", at(&[1.0 / 60.0]));
    }

    /// A new target mid-flight keeps the value and the speed: no jump, it turns smoothly.
    #[test]
    fn retargeting_keeps_value_and_velocity() {
        let mut s = Spring::new(34.0, 320.0, 28.0);
        s.set_target(450.0);
        for _ in 0..5 { s.step(1.0 / 60.0); }
        let (value, velocity) = (s.value, s.velocity);
        s.set_target(34.0);
        assert_eq!((s.value, s.velocity), (value, velocity));
        s.step(1.0 / 60.0);
        assert!((s.value - value).abs() < velocity.abs() / 60.0 + 1.0, "{value} -> {}", s.value);
    }
}

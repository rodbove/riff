#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unit {
    Db,
    Percent,
    Ms,
    Hz,
    Plain,
    Semi,
    Choice(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: Unit,
    pub curve: f32,
}

impl Param {
    pub const fn new(name: &'static str, min: f32, max: f32, default: f32, unit: Unit) -> Self {
        Self {
            name,
            min,
            max,
            default,
            unit,
            curve: 1.0,
        }
    }

    pub const fn curved(self, curve: f32) -> Self {
        Self { curve, ..self }
    }

    pub const fn knob(name: &'static str, default: f32) -> Self {
        Self::new(name, 0.0, 10.0, default, Unit::Plain)
    }

    pub const fn choice(
        name: &'static str,
        options: &'static [&'static str],
        default: f32,
    ) -> Self {
        Self::new(
            name,
            0.0,
            (options.len() - 1) as f32,
            default,
            Unit::Choice(options),
        )
    }

    pub fn clamp(&self, v: f32) -> f32 {
        let v = v.clamp(self.min, self.max);
        if matches!(self.unit, Unit::Choice(_) | Unit::Semi) {
            v.round()
        } else {
            v
        }
    }

    pub fn to_unit(&self, v: f32) -> f32 {
        let t = ((v - self.min) / (self.max - self.min)).clamp(0.0, 1.0);
        t.powf(1.0 / self.curve)
    }

    pub fn from_unit(&self, t: f32) -> f32 {
        self.clamp(self.min + t.clamp(0.0, 1.0).powf(self.curve) * (self.max - self.min))
    }

    pub fn text(&self, v: f32) -> String {
        match self.unit {
            Unit::Db => format!("{v:+.1} dB"),
            Unit::Percent => format!("{:.0}%", v * 100.0),
            Unit::Ms => {
                if v >= 1000.0 {
                    format!("{:.2} s", v / 1000.0)
                } else {
                    format!("{v:.0} ms")
                }
            }
            Unit::Hz => {
                if v >= 1000.0 {
                    format!("{:.1} kHz", v / 1000.0)
                } else {
                    format!("{v:.0} Hz")
                }
            }
            Unit::Plain => format!("{v:.1}"),
            Unit::Semi => format!("{v:+.0} st"),
            Unit::Choice(options) => {
                options[(v.round().max(0.0) as usize).min(options.len() - 1)].into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_mapping_a_knob {
        use super::*;

        #[test]
        fn round_trips_through_the_unit_range() {
            let p = Param::new("TIME", 10.0, 2000.0, 400.0, Unit::Ms).curved(3.0);
            assert!((p.from_unit(p.to_unit(400.0)) - 400.0).abs() < 1e-2);
        }

        #[test]
        fn snaps_choices() {
            let p = Param::choice("MODE", &["A", "B", "C"], 0.0);
            assert_eq!(p.from_unit(0.6), 1.0);
            assert_eq!(p.text(2.0), "C");
        }
    }
}

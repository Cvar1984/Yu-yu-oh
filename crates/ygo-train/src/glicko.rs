//! Glicko-2 ratings (Glickman, "Example of the Glicko-2 system", 2012).
//! A rating period updates every player from the games they played in it,
//! each against the opponent's rating at the start of the period.

use serde::{Deserialize, Serialize};

/// The Glicko-2 to Glicko scale factor.
const SCALE: f64 = 173.7178;
/// The system constant: how much volatility may change in a period.
pub const TAU: f64 = 0.5;

/// A rating on the familiar Glicko scale (1500 for a newcomer), its
/// deviation and its volatility.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rating {
    pub rating: f64,
    pub deviation: f64,
    pub volatility: f64,
}

impl Default for Rating {
    fn default() -> Self {
        Self {
            rating: 1500.0,
            deviation: 350.0,
            volatility: 0.06,
        }
    }
}

impl Rating {
    /// The rating minus twice the deviation: what the table ranks by, so
    /// a newcomer with few games does not top it by luck.
    pub fn conservative(&self) -> f64 {
        self.rating - 2.0 * self.deviation
    }

    /// The rating after a period with these games: each the opponent's
    /// rating at the start of the period and this player's score (1 a win,
    /// 0.5 a draw, 0 a loss). With no games only the deviation grows.
    pub fn update(&self, games: &[(Rating, f64)], tau: f64) -> Rating {
        let mu = (self.rating - 1500.0) / SCALE;
        let phi = self.deviation / SCALE;
        let sigma = self.volatility;
        if games.is_empty() {
            return Rating {
                deviation: (phi * phi + sigma * sigma).sqrt() * SCALE,
                ..*self
            };
        }
        let g =
            |phi_j: f64| 1.0 / (1.0 + 3.0 * phi_j * phi_j / (std::f64::consts::PI.powi(2))).sqrt();
        let (mut inv_v, mut sum) = (0.0, 0.0);
        for (opp, score) in games {
            let mu_j = (opp.rating - 1500.0) / SCALE;
            let g_j = g(opp.deviation / SCALE);
            let e = 1.0 / (1.0 + (-g_j * (mu - mu_j)).exp());
            inv_v += g_j * g_j * e * (1.0 - e);
            sum += g_j * (score - e);
        }
        let v = 1.0 / inv_v;
        let delta = v * sum;
        // The new volatility: the root of f by the Illinois method.
        let a = (sigma * sigma).ln();
        let f = |x: f64| {
            let ex = x.exp();
            ex * (delta * delta - phi * phi - v - ex) / (2.0 * (phi * phi + v + ex).powi(2))
                - (x - a) / (tau * tau)
        };
        let mut big_a = a;
        let mut big_b = if delta * delta > phi * phi + v {
            (delta * delta - phi * phi - v).ln()
        } else {
            let mut k = 1.0;
            while f(a - k * tau) < 0.0 {
                k += 1.0;
            }
            a - k * tau
        };
        let (mut fa, mut fb) = (f(big_a), f(big_b));
        while (big_b - big_a).abs() > 1e-6 {
            let c = big_a + (big_a - big_b) * fa / (fb - fa);
            let fc = f(c);
            if fc * fb <= 0.0 {
                big_a = big_b;
                fa = fb;
            } else {
                fa /= 2.0;
            }
            big_b = c;
            fb = fc;
        }
        let sigma_new = (big_a / 2.0).exp();
        let phi_star = (phi * phi + sigma_new * sigma_new).sqrt();
        let phi_new = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
        let mu_new = mu + phi_new * phi_new * sum;
        Rating {
            rating: mu_new * SCALE + 1500.0,
            deviation: phi_new * SCALE,
            volatility: sigma_new,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glickmans_worked_example() {
        let player = Rating {
            rating: 1500.0,
            deviation: 200.0,
            volatility: 0.06,
        };
        let opp = |rating, deviation| Rating {
            rating,
            deviation,
            volatility: 0.06,
        };
        let new = player.update(
            &[
                (opp(1400.0, 30.0), 1.0),
                (opp(1550.0, 100.0), 0.0),
                (opp(1700.0, 300.0), 0.0),
            ],
            TAU,
        );
        assert!((new.rating - 1464.06).abs() < 0.01, "{new:?}");
        assert!((new.deviation - 151.52).abs() < 0.01, "{new:?}");
        assert!((new.volatility - 0.05999).abs() < 0.00001, "{new:?}");
    }

    #[test]
    fn a_period_without_games_widens_the_deviation_only() {
        let r = Rating {
            rating: 1600.0,
            deviation: 100.0,
            volatility: 0.06,
        };
        let new = r.update(&[], TAU);
        assert_eq!(new.rating, 1600.0);
        assert!(new.deviation > 100.0 && new.deviation < 101.0, "{new:?}");
    }
}

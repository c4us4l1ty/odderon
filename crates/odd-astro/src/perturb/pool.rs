//! A propellant pool that venting and burning draw from together.
//!
//! FDR v3 integrated cryogenic boil-off *beside* the flight: the vent lost
//! mass, the record signed it, and the trajectory ignored it. The tug could
//! therefore reach its final burn carrying propellant that a real vehicle had
//! already released to space, and a record could claim a loss it never paid
//! for. `PropellantPool` is the v4 remedy: one pool per flight, and the only
//! way either process can remove mass from it.
//!
//! It is deliberately a *pure accounting* object with no ODE of its own. The
//! two consumers are
//!
//! * **The engine burn**, through [`BurnSchedule`]: each burn's mass flow is
//!   `F / (Isp · g₀)` until the pool is dry, at which point thrust is zero.
//! * **The tank vent**, through [`PropellantPool::set_vented`]: the tank model
//!   integrates its own temperature and lost mass, and reports the running
//!   total. Actually removing that mass from the vehicle is this type's job.
//!
//! Splitting it this way keeps the tank ODE exactly as validated (one shared
//! adaptive stepper, unchanged numerics) while still making the coupling
//! physical: the trajectory's mass is the fitted dry mass plus whatever the
//! pool still holds.
//!
//! A vent is subtracted from the tank it came out of, so an oxidizer boil-off
//! cannot quietly eat the fuel the last burn needs.

use crate::perturb::burn::{Burn, G0};

/// Why the pool is dry, which is what a debrief has to explain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flameout {
    /// Every kilogram scheduled for the flight was consumed before the last
    /// scheduled burn finished.
    ExhaustedDuringBurn,
}

/// The live propellant state of one flown stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropellantPool {
    /// Mass the stage left the pad with, kg. Fixed for the flight.
    initial_kg: f64,
    /// Oxidizer and fuel still aboard at saturated-liquid density, kg.
    remaining_kg: [f64; 2],
    /// Vent loss already removed from `remaining_kg`, per tank, kg.
    vented_kg: [f64; 2],
    /// Burn loss already removed from `remaining_kg`, kg.
    burned_kg: f64,
    /// True once the pool is dry.
    empty: bool,
    /// The mixture the engines were sized for, held fixed for the flight.
    of_ratio: f64,
}

impl PropellantPool {
    /// A pool holding `load_kg` split by mixture ratio. Returns `None` for a
    /// non-finite or non-positive load or an impossible ratio, because a pool
    /// that cannot be built must never be mistaken for a dry one.
    pub fn new(load_kg: f64, of_ratio: f64) -> Option<Self> {
        if !load_kg.is_finite()
            || load_kg <= 0.0
            || !of_ratio.is_finite()
            || of_ratio <= 0.0
        {
            return None;
        }
        let fuel_kg = load_kg / (1.0 + of_ratio);
        let oxidizer_kg = load_kg - fuel_kg;
        if !(fuel_kg > 0.0) || !(oxidizer_kg > 0.0) {
            return None;
        }
        Some(Self {
            initial_kg: load_kg,
            remaining_kg: [oxidizer_kg, fuel_kg],
            vented_kg: [0.0; 2],
            burned_kg: 0.0,
            empty: false,
            of_ratio,
        })
    }

    pub fn initial_kg(&self) -> f64 {
        self.initial_kg
    }

    /// Usable propellant still aboard: what drag, SRP and thrust divide by.
    pub fn remaining_kg(&self) -> f64 {
        self.remaining_kg[0] + self.remaining_kg[1]
    }

    pub fn vented_kg(&self) -> [f64; 2] {
        self.vented_kg
    }

    pub fn burned_kg(&self) -> f64 {
        self.burned_kg
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }

    /// Whether the pool carries the mixture the engine needs to run at all.
    /// A tank that boils dry starves the engine even while the other is full,
    /// which is why the two halves are tracked separately.
    pub fn has_both_fluids(&self) -> bool {
        self.remaining_kg[0] > 0.0 && self.remaining_kg[1] > 0.0
    }

    /// Remove the cumulative vent loss a tank model reports.
    ///
    /// `cumulative_kg` is the tank's *running total*, not an increment, so a
    /// caller can report it at every accepted step without double-counting.
    /// Returns the mass actually removed on this call. A loss above what is
    /// left is clamped to the tank's contents rather than underflowing the
    /// pool: a vent cannot release propellant that has already burned.
    pub fn set_vented(&mut self, tank: usize, cumulative_kg: f64) -> f64 {
        if tank >= 2 || !cumulative_kg.is_finite() || cumulative_kg < 0.0 {
            return 0.0;
        }
        // Venting is monotonic; a smaller report is a stale sample, not a
        // refill, and must not hand propellant back to the flight.
        let target = cumulative_kg.max(self.vented_kg[tank]);
        let delta = target - self.vented_kg[tank];
        let removed = delta.min(self.remaining_kg[tank]);
        self.remaining_kg[tank] -= removed;
        self.vented_kg[tank] += removed;
        self.refresh_empty();
        removed
    }

    /// Draw the mass one burn consumes over `duration_s` at its own flow rate,
    /// bounded by what is aboard and by how much of the burn window is left.
    ///
    /// Bounding by the *window* matters: a burn is a constant-force segment, so
    /// a pool that can only feed it for half the scheduled time must thrust
    /// for exactly that half, not for the whole window at reduced force.
    pub fn draw_burn(&mut self, burn: &Burn, window_s: f64) -> f64 {
        if !window_s.is_finite() || window_s <= 0.0 || self.empty {
            return 0.0;
        }
        let flow = burn.mass_flow_kg_s();
        if !flow.is_finite() || flow <= 0.0 {
            return 0.0;
        }
        let wanted = flow * window_s;
        // The engine stops when either fluid runs out.
        let available = self.burnable_while_mixed();
        let removed = wanted.min(available);
        if removed <= 0.0 {
            return 0.0;
        }
        // Split the draw by mixture ratio so the tanks stay imbalanced the way
        // they really do, and never take more than a tank holds.
        let of = self.of_ratio;
        let fuel_wanted = removed / (1.0 + of);
        let oxidizer_wanted = removed - fuel_wanted;
        let fuel_taken = fuel_wanted.min(self.remaining_kg[1]);
        let oxidizer_taken = oxidizer_wanted.min(self.remaining_kg[0]);
        self.remaining_kg[0] -= oxidizer_taken;
        self.remaining_kg[1] -= fuel_taken;
        let actually_removed = oxidizer_taken + fuel_taken;
        self.burned_kg += actually_removed;
        self.refresh_empty();
        actually_removed
    }

    /// How much the engine can consume before one tank runs dry.
    ///
    /// The limiting fluid sets the answer: with O/F = 5.2 the oxidizer is the
    /// brake, so `m_ox · (1 + O/F)` kilograms of mixture can flow. That is
    /// less than the total still aboard — the lean tank is unreachable — which
    /// is exactly the failure a flameout must report honestly.
    pub fn burnable_while_mixed(&self) -> f64 {
        let of = self.of_ratio;
        // One kilogram of mixture needs 1/(1+O/F) kg of fuel and
        // O/F/(1+O/F) kg of oxidizer, so each tank supports
        // `mass · (1 + O/F)` of mixture straight from its own contents.
        let by_oxidizer = self.remaining_kg[0] * (1.0 + of) / of;
        let by_fuel = self.remaining_kg[1] * (1.0 + of);
        by_oxidizer.min(by_fuel)
    }

    fn refresh_empty(&mut self) {
        self.empty = !self.has_both_fluids();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn burn(force: f64, isp: f64) -> Burn {
        Burn {
            start: 0.0,
            end: 1_000.0,
            force_newtons: [force, 0.0, 0.0],
            specific_impulse_seconds: isp,
        }
    }

    #[test]
    fn a_fresh_pool_holds_exactly_its_load() {
        let p = PropellantPool::new(12_800.0, 5.2).unwrap();
        assert!((p.remaining_kg() - 12_800.0).abs() < 1e-9);
        assert!(p.has_both_fluids());
        assert!(!p.is_empty());
        assert!(PropellantPool::new(0.0, 5.2).is_none());
        assert!(PropellantPool::new(100.0, 0.0).is_none());
        assert!(PropellantPool::new(f64::NAN, 5.2).is_none());
    }

    #[test]
    fn venting_removes_mass_and_is_monotonic() {
        let mut p = PropellantPool::new(1_000.0, 5.2).unwrap();
        let removed = p.set_vented(1, 10.0);
        assert!((removed - 10.0).abs() < 1e-12, "first report removes 10 kg");
        assert!((p.remaining_kg() - 990.0).abs() < 1e-9);
        // A smaller cumulative report is a stale sample, not a refill.
        let again = p.set_vented(1, 4.0);
        assert_eq!(again, 0.0);
        assert!((p.remaining_kg() - 990.0).abs() < 1e-9);
        // The same cumulative report does not double-count.
        assert_eq!(p.set_vented(1, 10.0), 0.0);
        assert!((p.vented_kg()[1] - 10.0).abs() < 1e-12);
    }

    #[test]
    fn a_vent_cannot_release_more_than_the_tank_holds() {
        let mut p = PropellantPool::new(100.0, 5.2).unwrap();
        let removed = p.set_vented(0, 1.0e9);
        let oxidizer = 100.0 / (1.0 + 5.2) * 5.2;
        assert!((removed - oxidizer).abs() < 1e-9, "{removed} vs {oxidizer}");
        assert!((p.remaining_kg() - 100.0 / 6.2).abs() < 1e-9);
        assert!(!p.has_both_fluids(), "a dry oxidizer tank starves the engine");
        assert!(p.is_empty());
    }

    #[test]
    fn a_burn_draws_its_rocket_equation_mass() {
        let mut p = PropellantPool::new(12_800.0, 5.2).unwrap();
        let b = burn(18_000.0, 450.0);
        let flow = b.mass_flow_kg_s();
        assert!((flow - 18_000.0 / (450.0 * G0)).abs() < 1e-12);
        let removed = p.draw_burn(&b, 100.0);
        assert!((removed - flow * 100.0).abs() < 1e-9);
        assert!((p.remaining_kg() - (12_800.0 - removed)).abs() < 1e-9);
    }

    #[test]
    fn a_pool_too_small_to_finish_the_window_reports_what_it_can() {
        // A 1 kg pool cannot feed 1000 s at this flow rate, and only the
        // mixture the limiting tank can reach is burnable at all.
        let mut p = PropellantPool::new(1.0, 5.2).unwrap();
        // A pool that starts on mixture is fully reachable: each tank alone
        // can feed the whole load, so no propellant is stranded yet.
        let reachable = p.burnable_while_mixed();
        assert!((reachable - 1.0).abs() < 1e-12, "{reachable}");
        let removed = p.draw_burn(&burn(18_000.0, 450.0), 1_000.0);
        assert!((removed - 1.0).abs() < 1e-9, "{removed} of a 1 kg pool");
        assert!(p.is_empty());
        // A dry pool gives nothing further.
        assert_eq!(p.draw_burn(&burn(18_000.0, 450.0), 100.0), 0.0);
    }

    #[test]
    fn the_limiting_fluid_sets_the_available_propellant() {
        // Boil off most of the oxidizer: the engine can only burn the fuel
        // the remaining oxidizer can consume, not the fuel alone.
        let mut p = PropellantPool::new(6_200.0, 5.2).unwrap();
        p.set_vented(0, 4_000.0);
        let oxidizer = p.remaining_kg[0];
        let expected = oxidizer * (1.0 + 5.2) / 5.2;
        assert_eq!(p.burnable_while_mixed(), expected);
        let removed = p.draw_burn(&burn(18_000.0, 450.0), 10_000.0);
        assert!((removed - expected).abs() < 1e-6, "{removed} vs {expected}");
        assert!(!p.has_both_fluids());
    }
}

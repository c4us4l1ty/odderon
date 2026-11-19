//! Which physics a signed record was flown with.
//!
//! The Flight Data Recorder is a frozen, versioned wire format. A schema is
//! not a cosmetic label: it selects the *model* a verifier must replay, so a
//! record issued under one model keeps being judged by that model forever.
//! Replaying a v3 certificate with v4's coupled mass would silently rewrite a
//! result the pilot already holds, which is exactly what a signed record
//! exists to prevent.
//!
//! This enum is the single dispatch point between the two. Every crate that
//! takes a schema version turns it into a `PhysicsRevision` here rather than
//! comparing raw integers, so "what changed in v3→v4" has one answer.
//!
//! ```text
//! v1, v2 → no tank model at all (pre-cryogenic schemas)
//! v3     → tanks signed and replayed, but loss is reported only
//! v4     → tanks, fitted dry mass and venting are coupled; catalog scoring
//! v5     → the same flight physics, scored against verified fitted dry mass
//! ```

/// One numbered model revision. Wire ids are the schema number the record
/// carries, not an independent counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsRevision {
    /// Schema 1 and 2. No cryogenic block; boil-off is identically zero.
    PreThermal,
    /// Schema 3. Tanks are integrated along the flown spline and their loss
    /// and vent history are signed and replayed, but the loss never touches
    /// the propagated mass, the burn fuel or the score.
    FrozenUncoupled,
    /// Schema 4. The vented mass leaves the vehicle and the dry mass is fitted
    /// hardware, but scoring retains the catalog dry-mass baseline.
    CoupledFitted,
    /// Schema 5. Flight physics is unchanged from v4; fuel validity and the
    /// remaining-propellant prediction are scored against verified fitted mass.
    FittedScoring,
}

impl PhysicsRevision {
    /// Revision a schema number implies. Unknown numbers are rejected by the
    /// recorder before this is reached; the mapping still returns `None`
    /// rather than guessing.
    pub fn for_schema(schema_version: u16) -> Option<Self> {
        match schema_version {
            0..=2 => Some(Self::PreThermal),
            3 => Some(Self::FrozenUncoupled),
            4 => Some(Self::CoupledFitted),
            5 => Some(Self::FittedScoring),
            _ => None,
        }
    }

    /// True when vented propellant is removed from the flown mass and the
    /// remaining load is what the burns, drag and SRP divide by.
    pub fn couples_vented_mass(self) -> bool {
        matches!(self, Self::CoupledFitted | Self::FittedScoring)
    }

    /// True when score fuel terms use the verified fitted dry mass.
    pub fn scores_fitted_mass(self) -> bool {
        matches!(self, Self::FittedScoring)
    }

    /// True when a tank model is signed at all.
    pub fn has_tanks(self) -> bool {
        !matches!(self, Self::PreThermal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_issued_schema_maps_to_one_revision() {
        assert_eq!(
            PhysicsRevision::for_schema(1),
            Some(PhysicsRevision::PreThermal)
        );
        assert_eq!(
            PhysicsRevision::for_schema(2),
            Some(PhysicsRevision::PreThermal)
        );
        assert_eq!(
            PhysicsRevision::for_schema(3),
            Some(PhysicsRevision::FrozenUncoupled)
        );
        assert_eq!(
            PhysicsRevision::for_schema(4),
            Some(PhysicsRevision::CoupledFitted)
        );
        assert_eq!(
            PhysicsRevision::for_schema(0),
            Some(PhysicsRevision::PreThermal)
        );
        assert_eq!(
            PhysicsRevision::for_schema(5),
            Some(PhysicsRevision::FittedScoring)
        );
        assert_eq!(PhysicsRevision::for_schema(6), None);
    }

    #[test]
    fn only_the_newest_revision_couples_mass() {
        assert!(!PhysicsRevision::PreThermal.couples_vented_mass());
        assert!(!PhysicsRevision::FrozenUncoupled.couples_vented_mass());
        assert!(PhysicsRevision::CoupledFitted.couples_vented_mass());
        assert!(PhysicsRevision::FittedScoring.couples_vented_mass());
        assert!(!PhysicsRevision::CoupledFitted.scores_fitted_mass());
        assert!(PhysicsRevision::FittedScoring.scores_fitted_mass());
        assert!(!PhysicsRevision::PreThermal.has_tanks());
        assert!(PhysicsRevision::FrozenUncoupled.has_tanks());
        assert!(PhysicsRevision::CoupledFitted.has_tanks());
    }
}

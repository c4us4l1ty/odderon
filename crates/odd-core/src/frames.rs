//! Local frame translations and instantaneous kinematic transforms.
//!
//! A star's coarse catalog anchor is kept separate from local meter offsets.
//! Perform nearby calculations in their shared frame rather than subtracting
//! enormous global floating-point positions. This module does not yet provide
//! time-dependent orientation models or ephemerides. A frame tree snapshot
//! uses caller-supplied transforms and split galactic anchors.

/// Coarse ICRS-aligned stellar anchor in integer milliparsecs relative to the
/// Solar System barycenter. This is catalog indexing, not a precise location;
/// one milliparsec is approximately 3.09e13 m. Catalog residuals must be
/// stored separately before using an anchor for astrometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GalacticAnchor {
    pub milliparsecs: [i64; 3],
}

/// An origin in its parent frame, in meters. Both frames have the same axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Translation {
    origin_m: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NonFiniteCoordinate;

impl Translation {
    pub fn new(origin_m: [f64; 3]) -> Result<Self, NonFiniteCoordinate> {
        if origin_m.iter().all(|x| x.is_finite()) {
            Ok(Self { origin_m })
        } else {
            Err(NonFiniteCoordinate)
        }
    }

    /// Convert a child-relative position to its parent frame in meters.
    pub fn to_parent(self, child_m: [f64; 3]) -> Result<[f64; 3], NonFiniteCoordinate> {
        self.apply(child_m, false)
    }

    /// Convert a parent-relative position to this frame in meters.
    pub fn from_parent(self, parent_m: [f64; 3]) -> Result<[f64; 3], NonFiniteCoordinate> {
        self.apply(parent_m, true)
    }

    fn apply(self, position: [f64; 3], subtract: bool) -> Result<[f64; 3], NonFiniteCoordinate> {
        let mut result = [0.0; 3];
        for i in 0..3 {
            result[i] = if subtract {
                position[i] - self.origin_m[i]
            } else {
                position[i] + self.origin_m[i]
            };
        }
        if result.iter().all(|x| x.is_finite()) {
            Ok(result)
        } else {
            Err(NonFiniteCoordinate)
        }
    }
}

/// Convert a camera-relative simulation position to a lossy f32 render position.
/// Never use this output to reconstruct simulation coordinates.
pub fn render_position(camera_relative_m: [f64; 3]) -> Result<[f32; 3], NonFiniteCoordinate> {
    let result = camera_relative_m.map(|x| x as f32);
    if result.iter().all(|x| x.is_finite()) {
        Ok(result)
    } else {
        Err(NonFiniteCoordinate)
    }
}

/// A right-handed, orthonormal rotation from child axes to parent axes.
///
/// Rows are in parent-axis order and columns are child basis vectors expressed
/// in parent coordinates. Matrices are checked to within 1e-12, not silently
/// corrected; callers must provide the rotation appropriate to the epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    matrix: [[f64; 3]; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    NonFinite,
    InvalidRotation,
}

impl Rotation {
    pub const IDENTITY: Self = Self {
        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    /// Rejects non-finite matrices, scaling, shear, and reflections.
    pub fn new(matrix: [[f64; 3]; 3]) -> Result<Self, FrameError> {
        if !matrix.iter().flatten().all(|x| x.is_finite()) {
            return Err(FrameError::NonFinite);
        }
        const TOLERANCE: f64 = 1e-12;
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                let dot = (0..3).map(|k| matrix[k][i] * matrix[k][j]).sum::<f64>();
                if !dot.is_finite() || (dot - expected).abs() > TOLERANCE {
                    return Err(FrameError::InvalidRotation);
                }
            }
        }
        let [a, b, c] = matrix;
        let determinant = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
        if (determinant - 1.0).abs() > TOLERANCE {
            return Err(FrameError::InvalidRotation);
        }
        Ok(Self { matrix })
    }

    fn apply(self, vector: [f64; 3]) -> [f64; 3] {
        self.matrix.map(|row| dot(row, vector))
    }

    fn apply_inverse(self, vector: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| {
            self.matrix[0][i] * vector[0]
                + self.matrix[1][i] * vector[1]
                + self.matrix[2][i] * vector[2]
        })
    }
}

/// Position in meters and coordinate velocity in meters per second, in one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub position_m: [f64; 3],
    pub velocity_m_s: [f64; 3],
}

/// A child frame's instantaneous pose and motion relative to its parent.
///
/// Origin, origin velocity, and angular velocity are expressed in parent axes;
/// `rotation` maps child vectors into parent axes. The parent may itself rotate:
/// velocities are coordinate derivatives in the respective frames. The caller
/// must provide all inputs evaluated at the *same epoch*; this is a snapshot,
/// not an Earth orientation model or an ephemeris.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KinematicTransform {
    origin_m: [f64; 3],
    origin_velocity_m_s: [f64; 3],
    rotation: Rotation,
    angular_velocity_rad_s: [f64; 3],
}

impl KinematicTransform {
    pub fn new(
        origin_m: [f64; 3],
        origin_velocity_m_s: [f64; 3],
        rotation: Rotation,
        angular_velocity_rad_s: [f64; 3],
    ) -> Result<Self, FrameError> {
        if !origin_m
            .iter()
            .chain(&origin_velocity_m_s)
            .chain(&angular_velocity_rad_s)
            .all(|x| x.is_finite())
        {
            return Err(FrameError::NonFinite);
        }
        Ok(Self {
            origin_m,
            origin_velocity_m_s,
            rotation,
            angular_velocity_rad_s,
        })
    }

    /// `r_parent = origin + R r_child`; `v_parent = origin_dot + R v_child + ω × (R r_child)`.
    pub fn to_parent(self, state: State) -> Result<State, FrameError> {
        check_state(state)?;
        let relative_position = self.rotation.apply(state.position_m);
        let relative_velocity = self.rotation.apply(state.velocity_m_s);
        let spin_velocity = cross(self.angular_velocity_rad_s, relative_position);
        let result = State {
            position_m: add(self.origin_m, relative_position),
            velocity_m_s: add(
                self.origin_velocity_m_s,
                add(relative_velocity, spin_velocity),
            ),
        };
        check_state(result)?;
        Ok(result)
    }

    /// Inverse of `to_parent`, including the rotation-induced velocity term.
    pub fn from_parent(self, state: State) -> Result<State, FrameError> {
        check_state(state)?;
        let relative_position = sub(state.position_m, self.origin_m);
        let relative_velocity = sub(
            sub(state.velocity_m_s, self.origin_velocity_m_s),
            cross(self.angular_velocity_rad_s, relative_position),
        );
        let result = State {
            position_m: self.rotation.apply_inverse(relative_position),
            velocity_m_s: self.rotation.apply_inverse(relative_velocity),
        };
        check_state(result)?;
        Ok(result)
    }
}

fn check_state(state: State) -> Result<(), FrameError> {
    if state
        .position_m
        .iter()
        .chain(&state.velocity_m_s)
        .all(|x| x.is_finite())
    {
        Ok(())
    } else {
        Err(FrameError::NonFinite)
    }
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Identifiers in a snapshot frame tree. IDs are application-assigned and
/// scoped to their variant; the galactic origin is the Solar System barycenter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrameId {
    Galactic,
    StellarSystem(u64),
    BodyCentered(u64),
    BodyFixed(u64),
    Camera(u64),
}

/// A frame-qualified state, with TDB epoch to prevent mixing snapshots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramedState {
    pub frame: FrameId,
    pub epoch: crate::epoch::Epoch,
    pub state: State,
}

/// A galactic state retains a coarse anchor plus a local SI-meter residual.
/// It is not a single enormous f64 position, so nearby offsets are preserved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GalacticState {
    pub anchor: GalacticAnchor,
    pub epoch: crate::epoch::Epoch,
    /// Low-order correction to state.position_m, in meters, in galactic axes.
    /// Both terms are required when transferring this state back to a local frame.
    pub position_correction_m: [f64; 3],
    pub state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeError {
    UnknownFrame,
    DuplicateFrame,
    InvalidParent,
    GalacticNeedsSplitState,
    EpochMismatch,
    NonFinite,
}

#[derive(Clone, Copy, Debug)]
struct FrameNode {
    parent: FrameId,
    pose: KinematicTransform,
    anchor: Option<GalacticAnchor>,
}

/// An immutable-epoch snapshot of a frame hierarchy. Pose data must be
/// supplied by a future ephemeris/orientation model at exactly this epoch.
/// The parent must already exist when a node is added, so cycles are impossible.
#[derive(Clone, Debug)]
pub struct FrameTree {
    epoch: crate::epoch::Epoch,
    nodes: std::collections::BTreeMap<FrameId, FrameNode>,
}

impl FrameTree {
    pub fn new(epoch: crate::epoch::Epoch) -> Self {
        Self {
            epoch,
            nodes: std::collections::BTreeMap::new(),
        }
    }

    /// Add a stellar frame relative to a fixed coarse galactic anchor. Its
    /// pose's origin is the residual in meters, not the global position.
    pub fn add_stellar_system(
        &mut self,
        star: u64,
        anchor: GalacticAnchor,
        pose: KinematicTransform,
    ) -> Result<(), TreeError> {
        self.insert(
            FrameId::StellarSystem(star),
            FrameNode {
                parent: FrameId::Galactic,
                pose,
                anchor: Some(anchor),
            },
        )
    }

    /// Add a non-galactic, non-stellar child to an existing local frame.
    pub fn add_frame(
        &mut self,
        id: FrameId,
        parent: FrameId,
        pose: KinematicTransform,
    ) -> Result<(), TreeError> {
        if matches!(id, FrameId::Galactic | FrameId::StellarSystem(_))
            || parent == FrameId::Galactic
        {
            return Err(TreeError::InvalidParent);
        }
        if !self.nodes.contains_key(&parent) {
            return Err(TreeError::UnknownFrame);
        }
        self.insert(
            id,
            FrameNode {
                parent,
                pose,
                anchor: None,
            },
        )
    }

    fn insert(&mut self, id: FrameId, node: FrameNode) -> Result<(), TreeError> {
        if self.nodes.contains_key(&id) {
            return Err(TreeError::DuplicateFrame);
        }
        self.nodes.insert(id, node);
        Ok(())
    }

    fn path(&self, mut id: FrameId) -> Result<Vec<FrameId>, TreeError> {
        if id == FrameId::Galactic {
            return Err(TreeError::GalacticNeedsSplitState);
        }
        let mut path = Vec::new();
        loop {
            let node = self.nodes.get(&id).ok_or(TreeError::UnknownFrame)?;
            path.push(id);
            if node.parent == FrameId::Galactic {
                break;
            }
            id = node.parent;
        }
        Ok(path)
    }

    fn check_epoch(&self, epoch: crate::epoch::Epoch) -> Result<(), TreeError> {
        if self.epoch == epoch {
            Ok(())
        } else {
            Err(TreeError::EpochMismatch)
        }
    }

    /// Transform local frames through their lowest common ancestor. Across
    /// star systems, a coarse-anchor subtraction is required and precision is
    /// limited by f64 at the separation scale. Galactic states use the split
    /// representation and cannot be returned through this local-state API.
    pub fn transform(&self, source: FramedState, to: FrameId) -> Result<FramedState, TreeError> {
        self.check_epoch(source.epoch)?;
        let from_path = self.path(source.frame)?;
        let to_path = self.path(to)?;
        let common = from_path.iter().position(|id| to_path.contains(id));
        if let Some(from_end) = common {
            let ancestor = from_path[from_end];
            let to_end = to_path
                .iter()
                .position(|id| *id == ancestor)
                .ok_or(TreeError::UnknownFrame)?;
            let mut state = PreciseState::from_state(source.state)?;
            for id in &from_path[..from_end] {
                state = state.to_parent(self.nodes[id].pose)?;
            }
            for id in to_path[..to_end].iter().rev() {
                state = state.into_child(self.nodes[id].pose)?;
            }
            Ok(FramedState {
                frame: to,
                epoch: self.epoch,
                state: state.rounded()?,
            })
        } else {
            self.from_galactic(self.to_galactic(source)?, to)
        }
    }

    /// Transform to galactic axes while retaining the stellar anchor separately.
    pub fn to_galactic(&self, source: FramedState) -> Result<GalacticState, TreeError> {
        self.check_epoch(source.epoch)?;
        let path = self.path(source.frame)?;
        let mut state = PreciseState::from_state(source.state)?;
        for id in &path {
            state = state.to_parent(self.nodes[id].pose)?;
        }
        let anchor = self.nodes[path.last().ok_or(TreeError::UnknownFrame)?]
            .anchor
            .ok_or(TreeError::InvalidParent)?;
        Ok(GalacticState {
            anchor,
            epoch: self.epoch,
            position_correction_m: state.position.low,
            state: State {
                position_m: state.position.high,
                velocity_m_s: state.velocity,
            },
        })
    }

    /// Transform from a split galactic state to a local frame. If the anchors
    /// differ, the offset conversion is necessarily rounded to f64 meters.
    pub fn from_galactic(
        &self,
        source: GalacticState,
        to: FrameId,
    ) -> Result<FramedState, TreeError> {
        self.check_epoch(source.epoch)?;
        let path = self.path(to)?;
        let anchor = self.nodes[path.last().ok_or(TreeError::UnknownFrame)?]
            .anchor
            .ok_or(TreeError::InvalidParent)?;
        let mut state = PreciseState::from_state(source.state)?;
        if !source.position_correction_m.iter().all(|v| v.is_finite()) {
            return Err(TreeError::NonFinite);
        }
        for i in 0..3 {
            state
                .position
                .add_component(i, source.position_correction_m[i])?;
            let delta =
                i128::from(source.anchor.milliparsecs[i]) - i128::from(anchor.milliparsecs[i]);
            state
                .position
                .add_component(i, (delta as f64) * (crate::consts::PARSEC_M / 1000.0))?;
        }
        for id in path.iter().rev() {
            state = state.into_child(self.nodes[id].pose)?;
        }
        Ok(FramedState {
            frame: to,
            epoch: self.epoch,
            state: state.rounded()?,
        })
    }
}

// Two-term compensated positions preserve small translations through large
// frame origins. Products in rotations are still rounded f64 operations; this
// is not arbitrary precision, and public f64 states remain magnitude-limited.
#[derive(Clone, Copy)]
struct SplitPosition {
    high: [f64; 3],
    low: [f64; 3],
}

impl SplitPosition {
    fn from_high(high: [f64; 3]) -> Self {
        Self {
            high,
            low: [0.0; 3],
        }
    }

    // Knuth TwoSum for finite, non-overflowing addition, followed by
    // renormalization of the two-term expansion. See Ogita, Rump & Oishi
    // (2005), Algorithm 3.1: https://www.tuhh.de/ti3/paper/rump/OgRuOi05.pdf
    fn add_component(&mut self, i: usize, value: f64) -> Result<(), TreeError> {
        let (sum, error) = two_sum(self.high[i], value);
        let (high, low) = two_sum(sum, self.low[i] + error);
        if !high.is_finite() || !low.is_finite() {
            return Err(TreeError::NonFinite);
        }
        self.high[i] = high;
        self.low[i] = low;
        Ok(())
    }

    fn translate(mut self, vector: [f64; 3], sign: f64) -> Result<Self, TreeError> {
        for (i, component) in vector.iter().enumerate() {
            self.add_component(i, sign * component)?;
        }
        Ok(self)
    }

    fn rotate(self, rotation: Rotation, transpose: bool) -> Result<Self, TreeError> {
        let mut result = Self::from_high([0.0; 3]);
        for i in 0..3 {
            for j in 0..3 {
                let coefficient = if transpose {
                    rotation.matrix[j][i]
                } else {
                    rotation.matrix[i][j]
                };
                result.add_component(i, coefficient * self.high[j])?;
                result.add_component(i, coefficient * self.low[j])?;
            }
        }
        Ok(result)
    }

    fn rounded(self) -> [f64; 3] {
        std::array::from_fn(|i| self.high[i] + self.low[i])
    }
}

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let b_virtual = sum - a;
    let error = (a - (sum - b_virtual)) + (b - b_virtual);
    (sum, error)
}

#[derive(Clone, Copy)]
struct PreciseState {
    position: SplitPosition,
    velocity: [f64; 3],
}

impl PreciseState {
    fn from_state(state: State) -> Result<Self, TreeError> {
        check_state(state).map_err(|_| TreeError::NonFinite)?;
        Ok(Self {
            position: SplitPosition::from_high(state.position_m),
            velocity: state.velocity_m_s,
        })
    }

    fn rounded(self) -> Result<State, TreeError> {
        let state = State {
            position_m: self.position.rounded(),
            velocity_m_s: self.velocity,
        };
        check_state(state).map_err(|_| TreeError::NonFinite)?;
        Ok(state)
    }

    fn to_parent(self, pose: KinematicTransform) -> Result<Self, TreeError> {
        let rotated = self.position.rotate(pose.rotation, false)?;
        let velocity = add(
            pose.origin_velocity_m_s,
            add(
                pose.rotation.apply(self.velocity),
                cross(pose.angular_velocity_rad_s, rotated.rounded()),
            ),
        );
        let result = Self {
            position: rotated.translate(pose.origin_m, 1.0)?,
            velocity,
        };
        result.rounded()?;
        Ok(result)
    }

    fn into_child(self, pose: KinematicTransform) -> Result<Self, TreeError> {
        let relative = self.position.translate(pose.origin_m, -1.0)?;
        let velocity = pose.rotation.apply_inverse(sub(
            sub(self.velocity, pose.origin_velocity_m_s),
            cross(pose.angular_velocity_rad_s, relative.rounded()),
        ));
        let result = Self {
            position: relative.rotate(pose.rotation, true)?,
            velocity,
        };
        result.rounded()?;
        Ok(result)
    }
}

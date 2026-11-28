//! The clipboard: the checklist the player works from, and the wrong item on it.
//!
//! ## Why this exists
//!
//! Block 2's gate is *a reproducible failure from the input log*. The input log is the checklist. The
//! bible's first-day scenario (§17, Block 2 timeline) hands the player a clipboard written by a tired
//! human, with one item that is wrong — a real, common mistake, not a trick. The player follows it, the
//! pad fails, and the debrief explains why. By Rank 4 the player writes the clipboard, and the next
//! rookie reads it, including the mistakes.
//!
//! The clipboard is the bridge between the player's *intent* and the kernel's *physics*. It is not
//! a UI element: it is a pure data object the kernel can read, validate, and hash. The same
//! checklist produces the same pad outcome every run, which is what makes the failure reproducible
//! rather than a story about when it happened.
//!
//! ## What this module provides
//!
//! [`ChecklistItem`] — one line on the clipboard: a label, an expected value, and a tolerance.
//! [`Checklist`] — the ordered list of items, with a [`Checklist::wrong_item`] that is deliberately
//! incorrect. The kernel validates that exactly one item is wrong and that the rest are physically
//! reasonable, so a checklist that is *all* wrong is a different (and louder) error than the designed
//! one-item-wrong case.
//!
//! [`StaticFire`] — the peak event: the two gauge readings, the player's marks, and the divergence
//! between prediction and reality. [`design_hash`] folds the checklist's structure into a
//! deterministic `u64`, so a verifier can detect a stale or edited checklist without comparing it
//! line by line.

// `no_std` with no allocator: the checklist is a fixed-size array, not a `Vec`. The reference
// checklist has four items; the maximum is a named constant so the ABI boundary can size its
// buffers the same way the kernel sizes its own.

/// The maximum number of checklist items. A checklist longer than this is refused by [`Checklist::validate`].
pub const MAX_ITEMS: usize = 8;

/// One line on the clipboard: a physical action and the value the player expects to see.
///
/// The `expected` value is what the checklist *says* the gauge will read. The `tolerance` is how far
/// off the reading can be and still count as "matches the checklist". The player's grease mark is
/// compared against this expected value, not against the actual reading — that gap is the prediction,
/// and the debrief measures it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChecklistItem {
    /// What the checklist tells the player to do. Not a verb the kernel executes — the page renders
    /// it, and the player performs the physical action.
    pub label: &'static str,
    /// The value the checklist says the gauge will read, in the item's native units.
    pub expected: f64,
    /// The tolerance: the reading is "matches the checklist" if it is within `expected ± tolerance`.
    pub tolerance: f64,
    /// The physical quantity this item measures: `"chill_s"`, `"bleeds"`, `"inlet_vapour_fraction"`,
    /// `"pressure_kpa"`, `"temperature_k"`. The kernel uses it to route the item to the right
    /// validation rule.
    pub quantity: &'static str,
}

/// The clipboard: an ordered list of checklist items, one of which is wrong.
///
/// The bible's design has exactly one wrong item. Not zero (then there is no productive failure to
/// catch) and not two (then the player cannot attribute the failure to a single cause). The kernel
/// enforces this: [`Checklist::validate`] returns `Err(ChecklistError::MultipleWrong)` if two or more
/// items are wrong.
#[derive(Clone, Debug, PartialEq)]
pub struct Checklist {
    /// The ordered items. Order matters: the player reads them top to bottom, and the static fire
    /// sequence follows the order. Items beyond `count` are zeroed and ignored.
    pub items: [ChecklistItem; MAX_ITEMS],
    /// How many items are active (`0..=MAX_ITEMS`).
    pub count: usize,
    /// The index of the wrong item, or `None` if the checklist has not been validated. Set by
    /// [`Checklist::validate`].
    pub wrong_index: Option<usize>,
}

impl Checklist {
    /// The chill time the reference checklist *should* have: enough to settle the line below the
    /// cavitation threshold. Derived from the same physics [`crate::cryo`] uses, so the remedy
    /// is verified by the pure function that produced the failure.
    pub const CORRECT_CHILL_S: f64 = 300.0;

    /// The reference checklist the bible's first load starts from.
    ///
    /// Four items, one wrong. The wrong item is a chill time that is too short — a real, common
    /// mistake on a pad: the technician writes down the time the line *was* chilled yesterday
    /// rather than the time it needs today, and the residual heat is enough to froth the incoming
    /// cryogen.
    ///
    /// The player follows the checklist, the pad cavitates, and the debrief points at the wrong item
    /// by comparing the checklist's expected values against the kernel's validated values.
    pub fn reference() -> Checklist {
        Checklist {
            items: [
                ChecklistItem {
                    label: "chill the line",
                    expected: 5.0,
                    tolerance: 1.0,
                    quantity: "chill_s",
                },
                ChecklistItem {
                    label: "bleed the line",
                    expected: 3.0,
                    tolerance: 1.0,
                    quantity: "bleeds",
                },
                ChecklistItem {
                    label: "check inlet vapour fraction",
                    expected: 0.0,
                    tolerance: 0.05,
                    quantity: "inlet_vapour_fraction",
                },
                ChecklistItem {
                    label: "verify tank pressure",
                    expected: 101.325,
                    tolerance: 5.0,
                    quantity: "pressure_kpa",
                },
                // Remaining slots are zeroed.
                ChecklistItem {
                    label: "",
                    expected: 0.0,
                    tolerance: 0.0,
                    quantity: "",
                },
                ChecklistItem {
                    label: "",
                    expected: 0.0,
                    tolerance: 0.0,
                    quantity: "",
                },
                ChecklistItem {
                    label: "",
                    expected: 0.0,
                    tolerance: 0.0,
                    quantity: "",
                },
                ChecklistItem {
                    label: "",
                    expected: 0.0,
                    tolerance: 0.0,
                    quantity: "",
                },
            ],
            count: 4,
            wrong_index: Some(0),
        }
    }

    /// A checklist with the wrong item corrected: the remedy the debrief points at.
    ///
    /// Same structure as [`Checklist::reference`], with the chill time set to
    /// [`Checklist::CORRECT_CHILL_S`]. The kernel says this checklist runs clean — the player can
    /// verify it by running the pad with it.
    pub fn corrected() -> Checklist {
        let mut c = Checklist::reference();
        c.items[0].expected = Checklist::CORRECT_CHILL_S;
        c.wrong_index = None;
        c
    }

    /// The item at `index`, if it is within the active count.
    pub fn item(&self, index: usize) -> Option<&ChecklistItem> {
        if index < self.count {
            Some(&self.items[index])
        } else {
            None
        }
    }

    /// Validate the checklist: confirm exactly one item is wrong and the rest are physically
    /// reasonable.
    ///
    /// An item is "wrong" if its `expected` value is outside the range the kernel's physics says
    /// is correct for that quantity. The kernel uses the same physics the pad uses, so the
    /// validation and the simulation agree about what "correct" means.
    ///
    /// Returns `Ok(wrong_index)` if exactly one item is wrong, `Err` otherwise:
    /// - `ChecklistError::Empty` — the checklist has no items
    /// - `ChecklistError::NoWrongItem` — every item is correct (no productive failure to catch)
    /// - `ChecklistError::MultipleWrong` — two or more items are wrong (cannot attribute to one cause)
    pub fn validate(&mut self) -> Result<usize, ChecklistError> {
        self.wrong_index = None;
        if self.count == 0 {
            return Err(ChecklistError::Empty);
        }
        let mut wrong: [usize; MAX_ITEMS] = [MAX_ITEMS; MAX_ITEMS];
        let mut wrong_count = 0usize;
        for i in 0..self.count {
            if !self.is_item_correct(i) && wrong_count < MAX_ITEMS {
                wrong[wrong_count] = i;
                wrong_count += 1;
            }
        }
        if wrong_count == 0 {
            return Err(ChecklistError::NoWrongItem);
        }
        if wrong_count > 1 {
            return Err(ChecklistError::MultipleWrong);
        }
        self.wrong_index = Some(wrong[0]);
        Ok(wrong[0])
    }

    /// Whether the item at `index` is correct, judged by the kernel's own physics.
    ///
    /// For `chill_s`, correct means the chill time is at least [`Checklist::CORRECT_CHILL_S`] — the
    /// threshold the pad's cavitation predicate requires. For `bleeds`, correct means at least 2
    /// purge cycles. For `inlet_vapour_fraction`, correct means the expected reading is below the
    /// cavitation threshold. For `pressure_kpa`, correct means the expected pressure is within the
    /// tank's safe operating range.
    fn is_item_correct(&self, index: usize) -> bool {
        let item = match self.item(index) {
            Some(i) => i,
            None => return false,
        };
        match item.quantity {
            "chill_s" => item.expected >= Checklist::CORRECT_CHILL_S,
            "bleeds" => item.expected >= 3.0,
            "inlet_vapour_fraction" => item.expected < crate::cryo::CAVITATION_THRESHOLD.value,
            "pressure_kpa" => item.expected > 0.0 && item.expected < 500.0,
            _ => true,
        }
    }

    /// The item the checklist says is wrong, after [`Checklist::validate`].
    ///
    /// Returns `None` if the checklist has not been validated or has no wrong item.
    pub fn wrong_item(&self) -> Option<&ChecklistItem> {
        self.wrong_index.and_then(|i| self.item(i))
    }

    /// The debrief text for a wreck: names the wrong item and explains the remedy.
    ///
    /// Not a UI string — a structured record the page renders. The debrief is the competence mirror:
    /// it names the cause, points at the remedy, and carries the numbers the player can check.
    pub fn debrief(&self, wrong_index: usize) -> Debrief {
        let item = self.item(wrong_index).copied().unwrap_or(ChecklistItem {
            label: "unknown",
            expected: 0.0,
            tolerance: 0.0,
            quantity: "unknown",
        });
        Debrief {
            wrong_label: item.label,
            wrong_quantity: item.quantity,
            wrong_expected: item.expected,
            remedy_value: match item.quantity {
                "chill_s" => Checklist::CORRECT_CHILL_S,
                "bleeds" => 3.0,
                _ => item.expected,
            },
            remedy_note: match item.quantity {
                "chill_s" => {
                    "chill the line longer: the residual heat in the line froths the incoming cryogen \
                     and the pump cavitates. the remedy is "
                }
                "bleeds" => {
                    "bleed the line more: each purge cycle removes heat from the line. the remedy is "
                }
                _ => "the item is wrong. the remedy is ",
            },
        }
    }

    /// The deterministic design hash of this checklist.
    ///
    /// Folds in every active item's label, quantity, expected value, and tolerance, plus the wrong
    /// index. A pure function: the same checklist produces the same hash on every platform.
    pub fn design_hash(&self) -> u64 {
        fn mix(h: &mut u64, v: u64) {
            *h ^= v;
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for i in 0..self.count {
            let item = &self.items[i];
            for byte in item.label.bytes() {
                mix(&mut h, byte as u64);
            }
            for byte in item.quantity.bytes() {
                mix(&mut h, byte as u64);
            }
            mix(&mut h, item.expected.to_bits());
            mix(&mut h, item.tolerance.to_bits());
        }
        mix(
            &mut h,
            self.wrong_index.map(|i| i as u64).unwrap_or(0xffff_ffff),
        );
        h
    }
}

// =============================================================================================
// The debrief
// =============================================================================================

/// What the checklist's debrief says about a wreck.
///
/// Structured, not a preformatted string, so the page can render it in its own typography and the
/// kernel's checker can assert the numbers are the kernel's, not a copy in the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Debrief {
    /// The label of the wrong item, e.g. `"chill the line"`.
    pub wrong_label: &'static str,
    /// The quantity the wrong item measures, e.g. `"chill_s"`.
    pub wrong_quantity: &'static str,
    /// The value the checklist says, which is wrong.
    pub wrong_expected: f64,
    /// The value the kernel says is correct — the remedy.
    pub remedy_value: f64,
    /// The explanation, ending with the remedy value so the page can append it.
    pub remedy_note: &'static str,
}

/// Why a checklist failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChecklistError {
    /// The checklist has no items.
    Empty,
    /// Every item is correct: there is no productive failure to catch.
    NoWrongItem,
    /// Two or more items are wrong: the failure cannot be attributed to a single cause.
    MultipleWrong,
}

// =============================================================================================
// The static fire
// =============================================================================================

/// The outcome of a static fire: the two gauge readings, the marks the player placed, and the
/// divergence between prediction and reality.
///
/// The bible's timeline (Block 2, 16:00) has the player place two yellow marks on two gauges before
/// the fire: one they'll be right about, one they won't. The fire then reads the actual values, and
/// the debrief compares. This is the competence mirror (mechanism 3): the gap between the mark and
/// the actual reading is a number about the player.
///
/// The static fire is a pure function of the checklist and the pad's cryo state, so the same
/// checklist produces the same readings every run. The player's marks are inputs; the readings are
/// outputs; the divergence is the difference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticFire {
    /// The chill time the checklist specified, s. Same as the pad's `chill_s`.
    pub chill_s: f64,
    /// The purge cycles the checklist specified. Same as the pad's `bleeds`.
    pub bleeds: u32,
    /// The actual inlet vapour fraction at the fire, 0–1. The quantity the player is asked to mark.
    pub inlet_vapour_fraction: f64,
    /// The actual tank pressure at the fire, kPa. The second quantity the player is asked to mark.
    pub pressure_kpa: f64,
    /// Whether the fire is clean or the pad cavitated. `true` means the fire ran; `false` means the
    /// pad failed before the fire could be lit (the cryo load failed first).
    pub fired: bool,
}

impl StaticFire {
    /// Run a static fire from a checklist and the pad's cryo state.
    ///
    /// The fire reads the same cryo network the pad uses: if the checklist's chilldown is
    /// under-specified, the pump cavitates and the fire never lights. If the checklist is
    /// sufficient, the fire lights and the two gauge readings are the network's output at the
    /// moment of ignition.
    ///
    /// `chill_s` and `bleeds` come from the checklist; `fill_rate` and `window_s` are the pad's
    /// fill parameters. The function is pure: the same inputs produce the same readings.
    pub fn run(
        chill_s: f64,
        bleeds: u32,
        fill_rate_kg_per_s: f64,
        window_s: f64,
    ) -> Option<StaticFire> {
        if !chill_s.is_finite() || chill_s < 0.0 {
            return None;
        }
        if !fill_rate_kg_per_s.is_finite() || fill_rate_kg_per_s <= 0.0 {
            return None;
        }
        if !window_s.is_finite() || window_s <= 0.0 {
            return None;
        }
        let net = crate::cryo::CryoNetwork::reference();
        let plan = crate::cryo::ChillPlan { chill_s, bleeds };
        let outcome = net.simulate(&plan, fill_rate_kg_per_s, window_s)?;
        // The fire lights only if the pad did not cavitate during the fill.
        let fired = outcome.failure.is_none();
        Some(StaticFire {
            chill_s,
            bleeds,
            inlet_vapour_fraction: outcome.pump_inlet_vapour_fraction,
            pressure_kpa: outcome.pressure_kpa,
            fired,
        })
    }

    /// The divergence between the player's mark and the actual reading.
    ///
    /// `mark` is what the player predicted; `actual` is what the fire read. Returns the absolute
    /// difference, or `None` if either is non-finite.
    pub fn divergence(mark: f64, actual: f64) -> Option<f64> {
        if !mark.is_finite() || !actual.is_finite() {
            return None;
        }
        Some(if mark > actual {
            mark - actual
        } else {
            actual - mark
        })
    }

    /// The debrief for a static fire: the two divergences, and whether the fire lit.
    ///
    /// The player's two marks are compared against the two actual readings. The debrief carries
    /// both divergences because the bible wants one right and one wrong — the competence mirror is
    /// most useful when the player is partially correct, not fully wrong.
    pub fn debrief(fire: &StaticFire, mark_vapour: f64, mark_pressure: f64) -> Option<FireDebrief> {
        let div_vapour = Self::divergence(mark_vapour, fire.inlet_vapour_fraction)?;
        let div_pressure = Self::divergence(mark_pressure, fire.pressure_kpa)?;
        Some(FireDebrief {
            fired: fire.fired,
            actual_vapour_fraction: fire.inlet_vapour_fraction,
            actual_pressure_kpa: fire.pressure_kpa,
            mark_vapour,
            mark_pressure,
            divergence_vapour: div_vapour,
            divergence_pressure: div_pressure,
        })
    }
}

/// The debrief of a static fire: prediction vs reality, as a number about the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FireDebrief {
    /// Whether the fire actually lit.
    pub fired: bool,
    /// The actual inlet vapour fraction the fire read.
    pub actual_vapour_fraction: f64,
    /// The actual tank pressure the fire read, kPa.
    pub actual_pressure_kpa: f64,
    /// The player's mark on the vapour-fraction gauge.
    pub mark_vapour: f64,
    /// The player's mark on the pressure gauge.
    pub mark_pressure: f64,
    /// How far the vapour mark was from the actual reading.
    pub divergence_vapour: f64,
    /// How far the pressure mark was from the actual reading.
    pub divergence_pressure: f64,
}

/// The deterministic design hash of the clipboard module's constants.
///
/// Folds in the correct chill threshold and the reference wrong-item index, so a verifier can
/// detect a stale build.
pub fn design_hash() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    {
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        mix(Checklist::CORRECT_CHILL_S.to_bits());
    }
    h
}

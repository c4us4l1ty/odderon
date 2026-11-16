//! Print-ready technical memorandum.
//!
//! Deterministic by construction: no wall-clock timestamps, no random ids,
//! no floating formatting that depends on locale, and the QR matrix is a
//! pure function of the signed payload. Two runs over the same flight emit
//! byte-identical HTML and SVG.
//!
//! The document is self-contained (inline CSS and inline SVG) so it prints
//! to PDF from any browser without fetching an asset, and every figure is
//! vector so a printed trajectory stays measurable.
use crate::fdr::{AttemptMode, Fdr};
use crate::json::escape_html;
use crate::metrics::{coalition_label, name, unit, PLAYERS};
use crate::qr::Qr;

/// One commanded burn as executed, for the maneuver schedule.
#[derive(Clone, Debug)]
pub struct BurnRow {
    pub start_s: f64,
    pub duration_s: f64,
    pub delta_v_tnw: [f64; 3],
    /// Exact swept central angle for this burn alone, rad. NaN when the
    /// isolated measurement is unavailable.
    pub sweep_rad: f64,
}

/// Flight geometry the recorder does not carry: the two trajectories the
/// figure compares, in the planner's plane projection, in metres.
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    pub level1: Vec<[f64; 2]>,
    pub level3: Vec<[f64; 2]>,
    pub burns: Vec<BurnRow>,
    pub earth_radius_m: f64,
}

fn number(value: f64, decimals: usize) -> String {
    if value.is_finite() {
        format!("{value:.decimals$}")
    } else {
        "&#8212;".to_string()
    }
}

fn signed(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "&#8212;".to_string();
    }
    if value >= 0.0 {
        format!("+{value:.decimals$}")
    } else {
        format!("{value:.decimals$}")
    }
}

/// Receipt material that the caller has already authenticated against its
/// configured server key and bound to this FDR. Kept independent of the
/// authority crate so reporting does not depend on the server implementation.
#[derive(Clone, Debug)]
pub struct ReceiptAnnex {
    pub server_public_key: [u8; 32],
    /// Canonical payload followed by the 64-byte server signature.
    pub receipt_bytes: Vec<u8>,
    pub attempt_num: u32,
    pub mode: AttemptMode,
    pub score_s: f64,
    pub elo_before: f64,
    pub elo_after: f64,
    pub mission_elo_after: f64,
    pub timestamp_s: i64,
}

/// Section 2 figure: Level-1 prediction against the Level-3 flight.
pub fn trajectory_svg(geometry: &Geometry) -> String {
    const SIZE: f64 = 640.0;
    const PAD: f64 = 44.0;
    let mut extent: f64 = geometry.earth_radius_m.max(1.0);
    for point in geometry.level1.iter().chain(geometry.level3.iter()) {
        extent = extent.max(point[0].abs()).max(point[1].abs());
    }
    let extent = extent * 1.08;
    let scale = (SIZE / 2.0 - PAD) / extent;
    let project = |point: &[f64; 2]| (SIZE / 2.0 + point[0] * scale, SIZE / 2.0 - point[1] * scale);
    let path = |points: &[[f64; 2]]| {
        let mut out = String::new();
        for (index, point) in points.iter().enumerate() {
            let (x, y) = project(point);
            out.push_str(&format!(
                "{}{x:.3} {y:.3}",
                if index == 0 { 'M' } else { 'L' }
            ));
        }
        out
    };
    let mut out = String::new();
    out.push_str(&format!(
        "<svg id=\"trajectory-figure\" class=\"figure\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {SIZE} {SIZE}\" width=\"640\" height=\"640\" role=\"img\" aria-label=\"Level-1 predicted trajectory compared with the Level-3 integrated flight\">"
    ));
    out.push_str(&format!(
        "<rect width=\"{SIZE}\" height=\"{SIZE}\" fill=\"#ffffff\"/>"
    ));
    // Reference grid in units of Earth radii keeps the figure measurable.
    let radii = (extent / geometry.earth_radius_m.max(1.0)).ceil() as usize;
    for step in 1..=radii {
        let r = geometry.earth_radius_m * step as f64 * scale;
        if r > SIZE / 2.0 - PAD + 1.0 {
            break;
        }
        out.push_str(&format!(
            "<circle cx=\"{:.3}\" cy=\"{:.3}\" r=\"{r:.3}\" fill=\"none\" stroke=\"#d8d8d8\" stroke-width=\"0.6\"/>",
            SIZE / 2.0,
            SIZE / 2.0
        ));
        out.push_str(&format!(
            "<text x=\"{:.3}\" y=\"{:.3}\" font-size=\"9\" fill=\"#666666\">{step} R&#8853;</text>",
            SIZE / 2.0 + r + 2.0,
            SIZE / 2.0 - 3.0
        ));
    }
    out.push_str(&format!(
        "<circle cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" fill=\"#eef2f4\" stroke=\"#8a9aa2\" stroke-width=\"1\"/>",
        SIZE / 2.0,
        SIZE / 2.0,
        geometry.earth_radius_m * scale
    ));
    if !geometry.level1.is_empty() {
        out.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"#1f6f8b\" stroke-width=\"1.6\" stroke-dasharray=\"6 4\"/>",
            path(&geometry.level1)
        ));
    }
    if !geometry.level3.is_empty() {
        out.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"#9a6b12\" stroke-width=\"1.8\"/>",
            path(&geometry.level3)
        ));
    }
    let legend_x = PAD + 36.0;
    for (row, colour, dash, label) in [
        (
            0.0,
            "#1f6f8b",
            " stroke-dasharray=\"6 4\"",
            "Level 1 &#183; impulsive prediction",
        ),
        (16.0, "#9a6b12", "", "Level 3 &#183; finite-burn flight"),
    ] {
        let y = PAD + row;
        out.push_str(&format!(
            "<g font-size=\"11\" font-family=\"Georgia,serif\"><line x1=\"{PAD}\" y1=\"{y:.3}\" x2=\"{:.3}\" y2=\"{y:.3}\" stroke=\"{colour}\" stroke-width=\"1.7\"{dash}/><text x=\"{legend_x:.3}\" y=\"{:.3}\" fill=\"#222222\">{label}</text></g>",
            PAD + 30.0,
            y + 4.0
        ));
    }
    out.push_str("</svg>");
    out
}

fn table(caption: &str, headers: &[&str], rows: &[Vec<String>], id: &str) -> String {
    let mut out = format!("<table id=\"{id}\"><caption>{caption}</caption><thead><tr>");
    for header in headers {
        out.push_str(&format!("<th scope=\"col\">{header}</th>"));
    }
    out.push_str("</tr></thead><tbody>");
    for row in rows {
        out.push_str("<tr>");
        for (index, cell) in row.iter().enumerate() {
            if index == 0 {
                out.push_str(&format!("<th scope=\"row\">{cell}</th>"));
            } else {
                out.push_str(&format!("<td>{cell}</td>"));
            }
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table>");
    out
}

/// What the QR actually points at, for the header's "Scan at:" line.
///
/// An absolute URL can carry a self-contained `#verify=` fragment or a
/// public receipt lookup `#flight=`. Bare fragments are explicitly labeled
/// as relative instead of inventing a host.
fn scan_target(payload: &str) -> &str {
    if payload.starts_with("#verify=") {
        "this deployment, #verify= fragment (relative to the serving origin)"
    } else if payload.starts_with("#flight=") {
        "this deployment, #flight= fragment (relative to the serving origin)"
    } else {
        payload
    }
}

/// The complete dossier. `payload` is the QR text; pass
/// `fdr.verify_payload()` unless an absolute URL is wanted.
pub fn html(fdr: &Fdr, geometry: &Geometry, payload: &str) -> Result<String, &'static str> {
    html_with_receipt(fdr, geometry, payload, None)
}

/// The caller must verify the receipt signature and FDR binding before
/// supplying the annex; `None` always renders an unverified local result.
pub fn html_with_receipt(
    fdr: &Fdr,
    geometry: &Geometry,
    payload: &str,
    receipt: Option<&ReceiptAnnex>,
) -> Result<String, &'static str> {
    let qr = Qr::new(payload)?;
    let digest = crate::hex::encode(&fdr.digest());
    let signature = crate::hex::encode(&fdr.signature);
    let public_key = crate::hex::encode(&fdr.public_key);
    let public_id = odd_identity::public_id(&fdr.public_key);
    let delta = fdr.elo_after - fdr.elo_before;
    let metric = |index: usize| fdr.metrics[index];

    let mut body = String::new();
    body.push_str(&format!(
        "<header><p class=\"memorandum\">ODDERON TECHNICAL MEMORANDUM &#183; FLIGHT DATA RECORDER &#183; SCHEMA {}</p><h1>Contract {} &#8212; post-flight verification dossier</h1><p class=\"subtitle\">{} attempt &#183; {}</p><p class=\"scan-at\">Scan at: <code>{}</code></p></header>",
        fdr.schema_version,
        fdr.contract_id,
        fdr.attempt_mode.label(),
        escape_html(&fdr.callsign),
        escape_html(scan_target(payload))
    ));

    // Section 1.
    let identity_rows = vec![
        vec!["Callsign".into(), escape_html(&fdr.callsign)],
        vec!["Public identifier".into(), escape_html(&public_id)],
        vec![
            "Flight key identifier (public key)".into(),
            format!("<code>{public_key}</code>"),
        ],
        vec!["Attempt mode".into(), fdr.attempt_mode.label().into()],
        vec![
            "Client-recorded Elo (not authoritative)".into(),
            format!(
                "{} &#8594; {} ({})",
                number(fdr.elo_before, 1),
                number(fdr.elo_after, 1),
                signed(delta, 1)
            ),
        ],
        vec![
            "Mission score".into(),
            format!("{} %", number(metric(4) * 100.0, 2)),
        ],
        vec![
            "Signature digest (SHA-256 of canonical bytes)".into(),
            format!("<code>{digest}</code>"),
        ],
        vec![
            "Ed25519 signature".into(),
            format!("<code class=\"wrap\">{signature}</code>"),
        ],
    ];
    body.push_str(
        "<section id=\"section-1\"><h2>1. Flight identification and cryptographic summary</h2>",
    );
    body.push_str(&table(
        "Table 1. Identity, rating and signature material.",
        &["Field", "Value"],
        &identity_rows,
        "identity-table",
    ));
    body.push_str(&format!(
        "<p>The recorder is signed over a canonical byte string domain-separated with <code>{}</code>. The signature covers the command timeline, all {} debrief fields, the terminal-state hash, the callsign and the client-recorded rating fields, so none of them can be edited independently. These client fields are not a server rating. Only a valid, FDR-bound server receipt in Annex D certifies a ranked result.</p>",
        escape_html(String::from_utf8_lossy(crate::fdr::SIGN_DOMAIN).trim_end_matches('\0')),
        crate::fdr::METRIC_COUNT
    ));
    body.push_str("</section>");

    // Section 2.
    body.push_str(
        "<section id=\"section-2\"><h2>2. Trajectory reconstruction and maneuver schedule</h2>",
    );
    body.push_str("<figure>");
    body.push_str(&trajectory_svg(geometry));
    body.push_str("<figcaption>Figure 1. Level-1 impulsive prediction (dashed) against the Level-3 finite-burn integration (solid), plane projection, J2000. Concentric circles are integer Earth radii.</figcaption></figure>");
    let mut burn_rows = Vec::new();
    for (index, burn) in geometry.burns.iter().enumerate() {
        burn_rows.push(vec![
            format!("Burn {}", index + 1),
            number(burn.start_s, 3),
            number(burn.duration_s, 3),
            number(burn.delta_v_tnw[0], 6),
            number(burn.delta_v_tnw[1], 6),
            number(burn.delta_v_tnw[2], 6),
            number(burn.sweep_rad, 9),
        ]);
    }
    if burn_rows.is_empty() {
        burn_rows.push(vec![
            "No commanded burn".into(),
            "&#8212;".into(),
            "&#8212;".into(),
            "&#8212;".into(),
            "&#8212;".into(),
            "&#8212;".into(),
            "&#8212;".into(),
        ]);
    }
    body.push_str(&table(
        "Table 2. Commanded maneuvers in the tangential-normal-radial frame. Each swept angle is the exact substep integral for that burn; the parts sum to the total in Table 3.",
        &[
            "Burn",
            "Start (s)",
            "Duration (s)",
            "&#916;v<sub>T</sub> (m/s)",
            "&#916;v<sub>N</sub> (m/s)",
            "&#916;v<sub>W</sub> (m/s)",
            "&#916;&#952; (rad)",
        ],
        &burn_rows,
        "maneuver-table",
    ));
    let loss_rows = vec![
        vec![
            "Commanded ideal &#916;v".into(),
            number(metric(20), 6),
            "m/s".into(),
        ],
        vec!["Steering loss".into(), number(metric(21), 6), "m/s".into()],
        vec!["Gravity loss".into(), number(metric(22), 6), "m/s".into()],
        vec!["Speed deficit".into(), number(metric(23), 6), "m/s".into()],
        vec![
            "Total burn duration".into(),
            number(metric(25), 3),
            "s".into(),
        ],
        vec![
            "Swept true-anomaly arc &#916;&#952;".into(),
            format!(
                "{} rad / {}&#176;",
                number(metric(57), 9),
                number(metric(57).to_degrees(), 6)
            ),
            "exact integral".into(),
        ],
        vec![
            "Identity residual".into(),
            number(metric(24), 9),
            "m/s".into(),
        ],
    ];
    body.push_str(&table(
        "Table 3. Finite-burn loss isolated in the two-body model. The swept arc is the exact integral of |r&#215;v|/|r|&#178; accumulated during integration substeps, not a chord sum over decimated keyframes.",
        &["Quantity", "Value", "Unit"],
        &loss_rows,
        "loss-table",
    ));
    body.push_str("</section>");

    // Section 3.
    body.push_str("<section id=\"section-3\"><h2>3. Prediction versus realized flight and error attribution</h2>");
    // The prediction baseline records propellant REMAINING above dry mass,
    // matching the locked field, not propellant consumed.
    let propellant = metric(0) - fdr.dry_mass_kg;
    let prediction_rows = vec![
        vec![
            "Commanded &#916;v".into(),
            number(fdr.prediction_baseline[0], 3),
            number(metric(5), 3),
            signed(metric(5) - fdr.prediction_baseline[0], 3),
            "m/s".into(),
        ],
        vec![
            "Apoapsis altitude".into(),
            number(fdr.prediction_baseline[1] / 1000.0, 3),
            number(metric(1) / 1000.0, 3),
            signed((metric(1) - fdr.prediction_baseline[1]) / 1000.0, 3),
            "km".into(),
        ],
        vec![
            "Periapsis altitude".into(),
            number(fdr.prediction_baseline[2] / 1000.0, 3),
            number(metric(2) / 1000.0, 3),
            signed((metric(2) - fdr.prediction_baseline[2]) / 1000.0, 3),
            "km".into(),
        ],
        vec![
            "Remaining propellant".into(),
            number(fdr.prediction_baseline[3], 3),
            number(propellant, 3),
            signed(propellant - fdr.prediction_baseline[3], 3),
            "kg".into(),
        ],
        vec![
            "Flight time".into(),
            number(fdr.prediction_baseline[4], 3),
            number(metric(29), 3),
            signed(metric(29) - fdr.prediction_baseline[4], 3),
            "s".into(),
        ],
        vec![
            "Target radius miss".into(),
            number(fdr.prediction_baseline[5] / 1000.0, 3),
            number(metric(40) / 1000.0, 3),
            signed((metric(40) - fdr.prediction_baseline[5]) / 1000.0, 3),
            "km".into(),
        ],
    ];
    body.push_str(&table(
        "Table 4. The locked prediction baseline against the integrated outcome.",
        &["Metric", "Predicted", "Realized", "Error", "Unit"],
        &prediction_rows,
        "prediction-table",
    ));
    body.push_str("<p id=\"score-dry-mass-note\">The realized remaining propellant in this table uses the fitted dry mass. For frozen replay compatibility, the score's fuel threshold and prediction term use the catalog baseline dry mass instead; its fuel result can therefore differ from this table.</p>");
    let mut shapley_rows = Vec::new();
    for (player, label) in PLAYERS.iter().enumerate() {
        shapley_rows.push(vec![
            (*label).into(),
            number(metric(6 + player) * 100.0, 3),
            signed(metric(10 + player), 9),
        ]);
    }
    body.push_str(&table(
        "Table 5. Exact four-player Shapley attribution of the outcome error.",
        &["Player", "Share (%)", "Signed contribution"],
        &shapley_rows,
        "shapley-table",
    ));
    let mut coalition_rows = Vec::new();
    for mask in 0..16usize {
        coalition_rows.push(vec![
            format!("{mask:04b}"),
            coalition_label(mask),
            number(metric(41 + mask), 9),
        ]);
    }
    body.push_str(&table(
        "Table 6. All sixteen coalition propagations. Bit order is finite burn, J2, Sun and Moon, drag and solar radiation pressure.",
        &["Mask", "Enabled physics", "Metric"],
        &coalition_rows,
        "coalition-table",
    ));
    body.push_str(&format!(
        "<p>Total error {} &#183; reference metric {} &#183; full-physics metric {} &#183; efficiency residual {} &#183; common attribution epoch {} s{}.</p>",
        number(metric(14), 9),
        number(metric(15), 9),
        number(metric(16), 9),
        number(metric(17), 12),
        number(metric(18), 6),
        if metric(19) == 1.0 { " (truncated at the earliest terminal event)" } else { "" }
    ));
    body.push_str("</section>");

    // Section 4.
    body.push_str("<section id=\"section-4\"><h2>4. Independent verification</h2>");
    body.push_str("<figure class=\"qr-figure\">");
    body.push_str(&qr.svg("verification-qr"));
    body.push_str("<figcaption>Figure 2. Verification payload. Append this fragment to the mission page address, or use Verify / Replay Dossier and paste the recorder file. The fragment is origin-independent so the same code works on any deployment.</figcaption></figure>");
    body.push_str(&format!(
        "<pre id=\"verification-block\">schema_version   {}\ncontract_id      {}\nattempt_mode     {}\npublic_id        {}\npublic_key       {}\ncanonical_digest {}\ned25519_signature\n{}\nfinal_state_hash {}\nmetric_count     {}\ncommand_count    {}\nqr_ecc           {}\nverify_payload   {}</pre>",
        fdr.schema_version,
        fdr.contract_id,
        fdr.attempt_mode.label(),
        escape_html(&public_id),
        public_key,
        digest,
        signature,
        crate::hex::encode(&fdr.final_state_hash),
        crate::fdr::METRIC_COUNT,
        fdr.commands.len(),
        qr.ecc_label(),
        escape_html(payload)
    ));
    body.push_str("<p>Verification performs two independent checks. First the Ed25519 signature must cover the canonical bytes rebuilt from the typed fields; any edit, including one unit in the last place of a metric or one microsecond of command timing, fails with <code>SignatureMismatch</code>. Second the Level-3 integrator is re-run from the command timeline and every one of the recorded fields and the terminal-state hash must reproduce bit for bit, or the document fails with <code>PhysicsReplayMismatch</code>.</p>");
    body.push_str("</section>");

    // Machine-readable annex: every signed field, in order.
    body.push_str("<section id=\"annex-a\"><h2>Annex A. Recorded debrief fields</h2>");
    let mut metric_rows = Vec::new();
    for index in 0..crate::fdr::METRIC_COUNT {
        metric_rows.push(vec![
            format!("{index:02} &#183; {}", escape_html(name(index))),
            number(metric(index), 12),
            escape_html(unit(index)),
        ]);
    }
    body.push_str(&table(
        "Table A1. The complete signed report. An em dash marks an undefined quantity (NaN in the canonical form).",
        &["Index and field", "Value", "Unit"],
        &metric_rows,
        "metric-table",
    ));
    body.push_str("</section>");

    body.push_str(&engine_annex(fdr));
    body.push_str(&thermal_annex(fdr));
    body.push_str(&receipt_annex(receipt));

    let mode_class = if fdr.attempt_mode == AttemptMode::Ranked {
        "ranked"
    } else {
        "practice"
    };
    Ok(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Odderon TM &#183; Contract {} &#183; {}</title><style>{CSS}</style></head><body class=\"{mode_class}\">{body}<footer>Odderon flight data recorder &#183; schema {} &#183; digest <code>{digest}</code></footer></body></html>\n",
        fdr.contract_id,
        escape_html(&fdr.callsign),
        fdr.schema_version
    ))
}

/// Annex D includes the signed bytes, not just a visual badge, so an
/// independent verifier can check exactly what the authority signed.
fn receipt_annex(receipt: Option<&ReceiptAnnex>) -> String {
    let mut out = String::from("<section id=\"annex-d\"><h2>Annex D. Server receipt</h2>");
    let Some(r) = receipt else {
        out.push_str("<p>UNVERIFIED LOCAL / PRACTICE. No valid server receipt was supplied for this flight.</p></section>");
        return out;
    };
    let label = if r.mode == AttemptMode::Ranked {
        "&#9733; CERTIFIED SERVER-RANKED RECEIPT"
    } else {
        "SERVER-VERIFIED PRACTICE"
    };
    out.push_str(&format!("<p><strong>{label}</strong>. The application checked this receipt against its configured server key and the replayed flight. Verify the embedded bytes and key independently before trusting a printed copy.</p>"));
    let rows = vec![
        vec!["Attempt".into(), r.attempt_num.to_string()],
        vec!["Mode".into(), r.mode.label().into()],
        vec!["Score".into(), r.score_s.to_string()],
        vec!["Server Elo before".into(), r.elo_before.to_string()],
        vec!["Server Elo after".into(), r.elo_after.to_string()],
        vec!["Mission Elo after".into(), r.mission_elo_after.to_string()],
        vec![
            "Server timestamp (Unix seconds)".into(),
            r.timestamp_s.to_string(),
        ],
        vec![
            "Server public key".into(),
            format!("<code>{}</code>", crate::hex::encode(&r.server_public_key)),
        ],
        vec![
            "Canonical receipt and signature (hex)".into(),
            format!(
                "<code class=\"wrap\">{}</code>",
                crate::hex::encode(&r.receipt_bytes)
            ),
        ],
    ];
    out.push_str(&table(
        "Table D1. Server-signed receipt. The final 64 bytes are the Ed25519 signature.",
        &["Field", "Value"],
        &rows,
        "receipt-table",
    ));
    out.push_str("</section>");
    out
}

/// Annex B: the signed engine design and the performance recomputed from it.
/// A legacy record has no design, and says so rather than printing a claim as
/// if it had been checked.
fn engine_annex(fdr: &Fdr) -> String {
    let mut out = String::from(
        "<section id=\"annex-b\"><h2>Annex B. Engine design and recomputed performance</h2>",
    );
    let Some(spec) = &fdr.engine else {
        out.push_str("<p id=\"engine-legacy\">This record predates engine certification (schema 1). Its thrust and specific impulse are recorded claims that no design backs; the replay reproduces the flight with those numbers, but the engine itself was never verified.</p></section>");
        return out;
    };
    let Ok(solution) = odd_propulsion::solve_engine(spec, 0.0) else {
        out.push_str("<p id=\"engine-legacy\">The signed engine design does not solve inside the model envelope.</p></section>");
        return out;
    };
    let geometry = spec.geometry();
    let rows = vec![
        vec![
            "Propellant pair".into(),
            escape_html(spec.propellant.label()),
        ],
        vec!["Mixture ratio O/F".into(), number(spec.of_ratio, 4)],
        vec![
            "Chamber pressure".into(),
            format!("{} MPa", number(spec.p_c_pa / 1e6, 4)),
        ],
        vec![
            "Throat radius / diameter".into(),
            format!(
                "{} / {} mm",
                number(spec.r_throat_m * 1000.0, 3),
                number(spec.r_throat_m * 2000.0, 3)
            ),
        ],
        vec![
            "Exit radius".into(),
            format!("{} mm", number(spec.r_exit_m * 1000.0, 3)),
        ],
        vec![
            "Expansion ratio &#949;".into(),
            number(geometry.expansion_ratio(), 4),
        ],
        vec![
            "Bell length fraction / length".into(),
            format!(
                "{} / {} m",
                number(spec.length_fraction, 3),
                number(geometry.length_m(), 4)
            ),
        ],
        vec![
            "Chamber temperature".into(),
            format!("{} K", number(solution.chamber_temperature_k, 2)),
        ],
        vec![
            "Effective molar mass".into(),
            format!("{} g/mol", number(solution.molar_mass_kg_mol * 1000.0, 4)),
        ],
        vec![
            "Specific heat ratio &#947;".into(),
            number(solution.gamma, 5),
        ],
        vec![
            "Bulk propellant density".into(),
            format!("{} kg/m&#179;", number(solution.bulk_density_kg_m3, 3)),
        ],
        vec![
            "Characteristic velocity c*".into(),
            format!("{} m/s", number(solution.c_star_m_s, 3)),
        ],
        vec![
            "Mass flow".into(),
            format!("{} kg/s", number(solution.mass_flow_kg_s, 6)),
        ],
        vec![
            "Exit Mach / pressure / temperature".into(),
            format!(
                "{} / {} kPa / {} K",
                number(solution.exit_mach, 4),
                number(solution.exit_pressure_pa / 1000.0, 4),
                number(solution.exit_temperature_k, 2)
            ),
        ],
        vec![
            "Bell divergence factor &#955;".into(),
            number(solution.divergence_factor, 6),
        ],
        vec![
            "Vacuum thrust".into(),
            format!("{} kN", number(solution.thrust_n / 1000.0, 4)),
        ],
        vec![
            "Vacuum specific impulse".into(),
            format!("{} s", number(solution.specific_impulse_s, 4)),
        ],
        vec![
            "Engine dry mass".into(),
            format!("{} kg", number(solution.dry_mass_kg, 4)),
        ],
        vec![
            "Turbopump shaft power".into(),
            format!("{} kW", number(solution.pump_power_w / 1000.0, 3)),
        ],
        vec![
            "Throat heat flux".into(),
            format!(
                "{} MW/m&#178;",
                number(solution.throat_heat_flux_w_m2 / 1e6, 4)
            ),
        ],
        vec![
            "Throat wall temperature".into(),
            format!(
                "{} K {}",
                number(solution.throat_wall_temperature_k, 2),
                if solution.thermal_limit_exceeded {
                    "&#8212; <strong>over the 1350 K liner limit</strong>"
                } else {
                    "(within the 1350 K liner limit)"
                }
            ),
        ],
    ];
    out.push_str(&table(
        "Table B1. The engine design carried in the signed bytes, and the performance the verifier recomputed from it. Thrust, specific impulse and engine mass are outputs of the design, never client inputs.",
        &["Field", "Value"],
        &rows,
        "engine-table",
    ));
    out.push_str("<figure>");
    out.push_str(&engine_svg(spec, &solution));
    out.push_str("<figcaption>Figure B1. Axisymmetric chamber and bell contour at true relative scale, with the quasi-1D Mach profile along the axis. The wall is the parabolic Rao contour the solver used; the dashed line is the centreline.</figcaption></figure>");
    out.push_str("</section>");
    out
}

/// Annex C: the cryogenic fit-out, the boil-off it produced and who sized it.
///
/// Every number here is inside the signed bytes and was recomputed by the
/// verifier, so the annex distinguishes a measured zero (storables) from a
/// record that predates the thermal block entirely.
fn thermal_annex(fdr: &Fdr) -> String {
    let mut out = String::from(
        "<section id=\"annex-c\"><h2>Annex C. Cryogenic balance, vent history and delegation</h2>",
    );
    if fdr.schema_version < 3 {
        out.push_str("<p id=\"thermal-legacy\">This record predates schema 3, so it carries no thermal specification, vent history or delegation provenance. No boil-off claim is made for it; the replay reproduces the trajectory only.</p>");
        out.push_str("</section>");
        return out;
    }

    let departments = [
        (0b0001, "Propulsion"),
        (0b0010, "Thermal / cryogenic"),
        (0b0100, "Power"),
        (0b1000, "Guidance, navigation and control"),
    ];
    let delegated: Vec<&str> = departments
        .iter()
        .filter(|(bit, _)| fdr.delegation_mask & bit != 0)
        .map(|(_, name)| *name)
        .collect();
    let provisioning = if delegated.is_empty() {
        "Every discipline was sized by hand; the efficiency component is uncapped.".to_string()
    } else {
        format!(
            "Delegated to rule-of-thumb chiefs: {}. Each delegated discipline carried +35 % mass and +30 % cost, and the efficiency component of the score is capped at 0.65.",
            escape_html(&delegated.join(", "))
        )
    };
    out.push_str(&format!(
        "<p id=\"delegation-provenance\">Delegation bitmask <code>0b{:04b}</code>. {}</p>",
        fdr.delegation_mask, provisioning
    ));

    let total_loss = fdr.boil_off_kg[0] + fdr.boil_off_kg[1];
    let mut rows = vec![
        vec![
            "Oxidizer boiled off".into(),
            format!("{} kg", number(fdr.boil_off_kg[0], 6)),
        ],
        vec![
            "Fuel boiled off".into(),
            format!("{} kg", number(fdr.boil_off_kg[1], 6)),
        ],
        vec![
            "Total propellant lost".into(),
            format!("{} kg", number(total_loss, 6)),
        ],
        vec![
            "Cryocooler electricity".into(),
            format!("{} MJ", number(fdr.cryocooler_energy_j / 1e6, 6)),
        ],
        vec![
            "Relief-valve openings".into(),
            fdr.vent_events.len().to_string(),
        ],
    ];
    if let Some(spec) = fdr.thermal_specs.first() {
        // v4 couples the vent into the flight, so the old "we do not subtract
        // it" and "the tanks do not fit the airframe" disclaimers are no longer
        // true. A v1–v3 certificate keeps its original wording below.
        if fdr.schema_version >= crate::fdr::SCHEMA_VERSION {
            out.push_str("<p id=\"thermal-coupling-note\">Vented mass is drawn from the same propellant pool the engine burns, so it reduces both the remaining fuel and the mass every acceleration divides by. A pool emptied by boil-off truncates thrust and is signed as a flameout.</p>");
            out.push_str("<p id=\"tank-volume-note\">Tank volume, mixture split and ullage come from the geometry the contract card sizes the airframe with, so the signed stage fits the vehicle it describes.</p>");
        } else {
            out.push_str("<p id=\"thermal-coupling-note\">This is a pre-v4 certificate. Its thermal loss is integrated and signed but was not subtracted from propagated mass or the burn fuel budget under the model it was flown with.</p>");
            out.push_str("<p id=\"tank-volume-note\">This is a pre-v4 certificate. Its tank volumes and ullage were not reconciled with the catalog airframe, so the replay is not a tank-fit proof.</p>");
        }
        rows.extend([
            vec!["Insulation layers".into(), spec.mli_layers.to_string()],
            vec![
                "Effective blanket emittance".into(),
                number(spec.epsilon_eff, 8),
            ],
            vec![
                "Support heat leak".into(),
                format!("{} W/K", number(spec.strut_conductance_w_k, 4)),
            ],
            vec![
                "Relief pressure".into(),
                format!("{} kPa", number(spec.relief_pressure_pa / 1e3, 3)),
            ],
            vec![
                "Radiator area".into(),
                format!("{} m&#178;", number(spec.radiator_area_m2, 3)),
            ],
            vec![
                "Commanded cooler lift".into(),
                format!("{} W", number(spec.cryocooler_w, 3)),
            ],
            vec![
                "Cooler Carnot fraction".into(),
                number(spec.cryocooler_carnot_fraction, 4),
            ],
            vec![
                "Array output at 1 AU".into(),
                format!("{} W", number(spec.array_w_at_1au, 1)),
            ],
        ]);
    } else {
        out.push_str("<p id=\"thermal-storable\">This stage carries no cryogenic tank block: its propellants are storable, so boil-off is identically zero rather than unmeasured.</p>");
    }
    out.push_str(&table(
        "Table C1. The signed cryogenic fit-out and the loss the verifier recomputed by integrating the heat balance along the flown trajectory. Tank pressure, temperature and every vent opening below are outputs of that integration, never client claims.",
        &["Field", "Value"],
        &rows,
        "thermal-table",
    ));

    if !fdr.vent_events.is_empty() {
        let vents: Vec<Vec<String>> = fdr
            .vent_events
            .iter()
            .map(|event| {
                vec![
                    number(event.time_us as f64 / 1e6, 3),
                    if event.tank == 0 {
                        "Oxidizer".into()
                    } else {
                        "Fuel".into()
                    },
                    number(event.pressure_pa / 1e3, 3),
                    number(event.temperature_k, 4),
                ]
            })
            .collect();
        out.push_str(&table(
            "Table C2. Every relief-valve opening, in flight order. The verifier reproduces this list event for event; a fabricated opening or a shifted timestamp fails the replay.",
            &["Flight time (s)", "Tank", "Pressure (kPa)", "Temperature (K)"],
            &vents,
            "vent-table",
        ));
    }
    out.push_str("</section>");
    out
}

/// Vector cross-section of the signed engine: chamber, convergent, bell and
/// the Mach profile. No raster assets, so it prints at any scale.
pub fn engine_svg(
    spec: &odd_propulsion::EngineSpec,
    solution: &odd_propulsion::EngineSolution,
) -> String {
    const W: f64 = 640.0;
    const H: f64 = 300.0;
    let geometry = spec.geometry();
    let chamber_radius = solution.chamber_radius_m;
    let chamber_length = solution.chamber_length_m;
    let convergent = (chamber_radius - spec.r_throat_m) / 0.577_350_269_189_625_7;
    let total_length = chamber_length + convergent + geometry.length_m();
    let scale = ((W - 80.0) / total_length.max(1e-6))
        .min((H - 90.0) / (2.0 * chamber_radius.max(spec.r_exit_m)).max(1e-6));
    let axis = H * 0.5;
    let x0 = 40.0;
    let x = |metres: f64| x0 + metres * scale;
    let y = |radius: f64| axis - radius * scale;
    let throat_x = x(chamber_length + convergent);
    let mut out = format!(
        "<svg id=\"engine-figure\" class=\"figure\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {H}\" width=\"{W}\" height=\"{H}\" role=\"img\" aria-label=\"Axisymmetric engine cross-section and axial Mach profile\">"
    );
    out.push_str(&format!(
        "<line x1=\"{x0}\" y1=\"{axis}\" x2=\"{}\" y2=\"{axis}\" stroke=\"#888\" stroke-width=\"0.7\" stroke-dasharray=\"5 4\"/>",
        x(total_length)
    ));
    // Upper wall: chamber barrel, convergent, then the sampled bell.
    let mut upper = format!(
        "M {} {} L {} {} L {} {}",
        x(0.0),
        y(chamber_radius),
        x(chamber_length),
        y(chamber_radius),
        throat_x,
        y(spec.r_throat_m)
    );
    let stations = odd_propulsion::flow_profile(spec, 0.0, 96).unwrap_or_default();
    for station in &stations {
        upper.push_str(&format!(
            " L {} {}",
            throat_x + station.x_norm * geometry.length_m() * scale,
            y(station.radius_m)
        ));
    }
    out.push_str(&format!(
        "<path d=\"{upper}\" fill=\"none\" stroke=\"#111\" stroke-width=\"1.6\"/>"
    ));
    // Mirror below the centreline.
    let mut lower = format!(
        "M {} {} L {} {} L {} {}",
        x(0.0),
        2.0 * axis - y(chamber_radius),
        x(chamber_length),
        2.0 * axis - y(chamber_radius),
        throat_x,
        2.0 * axis - y(spec.r_throat_m)
    );
    for station in &stations {
        lower.push_str(&format!(
            " L {} {}",
            throat_x + station.x_norm * geometry.length_m() * scale,
            2.0 * axis - y(station.radius_m)
        ));
    }
    out.push_str(&format!(
        "<path d=\"{lower}\" fill=\"none\" stroke=\"#111\" stroke-width=\"1.6\"/>"
    ));
    out.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#111\" stroke-width=\"1.6\"/>",
        x(0.0),
        y(chamber_radius),
        x(0.0),
        2.0 * axis - y(chamber_radius)
    ));
    // Mach profile, scaled into the lower margin.
    if let Some(max_mach) = stations.last().map(|s| s.mach) {
        let mut profile = String::new();
        for (index, station) in stations.iter().enumerate() {
            let px = throat_x + station.x_norm * geometry.length_m() * scale;
            let py = H - 18.0 - 46.0 * station.mach / max_mach.max(1e-9);
            profile.push_str(&format!(
                "{}{} {}",
                if index == 0 { "M " } else { " L " },
                px,
                py
            ));
        }
        out.push_str(&format!(
            "<path d=\"{profile}\" fill=\"none\" stroke=\"#b3541e\" stroke-width=\"1.2\"/>"
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"#b3541e\">M(x), exit {}</text>",
            throat_x,
            H - 70.0,
            number(max_mach, 3)
        ));
    }
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"#111\">throat {} mm</text>",
        throat_x - 20.0,
        y(spec.r_throat_m) - 8.0,
        number(spec.r_throat_m * 2000.0, 2)
    ));
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"#111\">exit {} mm &#183; &#949; = {}</text>",
        x(total_length) - 150.0,
        y(spec.r_exit_m) - 8.0,
        number(spec.r_exit_m * 2000.0, 2),
        number(geometry.expansion_ratio(), 2)
    ));
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"#111\">{} &#183; O/F {} &#183; {} MPa</text>",
        x0,
        20.0,
        escape_html(spec.propellant.label()),
        number(spec.of_ratio, 3),
        number(spec.p_c_pa / 1e6, 3)
    ));
    out.push_str("</svg>");
    out
}

const CSS: &str = "@page{size:A4;margin:18mm 16mm}body{font-family:Georgia,'Times New Roman',serif;color:#111;background:#fff;margin:0 auto;padding:24px;max-width:52rem;line-height:1.5}h1{font-size:1.6rem;font-weight:400;margin:.2rem 0}h2{font-size:1.1rem;margin:1.8rem 0 .6rem;border-bottom:1px solid #ccc;padding-bottom:.25rem}.memorandum{font-size:.66rem;letter-spacing:.16em;text-transform:uppercase;color:#555;margin:0}.subtitle{font-size:.9rem;color:#444;margin:.2rem 0 1rem}table{border-collapse:collapse;width:100%;font-size:.76rem;margin:.8rem 0 1.2rem}caption{caption-side:top;text-align:left;font-size:.72rem;color:#444;padding-bottom:.35rem}th,td{border:1px solid #bbb;padding:4px 6px;text-align:right}th[scope=row],thead th:first-child{text-align:left}thead th{background:#f0f0ee}code{font-family:ui-monospace,'Courier New',monospace;font-size:.72rem}code.wrap{word-break:break-all}figure{margin:1rem 0;text-align:center}figcaption{font-size:.72rem;color:#444;text-align:left;margin-top:.4rem}.figure{max-width:100%;height:auto;border:1px solid #ddd}.qr-figure{display:flex;gap:1rem;align-items:flex-start;text-align:left}.qr{flex:0 0 auto;border:1px solid #ddd}pre{font-family:ui-monospace,'Courier New',monospace;font-size:.68rem;background:#f7f7f5;border:1px solid #ddd;padding:.7rem;overflow-x:auto;white-space:pre-wrap;word-break:break-all}footer{margin-top:2rem;border-top:1px solid #ccc;padding-top:.5rem;font-size:.66rem;color:#555}section{break-inside:auto}table,figure{break-inside:avoid}@media print{body{padding:0}a{color:inherit}}";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fdr::{Command, METRIC_COUNT, SCHEMA_VERSION};

    pub(crate) fn sample() -> Fdr {
        let mut fdr = Fdr {
            schema_version: SCHEMA_VERSION,
            thermal_specs: Vec::new(),
            delegation_mask: 0,
            vent_events: Vec::new(),
            boil_off_kg: [0.0; 2],
            cryocooler_energy_j: 0.0,
            flown_burn_windows: vec![],
            flameout_time_s: None,
            propellant_remaining_kg: None,
            contract_id: 101,
            attempt_mode: AttemptMode::Ranked,
            callsign: "TEST PILOT".into(),
            epoch_s: 820_497_600,
            duration_us: 10_800_000_000,
            initial_mass_kg: 1200.0,
            dry_mass_kg: 1020.0,
            thrust_n: 2400.0,
            specific_impulse_s: 310.0,
            engine: Some(odd_propulsion::EngineSpec {
                propellant: odd_propulsion::Propellant::NtoMmh,
                of_ratio: 2.0,
                p_c_pa: 1.5e6,
                r_throat_m: 0.0164,
                r_exit_m: 0.1216,
                length_fraction: 0.8,
            }),
            initial_state: [6_638_137.0, 0.0, 0.0, 0.0, 7748.0, 0.0],
            node_metric: false,
            prediction_baseline: [180.0, 550_000.0, 260_000.0, 120.0, 10_800.0, f64::NAN],
            commands: vec![Command {
                time_us: 200_000_000,
                delta_v_tnw: [81.5, 0.0, 0.0],
                guidance: None,
            }],
            metrics: [f64::NAN; METRIC_COUNT],
            final_state_hash: [7u8; 32],
            elo_before: 1200.0,
            elo_after: 1203.5,
            public_key: [0u8; 32],
            signature: [0u8; 64],
        };
        fdr.metrics[0] = 1080.0;
        fdr.metrics[1] = 550_100.0;
        fdr.metrics[2] = 259_000.0;
        fdr.metrics[4] = 0.93;
        fdr.metrics[5] = 181.0;
        fdr.metrics[20] = 181.0;
        fdr.metrics[21] = 0.4;
        fdr.metrics[22] = 1.2;
        fdr.metrics[25] = 92.0;
        fdr.metrics[29] = 10_800.0;
        fdr.metrics[57] = 0.02;
        for (player, share) in [0.6, 0.3, 0.07, 0.03].into_iter().enumerate() {
            fdr.metrics[6 + player] = share;
            fdr.metrics[10 + player] = share * 1000.0;
        }
        for mask in 0..16 {
            fdr.metrics[41 + mask] = 550_000.0 + mask as f64;
        }
        fdr
    }

    fn geometry() -> Geometry {
        Geometry {
            level1: vec![[6_638_137.0, 0.0], [0.0, 6_928_137.0], [-6_638_137.0, 0.0]],
            level3: vec![
                [6_638_137.0, 0.0],
                [10.0, 6_928_200.0],
                [-6_638_900.0, 12.0],
            ],
            burns: vec![BurnRow {
                start_s: 200.0,
                duration_s: 41.0,
                delta_v_tnw: [81.5, 0.0, 0.0],
                sweep_rad: 0.02,
            }],
            earth_radius_m: 6_378_137.0,
        }
    }

    #[test]
    fn dossier_is_byte_identical_across_runs() {
        let fdr = sample();
        let geometry = geometry();
        let payload = fdr.verify_payload();
        let first = html(&fdr, &geometry, &payload).unwrap();
        let second = html(&fdr, &geometry, &payload).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), second.len());
    }

    #[test]
    fn all_four_sections_and_the_annex_are_present() {
        let fdr = sample();
        let document = html(&fdr, &geometry(), &fdr.verify_payload()).unwrap();
        for id in [
            "section-1",
            "section-2",
            "section-3",
            "section-4",
            "annex-c",
            "annex-a",
            "annex-b",
            "engine-table",
            "engine-figure",
        ] {
            assert!(document.contains(&format!("id=\"{id}\"")), "missing {id}");
        }
        for id in [
            "identity-table",
            "trajectory-figure",
            "maneuver-table",
            "loss-table",
            "prediction-table",
            "shapley-table",
            "coalition-table",
            "verification-qr",
            "verification-block",
            "metric-table",
        ] {
            assert!(document.contains(&format!("id=\"{id}\"")), "missing {id}");
        }
        assert!(document.contains("id=\"score-dry-mass-note\""));
        assert!(document.contains(
            "score's fuel threshold and prediction term use the catalog baseline dry mass"
        ));
        assert_eq!(
            document.matches("<svg").count(),
            3,
            "trajectory, QR and engine schematic must all be vector"
        );
        assert!(
            !document.contains("<img"),
            "no raster assets in a print dossier"
        );
        assert!(
            document.contains("@page"),
            "print geometry must be declared"
        );
        // All sixteen coalitions are enumerated.
        assert!(document.matches("<tr><th scope=\"row\">").count() >= 16);
    }

    /// The header must say where the QR points, and must not invent a host
    /// when the dossier was built with a bare fragment.
    #[test]
    fn the_header_names_the_scan_target() {
        let fdr = sample();
        let relative = html(&fdr, &geometry(), &fdr.verify_payload()).unwrap();
        assert!(relative.contains("Scan at:"));
        assert!(relative.contains("relative to the serving origin"));
        let absolute = html(
            &fdr,
            &geometry(),
            &format!("https://flights.example.org/verify{}", fdr.verify_payload()),
        )
        .unwrap();
        assert!(absolute.contains("Scan at:"));
        assert!(absolute.contains("https://flights.example.org/verify#verify="));
        let ranked = html(&fdr, &geometry(), "#flight=abcd").unwrap();
        assert!(ranked.contains("#flight= fragment (relative to the serving origin)"));
        let ranked_absolute = html(
            &fdr,
            &geometry(),
            "https://flights.example.org/mission.html#flight=abcd",
        )
        .unwrap();
        assert!(ranked_absolute.contains("https://flights.example.org/mission.html#flight=abcd"));
    }

    #[test]
    fn a_callsign_cannot_inject_markup() {
        let mut fdr = sample();
        fdr.callsign = "<script>x</script>".into();
        // The shape check rejects the charset before rendering matters, but
        // the renderer must also escape rather than trust its caller.
        let document = html(&fdr, &geometry(), "#verify=abc").unwrap();
        assert!(!document.contains("<script>"));
        assert!(document.contains("&lt;script&gt;"));
    }

    /// The dossier must print the cryogenic balance and who sized the stage,
    /// and must distinguish a storable zero from a pre-v3 record.
    #[test]
    fn the_thermal_annex_reports_loss_vents_and_delegation() {
        let mut fdr = sample();
        fdr.schema_version = 3;
        fdr.delegation_mask = 0b0101;
        fdr.boil_off_kg = [12.5, 582.125];
        fdr.cryocooler_energy_j = 6_991_965_600.0;
        fdr.vent_events = vec![crate::VentEvent {
            time_us: 1_500_000,
            tank: 1,
            pressure_pa: 200_000.0,
            temperature_k: 23.234_177_419_9,
        }];
        let document = html(&fdr, &geometry(), &fdr.verify_payload()).unwrap();
        assert!(document.contains("id=\"annex-c\""));
        assert!(document.contains("id=\"thermal-table\""));
        assert!(document.contains("id=\"vent-table\""));
        assert!(
            document.contains("594.625000"),
            "the total loss must be printed"
        );
        assert!(
            document.contains("6991.965600"),
            "the cooler draw must be printed"
        );
        assert!(
            document.contains("23.2342"),
            "the vent temperature must be printed"
        );
        let provenance = document.contains("id=\"delegation-provenance\"");
        assert!(provenance);
        assert!(document.contains("Propulsion, Power"));
        assert!(document.contains("capped at 0.65"));

        // A fully manual stage says so instead of implying a chief sized it.
        fdr.delegation_mask = 0;
        let manual = html(&fdr, &geometry(), &fdr.verify_payload()).unwrap();
        assert!(manual.contains("sized by hand"));

        // A legacy record makes no thermal claim at all.
        fdr.schema_version = 2;
        let legacy = html(&fdr, &geometry(), &fdr.verify_payload()).unwrap();
        assert!(legacy.contains("id=\"thermal-legacy\""));
        assert!(!legacy.contains("id=\"vent-table\""));
    }
}

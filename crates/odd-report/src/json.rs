//! Minimal deterministic JSON writer.
//!
//! Hand-rolled rather than pulled from a serializer so key order, number
//! formatting and whitespace are decisions this crate makes explicitly: two
//! runs must emit identical bytes. Numbers use Rust's shortest round-trip
//! form. NaN and infinities are not JSON numbers, so they become `null` and
//! the reader maps `null` back to NaN; every undefined metric in the report
//! is NaN by design, so this mapping is part of the format contract.

pub struct Json {
    out: String,
}

impl Default for Json {
    fn default() -> Self {
        Self::new()
    }
}

impl Json {
    pub fn new() -> Self {
        Self { out: String::new() }
    }
    pub fn object(&mut self, body: impl FnOnce(&mut Object)) {
        self.out.push('{');
        let mut object = Object {
            out: &mut self.out,
            first: true,
        };
        body(&mut object);
        self.out.push('}');
    }
    pub fn finish(self) -> String {
        self.out
    }
}

pub struct Object<'a> {
    out: &'a mut String,
    first: bool,
}

impl Object<'_> {
    fn key(&mut self, name: &str) {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        escape_into(self.out, name);
        self.out.push(':');
    }
    pub fn number(&mut self, name: &str, value: f64) {
        self.key(name);
        self.raw_number(value);
    }
    fn raw_number(&mut self, value: f64) {
        if value.is_finite() {
            // Display for f64 is the shortest string that parses back to the
            // same bits, which is what a canonical transport needs.
            self.out.push_str(&value.to_string());
        } else {
            self.out.push_str("null");
        }
    }
    pub fn string(&mut self, name: &str, value: &str) {
        self.key(name);
        escape_into(self.out, value);
    }
    pub fn boolean(&mut self, name: &str, value: bool) {
        self.key(name);
        self.out.push_str(if value { "true" } else { "false" });
    }
    pub fn array(&mut self, name: &str, values: &[f64]) {
        self.key(name);
        self.out.push('[');
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                self.out.push(',');
            }
            self.raw_number(*value);
        }
        self.out.push(']');
    }
    pub fn commands(&mut self, name: &str, commands: &[crate::fdr::Command]) {
        self.key(name);
        self.out.push('[');
        for (index, command) in commands.iter().enumerate() {
            if index > 0 {
                self.out.push(',');
            }
            self.out.push('{');
            let mut inner = Object {
                out: self.out,
                first: true,
            };
            inner.number("time_us", command.time_us as f64);
            inner.array("delta_v_tnw", &command.delta_v_tnw);
            match command.guidance {
                Some(target) => inner.array("guidance", &target),
                None => {
                    inner.key("guidance");
                    inner.out.push_str("null");
                }
            }
            self.out.push('}');
        }
        self.out.push(']');
    }

    /// Per-stage cryogenic fit-out. Transport only: the signed bytes come
    /// from `Fdr::canonical_bytes`, never from this rendering.
    pub fn thermal_specs(&mut self, name: &str, specs: &[crate::fdr::StageThermalSpec]) {
        self.key(name);
        self.out.push('[');
        for (index, spec) in specs.iter().enumerate() {
            if index > 0 {
                self.out.push(',');
            }
            self.out.push('{');
            let mut o = Object {
                out: self.out,
                first: true,
            };
            o.number("stage", spec.stage as f64);
            o.array("tank_volumes_m3", &spec.tank_volumes_m3);
            o.number("propellant_id", spec.propellant_id as f64);
            o.number("mli_layers", spec.mli_layers as f64);
            o.number("epsilon_eff", spec.epsilon_eff);
            o.number("tank_shell_material", spec.tank_shell_material as f64);
            o.number("absorptivity", spec.absorptivity);
            o.number("outer_epsilon", spec.outer_epsilon);
            o.number("strut_conductance_w_k", spec.strut_conductance_w_k);
            o.number("relief_pressure_pa", spec.relief_pressure_pa);
            o.number("radiator_area_m2", spec.radiator_area_m2);
            o.number("radiator_epsilon", spec.radiator_epsilon);
            o.number("cryocooler_w", spec.cryocooler_w);
            o.number(
                "cryocooler_carnot_fraction",
                spec.cryocooler_carnot_fraction,
            );
            o.number("array_w_at_1au", spec.array_w_at_1au);
            o.number("avionics_w", spec.avionics_w);
            o.number("electric_propulsion_w", spec.electric_propulsion_w);
            o.number("structure_temperature_k", spec.structure_temperature_k);
            o.number("sink_temperature_k", spec.sink_temperature_k);
            self.out.push('}');
        }
        self.out.push(']');
    }

    /// Exact relief-valve openings.
    pub fn vent_events(&mut self, name: &str, events: &[crate::fdr::VentEvent]) {
        self.key(name);
        self.out.push('[');
        for (index, event) in events.iter().enumerate() {
            if index > 0 {
                self.out.push(',');
            }
            self.out.push('{');
            let mut o = Object {
                out: self.out,
                first: true,
            };
            o.number("time_us", event.time_us as f64);
            o.number("tank", event.tank as f64);
            o.number("pressure_pa", event.pressure_pa);
            o.number("temperature_k", event.temperature_k);
            self.out.push('}');
        }
        self.out.push(']');
    }
}

/// JSON string escaping. Control characters use the short forms where JSON
/// defines them and \u00XX otherwise, so the output is stable.
pub fn escape_into(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn escape(value: &str) -> String {
    let mut out = String::new();
    escape_into(&mut out, value);
    out
}

/// HTML text escaping for the dossier. The callsign is user-supplied, so it
/// must never be able to introduce markup into a printable document.
pub fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

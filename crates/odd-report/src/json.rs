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

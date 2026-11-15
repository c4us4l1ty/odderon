//! Deterministic vector QR. The matrix comes from qrcodegen at a fixed error
//! correction level, so the same payload always yields the same modules and
//! therefore the same SVG bytes.
//!
//! Dark modules are emitted as one rectangle per horizontal run. That keeps
//! the document small while staying pure vector, and a decoder can rebuild
//! the matrix from the rectangles without rendering the page.
use qrcodegen::{QrCode, QrCodeEcc};

/// Medium recovers ~15% of codewords: enough for a printed dossier that gets
/// folded, while keeping the module count low for a ~1 kB payload.
pub const ECC: QrCodeEcc = QrCodeEcc::Medium;
const QUIET: i32 = 4;

pub struct Qr {
    pub size: i32,
    /// (x, y, run length) for every dark run, row-major.
    pub runs: Vec<(i32, i32, i32)>,
}

impl Qr {
    pub fn new(payload: &str) -> Result<Self, &'static str> {
        let code =
            QrCode::encode_text(payload, ECC).map_err(|_| "payload too long for a QR code")?;
        let size = code.size();
        let mut runs = Vec::new();
        for y in 0..size {
            let mut x = 0;
            while x < size {
                if !code.get_module(x, y) {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < size && code.get_module(x, y) {
                    x += 1;
                }
                runs.push((start, y, x - start));
            }
        }
        Ok(Self { size, runs })
    }

    /// Side length including the quiet zone, in modules.
    pub fn outer(&self) -> i32 {
        self.size + 2 * QUIET
    }

    /// Rebuild the module matrix. Used by the determinism gate to decode the
    /// emitted SVG rather than trusting the encoder a second time.
    pub fn matrix(&self) -> Vec<Vec<bool>> {
        let outer = self.outer() as usize;
        let mut grid = vec![vec![false; outer]; outer];
        for (x, y, length) in &self.runs {
            for offset in 0..*length {
                grid[(*y + QUIET) as usize][(*x + QUIET + offset) as usize] = true;
            }
        }
        grid
    }

    pub fn svg(&self, id: &str) -> String {
        let outer = self.outer();
        let mut out = String::new();
        out.push_str(&format!(
            "<svg id=\"{id}\" class=\"qr\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {outer} {outer}\" width=\"260\" height=\"260\" shape-rendering=\"crispEdges\" role=\"img\" aria-label=\"QR code carrying the verification payload\" data-qr-size=\"{}\" data-qr-quiet=\"{QUIET}\" data-qr-ecc=\"M\">",
            self.size
        ));
        out.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"{outer}\" height=\"{outer}\" fill=\"#ffffff\"/>"
        ));
        out.push_str("<g fill=\"#000000\">");
        for (x, y, length) in &self.runs {
            out.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{length}\" height=\"1\"/>",
                x + QUIET,
                y + QUIET
            ));
        }
        out.push_str("</g></svg>");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_is_stable_and_runs_rebuild_the_matrix() {
        let first = Qr::new("#verify=odderon").unwrap();
        let second = Qr::new("#verify=odderon").unwrap();
        assert_eq!(first.svg("q"), second.svg("q"));
        assert_eq!(first.runs, second.runs);
        let code = QrCode::encode_text("#verify=odderon", ECC).unwrap();
        let matrix = first.matrix();
        for y in 0..first.size {
            for x in 0..first.size {
                assert_eq!(
                    matrix[(y + QUIET) as usize][(x + QUIET) as usize],
                    code.get_module(x, y),
                    "module {x},{y}"
                );
            }
        }
        // The quiet zone must be blank or scanners cannot find the finder.
        for row in matrix.iter().take(QUIET as usize) {
            assert!(row.iter().all(|dark| !dark));
        }
    }
}

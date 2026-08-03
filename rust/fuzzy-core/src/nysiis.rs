//! NYSIIS, exact port of `src/fuzzy.pyx` lines 81-185 per architecture.md
//! section 5.2, including every documented quirk (unicode-uppercase then
//! `[^A-Z]` strip, trailing S/Z strip, MAC/PF prefixes, suffix loop, the
//! empty-string `'' in 'AEIOU'` quirk, consecutive-duplicate collapse).

/// NYSIIS code of `s`.
///
/// Pinned signature (architecture.md section 4). Binding data points:
/// `nysiis("fuzzy") == "FASY"`, `nysiis("") == ""`, `nysiis("123") == ""`.
///
/// Scaffold stub: panics until the nysiis-port feature lands.
pub fn nysiis(_s: &str) -> String {
    unimplemented!("nysiis: ported by the nysiis-port feature (architecture.md section 5.2)")
}

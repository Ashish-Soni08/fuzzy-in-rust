//! fuzzy-py: PyO3 0.23 adapter exposing the fuzzy-core port as the Python
//! module `fuzzy` (architecture.md section 6).
//!
//! API contract:
//! - `Soundex(size)` — callable class, `__call__(str) -> str`. Negative size
//!   raises `ValueError`; a missing size or non-str argument raises
//!   `TypeError` (natural PyO3 extraction).
//! - `DMetaphone(size=0)` — callable class, `__call__(str) ->
//!   list[bytes | None]`. `size=0` is unlimited; negative size raises
//!   `ValueError`; non-ASCII input raises `UnicodeEncodeError` (original
//!   Double Metaphone behavior preserved, architecture.md section 2).
//! - `nysiis(str) -> str`.
//!
//! FFI safety invariants are fully encapsulated inside PyO3's macro
//! expansions; this crate adds no handwritten raw blocks of its own
//! (hackathon discipline rule, verified by the VAL-PAR-017 sweep).

use pyo3::exceptions::{PyUnicodeEncodeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};

/// Soundex encoder with the FIXED semantics of architecture.md section 5.1
/// (upstream bugs #14/#15 fixed: `size > 4` never zero-pads, non-ASCII input
/// is unicode-uppercased then filtered to A-Z instead of raising).
///
/// Construct with a `size`, then call the instance with a string:
/// `Soundex(4)('fuzzy') == 'F200'`. `size <= 4` right-pads with `'0'` to
/// exactly `size`; `size > 4` is a maximum length only; `size == 0` yields
/// the empty string.
#[pyclass(name = "Soundex", module = "fuzzy")]
struct Soundex {
    size: usize,
}

#[pymethods]
impl Soundex {
    #[new]
    fn new(size: isize) -> PyResult<Self> {
        // Defensive validation (architecture.md section 6): the original took
        // a C int with no checks (UB on negatives); the port rejects them.
        let size = usize::try_from(size)
            .map_err(|_| PyValueError::new_err("size must be non-negative"))?;
        Ok(Self { size })
    }

    fn __call__(&self, s: &str) -> String {
        fuzzy_core::soundex(self.size, s)
    }
}

/// Double Metaphone encoder, exact port of the original C algorithm
/// (architecture.md section 5.3).
///
/// `DMetaphone(size=0)`; calling the instance with an ASCII string returns a
/// two-element list `[primary, secondary]` of `bytes` or `None` (secondary
/// is `None` when it equals the primary; an empty code is `None`). `size=0`
/// means unlimited (the core caps codes at 4 chars); sizes 1-3 truncate.
/// Non-ASCII input raises `UnicodeEncodeError`, as the original did.
#[pyclass(name = "DMetaphone", module = "fuzzy")]
struct DMetaphone {
    size: usize,
}

#[pymethods]
impl DMetaphone {
    #[new]
    #[pyo3(signature = (size = 0))]
    fn new(size: isize) -> PyResult<Self> {
        let size = usize::try_from(size)
            .map_err(|_| PyValueError::new_err("size must be non-negative"))?;
        Ok(Self { size })
    }

    fn __call__<'py>(&self, py: Python<'py>, s: &str) -> PyResult<Bound<'py, PyList>> {
        let (primary, secondary) = fuzzy_core::dmetaphone_with_size(self.size, s)
            .map_err(|e| unicode_encode_error(s, &e))?;
        // Type fidelity (VAL-PAR-009): a real list of bytes / None.
        let out = PyList::empty(py);
        for code in [primary, secondary] {
            match code {
                Some(bytes) => out.append(PyBytes::new(py, &bytes))?,
                None => out.append(py.None())?,
            }
        }
        Ok(out)
    }
}

/// Map fuzzy-core's `NonAsciiError` onto the exception the original Cython
/// wrapper raised: `UnicodeEncodeError('ascii', s, start, end, 'ordinal not
/// in range(128)')`. Every character before the offending one is ASCII, so
/// the byte position equals the character index Python expects.
fn unicode_encode_error(s: &str, e: &fuzzy_core::NonAsciiError) -> PyErr {
    let start = e.byte_position();
    PyUnicodeEncodeError::new_err((
        "ascii",
        s.to_string(),
        start,
        start + 1,
        "ordinal not in range(128)",
    ))
}

/// NYSIIS code of `s` — exact port including quirks (architecture.md
/// section 5.2): unicode-uppercase then strip non-A-Z, `nysiis('') == ''`.
#[pyfunction]
fn nysiis(s: &str) -> String {
    fuzzy_core::nysiis(s)
}

/// The Python module. Its name MUST be `fuzzy` — the original test suite
/// does `import fuzzy`.
#[pymodule]
fn fuzzy(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Soundex>()?;
    m.add_class::<DMetaphone>()?;
    m.add_function(wrap_pyfunction!(nysiis, m)?)?;
    Ok(())
}

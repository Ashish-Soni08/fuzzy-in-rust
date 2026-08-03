# coding: utf-8
"""Port-team tests for the Rust port of `fuzzy` (Port Mortem 2026, Track D).

These are NEW tests written by the port team. They are NOT the preserved
original suite — that lives untouched in tests/original/ (byte-identical,
SHA-256 pinned at kickoff, see tests/original/KICKOFF.md). This suite
exercises the installed Python module `fuzzy` (the PyO3 adapter over the
Rust core, installed into the venv with `maturin develop`) and pins:

* the two fixed upstream bugs — yougov/fuzzy#14 (Soundex padding semantics)
  and yougov/fuzzy#15 (Soundex non-ASCII input raising UnicodeEncodeError);
* anchor data points shared with the original README/tests;
* DMetaphone return-type fidelity (list of bytes-or-None);
* error fidelity (ValueError / TypeError / UnicodeEncodeError);
* the size-0 edge cases.

The native Rust suites live in rust/fuzzy-core/tests (145 tests) and
rust/fuzzy-cli/tests (20 tests) and run via `cargo test --workspace`.
See tests/port/README.md.
"""

import pytest

import fuzzy


# --- Fixed upstream bugs (#14, #15) ---------------------------------------


def test_soundex_size8_test_no_padding():
    # Bug #14: size is a maximum, not a pad target, when size > 4.
    # The original padded to the requested size ('T2300000'); the port
    # implements the intended semantics.
    assert fuzzy.Soundex(8)('Test') == 'T23'


def test_soundex_size8_non_ascii():
    # Bug #15: the original raised UnicodeEncodeError on non-ASCII input
    # (Cython c_string_encoding=ascii). The port unicode-uppercases, then
    # filters to A-Z, so 'é' is dropped without error.
    assert fuzzy.Soundex(8)('Jéroboam') == 'J615'


# --- Anchor data points ----------------------------------------------------


def test_soundex_anchor_fuzzy():
    assert fuzzy.Soundex(4)('fuzzy') == 'F200'


def test_nysiis_anchor_fuzzy():
    assert fuzzy.nysiis('fuzzy') == 'FASY'


def test_dmetaphone_mayer():
    assert fuzzy.DMetaphone()('mayer') == [b'MR', None]


# --- Type fidelity ---------------------------------------------------------


def test_dmetaphone_return_type_fidelity():
    # The wrapper returns a list of exactly two elements; each element is
    # bytes or None (never str, never a tuple).
    res = fuzzy.DMetaphone()('mayer')
    assert isinstance(res, list)
    assert len(res) == 2
    for element in res:
        assert element is None or isinstance(element, bytes)


def test_dmetaphone_distinct_secondary_is_bytes():
    # When primary and secondary differ, both are kept as bytes.
    res = fuzzy.DMetaphone()('czerny')
    assert res == [b'SRN', b'XRN']
    assert all(isinstance(element, bytes) for element in res)


def test_dmetaphone_empty_input_returns_none_pair():
    assert fuzzy.DMetaphone()('') == [None, None]


# --- Error fidelity --------------------------------------------------------


def test_soundex_negative_size_raises_value_error():
    with pytest.raises(ValueError):
        fuzzy.Soundex(-1)


def test_dmetaphone_negative_size_raises_value_error():
    with pytest.raises(ValueError):
        fuzzy.DMetaphone(-1)


def test_dmetaphone_non_ascii_raises_unicode_encode_error():
    # Original Double Metaphone behavior is preserved: ASCII-only input.
    with pytest.raises(UnicodeEncodeError):
        fuzzy.DMetaphone()('Jéroboam')


def test_soundex_missing_or_wrong_size_raises_type_error():
    with pytest.raises(TypeError):
        fuzzy.Soundex('fuzzy')  # a word where the int size belongs
    with pytest.raises(TypeError):
        fuzzy.Soundex()  # missing required size argument


# --- Size-0 edge cases -----------------------------------------------------


def test_soundex_size_zero_returns_empty_string():
    assert fuzzy.Soundex(0)('fuzzy') == ''


def test_dmetaphone_size_zero_is_unlimited():
    # size 0 means "no truncation" (the core's own 4-char cap still
    # applies): 'bcdfgh' keeps its full 4-char primary, while size 2
    # truncates it.
    assert fuzzy.DMetaphone(0)('bcdfgh') == [b'PKFK', None]
    assert fuzzy.DMetaphone(2)('bcdfgh') == [b'PK', None]

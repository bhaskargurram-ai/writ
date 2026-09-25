//! The policy packs bundled into the binary (see `build.rs`).

/// `(pack id, pack.yaml source)`, sorted by id.
pub(crate) const BUNDLED: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/bundled_packs.rs"));

/// The bundled source of `id`, if this build ships it.
pub(crate) fn bundled(id: &str) -> Option<&'static str> {
    BUNDLED
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, src)| *src)
}

/// The ids of every bundled pack, comma-separated (for messages).
pub(crate) fn names() -> String {
    BUNDLED
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

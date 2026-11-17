use crate::routing::{Preset, StemKind, StemRef};

/// Detect a UVR5 Vocal Split import and return the matching preset.
///
/// A split set must contain lead vocals, background vocals, drums, bass, and
/// either an "Other" or "Instrumental" stem. A redundant combined `(Vocals)`
/// stem may also be present; the preset intentionally leaves it unrouted.
pub fn vocal_split_preset(stems: &[StemRef]) -> Option<Preset> {
    let has = |kind: StemKind| stems.iter().any(|stem| stem.kind() == kind);
    (has(StemKind::LeadVocals)
        && has(StemKind::BackingVocals)
        && has(StemKind::Drums)
        && has(StemKind::Bass)
        && (has(StemKind::Other) || has(StemKind::Instrumental)))
    .then_some(Preset::VocalSplit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stems(names: &[&str]) -> Vec<StemRef> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| StemRef::new(index as u64 + 1, *name))
            .collect()
    }

    #[test]
    fn detects_five_and_six_stem_vocal_split_sets() {
        let five = stems(&[
            "Song (Lead Vocals).flac",
            "Song (Background Vocals).flac",
            "Song (Drums).flac",
            "Song (Bass).flac",
            "Song (Other).flac",
        ]);
        assert_eq!(vocal_split_preset(&five), Some(Preset::VocalSplit));

        let six = stems(&[
            "Song (Vocals).flac",
            "Song (Lead Vocals).flac",
            "Song (Background Vocals).flac",
            "Song (Drums).flac",
            "Song (Bass).flac",
            "Song (Other).flac",
        ]);
        assert_eq!(vocal_split_preset(&six), Some(Preset::VocalSplit));

        let instrumental = stems(&[
            "Song (Lead Vocals).wav",
            "Song (Harmony).wav",
            "Song (Drums).wav",
            "Song (Bass).wav",
            "Song (Instrumental).wav",
        ]);
        assert_eq!(vocal_split_preset(&instrumental), Some(Preset::VocalSplit));
    }

    #[test]
    fn does_not_auto_route_incomplete_or_combined_only_sets() {
        let combined_only = stems(&[
            "Song (Vocals).flac",
            "Song (Drums).flac",
            "Song (Bass).flac",
            "Song (Other).flac",
        ]);
        assert_eq!(vocal_split_preset(&combined_only), None);

        let missing_backing = stems(&[
            "Song (Lead Vocals).flac",
            "Song (Drums).flac",
            "Song (Bass).flac",
            "Song (Other).flac",
        ]);
        assert_eq!(vocal_split_preset(&missing_backing), None);
    }
}

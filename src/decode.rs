use anyhow::{bail, Result};

use crate::model::{ChannelLayout, MAX_CHANNELS};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceChannelOrder {
    #[default]
    Smpte51,
    FilmDts51,
}

impl SourceChannelOrder {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Smpte51 => "SMPTE 5.1: FL FR FC LFE SL SR",
            Self::FilmDts51 => "Film/DTS 5.1: FL FC FR SL SR LFE",
        }
    }
}

/// Remap an arbitrary 6-channel source into canonical SMPTE order.
pub fn remap_5_1(planes: &mut Vec<Vec<f32>>, order: SourceChannelOrder) {
    if planes.len() != 6 || order == SourceChannelOrder::Smpte51 {
        return;
    }
    let remapped: Vec<Vec<f32>> = [0usize, 2, 1, 5, 3, 4]
        .into_iter()
        .map(|index| std::mem::take(&mut planes[index]))
        .collect();
    *planes = remapped;
}

/// Fold canonical 7.1 planes into canonical 5.1:
/// SL += 0.707 * BL, SR += 0.707 * BR.
pub fn fold_71_to_51(planes: &[Vec<f32>]) -> Result<[Vec<f32>; MAX_CHANNELS]> {
    if planes.len() != 8 {
        bail!("7.1 fold needs 8 planes, got {}", planes.len());
    }
    let mut output: [Vec<f32>; MAX_CHANNELS] = std::array::from_fn(|_| Vec::new());
    output[..4].clone_from_slice(&planes[..4]);
    let mut sl = planes[6].clone();
    let mut sr = planes[7].clone();
    for (frame, sample) in sl.iter_mut().enumerate() {
        *sample += planes[4].get(frame).copied().unwrap_or(0.0) * std::f32::consts::FRAC_1_SQRT_2;
    }
    for (frame, sample) in sr.iter_mut().enumerate() {
        *sample += planes[5].get(frame).copied().unwrap_or(0.0) * std::f32::consts::FRAC_1_SQRT_2;
    }
    output[4] = sl;
    output[5] = sr;
    Ok(output)
}

/// Convert 5.1 or 7.1 source planes to a canonical 5.1 bus.
pub fn canonicalize_51(
    planes: Vec<Vec<f32>>,
    order: SourceChannelOrder,
) -> Result<[Vec<f32>; MAX_CHANNELS]> {
    match planes.len() {
        6 => {
            let mut planes = planes;
            remap_5_1(&mut planes, order);
            let mut output: [Vec<f32>; MAX_CHANNELS] = std::array::from_fn(|_| Vec::new());
            for (index, plane) in planes.into_iter().enumerate() {
                output[index] = plane;
            }
            Ok(output)
        }
        8 => fold_71_to_51(&planes),
        other => bail!("source has {other} channels; expected 5.1 or 7.1"),
    }
}

pub fn source_layout(channels: usize) -> Option<ChannelLayout> {
    ChannelLayout::from_channel_count(channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn film_dts_source_order_maps_into_smpte() {
        let mut planes: Vec<Vec<f32>> = (0..6).map(|index| vec![index as f32]).collect();
        remap_5_1(&mut planes, SourceChannelOrder::FilmDts51);
        assert_eq!(
            planes
                .iter()
                .map(|plane| plane[0] as usize)
                .collect::<Vec<_>>(),
            vec![0, 2, 1, 5, 3, 4]
        );
        let mut identity: Vec<Vec<f32>> = (0..6).map(|index| vec![index as f32]).collect();
        remap_5_1(&mut identity, SourceChannelOrder::Smpte51);
        assert_eq!(
            identity
                .iter()
                .map(|plane| plane[0] as usize)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn seven_one_fold_is_minus_3db_into_surrounds() {
        let planes: Vec<Vec<f32>> = (0..8).map(|index| vec![(index + 1) as f32; 4]).collect();
        let folded = fold_71_to_51(&planes).unwrap();
        assert_eq!(folded[0][0], 1.0);
        assert_eq!(folded[3][0], 4.0);
        assert!((folded[4][0] - (7.0 + 5.0 * std::f32::consts::FRAC_1_SQRT_2)).abs() < 1e-5);
        assert!((folded[5][0] - (8.0 + 6.0 * std::f32::consts::FRAC_1_SQRT_2)).abs() < 1e-5);
    }
}

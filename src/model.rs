use serde::{Deserialize, Serialize};

pub const MAX_CHANNELS: usize = 8;

/// Canonical storage order. Export order depends on [`ChannelLayout`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Fl,
    Fr,
    Fc,
    Lfe,
    Bl,
    Br,
    Sl,
    Sr,
}

impl Channel {
    pub const ALL: [Self; MAX_CHANNELS] = [
        Self::Fl,
        Self::Fr,
        Self::Fc,
        Self::Lfe,
        Self::Bl,
        Self::Br,
        Self::Sl,
        Self::Sr,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Fl => 0,
            Self::Fr => 1,
            Self::Fc => 2,
            Self::Lfe => 3,
            Self::Bl => 4,
            Self::Br => 5,
            Self::Sl => 6,
            Self::Sr => 7,
        }
    }

    pub const fn short(self) -> &'static str {
        match self {
            Self::Fl => "FL",
            Self::Fr => "FR",
            Self::Fc => "FC",
            Self::Lfe => "LFE",
            Self::Bl => "BL",
            Self::Br => "BR",
            Self::Sl => "SL",
            Self::Sr => "SR",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Fl => "Front Left",
            Self::Fr => "Front Right",
            Self::Fc => "Front Center",
            Self::Lfe => "Subwoofer",
            Self::Bl => "Back Left",
            Self::Br => "Back Right",
            Self::Sl => "Surround Left",
            Self::Sr => "Surround Right",
        }
    }

    pub const fn is_surround(self) -> bool {
        matches!(self, Self::Sl | Self::Sr)
    }

    pub const fn is_back(self) -> bool {
        matches!(self, Self::Bl | Self::Br)
    }
}

/// 5.1 keeps SMPTE order FL, FR, FC, LFE, SL, SR.
/// 7.1 uses FL, FR, FC, LFE, BL, BR, SL, SR for export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelLayout {
    #[default]
    #[serde(rename = "5.1")]
    Surround51,
    #[serde(rename = "7.1")]
    Surround71,
}

impl ChannelLayout {
    const SIX: [Channel; 6] = [
        Channel::Fl,
        Channel::Fr,
        Channel::Fc,
        Channel::Lfe,
        Channel::Sl,
        Channel::Sr,
    ];
    const EIGHT: [Channel; 8] = [
        Channel::Fl,
        Channel::Fr,
        Channel::Fc,
        Channel::Lfe,
        Channel::Bl,
        Channel::Br,
        Channel::Sl,
        Channel::Sr,
    ];

    pub const fn channels(self) -> &'static [Channel] {
        match self {
            Self::Surround51 => &Self::SIX,
            Self::Surround71 => &Self::EIGHT,
        }
    }

    pub const fn count(self) -> usize {
        match self {
            Self::Surround51 => 6,
            Self::Surround71 => 8,
        }
    }

    pub const fn mask(self) -> u32 {
        match self {
            Self::Surround51 => 0x003F,
            Self::Surround71 => 0x063F,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Surround51 => "5.1 Surround (6ch)",
            Self::Surround71 => "7.1 Surround (8ch)",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Surround51 => "5.1",
            Self::Surround71 => "7.1",
        }
    }

    pub const fn is_71(self) -> bool {
        matches!(self, Self::Surround71)
    }

    pub fn bus_index(self, channel: Channel) -> Option<usize> {
        self.channels()
            .iter()
            .position(|candidate| *candidate == channel)
    }

    pub fn contains(self, channel: Channel) -> bool {
        self.bus_index(channel).is_some()
    }

    pub const fn default_surround_delay_ms(self) -> f32 {
        match self {
            Self::Surround51 => 15.0,
            Self::Surround71 => 12.0,
        }
    }

    pub const fn default_back_delay_ms(self) -> f32 {
        22.0
    }

    pub fn from_channel_count(channels: usize) -> Option<Self> {
        match channels {
            6 => Some(Self::Surround51),
            8 => Some(Self::Surround71),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_order_and_masks_are_locked() {
        assert_eq!(
            ChannelLayout::Surround51.channels(),
            &[
                Channel::Fl,
                Channel::Fr,
                Channel::Fc,
                Channel::Lfe,
                Channel::Sl,
                Channel::Sr
            ]
        );
        assert_eq!(
            ChannelLayout::Surround71.channels(),
            &[
                Channel::Fl,
                Channel::Fr,
                Channel::Fc,
                Channel::Lfe,
                Channel::Bl,
                Channel::Br,
                Channel::Sl,
                Channel::Sr
            ]
        );
        assert_eq!(ChannelLayout::Surround51.mask(), 0x003F);
        assert_eq!(ChannelLayout::Surround71.mask(), 0x063F);
    }
}

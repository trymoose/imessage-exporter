/// Traditional text effect container
///
/// Message text may contain any number of traditional styles or one animation
///
/// Read more about text styles [here](https://www.apple.com/newsroom/2024/06/ios-18-makes-iphone-more-personal-capable-and-intelligent-than-ever/).
#[derive(Debug, PartialEq, Eq)]
pub enum Style {
    /// **Bold** styled text
    Bold,
    /// *Italic* styled text
    Italic,
    /// ~~Strikethrough~~ formatted text
    Strikethrough,
    /// <u>Underline</u> styled text
    Underline,
}

/// Animated text effect container
///
/// Message text may contain any number of traditional styles or one animation
///
/// Read more about text styles [here](https://www.apple.com/newsroom/2024/06/ios-18-makes-iphone-more-personal-capable-and-intelligent-than-ever/).
#[derive(Debug, PartialEq, Eq)]
pub enum Animation {
    Big,
    Small,
    Shake,
    Nod,
    Explode,
    Ripple,
    Bloom,
    Jitter,
}

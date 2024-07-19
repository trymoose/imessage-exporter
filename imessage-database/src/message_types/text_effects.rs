/// Text effect container
///
/// Message text may contain any number of traditional styles or one animation.
/// 
/// Read more about text styles [here](https://www.apple.com/newsroom/2024/06/ios-18-makes-iphone-more-personal-capable-and-intelligent-than-ever/).
#[derive(Debug, PartialEq, Eq)]
pub enum TextEffect {
    /// Default, unstyled text
    Default,
    /// A [mentioned](https://support.apple.com/guide/messages/mention-a-person-icht306ee34b/mac) contact in the conversation
    Mention,
    /// Traditional formatting styles
    Styles(Vec<Style>),
    /// Animation applied to the text
    Animated(Animation),
}

/// Traditional text effect container
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

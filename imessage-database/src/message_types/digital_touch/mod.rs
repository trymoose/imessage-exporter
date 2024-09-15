/*!
[Digital Touch](https://support.apple.com/guide/ipod-touch/send-a-digital-touch-effect-iph3fadba219/ios) messages are animated doodles, taps, fireballs, lips, heartbeats, and heartbreaks.
*/

pub use models::DigitalTouchMessage;

pub(crate) mod digital_touch_proto;
pub mod models;
pub mod tap;
pub mod sketch;

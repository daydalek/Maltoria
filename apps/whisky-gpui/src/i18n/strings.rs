//! Static string tables generated from `locales/*.toml` by `build.rs`.

use super::Locale;

include!(concat!(env!("OUT_DIR"), "/strings.rs"));

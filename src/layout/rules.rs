//! Rule matching helpers. Pure functions, no I/O.

use crate::config::Config;

/// Match result for a window class or layer name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleMatch {
    /// A force-rule matched; contains the forced XKB layout.
    Forced(String),
    /// No rule matched — use the default / per-window-memory logic.
    Free,
}

/// Evaluate force rules against a window class.
pub fn match_class(cfg: &Config, class: &str) -> RuleMatch {
    match cfg.force_layout.layout_for_class(class) {
        Some(layout) => RuleMatch::Forced(layout.to_owned()),
        None         => RuleMatch::Free,
    }
}

/// Evaluate force rules against a layer surface name.
pub fn match_layer(cfg: &Config, layer: &str) -> RuleMatch {
    match cfg.force_layout.layout_for_layer(layer) {
        Some(layout) => RuleMatch::Forced(layout.to_owned()),
        None         => RuleMatch::Free,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ForceLayoutConfig, ForceRule, KeyboardConfig};

    fn make_cfg(rules: Vec<ForceRule>) -> Config {
        Config {
            keyboard: KeyboardConfig {
                device:  "test".into(),
                layouts: vec!["us".into(), "ru".into(), "de".into()],
            },
            force_layout: ForceLayoutConfig { rules },
            ..Config::default()
        }
    }

    #[test]
    fn glob_exact() {
        let cfg = make_cfg(vec![ForceRule {
            layout:         "us".into(),
            apps:           vec!["nvim".into()],
            layers:         vec![],
            layer_contains: vec![],
        }]);
        assert_eq!(match_class(&cfg, "nvim"),   RuleMatch::Forced("us".into()));
        assert_eq!(match_class(&cfg, "NVIM"),   RuleMatch::Forced("us".into())); // case-insensitive
        assert_eq!(match_class(&cfg, "neovim"), RuleMatch::Free);
    }

    #[test]
    fn glob_star() {
        let cfg = make_cfg(vec![ForceRule {
            layout:         "us".into(),
            apps:           vec!["org.telegram.*".into()],
            layers:         vec![],
            layer_contains: vec![],
        }]);
        assert_eq!(match_class(&cfg, "org.telegram.desktop"), RuleMatch::Forced("us".into()));
        assert_eq!(match_class(&cfg, "org.telegram."),        RuleMatch::Forced("us".into()));
        assert_eq!(match_class(&cfg, "telegram"),             RuleMatch::Free);
    }

    #[test]
    fn layer_contains() {
        let cfg = make_cfg(vec![ForceRule {
            layout:         "us".into(),
            apps:           vec![],
            layers:         vec![],
            layer_contains: vec!["launcher".into()],
        }]);
        assert_eq!(match_layer(&cfg, "rofi-launcher"), RuleMatch::Forced("us".into()));
        assert_eq!(match_layer(&cfg, "rofi"),          RuleMatch::Free);
    }
}

//! `[pipeline]`: each stage's section, with its model and that model's
//! options, each defined by its stage. The app passes the section to
//! `Pipeline::new` and `set_config`, and a request may bring its own. An
//! invalid section fails to parse.

use serde::{Deserialize, Serialize};

use crate::{DetectorConfig, InpainterConfig, RecognizerConfig, TranslatorConfig};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[derive(specta::Type)]
pub struct PipelineConfig {
    pub detector: DetectorConfig,
    pub recognizer: RecognizerConfig,
    pub inpainter: InpainterConfig,
    pub translator: TranslatorConfig,
}

impl koharu_config::Section for PipelineConfig {
    const NAME: &'static str = "pipeline";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invalid_section_fails_to_parse() {
        let parse = |section: serde_json::Value| serde_json::from_value::<PipelineConfig>(section);
        assert!(parse(serde_json::json!({"detector": {"text": 0.3}})).is_ok());
        assert!(parse(serde_json::json!({"detector": {"text": 1.5}})).is_err());
        assert!(parse(serde_json::json!({"translator": {"target": "xx"}})).is_err());
    }

    #[test]
    fn text_colour_preferences_preserve_defaults_and_round_trip() {
        let default: PipelineConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(default.detector.detect_text_color, None);
        assert_eq!(default.detector.default_text_color, None);

        let config = PipelineConfig {
            detector: DetectorConfig {
                detect_text_color: Some(false),
                default_text_color: Some(koharu_scene::Rgba([0.0, 0.0, 0.0, 1.0])),
                ..Default::default()
            },
            ..Default::default()
        };
        let serialized = serde_json::to_string(&config).unwrap();
        let restored: PipelineConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored, config);
    }
}

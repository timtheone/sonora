use crate::config::{AppSettings, ModelProfile};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HardwareTier {
    Low,
    Mid,
    High,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProfileTuning {
    pub min_chunk_samples: usize,
    pub partial_cadence_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelStatus {
    pub profile: ModelProfile,
    pub hardware_tier: HardwareTier,
    pub model_path: String,
    pub model_exists: bool,
    pub checked_paths: Vec<String>,
    pub tuning: ProfileTuning,
}

pub fn detect_hardware_tier(logical_cores: usize) -> HardwareTier {
    match logical_cores {
        0..=4 => HardwareTier::Low,
        5..=8 => HardwareTier::Mid,
        _ => HardwareTier::High,
    }
}

pub fn recommended_profile_for_tier(tier: HardwareTier) -> ModelProfile {
    match tier {
        HardwareTier::Low => ModelProfile::Fast,
        HardwareTier::Mid | HardwareTier::High => ModelProfile::Balanced,
    }
}

pub fn tuning_for_profile(profile: ModelProfile) -> ProfileTuning {
    match profile {
        ModelProfile::Fast => ProfileTuning {
            min_chunk_samples: 1024,
            partial_cadence_ms: 250,
        },
        ModelProfile::Balanced => ProfileTuning {
            min_chunk_samples: 2048,
            partial_cadence_ms: 400,
        },
    }
}

pub fn default_model_relative_path(profile: ModelProfile) -> &'static str {
    match profile {
        ModelProfile::Fast => "models/ggml-tiny.en-q8_0.bin",
        ModelProfile::Balanced => "models/ggml-base.en-q5_1.bin",
    }
}

pub fn resolve_model_candidates(
    settings: &AppSettings,
    resource_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let default_relative = default_model_relative_path(settings.model_profile);
    let default_file_name = Path::new(default_relative)
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_default();

    let mut candidates = Vec::<PathBuf>::new();

    if let Some(path) = &settings.model_path {
        let override_path = PathBuf::from(path);
        candidates.push(override_path.clone());

        if override_path.is_relative() {
            candidates.push(PathBuf::from("src-tauri/resources").join(&override_path));
            if let Some(resources) = resource_dir {
                candidates.push(resources.join(&override_path));
                candidates.push(resources.join("resources").join(&override_path));
            }
        }
    }

    candidates.push(PathBuf::from(default_relative));

    candidates.push(PathBuf::from("src-tauri/resources").join(default_relative));

    if let Some(resources) = resource_dir {
        candidates.push(resources.join(default_relative));
        candidates.push(resources.join("resources").join(default_relative));
        candidates.push(resources.join("models").join(&default_file_name));
        candidates.push(resources.join(&default_file_name));
    }

    dedupe_paths(candidates)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::<String>::new();
    paths
        .into_iter()
        .filter(|path| {
            let key = path.to_string_lossy().to_string();
            seen.insert(key)
        })
        .collect()
}

pub fn resolve_model_path(settings: &AppSettings, resource_dir: Option<&Path>) -> PathBuf {
    let candidates = resolve_model_candidates(settings, resource_dir);
    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }

    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from(default_model_relative_path(settings.model_profile)))
}

pub fn build_model_status(
    settings: &AppSettings,
    logical_cores: usize,
    resource_dir: Option<&Path>,
) -> ModelStatus {
    let hardware_tier = detect_hardware_tier(logical_cores);
    let checked_paths = resolve_model_candidates(settings, resource_dir)
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let model_path = resolve_model_path(settings, resource_dir);

    ModelStatus {
        profile: settings.model_profile,
        hardware_tier,
        model_path: model_path.to_string_lossy().to_string(),
        model_exists: model_path.exists(),
        checked_paths,
        tuning: tuning_for_profile(settings.model_profile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppSettings, ModelProfile};

    #[test]
    fn hardware_tier_mapping_prefers_fast_for_low_spec() {
        assert_eq!(detect_hardware_tier(2), HardwareTier::Low);
        assert_eq!(recommended_profile_for_tier(HardwareTier::Low), ModelProfile::Fast);
    }

    #[test]
    fn hardware_tier_mapping_prefers_balanced_for_mid_high() {
        assert_eq!(detect_hardware_tier(6), HardwareTier::Mid);
        assert_eq!(detect_hardware_tier(12), HardwareTier::High);
        assert_eq!(
            recommended_profile_for_tier(HardwareTier::Mid),
            ModelProfile::Balanced
        );
        assert_eq!(
            recommended_profile_for_tier(HardwareTier::High),
            ModelProfile::Balanced
        );
    }

    #[test]
    fn resolves_default_model_path_from_profile() {
        let settings = AppSettings {
            model_profile: ModelProfile::Fast,
            model_path: None,
            ..AppSettings::default()
        };

        assert_eq!(
            resolve_model_path(&settings, None).to_string_lossy(),
            "models/ggml-tiny.en-q8_0.bin"
        );
    }

    #[test]
    fn uses_explicit_model_path_when_set() {
        let settings = AppSettings {
            model_profile: ModelProfile::Balanced,
            model_path: Some("C:/models/custom.bin".to_string()),
            ..AppSettings::default()
        };

        assert_eq!(
            resolve_model_path(&settings, None).to_string_lossy(),
            "C:/models/custom.bin"
        );

        let candidates = resolve_model_candidates(&settings, None);
        assert!(candidates.len() > 1);
    }

    #[test]
    fn includes_resource_candidate_when_provided() {
        let settings = AppSettings {
            model_profile: ModelProfile::Balanced,
            model_path: None,
            ..AppSettings::default()
        };

        let candidates = resolve_model_candidates(&settings, Some(Path::new("/app/resources")));
        assert!(candidates
            .iter()
            .any(|path| path == &PathBuf::from("/app/resources/models/ggml-base.en-q5_1.bin")));
    }

    #[test]
    fn returns_tuning_values_for_profiles() {
        let fast = tuning_for_profile(ModelProfile::Fast);
        let balanced = tuning_for_profile(ModelProfile::Balanced);

        assert!(fast.min_chunk_samples < balanced.min_chunk_samples);
        assert!(fast.partial_cadence_ms < balanced.partial_cadence_ms);
    }
}

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Ready,
    NeedsSetup,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentHealth {
    pub os: String,
    pub session_type: SessionType,
    pub input_injection_permission: PermissionState,
    pub notes: Vec<String>,
}

pub fn session_type_from_env(value: Option<&str>) -> SessionType {
    match value {
        Some(raw) if raw.eq_ignore_ascii_case("x11") => SessionType::X11,
        Some(raw) if raw.eq_ignore_ascii_case("wayland") => SessionType::Wayland,
        _ => SessionType::Unknown,
    }
}

pub fn detect_environment_health() -> EnvironmentHealth {
    let os = std::env::consts::OS.to_string();
    let session_type = session_type_from_env(std::env::var("XDG_SESSION_TYPE").ok().as_deref());

    let (permission, mut notes) = permission_and_notes_for_os(&os, session_type);

    if session_type == SessionType::Wayland {
        notes.push(
            "Wayland may block global text injection; use X11 for full dictation support in v1."
                .to_string(),
        );
    }

    EnvironmentHealth {
        os,
        session_type,
        input_injection_permission: permission,
        notes,
    }
}

fn permission_and_notes_for_os(
    os: &str,
    session_type: SessionType,
) -> (PermissionState, Vec<String>) {
    match os {
        "macos" => (
            PermissionState::NeedsSetup,
            vec![
                "Grant Accessibility and Input Monitoring permissions for global input insertion."
                    .to_string(),
            ],
        ),
        "windows" => (
            PermissionState::Unknown,
            vec![
                "Input injection can fail for elevated/protected apps; run with matching integrity level."
                    .to_string(),
            ],
        ),
        "linux" => {
            if session_type == SessionType::X11 {
                (
                    PermissionState::Ready,
                    vec!["X11 session detected; global input path is supported in v1.".to_string()],
                )
            } else {
                (
                    PermissionState::NeedsSetup,
                    vec![
                        "Non-X11 session detected; switch to X11 for supported global insertion behavior."
                            .to_string(),
                    ],
                )
            }
        }
        _ => (
            PermissionState::Unknown,
            vec!["Unsupported OS for guaranteed v1 behavior.".to_string()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_session_type_values() {
        assert_eq!(session_type_from_env(Some("x11")), SessionType::X11);
        assert_eq!(session_type_from_env(Some("X11")), SessionType::X11);
        assert_eq!(session_type_from_env(Some("wayland")), SessionType::Wayland);
        assert_eq!(session_type_from_env(None), SessionType::Unknown);
    }

    #[test]
    fn linux_x11_marked_ready() {
        let (permission, notes) = permission_and_notes_for_os("linux", SessionType::X11);
        assert_eq!(permission, PermissionState::Ready);
        assert!(!notes.is_empty());
    }

    #[test]
    fn linux_non_x11_needs_setup() {
        let (permission, notes) = permission_and_notes_for_os("linux", SessionType::Wayland);
        assert_eq!(permission, PermissionState::NeedsSetup);
        assert!(!notes.is_empty());
    }
}

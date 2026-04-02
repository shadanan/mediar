use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub shows_dir: Option<PathBuf>,
    pub movies_dir: Option<PathBuf>,
}

fn config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("mediar")
        .join("config.json"))
}

pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&contents)?)
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(config)?;
    fs::write(&path, contents)?;
    Ok(())
}

pub fn resolve_target(
    explicit_target: Option<&str>,
    in_place: bool,
    source: &Path,
    configured_dir: Option<&PathBuf>,
) -> Result<PathBuf> {
    if let Some(target) = explicit_target {
        return Ok(PathBuf::from(target));
    }
    if let Some(dir) = configured_dir {
        return Ok(dir.clone());
    }
    if in_place {
        return source
            .parent()
            .map(|p| p.to_path_buf())
            .context("Failed to get parent directory of source");
    }
    Err(anyhow!(
        "No target directory specified. Provide a target argument, configure a default with \
         `mediar config`, or use --in-place."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_target_explicit() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let explicit = "/some/explicit/path";

        let result = resolve_target(Some(explicit), false, &source, None).unwrap();
        assert_eq!(result, PathBuf::from(explicit));
    }

    #[test]
    fn test_resolve_target_configured_shows() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let configured = PathBuf::from("/configured/Shows");

        let result = resolve_target(None, false, &source, Some(&configured)).unwrap();
        assert_eq!(result, configured);
    }

    #[test]
    fn test_resolve_target_explicit_takes_priority_over_configured() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let configured = PathBuf::from("/configured/Shows");
        let explicit = "/some/explicit/path";

        let result = resolve_target(Some(explicit), false, &source, Some(&configured)).unwrap();
        assert_eq!(result, PathBuf::from(explicit));
    }

    #[test]
    fn test_resolve_target_in_place() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");

        let result = resolve_target(None, true, &source, None).unwrap();
        assert_eq!(result, temp.path());
    }

    #[test]
    fn test_resolve_target_in_place_ignores_configured() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let configured = PathBuf::from("/configured/Shows");

        // --in-place should NOT override a configured directory; explicit > configured > in-place
        // When configured is set, it wins over in-place.
        let result = resolve_target(None, true, &source, Some(&configured)).unwrap();
        assert_eq!(result, configured);
    }

    #[test]
    fn test_resolve_target_no_target_errors() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");

        let result = resolve_target(None, false, &source, None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No target directory specified")
        );
    }

    #[test]
    fn test_config_default_is_empty() {
        let config = Config::default();
        assert!(config.shows_dir.is_none());
        assert!(config.movies_dir.is_none());
    }

    #[test]
    fn test_save_and_load_config() {
        let temp = TempDir::new().unwrap();
        // Override HOME to point to our temp dir so save/load use an isolated path.
        // We test the serialisation round-trip directly instead.
        let config = Config {
            shows_dir: Some(PathBuf::from("/media/Shows")),
            movies_dir: Some(PathBuf::from("/media/Movies")),
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.shows_dir, Some(PathBuf::from("/media/Shows")));
        assert_eq!(restored.movies_dir, Some(PathBuf::from("/media/Movies")));

        // Verify partial config (only shows set) also round-trips cleanly.
        let partial = Config {
            shows_dir: Some(PathBuf::from("/media/Shows")),
            movies_dir: None,
        };
        let json2 = serde_json::to_string_pretty(&partial).unwrap();
        let restored2: Config = serde_json::from_str(&json2).unwrap();
        assert_eq!(restored2.shows_dir, Some(PathBuf::from("/media/Shows")));
        assert!(restored2.movies_dir.is_none());

        drop(temp);
    }
}

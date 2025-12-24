use anyhow::{Context, Result};
use regex::Regex;
use std::{collections::HashSet, path::Path};

pub fn episode_id(season: i32, episode: i32) -> String {
    format!("S{:02}E{:02}", season, episode)
}

pub fn parse_ext(path: &Path) -> Option<String> {
    if path.is_dir() {
        return None;
    }

    let Some(ext) = path.extension() else {
        return None;
    };

    let Some(ext) = ext.to_str() else {
        return None;
    };

    let ext = ext.to_lowercase();

    let video_extensions = ["mp4", "mkv", "avi", "mov", "flv", "wmv", "webm"]
        .into_iter()
        .map(|ext| ext.to_string())
        .collect::<HashSet<_>>();
    if !video_extensions.contains(&ext) {
        return None;
    }

    Some(ext)
}

pub fn parse_season_episode(path: &Path) -> Result<String> {
    let file_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .context("Invalid filename")?;

    let re = Regex::new(r"[Ss](\d+)[Ee](\d+)")?;

    let caps = re
        .captures(file_name)
        .context("No season/episode pattern found")?;

    let season = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .context("Invalid season number")?;

    let episode = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .context("Invalid episode number")?;

    Ok(format!("S{:02}E{:02}", season, episode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_video_format_with_valid_extensions() {
        assert_eq!(parse_ext(Path::new("video.mp4")).as_deref(), Some("mp4"));
        assert_eq!(parse_ext(Path::new("movie.mkv")).as_deref(), Some("mkv"));
        assert_eq!(parse_ext(Path::new("film.avi")).as_deref(), Some("avi"));
        assert_eq!(parse_ext(Path::new("clip.mov")).as_deref(), Some("mov"));
        assert_eq!(parse_ext(Path::new("stream.flv")).as_deref(), Some("flv"));
        assert_eq!(parse_ext(Path::new("file.wmv")).as_deref(), Some("wmv"));
        assert_eq!(parse_ext(Path::new("web.webm")).as_deref(), Some("webm"));
    }

    #[test]
    fn test_get_video_format_with_invalid_extensions() {
        assert_eq!(parse_ext(Path::new("image.jpg")), None);
        assert_eq!(parse_ext(Path::new("document.txt")), None);
        assert_eq!(parse_ext(Path::new("audio.mp3")), None);
        assert_eq!(parse_ext(Path::new("archive.zip")), None);
    }

    #[test]
    fn test_get_video_format_with_directory() {
        assert_eq!(parse_ext(Path::new("some_directory/")), None);
    }

    #[test]
    fn test_get_video_format_with_no_extension() {
        assert_eq!(parse_ext(Path::new("noextension")), None);
    }

    #[test]
    fn test_get_video_format_with_uppercase_extension() {
        assert_eq!(parse_ext(Path::new("video.MP4")), Some("mp4".to_string()));
    }

    #[test]
    fn test_get_video_format_with_multiple_dots() {
        assert_eq!(
            parse_ext(Path::new("my.video.file.mp4")),
            Some("mp4".to_string())
        );
    }

    #[test]
    fn test_parse_season_episode_valid_pattern() {
        let result = parse_season_episode(Path::new("show_s01e05.mkv"));
        assert_eq!(result.unwrap(), "S01E05");
    }

    #[test]
    fn test_parse_season_episode_uppercase_pattern() {
        let result = parse_season_episode(Path::new("Series_S10E23.mp4"));
        assert_eq!(result.unwrap(), "S10E23");
    }

    #[test]
    fn test_parse_season_episode_mixed_case() {
        let result = parse_season_episode(Path::new("show_s02E15.avi"));
        assert_eq!(result.unwrap(), "S02E15");
    }

    #[test]
    fn test_parse_season_episode_no_pattern() {
        let result = parse_season_episode(Path::new("video.mp4"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_season_episode_invalid_numbers() {
        let result = parse_season_episode(Path::new("show_saXebX.mkv"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_season_episode_with_text() {
        let result = parse_season_episode(Path::new("The Show s03e07 The Episode Name.mp4"));
        assert_eq!(result.unwrap(), "S03E07");
    }
}

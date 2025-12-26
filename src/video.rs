use anyhow::{Context, Result};
use regex::Regex;
use std::{collections::HashSet, path::Path};

pub fn episode_id(season: i32, episode: i32) -> String {
    format!("S{:02}E{:02}", season, episode)
}

/// Extract the title from a filename by removing metadata patterns
/// Returns the cleaned title as a string
pub fn extract_title(path: &Path) -> Option<String> {
    let file_name = path.file_stem().and_then(|name| name.to_str())?;

    // Patterns that indicate the start of metadata (case insensitive)
    let metadata_patterns = [
        r"[Ss]\d+",
        r"[Ee]\d+",
        r"\d{4}",
        r"\d{3,4}p",
        r"(?i)(bluray|brrip|webrip|web-dl|hdtv|dvdrip|xvid|x264|x265|h264|h265)",
        r"(?i)(proper|repack|internal|limited|unrated|extended|directors.cut)",
        r"\[.*?\]",
    ];

    let combined_pattern = metadata_patterns.join("|");
    let re = Regex::new(&combined_pattern).ok()?;

    // Find the first match of any metadata pattern
    let title_end = re
        .find(file_name)
        .map(|m| m.start())
        .unwrap_or(file_name.len());

    // Extract the title portion
    let title = &file_name[..title_end];

    // Clean up the title: replace dots, underscores, and multiple spaces with single space
    let cleaned = title.replace(['.', '_', '-'], " ");

    // Trim and collapse multiple spaces
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

pub fn is_tv_show(path: &Path) -> bool {
    parse_season_episode(path).is_ok()
}

pub fn parse_ext(path: &Path) -> Option<String> {
    if path.is_dir() {
        return None;
    }

    let ext = path.extension()?.to_str()?.to_lowercase();

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

    #[test]
    fn test_episode_id() {
        assert_eq!(episode_id(1, 1), "S01E01");
        assert_eq!(episode_id(5, 12), "S05E12");
        assert_eq!(episode_id(10, 99), "S10E99");
    }

    #[test]
    fn test_parse_season_episode_complex_filename() {
        let result = parse_season_episode(Path::new(
            "[Group] Show Name - s02e15 - Episode Title [1080p].mkv",
        ));
        assert_eq!(result.unwrap(), "S02E15");
    }

    #[test]
    fn test_parse_season_episode_with_year() {
        let result = parse_season_episode(Path::new("Show.2024.s01e03.720p.mp4"));
        assert_eq!(result.unwrap(), "S01E03");
    }

    #[test]
    fn test_parse_ext_case_insensitive() {
        assert_eq!(parse_ext(Path::new("video.MP4")), Some("mp4".to_string()));
        assert_eq!(parse_ext(Path::new("video.MKV")), Some("mkv".to_string()));
        assert_eq!(parse_ext(Path::new("video.AVI")), Some("avi".to_string()));
    }

    #[test]
    fn test_parse_ext_with_path() {
        assert_eq!(
            parse_ext(Path::new("/path/to/video.mp4")),
            Some("mp4".to_string())
        );
        assert_eq!(
            parse_ext(Path::new("relative/path/video.mkv")),
            Some("mkv".to_string())
        );
    }

    #[test]
    fn test_extract_title_simple() {
        assert_eq!(
            extract_title(Path::new("Movie Name.mkv")),
            Some("Movie Name".to_string())
        );
    }

    #[test]
    fn test_extract_title_with_season_episode() {
        assert_eq!(
            extract_title(Path::new("Show.Title.S01E01.720p.mkv")),
            Some("Show Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_with_year() {
        assert_eq!(
            extract_title(Path::new("Movie.Title.1999.1080p.BluRay.mkv")),
            Some("Movie Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_with_quality() {
        assert_eq!(
            extract_title(Path::new("Movie_Title_BluRay_1080p.mkv")),
            Some("Movie Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_with_brackets() {
        assert_eq!(
            extract_title(Path::new("Show Name [1080p].mkv")),
            Some("Show Name".to_string())
        );
    }

    #[test]
    fn test_is_tv_show() {
        assert!(is_tv_show(Path::new("Show.S01E01.mkv")));
        assert!(is_tv_show(Path::new("series_s02e10.mp4")));
        assert!(!is_tv_show(Path::new("Movie.2020.mkv")));
        assert!(!is_tv_show(Path::new("Film.Name.1080p.mp4")));
    }
}

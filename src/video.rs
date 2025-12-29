use anyhow::{Context, Result};
use core::fmt;
use regex::Regex;
use std::{collections::HashSet, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Show,
    Movie,
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Show => "TV Show",
            ContentType::Movie => "Movie",
        }
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn episode_id(season: i32, episode: i32) -> String {
    format!("S{:02}E{:02}", season, episode)
}

/// Extract the title from a filename by removing metadata patterns
/// Returns the cleaned title as a string
pub fn parse_title(path: &Path) -> Option<String> {
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
        r"\(.*?\)",
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

    let cleaned = title
        .replace(['.', '_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

pub fn parse_content_type(path: &Path) -> ContentType {
    if parse_episode_id(path).is_ok() {
        ContentType::Show
    } else {
        ContentType::Movie
    }
}

pub fn parse_extension(path: &Path) -> Option<String> {
    if path.is_dir() {
        return None;
    }

    let ext = path.extension()?.to_str()?.to_lowercase();

    let allowed_formats = ["mp4", "mkv", "avi", "mov", "flv", "wmv", "webm", "srt"]
        .into_iter()
        .map(|ext| ext.to_string())
        .collect::<HashSet<_>>();
    if !allowed_formats.contains(&ext) {
        return None;
    }

    Some(ext)
}

pub fn parse_episode_id(path: &Path) -> Result<String> {
    let path_str = path.to_string_lossy();

    let season_regex = Regex::new(r"[Ss](?:eason)?[._\-\s]*(\d+)")?;
    let season_match = season_regex
        .captures_iter(&path_str)
        .last()
        .context("Failed to extract season number")?
        .get(1)
        .context("Failed to extract season number")?;

    let episode_regex = Regex::new(r"(?:[Ee](?:pisode)?\s*|\b)(\d{1,2})(?:[._\-]|\b)")?;
    let episode_match = episode_regex
        .captures_at(&path_str, season_match.end())
        .context("Failed to extract episode number")?
        .get(1)
        .context("Failed to extract episode number")?;

    Ok(format!(
        "S{:02}E{:02}",
        season_match.as_str().parse::<i32>()?,
        episode_match.as_str().parse::<i32>()?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episode_id() {
        assert_eq!(episode_id(1, 1), "S01E01");
        assert_eq!(episode_id(5, 12), "S05E12");
        assert_eq!(episode_id(10, 99), "S10E99");
    }

    #[test]
    fn test_parse_title_simple() {
        assert_eq!(
            parse_title(Path::new("Movie Name.mkv")),
            Some("Movie Name".to_string())
        );
    }

    #[test]
    fn test_parse_title_with_season_episode() {
        assert_eq!(
            parse_title(Path::new("Show.Title.S01E01.720p.mkv")),
            Some("Show Title".to_string())
        );
    }

    #[test]
    fn test_parse_title_with_year() {
        assert_eq!(
            parse_title(Path::new("Movie.Title.1999.1080p.BluRay.mkv")),
            Some("Movie Title".to_string())
        );
    }

    #[test]
    fn test_parse_title_with_quality() {
        assert_eq!(
            parse_title(Path::new("Movie_Title_BluRay_1080p.mkv")),
            Some("Movie Title".to_string())
        );
    }

    #[test]
    fn test_parse_title_with_brackets() {
        assert_eq!(
            parse_title(Path::new("Show Name [1080p].mkv")),
            Some("Show Name".to_string())
        );
    }

    #[test]
    fn test_parse_content_type() {
        assert_eq!(
            parse_content_type(Path::new("Show.S01E01.mkv")),
            ContentType::Show
        );
        assert_eq!(
            parse_content_type(Path::new("show_s02e10.mp4")),
            ContentType::Show
        );
        assert_eq!(
            parse_content_type(Path::new("Movie.2020.mkv")),
            ContentType::Movie
        );
        assert_eq!(
            parse_content_type(Path::new("Film.1080p.mp4")),
            ContentType::Movie
        );
    }

    #[test]
    fn test_parse_extension_with_valid_extensions() {
        assert_eq!(
            parse_extension(Path::new("video.mp4")).as_deref(),
            Some("mp4")
        );
        assert_eq!(
            parse_extension(Path::new("movie.mkv")).as_deref(),
            Some("mkv")
        );
        assert_eq!(
            parse_extension(Path::new("film.avi")).as_deref(),
            Some("avi")
        );
        assert_eq!(
            parse_extension(Path::new("clip.mov")).as_deref(),
            Some("mov")
        );
        assert_eq!(
            parse_extension(Path::new("stream.flv")).as_deref(),
            Some("flv")
        );
        assert_eq!(
            parse_extension(Path::new("file.wmv")).as_deref(),
            Some("wmv")
        );
        assert_eq!(
            parse_extension(Path::new("web.webm")).as_deref(),
            Some("webm")
        );
    }

    #[test]
    fn test_parse_extension_with_invalid_extensions() {
        assert_eq!(parse_extension(Path::new("image.jpg")), None);
        assert_eq!(parse_extension(Path::new("document.txt")), None);
        assert_eq!(parse_extension(Path::new("audio.mp3")), None);
        assert_eq!(parse_extension(Path::new("archive.zip")), None);
    }

    #[test]
    fn test_parse_extension_with_directory() {
        assert_eq!(parse_extension(Path::new("some_directory/")), None);
    }

    #[test]
    fn test_parse_extension_with_no_extension() {
        assert_eq!(parse_extension(Path::new("noextension")), None);
    }

    #[test]
    fn test_parse_extension_with_uppercase_extension() {
        assert_eq!(
            parse_extension(Path::new("video.MP4")),
            Some("mp4".to_string())
        );
    }

    #[test]
    fn test_parse_extension_with_multiple_dots() {
        assert_eq!(
            parse_extension(Path::new("my.video.file.mp4")),
            Some("mp4".to_string())
        );
    }

    #[test]
    fn test_parse_extension_case_insensitive() {
        assert_eq!(
            parse_extension(Path::new("video.MP4")),
            Some("mp4".to_string())
        );
        assert_eq!(
            parse_extension(Path::new("video.MKV")),
            Some("mkv".to_string())
        );
        assert_eq!(
            parse_extension(Path::new("video.AVI")),
            Some("avi".to_string())
        );
    }

    #[test]
    fn test_parse_extension_with_path() {
        assert_eq!(
            parse_extension(Path::new("/path/to/video.mp4")),
            Some("mp4".to_string())
        );
        assert_eq!(
            parse_extension(Path::new("relative/path/video.mkv")),
            Some("mkv".to_string())
        );
    }

    #[test]
    fn test_parse_episode_id_valid_pattern() {
        let result = parse_episode_id(Path::new("show_s01e05.mkv"));
        assert_eq!(result.unwrap(), "S01E05");
    }

    #[test]
    fn test_parse_episode_id_uppercase_pattern() {
        let result = parse_episode_id(Path::new("Series_S10E23.mp4"));
        assert_eq!(result.unwrap(), "S10E23");
    }

    #[test]
    fn test_parse_episode_id_mixed_case() {
        let result = parse_episode_id(Path::new("show_s02E15.avi"));
        assert_eq!(result.unwrap(), "S02E15");
    }

    #[test]
    fn test_parse_episode_id_space_separated() {
        let result = parse_episode_id(Path::new("Show S02 E15.avi"));
        assert_eq!(result.unwrap(), "S02E15");
    }

    #[test]
    fn test_parse_episode_id_period_separated() {
        let result = parse_episode_id(Path::new("show.S02.E15.avi"));
        assert_eq!(result.unwrap(), "S02E15");
    }

    #[test]
    fn test_parse_episode_id_no_pattern() {
        let result = parse_episode_id(Path::new("video.mp4"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_episode_id_invalid_numbers() {
        let result = parse_episode_id(Path::new("show_saXebX.mkv"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_episode_id_with_text() {
        let result = parse_episode_id(Path::new("The Show s03e07 The Episode Name.mp4"));
        assert_eq!(result.unwrap(), "S03E07");
    }

    #[test]
    fn test_parse_episode_id_complex_filename() {
        let result = parse_episode_id(Path::new(
            "[Group] Show Name - s02e15 - Episode Title [1080p].mkv",
        ));
        assert_eq!(result.unwrap(), "S02E15");
    }

    #[test]
    fn test_parse_episode_id_with_year() {
        let result = parse_episode_id(Path::new("Show.2024.s01e03.720p.mp4"));
        assert_eq!(result.unwrap(), "S01E03");
    }

    #[test]
    fn test_parse_episode_id_from_directory() {
        let result = parse_episode_id(Path::new("Season 01/01 Pilot.mp4"));
        assert_eq!(result.unwrap(), "S01E01");
    }

    #[test]
    fn test_parse_episode_id_from_directory_short() {
        let result = parse_episode_id(Path::new("S02/05 Episode Name.mkv"));
        assert_eq!(result.unwrap(), "S02E05");
    }

    #[test]
    fn test_parse_episode_id_from_directory_with_metadata() {
        let result = parse_episode_id(Path::new(
            "Show.Season.01.720p.x264.AC3/Show.01.720p.x264.AC3.mkv",
        ));
        assert_eq!(result.unwrap(), "S01E01");
    }

    #[test]
    fn test_parse_episode_id_standalone_episode_with_dot() {
        let result = parse_episode_id(Path::new("Season.10/08.Episode.Title.mkv"));
        assert_eq!(result.unwrap(), "S10E08");
    }

    #[test]
    fn test_parse_episode_id_standalone_episode_with_dash() {
        let result = parse_episode_id(Path::new("Season-03/12-Episode-Title.mp4"));
        assert_eq!(result.unwrap(), "S03E12");
    }

    #[test]
    fn test_parse_episode_id_standalone_episode_with_underscore() {
        let result = parse_episode_id(Path::new("Season_03/12_Episode_Title.mp4"));
        assert_eq!(result.unwrap(), "S03E12");
    }

    #[test]
    fn test_parse_episode_id_prefers_filename_pattern() {
        // Should prefer S02E03 from filename over Season 01 from directory
        let result = parse_episode_id(Path::new("Season.01/Show.S02E03.mkv"));
        assert_eq!(result.unwrap(), "S02E03");
    }
}

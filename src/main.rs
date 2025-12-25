use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use mediar::{
    tmdb::{MovieSearchResult, Show, TmdbClient, TvSearchResult},
    video::{parse_ext, parse_season_episode},
};
use sanitize_filename::sanitize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tabled::{Table, Tabled, settings::Style};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
enum Mode {
    Move,
    Copy,
    Link,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Search for TV shows and movies
    Search {
        /// The search query
        query: String,
        /// Filter by language (e.g., en, es, fr)
        #[arg(long)]
        language: Option<String>,
        /// Filter by minimum popularity (default: 1.0)
        #[arg(long, default_value = "1.0")]
        min_popularity: f64,
    },
    /// Move files to the target directory
    Move {
        source: String,
        target: Option<String>,
        #[arg(long)]
        series_id: i32,
        #[arg(long)]
        dry_run: bool,
    },
    /// Copy files to the target directory
    Copy {
        source: String,
        target: Option<String>,
        #[arg(long)]
        series_id: i32,
        #[arg(long)]
        dry_run: bool,
    },
    /// Create hard links in the target directory
    Link {
        source: String,
        target: Option<String>,
        #[arg(long)]
        series_id: i32,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

fn organize(
    mode: Mode,
    source: &Path,
    target: Option<&Path>,
    show: &Show,
    dry_run: bool,
) -> Result<()> {
    let target = target
        .or_else(|| Path::parent(source))
        .context("Failed to determine target")?;

    let episodes = show.episodes();
    let title = sanitize(format!("{} ({})", show.name, show.year));

    let mut transactions: Vec<(PathBuf, PathBuf)> = Vec::new();

    for entry in WalkDir::new(source).sort_by_file_name() {
        let entry = entry?;
        let old = entry.path().to_path_buf();

        let Some(ext) = parse_ext(&old) else {
            continue;
        };

        let episode_id = match parse_season_episode(&old) {
            Ok(episode_id) => episode_id,
            Err(_) => {
                println!("Skip: {}", old.to_string_lossy().yellow());
                continue;
            }
        };

        let episode = episodes
            .get(&episode_id)
            .context(format!("Unable to get metadata for {:?}", episode_id))?;

        let new = target
            .to_path_buf()
            .join(&title)
            .join(format!("Season {:02}", episode.season_number))
            .join(sanitize(format!(
                "{} - {} - {}.{}",
                show.name, episode_id, episode.name, ext
            )));

        if old != new && !new.exists() {
            transactions.push((old, new));
        }
    }

    for (old, new) in transactions {
        let parent = new.parent().context("Failed to get parent")?;
        match mode {
            Mode::Copy => {
                println!("Copy: {}", old.to_string_lossy().blue());
                println!("↪ To: {}", new.to_string_lossy().blue());
                if !dry_run {
                    fs::create_dir_all(parent)?;
                    fs::copy(old, new)?;
                }
            }
            Mode::Move => {
                println!("Move: {}", old.to_string_lossy().red());
                println!("↪ To: {}", new.to_string_lossy().red());
                if !dry_run {
                    fs::create_dir_all(parent)?;
                    fs::rename(old, new)?;
                }
            }
            Mode::Link => {
                println!("Link: {}", old.to_string_lossy().green());
                println!("↪ To: {}", new.to_string_lossy().green());
                if !dry_run {
                    fs::create_dir_all(parent)?;
                    fs::hard_link(old, new)?;
                }
            }
        };
    }

    Ok(())
}

#[derive(Tabled)]
struct SearchResultDisplay {
    #[tabled(rename = "ID")]
    id: i32,
    #[tabled(rename = "")]
    r#type: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "🌐")]
    language: String,
    #[tabled(rename = "⭐")]
    popularity: String,
    #[tabled(rename = "Year")]
    year: String,
    #[tabled(rename = "TMDB Link")]
    link: String,
}

impl From<TvSearchResult> for SearchResultDisplay {
    fn from(result: TvSearchResult) -> Self {
        Self {
            id: result.id,
            r#type: "📺".to_string(),
            name: result.name,
            language: result
                .original_language
                .unwrap_or_else(|| "N/A".to_string()),
            popularity: result
                .popularity
                .map(|p| format!("{:.1}", p))
                .unwrap_or_else(|| "N/A".to_string()),
            year: result
                .first_air_date
                .as_ref()
                .and_then(|date| date.split('-').next().map(|s| s.to_string()))
                .unwrap_or_default(),
            link: format!("https://www.themoviedb.org/tv/{}", result.id),
        }
    }
}

impl From<MovieSearchResult> for SearchResultDisplay {
    fn from(result: MovieSearchResult) -> Self {
        Self {
            id: result.id,
            r#type: "🎬".to_string(),
            name: result.title,
            language: result
                .original_language
                .unwrap_or_else(|| "N/A".to_string()),
            popularity: result
                .popularity
                .map(|p| format!("{:.1}", p))
                .unwrap_or_else(|| "N/A".to_string()),
            year: result
                .release_date
                .as_ref()
                .and_then(|date| date.split('-').next().map(|s| s.to_string()))
                .unwrap_or_default(),
            link: format!("https://www.themoviedb.org/movie/{}", result.id),
        }
    }
}

fn filter_and_sort_search_results(
    results: Vec<SearchResultDisplay>,
    language: &Option<String>,
    min_popularity: f64,
    query: &str,
) -> Vec<SearchResultDisplay> {
    let query_lower = query.to_lowercase();

    let mut filtered: Vec<_> = results
        .into_iter()
        .filter(|result| {
            // Filter by language if specified
            let lang_match = language
                .as_ref()
                .map(|lang| &result.language == lang || result.language == "N/A")
                .unwrap_or(true);

            // Check if it's an exact match
            let is_exact_match = result.name.to_lowercase() == query_lower;

            // Filter by minimum popularity (skip this check for exact matches)
            let pop_match = if is_exact_match {
                true
            } else {
                result
                    .popularity
                    .parse::<f64>()
                    .map(|p| p >= min_popularity)
                    .unwrap_or(false)
            };

            lang_match && pop_match
        })
        .collect();

    // Sort by match type (exact, prefix, other) then by popularity within each group
    filtered.sort_by_key(|result| {
        let name_lower = result.name.to_lowercase();

        // Determine match type (0 = exact, 1 = prefix, 2 = other)
        let match_type = if name_lower == query_lower {
            0
        } else if name_lower.starts_with(&query_lower) {
            1
        } else {
            2
        };

        // Convert popularity to negative integer for descending sort
        let popularity = result.popularity.parse::<f64>().unwrap_or(0.0);
        let neg_popularity = -(popularity * 100.0) as i64;

        (match_type, neg_popularity)
    });

    filtered
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let args = Args::parse();

    let client = TmdbClient::new()?;

    match args.command {
        Commands::Search {
            query,
            language,
            min_popularity,
        } => {
            // Search both TV and movies in parallel
            let (tv_response, movie_response) =
                tokio::join!(client.search_tv(&query), client.search_movie(&query));

            let tv_response = tv_response?;
            let movie_response = movie_response?;

            // Convert all results to SearchResultDisplay
            let tv_results: Vec<SearchResultDisplay> = tv_response
                .results
                .into_iter()
                .map(SearchResultDisplay::from)
                .collect();

            let movie_results: Vec<SearchResultDisplay> = movie_response
                .results
                .into_iter()
                .map(SearchResultDisplay::from)
                .collect();

            // Combine all results
            let mut all_results = Vec::new();
            all_results.extend(tv_results);
            all_results.extend(movie_results);

            // Filter and sort combined results
            let filtered_results =
                filter_and_sort_search_results(all_results, &language, min_popularity, &query);

            if filtered_results.is_empty() {
                println!("No results found for: {}", query.yellow());
                return Ok(());
            }

            let total_results = tv_response.total_results + movie_response.total_results;

            let table = Table::new(&filtered_results)
                .with(Style::rounded())
                .to_string();
            println!("\n{}", table);
            println!(
                "\nFound {} results ({} TV, {} movies)",
                total_results, tv_response.total_results, movie_response.total_results
            );
            Ok(())
        }
        Commands::Move {
            source,
            target,
            series_id,
            dry_run,
        } => {
            let show = client.show(series_id).await?;
            let source = Path::new(&source);
            let target = target.as_ref().map(Path::new);
            organize(Mode::Move, source, target, &show, dry_run)
        }
        Commands::Copy {
            source,
            target,
            series_id,
            dry_run,
        } => {
            let show = client.show(series_id).await?;
            let source = Path::new(&source);
            let target = target.as_ref().map(Path::new);
            organize(Mode::Copy, source, target, &show, dry_run)
        }
        Commands::Link {
            source,
            target,
            series_id,
            dry_run,
        } => {
            let show = client.show(series_id).await?;
            let source = Path::new(&source);
            let target = target.as_ref().map(Path::new);
            organize(Mode::Link, source, target, &show, dry_run)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mediar::tmdb::{Show, TvSeason, TvSeasonEpisode};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_show() -> Show {
        Show {
            id: 42,
            name: "Show Name".to_string(),
            overview: "Test show".to_string(),
            year: 2008,
            first_air_date: "2008-01-20".to_string(),
            number_of_episodes: 4,
            number_of_seasons: 2,
            seasons: vec![
                TvSeason {
                    id: 1,
                    season_number: 1,
                    name: "Season 1".to_string(),
                    overview: "First season".to_string(),
                    air_date: "2008-01-20".to_string(),
                    episodes: vec![
                        TvSeasonEpisode {
                            id: 101,
                            season_number: 1,
                            episode_number: 1,
                            name: "One".to_string(),
                            overview: "Pilot".to_string(),
                            air_date: "2008-01-20".to_string(),
                        },
                        TvSeasonEpisode {
                            id: 102,
                            season_number: 1,
                            episode_number: 2,
                            name: "Two".to_string(),
                            overview: "Second episode".to_string(),
                            air_date: "2008-01-27".to_string(),
                        },
                    ],
                },
                TvSeason {
                    id: 2,
                    season_number: 2,
                    name: "Season 2".to_string(),
                    overview: "Second season".to_string(),
                    air_date: "2009-03-08".to_string(),
                    episodes: vec![
                        TvSeasonEpisode {
                            id: 201,
                            season_number: 2,
                            episode_number: 1,
                            name: "Three".to_string(),
                            overview: "Season 2 premiere".to_string(),
                            air_date: "2009-03-08".to_string(),
                        },
                        TvSeasonEpisode {
                            id: 202,
                            season_number: 2,
                            episode_number: 2,
                            name: "Four".to_string(),
                            overview: "Fourth episode".to_string(),
                            air_date: "2009-03-08".to_string(),
                        },
                    ],
                },
            ],
        }
    }

    fn test_episode_files() -> Vec<PathBuf> {
        vec![
            Path::new("s01").join("Show.S01E01.mkv").to_path_buf(),
            Path::new("s01").join("Show.S01E02.mp4").to_path_buf(),
            Path::new("s02").join("Show.S02E01.avi").to_path_buf(),
        ]
    }

    fn test_other_files() -> Vec<PathBuf> {
        vec![
            Path::new("readme.txt").to_path_buf(),
            Path::new("trailer.mp4").to_path_buf(),
            Path::new("s01").join("Show.S01E01.srt").to_path_buf(),
            Path::new("s02").join("Show.S02E01.thumb.jpg").to_path_buf(),
        ]
    }

    fn test_files() -> Vec<PathBuf> {
        [test_episode_files(), test_other_files()].concat()
    }

    fn create_test_files(base_dir: &Path, files: &[PathBuf]) {
        for file_name in files {
            let file_path = base_dir.join(file_name);
            fs::create_dir_all(file_path.parent().unwrap()).unwrap();
            fs::File::create(&file_path).unwrap();
        }
    }

    #[test]
    fn test_organize_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        fs::create_dir_all(&source).unwrap();
        create_test_files(&source, &test_files());

        let show = create_test_show();

        let result = organize(Mode::Move, &source, Some(&target), &show, true);

        assert!(
            result.is_ok(),
            "organize should succeed: {:?}",
            result.err()
        );

        for file_name in &test_files() {
            let original_path = source.join(file_name);
            assert!(
                original_path.exists(),
                "File should still exist in dry run mode: {:?}",
                file_name
            );
        }

        assert!(!target.exists(), "Target should not exist in dry run mode");
    }

    #[test]
    fn test_organize_copy() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        create_test_files(&source, &test_files());

        let show = create_test_show();

        let result = organize(Mode::Copy, &source, Some(&target), &show, false);

        assert!(
            result.is_ok(),
            "organize should succeed: {:?}",
            result.err()
        );

        for file_name in &test_files() {
            let original_path = source.join(file_name);
            assert!(
                original_path.exists(),
                "Original file should still exist after copy: {:?}",
                file_name
            );
        }

        let show_dir = target.join("Show Name (2008)");
        assert!(show_dir.exists(), "Show directory should exist");

        let season1_dir = show_dir.join("Season 01");
        assert!(season1_dir.exists(), "Season 1 directory should exist");

        let season2_dir = show_dir.join("Season 02");
        assert!(season2_dir.exists(), "Season 2 directory should exist");

        for expected_file in [
            season1_dir.join("Show Name - S01E01 - One.mkv"),
            season1_dir.join("Show Name - S01E02 - Two.mp4"),
            season2_dir.join("Show Name - S02E01 - Three.avi"),
        ] {
            assert!(
                expected_file.exists(),
                "Expected file should exist: {:?}",
                expected_file
            );
        }
    }

    #[test]
    fn test_organize_rename_inplace() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");

        create_test_files(&source, &test_files());

        let show = create_test_show();

        let result = organize(Mode::Move, &source, None, &show, false);

        assert!(
            result.is_ok(),
            "organize should succeed: {:?}",
            result.err()
        );

        for file_name in &test_episode_files() {
            let original_path = source.join(file_name);
            assert!(
                !original_path.exists(),
                "Original video file should be moved/renamed: {:?}",
                file_name
            );
        }

        for file_name in &test_other_files() {
            let original_path = source.join(file_name);
            assert!(
                original_path.exists(),
                "Regular file should remain untouched: {:?}",
                file_name
            );
        }

        let parent = temp_dir.path();
        let show_dir = parent.join("Show Name (2008)");
        assert!(show_dir.exists(), "Show directory should exist in parent");

        let season1_dir = show_dir.join("Season 01");
        let season2_dir = show_dir.join("Season 02");
        assert!(season1_dir.exists(), "Season 1 directory should exist");
        assert!(season2_dir.exists(), "Season 2 directory should exist");

        for expected_file in [
            season1_dir.join("Show Name - S01E01 - One.mkv"),
            season1_dir.join("Show Name - S01E02 - Two.mp4"),
            season2_dir.join("Show Name - S02E01 - Three.avi"),
        ] {
            assert!(
                expected_file.exists(),
                "Renamed file should exist: {:?}",
                expected_file
            );
        }
    }
}

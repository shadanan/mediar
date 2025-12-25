use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mediar::{
    tmdb::{Show, TmdbClient},
    video::{parse_ext, parse_season_episode},
};
use sanitize_filename::sanitize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

type Transaction = (PathBuf, PathBuf);

#[derive(Debug, Clone)]
enum Mode {
    Move,
    Copy,
    Link,
}

#[derive(Subcommand, Debug)]
enum Commands {
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

fn commit(mode: &Mode, old: &Path, new: &Path) -> Result<()> {
    let parent = new.parent().context("Failed to get parent")?;
    fs::create_dir_all(parent)?;
    match mode {
        Mode::Copy => {
            fs::copy(old, new)?;
        }
        Mode::Move => {
            fs::rename(old, new)?;
        }
        Mode::Link => {
            fs::hard_link(old, new)?;
        }
    };
    Ok(())
}

fn commit_all(mode: &Mode, transactions: Vec<Transaction>) -> Result<()> {
    for transaction in transactions {
        commit(mode, &transaction.0, &transaction.1)?;
    }
    Ok(())
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
                println!("Skip {:?}", old);
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
            println!("{:#?} -> {:#?}", old, new);
            transactions.push((old, new));
        }
    }

    if !dry_run {
        commit_all(&mode, transactions)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let args = Args::parse();

    let client = TmdbClient::new()?;

    match args.command {
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
    use mediar::tmdb::{Episode, Season, Show};
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
                Season {
                    id: 1,
                    season_number: 1,
                    name: "Season 1".to_string(),
                    overview: "First season".to_string(),
                    air_date: "2008-01-20".to_string(),
                    episodes: vec![
                        Episode {
                            id: 101,
                            season_number: 1,
                            episode_number: 1,
                            name: "One".to_string(),
                            overview: "Pilot".to_string(),
                            air_date: "2008-01-20".to_string(),
                        },
                        Episode {
                            id: 102,
                            season_number: 1,
                            episode_number: 2,
                            name: "Two".to_string(),
                            overview: "Second episode".to_string(),
                            air_date: "2008-01-27".to_string(),
                        },
                    ],
                },
                Season {
                    id: 2,
                    season_number: 2,
                    name: "Season 2".to_string(),
                    overview: "Second season".to_string(),
                    air_date: "2009-03-08".to_string(),
                    episodes: vec![
                        Episode {
                            id: 201,
                            season_number: 2,
                            episode_number: 1,
                            name: "Three".to_string(),
                            overview: "Season 2 premiere".to_string(),
                            air_date: "2009-03-08".to_string(),
                        },
                        Episode {
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

    fn test_video_files() -> Vec<PathBuf> {
        vec![
            Path::new("s01").join("Show.S01E01.mkv").to_path_buf(),
            Path::new("s01").join("Show.S01E02.mp4").to_path_buf(),
            Path::new("s02").join("Show.S02E01.avi").to_path_buf(),
        ]
    }

    fn test_regular_files() -> Vec<PathBuf> {
        vec![
            Path::new("readme.txt").to_path_buf(),
            Path::new("s01").join("Show.S01E01.srt").to_path_buf(),
            Path::new("s02").join("Show.S02E01.thumb.jpg").to_path_buf(),
        ]
    }

    fn test_files() -> Vec<PathBuf> {
        [test_video_files(), test_regular_files()].concat()
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

        for file_name in &test_video_files() {
            let original_path = source.join(file_name);
            assert!(
                !original_path.exists(),
                "Original video file should be moved/renamed: {:?}",
                file_name
            );
        }

        for file_name in &test_regular_files() {
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

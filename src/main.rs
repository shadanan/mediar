use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use mediar::{
    tmdb::TmdbClient,
    video::{parse_ext, parse_season_episode},
};
use sanitize_filename::sanitize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

type Transaction = (PathBuf, PathBuf);

#[derive(Debug, Clone, ValueEnum)]
enum Mode {
    Move,
    Copy,
    Link,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    source: String,

    target: Option<String>,

    #[arg(long, value_enum, default_value_t = Mode::Link)]
    mode: Mode,

    #[arg(long)]
    series_id: i32,

    #[arg(long)]
    dry_run: bool,
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

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv()?;
    let args = Args::parse();

    let source = Path::new(&args.source);
    let target = args
        .target
        .as_ref()
        .map(Path::new)
        .or_else(|| Path::parent(source))
        .context("Failed to determine target")?;

    let client = TmdbClient::new()?;
    let show = client.show(args.series_id).await?;
    let episodes = show.episodes();
    let title = sanitize(format!("{} ({})", show.name, show.year));

    let mut transactions: Vec<(PathBuf, PathBuf)> = Vec::new();

    for entry in WalkDir::new(source) {
        let entry = entry?;
        let old = entry.path().to_path_buf();

        let Some(ext) = parse_ext(&old) else {
            continue;
        };

        let episode_id = parse_season_episode(&old)?;
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

    if !args.dry_run {
        commit_all(&args.mode, transactions)?;
    }
    Ok(())
}

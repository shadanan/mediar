mod config;
mod tmdb;
mod video;

use crate::{
    config::{load_config, resolve_target, save_config},
    tmdb::{Movie, MovieSearchResult, Show, TmdbClient, TvSearchResult},
    video::{
        ContentType, parse_content_type, parse_episode_id, parse_extension, parse_subtitle_tag,
        parse_title,
    },
};
use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use colored::{ColoredString, Colorize};
use inquire::{Confirm, Select};
use sanitize_filename::sanitize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use tabled::{Table, Tabled, settings::Style};
use textwrap::{Options, termwidth, wrap};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
enum Mode {
    Move,
    Copy,
    Link,
}

enum Content {
    Show(Show),
    Movie(Movie),
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
    /// Configure default target directories and API token
    Config {
        /// Set the default shows directory
        #[arg(long)]
        shows: Option<String>,
        /// Set the default movies directory
        #[arg(long)]
        movies: Option<String>,
        /// Set the TMDB API token
        #[arg(long)]
        token: Option<String>,
    },
    /// Move files to the target directory
    Move {
        #[arg(required = true, num_args = 1..)]
        sources: Vec<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        tv_id: Option<i32>,
        #[arg(long)]
        movie_id: Option<i32>,
        /// Organize files in place (use source's parent as target)
        #[arg(long)]
        in_place: bool,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Copy files to the target directory
    Copy {
        #[arg(required = true, num_args = 1..)]
        sources: Vec<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        tv_id: Option<i32>,
        #[arg(long)]
        movie_id: Option<i32>,
        /// Organize files in place (use source's parent as target)
        #[arg(long)]
        in_place: bool,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Create hard links in the target directory
    Link {
        #[arg(required = true, num_args = 1..)]
        sources: Vec<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        tv_id: Option<i32>,
        #[arg(long)]
        movie_id: Option<i32>,
        /// Organize files in place (use source's parent as target)
        #[arg(long)]
        in_place: bool,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

fn print_wrapped(start: ColoredString, text: ColoredString) {
    let indent = " ".repeat(start.chars().count());
    let start = start.to_string();
    let text = text.to_string();
    let lines = wrap(
        &text,
        Options::new(termwidth())
            .initial_indent(&start)
            .subsequent_indent(&indent),
    );
    for line in lines {
        println!("{}", line);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

fn print_operations(mode: &Mode, operations: &[(PathBuf, PathBuf)]) -> Result<()> {
    const MAX_DISPLAY: usize = 10;

    for (index, (old, new)) in operations.iter().enumerate() {
        if index == MAX_DISPLAY
            && !Confirm::new("Show all operations?")
                .with_default(false)
                .prompt()?
        {
            break;
        }

        match mode {
            Mode::Copy => {
                print_wrapped("Copy ".clear(), old.to_string_lossy().dimmed().green());
                print_wrapped("  ↪  ".bold(), new.to_string_lossy().bold().green());
            }
            Mode::Move => {
                print_wrapped("Move ".clear(), old.to_string_lossy().dimmed().red());
                print_wrapped("  ↪  ".bold(), new.to_string_lossy().bold().red());
            }
            Mode::Link => {
                print_wrapped("Link ".clear(), old.to_string_lossy().dimmed().blue());
                print_wrapped("  ↪  ".bold(), new.to_string_lossy().bold().blue());
            }
        }
    }

    Ok(())
}

fn execute_operation(mode: &Mode, old: PathBuf, new: PathBuf) -> Result<()> {
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
    }
    Ok(())
}

fn confirm_operations(auto_confirm: bool) -> Result<bool> {
    if auto_confirm {
        return Ok(true);
    }

    Ok(Confirm::new("Proceed with operations?")
        .with_default(true)
        .prompt()?)
}

enum PendingEntry {
    Direct(PathBuf, PathBuf),
    Subtitle {
        old: PathBuf,
        dir: PathBuf,
        stem: String,
        language: String,
        variant: Option<String>,
    },
}

fn subtitle_filename(
    stem: &str,
    language: &str,
    variant: &Option<String>,
    version: usize,
) -> String {
    match (variant, version) {
        (Some(variant), 0) => format!("{stem}.{language}.{variant}.srt"),
        (None, 0) => format!("{stem}.{language}.srt"),
        (Some(variant), v) => format!("{stem}.{language}.{variant}.v{v}.srt"),
        (None, v) => format!("{stem}.{language}.v{v}.srt"),
    }
}

fn collect_operations<F>(source: &Path, mut base_builder: F) -> Result<Vec<(PathBuf, PathBuf)>>
where
    F: FnMut(&Path) -> Result<Option<(PathBuf, String)>>,
{
    let mut pending: Vec<PendingEntry> = Vec::new();

    for entry in WalkDir::new(source).sort_by_file_name() {
        let entry = entry?;
        let old = entry.path().to_path_buf();

        let Some(ext) = parse_extension(&old) else {
            continue;
        };

        let Some((dir, stem)) = base_builder(&old)? else {
            continue;
        };

        if ext == "srt" {
            let (language, variant) = parse_subtitle_tag(&old);
            pending.push(PendingEntry::Subtitle {
                old,
                dir,
                stem,
                language,
                variant,
            });
        } else {
            let new = dir.join(format!("{stem}.{ext}"));
            pending.push(PendingEntry::Direct(old, new));
        }
    }

    let mut groups: HashMap<(PathBuf, String, String, Option<String>), Vec<usize>> = HashMap::new();
    for (index, entry) in pending.iter().enumerate() {
        if let PendingEntry::Subtitle {
            dir,
            stem,
            language,
            variant,
            ..
        } = entry
        {
            groups
                .entry((dir.clone(), stem.clone(), language.clone(), variant.clone()))
                .or_default()
                .push(index);
        }
    }

    let mut operations: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut seen_outputs: HashSet<PathBuf> = HashSet::new();

    for (index, entry) in pending.into_iter().enumerate() {
        let (old, new) = match entry {
            PendingEntry::Direct(old, new) => (old, new),
            PendingEntry::Subtitle {
                old,
                dir,
                stem,
                language,
                variant,
            } => {
                let key = (dir.clone(), stem.clone(), language.clone(), variant.clone());
                let group = &groups[&key];
                let version = if group.len() > 1 {
                    group.iter().position(|&i| i == index).unwrap() + 1
                } else {
                    0
                };
                let new = dir.join(subtitle_filename(&stem, &language, &variant, version));
                (old, new)
            }
        };

        if old == new {
            continue;
        }
        if new.exists() {
            print_wrapped("Skip ".clear(), old.to_string_lossy().yellow());
            print_wrapped(
                "  ↪  ".bold(),
                format!("{} already exists", new.to_string_lossy())
                    .bold()
                    .yellow(),
            );
            continue;
        }
        if !seen_outputs.insert(new.clone()) {
            return Err(anyhow!(
                "Multiple input files map to the same output: {}",
                new.display()
            ));
        }
        operations.push((old, new));
    }

    Ok(operations)
}

fn execute_operations(
    mode: &Mode,
    operations: Vec<(PathBuf, PathBuf)>,
    auto_confirm: bool,
) -> Result<()> {
    if operations.is_empty() {
        println!("{} No files to process.", "✗".bold().yellow());
        return Ok(());
    }

    print_operations(mode, &operations)?;

    if !confirm_operations(auto_confirm)? {
        println!("{} Cancelled.", "✗".bold().yellow());
        return Ok(());
    }

    for (old, new) in operations {
        execute_operation(mode, old, new)?;
    }

    println!("{} Done.", "✓".bold().green());
    Ok(())
}

fn build_tv_operations(
    source: &Path,
    target: &Path,
    show: &Show,
) -> Result<Vec<(PathBuf, PathBuf)>> {
    let episodes = show.episodes();
    let title = sanitize(format!("{} ({})", show.name, show.year));

    collect_operations(source, |old| {
        let episode_id = match parse_episode_id(old) {
            Ok(episode_id) => episode_id,
            Err(err) => {
                print_wrapped("Skip ".clear(), old.to_string_lossy().yellow());
                print_wrapped("  ↪  ".bold(), err.to_string().bold().yellow());
                return Ok(None);
            }
        };

        let episode = episodes
            .get(&episode_id)
            .with_context(|| format!("Unable to get metadata for {episode_id:?}"))?;

        let dir = target
            .to_path_buf()
            .join(&title)
            .join(format!("Season {:02}", episode.season_number));
        let stem = sanitize(format!("{} - {} - {}", show.name, episode_id, episode.name));

        Ok(Some((dir, stem)))
    })
}

fn build_movie_operations(
    source: &Path,
    target: &Path,
    movie: &Movie,
) -> Result<Vec<(PathBuf, PathBuf)>> {
    let year = movie
        .release_date
        .split('-')
        .next()
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(0);

    let title = sanitize(format!("{} ({})", movie.title, year));

    collect_operations(source, |_old| {
        let dir = target.to_path_buf().join(&title);
        let stem = sanitize(format!("{} ({})", movie.title, year));

        Ok(Some((dir, stem)))
    })
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
    language: Option<&str>,
    min_popularity: f64,
    query: &str,
) -> Vec<SearchResultDisplay> {
    let query_lower = query.to_lowercase();

    let mut filtered: Vec<_> = results
        .into_iter()
        .filter(|result| {
            let lang_match = language
                .map(|lang| result.language == lang || result.language == "N/A")
                .unwrap_or(true);

            let is_exact_match = result.name.to_lowercase() == query_lower;
            let pop_match = is_exact_match
                || result
                    .popularity
                    .parse::<f64>()
                    .map(|p| p >= min_popularity)
                    .unwrap_or(false);

            lang_match && pop_match
        })
        .collect();

    const EXACT: i32 = 0;
    const PREFIX: i32 = 1;
    const OTHER: i32 = 2;
    filtered.sort_by_key(|result| {
        let name_lower = result.name.to_lowercase();

        let match_type = if name_lower == query_lower {
            EXACT
        } else if name_lower.starts_with(&query_lower) {
            PREFIX
        } else {
            OTHER
        };

        // Convert popularity to negative integer for descending sort
        let popularity = result.popularity.parse::<f64>().unwrap_or(0.0);
        let neg_popularity = -(popularity * 100.0) as i64;

        (match_type, neg_popularity)
    });

    filtered
}

fn select_from_results<T>(
    results: &[T],
    prompt: &str,
    no_results_msg: &str,
    format_option: impl Fn(&T) -> String,
    get_name: impl Fn(&T) -> &str,
    get_id: impl Fn(&T) -> i32,
) -> Result<i32> {
    if results.is_empty() {
        return Err(anyhow!("{}", no_results_msg));
    }

    let options: Vec<String> = results.iter().map(&format_option).collect();
    let selection = Select::new(prompt, options).with_page_size(10).prompt()?;

    let selected_index = results
        .iter()
        .position(|result| format_option(result) == selection)
        .context("Failed to find selected item")?;

    let selected_result = &results[selected_index];
    println!(
        "Selected: {} (ID: {})",
        get_name(selected_result).green(),
        get_id(selected_result)
    );

    Ok(get_id(selected_result))
}

async fn select_tv_show(client: &TmdbClient, query: &str) -> Result<Show> {
    let response = client.search_tv(query).await?;

    let id = select_from_results(
        &response.results,
        "Select a TV show:",
        &format!("No TV shows found for query: {}", query),
        |result| {
            let year = result
                .first_air_date
                .as_ref()
                .and_then(|date| date.split('-').next())
                .unwrap_or("N/A");
            format!(
                "{} ({}) - ID: {} - Popularity: {:.1}",
                result.name,
                year,
                result.id,
                result.popularity.unwrap_or(0.0)
            )
        },
        |result| &result.name,
        |result| result.id,
    )?;

    client.show(id).await
}

async fn select_movie(client: &TmdbClient, query: &str) -> Result<Movie> {
    let response = client.search_movie(query).await?;

    let id = select_from_results(
        &response.results,
        "Select a movie:",
        &format!("No movies found for query: {}", query),
        |result| {
            let year = result
                .release_date
                .as_ref()
                .and_then(|date| date.split('-').next())
                .unwrap_or("N/A");
            format!(
                "{} ({}) - ID: {} - Popularity: {:.1}",
                result.title,
                year,
                result.id,
                result.popularity.unwrap_or(0.0)
            )
        },
        |result| &result.title,
        |result| result.id,
    )?;

    client.movie(id).await
}

async fn auto_detect_and_select(client: &TmdbClient, source: &Path) -> Result<Content> {
    let mut sample_video: Option<PathBuf> = None;
    for entry in WalkDir::new(source).max_depth(3) {
        let entry = entry?;
        if parse_extension(entry.path()).is_some() {
            sample_video = Some(entry.path().to_path_buf());
            break;
        }
    }

    let sample_video = sample_video.context("No video files found in source directory")?;

    let detected_title = parse_title(&sample_video).unwrap_or_default();

    let detected_type = parse_content_type(&sample_video);

    let selected_type = Select::new("Search for:", vec![ContentType::Show, ContentType::Movie])
        .with_starting_cursor(if detected_type == ContentType::Show {
            0
        } else {
            1
        })
        .prompt()?;

    let title = inquire::Text::new(&format!("{} Title:", selected_type))
        .with_initial_value(&detected_title)
        .prompt()?;

    match selected_type {
        ContentType::Show => {
            let show = select_tv_show(client, &title).await?;
            Ok(Content::Show(show))
        }
        ContentType::Movie => {
            let movie = select_movie(client, &title).await?;
            Ok(Content::Movie(movie))
        }
    }
}

async fn run_operation(
    mode: Mode,
    sources: Vec<String>,
    target: Option<String>,
    tv_id: Option<i32>,
    movie_id: Option<i32>,
    in_place: bool,
    yes: bool,
) -> Result<()> {
    let config = load_config()?;
    let client = create_client(&config)?;
    let content = match (tv_id, movie_id) {
        (Some(id), None) => Content::Show(client.show(id).await?),
        (None, Some(id)) => Content::Movie(client.movie(id).await?),
        (Some(_), Some(_)) => return Err(anyhow!("Cannot specify both --tv-id and --movie-id")),
        (None, None) => auto_detect_and_select(&client, &PathBuf::from(&sources[0])).await?,
    };
    let mut all_operations = Vec::new();
    for source_str in &sources {
        let source = PathBuf::from(source_str);
        match &content {
            Content::Show(show) => {
                let t = resolve_target(
                    target.as_deref(),
                    in_place,
                    &source,
                    config.shows_dir.as_ref(),
                )?;
                all_operations.extend(build_tv_operations(&source, &t, show)?);
            }
            Content::Movie(movie) => {
                let t = resolve_target(
                    target.as_deref(),
                    in_place,
                    &source,
                    config.movies_dir.as_ref(),
                )?;
                all_operations.extend(build_movie_operations(&source, &t, movie)?);
            }
        }
    }
    execute_operations(&mode, all_operations, yes)
}

fn create_client(config: &crate::config::Config) -> Result<TmdbClient> {
    let token = config
        .tmdb_api_token
        .clone()
        .or_else(|| std::env::var("TMDB_API_TOKEN").ok())
        .context(
            "TMDB API token not set. Run `mediar config` to configure it, or set the \
             TMDB_API_TOKEN environment variable.",
        )?;
    TmdbClient::new(token)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let args = Args::parse();

    match args.command {
        Commands::Search {
            query,
            language,
            min_popularity,
        } => {
            let config = load_config()?;
            let client = create_client(&config)?;
            let (tv_response, movie_response) =
                tokio::join!(client.search_tv(&query), client.search_movie(&query));

            let tv_response = tv_response?;
            let movie_response = movie_response?;

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
            let all_results: Vec<_> = tv_results.into_iter().chain(movie_results).collect();

            let filtered_results = filter_and_sort_search_results(
                all_results,
                language.as_deref(),
                min_popularity,
                &query,
            );

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
        Commands::Config {
            shows,
            movies,
            token,
        } => {
            let mut config = load_config()?;

            if shows.is_none() && movies.is_none() && token.is_none() {
                // Interactive mode: present editable fields pre-populated with current values
                let shows_initial = config
                    .shows_dir
                    .as_deref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let movies_initial = config
                    .movies_dir
                    .as_deref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let token_initial = config.tmdb_api_token.clone().unwrap_or_default();

                let shows_input = inquire::Text::new("Shows directory:")
                    .with_initial_value(&shows_initial)
                    .prompt()?;
                let movies_input = inquire::Text::new("Movies directory:")
                    .with_initial_value(&movies_initial)
                    .prompt()?;
                let token_input = inquire::Text::new("TMDB API token:")
                    .with_initial_value(&token_initial)
                    .prompt()?;

                config.shows_dir = if shows_input.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(shows_input))
                };
                config.movies_dir = if movies_input.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(movies_input))
                };
                config.tmdb_api_token = if token_input.is_empty() {
                    None
                } else {
                    Some(token_input)
                };

                save_config(&config)?;
                println!("{} Config saved.", "✓".bold().green());
            } else {
                if let Some(path) = shows {
                    config.shows_dir = Some(PathBuf::from(path));
                }
                if let Some(path) = movies {
                    config.movies_dir = Some(PathBuf::from(path));
                }
                if let Some(t) = token {
                    config.tmdb_api_token = Some(t);
                }
                save_config(&config)?;
                println!("{} Config saved.", "✓".bold().green());
            }
            Ok(())
        }
        Commands::Move {
            sources,
            target,
            tv_id,
            movie_id,
            in_place,
            yes,
        } => run_operation(Mode::Move, sources, target, tv_id, movie_id, in_place, yes).await,
        Commands::Copy {
            sources,
            target,
            tv_id,
            movie_id,
            in_place,
            yes,
        } => run_operation(Mode::Copy, sources, target, tv_id, movie_id, in_place, yes).await,
        Commands::Link {
            sources,
            target,
            tv_id,
            movie_id,
            in_place,
            yes,
        } => run_operation(Mode::Link, sources, target, tv_id, movie_id, in_place, yes).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmdb::{Movie, Show, TvSeason, TvSeasonEpisode};
    use std::fs;
    use tempfile::TempDir;

    fn organize_tv(
        mode: Mode,
        source: &Path,
        target: &Path,
        show: &Show,
        auto_confirm: bool,
    ) -> Result<()> {
        execute_operations(
            &mode,
            build_tv_operations(source, target, show)?,
            auto_confirm,
        )
    }

    fn organize_movie(
        mode: Mode,
        source: &Path,
        target: &Path,
        movie: &Movie,
        auto_confirm: bool,
    ) -> Result<()> {
        execute_operations(
            &mode,
            build_movie_operations(source, target, movie)?,
            auto_confirm,
        )
    }

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
                    episodes: vec![
                        TvSeasonEpisode {
                            id: 101,
                            season_number: 1,
                            episode_number: 1,
                            name: "One".to_string(),
                            overview: "Pilot".to_string(),
                        },
                        TvSeasonEpisode {
                            id: 102,
                            season_number: 1,
                            episode_number: 2,
                            name: "Two".to_string(),
                            overview: "Second episode".to_string(),
                        },
                    ],
                },
                TvSeason {
                    id: 2,
                    season_number: 2,
                    name: "Season 2".to_string(),
                    overview: "Second season".to_string(),
                    episodes: vec![
                        TvSeasonEpisode {
                            id: 201,
                            season_number: 2,
                            episode_number: 1,
                            name: "Three".to_string(),
                            overview: "Season 2 premiere".to_string(),
                        },
                        TvSeasonEpisode {
                            id: 202,
                            season_number: 2,
                            episode_number: 2,
                            name: "Four".to_string(),
                            overview: "Fourth episode".to_string(),
                        },
                    ],
                },
            ],
        }
    }

    fn test_episode_files() -> Vec<PathBuf> {
        vec![
            Path::new("s01").join("Show.S01E01.mkv").to_path_buf(),
            Path::new("s01").join("Show.S01E01.srt").to_path_buf(),
            Path::new("s01").join("02.Show.mp4").to_path_buf(),
            Path::new("s02").join("Show.S02E01.avi").to_path_buf(),
        ]
    }

    fn test_other_files() -> Vec<PathBuf> {
        vec![
            Path::new("readme.txt").to_path_buf(),
            Path::new("trailer.mp4").to_path_buf(),
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
    fn test_organize_with_autoconfirm() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        fs::create_dir_all(&source).unwrap();
        create_test_files(&source, &test_files());

        let show = create_test_show();

        let result = organize_tv(Mode::Move, &source, &target, &show, true);

        assert!(
            result.is_ok(),
            "organize should succeed: {:?}",
            result.err()
        );

        // With auto-confirm, files should be moved
        for file_name in &test_episode_files() {
            let original_path = source.join(file_name);
            assert!(
                !original_path.exists(),
                "Original video file should be moved: {:?}",
                file_name
            );
        }

        let show_dir = target.join("Show Name (2008)");
        assert!(show_dir.exists(), "Show directory should exist");
    }

    #[test]
    fn test_organize_copy() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        create_test_files(&source, &test_files());

        let show = create_test_show();

        let result = organize_tv(Mode::Copy, &source, &target, &show, true);

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
            season1_dir.join("Show Name - S01E01 - One.en.srt"),
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

        let target = source.parent().unwrap();
        let result = organize_tv(Mode::Move, &source, target, &show, true);

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
            season1_dir.join("Show Name - S01E01 - One.en.srt"),
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

    #[test]
    fn test_organize_tv_numbered_subtitle_versions() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        let episode_files = vec![
            Path::new("Show.S01E01.1080p.BluRay.x265.mkv").to_path_buf(),
            Path::new("Subs/Show.S01E01.1080p.BluRay.x265/3_English.srt").to_path_buf(),
            Path::new("Subs/Show.S01E01.1080p.BluRay.x265/4_English.srt").to_path_buf(),
        ];

        create_test_files(&source, &episode_files);

        let show = create_test_show();

        let result = organize_tv(Mode::Copy, &source, &target, &show, true);
        assert!(
            result.is_ok(),
            "organize should succeed: {:?}",
            result.err()
        );

        let season1_dir = target.join("Show Name (2008)").join("Season 01");
        for expected_file in [
            season1_dir.join("Show Name - S01E01 - One.mkv"),
            season1_dir.join("Show Name - S01E01 - One.en.v1.srt"),
            season1_dir.join("Show Name - S01E01 - One.en.v2.srt"),
        ] {
            assert!(
                expected_file.exists(),
                "Expected file should exist: {:?}",
                expected_file
            );
        }
    }

    fn create_test_movie() -> Movie {
        Movie {
            id: 550,
            title: "Movie Name".to_string(),
            overview: "Movie description".to_string(),
            release_date: "1999-10-15".to_string(),
            original_language: "en".to_string(),
            popularity: 63.869,
        }
    }

    #[test]
    fn test_organize_movie_copy() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        let movie_files = vec![Path::new("Movie.Name.1080p.mkv").to_path_buf()];

        create_test_files(&source, &movie_files);

        let movie = create_test_movie();

        let result = organize_movie(Mode::Copy, &source, &target, &movie, true);

        assert!(
            result.is_ok(),
            "organize_movie should succeed: {:?}",
            result.err()
        );

        for file_name in &movie_files {
            let original_path = source.join(file_name);
            assert!(
                original_path.exists(),
                "Original file should still exist after copy: {:?}",
                file_name
            );
        }

        let movie_dir = target.join("Movie Name (1999)");
        assert!(movie_dir.exists(), "Movie directory should exist");

        let expected_file = movie_dir.join("Movie Name (1999).mkv");
        assert!(
            expected_file.exists(),
            "Movie file should exist in target: {:?}",
            expected_file
        );
    }

    #[test]
    fn test_organize_movie_multiple_files_different_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        let movie_files = vec![
            Path::new("Movie.Name.1999.mkv").to_path_buf(),
            Path::new("Movie.Name.1999.srt").to_path_buf(),
        ];

        create_test_files(&source, &movie_files);

        let movie = create_test_movie();

        let result = organize_movie(Mode::Copy, &source, &target, &movie, true);

        assert!(
            result.is_ok(),
            "organize_movie should succeed with multiple files of different extensions"
        );

        let movie_dir = target.join("Movie Name (1999)");
        assert!(movie_dir.exists(), "Movie directory should exist");

        let expected_mkv = movie_dir.join("Movie Name (1999).mkv");
        assert!(
            expected_mkv.exists(),
            "Movie mkv file should exist in target: {:?}",
            expected_mkv
        );

        let expected_srt = movie_dir.join("Movie Name (1999).en.srt");
        assert!(
            expected_srt.exists(),
            "Movie srt file should exist in target: {:?}",
            expected_srt
        );
    }

    #[test]
    fn test_organize_movie_multiple_subtitle_languages() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        let movie_files = vec![
            Path::new("Movie.Name.1999.mkv").to_path_buf(),
            Path::new("Movie.Name.1999.en.srt").to_path_buf(),
            Path::new("Movie.Name.1999.fr.srt").to_path_buf(),
        ];

        create_test_files(&source, &movie_files);

        let movie = create_test_movie();

        let result = organize_movie(Mode::Copy, &source, &target, &movie, true);
        assert!(
            result.is_ok(),
            "organize_movie should succeed: {:?}",
            result.err()
        );

        let movie_dir = target.join("Movie Name (1999)");
        for expected_file in [
            movie_dir.join("Movie Name (1999).mkv"),
            movie_dir.join("Movie Name (1999).en.srt"),
            movie_dir.join("Movie Name (1999).fr.srt"),
        ] {
            assert!(
                expected_file.exists(),
                "Expected file should exist: {:?}",
                expected_file
            );
        }
    }

    #[test]
    fn test_organize_movie_multiple_english_subtitle_variants() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        let movie_files = vec![
            Path::new("Movie.Name.1999.mkv").to_path_buf(),
            Path::new("Movie.Name.1999.en.srt").to_path_buf(),
            Path::new("Movie.Name.1999.en.sdh.srt").to_path_buf(),
        ];

        create_test_files(&source, &movie_files);

        let movie = create_test_movie();

        let result = organize_movie(Mode::Copy, &source, &target, &movie, true);
        assert!(
            result.is_ok(),
            "organize_movie should succeed: {:?}",
            result.err()
        );

        let movie_dir = target.join("Movie Name (1999)");
        for expected_file in [
            movie_dir.join("Movie Name (1999).mkv"),
            movie_dir.join("Movie Name (1999).en.srt"),
            movie_dir.join("Movie Name (1999).en.sdh.srt"),
        ] {
            assert!(
                expected_file.exists(),
                "Expected file should exist: {:?}",
                expected_file
            );
        }
    }

    #[test]
    fn test_organize_movie_duplicate_extension_fails() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        let movie_files = vec![
            Path::new("Movie.Name.1999.1080p.mkv").to_path_buf(),
            Path::new("Movie.Name.1999.720p.mkv").to_path_buf(),
        ];

        create_test_files(&source, &movie_files);

        let movie = create_test_movie();

        let result = organize_movie(Mode::Copy, &source, &target, &movie, true);

        assert!(
            result.is_err(),
            "organize_movie should fail when multiple files map to same output"
        );

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Multiple input files map to the same output"),
            "Error message should mention duplicate output: {}",
            error_msg
        );
    }

    #[test]
    fn test_organize_movie_with_autoconfirm() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        fs::create_dir_all(&source).unwrap();

        let movie_files = vec![Path::new("Movie.Name.1999.mkv").to_path_buf()];

        create_test_files(&source, &movie_files);

        let movie = create_test_movie();

        let result = organize_movie(Mode::Move, &source, &target, &movie, true);

        assert!(
            result.is_ok(),
            "organize_movie should succeed: {:?}",
            result.err()
        );

        // With auto-confirm, file should be moved
        for file_name in &movie_files {
            let original_path = source.join(file_name);
            assert!(
                !original_path.exists(),
                "File should be moved with auto-confirm: {:?}",
                file_name
            );
        }

        let movie_dir = target.join("Movie Name (1999)");
        assert!(movie_dir.exists(), "Movie directory should exist");
    }
}

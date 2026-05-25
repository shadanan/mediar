use anyhow::Result;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::video::episode_id;

const BASE_URL: &str = "https://api.themoviedb.org/3";

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Tv {
    pub id: i32,
    pub name: String,
    pub overview: String,
    pub first_air_date: String,
    pub number_of_episodes: i32,
    pub number_of_seasons: i32,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TvSeasonEpisode {
    pub id: i32,
    pub season_number: i32,
    pub episode_number: i32,
    pub name: String,
    pub overview: String,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TvSeason {
    pub id: i32,
    pub season_number: i32,
    pub name: String,
    pub overview: String,
    pub episodes: Vec<TvSeasonEpisode>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Show {
    pub id: i32,
    pub name: String,
    pub overview: String,
    pub year: i32,
    pub first_air_date: String,
    pub number_of_episodes: i32,
    pub number_of_seasons: i32,
    pub seasons: Vec<TvSeason>,
}

#[derive(Debug, Deserialize)]
struct EpisodeGroupResult {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct EpisodeGroupsResponse {
    pub results: Vec<EpisodeGroupResult>,
}

#[derive(Debug, Deserialize)]
struct EpisodeGroupEpisode {
    pub id: i32,
    pub name: String,
    pub overview: String,
    pub order: i32,
}

#[derive(Debug, Deserialize)]
struct EpisodeGroupSeason {
    pub order: i32,
    pub name: String,
    pub episodes: Vec<EpisodeGroupEpisode>,
}

#[derive(Debug, Deserialize)]
struct EpisodeGroupDetail {
    pub groups: Vec<EpisodeGroupSeason>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TvSearchResult {
    pub id: i32,
    pub name: String,
    pub overview: String,
    pub first_air_date: Option<String>,
    pub original_language: Option<String>,
    pub popularity: Option<f64>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TvSearchResponse {
    pub page: i32,
    pub results: Vec<TvSearchResult>,
    pub total_pages: i32,
    pub total_results: i32,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Movie {
    pub id: i32,
    pub title: String,
    pub overview: String,
    pub release_date: String,
    pub original_language: String,
    pub popularity: f64,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct MovieSearchResult {
    pub id: i32,
    pub title: String,
    pub overview: String,
    pub release_date: Option<String>,
    pub original_language: Option<String>,
    pub popularity: Option<f64>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct MovieSearchResponse {
    pub page: i32,
    pub results: Vec<MovieSearchResult>,
    pub total_pages: i32,
    pub total_results: i32,
}

trait ResponseExt {
    async fn decode<T: for<'de> Deserialize<'de>>(self) -> Result<T>;
}

impl ResponseExt for reqwest::Response {
    async fn decode<T: for<'de> Deserialize<'de>>(self) -> Result<T> {
        let url = self.url().to_string();
        let text = self.text().await?;
        serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize response from {url}: {e}:\n{text}"))
    }
}

impl Show {
    pub fn episodes(&self) -> HashMap<String, &TvSeasonEpisode> {
        self.seasons
            .iter()
            .flat_map(|season| {
                season.episodes.iter().map(move |episode| {
                    (
                        episode_id(season.season_number, episode.episode_number),
                        episode,
                    )
                })
            })
            .collect()
    }
}

fn seasons_from_episode_group(detail: EpisodeGroupDetail) -> Vec<TvSeason> {
    let (zero_groups, mut positive_groups): (Vec<_>, Vec<_>) =
        detail.groups.into_iter().partition(|g| g.order == 0);
    positive_groups.sort_by_key(|g| g.order);

    let make_season = |season_number: i32, group: EpisodeGroupSeason| {
        let episodes = group
            .episodes
            .into_iter()
            .map(|ep| TvSeasonEpisode {
                id: ep.id,
                season_number,
                episode_number: ep.order + 1,
                name: ep.name,
                overview: ep.overview,
            })
            .collect();
        TvSeason {
            id: season_number,
            season_number,
            name: group.name,
            overview: String::new(),
            episodes,
        }
    };

    let mut seasons: Vec<TvSeason> = zero_groups
        .into_iter()
        .map(|group| make_season(0, group))
        .collect();

    seasons.extend(
        positive_groups
            .into_iter()
            .enumerate()
            .map(|(i, group)| make_season((i + 1) as i32, group)),
    );

    seasons
}

pub struct TmdbClient {
    client: reqwest::Client,
    token: String,
}

impl TmdbClient {
    pub fn new(token: String) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            token,
        })
    }

    pub async fn show(&self, id: i32) -> Result<Show> {
        let (series, episode_groups) =
            futures::future::try_join(self.series(id), self.episode_groups(id)).await?;

        let seasons =
            if let Some(group) = episode_groups.results.iter().find(|g| g.name == "Seasons") {
                let detail = self.episode_group(&group.id).await?;
                seasons_from_episode_group(detail)
            } else {
                let mut seasons = try_join_all(
                    (1..=series.number_of_seasons)
                        .map(|season_number| self.season(id, season_number))
                        .collect::<Vec<_>>(),
                )
                .await?;
                if let Ok(season0) = self.season(id, 0).await
                    && !season0.episodes.is_empty()
                {
                    seasons.insert(0, season0);
                }
                seasons
            };

        let number_of_seasons = seasons.len() as i32;
        let number_of_episodes = seasons.iter().map(|s| s.episodes.len() as i32).sum();
        let year = series
            .first_air_date
            .split('-')
            .next()
            .and_then(|y| y.parse().ok())
            .unwrap_or(0);

        Ok(Show {
            id: series.id,
            name: series.name,
            overview: series.overview,
            year,
            first_air_date: series.first_air_date,
            number_of_episodes,
            number_of_seasons,
            seasons,
        })
    }

    async fn episode_groups(&self, id: i32) -> Result<EpisodeGroupsResponse> {
        self.client
            .get(format!("{}/tv/{}/episode_groups", BASE_URL, id))
            .bearer_auth(&self.token)
            .send()
            .await?
            .decode()
            .await
    }

    async fn episode_group(&self, group_id: &str) -> Result<EpisodeGroupDetail> {
        self.client
            .get(format!("{}/tv/episode_group/{}", BASE_URL, group_id))
            .bearer_auth(&self.token)
            .send()
            .await?
            .decode()
            .await
    }

    pub async fn series(&self, id: i32) -> Result<Tv> {
        self.client
            .get(format!("{}/tv/{}", BASE_URL, id))
            .bearer_auth(&self.token)
            .send()
            .await?
            .decode()
            .await
    }

    pub async fn season(&self, id: i32, season: i32) -> Result<TvSeason> {
        self.client
            .get(format!("{}/tv/{}/season/{}", BASE_URL, id, season))
            .bearer_auth(&self.token)
            .send()
            .await?
            .decode()
            .await
    }

    pub async fn search_tv(&self, query: &str) -> Result<TvSearchResponse> {
        self.client
            .get(format!("{}/search/tv", BASE_URL))
            .bearer_auth(&self.token)
            .query(&[("query", query)])
            .send()
            .await?
            .decode()
            .await
    }

    pub async fn search_movie(&self, query: &str) -> Result<MovieSearchResponse> {
        self.client
            .get(format!("{}/search/movie", BASE_URL))
            .bearer_auth(&self.token)
            .query(&[("query", query)])
            .send()
            .await?
            .decode()
            .await
    }

    pub async fn movie(&self, id: i32) -> Result<Movie> {
        self.client
            .get(format!("{}/movie/{}", BASE_URL, id))
            .bearer_auth(&self.token)
            .send()
            .await?
            .decode()
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episode_id_generation() {
        let episode_id = episode_id(1, 5);
        assert_eq!(episode_id, "S01E05");
    }

    #[test]
    fn test_episode_id_double_digits() {
        let episode_id = episode_id(10, 23);
        assert_eq!(episode_id, "S10E23");
    }

    #[test]
    fn test_episode_id_single_digit() {
        let episode_id = episode_id(2, 7);
        assert_eq!(episode_id, "S02E07");
    }

    #[test]
    fn test_show_episodes_mapping() {
        let show = Show {
            id: 1,
            name: "Test Show".to_string(),
            overview: "A test show".to_string(),
            year: 2020,
            first_air_date: "2020-01-01".to_string(),
            number_of_episodes: 2,
            number_of_seasons: 1,
            seasons: vec![TvSeason {
                id: 1,
                season_number: 1,
                name: "Season 1".to_string(),
                overview: "First season".to_string(),
                episodes: vec![
                    TvSeasonEpisode {
                        id: 1,
                        season_number: 1,
                        episode_number: 1,
                        name: "Pilot".to_string(),
                        overview: "First episode".to_string(),
                    },
                    TvSeasonEpisode {
                        id: 2,
                        season_number: 1,
                        episode_number: 2,
                        name: "Second Episode".to_string(),
                        overview: "Second episode".to_string(),
                    },
                ],
            }],
        };

        let episodes = show.episodes();
        assert_eq!(episodes.len(), 2);
        assert!(episodes.contains_key("S01E01"));
        assert!(episodes.contains_key("S01E02"));
        assert_eq!(episodes.get("S01E01").unwrap().name, "Pilot");
        assert_eq!(episodes.get("S01E02").unwrap().name, "Second Episode");
    }

    #[test]
    fn test_show_episodes_multiple_seasons() {
        let show = Show {
            id: 1,
            name: "Test Show".to_string(),
            overview: "A test show".to_string(),
            year: 2020,
            first_air_date: "2020-01-01".to_string(),
            number_of_episodes: 3,
            number_of_seasons: 2,
            seasons: vec![
                TvSeason {
                    id: 1,
                    season_number: 1,
                    name: "Season 1".to_string(),
                    overview: "First season".to_string(),
                    episodes: vec![TvSeasonEpisode {
                        id: 1,
                        season_number: 1,
                        episode_number: 1,
                        name: "Pilot".to_string(),
                        overview: "First episode".to_string(),
                    }],
                },
                TvSeason {
                    id: 2,
                    season_number: 2,
                    name: "Season 2".to_string(),
                    overview: "Second season".to_string(),
                    episodes: vec![
                        TvSeasonEpisode {
                            id: 2,
                            season_number: 2,
                            episode_number: 1,
                            name: "Season 2 Premiere".to_string(),
                            overview: "First episode of season 2".to_string(),
                        },
                        TvSeasonEpisode {
                            id: 3,
                            season_number: 2,
                            episode_number: 2,
                            name: "Episode 2".to_string(),
                            overview: "Second episode of season 2".to_string(),
                        },
                    ],
                },
            ],
        };

        let episodes = show.episodes();
        assert_eq!(episodes.len(), 3);
        assert!(episodes.contains_key("S01E01"));
        assert!(episodes.contains_key("S02E01"));
        assert!(episodes.contains_key("S02E02"));
    }

    #[test]
    fn test_show_episodes_empty() {
        let show = Show {
            id: 1,
            name: "Test Show".to_string(),
            overview: "A test show".to_string(),
            year: 2020,
            first_air_date: "2020-01-01".to_string(),
            number_of_episodes: 0,
            number_of_seasons: 0,
            seasons: vec![],
        };

        let episodes = show.episodes();
        assert_eq!(episodes.len(), 0);
    }
}

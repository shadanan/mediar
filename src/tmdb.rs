use anyhow::Result;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::video::episode_id;

const BASE_URL: &str = "https://api.themoviedb.org/3";

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Series {
    pub id: i32,
    pub name: String,
    pub overview: String,
    pub first_air_date: String,
    pub number_of_episodes: i32,
    pub number_of_seasons: i32,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Season {
    pub id: i32,
    pub season_number: i32,
    pub name: String,
    pub overview: String,
    pub air_date: String,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Episode {
    pub id: i32,
    pub season_number: i32,
    pub episode_number: i32,
    pub name: String,
    pub overview: String,
    pub air_date: String,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct SearchResult {
    pub id: i32,
    pub name: String,
    pub overview: String,
    pub first_air_date: Option<String>,
    pub original_language: Option<String>,
    pub popularity: Option<f64>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct SearchResponse {
    pub page: i32,
    pub results: Vec<SearchResult>,
    pub total_pages: i32,
    pub total_results: i32,
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
    pub seasons: Vec<Season>,
}

impl Show {
    pub fn episodes(&self) -> HashMap<String, &Episode> {
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

pub struct TmdbClient {
    client: reqwest::Client,
    token: String,
}

impl TmdbClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            token: std::env::var("TMDB_API_TOKEN")?,
        })
    }

    pub async fn show(&self, id: i32) -> Result<Show> {
        let series = self.series(id).await?;
        let seasons = try_join_all(
            (1..=series.number_of_seasons)
                .map(|season_number| self.season(id, season_number))
                .collect::<Vec<_>>(),
        )
        .await?;
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
            number_of_episodes: series.number_of_episodes,
            number_of_seasons: series.number_of_seasons,
            seasons,
        })
    }

    pub async fn series(&self, id: i32) -> Result<Series> {
        Ok(self
            .client
            .get(format!("{}/tv/{}", BASE_URL, id))
            .bearer_auth(&self.token)
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn season(&self, id: i32, season: i32) -> Result<Season> {
        Ok(self
            .client
            .get(format!("{}/tv/{}/season/{}", BASE_URL, id, season))
            .bearer_auth(&self.token)
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn search_tv(&self, query: &str) -> Result<SearchResponse> {
        Ok(self
            .client
            .get(format!("{}/search/tv", BASE_URL))
            .bearer_auth(&self.token)
            .query(&[("query", query)])
            .send()
            .await?
            .json()
            .await?)
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
            seasons: vec![Season {
                id: 1,
                season_number: 1,
                name: "Season 1".to_string(),
                overview: "First season".to_string(),
                air_date: "2020-01-01".to_string(),
                episodes: vec![
                    Episode {
                        id: 1,
                        season_number: 1,
                        episode_number: 1,
                        name: "Pilot".to_string(),
                        overview: "First episode".to_string(),
                        air_date: "2020-01-01".to_string(),
                    },
                    Episode {
                        id: 2,
                        season_number: 1,
                        episode_number: 2,
                        name: "Second Episode".to_string(),
                        overview: "Second episode".to_string(),
                        air_date: "2020-01-08".to_string(),
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
                Season {
                    id: 1,
                    season_number: 1,
                    name: "Season 1".to_string(),
                    overview: "First season".to_string(),
                    air_date: "2020-01-01".to_string(),
                    episodes: vec![Episode {
                        id: 1,
                        season_number: 1,
                        episode_number: 1,
                        name: "Pilot".to_string(),
                        overview: "First episode".to_string(),
                        air_date: "2020-01-01".to_string(),
                    }],
                },
                Season {
                    id: 2,
                    season_number: 2,
                    name: "Season 2".to_string(),
                    overview: "Second season".to_string(),
                    air_date: "2021-01-01".to_string(),
                    episodes: vec![
                        Episode {
                            id: 2,
                            season_number: 2,
                            episode_number: 1,
                            name: "Season 2 Premiere".to_string(),
                            overview: "First episode of season 2".to_string(),
                            air_date: "2021-01-01".to_string(),
                        },
                        Episode {
                            id: 3,
                            season_number: 2,
                            episode_number: 2,
                            name: "Episode 2".to_string(),
                            overview: "Second episode of season 2".to_string(),
                            air_date: "2021-01-08".to_string(),
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

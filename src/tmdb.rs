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
            year: year,
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
}

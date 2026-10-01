use serde::Deserialize;
use std::fmt;

/// One game's entry in `data/games.json`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub yaw: f64,
    /// Horizontal FOV at 16:9 when the game fixes it.
    #[serde(default)]
    pub default_hfov: Option<f64>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug)]
pub enum GamesError {
    Parse(serde_json::Error),
    BadYaw(String),
    DuplicateId(String),
}

impl fmt::Display for GamesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GamesError::Parse(e) => write!(f, "invalid games file: {e}"),
            GamesError::BadYaw(id) => write!(f, "game '{id}' has a non-positive yaw"),
            GamesError::DuplicateId(id) => write!(f, "game id '{id}' appears twice"),
        }
    }
}

impl std::error::Error for GamesError {}

/// Parses and validates a games file.
pub fn load(json: &str) -> Result<Vec<Game>, GamesError> {
    let games: Vec<Game> = serde_json::from_str(json).map_err(GamesError::Parse)?;
    for (i, g) in games.iter().enumerate() {
        if !(g.yaw.is_finite() && g.yaw > 0.0) {
            return Err(GamesError::BadYaw(g.id.clone()));
        }
        if games[..i].iter().any(|o| o.id == g.id) {
            return Err(GamesError::DuplicateId(g.id.clone()));
        }
    }
    Ok(games)
}

/// Case-insensitive lookup by id.
pub fn find<'a>(games: &'a [Game], id: &str) -> Option<&'a Game> {
    games.iter().find(|g| g.id.eq_ignore_ascii_case(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_file_is_valid() {
        let games = load(include_str!("../data/games.json")).unwrap();
        assert!(games.len() >= 10);
        assert!(find(&games, "CS2").is_some());
    }

    #[test]
    fn rejects_bad_yaw_and_duplicates() {
        assert!(matches!(
            load(r#"[{"id":"a","name":"A","yaw":0}]"#),
            Err(GamesError::BadYaw(_))
        ));
        assert!(matches!(
            load(r#"[{"id":"a","name":"A","yaw":1},{"id":"a","name":"B","yaw":2}]"#),
            Err(GamesError::DuplicateId(_))
        ));
        assert!(matches!(load("not json"), Err(GamesError::Parse(_))));
    }

    #[test]
    fn web_copy_matches_data_file() {
        let a = include_str!("../data/games.json");
        let b = include_str!("../web/games.json");
        assert_eq!(a, b, "run scripts/build-web.ps1");
    }
}

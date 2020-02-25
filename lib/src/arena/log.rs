use regex::Regex;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::BufRead;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetPlayerCardsV3Payload {
  pub id: u64,
  pub payload: HashMap<String, usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Log {
  pub collection: Option<GetPlayerCardsV3Payload>,
}

#[derive(Debug)]
pub enum LogError {
  BadPayload,
}

impl fmt::Display for LogError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "log error")
  }
}

impl Error for LogError {
  fn description(&self) -> &str {
    match self {
      &Self::BadPayload => "bad payload",
    }
  }

  fn source(&self) -> Option<&(dyn Error + 'static)> {
    None
  }
}

impl Log {
  pub fn from_str(log: &str) -> Result<Self, LogError> {
    lazy_static! {
        //https://regex101.com/r/OluNfe/3
        static ref GET_PLAYER_CARDS_V3_REGEX : Regex =
            Regex::new(r"^.*<== PlayerInventory.GetPlayerCardsV3\s?(?P<payload>.*)")
                .expect("Failed to compile GET_PLAYER_CARDS_V3_REGEX");
    }
    let cursor = std::io::Cursor::new(log);
    let lines_iter = cursor.lines().map(|l| l.unwrap());
    let mut collections: Vec<GetPlayerCardsV3Payload> = Vec::new();
    for line in lines_iter {
      if let Some(caps) = GET_PLAYER_CARDS_V3_REGEX.captures(&line) {
        let payload = &caps["payload"];
        if let Ok(payload) = serde_json::from_str(payload) {
          collections.push(payload);
        }
      }
    }
    Ok(Self {
      collection: collections.last().map(|c| c.clone()),
    })
  }
}

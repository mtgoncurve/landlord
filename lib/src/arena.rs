//! # Structures related to the downloaded game files and the log
//!

#[derive(Debug, Serialize, Deserialize)]
pub enum IsoCode {
    #[serde(rename = "en-US")]
    EnUS,
    #[serde(other)]
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataLoc {
    #[serde(rename = "isoCode")]
    pub iso_code: IsoCode,
    pub keys: Vec<DataKey>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataKey {
    pub id: u64,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataCard {
    grpid: u64,
    titleid: u64,
}

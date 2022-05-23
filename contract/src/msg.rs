use cosmwasm_std::CanonicalAddr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Color, Shape};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub entropy: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    StartMatch {
        entropy: u64,
        rows: u32,
        cols: u32,
    },
    RevealCard {
        entropy: u64,
        match_id: String,
        pos: (u32, u32),
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetPlayer {
        address: CanonicalAddr,
    },
    GetCard {
        match_id: String,
        row: u32,
        col: u32,
    },
    GetMatch {
        match_id: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PlayerResponse {
    pub matches: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CardResponse {
    pub shape: Shape,
    pub color: Color,
    pub pos: (u32, u32),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MatchResponse {
    pub size: (u32, u32),
    pub attempts: u32,
    pub cards: Vec<Vec<Option<CardResponse>>>,
}

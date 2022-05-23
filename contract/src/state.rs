use cosmwasm_std::{CanonicalAddr, HumanAddr, StdError, StdResult, Storage};
use cosmwasm_storage::{
    bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton,
    Singleton,
};
use rand::distributions::{Distribution, Standard};
use rand::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub static RANDOM_KEY: &[u8] = b"random";
pub static PLAYER_KEY: &[u8] = b"player";
pub static MATCH_KEY: &[u8] = b"match";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Random {
    pub seed: [u8; 32],
    pub counter: u32,
}

impl Random {
    pub fn empty() -> Self {
        Self {
            seed: [0u8; 32],
            counter: 0,
        }
    }

    pub fn input_entropy(&mut self, input: u64, sender: HumanAddr, block_height: u64) {
        let mut next_seed = self.seed.to_vec();
        next_seed.extend(input.to_be_bytes());
        next_seed.extend(sender.0.as_bytes());
        next_seed.extend(block_height.to_be_bytes());
        next_seed.extend(self.counter.to_be_bytes());
        self.seed = Sha256::digest(&next_seed).into();
        self.counter += 1;
    }
}

pub fn storage_random<S: Storage>(storage: &mut S) -> Singleton<S, Random> {
    singleton(storage, RANDOM_KEY)
}

pub fn storage_random_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, Random> {
    singleton_read(storage, RANDOM_KEY)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Player {
    pub address: CanonicalAddr,
    pub matches: Vec<String>,
}

pub fn storage_player<S: Storage>(storage: &mut S) -> Bucket<S, Player> {
    bucket(PLAYER_KEY, storage)
}

pub fn storage_player_read<S: Storage>(storage: &S) -> ReadonlyBucket<S, Player> {
    bucket_read(PLAYER_KEY, storage)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[repr(u8)]
pub enum Shape {
    Triangle,
    Square,
    Circle,
    Diamond,
    Trapezoid,
    Oval,
    Pentagon,
    Hexagon,
    Octagon,
}

impl Distribution<Shape> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Shape {
        match rng.gen_range(0..9) {
            0 => Shape::Triangle,
            1 => Shape::Square,
            2 => Shape::Circle,
            3 => Shape::Diamond,
            4 => Shape::Trapezoid,
            5 => Shape::Oval,
            6 => Shape::Pentagon,
            7 => Shape::Hexagon,
            _ => Shape::Octagon,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[repr(u8)]
pub enum Color {
    Red,
    Blue,
    Yellow,
    Purple,
    Green,
    Orange,
    Brown,
    Gray,
    Black,
}

impl Distribution<Color> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Color {
        match rng.gen_range(0..9) {
            0 => Color::Red,
            1 => Color::Blue,
            2 => Color::Yellow,
            3 => Color::Purple,
            4 => Color::Green,
            5 => Color::Orange,
            6 => Color::Brown,
            7 => Color::Gray,
            _ => Color::Black,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Card {
    pub shape: Shape,
    pub color: Color,
    pub is_revealed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Match {
    pub player: CanonicalAddr,
    pub size: (u32, u32),
    pub cards: Vec<Vec<Card>>,
    pub last_reveal: Option<(u32, u32)>,
    pub attempts: u32,
}

impl Match {
    pub fn out_of_bounds(&self, row: usize, col: usize) -> StdResult<()> {
        let (rows, cols) = self.size;
        if row as u32 >= rows || col as u32 >= cols {
            Err(StdError::NotFound {
                kind: "Card".to_string(),
                backtrace: None,
            })
        } else {
            Ok(())
        }
    }

    pub fn card_at(&self, row: usize, col: usize) -> StdResult<Card> {
        self.out_of_bounds(row, col)?;
        let card = self.cards.get(row).unwrap().get(col).unwrap();
        Ok(card.clone())
    }

    pub fn does_match(
        &self,
        first_pos: (usize, usize),
        second_pos: (usize, usize),
    ) -> StdResult<bool> {
        let first_card = self.card_at(first_pos.0, first_pos.1)?;
        let second_card = self.card_at(second_pos.0, second_pos.1)?;
        Ok(first_card.shape == second_card.shape && first_card.color == second_card.color)
    }

    pub fn reveal(&mut self, row: usize, col: usize) -> StdResult<()> {
        self.out_of_bounds(row, col)?;
        let card = self
            .cards
            .get_mut(row as usize)
            .unwrap()
            .get_mut(col as usize)
            .unwrap();
        card.is_revealed = true;
        Ok(())
    }
}

pub fn storage_match<S: Storage>(storage: &mut S) -> Bucket<S, Match> {
    bucket(MATCH_KEY, storage)
}

pub fn storage_match_read<S: Storage>(storage: &S) -> ReadonlyBucket<S, Match> {
    bucket_read(MATCH_KEY, storage)
}

use core::iter;
use cosmwasm_std::{
    to_binary, Api, Binary, CanonicalAddr, Env, Extern, HandleResponse, InitResponse, Querier,
    StdError, StdResult, Storage,
};
use getrandom::register_custom_getrandom;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use uuid::Uuid;

use crate::msg::{CardResponse, HandleMsg, InitMsg, MatchResponse, PlayerResponse, QueryMsg};
use crate::state::{
    storage_match, storage_match_read, storage_player, storage_player_read, storage_random,
    storage_random_read, Card, Match, Player, Random,
};

fn fill_with_nothing(_dest: &mut [u8]) -> Result<(), getrandom::Error> {
    Ok(())
}

register_custom_getrandom!(fill_with_nothing);

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let mut random = Random::empty();
    random.input_entropy(msg.entropy, env.message.sender, env.block.height);
    storage_random(&mut deps.storage).save(&random)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::StartMatch {
            entropy,
            rows,
            cols,
        } => try_start_match(deps, env, entropy, rows, cols),
        HandleMsg::RevealCard {
            entropy,
            match_id,
            pos,
        } => try_reveal_card(deps, env, entropy, match_id, pos),
    }
}

pub fn try_start_match<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    entropy: u64,
    rows: u32,
    cols: u32,
) -> StdResult<HandleResponse> {
    let sender = deps.api.canonical_address(&env.message.sender)?;

    storage_random(&mut deps.storage).update(|mut random| {
        random.input_entropy(entropy, env.message.sender, env.block.height);
        Ok(random)
    })?;
    let random = storage_random_read(&deps.storage).load()?;
    let mut rng = ChaCha20Rng::from_seed(random.seed);

    let cards: Vec<Card> = iter::repeat_with(|| Card {
        shape: rng.gen(),
        color: rng.gen(),
        is_revealed: false,
    })
    .take((rows * cols / 2) as usize)
    .collect();
    let mut cards = [cards.as_slice(), cards.as_slice()].concat();
    cards.shuffle(&mut rng);
    let cards = cards
        .chunks(cols as usize)
        .map(|row| row.to_vec())
        .collect();

    let mut match_id_buf = [0u8; 16];
    rng.fill(&mut match_id_buf);
    let match_id = Uuid::from_slice(&match_id_buf)
        .unwrap()
        .simple()
        .to_string();

    let game_match = Match {
        player: sender.clone(),
        size: (rows, cols),
        cards,
        last_reveal: None,
        attempts: 0,
    };
    storage_match(&mut deps.storage).save(match_id.as_bytes(), &game_match)?;

    match storage_player_read(&mut deps.storage).may_load(sender.as_slice())? {
        Some(mut player) => {
            player.matches.push(match_id.clone());
            storage_player(&mut deps.storage).save(sender.as_slice(), &player)?;
        }
        None => {
            let player = Player {
                address: sender.clone(),
                matches: vec![match_id.clone()],
            };
            storage_player(&mut deps.storage).save(sender.as_slice(), &player)?;
        }
    }

    Ok(HandleResponse {
        data: Some(to_binary(&match_id)?),
        ..HandleResponse::default()
    })
}

pub fn try_reveal_card<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    entropy: u64,
    match_id: String,
    pos: (u32, u32),
) -> StdResult<HandleResponse> {
    let sender = deps.api.canonical_address(&env.message.sender)?;

    storage_random(&mut deps.storage).update(|mut random| {
        random.input_entropy(entropy, env.message.sender, env.block.height);
        Ok(random)
    })?;

    let mut game_match = storage_match_read(&deps.storage).load(match_id.as_bytes())?;
    if sender != game_match.player {
        return Err(StdError::Unauthorized { backtrace: None });
    }

    let card = game_match.card_at(pos.0 as usize, pos.1 as usize)?;
    if card.is_revealed {
        return Err(StdError::GenericErr {
            msg: "Card already revealed.".to_string(),
            backtrace: None,
        });
    }

    match game_match.last_reveal {
        Some(last_pos) => {
            let pos = (pos.0 as usize, pos.1 as usize);
            let last_pos = (last_pos.0 as usize, last_pos.1 as usize);
            if game_match.does_match(pos, last_pos)? {
                game_match.reveal(pos.0, pos.1)?;
                game_match.reveal(last_pos.0, last_pos.1)?;
            } else {
                game_match.attempts += 1;
            }
            game_match.last_reveal = None;
        }
        None => game_match.last_reveal = Some(pos),
    }
    storage_match(&mut deps.storage).save(match_id.as_bytes(), &game_match)?;

    let res = CardResponse {
        shape: card.shape,
        color: card.color,
        pos,
    };
    Ok(HandleResponse {
        data: Some(to_binary(&res)?),
        ..HandleResponse::default()
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetPlayer { address } => to_binary(&query_player(deps, address)?),
        QueryMsg::GetCard { match_id, row, col } => {
            to_binary(&query_card(deps, match_id, row, col)?)
        }
        QueryMsg::GetMatch { match_id } => to_binary(&query_match(deps, match_id)?),
    }
}

fn query_player<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: CanonicalAddr,
) -> StdResult<PlayerResponse> {
    let player = storage_player_read(&deps.storage).load(address.as_slice())?;
    Ok(PlayerResponse {
        matches: player.matches,
    })
}

fn query_card<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    match_id: String,
    row: u32,
    col: u32,
) -> StdResult<CardResponse> {
    let game_match = storage_match_read(&deps.storage).load(match_id.as_bytes())?;
    let card = game_match.card_at(row as usize, col as usize)?;
    if !card.is_revealed {
        return Err(StdError::Unauthorized { backtrace: None });
    }
    Ok(CardResponse {
        shape: card.shape,
        color: card.color,
        pos: (row, col),
    })
}

fn query_match<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    match_id: String,
) -> StdResult<MatchResponse> {
    let game_match = storage_match_read(&deps.storage).load(match_id.as_bytes())?;
    let cards = game_match
        .cards
        .iter()
        .enumerate()
        .map(|(row, card_row)| {
            card_row
                .iter()
                .enumerate()
                .map(|(col, card)| {
                    if card.is_revealed {
                        Some(CardResponse {
                            shape: card.shape.clone(),
                            color: card.color.clone(),
                            pos: (row as u32, col as u32),
                        })
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    Ok(MatchResponse {
        size: game_match.size,
        attempts: game_match.attempts,
        cards,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Color, Shape};
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{from_binary, HumanAddr, StdError};

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let mut rng = thread_rng();
        let msg = InitMsg { entropy: rng.gen() };
        let env = mock_env("creator", &[]);

        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(res.messages.len(), 0);
    }

    #[test]
    fn start_match() {
        let mut deps = mock_dependencies(20, &[]);

        let mut rng = thread_rng();
        let msg = InitMsg { entropy: rng.gen() };
        let env = mock_env("creator", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::StartMatch {
            entropy: rng.gen(),
            rows: 4,
            cols: 4,
        };
        let res = handle(&mut deps, env, msg).unwrap();
        let match_id: String = from_binary(&res.data.unwrap()).unwrap();

        let msg = QueryMsg::GetPlayer { address: deps.api.canonical_address(&HumanAddr("player".into())).unwrap() };
        let res = query(&deps, msg).unwrap();
        let player: PlayerResponse = from_binary(&res).unwrap();
        assert_eq!(player.matches, vec![match_id.clone()]);

        let msg = QueryMsg::GetMatch { match_id };
        let res = query(&deps, msg).unwrap();
        let game_match: MatchResponse = from_binary(&res).unwrap();
        assert_eq!(game_match.size, (4, 4));
        assert_eq!(game_match.attempts, 0);
        assert_eq!(game_match.cards, iter::repeat(iter::repeat(None).take(4).collect()).take(4).collect::<Vec<Vec<Option<CardResponse>>>>());
    }

    #[test]
    fn reveal_card() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg { entropy: 100 };
        let env = mock_env("creator", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::StartMatch {
            entropy: 200,
            rows: 4,
            cols: 4,
        };
        let res = handle(&mut deps, env, msg).unwrap();
        let match_id: String = from_binary(&res.data.unwrap()).unwrap();

        let unauth_env = mock_env("wrong_player", &[]);
        let msg = HandleMsg::RevealCard {
            entropy: 300,
            match_id,
            pos: (2, 2),
        };
        let res = handle(&mut deps, unauth_env, msg.clone());
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        let auth_env = mock_env("player", &[]);
        let res = handle(&mut deps, auth_env, msg).unwrap();
        let card: CardResponse = from_binary(&res.data.unwrap()).unwrap();
        assert_eq!(card.shape, Shape::Pentagon);
        assert_eq!(card.color, Color::Red);
        assert_eq!(card.pos, (2, 2));
    }

    #[test]
    fn reveal_card_miss() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg { entropy: 100 };
        let env = mock_env("creator", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::StartMatch {
            entropy: 200,
            rows: 4,
            cols: 4,
        };
        let res = handle(&mut deps, env, msg).unwrap();
        let match_id: String = from_binary(&res.data.unwrap()).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::RevealCard {
            entropy: 300,
            match_id: match_id.clone(),
            pos: (2, 2),
        };
        let res = handle(&mut deps, env, msg.clone()).unwrap();
        let card: CardResponse = from_binary(&res.data.unwrap()).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::RevealCard {
            entropy: 400,
            match_id: match_id.clone(),
            pos: (1, 1),
        };
        let res = handle(&mut deps, env, msg.clone()).unwrap();
        let card2: CardResponse = from_binary(&res.data.unwrap()).unwrap();

        assert!(!(card.shape == card2.shape && card.color == card2.color), "Cards must not match.");

        let msg = QueryMsg::GetCard { match_id: match_id.clone(), row: 2, col: 2 };
        let res = query(&deps, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        let msg = QueryMsg::GetCard { match_id: match_id.clone(), row: 1, col: 1 };
        let res = query(&deps, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        let msg = QueryMsg::GetMatch { match_id };
        let res = query(&deps, msg).unwrap();
        let game_match: MatchResponse = from_binary(&res).unwrap();
        assert_eq!(game_match.attempts, 1);
    }

    #[test]
    fn reveal_card_hit() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg { entropy: 100 };
        let env = mock_env("creator", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::StartMatch {
            entropy: 200,
            rows: 4,
            cols: 4,
        };
        let res = handle(&mut deps, env, msg).unwrap();
        let match_id: String = from_binary(&res.data.unwrap()).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::RevealCard {
            entropy: 300,
            match_id: match_id.clone(),
            pos: (2, 2),
        };
        let res = handle(&mut deps, env, msg.clone()).unwrap();
        let card: CardResponse = from_binary(&res.data.unwrap()).unwrap();

        let env = mock_env("player", &[]);
        let msg = HandleMsg::RevealCard {
            entropy: 400,
            match_id: match_id.clone(),
            pos: (3, 0),
        };
        let res = handle(&mut deps, env, msg.clone()).unwrap();
        let card2: CardResponse = from_binary(&res.data.unwrap()).unwrap();

        assert!(card.shape == card2.shape && card.color == card2.color, "Cards must match.");

        let msg = QueryMsg::GetCard { match_id: match_id.clone(), row: 2, col: 2 };
        let res = query(&deps, msg).unwrap();
        let card: CardResponse = from_binary(&res).unwrap();
        assert_eq!(card.shape, Shape::Pentagon);
        assert_eq!(card.color, Color::Red);
        assert_eq!(card.pos, (2, 2));

        let msg = QueryMsg::GetCard { match_id: match_id.clone(), row: 3, col: 0 };
        let res = query(&deps, msg).unwrap();
        let card: CardResponse = from_binary(&res).unwrap();
        assert_eq!(card.shape, Shape::Pentagon);
        assert_eq!(card.color, Color::Red);
        assert_eq!(card.pos, (3, 0));

        let msg = QueryMsg::GetMatch { match_id };
        let res = query(&deps, msg).unwrap();
        let game_match: MatchResponse = from_binary(&res).unwrap();
        assert_eq!(game_match.attempts, 0);
    }
}

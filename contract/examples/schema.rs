use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use memory_match_contract::msg::{CardResponse, HandleMsg, InitMsg, MatchResponse, PlayerResponse, QueryMsg};
use memory_match_contract::state::{Card, Color, Match, Player, Random, Shape};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InitMsg), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(PlayerResponse), &out_dir);
    export_schema(&schema_for!(CardResponse), &out_dir);
    export_schema(&schema_for!(MatchResponse), &out_dir);
    export_schema(&schema_for!(Random), &out_dir);
    export_schema(&schema_for!(Player), &out_dir);
    export_schema(&schema_for!(Shape), &out_dir);
    export_schema(&schema_for!(Color), &out_dir);
    export_schema(&schema_for!(Card), &out_dir);
    export_schema(&schema_for!(Match), &out_dir);
}

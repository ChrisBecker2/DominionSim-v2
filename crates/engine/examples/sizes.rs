use dominion_engine::state::{Frame, GameState, PlayerState, TurnState};
fn main() {
    println!("GameState {} PlayerState {} TurnState {} Frame {}", std::mem::size_of::<GameState>(), std::mem::size_of::<PlayerState>(), std::mem::size_of::<TurnState>(), std::mem::size_of::<Frame>());
}

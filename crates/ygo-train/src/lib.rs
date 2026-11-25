//! Training the neural bot (M6, `docs/m6-plan.md`): learning from game
//! records (`imitation`: the moves a player made and who won) and by
//! self-play (`selfplay`: the network-guided search's visit shares and
//! who won), both trained with `imitation::train_on`.

pub mod imitation;
pub mod selfplay;

//! Training the neural bot (M6, `docs/m6-plan.md`): imitation of the
//! heuristic and search bots from game records (`imitation`), and
//! distillation of the network-guided search into the network on the
//! network's own games (`distill`).

pub mod distill;
pub mod imitation;

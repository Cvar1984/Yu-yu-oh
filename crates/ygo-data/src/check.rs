use crate::{Banlist, CardDb, CardId, Collection, Deck, Rules};

/// One reason a deck is not legal. A legal deck has none.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeckProblem {
    #[error("unknown card id {0}")]
    UnknownCard(CardId),
    #[error("Main Deck has {size} cards; rules allow {min} to {max}")]
    MainSize { size: usize, min: usize, max: usize },
    #[error("Extra Deck has {size} cards; rules allow at most {max}")]
    ExtraSize { size: usize, max: usize },
    #[error("{name} ({id}) belongs in the Extra Deck, not the Main Deck")]
    ExtraCardInMain { id: CardId, name: String },
    #[error("{name} ({id}) belongs in the Main Deck, not the Extra Deck")]
    MainCardInExtra { id: CardId, name: String },
    #[error("{copies} copies of {name} ({id}); limit is {limit}")]
    OverLimit {
        id: CardId,
        name: String,
        copies: usize,
        limit: usize,
    },
    #[error("{copies} copies of {name} ({id}) but the collection has {owned}")]
    NotOwned {
        id: CardId,
        name: String,
        copies: usize,
        owned: usize,
    },
}

/// Checks a deck against the card pool, ban list, deck rules and, when given,
/// a collection. Returns every problem found, in a stable order.
pub fn check_deck(
    deck: &Deck,
    cards: &CardDb,
    banlist: &Banlist,
    rules: &Rules,
    collection: Option<&Collection>,
) -> Vec<DeckProblem> {
    let mut problems = Vec::new();

    let main = deck.main_size();
    if main < rules.deck.main_min || main > rules.deck.main_max {
        problems.push(DeckProblem::MainSize {
            size: main,
            min: rules.deck.main_min,
            max: rules.deck.main_max,
        });
    }
    if deck.extra_size() > rules.deck.extra_max {
        problems.push(DeckProblem::ExtraSize {
            size: deck.extra_size(),
            max: rules.deck.extra_max,
        });
    }

    for (id, copies) in deck.copies() {
        let Some(card) = cards.get(id) else {
            problems.push(DeckProblem::UnknownCard(id));
            continue;
        };
        let in_main = deck.in_main(id);
        let in_extra = deck.in_extra(id);
        if card.card_type.is_extra_deck() && in_main {
            problems.push(DeckProblem::ExtraCardInMain {
                id,
                name: card.name.clone(),
            });
        }
        if !card.card_type.is_extra_deck() && in_extra {
            problems.push(DeckProblem::MainCardInExtra {
                id,
                name: card.name.clone(),
            });
        }
        let limit = banlist.limit_for(id, rules.deck.default_copy_limit);
        if copies > limit {
            problems.push(DeckProblem::OverLimit {
                id,
                name: card.name.clone(),
                copies,
                limit,
            });
        }
        if let Some(collection) = collection {
            let owned = collection.owned(id);
            if copies > owned {
                problems.push(DeckProblem::NotOwned {
                    id,
                    name: card.name.clone(),
                    copies,
                    owned,
                });
            }
        }
    }

    problems
}
